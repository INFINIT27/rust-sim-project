//! Unit and property tests for [`crate::physics_utils::relativity::length_contraction`].
//!
//! Compiled into `src/physics_utils/relativity/length_contraction.rs` via `#[path]`.
//!
//! The one test here that could not be written anywhere else is
//! `contraction_and_dilation_scale_by_gamma_in_opposite_directions`. Length contraction *divides* by
//! `γ` where time dilation *multiplies*, and since both modules are three-line wrappers around the
//! same factor, wiring one to the other's helper would produce code that passes every test about
//! magnitudes and is physically inverted. That test is the guard against it.

use super::*;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::time_dilation::dilated_time;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_length_contraction_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// Direction, and the trap.
// -----------------------------------------------------------------------

#[test]
fn contraction_and_dilation_scale_by_gamma_in_opposite_directions() {
    // The single most valuable test in this module. Both effects "diminish the moving thing", so a
    // magnitude test cannot tell a correct implementation from one that divides where it should
    // multiply. This pins the *direction*: a unit interval grows while a unit rod shrinks, and their
    // product is exactly one.
    for beta in [0.1, 0.5, 0.866, 0.99] {
        let v = beta * C;

        let time = dilated_time(1.0, v).expect("subluminal");
        let length = contracted_length(1.0, v).expect("subluminal");

        assert!(time > 1.0, "beta {beta}: time must stretch, got {time}");
        assert!(
            length < 1.0,
            "beta {beta}: length must shrink, got {length}"
        );
        assert!(
            (time * length - 1.0).abs() < 1.0e-12,
            "beta {beta}: gamma and 1/gamma must be reciprocal, got {}",
            time * length
        );
    }
}

#[test]
fn a_gamma_of_two_halves_and_doubles() {
    let v = 3.0_f64.sqrt() / 2.0 * C;

    let short = contracted_length(1.0, v).expect("subluminal");
    assert!((short - 0.5).abs() < 1.0e-12, "got {short}");

    let rest = proper_length(0.5, v).expect("subluminal");
    assert!((rest - 1.0).abs() < 1.0e-12, "got {rest}");
}

#[test]
fn the_moving_ruler_is_always_the_shorter_one() {
    for beta in [0.1, 0.5, 0.9, 0.999] {
        assert!(contracted_length(1.0, beta * C).unwrap() < 1.0);
        assert!(proper_length(1.0, beta * C).unwrap() > 1.0);
    }
}

#[test]
fn direction_of_travel_does_not_matter() {
    for beta in [0.3, 0.7, 0.95] {
        assert_eq!(
            contracted_length(1.0, beta * C),
            contracted_length(1.0, -beta * C)
        );
        assert_eq!(proper_length(1.0, beta * C), proper_length(1.0, -beta * C));
    }
}

// -----------------------------------------------------------------------
// Against figures a reader can check.
// -----------------------------------------------------------------------

#[test]
fn earths_diameter_contracts_by_a_few_centimetres() {
    // 12,742 km at 29,780 m/s. Six centimetres out of twelve thousand kilometres — the number that
    // says why `experiment_3` is right to ignore relativity entirely.
    let deficit = length_contraction_deficit(12_742_000.0, 29_780.0).expect("subluminal");

    assert!(
        (deficit - 0.0629).abs() < 0.001,
        "expected about 6.3 cm, got {deficit} m"
    );
}

#[test]
fn the_iss_contracts_by_tens_of_nanometres() {
    // 109 m at 7,660 m/s.
    let deficit = length_contraction_deficit(109.0, 7_660.0).expect("subluminal");
    let nanometres = deficit * 1.0e9;

    assert!(
        (nanometres - 35.6).abs() < 1.0,
        "expected about 36 nm, got {nanometres} nm"
    );
}

#[test]
fn a_light_year_of_travel_is_a_short_hop_to_the_traveller() {
    // The consequence that makes interstellar travel thinkable, and a good check that the division
    // goes the right way: at beta = 0.999 the distance to Proxima Centauri, 4.246 ly, measures
    // 0.19 ly in the ship's frame.
    let proxima_m = 4.246 * 9.460_730_472_580_8e15;
    let onboard = contracted_length(proxima_m, 0.999 * C).expect("subluminal");
    let light_years = onboard / 9.460_730_472_580_8e15;

    assert!(
        (light_years - 0.19).abs() < 0.01,
        "expected about 0.19 ly, got {light_years}"
    );
}

