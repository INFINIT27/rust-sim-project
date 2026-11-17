//! Unit tests for the [`crate::projects::interstellar::earth_moon`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/projects/interstellar/earth_moon.rs` via `#[path]`, not as a standalone integration test.
//!
//! # What these tests are for
//!
//! The experiment claims two things, and each needs its own evidence:
//!
//! 1. **Everything is at real relative size.** The radii, the separation and the masses are checked
//!    against the published figures, and the ratios a viewer reads off the screen (Moon to Earth,
//!    orbit to Earth) are checked on the built scene, not on the table.
//! 2. **Both bodies orbit their barycentre.** The barycentre is at the origin and at rest, it lies
//!    inside the Earth, and through the real integrator the Moon comes back after one period with
//!    the separation unchanged.

use super::*;
use crate::core::scene::Prop;
use crate::projects::interstellar::constants::EARTH_RADIUS_M;
use crate::time::constants::SECONDS_PER_DAY;

/// The Earth is added first and there is no floor before it.
const EARTH_INDEX: usize = 0;
/// The Moon is added second.
const MOON_INDEX: usize = 1;

fn built() -> Experiment {
    earth_moon_system()
}

fn radius_of(prop: &Prop) -> f32 {
    match &prop.body {
        Body::Sphere { radius, .. } => *radius,
        other => panic!("expected a sphere, got {other:?}"),
    }
}

// -----------------------------------------------------------------------
// The data, against published astronomy.
// -----------------------------------------------------------------------

#[test]
fn the_two_bodies_are_the_earth_and_the_moon_from_the_shared_tables() {
    let earth = earth_moon_primary();
    let moon = earth_moon_satellite();

    // By name, so the fallback row is provably not what is being used.
    assert_eq!(earth.name, "Earth");
    assert_eq!(moon.name, "Moon");
    assert!(earth.is_valid() && moon.is_valid());

    // The same rows `experiment_3` reads.
    assert_eq!(earth.radius_m, EARTH_RADIUS_M);
    assert_eq!(moon, EARTH_MOONS[0]);
}

#[test]
fn the_masses_radii_and_distance_are_the_published_ones() {
    let earth = earth_moon_primary();
    let moon = earth_moon_satellite();

    assert!((earth.mass_kg / 5.9722e24 - 1.0).abs() < 1.0e-3);
    assert!((earth.radius_m / 6.371e6 - 1.0).abs() < 1.0e-3);
    assert!((moon.mass_kg / 7.342e22 - 1.0).abs() < 1.0e-3);
    assert!((moon.radius_m / 1.7374e6 - 1.0).abs() < 1.0e-3);
    assert!((moon.semi_major_axis_m / 3.844e8 - 1.0).abs() < 1.0e-3);

    // The familiar ratio: the Earth is 81.3 Moon masses.
    assert!((earth.mass_kg / moon.mass_kg - 81.3).abs() < 0.1);
}

#[test]
fn the_period_is_the_sidereal_month() {
    let days = earth_moon_orbit_period_seconds() / SECONDS_PER_DAY;

    // 27.28 from a circular two-body orbit; 27.32 measured.
    assert!(
        (days / 27.32 - 1.0).abs() < 0.005,
        "the orbit takes {days} days"
    );
}

#[test]
fn the_relative_speed_is_the_moons_orbital_speed() {
    let speed_m_s = earth_moon_orbit_speed_units_s() * METRES_PER_UNIT;

    // 1022 m/s is the Moon's measured mean orbital speed.
    assert!(
        (speed_m_s / 1022.0 - 1.0).abs() < 0.005,
        "the relative speed is {speed_m_s} m/s"
    );
}

// -----------------------------------------------------------------------
// The scene: real relative sizes.
// -----------------------------------------------------------------------

#[test]
fn the_scene_is_two_movable_spheres_and_no_floor() {
    let experiment = built();

    assert_eq!(experiment.scene.props.len(), 2);
    assert!(experiment
        .scene
        .props
        .iter()
        .all(|prop| !prop.immovable && matches!(prop.body, Body::Sphere { .. })));
}

#[test]
fn both_bodies_are_stored_at_their_true_radius() {
    let experiment = built();
    let earth = &experiment.scene.props[EARTH_INDEX];
    let moon = &experiment.scene.props[MOON_INDEX];

    assert!((f64::from(radius_of(earth)) * METRES_PER_UNIT / 6.371e6 - 1.0).abs() < 1.0e-5);
    assert!((f64::from(radius_of(moon)) * METRES_PER_UNIT / 1.7374e6 - 1.0).abs() < 1.0e-5);

    // One radius each: nothing is drawn at a size it does not collide at.
    assert!(earth.contact_radius_units.is_none());
    assert!(moon.contact_radius_units.is_none());
}

