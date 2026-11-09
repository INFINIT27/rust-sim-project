//! Unit and property tests for the [`crate::physics_utils::newtons_laws`]
//! module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/physics_utils/newtons_laws.rs` via `#[path]`, not as a standalone
//! integration test, so these tests keep access to the module's private items
//! and to `super::*`.
//!
//! Properties first — the algebraic content of each law, which is what a
//! plausible-but-wrong implementation violates — then hand-written cases pinning
//! exact values and boundary behaviour.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/newtons_laws_tests.txt";

/// Relative-error comparison. Values here span many orders of magnitude, so an
/// absolute epsilon would be either vacuous or impossible depending on scale.
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

/// Masses from a microgram to a thousand tonnes, in kilograms.
fn valid_mass() -> impl Strategy<Value = f64> {
    1.0e-9..1.0e6
}

/// Signed velocities and accelerations, bounded so no product can overflow.
fn signed_quantity() -> impl Strategy<Value = f64> {
    -1.0e6..1.0e6_f64
}

/// Non-negative durations, in seconds.
fn valid_duration() -> impl Strategy<Value = f64> {
    0.0..1.0e6_f64
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1 (first law): inertial motion is additive over time.
    ///
    /// Advancing `t₁` then `t₂` must land in the same place as advancing
    /// `t₁ + t₂` in one call. This is the precise sense in which the first law
    /// says a force-free body does not need integrating: there is no truncation
    /// error to accumulate, so step size is irrelevant.
    #[test]
    fn inertial_motion_is_additive_over_time(
        start in signed_quantity(),
        velocity in signed_quantity(),
        first in 0.0..1.0e3_f64,
        second in 0.0..1.0e3_f64,
    ) {
        let stepwise = inertial_position(inertial_position(start, velocity, first), velocity, second);
        let in_one_go = inertial_position(start, velocity, first + second);

        assert_close(stepwise, in_one_go, 1.0e-12, "two steps must equal one combined step");
    }

    /// Property 2 (first law): zero elapsed time never moves a body.
    #[test]
    fn inertial_motion_over_zero_time_is_a_no_op(
        start in signed_quantity(),
        velocity in signed_quantity(),
    ) {
        prop_assert_eq!(inertial_position(start, velocity, 0.0), start);
    }

    /// Property 3 (first law): a body at rest stays at rest, for any duration.
    ///
    /// The other half of the law, and the one a sign error or a stray `+ dt`
    /// would break.
    #[test]
    fn a_body_at_rest_stays_at_rest(start in signed_quantity(), elapsed in valid_duration()) {
        prop_assert_eq!(inertial_position(start, 0.0, elapsed), start);
    }

    /// Property 4 (second law): force and acceleration are exact inverses.
    ///
    /// `a = F/m` must undo `F = m·a` for every valid mass. Round-tripping is the
    /// strongest available statement that the two functions encode the *same*
    /// law rather than two independently plausible ones.
    #[test]
    fn force_and_acceleration_round_trip(
        mass in valid_mass(),
        acceleration in signed_quantity(),
    ) {
        let force = net_force(mass, acceleration);
        let recovered = acceleration_from_net_force(force, mass);

        assert_close(recovered, acceleration, 1.0e-12, "a -> F -> a must round-trip");
    }

    /// Property 5 (second law): force is linear in mass at fixed acceleration.
    #[test]
    fn force_is_linear_in_mass(mass in 1.0e-9..1.0e5_f64, acceleration in signed_quantity()) {
        let base = net_force(mass, acceleration);
        let doubled = net_force(2.0 * mass, acceleration);

        assert_close(doubled, 2.0 * base, 1.0e-12, "doubling the mass must double the force");
    }

    /// Property 6 (second law): acceleration falls as mass rises.
    ///
    /// For the same force, a heavier body accelerates no more than a lighter
    /// one. Independent of the exact `1/m` shape, so it also pins the *sign* of
    /// the mass dependence.
    #[test]
    fn heavier_bodies_accelerate_no_more_for_the_same_force(
        force in 0.0..1.0e6_f64,
        light in valid_mass(),
        extra in 0.0..1.0e6_f64,
    ) {
        let heavy = light + extra;
        let light_acceleration = acceleration_from_net_force(force, light);
        let heavy_acceleration = acceleration_from_net_force(force, heavy);

        prop_assert!(
            heavy_acceleration <= light_acceleration,
            "{} kg accelerated at {:e}, {} kg at {:e} under the same {:e} N",
            heavy, heavy_acceleration, light, light_acceleration, force
        );
    }

    /// Property 7 (second law implies the first): no force means no
    /// acceleration.
    ///
    /// The bridge between the two laws, and the reason this module has no
    /// separate "velocity is unchanged" function.
    #[test]
    fn zero_net_force_produces_zero_acceleration(mass in valid_mass()) {
        prop_assert_eq!(acceleration_from_net_force(0.0, mass), 0.0);
    }

    /// Property 8 (third law): an action-reaction pair always sums to zero.
    ///
    /// Asserted bit-exactly. Negation is exact in IEEE-754, so there is no
    /// tolerance to grant and any failure is a real defect.
    #[test]
    fn action_and_reaction_sum_to_zero(force in signed_quantity()) {
        prop_assert_eq!(force + reaction_force(force), 0.0);
    }

    /// Property 9 (third law): reaction is an involution.
    ///
    /// The reaction to the reaction is the original action — the two bodies'
    /// roles are symmetric, so applying the law twice must return you to where
    /// you started.
    #[test]
    fn reaction_is_its_own_inverse(force in signed_quantity()) {
        prop_assert_eq!(reaction_force(reaction_force(force)), force);
    }

    /// Property 10 (third law): the reaction has equal magnitude and, for a
    /// nonzero action, the opposite sign.
    #[test]
    fn reaction_has_equal_magnitude_and_opposite_sign(force in signed_quantity()) {
        let reaction = reaction_force(force);

        prop_assert_eq!(reaction.abs(), force.abs());
        if force != 0.0 {
            prop_assert!(
                reaction.signum() != force.signum(),
                "{:e} and {:e} must have opposite signs", force, reaction
            );
        }
    }
}

