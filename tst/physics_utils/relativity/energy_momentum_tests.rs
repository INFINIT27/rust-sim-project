//! Unit and property tests for [`crate::physics_utils::relativity::energy_momentum`].
//!
//! Compiled into `src/physics_utils/relativity/energy_momentum.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **The relation agrees with `energy.rs` and `momentum.rs`.** Those start from a velocity; this
//!    module never sees one. `E = γmc²` and `p = γmv` fed into `energy_from_momentum` and
//!    `invariant_mass` must return the same energy and the same mass.
//! 2. **Zero mass is light, not an error.** `E = pc`, velocity exactly `c`, invariant mass exactly
//!    zero.
//! 3. **The mass of a system is not the sum of its parts**, and the published mass defects of
//!    nuclear reactions are that statement measured.

use super::*;
use crate::physics_utils::relativity::energy::{rest_energy, total_energy};
use crate::physics_utils::relativity::momentum::relativistic_momentum;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_energy_momentum_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

/// The electron-volt in joules. Exact since the 2019 SI redefinition.
const ELECTRON_VOLT_J: f64 = 1.602_176_634e-19;
/// CODATA 2018.
const ELECTRON_MASS_KG: f64 = 9.109_383_701_5e-31;
/// CODATA 2018.
const PROTON_MASS_KG: f64 = 1.672_621_923_69e-27;
/// The atomic mass unit, CODATA 2018.
const ATOMIC_MASS_UNIT_KG: f64 = 1.660_539_066_60e-27;

// -----------------------------------------------------------------------
// The relation at values checkable by hand.
// -----------------------------------------------------------------------

#[test]
fn a_body_at_rest_has_its_rest_energy() {
    for mass in [ELECTRON_MASS_KG, 1.0, 5.972e24] {
        let energy = energy_from_momentum(mass, 0.0).expect("valid");
        let rest = rest_energy(mass).expect("valid");
        assert!((energy / rest - 1.0).abs() < 1.0e-15);
        assert_eq!(momentum_from_energy(mass, energy), Some(0.0));
    }
}

#[test]
fn the_three_four_five_triangle() {
    // pc = 3, mc^2 = 4, E = 5, in units where c^2 joules is one.
    let unit = C * C;
    let energy = energy_from_momentum(4.0, 3.0 * C).expect("valid");
    assert!((energy / unit - 5.0).abs() < 1.0e-14);

    let momentum = momentum_from_energy(4.0, 5.0 * unit).expect("valid");
    assert!((momentum / C - 3.0).abs() < 1.0e-14);

    let mass = invariant_mass(5.0 * unit, 3.0 * C).expect("valid");
    assert!((mass - 4.0).abs() < 1.0e-14);

    let velocity = velocity_from_energy_and_momentum(5.0 * unit, 3.0 * C).expect("valid");
    assert!((velocity / C - 0.6).abs() < 1.0e-15);
}

#[test]
fn the_sign_of_the_momentum_does_not_change_the_energy() {
    assert_eq!(
        energy_from_momentum(2.0, 1.0e9),
        energy_from_momentum(2.0, -1.0e9)
    );
    assert_eq!(
        invariant_mass(1.0e20, 1.0e9),
        invariant_mass(1.0e20, -1.0e9)
    );
    assert_eq!(
        velocity_from_energy_and_momentum(1.0e20, -1.0e9),
        velocity_from_energy_and_momentum(1.0e20, 1.0e9).map(|v| -v)
    );
}

// -----------------------------------------------------------------------
// Light.
// -----------------------------------------------------------------------

#[test]
fn zero_mass_is_light() {
    let momentum = 2.5e-27;
    let energy = energy_from_momentum(0.0, momentum).expect("zero mass is allowed here");

    assert_eq!(energy, momentum * C);
    assert_eq!(momentum_from_energy(0.0, energy), Some(energy / C));
    assert_eq!(invariant_mass(energy, energy / C), Some(0.0));
    assert_eq!(
        velocity_from_energy_and_momentum(energy, energy / C),
        Some(C)
    );
    assert_eq!(photon_momentum(energy), Some(energy / C));
}

#[test]
fn a_green_photon() {
    // 532 nm, the common green laser line: E = hc/lambda = 2.33 eV, p = h/lambda = 1.245e-27.
    let planck = 6.626_070_15e-34;
    let energy = planck * C / 532.0e-9;
    assert!((energy / ELECTRON_VOLT_J - 2.3305).abs() < 1.0e-3);

    let momentum = photon_momentum(energy).expect("valid");
    assert!((momentum - planck / 532.0e-9).abs() < 1.0e-40);
    assert!((momentum / 1.245e-27 - 1.0).abs() < 1.0e-3);
}

#[test]
fn sunlight_on_a_sail() {
    // 1,361 W/m^2 at Earth. Absorbed, it pushes with I/c = 4.54 micronewtons per square metre.
    let pressure = photon_momentum(1_361.0).expect("valid");
    assert!((pressure / 1.0e-6 - 4.54).abs() < 0.01, "got {pressure:e}");
}

