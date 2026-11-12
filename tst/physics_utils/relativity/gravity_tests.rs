//! Unit and property tests for [`crate::physics_utils::relativity::gravity`].
//!
//! Compiled into `src/physics_utils/relativity/gravity.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! Every function is checked against a measurement, because each is a single line of arithmetic and
//! a test that recomputed it would assert nothing:
//!
//! - Mercury's perihelion advances `42.98″` per century.
//! - Starlight grazing the Sun bends by `1.75″`.
//! - GPS clocks gain `38.5 µs` a day, and the special-relativistic part of that is the `−7.2 µs`
//!   that `time_dilation.rs` computes on its own.
//! - Pound and Rebka measured `2.46e-15` over `22.5 m`.
//!
//! Plus direction: a deeper clock is the slow one, and light climbing out is reddened.
//!
//! Tolerances on physical figures are a few parts in `1e4`, since `G` is known to `2e-5`.

use super::*;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::physics_utils::relativity::time_dilation::clock_lag_seconds;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_gravity_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

/// IAU nominal solar mass parameter over CODATA `G`.
const SUN_MASS_KG: f64 = 1.988_47e30;
/// IAU nominal solar radius.
const SUN_RADIUS_M: f64 = 6.957e8;
const EARTH_MASS_KG: f64 = 5.972_2e24;
/// Mean radius.
const EARTH_RADIUS_M: f64 = 6.371e6;
const DAY_S: f64 = 86_400.0;
const MICROSECOND: f64 = 1.0e-6;

fn arcseconds(radians: f64) -> f64 {
    radians.to_degrees() * 3_600.0
}

// -----------------------------------------------------------------------
// The Schwarzschild radius.
// -----------------------------------------------------------------------

#[test]
fn the_schwarzschild_radii_of_the_sun_and_earth() {
    let sun = schwarzschild_radius(SUN_MASS_KG).expect("valid");
    let earth = schwarzschild_radius(EARTH_MASS_KG).expect("valid");
    assert!((sun - 2_953.25).abs() < 0.1, "Sun: {sun} m");
    assert!((earth - 8.870e-3).abs() < 1.0e-6, "Earth: {earth} m");
}

#[test]
fn the_radius_is_proportional_to_the_mass() {
    let one = schwarzschild_radius(1.0e30).expect("valid");
    let ten = schwarzschild_radius(1.0e31).expect("valid");
    assert!((ten / one - 10.0).abs() < 1.0e-14);
}

#[test]
fn sagittarius_a_star_would_fit_inside_mercurys_orbit() {
    // 4.3 million solar masses: a horizon radius of 12.7 million km, 0.085 au.
    let radius = schwarzschild_radius(4.3e6 * SUN_MASS_KG).expect("valid");
    assert!(
        (radius / 1.0e9 - 12.7).abs() < 0.05,
        "{} million km",
        radius / 1.0e9
    );
    assert!(radius < 5.79e10);
}

// -----------------------------------------------------------------------
// Clocks.
// -----------------------------------------------------------------------

#[test]
fn the_deeper_clock_is_the_slower_one() {
    let low = gravitational_time_dilation_factor(EARTH_MASS_KG, EARTH_RADIUS_M).expect("valid");
    let high =
        gravitational_time_dilation_factor(EARTH_MASS_KG, 10.0 * EARTH_RADIUS_M).expect("valid");
    assert!(low < high, "lower must run slower");
    assert!(high < 1.0, "and both slower than a clock far away");

    let low_deficit = gravitational_rate_deficit(EARTH_MASS_KG, EARTH_RADIUS_M).expect("valid");
    let high_deficit =
        gravitational_rate_deficit(EARTH_MASS_KG, 10.0 * EARTH_RADIUS_M).expect("valid");
    assert!(low_deficit > high_deficit && high_deficit > 0.0);
}

#[test]
fn the_factor_at_simple_multiples_of_the_horizon() {
    let mass = SUN_MASS_KG;
    let horizon = schwarzschild_radius(mass).expect("valid");
    // sqrt(1 - 1/n): 1/2 at 4/3, sqrt(1/2) at 2, sqrt(3)/2 at 4.
    for (multiple, expected) in [
        (4.0 / 3.0, 0.5),
        (2.0, std::f64::consts::FRAC_1_SQRT_2),
        (4.0, 3.0_f64.sqrt() / 2.0),
    ] {
        let factor = gravitational_time_dilation_factor(mass, multiple * horizon).expect("valid");
        assert!(
            (factor - expected).abs() < 1.0e-14,
            "at {multiple} r_s: {factor}"
        );

        let deficit = gravitational_rate_deficit(mass, multiple * horizon).expect("valid");
        assert!((deficit - (1.0 - expected)).abs() < 1.0e-14);
    }
}

#[test]
fn nothing_stays_at_rest_at_or_inside_the_horizon() {
    let horizon = schwarzschild_radius(SUN_MASS_KG).expect("valid");
    for radius in [horizon, 0.5 * horizon, 1.0e-3] {
        assert_eq!(
            gravitational_time_dilation_factor(SUN_MASS_KG, radius),
            None
        );
        assert_eq!(gravitational_rate_deficit(SUN_MASS_KG, radius), None);
        assert_eq!(gravitational_redshift(SUN_MASS_KG, radius, 1.0e12), None);
        assert_eq!(gravitational_redshift(SUN_MASS_KG, 1.0e12, radius), None);
        assert_eq!(light_deflection_angle(SUN_MASS_KG, radius), None);
    }
}

