//! Unit tests for the [`crate::experiments::solar_system`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/experiments/solar_system.rs`
//! via `#[path]`, not as a standalone integration test.
//!
//! # What these tests are for
//!
//! This experiment's whole claim is that the numbers are **real**, so most of what follows
//! checks the scene against published astronomy rather than against itself: orbital speeds
//! in km/s, periods in years, size ratios against Earth. A test that only compared the
//! scene to the constants that built it would pass just as happily with the mass of the
//! Sun mistyped.
//!
//! Since amendment A16 there are **no exceptions left to pin**: every body is stored, drawn, and
//! collided at its true size, and visibility is the renderer's job. So the tests that used to
//! guard two different exaggerations now assert the opposite — that nothing is scaled, that the
//! Sun is correctly proportioned against Jupiter, and that the Sun no longer reaches any planet's
//! orbit. Each names the defect it replaced, because "there is no longer a lie here" is only
//! meaningful if the lie is on the record.

use super::*;
use crate::experiments::constants::{
    EARTH_MOONS, EARTH_RADIUS_M, JOVIAN_MOONS, MOON_SYSTEMS, SOLAR_SYSTEM_SOFTENING_UNITS,
    SOLAR_SYSTEM_SUN_RADIUS_M, SOLAR_SYSTEM_TRAIL_LENGTH_UNITS,
};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::time::constants::SECONDS_PER_DAY;

/// Index of the Sun in the built scene: after the floor, before the planets.
const SUN_INDEX: usize = 1;
/// Index of the first planet (Mercury).
const FIRST_PLANET_INDEX: usize = 2;

/// The scene's props, and the spec, for a freshly built solar system.
fn built() -> Experiment {
    solar_system()
}

/// The Sun's prop.
fn sun(experiment: &Experiment) -> &Prop {
    &experiment.scene.props[SUN_INDEX]
}

/// The eight planet props, in table order.
///
/// Bounded explicitly rather than "everything after the Sun", because the scene also carries the
/// moons — and a moon lies in almost exactly the same direction from the Sun as its primary does,
/// which broke the "no two planets start on the same line" test the first time the moons went in.
fn planets(experiment: &Experiment) -> &[Prop] {
    &experiment.scene.props[FIRST_PLANET_INDEX..FIRST_PLANET_INDEX + SOLAR_SYSTEM_PLANETS.len()]
}

/// The prop for a named planet, found through the table rather than by a literal index.
fn planet<'a>(experiment: &'a Experiment, name: &str) -> &'a Prop {
    let row = SOLAR_SYSTEM_PLANETS
        .iter()
        .position(|planet| planet.name == name)
        .unwrap_or_else(|| panic!("no planet named {name}"));

    &planets(experiment)[row]
}

/// The moon props of one system, in table order.
///
/// The offset is computed by walking [`MOON_SYSTEMS`] rather than written as a literal, so adding a
/// system cannot silently shift another one's slice. That is not hypothetical: the Jupiter-only
/// version of this helper was `props[..]` to the end, and adding Earth's Moon put a fifth prop in
/// the slice the Jovian tests were reading.
fn moons_of<'a>(experiment: &'a Experiment, primary: &str) -> &'a [Prop] {
    let mut start = FIRST_PLANET_INDEX + SOLAR_SYSTEM_PLANETS.len();

    for system in MOON_SYSTEMS {
        if system.primary == primary {
            return &experiment.scene.props[start..start + system.moons.len()];
        }
        start += system.moons.len();
    }

    panic!("no moon system for {primary}");
}

/// A planet's radius as drawn, pulled out of its `Body::Sphere`.
fn sphere_radius(prop: &Prop) -> f32 {
    match &prop.body {
        Body::Sphere { radius, .. } => *radius,
        other => panic!("expected a sphere, got {other:?}"),
    }
}

// -----------------------------------------------------------------------
// The data table itself.
// -----------------------------------------------------------------------

#[test]
fn the_table_is_eight_valid_planets() {
    assert_eq!(SOLAR_SYSTEM_PLANETS.len(), 8);

    for planet in SOLAR_SYSTEM_PLANETS {
        assert!(
            planet.is_valid(),
            "{} is not a valid row: {planet:?}",
            planet.name
        );
    }
}

#[test]
fn the_table_runs_outward_from_the_sun() {
    // Catalog order is display order — `--list` and the overlay both read it — and an
    // out-of-order row would make "the outermost planet" wrong wherever it is assumed.
    for pair in SOLAR_SYSTEM_PLANETS.windows(2) {
        assert!(
            pair[0].semi_major_axis_m < pair[1].semi_major_axis_m,
            "{} is not inside {}",
            pair[0].name,
            pair[1].name
        );
    }
}

#[test]
fn no_two_rows_name_the_same_planet() {
    for (index, planet) in SOLAR_SYSTEM_PLANETS.iter().enumerate() {
        for other in &SOLAR_SYSTEM_PLANETS[index + 1..] {
            assert_ne!(planet.name, other.name, "{} appears twice", planet.name);
        }
    }
}

