//! Force and acceleration when `F = ma` no longer holds.
//!
//! The relativistic counterpart to [`newtons_laws`](crate::physics_utils::newtons_laws), and the place
//! where relativity stops being a correction and changes the *shape* of the law.
//!
//! # `F = ma` does not fail by a factor — it fails by being the wrong form
//!
//! Newton's second law survives as `F = dp/dt`. What does not survive is `dp/dt = ma`, because `p = γmv`
//! and `γ` depends on `v`, so differentiating picks up a second term. Working it through for a force
//! **along** the motion:
//!
//! ```text
//! dp/dt = m·d(γv)/dt = m·(γ + v·dγ/dv)·a = mγ(1 + γ²β²)·a = γ³·m·a
//! ```
//!
//! and for a force **across** it — where `|v|` and therefore `γ` are momentarily unchanged:
//!
//! ```text
//! dp⊥/dt = γ·m·a⊥
//! ```
//!
//! **The same force produces different accelerations depending on its direction**, differing by exactly
//! `γ²`. At `β = 0.9` that is a factor of `5.26`. So force and acceleration are **not parallel** in
//! general: push a fast-moving body at `45°` to its motion and it accelerates in some other direction
//! entirely.
//!
//! | `β` | `F∥/(ma)` = `γ³` | `F⊥/(ma)` = `γ` | ratio `γ²` |
//! | --- | --- | --- | --- |
//! | `0` | `1.000` | `1.000` | `1.00` |
//! | `0.5` | `1.540` | `1.155` | `1.33` |
//! | `0.9` | `12.07` | `2.294` | `5.26` |
//! | `0.99` | `356.2` | `7.089` | `50.3` |
//!
//! # Why there is no `relativistic_mass` function here
//!
//! Because the two factors above were once called "longitudinal mass" (`γ³m`) and "transverse mass"
//! (`γm`), and the existence of *two* of them is the proof that the idea does not work. No single
//! number multiplying `a` reproduces `dp/dt`, so "mass increases with speed" cannot be made into a
//! consistent law — it is a mnemonic that happens to work for the transverse case and fails for the
//! longitudinal one. Mass in this crate is invariant rest mass, and the `γ`s live in the formulas where
//! they can be seen.
//!
//! # The speed limit, mechanically
//!
//! Under a constant force the longitudinal acceleration falls off as `γ⁻³`, i.e. as `(1 − β²)^{3/2}`:
//!
//! | `β` | `a` under unit force on unit mass |
//! | --- | --- |
//! | `0` | `1.0` |
//! | `0.9` | `8.3e-2` |
//! | `0.99` | `2.8e-3` |
//! | `0.99999` | `8.9e-8` |
//!
//! Nothing forbids `v = c`; it simply takes forever to get there. An LHC proton at `6.8 TeV` has
//! `γ³ = 3.8e11`, so accelerating it by `1 m/s²` along its motion needs `3.8e11` times the Newtonian
//! force — which is why a collider's last few `TeV` are so much more expensive than its first.
//!
//! # Single axis, again
//!
//! "Longitudinal" and "transverse" here mean *relative to the velocity*, and each function takes
//! scalars. A caller with a force at a general angle must resolve it into those two components, apply
//! these separately, and recombine — and the resulting acceleration will not be parallel to the force.
//! This module cannot do that resolution because it never sees a direction; see the sub-layer's module
//! docs on why a genuinely 3-D treatment needs a `4×4` transformation rather than componentwise calls.

use super::lorentz::lorentz_factor;

