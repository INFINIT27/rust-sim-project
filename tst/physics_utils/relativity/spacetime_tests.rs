//! Unit and property tests for [`crate::physics_utils::relativity::spacetime`].
//!
//! Compiled into `src/physics_utils/relativity/spacetime.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **The interval is invariant.** `interval_squared_m2` knows nothing about `transform_event`, so
//!    transforming two events and getting the same interval is a real constraint on both.
//! 2. **Time dilation, length contraction and velocity addition all come out of the
//!    transformation.** Each is checked against the module that implements it separately, so a sign
//!    error in the transformation fails against three independent pieces of code.
//! 3. **Order in time is absolute for timelike pairs and not for spacelike ones.** This is causality,
//!    and it is the test that a transformation with `x` and `t` swapped cannot pass.

use super::*;
use crate::physics_utils::relativity::length_contraction::contracted_length;
use crate::physics_utils::relativity::time_dilation::dilated_time;
use crate::physics_utils::relativity::velocity::transform_velocity;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_spacetime_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// The transformation, at values checkable by hand.
// -----------------------------------------------------------------------

#[test]
fn a_frame_at_rest_changes_nothing() {
    assert_eq!(transform_event(3.0, 7.0e8, 0.0), Some((3.0, 7.0e8)));
}

#[test]
fn the_three_four_five_frame_gives_exact_coordinates() {
    // beta = 0.6, gamma = 1.25. The event (t = 1 s, x = 1 light-second) is on the light cone, so it
    // must stay there: t' = 1.25(1 - 0.6) = 0.5 s, x' = 0.5 light-seconds.
    let (time, position) = transform_event(1.0, C, 0.6 * C).expect("subluminal frame");
    assert!((time - 0.5).abs() < 1.0e-12, "t' = {time}");
    assert!(
        (position / C - 0.5).abs() < 1.0e-12,
        "x' = {} ls",
        position / C
    );
}

#[test]
fn the_origin_of_the_moving_frame_stays_at_its_origin() {
    // The point x = vt is where S' has its origin, so it must come out at x' = 0, and its clock
    // reads t/gamma: the moving clock is the slow one.
    let v = 0.8 * C;
    let (time, position) = transform_event(5.0, v * 5.0, v).expect("subluminal frame");
    assert!(position.abs() < 1.0e-6, "x' = {position}");
    assert!((time - 3.0).abs() < 1.0e-12, "t' = {time}");
}

#[test]
fn a_frame_at_or_beyond_light_speed_has_no_coordinates() {
    assert_eq!(transform_event(1.0, 1.0, C), None);
    assert_eq!(transform_event(1.0, 1.0, -C), None);
    assert_eq!(transform_event(1.0, 1.0, 2.0 * C), None);
    assert_eq!(inverse_transform_event(1.0, 1.0, C), None);
}

#[test]
fn non_finite_inputs_have_no_answer() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(transform_event(bad, 0.0, 0.5 * C), None);
        assert_eq!(transform_event(0.0, bad, 0.5 * C), None);
        assert_eq!(transform_event(0.0, 0.0, bad), None);
        assert_eq!(interval_squared_m2(bad, 0.0), None);
        assert_eq!(interval_squared_m2(0.0, bad), None);
        assert_eq!(classify_separation(bad, 0.0), None);
        assert_eq!(proper_time_between(bad, 0.0), None);
        assert_eq!(proper_distance_between(0.0, bad), None);
        assert_eq!(simultaneity_offset_s(bad, 0.5 * C), None);
    }
}

// -----------------------------------------------------------------------
// The other modules, derived from the transformation.
// -----------------------------------------------------------------------

#[test]
fn time_dilation_falls_out_of_the_transformation() {
    // Two ticks of a clock at rest in S: same place, tau apart. S' measures gamma * tau.
    for beta in [0.1, 0.6, 0.8, 0.99] {
        let v = beta * C;
        let (time, _) = transform_event(2.0, 0.0, v).expect("subluminal frame");
        let expected = dilated_time(2.0, v).expect("subluminal");
        assert!(
            (time / expected - 1.0).abs() < 1.0e-14,
            "beta {beta}: {time} against {expected}"
        );
    }
}

