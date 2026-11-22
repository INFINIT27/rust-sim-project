//! Tests for [`crate::physics_utils::soft_contact`], `#[path]`-included into it.
//!
//! The laws are checked against their closed forms, and the one claim that ties them together is
//! checked by integration: a pair of bodies colliding through the spring-dashpot, stepped finely,
//! must leave at the restitution its damping ratio was set from.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/soft_contact_tests.txt";

/// Collide two bodies of `m1`, `m2` closing at `speed` through a contact of stiffness `k` and the
/// damping ratio for `restitution`, with steps of `1/steps` of the contact time. Returns the
/// speed they part at.
fn collide(m1: f64, m2: f64, k: f64, restitution: f64, speed: f64, steps: f64) -> f64 {
    collide_damped(
        m1,
        m2,
        k,
        damping_ratio_for_restitution(restitution),
        speed,
        steps,
    )
}

/// As [`collide`], with the damping ratio given directly.
fn collide_damped(m1: f64, m2: f64, k: f64, zeta: f64, speed: f64, steps: f64) -> f64 {
    let reduced = m1 * m2 / (m1 + m2);
    let c = 2.0 * zeta * (reduced * k).sqrt();
    let dt = contact_duration(reduced, k) / steps;
    // Separation along the normal: overlap is `-gap`. Start just touching, closing.
    let (mut x1, mut x2, mut v1, mut v2) = (0.0, 0.0, speed, 0.0);
    for _ in 0..(100.0 * steps) as usize {
        let overlap = x1 - x2;
        if overlap < 0.0 && v2 - v1 > 0.0 {
            break;
        }
        let force = if overlap > 0.0 {
            spring_dashpot_force(k, c, overlap, v2 - v1)
        } else {
            0.0
        };
        v1 -= force / m1 * dt;
        v2 += force / m2 * dt;
        x1 += v1 * dt;
        x2 += v2 * dt;
    }
    v2 - v1
}

#[test]
fn the_stiffness_is_the_modulus_times_the_reduced_radius() {
    assert_eq!(contact_stiffness(2.0e9, 1.0, 3.0), 2.0e9 * 0.75);
}

#[test]
fn the_contact_lasts_half_a_period() {
    let t = contact_duration(4.0, 100.0);
    assert!((t - std::f64::consts::PI * 0.2).abs() < 1.0e-12);
}

#[test]
fn the_force_only_pushes() {
    assert_eq!(spring_dashpot_force(10.0, 0.0, 0.5, 0.0), 5.0);
    assert_eq!(spring_dashpot_force(10.0, 100.0, 0.5, 1.0), 0.0);
    assert_eq!(spring_dashpot_force(10.0, 0.0, f64::NAN, 0.0), 0.0);
}

#[test]
fn an_undamped_contact_returns_everything_and_a_critical_one_e_to_the_minus_two() {
    let elastic = collide(1.0, 1.0, 1.0e4, 1.0, 3.0, 400.0);
    assert!((elastic / 3.0 - 1.0).abs() < 0.01, "parted at {elastic}");
    // The textbook says critical damping stops the pair. A contact that never pulls lets go
    // first, at exp(−2) of the approach speed (Schwager & Pöschel 2007), and the integration
    // finds the same.
    let critical = collide_damped(1.0, 1.0, 1.0e4, 1.0, 3.0, 2000.0);
    assert!(
        (critical / 3.0 - (-2.0_f64).exp()).abs() < 0.005,
        "parted at {:.4} of the approach speed",
        critical / 3.0
    );
}

#[test]
fn the_exact_relation_matches_integration_across_the_regimes() {
    // Light, past 1/√2, critical, and overdamped: the closed form and a fine integration agree.
    for zeta in [0.2, 0.8, 1.0, 1.5, 3.0] {
        let integrated = collide_damped(2.0, 3.0, 1.0e4, zeta, 1.0, 4000.0);
        let exact = restitution_for_damping_ratio(zeta);
        assert!(
            (integrated - exact).abs() < 0.005,
            "ζ = {zeta}: integrated {integrated:.4}, exact {exact:.4}"
        );
    }
}

#[test]
fn a_plastic_contact_needs_infinite_damping() {
    assert_eq!(damping_ratio_for_restitution(0.0), f64::INFINITY);
    assert_eq!(restitution_for_damping_ratio(f64::INFINITY), 0.0);
    // The restitution falls steadily as the damping grows.
    let mut previous = 1.0;
    for step in 1..100 {
        let e = restitution_for_damping_ratio(f64::from(step) * 0.05);
        assert!(
            e < previous,
            "ζ = {}: {e} after {previous}",
            f64::from(step) * 0.05
        );
        previous = e;
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn invalid_input_is_declined_in_release() {
    assert_eq!(contact_stiffness(-1.0, 1.0, 1.0), 0.0);
    assert_eq!(damping_ratio_for_restitution(2.0), f64::INFINITY);
    assert_eq!(contact_duration(0.0, 1.0), f64::INFINITY);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// The damping ratio set from a restitution gives that restitution back, whatever the masses
    /// and the speed: the contact law and the instant law agree on what `e` means.
    #[test]
    fn the_damping_gives_back_the_restitution_it_was_set_from(
        restitution in 0.05_f64..0.95,
        m1 in 0.1_f64..10.0,
        m2 in 0.1_f64..10.0,
        speed in 0.1_f64..100.0,
    ) {
        let parted = collide(m1, m2, 1.0e4, restitution, speed, 2000.0);
        prop_assert!(
            (parted / speed - restitution).abs() < 0.01,
            "e = {restitution}: parted at {:.4} of the approach speed", parted / speed
        );
    }

    /// The stiffness is symmetric in the two radii.
    #[test]
    fn the_stiffness_is_symmetric(e in 1.0e6_f64..1.0e13, a in 1.0_f64..1.0e6, b in 1.0_f64..1.0e6) {
        let (ab, ba) = (contact_stiffness(e, a, b), contact_stiffness(e, b, a));
        prop_assert!((ab - ba).abs() <= 1.0e-12 * ab);
    }
}
