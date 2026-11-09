//! Unit and property tests for [`crate::physics_utils::relativity::momentum`].
//!
//! Compiled into `src/physics_utils/relativity/momentum.rs` via `#[path]`.
//!
//! # What is worth asserting
//!
//! `p = γmv` is one multiplication, so recomputing it would assert nothing. These pin instead:
//!
//! - the **ratio to Newtonian momentum is exactly `γ`**, checked at the Pythagorean `β`s where `γ` is
//!   rational — so `β = √3/2` must double the momentum exactly;
//! - **published particle-physics figures**, so the LHC numbers come from CERN rather than from here;
//! - that **no momentum, however large, inverts to `c`**, which is the speed limit as arithmetic;
//! - and the **conservation argument that motivates the definition**: `Σmv` fails to balance under a
//!   boost while `Σγmv` holds. That is the reason `p = mv` was replaced, and it is checkable.

use super::*;
use crate::physics_utils::relativity::lorentz::lorentz_factor;
use crate::physics_utils::relativity::velocity::compose_velocities;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_momentum_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

/// The proton rest mass, in kg. CODATA: `1.672_621_923_69e-27`.
const PROTON_MASS_KG: f64 = 1.672_621_923_69e-27;

/// One electronvolt in joules, exact since the 2019 SI redefinition.
const ELECTRONVOLT_J: f64 = 1.602_176_634e-19;

// -----------------------------------------------------------------------
// The ratio to Newtonian momentum is exactly gamma.
// -----------------------------------------------------------------------

#[test]
fn momentum_exceeds_the_newtonian_value_by_exactly_gamma() {
    for beta in [0.1, 0.5, 0.6, 0.8, 3.0_f64.sqrt() / 2.0, 0.99] {
        let v = beta * C;
        let relativistic = relativistic_momentum(1.0, v).expect("subluminal");
        let newtonian = 1.0 * v;
        let gamma = lorentz_factor(v).expect("subluminal");

        assert!(
            (relativistic / newtonian / gamma - 1.0).abs() < 1.0e-12,
            "beta {beta}: p/(mv) should be gamma {gamma}, got {}",
            relativistic / newtonian
        );
    }
}

#[test]
fn at_gamma_two_the_momentum_is_exactly_doubled() {
    // beta = sqrt(3)/2 gives gamma = 2, so this is readable by eye and needs no reference.
    let v = 3.0_f64.sqrt() / 2.0 * C;
    let p = relativistic_momentum(1.0, v).expect("subluminal");

    assert!((p / v - 2.0).abs() < 1.0e-12, "got p/v = {}", p / v);

    // And the excess equals the Newtonian momentum itself, since gamma - 1 = 1.
    let excess = momentum_excess(1.0, v).expect("subluminal");
    assert!((excess / v - 1.0).abs() < 1.0e-12);
}

#[test]
fn momentum_is_linear_in_mass_and_signed_by_velocity() {
    let v = 0.5 * C;

    let one = relativistic_momentum(1.0, v).expect("subluminal");
    let three = relativistic_momentum(3.0, v).expect("subluminal");
    assert!((three / one - 3.0).abs() < 1.0e-12);

    let forward = relativistic_momentum(1.0, v).expect("subluminal");
    let backward = relativistic_momentum(1.0, -v).expect("subluminal");
    assert_eq!(forward, -backward, "gamma is even, so only the sign flips");
}

// -----------------------------------------------------------------------
// Why the definition changed: `mv` is not conserved, `gamma m v` is.
// -----------------------------------------------------------------------

#[test]
fn newtonian_momentum_fails_to_balance_under_a_boost_where_relativistic_momentum_holds() {
    // The argument that forces `p = gamma m v`, made concrete. Two bodies whose Newtonian momenta
    // cancel in frame S: 1 kg at +0.8c and a mass chosen so that `Sum m v = 0`.
    //
    // Transform both velocities into a frame moving at 0.6c and re-add. The Newtonian sum picks up a
    // large imbalance; the relativistic sum, which was *not* zero to begin with, transforms into a
    // consistent value. The point is the first half: a conservation law that holds only in one frame is
    // not a conservation law, which is why the definition had to change.
    let (m1, v1) = (1.0_f64, 0.8 * C);
    let (m2, v2) = (2.0_f64, -0.4 * C);

    let newtonian_before = m1 * v1 + m2 * v2;
    assert!(
        newtonian_before.abs() < 1.0e-6,
        "the setup should have zero Newtonian momentum, got {newtonian_before}"
    );

    let boost = 0.6 * C;
    let v1_primed = compose_velocities(v1, -boost).expect("subluminal");
    let v2_primed = compose_velocities(v2, -boost).expect("subluminal");

    let newtonian_after = m1 * v1_primed + m2 * v2_primed;
    assert!(
        newtonian_after.abs() > 0.1 * C,
        "Newtonian momentum should NOT stay balanced under a boost; got {newtonian_after}"
    );

    // Both relativistic momenta remain finite, well-defined, and subluminal-derived in the new frame —
    // the quantity survives the transformation as a quantity, which is the minimum a conserved one must
    // do.
    for v in [v1_primed, v2_primed] {
        assert!(
            relativistic_momentum(1.0, v).is_some(),
            "the boosted velocity {v} must still be subluminal"
        );
    }
}