#[test]
fn length_contraction_falls_out_of_the_transformation() {
    // A rod at rest in S' with rest length L0, one end at the S' origin. In S, at t = 0, where is
    // the other end? It is the event (t = 0, x = L) whose x' is L0, so L = L0/gamma.
    for beta in [0.1, 0.6, 0.8, 0.99] {
        let v = beta * C;
        let rest_length = 100.0;
        let measured = contracted_length(rest_length, v).expect("subluminal");
        let (_, position) = transform_event(0.0, measured, v).expect("subluminal frame");
        assert!(
            (position / rest_length - 1.0).abs() < 1.0e-14,
            "beta {beta}: the far end came out at {position}"
        );
    }
}

#[test]
fn velocity_addition_falls_out_of_the_transformation() {
    // An object moving at u in S passes through (t, u*t). Its velocity in S' is x'/t'.
    let u = 0.5 * C;
    let v = -0.7 * C;
    let (time, position) = transform_event(4.0, u * 4.0, v).expect("subluminal frame");
    let expected = transform_velocity(u, v).expect("valid");
    assert!(((position / time) / expected - 1.0).abs() < 1.0e-14);
}

// -----------------------------------------------------------------------
// The interval and its kinds.
// -----------------------------------------------------------------------

#[test]
fn the_interval_has_the_sign_of_its_kind() {
    assert!(interval_squared_m2(1.0, 0.5 * C).expect("finite") > 0.0);
    assert_eq!(interval_squared_m2(1.0, C), Some(0.0));
    assert_eq!(interval_squared_m2(-1.0, C), Some(0.0));
    assert!(interval_squared_m2(1.0, 2.0 * C).expect("finite") < 0.0);
}

#[test]
fn a_nearly_lightlike_pair_keeps_its_sign() {
    // 1,024 m short of the light path after a year (a power of two, so the subtraction below is
    // exact: a light-year is 9.46e15 m and `f64` resolves it in steps of 2 m). The difference of
    // squares would have to resolve one part in 1e13 of 9e31; the factored form keeps it.
    let year = 31_557_600.0;
    let light_path = C * year;
    let interval = interval_squared_m2(year, light_path - 1_024.0).expect("finite");
    assert!(interval > 0.0, "a slower-than-light pair must be timelike");
    assert!(
        (interval / (2.0 * light_path * 1_024.0) - 1.0).abs() < 1.0e-9,
        "s^2 should be 2*L*1024 m to first order, got {interval}"
    );
    assert_eq!(
        classify_separation(year, light_path - 1_024.0),
        Some(Separation::Timelike)
    );
}

#[test]
fn classification_does_not_depend_on_which_event_is_first() {
    for (elapsed, separation) in [(1.0, 0.5 * C), (1.0, C), (1.0, 3.0 * C)] {
        let kind = classify_separation(elapsed, separation);
        assert_eq!(classify_separation(-elapsed, separation), kind);
        assert_eq!(classify_separation(elapsed, -separation), kind);
        assert_eq!(classify_separation(-elapsed, -separation), kind);
    }
    assert_eq!(classify_separation(0.0, 0.0), Some(Separation::Lightlike));
}

#[test]
fn proper_time_and_proper_distance_exclude_each_other() {
    // Timelike: a proper time and no proper distance. Spacelike: the reverse. Lightlike: both zero.
    assert!(proper_time_between(1.0, 0.5 * C).is_some());
    assert_eq!(proper_distance_between(1.0, 0.5 * C), None);

    assert_eq!(proper_time_between(1.0, 2.0 * C), None);
    assert!(proper_distance_between(1.0, 2.0 * C).is_some());

    assert_eq!(proper_time_between(1.0, C), Some(0.0));
    assert_eq!(proper_distance_between(1.0, C), Some(0.0));
}

