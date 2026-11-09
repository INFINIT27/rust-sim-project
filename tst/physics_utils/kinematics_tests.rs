//! Unit and property tests for the [`crate::physics_utils::kinematics`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/physics_utils/kinematics.rs` via `#[path]`, not as a standalone
//! integration test, so these tests keep access to the module's private items and
//! to `super::*`.
//!
//! The interesting properties here are the *cross-checks*: the seven relations
//! are not independent, so each can be verified against the others. A round-trip
//! through `velocity_after` and `time_to_reach_velocity`, or agreement between
//! `position_after(.., a = 0)` and `inertial_position`, pins far more than any
//! single formula asserted against a hand-computed number.

use super::*;
use crate::physics_utils::newtons_laws::{acceleration_from_net_force, inertial_position};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/kinematics_tests.txt";

/// Relative-error comparison against the expected magnitude.
fn assert_close(actual: f64, expected: f64, relative_tolerance: f64, what: &str) {
    let error = (actual - expected).abs();
    let scale = expected.abs().max(f64::MIN_POSITIVE);
    assert!(
        error / scale <= relative_tolerance,
        "{}: expected {:e}, got {:e} (relative error {:e} exceeds {:e})",
        what,
        expected,
        actual,
        error / scale,
        relative_tolerance
    );
}

/// Positions, in metres.
fn valid_position() -> impl Strategy<Value = f64> {
    -1.0e6..1.0e6_f64
}

/// Signed velocities, in metres per second.
fn valid_velocity() -> impl Strategy<Value = f64> {
    -1.0e4..1.0e4_f64
}

/// Signed accelerations, in metres per second squared.
fn valid_acceleration() -> impl Strategy<Value = f64> {
    -1.0e3..1.0e3_f64
}

/// Non-negative durations, in seconds. Bounded so `½·a·t²` stays well inside
/// `f64` and the cross-check tolerances stay meaningful.
fn valid_duration() -> impl Strategy<Value = f64> {
    0.0..1.0e3_f64
}

