//! Tests for [`crate::compute::particles`], `#[path]`-included into it.
//!
//! The GPU path is held to the CPU path it mirrors: the same scene stepped both ways must agree to
//! the `f32` rounding over a run where nothing chaotic has had time to grow, a contact must rebound
//! at its material's restitution, and the whole giant impact must come out the same (the planets
//! meet on time and merge, with momentum conserved and energy dissipated).
//!
//! # Machines without a GPU
//!
//! Not supported: particle simulations are GPU-only, so these tests fail without a GPU. They are
//! compiled only with the `gpu-tests` feature (the default); see `tst/README.md`.

use super::*;
use crate::collision::aggregate::{contact_clusters, impact_outcome, ImpactOutcome};
use crate::collision::collision::particle_energy;
use crate::core::scene::{Aabb, CollisionSettings, Color, Material, Pose, Prop, Quat, Vec3};
use crate::projects::interstellar::giant_impact::{
    giant_impact, giant_impact_seconds, giant_impact_speed_m_s, giant_impact_with,
};

/// A GPU stepper. Particle simulations are GPU-only (see `src/compute/README.md`), so a machine
/// without a GPU fails these tests rather than skipping them. Returns `Option` so each test keeps its
/// `let Some(..) else` shape; it is always `Some`.
fn gpu() -> Option<GpuParticles> {
    Some(
        GpuParticles::request()
            .expect("no GPU found: particle simulations need one (lavapipe will do)"),
    )
}

/// The rubble planets at `particles`: what the tests below were measured with, before fluid planets.
fn settings(particles: u32) -> CollisionSettings {
    CollisionSettings {
        particles_per_body: particles,
        planet_model: crate::core::scene::PlanetModel::Rubble,
        ..CollisionSettings::default()
    }
}