#[test]
fn the_proportions_on_screen_are_the_real_ones() {
    let experiment = built();
    let earth = &experiment.scene.props[EARTH_INDEX];
    let moon = &experiment.scene.props[MOON_INDEX];

    // The Moon is 0.2727 of the Earth's radius.
    let size_ratio = radius_of(moon) / radius_of(earth);
    assert!((size_ratio - 0.2727).abs() < 1.0e-3, "got {size_ratio}");

    // And 60.3 Earth radii away, centre to centre.
    let separation = (moon.body.center() - earth.body.center()).length();
    let in_earth_radii = separation / f64::from(radius_of(earth));
    assert!(
        (in_earth_radii - 60.34).abs() < 0.05,
        "got {in_earth_radii}"
    );
}

#[test]
fn the_masses_are_the_real_ones() {
    let experiment = built();

    let earth_mass = f64::from(experiment.scene.props[EARTH_INDEX].mass_kg);
    let moon_mass = f64::from(experiment.scene.props[MOON_INDEX].mass_kg);

    assert!((earth_mass / 5.9722e24 - 1.0).abs() < 1.0e-6);
    assert!((moon_mass / 7.342e22 - 1.0).abs() < 1.0e-6);
}

#[test]
fn the_bodies_are_named() {
    let experiment = built();

    assert_eq!(
        experiment
            .names
            .label(crate::core::scene::BodyId(EARTH_INDEX as u64)),
        "Earth"
    );
    assert_eq!(
        experiment
            .names
            .label(crate::core::scene::BodyId(MOON_INDEX as u64)),
        "Moon"
    );
}

// -----------------------------------------------------------------------
// The barycentre.
// -----------------------------------------------------------------------

#[test]
fn the_barycentre_is_at_the_origin_and_at_rest() {
    let experiment = built();
    let earth = &experiment.scene.props[EARTH_INDEX];
    let moon = &experiment.scene.props[MOON_INDEX];

    let earth_mass = f64::from(earth.mass_kg);
    let moon_mass = f64::from(moon.mass_kg);
    let total = earth_mass + moon_mass;

    let centre = (earth.body.center() * earth_mass + moon.body.center() * moon_mass) / total;
    let drift = (earth.velocity_units_s * earth_mass + moon.velocity_units_s * moon_mass) / total;

    // Against the 0.3844-unit separation and the 1.0e-6 units/s orbital speed. The residue is the
    // masses having been rounded to `f32` on the props.
    assert!(centre.length() < 1.0e-8, "the barycentre is at {centre:?}");
    assert!(
        drift.length() < 1.0e-13,
        "the barycentre drifts at {drift:?}"
    );
}

#[test]
fn the_barycentre_is_inside_the_earth() {
    let experiment = built();
    let earth = &experiment.scene.props[EARTH_INDEX];

    let offset_km = earth.body.center().length() * METRES_PER_UNIT / 1000.0;

    // 4,670 km from the Earth's centre, under a 6,371 km surface.
    assert!((offset_km - 4_668.0).abs() < 10.0, "got {offset_km} km");
    assert!(earth.body.center().length() < f64::from(radius_of(earth)));
}

#[test]
fn both_velocities_are_tangential_and_opposite() {
    let experiment = built();
    let earth = &experiment.scene.props[EARTH_INDEX];
    let moon = &experiment.scene.props[MOON_INDEX];

    for prop in [earth, moon] {
        let radial = prop.body.center().normalize();
        let along = prop.velocity_units_s.normalize().dot(radial);

        assert!(along.abs() < 1.0e-9, "a radial component of {along}");
        assert_eq!(prop.velocity_units_s.y, 0.0, "the orbit is horizontal");
    }

    let cosine = earth
        .velocity_units_s
        .normalize()
        .dot(moon.velocity_units_s.normalize());
    assert!((cosine + 1.0).abs() < 1.0e-9, "they should be antiparallel");

    // The two speeds add up to the relative speed, which is the whole point of the split.
    let sum = earth.velocity_units_s.length() + moon.velocity_units_s.length();
    assert!((sum / earth_moon_orbit_speed_units_s() - 1.0).abs() < 1.0e-9);
}

#[test]
fn the_moon_orbits_prograde() {
    // Counterclockwise seen from above (+Y), the convention the other orbital scenes use: the
    // Moon's angular momentum about the origin points up.
    let experiment = built();
    let moon = &experiment.scene.props[MOON_INDEX];

    let angular = moon.body.center().cross(moon.velocity_units_s);
    assert!(angular.y > 0.0, "got {angular:?}");
}

// -----------------------------------------------------------------------
// The spec.
// -----------------------------------------------------------------------

#[test]
fn the_spec_frames_the_whole_orbit() {
    let experiment = built();
    let spec = experiment.spec;

    assert_eq!(spec.id, ExperimentId::EarthMoon);
    assert!(spec.is_valid());
    assert_eq!(spec.time_scale, EARTH_MOON_TIME_SCALE);
    assert_eq!(spec.trail_length_units, Some(EARTH_MOON_TRAIL_LENGTH_UNITS));
    assert_eq!(spec.camera_target_units, Vec3::ZERO);

    let moon_distance = experiment.scene.props[MOON_INDEX].body.center().length() as f32;
    assert!(
        spec.framed_radius_units > moon_distance,
        "the opening view must contain the Moon's whole circle"
    );
}

