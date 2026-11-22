//! Tests for [`crate::compute::particles`], `#[path]`-included into it.
//!
//! The GPU path is held to the CPU path it mirrors: the same scene stepped both ways must agree to
//! the `f32` rounding over a run where nothing chaotic has had time to grow, a contact must rebound
//! at its material's restitution, and the whole giant impact must come out the same (the planets
//! meet on time and merge, with momentum conserved and energy dissipated).
//!
//! # Machines without a GPU
//!
//! Every test that needs a GPU asks for one and, finding none, prints that it skipped and passes,
//! so the suite runs anywhere. Set `RGA_REQUIRE_GPU` to make a missing GPU a failure instead; CI
//! installs Mesa's software Vulkan driver (`lavapipe`) so the GPU path is tested there too.

use super::*;
use crate::collision::aggregate::{contact_clusters, impact_outcome, ImpactOutcome};
use crate::collision::collision::particle_energy;
use crate::core::scene::{Aabb, CollisionSettings, Color, Material, Pose, Prop, Quat, Vec3};
use crate::projects::interstellar::giant_impact::{
    giant_impact, giant_impact_seconds, giant_impact_speed_m_s, giant_impact_with,
};

/// Set to anything to make a missing GPU fail the tests instead of skipping them.
const REQUIRE_GPU_ENV: &str = "RGA_REQUIRE_GPU";

/// A GPU stepper, or `None` (after saying so) when the machine has no GPU.
fn gpu() -> Option<GpuParticles> {
    let found = GpuParticles::request();
    if found.is_none() {
        assert!(
            std::env::var_os(REQUIRE_GPU_ENV).is_none(),
            "{REQUIRE_GPU_ENV} is set and no GPU was found"
        );
        eprintln!("skipped: no GPU on this machine");
    }
    found
}

fn settings(particles: u32) -> CollisionSettings {
    CollisionSettings {
        particles_per_body: particles,
        ..CollisionSettings::default()
    }
}

fn seconds(value: f64) -> TimeStep {
    TimeStep::from_seconds(value).unwrap()
}

#[test]
fn the_shader_s_workgroup_is_the_constant() {
    assert!(PARTICLES_SHADER.contains(&format!("const WORKGROUP: u32 = {GPU_WORKGROUP_SIZE}u;")));
}

#[test]
fn only_scenes_made_entirely_of_particles_go_to_the_gpu() {
    let mut scene = giant_impact().scene;
    assert!(can_step_on_gpu(&scene));

    let mut with_a_rigid_body = scene.clone();
    with_a_rigid_body.props[0].material = Material::ELASTIC;
    assert!(!can_step_on_gpu(&with_a_rigid_body));

    let mut with_an_absorber = scene.clone();
    with_an_absorber.props[0].absorbs = true;
    assert!(!can_step_on_gpu(&with_an_absorber));

    scene.gravity_model = GravityModel::Schwarzschild;
    assert!(!can_step_on_gpu(&scene));
}

#[test]
fn a_scene_the_gpu_cannot_step_is_left_alone() {
    let Some(mut gpu) = gpu() else { return };
    let mut scene = giant_impact().scene;
    scene.props[0].material = Material::ELASTIC;
    let before = scene.clone();
    assert_eq!(gpu.step(&mut scene, seconds(60.0)), None);
    assert_eq!(scene, before);
}

#[test]
fn the_gpu_and_the_cpu_agree_to_the_rounding_over_an_hour() {
    // Two planets of particles settling under their own weight for an hour, both ways, at the
    // same substeps. Measured on an RTX 4070 SUPER: the furthest any particle has drifted from its
    // CPU twin is 16 m, on particles 1,500 km across.
    let Some(mut gpu) = gpu() else { return };
    gpu.solver = GpuSolver::EXACT;
    let mut cpu_scene = giant_impact_with(&settings(50)).scene;
    let mut gpu_scene = cpu_scene.clone();
    let step = seconds(60.0);
    for _ in 0..60 {
        cpu_scene.step(step);
        assert!(gpu.step(&mut gpu_scene, step).is_some());
    }
    let worst = cpu_scene
        .props
        .iter()
        .zip(&gpu_scene.props)
        .map(|(a, b)| (a.body.center() - b.body.center()).length() * METRES_PER_UNIT)
        .fold(0.0, f64::max);
    assert!(
        worst < 500.0,
        "a particle drifted {worst:.1} m from the CPU's"
    );
    for (a, b) in cpu_scene.props.iter().zip(&gpu_scene.props) {
        let gap = (a.velocity_units_s - b.velocity_units_s).length() * METRES_PER_UNIT;
        assert!(gap < 1.0, "velocities {gap:.3} m/s apart");
    }
}