/// Strictly positive durations, for the two averaging relations.
fn positive_duration() -> impl Strategy<Value = f64> {
    1.0e-6..1.0e3_f64
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1: `average_velocity` inverts `position_after`.
    ///
    /// The mean velocity over an interval, recomputed from the endpoints, must
    /// match the analytic mean `v₀ + ½·a·t` for constant acceleration. This ties
    /// the definition of velocity to the trajectory formula.
    #[test]
    fn average_velocity_matches_the_analytic_mean(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        elapsed in positive_duration(),
    ) {
        let finish = position_after(start, velocity, acceleration, elapsed);
        let measured = average_velocity(start, finish, elapsed);
        let analytic = velocity + 0.5 * acceleration * elapsed;

        assert_close(measured, analytic, 1.0e-9, "mean velocity must be v0 + a*t/2");
    }

    /// Property 2: for constant acceleration the mean velocity is the midpoint of
    /// the endpoint velocities.
    ///
    /// An independent statement of the same fact, in terms of velocities rather
    /// than positions — so a sign error in `displacement` cannot satisfy both.
    #[test]
    fn mean_velocity_is_the_midpoint_of_endpoint_velocities(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        elapsed in positive_duration(),
    ) {
        let finish = position_after(start, velocity, acceleration, elapsed);
        let final_velocity = velocity_after(velocity, acceleration, elapsed);

        assert_close(
            average_velocity(start, finish, elapsed),
            0.5 * (velocity + final_velocity),
            1.0e-9,
            "mean velocity must equal (v0 + v1)/2",
        );
    }

    /// Property 3: `average_acceleration` inverts `velocity_after`.
    #[test]
    fn average_acceleration_recovers_the_acceleration(
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        elapsed in positive_duration(),
    ) {
        let final_velocity = velocity_after(velocity, acceleration, elapsed);
        let recovered = average_acceleration(velocity, final_velocity, elapsed);

        assert_close(recovered, acceleration, 1.0e-9, "mean acceleration must recover a");
    }

    /// Property 4: advancing in two steps equals advancing once.
    ///
    /// The exactness claim the module doc makes. Because both relations are the
    /// closed-form solution rather than a numerical integrator, step size must not
    /// change the answer — a body's trajectory is not a function of how often you
    /// looked at it.
    #[test]
    fn advancing_in_two_steps_equals_advancing_once(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        first in 0.0..100.0_f64,
        second in 0.0..100.0_f64,
    ) {
        let state = MotionState::new(start, velocity, acceleration);

        let stepwise = state.advance(first).advance(second);
        let in_one_go = state.advance(first + second);

        assert_close(stepwise.position_m, in_one_go.position_m, 1.0e-9, "position");
        assert_close(stepwise.velocity_m_s, in_one_go.velocity_m_s, 1.0e-9, "velocity");
        prop_assert_eq!(stepwise.acceleration_m_s2, in_one_go.acceleration_m_s2);
    }

    /// Property 5: many small steps agree with one large step.
    ///
    /// The same claim stressed harder — 100 steps rather than 2 — which is what a
    /// caller wondering whether to shrink its timestep actually wants to know.
    #[test]
    fn a_hundred_small_steps_agree_with_one_large_step(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        total in 1.0e-3..100.0_f64,
    ) {
        let state = MotionState::new(start, velocity, acceleration);
        let step = total / 100.0;

        let mut stepwise = state;
        for _ in 0..100 {
            stepwise = stepwise.advance(step);
        }
        let in_one_go = state.advance(total);

        assert_close(stepwise.position_m, in_one_go.position_m, 1.0e-8, "position");
        assert_close(stepwise.velocity_m_s, in_one_go.velocity_m_s, 1.0e-8, "velocity");
    }

    /// Property 6: with zero acceleration, kinematics reduces to Newton's first
    /// law.
    ///
    /// A cross-module identity, and the seam between the two files: asserted
    /// bit-exactly, because `position_after` with `a == 0.0` adds `0.5 * 0.0 * t²`
    /// — an exact zero — to the same expression `inertial_position` computes.
    #[test]
    fn zero_acceleration_reduces_to_inertial_motion(
        start in valid_position(),
        velocity in valid_velocity(),
        elapsed in valid_duration(),
    ) {
        prop_assert_eq!(
            position_after(start, velocity, 0.0, elapsed),
            inertial_position(start, velocity, elapsed)
        );
    }

    /// Property 7: zero elapsed time changes nothing.
    #[test]
    fn zero_elapsed_time_is_a_no_op(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
    ) {
        prop_assert_eq!(position_after(start, velocity, acceleration, 0.0), start);
        prop_assert_eq!(velocity_after(velocity, acceleration, 0.0), velocity);
        prop_assert_eq!(displacement(velocity, acceleration, 0.0), 0.0);

        let state = MotionState::new(start, velocity, acceleration);
        prop_assert_eq!(state.advance(0.0), state);
    }

    /// Property 8: `position_after` is `initial_position + displacement`.
    ///
    /// Bit-exact, because that is literally how it is implemented — this pins the
    /// relationship so a future rewrite of either cannot drift from the other.
    #[test]
    fn position_after_is_start_plus_displacement(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        elapsed in valid_duration(),
    ) {
        prop_assert_eq!(
            position_after(start, velocity, acceleration, elapsed),
            start + displacement(velocity, acceleration, elapsed)
        );
    }

    /// Property 9: the timeless relation agrees with the time-based ones.
    ///
    /// Take a trajectory forward by `t`, then ask `speed_from_displacement` about
    /// the displacement it covered. The answer must be the speed
    /// `velocity_after` reports. Restricted to non-negative velocity and
    /// acceleration so the motion cannot reverse — where it can, the sign is
    /// genuinely unrecoverable and the function only claims a magnitude.
    #[test]
    fn timeless_relation_agrees_with_the_time_based_ones(
        velocity in 0.0..1.0e4_f64,
        acceleration in 0.0..1.0e3_f64,
        elapsed in valid_duration(),
    ) {
        let covered = displacement(velocity, acceleration, elapsed);
        let expected = velocity_after(velocity, acceleration, elapsed);

        let speed = speed_from_displacement(velocity, acceleration, covered)
            .expect("forward motion under non-negative acceleration is always reachable");

        assert_close(speed, expected, 1.0e-6, "speed at a known displacement");
    }

    /// Property 10: `speed_from_displacement` is never negative, and is `None`
    /// exactly when the radicand is.
    #[test]
    fn speed_from_displacement_is_non_negative_or_none(
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        offset in -1.0e6..1.0e6_f64,
    ) {
        let radicand = velocity * velocity + 2.0 * acceleration * offset;

        match speed_from_displacement(velocity, acceleration, offset) {
            Some(speed) => {
                prop_assert!(radicand >= 0.0, "got Some({:e}) for radicand {:e}", speed, radicand);
                prop_assert!(speed >= 0.0, "speed must be non-negative, got {:e}", speed);
                prop_assert!(speed.is_finite(), "speed must be finite, got {:e}", speed);
            }
            None => prop_assert!(radicand < 0.0, "got None for radicand {:e}", radicand),
        }
    }

    /// Property 11: `time_to_reach_velocity` round-trips through
    /// `velocity_after`.
    ///
    /// Ask where a body will be after `t`, then ask when it gets there; the answer
    /// must be `t`. Acceleration is bounded away from zero so a target is always
    /// reachable.
    #[test]
    fn time_to_reach_velocity_round_trips(
        velocity in valid_velocity(),
        acceleration in prop_oneof![-1.0e3..-1.0e-3_f64, 1.0e-3..1.0e3_f64],
        elapsed in 1.0e-3..1.0e3_f64,
    ) {
        let target = velocity_after(velocity, acceleration, elapsed);
        let recovered = time_to_reach_velocity(velocity, target, acceleration)
            .expect("a target produced by velocity_after must be reachable");

        assert_close(recovered, elapsed, 1.0e-6, "recovered time");
    }

    /// Property 12: the returned time is never negative.
    ///
    /// The whole point of returning `Option`: a solution in the past is reported
    /// as unreachable, never as a negative duration that a caller might add to its
    /// clock.
    #[test]
    fn time_to_reach_velocity_is_never_negative(
        velocity in valid_velocity(),
        target in valid_velocity(),
        acceleration in valid_acceleration(),
    ) {
        if let Some(elapsed) = time_to_reach_velocity(velocity, target, acceleration) {
            prop_assert!(elapsed >= 0.0, "time must be non-negative, got {:e}", elapsed);
            prop_assert!(elapsed.is_finite(), "time must be finite, got {:e}", elapsed);
        }
    }

    /// Property 13: `MotionState` methods agree with the free functions they
    /// delegate to.
    ///
    /// Bit-exact. Guards the one real risk of having both an object and a
    /// functional API: that they drift into two implementations of the same law.
    #[test]
    fn motion_state_delegates_faithfully(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        mass in 1.0e-3..1.0e6_f64,
        elapsed in valid_duration(),
    ) {
        let state = MotionState::new(start, velocity, acceleration);
        let advanced = state.advance(elapsed);

        prop_assert_eq!(
            advanced.position_m,
            position_after(start, velocity, acceleration, elapsed)
        );
        prop_assert_eq!(
            advanced.velocity_m_s,
            velocity_after(velocity, acceleration, elapsed)
        );
        prop_assert_eq!(
            state.momentum(mass),
            crate::physics_utils::conservation_laws::momentum(mass, velocity)
        );
        prop_assert_eq!(
            state.kinetic_energy(mass),
            crate::physics_utils::conservation_laws::kinetic_energy(mass, velocity)
        );
        prop_assert_eq!(
            state.net_force(mass),
            crate::physics_utils::newtons_laws::net_force(mass, acceleration)
        );
        prop_assert_eq!(state.speed(), velocity.abs());
    }

    /// Property 14: `advance` carries acceleration over untouched.
    ///
    /// The documented division of labour: this module integrates motion, the
    /// caller decides the forces. A step that quietly damped or zeroed
    /// acceleration would break every caller's force model.
    #[test]
    fn advance_preserves_acceleration(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        elapsed in valid_duration(),
    ) {
        let state = MotionState::new(start, velocity, acceleration);

        prop_assert_eq!(state.advance(elapsed).acceleration_m_s2, acceleration);
    }

    /// Property 15: a valid state stays valid across a step.
    #[test]
    fn advance_preserves_validity(
        start in valid_position(),
        velocity in valid_velocity(),
        acceleration in valid_acceleration(),
        elapsed in valid_duration(),
    ) {
        let state = MotionState::new(start, velocity, acceleration);
        prop_assert!(state.is_valid());
        prop_assert!(state.advance(elapsed).is_valid());
    }

    /// Property 16: the second law closes the loop from force to trajectory.
    ///
    /// Convert a force to an acceleration, advance, then recover the mean
    /// acceleration from the velocity change; it must be the force divided by the
    /// mass. This is the full chain `newtons_laws` → `kinematics` → back, which is
    /// how a caller will actually use both modules together.
    #[test]
    fn force_to_trajectory_and_back(
        force in -1.0e6..1.0e6_f64,
        mass in 1.0e-3..1.0e3_f64,
        velocity in valid_velocity(),
        elapsed in positive_duration(),
    ) {
        let acceleration = acceleration_from_net_force(force, mass);
        let state = MotionState::new(0.0, velocity, acceleration);
        let advanced = state.advance(elapsed);

        let recovered = average_acceleration(velocity, advanced.velocity_m_s, elapsed);

        assert_close(recovered, force / mass, 1.0e-9, "F/m recovered from the trajectory");
    }
}