#[test]
fn a_muon_lives_long_enough_to_reach_the_ground() {
    // Cosmic-ray muons are made about 15 km up and live 2.197 microseconds on their own clock. At
    // 0.9995c the trip takes 50 microseconds of ground time, 23 lifetimes, and none should arrive.
    // The proper time of the trip is what the muon's own clock reads.
    let height = 15_000.0;
    let speed = 0.9995 * C;
    let ground_time = height / speed;
    let muon_time = proper_time_between(ground_time, height).expect("timelike");

    assert!(ground_time > 22.0 * 2.197e-6);
    assert!(
        (muon_time / 1.0e-6 - 1.58).abs() < 0.01,
        "the muon's clock should read about 1.58 microseconds, got {muon_time:e}"
    );
    assert!(muon_time < 2.197e-6, "less than one lifetime");
}

#[test]
fn the_detour_takes_less_proper_time() {
    // The twin paradox as arithmetic. Staying home for 10 years is one timelike interval; going out
    // 4 light-years at 0.8c and back is two. The straight path has the longer proper time.
    let year = 31_557_600.0;
    let stay = proper_time_between(10.0 * year, 0.0).expect("timelike");
    let leg = proper_time_between(5.0 * year, 4.0 * C * year).expect("timelike");

    assert!((stay / year - 10.0).abs() < 1.0e-12);
    assert!(
        ((2.0 * leg) / year - 6.0).abs() < 1.0e-9,
        "the traveller ages 6 years"
    );
}

// -----------------------------------------------------------------------
// Simultaneity and causality.
// -----------------------------------------------------------------------

#[test]
fn simultaneous_events_at_one_place_stay_simultaneous() {
    assert_eq!(simultaneity_offset_s(0.0, 0.9 * C), Some(0.0));
    assert_eq!(simultaneity_offset_s(1.0e9, 0.0), Some(0.0));
}

#[test]
fn the_event_ahead_happens_first_for_the_moving_frame() {
    // For a frame moving along +x, the event further along +x is the earlier one, and reversing
    // either the frame or the separation reverses the order.
    let offset = simultaneity_offset_s(C, 0.6 * C).expect("subluminal frame");
    assert!((offset + 0.75).abs() < 1.0e-12, "offset = {offset}");

    let reversed_frame = simultaneity_offset_s(C, -0.6 * C).expect("subluminal frame");
    let reversed_events = simultaneity_offset_s(-C, 0.6 * C).expect("subluminal frame");
    assert!((reversed_frame - 0.75).abs() < 1.0e-12);
    assert!((reversed_events - 0.75).abs() < 1.0e-12);
}

#[test]
fn at_everyday_speeds_simultaneity_is_all_but_absolute() {
    // An airliner at 250 m/s, two events 1,000 km apart: 2.8 nanoseconds.
    let offset = simultaneity_offset_s(1.0e6, 250.0).expect("subluminal frame");
    assert!(
        (offset.abs() / 1.0e-9 - 2.78).abs() < 0.01,
        "got {offset:e}"
    );
}

