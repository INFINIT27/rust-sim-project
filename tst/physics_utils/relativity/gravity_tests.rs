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
// Domain.
// -----------------------------------------------------------------------

#[test]
fn bad_masses_and_radii_have_no_answer() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(schwarzschild_radius(bad), None);
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
