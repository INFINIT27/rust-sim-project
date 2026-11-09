//! Unit and property tests for the
//! [`crate::physics_utils::conservation_laws`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/physics_utils/conservation_laws.rs` via `#[path]`, not as a standalone
//! integration test, so these tests keep access to the module's private items —
//! including `compensated_sum` — and to `super::*`.
//!
//! The centre of gravity here is the pair of collision solvers, because they are
//! the only place both conservation laws are asserted against something the
//! crate itself produces rather than against a hand-computed transition. The
//! elastic solver must preserve momentum *and* kinetic energy; the inelastic one
//! must preserve momentum and *strictly reduce* kinetic energy. Getting both
//! directions is what makes the energy law falsifiable.

use super::*;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/conservation_laws_tests.txt";

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

/// Masses from a gram to a thousand tonnes, in kilograms. Strictly positive, so
/// the same strategy serves the collision solvers.
fn valid_mass() -> impl Strategy<Value = f64> {
    1.0e-3..1.0e6
}

/// Signed velocities, bounded so `m·v²` cannot overflow.
fn valid_velocity() -> impl Strategy<Value = f64> {
    -1.0e6..1.0e6_f64
}

/// A system of one to eight bodies.
fn valid_system() -> impl Strategy<Value = Vec<(f64, f64)>> {
    prop::collection::vec((valid_mass(), valid_velocity()), 1..8)
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1: an elastic collision conserves momentum.
    #[test]
    fn elastic_collision_conserves_momentum(
        mass_a in valid_mass(),
        velocity_a in valid_velocity(),
        mass_b in valid_mass(),
        velocity_b in valid_velocity(),
    ) {
        let before = [(mass_a, velocity_a), (mass_b, velocity_b)];
        let (out_a, out_b) = elastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b);
        let after = [(mass_a, out_a), (mass_b, out_b)];

        prop_assert!(
            momentum_is_conserved(&before, &after, 1.0e-12),
            "momentum {:e} -> {:e} for m=({:e},{:e}) v=({:e},{:e})",
            total_momentum(&before), total_momentum(&after),
            mass_a, mass_b, velocity_a, velocity_b
        );
    }

    /// Property 2: an elastic collision conserves kinetic energy.
    ///
    /// The defining property of elasticity, and the one the textbook
    /// `(m_a − m_b)` form loses to cancellation for nearly equal masses — which
    /// is why the implementation uses the relative-velocity form instead.
    #[test]
    fn elastic_collision_conserves_kinetic_energy(
        mass_a in valid_mass(),
        velocity_a in valid_velocity(),
        mass_b in valid_mass(),
        velocity_b in valid_velocity(),
    ) {
        let before = [(mass_a, velocity_a), (mass_b, velocity_b)];
        let (out_a, out_b) = elastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b);
        let after = [(mass_a, out_a), (mass_b, out_b)];

        prop_assert!(
            kinetic_energy_is_conserved(&before, &after, 1.0e-9),
            "kinetic energy {:e} -> {:e} for m=({:e},{:e}) v=({:e},{:e})",
            total_kinetic_energy(&before), total_kinetic_energy(&after),
            mass_a, mass_b, velocity_a, velocity_b
        );
    }

    /// Property 3: an elastic collision reverses the relative velocity exactly.
    ///
    /// An equivalent statement of elasticity that is independent of the energy
    /// bookkeeping: the pair separates as fast as it approached.
    #[test]
    fn elastic_collision_reverses_relative_velocity(
        mass_a in valid_mass(),
        velocity_a in valid_velocity(),
        mass_b in valid_mass(),
        velocity_b in valid_velocity(),
    ) {
        let approach = velocity_a - velocity_b;
        let (out_a, out_b) = elastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b);
        let separation = out_b - out_a;

        assert_close(separation, approach, 1.0e-9, "separation speed must equal approach speed");
    }

    /// Property 4: a perfectly inelastic collision conserves momentum.
    #[test]
    fn inelastic_collision_conserves_momentum(
        mass_a in valid_mass(),
        velocity_a in valid_velocity(),
        mass_b in valid_mass(),
        velocity_b in valid_velocity(),
    ) {
        let before = [(mass_a, velocity_a), (mass_b, velocity_b)];
        let merged = perfectly_inelastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b);
        let after = [(mass_a + mass_b, merged)];

        prop_assert!(
            momentum_is_conserved(&before, &after, 1.0e-12),
            "momentum {:e} -> {:e}", total_momentum(&before), total_momentum(&after)
        );
    }

    /// Property 5: a perfectly inelastic collision never *gains* kinetic energy.
    ///
    /// The one-directional half of the energy law. Stated as `<=` rather than
    /// `<` because bodies already moving together lose nothing — see the
    /// hand-written case below for the strict-loss claim.
    #[test]
    fn inelastic_collision_never_gains_kinetic_energy(
        mass_a in valid_mass(),
        velocity_a in valid_velocity(),
        mass_b in valid_mass(),
        velocity_b in valid_velocity(),
    ) {
        let before = [(mass_a, velocity_a), (mass_b, velocity_b)];
        let merged = perfectly_inelastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b);
        let after = [(mass_a + mass_b, merged)];

        let energy_before = total_kinetic_energy(&before);
        let energy_after = total_kinetic_energy(&after);

        prop_assert!(
            energy_after <= energy_before * (1.0 + 1.0e-9),
            "inelastic collision gained energy: {:e} -> {:e}", energy_before, energy_after
        );
    }

    /// Property 6: the merged velocity always lies between the two inputs.
    ///
    /// It is a mass-weighted average, so it cannot overshoot either body — a
    /// sign error in the weighting would.
    #[test]
    fn merged_velocity_lies_between_the_inputs(
        mass_a in valid_mass(),
        velocity_a in valid_velocity(),
        mass_b in valid_mass(),
        velocity_b in valid_velocity(),
    ) {
        let merged = perfectly_inelastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b);
        let low = velocity_a.min(velocity_b);
        let high = velocity_a.max(velocity_b);
        let slack = (high - low).abs() * 1.0e-9;

        prop_assert!(
            merged >= low - slack && merged <= high + slack,
            "{:e} is outside [{:e}, {:e}]", merged, low, high
        );
    }

    /// Property 7: momentum is additive over the system.
    ///
    /// The total equals the sum of the per-body values, which is what licenses
    /// `total_momentum` as a stand-in for the physical quantity.
    ///
    /// # The tolerance is scaled by the sum's condition number, deliberately
    ///
    /// The reference here is a **naive** `iter().sum()`, while `total_momentum` uses
    /// Neumaier compensated summation — so when the terms nearly cancel, the *reference*
    /// is the inaccurate side. A fixed `1e-12` relative tolerance asserted that the
    /// accurate sum matches the inaccurate one, and proptest eventually found a system
    /// where it does not: four bodies with momenta of order `1e11` summing to `1.7e7`, a
    /// cancellation of four orders of magnitude, giving a relative error of `1.8e-12`.
    ///
    /// `Σ|mᵢ·vᵢ| / |Σ mᵢ·vᵢ|` is exactly the factor by which cancellation amplifies the
    /// reference's rounding error, so scaling by it keeps the property meaningful — the
    /// two sums must still agree to within the naive sum's own achievable accuracy — while
    /// no longer blaming the compensated result for being better.
    #[test]
    fn total_momentum_is_the_sum_of_body_momenta(system in valid_system()) {
        let summed: f64 = system.iter().map(|&(m, v)| momentum(m, v)).sum();

        let absolute: f64 = system.iter().map(|&(m, v)| momentum(m, v).abs()).sum();
        let condition = absolute / summed.abs().max(f64::MIN_POSITIVE);

        assert_close(
            total_momentum(&system),
            summed,
            1.0e-12 * condition.max(1.0),
            "total momentum must equal the sum of body momenta",
        );
    }

    /// Property 8: total kinetic energy is non-negative and additive.
    ///
    /// Unlike momentum, kinetic energy cannot cancel, so the total is a floor of
    /// zero rather than a signed sum.
    #[test]
    fn total_kinetic_energy_is_non_negative_and_additive(system in valid_system()) {
        let total = total_kinetic_energy(&system);
        let summed: f64 = system.iter().map(|&(m, v)| kinetic_energy(m, v)).sum();

        prop_assert!(total >= 0.0, "kinetic energy must be non-negative, got {:e}", total);
        assert_close(total, summed, 1.0e-12, "total must equal the sum of body energies");
    }

    /// Property 9: an unchanged system conserves both quantities exactly.
    ///
    /// The reflexive case. Guards against a check that is accidentally always
    /// `false` — the failure mode the release-fallback behaviour makes plausible.
    #[test]
    fn an_unchanged_system_conserves_everything(system in valid_system()) {
        prop_assert!(momentum_is_conserved(&system, &system, 0.0));
        prop_assert!(kinetic_energy_is_conserved(&system, &system, 0.0));
    }

    /// Property 10: adding momentum from nowhere is detected.
    ///
    /// Guards the opposite failure: a check that is accidentally always `true`.
    /// The extra body carries a momentum comparable to the system's own scale, so
    /// no tolerance could excuse it.
    #[test]
    fn injected_momentum_is_detected(system in valid_system(), extra in 1.0..1.0e3_f64) {
        let scale: f64 = system.iter().map(|&(m, v)| (m * v).abs()).sum::<f64>().max(1.0);
        let mut tampered = system.clone();
        tampered.push((scale * extra, 1.0));

        prop_assert!(
            !momentum_is_conserved(&system, &tampered, 1.0e-12),
            "injecting {:e} kg·m/s went undetected", scale * extra
        );
    }

    /// Property 11: gravitational potential energy is symmetric and negative.
    ///
    /// Symmetry is bit-exact by construction: the implementation forms the mass
    /// product before dividing, and IEEE-754 multiplication is commutative.
    #[test]
    fn gravitational_potential_energy_is_symmetric_and_non_positive(
        mass_a in 1.0e-3..1.0e20_f64,
        mass_b in 1.0e-3..1.0e20_f64,
        distance in 1.0e-3..1.0e15_f64,
    ) {
        let forward = gravitational_potential_energy(mass_a, mass_b, distance);
        let swapped = gravitational_potential_energy(mass_b, mass_a, distance);

        prop_assert_eq!(forward, swapped);
        prop_assert!(forward <= 0.0, "potential energy must be non-positive, got {:e}", forward);
        prop_assert!(forward.is_finite(), "potential energy must be finite, got {:e}", forward);
    }

    /// Property 12: potential energy rises toward zero with separation.
    ///
    /// The sign convention's observable consequence: pulling a bound pair apart
    /// takes work, so `U` increases (becomes less negative) with `r`.
    #[test]
    fn potential_energy_rises_toward_zero_with_separation(
        mass_a in 1.0e-3..1.0e20_f64,
        mass_b in 1.0e-3..1.0e20_f64,
        near in 1.0e-3..1.0e14_f64,
        extra in 0.0..1.0e14_f64,
    ) {
        let far = near + extra;
        let closer = gravitational_potential_energy(mass_a, mass_b, near);
        let further = gravitational_potential_energy(mass_a, mass_b, far);

        prop_assert!(
            further >= closer,
            "U({:e} m) = {:e} must be >= U({:e} m) = {:e}", far, further, near, closer
        );
    }

    /// Property 13: compensated summation agrees with the naive sum.
    ///
    /// Exercises the private helper directly. The compensated result must never
    /// be *worse* than a plain sum for well-conditioned input, so agreement here
    /// is a sanity check that the compensation term is not being added with the
    /// wrong sign — a mistake that would double the error rather than cancel it.
    #[test]
    fn compensated_sum_agrees_with_the_naive_sum(terms in prop::collection::vec(-1.0e6..1.0e6_f64, 1..32)) {
        let naive: f64 = terms.iter().sum();
        let compensated = compensated_sum(terms.iter().copied());
        let scale = terms.iter().map(|t| t.abs()).sum::<f64>().max(f64::MIN_POSITIVE);

        prop_assert!(
            (compensated - naive).abs() <= 1.0e-12 * scale,
            "compensated {:e} vs naive {:e} over {} terms", compensated, naive, terms.len()
        );
    }
}

