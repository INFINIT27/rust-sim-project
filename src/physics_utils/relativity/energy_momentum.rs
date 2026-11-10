//! The energy–momentum relation `E² = (pc)² + (mc²)²`, and what it says about mass.
//!
//! [`energy`](super::energy) and [`momentum`](super::momentum) each start from a velocity. This
//! module relates the two directly, with no velocity in sight, and that buys three things.
//!
//! 1. **Light fits.** A photon has no rest frame, no `γ`, and `γmv` is the wrong formula for it. But
//!    setting `m = 0` here gives `E = pc`, which is right. So the functions below accept a mass of
//!    **zero**, where the rest of the sub-layer rejects it.
//! 2. **Mass is the invariant.** `E` and `p` both change from frame to frame; `E² − (pc)²` does not,
//!    and it equals `(mc²)²`. That is the definition of mass that survives relativity:
//!    [`invariant_mass`].
//! 3. **The mass of a system is not the sum of its parts.** Apply [`invariant_mass`] to the *total*
//!    energy and momentum of several bodies and the result exceeds the sum of their masses by their
//!    internal kinetic energy over `c²`. Two photons flying apart have mass; one photon does not. A
//!    hot body is heavier than a cold one.
//!
//! # Mass and energy
//!
//! Because of the third point, energy released by a system at rest shows up as lost mass, and
//! [`energy_released`] and [`mass_equivalent`] convert between the two. `c²` is a large number:
//!
//! | Process | Fraction of the mass released |
//! | --- | --- |
//! | Burning hydrogen in oxygen | `1.8e-10` |
//! | Uranium fission | `9e-4` |
//! | Deuterium–tritium fusion | `3.8e-3` |
//! | Matter meeting antimatter | `1` |
//!
//! The Sun's `3.83e26 W` costs it about `4.3` million tonnes of mass every second.

use super::constants::SPEED_OF_LIGHT_M_S;

/// Whether a mass is acceptable here: finite and **not negative**. Zero means light.
fn valid_mass(mass_kg: f64) -> bool {
    mass_kg.is_finite() && mass_kg >= 0.0
}

/// Total energy from momentum: `E = √((pc)² + (mc²)²)`, in joules.
///
/// # Parameters
///
/// - `mass_kg`: rest mass, finite and not negative. **Zero is allowed** and gives `E = |p|c`.
/// - `momentum_kg_m_s`: finite; the sign does not matter.
///
/// # Returns
///
/// `None` if an input is outside those ranges or the result overflows. Computed with `hypot`, so
/// neither square is formed and a large momentum does not overflow early.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::energy_momentum::energy_from_momentum;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest: E = mc^2.
/// assert_eq!(energy_from_momentum(1.0, 0.0), Some(c * c));
///
/// // Light: E = pc.
/// assert_eq!(energy_from_momentum(0.0, 2.0), Some(2.0 * c));
///
/// // The 3-4-5 triangle: pc = 3, mc^2 = 4, E = 5 (in units of c^2 joules).
/// let energy = energy_from_momentum(4.0, 3.0 * c).unwrap();
/// assert!((energy / (c * c) - 5.0).abs() < 1.0e-12);
/// ```
pub fn energy_from_momentum(mass_kg: f64, momentum_kg_m_s: f64) -> Option<f64> {
    if !valid_mass(mass_kg) || !momentum_kg_m_s.is_finite() {
        return None;
    }

    // `c·hypot(p, mc)`: one factor of `c` stays outside, so the intermediate values are a momentum
    // and not an energy squared.
    let energy = SPEED_OF_LIGHT_M_S * momentum_kg_m_s.hypot(mass_kg * SPEED_OF_LIGHT_M_S);

    energy.is_finite().then_some(energy)
}

/// The size of the momentum of a body with a given mass and total energy:
/// `|p| = √(E² − (mc²)²)/c`.
///
/// The direction is not recoverable from an energy, so the result is a magnitude.
///
/// # Returns
///
/// `kg·m/s`, not negative. `None` if the energy is below the rest energy `mc²` (no body has less),
/// or an input is out of range. Zero mass is allowed and gives `E/c`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::energy_momentum::momentum_from_energy;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // E = 5, mc^2 = 4 gives pc = 3.
/// let momentum = momentum_from_energy(4.0, 5.0 * c * c).unwrap();
/// assert!((momentum / c - 3.0).abs() < 1.0e-12);
///
/// // Less than the rest energy is not a slower body. It is no body.
/// assert_eq!(momentum_from_energy(4.0, 3.0 * c * c), None);
/// ```
pub fn momentum_from_energy(mass_kg: f64, energy_j: f64) -> Option<f64> {
    if !valid_mass(mass_kg) || !energy_j.is_finite() || energy_j < 0.0 {
        return None;
    }

    // In momentum units: `E/c` against `mc`. Factored so a body barely moving keeps its digits.
    let energy_over_c = energy_j / SPEED_OF_LIGHT_M_S;
    let mass_times_c = mass_kg * SPEED_OF_LIGHT_M_S;

    if energy_over_c < mass_times_c {
        return None;
    }

    let momentum = ((energy_over_c - mass_times_c) * (energy_over_c + mass_times_c)).sqrt();

    momentum.is_finite().then_some(momentum)
}

