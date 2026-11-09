//! Unit and property tests for [`crate::physics_utils::relativity::dynamics`].
//!
//! Compiled into `src/physics_utils/relativity/dynamics.rs` via `#[path]`.
//!
//! # The test that carries this module
//!
//! `the_longitudinal_factor_is_the_derivative_of_momentum`. The `γ³` in
//! [`longitudinal_force`] is not a definition — it is a *claim* that `d(γmv)/dt` equals `γ³ma`, and a
//! test that recomputed `γ³` would be asserting the implementation against itself. So instead the
//! momentum is differentiated **numerically**, with a central difference, and compared. That checks the
//! factor against `F = dp/dt`, which is the law relativity actually keeps.
//!
//! Everything else here is about the consequence that makes this module different from
//! [`newtons_laws`](crate::physics_utils::newtons_laws): the same force produces two different
//! accelerations depending on its direction, so force and acceleration are not parallel and no single
//! "relativistic mass" can exist.

use super::*;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::lorentz::lorentz_factor;
use crate::physics_utils::relativity::momentum::relativistic_momentum;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_dynamics_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

// -----------------------------------------------------------------------
// The gamma-cubed factor, against `F = dp/dt` rather than against itself.
// -----------------------------------------------------------------------

#[test]
fn the_longitudinal_factor_is_the_derivative_of_momentum() {
    // `F = dp/dt` is the law that survives; `gamma^3 m a` is a derived consequence of it. This
    // differentiates `p(t) = gamma(v + a t) m (v + a t)` numerically and checks the result, so the
    // exponent is verified rather than restated. A `gamma^2` or `gamma^4` fails immediately, and so
    // does a missing term in the product rule.
    let mass = 1.0;
    let acceleration = 1.0e6;
    let step = 1.0e-3;

    for beta in [0.1, 0.3, 0.5, 0.7, 0.9] {
        let v = beta * C;

        let momentum_at = |offset: f64| {
            relativistic_momentum(mass, v + acceleration * offset).expect("subluminal")
        };
        let numeric = (momentum_at(step) - momentum_at(-step)) / (2.0 * step);

        let analytic = longitudinal_force(mass, v, acceleration).expect("subluminal");

        assert!(
            (numeric / analytic - 1.0).abs() < 1.0e-6,
            "beta {beta}: dp/dt came out {numeric:e} but gamma^3 m a is {analytic:e}"
        );
    }
}

#[test]
fn the_longitudinal_factor_is_gamma_cubed_and_the_transverse_one_is_gamma() {
    for beta in [0.0, 0.5, 0.9, 0.99] {
        let v = beta * C;
        let gamma = lorentz_factor(v).expect("subluminal");

        let along = longitudinal_force(1.0, v, 1.0).expect("subluminal");
        let across = transverse_force(1.0, v, 1.0).expect("subluminal");

        assert!(
            (along / gamma.powi(3) - 1.0).abs() < 1.0e-12,
            "beta {beta}: longitudinal factor {along}, expected gamma^3 {}",
            gamma.powi(3)
        );
        assert!(
            (across / gamma - 1.0).abs() < 1.0e-12,
            "beta {beta}: transverse factor {across}, expected gamma {gamma}"
        );
    }
}

// -----------------------------------------------------------------------
// The consequence: force and acceleration are not parallel.
// -----------------------------------------------------------------------

#[test]
fn the_same_force_gives_two_different_accelerations_differing_by_gamma_squared() {
    // The single most important fact in this module, and the reason there is no `relativistic_mass`
    // function. If one number multiplying `a` could reproduce `dp/dt`, this ratio would be 1.
    for beta in [0.5, 0.9, 0.99] {
        let v = beta * C;
        let gamma = lorentz_factor(v).expect("subluminal");

        let along = longitudinal_acceleration(1.0, v, 1.0).expect("subluminal");
        let across = transverse_acceleration(1.0, v, 1.0).expect("subluminal");

        assert!(
            across > along,
            "beta {beta}: a transverse push must accelerate more than a longitudinal one"
        );
        assert!(
            (across / along / (gamma * gamma) - 1.0).abs() < 1.0e-12,
            "beta {beta}: the ratio should be gamma^2 = {}, got {}",
            gamma * gamma,
            across / along
        );
    }
}

#[test]
fn the_two_directions_agree_only_at_rest() {
    // At v = 0 both reduce to Newton, so a test at rest cannot distinguish them — which is exactly why
    // the tests above are at relativistic speed.
    assert_eq!(
        longitudinal_force(2.0, 0.0, 3.0),
        transverse_force(2.0, 0.0, 3.0)
    );
    assert_eq!(
        longitudinal_acceleration(2.0, 0.0, 6.0),
        transverse_acceleration(2.0, 0.0, 6.0)
    );

    for beta in [0.1, 0.5, 0.9] {
        let v = beta * C;
        assert_ne!(
            longitudinal_force(1.0, v, 1.0),
            transverse_force(1.0, v, 1.0),
            "beta {beta}: the two must differ once moving"
        );
    }
}