// ---- First law: hand-written cases ----

/// Uniform motion over a whole number of seconds.
#[test]
fn inertial_position_advances_by_velocity_times_time() {
    assert_eq!(inertial_position(0.0, 3.0, 2.0), 6.0);
    assert_eq!(inertial_position(10.0, -4.0, 0.5), 8.0);
    assert_eq!(inertial_position(-2.5, 1.0, 4.0), 1.5);
}

/// A negative velocity moves the body the other way along the axis.
#[test]
fn negative_velocity_moves_backwards_along_the_axis() {
    assert_eq!(inertial_position(0.0, -7.0, 3.0), -21.0);
}

// ---- Second law: hand-written cases ----

/// The textbook case, in both directions.
#[test]
fn second_law_relates_two_kilograms_and_three_metres_per_second_squared() {
    assert_eq!(net_force(2.0, 3.0), 6.0);
    assert_eq!(acceleration_from_net_force(6.0, 2.0), 3.0);
}

/// A zero mass is *allowed* for `net_force` and yields no force.
///
/// The asymmetry with `acceleration_from_net_force` is deliberate and worth
/// pinning: `m·a` is defined at `m = 0`, `F/m` is not.
#[test]
fn zero_mass_produces_zero_force() {
    assert_eq!(net_force(0.0, 9.81), 0.0);
}

/// Force follows the sign of the acceleration.
#[test]
fn force_follows_the_sign_of_the_acceleration() {
    assert_eq!(net_force(3.0, -2.0), -6.0);
    assert_eq!(acceleration_from_net_force(-6.0, 3.0), -2.0);
}

/// Weight at the Earth's surface: 70 kg under standard gravity is ~687 N.
///
/// An anchor against an independently known figure, so the second law is not
/// only tested against itself.
#[test]
fn weight_of_a_seventy_kilogram_mass_under_standard_gravity() {
    const STANDARD_GRAVITY_M_S2: f64 = 9.806_65;

    assert_close(
        net_force(70.0, STANDARD_GRAVITY_M_S2),
        686.4655,
        1.0e-6,
        "weight of 70 kg at g",
    );
}

// ---- Third law: hand-written cases ----

/// Equal magnitude, opposite direction.
#[test]
fn reaction_force_negates_the_action() {
    assert_eq!(reaction_force(5.0), -5.0);
    assert_eq!(reaction_force(-2.5), 2.5);
}

/// A zero action has a zero reaction.
///
/// Note the implementation returns `-0.0` here, which `==` treats as equal to
/// `0.0`. Pinned so nobody "fixes" it into a special case.
#[test]
fn zero_action_has_zero_reaction() {
    assert_eq!(reaction_force(0.0), 0.0);
    assert!(reaction_force(0.0).is_sign_negative());
}

// ---- Precondition violations ----
//
// The contract is split by build profile — descriptive `debug_assert!` panics in
// debug, a documented benign value in release — so these two groups are mutually
// exclusive by `cfg`, mirroring `tst/core/scene_tests.rs`. Both `cargo test` and
// `cargo test --release` must pass.

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "inertial_position: elapsed_s")]
fn negative_elapsed_time_panics_in_debug() {
    inertial_position(0.0, 1.0, -1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "inertial_position: velocity_m_s")]
fn non_finite_velocity_panics_in_debug() {
    inertial_position(0.0, f64::NAN, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "net_force: mass_kg")]
fn negative_mass_panics_in_debug_for_net_force() {
    net_force(-1.0, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "acceleration_from_net_force: mass_kg")]
fn zero_mass_panics_in_debug_for_acceleration() {
    acceleration_from_net_force(1.0, 0.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "acceleration_from_net_force: net_force_n")]
fn non_finite_force_panics_in_debug() {
    acceleration_from_net_force(f64::INFINITY, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "reaction_force: action_force_n")]
fn non_finite_action_panics_in_debug() {
    reaction_force(f64::NAN);
}

/// In release, each precondition violation returns its documented fallback.
///
/// Note that `inertial_position` falls back to the *initial position* rather
/// than `0.0`: teleporting a body to the origin would be a far worse failure
/// mode than leaving it where it was.
#[test]
#[cfg(not(debug_assertions))]
fn precondition_violations_return_documented_fallbacks_in_release() {
    // First law: the body stays where it was.
    assert_eq!(inertial_position(4.0, 1.0, -1.0), 4.0);
    assert_eq!(inertial_position(4.0, f64::NAN, 1.0), 4.0);
    assert_eq!(inertial_position(4.0, 1.0, f64::INFINITY), 4.0);

    // Second law: no force, no acceleration.
    assert_eq!(net_force(-1.0, 1.0), 0.0);
    assert_eq!(net_force(f64::NAN, 1.0), 0.0);
    assert_eq!(net_force(1.0, f64::INFINITY), 0.0);
    assert_eq!(acceleration_from_net_force(1.0, 0.0), 0.0);
    assert_eq!(acceleration_from_net_force(1.0, -2.0), 0.0);
    assert_eq!(acceleration_from_net_force(f64::NAN, 1.0), 0.0);

    // Third law: no action, no reaction.
    assert_eq!(reaction_force(f64::NAN), 0.0);
    assert_eq!(reaction_force(f64::INFINITY), 0.0);
}
