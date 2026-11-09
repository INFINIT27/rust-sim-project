//! Unit and property tests for [`crate::physics_utils::relativity::lorentz`].
//!
//! Compiled into `src/physics_utils/relativity/lorentz.rs` via `#[path]`.
//!
//! # What is worth asserting about `γ`
//!
//! `γ` is one line of arithmetic, so a test that recomputed `1/√(1−β²)` would assert nothing. These
//! instead pin:
//!
//! - **exact rational values** at the Pythagorean `β`s — `0.6 → 5/4`, `0.8 → 5/3`, `√3/2 → 2` — which
//!   are checkable against any textbook and cannot be produced by a wrong formula;
//! - **published engineering figures**, so the GPS and ISS numbers come from the aerospace literature
//!   rather than from this implementation;
//! - the **domain**, including that `|v| = c` exactly is rejected;
//! - and the **numerical claim** that `lorentz_factor_minus_one` beats subtracting `1.0`, which is the
//!   only reason that function exists and would otherwise be an untested assertion in a doc comment.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. Pinned for the reason given in
/// `newtons_laws_tests`: `#[path]` makes proptest's `file!()`-derived default resolve into `src/`.
const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_lorentz_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// Exact values, against arithmetic anyone can redo by hand.
// -----------------------------------------------------------------------

#[test]
fn gamma_is_exactly_one_at_rest() {
    assert_eq!(lorentz_factor(0.0), Some(1.0));
    assert_eq!(lorentz_factor_minus_one(0.0), Some(0.0));
}

#[test]
fn the_pythagorean_betas_give_exact_rational_gammas() {
    // The three cases every textbook uses, because 1−β² is a perfect square: β=3/5 gives γ=5/4,
    // β=4/5 gives γ=5/3, and β=√3/2 gives γ=2. Any sign error or misplaced exponent misses these.
    let cases = [
        (0.6, 1.25),
        (0.8, 5.0 / 3.0),
        (3.0_f64.sqrt() / 2.0, 2.0),
        (5.0 / 13.0, 13.0 / 12.0),
        (12.0 / 13.0, 13.0 / 5.0),
    ];

    for (beta, expected) in cases {
        let gamma = lorentz_factor(beta * C).expect("subluminal");
        assert!(
            (gamma / expected - 1.0).abs() < 1.0e-12,
            "beta {beta} gave gamma {gamma}, expected {expected}"
        );
    }
}

#[test]
fn gamma_matches_published_engineering_figures() {
    // From the aerospace and metrology literature, not from this code. The GPS number is the one that
    // matters: it is a correction real hardware applies.
    let cases = [
        ("GPS satellite", 3_874.0, 8.35e-11),
        ("ISS", 7_660.0, 3.26e-10),
        ("Earth's orbit", 29_780.0, 4.93e-9),
        ("Mercury's orbit", 47_360.0, 1.25e-8),
    ];

    for (what, speed, expected_gamma_minus_one) in cases {
        let value = lorentz_factor_minus_one(speed).expect("subluminal");
        assert!(
            (value / expected_gamma_minus_one - 1.0).abs() < 0.01,
            "{what}: gamma-1 came out {value:e}, expected about {expected_gamma_minus_one:e}"
        );
    }
}

#[test]
fn beta_is_a_plain_unit_conversion() {
    assert_eq!(beta_from_velocity(0.0), Some(0.0));
    assert_eq!(beta_from_velocity(C), Some(1.0));
    assert_eq!(beta_from_velocity(-C), Some(-1.0));
    assert_eq!(beta_from_velocity(C / 2.0), Some(0.5));

    // Superluminal beta is returned, not rejected — see the function's docs on why the domain check
    // belongs to the callers that care.
    assert_eq!(beta_from_velocity(2.0 * C), Some(2.0));
    assert_eq!(beta_from_velocity(f64::NAN), None);
}

// -----------------------------------------------------------------------
// The domain.
// -----------------------------------------------------------------------

#[test]
fn light_speed_itself_has_no_lorentz_factor() {
    // Exactly `c` is rejected, not merely `> c`. A massive object never reaches it, and light has no
    // rest frame — so there is no interval for `γ` to scale.
    assert_eq!(lorentz_factor(C), None);
    assert_eq!(lorentz_factor(-C), None);
    assert_eq!(lorentz_factor_minus_one(C), None);
    assert!(!is_subluminal(C));
}

#[test]
fn superluminal_and_non_finite_speeds_have_no_answer() {
    for bad in [
        2.0 * C,
        -2.0 * C,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        assert_eq!(lorentz_factor(bad), None, "gamma of {bad}");
        assert_eq!(lorentz_factor_minus_one(bad), None, "gamma-1 of {bad}");
        assert!(!is_subluminal(bad), "{bad} is not subluminal");
    }
}

