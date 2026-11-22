//! Tests for [`crate::projects::interstellar::moon_forming`], `#[path]`-included into it.
//!
//! The start is the orbit the module says (its angular momentum, its time to contact), and the run
//! keeps every kilogram-metre-squared-per-second of angular momentum, orbit and particle spin
//! together, while the planets merge into one spinning body.

use super::*;
use crate::collision::aggregate::{contact_clusters, impact_outcome};
use crate::collision::collision::{particle_energy, physical_radius_m};
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::Prop;
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
fn the_impact_meets_on_time_merges_and_keeps_its_angular_momentum() {
    let mut scene = moon_forming_impact().scene;
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