#[test]
fn the_masses_and_distances_are_the_published_ones() {
    // Spot checks against the planetary fact sheet, in the units it publishes: a typo in
    // an exponent here is the kind of error that produces a plausible-looking scene with
    // completely wrong physics.
    let by_name = |name: &str| {
        SOLAR_SYSTEM_PLANETS
            .iter()
            .find(|planet| planet.name == name)
            .copied()
            .unwrap_or_else(|| panic!("{name} is missing from the table"))
    };

    let earth = by_name("Earth");
    assert!((earth.mass_kg - 5.972e24).abs() < 1.0e21, "Earth's mass");
    assert!(
        (earth.semi_major_axis_m / 1.495_979e11 - 1.0).abs() < 1.0e-3,
        "Earth is one astronomical unit out"
    );

    let jupiter = by_name("Jupiter");
    assert!(
        (jupiter.mass_kg / earth.mass_kg - 317.8).abs() < 1.0,
        "Jupiter is 318 Earth masses"
    );

    let neptune = by_name("Neptune");
    assert!(
        (neptune.semi_major_axis_m / earth.semi_major_axis_m - 30.05).abs() < 0.05,
        "Neptune is 30 AU out"
    );
}

// -----------------------------------------------------------------------
// Size: true throughout, since amendment A16.
// -----------------------------------------------------------------------

#[test]
fn every_body_is_stored_at_its_true_size() {
    // Amendment A16: there is no size exaggeration anywhere any more. `radius_units` is a unit
    // conversion and nothing else, so the Earth is the 0.0064 units it really is.
    assert!((radius_units(EARTH_RADIUS_M) - 0.006371).abs() < 1.0e-9);

    let experiment = built();
    for (planet, prop) in SOLAR_SYSTEM_PLANETS.into_iter().zip(planets(&experiment)) {
        let stored = sphere_radius(prop);
        let truth = (planet.radius_m / crate::constants::METRES_PER_UNIT) as f32;

        assert!(
            (stored / truth - 1.0).abs() < 1.0e-6,
            "{} is stored at {stored} units against a true {truth}",
            planet.name
        );
    }
}

#[test]
fn every_size_ratio_is_real_because_nothing_is_scaled() {
    // These are the published mean-radius ratios to Earth. They were preserved before A16 too,
    // because the exaggeration was uniform — but now they hold for the trivial reason that the
    // stored numbers *are* the measurements.
    let expected: [(&str, f32); 8] = [
        ("Mercury", 0.383),
        ("Venus", 0.950),
        ("Earth", 1.000),
        ("Mars", 0.532),
        ("Jupiter", 10.973),
        ("Saturn", 9.140),
        ("Uranus", 3.981),
        ("Neptune", 3.865),
    ];

    let experiment = built();
    let earth = sphere_radius(&planets(&experiment)[2]);

    for ((name, ratio), prop) in expected.into_iter().zip(planets(&experiment)) {
        let got = sphere_radius(prop) / earth;
        assert!(
            (got / ratio - 1.0).abs() < 1.0e-3,
            "{name} is {got} Earth radii, expected {ratio}"
        );
    }
}

#[test]
fn the_sun_is_now_correctly_proportioned_against_jupiter() {
    // The defect A16 removed. The Sun's true radius is 109.2 Earth radii and Jupiter's is 10.97,
    // so the Sun should be 9.95x Jupiter. Under the old 157x exaggeration the Sun had to be
    // *capped* at 20 units — because at 109 units it swallowed Mercury's orbit — which left
    // Jupiter looking 5.5x too large beside it. With true sizes there is nothing to cap.
    let experiment = built();
    let sun = sphere_radius(sun(&experiment));
    let jupiter = sphere_radius(&planets(&experiment)[4]);

    assert!(
        (sun / jupiter - 9.95).abs() < 0.05,
        "the Sun is {}x Jupiter, and the true ratio is 9.95",
        sun / jupiter
    );
    assert!(
        (sun - 0.6957).abs() < 1.0e-3,
        "and it is stored at its true 0.6957 units, not a capped 20"
    );
}

#[test]
fn the_sun_no_longer_reaches_any_planets_orbit() {
    // The reason the old cap existed: on the 157x scale the Sun was 109.2 units, larger than
    // Mercury's whole 57.9-unit orbit, so the four terrestrial planets started inside it. At its
    // true 0.6957 units it clears Mercury's perihelion by a factor of 66.
    let experiment = built();
    let sun_radius = sphere_radius(sun(&experiment));
    let mercury_perihelion = (4.600e10 / crate::constants::METRES_PER_UNIT) as f32;

    assert!(
        mercury_perihelion / sun_radius > 60.0,
        "the Sun is {sun_radius} units against a {mercury_perihelion}-unit perihelion"
    );

    // And what the old exaggeration would have made it, for the record.
    let old_scale =
        (crate::experiments::constants::SOLAR_SYSTEM_SUN_RADIUS_M / EARTH_RADIUS_M) as f32;
    assert!(
        old_scale > mercury_perihelion,
        "the 157x Sun ({old_scale} units) really did contain Mercury's orbit"
    );
}

