//! Tests for [`crate::projects::interstellar::moon_forming`], `#[path]`-included into it.
//!
//! The start is the orbit the module says (its angular momentum, its time to contact), and the run
//! keeps every kilogram-metre-squared-per-second of angular momentum, orbit and particle spin
//! together, while the planets merge into one spinning body.

use super::*;
use crate::collision::aggregate::{contact_clusters, impact_outcome};
use crate::collision::collision::{particle_energy, physical_radius_m};
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{PlanetModel, Prop};
use crate::physics_utils::thermal::sphere_moment_of_inertia;
use crate::time::simulation::TimeStep;

/// Total angular momentum of `props` about their centre of mass, orbit and spin, kg·m²/s.
fn angular_momentum(props: &[Prop]) -> DVec3 {
    let mass: f64 = props.iter().map(|p| f64::from(p.mass_kg)).sum();
    let centre = props
        .iter()
        .map(|p| p.body.center() * METRES_PER_UNIT * f64::from(p.mass_kg))
        .sum::<DVec3>()
        / mass;
    let drift = props
        .iter()
        .map(|p| p.velocity_units_s * METRES_PER_UNIT * f64::from(p.mass_kg))
        .sum::<DVec3>()
        / mass;
    props
        .iter()
        .map(|p| {
            let m = f64::from(p.mass_kg);
            let r = p.body.center() * METRES_PER_UNIT - centre;
            let v = p.velocity_units_s * METRES_PER_UNIT - drift;
            let inertia = sphere_moment_of_inertia(m, physical_radius_m(p).unwrap_or(0.0));
            r.cross(v) * m + p.spin_rad_s * inertia
        })
        .sum()
}

#[test]
fn the_start_carries_the_orbit_s_angular_momentum_about_the_z_axis() {
    let scene = moon_forming_impact().scene;
    let l = angular_momentum(&scene.props);
    let expected = moon_forming_angular_momentum();
    assert!(
        (l.z / expected - 1.0).abs() < 1.0e-6,
        "{l} against {expected:e}"
    );
    assert!(l.x.abs() < 1.0e-9 * expected && l.y.abs() < 1.0e-9 * expected);
}

#[test]
fn it_is_registered_as_experiment_8() {
    let id = ExperimentId::MoonForming;
    assert_eq!(id.slug(), "experiment_8");
    assert!(ExperimentId::ALL.contains(&id));
    assert!(id.matches("moon-forming") && id.matches("8"));
    assert_eq!(id.build_default().spec.id, id);
}

#[test]
fn rubble_planets_meet_on_time_merge_and_keep_their_angular_momentum() {
    let mut scene = moon_forming_impact_with(&CollisionSettings {
        planet_model: PlanetModel::Rubble,
        ..CollisionSettings::default()
    })
    .scene;
    let softening = scene.softening_units;
    let start_l = angular_momentum(&scene.props);
    let start_e = particle_energy(&scene.props, softening);
    let step = TimeStep::from_seconds(60.0).unwrap();
    let mut elapsed = 0.0;
    let mut met = None;
    let mut worst_l = 0.0_f64;
    let mut worst_e = 0.0_f64;
    while elapsed < 24.0 * 3600.0 {
        scene.step(step);
        elapsed += 60.0;
        if met.is_none() && contact_clusters(&scene.props).len() < 2 {
            met = Some(elapsed);
        }
        if ((elapsed / 60.0) as u64).is_multiple_of(30) {
            worst_l = worst_l.max((angular_momentum(&scene.props) - start_l).length());
            let e = particle_energy(&scene.props, softening).total();
            worst_e = worst_e.max((e - start_e.total()).abs());
        }
    }

    // They touch when Barker's equation says, give or take the lattice: the outermost particles
    // sit up to a particle radius inside each planet's surface (the impactor is `19` particles at
    // this detail), and at `45°` the centres close at only `cos 45°` of the impact speed, so the
    // shortfall shows as minutes. Measured: `3.8%` late.
    let met = met.expect("the planets met");
    let law = moon_forming_impact_seconds();
    assert!(
        (met - law).abs() < 0.05 * law,
        "met after {met} s, the law says {law} s"
    );

    // Angular momentum: every force is central or, for friction, comes with the torque that keeps
    // the books; what is left is `f64` rounding.
    assert!(
        worst_l < 1.0e-9 * start_l.length(),
        "angular momentum moved by {worst_l:.3e} of {:.3e}",
        start_l.length()
    );
    // Energy, heat included, to the integrator's error. Measured: `0.11%` of the binding energy.
    assert!(
        worst_e < 2.0e-3 * start_e.gravity.abs(),
        "energy drifted {worst_e:.3e} of {:.3e}",
        start_e.gravity.abs()
    );

    // One spinning body with nearly everything, and some material on orbits that clear its
    // surface. Measured at the default detail after a day: `95%` in the body, `3%` falling back,
    // `1.5%` in the disk (three particles, `1.4` lunar masses), `0.45%` escaping.
    let outcome = impact_outcome(&scene.props);
    assert!(outcome.largest_remnant() > 0.9, "{outcome:?}");
    assert!(outcome.disk > 0.0, "a disk: {outcome:?}");
    assert!(outcome.escaping < 0.05, "{outcome:?}");
    let body = &contact_clusters(&scene.props)[0];
    let members: Vec<Prop> = body.iter().map(|&i| scene.props[i].clone()).collect();
    let spin = angular_momentum(&members);
    assert!(
        spin.z > 0.5 * start_l.z,
        "the merged body spins with most of the impact's angular momentum: {:.3e} of {:.3e}",
        spin.z,
        start_l.z
    );
}