// -----------------------------------------------------------------------
// The speed limit, mechanically.
// -----------------------------------------------------------------------

#[test]
fn a_constant_force_buys_less_and_less_acceleration() {
    // No clamp anywhere in this crate enforces `v < c`. This factor is what enforces it: the
    // acceleration falls off as `(1 - beta^2)^(3/2)` and reaches zero only in the limit.
    let cases = [
        (0.0, 1.0),
        (0.9, 8.2819e-2),
        (0.99, 2.8072e-3),
        (0.99999, 8.9442e-8),
    ];

    let mut previous = f64::INFINITY;

    for (beta, expected) in cases {
        let a = longitudinal_acceleration(1.0, beta * C, 1.0).expect("subluminal");

        assert!(
            (a / expected - 1.0).abs() < 1.0e-3,
            "beta {beta}: expected about {expected:e}, got {a:e}"
        );
        assert!(a > 0.0, "it must never actually reach zero, got {a}");
        assert!(a < previous, "it must decrease monotonically");
        previous = a;
    }
}

#[test]
fn an_lhc_proton_needs_a_hundred_billion_times_the_newtonian_force() {
    // At 6.8 TeV a proton has gamma about 7247, so gamma^3 is 3.8e11. This is why a collider's last
    // few TeV cost so much more than its first — and it is a real engineering figure, not a curiosity.
    let gamma = 7247.4_f64;
    let beta = (1.0 - 1.0 / (gamma * gamma)).sqrt();
    let mass = 1.672_621_923_69e-27;

    let relativistic = longitudinal_force(mass, beta * C, 1.0).expect("subluminal");
    let newtonian = mass * 1.0;

    assert!(
        (relativistic / newtonian / 3.807e11 - 1.0).abs() < 0.01,
        "the ratio came out {:e}, expected about 3.8e11",
        relativistic / newtonian
    );
}

// -----------------------------------------------------------------------
// The Newtonian limit.
// -----------------------------------------------------------------------

#[test]
fn at_rest_it_is_newtons_second_law_exactly() {
    // Not approximately — `gamma` is exactly 1.0 at v = 0, so these are bit-identical to `F = ma`.
    assert_eq!(longitudinal_force(2.0, 0.0, 3.0), Some(6.0));
    assert_eq!(transverse_force(2.0, 0.0, 3.0), Some(6.0));
    assert_eq!(longitudinal_acceleration(2.0, 0.0, 6.0), Some(3.0));
    assert_eq!(transverse_acceleration(2.0, 0.0, 6.0), Some(3.0));
}

#[test]
fn at_solar_system_speeds_it_is_newtonian_to_eight_decimal_places() {
    // The figure that says why `experiment_3`'s integrator is right to be Newtonian. At Earth's
    // orbital speed the longitudinal correction is 3*(gamma-1) = 1.5e-8.
    let v = 29_780.0;
    let relativistic = longitudinal_force(1.0, v, 1.0).expect("subluminal");

    let fractional_excess = relativistic - 1.0;
    assert!(
        (fractional_excess / 1.48e-8 - 1.0).abs() < 0.02,
        "expected about 1.48e-8, got {fractional_excess:e}"
    );
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn a_massless_body_is_rejected_by_every_function() {
    // `acceleration_from_net_force` in the parent layer rejects a zero mass and is right to; dividing
    // by it is not a question with an answer.
    for mass in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(longitudinal_force(mass, 0.5 * C, 1.0), None, "mass {mass}");
        assert_eq!(transverse_force(mass, 0.5 * C, 1.0), None, "mass {mass}");
        assert_eq!(
            longitudinal_acceleration(mass, 0.5 * C, 1.0),
            None,
            "mass {mass}"
        );
        assert_eq!(
            transverse_acceleration(mass, 0.5 * C, 1.0),
            None,
            "mass {mass}"
        );
    }
}

#[test]
fn superluminal_and_non_finite_inputs_have_no_answer() {
    for bad in [C, 2.0 * C, f64::NAN, f64::INFINITY] {
        assert_eq!(longitudinal_force(1.0, bad, 1.0), None, "velocity {bad}");
        assert_eq!(transverse_force(1.0, bad, 1.0), None, "velocity {bad}");
        assert_eq!(
            longitudinal_acceleration(1.0, bad, 1.0),
            None,
            "velocity {bad}"
        );
        assert_eq!(
            transverse_acceleration(1.0, bad, 1.0),
            None,
            "velocity {bad}"
        );
    }

    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            longitudinal_force(1.0, 0.5 * C, bad),
            None,
            "acceleration {bad}"
        );
        assert_eq!(
            longitudinal_acceleration(1.0, 0.5 * C, bad),
            None,
            "force {bad}"
        );
    }
}