#[test]
fn a_nonsense_radius_yields_zero_rather_than_a_nan() {
    // Release behaviour: zero, which `Body::is_valid` then rejects. That is deliberate — with no
    // exaggeration left to hide inside, a bad measurement should fail validation loudly rather
    // than be silently replaced by an Earth-sized stand-in.
    #[cfg(not(debug_assertions))]
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(radius_units(bad), 0.0, "{bad} should give zero");
    }

    assert!(radius_units(EARTH_RADIUS_M) > 0.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "radius must be finite and positive")]
fn a_zero_radius_panics_in_debug() {
    let _ = radius_units(0.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "central mass must be finite and positive")]
fn a_zero_central_mass_panics_in_debug() {
    let _ = circular_orbit_speed_units_s(1.0e11, 0.0);
}

// -----------------------------------------------------------------------
// Distances and speeds: checked against published astronomy.
// -----------------------------------------------------------------------

#[test]
fn every_planet_starts_at_its_real_distance() {
    let experiment = built();
    let centre = DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS);

    for (planet, prop) in SOLAR_SYSTEM_PLANETS.into_iter().zip(planets(&experiment)) {
        let expected = planet.semi_major_axis_m / crate::constants::METRES_PER_UNIT;
        let actual = (prop.body.center() - centre).length();

        assert!(
            (actual / expected - 1.0).abs() < 1.0e-4,
            "{} is at {actual} units, expected {expected}",
            planet.name
        );
    }
}

#[test]
fn every_planet_moves_at_its_real_orbital_speed() {
    // Published mean orbital velocities, in km/s. The circular-orbit speed at the mean
    // distance is within a percent of these for every planet — Mercury is the worst, at
    // 1.1%, because its real orbit is the most eccentric.
    let expected_km_s: [(&str, f64); 8] = [
        ("Mercury", 47.36),
        ("Venus", 35.02),
        ("Earth", 29.78),
        ("Mars", 24.07),
        ("Jupiter", 13.06),
        ("Saturn", 9.68),
        ("Uranus", 6.80),
        ("Neptune", 5.43),
    ];

    let experiment = built();

    for ((name, published), prop) in expected_km_s.into_iter().zip(planets(&experiment)) {
        let km_s = prop.speed_m_s() / 1000.0;

        assert!(
            (km_s / published - 1.0).abs() < 0.02,
            "{name} moves at {km_s} km/s, published {published}"
        );
    }
}

#[test]
fn the_orbital_periods_are_the_real_years() {
    // Kepler's third law, checked against the years people know: this is the strongest
    // single statement that the distances and the Sun's mass are both right, since the
    // period depends on the ratio between them.
    let expected_days: [(&str, f64); 8] = [
        ("Mercury", 87.97),
        ("Venus", 224.70),
        ("Earth", 365.26),
        ("Mars", 686.98),
        ("Jupiter", 4332.6),
        ("Saturn", 10_759.0),
        ("Uranus", 30_688.5),
        ("Neptune", 60_182.0),
    ];

    for ((name, days), planet) in expected_days.into_iter().zip(SOLAR_SYSTEM_PLANETS) {
        let period_days =
            circular_orbit_period_seconds(planet.semi_major_axis_m, SOLAR_SYSTEM_SUN_MASS_KG)
                / SECONDS_PER_DAY;

        assert!(
            (period_days / days - 1.0).abs() < 0.02,
            "{name}'s year is {period_days} days, published {days}"
        );
    }
}

#[test]
fn the_speed_formula_is_the_textbook_one() {
    // v = sqrt(GM/r), independently evaluated: guards the units conversion, which is the
    // only part of this that is easy to get wrong by a factor of a billion.
    let radius_m = 1.4960e11;
    let expected_m_s =
        (GRAVITATIONAL_CONSTANT * f64::from(SOLAR_SYSTEM_SUN_MASS_KG) / radius_m).sqrt();
    let got_m_s = circular_orbit_speed_units_s(radius_m, SOLAR_SYSTEM_SUN_MASS_KG)
        * crate::constants::METRES_PER_UNIT;

    assert!((got_m_s / expected_m_s - 1.0).abs() < 1.0e-6);
}