#[test]
fn the_deficit_survives_where_subtracting_from_one_does_not() {
    // Earth's own field at the Moon's distance: 1.15e-11. `1 - factor` keeps five digits of that.
    let radius = 3.844e8;
    let stable = gravitational_rate_deficit(EARTH_MASS_KG, radius).expect("valid");
    let naive = 1.0 - gravitational_time_dilation_factor(EARTH_MASS_KG, radius).expect("valid");

    let weak_field = 0.5 * schwarzschild_radius(EARTH_MASS_KG).expect("valid") / radius;
    assert!(
        (stable / weak_field - 1.0).abs() < 1.0e-10,
        "GM/(r c^2) to first order"
    );
    assert!(
        (naive / stable - 1.0).abs() > 1.0e-8,
        "if the naive form is this good, the stable one has lost its reason to exist"
    );
    assert!(
        (naive / stable - 1.0).abs() < 1.0e-4,
        "but it is not nonsense either"
    );
}

#[test]
fn a_clock_on_earths_surface_loses_sixty_microseconds_a_day_to_earths_gravity() {
    let deficit = gravitational_rate_deficit(EARTH_MASS_KG, EARTH_RADIUS_M).expect("valid");
    assert!((deficit * DAY_S / MICROSECOND - 60.14).abs() < 0.02);
}

#[test]
fn the_weak_field_deficit_is_g_times_height_over_c_squared() {
    // Two clocks a metre apart in height near the ground differ by g*h/c^2 = 1.09e-16: the size of
    // effect optical clocks now resolve.
    let low = gravitational_rate_deficit(EARTH_MASS_KG, EARTH_RADIUS_M).expect("valid");
    let high = gravitational_rate_deficit(EARTH_MASS_KG, EARTH_RADIUS_M + 1_000.0).expect("valid");
    let per_metre = (low - high) / 1_000.0;

    let surface_gravity = crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT
        * EARTH_MASS_KG
        / (EARTH_RADIUS_M * EARTH_RADIUS_M);
    assert!(
        (surface_gravity / STANDARD_GRAVITY_M_S2 - 1.0).abs() < 0.002,
        "sanity: g"
    );
    assert!((per_metre / (surface_gravity / (C * C)) - 1.0).abs() < 1.0e-3);
    assert!((per_metre / 1.09e-16 - 1.0).abs() < 0.01);
}

// -----------------------------------------------------------------------
// Redshift.
// -----------------------------------------------------------------------

#[test]
fn light_climbing_out_is_reddened_and_light_falling_in_is_blued() {
    let out = gravitational_redshift(SUN_MASS_KG, SUN_RADIUS_M, 1.496e11).expect("valid");
    let back = gravitational_redshift(SUN_MASS_KG, 1.496e11, SUN_RADIUS_M).expect("valid");
    assert!(out > 0.0, "climbing out must redden");
    assert!(back < 0.0, "falling in must blue");
    assert!(((1.0 + out) * (1.0 + back) - 1.0).abs() < 1.0e-15);
}

#[test]
fn the_suns_surface_redshift_is_two_parts_per_million() {
    // GM/(R c^2) = 2.12e-6, measured in solar spectral lines as a 636 m/s equivalent velocity.
    let z = gravitational_redshift(SUN_MASS_KG, SUN_RADIUS_M, 1.0e20).expect("valid");
    assert!((z / 2.1225e-6 - 1.0).abs() < 2.0e-4, "z = {z:e}");
    assert!((z * C - 636.3).abs() < 0.2);
}

#[test]
fn pound_and_rebka_measured_the_shift_over_a_tower() {
    // 22.5 m up the Jefferson tower at Harvard, 1960: g*h/c^2 = 2.46e-15. The two radii differ in
    // the seventh significant figure and the result still has to come out.
    let z = gravitational_redshift(EARTH_MASS_KG, EARTH_RADIUS_M, EARTH_RADIUS_M + 22.5)
        .expect("valid");
    assert!((z / 2.46e-15 - 1.0).abs() < 0.01, "z = {z:e}");
}

#[test]
fn the_redshift_agrees_with_the_ratio_of_clock_rates() {
    // 1 + z is the emitter's clock rate over the observer's, inverted. Strong field, so the naive
    // form has digits to compare against.
    let horizon = schwarzschild_radius(SUN_MASS_KG).expect("valid");
    let z = gravitational_redshift(SUN_MASS_KG, 1.5 * horizon, 6.0 * horizon).expect("valid");
    let emitter = gravitational_time_dilation_factor(SUN_MASS_KG, 1.5 * horizon).expect("valid");
    let observer = gravitational_time_dilation_factor(SUN_MASS_KG, 6.0 * horizon).expect("valid");
    assert!((1.0 + z - observer / emitter).abs() < 1.0e-14);
}

#[test]
fn light_from_a_neutron_star_is_shifted_by_a_quarter() {
    // 1.4 solar masses, 12 km: 1 + z = 1/sqrt(1 - 4.135/12) = 1.235.
    let z = gravitational_redshift(1.4 * SUN_MASS_KG, 12_000.0, 1.0e20).expect("valid");
    assert!((z - 0.235).abs() < 0.001, "z = {z}");
}

// -----------------------------------------------------------------------
// Mercury.
// -----------------------------------------------------------------------

#[test]
fn mercurys_perihelion_advances_43_arcseconds_a_century() {
    // a = 0.387098 au, e = 0.205630, period 87.9691 days. The observed anomalous advance is
    // 42.98 +- 0.04 arcseconds per Julian century.
    let per_orbit =
        perihelion_precession_per_orbit(SUN_MASS_KG, 5.790_905e10, 0.205_630).expect("valid");
    let orbits_per_century = 36_525.0 / 87.969_1;
    let per_century = arcseconds(per_orbit) * orbits_per_century;

    assert!(
        (per_century - 42.98).abs() < 0.02,
        "got {per_century} arcseconds"
    );
}

