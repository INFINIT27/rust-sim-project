//! Tests for [`crate::projects::interstellar::giant_impact`]: `experiment_1`.
//!
//! The scene is checked against what its docs say it is built from, and then run through
//! `Scene::step`: a planet of particles must hold together under its own gravity, and the two
//! planets must meet when the free-fall law says, merge into one body through the simulated
//! contact alone, conserve momentum, and lose energy to the contact's damping (the plastic part of
//! the merge) without the integrator ever creating more than a sliver of it.

use super::*;
use crate::collision::aggregate::contact_clusters;
use crate::core::constants::DEFAULT_PARTICLES_PER_BODY;
use crate::core::scene::Body;
use crate::projects::interstellar::catalog::ExperimentId;
use crate::projects::interstellar::constants::GIANT_IMPACT_SOFTENING_UNITS;
use crate::time::simulation::TimeStep;

const EARTH_KG: f64 = 5.9722e24;
const MARS_KG: f64 = 6.4171e23;

fn mass_of(scene: &Scene, members: &[usize]) -> f64 {
    members
        .iter()
        .map(|&i| f64::from(scene.props[i].mass_kg))
        .sum()
}

fn total_mass(scene: &Scene) -> f64 {
    scene.props.iter().map(|p| f64::from(p.mass_kg)).sum()
}

fn momentum(scene: &Scene) -> DVec3 {
    scene
        .props
        .iter()
        .map(|p| p.velocity_units_s * METRES_PER_UNIT * f64::from(p.mass_kg))
        .sum()
}

/// Kinetic and (softened, as the integrator softens it) gravitational energy, J.
fn energy(scene: &Scene) -> (f64, f64) {
    let softening = f64::from(scene.softening_units) * METRES_PER_UNIT;
    let mut kinetic = 0.0;
    let mut potential = 0.0;
    for (i, a) in scene.props.iter().enumerate() {
        kinetic +=
            0.5 * f64::from(a.mass_kg) * (a.velocity_units_s * METRES_PER_UNIT).length_squared();
        for b in &scene.props[i + 1..] {
            let r = (a.body.center() - b.body.center()).length() * METRES_PER_UNIT;
            potential -= GRAVITATIONAL_CONSTANT * f64::from(a.mass_kg) * f64::from(b.mass_kg)
                / (r * r + softening * softening).sqrt();
        }
    }
    (kinetic, potential)
}

/// How far the outermost particle of a cluster reaches from the cluster's centre of mass, m.
fn extent_m(scene: &Scene, members: &[usize]) -> f64 {
    let centre = members
        .iter()
        .map(|&i| scene.props[i].body.center() * f64::from(scene.props[i].mass_kg))
        .sum::<DVec3>()
        / mass_of(scene, members);
    members
        .iter()
        .map(|&i| {
            let Body::Sphere { radius, .. } = scene.props[i].body else {
                panic!("particles are spheres")
            };
            ((scene.props[i].body.center() - centre).length() + f64::from(radius)) * METRES_PER_UNIT
        })
        .fold(0.0, f64::max)
}

fn settings(particles: u32) -> CollisionSettings {
    CollisionSettings {
        particles_per_body: particles,
        ..CollisionSettings::default()
    }
}

/// The rubble planets (springs and friction), at `particles`: what the tests written before fluid
/// planets measured, kept so the two models can be compared.
fn rubble(particles: u32) -> CollisionSettings {
    CollisionSettings {
        planet_model: PlanetModel::Rubble,
        ..settings(particles)
    }
}

#[test]
fn it_is_experiment_1_and_the_default() {
    assert_eq!(ExperimentId::GiantImpact.slug(), "experiment_1");
    assert_eq!(ExperimentId::default(), ExperimentId::GiantImpact);
    let spec = giant_impact().spec;
    assert_eq!(spec.id, ExperimentId::GiantImpact);
    assert!(spec.is_valid());
    assert!(!spec.launches, "no particle is a place to fly to");
}

#[test]
fn each_rubble_planet_is_a_pile_of_particles_with_its_true_mass() {
    let scene = giant_impact_with(&rubble(DEFAULT_PARTICLES_PER_BODY)).scene;
    let clusters = contact_clusters(&scene.props);
    assert_eq!(clusters.len(), 2, "two separate piles");
    assert!((mass_of(&scene, &clusters[0]) / EARTH_KG - 1.0).abs() < 1.0e-5);
    assert!((mass_of(&scene, &clusters[1]) / MARS_KG - 1.0).abs() < 1.0e-5);
    assert!(scene
        .props
        .iter()
        .all(|p| p.is_valid() && p.material == PLANETARY_PARTICLE_MATERIAL));
    // At the default detail: about 200 particles for the Earth, a tenth as many for the impactor.
    assert!(
        (180..=220).contains(&clusters[0].len()),
        "{}",
        clusters[0].len()
    );
    assert!(
        (10..=40).contains(&clusters[1].len()),
        "{}",
        clusters[1].len()
    );
}