#[test]
fn a_nonsense_orbit_yields_no_speed_rather_than_a_nan() {
    // Release-only for the same reason as `a_nonsense_radius_...`: in debug these inputs
    // are contract violations and fire a `debug_assert!`.
    #[cfg(not(debug_assertions))]
    for (radius, mass) in [
        (0.0, 1.989e30_f32),
        (-1.0, 1.989e30),
        (f64::NAN, 1.989e30),
        (1.0e11, 0.0),
        (1.0e11, f32::NAN),
    ] {
        let speed = circular_orbit_speed_units_s(radius, mass);
        assert_eq!(speed, 0.0, "({radius}, {mass}) gave {speed}");
        assert_eq!(circular_orbit_period_seconds(radius, mass), 0.0);
    }

    assert!(circular_orbit_speed_units_s(1.0e11, SOLAR_SYSTEM_SUN_MASS_KG) > 0.0);
    assert!(circular_orbit_period_seconds(1.0e11, SOLAR_SYSTEM_SUN_MASS_KG) > 0.0);
}

// -----------------------------------------------------------------------
// The initial condition: tangential, coplanar, prograde, and momentum-free.
// -----------------------------------------------------------------------

#[test]
fn every_velocity_is_exactly_tangential_and_in_the_ecliptic() {
    let experiment = built();
    let centre = DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS);

    for (planet, prop) in SOLAR_SYSTEM_PLANETS.into_iter().zip(planets(&experiment)) {
        let offset = prop.body.center() - centre;

        assert_eq!(
            offset.y, 0.0,
            "{} is out of the ecliptic plane",
            planet.name
        );
        assert_eq!(
            prop.velocity_units_s.y, 0.0,
            "{} has a vertical velocity",
            planet.name
        );

        let radial = offset.normalize();
        let direction = prop.velocity_units_s.normalize();

        assert!(
            direction.dot(radial).abs() < 1.0e-3,
            "{} has a radial velocity component: {}",
            planet.name,
            direction.dot(radial)
        );
    }
}

#[test]
fn every_planet_orbits_prograde() {
    // All eight go the same way round — counterclockwise seen from +Y, as the real ones do
    // seen from ecliptic north. Angular momentum `r × v` therefore points along +Y for
    // every planet; one row with the sign flipped would show up here and nowhere else.
    let experiment = built();
    let centre = DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS);

    for (planet, prop) in SOLAR_SYSTEM_PLANETS.into_iter().zip(planets(&experiment)) {
        let angular = (prop.body.center() - centre).cross(prop.velocity_units_s);

        assert!(
            angular.y > 0.0,
            "{} orbits retrograde: L = {angular:?}",
            planet.name
        );
    }
}

#[test]
fn the_planets_do_not_all_start_in_a_line() {
    // Real J2000 longitudes, so the arrangement is the real one — and a lineup would give
    // the system a large spurious momentum in one direction.
    let experiment = built();
    let centre = DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS);

    let directions: Vec<DVec3> = planets(&experiment)
        .iter()
        .map(|prop| (prop.body.center() - centre).normalize())
        .collect();

    let mut max_alignment = 0.0_f64;
    for (index, direction) in directions.iter().enumerate() {
        for other in &directions[index + 1..] {
            max_alignment = max_alignment.max(direction.dot(*other).abs());
        }
    }

    assert!(
        max_alignment < 0.999,
        "some pair of planets starts on the same line: {max_alignment}"
    );
}

#[test]
fn the_system_has_no_net_momentum() {
    // The barycentric frame. Without the Sun's compensating velocity the total would be
    // ~3e31 kg·units/s and the whole system would drift out of frame; see the module docs.
    let experiment = built();

    let total: DVec3 = experiment
        .scene
        .props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(|prop| prop.velocity_units_s * f64::from(prop.mass_kg))
        .sum();

    // Scaled against the largest single momentum, because comparing a cancelled sum to an
    // absolute tolerance says more about `f32`'s exponent than about the physics.
    let largest = experiment
        .scene
        .props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(|prop| (prop.velocity_units_s * f64::from(prop.mass_kg)).length())
        .fold(0.0_f64, f64::max);

    assert!(
        total.length() / largest < 1.0e-5,
        "net momentum is {total:?} against a largest term of {largest}"
    );
}

#[test]
fn the_sun_carries_the_compensating_momentum() {
    // Not at rest, and not arbitrary: its velocity is the planets' momentum divided by its
    // own mass, which for the real solar system is about 15 m/s. That wobble is the signal
    // real radial-velocity exoplanet searches measure, and it is visible in the Sun's trail.
    let experiment = built();
    let velocity = sun(&experiment).velocity_units_s;
    let m_s = sun(&experiment).speed_m_s();

    assert_ne!(velocity, DVec3::ZERO, "the Sun must not start at rest");
    assert_eq!(velocity.y, 0.0, "the Sun stays in the ecliptic plane");
    assert!(
        (5.0..30.0).contains(&m_s),
        "the Sun's barycentric speed is {m_s} m/s, expected ~15"
    );
}

// -----------------------------------------------------------------------
// The scene as a whole.
// -----------------------------------------------------------------------