fn seconds(value: f64) -> TimeStep {
    TimeStep::from_seconds(value).unwrap()
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
fn the_gpu_and_the_cpu_agree_over_an_hour() {
    // Two planets of particles settling under their own weight for an hour, both ways. Not to the
    // rounding any more: the GPU integrates with individual time steps and a kick-drift-kick
    // leapfrog, the frozen CPU with one step and symplectic Euler, whose velocities sit half a
    // step's kick (`a·dt/2`) from the leapfrog's. Measured on an RTX 4070 SUPER: `740 m` at worst on
    // particles 1,500 km across, velocities within `0.051 m/s` (before the leapfrog, with the same
    // integrator both sides: `16 m`). The bounds catch a physics mismatch, not the integrators'.
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
    let worst_velocity = cpu_scene
        .props
        .iter()
        .zip(&gpu_scene.props)
        .map(|(a, b)| (a.velocity_units_s - b.velocity_units_s).length() * METRES_PER_UNIT)
        .fold(0.0, f64::max);
    eprintln!("MEASURE rubble hour: worst position {worst:.1} m, velocity {worst_velocity:.3} m/s");
    assert!(
        worst < 2000.0,
        "a particle drifted {worst:.1} m from the CPU's"
    );
    assert!(
        worst_velocity < 0.5,
        "velocities {worst_velocity:.3} m/s apart"
    );
}

#[test]
fn fluid_planets_on_the_gpu_agree_with_the_cpu_with_either_neighbour_search() {
    // Two fluid planets (iron cores, basalt mantles) settling for ten minutes, both ways, at the same
    // steps: the GPU's density, pressure, viscosity and energy equation against the CPU's. The GPU
    // integrates with a kick-drift-kick leapfrog, the frozen CPU with symplectic Euler, and a fluid
    // planet settling is far from equilibrium, so the two part by `a·dt/2·t`: `1.6 km` after the
    // first minute, levelling off near `6–7 km` (measured on an RTX 4070 SUPER: `7.5 km` at worst on
    // particles over a thousand kilometres across, densities within `2e-3`, heat within
    // `1.9e4 J/kg`). With one integrator both sides it was `55 m`. The bounds catch a physics
    // mismatch (a wrong pressure or energy term puts the density off by tens of percent), not the
    // integrators' difference.
    use crate::core::scene::PlanetModel;
    let Some(mut gpu) = gpu() else { return };
    let fluid = CollisionSettings {
        particles_per_body: 200,
        planet_model: PlanetModel::Fluid,
        ..CollisionSettings::default()
    };
    for neighbours in NeighbourSearch::ALL {
        gpu.solver = GpuSolver {
            neighbours,
            ..GpuSolver::EXACT
        };
        let mut cpu_scene = giant_impact_with(&fluid).scene;
        let mut gpu_scene = cpu_scene.clone();
        let step = seconds(60.0);
        for _ in 0..10 {
            let substeps = crate::collision::resolve::substep_count(&cpu_scene.props, step);
            cpu_scene.step(step);
            assert!(gpu.step_in(&mut gpu_scene, step, substeps));
        }
        let worst = cpu_scene
            .props
            .iter()
            .zip(&gpu_scene.props)
            .map(|(a, b)| (a.body.center() - b.body.center()).length() * METRES_PER_UNIT)
            .fold(0.0, f64::max);
        let worst_density = cpu_scene
            .props
            .iter()
            .zip(&gpu_scene.props)
            .map(|(a, b)| (a.density_kg_m3 / b.density_kg_m3 - 1.0).abs())
            .fold(0.0, f64::max);
        let worst_heat = cpu_scene
            .props
            .iter()
            .zip(&gpu_scene.props)
            .map(|(a, b)| (a.heat_j_kg - b.heat_j_kg).abs())
            .fold(0.0, f64::max);
        eprintln!(
            "MEASURE fluid {neighbours:?}: worst position {worst:.1} m, density {worst_density:.2e}, heat {worst_heat:.3e} J/kg"
        );
        assert!(
            worst < 20_000.0,
            "{neighbours:?}: a particle drifted {worst:.1} m from the CPU's"
        );
        assert!(
            worst_density < 1.0e-2,
            "{neighbours:?}: densities {worst_density:.2e} apart"
        );
        assert!(
            worst_heat < 1.0e5,
            "{neighbours:?}: heat {worst_heat:.3e} J/kg apart"
        );
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
    let mut scene = giant_impact_with(&settings(
        crate::core::constants::DEFAULT_PARTICLES_PER_BODY,
    ))
    .scene;
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
        for neighbours in NeighbourSearch::ALL {
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
            for neighbours in NeighbourSearch::ALL {
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
        moon_forming_angular_momentum, moon_forming_impact_with,
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
        let mut scene = moon_forming_impact_with(&settings(
            crate::core::constants::DEFAULT_PARTICLES_PER_BODY,
        ))
        .scene;
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
    let model = match std::env::var("RGA_MEASURE_MODEL").as_deref() {
        Ok("rubble") => crate::core::scene::PlanetModel::Rubble,
        _ => crate::core::scene::PlanetModel::Fluid,
    };
    for n in sizes {
        gpu.solver = if n > 50_000 {
            GpuSolver::SCALABLE
        } else {
            GpuSolver::EXACT
        };
        let mut scene = moon_forming_impact_with(&CollisionSettings {
            planet_model: model,
            ..settings(n)
        })
        .scene;
        // The impactor's elongation towards the Earth as they close, until contact.
        let split = 0.5
            * crate::projects::interstellar::constants::MOON_FORMING_START_CONTACTS
            * crate::projects::interstellar::giant_impact::giant_impact_contact_m()
            / METRES_PER_UNIT;
        let impactor: Vec<usize> = (0..scene.props.len())
            .filter(|&i| scene.props[i].body.center().x > split)
            .collect();
        let target: Vec<usize> = (0..scene.props.len())
            .filter(|&i| scene.props[i].body.center().x <= split)
            .collect();
        let centre = |scene: &Scene, members: &[usize]| {
            members
                .iter()
                .map(|&i| scene.props[i].body.center())
                .sum::<DVec3>()
                / members.len() as f64
        };
        let elongation = |scene: &Scene| {
            let (own, other) = (centre(scene, &impactor), centre(scene, &target));
            let axis = (other - own).normalize();
            let (mut along, mut across) = (0.0, 0.0);
            for &i in &impactor {
                let offset = scene.props[i].body.center() - own;
                let parallel = offset.dot(axis);
                along += parallel * parallel;
                across += 0.5 * (offset.length_squared() - parallel * parallel);
            }
            (
                (along / across).sqrt(),
                (other - own).length() * METRES_PER_UNIT,
            )
        };
        let contact = crate::projects::interstellar::giant_impact::giant_impact_contact_m();
        let (start_shape, _) = elongation(&scene);
        let t = std::time::Instant::now();
        loop {
            assert!(gpu.step(&mut scene, seconds(30.0)).is_some());
            let (shape, distance) = elongation(&scene);
            if distance < 1.1 * contact {
                eprintln!(
                    "MEASURE n={n} {model:?}: impactor elongation {start_shape:.3} at the start, {shape:.3} at {:.2} contact distances ({:.0} s)",
                    distance / contact,
                    t.elapsed().as_secs_f64()
                );
                break;
            }
        }
        for hour in 1..=24 {
            for _ in 0..60 {
                assert!(gpu.step(&mut scene, seconds(60.0)).is_some());
            }
            if hour % 6 == 0 {
                let o = impact_outcome(&scene.props);
                eprintln!(
                    "MEASURE n={n} bodies {} h{hour} ({:.0} s): largest {:.4} disk {:.4} falling {:.4} escaping {:.4} body radius {:.0} km",
                    scene.props.len(), t.elapsed().as_secs_f64(), o.largest, o.disk, o.falling_back, o.escaping, o.radius_m / 1e3
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

/// Where a frame of fluid `experiment_8` goes, at increasing detail: the CPU's substep count, the
/// GPU frame's fixed cost (sort, upload, read-back: a frame of one substep) and each substep's cost
/// (from a frame of eight), for both solvers. `RGA_MEASURE_SIZES` sets the particle counts. Run with
/// `cargo test --release -- --ignored measure_frame_breakdown --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_frame_breakdown() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    use std::time::Instant;
    let Some(mut gpu) = gpu() else { return };
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![2000, 20000, 100000]);
    let frame = seconds(1000.0 / 60.0 * 2.0); // the 2000× clock at 60 frames a second, 33 s
    for n in sizes {
        let built = Instant::now();
        let mut scene = moon_forming_impact_with(&CollisionSettings {
            planet_model: crate::core::scene::PlanetModel::Fluid,
            ..settings(n)
        })
        .scene;
        let build_s = built.elapsed().as_secs_f64();
        for solver in [GpuSolver::EXACT, GpuSolver::SCALABLE] {
            gpu.solver = solver;
            // Warm up: pipelines, buffers.
            assert!(gpu.step_in(&mut scene, seconds(1.0), 1));
            let t = Instant::now();
            let mut substeps = 0;
            for _ in 0..5 {
                substeps = crate::collision::resolve::substep_count(&scene.props, frame);
            }
            let count_ms = t.elapsed().as_secs_f64() * 1e3 / 5.0;
            let t = Instant::now();
            let _ = crate::collision::collision::fluid_courant_seconds(&scene.props);
            let courant_ms = t.elapsed().as_secs_f64() * 1e3;
            eprintln!("MEASURE   of which the fluid Courant step {courant_ms:.1} ms");
            let t = Instant::now();
            let rows = crate::ui::overlay::distance_lines(&scene).len();
            let overlay_ms = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let mut moved = 0.0_f32;
            let ids: Vec<_> = scene.iter().map(|(id, _)| id).collect();
            for id in ids {
                if let Some(prop) = scene.get(id) {
                    moved += crate::render::bridge::transform_from(&prop.body)
                        .translation
                        .x;
                }
            }
            let sync_ms = t.elapsed().as_secs_f64() * 1e3;
            eprintln!("MEASURE   overlay distance rows {overlay_ms:.1} ms ({rows} rows), transform sync {sync_ms:.2} ms ({moved:.0})");
            let t = Instant::now();
            for _ in 0..3 {
                assert!(gpu.step_in(&mut scene, seconds(1.0), 1));
            }
            let one = t.elapsed().as_secs_f64() * 1e3 / 3.0;
            let t = Instant::now();
            for _ in 0..3 {
                assert!(gpu.step_in(&mut scene, seconds(8.0), 8));
            }
            let eight = t.elapsed().as_secs_f64() * 1e3 / 3.0;
            let per_substep = (eight - one) / 7.0;
            let fixed = one - per_substep;
            eprintln!(
                "MEASURE n={n} bodies {} {:?}/{:?}: build {build_s:.2} s, substep count {count_ms:.1} ms ({substeps} substeps a frame), fixed {fixed:.1} ms, per substep {per_substep:.2} ms, frame {:.0} ms",
                scene.props.len(),
                solver.gravity,
                solver.neighbours,
                count_ms + fixed + per_substep * f64::from(substeps)
            );
        }
    }
}

#[test]
fn the_kernels_compile_without_validation_errors() {
    let device = GpuDevice::request().expect("a GPU");
    device
        .wgpu()
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let stepper = GpuParticles::new(device);
    let error = bevy::tasks::block_on(stepper.gpu.wgpu().pop_error_scope());
    assert!(error.is_none(), "{error:?}");
}

/// The resident path frame by frame: upload fluid `experiment_8` once, then advance it a frame at a
/// time as the app does, waiting for the GPU after each to time it and reading the timing back.
/// `RGA_MEASURE_SIZES` sets the counts. Run with
/// `cargo test --release -- --ignored measure_resident_frames --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_resident_frames() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    use std::time::Instant;
    let mut gpu = gpu().unwrap();
    gpu.cooperative = std::env::var("RGA_MEASURE_COOPERATIVE").map_or(true, |v| v != "0");
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![20000]);
    let frame = seconds(1000.0 / 60.0 * 2.0);
    for n in sizes {
        let scene = moon_forming_impact_with(&CollisionSettings {
            planet_model: crate::core::scene::PlanetModel::Fluid,
            ..settings(n)
        })
        .scene;
        let grid = |gravity| GpuSolver {
            gravity,
            neighbours: NeighbourSearch::Grid,
            ..GpuSolver::DEFAULT
        };
        let theta: Vec<f32> = std::env::var("RGA_MEASURE_THETA")
            .ok()
            .map(|s| s.split(',').filter_map(|t| t.parse().ok()).collect())
            .unwrap_or_default();
        let mut solvers = vec![
            GpuSolver::EXACT,
            GpuSolver::SCALABLE,
            grid(GravitySolver::Direct),
            grid(GravitySolver::BarnesHut),
        ];
        if !theta.is_empty() {
            let neighbours = if std::env::var("RGA_MEASURE_GRID").is_ok() {
                NeighbourSearch::Grid
            } else {
                NeighbourSearch::Tree
            };
            solvers = theta
                .iter()
                .map(|&t| GpuSolver {
                    opening_angle: t,
                    neighbours,
                    ..GpuSolver::SCALABLE
                })
                .collect();
        }
        for solver in solvers {
            gpu.solver = solver;
            assert!(gpu.upload(&scene));
            let mut total = 0.0;
            for f in 0..20 {
                let t = Instant::now();
                assert!(gpu.advance(frame, 0.0));
                let (substeps, left) = gpu.finish().unwrap();
                let ms = t.elapsed().as_secs_f64() * 1e3;
                if f >= 5 {
                    total += ms;
                }
                if f % 5 == 4 {
                    eprintln!(
                        "MEASURE n={n} {:?}/{:?} frame {f}: {ms:.1} ms, {substeps} substeps, {left:.2} s left",
                        solver.gravity, solver.neighbours
                    );
                }
            }
            eprintln!(
                "MEASURE n={n} {:?}/{:?} θ {}: mean {:.1} ms a frame over the last 15",
                solver.gravity,
                solver.neighbours,
                solver.opening_angle,
                total / 15.0
            );
        }
    }
}

/// The GPU's kick-drift-kick leapfrog is second order: halving the step quarters the error. Two
/// planet-mass particles on a circular orbit (a period of `13.5 h`) for an hour, gravity only, at
/// `n`, `2n` and `8n` fixed steps: `|x(n) − x(8n)| / |x(2n) − x(8n)|` is `(1 − 1/64)/(1/4 − 1/64)
/// ≈ 4.2` for second order, `(1 − 1/8)/(1/2 − 1/8) ≈ 2.3` for first (the symplectic Euler it
/// replaced). A smooth problem: the fluid and rubble planets' contacts, viscosity switch and pressure
/// floor are not, and do not show the order.
#[test]
fn the_gpu_leapfrog_is_second_order_on_an_orbit() {
    let Some(mut gpu) = gpu() else { return };
    gpu.solver = GpuSolver::EXACT;
    let mass = 1.0e24_f64;
    let separation = 2.0e7_f64;
    let relative = (crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT * 2.0 * mass
        / separation)
        .sqrt();
    let particle = |x: f64, vy: f64| {
        Prop::new(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::new(x / METRES_PER_UNIT, 0.0, 0.0),
                    rotation: Quat::IDENTITY,
                },
                radius: (1.0e5 / METRES_PER_UNIT) as f32,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            },
            DVec3::new(0.0, vy / METRES_PER_UNIT, 0.0),
            mass as f32,
        )
        .with_material(Material::new(0.5, 0.0).with_modulus(1.0e9))
    };
    let mut start = Scene::empty(Aabb {
        min: Vec3::splat(-1.0),
        max: Vec3::splat(1.0),
    });
    start.add(particle(-0.5 * separation, -0.5 * relative));
    start.add(particle(0.5 * separation, 0.5 * relative));
    // `Scene::empty` softens gravity over a world unit (`1e9 m`), which would all but switch it off.
    start.softening_units = 0.0;
    let run = |gpu: &mut GpuParticles, steps: u32| -> Vec<DVec3> {
        let mut scene = start.clone();
        assert!(gpu.step_in(&mut scene, seconds(3600.0), steps));
        scene
            .props
            .iter()
            .map(|p| p.body.center() * METRES_PER_UNIT)
            .collect()
    };
    let steps = 8;
    let coarse = run(&mut gpu, steps);
    let middle = run(&mut gpu, 2 * steps);
    let fine = run(&mut gpu, 8 * steps);
    let gap = |a: &[DVec3]| -> f64 {
        a.iter()
            .zip(&fine)
            .map(|(x, y)| (*x - *y).length_squared())
            .sum::<f64>()
            .sqrt()
    };
    let omega = relative / separation;
    let angle = omega * 3600.0;
    let exact = DVec3::new(
        0.5 * separation * angle.cos(),
        0.5 * separation * angle.sin(),
        0.0,
    );
    eprintln!(
        "MEASURE orbit: softening {} units; errors from the exact orbit {:.1}, {:.1}, {:.1} m",
        start.softening_units,
        (coarse[1] - exact).length(),
        (middle[1] - exact).length(),
        (fine[1] - exact).length()
    );
    let ratio = gap(&coarse) / gap(&middle);
    eprintln!(
        "MEASURE orbit convergence: gaps {:.1} m and {:.1} m, ratio {ratio:.2}",
        gap(&coarse),
        gap(&middle)
    );
    assert!(ratio > 3.5, "not second order: ratio {ratio:.2}");
}

/// The cell list finds exactly the neighbours every-pair testing finds: fluid planets stepped the
/// same way with both come out the same to `f32` summation order, and so does a rubble pair.
#[test]
fn the_cell_list_finds_what_every_pair_finds() {
    use crate::core::scene::PlanetModel;
    let Some(mut gpu) = gpu() else { return };
    for model in [PlanetModel::Fluid, PlanetModel::Rubble] {
        let settings = CollisionSettings {
            particles_per_body: 200,
            planet_model: model,
            ..CollisionSettings::default()
        };
        let start = giant_impact_with(&settings).scene;
        let run = |gpu: &mut GpuParticles, neighbours: NeighbourSearch| -> Scene {
            gpu.solver = GpuSolver {
                neighbours,
                ..GpuSolver::EXACT
            };
            let mut scene = start.clone();
            assert!(gpu.step_in(&mut scene, seconds(60.0), 12));
            scene
        };
        let pairs = run(&mut gpu, NeighbourSearch::AllPairs);
        let grid = run(&mut gpu, NeighbourSearch::Grid);
        let worst = pairs
            .props
            .iter()
            .zip(&grid.props)
            .map(|(a, b)| (a.body.center() - b.body.center()).length() * METRES_PER_UNIT)
            .fold(0.0, f64::max);
        let worst_density = pairs
            .props
            .iter()
            .zip(&grid.props)
            .filter(|(a, _)| a.density_kg_m3 > 0.0)
            .map(|(a, b)| (a.density_kg_m3 / b.density_kg_m3 - 1.0).abs())
            .fold(0.0, f64::max);
        eprintln!(
            "MEASURE cell list {model:?}: worst position {worst:.3} m, density {worst_density:.2e}"
        );
        assert!(worst < 5.0, "{model:?}: {worst:.3} m apart");
        assert!(
            worst_density < 1.0e-4,
            "{model:?}: densities {worst_density:.2e} apart"
        );
    }
}

/// Every particle's acceleration from one evaluation of `scene` with `solver`, in scene order:
/// upload, one priming tick (forces, nothing moved), and the integrator's stored accelerations.
fn accelerations(gpu: &mut GpuParticles, scene: &Scene, solver: GpuSolver) -> Vec<DVec3> {
    gpu.solver = solver;
    assert!(gpu.upload(scene));
    gpu.encode_advance(0.0, 0, true);
    let count = scene.props.len();
    let buffers = gpu.buffers.as_ref().unwrap();
    let particle_bytes = count as u64 * BYTES_PER_PARTICLE;
    let mut encoder = gpu
        .gpu
        .wgpu()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_buffer_to_buffer(&buffers.particles, 0, &buffers.readback, 0, particle_bytes);
    gpu.gpu.queue.submit(Some(encoder.finish()));
    let state = MapState::new();
    map_async(&buffers.readback, particle_bytes, &state);
    gpu.gpu.wgpu().poll(wgpu::Maintain::Wait);
    let particles = read_mapped(&buffers.readback, particle_bytes);
    let slot_bytes = count as u64 * BYTES_PER_SLOT;
    let mut encoder = gpu
        .gpu
        .wgpu()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_buffer_to_buffer(&buffers.kinematics, 0, &buffers.readback, 0, slot_bytes);
    gpu.gpu.queue.submit(Some(encoder.finish()));
    let state = MapState::new();
    map_async(&buffers.readback, slot_bytes, &state);
    gpu.gpu.wgpu().poll(wgpu::Maintain::Wait);
    let slots = read_mapped(&buffers.readback, slot_bytes);
    let mut out = vec![DVec3::ZERO; count];
    for (particle, slot) in particles
        .chunks_exact(FLOATS_PER_PARTICLE)
        .zip(slots.chunks_exact(16))
    {
        out[particle[15] as usize] =
            DVec3::new(f64::from(slot[0]), f64::from(slot[1]), f64::from(slot[2]));
    }
    out
}

/// The median and 99th-percentile relative error of `approximate` against `exact`.
fn force_errors(approximate: &[DVec3], exact: &[DVec3]) -> (f64, f64) {
    let mut errors: Vec<f64> = approximate
        .iter()
        .zip(exact)
        .map(|(a, e)| (*a - *e).length() / e.length().max(1e-30))
        .collect();
    errors.sort_by(f64::total_cmp);
    (errors[errors.len() / 2], errors[errors.len() * 99 / 100])
}

/// Barnes–Hut's force error against the direct sum, at several opening angles, on the planets of
/// `experiment_8` spread apart so only gravity acts. Run with
/// `cargo test --release -- --ignored measure_tree_force_error --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_tree_force_error() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    let mut gpu = gpu().unwrap();
    let mut scene = moon_forming_impact_with(&settings(2000)).scene;
    let centre =
        scene.props.iter().map(|p| p.body.center()).sum::<DVec3>() / scene.props.len() as f64;
    for prop in &mut scene.props {
        let offset = (prop.body.center() - centre) * 0.5;
        prop.body.translate(offset);
    }
    let exact = accelerations(&mut gpu, &scene, GpuSolver::EXACT);
    for (theta, cooperative) in [0.3_f32, 0.5, 0.7, 0.9]
        .into_iter()
        .flat_map(|t| [(t, false), (t, true)])
    {
        gpu.cooperative = cooperative;
        let tree = accelerations(
            &mut gpu,
            &scene,
            GpuSolver {
                gravity: GravitySolver::BarnesHut,
                neighbours: NeighbourSearch::AllPairs,
                opening_angle: theta,
            },
        );
        let (median, p99) = force_errors(&tree, &exact);
        eprintln!("MEASURE θ = {theta}, cooperative {cooperative}: median {median:.2e}, 99th percentile {p99:.2e}");
    }
}

/// The tree's forces against the direct sum, on the planets of `experiment_8` spread apart so only
/// gravity acts, at `θ = 0.5`. Measured on an RTX 4070 SUPER (median, 99th percentile): monopoles
/// walked by each particle `4.3e-3`, `2.1e-2`; with quadrupoles `5.2e-4`, `2.9e-3`; with the
/// cooperative walk `5.0e-5`, `1.0e-3` (its opening test, against the workgroup's box, is stricter).
#[test]
fn the_tree_s_forces_match_the_direct_sum() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    let Some(mut gpu) = gpu() else { return };
    let mut scene = moon_forming_impact_with(&settings(2000)).scene;
    let centre =
        scene.props.iter().map(|p| p.body.center()).sum::<DVec3>() / scene.props.len() as f64;
    for prop in &mut scene.props {
        let offset = (prop.body.center() - centre) * 0.5;
        prop.body.translate(offset);
    }
    let exact = accelerations(&mut gpu, &scene, GpuSolver::EXACT);
    let tree = GpuSolver {
        gravity: GravitySolver::BarnesHut,
        neighbours: NeighbourSearch::AllPairs,
        opening_angle: 0.5,
    };
    for (cooperative, median_bound, p99_bound) in [(false, 2.0e-3, 1.0e-2), (true, 5.0e-4, 5.0e-3)]
    {
        gpu.cooperative = cooperative;
        let (median, p99) = force_errors(&accelerations(&mut gpu, &scene, tree), &exact);
        eprintln!(
            "MEASURE tree forces, cooperative {cooperative}: median {median:.2e}, 99th {p99:.2e}"
        );
        assert!(
            median < median_bound,
            "cooperative {cooperative}: median {median:.2e}"
        );
        assert!(
            p99 < p99_bound,
            "cooperative {cooperative}: 99th percentile {p99:.2e}"
        );
    }
    gpu.cooperative = true;
}

/// The app's frame loop on the resident path, at several time scales: advance by the scale times
/// the last frame's real length, wait for the GPU, and report the simulated seconds per real second
/// actually reached. `RGA_MEASURE_SIZES` sets the detail. Run with
/// `cargo test --release -- --ignored measure_time_scale_ceiling --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_time_scale_ceiling() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    use std::time::Instant;
    let mut gpu = gpu().unwrap();
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![200, 2000, 20000]);
    for n in sizes {
        let scene = moon_forming_impact_with(&CollisionSettings {
            planet_model: crate::core::scene::PlanetModel::Fluid,
            ..settings(n)
        })
        .scene;
        gpu.solver = GpuSolver::SCALABLE;
        for scale in [1.0e3_f64, 1.0e4, 1.0e5] {
            assert!(gpu.upload(&scene));
            let mut real = 1.0 / 60.0;
            let start = Instant::now();
            let mut simulated_start = None;
            while start.elapsed().as_secs_f64() < 3.0 {
                let t = Instant::now();
                assert!(gpu.advance(seconds(scale * real), real as f32));
                gpu.finish();
                real = t.elapsed().as_secs_f64().max(1.0e-4);
                if simulated_start.is_none() && start.elapsed().as_secs_f64() > 1.0 {
                    simulated_start = Some((Instant::now(), gpu.simulated_seconds().unwrap()));
                }
            }
            let (t0, s0) = simulated_start.unwrap();
            let rate = (gpu.simulated_seconds().unwrap() - s0) / t0.elapsed().as_secs_f64();
            eprintln!("MEASURE n={n} scale {scale:.0e}: {rate:.0} simulated s per real s, last frame {:.1} ms", real * 1e3);
        }
    }
}

