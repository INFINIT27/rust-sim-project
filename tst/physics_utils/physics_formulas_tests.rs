//! Unit and property tests for the
//! [`crate::physics_utils::physics_formulas`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/physics_utils/physics_formulas.rs` via `#[path]`, not as a standalone
//! integration test, so these tests keep access to the module's private items
//! and to `super::*`.
//!
//! Structure, mirroring `tst/core/scene_tests.rs`: proptest strategies and
//! properties first (the algebraic laws the formula must obey for *all*
//! inputs), then hand-written cases that pin exact values, known real-world
//! quantities, and boundary behaviour.

use super::*;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into
/// `src/physics_utils/physics_formulas.rs` by `#[path]`, `file!()` is
/// `src/physics_utils/../../tst/physics_utils/physics_formulas_tests.rs`, and
/// proptest does not normalise the `..` components — the default resolved to
/// `src/physics_utils/tst/physics_utils/physics_formulas_tests.txt`, a bogus
/// directory shadowing the test tree from inside `src/`.
///
/// The path is relative to the package root, which is Cargo's working directory
/// for tests. Commit the seeds it produces: they are the counterexamples this
/// suite has already paid for.
const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/physics_formulas_tests.txt";

/// Relative-error comparison, for values whose magnitudes span 60+ orders of
/// magnitude. An absolute tolerance is meaningless here: `1e-9` is enormous
/// next to a `1e-11` force and invisible next to a `1e22` one.
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

// Bounded strategies. The ranges are wide enough to cover dust grains through
// stars and millimetres through light-hours, and narrow enough that no
// intermediate or final value can overflow `f64` — so any non-finite result a
// property observes is a real bug, not a strategy artefact.
//
// Worst case: G * (1e30 / 1e-3) * (1e30 / 1e-3) = 6.7e55, well inside f64's
// ~1.8e308 ceiling.

/// Masses from a microgram to a few solar masses, in kilograms.
fn valid_mass() -> impl Strategy<Value = f64> {
    1.0e-9..1.0e30
}

/// Separations from a millimetre to ~1000 astronomical units, in metres.
fn valid_distance() -> impl Strategy<Value = f64> {
    1.0e-3..1.0e15
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1: the result is always a usable force.
    ///
    /// For any valid mass pair and separation the force must be finite and
    /// non-negative — gravity attracts, never repels, and never produces
    /// `NaN`/`inf` for in-range input.
    #[test]
    fn force_is_finite_and_non_negative(
        mass_a in valid_mass(),
        mass_b in valid_mass(),
        distance in valid_distance(),
    ) {
        let force = gravitational_constant_calculator(mass_a, mass_b, distance);

        prop_assert!(force.is_finite(), "force must be finite, got {:e}", force);
        prop_assert!(force >= 0.0, "force must be non-negative, got {:e}", force);
    }

    /// Property 2: the force is symmetric in the two masses.
    ///
    /// Newton's third law: each body pulls on the other with equal magnitude,
    /// so swapping the arguments cannot change the answer.
    ///
    /// Asserted bit-exactly, which is a real constraint on the
    /// implementation's grouping rather than a tautology: this property
    /// originally failed at 1 ULP (`mass_a = 1e-9`, `mass_b = 8.77e29`,
    /// `distance = 1e-3`) because `G` was folded into one mass before the
    /// other was applied. Grouping the two `m / r` ratios together first makes
    /// the result commutative by IEEE-754. Do not weaken this to a tolerance —
    /// the exact form is the guarantee.
    #[test]
    fn force_is_symmetric_in_the_two_masses(
        mass_a in valid_mass(),
        mass_b in valid_mass(),
        distance in valid_distance(),
    ) {
        let forward = gravitational_constant_calculator(mass_a, mass_b, distance);
        let swapped = gravitational_constant_calculator(mass_b, mass_a, distance);

        prop_assert_eq!(
            forward, swapped,
            "force must not depend on argument order: {:e} vs {:e}", forward, swapped
        );
    }

    /// Property 3: the force is linear in each mass.
    ///
    /// Doubling one mass doubles the attraction. Checked with a relative
    /// tolerance because scaling a mass changes the rounding of the
    /// intermediate products.
    #[test]
    fn force_is_linear_in_each_mass(
        mass_a in 1.0e-9..1.0e29_f64,
        mass_b in valid_mass(),
        distance in valid_distance(),
    ) {
        let base = gravitational_constant_calculator(mass_a, mass_b, distance);
        let doubled = gravitational_constant_calculator(2.0 * mass_a, mass_b, distance);

        assert_close(doubled, 2.0 * base, 1.0e-12, "doubling mass_a must double the force");
    }

    /// Property 4: the force obeys the inverse-square law.
    ///
    /// Doubling the separation quarters the attraction — the defining shape of
    /// the law, and the one thing a plausible-but-wrong implementation (say,
    /// dividing by `r` instead of `r²`) would get wrong while still passing
    /// the finiteness, symmetry, and linearity properties above.
    #[test]
    fn force_obeys_the_inverse_square_law(
        mass_a in valid_mass(),
        mass_b in valid_mass(),
        distance in 1.0e-3..1.0e14_f64,
    ) {
        let base = gravitational_constant_calculator(mass_a, mass_b, distance);
        let at_double_distance =
            gravitational_constant_calculator(mass_a, mass_b, 2.0 * distance);

        assert_close(
            at_double_distance,
            base / 4.0,
            1.0e-12,
            "doubling the separation must quarter the force",
        );
    }

    /// Property 5: the force decreases monotonically with separation.
    ///
    /// Weaker than the inverse-square property but independent of the exact
    /// exponent, so it also pins the *sign* of the distance dependence: moving
    /// two bodies apart can never increase their mutual attraction.
    #[test]
    fn force_never_increases_with_separation(
        mass_a in valid_mass(),
        mass_b in valid_mass(),
        near in valid_distance(),
        extra in 0.0..1.0e15_f64,
    ) {
        let far = near + extra;
        let force_near = gravitational_constant_calculator(mass_a, mass_b, near);
        let force_far = gravitational_constant_calculator(mass_a, mass_b, far);

        prop_assert!(
            force_far <= force_near,
            "force at {:e} m ({:e} N) must not exceed force at {:e} m ({:e} N)",
            far, force_far, near, force_near
        );
    }
}