// ---- Averaging relations: hand-written cases ----

/// Mean velocity is displacement over time, with sign.
#[test]
fn average_velocity_is_displacement_over_time() {
    assert_eq!(average_velocity(0.0, 10.0, 2.0), 5.0);
    assert_eq!(average_velocity(10.0, 0.0, 2.0), -5.0);
    assert_eq!(average_velocity(3.0, 3.0, 1.0), 0.0);
}

/// Mean acceleration is velocity change over time, with sign.
#[test]
fn average_acceleration_is_velocity_change_over_time() {
    assert_eq!(average_acceleration(0.0, 60.0, 5.0), 12.0);
    assert_eq!(average_acceleration(20.0, 0.0, 4.0), -5.0);
    assert_eq!(average_acceleration(7.0, 7.0, 2.0), 0.0);
}

// ---- Forward relations: hand-written cases ----

/// Free fall from rest for two seconds, taking down as negative.
///
/// `v = −9.81 · 2` and `s = ½ · −9.81 · 4` are both exact in `f64` because
/// multiplying by 2 and 4 only shifts the exponent.
#[test]
fn free_fall_from_rest_for_two_seconds() {
    assert_eq!(velocity_after(0.0, -9.81, 2.0), -19.62);
    assert_eq!(displacement(0.0, -9.81, 2.0), -19.62);
    assert_eq!(position_after(0.0, 0.0, -9.81, 2.0), -19.62);
}