/// The fluid giant impact at `particles` a planet, every input set here.
fn fluid_impact(particles: u32) -> Scene {
    giant_impact_with(&CollisionSettings {
        particles_per_body: particles,
        planet_model: crate::core::scene::PlanetModel::Fluid,
        ..CollisionSettings::default()
    })
    .scene
}

#[test]
fn the_pass_timer_times_every_kernel_group_the_solver_runs() {
    use crate::compute::profiler::GpuPhase;
    let Some(mut gpu) = gpu() else { return };
    assert!(
        gpu.set_profiling(true),
        "the GPU cannot write timestamps: {}",
        gpu.describe()
    );
    gpu.solver = GpuSolver {
        gravity: GravitySolver::BarnesHut,
        neighbours: NeighbourSearch::Grid,
        opening_angle: 0.7,
    };
    gpu.cooperative = true;
    assert!(gpu.upload(&fluid_impact(200)));
    for _ in 0..6 {
        assert!(gpu.advance(seconds(60.0), 0.0));
        gpu.finish();
    }
    let profile = gpu.profile_since_reset().unwrap();
    eprintln!("{profile}");
    assert!(profile.frames >= 1, "{profile}");
    for phase in [
        GpuPhase::Sort,
        GpuPhase::Restore,
        GpuPhase::Drift,
        GpuPhase::Refit,
        GpuPhase::Grid,
        GpuPhase::Density,
        GpuPhase::GravityWalk,
        GpuPhase::GridContacts,
        GpuPhase::Kick,
    ] {
        assert!(profile.phase(phase) > 0.0, "{phase:?} untimed: {profile}");
    }
    // Neither of the kernels this solver does not use.
    assert_eq!(profile.phase(GpuPhase::PairForces), 0.0);
    assert_eq!(profile.phase(GpuPhase::TreeWalk), 0.0);
    assert!(profile.span_ms >= profile.passes_ms() * 0.999, "{profile}");
    assert!(profile.advances >= 1 && profile.ticks > 0.0, "{profile}");
    assert!(
        profile.active_fraction > 0.0 && profile.active_fraction <= 1.0,
        "{profile}"
    );
    // Off again: no timer, no figures.
    assert!(!gpu.set_profiling(false));
    assert!(gpu.profile().is_none());
}