#[test]
fn the_scene_is_a_floor_a_sun_eight_planets_and_five_moons() {
    let experiment = built();

    let moon_count: usize = MOON_SYSTEMS.iter().map(|system| system.moons.len()).sum();

    // Earth's one and Jupiter's four. Summed from the table rather than written as `5`, so a new
    // system is covered by this test the moment it is added.
    assert_eq!(moon_count, EARTH_MOONS.len() + JOVIAN_MOONS.len());
    assert_eq!(
        experiment.scene.props.len(),
        SOLAR_SYSTEM_PLANETS.len() + moon_count + 2
    );
    assert!(
        experiment.scene.props[0].immovable,
        "prop 0 is the shared floor"
    );
    assert!(
        (sphere_radius(sun(&experiment)) - radius_units(SOLAR_SYSTEM_SUN_RADIUS_M)).abs() < 1.0e-9,
        "prop 1 is the Sun, at its true radius"
    );

    for prop in &experiment.scene.props[FIRST_PLANET_INDEX..] {
        assert!(prop.is_valid());
        assert!(
            prop.responds_to_forces(),
            "every planet and moon must be movable and massive"
        );
    }
}

#[test]
fn every_moon_orbits_its_primary_and_not_the_sun() {
    // The initial condition that makes them moons rather than five extra planets: each one's
    // heliocentric velocity is **its primary's plus** its orbital velocity about that primary.
    // Omitting the first term would leave them nearly at rest relative to the Sun and they would
    // fall in.
    //
    // Loops over `MOON_SYSTEMS`, so Earth's Moon is checked by the same arithmetic that checks the
    // Galilean four and a new system needs no new test.
    let experiment = built();

    for system in MOON_SYSTEMS {
        let primary_row = SOLAR_SYSTEM_PLANETS
            .iter()
            .find(|planet| planet.name == system.primary)
            .unwrap_or_else(|| panic!("{} is not a planet", system.primary));
        let primary = planet(&experiment, system.primary);
        let moons = moons_of(&experiment, system.primary);

        assert_eq!(moons.len(), system.moons.len());

        for (moon, prop) in system.moons.iter().zip(moons) {
            check_moon(moon, prop, primary, primary_row.mass_kg);
        }
    }
}

/// One moon's initial condition against its primary's: distance, tangency, speed, and carry.
///
/// Split out so the loop above reads as the claim rather than as the arithmetic, and so both the
/// Earth and Jupiter systems are held to *identical* assertions — a copy per system is how the two
/// stop being the same test.
fn check_moon(
    moon: &crate::experiments::solar_system::Planet,
    prop: &Prop,
    primary: &Prop,
    primary_mass_kg: f64,
) {
    {
        let jupiter = primary;
        let offset = prop.body.center() - jupiter.body.center();
        let expected = moon.semi_major_axis_m / crate::constants::METRES_PER_UNIT;

        assert!(
            (offset.length() / expected - 1.0).abs() < 1.0e-3,
            "{} is {} units from Jupiter, expected {expected}",
            moon.name,
            offset.length()
        );

        // Relative to Jupiter the velocity is tangential, at the circular speed about *Jupiter* —
        // not about the Sun.
        let relative = prop.velocity_units_s - jupiter.velocity_units_s;
        assert!(
            relative.normalize().dot(offset.normalize()).abs() < 1.0e-2,
            "{}'s velocity relative to Jupiter is not tangential",
            moon.name
        );

        let expected_speed = crate::experiments::solar_system::circular_orbit_speed_units_s(
            moon.semi_major_axis_m,
            primary_mass_kg as f32,
        );
        assert!(
            (relative.length() / expected_speed - 1.0).abs() < 1.0e-2,
            "{} moves at {} about Jupiter, expected {expected_speed}",
            moon.name,
            relative.length()
        );

        // And it is carried along by Jupiter: its heliocentric speed is dominated by Jupiter's.
        assert!(
            prop.velocity_units_s.length() > 0.3 * jupiter.velocity_units_s.length(),
            "{} should share its primary's orbital motion",
            moon.name
        );
    }
}

#[test]
fn nothing_overlaps_anything_at_true_scale() {
    // The property that makes moons possible here at all: Jupiter is 0.0699 units and Io orbits at
    // 0.4217, so the planet's hull is 6.03 Io-orbit-radii *inside* the moon's path. Under the old
    // 157x drawn radius it was 26x the orbit and all four moons started inside the planet — and
    // Earth's hull was 2.6x its Moon's entire orbit.
    let experiment = built();

    for system in MOON_SYSTEMS {
        let primary = planet(&experiment, system.primary);
        let primary_radius = sphere_radius(primary);

        for (moon, prop) in system
            .moons
            .iter()
            .zip(moons_of(&experiment, system.primary))
        {
            let orbit = (prop.body.center() - primary.body.center()).length();
            let clearance = orbit - f64::from(primary_radius) - f64::from(sphere_radius(prop));

            assert!(
                clearance > 0.0,
                "{} starts inside {}: {clearance} units of clearance",
                moon.name,
                system.primary
            );
        }
    }

    // Both figures against published astronomy rather than against the scene: Io at 6.03 Jupiter
    // radii and the Moon at 60.3 Earth radii. The second is the one worth stating — a body orbiting
    // at sixty times its primary's radius is why the Earth-Moon system is drawable at true scale
    // when nothing else in the crate was.
    let io_in_jupiter_radii =
        (JOVIAN_MOONS[0].semi_major_axis_m / SOLAR_SYSTEM_PLANETS[4].radius_m) as f32;
    assert!(
        (io_in_jupiter_radii - 6.03).abs() < 0.05,
        "Io should orbit at the real 6.03 Jupiter radii, got {io_in_jupiter_radii}"
    );

    let moon_in_earth_radii = (EARTH_MOONS[0].semi_major_axis_m / EARTH_RADIUS_M) as f32;
    assert!(
        (moon_in_earth_radii - 60.3).abs() < 0.1,
        "the Moon should orbit at the real 60.3 Earth radii, got {moon_in_earth_radii}"
    );
}