/// A body thrown straight up returns to its starting point.
///
/// Thrown at `9.81 m/s` under `−9.81 m/s²`: at `t = 2 s` the displacement is zero
/// and the velocity is exactly reversed. A good check on the `½·a·t²` term's sign
/// and coefficient, both of which a plausible typo would break.
#[test]
fn a_body_thrown_up_returns_with_reversed_velocity() {
    assert_eq!(displacement(9.81, -9.81, 2.0), 0.0);
    assert_eq!(velocity_after(9.81, -9.81, 2.0), -9.81);
}

/// Constant velocity gives linear displacement.
#[test]
fn constant_velocity_gives_linear_displacement() {
    assert_eq!(displacement(3.0, 0.0, 4.0), 12.0);
    assert_eq!(velocity_after(3.0, 0.0, 4.0), 3.0);
}

// ---- Timeless relation: hand-written cases ----

/// Impact speed after falling a known distance.
///
/// Dropped from rest, `19.62 m` down at `9.81 m/s²`: `√(2 · 9.81 · 19.62)`.
#[test]
fn impact_speed_after_a_known_drop() {
    let speed = speed_from_displacement(0.0, -9.81, -19.62).expect("the ground is reachable");

    assert_close(speed, 19.62, 1.0e-12, "impact speed");
}

/// A displacement beyond the apex is unreachable.
///
/// Thrown up at `10 m/s`, the apex is at `10²/(2 · 9.81) ≈ 5.1 m`; `100 m` is
/// never reached, so the radicand is negative and the answer is `None` rather
/// than a `NaN` or a fabricated zero.
#[test]
fn a_displacement_beyond_the_apex_is_unreachable() {
    assert_eq!(speed_from_displacement(10.0, -9.81, 100.0), None);
}

/// At the apex itself the speed is zero.
///
/// The boundary of the reachable set: the radicand is exactly zero here in
/// principle, so this also confirms the `< 0.0` test is not `<= 0.0`.
#[test]
fn at_the_apex_the_speed_is_zero() {
    let apex_height = 10.0 * 10.0 / (2.0 * 9.81);
    let speed = speed_from_displacement(10.0, -9.81, apex_height).expect("the apex is reachable");

    assert!(
        speed < 1.0e-6,
        "speed at the apex should be ~0, got {:e}",
        speed
    );
}

