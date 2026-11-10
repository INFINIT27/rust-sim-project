//! Unit and property tests for the [`crate::time::simulation`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/time/simulation.rs` via `#[path]`, not as a standalone integration test.
//!
//! Two subjects, and the split matters: [`TimeStep`] is a *guarantee* — the tests
//! assert that nothing invalid can be built and that the guarantee survives
//! splitting — and [`SimulationClock`] is a *policy*: clamp, scale, accumulate,
//! pause. The clock tests are written against real frame deltas so the arithmetic is
//! checked in the units it actually runs in.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/time/proptest-regressions/simulation_tests.txt";

// -----------------------------------------------------------------------
// TimeStep: the guarantee.
// -----------------------------------------------------------------------

#[test]
fn a_valid_duration_becomes_a_step() {
    let step = TimeStep::from_seconds(0.25).expect("a quarter second is a duration");

    assert_eq!(step.seconds(), 0.25);
    assert_eq!(step.seconds_f32(), 0.25_f32);
    assert!(step.is_valid());
    assert!(!step.is_zero());
}

#[test]
fn zero_is_a_legal_step() {
    assert!(TimeStep::ZERO.is_zero());
    assert!(TimeStep::ZERO.is_valid());
    assert_eq!(TimeStep::ZERO.seconds(), 0.0);
    assert_eq!(TimeStep::from_seconds(0.0), Some(TimeStep::ZERO));
    // And `Default` is zero, so a derived struct holding one starts stopped.
    assert_eq!(TimeStep::default(), TimeStep::ZERO);
}

#[test]
fn nothing_invalid_can_become_a_step() {
    // This is the enforcement the whole interface rests on: there is no other
    // constructor from a float, so a physics function cannot be handed one of these.
    assert_eq!(TimeStep::from_seconds(-1.0), None);
    assert_eq!(TimeStep::from_seconds(-1.0e-300), None);
    assert_eq!(TimeStep::from_seconds(f64::NAN), None);
    assert_eq!(TimeStep::from_seconds(f64::INFINITY), None);
    assert_eq!(TimeStep::from_seconds(f64::NEG_INFINITY), None);
}

#[test]
fn splitting_a_step_divides_it_and_keeps_it_valid() {
    let step = TimeStep::from_seconds(1.0).unwrap();

    assert_eq!(step.split(4).seconds(), 0.25);
    assert_eq!(step.split(1), step);
    assert!(step.split(7).is_valid());
}

#[test]
fn splitting_into_zero_parts_yields_zero_rather_than_dividing_by_zero() {
    let step = TimeStep::from_seconds(1.0).unwrap();
    assert_eq!(step.split(0), TimeStep::ZERO);
}

// -----------------------------------------------------------------------
// clamp_real_step: Requirement 13, as a function.
// -----------------------------------------------------------------------

#[test]
fn clamping_follows_requirement_13() {
    // 13.1: in-range passes through.
    assert_eq!(clamp_real_step(0.016, 0.1), 0.016);
    // 13.2: larger saturates.
    assert_eq!(clamp_real_step(2.5, 0.1), 0.1);
    // 13.3: negative and non-finite become zero.
    assert_eq!(clamp_real_step(-0.5, 0.1), 0.0);
    assert_eq!(clamp_real_step(f32::NAN, 0.1), 0.0);
    assert_eq!(clamp_real_step(f32::INFINITY, 0.1), 0.0);
    assert_eq!(clamp_real_step(f32::NEG_INFINITY, 0.1), 0.0);
    // Exactly at the cap is in range.
    assert_eq!(clamp_real_step(0.1, 0.1), 0.1);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "max_real_step_s must be finite and strictly positive")]
fn clamping_against_a_nonsense_cap_panics_in_debug() {
    let _ = clamp_real_step(0.016, 0.0);
}

#[cfg(not(debug_assertions))]
#[test]
fn clamping_against_a_nonsense_cap_stalls_in_release() {
    // Returning 0.0 stalls the simulation rather than stepping it by an unbounded
    // amount, which is the safer of the two failures.
    assert_eq!(clamp_real_step(0.016, 0.0), 0.0);
    assert_eq!(clamp_real_step(0.016, f32::NAN), 0.0);
}

// -----------------------------------------------------------------------
// SimulationClock: the policy.
// -----------------------------------------------------------------------

#[test]
fn a_new_clock_starts_at_zero_running_and_valid() {
    let clock = SimulationClock::new(1000.0);

    assert_eq!(clock.elapsed_seconds(), 0.0);
    assert_eq!(clock.scale(), 1000.0);
    assert!(!clock.is_paused());
    assert!(clock.last_step().is_zero());
    assert!(clock.is_valid());
    assert_eq!(clock.max_real_step_seconds(), MAX_REAL_STEP_SECONDS);
}

#[test]
fn the_default_clock_is_real_time() {
    let clock = SimulationClock::default();
    assert_eq!(clock.scale(), DEFAULT_TIME_SCALE);
}

