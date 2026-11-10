//! Unit tests for the [`crate::experiments::jovian`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/experiments/jovian.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! # What these tests are for
//!
//! This experiment claims its **physics is exactly real**, which is a stronger claim than any
//! other experiment makes and needs three kinds of evidence:
//!
//! 1. **The numbers are the published ones** — masses, radii, orbital distances, and periods
//!    checked against the satellite fact sheet in the units it quotes, so a mistyped exponent
//!    fails even though the scene it produces is self-consistent.
//! 2. **Nothing physical reads a drawn radius.** Every prop's contact radius is its *true*
//!    radius, and the drawn radius is `4×` larger. If those two were ever conflated again the
//!    moons would start inside Jupiter, which is the defect this experiment exists to have
//!    fixed.
//! 3. **The two crate changes are load-bearing, and the tests say why.** One test asserts that
//!    the *old* crate-wide softening would leave under `20%` of Newton's force at Io's orbit,
//!    and another that the old drawn radius would swallow the moons. Those two failures are the
//!    reason `Scene::softening_units` and `Prop::contact_radius_units` exist, and a test is
//!    where that reason survives.
//!
//! And one end-to-end: Io is run through the real integrator for one real orbital period and has
//! to come back to where it started, on the same circle.

use super::*;
use crate::core::constants::GRAVITY_SOFTENING_UNITS;
use crate::core::scene::Prop;
use crate::experiments::constants::SOLAR_SYSTEM_PLANETS;
use crate::time::constants::SECONDS_PER_DAY;

/// Index of Jupiter in the built scene: first, and there is no floor before it.
const JUPITER_INDEX: usize = 0;

fn built() -> Experiment {
    jovian_system()
}

fn moons_of(experiment: &Experiment) -> &[Prop] {
    &experiment.scene.props[JUPITER_INDEX + 1..]
}

fn drawn_radius(prop: &Prop) -> f32 {
    match &prop.body {
        Body::Sphere { radius, .. } => *radius,
        other => panic!("expected a sphere, got {other:?}"),
    }
}

// -----------------------------------------------------------------------
// The table, against published astronomy.
// -----------------------------------------------------------------------

#[test]
fn the_table_is_four_valid_moons_ordered_outward() {
    assert_eq!(JOVIAN_MOONS.len(), 4);

    for moon in JOVIAN_MOONS {
        assert!(
            moon.is_valid(),
            "{} is not a valid row: {moon:?}",
            moon.name
        );
    }
    for pair in JOVIAN_MOONS.windows(2) {
        assert!(
            pair[0].semi_major_axis_m < pair[1].semi_major_axis_m,
            "{} is not inside {}",
            pair[0].name,
            pair[1].name
        );
    }
}

#[test]
fn the_moons_are_the_galilean_four_in_order() {
    let names: Vec<&str> = JOVIAN_MOONS.iter().map(|moon| moon.name).collect();
    assert_eq!(names, vec!["Io", "Europa", "Ganymede", "Callisto"]);
}

#[test]
fn the_masses_and_orbits_are_the_published_ones() {
    // In the units the satellite fact sheet quotes: masses relative to the Moon's 7.35e22 kg
    // are the familiar comparison, and orbital distances in thousands of km.
    let expected = [
        ("Io", 8.932e22, 421_700.0),
        ("Europa", 4.800e22, 671_034.0),
        ("Ganymede", 1.4819e23, 1_070_412.0),
        ("Callisto", 1.0759e23, 1_882_700.0),
    ];

    for ((name, mass_kg, orbit_km), moon) in expected.into_iter().zip(JOVIAN_MOONS) {
        assert_eq!(name, moon.name);
        assert!(
            (moon.mass_kg / mass_kg - 1.0).abs() < 1.0e-3,
            "{name} has mass {}",
            moon.mass_kg
        );
        assert!(
            (moon.semi_major_axis_m / (orbit_km * 1000.0) - 1.0).abs() < 1.0e-3,
            "{name} orbits at {} m",
            moon.semi_major_axis_m
        );
    }
}