/// Zero displacement returns the initial speed, whatever the acceleration.
#[test]
fn zero_displacement_returns_the_initial_speed() {
    assert_eq!(speed_from_displacement(7.0, -9.81, 0.0), Some(7.0));
    assert_eq!(speed_from_displacement(-7.0, 3.0, 0.0), Some(7.0));
}

// ---- Time relation: hand-written cases ----

/// The ordinary case, and the already-there case.
#[test]
fn time_to_reach_velocity_solves_the_ordinary_cases() {
    assert_eq!(time_to_reach_velocity(0.0, 60.0, 12.0), Some(5.0));
    assert_eq!(time_to_reach_velocity(20.0, 0.0, -5.0), Some(4.0));
    assert_eq!(time_to_reach_velocity(5.0, 5.0, 0.0), Some(0.0));
    assert_eq!(time_to_reach_velocity(5.0, 5.0, 3.0), Some(0.0));
}

/// A coasting body never reaches a different velocity.
///
/// The case that would divide by zero if `Option` were not in play. Note it is
/// reported as unreachable rather than as an error: the inputs are perfectly
/// valid, the question just has no answer.
#[test]
fn a_coasting_body_never_reaches_a_different_velocity() {
    assert_eq!(time_to_reach_velocity(5.0, 9.0, 0.0), None);
    assert_eq!(time_to_reach_velocity(5.0, -9.0, 0.0), None);
}

/// Accelerating away from the target puts the solution in the past.
#[test]
fn accelerating_away_from_the_target_is_unreachable() {
    assert_eq!(time_to_reach_velocity(0.0, 10.0, -2.0), None);
    assert_eq!(time_to_reach_velocity(0.0, -10.0, 2.0), None);
}

// ---- MotionState: hand-written cases ----

/// The default state is at rest at the origin.
#[test]
fn default_motion_state_is_at_rest_at_the_origin() {
    let state = MotionState::default();

    assert_eq!(state.position_m, 0.0);
    assert_eq!(state.velocity_m_s, 0.0);
    assert_eq!(state.acceleration_m_s2, 0.0);
    assert!(state.is_valid());
}

/// `at_rest` sets the position and nothing else.
#[test]
fn at_rest_sets_only_the_position() {
    assert_eq!(MotionState::at_rest(5.0), MotionState::new(5.0, 0.0, 0.0));
}

/// A coasting state advances linearly and keeps its velocity.
#[test]
fn a_coasting_state_advances_linearly() {
    let coasting = MotionState::new(0.0, 3.0, 0.0);

    assert_eq!(coasting.advance(2.0), MotionState::new(6.0, 3.0, 0.0));
    assert_eq!(coasting.advance(0.0), coasting);
}

/// A dropped body's state after two seconds.
#[test]
fn a_dropped_body_advances_under_gravity() {
    let dropped = MotionState::new(100.0, 0.0, -9.81);
    let after = dropped.advance(2.0);

    assert_close(after.position_m, 80.38, 1.0e-12, "position after 2 s");
    assert_eq!(after.velocity_m_s, -19.62);
    assert_eq!(after.acceleration_m_s2, -9.81);
}

/// `is_valid` rejects each non-finite component.
#[test]
fn is_valid_rejects_non_finite_components() {
    assert!(!MotionState::new(f64::NAN, 0.0, 0.0).is_valid());
    assert!(!MotionState::new(0.0, f64::INFINITY, 0.0).is_valid());
    assert!(!MotionState::new(0.0, 0.0, f64::NEG_INFINITY).is_valid());
    assert!(MotionState::new(1.0, 2.0, 3.0).is_valid());
}

/// `speed` is the magnitude of the velocity.
#[test]
fn speed_is_the_magnitude_of_the_velocity() {
    assert_eq!(MotionState::new(0.0, -4.0, 0.0).speed(), 4.0);
    assert_eq!(MotionState::new(0.0, 4.0, 0.0).speed(), 4.0);
    assert_eq!(MotionState::new(0.0, 0.0, 0.0).speed(), 0.0);
}

/// The derived quantities match hand-computed values.
#[test]
fn derived_quantities_match_hand_computed_values() {
    let state = MotionState::new(0.0, 3.0, -9.81);

    assert_eq!(state.momentum(2.0), 6.0);
    assert_eq!(state.kinetic_energy(2.0), 9.0);
    assert_close(state.net_force(2.0), -19.62, 1.0e-12, "F = m*a");
}