#[test]
fn the_pass_timer_leaves_the_simulation_as_it_was() {
    let Some(mut gpu) = gpu() else { return };
    // The tree's neighbours: no atomic scatter, so the same dispatches give the same bits.
    gpu.solver = GpuSolver {
        gravity: GravitySolver::BarnesHut,
        neighbours: NeighbourSearch::Tree,
        opening_angle: 0.7,
    };
    let start = fluid_impact(200);
    let mut run = |profiling: bool| -> Scene {
        gpu.set_profiling(profiling);
        let mut scene = start.clone();
        assert!(gpu.step_in(&mut scene, seconds(120.0), 16));
        scene
    };
    let plain = run(false);
    let timed = run(true);
    for (a, b) in plain.props.iter().zip(&timed.props) {
        assert_eq!(a.body.center(), b.body.center());
        assert_eq!(a.velocity_units_s, b.velocity_units_s);
        assert_eq!(a.heat_j_kg, b.heat_j_kg);
    }
}

/// Where the GPU's frame goes, pass by pass, for fluid `experiment_8` advanced a frame at a time
/// as the app does (each frame's real length sets the next frame's substep budget). Run with
/// `cargo test --release -- --ignored measure_gpu_profile --nocapture`. `RGA_MEASURE_SIZES` sets the
/// particles a planet (default `20000`), `RGA_MEASURE_THETA` the opening angle (`0.7`), and
/// `RGA_MEASURE_NEIGHBOURS` `grid` (the default) or `tree`.
#[test]
#[ignore = "measurement"]
fn measure_gpu_profile() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    use std::time::Instant;
    let mut gpu = gpu().unwrap();
    assert!(
        gpu.set_profiling(true),
        "no timestamps on {}",
        gpu.describe()
    );
    let sizes: Vec<u32> = std::env::var("RGA_MEASURE_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![20000]);
    let theta = std::env::var("RGA_MEASURE_THETA")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(0.7);
    let neighbours = match std::env::var("RGA_MEASURE_NEIGHBOURS").as_deref() {
        Ok("tree") => NeighbourSearch::Tree,
        _ => NeighbourSearch::Grid,
    };
    gpu.solver = GpuSolver {
        gravity: GravitySolver::BarnesHut,
        neighbours,
        opening_angle: theta,
    };
    // The 1000× clock at 30 frames a second.
    let frame = seconds(1000.0 / 30.0);
    for n in sizes {
        let scene = moon_forming_impact_with(&CollisionSettings {
            particles_per_body: n,
            planet_model: crate::core::scene::PlanetModel::Fluid,
            ..CollisionSettings::default()
        })
        .scene;
        assert!(gpu.upload(&scene));
        let mut real = 0.0_f32;
        let mut wall = 0.0;
        let frames = 40;
        let warmup = 10;
        for f in 0..frames {
            if f == warmup {
                gpu.reset_profile();
            }
            let t = Instant::now();
            assert!(gpu.advance(frame, real));
            gpu.finish();
            real = t.elapsed().as_secs_f32();
            if f >= warmup {
                wall += f64::from(real) * 1e3;
            }
        }
        let profile = gpu.profile_since_reset().unwrap();
        eprintln!(
            "MEASURE n={n} ({} particles) {:?} θ {theta}: wall {:.1} ms a frame (waiting for the GPU)",
            scene.props.len(),
            neighbours,
            wall / f64::from(frames - warmup)
        );
        for line in profile.lines() {
            eprintln!("MEASURE   {line}");
        }
        eprintln!("MEASURE   {profile}");
    }
}