#[test]
fn earths_moon_matches_published_figures() {
    // Against sources, not against the scene: a mistyped exponent in the table would still produce
    // a self-consistent orbit.
    let moon = EARTH_MOONS[0];

    assert!((moon.mass_kg / 7.342e22 - 1.0).abs() < 1.0e-3, "mass");
    assert!((moon.radius_m / 1.7374e6 - 1.0).abs() < 1.0e-3, "radius");
    assert!(
        (moon.semi_major_axis_m / 3.844e8 - 1.0).abs() < 1.0e-3,
        "semi-major axis"
    );

    // The Moon is 27.3% of Earth's radius and 1.23% of its mass — the largest moon-to-planet size
    // ratio in the solar system, which is the reason it is worth having in the scene at all.
    assert!(
        (moon.radius_m / EARTH_RADIUS_M - 0.273).abs() < 0.005,
        "radius ratio to Earth"
    );
    assert!(
        (moon.mass_kg / SOLAR_SYSTEM_PLANETS[2].mass_kg - 0.0123).abs() < 5.0e-4,
        "mass ratio to Earth"
    );

    // The derived orbit, against the published 27.32 days and 1.022 km/s. Circular where the real
    // one is eccentric, so the tolerance is a percent rather than a part in a thousand.
    let period = crate::experiments::solar_system::circular_orbit_period_seconds(
        moon.semi_major_axis_m,
        SOLAR_SYSTEM_PLANETS[2].mass_kg as f32,
    );
    assert!(
        (period / 86_400.0 / 27.32 - 1.0).abs() < 0.01,
        "sidereal period came out {} days",
        period / 86_400.0
    );

    let speed = crate::experiments::solar_system::circular_orbit_speed_units_s(
        moon.semi_major_axis_m,
        SOLAR_SYSTEM_PLANETS[2].mass_kg as f32,
    ) * crate::constants::METRES_PER_UNIT;
    assert!(
        (speed / 1022.0 - 1.0).abs() < 0.01,
        "orbital speed came out {speed} m/s"
    );
}

#[test]
fn the_moon_is_just_too_small_to_cover_the_sun() {
    // The claim `solar_system.rs` makes: with the real numbers the Moon subtends slightly *less*
    // than the Sun from Earth, so an eclipse here is annular rather than total. Checked against the
    // published mean ratio of 0.9716 and then put through the shipped shadow arithmetic, because
    // "the geometry is right" and "the renderer draws it that way" are two claims.
    let sun_angle = (SOLAR_SYSTEM_SUN_RADIUS_M / SOLAR_SYSTEM_PLANETS[2].semi_major_axis_m).asin();
    let moon_angle = (EARTH_MOONS[0].radius_m / EARTH_MOONS[0].semi_major_axis_m).asin();
    let ratio = moon_angle / sun_angle;

    assert!(
        (ratio - 0.9716).abs() < 0.005,
        "the Moon-to-Sun angular ratio should be the published 0.9716, got {ratio}"
    );
    assert!(
        ratio < 1.0,
        "a ratio at or above 1 would make eclipses total; annular ones exist because it is below"
    );

    // Perfectly aligned: Earth's surface, the Moon at its true distance, the Sun beyond it.
    let metres_per_unit = crate::constants::METRES_PER_UNIT as f32;
    let point = Vec3::new(0.0, 0.0, (EARTH_RADIUS_M as f32) / metres_per_unit);
    let moon_centre = Vec3::new(
        0.0,
        0.0,
        (EARTH_MOONS[0].semi_major_axis_m as f32) / metres_per_unit,
    );
    let sun_centre = Vec3::new(
        0.0,
        0.0,
        (SOLAR_SYSTEM_PLANETS[2].semi_major_axis_m as f32) / metres_per_unit,
    );

    let visible = crate::render::eclipse::star_visible_fraction(
        point,
        sun_centre,
        radius_units(SOLAR_SYSTEM_SUN_RADIUS_M),
        moon_centre,
        radius_units(EARTH_MOONS[0].radius_m),
    );

    // `1 - ratio^2` — the annular case. The point is on Earth's *surface*, one Earth radius closer to
    // the Moon than the geocentric ratio above assumes, which is 1.7% of the Moon's orbit and makes it
    // look 1.7% bigger: 97.7% of the Sun covered rather than 94.5%. That is real, not a discrepancy —
    // being on the surface is why total eclipses are possible at all.
    assert!(
        visible > 0.0,
        "an annular eclipse is not total; a 0 here means the annular case was skipped"
    );
    assert!(
        (visible - 0.023).abs() < 0.01,
        "expected about 2.3% of the Sun still visible from the surface, got {}%",
        visible * 100.0
    );
}