// ---- Momentum: hand-written cases ----

/// `p = m·v`, including the sign.
#[test]
fn momentum_is_mass_times_velocity() {
    assert_eq!(momentum(2.0, 3.0), 6.0);
    assert_eq!(momentum(2.0, -3.0), -6.0);
    assert_eq!(momentum(0.0, 5.0), 0.0);
}

/// A system whose bodies cancel has exactly zero total momentum.
///
/// The case that motivates scaling `momentum_is_conserved` by *absolute*
/// momentum rather than by the total: this system's total is zero while a great
/// deal is moving.
#[test]
fn opposed_bodies_cancel_to_zero_total_momentum() {
    assert_eq!(total_momentum(&[(2.0, 5.0), (5.0, -2.0)]), 0.0);
}

/// An empty system has zero momentum and zero energy.
#[test]
fn an_empty_system_has_zero_momentum_and_energy() {
    assert_eq!(total_momentum(&[]), 0.0);
    assert_eq!(total_kinetic_energy(&[]), 0.0);
}

// ---- Energy: hand-written cases ----

/// `T = ½·m·v²`, and direction does not matter.
#[test]
fn kinetic_energy_is_half_mass_times_velocity_squared() {
    assert_eq!(kinetic_energy(2.0, 3.0), 9.0);
    assert_eq!(kinetic_energy(2.0, -3.0), 9.0);
    assert_eq!(kinetic_energy(4.0, 0.0), 0.0);
}

