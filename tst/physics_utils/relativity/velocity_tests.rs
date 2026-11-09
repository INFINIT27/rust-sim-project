//! Unit and property tests for [`crate::physics_utils::relativity::velocity`].
//!
//! Compiled into `src/physics_utils/relativity/velocity.rs` via `#[path]`.
//!
//! # The two claims worth the most here
//!
//! 1. **`c` is invariant** — the second postulate of special relativity coming out of arithmetic, and
//!    the reason `transform_velocity` accepts an object at `±c` while refusing a frame at `±c`. Exact
//!    in real arithmetic; in `f64` it holds to `1.9e-12` relative, worst as `β → 1`.
//!    `light_speed_is_invariant_to_within_a_rounding` pins that bound, and
//!    `the_invariance_is_often_bit_exact_but_not_always` records the measurement so nobody strengthens
//!    the claim back to equality — an earlier version of these tests did, and it failed at `β = 0.9`.
//! 2. **You cannot reach `c` by adding velocities** — driven as a property over a wide range of pairs,
//!    since it is the claim a reader is least likely to take on trust.
//!
//! Plus the one distinguishing test: `transform_velocity` and `compose_velocities` must *not* be the
//! same function. Both reduce to Galilean addition at low speed and both look symmetric, so a test at
//! everyday speeds cannot tell them apart — `the_two_directions_are_not_the_same_function` is what
//! catches a swapped sign.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_velocity_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// The invariance of c. The centrepiece.
// -----------------------------------------------------------------------

#[test]
fn light_speed_is_invariant_to_within_a_rounding() {
    // The second postulate. `(c-v)/(1-v/c) = c(c-v)/(c-v) = c` exactly in real arithmetic; in f64 the
    // numerator and denominator round independently, so the tolerance is `1e-11` relative. That bound
    // is measured, not guessed — see `the_invariance_is_often_bit_exact_but_not_always`.
    for beta in [0.0, 0.1, 0.5, 0.9, 0.999, 0.999_99, -0.5, -0.9] {
        let v = beta * C;

        for (signed_c, name) in [(C, "forward"), (-C, "backward")] {
            let transformed = transform_velocity(signed_c, v)
                .unwrap_or_else(|| panic!("a {name} photon at beta {beta} must transform"));
            assert!(
                (transformed / signed_c - 1.0).abs() < 1.0e-11,
                "a {name} photon seen from beta {beta} came out {}c",
                transformed / C
            );

            let composed = compose_velocities(signed_c, v).expect("valid");
            assert!(
                (composed / signed_c - 1.0).abs() < 1.0e-11,
                "composing a {name} photon with beta {beta} gave {}c",
                composed / C
            );
        }
    }
}

#[test]
fn the_invariance_is_often_bit_exact_but_not_always() {
    // The measurement behind the tolerance above, recorded so nobody tightens it back to `assert_eq!`.
    // An earlier version of these tests did exactly that and failed at beta = 0.9, one ULP low.
    //
    // Over 10,000 values of beta: bit-identical for most, and the worst deviation is a couple of parts
    // in 1e12, reached as beta approaches 1 where `1 - beta^2` loses its significant digits.
    let mut exact = 0;
    let mut worst = 0.0_f64;
    let samples = 10_000;

    for i in 1..=samples {
        let beta = (i as f64 / (samples as f64 + 1.0)) * 0.999_999;
        let result = transform_velocity(C, beta * C).expect("subluminal frame");

        if result == C {
            exact += 1;
        }
        worst = worst.max((result / C - 1.0).abs());
    }

    assert!(
        exact * 2 > samples,
        "the majority of betas should still be bit-exact; only {exact} of {samples} were"
    );
    assert!(
        worst < 1.0e-11,
        "worst relative deviation was {worst:e}, which exceeds the tolerance the other tests use"
    );
    assert!(
        worst > 0.0,
        "if this is now exact everywhere, the tolerance elsewhere can be tightened to equality"
    );
}