#[test]
fn the_detail_setting_sets_the_particle_count() {
    let coarse = giant_impact_with(&settings(50)).scene.props.len();
    let fine = giant_impact_with(&settings(400)).scene.props.len();
    assert!(fine > 6 * coarse, "{coarse} → {fine}");
    assert_eq!(
        ExperimentId::GiantImpact
            .build_with(1, &settings(50))
            .scene
            .collision
            .particles_per_body,
        50
    );
}

#[test]
fn they_start_at_rest_about_their_centre_of_mass() {
    let scene = giant_impact().scene;
    assert!(scene
        .props
        .iter()
        .all(|p| p.velocity_units_s == DVec3::ZERO));
    let moment: DVec3 = scene
        .props
        .iter()
        .map(|p| p.body.center() * f64::from(p.mass_kg))
        .sum();
    let separation = 10.0 * giant_impact_contact_m() / METRES_PER_UNIT;
    // The masses are `f32`, so the centre of mass is exact to about `1e-7` of the span.
    assert!(moment.length() / total_mass(&scene) < 1.0e-6 * separation);
}

#[test]
fn the_quoted_impact_time_and_speed_are_what_the_laws_give() {
    assert!((giant_impact_seconds() / 3600.0 - 14.0).abs() < 0.1);
    assert!((giant_impact_speed_m_s() - 9_023.0).abs() < 10.0);
}

#[test]
fn a_planet_of_particles_holds_together_under_its_own_gravity() {
    // The Earth alone: the pile settles a few percent under its weight and then stays one body.
    let earth = giant_impact_target();
    let body = crate::collision::aggregate::ParticleBody {
        centre_units: DVec3::ZERO,
        velocity_units_s: DVec3::ZERO,
        mass_kg: earth.mass_kg,
        radius_m: earth.radius_m,
        color: earth.color,
        material: PLANETARY_PARTICLE_MATERIAL,
    };
    let mut scene = Scene::empty(crate::core::scene::Aabb {
        min: Vec3::splat(-1.0),
        max: Vec3::splat(1.0),
    });
    scene.softening_units = crate::projects::interstellar::constants::GIANT_IMPACT_SOFTENING_UNITS;
    for particle in crate::collision::aggregate::particle_body(
        &body,
        crate::collision::aggregate::lattice_spacing_m(earth.radius_m, 200),
    ) {
        scene.add(particle);
    }
    let all: Vec<usize> = (0..scene.props.len()).collect();
    let before = extent_m(&scene, &all);
    let (_, binding) = energy(&scene);

    let step = TimeStep::from_seconds(60.0).unwrap();
    for _ in 0..240 {
        scene.step(step);
    }

    let clusters = contact_clusters(&scene.props);
    assert_eq!(clusters.len(), 1, "the pile stays one body");
    let after = extent_m(&scene, &clusters[0]);
    let shrink = 1.0 - after / before;
    assert!(
        (0.0..0.08).contains(&shrink),
        "the pile compresses a little under its weight, not slumps: {:.1}%",
        100.0 * shrink
    );
    let (kinetic, _) = energy(&scene);
    assert!(
        kinetic < 1.0e-6 * binding.abs(),
        "settled: kinetic energy {kinetic:.3e} J against a binding of {binding:.3e} J"
    );
}

/// A fluid Earth alone, of about `count` particles, built in hydrostatic equilibrium.
fn fluid_earth(count: u32) -> Scene {
    let mut scene = Scene::empty(crate::core::scene::Aabb {
        min: Vec3::splat(-1.0),
        max: Vec3::splat(1.0),
    });
    scene.softening_units = crate::projects::interstellar::constants::GIANT_IMPACT_SOFTENING_UNITS;
    let earth = giant_impact_target();
    let fluid = CollisionSettings {
        particles_per_body: count,
        planet_model: PlanetModel::Fluid,
        ..CollisionSettings::default()
    };
    // Built as one planet of a pair whose second is itself, then the second removed.
    add_planets(
        &mut scene,
        &fluid,
        [
            (&earth, DVec3::ZERO, DVec3::ZERO),
            (&earth, DVec3::new(0.5, 0.0, 0.0), DVec3::ZERO),
        ],
    );
    scene.props.retain(|p| p.body.center().x < 0.25);
    scene
}