#[test]
fn a_speed_a_hair_under_c_still_returns_a_usable_number_or_nothing() {
    // The boundary where `1 − β²` rounds to zero and the naive formula hands back an infinity. The
    // contract is that a `Some` is always usable, so this must be either finite or `None` — never
    // `Some(inf)`, which would poison arithmetic downstream while looking like a velocity.
    for scale in [1.0 - 1.0e-15, 1.0 - 1.0e-16, 1.0 - f64::EPSILON] {
        if let Some(gamma) = lorentz_factor(scale * C) {
            assert!(gamma.is_finite() && gamma >= 1.0, "got {gamma}");
        }
    }
}

// -----------------------------------------------------------------------
// The numerical claim that justifies `lorentz_factor_minus_one`.
// -----------------------------------------------------------------------

#[test]
fn the_stable_form_beats_subtracting_one_at_solar_system_speeds() {
    // The measurement in the function's own docs, asserted rather than claimed. At Earth's orbital
    // speed the two forms disagree in the ninth significant figure — the naive one has thrown away
    // half its digits to cancellation.
    let v = 29_780.0;

    let naive = lorentz_factor(v).unwrap() - 1.0;
    let stable = lorentz_factor_minus_one(v).unwrap();

    assert!(stable > 0.0);
    let disagreement = (naive - stable).abs() / stable;
    assert!(
        disagreement > 1.0e-10,
        "the two forms agreed to {disagreement:e}; if the naive form is now this good, \
         `lorentz_factor_minus_one` has lost its reason to exist"
    );
    assert!(
        disagreement < 1.0e-6,
        "they should differ in the ninth digit, not wildly: {disagreement:e}"
    );
}

#[test]
fn the_naive_form_underflows_where_the_stable_one_does_not() {
    // At a walking pace `γ` is `1 + 5.6e-18`, and `f64` cannot hold that beside `1.0` — so the naive
    // difference is exactly zero, an answer that says "relativity does not apply" rather than "this
    // is too small to represent".
    assert_eq!(lorentz_factor(1.0).unwrap() - 1.0, 0.0);

    let stable = lorentz_factor_minus_one(1.0).unwrap();
    assert!(stable > 0.0, "the stable form must still have an answer");
    assert!(
        (stable / 5.563e-18 - 1.0).abs() < 0.01,
        "expected about 5.563e-18, got {stable:e}"
    );
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// `γ ≥ 1` for every subluminal speed, and `= 1` only at rest.
    ///
    /// The defining inequality. A formula with a sign error — `1/√(1+β²)`, say — is below one
    /// everywhere and fails on the first case.
    #[test]
    fn gamma_is_never_below_one(beta in -0.999_999_f64..0.999_999) {
        let gamma = lorentz_factor(beta * C).expect("subluminal");
        prop_assert!(gamma >= 1.0, "gamma {gamma} for beta {beta}");
    }

    /// `γ` depends only on speed, not direction.
    ///
    /// `γ` is a function of `v²`, so a boost and its reverse dilate identically. This is what lets
    /// every function downstream document "the sign is irrelevant".
    #[test]
    fn gamma_is_even_in_velocity(beta in -0.999_f64..0.999) {
        let forward = lorentz_factor(beta * C).expect("subluminal");
        let backward = lorentz_factor(-beta * C).expect("subluminal");
        prop_assert_eq!(forward, backward);
    }

    /// `γ` grows monotonically with speed.
    #[test]
    fn faster_is_always_more_dilated(a in 0.0_f64..0.99, b in 0.0_f64..0.99) {
        prop_assume!((a - b).abs() > 1.0e-6);

        let (slower, faster) = if a < b { (a, b) } else { (b, a) };
        let g_slow = lorentz_factor(slower * C).expect("subluminal");
        let g_fast = lorentz_factor(faster * C).expect("subluminal");

        prop_assert!(g_fast > g_slow, "beta {faster} gave {g_fast}, beta {slower} gave {g_slow}");
    }

    /// The stable and naive forms agree wherever the naive one has digits left.
    ///
    /// Above `β ≈ 1e-4` there is no cancellation worth worrying about, so the two must match closely.
    /// This is what stops `lorentz_factor_minus_one` from being a second, subtly different `γ`.
    #[test]
    fn the_two_forms_agree_where_cancellation_is_not_a_problem(beta in 0.001_f64..0.999) {
        let naive = lorentz_factor(beta * C).expect("subluminal") - 1.0;
        let stable = lorentz_factor_minus_one(beta * C).expect("subluminal");

        prop_assert!(
            (naive - stable).abs() / stable < 1.0e-9,
            "beta {}: naive {:e} vs stable {:e}", beta, naive, stable
        );
    }

    /// `is_subluminal` agrees with `lorentz_factor` about the domain, exactly.
    ///
    /// Two guards for one rule is how they drift apart; this is what keeps them one rule.
    #[test]
    fn the_guard_and_the_factor_agree_on_the_domain(v in -2.0_f64..2.0) {
        let speed = v * C;
        prop_assert_eq!(is_subluminal(speed), lorentz_factor(speed).is_some());
    }
}