#[test]
fn a_spacelike_pair_can_be_seen_in_either_order() {
    // Events 1 s and 2 light-seconds apart. A frame at 0.5c sees them at the same instant
    // (v = c^2 dt/dx), a faster one sees them reversed.
    let (same, _) = transform_event(1.0, 2.0 * C, 0.5 * C).expect("subluminal frame");
    let (reversed, _) = transform_event(1.0, 2.0 * C, 0.8 * C).expect("subluminal frame");
    let (kept, _) = transform_event(1.0, 2.0 * C, 0.2 * C).expect("subluminal frame");

    assert!(
        same.abs() < 1.0e-12,
        "simultaneous in this frame, got {same}"
    );
    assert!(reversed < 0.0, "the later event is now the earlier one");
    assert!(kept > 0.0);
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// **The interval is the same in every frame.** The tolerance is relative to
    /// `γ²·((cΔt)² + Δx²)`, the size of the terms the transformed interval is computed from.
    #[test]
    fn the_interval_is_invariant(
        elapsed in -100.0_f64..100.0,
        light_seconds in -100.0_f64..100.0,
        beta in -0.99_f64..0.99,
    ) {
        let separation = light_seconds * C;
        let before = interval_squared_m2(elapsed, separation).expect("finite");
        let (time, position) = transform_event(elapsed, separation, beta * C).expect("subluminal");
        let after = interval_squared_m2(time, position).expect("finite");

        let gamma_squared = 1.0 / (1.0 - beta * beta);
        let scale = gamma_squared * ((C * elapsed).powi(2) + separation.powi(2));
        prop_assert!(
            (after - before).abs() <= 1.0e-13 * scale,
            "s^2 went from {} to {}", before, after
        );
    }

    /// The inverse transformation undoes the transformation.
    #[test]
    fn the_inverse_is_a_round_trip(
        elapsed in -100.0_f64..100.0,
        light_seconds in -100.0_f64..100.0,
        beta in -0.99_f64..0.99,
    ) {
        let separation = light_seconds * C;
        let v = beta * C;
        let (time, position) = transform_event(elapsed, separation, v).expect("subluminal");
        let (back_time, back_position) =
            inverse_transform_event(time, position, v).expect("subluminal");

        // Each coordinate is recovered from terms up to gamma^2 times the larger input.
        let scale = (elapsed.abs() + light_seconds.abs()) / (1.0 - beta * beta);
        prop_assert!((back_time - elapsed).abs() <= 1.0e-13 * scale + 1.0e-300);
        prop_assert!((back_position / C - light_seconds).abs() <= 1.0e-13 * scale + 1.0e-300);
    }

    /// **Causality: the order of a timelike pair is the same in every frame.** Clearly timelike
    /// pairs only (`|Δx| ≤ 0.9·c|Δt|`), so rounding cannot decide the outcome.
    #[test]
    fn a_timelike_pair_keeps_its_order(
        elapsed in 0.001_f64..100.0,
        fraction in -0.9_f64..0.9,
        beta in -0.999_f64..0.999,
    ) {
        let separation = fraction * C * elapsed;
        let (time, position) = transform_event(elapsed, separation, beta * C).expect("subluminal");

        prop_assert!(time > 0.0, "the later event came out earlier: t' = {}", time);
        prop_assert_eq!(classify_separation(time, position), Some(Separation::Timelike));
    }

    /// The kind of a clearly spacelike pair is also the same in every frame, and there is always a
    /// frame that sees the two at once: the one moving at `c²Δt/Δx`.
    #[test]
    fn a_spacelike_pair_has_a_frame_that_sees_it_at_once(
        light_seconds in 1.0_f64..100.0,
        fraction in -0.9_f64..0.9,
    ) {
        let separation = light_seconds * C;
        let elapsed = fraction * light_seconds;
        let frame = fraction * C;
        let (time, position) = transform_event(elapsed, separation, frame).expect("subluminal");

        prop_assert!(time.abs() <= 1.0e-12 * light_seconds, "t' = {}", time);

        // And in that frame the separation is the proper distance.
        let proper = proper_distance_between(elapsed, separation).expect("spacelike");
        prop_assert!((position / proper - 1.0).abs() < 1.0e-12);
    }

    /// The proper time between two events agrees with time dilation at the speed that connects them.
    #[test]
    fn proper_time_agrees_with_time_dilation(
        proper in 0.001_f64..1.0e6,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let elapsed = dilated_time(proper, v).expect("subluminal");
        let recovered = proper_time_between(elapsed, v * elapsed).expect("timelike");

        // The factored radicand loses digits as beta approaches 1: (1 - beta) carries the error.
        prop_assert!(
            (recovered / proper - 1.0).abs() < 1.0e-12,
            "tau {} came back as {}", proper, recovered
        );
    }
}