/// Unit masses one metre apart attract with exactly `G` newtons.
///
/// This is the definition of the gravitational constant, so it must hold
/// bit-exactly, not approximately: with all three inputs at `1.0` the formula
/// reduces to `(G * 1.0 / 1.0) * (1.0 / 1.0)`, and any deviation means an
/// extra factor has crept in.
#[test]
fn unit_masses_one_metre_apart_attract_with_exactly_g() {
    let force = gravitational_constant_calculator(1.0, 1.0, 1.0);

    assert_eq!(force, GRAVITATIONAL_CONSTANT);
}

/// A worked example with no coincidental ones: 10 kg and 5 kg, 2 m apart.
///
/// `F = G * 10 * 5 / 4 = 12.5 * G`. Pins the arithmetic against a value
/// computed independently of the implementation's evaluation order.
#[test]
fn force_matches_hand_computed_value_for_ordinary_masses() {
    let force = gravitational_constant_calculator(10.0, 5.0, 2.0);

    assert_close(
        force,
        12.5 * GRAVITATIONAL_CONSTANT,
        1.0e-15,
        "G * 10 kg * 5 kg / (2 m)²",
    );
}

/// The Earth–Sun attraction is ~3.54e22 N.
///
/// A real-world anchor: masses `5.972e24` and `1.989e30` kg at a mean
/// separation of `1.496e11` m. The 0.5% tolerance covers the rounding in these
/// three published figures, which dominates `G`'s own uncertainty. This case
/// also proves the `f64` choice is load-bearing — the mass product alone is
/// `~1.19e55`, which overflows `f32`.
#[test]
fn earth_sun_attraction_matches_the_known_value() {
    const EARTH_MASS_KG: f64 = 5.972e24;
    const SUN_MASS_KG: f64 = 1.989e30;
    const MEAN_ORBITAL_RADIUS_M: f64 = 1.496e11;

    let force =
        gravitational_constant_calculator(EARTH_MASS_KG, SUN_MASS_KG, MEAN_ORBITAL_RADIUS_M);

    assert_close(force, 3.54e22, 5.0e-3, "Earth-Sun gravitational attraction");
}

/// Weight at the Earth's surface recovers `g ≈ 9.8 m/s²`.
///
/// A second independent anchor, and a different dimension of the same formula:
/// dividing the force on a 1 kg mass at the Earth's radius by that mass must
/// give standard surface gravity. Catches an error in the constant's exponent
/// that a mass-ratio test would not.
#[test]
fn surface_gravity_recovers_nine_point_eight_metres_per_second_squared() {
    const EARTH_MASS_KG: f64 = 5.972e24;
    const EARTH_MEAN_RADIUS_M: f64 = 6.371e6;
    const TEST_MASS_KG: f64 = 1.0;

    let force = gravitational_constant_calculator(EARTH_MASS_KG, TEST_MASS_KG, EARTH_MEAN_RADIUS_M);
    let acceleration = force / TEST_MASS_KG;

    assert_close(acceleration, 9.82, 5.0e-3, "Earth surface gravity");
}

/// A zero mass produces zero force, from either argument position.
///
/// `0.0` is a valid input (the precondition is *non-negative*, not positive),
/// so this must return cleanly rather than tripping an assertion, and the
/// result must be exactly zero rather than a denormal.
#[test]
fn zero_mass_produces_zero_force() {
    assert_eq!(gravitational_constant_calculator(0.0, 5.0, 1.0), 0.0);
    assert_eq!(gravitational_constant_calculator(5.0, 0.0, 1.0), 0.0);
    assert_eq!(gravitational_constant_calculator(0.0, 0.0, 1.0), 0.0);
}