#[test]
fn a_frame_may_not_move_at_light_speed_but_an_object_may() {
    // The asymmetry that makes the test above possible, and it is physics rather than convenience:
    // light has no rest frame, so `c` is a legitimate velocity to transform and an illegitimate frame
    // to transform into.
    assert!(transform_velocity(C, 0.5 * C).is_some());
    assert_eq!(transform_velocity(0.5 * C, C), None);
    assert_eq!(transform_velocity(0.5 * C, -C), None);

    // And beyond `c` nothing is accepted on either side.
    assert_eq!(transform_velocity(1.5 * C, 0.5 * C), None);
    assert_eq!(transform_velocity(0.5 * C, 1.5 * C), None);
}

// -----------------------------------------------------------------------
// Velocities do not add the way they used to.
// -----------------------------------------------------------------------

#[test]
fn half_light_speed_twice_is_four_fifths_not_all_of_it() {
    // The textbook example: (0.5 + 0.5)/(1 + 0.25) = 0.8 exactly.
    let sum = compose_velocities(0.5 * C, 0.5 * C).expect("valid");

    assert!(
        (sum / C - 0.8).abs() < 1.0e-12,
        "expected 0.8c, got {}c",
        sum / C
    );
}

#[test]
fn two_near_light_velocities_still_fall_short() {
    // 0.9c + 0.9c = 0.994475c, against a Galilean 1.8c. The single most counter-intuitive number in
    // elementary relativity.
    let sum = compose_velocities(0.9 * C, 0.9 * C).expect("valid");

    assert!(sum < C, "composed to {}c", sum / C);
    assert!(
        (sum / C - 0.994_475_138).abs() < 1.0e-8,
        "expected 0.994475c, got {}c",
        sum / C
    );
}

#[test]
fn at_everyday_speeds_it_is_galilean_addition() {
    // The correspondence principle: 3 m/s plus 4 m/s is 7 m/s to well past any precision anyone could
    // measure. The relativistic correction here is about 1e-16 m/s.
    let sum = compose_velocities(3.0, 4.0).expect("valid");

    assert!((sum - 7.0).abs() < 1.0e-12, "got {sum}");

    // And a genuinely fast aircraft: 1000 m/s plus 1000 m/s.
    let sum = compose_velocities(1_000.0, 1_000.0).expect("valid");
    assert!((sum - 2_000.0).abs() < 1.0e-6, "got {sum}");
}

// -----------------------------------------------------------------------
// The two directions are different functions.
// -----------------------------------------------------------------------

#[test]
fn the_two_directions_are_not_the_same_function() {
    // The guard against a swapped sign. Both reduce to Galilean addition at low speed and both look
    // symmetric, so only a relativistic asymmetric case separates them: transforming *removes* the
    // frame's motion, composing *adds* it.
    let u = 0.3 * C;
    let v = 0.6 * C;

    let removed = transform_velocity(u, v).expect("valid");
    let added = compose_velocities(u, v).expect("valid");

    assert!(
        removed < 0.0,
        "removing a faster frame should reverse the sign, got {removed}"
    );
    assert!(added > u, "adding should increase it, got {added}");
    assert!(
        (removed - added).abs() > 0.1 * C,
        "the two directions must differ substantially: {removed} vs {added}"
    );
}

#[test]
fn composing_is_transforming_with_the_frame_reversed() {
    // The identity the implementation relies on, pinned so the delegation cannot be "optimised" into a
    // second copy of the algebra with its own sign error.
    for (u, v) in [(0.3, 0.6), (-0.5, 0.2), (0.0, 0.9), (0.99, -0.99)] {
        assert_eq!(
            compose_velocities(u * C, v * C),
            transform_velocity(u * C, -v * C),
            "u {u}c, v {v}c"
        );
    }
}