#[test]
fn advancing_scales_the_delta_and_accumulates_it() {
    // 1/64 and 1/32 of a second: exactly representable in both `f32` and `f64`, so
    // the assertions can be exact instead of approximate. A `0.01` frame would not
    // be — `0.01f32 as f64` is `0.009999999776...`, and asserting `10.0` against it
    // fails for reasons that have nothing to do with the clock.
    let mut clock = SimulationClock::new(1000.0);

    assert_eq!(clock.advance(1.0 / 64.0).seconds(), 15.625);
    assert_eq!(clock.advance(1.0 / 32.0).seconds(), 31.25);
    assert_eq!(clock.elapsed_seconds(), 46.875);
    assert_eq!(clock.last_step().seconds(), 31.25);
}

#[test]
fn the_clamp_applies_before_the_scale() {
    // The cap is on the *real* interval, so the largest simulated step is
    // `max_real_step_seconds() * scale()`. Scaling first would make the cap mean a
    // different thing at every scale, which is why the order is asserted here.
    let mut clock = SimulationClock::new(1000.0);

    let step = clock.advance(5.0);
    assert_eq!(
        step.seconds(),
        f64::from(MAX_REAL_STEP_SECONDS) * 1000.0,
        "a 5-second stall must be capped at the real ceiling, then scaled"
    );
}

#[test]
fn an_unusable_delta_produces_a_zero_step_and_stops_the_clock() {
    let mut clock = SimulationClock::new(1000.0);
    clock.advance(0.01);
    let before = clock.elapsed_seconds();

    assert!(clock.advance(f32::NAN).is_zero());
    assert!(clock.advance(-1.0).is_zero());
    assert_eq!(
        clock.elapsed_seconds(),
        before,
        "elapsed time must not move"
    );
    assert!(clock.is_valid());
}

#[test]
fn pausing_stops_simulated_time_without_stopping_the_caller() {
    let mut clock = SimulationClock::new(1000.0);
    clock.advance(1.0 / 64.0);
    let elapsed = clock.elapsed_seconds();

    clock.set_paused(true);
    assert!(clock.advance(1.0 / 64.0).is_zero());
    assert!(clock.advance(1.0 / 64.0).is_zero());
    assert_eq!(clock.elapsed_seconds(), elapsed);

    clock.set_paused(false);
    assert_eq!(clock.advance(1.0 / 64.0).seconds(), 15.625);
    assert_eq!(clock.elapsed_seconds(), elapsed + 15.625);
}

#[test]
fn toggling_flips_the_pause_state() {
    let mut clock = SimulationClock::default();

    clock.toggle_paused();
    assert!(clock.is_paused());
    clock.toggle_paused();
    assert!(!clock.is_paused());
}

#[test]
fn a_single_step_request_lets_exactly_one_frame_through() {
    let mut clock = SimulationClock::new(1000.0);
    clock.set_paused(true);

    clock.request_single_step();
    assert!(clock.single_step_pending());

    assert_eq!(
        clock.advance(1.0 / 64.0).seconds(),
        15.625,
        "the queued frame runs"
    );
    assert!(!clock.single_step_pending(), "and the request is consumed");
    assert!(
        clock.advance(1.0 / 64.0).is_zero(),
        "the clock re-pauses itself"
    );
    assert_eq!(clock.elapsed_seconds(), 15.625);
}

#[test]
fn single_step_requests_do_not_accumulate() {
    // A boolean, not a counter: a held key must not queue up frames and then run the
    // simulation away when it is released.
    let mut clock = SimulationClock::new(1000.0);
    clock.set_paused(true);

    for _ in 0..10 {
        clock.request_single_step();
    }

    assert_eq!(clock.advance(1.0 / 64.0).seconds(), 15.625);
    assert!(clock.advance(1.0 / 64.0).is_zero());
    assert_eq!(clock.elapsed_seconds(), 15.625, "one frame, not ten");
}

#[test]
fn a_step_request_while_running_does_not_lie_in_wait() {
    // Requesting a step while running is meaningless; leaving the flag set would fire
    // the moment the user next paused, which would look like the pause key failing.
    let mut clock = SimulationClock::new(1000.0);

    clock.request_single_step();
    clock.advance(1.0 / 64.0);
    assert!(!clock.single_step_pending());

    clock.set_paused(true);
    assert!(clock.advance(1.0 / 64.0).is_zero());
}

#[test]
fn the_scale_is_clamped_into_its_usable_range() {
    let mut clock = SimulationClock::default();

    clock.set_scale(MAX_TIME_SCALE * 1000.0);
    assert_eq!(clock.scale(), MAX_TIME_SCALE);

    clock.set_scale(0.0);
    assert_eq!(
        clock.scale(),
        MIN_TIME_SCALE,
        "below the floor, pause instead"
    );

    clock.set_scale(-5.0);
    assert_eq!(clock.scale(), MIN_TIME_SCALE);

    assert!(clock.is_valid());
}