// ---- Precondition violations ----
//
// Split by build profile, mirroring `tst/core/scene_tests.rs`: descriptive
// `debug_assert!` panics in debug, documented benign values in release. Both
// `cargo test` and `cargo test --release` must pass.
//
// Note that the two `Option`-returning functions appear in *neither* group for
// their unreachable cases: `None` there is a normal result, covered by the tests
// above. They appear here only for genuinely invalid input.

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "average_velocity: elapsed_s")]
fn zero_elapsed_panics_in_debug_for_average_velocity() {
    average_velocity(0.0, 1.0, 0.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "average_acceleration: elapsed_s")]
fn zero_elapsed_panics_in_debug_for_average_acceleration() {
    average_acceleration(0.0, 1.0, 0.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "velocity_after: elapsed_s")]
fn negative_elapsed_panics_in_debug_for_velocity_after() {
    velocity_after(0.0, 1.0, -1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "displacement: acceleration_m_s2")]
fn non_finite_acceleration_panics_in_debug_for_displacement() {
    displacement(0.0, f64::NAN, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "position_after: initial_position_m")]
fn non_finite_start_panics_in_debug_for_position_after() {
    position_after(f64::INFINITY, 0.0, 0.0, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "speed_from_displacement: displacement_m")]
fn non_finite_displacement_panics_in_debug() {
    speed_from_displacement(0.0, 1.0, f64::NAN);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "time_to_reach_velocity: acceleration_m_s2")]
fn non_finite_acceleration_panics_in_debug_for_time() {
    time_to_reach_velocity(0.0, 1.0, f64::NAN);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "MotionState::advance: dt_s")]
fn negative_dt_panics_in_debug_for_advance() {
    MotionState::new(0.0, 1.0, 0.0).advance(-1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "MotionState::advance: every component")]
fn an_invalid_state_panics_in_debug_for_advance() {
    MotionState::new(f64::NAN, 1.0, 0.0).advance(1.0);
}

/// In release, each precondition violation returns its documented fallback.
///
/// Note the three different fallbacks, each chosen for its failure mode rather
/// than for uniformity: the averaging relations return `0.0`, the forward
/// relations return the input they were advancing *from* (so a body coasts or
/// stays put rather than being teleported to the origin or stopped dead), and the
/// `Option` functions return `None`.
#[test]
#[cfg(not(debug_assertions))]
fn precondition_violations_return_documented_fallbacks_in_release() {
    // Averaging relations: no interval, no answer.
    assert_eq!(average_velocity(0.0, 1.0, 0.0), 0.0);
    assert_eq!(average_velocity(0.0, 1.0, -1.0), 0.0);
    assert_eq!(average_velocity(f64::NAN, 1.0, 1.0), 0.0);
    assert_eq!(average_acceleration(0.0, 1.0, 0.0), 0.0);
    assert_eq!(average_acceleration(0.0, f64::NAN, 1.0), 0.0);

    // Forward relations: the body coasts or stays put.
    assert_eq!(velocity_after(7.0, 1.0, -1.0), 7.0);
    assert_eq!(velocity_after(7.0, f64::NAN, 1.0), 7.0);
    assert_eq!(displacement(1.0, 1.0, -1.0), 0.0);
    assert_eq!(displacement(1.0, f64::INFINITY, 1.0), 0.0);
    assert_eq!(position_after(5.0, 1.0, 1.0, -1.0), 5.0);
    assert_eq!(position_after(5.0, f64::NAN, 1.0, 1.0), 5.0);

    // Option functions: no answer rather than a wrong one.
    assert_eq!(speed_from_displacement(f64::NAN, 1.0, 1.0), None);
    assert_eq!(speed_from_displacement(0.0, f64::INFINITY, 1.0), None);
    assert_eq!(time_to_reach_velocity(f64::NAN, 1.0, 1.0), None);
    assert_eq!(time_to_reach_velocity(0.0, 1.0, f64::NAN), None);

    // MotionState: the state is returned unchanged.
    let invalid = MotionState::new(f64::NAN, 1.0, 0.0);
    assert_eq!(invalid.advance(1.0).velocity_m_s, 1.0);

    let valid = MotionState::new(5.0, 1.0, 0.0);
    assert_eq!(valid.advance(-1.0), valid);
    assert_eq!(valid.advance(f64::NAN), valid);
}