/// The force needed to produce `acceleration_m_s2` **along** the direction of motion: `F = γ³ma`.
///
/// The longitudinal case, and the expensive one. See the module docs for the derivation from
/// `F = dp/dt` — and note that `dynamics_tests` checks this against a *numerical derivative* of
/// [`relativistic_momentum`](super::momentum::relativistic_momentum) rather than against the formula,
/// so the `γ³` is verified to be `dp/dt` and not merely asserted to be.
///
/// # Parameters
///
/// - `mass_kg` — rest mass, finite and strictly positive.
/// - `velocity_m_s` — the body's current speed, finite with `|v| < c`. Signed, but only `v²` matters.
/// - `acceleration_m_s2` — the desired acceleration along the motion, finite. Signed.
///
/// # Returns
///
/// The force in newtons, or `None` outside the domain above.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{longitudinal_force, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest it is Newton's second law exactly.
/// assert_eq!(longitudinal_force(2.0, 0.0, 3.0), Some(6.0));
///
/// // At gamma = 2 it costs 8x the Newtonian force, not 2x: the factor is gamma cubed.
/// let f = longitudinal_force(1.0, 3.0_f64.sqrt() / 2.0 * c, 1.0).unwrap();
/// assert!((f - 8.0).abs() < 1.0e-9, "got {f}");
/// ```
pub fn longitudinal_force(mass_kg: f64, velocity_m_s: f64, acceleration_m_s2: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 || !acceleration_m_s2.is_finite() {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let force = gamma * gamma * gamma * mass_kg * acceleration_m_s2;

    force.is_finite().then_some(force)
}

/// The force needed to produce `acceleration_m_s2` **across** the direction of motion: `F = γma`.
///
/// The transverse case, cheaper than the longitudinal one by `γ²` because turning a body does not change
/// its speed, so `γ` is momentarily constant and only the direction of `p` rotates.
///
/// This is the factor that makes a cyclotron stop working at relativistic speeds and a synchrotron
/// necessary — the magnetic force is transverse, so the orbital period picks up exactly one factor of
/// `γ`.
///
/// # Returns
///
/// The force in newtons, on the same domain as [`longitudinal_force`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{longitudinal_force, transverse_force, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
/// let v = 3.0_f64.sqrt() / 2.0 * c; // gamma = 2
///
/// // At rest, Newton's second law.
/// assert_eq!(transverse_force(2.0, 0.0, 3.0), Some(6.0));
///
/// // At gamma = 2 a transverse push costs 2x, against 8x along the motion.
/// let across = transverse_force(1.0, v, 1.0).unwrap();
/// let along = longitudinal_force(1.0, v, 1.0).unwrap();
/// assert!((across - 2.0).abs() < 1.0e-9);
/// assert!((along / across - 4.0).abs() < 1.0e-9, "they differ by gamma squared");
/// ```
pub fn transverse_force(mass_kg: f64, velocity_m_s: f64, acceleration_m_s2: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 || !acceleration_m_s2.is_finite() {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let force = gamma * mass_kg * acceleration_m_s2;

    force.is_finite().then_some(force)
}

/// The acceleration a force **along** the motion actually produces: `a = F/(γ³m)`.
///
/// The inverse of [`longitudinal_force`], and the relativistic replacement for
/// [`acceleration_from_net_force`](crate::physics_utils::newtons_laws::acceleration_from_net_force).
///
/// **This is where the speed limit lives.** Hold `force_n` constant and let `v → c`: the acceleration
/// falls off as `(1 − β²)^{3/2}` and reaches zero only in the limit. No clamp anywhere in this crate
/// enforces `v < c`; this factor is what enforces it.
///
/// # Returns
///
/// The acceleration in `m/s²`, or `None` if the mass is not positive-finite, the force is not finite, or
/// `|velocity_m_s| ≥ c`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{longitudinal_acceleration, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest, F/m.
/// assert_eq!(longitudinal_acceleration(2.0, 0.0, 6.0), Some(3.0));
///
/// // The same force buys less and less acceleration as the speed climbs.
/// let slow = longitudinal_acceleration(1.0, 0.0, 1.0).unwrap();
/// let fast = longitudinal_acceleration(1.0, 0.99 * c, 1.0).unwrap();
/// assert!(fast < slow / 300.0, "gamma cubed is 356 at beta = 0.99");
/// assert!(fast > 0.0, "it never actually reaches zero");
/// ```
pub fn longitudinal_acceleration(mass_kg: f64, velocity_m_s: f64, force_n: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 || !force_n.is_finite() {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let acceleration = force_n / (gamma * gamma * gamma * mass_kg);

    acceleration.is_finite().then_some(acceleration)
}

/// The acceleration a force **across** the motion actually produces: `a = F/(γm)`.
///
/// The inverse of [`transverse_force`]. Larger than the longitudinal answer for the same force, by
/// exactly `γ²` — which is the whole reason both functions exist rather than one.
///
/// # Returns
///
/// The acceleration in `m/s²`, on the same domain as [`longitudinal_acceleration`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{longitudinal_acceleration, transverse_acceleration, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
/// let v = 0.9 * c;
///
/// let across = transverse_acceleration(1.0, v, 1.0).unwrap();
/// let along = longitudinal_acceleration(1.0, v, 1.0).unwrap();
///
/// // The same force, two different accelerations — so force and acceleration are not parallel.
/// assert!(across > along);
/// assert!((across / along - 2.2942_f64.powi(2)).abs() < 1.0e-3, "the ratio is gamma squared");
/// ```
pub fn transverse_acceleration(mass_kg: f64, velocity_m_s: f64, force_n: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 || !force_n.is_finite() {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let acceleration = force_n / (gamma * mass_kg);

    acceleration.is_finite().then_some(acceleration)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/dynamics_tests.rs"]
mod tests;