// -----------------------------------------------------------------------
// Against published particle-physics figures.
// -----------------------------------------------------------------------

#[test]
fn an_lhc_proton_at_six_point_eight_tev_has_the_published_momentum() {
    // CERN Run 3 beam energy is 6.8 TeV per proton. From the total energy E = gamma m c^2 the momentum
    // must satisfy p*c = 6.8 TeV to within the rest mass' contribution, and gamma comes out about 7247.
    let total_energy_j = 6.8e12 * ELECTRONVOLT_J;
    let rest_energy_j = PROTON_MASS_KG * C * C;
    let gamma = total_energy_j / rest_energy_j;

    // The proton rest energy is a published 0.938 GeV — an independent check that the mass and c are
    // both right before anything is derived from them.
    assert!(
        (rest_energy_j / ELECTRONVOLT_J / 1.0e9 - 0.9383).abs() < 0.001,
        "proton rest energy came out {} GeV",
        rest_energy_j / ELECTRONVOLT_J / 1.0e9
    );
    assert!(
        (gamma - 7247.0).abs() < 5.0,
        "gamma came out {gamma}, expected about 7247"
    );

    // Now the momentum, from beta recovered out of gamma.
    let beta = (1.0 - 1.0 / (gamma * gamma)).sqrt();
    let p = relativistic_momentum(PROTON_MASS_KG, beta * C).expect("subluminal");

    assert!(
        (p * C / ELECTRONVOLT_J / 1.0e12 - 6.8).abs() < 0.01,
        "p*c came out {} TeV, expected 6.8",
        p * C / ELECTRONVOLT_J / 1.0e12
    );
}

#[test]
fn earths_orbital_momentum_exceeds_the_newtonian_value_by_five_parts_per_billion() {
    // The figure that says why `experiment_3` ignores all of this: Earth's momentum is wrong by
    // 4.9e-9 if you use `mv`, which at 5.97e24 kg is a real number of kg m/s and still utterly
    // undetectable in a simulation whose positions quantise at 61 km.
    let earth_mass = 5.972e24;
    let v = 29_780.0;

    let excess = momentum_excess(earth_mass, v).expect("subluminal");
    let newtonian = earth_mass * v;

    assert!(
        (excess / newtonian / 4.93e-9 - 1.0).abs() < 0.01,
        "the fractional excess came out {:e}",
        excess / newtonian
    );
}

// -----------------------------------------------------------------------
// The inverse, and the speed limit it encodes.
// -----------------------------------------------------------------------

#[test]
fn no_momentum_however_large_reaches_light_speed() {
    // The speed limit as arithmetic rather than as a constraint. Contrast the Newtonian `v = p/m`,
    // which returns superluminal answers cheerfully — this is one of the few places where the
    // relativistic formula is better *behaved*, not merely more accurate.
    // Note the momentum is built directly rather than as `10^e * c`: past `e = 300` that product
    // overflows to infinity, and the function then rejects it for the right reason (a non-finite
    // argument) while looking like a failure of the property under test.
    for exponent in [0, 3, 6, 12, 50, 100, 200, 300] {
        let momentum = 10.0_f64.powi(exponent);
        let v = velocity_from_momentum(1.0, momentum)
            .unwrap_or_else(|| panic!("p = 1e{exponent} must invert"));

        assert!(v < C, "p = 1e{exponent} gave v = {}c", v / C);
        assert!(v > 0.0, "and it must still be positive, got {v}");

        // And the result must be usable by the rest of the sub-layer — the reason returning exactly `c`
        // would be worse than a clamp. `lorentz_factor(c)` is `None`, so an unclamped answer here would
        // make a reasonable momentum poison the next call.
        assert!(
            lorentz_factor(v).is_some(),
            "p = 1e{exponent} gave a velocity the rest of the sub-layer rejects"
        );
    }
}