#[test]
fn a_non_finite_scale_is_ignored_rather_than_defaulted() {
    let mut clock = SimulationClock::new(1000.0);

    clock.set_scale(f32::NAN);
    assert_eq!(
        clock.scale(),
        1000.0,
        "the clock must not jump to real time"
    );
    clock.set_scale(f32::INFINITY);
    assert_eq!(clock.scale(), 1000.0);
}

#[test]
fn a_non_finite_construction_scale_falls_back_to_real_time() {
    // A constructor has no previous value to keep, so it must choose something usable.
    assert_eq!(SimulationClock::new(f32::NAN).scale(), DEFAULT_TIME_SCALE);
    assert!(SimulationClock::new(f32::NAN).is_valid());
}

#[test]
fn multiplying_the_scale_moves_it_and_stays_clamped() {
    let mut clock = SimulationClock::new(1000.0);

    clock.multiply_scale(2.0);
    assert_eq!(clock.scale(), 2000.0);
    clock.multiply_scale(0.5);
    assert_eq!(clock.scale(), 1000.0);

    // Runs out of range rather than out of validity.
    for _ in 0..200 {
        clock.multiply_scale(2.0);
    }
    assert_eq!(clock.scale(), MAX_TIME_SCALE);
    assert!(clock.is_valid());
}

#[test]
fn a_zero_or_negative_factor_is_refused() {
    let mut clock = SimulationClock::new(1000.0);

    // Zero would freeze the simulation in a way that looks like a hang rather than a
    // pause; negative would mean reverse time, which this crate does not implement.
    clock.multiply_scale(0.0);
    assert_eq!(clock.scale(), 1000.0);
    clock.multiply_scale(-2.0);
    assert_eq!(clock.scale(), 1000.0);
    clock.multiply_scale(f32::NAN);
    assert_eq!(clock.scale(), 1000.0);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Whatever the frame delta and the scale, a clock stays valid and never hands
    /// out an invalid step. This is the property every physics function downstream
    /// relies on instead of validating for itself.
    #[test]
    fn a_clock_never_produces_an_invalid_step(
        scale in 1.0_f32..1.0e8,
        deltas in prop::collection::vec(-0.5_f32..2.0, 1..30),
    ) {
        let mut clock = SimulationClock::new(scale);

        for delta in deltas {
            let step = clock.advance(delta);
            prop_assert!(step.is_valid());
            prop_assert!(clock.is_valid());
            prop_assert!(step.seconds() <= f64::from(MAX_REAL_STEP_SECONDS) * f64::from(clock.scale()) + 1.0e-6);
        }
    }

    /// Elapsed time is the sum of the steps handed out — the clock is an accumulator
    /// and nothing else, which is what makes it comparable against
    /// `Scene::two_body_free_fall_seconds`.
    #[test]
    fn elapsed_time_is_the_sum_of_the_steps(
        deltas in prop::collection::vec(0.0_f32..0.05, 1..40),
    ) {
        let mut clock = SimulationClock::new(1000.0);
        let mut total = 0.0_f64;

        for delta in deltas {
            total += clock.advance(delta).seconds();
        }

        prop_assert!((clock.elapsed_seconds() - total).abs() <= 1.0e-9 * total.max(1.0));
    }

    /// A paused clock is inert: no step, no elapsed time, whatever it is fed.
    #[test]
    fn a_paused_clock_never_advances(deltas in prop::collection::vec(0.0_f32..1.0, 1..20)) {
        let mut clock = SimulationClock::new(1.0e6);
        clock.set_paused(true);

        for delta in deltas {
            prop_assert!(clock.advance(delta).is_zero());
        }

        prop_assert_eq!(clock.elapsed_seconds(), 0.0);
    }

    /// Splitting a step never produces a longer one, and the parts are valid — the
    /// guarantee substepping depends on.
    #[test]
    fn substeps_are_never_longer_than_their_parent(seconds in 0.0_f64..1.0e6, parts in 1_u32..256) {
        let step = TimeStep::from_seconds(seconds).unwrap();
        let part = step.split(parts);

        prop_assert!(part.is_valid());
        prop_assert!(part.seconds() <= step.seconds());
    }
}

#[test]
fn a_hold_stops_the_clock_and_the_pause_key_cannot_release_it() {
    let mut clock = SimulationClock::new(10.0);
    clock.hold();
    assert!(clock.advance(0.05).is_zero(), "held, though not paused");

    // `P` during a plan: the user's pause flips, the hold stays.
    clock.toggle_paused();
    clock.toggle_paused();
    assert!(!clock.is_paused());
    assert!(clock.advance(0.05).is_zero(), "still held");

    clock.release();
    assert!(
        !clock.advance(0.05).is_zero(),
        "running again once released"
    );
}

#[test]
fn a_pause_set_during_a_hold_survives_its_release() {
    let mut clock = SimulationClock::new(10.0);
    clock.hold();
    clock.toggle_paused();
    clock.release();
    assert!(clock.is_paused());
    assert!(
        clock.advance(0.05).is_zero(),
        "the user's pause outlasts the plan"
    );
}