/// How much the impactor is stretched towards the Earth: the root-mean-square extent of its
/// particles along the line between the two planets' centres of mass, over the mean of the two
/// directions across it. `1` for a round body; a tide makes it more.
fn impactor_elongation(scene: &Scene, impactor: &[usize], target: &[usize]) -> f64 {
    let centre = |members: &[usize]| {
        let mass: f64 = members
            .iter()
            .map(|&i| f64::from(scene.props[i].mass_kg))
            .sum();
        members
            .iter()
            .map(|&i| scene.props[i].body.center() * f64::from(scene.props[i].mass_kg))
            .sum::<DVec3>()
            / mass
    };
    let (own, other) = (centre(impactor), centre(target));
    let axis = (other - own).normalize();
    let (mut along, mut across) = (0.0, 0.0);
    for &i in impactor {
        let offset = scene.props[i].body.center() - own;
        let parallel = offset.dot(axis);
        along += parallel * parallel;
        across += 0.5 * (offset.length_squared() - parallel * parallel);
    }
    (along / across).sqrt()
}

/// Theia's elongation at the start and just before contact (centres within `1.1` contact
/// distances), for `settings`.
fn elongation_before_contact(settings: &CollisionSettings) -> (f64, f64) {
    let mut scene = moon_forming_impact_with(settings).scene;
    // The impactor starts four contact distances out along X; half that is clear of both surfaces.
    let split = 0.5 * MOON_FORMING_START_CONTACTS * giant_impact_contact_m() / METRES_PER_UNIT;
    let impactor: Vec<usize> = (0..scene.props.len())
        .filter(|&i| scene.props[i].body.center().x > split)
        .collect();
    let target: Vec<usize> = (0..scene.props.len())
        .filter(|&i| scene.props[i].body.center().x <= split)
        .collect();
    let at_start = impactor_elongation(&scene, &impactor, &target);
    let contact = giant_impact_contact_m() / METRES_PER_UNIT;
    let centre = |scene: &Scene, members: &[usize]| {
        members
            .iter()
            .map(|&i| scene.props[i].body.center())
            .sum::<DVec3>()
            / members.len() as f64
    };
    let step = TimeStep::from_seconds(30.0).unwrap();
    for _ in 0..(4 * 120) {
        scene.step(step);
        if (centre(&scene, &impactor) - centre(&scene, &target)).length() < 1.1 * contact {
            break;
        }
    }
    (at_start, impactor_elongation(&scene, &impactor, &target))
}

#[test]
#[ignore = "about two minutes in release: two planets of 2,000 and 214 particles each, both models, through 1.7 simulated hours on the CPU"]
fn a_fluid_impactor_is_stretched_by_the_tide_before_it_hits_and_a_rubble_one_is_not() {
    // Measured: fluid `1.000` → `1.296`, rubble `1.000` → `1.010`. On the GPU at `20,000` particles a
    // planet, fluid reaches `1.307`: the stretch does not depend on the detail.
    let detail = |planet_model| CollisionSettings {
        particles_per_body: 2000,
        planet_model,
        ..CollisionSettings::default()
    };
    let (fluid_start, fluid) = elongation_before_contact(&detail(PlanetModel::Fluid));
    let (rubble_start, rubble) = elongation_before_contact(&detail(PlanetModel::Rubble));
    println!("fluid: {fluid_start:.3} → {fluid:.3};  rubble: {rubble_start:.3} → {rubble:.3}");
    assert!(
        fluid / fluid_start > 1.05,
        "fluid {fluid_start:.3} → {fluid:.3}"
    );
    assert!(
        rubble / rubble_start < 1.02,
        "rubble {rubble_start:.3} → {rubble:.3}"
    );
}

#[test]
fn after_a_fluid_impact_no_particle_is_drawn_larger_than_in_uncompressed_rock() {
    // Hot and flung particles measure low densities, down to a particle alone measuring only itself;
    // drawn at those densities they swelled to `5.7` times their size, and the merged body read
    // `16,700 km` across, which hid most of the disk from `impact_outcome`.
    use crate::collision::collision::fluid_radius_m;
    let mut scene = moon_forming_impact().scene;
    assert_eq!(scene.collision.planet_model, PlanetModel::Fluid);
    let step = TimeStep::from_seconds(60.0).unwrap();
    for _ in 0..(6 * 60) {
        scene.step(step);
    }
    for prop in &scene.props {
        let eos = prop.material.thermal.unwrap().parameters();
        let largest = fluid_radius_m(
            f64::from(prop.mass_kg),
            eos.reference_density_kg_m3,
            eos.reference_density_kg_m3,
        );
        let radius = physical_radius_m(prop).unwrap();
        assert!(
            radius <= largest * (1.0 + 1e-6),
            "{radius} against {largest}"
        );
    }
    // The merged Earth and impactor, about 6,700 km in radius once settled; six hours in, at this
    // detail, still puffed up by the impact's heat. Measured: `9,600 km`.
    let outcome = impact_outcome(&scene.props);
    assert!(
        outcome.radius_m < 1.2e7,
        "the body's radius {:.0} km",
        outcome.radius_m / 1e3
    );
}