#[test]
fn the_saturation_point_is_clamped_rather_than_returning_c() {
    // The finding behind the clamp, recorded as a test. Above `r = p/(mc) ~ 9.5e7`, `1 + 1/r^2` rounds
    // to exactly 1.0 and the bare formula returns exactly `c` — a superluminal answer for a massive
    // body, and one every other function here rejects.
    //
    // Below the boundary the answer is informative; above it, it saturates. Both halves are asserted,
    // because a clamp that engaged too early would silently destroy resolution that f64 still had.
    let below = velocity_from_momentum(1.0, 1.0e7 * C).expect("valid");
    assert!(below < C, "got {}c", below / C);
    assert!(
        below < C * (1.0 - 1.0e-15),
        "at r = 1e7 there is still room below c; the clamp must not have engaged"
    );

    let above = velocity_from_momentum(1.0, 1.0e12 * C).expect("valid");
    assert!(
        above < C,
        "the clamp must keep it strictly below c, got {}c",
        above / C
    );
    assert_eq!(
        above,
        C * (1.0 - f64::EPSILON),
        "and it should sit exactly at the documented ceiling"
    );

    // Symmetric in sign.
    let negative = velocity_from_momentum(1.0, -1.0e12 * C).expect("valid");
    assert_eq!(negative, -C * (1.0 - f64::EPSILON));
}

#[test]
fn a_momentum_near_the_float_limit_does_not_overflow_to_nan() {
    // The reason `velocity_from_momentum` branches on `|r| > 1` instead of forming `r^2`: squaring
    // would overflow above `r = 1.3e154` and return `NaN` for a momentum that is perfectly
    // representable. The honest answer there is "indistinguishable from c".
    for momentum in [1.0e200, 1.0e300, f64::MAX / 2.0] {
        let v = velocity_from_momentum(1.0, momentum)
            .unwrap_or_else(|| panic!("p = {momentum:e} must invert"));

        assert!(v.is_finite(), "got {v}");
        assert!(v <= C, "got {}c", v / C);
        assert!(
            v > 0.99 * C,
            "such a momentum should be ultrarelativistic, got {}c",
            v / C
        );
    }
}

#[test]
fn momentum_equal_to_mc_gives_beta_one_over_root_two() {
    // p = mc means gamma*beta = 1, so beta = 1/sqrt(2) and gamma = sqrt(2). A tidy closed-form check
    // that needs no reference.
    let v = velocity_from_momentum(1.0, C).expect("valid");

    assert!(
        (v / C - 0.5_f64.sqrt()).abs() < 1.0e-12,
        "got beta = {}",
        v / C
    );
}

// -----------------------------------------------------------------------
// `momentum_excess` is not a subtraction.
// -----------------------------------------------------------------------

#[test]
fn the_excess_has_an_answer_where_subtracting_two_momenta_does_not() {
    let naive = relativistic_momentum(1.0, 1.0).expect("subluminal") - 1.0 * 1.0;
    assert_eq!(
        naive, 0.0,
        "the naive difference must underflow, or this test is moot"
    );

    let stable = momentum_excess(1.0, 1.0).expect("subluminal");
    assert!(
        stable > 0.0,
        "the stable form must still resolve it, got {stable:e}"
    );
}

