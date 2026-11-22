//! Unit and property tests for the [`crate::physics_utils::impacts`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/physics_utils/impacts.rs` via
//! `#[path]`. The laws are pinned at hand-worked points (the Earth and the Moon, the Earth and a
//! Mars-mass impactor) and their shapes are checked over the input space: symmetry in the two
//! bodies, continuity at the super-catastrophic onset, and a remnant that only shrinks as the
//! impact grows.

use super::*;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/impacts_tests.txt";

const EARTH_KG: f64 = 5.9722e24;
const EARTH_M: f64 = 6.371e6;
const MARS_KG: f64 = 6.4171e23;
const MARS_M: f64 = 3.3895e6;

#[test]
fn the_escape_speed_pays_the_binding_energy_exactly() {
    let v = mutual_escape_speed(EARTH_KG, MARS_KG, EARTH_M, MARS_M);
    let reduced = EARTH_KG * MARS_KG / (EARTH_KG + MARS_KG);
    let kinetic = 0.5 * reduced * v * v;
    let binding = GRAVITATIONAL_CONSTANT * EARTH_KG * MARS_KG / (EARTH_M + MARS_M);
    assert!((kinetic / binding - 1.0).abs() < 1.0e-12);
    assert!((v - 9_511.0).abs() < 5.0, "Earth and Mars touching: {v}");
}

#[test]
fn the_specific_impact_energy_is_the_relative_energy_per_kilogram() {
    let q = specific_impact_energy(EARTH_KG, MARS_KG, 9_000.0);
    let reduced = EARTH_KG * MARS_KG / (EARTH_KG + MARS_KG);
    let expected = 0.5 * reduced * 9_000.0 * 9_000.0 / (EARTH_KG + MARS_KG);
    assert!((q / expected - 1.0).abs() < 1.0e-12);
    assert_eq!(specific_impact_energy(1.0, 1.0, 0.0), 0.0);
}

#[test]
fn the_disruption_threshold_for_equal_bodies_is_the_bare_law() {
    // γ = 1 makes the mass-ratio factor exactly 1.
    let mass = 1.0e22;
    let combined_radius = (3.0 * 2.0 * mass / (4.0 * std::f64::consts::PI * 1000.0)).cbrt();
    let expected = 5.0
        * 0.8
        * std::f64::consts::PI
        * 1000.0
        * GRAVITATIONAL_CONSTANT
        * combined_radius.powi(2);
    let q = catastrophic_disruption_threshold(mass, mass);
    assert!((q / expected - 1.0).abs() < 1.0e-12);
}

#[test]
fn an_unequal_pair_is_harder_to_disrupt_than_an_equal_one() {
    let total = 1.0e24;
    let equal = catastrophic_disruption_threshold(total / 2.0, total / 2.0);
    let unequal = catastrophic_disruption_threshold(0.9 * total, 0.1 * total);
    assert!(unequal > equal);
}

#[test]
fn the_earth_and_a_mars_mass_body_from_rest_lose_about_half_a_percent() {
    // The pair released from rest at ten contact distances meets at √0.9 of the escape speed.
    let v = mutual_escape_speed(EARTH_KG, MARS_KG, EARTH_M, MARS_M) * 0.9_f64.sqrt();
    let q = specific_impact_energy(EARTH_KG, MARS_KG, v);
    let q_star = catastrophic_disruption_threshold(EARTH_KG, MARS_KG);
    let debris = 1.0 - largest_remnant_fraction(q, q_star);
    assert!(
        (0.004..0.007).contains(&debris),
        "Q_R = {q:.3e}, Q*_RD = {q_star:.3e}, debris {debris:.4}"
    );
}

#[test]
fn the_remnant_law_passes_through_its_defining_points() {
    assert_eq!(largest_remnant_fraction(0.0, 1.0), 1.0);
    assert_eq!(largest_remnant_fraction(1.0, 1.0), 0.5);
    assert!((largest_remnant_fraction(1.8, 1.0) - 0.1).abs() < 1.0e-12);
    // Past the onset the power law: twice the onset energy leaves 2^-1.5 of 10%.
    assert!((largest_remnant_fraction(3.6, 1.0) - 0.1 * 2.0_f64.powf(-1.5)).abs() < 1.0e-12);
    assert_eq!(largest_remnant_fraction(5.0, f64::INFINITY), 1.0);
}

#[cfg(not(debug_assertions))]
#[test]
fn invalid_input_is_declined_in_release() {
    assert_eq!(mutual_escape_speed(0.0, 0.0, 1.0, 1.0), 0.0);
    assert_eq!(specific_impact_energy(0.0, 1.0, 1.0), 0.0);
    assert_eq!(catastrophic_disruption_threshold(-1.0, 1.0), f64::INFINITY);
    assert_eq!(largest_remnant_fraction(f64::NAN, 1.0), 1.0);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Neither body is special: swapping them changes nothing.
    #[test]
    fn every_law_is_symmetric_in_the_two_bodies(
        a in 1.0e15..1.0e27_f64,
        b in 1.0e15..1.0e27_f64,
        ra in 1.0e3..1.0e8_f64,
        rb in 1.0e3..1.0e8_f64,
        v in 0.0..5.0e4_f64,
    ) {
        prop_assert_eq!(mutual_escape_speed(a, b, ra, rb), mutual_escape_speed(b, a, rb, ra));
        let (qab, qba) = (specific_impact_energy(a, b, v), specific_impact_energy(b, a, v));
        prop_assert!((qab - qba).abs() <= 1.0e-12 * qab.max(1.0));
        let (tab, tba) = (catastrophic_disruption_threshold(a, b), catastrophic_disruption_threshold(b, a));
        prop_assert!((tab - tba).abs() <= 1.0e-12 * tab);
    }

    /// A harder impact never leaves a bigger remnant, and the remnant is always a real share.
    #[test]
    fn the_remnant_shrinks_as_the_impact_grows(q1 in 0.0..10.0_f64, q2 in 0.0..10.0_f64) {
        let (low, high) = (q1.min(q2), q1.max(q2));
        let (f_low, f_high) = (largest_remnant_fraction(low, 1.0), largest_remnant_fraction(high, 1.0));
        prop_assert!(f_high <= f_low);
        prop_assert!(f_high > 0.0 && f_low <= 1.0);
    }
}