#[test]
fn a_fluid_planet_holds_its_hydrostatic_shape_and_its_energy() {
    // A fluid Earth of 200 particles alone for four hours. Measured: its outermost particles sink
    // `15–19%` in the first twenty minutes, as the kernel reaching past the surface reads the outer
    // mantle light and its pressure falls to nothing (at `2,000` particles, `10%`), and then it stays
    // one quiet body, its kinetic energy under `1e-3` of the binding energy and its total energy, heat
    // included, within `2e-4` of it.
    use crate::collision::collision::particle_energy;
    let mut scene = fluid_earth(200);
    let softening = scene.softening_units;
    let all: Vec<usize> = (0..scene.props.len()).collect();
    let before = extent_m(&scene, &all);
    let start = particle_energy(&scene.props, softening);
    let binding = start.gravity.abs();

    let step = TimeStep::from_seconds(60.0).unwrap();
    let mut worst = 0.0_f64;
    for minute in 1..=240 {
        scene.step(step);
        if minute % 10 == 0 {
            let now = particle_energy(&scene.props, softening);
            worst = worst.max((now.total() - start.total()).abs());
            assert_eq!(
                contact_clusters(&scene.props).len(),
                1,
                "one body after {minute} minutes"
            );
        }
    }
    let after = extent_m(&scene, &contact_clusters(&scene.props)[0]);
    let shrink = 1.0 - after / before;
    assert!(
        (0.0..0.25).contains(&shrink),
        "the surface settles, it does not collapse or fly apart: {:.1}%",
        100.0 * shrink
    );
    assert!(
        worst < 1.0e-3 * binding,
        "energy drifted {:.2e} of the binding energy",
        worst / binding
    );
    let end = particle_energy(&scene.props, softening);
    assert!(
        end.kinetic < 2.0e-3 * binding,
        "settled: kinetic {:.2e} of the binding energy",
        end.kinetic / binding
    );
    assert!(
        scene.props.iter().all(|p| p.is_valid()),
        "every particle valid"
    );
}

#[test]
fn a_fluid_planet_is_built_in_layers_with_its_true_mass() {
    let scene = giant_impact_with(&settings(200)).scene;
    assert_eq!(scene.collision.planet_model, PlanetModel::Fluid);
    let clusters = contact_clusters(&scene.props);
    assert_eq!(clusters.len(), 2, "two separate planets");
    for (cluster, mass) in [(&clusters[0], EARTH_KG), (&clusters[1], MARS_KG)] {
        assert!((mass_of(&scene, cluster) / mass - 1.0).abs() < 1.0e-5);
        let core: f64 = cluster
            .iter()
            .filter(|&&i| scene.props[i].material == PLANETARY_IRON_FLUID)
            .map(|&i| f64::from(scene.props[i].mass_kg))
            .sum();
        // The innermost 30% of the particles by count, to one particle: the impactor has about 21.
        let share = core / mass_of(&scene, cluster);
        assert!(
            (share - PLANET_CORE_MASS_FRACTION).abs() <= 1.0 / cluster.len() as f64,
            "core {:.1}% of the mass",
            100.0 * share
        );
        // Denser at the centre than at the surface.
        let densest = cluster
            .iter()
            .map(|&i| scene.props[i].density_kg_m3)
            .fold(0.0, f64::max);
        assert!(
            densest > 2.0 * crate::physics_utils::physics_constants::BASALT_TILLOTSON_RHO0_KG_M3
        );
    }
    assert!(scene.props.iter().all(|p| p.is_valid()
        && (p.material == PLANETARY_IRON_FLUID || p.material == PLANETARY_ROCK_FLUID)));
    // Equal masses: the impactor gets its share of the count by mass, about 21 of 200.
    assert!(
        (180..=220).contains(&clusters[0].len()),
        "{}",
        clusters[0].len()
    );
    assert!(
        (15..=30).contains(&clusters[1].len()),
        "{}",
        clusters[1].len()
    );
    // Gravity softened over half a particle spacing.
    assert!(scene.softening_units > GIANT_IMPACT_SOFTENING_UNITS);
}