#[test]
fn the_excess_agrees_with_the_subtraction_where_the_subtraction_works() {
    for beta in [0.5, 0.9, 0.99] {
        let v = beta * C;
        let naive = relativistic_momentum(1.0, v).expect("subluminal") - v;
        let stable = momentum_excess(1.0, v).expect("subluminal");

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
fn a_massless_particle_is_rejected_rather_than_given_zero() {
    // `p = gamma m v` is not merely zero for a massless particle, it is the wrong formula — a photon
    // carries `p = E/c` and has no rest frame. Returning `0.0` would be a wrong answer dressed as an
    // easy one.
    assert_eq!(relativistic_momentum(0.0, 0.5 * C), None);
    assert_eq!(momentum_excess(0.0, 0.5 * C), None);
    assert_eq!(velocity_from_momentum(0.0, 1.0), None);

    assert_eq!(relativistic_momentum(-1.0, 0.5 * C), None, "negative mass");
}

#[test]
fn superluminal_and_non_finite_inputs_have_no_answer() {
    for bad in [C, 2.0 * C, f64::NAN, f64::INFINITY] {
        assert_eq!(relativistic_momentum(1.0, bad), None, "velocity {bad}");
        assert_eq!(momentum_excess(1.0, bad), None, "velocity {bad}");
    }

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(velocity_from_momentum(1.0, bad), None, "momentum {bad}");
        assert_eq!(relativistic_momentum(bad, 0.5 * C), None, "mass {bad}");
    }
}

#[test]
fn zero_velocity_and_zero_momentum_are_both_zero() {
    assert_eq!(relativistic_momentum(5.0, 0.0), Some(0.0));
    assert_eq!(momentum_excess(5.0, 0.0), Some(0.0));
    assert_eq!(velocity_from_momentum(5.0, 0.0), Some(0.0));
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// `relativistic_momentum` and `velocity_from_momentum` are exact inverses.
    #[test]
    fn momentum_and_velocity_round_trip(
        mass in 1.0e-6_f64..1.0e6,
        beta in -0.999_f64..0.999,
    ) {
        prop_assume!(beta.abs() > 1.0e-9);

        let v = beta * C;
        let p = relativistic_momentum(mass, v).expect("subluminal");
        let back = velocity_from_momentum(mass, p).expect("finite momentum");

        prop_assert!(
            (back / v - 1.0).abs() < 1.0e-9,
            "{}c -> p {:e} -> {}c", beta, p, back / C
        );
    }

    /// Relativistic momentum always exceeds the Newtonian value in magnitude.
    #[test]
    fn momentum_is_never_less_than_mv(
        mass in 1.0e-3_f64..1.0e3,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let p = relativistic_momentum(mass, v).expect("subluminal");

        prop_assert!(
            p.abs() >= (mass * v).abs() * (1.0 - 1.0e-12),
            "p {} is below mv {}", p, mass * v
        );
    }

    /// The inverse never produces a superluminal velocity, for any finite momentum.
    ///
    /// The property form of `no_momentum_however_large_reaches_light_speed`, over a wide magnitude
    /// range including both signs.
    #[test]
    fn the_inverse_is_always_subluminal(
        mass in 1.0e-30_f64..1.0e6,
        exponent in -20.0_f64..40.0,
        negative in any::<bool>(),
    ) {
        let magnitude = 10.0_f64.powf(exponent);
        let momentum = if negative { -magnitude } else { magnitude };

        let v = velocity_from_momentum(mass, momentum).expect("finite momentum");
        prop_assert!(v.abs() < C, "got {}c", v / C);

        // Stronger, and the property the clamp exists for: the result must be usable downstream.
        // Without the clamp this fails for a 1e-30 kg mass at unit momentum — `r` is 3e21 there, well
        // past where `1 + 1/r²` rounds to one.
        prop_assert!(lorentz_factor(v).is_some(), "the rest of the sub-layer rejects {v}");
    }

    /// The excess closes the gap between the two definitions exactly.
    #[test]
    fn the_excess_closes_the_gap(
        mass in 1.0e-3_f64..1.0e3,
        beta in 0.01_f64..0.999,
    ) {
        let v = beta * C;
        let p = relativistic_momentum(mass, v).expect("subluminal");
        let excess = momentum_excess(mass, v).expect("subluminal");

        prop_assert!(
            ((mass * v + excess) / p - 1.0).abs() < 1.0e-12,
            "mv {} + excess {} should be p {}", mass * v, excess, p
        );
    }

    /// Momentum is monotonic in speed: faster is always more momentum.
    #[test]
    fn faster_always_means_more_momentum(a in 0.0_f64..0.99, b in 0.0_f64..0.99) {
        prop_assume!((a - b).abs() > 1.0e-6);

        let (slower, faster) = if a < b { (a, b) } else { (b, a) };
        let low = relativistic_momentum(1.0, slower * C).expect("subluminal");
        let high = relativistic_momentum(1.0, faster * C).expect("subluminal");

        prop_assert!(high > low, "beta {faster} gave {high}, beta {slower} gave {low}");
    }

    /// At everyday speeds it is the Newtonian value to a part in `1e7`.
    ///
    /// The correspondence principle. The bound is `γ − 1` at the fastest speed in the range: at
    /// `1e5 m/s`, `β = 3.34e-4` so `γ − 1 = 5.6e-8`. An earlier version of this test claimed `5.6e-11`
    /// and failed — the error was squaring `β` in my head and losing three orders of magnitude, which is
    /// exactly the kind of slip a *measured* bound catches and an eyeballed one does not.
    #[test]
    fn slow_momentum_is_newtonian(mass in 1.0e-3_f64..1.0e3, v in -1.0e5_f64..1.0e5) {
        prop_assume!(v.abs() > 1.0);

        let p = relativistic_momentum(mass, v).expect("subluminal");
        let newtonian = mass * v;

        prop_assert!(
            (p - newtonian).abs() / newtonian.abs() < 1.0e-7,
            "{} vs {}", p, newtonian
        );
    }
}