/// Kinetic energy is quadratic in velocity: doubling speed quadruples energy.
#[test]
fn doubling_speed_quadruples_kinetic_energy() {
    assert_eq!(kinetic_energy(1.0, 4.0), 4.0 * kinetic_energy(1.0, 2.0));
}

/// Two 1 kg masses 1 m apart have `U = −G`.
///
/// Pinned bit-exactly: with all three inputs at `1.0` the formula reduces to
/// `-(G · 1 / 1)`, so any deviation means an extra factor has crept in.
#[test]
fn unit_masses_one_metre_apart_have_potential_energy_of_minus_g() {
    assert_eq!(
        gravitational_potential_energy(1.0, 1.0, 1.0),
        -GRAVITATIONAL_CONSTANT
    );
}

/// The Earth–Sun pair's potential energy is about `−5.30e33 J`.
///
/// A real-world anchor: `U = −G·m☉·m🜨/r` with the same figures the force test
/// uses. Also confirms the `f64` requirement — the mass product alone is `1.19e55`.
#[test]
fn earth_sun_potential_energy_matches_the_known_value() {
    const EARTH_MASS_KG: f64 = 5.972e24;
    const SUN_MASS_KG: f64 = 1.989e30;
    const MEAN_ORBITAL_RADIUS_M: f64 = 1.496e11;

    let potential =
        gravitational_potential_energy(EARTH_MASS_KG, SUN_MASS_KG, MEAN_ORBITAL_RADIUS_M);

    assert_close(potential, -5.30e33, 5.0e-3, "Earth-Sun potential energy");
}