#[test]
fn the_other_planets_precess_far_less() {
    // Venus 8.62, Earth 3.84 arcseconds per century.
    let venus = arcseconds(
        perihelion_precession_per_orbit(SUN_MASS_KG, 1.082_09e11, 0.006_77).expect("valid"),
    ) * 36_525.0
        / 224.701;
    let earth = arcseconds(
        perihelion_precession_per_orbit(SUN_MASS_KG, 1.495_98e11, 0.016_71).expect("valid"),
    ) * 36_525.0
        / 365.256;

    assert!((venus - 8.62).abs() < 0.02, "Venus: {venus}");
    assert!((earth - 3.84).abs() < 0.02, "Earth: {earth}");
}

#[test]
fn an_eccentric_orbit_precesses_more_than_a_round_one() {
    let round = perihelion_precession_per_orbit(SUN_MASS_KG, 1.0e11, 0.0).expect("valid");
    let eccentric = perihelion_precession_per_orbit(SUN_MASS_KG, 1.0e11, 0.6).expect("valid");
    assert!((eccentric / round - 1.0 / 0.64).abs() < 1.0e-14);
}

#[test]
fn an_open_orbit_has_no_perihelion_to_advance() {
    for eccentricity in [1.0, 1.5, -0.1, f64::NAN] {
        assert_eq!(
            perihelion_precession_per_orbit(SUN_MASS_KG, 1.0e11, eccentricity),
            None
        );
    }
    for axis in [0.0, -1.0e11, f64::NAN, f64::INFINITY] {
        assert_eq!(
            perihelion_precession_per_orbit(SUN_MASS_KG, axis, 0.2),
            None
        );
    }
}

// -----------------------------------------------------------------------
// Light bending.
// -----------------------------------------------------------------------

#[test]
fn starlight_grazing_the_sun_bends_by_1_75_arcseconds() {
    let angle = light_deflection_angle(SUN_MASS_KG, SUN_RADIUS_M).expect("valid");
    assert!(
        (arcseconds(angle) - 1.751).abs() < 0.001,
        "got {}",
        arcseconds(angle)
    );
}

#[test]
fn the_bending_is_twice_the_newtonian_value() {
    // A particle at speed c passing a mass at distance b is deflected, in Newtonian gravity, by
    // 2GM/(c^2 b): 0.875 arcseconds at the Sun. Eddington's 1919 result distinguished the two.
    let angle = light_deflection_angle(SUN_MASS_KG, SUN_RADIUS_M).expect("valid");
    let newtonian =
        2.0 * crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT * SUN_MASS_KG
            / (C * C * SUN_RADIUS_M);
    assert!((angle / newtonian - 2.0).abs() < 1.0e-14);
    assert!((arcseconds(newtonian) - 0.875).abs() < 0.001);
}

#[test]
fn the_bending_falls_off_with_distance() {
    let near = light_deflection_angle(SUN_MASS_KG, SUN_RADIUS_M).expect("valid");
    let far = light_deflection_angle(SUN_MASS_KG, 4.0 * SUN_RADIUS_M).expect("valid");
    assert!((near / far - 4.0).abs() < 1.0e-14);
}

// -----------------------------------------------------------------------
// Orbiting clocks: where special and general relativity meet.
// -----------------------------------------------------------------------

#[test]
fn gps_clocks_gain_38_microseconds_a_day() {
    // Orbit radius 26,560 km. The correction real receivers depend on: without it positions would
    // drift by about 11 km a day.
    let offset = orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, 2.656e7).expect("valid");
    let per_day = offset * DAY_S / MICROSECOND;

    assert!(
        (per_day - 38.5).abs() < 0.1,
        "got {per_day} microseconds a day"
    );
    assert!(
        (offset * DAY_S * C / 1_000.0 - 11.5).abs() < 0.1,
        "as a range error, in km"
    );
}

#[test]
fn the_gps_figure_is_the_gravity_term_minus_this_sub_layers_time_dilation() {
    // The parent README says the -7.2 microseconds this sub-layer computes is only the
    // special-relativistic half. Here are both halves, from separate code.
    let orbit = 2.656e7;
    let gravity_term = gravitational_rate_deficit(EARTH_MASS_KG, EARTH_RADIUS_M).expect("valid")
        - gravitational_rate_deficit(EARTH_MASS_KG, orbit).expect("valid");

    let orbital_speed =
        (crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT * EARTH_MASS_KG / orbit)
            .sqrt();
    let speed_term = clock_lag_seconds(DAY_S, orbital_speed).expect("valid") / DAY_S;

    assert!((gravity_term * DAY_S / MICROSECOND - 45.7).abs() < 0.1);
    assert!((speed_term * DAY_S / MICROSECOND - 7.2).abs() < 0.1);

    let combined = orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, orbit).expect("valid");
    // The combined form is first order; the two exact halves differ from it at second order.
    assert!(((gravity_term - speed_term) / combined - 1.0).abs() < 1.0e-8);
}

#[test]
fn the_space_station_clock_runs_slow_instead() {
    // 400 km up, speed wins over height: -24.7 microseconds a day.
    let offset = orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, 6.771e6).expect("valid");
    assert!((offset * DAY_S / MICROSECOND + 24.7).abs() < 0.1);
}

#[test]
fn the_two_effects_cancel_at_one_and_a_half_radii() {
    let offset = orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, 1.5 * EARTH_RADIUS_M)
        .expect("valid");
    assert!(offset.abs() < 1.0e-24, "got {offset:e}");

    let below = orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, 1.4 * EARTH_RADIUS_M)
        .expect("valid");
    let above = orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, 1.6 * EARTH_RADIUS_M)
        .expect("valid");
    assert!(below < 0.0 && above > 0.0);
}

#[test]
fn an_orbit_below_the_ground_has_no_answer() {
    assert_eq!(
        orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, 0.9 * EARTH_RADIUS_M),
        None
    );
    assert!(orbiting_clock_rate_offset(EARTH_MASS_KG, EARTH_RADIUS_M, EARTH_RADIUS_M).is_some());
}