#[test]
fn a_contact_on_the_gpu_rebounds_at_its_restitution() {
    // The CPU test of the same name, on the GPU: two 1 m, 1 t particles closing at 2 m/s.
    let Some(mut gpu) = gpu() else { return };
    let particle = |x: f64, vx: f64| {
        Prop::new(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::new(x / METRES_PER_UNIT, 0.0, 0.0),
                    rotation: Quat::IDENTITY,
                },
                radius: (1.0 / METRES_PER_UNIT) as f32,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            },
            DVec3::new(vx / METRES_PER_UNIT, 0.0, 0.0),
            1000.0,
        )
        .with_material(Material::new(0.5, 0.0).with_modulus(1.0e9))
    };
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-1.0),
        max: Vec3::splat(1.0),
    });
    scene.add(particle(0.0, 1.0));
    scene.add(particle(2.0, -1.0));
    for _ in 0..200 {
        assert!(gpu.step(&mut scene, seconds(1.0e-3)).is_some());
    }
    let parting =
        (scene.props[1].velocity_units_s.x - scene.props[0].velocity_units_s.x) * METRES_PER_UNIT;
    assert!(
        (parting / 2.0 - 0.5).abs() < 0.03,
        "parted at {:.3} of the approach speed",
        parting / 2.0
    );
}

/// The whole giant impact on the GPU with `solver`: when the planets meet, where the mass goes, the
/// largest energy drift as a fraction of the binding energy, and the momentum left over a fraction
/// of the impact's.
fn giant_impact_on_gpu(
    gpu: &mut GpuParticles,
    solver: GpuSolver,
) -> (f64, ImpactOutcome, f64, f64) {
    gpu.solver = solver;
    let mut scene = giant_impact().scene;
    let softening = scene.softening_units;
    let start = particle_energy(&scene.props, softening);
    let mass: f64 = scene.props.iter().map(|p| f64::from(p.mass_kg)).sum();
    let momentum = |scene: &Scene| -> DVec3 {
        scene
            .props
            .iter()
            .map(|p| p.velocity_units_s * METRES_PER_UNIT * f64::from(p.mass_kg))
            .sum()
    };
    let step = seconds(60.0);
    let mut elapsed = 0.0;
    let mut met = None;
    let mut drift = 0.0_f64;
    while elapsed < 30.0 * 3600.0 {
        assert!(gpu.step(&mut scene, step).is_some());
        elapsed += 60.0;
        if met.is_none() && contact_clusters(&scene.props).len() < 2 {
            met = Some(elapsed);
        }
        if ((elapsed / 60.0) as u64).is_multiple_of(30) {
            let now = particle_energy(&scene.props, softening).total();
            drift = drift.max((now - start.total()).abs());
        }
    }
    (
        met.expect("the planets met"),
        impact_outcome(&scene.props),
        drift / start.gravity.abs(),
        momentum(&scene).length() / (mass * giant_impact_speed_m_s()),
    )
}