/// Total mechanical energy is the sum, and its sign is the binding condition.
#[test]
fn total_mechanical_energy_sign_distinguishes_bound_from_unbound() {
    assert_eq!(total_mechanical_energy(3.0, -5.0), -2.0);
    assert!(total_mechanical_energy(3.0, -5.0) < 0.0, "bound");
    assert!(total_mechanical_energy(7.0, -5.0) > 0.0, "unbound");
    assert_eq!(total_mechanical_energy(5.0, -5.0), 0.0, "exactly escaping");
}

// ---- Conservation checks: hand-written cases ----

/// A merge that conserves momentum passes; one that does not, fails.
#[test]
fn momentum_check_accepts_a_valid_merge_and_rejects_an_invalid_one() {
    let before = [(2.0, 3.0), (1.0, 0.0)];

    assert!(momentum_is_conserved(&before, &[(3.0, 2.0)], 1.0e-12));
    assert!(!momentum_is_conserved(&before, &[(3.0, 3.0)], 1.0e-12));
}

/// Momentum appearing in a system that was at rest is detected.
///
/// Note precisely how the scale rule behaves here, because it is easy to get
/// wrong: the scale is the larger absolute momentum of the *two* states, so it is
/// zero only when both are at rest — and then both totals are `0` and
/// conservation holds trivially. As soon as the `after` state moves, the scale is
/// nonzero and the ordinary relative comparison applies. So detection depends on
/// the tolerance being sane, not on a special case.
#[test]
fn momentum_appearing_from_rest_is_detected() {
    let at_rest = [(1.0, 0.0), (2.0, 0.0)];

    // Genuinely unchanged: conserved even at zero tolerance.
    assert!(momentum_is_conserved(&at_rest, &at_rest, 0.0));

    // Momentum invented from nothing, at any realistic tolerance.
    assert!(!momentum_is_conserved(&at_rest, &[(1.0, 1.0)], 1.0e-12));
    assert!(!momentum_is_conserved(
        &at_rest,
        &[(1.0, 1.0e-300)],
        1.0e-12
    ));

    // A 100% tolerance really does accept anything, by construction. Pinned so
    // the permissiveness is a documented choice rather than a surprise.
    assert!(momentum_is_conserved(&at_rest, &[(1.0, 1.0e-300)], 1.0));
}