// -----------------------------------------------------------------------
// The photon sphere and the innermost stable circular orbit.
// -----------------------------------------------------------------------

/// Where `f` peaks (or bottoms out, with `sign = -1`) on `[low, high]`, found by scanning.
fn extremum(low: f64, high: f64, sign: f64, f: impl Fn(f64) -> f64) -> f64 {
    let steps = 200_000;
    (0..=steps)
        .map(|i| low + (high - low) * f64::from(i) / f64::from(steps))
        .max_by(|a, b| (sign * f(*a)).total_cmp(&(sign * f(*b))))
        .expect("non-empty scan")
}

#[test]
fn the_photon_sphere_is_where_lights_effective_potential_peaks() {
    // Light orbiting a Schwarzschild mass feels V(r) = (1 - r_s/r)/r^2 (times L^2). Its one
    // maximum is the only circular orbit light has. Found by scanning, not from the formula.
    let horizon = schwarzschild_radius(SUN_MASS_KG).expect("valid");
    let peak = extremum(1.01 * horizon, 5.0 * horizon, 1.0, |r| {
        (1.0 - horizon / r) / (r * r)
    });
    let photon = photon_sphere_radius(SUN_MASS_KG).expect("valid");

    assert!(
        (peak / photon - 1.0).abs() < 1.0e-4,
        "peak at {} r_s",
        peak / horizon
    );
}

#[test]
fn the_isco_is_where_a_circular_orbit_needs_the_least_angular_momentum() {
    // A circular orbit at radius r needs angular momentum L^2 = r_s c^2 r^2 / (2r - 3 r_s). Inside
    // the minimum of that curve, a slightly disturbed orbit has no stable radius to settle at. The
    // minimum is the ISCO. Found by scanning.
    let horizon = schwarzschild_radius(SUN_MASS_KG).expect("valid");
    let minimum = extremum(1.6 * horizon, 10.0 * horizon, -1.0, |r| {
        r * r / (2.0 * r - 3.0 * horizon)
    });
    let isco = innermost_stable_circular_orbit_radius(SUN_MASS_KG).expect("valid");

    assert!(
        (minimum / isco - 1.0).abs() < 1.0e-4,
        "minimum at {} r_s",
        minimum / horizon
    );
}

#[test]
fn the_three_radii_of_sgr_a_star() {
    // 4.3 million solar masses: horizon 12.7, photon sphere 19.0, ISCO 38.1 million km.
    let mass = 8.5467e36;
    let horizon = schwarzschild_radius(mass).expect("valid");
    let photon = photon_sphere_radius(mass).expect("valid");
    let isco = innermost_stable_circular_orbit_radius(mass).expect("valid");

    assert!((horizon / 1.0e9 - 12.69).abs() < 0.01);
    assert!((photon / 1.0e9 - 19.04).abs() < 0.01);
    assert!((isco / 1.0e9 - 38.08).abs() < 0.01);
    assert!(horizon < photon && photon < isco);
}

#[test]
fn an_orbit_at_sgr_a_stars_isco_takes_half_an_hour() {
    // The period at the ISCO, as a distant clock measures it, is Kepler's 2 pi sqrt(r^3/GM) exactly
    // (a Schwarzschild result, not an approximation): 2 pi 6^1.5 GM/c^3. For a non-spinning Sgr A*
    // at 4.3 million solar masses that is 32.6 minutes; flares from near the hole repeat on
    // timescales like this.
    let mass = 8.5467e36;
    let isco = innermost_stable_circular_orbit_radius(mass).expect("valid");
    let gm = crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT * mass;
    let period = 2.0 * std::f64::consts::PI * (isco.powi(3) / gm).sqrt();

    assert!(
        (period / 60.0 - 32.58).abs() < 0.05,
        "got {} minutes",
        period / 60.0
    );
}

// -----------------------------------------------------------------------
// Flamm's paraboloid.
// -----------------------------------------------------------------------

#[test]
fn a_ruler_along_flamms_sheet_measures_the_true_radial_distance() {
    // The defining property, checked without the formula: the proper radial distance between
    // circles at r1 and r2 is the integral of dr/sqrt(1 - r_s/r), computed here by Simpson's rule
    // away from the throat, and the arc length along the sheet between the same circles must agree.
    let mass = 8.5467e36;
    let horizon = schwarzschild_radius(mass).expect("valid");
    let (inner, outer) = (1.5 * horizon, 10.0 * horizon);
    let steps = 20_000;
    let h = (outer - inner) / f64::from(steps);

    let mut proper = 0.0;
    let mut along_sheet = 0.0;
    let mut previous = (inner, flamm_paraboloid_height(mass, inner).expect("valid"));
    for i in 0..=steps {
        let r = inner + h * f64::from(i);
        let weight = if i == 0 || i == steps {
            1.0
        } else if i % 2 == 1 {
            4.0
        } else {
            2.0
        };
        proper += weight / (1.0 - horizon / r).sqrt();

        if i > 0 {
            let z = flamm_paraboloid_height(mass, r).expect("valid");
            along_sheet += (r - previous.0).hypot(z - previous.1);
            previous = (r, z);
        }
    }
    proper *= h / 3.0;

    assert!(
        (along_sheet / proper - 1.0).abs() < 1.0e-6,
        "sheet {along_sheet:e} against proper {proper:e}"
    );
    assert!(proper > 1.1 * (outer - inner), "space really is stretched");
}

