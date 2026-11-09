//! Unit and property tests for [`crate::physics_utils::relativity::time_dilation`].
//!
//! Compiled into `src/physics_utils/relativity/time_dilation.rs` via `#[path]`.
//!
//! The claims that matter here are about *direction* — which clock reads less — and about the
//! published engineering figures. `dilated_time` is one multiplication, so a test that redid the
//! multiplication would assert nothing; a test that says "the GPS satellite loses 7.2 µs a day"
//! catches a `γ` used upside down, which is the actual failure mode.

use super::*;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_time_dilation_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// Direction: which clock reads less. Every sign confusion lands here.
// -----------------------------------------------------------------------

#[test]
fn the_moving_clock_always_reads_less() {
    // The entire content of time dilation, stated as an inequality rather than a formula.
    for beta in [0.1, 0.5, 0.9, 0.999] {
        let coordinate = dilated_time(1.0, beta * C).expect("subluminal");
        let proper = proper_time(1.0, beta * C).expect("subluminal");

        assert!(
            coordinate > 1.0,
            "beta {beta}: one proper second must be more than one of ours, got {coordinate}"
        );
        assert!(
            proper < 1.0,
            "beta {beta}: one of our seconds must be less than one proper, got {proper}"
        );
    }
}

#[test]
fn a_gamma_of_two_doubles_and_halves() {
    // beta = sqrt(3)/2 gives gamma exactly 2, so the two directions are readable by eye.
    let v = 3.0_f64.sqrt() / 2.0 * C;

    let out = dilated_time(1.0, v).expect("subluminal");
    assert!((out - 2.0).abs() < 1.0e-12, "got {out}");

    let back = proper_time(2.0, v).expect("subluminal");
    assert!((back - 1.0).abs() < 1.0e-12, "got {back}");
}

#[test]
fn direction_of_travel_does_not_matter() {
    // gamma depends on v squared, so approaching and receding dilate identically.
    for beta in [0.3, 0.7, 0.95] {
        assert_eq!(dilated_time(1.0, beta * C), dilated_time(1.0, -beta * C));
        assert_eq!(proper_time(1.0, beta * C), proper_time(1.0, -beta * C));
        assert_eq!(
            clock_lag_seconds(1.0, beta * C),
            clock_lag_seconds(1.0, -beta * C)
        );
    }
}

// -----------------------------------------------------------------------
// Against published figures rather than against the code.
// -----------------------------------------------------------------------

#[test]
fn a_gps_satellite_loses_about_seven_microseconds_a_day() {
    // The published special-relativistic figure for the GPS constellation is -7.2 us/day at an
    // orbital speed of about 3874 m/s. This is a correction real hardware applies, so it is a genuine
    // external check on both the formula and the sign.
    let lag = clock_lag_seconds(86_400.0, 3_874.0).expect("subluminal");
    let microseconds = lag * 1.0e6;

    assert!(
        (microseconds - 7.2).abs() < 0.1,
        "expected about 7.2 us/day, got {microseconds}"
    );
}

#[test]
fn the_gps_figure_is_only_the_special_relativistic_half() {
    // Worth pinning as a limitation, not just a value: the *net* GPS correction is about +38 us/day,
    // because gravitational time dilation runs the other way and is larger. This crate has no general
    // relativity, so anyone reading 7.2 us as "the GPS correction" would be wrong by a factor of five
    // and in the wrong direction.
    let sr_only = clock_lag_seconds(86_400.0, 3_874.0).expect("subluminal") * 1.0e6;

    assert!(
        sr_only < 38.0,
        "the SR term alone must be smaller than the net correction; got {sr_only}"
    );
}

#[test]
fn the_iss_loses_about_twenty_eight_microseconds_a_day() {
    let lag = clock_lag_seconds(86_400.0, 7_660.0).expect("subluminal") * 1.0e6;

    assert!(
        (lag - 28.2).abs() < 0.5,
        "expected about 28.2 us/day, got {lag}"
    );
}

#[test]
fn earths_orbital_motion_costs_about_a_sixth_of_a_second_a_year() {
    // The figure relevant to this crate: at 29,780 m/s, Earth's clock falls 0.156 s behind the Sun's
    // frame per year. Small, real, and far below anything `experiment_3` could show.
    let julian_year_s = 365.25 * 86_400.0;
    let lag = clock_lag_seconds(julian_year_s, 29_780.0).expect("subluminal");

    assert!(
        (lag - 0.156).abs() < 0.005,
        "expected about 0.156 s/year, got {lag}"
    );
}

#[test]
fn a_muon_at_point_nine_nine_nine_four_c_lives_about_twenty_nine_times_longer() {
    // The textbook demonstration, and the reason cosmic-ray muons reach the ground at all: a 2.197 us
    // proper lifetime at beta = 0.9994 becomes about 63 us in the lab.
    let proper_lifetime_s = 2.197e-6;
    let lab = dilated_time(proper_lifetime_s, 0.9994 * C).expect("subluminal");

    assert!(
        (lab / proper_lifetime_s - 28.87).abs() < 0.1,
        "gamma came out {}",
        lab / proper_lifetime_s
    );
    assert!(
        (lab * 1.0e6 - 63.4).abs() < 0.5,
        "lab lifetime came out {} us",
        lab * 1.0e6
    );
}