/// Extreme but in-range inputs stay finite.
///
/// The stress case for the scaled evaluation order: the naive
/// `G * m_a * m_b / r²` would form a `1e60` intermediate here. `f64` survives
/// that too, but the assertion documents the ceiling the implementation's
/// comment claims and would fail immediately if anyone narrowed these values
/// to `f32`.
#[test]
fn very_large_masses_at_very_small_separations_stay_finite() {
    let force = gravitational_constant_calculator(1.0e30, 1.0e30, 1.0e-3);

    assert!(
        force.is_finite(),
        "force must stay finite for extreme in-range input, got {:e}",
        force
    );
    assert!(force > 0.0, "force must be positive, got {:e}", force);
    assert_close(force, 6.674_30e55, 1.0e-12, "G * 1e30 * 1e30 / (1e-3)²");
}

/// A tiny separation yields a huge but finite force, with no singularity.
///
/// Confirms the function stays total as `r` shrinks: the result grows without
/// the implementation ever dividing by zero, because `distance_m == 0.0` is
/// rejected by the precondition instead.
#[test]
fn shrinking_separation_grows_the_force_without_a_singularity() {
    let near = gravitational_constant_calculator(1.0, 1.0, 1.0e-6);
    let nearer = gravitational_constant_calculator(1.0, 1.0, 1.0e-9);

    assert!(near.is_finite() && nearer.is_finite());
    assert!(
        nearer > near,
        "closer masses must attract more strongly: {:e} vs {:e}",
        nearer,
        near
    );
    assert_close(near, GRAVITATIONAL_CONSTANT * 1.0e12, 1.0e-12, "r = 1e-6 m");
}

// Precondition violations. The contract is deliberately split by build
// profile — descriptive `debug_assert!` panics in debug, a benign `0.0` in
// release — so the two sets of tests below are mutually exclusive by `cfg`.
// This mirrors `tst/core/scene_tests.rs`, and it is what lets
// `cargo test --release` pass alongside `cargo test`.

/// A `NaN` mass trips the debug assertion.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_constant_calculator: mass_a_kg")]
fn nan_mass_panics_in_debug() {
    gravitational_constant_calculator(f64::NAN, 1.0, 1.0);
}

/// An infinite mass trips the debug assertion.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_constant_calculator: mass_b_kg")]
fn infinite_mass_panics_in_debug() {
    gravitational_constant_calculator(1.0, f64::INFINITY, 1.0);
}

/// A negative mass trips the debug assertion.
///
/// Negative mass is not physical, and admitting it would silently turn the
/// force repulsive rather than producing an obviously wrong magnitude.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_constant_calculator: mass_a_kg")]
fn negative_mass_panics_in_debug() {
    gravitational_constant_calculator(-1.0, 1.0, 1.0);
}

/// A zero separation trips the debug assertion.
///
/// Coincident point masses are a modelling error, not a physical infinity, so
/// the contract rejects them instead of returning `inf`.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_constant_calculator: distance_m")]
fn zero_distance_panics_in_debug() {
    gravitational_constant_calculator(1.0, 1.0, 0.0);
}

/// A negative separation trips the debug assertion.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_constant_calculator: distance_m")]
fn negative_distance_panics_in_debug() {
    gravitational_constant_calculator(1.0, 1.0, -5.0);
}

/// A `NaN` separation trips the debug assertion.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_constant_calculator: distance_m")]
fn nan_distance_panics_in_debug() {
    gravitational_constant_calculator(1.0, 1.0, f64::NAN);
}

/// In release builds every precondition violation returns `0.0` instead.
///
/// The release contract is that a bad input can never inject `NaN` or `inf`
/// into a caller's simulation state, so each rejected case is checked
/// explicitly rather than merely "not panicking".
#[test]
#[cfg(not(debug_assertions))]
fn precondition_violations_return_zero_in_release() {
    for (mass_a, mass_b, distance, case) in [
        (f64::NAN, 1.0, 1.0, "NaN mass_a"),
        (1.0, f64::NAN, 1.0, "NaN mass_b"),
        (f64::INFINITY, 1.0, 1.0, "infinite mass_a"),
        (1.0, f64::INFINITY, 1.0, "infinite mass_b"),
        (-1.0, 1.0, 1.0, "negative mass_a"),
        (1.0, -1.0, 1.0, "negative mass_b"),
        (1.0, 1.0, 0.0, "zero distance"),
        (1.0, 1.0, -5.0, "negative distance"),
        (1.0, 1.0, f64::NAN, "NaN distance"),
        (1.0, 1.0, f64::INFINITY, "infinite distance"),
    ] {
        let force = gravitational_constant_calculator(mass_a, mass_b, distance);

        assert_eq!(
            force, 0.0,
            "{} must return exactly 0.0 in release, got {:e}",
            case, force
        );
    }
}