#[test]
fn the_sheet_meets_the_horizon_vertically_and_flattens_slowly() {
    let mass = SUN_MASS_KG;
    let horizon = schwarzschild_radius(mass).expect("valid");
    let slope = |r: f64| {
        let dr = 1.0e-6 * r;
        (flamm_paraboloid_height(mass, r + dr).unwrap() - flamm_paraboloid_height(mass, r).unwrap())
            / dr
    };

    assert!(slope(1.000_001 * horizon) > 100.0, "steep at the throat");
    assert!(
        (slope(5.0 * horizon) - 0.5).abs() < 1.0e-4,
        "sqrt(r_s/(r - r_s)) = 1/2 at 5 r_s"
    );
    assert!(
        slope(1.0e4 * horizon) < 0.011,
        "nearly flat far out, though never quite"
    );
}

#[test]
fn the_sheet_has_no_height_inside_the_horizon() {
    let horizon = schwarzschild_radius(SUN_MASS_KG).expect("valid");
    assert_eq!(flamm_paraboloid_height(SUN_MASS_KG, 0.99 * horizon), None);
    assert_eq!(flamm_paraboloid_height(SUN_MASS_KG, f64::NAN), None);
    assert_eq!(flamm_paraboloid_height(0.0, horizon), None);
}

// -----------------------------------------------------------------------
// The equation of motion.
// -----------------------------------------------------------------------

type State = ([f64; 3], [f64; 3]);

fn add(a: [f64; 3], b: [f64; 3], scale: f64) -> [f64; 3] {
    [
        a[0] + scale * b[0],
        a[1] + scale * b[1],
        a[2] + scale * b[2],
    ]
}

fn length(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

/// One classical Runge–Kutta step of the Schwarzschild equation of motion. Fourth order, so the
/// tests measure the formula and not the integrator.
fn rk4(mass: f64, (x, v): State, dt: f64) -> Option<State> {
    let accel = |x, v| schwarzschild_acceleration(mass, x, v);
    let a1 = accel(x, v)?;
    let (x2, v2) = (add(x, v, 0.5 * dt), add(v, a1, 0.5 * dt));
    let a2 = accel(x2, v2)?;
    let (x3, v3) = (add(x, v2, 0.5 * dt), add(v, a2, 0.5 * dt));
    let a3 = accel(x3, v3)?;
    let (x4, v4) = (add(x, v3, dt), add(v, a3, dt));
    let a4 = accel(x4, v4)?;

    let mut nx = x;
    let mut nv = v;
    for i in 0..3 {
        nx[i] += dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]);
        nv[i] += dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]);
    }
    Some((nx, nv))
}

/// The geodesic's conserved energy per unit rest energy, `E/(mc²)`, and angular momentum per unit
/// mass, from coordinate position and velocity. `dτ/dt = √(A − ṙ²/(Ac²) − v⊥²/c²)`.
fn conserved(horizon: f64, (x, v): State) -> (f64, f64) {
    let r = length(x);
    let a = 1.0 - horizon / r;
    let radial = (x[0] * v[0] + x[1] * v[1] + x[2] * v[2]) / r;
    let transverse_sq = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) - radial * radial;
    let c2 = C * C;
    let rate = (a - radial * radial / (a * c2) - transverse_sq / c2).sqrt();
    (a / rate, r * transverse_sq.sqrt() / rate)
}

const HOLE_KG: f64 = 8.5467e36;

fn horizon_m() -> f64 {
    schwarzschild_radius(HOLE_KG).expect("valid")
}

fn gm() -> f64 {
    crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT * HOLE_KG
}

/// The eccentric orbit `experiment_6` flies: released at 9 r_s with 0.873 of the circular speed.
/// Worked by hand from the conserved energy and angular momentum, its periapsis is 4.5 r_s.
fn eccentric_start() -> State {
    let r = 9.0 * horizon_m();
    let speed = 0.873 * (gm() / r).sqrt();
    ([r, 0.0, 0.0], [0.0, speed, 0.0])
}

#[test]
fn a_circular_orbit_runs_at_keplers_rate_at_every_radius() {
    // Omega^2 = GM/r^3 in coordinate time, exactly, so the acceleration is exactly v^2/r inward.
    for multiple in [1.6, 2.0, 3.0, 5.0, 100.0] {
        let r = multiple * horizon_m();
        let v = (gm() / r).sqrt();
        let a = schwarzschild_acceleration(HOLE_KG, [0.0, 0.0, r], [v, 0.0, 0.0]).expect("valid");
        assert!(
            (a[2] / (-v * v / r) - 1.0).abs() < 1.0e-12,
            "at {multiple} r_s"
        );
        assert!(a[0].abs() < 1.0e-12 * v * v / r && a[1] == 0.0);
    }
}

#[test]
fn light_circles_at_the_photon_sphere_at_c_over_root_three() {
    // At 1.5 r_s a tangential coordinate speed of c/sqrt(3) is a null orbit (A c^2 = v^2 there),
    // and the acceleration is exactly its centripetal v^2/r.
    let r = photon_sphere_radius(HOLE_KG).expect("valid");
    let v = C / 3.0_f64.sqrt();
    let a = schwarzschild_acceleration(HOLE_KG, [r, 0.0, 0.0], [0.0, v, 0.0]).expect("valid");
    assert!((a[0] / (-v * v / r) - 1.0).abs() < 1.0e-12);
    assert!(
        (1.0 - horizon_m() / r - (v / C).powi(2)).abs() < 1.0e-12,
        "a null orbit"
    );
}

#[test]
fn far_away_and_slow_it_is_newton() {
    let r = 1.0e6 * horizon_m();
    let a = schwarzschild_acceleration(HOLE_KG, [r, 0.0, 0.0], [10.0, 300.0, 0.0]).expect("valid");
    let newton = -gm() / (r * r);
    assert!(
        (a[0] / newton - 1.0).abs() < 2.0e-6,
        "corrections of order r_s/r"
    );
}