#[test]
fn the_giant_impact_on_the_gpu_comes_out_as_on_the_cpu_with_every_algorithm() {
    // Measured on an RTX 4070 SUPER; see `src/compute/README.md` for the table.
    let Some(mut gpu) = gpu() else { return };
    let law = giant_impact_seconds();
    for gravity in [GravitySolver::Direct, GravitySolver::BarnesHut] {
        for neighbours in [NeighbourSearch::AllPairs, NeighbourSearch::Tree] {
            let solver = GpuSolver {
                gravity,
                neighbours,
                ..GpuSolver::DEFAULT
            };
            let (met, outcome, drift, momentum) = giant_impact_on_gpu(&mut gpu, solver);
            eprintln!(
                "MEASURE {gravity:?} {neighbours:?}: met {:.2} h, {outcome:?}, drift {drift:.2e}, momentum {momentum:.2e}",
                met / 3600.0
            );
            assert!(
                (met - law).abs() < 0.01 * law,
                "{solver:?}: met after {met} s"
            );
            // A perfect merge, as on the CPU: nothing escapes and what is lofted comes back.
            assert_eq!(outcome.escaping, 0.0, "{solver:?}: {outcome:?}");
            assert!(outcome.largest_remnant() > 0.999, "{solver:?}: {outcome:?}");
            assert!(outcome.largest > 0.9, "{solver:?}: {outcome:?}");
            // Energy within a percent of the binding energy, in `f32` and through the tree.
            assert!(drift < 0.01, "{solver:?}: energy drifted {drift:.3e}");
            // Every force is computed twice, once from each side, in `f32`; Barnes–Hut's are not
            // even equal and opposite, since each particle sees the others grouped differently.
            // Measured: `3e-7` of the impact's momentum with direct gravity, `3e-3` with the tree.
            let tolerance = match gravity {
                GravitySolver::Direct => 1.0e-5,
                GravitySolver::BarnesHut => 1.0e-2,
            };
            assert!(momentum < tolerance, "{solver:?}: momentum {momentum:.3e}");
        }
    }
}

/// Time one frame of `experiment_1` (`33` simulated seconds, the `2000×` clock at `60` frames a
/// second) on the GPU with every algorithm, at increasing detail. Run with
/// `cargo test --release -- --ignored measure_gpu_speed --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_gpu_speed() {
    let Some(mut gpu) = gpu() else { return };
    let step = seconds(2000.0 / 60.0);
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![200, 2000, 20_000]);
    for n in sizes {
        let built = std::time::Instant::now();
        let scene = giant_impact_with(&settings(n)).scene;
        let build_ms = built.elapsed().as_secs_f64() * 1e3;
        for gravity in [GravitySolver::Direct, GravitySolver::BarnesHut] {
            for neighbours in [NeighbourSearch::AllPairs, NeighbourSearch::Tree] {
                let only = std::env::var("RGA_MEASURE_ONLY").ok();
                if only
                    .as_deref()
                    .is_some_and(|o| o != format!("{gravity:?}+{neighbours:?}"))
                {
                    continue;
                }
                if gravity == GravitySolver::Direct && scene.props.len() > 150_000 {
                    continue;
                }
                let theta: f32 = std::env::var("RGA_MEASURE_THETA")
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(0.5);
                gpu.solver = GpuSolver {
                    gravity,
                    neighbours,
                    opening_angle: theta,
                };
                let mut s = scene.clone();
                gpu.step(&mut s, step);
                let t = std::time::Instant::now();
                let mut substeps = 0;
                for _ in 0..3 {
                    substeps = gpu.step(&mut s, step).unwrap();
                }
                let ms = t.elapsed().as_secs_f64() * 1e3 / 3.0;
                eprintln!(
                    "MEASURE n={n} bodies {} build {build_ms:.0} ms substeps {substeps} {gravity:?}+{neighbours:?}: {ms:.2} ms/frame",
                    scene.props.len()
                );
            }
        }
    }
}

#[test]
fn the_moon_forming_impact_on_the_gpu_keeps_its_angular_momentum_with_either_solver() {
    use crate::projects::interstellar::moon_forming::{
        moon_forming_angular_momentum, moon_forming_impact,
    };
    let Some(mut gpu) = gpu() else { return };
    let angular_momentum = |props: &[crate::core::scene::Prop]| -> f64 {
        props
            .iter()
            .map(|p| {
                let m = f64::from(p.mass_kg);
                let r = p.body.center() * METRES_PER_UNIT;
                let v = p.velocity_units_s * METRES_PER_UNIT;
                let radius = crate::collision::collision::physical_radius_m(p).unwrap_or(0.0);
                let inertia = crate::physics_utils::thermal::sphere_moment_of_inertia(m, radius);
                (r.cross(v) * m + p.spin_rad_s * inertia).z
            })
            .sum()
    };
    for solver in [GpuSolver::EXACT, GpuSolver::SCALABLE] {
        gpu.solver = solver;
        let mut scene = moon_forming_impact().scene;
        let start = angular_momentum(&scene.props);
        for _ in 0..(12 * 60) {
            assert!(gpu.step(&mut scene, seconds(60.0)).is_some());
        }
        let end = angular_momentum(&scene.props);
        let outcome = impact_outcome(&scene.props);
        eprintln!("MEASURE moon-forming {solver:?}: L {start:.4e} → {end:.4e}, {outcome:?}");
        // The exact sums keep it to `f32` rounding (measured: four digits). Barnes–Hut's forces are
        // not equal and opposite: a particle sees a distant group as a point mass, the group sees it
        // through other groupings. Measured at `θ = 0.5`: `0.33%` gained over `12` hours.
        let tolerance = match solver.gravity {
            GravitySolver::Direct => 1.0e-3,
            GravitySolver::BarnesHut => 1.0e-2,
        };
        assert!(
            (end - start).abs() < tolerance * moon_forming_angular_momentum(),
            "{solver:?}: {start:.4e} → {end:.4e}"
        );
        assert!(outcome.largest_remnant() > 0.9, "{solver:?}: {outcome:?}");
    }
}