#[test]
fn a_stationary_frame_changes_nothing_and_a_stationary_object_takes_the_frames_speed() {
    for beta in [0.1, 0.5, 0.9] {
        // No relative motion, no transformation.
        assert_eq!(transform_velocity(beta * C, 0.0), Some(beta * C));
        assert_eq!(compose_velocities(beta * C, 0.0), Some(beta * C));

        // An object at rest in S recedes at exactly the frame's speed as seen from S'.
        let seen = transform_velocity(0.0, beta * C).expect("valid");
        assert!(
            (seen + beta * C).abs() < 1.0e-6,
            "beta {beta}: expected -{}, got {seen}",
            beta * C
        );
    }
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn non_finite_inputs_have_no_answer() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(transform_velocity(bad, 0.5 * C), None, "object {bad}");
        assert_eq!(transform_velocity(0.5 * C, bad), None, "frame {bad}");
        assert_eq!(compose_velocities(bad, 0.5 * C), None);
        assert_eq!(compose_velocities(0.5 * C, bad), None);
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

    /// **Composing two subluminal velocities never reaches `c`.**
    ///
    /// The property that makes `c` a speed limit rather than a convention. Driven over the full range
    /// of both arguments including near-`c` pairs, because "0.9c + 0.9c < c" is the claim a reader is
    /// least likely to accept without evidence.
    #[test]
    fn you_cannot_reach_light_speed_by_adding(
        u in -0.999_999_f64..0.999_999,
        v in -0.999_999_f64..0.999_999,
    ) {
        let sum = compose_velocities(u * C, v * C).expect("both subluminal");

        prop_assert!(
            sum.abs() < C,
            "{}c + {}c composed to {}c", u, v, sum / C
        );
    }

    /// The same, for the transforming direction.
    #[test]
    fn transforming_never_produces_a_superluminal_velocity(
        u in -0.999_999_f64..0.999_999,
        v in -0.999_999_f64..0.999_999,
    ) {
        let result = transform_velocity(u * C, v * C).expect("both subluminal");
        prop_assert!(result.abs() < C, "got {}c", result / C);
    }

    /// Transforming into a frame and back out again is the identity.
    #[test]
    fn a_boost_and_its_inverse_cancel(
        u in -0.99_f64..0.99,
        v in -0.99_f64..0.99,
    ) {
        let there = transform_velocity(u * C, v * C).expect("subluminal");
        let back = compose_velocities(there, v * C).expect("subluminal");

        prop_assert!(
            (back - u * C).abs() / C < 1.0e-9,
            "{}c -> {}c -> {}c", u, there / C, back / C
        );
    }

    /// Composition is commutative: the order of the two velocities does not matter.
    ///
    /// True for collinear boosts and *false* in general — two perpendicular boosts do not commute,
    /// which is why this module is single-axis. Worth pinning as the reason that restriction exists.
    #[test]
    fn collinear_composition_commutes(
        u in -0.99_f64..0.99,
        v in -0.99_f64..0.99,
    ) {
        let one = compose_velocities(u * C, v * C).expect("subluminal");
        let other = compose_velocities(v * C, u * C).expect("subluminal");

        prop_assert!((one - other).abs() / C < 1.0e-12, "{one} vs {other}");
    }

    /// Composition agrees with Galilean addition when both speeds are slow.
    ///
    /// The correspondence principle, as a property rather than one example: below `1e5 m/s` the
    /// relativistic correction is under a part in `1e6`, so anything that disagreed by more than that
    /// would have a term in the wrong place.
    #[test]
    fn slow_composition_is_newtonian(
        u in -1.0e5_f64..1.0e5,
        v in -1.0e5_f64..1.0e5,
    ) {
        let relativistic = compose_velocities(u, v).expect("subluminal");
        let galilean = u + v;

        prop_assume!(galilean.abs() > 1.0);
        prop_assert!(
            (relativistic - galilean).abs() / galilean.abs() < 1.0e-6,
            "{} vs {}", relativistic, galilean
        );
    }

    /// Composition is monotonic in the object's velocity, for a fixed frame.
    ///
    /// A faster object in `S'` is a faster object in `S`. Cheap to state and it rules out a whole
    /// family of sign and denominator errors that the exact-value tests would miss for some inputs.
    #[test]
    fn a_faster_object_stays_faster(
        a in -0.99_f64..0.99,
        b in -0.99_f64..0.99,
        v in -0.99_f64..0.99,
    ) {
        prop_assume!((a - b).abs() > 1.0e-6);

        let (slower, faster) = if a < b { (a, b) } else { (b, a) };
        let slow = compose_velocities(slower * C, v * C).expect("subluminal");
        let fast = compose_velocities(faster * C, v * C).expect("subluminal");

        prop_assert!(fast > slow, "{fast} should exceed {slow}");
    }
}