#[test]
fn the_moons_force_the_scene_to_substep_and_it_is_affordable() {
    // The reason the moons were removed once and the reason they are back. One frame at this clock is
    // 10.9% of Io's whole orbit, so the displacement-based substep rule alone would give 3 substeps
    // where ~900 integration steps per orbit are needed. The dynamical-time criterion asks for ~98,
    // which is 3.2 ms a frame in release — a fifth of a 60 Hz budget, measured, not assumed.
    use crate::collision::constants::MAX_SUBSTEPS;
    use crate::collision::resolve::substep_count;
    use crate::time::simulation::SimulationClock;

    let Experiment { scene, spec, .. } = built();
    let mut clock = SimulationClock::new(spec.time_scale);
    let step = clock.advance(1.0 / 60.0);

    let substeps = substep_count(&scene.props, step);

    assert!(
        (80..140).contains(&substeps),
        "the moons should ask for around 98 substeps, got {substeps}"
    );
    assert!(
        substeps < MAX_SUBSTEPS,
        "and the ceiling must not be silently capping it: {substeps} against {MAX_SUBSTEPS}"
    );

    // Without the moons the same scene needs 4 — and those 4 come from the *displacement* rule, not
    // the dynamical one: Mercury covers 0.798 units a frame against a 0.25-unit cap. So the moons
    // account for the other 94, and a scene with no tight orbit in it pays nothing for them.
    //
    // This assertion found a bug in `orbital_substeps`. It read 6, because the immovable, massless
    // floor sits 30 units below a 1.989e30 kg Sun and the pair was not being skipped — two props
    // that cannot interact were setting the timestep for the whole scene.
    let planets_only: Vec<_> =
        scene.props[..FIRST_PLANET_INDEX + SOLAR_SYSTEM_PLANETS.len()].to_vec();
    let without_moons = substep_count(&planets_only, step);

    assert_eq!(
        without_moons, 4,
        "the planets alone are displacement-bound, not orbit-bound"
    );
    assert!(
        substeps > 20 * without_moons,
        "the moons are what make this scene expensive: {substeps} against {without_moons}"
    );
}

#[test]
fn the_scene_softens_far_below_the_size_of_its_smallest_body() {
    // Softening models a body's *finite size*, so it has to be smaller than the bodies. Now that
    // radii are true, the largest body in the scene is the Sun at 0.696 units — so the crate-wide
    // default of 1.0 would be larger than the Sun itself, which is why this scene sets its own.
    let scene = built().scene;

    assert_eq!(scene.softening_units, SOLAR_SYSTEM_SOFTENING_UNITS);
    assert!(
        scene.softening_units < radius_units(SOLAR_SYSTEM_SUN_RADIUS_M),
        "epsilon must be smaller than the body it stands in for"
    );
    assert!(
        crate::core::constants::GRAVITY_SOFTENING_UNITS > radius_units(SOLAR_SYSTEM_SUN_RADIUS_M),
        "and the default is not: it exceeds the Sun's true radius"
    );
}

#[test]
fn nothing_starts_touching_anything() {
    // The generic version of this runs over the whole catalog in `catalog_tests`; this one
    // says *how much* clearance there is, because the Sun's drawn radius is the one number
    // in the scene that could quietly eat a planet.
    let experiment = built();
    let centre = DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS);
    let sun_radius = sphere_radius(sun(&experiment));

    for (planet, prop) in SOLAR_SYSTEM_PLANETS.into_iter().zip(planets(&experiment)) {
        let gap = (prop.body.center() - centre).length()
            - f64::from(sun_radius)
            - f64::from(sphere_radius(prop));

        assert!(
            gap > 1.0,
            "{} starts only {gap} units clear of the drawn Sun",
            planet.name
        );
    }
}

#[test]
fn the_whole_system_is_above_the_floor() {
    // The ecliptic has to clear the floor's collision slab by more than the largest body's
    // radius, or the experiment starts by pushing the Sun out of the ground.
    let experiment = built();

    for prop in experiment.scene.props.iter().filter(|prop| !prop.immovable) {
        let lowest = prop.body.center().y - f64::from(sphere_radius(prop));
        assert!(lowest > 0.0, "a body reaches y = {lowest}, below the floor");
    }
}