/// The Moon-forming impact on the GPU at higher detail, for a simulated day: where the mass goes.
/// `RGA_MEASURE_SIZES` sets the particle counts. Run with
/// `cargo test --release -- --ignored measure_moon_forming_detail --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_moon_forming_detail() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    let Some(mut gpu) = gpu() else { return };
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![200, 2000]);
    for n in sizes {
        gpu.solver = GpuSolver::EXACT;
        let mut scene = moon_forming_impact_with(&settings(n)).scene;
        let t = std::time::Instant::now();
        for hour in 1..=24 {
            for _ in 0..60 {
                assert!(gpu.step(&mut scene, seconds(60.0)).is_some());
            }
            if hour % 6 == 0 {
                let o = impact_outcome(&scene.props);
                eprintln!(
                    "MEASURE n={n} bodies {} h{hour} ({:.0} s): largest {:.4} disk {:.4} falling {:.4} escaping {:.4}",
                    scene.props.len(), t.elapsed().as_secs_f64(), o.largest, o.disk, o.falling_back, o.escaping
                );
            }
        }
    }
}

/// The head-on impact at twice the escape speed, where Leinhardt & Stewart's law says some mass is
/// lost, on the GPU at increasing detail. `RGA_MEASURE_SIZES` sets the counts. Run with
/// `cargo test --release -- --ignored measure_debris_above_escape --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_debris_above_escape() {
    use crate::physics_utils::impacts::{
        catastrophic_disruption_threshold, largest_remnant_fraction, mutual_escape_speed,
        specific_impact_energy,
    };
    use crate::projects::interstellar::giant_impact::{giant_impact_impactor, giant_impact_target};
    let Some(mut gpu) = gpu() else { return };
    let (target, impactor) = (giant_impact_target(), giant_impact_impactor());
    let escape = mutual_escape_speed(
        target.mass_kg,
        impactor.mass_kg,
        target.radius_m,
        impactor.radius_m,
    );
    let factor = std::env::var("RGA_MEASURE_FACTOR")
        .ok()
        .and_then(|f| f.parse().ok())
        .unwrap_or(2.0);
    let q = specific_impact_energy(target.mass_kg, impactor.mass_kg, factor * escape);
    let law = largest_remnant_fraction(
        q,
        catastrophic_disruption_threshold(target.mass_kg, impactor.mass_kg),
    );
    eprintln!("MEASURE {factor}x escape: Leinhardt & Stewart's largest remnant {law:.4}");
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![200, 2000]);
    for n in sizes {
        gpu.solver = GpuSolver::EXACT;
        let mut scene = crate::projects::interstellar::giant_impact::giant_impact_at_speed(
            &settings(n),
            factor * escape,
        )
        .scene;
        for hour in 1..=24 {
            for _ in 0..60 {
                assert!(gpu.step(&mut scene, seconds(60.0)).is_some());
            }
            if hour % 8 == 0 {
                let o = impact_outcome(&scene.props);
                eprintln!(
                    "MEASURE n={n} h{hour}: largest remnant {:.4} (body {:.4}, falling {:.4}, disk {:.4}) escaping {:.4}",
                    o.largest_remnant(), o.largest, o.falling_back, o.disk, o.escaping
                );
            }
        }
    }
}