#[test]
fn rubble_planets_meet_on_time_and_merge_with_every_joule_accounted_for() {
    use crate::collision::aggregate::impact_outcome;
    use crate::collision::collision::particle_energy;
    let mut scene = giant_impact_with(&rubble(DEFAULT_PARTICLES_PER_BODY)).scene;
    let softening = scene.softening_units;
    let start_momentum = momentum(&scene);
    let start = particle_energy(&scene.props, softening);
    let binding = start.gravity.abs();
    let mass = total_mass(&scene);
    let count = scene.props.len();

    let step = TimeStep::from_seconds(60.0).unwrap();
    let mut elapsed = 0.0;
    let mut met_at = None;
    let mut worst_drift = 0.0_f64;
    let mut peak_kinetic = 0.0_f64;
    while elapsed < 30.0 * 3600.0 {
        scene.step(step);
        elapsed += 60.0;
        if met_at.is_none() && contact_clusters(&scene.props).len() < 2 {
            met_at = Some(elapsed);
        }
        if ((elapsed / 60.0) as u64).is_multiple_of(30) {
            let now = particle_energy(&scene.props, softening);
            worst_drift = worst_drift.max((now.total() - start.total()).abs());
            peak_kinetic = peak_kinetic.max(now.kinetic);
        }
    }

    // They meet when the free fall of their centres says, to within a step and the few percent
    // each pile settles.
    let met = met_at.expect("the planets met");
    let law = giant_impact_seconds();
    assert!(
        (met - law).abs() < 0.01 * law,
        "met after {:.2} h, the law says {:.2} h",
        met / 3600.0,
        law / 3600.0
    );

    // A perfect merge, as Leinhardt & Stewart (2012) find below the mutual escape speed: nothing
    // escapes, and what the hot impact lofts is bound and coming back down. Measured at the default
    // detail: `96%` in the body at once, `4%` falling back, none escaping.
    let outcome = impact_outcome(&scene.props);
    assert_eq!(outcome.escaping, 0.0, "{outcome:?}");
    assert!(outcome.largest_remnant() > 0.999, "{outcome:?}");
    assert!(outcome.largest > 0.9, "{outcome:?}");
    assert_eq!(
        scene.props.len(),
        count,
        "no particle was removed or created"
    );

    // Momentum: there was none, and every force is equal and opposite.
    let scale = mass * giant_impact_speed_m_s();
    assert!(
        (momentum(&scene) - start_momentum).length() < 1.0e-9 * scale,
        "momentum {:?}",
        momentum(&scene)
    );

    // Energy: what the contacts take from the motion is heat, so the total holds, to the
    // integrator's error. Measured: within `0.04%` of the binding energy, at the moment of impact.
    let end = particle_energy(&scene.props, softening);
    assert!(
        worst_drift < 1.0e-3 * binding,
        "energy drifted {:.3e} J of a {binding:.3e} J binding",
        worst_drift
    );
    assert!(
        end.heat > 0.1 * binding,
        "the merge turned a tenth of the binding energy into heat: {end:?}"
    );
    assert!(
        end.kinetic < 1.0e-3 * peak_kinetic,
        "and the merged body has come to rest: {:.3e} J of a peak {peak_kinetic:.3e} J",
        end.kinetic
    );
}

#[test]
fn at_twice_the_escape_speed_the_hot_rubble_impact_throws_off_about_what_the_scaling_law_says() {
    // Leinhardt & Stewart (2012) give the largest remnant at `0.976` for this impact with their
    // central constant `c* = 5`, and `0.959` to `0.983` across their fit's `c* = 5 ± 2`. Without
    // heat the particle model threw nothing off at all. Measured on the CPU at the default detail:
    // `0.964` remaining, `3.6%` escaping; on the GPU from `2,000` to `20,000` particles per planet,
    // `0.95` to `0.96`: at the edge of their range.
    use crate::collision::aggregate::impact_outcome;
    use crate::physics_utils::impacts::mutual_escape_speed;
    let (target, impactor) = (giant_impact_target(), giant_impact_impactor());
    let escape = mutual_escape_speed(
        target.mass_kg,
        impactor.mass_kg,
        target.radius_m,
        impactor.radius_m,
    );
    let mut scene = giant_impact_at_speed(&rubble(DEFAULT_PARTICLES_PER_BODY), 2.0 * escape).scene;
    let step = TimeStep::from_seconds(60.0).unwrap();
    for _ in 0..(16 * 60) {
        scene.step(step);
    }
    let outcome = impact_outcome(&scene.props);
    assert!(outcome.escaping > 0.01, "debris escapes: {outcome:?}");
    assert!(
        (0.94..0.99).contains(&outcome.largest_remnant()),
        "{outcome:?}"
    );
}