#[test]
fn the_geodesics_energy_and_angular_momentum_are_conserved() {
    // A strong test of the formula: these two quantities come from the metric, not from the
    // equation of motion, and an error in any term of the acceleration breaks one of them.
    let mut state = eccentric_start();
    let (energy, momentum) = conserved(horizon_m(), state);
    let dt = 0.5;

    for _ in 0..40_000 {
        state = rk4(HOLE_KG, state, dt).expect("stays outside the horizon");
    }

    let (energy_after, momentum_after) = conserved(horizon_m(), state);
    assert!(
        (energy_after / energy - 1.0).abs() < 1.0e-10,
        "{energy} -> {energy_after}"
    );
    assert!((momentum_after / momentum - 1.0).abs() < 1.0e-10);
    assert!(energy < 1.0, "a bound orbit");
}

#[test]
fn the_eccentric_orbit_reaches_the_periapsis_worked_by_hand() {
    // Released at apoapsis 9 r_s; the turning-point equation gives periapsis 4.5 r_s. Newton, for
    // the same release, would give 5.54 r_s.
    let mut state = eccentric_start();
    let mut nearest = f64::INFINITY;
    for _ in 0..12_000 {
        state = rk4(HOLE_KG, state, 0.5).expect("bound");
        nearest = nearest.min(length(state.0));
    }
    assert!(
        (nearest / horizon_m() / 4.5 - 1.0).abs() < 2.0e-3,
        "periapsis {} r_s",
        nearest / horizon_m()
    );
}

/// The angle by which the periapsis advances per radial period, measured by integrating.
fn measured_precession(start: State, dt: f64, steps: usize) -> f64 {
    let mut state = start;
    let mut previous_radial = -1.0;
    let mut previous_r = length(state.0);
    let mut periapses = Vec::new();

    for _ in 0..steps {
        state = rk4(HOLE_KG, state, dt).expect("bound");
        let r = length(state.0);
        let radial = r - previous_r;
        if previous_radial < 0.0 && radial >= 0.0 {
            periapses.push(state.0[1].atan2(state.0[0]));
        }
        previous_radial = radial;
        previous_r = r;
        if periapses.len() == 2 {
            break;
        }
    }

    assert_eq!(periapses.len(), 2, "did not see two periapses");
    (periapses[1] - periapses[0]).rem_euclid(2.0 * std::f64::consts::PI)
}

#[test]
fn a_weak_field_orbit_precesses_as_the_first_order_formula_says() {
    // Released at 2000 r_s with 0.8 of circular speed: semi-latus rectum about 1280 r_s, and the
    // first-order advance 3 pi r_s / p is 0.42 degrees per orbit. The exact equation must agree to
    // terms of order r_s/p.
    let r = 2000.0 * horizon_m();
    let f: f64 = 0.8;
    let speed = f * (gm() / r).sqrt();
    let periapsis = r * f * f / (2.0 - f * f);
    let semi_major = 0.5 * (r + periapsis);
    let eccentricity = (r - periapsis) / (r + periapsis);
    let period = 2.0 * std::f64::consts::PI * (semi_major.powi(3) / gm()).sqrt();

    let measured = measured_precession(
        ([r, 0.0, 0.0], [0.0, speed, 0.0]),
        period / 40_000.0,
        100_000,
    );
    let expected =
        perihelion_precession_per_orbit(HOLE_KG, semi_major, eccentricity).expect("valid");

    assert!(
        (measured / expected - 1.0).abs() < 0.01,
        "measured {} degrees against {}",
        measured.to_degrees(),
        expected.to_degrees()
    );
}

#[test]
fn close_in_the_precession_is_enormous() {
    // The orbit experiment_6 flies, between 4.5 and 9 r_s. The first-order formula is useless
    // here; the exact equation turns the periapsis by well over a radian per orbit.
    let measured = measured_precession(eccentric_start(), 0.5, 40_000);
    assert!(
        measured.to_degrees() > 90.0,
        "{} degrees",
        measured.to_degrees()
    );
}

/// Integrate until the body reaches `capture` (in r_s) or `steps` run out; true if captured.
fn plunges(start: State, capture: f64, steps: usize) -> bool {
    let mut state = start;
    for _ in 0..steps {
        if length(state.0) <= capture * horizon_m() {
            return true;
        }
        match rk4(HOLE_KG, state, 0.25) {
            Some(next) => state = next,
            None => return true,
        }
    }
    false
}

#[test]
fn outside_the_isco_a_nudged_circular_orbit_stays_near_its_circle() {
    let r = 4.0 * horizon_m();
    let speed = 0.995 * (gm() / r).sqrt();
    let mut state = ([r, 0.0, 0.0], [0.0, speed, 0.0]);
    let mut nearest = f64::INFINITY;
    for _ in 0..40_000 {
        state = rk4(HOLE_KG, state, 0.25).expect("stable");
        nearest = nearest.min(length(state.0));
    }
    assert!(
        nearest > 3.5 * horizon_m(),
        "dipped to {} r_s",
        nearest / horizon_m()
    );
}

#[test]
fn inside_the_isco_a_nudged_circular_orbit_plunges() {
    // The same 0.5% nudge at 2.5 r_s, where Newton would give an orbit dipping only to 2.45 r_s.
    let r = 2.5 * horizon_m();
    let speed = 0.995 * (gm() / r).sqrt();
    assert!(plunges(([r, 0.0, 0.0], [0.0, speed, 0.0]), 1.0101, 40_000));
}