#[test]
fn nothing_carries_more_momentum_than_light_of_the_same_energy() {
    let energy = 1.0;
    let light = energy / C;
    assert_eq!(invariant_mass(energy, 1.01 * light), None);
    assert_eq!(
        velocity_from_energy_and_momentum(energy, 1.01 * light),
        None
    );
    assert_eq!(
        velocity_from_energy_and_momentum(energy, -1.01 * light),
        None
    );
}

// -----------------------------------------------------------------------
// Published figures.
// -----------------------------------------------------------------------

#[test]
fn an_electron_and_a_positron_make_two_511_kev_photons() {
    // At rest, total energy 2 m_e c^2 and total momentum zero. The two photons leave back to back,
    // each with 510.999 keV, which is the line PET scanners detect.
    let total = energy_released(2.0 * ELECTRON_MASS_KG, 0.0).expect("valid");
    let each_kev = 0.5 * total / ELECTRON_VOLT_J / 1.0e3;
    assert!((each_kev - 510.999).abs() < 0.001, "got {each_kev} keV");

    // The pair of photons has the mass of the pair of particles, though neither photon has any.
    let each = 0.5 * total;
    assert_eq!(invariant_mass(each, each / C), Some(0.0));
    let together = invariant_mass(total, 0.0).expect("valid");
    assert!((together / (2.0 * ELECTRON_MASS_KG) - 1.0).abs() < 1.0e-15);
}

#[test]
fn an_lhc_proton_carries_its_energy_as_momentum() {
    // 6.8 TeV total against a rest energy of 0.938 GeV: pc equals E to one part in 1e8, and the
    // speed is 3 m/s short of c.
    let energy = 6.8e12 * ELECTRON_VOLT_J;
    let momentum = momentum_from_energy(PROTON_MASS_KG, energy).expect("valid");
    let shortfall = 1.0 - momentum * C / energy;
    assert!(
        (shortfall / 9.52e-9 - 1.0).abs() < 0.01,
        "1 - pc/E = {shortfall:e}"
    );

    let velocity = velocity_from_energy_and_momentum(energy, momentum).expect("valid");
    assert!(velocity < C);
    assert!(
        (C - velocity - 2.85).abs() < 0.1,
        "c - v = {} m/s",
        C - velocity
    );

    let mass = invariant_mass(energy, momentum).expect("valid");
    assert!(
        (mass / PROTON_MASS_KG - 1.0).abs() < 1.0e-6,
        "the mass comes back out of a number 7,000 times larger"
    );
}

#[test]
fn deuterium_tritium_fusion_releases_17_6_mev() {
    // D + T -> He-4 + n. Atomic masses in u (AME 2016): the electrons cancel on both sides.
    let before = (2.014_101_778 + 3.016_049_281) * ATOMIC_MASS_UNIT_KG;
    let after = (4.002_603_254 + 1.008_664_916) * ATOMIC_MASS_UNIT_KG;
    let energy = energy_released(before, after).expect("valid");

    assert!(
        (energy / ELECTRON_VOLT_J / 1.0e6 - 17.59).abs() < 0.01,
        "got {} MeV",
        energy / ELECTRON_VOLT_J / 1.0e6
    );
    assert!(((before - after) / before - 3.75e-3).abs() < 1.0e-5);
}

#[test]
fn the_sun_loses_four_million_tonnes_a_second() {
    // Luminosity 3.828e26 W (IAU nominal).
    let per_second = mass_equivalent(3.828e26).expect("valid");
    assert!(
        (per_second / 1.0e9 - 4.26).abs() < 0.01,
        "got {per_second:e} kg/s"
    );
}

#[test]
fn splitting_a_nucleus_costs_energy_when_the_parts_are_heavier() {
    // Helium-4 into two deuterons: the products are heavier, so the "release" is negative, 23.8 MeV.
    let before = 4.002_603_254 * ATOMIC_MASS_UNIT_KG;
    let after = 2.0 * 2.014_101_778 * ATOMIC_MASS_UNIT_KG;
    let energy = energy_released(before, after).expect("valid");
    assert!((energy / ELECTRON_VOLT_J / 1.0e6 + 23.85).abs() < 0.01);
}