#[test]
fn ganymede_is_the_largest_moon_in_the_solar_system() {
    // A fact worth pinning because it is the one thing most people know about this system, and
    // it is a check on the radii: Ganymede is larger than Mercury.
    let ganymede = JOVIAN_MOONS[2];
    let mercury = SOLAR_SYSTEM_PLANETS[0];

    assert_eq!(ganymede.name, "Ganymede");
    assert!(
        ganymede.radius_m > mercury.radius_m,
        "Ganymede ({} m) should be larger than Mercury ({} m)",
        ganymede.radius_m,
        mercury.radius_m
    );
    assert!(
        JOVIAN_MOONS
            .iter()
            .all(|moon| moon.radius_m <= ganymede.radius_m),
        "and the largest of the four"
    );
}

#[test]
fn the_orbital_periods_are_the_real_days() {
    // The strongest single check that the distances and Jupiter's mass are both right, since the
    // period depends on the ratio between them.
    let expected_days = [
        ("Io", 1.769),
        ("Europa", 3.551),
        ("Ganymede", 7.155),
        ("Callisto", 16.689),
    ];

    for ((name, days), moon) in expected_days.into_iter().zip(JOVIAN_MOONS) {
        let got = jovian_orbit_period_seconds(moon.semi_major_axis_m) / SECONDS_PER_DAY;

        assert!(
            (got / days - 1.0).abs() < 0.01,
            "{name}'s period is {got} days, published {days}"
        );
    }
}

#[test]
fn the_laplace_resonance_falls_out_of_the_real_distances() {
    // Io : Europa : Ganymede are locked 1:2:4, and nothing here puts that in by hand — it is a
    // consequence of the published semi-major axes. If a distance were mistyped this is where it
    // would show, because the resonance is far more sensitive than any single figure.
    let period = |index: usize| jovian_orbit_period_seconds(JOVIAN_MOONS[index].semi_major_axis_m);

    let europa_io = period(1) / period(0);
    let ganymede_europa = period(2) / period(1);

    assert!(
        (europa_io - 2.0).abs() < 0.02,
        "Europa/Io = {europa_io}, expected ~2"
    );
    assert!(
        (ganymede_europa - 2.0).abs() < 0.02,
        "Ganymede/Europa = {ganymede_europa}, expected ~2"
    );
    assert!(
        (period(2) / period(0) - 4.0).abs() < 0.06,
        "Ganymede/Io = {}, expected ~4",
        period(2) / period(0)
    );

    // Callisto is deliberately *not* in the resonance, and that is real: its period ratio to
    // Ganymede is 2.33, not 2.
    assert!(
        (period(3) / period(2) - 2.0).abs() > 0.2,
        "Callisto is not part of the Laplace resonance"
    );
}

#[test]
fn jupiter_comes_from_the_solar_system_table() {
    // One home for Jupiter's mass and radius: `experiment_3` orbits it around the Sun and
    // `experiment_4` orbits moons around it, and they can never disagree.
    let primary = jovian_primary();

    assert_eq!(primary.name, "Jupiter");
    assert_eq!(primary.mass_kg, SOLAR_SYSTEM_PLANETS[4].mass_kg);
    assert!((primary.mass_kg / 1.8982e27 - 1.0).abs() < 1.0e-3);
}

// -----------------------------------------------------------------------
// Why this experiment needed two changes to the crate.
// -----------------------------------------------------------------------

#[test]
fn the_old_crate_wide_softening_would_have_made_these_orbits_impossible() {
    // The reason `Scene::softening_units` exists. Plummer softening evaluates the force at
    // sqrt(r² + ε²), leaving a fraction r²/(r² + ε²) of Newton — so the crate default of 1.0
    // unit leaves 15% at Io's orbit and puts the circular speed out by a factor of 2.6.
    let io_orbit_units = f64::from((JOVIAN_MOONS[0].semi_major_axis_m / METRES_PER_UNIT) as f32);
    let fraction = |epsilon: f64| {
        let r2 = io_orbit_units * io_orbit_units;
        r2 / (r2 + epsilon * epsilon)
    };

    let with_default = fraction(f64::from(GRAVITY_SOFTENING_UNITS));
    assert!(
        with_default < 0.20,
        "the crate default leaves {with_default} of Newton at Io's orbit — the docs say 15%"
    );

    let with_ours = fraction(f64::from(JOVIAN_SOFTENING_UNITS));
    assert!(
        with_ours > 0.9999,
        "this scene's softening leaves {with_ours}, which must be indistinguishable from Newton"
    );

    // And the scene really uses its own, not the default. Compared through the built scene
    // rather than constant-to-constant, which clippy rejects as an assertion with a constant
    // value — and which would anyway say nothing about what the experiment does.
    let scene = built().scene;
    assert_eq!(scene.softening_units, JOVIAN_SOFTENING_UNITS);
    assert!(
        scene.softening_units < GRAVITY_SOFTENING_UNITS,
        "the scene must soften less than the crate default, not more"
    );
}