#[test]
fn a_body_falling_from_far_away_never_crosses_the_horizon() {
    // Released with the speed of a fall from rest at infinity, dr/dt = -c A sqrt(r_s/r), it slows
    // as it nears the horizon and creeps towards it for ever, as a distant observer sees it.
    let horizon = horizon_m();
    let r = 10.0 * horizon;
    let a = 1.0 - horizon / r;
    let mut state = ([r, 0.0, 0.0], [-C * a * (horizon / r).sqrt(), 0.0, 0.0]);

    let mut fastest = 0.0_f64;
    for _ in 0..200_000 {
        state = rk4(HOLE_KG, state, 0.05).expect("never inside the horizon");
        fastest = fastest.max(length(state.1));
    }

    let r = length(state.0);
    assert!(r > horizon, "crossed");
    assert!(
        r < 1.001 * horizon,
        "and got very close: {} r_s",
        r / horizon
    );
    assert!(length(state.1) < 1.0e-3 * C, "almost stopped");
    // The fastest coordinate speed of such a fall is 2c/(3 sqrt 3) = 0.385c, at r = 3 r_s.
    assert!(
        (fastest / C - 2.0 / (3.0 * 3.0_f64.sqrt())).abs() < 1.0e-4,
        "{}c",
        fastest / C
    );
}

// -----------------------------------------------------------------------
// Light.
// -----------------------------------------------------------------------

/// Is `(x, v)` a null state: `A c² = ṙ²/A + v⊥²`? Returns the relative mismatch.
fn null_mismatch(horizon: f64, (x, v): State) -> f64 {
    let r = length(x);
    let a = 1.0 - horizon / r;
    let radial = (x[0] * v[0] + x[1] * v[1] + x[2] * v[2]) / r;
    let transverse_sq = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) - radial * radial;
    (radial * radial / a + transverse_sq) / (a * C * C) - 1.0
}

/// Fire a ray along +x from `start_rs` horizon radii out on -x, offset sideways by impact
/// parameter `b_rs`, and integrate until it is captured or has travelled well past the hole.
/// Returns `None` if captured, or the final direction of travel.
fn fire(b_rs: f64, start_rs: f64) -> Option<[f64; 3]> {
    let horizon = horizon_m();
    let x = [-start_rs * horizon, b_rs * horizon, 0.0];
    let v = light_velocity(HOLE_KG, x, [1.0, 0.0, 0.0]).expect("outside the horizon");
    let mut state = (x, v);

    loop {
        let r = length(state.0);
        if r <= 1.01 * horizon {
            return None;
        }
        if r > 1.2 * start_rs * horizon && state.0[0] * state.1[0] + state.0[1] * state.1[1] > 0.0 {
            let speed = length(state.1);
            return Some(state.1.map(|c| c / speed));
        }
        // Steps shrink near the hole, where the path bends hardest.
        let dt = (0.02 * (r - horizon) / C).min(2.0);
        state = rk4(HOLE_KG, state, dt)?;
    }
}

#[test]
fn light_velocity_satisfies_the_null_condition_in_every_direction() {
    let horizon = horizon_m();
    for multiple in [1.05, 1.5, 3.0, 10.0] {
        let x = [multiple * horizon * 0.6, multiple * horizon * 0.8, 0.0];
        for direction in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.3, -0.7, 0.2],
            [-0.6, -0.8, 0.0],
        ] {
            let v = light_velocity(HOLE_KG, x, direction).expect("valid");
            assert!(
                null_mismatch(horizon, (x, v)).abs() < 1.0e-12,
                "at {multiple} r_s"
            );
        }
    }
}

#[test]
fn far_from_the_hole_light_moves_at_c() {
    let v =
        light_velocity(HOLE_KG, [1.0e9 * horizon_m(), 0.0, 0.0], [0.0, 1.0, 1.0]).expect("valid");
    assert!((length(v) / C - 1.0).abs() < 1.0e-8);
}

#[test]
fn a_ray_stays_a_ray_as_it_bends() {
    // The equation of motion keeps a null state null: light remains light along its path.
    let horizon = horizon_m();
    let x = [-10.0 * horizon, 3.0 * horizon, 0.0];
    let mut state = (
        x,
        light_velocity(HOLE_KG, x, [1.0, 0.0, 0.0]).expect("valid"),
    );
    for _ in 0..20_000 {
        state = rk4(HOLE_KG, state, 0.05).expect("not captured");
    }
    assert!(null_mismatch(horizon, state).abs() < 1.0e-9);
}

#[test]
fn light_aimed_inside_the_critical_impact_parameter_is_captured_and_outside_escapes() {
    // The critical impact parameter is (3 sqrt 3 / 2) r_s = 2.598 r_s: the photon sphere's shadow.
    assert!(fire(2.55, 30.0).is_none(), "2.55 r_s is inside: captured");
    assert!(fire(2.65, 30.0).is_some(), "2.65 r_s is outside: escapes");
}

#[test]
fn light_just_outside_the_critical_parameter_is_turned_around() {
    // Close to the critical value a ray whirls round the photon sphere before leaving: at 2.65 r_s
    // it comes out heading back the way it came.
    let direction = fire(2.65, 30.0).expect("escapes");
    assert!(
        direction[0] < 0.0,
        "turned by more than 90 degrees: {direction:?}"
    );
}

#[test]
fn a_distant_ray_bends_by_the_first_order_angle() {
    // At b = 500 r_s the deflection is 2 r_s / b = 0.004 rad (4GM/(c^2 b)), to terms of order r_s/b.
    let direction = fire(500.0, 50_000.0).expect("escapes");
    let bent = (-direction[1]).atan2(direction[0]);
    let expected = light_deflection_angle(HOLE_KG, 500.0 * horizon_m()).expect("valid");
    assert!(
        (bent / expected - 1.0).abs() < 0.01,
        "bent {bent} rad against {expected}"
    );
}

