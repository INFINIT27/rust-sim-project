//! Unit and property tests for [`crate::physics_utils::relativity::doppler`].
//!
//! Compiled into `src/physics_utils/relativity/doppler.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **Direction.** A receding source is lowered in frequency and has a positive redshift. Both
//!    directions have the same magnitude in logarithm, so only a test of the sign can tell a correct
//!    implementation from an inverted one.
//! 2. **The factor follows from the event transformation.** Two wave crests are two events;
//!    transforming them with `transform_event` and timing their arrival gives the Doppler factor
//!    without using its formula.
//! 3. **Factors multiply when velocities compose**, checked against `compose_velocities`.
//! 4. **The transverse shift is time dilation**, checked against `lorentz_factor`.

use super::*;
use crate::physics_utils::relativity::lorentz::lorentz_factor;
use crate::physics_utils::relativity::spacetime::transform_event;
use crate::physics_utils::relativity::velocity::compose_velocities;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use std::f64::consts::PI;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_doppler_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// Direction and exact values.
// -----------------------------------------------------------------------

#[test]
fn receding_lowers_the_frequency_and_approaching_raises_it() {
    let receding = doppler_factor(0.3 * C).expect("subluminal");
    let approaching = doppler_factor(-0.3 * C).expect("subluminal");

    assert!(receding < 1.0, "moving apart must redden");
    assert!(approaching > 1.0, "closing must blue");
    assert!((receding * approaching - 1.0).abs() < 1.0e-15);

    assert!(redshift(0.3 * C).expect("subluminal") > 0.0);
    assert!(redshift(-0.3 * C).expect("subluminal") < 0.0);
}

#[test]
fn the_factor_is_rational_at_the_pythagorean_speeds() {
    // sqrt((1 - b)/(1 + b)): 1/2 at 3/5, 1/3 at 4/5, 2/3 at 5/13, 1/5 at 12/13.
    for (beta, expected) in [
        (3.0 / 5.0, 1.0 / 2.0),
        (4.0 / 5.0, 1.0 / 3.0),
        (5.0 / 13.0, 2.0 / 3.0),
        (12.0 / 13.0, 1.0 / 5.0),
    ] {
        let factor = doppler_factor(beta * C).expect("subluminal");
        assert!((factor - expected).abs() < 1.0e-14, "beta {beta}: {factor}");

        let z = redshift(beta * C).expect("subluminal");
        assert!(
            (z - (1.0 / expected - 1.0)).abs() < 1.0e-13,
            "beta {beta}: z = {z}"
        );
    }
}

#[test]
fn at_rest_nothing_shifts() {
    assert_eq!(doppler_factor(0.0), Some(1.0));
    assert_eq!(redshift(0.0), Some(0.0));
    assert_eq!(transverse_doppler_factor(0.0), Some(1.0));
    assert_eq!(observed_frequency(5.0e14, 0.0), Some(5.0e14));
    assert_eq!(velocity_from_redshift(0.0), Some(0.0));
}

#[test]
fn hydrogen_alpha_from_a_receding_galaxy() {
    // The H-alpha line is at 656.28 nm. From a source receding at 0.1c it is observed at
    // 656.28 * sqrt(1.1/0.9) = 725.55 nm: z = 0.10554.
    let z = redshift(0.1 * C).expect("subluminal");
    assert!((z - 0.105_54).abs() < 1.0e-5, "z = {z}");
    assert!((656.28 * (1.0 + z) - 725.55).abs() < 0.01);

    // And the frequency drops by the same ratio.
    let emitted = C / 656.28e-9;
    let observed = observed_frequency(emitted, 0.1 * C).expect("valid");
    assert!((emitted / observed - (1.0 + z)).abs() < 1.0e-14);
}

#[test]
fn a_radar_echo_is_shifted_twice() {
    // A police radar at 24.15 GHz and a car approaching at 40 m/s. The car receives a raised
    // frequency and re-emits it, so the echo is shifted by the factor squared: 2*f*v/c = 6,444 Hz.
    let sent = 24.15e9;
    let at_car = observed_frequency(sent, -40.0).expect("valid");
    let echo = observed_frequency(at_car, -40.0).expect("valid");
    assert!(
        ((echo - sent) - 6_444.0).abs() < 5.0,
        "beat = {} Hz",
        echo - sent
    );
}

// -----------------------------------------------------------------------
// Redshift: the stable form.
// -----------------------------------------------------------------------