#[test]
fn the_bounds_contain_both_bodies() {
    let experiment = built();
    let bounds = experiment.scene.bounds;
    assert!(bounds.is_valid());

    for prop in &experiment.scene.props {
        let centre = prop.body.center().as_vec3();

        assert!(
            centre.cmpge(bounds.min).all() && centre.cmple(bounds.max).all(),
            "{centre:?} is outside {bounds:?}"
        );
    }
}

#[test]
fn an_orbit_is_watchable_at_the_shipped_clock() {
    let watching = earth_moon_orbit_period_seconds() / f64::from(EARTH_MOON_TIME_SCALE);

    assert!(
        (10.0..60.0).contains(&watching),
        "one orbit takes {watching} s of watching"
    );
}

#[test]
fn the_trail_is_shorter_than_the_orbit() {
    let experiment = built();
    let circumference =
        std::f64::consts::TAU * experiment.scene.props[MOON_INDEX].body.center().length();

    assert!(f64::from(EARTH_MOON_TRAIL_LENGTH_UNITS) < circumference);
}

#[test]
fn the_softening_leaves_newtons_force_alone() {
    let experiment = built();
    let separation = (experiment.scene.props[MOON_INDEX].body.center()
        - experiment.scene.props[EARTH_INDEX].body.center())
    .length();
    let softening = f64::from(experiment.scene.softening_units);

    assert_eq!(experiment.scene.softening_units, EARTH_MOON_SOFTENING_UNITS);

    // Plummer softening leaves (r²/(r² + ε²))^1.5 of the force.
    let kept =
        (separation * separation / (separation * separation + softening * softening)).powf(1.5);
    assert!(kept > 0.9999, "softening keeps only {kept} of the force");
}

#[test]
fn the_experiment_is_deterministic() {
    assert_eq!(
        ExperimentId::EarthMoon.build(1).scene,
        ExperimentId::EarthMoon.build(2).scene
    );
}

// -----------------------------------------------------------------------
// Does it actually orbit? The end-to-end checks.
// -----------------------------------------------------------------------

/// Run the built scene for `periods` orbital periods at 60 frames a second on its own clock.
fn run_for(periods: f64) -> Scene {
    use crate::time::simulation::SimulationClock;

    let Experiment {
        mut scene, spec, ..
    } = built();
    let mut clock = SimulationClock::new(spec.time_scale);

    let frame = 1.0 / 60.0;
    let simulated = periods * earth_moon_orbit_period_seconds();
    let frames = (simulated / (f64::from(spec.time_scale) * frame)).round() as usize;

    for _ in 0..frames {
        scene.step(clock.advance(frame as f32));
    }

    scene
}

#[test]
fn the_moon_completes_one_orbit_in_one_period_and_comes_back() {
    let start = built();
    let start_offset =
        start.scene.props[MOON_INDEX].body.center() - start.scene.props[EARTH_INDEX].body.center();

    let scene = run_for(1.0);
    let end_offset = scene.props[MOON_INDEX].body.center() - scene.props[EARTH_INDEX].body.center();

    assert!(
        (end_offset.length() / start_offset.length() - 1.0).abs() < 0.02,
        "the separation drifted from {} to {}",
        start_offset.length(),
        end_offset.length()
    );

    let cosine = end_offset
        .normalize()
        .dot(start_offset.normalize())
        .clamp(-1.0, 1.0);
    let degrees_short = cosine.acos().to_degrees();
    assert!(
        degrees_short < 10.0,
        "after one period the Moon is {degrees_short}° from where it started"
    );
}

#[test]
fn half_a_period_puts_the_moon_on_the_far_side() {
    let start = built();
    let start_offset =
        start.scene.props[MOON_INDEX].body.center() - start.scene.props[EARTH_INDEX].body.center();

    let scene = run_for(0.5);
    let end_offset = scene.props[MOON_INDEX].body.center() - scene.props[EARTH_INDEX].body.center();

    let cosine = end_offset.normalize().dot(start_offset.normalize());
    assert!(cosine < -0.98, "got a cosine of {cosine}");
}

#[test]
fn the_system_holds_together_for_several_orbits() {
    // Five months. The separation is checked, not the phase: where the Moon is after five orbits
    // is the integrator's phase error, which this crate does not pin.
    let scene = run_for(5.0);

    assert_eq!(scene.props.len(), 2, "nothing was absorbed or removed");

    let earth = &scene.props[EARTH_INDEX];
    let moon = &scene.props[MOON_INDEX];
    let separation = (moon.body.center() - earth.body.center()).length();

    assert!(
        (separation / 0.3844 - 1.0).abs() < 0.05,
        "the separation is {separation} units"
    );

    // The barycentre has not wandered: momentum was zero and gravity is an internal force.
    let earth_mass = f64::from(earth.mass_kg);
    let moon_mass = f64::from(moon.mass_kg);
    let centre = (earth.body.center() * earth_mass + moon.body.center() * moon_mass)
        / (earth_mass + moon_mass);

    assert!(
        centre.length() < 1.0e-3,
        "the barycentre moved to {centre:?}"
    );
    assert!(earth.is_valid() && moon.is_valid());
}