#[test]
fn the_solar_systems_drawn_jupiter_would_have_swallowed_every_moon() {
    // The reason `Prop::contact_radius_units` exists. `experiment_3` draws Jupiter at 157x life
    // size so it is visible from Mars's orbit; that drawn body is 26x the radius of Io's entire
    // orbit, so with one radius all four moons would start inside the planet and be ejected on
    // the first step.
    let jupiter = jovian_primary();
    let drawn_in_solar_system = (jupiter.radius_m / METRES_PER_UNIT) as f32
        / (crate::experiments::constants::EARTH_RADIUS_M / METRES_PER_UNIT) as f32;
    let io_orbit = (JOVIAN_MOONS[0].semi_major_axis_m / METRES_PER_UNIT) as f32;
    let callisto_orbit = (JOVIAN_MOONS[3].semi_major_axis_m / METRES_PER_UNIT) as f32;

    assert!(
        drawn_in_solar_system > 25.0 * io_orbit,
        "the drawn Jupiter is {drawn_in_solar_system} units against Io's {io_orbit}-unit orbit"
    );
    assert!(
        drawn_in_solar_system > callisto_orbit,
        "it even contains Callisto's orbit"
    );

    // The largest exaggeration that keeps Jupiter inside Io's orbit, which the docs quote as
    // 6.03x — two orders of magnitude below what the planets need.
    let ceiling = io_orbit / (jupiter.radius_m / METRES_PER_UNIT) as f32;
    assert!(
        (ceiling - 6.03).abs() < 0.05,
        "the ceiling is {ceiling}x, and the docs say 6.03"
    );
    assert!(JOVIAN_DISPLAY_EXAGGERATION < ceiling);
}

// -----------------------------------------------------------------------
// Drawn versus physical.
// -----------------------------------------------------------------------

#[test]
fn every_prop_is_stored_drawn_and_collided_at_one_true_radius() {
    // Amendment A16 removed this experiment's 4x display exaggeration as well: with the renderer
    // flooring *apparent* size, there is nothing left for a scene-level exaggeration to do. So a
    // body has exactly one radius — its real one — and needs no `contact_radius_units` override
    // at all, which is the simplest possible statement of "the physics is real".
    let experiment = built();
    let jupiter = jovian_primary();

    let expected = [
        jupiter.radius_m,
        JOVIAN_MOONS[0].radius_m,
        JOVIAN_MOONS[1].radius_m,
        JOVIAN_MOONS[2].radius_m,
        JOVIAN_MOONS[3].radius_m,
    ];

    for (radius_m, prop) in expected.into_iter().zip(&experiment.scene.props) {
        let truth = (radius_m / METRES_PER_UNIT) as f32;

        assert!(
            (drawn_radius(prop) / truth - 1.0).abs() < 1.0e-6,
            "stored at {} against a true {truth}",
            drawn_radius(prop)
        );
        assert_eq!(
            prop.contact_radius_units, None,
            "no override is needed when the drawn size is already the true one"
        );
    }
}

#[test]
fn jupiter_is_ten_times_ganymede_because_nothing_is_scaled() {
    // The size ratios that used to be preserved *by* a uniform exaggeration now hold for the
    // trivial reason that the numbers are the measurements. Jupiter's radius is 26.5x Ganymede's.
    let experiment = built();
    let jupiter = drawn_radius(&experiment.scene.props[JUPITER_INDEX]);
    let ganymede = drawn_radius(&moons_of(&experiment)[2]);

    let expected = jovian_primary().radius_m / JOVIAN_MOONS[2].radius_m;
    assert!(
        (f64::from(jupiter / ganymede) / expected - 1.0).abs() < 1.0e-3,
        "Jupiter is {}x Ganymede, expected {expected}",
        jupiter / ganymede
    );
}