#[test]
fn the_redshift_survives_where_subtracting_one_does_not() {
    // At 1 mm/s, beta is 3.3e-12 and z equals beta to twelve digits. The naive form subtracts 1 from
    // a number that differs from 1 in the twelfth place, and keeps about four digits.
    let v = 1.0e-3;
    let beta = v / C;
    let stable = redshift(v).expect("subluminal");
    let naive = 1.0 / doppler_factor(v).expect("subluminal") - 1.0;

    assert!(
        (stable / beta - 1.0).abs() < 1.0e-11,
        "stable z = {stable:e}"
    );
    assert!(
        (naive / beta - 1.0).abs() > 1.0e-7,
        "if the naive form is this good, `redshift` no longer needs its own formula"
    );
}

#[test]
fn at_low_speed_the_redshift_is_beta_plus_half_beta_squared() {
    // z = beta + beta^2/2 + ...; the second term is the relativistic (time dilation) part.
    let v = 29_780.0;
    let beta = v / C;
    let z = redshift(v).expect("subluminal");
    assert!((z - beta - 0.5 * beta * beta).abs() < beta.powi(3));
}

#[test]
fn a_redshift_of_minus_one_or_less_has_no_velocity() {
    assert_eq!(velocity_from_redshift(-1.0), None);
    assert_eq!(velocity_from_redshift(-2.0), None);
    assert_eq!(velocity_from_redshift(f64::NAN), None);
    assert_eq!(velocity_from_redshift(f64::INFINITY), None);
}

#[test]
fn a_huge_redshift_saturates_below_light_speed() {
    for z in [1.0e9, 1.0e100, 1.0e300] {
        let v = velocity_from_redshift(z).expect("valid redshift");
        assert!(v < C, "z = {z:e} gave exactly c");
        assert!(lorentz_factor(v).is_some());
    }
    // And the blueshift limit approaches -c the same way.
    let v = velocity_from_redshift(-1.0 + 1.0e-12).expect("valid redshift");
    assert!(v > -C && v < -0.999_999 * C);
}

// -----------------------------------------------------------------------
// The transverse shift and the general angle.
// -----------------------------------------------------------------------

#[test]
fn the_transverse_shift_is_time_dilation_and_nothing_else() {
    for beta in [0.01, 0.3, 0.6, 0.99] {
        let v = beta * C;
        let factor = transverse_doppler_factor(v).expect("subluminal");
        let gamma = lorentz_factor(v).expect("subluminal");
        assert!((factor * gamma - 1.0).abs() < 1.0e-14, "beta {beta}");
        assert!(factor < 1.0, "always a lowering");
        assert_eq!(
            transverse_doppler_factor(-v),
            Some(factor),
            "whichever way it crosses"
        );
    }
}

#[test]
fn the_ives_stilwell_average_shows_the_second_order_shift() {
    // Ives and Stilwell (1938) looked at a hydrogen beam from in front and behind. Classically the
    // two wavelengths average to the rest wavelength. Relativistically the average is gamma times
    // it: a shift of beta^2/2. Their beam was at about beta = 0.005.
    let v = 0.005 * C;
    let ahead = 1.0 / doppler_factor(-v).expect("subluminal");
    let behind = 1.0 / doppler_factor(v).expect("subluminal");
    let average = 0.5 * (ahead + behind);

    let gamma = lorentz_factor(v).expect("subluminal");
    assert!((average - gamma).abs() < 1.0e-15);
    assert!(
        (average - 1.0 - 1.25e-5).abs() < 1.0e-9,
        "beta^2/2 = 1.25e-5"
    );
}

#[test]
fn the_angle_form_reduces_to_the_three_named_cases() {
    for beta in [0.1, 0.6, 0.9] {
        let v = beta * C;
        let approaching = doppler_factor_at_angle(v, 0.0).expect("valid");
        let crossing = doppler_factor_at_angle(v, PI / 2.0).expect("valid");
        let receding = doppler_factor_at_angle(v, PI).expect("valid");

        assert!((approaching / doppler_factor(-v).expect("valid") - 1.0).abs() < 1.0e-14);
        assert!((crossing / transverse_doppler_factor(v).expect("valid") - 1.0).abs() < 1.0e-14);
        assert!((receding / doppler_factor(v).expect("valid") - 1.0).abs() < 1.0e-14);
    }
}

#[test]
fn there_is_an_angle_with_no_shift_at_all() {
    // Between "approaching, blue" and "crossing, red" the two effects cancel: at
    // cos(theta) = (1 - 1/gamma)/beta. For beta = 0.6 that is cos = 1/3.
    let v = 0.6 * C;
    let angle = (1.0_f64 / 3.0).acos();
    let factor = doppler_factor_at_angle(v, angle).expect("valid");
    assert!((factor - 1.0).abs() < 1.0e-14, "factor = {factor}");
}