#[test]
fn the_bounds_enclose_every_body() {
    // The shared `±50` bounds would be a false statement about a scene 4495 units across.
    let experiment = built();

    assert!(experiment.scene.bounds.is_valid());

    for prop in experiment.scene.props.iter().filter(|prop| !prop.immovable) {
        let centre = prop.body.center().as_vec3();
        let bounds = experiment.scene.bounds;

        assert!(
            centre.x >= bounds.min.x
                && centre.x <= bounds.max.x
                && centre.z >= bounds.min.z
                && centre.z <= bounds.max.z
                && centre.y >= bounds.min.y
                && centre.y <= bounds.max.y,
            "{centre:?} is outside {bounds:?}"
        );
    }
}

#[test]
fn the_spec_runs_and_frames_the_inner_system() {
    let experiment = built();
    let spec = experiment.spec;

    assert_eq!(spec.id, ExperimentId::SolarSystem);
    assert!(spec.is_valid());
    assert_eq!(spec.time_scale, SOLAR_SYSTEM_TIME_SCALE);
    assert_eq!(
        spec.trail_length_units,
        Some(SOLAR_SYSTEM_TRAIL_LENGTH_UNITS)
    );

    // Deliberately less than the scene's extent: Mars is framed, Jupiter is not. If this
    // ever becomes "everything", the opening view is showing nothing useful.
    let mars_orbit =
        (SOLAR_SYSTEM_PLANETS[3].semi_major_axis_m / crate::constants::METRES_PER_UNIT) as f32;
    let jupiter_orbit =
        (SOLAR_SYSTEM_PLANETS[4].semi_major_axis_m / crate::constants::METRES_PER_UNIT) as f32;

    assert!(spec.framed_radius_units > mars_orbit);
    assert!(spec.framed_radius_units < jupiter_orbit);
}

#[test]
fn the_experiment_is_deterministic() {
    // Every initial condition is a measurement, so the seed cannot change the scene.
    assert_eq!(
        ExperimentId::SolarSystem.build(1).scene,
        ExperimentId::SolarSystem.build(2).scene
    );
}

#[test]
fn the_trail_is_long_enough_to_show_an_orbit() {
    // The default 10-unit trail is a hundredth of Earth's orbit — an invisible stub. This
    // pins the reason this experiment asks for its own length.
    let earth_circumference = std::f32::consts::TAU
        * (SOLAR_SYSTEM_PLANETS[2].semi_major_axis_m / crate::constants::METRES_PER_UNIT) as f32;
    let fraction = SOLAR_SYSTEM_TRAIL_LENGTH_UNITS / earth_circumference;

    assert!(
        (0.2..0.8).contains(&fraction),
        "the trail covers {fraction} of Earth's orbit"
    );
}

// -----------------------------------------------------------------------
// Does it actually orbit? The end-to-end check.
// -----------------------------------------------------------------------

#[test]
fn earth_completes_one_orbit_in_one_year_and_comes_back() {
    // The claim that makes the experiment an experiment rather than a diagram: run the
    // real integrator at the shipped clock scale for one simulated year and Earth should
    // be back where it started, at the same distance from the Sun.
    //
    // Tolerances are set by the integrator, not by the physics: first-order symplectic
    // Euler at ~1900 steps per orbit closes the ellipse to well under a percent, and the
    // other seven planets perturb Earth by far less than that over a single year.
    use crate::time::simulation::SimulationClock;

    let Experiment {
        mut scene, spec, ..
    } = solar_system();
    let mut clock = SimulationClock::new(spec.time_scale);

    let earth_index = FIRST_PLANET_INDEX + 2;
    let centre = || DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS);
    let start = scene.props[earth_index].body.center() - centre();
    let start_radius = start.length();

    // One Earth year, in 1/60-second observer frames — the app's normal cadence.
    let year_seconds = circular_orbit_period_seconds(
        SOLAR_SYSTEM_PLANETS[2].semi_major_axis_m,
        SOLAR_SYSTEM_SUN_MASS_KG,
    );
    let frame = 1.0 / 60.0;
    let frames = (year_seconds / (f64::from(spec.time_scale) * frame)).round() as usize;

    for _ in 0..frames {
        scene.step(clock.advance(frame as f32));
    }

    let end = scene.props[earth_index].body.center() - centre();

    // Still on the same circle.
    assert!(
        (end.length() / start_radius - 1.0).abs() < 0.02,
        "Earth's distance drifted from {start_radius} to {}",
        end.length()
    );

    // And back at the same longitude, to within a few degrees of arc.
    let cosine = end.normalize().dot(start.normalize()).clamp(-1.0, 1.0);
    let degrees_short = cosine.acos().to_degrees();
    assert!(
        degrees_short < 10.0,
        "after one year Earth is {degrees_short}° from where it started"
    );

    // Nothing collided, nothing exploded.
    for prop in &scene.props {
        assert!(prop.is_valid());
    }
}
