//! Tests for [`crate::physics_utils::relativity::constants`].
//!
//! Compiled into `src/physics_utils/relativity/constants.rs` via `#[path]`.
//!
//! One constant, but it makes two claims worth pinning separately: that it is the right number, and
//! that it is **exact**. The second is what distinguishes `c` from `G` and is what lets the tests in
//! the rest of this sub-layer use float-epsilon tolerances instead of five significant figures.

use super::*;

#[test]
fn the_speed_of_light_is_the_defined_value() {
    // Against the SI definition, not against a measurement: since 1983 the metre has been defined so
    // that this number is exact. A test that allowed a tolerance here would be asserting the wrong
    // kind of fact.
    assert_eq!(SPEED_OF_LIGHT_M_S, 299_792_458.0);
}

#[test]
fn the_speed_of_light_is_exactly_representable() {
    // `299_792_458 < 2^53`, so `f64` holds it with no rounding. This is why `beta_from_velocity(c)`
    // returns exactly `1.0`, and it is the reason the invariance-of-`c` tests get as close as they do
    // — though not all the way to bit equality, since the velocity transformation's denominator does
    // its own rounding. See `velocity_tests`.
    assert_eq!(SPEED_OF_LIGHT_M_S as i64 as f64, SPEED_OF_LIGHT_M_S);
    assert_eq!(SPEED_OF_LIGHT_M_S as i64, 299_792_458_i64);
    assert!(SPEED_OF_LIGHT_M_S < (1_i64 << 53) as f64);
}

#[test]
fn light_travels_one_light_year_in_a_year() {
    // An independent cross-check from a different direction: the published length of a Julian
    // light-year is 9.4607e15 m, and it should fall out of c times 365.25 days.
    let julian_year_s = 365.25 * 86_400.0;
    let light_year_m = SPEED_OF_LIGHT_M_S * julian_year_s;

    assert!(
        (light_year_m / 9.460_730_472_580_8e15 - 1.0).abs() < 1.0e-12,
        "a light-year came out {light_year_m:e} m"
    );
}

#[test]
fn light_crosses_an_astronomical_unit_in_about_499_seconds() {
    // The other figure worth knowing, and the one relevant to this crate: sunlight takes 8 minutes
    // 19 seconds to reach Earth. Published value 499.0 s.
    let au_m = 1.495_978_707e11;
    let seconds = au_m / SPEED_OF_LIGHT_M_S;

    assert!(
        (seconds - 499.0).abs() < 0.5,
        "light crossed an AU in {seconds} s, expected about 499"
    );
}

#[test]
fn the_value_is_metres_per_second_not_kilometres() {
    // A reviewer asked whether `299_792_458` was km/s, since c is usually quoted as `3e5 km/s`. It is
    // not: both are the same speed and only the decimal point moves. Pinned because the digit string
    // invites the question, and a unit error here would be off by exactly 1000x — the kind of mistake
    // that makes every derived figure wrong by three orders of magnitude while still looking plausible.
    assert!(
        (SPEED_OF_LIGHT_M_S / 3.0e8 - 1.0).abs() < 0.01,
        "c should be about 3e8 in its own units, got {SPEED_OF_LIGHT_M_S:e}"
    );
    assert!(
        (SPEED_OF_LIGHT_M_S / 1000.0 / 3.0e5 - 1.0).abs() < 0.01,
        "and about 3e5 when converted to km/s"
    );

    // Light laps Earth's equator about 7.5 times a second. Wrong by 1000x either way and this fails.
    let equator_m = 4.007_5e7;
    let laps_per_second = SPEED_OF_LIGHT_M_S / equator_m;
    assert!(
        (laps_per_second - 7.5).abs() < 0.1,
        "expected about 7.5 laps of the equator per second, got {laps_per_second}"
    );

    // And reaches the Moon in 1.28 s.
    let moon_m = 3.844e8;
    assert!(
        (moon_m / SPEED_OF_LIGHT_M_S - 1.28).abs() < 0.01,
        "light should reach the Moon in 1.28 s"
    );
}