#[test]
fn zero_force_gives_zero_acceleration_and_the_reverse() {
    for beta in [0.0, 0.5, 0.99] {
        let v = beta * C;
        assert_eq!(longitudinal_acceleration(1.0, v, 0.0), Some(0.0));
        assert_eq!(transverse_acceleration(1.0, v, 0.0), Some(0.0));
        assert_eq!(longitudinal_force(1.0, v, 0.0), Some(0.0));
        assert_eq!(transverse_force(1.0, v, 0.0), Some(0.0));
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

    /// Force and acceleration are exact inverses, in both directions.
    #[test]
    fn force_and_acceleration_round_trip(
        mass in 1.0e-6_f64..1.0e6,
        beta in -0.999_f64..0.999,
        acceleration in -1.0e6_f64..1.0e6,
    ) {
        prop_assume!(acceleration.abs() > 1.0e-6);
        let v = beta * C;

        let force = longitudinal_force(mass, v, acceleration).expect("subluminal");
        let back = longitudinal_acceleration(mass, v, force).expect("subluminal");
        prop_assert!((back / acceleration - 1.0).abs() < 1.0e-9, "{} vs {}", back, acceleration);

        let force = transverse_force(mass, v, acceleration).expect("subluminal");
        let back = transverse_acceleration(mass, v, force).expect("subluminal");
        prop_assert!((back / acceleration - 1.0).abs() < 1.0e-9, "{} vs {}", back, acceleration);
    }

    /// A relativistic body is always harder to push than a Newtonian one, both ways.
    #[test]
    fn relativity_never_makes_a_push_cheaper(
        mass in 1.0e-3_f64..1.0e3,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let newtonian = mass * 1.0;

        let along = longitudinal_force(mass, v, 1.0).expect("subluminal");
        let across = transverse_force(mass, v, 1.0).expect("subluminal");

        prop_assert!(along >= newtonian * (1.0 - 1.0e-12), "{along} below {newtonian}");
        prop_assert!(across >= newtonian * (1.0 - 1.0e-12), "{across} below {newtonian}");
        prop_assert!(along >= across * (1.0 - 1.0e-12), "longitudinal must cost at least as much");
    }

    /// The ratio between the two directions is exactly `γ²`, at every speed.
    #[test]
    fn the_directional_ratio_is_always_gamma_squared(beta in -0.999_f64..0.999) {
        let v = beta * C;
        let gamma = lorentz_factor(v).expect("subluminal");

        let along = longitudinal_force(1.0, v, 1.0).expect("subluminal");
        let across = transverse_force(1.0, v, 1.0).expect("subluminal");

        prop_assert!(
            (along / across / (gamma * gamma) - 1.0).abs() < 1.0e-12,
            "beta {}: ratio {} vs gamma^2 {}", beta, along / across, gamma * gamma
        );
    }

    /// Both are linear in mass and in acceleration.
    #[test]
    fn force_is_linear_in_mass_and_acceleration(
        beta in -0.99_f64..0.99,
        mass in 1.0e-3_f64..1.0e3,
        acceleration in 1.0e-3_f64..1.0e3,
    ) {
        let v = beta * C;

        let single = longitudinal_force(mass, v, acceleration).expect("subluminal");
        let double_mass = longitudinal_force(2.0 * mass, v, acceleration).expect("subluminal");
        let double_acc = longitudinal_force(mass, v, 2.0 * acceleration).expect("subluminal");

        prop_assert!((double_mass / (2.0 * single) - 1.0).abs() < 1.0e-12);
        prop_assert!((double_acc / (2.0 * single) - 1.0).abs() < 1.0e-12);
    }

    /// A faster body accelerates less under the same force, monotonically.
    ///
    /// The speed limit as a property rather than a table of four values.
    #[test]
    fn a_faster_body_is_always_harder_to_accelerate(a in 0.0_f64..0.99, b in 0.0_f64..0.99) {
        prop_assume!((a - b).abs() > 1.0e-6);

        let (slower, faster) = if a < b { (a, b) } else { (b, a) };
        let slow = longitudinal_acceleration(1.0, slower * C, 1.0).expect("subluminal");
        let fast = longitudinal_acceleration(1.0, faster * C, 1.0).expect("subluminal");

        prop_assert!(fast < slow, "beta {faster} gave {fast}, beta {slower} gave {slow}");
        prop_assert!(fast > 0.0, "and it stays positive");
    }

    /// At everyday speeds both directions are Newton's second law to a part in `1e6`.
    ///
    /// The bound is set by the *longitudinal* factor, which is `γ³` — so the correction is `3(γ−1)`,
    /// three times the one in `slow_momentum_is_newtonian`. At `1e5 m/s` that is `1.7e-7`.
    #[test]
    fn slow_dynamics_is_newtonian(
        mass in 1.0e-3_f64..1.0e3,
        v in -1.0e5_f64..1.0e5,
        acceleration in -1.0e3_f64..1.0e3,
    ) {
        prop_assume!(acceleration.abs() > 1.0e-6);

        let newtonian = mass * acceleration;
        let along = longitudinal_force(mass, v, acceleration).expect("subluminal");
        let across = transverse_force(mass, v, acceleration).expect("subluminal");

        prop_assert!((along - newtonian).abs() / newtonian.abs() < 1.0e-6);
        prop_assert!((across - newtonian).abs() / newtonian.abs() < 1.0e-6);
    }
}