#[test]
fn nothing_visually_contains_anything() {
    // Free, now that nothing is exaggerated: Io orbits at 6.03 Jupiter radii, which is the real
    // figure. Before A16 this experiment drew every body 4x life size and this test was the upper
    // bound on that factor.
    let experiment = built();
    let jupiter_radius = drawn_radius(&experiment.scene.props[JUPITER_INDEX]);

    for (moon, prop) in JOVIAN_MOONS.into_iter().zip(moons_of(&experiment)) {
        let orbit = (moon.semi_major_axis_m / METRES_PER_UNIT) as f32;
        assert!(
            orbit - jupiter_radius - drawn_radius(prop) > 0.0,
            "{} is inside Jupiter",
            moon.name
        );
    }

    let io_in_jupiter_radii =
        (JOVIAN_MOONS[0].semi_major_axis_m / jovian_primary().radius_m) as f32;
    assert!(
        (io_in_jupiter_radii - 6.03).abs() < 0.05,
        "got {io_in_jupiter_radii}"
    );
}

#[test]
fn physically_nothing_is_remotely_close_to_touching() {
    // With true radii the margins are enormous — Jupiter's surface is 0.0699 units and Io orbits
    // at 0.4217 — which is why this experiment never exercises the contact solver at all.
    let experiment = built();
    let jupiter_radius = drawn_radius(&experiment.scene.props[JUPITER_INDEX]);

    for (moon, prop) in JOVIAN_MOONS.into_iter().zip(moons_of(&experiment)) {
        let orbit = (moon.semi_major_axis_m / METRES_PER_UNIT) as f32;
        let surfaces = jupiter_radius + drawn_radius(prop);

        assert!(
            orbit > 5.0 * surfaces,
            "{} orbits at {orbit} against combined surfaces of {surfaces}",
            moon.name
        );
    }
}

// -----------------------------------------------------------------------
// The initial condition.
// -----------------------------------------------------------------------

#[test]
fn every_moon_starts_at_its_real_distance_with_a_tangential_velocity() {
    let experiment = built();

    for (moon, prop) in JOVIAN_MOONS.into_iter().zip(moons_of(&experiment)) {
        let offset = prop.body.center();
        let expected = moon.semi_major_axis_m / METRES_PER_UNIT;

        assert!(
            (offset.length() / expected - 1.0).abs() < 1.0e-4,
            "{} is at {} units, expected {expected}",
            moon.name,
            offset.length()
        );
        assert_eq!(offset.y, 0.0, "{} is out of the orbital plane", moon.name);
        assert_eq!(prop.velocity_units_s.y, 0.0);

        let radial = offset.normalize();
        let direction = prop.velocity_units_s.normalize();
        assert!(
            direction.dot(radial).abs() < 1.0e-3,
            "{} has a radial velocity component",
            moon.name
        );
    }
}

#[test]
fn every_moon_moves_at_its_real_orbital_speed() {
    // Published mean orbital velocities, in km/s.
    let expected = [
        ("Io", 17.33),
        ("Europa", 13.74),
        ("Ganymede", 10.88),
        ("Callisto", 8.20),
    ];

    let experiment = built();

    for ((name, km_s), prop) in expected.into_iter().zip(moons_of(&experiment)) {
        let got = prop.speed_m_s() / 1000.0;

        assert!(
            (got / km_s - 1.0).abs() < 0.01,
            "{name} moves at {got} km/s, published {km_s}"
        );
    }
}

#[test]
fn every_moon_orbits_prograde() {
    let experiment = built();

    for (moon, prop) in JOVIAN_MOONS.into_iter().zip(moons_of(&experiment)) {
        let angular = prop.body.center().cross(prop.velocity_units_s);

        assert!(angular.y > 0.0, "{} orbits retrograde", moon.name);
    }
}

#[test]
fn the_moons_start_spread_and_not_symmetrically() {
    // The phases are arbitrary, so they may as well look like a moment in time rather than a
    // diagram: four moons at 90° intervals put two of them diametrically opposite, which reads
    // as a contrived alignment. No pair may be collinear with Jupiter, in either direction.
    let experiment = built();
    let directions: Vec<Vec3> = moons_of(&experiment)
        .iter()
        .map(|prop| prop.body.center().as_vec3().normalize())
        .collect();

    for (index, direction) in directions.iter().enumerate() {
        for other in &directions[index + 1..] {
            let alignment = direction.dot(*other);
            assert!(
                alignment.abs() < 0.99,
                "two moons start collinear with Jupiter: dot = {alignment}"
            );
        }
    }
}