#[test]
fn mass_and_energy_convert_both_ways() {
    for mass in [ELECTRON_MASS_KG, 1.0e-3, 1.0, 1.989e30] {
        let energy = rest_energy(mass).expect("valid");
        let back = mass_equivalent(energy).expect("valid");
        assert!((back / mass - 1.0).abs() < 1.0e-15);
        assert_eq!(energy_released(mass, 0.0), Some(energy));
    }
    assert_eq!(mass_equivalent(0.0), Some(0.0));
    assert_eq!(energy_released(3.0, 3.0), Some(0.0));
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn an_energy_below_the_rest_energy_belongs_to_no_body() {
    let rest = rest_energy(1.0).expect("valid");
    assert_eq!(momentum_from_energy(1.0, 0.99 * rest), None);
    assert_eq!(momentum_from_energy(1.0, 0.0), None);
    assert_eq!(momentum_from_energy(1.0, rest), Some(0.0));
}

#[test]
fn negative_and_non_finite_inputs_have_no_answer() {
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(energy_from_momentum(bad, 1.0), None);
        assert_eq!(momentum_from_energy(bad, 1.0), None);
        assert_eq!(momentum_from_energy(0.0, bad), None);
        assert_eq!(invariant_mass(bad, 0.0), None);
        assert_eq!(photon_momentum(bad), None);
        assert_eq!(mass_equivalent(bad), None);
        assert_eq!(energy_released(bad, 1.0), None);
        assert_eq!(energy_released(1.0, bad), None);
        assert_eq!(velocity_from_energy_and_momentum(bad, 0.0), None);
    }
    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(energy_from_momentum(1.0, bad), None);
        assert_eq!(invariant_mass(1.0, bad), None);
        assert_eq!(velocity_from_energy_and_momentum(1.0, bad), None);
    }
    assert_eq!(
        velocity_from_energy_and_momentum(0.0, 0.0),
        None,
        "no energy, nothing to move"
    );
}

#[test]
fn a_huge_momentum_does_not_overflow_early() {
    // p^2 c^2 would overflow at p ~ 1e145; hypot does not.
    let energy = energy_from_momentum(1.0, 1.0e200).expect("still representable");
    assert!((energy / (1.0e200 * C) - 1.0).abs() < 1.0e-15);
    assert_eq!(
        energy_from_momentum(1.0, f64::MAX),
        None,
        "this one really does overflow"
    );
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// **The relation holds for `E = γmc²` and `p = γmv`**, the two quantities computed elsewhere
    /// from a velocity.
    #[test]
    fn the_relation_agrees_with_energy_and_momentum(
        mass in 1.0e-30_f64..1.0e30,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let energy = total_energy(mass, v).expect("valid");
        let momentum = relativistic_momentum(mass, v).expect("valid");

        let from_momentum = energy_from_momentum(mass, momentum).expect("valid");
        prop_assert!((from_momentum / energy - 1.0).abs() < 1.0e-14);

        // Recovering v needs no amplification: beta = pc/E directly.
        let velocity = velocity_from_energy_and_momentum(energy, momentum).expect("valid");
        prop_assert!((velocity / C - beta).abs() < 1.0e-14);
    }

    /// **The invariant mass is the rest mass, whatever the speed.** `E − pc` is `E(1 − β)`, so the
    /// tolerance grows as `1/(1 − |β|)`.
    #[test]
    fn the_invariant_mass_is_the_rest_mass(
        mass in 1.0e-30_f64..1.0e30,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let energy = total_energy(mass, v).expect("valid");
        let momentum = relativistic_momentum(mass, v).expect("valid");

        // Rounding in E and p can put |p|c a hair above E only for beta far closer to 1 than this.
        let recovered = invariant_mass(energy, momentum).expect("E >= |p|c");
        let tolerance = 1.0e-15 / (1.0 - beta.abs());
        prop_assert!(
            (recovered / mass - 1.0).abs() < tolerance,
            "mass {} came back as {} at beta {}", mass, recovered, beta
        );
    }

    /// Momentum to energy and back, for massive bodies and for light.
    #[test]
    fn energy_and_momentum_round_trip(
        mass in prop_oneof![Just(0.0), 1.0e-30_f64..1.0e3],
        gamma_beta in 1.0e-3_f64..1.0e3,
    ) {
        // For light, gamma_beta just scales an arbitrary momentum.
        let momentum = if mass == 0.0 { gamma_beta } else { gamma_beta * mass * C };
        let energy = energy_from_momentum(mass, momentum).expect("valid");
        let back = momentum_from_energy(mass, energy).expect("E >= mc^2");

        // (E - mc^2) loses digits when the body is slow: relative error ~ eps / gamma_beta^2.
        let tolerance = 1.0e-15 * (1.0 + 1.0 / (gamma_beta * gamma_beta));
        prop_assert!((back / momentum - 1.0).abs() < tolerance);
    }

    /// The energy is at least the rest energy and at least `|p|c`.
    #[test]
    fn the_energy_exceeds_both_of_its_parts(
        mass in 0.0_f64..1.0e30,
        momentum in -1.0e40_f64..1.0e40,
    ) {
        let energy = energy_from_momentum(mass, momentum).expect("valid");
        prop_assert!(energy >= mass * C * C * (1.0 - 1.0e-15));
        prop_assert!(energy >= momentum.abs() * C * (1.0 - 1.0e-15));
    }
}