// -----------------------------------------------------------------------
// `clock_lag_seconds` is not a subtraction.
// -----------------------------------------------------------------------

#[test]
fn the_lag_has_an_answer_where_subtracting_two_times_does_not() {
    // The reason the function exists. At a walking pace over a day, `t - t/gamma` is exactly zero in
    // f64 because gamma is exactly 1.0; the stable form still resolves the real 4.8e-13 s.
    let day = 86_400.0;

    let naive = day - proper_time(day, 1.0).expect("subluminal");
    assert_eq!(
        naive, 0.0,
        "the naive difference must underflow, or this test is moot"
    );

    let stable = clock_lag_seconds(day, 1.0).expect("subluminal");
    assert!(
        stable > 0.0,
        "the stable form must still have an answer, got {stable}"
    );
}

#[test]
fn the_lag_agrees_with_the_subtraction_where_the_subtraction_still_works() {
    // At relativistic speeds there is no cancellation, so the two must match — which is what stops
    // `clock_lag_seconds` from being a second, subtly different definition of dilation.
    for beta in [0.5, 0.9, 0.99] {
        let v = beta * C;
        let naive = 10.0 - proper_time(10.0, v).expect("subluminal");
        let stable = clock_lag_seconds(10.0, v).expect("subluminal");

        assert!(
            (naive - stable).abs() / stable < 1.0e-12,
            "beta {beta}: naive {naive:e} vs stable {stable:e}"
        );
    }
}

// -----------------------------------------------------------------------
// Domain and degenerate inputs.
// -----------------------------------------------------------------------

#[test]
fn zero_time_stays_zero_and_zero_speed_changes_nothing() {
    assert_eq!(dilated_time(0.0, 0.9 * C), Some(0.0));
    assert_eq!(proper_time(0.0, 0.9 * C), Some(0.0));
    assert_eq!(clock_lag_seconds(0.0, 0.9 * C), Some(0.0));

    assert_eq!(dilated_time(5.0, 0.0), Some(5.0));
    assert_eq!(proper_time(5.0, 0.0), Some(5.0));
    assert_eq!(clock_lag_seconds(5.0, 0.0), Some(0.0));
}

#[test]
fn a_negative_interval_and_a_superluminal_speed_have_no_answer() {
    // Negative time is rejected rather than absorbed: unlike a velocity, an interval has no sign
    // convention that would make it meaningful, so it is a caller error and not a direction.
    assert_eq!(dilated_time(-1.0, 0.0), None);
    assert_eq!(proper_time(-1.0, 0.0), None);
    assert_eq!(clock_lag_seconds(-1.0, 0.0), None);

    for bad in [C, 2.0 * C, f64::NAN, f64::INFINITY] {
        assert_eq!(dilated_time(1.0, bad), None, "speed {bad}");
        assert_eq!(proper_time(1.0, bad), None, "speed {bad}");
        assert_eq!(clock_lag_seconds(1.0, bad), None, "speed {bad}");
    }

    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(dilated_time(bad, 0.5 * C), None);
        assert_eq!(proper_time(bad, 0.5 * C), None);
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

    /// `dilated_time` and `proper_time` are exact inverses.
    #[test]
    fn the_two_directions_round_trip(
        tau in 1.0e-6_f64..1.0e6,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let there = dilated_time(tau, v).expect("subluminal");
        let back = proper_time(there, v).expect("subluminal");

        prop_assert!(
            (back / tau - 1.0).abs() < 1.0e-12,
            "tau {} -> {} -> {}", tau, there, back
        );
    }

    /// The lag is exactly the gap between the two directions.
    ///
    /// Ties the third function to the first two, so it cannot become an independent claim about
    /// dilation that happens to look plausible.
    #[test]
    fn the_lag_is_the_difference_between_the_clocks(
        t in 1.0_f64..1.0e6,
        beta in 0.01_f64..0.999,
    ) {
        let v = beta * C;
        let proper = proper_time(t, v).expect("subluminal");
        let lag = clock_lag_seconds(t, v).expect("subluminal");

        prop_assert!(
            ((proper + lag) / t - 1.0).abs() < 1.0e-12,
            "proper {} + lag {} should be t {}", proper, lag, t
        );
    }

    /// Dilation scales linearly with the interval: twice the proper time, twice the coordinate time.
    #[test]
    fn dilation_is_linear_in_the_interval(
        tau in 1.0e-3_f64..1.0e3,
        beta in -0.99_f64..0.99,
    ) {
        let v = beta * C;
        let single = dilated_time(tau, v).expect("subluminal");
        let double = dilated_time(2.0 * tau, v).expect("subluminal");

        prop_assert!(
            (double / (2.0 * single) - 1.0).abs() < 1.0e-12,
            "{} vs {}", double, 2.0 * single
        );
    }

    /// The lag never exceeds the elapsed time, and is never negative.
    ///
    /// The moving clock can stop, but it cannot run backwards — so the lag is bounded by `t` however
    /// close to `c` the speed gets.
    #[test]
    fn the_lag_is_bounded_by_the_elapsed_time(
        t in 1.0_f64..1.0e6,
        beta in -0.999_999_f64..0.999_999,
    ) {
        let lag = clock_lag_seconds(t, beta * C).expect("subluminal");
        prop_assert!(lag >= 0.0, "negative lag {lag}");
        prop_assert!(lag <= t, "lag {lag} exceeds elapsed {t}");
    }
}