#[test]
fn light_has_no_velocity_at_the_horizon_or_without_a_direction() {
    let horizon = horizon_m();
    assert_eq!(
        light_velocity(HOLE_KG, [horizon, 0.0, 0.0], [1.0, 0.0, 0.0]),
        None
    );
    assert_eq!(
        light_velocity(HOLE_KG, [3.0 * horizon, 0.0, 0.0], [0.0; 3]),
        None
    );
    assert_eq!(
        light_velocity(HOLE_KG, [f64::NAN, 0.0, 0.0], [1.0, 0.0, 0.0]),
        None
    );
    assert_eq!(
        light_velocity(0.0, [1.0e12, 0.0, 0.0], [1.0, 0.0, 0.0]),
        None
    );
}

#[test]
fn the_equation_has_no_answer_at_or_inside_the_horizon() {
    let horizon = horizon_m();
    for r in [horizon, 0.5 * horizon, 0.0] {
        assert_eq!(
            schwarzschild_acceleration(HOLE_KG, [r, 0.0, 0.0], [0.0; 3]),
            None
        );
    }
    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            schwarzschild_acceleration(HOLE_KG, [bad, 0.0, 0.0], [0.0; 3]),
            None
        );
        assert_eq!(
            schwarzschild_acceleration(HOLE_KG, [1.0e12, 0.0, 0.0], [0.0, bad, 0.0]),
            None
        );
        assert_eq!(
            schwarzschild_acceleration(bad, [1.0e12, 0.0, 0.0], [0.0; 3]),
            None
        );
    }
    assert_eq!(
        schwarzschild_acceleration(0.0, [1.0e12, 0.0, 0.0], [0.0; 3]),
        None
    );
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn bad_masses_and_radii_have_no_answer() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(schwarzschild_radius(bad), None);
        assert_eq!(photon_sphere_radius(bad), None);
        assert_eq!(innermost_stable_circular_orbit_radius(bad), None);
        assert_eq!(gravitational_time_dilation_factor(bad, 1.0e9), None);
        assert_eq!(gravitational_time_dilation_factor(SUN_MASS_KG, bad), None);
        assert_eq!(gravitational_rate_deficit(bad, 1.0e9), None);
        assert_eq!(gravitational_rate_deficit(SUN_MASS_KG, bad), None);
        assert_eq!(gravitational_redshift(bad, 1.0e9, 1.0e10), None);
        assert_eq!(gravitational_redshift(SUN_MASS_KG, bad, 1.0e10), None);
        assert_eq!(gravitational_redshift(SUN_MASS_KG, 1.0e9, bad), None);
        assert_eq!(perihelion_precession_per_orbit(bad, 1.0e11, 0.2), None);
        assert_eq!(light_deflection_angle(bad, 1.0e9), None);
        assert_eq!(light_deflection_angle(SUN_MASS_KG, bad), None);
        assert_eq!(orbiting_clock_rate_offset(bad, 1.0e7, 2.0e7), None);
        assert_eq!(orbiting_clock_rate_offset(EARTH_MASS_KG, bad, 2.0e7), None);
        assert_eq!(orbiting_clock_rate_offset(EARTH_MASS_KG, 1.0e7, bad), None);
    }
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// The factor and the deficit are the same quantity, and a clock runs faster the higher it is.
    #[test]
    fn clocks_run_faster_higher_up(
        mass in 1.0e20_f64..1.0e40,
        low_multiple in 1.01_f64..1.0e6,
        ratio in 1.0_f64..1.0e6,
    ) {
        let horizon = schwarzschild_radius(mass).expect("valid");
        let low = low_multiple * horizon;
        let high = low * ratio;

        let low_factor = gravitational_time_dilation_factor(mass, low).expect("valid");
        let high_factor = gravitational_time_dilation_factor(mass, high).expect("valid");
        prop_assert!(low_factor > 0.0 && low_factor <= high_factor && high_factor <= 1.0);

        let deficit = gravitational_rate_deficit(mass, low).expect("valid");
        prop_assert!((deficit + low_factor - 1.0).abs() < 1.0e-15);
        prop_assert!(deficit > 0.0);
    }

    /// Redshifts chain: out to a relay and on to the observer is the same as going direct, and the
    /// way back undoes it.
    #[test]
    fn redshifts_chain_through_a_relay(
        emitter_multiple in 1.5_f64..1.0e4,
        relay_ratio in 0.5_f64..100.0,
        observer_ratio in 0.5_f64..100.0,
    ) {
        let mass = SUN_MASS_KG;
        let horizon = schwarzschild_radius(mass).expect("valid");
        let emitter = emitter_multiple * horizon;
        let relay = (emitter * relay_ratio).max(1.2 * horizon);
        let observer = (relay * observer_ratio).max(1.2 * horizon);

        let first = gravitational_redshift(mass, emitter, relay).expect("valid");
        let second = gravitational_redshift(mass, relay, observer).expect("valid");
        let direct = gravitational_redshift(mass, emitter, observer).expect("valid");
        let back = gravitational_redshift(mass, observer, emitter).expect("valid");

        prop_assert!(((1.0 + first) * (1.0 + second) / (1.0 + direct) - 1.0).abs() < 1.0e-13);
        prop_assert!(((1.0 + direct) * (1.0 + back) - 1.0).abs() < 1.0e-13);
        prop_assert!(direct == 0.0 || (direct > 0.0) == (observer > emitter));
    }

    /// In a weak field the exact deficit is the Newtonian potential over `c²`, with a relative
    /// correction of a quarter of `r_s/r`.
    #[test]
    fn the_weak_field_deficit_is_the_newtonian_potential(
        mass in 1.0e20_f64..1.0e32,
        multiple in 1.0e4_f64..1.0e12,
    ) {
        let horizon = schwarzschild_radius(mass).expect("valid");
        let deficit = gravitational_rate_deficit(mass, multiple * horizon).expect("valid");
        let potential = 0.5 / multiple;

        prop_assert!((deficit / potential - 1.0 - 0.25 / multiple).abs() < 1.0e-8 / multiple + 1.0e-14);
    }
}