/// The energy check is relative, so absolute drift scales with magnitude.
#[test]
fn energy_check_is_relative_to_magnitude() {
    assert!(energy_is_conserved(11.0, 11.0, 1.0e-12));
    assert!(energy_is_conserved(11.0, 11.0 + 1.0e-13, 1.0e-12));
    assert!(!energy_is_conserved(11.0, 11.0 + 1.0e-9, 1.0e-12));

    // The same absolute drift is negligible at a larger magnitude.
    assert!(energy_is_conserved(1.0e12, 1.0e12 + 1.0e-9, 1.0e-12));
}

/// Two zero totals are conserved; energy appearing from nothing is not.
///
/// Same subtlety as the momentum case: `0.0 -> 1e-300` is scaled by `1e-300`, so
/// it is a 100% relative change and only a tolerance of `1.0` would excuse it.
#[test]
fn energy_check_handles_zero_totals() {
    assert!(energy_is_conserved(0.0, 0.0, 0.0));
    assert!(!energy_is_conserved(0.0, 1.0e-300, 1.0e-12));
    assert!(!energy_is_conserved(0.0, 1.0, 1.0e-12));
}

// ---- Collisions: hand-written cases ----

/// Equal masses exchange velocities exactly.
///
/// The cleanest signature of a correct elastic solver, and exact in floats for
/// these inputs because the mass ratio is ½.
#[test]
fn equal_masses_exchange_velocities_in_an_elastic_collision() {
    assert_eq!(elastic_collision_1d(1.0, 5.0, 1.0, -2.0), (-2.0, 5.0));
    assert_eq!(elastic_collision_1d(2.0, 3.0, 2.0, 0.0), (0.0, 3.0));
}

/// A light body bouncing off a very heavy one reverses almost exactly.
///
/// The wall limit. As the mass ratio diverges the heavy body is barely deflected
/// and the light one's velocity is negated.
#[test]
fn a_light_body_bounces_off_a_heavy_one() {
    let (light_out, heavy_out) = elastic_collision_1d(1.0, 10.0, 1.0e9, 0.0);

    assert_close(light_out, -10.0, 1.0e-8, "light body should reverse");
    assert!(
        heavy_out.abs() < 1.0e-7,
        "heavy body should barely move, got {:e}",
        heavy_out
    );
}

/// Bodies already moving at the same velocity are unaffected.
///
/// There is no relative motion to redistribute, so both solvers must be
/// no-ops — and the inelastic one loses no energy, which is the boundary case of
/// the "never gains energy" property.
#[test]
fn equal_velocities_leave_both_solvers_as_no_ops() {
    assert_eq!(elastic_collision_1d(2.0, 4.0, 7.0, 4.0), (4.0, 4.0));
    assert_eq!(perfectly_inelastic_collision_1d(2.0, 4.0, 7.0, 4.0), 4.0);

    let before = [(2.0, 4.0), (7.0, 4.0)];
    let after = [(9.0, 4.0)];
    assert!(kinetic_energy_is_conserved(&before, &after, 1.0e-12));
}

/// A perfectly inelastic collision strictly loses kinetic energy.
///
/// The counterpart to the elastic energy property: this is the case that proves
/// `kinetic_energy_is_conserved` can return `false`, so the elastic tests are not
/// passing vacuously.
#[test]
fn inelastic_collision_strictly_loses_kinetic_energy() {
    let before = [(1.0, 8.0), (3.0, 0.0)];
    let merged = perfectly_inelastic_collision_1d(1.0, 8.0, 3.0, 0.0);
    let after = [(4.0, merged)];

    assert_eq!(merged, 2.0);
    assert_eq!(total_kinetic_energy(&before), 32.0);
    assert_eq!(total_kinetic_energy(&after), 8.0);

    assert!(
        momentum_is_conserved(&before, &after, 1.0e-12),
        "momentum must survive an inelastic collision"
    );
    assert!(
        !kinetic_energy_is_conserved(&before, &after, 1.0e-12),
        "kinetic energy must not"
    );
}