#[test]
fn the_system_has_no_net_momentum() {
    // The barycentric frame, as `experiment_3` does for the Sun. Jupiter's recoil is 2.5e-9
    // units/s — tiny, free, and correct.
    let experiment = built();

    let total: Vec3 = experiment
        .scene
        .props
        .iter()
        .map(|prop| prop.velocity_units_s.as_vec3() * prop.mass_kg)
        .sum();
    let largest = experiment
        .scene
        .props
        .iter()
        .map(|prop| (prop.velocity_units_s.as_vec3() * prop.mass_kg).length())
        .fold(0.0_f32, f32::max);

    assert!(
        total.length() / largest < 1.0e-5,
        "net momentum is {total:?} against a largest term of {largest}"
    );
    assert_ne!(
        experiment.scene.props[JUPITER_INDEX].velocity_units_s,
        DVec3::ZERO,
        "Jupiter must carry the compensating momentum"
    );
}

// -----------------------------------------------------------------------
// The scene, and the spec.
// -----------------------------------------------------------------------

#[test]
fn the_scene_is_jupiter_and_four_moons_with_no_floor() {
    // The first scene in the crate with no container: a ground plane 5000 units across, ruled
    // every 5, would sit under a system 1.9 units wide and mean nothing.
    let experiment = built();

    assert_eq!(experiment.scene.props.len(), JOVIAN_MOONS.len() + 1);
    assert!(
        experiment.scene.props.iter().all(|prop| !prop.immovable),
        "nothing here is fixed geometry"
    );
    assert!(
        !experiment
            .scene
            .props
            .iter()
            .any(|prop| matches!(prop.body, Body::Plane { .. })),
        "and there is no floor"
    );
    assert!(
        crate::render::grid::floor_half_size(&experiment.scene).is_none(),
        "so the grid finds nothing to measure and spawns none"
    );
}

#[test]
fn the_bounds_contain_every_body() {
    let experiment = built();
    assert!(experiment.scene.bounds.is_valid());

    for prop in &experiment.scene.props {
        let centre = prop.body.center().as_vec3();
        let bounds = experiment.scene.bounds;

        assert!(
            centre.x >= bounds.min.x
                && centre.x <= bounds.max.x
                && centre.y >= bounds.min.y
                && centre.y <= bounds.max.y
                && centre.z >= bounds.min.z
                && centre.z <= bounds.max.z,
            "{centre:?} is outside {bounds:?}"
        );
    }
}

#[test]
fn the_spec_frames_the_whole_system() {
    let experiment = built();
    let spec = experiment.spec;

    assert_eq!(spec.id, ExperimentId::Jovian);
    assert!(spec.is_valid());
    assert_eq!(spec.time_scale, JOVIAN_TIME_SCALE);
    assert_eq!(spec.trail_length_units, Some(JOVIAN_TRAIL_LENGTH_UNITS));

    // Unlike the solar system, this really is the whole scene — every body is in the frame.
    let outermost = (JOVIAN_MOONS[3].semi_major_axis_m / METRES_PER_UNIT) as f32;
    assert!(
        spec.framed_radius_units > outermost,
        "the opening view must contain Callisto's orbit"
    );
}

#[test]
fn one_clock_scale_suits_every_body_here() {
    // What makes this system showable where the solar system is not: the periods span 9.4x, not
    // 685x, so a single time scale genuinely works for all four moons.
    let periods: Vec<f64> = JOVIAN_MOONS
        .iter()
        .map(|moon| jovian_orbit_period_seconds(moon.semi_major_axis_m))
        .collect();
    let spread = periods[3] / periods[0];

    assert!(spread < 12.0, "the periods span {spread}x");

    // And at the shipped scale every one of them is watchable: seconds, not hours.
    for (moon, period) in JOVIAN_MOONS.into_iter().zip(&periods) {
        let watching = period / f64::from(JOVIAN_TIME_SCALE);
        assert!(
            (10.0..300.0).contains(&watching),
            "{} takes {watching} s of watching",
            moon.name
        );
    }
}