#[test]
fn the_angle_form_rejects_a_negative_speed() {
    // The direction is the angle's job. A negative speed would be a second way to say the same
    // thing, and the two could disagree.
    assert_eq!(doppler_factor_at_angle(-0.5 * C, 0.0), None);
    assert_eq!(doppler_factor_at_angle(0.5 * C, f64::NAN), None);
    assert_eq!(doppler_factor_at_angle(C, 0.0), None);
}

#[test]
fn light_speed_and_bad_inputs_have_no_factor() {
    for bad in [C, -C, 2.0 * C, f64::NAN, f64::INFINITY] {
        assert_eq!(doppler_factor(bad), None);
        assert_eq!(redshift(bad), None);
        assert_eq!(transverse_doppler_factor(bad), None);
        assert_eq!(observed_frequency(1.0, bad), None);
    }
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(observed_frequency(bad, 0.5 * C), None);
    }
}

// -----------------------------------------------------------------------
// Derived from the event transformation, without the formula.
// -----------------------------------------------------------------------

#[test]
fn the_factor_follows_from_transforming_two_wave_crests() {
    // A source at rest at the origin of S emits crests at t = 0 and t = T, towards +x. An observer
    // rides S', moving at +v: receding from the source. In S' the two emission events are at
    // (t1', x1') and (t2', x2'); each crest then travels at c to the observer at the S' origin, so it
    // arrives at t' + |x'|/c. The ratio of periods is the Doppler factor.
    let period = 1.0;
    for beta in [0.1, 0.6, 0.9] {
        let v = beta * C;
        let (first_time, first_position) = transform_event(0.0, 0.0, v).expect("subluminal");
        let (second_time, second_position) = transform_event(period, 0.0, v).expect("subluminal");

        let first_arrival = first_time + first_position.abs() / C;
        let second_arrival = second_time + second_position.abs() / C;
        let observed_period = second_arrival - first_arrival;

        let factor = doppler_factor(v).expect("subluminal");
        assert!(
            (period / observed_period / factor - 1.0).abs() < 1.0e-14,
            "beta {beta}: derived {} against {}",
            period / observed_period,
            factor
        );
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

    /// **Doppler factors multiply when velocities compose.** A relay that receives and re-emits
    /// shifts by its own factor, and the two shifts in a row equal the shift of the combined
    /// velocity.
    #[test]
    fn factors_multiply_under_composition(
        first in -0.99_f64..0.99,
        second in -0.99_f64..0.99,
    ) {
        let product = doppler_factor(first * C).expect("subluminal")
            * doppler_factor(second * C).expect("subluminal");
        let composed = compose_velocities(first * C, second * C).expect("valid");
        let direct = doppler_factor(composed).expect("subluminal");

        // (1 - beta) of the composed velocity carries the rounding; up to ~1e4 amplification.
        prop_assert!(
            (direct / product - 1.0).abs() < 1.0e-11,
            "{} then {}: {} against {}", first, second, direct, product
        );
    }

    /// Reversing the velocity inverts the factor.
    #[test]
    fn approach_and_recession_are_reciprocal(beta in -0.999_f64..0.999) {
        let there = doppler_factor(beta * C).expect("subluminal");
        let back = doppler_factor(-beta * C).expect("subluminal");
        prop_assert!((there * back - 1.0).abs() < 1.0e-13);
    }

    /// The redshift agrees with the factor wherever the factor has digits to spare, and has the sign
    /// of the velocity everywhere.
    #[test]
    fn the_redshift_matches_the_factor(beta in -0.99_f64..0.99) {
        let z = redshift(beta * C).expect("subluminal");
        let factor = doppler_factor(beta * C).expect("subluminal");
        prop_assert!(((1.0 + z) * factor - 1.0).abs() < 1.0e-13);
        prop_assert!(z > -1.0);
        prop_assert!(z == 0.0 || (z > 0.0) == (beta > 0.0));
    }

    /// Redshift to velocity and back.
    #[test]
    fn redshift_round_trips(beta in -0.99_f64..0.99) {
        let z = redshift(beta * C).expect("subluminal");
        let v = velocity_from_redshift(z).expect("z > -1");
        prop_assert!((v / C - beta).abs() < 1.0e-13, "beta {} came back as {}", beta, v / C);
    }

    /// The angle form is largest head-on, smallest directly away, and decreases in between.
    #[test]
    fn the_shift_decreases_as_the_source_turns_away(
        beta in 0.01_f64..0.99,
        near in 0.0_f64..PI,
        far in 0.0_f64..PI,
    ) {
        let (near, far) = if near <= far { (near, far) } else { (far, near) };
        let closer = doppler_factor_at_angle(beta * C, near).expect("valid");
        let further = doppler_factor_at_angle(beta * C, far).expect("valid");
        prop_assert!(closer >= further * (1.0 - 1.0e-14));
    }
}