/// A head-on pair with equal and opposite momentum stops dead.
#[test]
fn opposed_momenta_merge_to_rest() {
    assert_eq!(perfectly_inelastic_collision_1d(1.0, 3.0, 3.0, -1.0), 0.0);
}

// ---- Precondition violations ----
//
// Split by build profile, mirroring `tst/core/scene_tests.rs`: descriptive
// `debug_assert!` panics in debug, documented benign values in release. Both
// `cargo test` and `cargo test --release` must pass.

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "momentum: mass_kg")]
fn negative_mass_panics_in_debug_for_momentum() {
    momentum(-1.0, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "kinetic_energy: velocity_m_s")]
fn non_finite_velocity_panics_in_debug_for_kinetic_energy() {
    kinetic_energy(1.0, f64::NAN);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "total_momentum: every body")]
fn an_invalid_body_panics_in_debug_for_total_momentum() {
    total_momentum(&[(1.0, 1.0), (-2.0, 3.0)]);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_potential_energy: distance_m")]
fn zero_distance_panics_in_debug_for_potential_energy() {
    gravitational_potential_energy(1.0, 1.0, 0.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "total_mechanical_energy: potential_energy_j")]
fn non_finite_potential_panics_in_debug() {
    total_mechanical_energy(1.0, f64::NEG_INFINITY);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "momentum_is_conserved: relative_tolerance")]
fn negative_tolerance_panics_in_debug() {
    momentum_is_conserved(&[(1.0, 1.0)], &[(1.0, 1.0)], -1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "elastic_collision_1d: mass_a_kg")]
fn zero_mass_panics_in_debug_for_elastic_collision() {
    elastic_collision_1d(0.0, 1.0, 1.0, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "perfectly_inelastic_collision_1d: mass_b_kg")]
fn zero_mass_panics_in_debug_for_inelastic_collision() {
    perfectly_inelastic_collision_1d(1.0, 1.0, 0.0, 1.0);
}

/// In release, each precondition violation returns its documented fallback.
///
/// Note the deliberate inconsistency for `elastic_collision_1d`: it returns the
/// **inputs unchanged** rather than `(0.0, 0.0)`, because zeroing both velocities
/// would destroy momentum and corrupt a running simulation, whereas declining to
/// resolve the collision conserves everything trivially. The conservation checks
/// return `false`, because an unverifiable claim must never read as a satisfied
/// one.
#[test]
#[cfg(not(debug_assertions))]
fn precondition_violations_return_documented_fallbacks_in_release() {
    // Quantities collapse to zero.
    assert_eq!(momentum(-1.0, 1.0), 0.0);
    assert_eq!(momentum(1.0, f64::NAN), 0.0);
    assert_eq!(kinetic_energy(-1.0, 1.0), 0.0);
    assert_eq!(kinetic_energy(1.0, f64::INFINITY), 0.0);
    assert_eq!(total_momentum(&[(1.0, 1.0), (-2.0, 3.0)]), 0.0);
    assert_eq!(total_kinetic_energy(&[(1.0, f64::NAN)]), 0.0);
    assert_eq!(gravitational_potential_energy(1.0, 1.0, 0.0), 0.0);
    assert_eq!(gravitational_potential_energy(-1.0, 1.0, 1.0), 0.0);
    assert_eq!(total_mechanical_energy(f64::NAN, 1.0), 0.0);

    // Checks refuse to certify what they cannot verify.
    assert!(!momentum_is_conserved(&[(1.0, 1.0)], &[(1.0, 1.0)], -1.0));
    assert!(!momentum_is_conserved(
        &[(-1.0, 1.0)],
        &[(-1.0, 1.0)],
        1.0e-12
    ));
    assert!(!kinetic_energy_is_conserved(
        &[(f64::NAN, 1.0)],
        &[],
        1.0e-12
    ));
    assert!(!energy_is_conserved(f64::NAN, 0.0, 1.0e-12));

    // The elastic solver declines rather than zeroing.
    assert_eq!(elastic_collision_1d(0.0, 7.0, 1.0, -3.0), (7.0, -3.0));
    assert_eq!(elastic_collision_1d(1.0, 7.0, f64::NAN, -3.0), (7.0, -3.0));

    // The inelastic solver has no such fallback available.
    assert_eq!(perfectly_inelastic_collision_1d(0.0, 7.0, 1.0, -3.0), 0.0);
}