#[test]
fn the_experiment_is_deterministic() {
    assert_eq!(
        ExperimentId::Jovian.build(1).scene,
        ExperimentId::Jovian.build(2).scene
    );
}

#[test]
fn a_nonsense_radius_yields_a_drawable_body_rather_than_a_nan() {
    #[cfg(not(debug_assertions))]
    for bad in [0.0, -1.0, f64::NAN] {
        let drawn = jovian_drawn_radius_units(bad);
        assert!(drawn.is_finite() && drawn > 0.0, "{bad} gave {drawn}");
    }

    assert!(jovian_drawn_radius_units(1.8216e6) > 0.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "radius must be finite and positive")]
fn a_zero_radius_panics_in_debug() {
    let _ = jovian_drawn_radius_units(0.0);
}

// -----------------------------------------------------------------------
// Does it actually orbit? The end-to-end check.
// -----------------------------------------------------------------------

#[test]
fn io_completes_one_orbit_in_one_period_and_comes_back() {
    // The claim that makes this an experiment rather than a diagram, and the test that would
    // have failed under either of the two limits this work removed: with the crate-wide
    // softening Io keeps only 15% of Newton's pull and spirals outward immediately, and with the
    // drawn radius as the contact radius it never gets a first step at all.
    use crate::time::simulation::SimulationClock;

    let Experiment {
        mut scene, spec, ..
    } = jovian_system();
    let mut clock = SimulationClock::new(spec.time_scale);

    let io_index = JUPITER_INDEX + 1;
    let start = scene.props[io_index].body.center();
    let start_radius = start.length();

    let period = jovian_orbit_period_seconds(JOVIAN_MOONS[0].semi_major_axis_m);
    let frame = 1.0 / 60.0;
    let frames = (period / (f64::from(spec.time_scale) * frame)).round() as usize;

    for _ in 0..frames {
        scene.step(clock.advance(frame as f32));
    }

    let end = scene.props[io_index].body.center() - scene.props[JUPITER_INDEX].body.center();

    assert!(
        (end.length() / start_radius - 1.0).abs() < 0.02,
        "Io's orbital radius drifted from {start_radius} to {}",
        end.length()
    );

    let cosine = end.normalize().dot(start.normalize()).clamp(-1.0, 1.0);
    let degrees_short = cosine.acos().to_degrees();
    assert!(
        degrees_short < 10.0,
        "after one period Io is {degrees_short}° from where it started"
    );
}

#[test]
fn all_four_moons_stay_on_their_circles_for_several_orbits() {
    // Ten Io periods is about 1.4 Ganymede periods and 0.6 of Callisto's, so this exercises the
    // whole system rather than just the fastest body. Radii are checked rather than positions:
    // where a moon *is* after ten orbits is a statement about the integrator's phase error,
    // which this crate does not pin, but whether it is still on its orbit is a statement about
    // the physics.
    use crate::time::simulation::SimulationClock;

    let Experiment {
        mut scene, spec, ..
    } = jovian_system();
    let mut clock = SimulationClock::new(spec.time_scale);

    let starting_radii: Vec<f64> = scene.props[1..]
        .iter()
        .map(|prop| prop.body.center().length())
        .collect();

    let ten_io_periods = 10.0 * jovian_orbit_period_seconds(JOVIAN_MOONS[0].semi_major_axis_m);
    let frame = 1.0 / 60.0;
    let frames = (ten_io_periods / (f64::from(spec.time_scale) * frame)).round() as usize;

    for _ in 0..frames {
        scene.step(clock.advance(frame as f32));
    }

    let jupiter = scene.props[JUPITER_INDEX].body.center();

    for ((moon, prop), started) in JOVIAN_MOONS
        .into_iter()
        .zip(&scene.props[1..])
        .zip(starting_radii)
    {
        let radius = (prop.body.center() - jupiter).length();

        assert!(
            (radius / started - 1.0).abs() < 0.05,
            "{} moved from {started} units to {radius}",
            moon.name
        );
        assert!(prop.is_valid());
    }

    // Nothing was absorbed or removed along the way.
    assert_eq!(scene.props.len(), JOVIAN_MOONS.len() + 1);
}