// -----------------------------------------------------------------------
// `length_contraction_deficit` is not a subtraction.
// -----------------------------------------------------------------------

#[test]
fn the_deficit_has_an_answer_where_subtracting_two_lengths_does_not() {
    let naive = 1.0 - contracted_length(1.0, 1.0).expect("subluminal");
    assert_eq!(
        naive, 0.0,
        "the naive difference must underflow, or this test is moot"
    );

    let stable = length_contraction_deficit(1.0, 1.0).expect("subluminal");
    assert!(
        stable > 0.0,
        "the stable form must still resolve it, got {stable}"
    );
}

#[test]
fn the_deficit_agrees_with_the_subtraction_where_the_subtraction_works() {
    for beta in [0.5, 0.9, 0.99] {
        let v = beta * C;
        let naive = 10.0 - contracted_length(10.0, v).expect("subluminal");
        let stable = length_contraction_deficit(10.0, v).expect("subluminal");

        assert!(
            (naive - stable).abs() / stable < 1.0e-12,
            "beta {beta}: naive {naive:e} vs stable {stable:e}"
        );
    }
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn zero_length_stays_zero_and_zero_speed_changes_nothing() {
    assert_eq!(contracted_length(0.0, 0.9 * C), Some(0.0));
    assert_eq!(proper_length(0.0, 0.9 * C), Some(0.0));
    assert_eq!(length_contraction_deficit(0.0, 0.9 * C), Some(0.0));

    assert_eq!(contracted_length(3.0, 0.0), Some(3.0));
    assert_eq!(proper_length(3.0, 0.0), Some(3.0));
    assert_eq!(length_contraction_deficit(3.0, 0.0), Some(0.0));
}

#[test]
fn a_negative_length_and_a_superluminal_speed_have_no_answer() {
    // A negative length is rejected rather than treated as a direction. Unlike velocity, there is no
    // sign convention here that would make it mean anything.
    assert_eq!(contracted_length(-1.0, 0.0), None);
    assert_eq!(proper_length(-1.0, 0.0), None);
    assert_eq!(length_contraction_deficit(-1.0, 0.0), None);

    for bad in [C, 2.0 * C, f64::NAN, f64::INFINITY] {
        assert_eq!(contracted_length(1.0, bad), None, "speed {bad}");
        assert_eq!(proper_length(1.0, bad), None, "speed {bad}");
        assert_eq!(length_contraction_deficit(1.0, bad), None, "speed {bad}");
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

    /// The two directions are exact inverses.
    #[test]
    fn the_two_directions_round_trip(
        length in 1.0e-6_f64..1.0e9,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let short = contracted_length(length, v).expect("subluminal");
        let back = proper_length(short, v).expect("subluminal");

        prop_assert!(
            (back / length - 1.0).abs() < 1.0e-12,
            "{} -> {} -> {}", length, short, back
        );
    }

    /// A contracted length is never longer than its rest length, and never negative.
    #[test]
    fn contraction_only_ever_shortens(
        length in 0.0_f64..1.0e9,
        beta in -0.999_999_f64..0.999_999,
    ) {
        let short = contracted_length(length, beta * C).expect("subluminal");
        prop_assert!(short >= 0.0, "negative length {short}");
        prop_assert!(short <= length * (1.0 + 1.0e-12), "{short} exceeds {length}");
    }

    /// The deficit is exactly what contraction removed.
    #[test]
    fn the_deficit_closes_the_gap(
        length in 1.0_f64..1.0e9,
        beta in 0.01_f64..0.999,
    ) {
        let v = beta * C;
        let short = contracted_length(length, v).expect("subluminal");
        let deficit = length_contraction_deficit(length, v).expect("subluminal");

        prop_assert!(
            ((short + deficit) / length - 1.0).abs() < 1.0e-12,
            "short {} + deficit {} should be {}", short, deficit, length
        );
    }

    /// Contraction is linear in the length: a rod twice as long contracts twice as much.
    #[test]
    fn contraction_is_linear_in_the_length(
        length in 1.0e-3_f64..1.0e3,
        beta in -0.99_f64..0.99,
    ) {
        let v = beta * C;
        let single = contracted_length(length, v).expect("subluminal");
        let double = contracted_length(2.0 * length, v).expect("subluminal");

        prop_assert!(
            (double / (2.0 * single) - 1.0).abs() < 1.0e-12,
            "{} vs {}", double, 2.0 * single
        );
    }
}