/// The mass that goes with a total energy and a total momentum: `m = √(E² − (pc)²)/c²`.
///
/// The one quantity here every frame agrees on. Pass a single body's energy and momentum and it
/// returns that body's rest mass. Pass a **system's** totals and it returns the system's mass, which
/// is more than the sum of the parts whenever the parts move relative to each other.
///
/// # Returns
///
/// Kilograms, not negative; zero for light (`E = |p|c`). `None` if the energy is negative, if
/// `|p|c > E` (nothing has more momentum than light of the same energy), or an input is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::energy_momentum::invariant_mass;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // One photon has no mass.
/// assert_eq!(invariant_mass(1.0, 1.0 / c), Some(0.0));
///
/// // Two such photons flying apart have total energy 2 and total momentum 0. Together they have
/// // mass.
/// let mass = invariant_mass(2.0, 0.0).unwrap();
/// assert!((mass - 2.0 / (c * c)).abs() < 1.0e-30);
/// ```
pub fn invariant_mass(energy_j: f64, momentum_kg_m_s: f64) -> Option<f64> {
    if !energy_j.is_finite() || energy_j < 0.0 || !momentum_kg_m_s.is_finite() {
        return None;
    }

    let energy_over_c = energy_j / SPEED_OF_LIGHT_M_S;
    let momentum = momentum_kg_m_s.abs();

    if momentum > energy_over_c {
        return None;
    }

    let mass =
        ((energy_over_c - momentum) * (energy_over_c + momentum)).sqrt() / SPEED_OF_LIGHT_M_S;

    mass.is_finite().then_some(mass)
}

/// The velocity of anything with total energy `E` and momentum `p`: `v = pc²/E`.
///
/// Works for a massive body and for light alike, and needs no mass. For light it returns `±c`.
///
/// # Returns
///
/// Metres per second, signed like the momentum, at most `c` in magnitude. `None` if the energy is
/// not positive, `|p|c > E`, or an input is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::energy_momentum::velocity_from_energy_and_momentum;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // E = 5, pc = 3: beta = 3/5.
/// let v = velocity_from_energy_and_momentum(5.0 * c * c, 3.0 * c).unwrap();
/// assert!((v / c - 0.6).abs() < 1.0e-12);
///
/// // Light.
/// assert_eq!(velocity_from_energy_and_momentum(c, -1.0), Some(-c));
/// ```
pub fn velocity_from_energy_and_momentum(energy_j: f64, momentum_kg_m_s: f64) -> Option<f64> {
    if !energy_j.is_finite() || energy_j <= 0.0 || !momentum_kg_m_s.is_finite() {
        return None;
    }

    // `β = p/(E/c)`, a ratio of two momenta.
    let beta = momentum_kg_m_s / (energy_j / SPEED_OF_LIGHT_M_S);

    if !beta.is_finite() || beta.abs() > 1.0 {
        return None;
    }

    Some(SPEED_OF_LIGHT_M_S * beta)
}

/// The momentum of light carrying `energy_j`: `p = E/c`.
///
/// Why light can push a solar sail. The amount is small: a kilowatt of sunlight, fully absorbed,
/// pushes with `3.3` micronewtons.
///
/// # Returns
///
/// `kg·m/s`, or `None` if the energy is negative or not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::energy_momentum::photon_momentum;
///
/// // A joule of light.
/// let momentum = photon_momentum(1.0).unwrap();
/// assert!((momentum - 3.335_640_95e-9).abs() < 1.0e-17);
/// ```
pub fn photon_momentum(energy_j: f64) -> Option<f64> {
    if !energy_j.is_finite() || energy_j < 0.0 {
        return None;
    }

    Some(energy_j / SPEED_OF_LIGHT_M_S)
}

/// The mass that an amount of energy weighs: `m = E/c²`.
///
/// The inverse of [`rest_energy`](super::energy::rest_energy), except that zero energy is a
/// legitimate question here and has the answer zero.
///
/// # Returns
///
/// Kilograms, or `None` if the energy is negative or not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::energy_momentum::mass_equivalent;
///
/// // A kilowatt-hour weighs forty picograms.
/// let mass = mass_equivalent(3.6e6).unwrap();
/// assert!((mass - 4.005e-11).abs() < 1.0e-14);
/// ```
pub fn mass_equivalent(energy_j: f64) -> Option<f64> {
    if !energy_j.is_finite() || energy_j < 0.0 {
        return None;
    }

    Some(energy_j / SPEED_OF_LIGHT_M_S / SPEED_OF_LIGHT_M_S)
}

/// The energy released when a system at rest goes from `mass_before_kg` to `mass_after_kg`:
/// `(m_before − m_after)·c²`.
///
/// Positive when mass is lost (fusion, fission, burning, a hot body cooling). Negative when the
/// products are heavier, which means the process needs that much energy put in.
///
/// # Returns
///
/// Joules, or `None` if a mass is negative or not finite, or the result overflows.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::energy_momentum::energy_released;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // A gram that disappears entirely: about 21 kilotons of TNT.
/// let energy = energy_released(1.0e-3, 0.0).unwrap();
/// assert!((energy - 1.0e-3 * c * c).abs() < 1.0);
/// assert!((energy / 4.184e12 - 21.5).abs() < 0.1);
/// ```
pub fn energy_released(mass_before_kg: f64, mass_after_kg: f64) -> Option<f64> {
    if !valid_mass(mass_before_kg) || !valid_mass(mass_after_kg) {
        return None;
    }

    let energy = (mass_before_kg - mass_after_kg) * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S;

    energy.is_finite().then_some(energy)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/energy_momentum_tests.rs"]
mod tests;
