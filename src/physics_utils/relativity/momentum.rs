//! Relativistic momentum: `p = γmv`.
//!
//! The parent layer keeps momentum in
//! [`conservation_laws`](crate::physics_utils::conservation_laws) and force in
//! [`newtons_laws`](crate::physics_utils::newtons_laws); this sub-layer mirrors that split, with
//! [`dynamics`](super::dynamics) as the counterpart to `newtons_laws`.
//!
//! # Why `p = mv` had to go
//!
//! Not because it is inaccurate — because it is **not conserved**. Take an elastic collision that
//! conserves `Σmv` in one frame, transform every velocity into another frame with
//! [`compose_velocities`](super::velocity::compose_velocities), and `Σmv` no longer balances. A
//! conservation law that holds only in a preferred frame contradicts the first postulate, so either
//! momentum conservation goes or the definition does. Experiment says the definition.
//!
//! `p = γmv` is the unique correction that restores it, and it diverges as `v → c`, which is the
//! mechanism behind the speed limit: **finite force cannot produce unbounded momentum**, but unbounded
//! momentum is exactly what approaching `c` requires.
//!
//! # How much difference it makes
//!
//! `p/(mv) = γ`, so the table in [`lorentz`](super::lorentz) is also the table of momentum errors:
//!
//! | Motion | `p` exceeds `mv` by |
//! | --- | --- |
//! | Earth around the Sun | `4.9e-9` — five parts per billion |
//! | `β = 0.5` | `15%` |
//! | `β = 0.866` | `100%` — exactly double |
//! | `β = 0.99` | `609%` |
//! | An LHC proton at `6.8 TeV` | `7,247×` |
//!
//! Which says again that nothing this crate simulates needs it, and that a particle accelerator is
//! useless without it.

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::{lorentz_factor, lorentz_factor_minus_one};

/// Relativistic momentum `p = γmv`, in `kg·m/s`.
///
/// # Parameters
///
/// - `mass_kg` — **rest** mass, finite and strictly positive. Zero is rejected rather than absorbed: a
///   massless particle carries momentum `p = E/c` and has no rest frame, so `γmv` is not merely zero
///   for it, it is the wrong formula. Returning `0.0` would be a wrong answer dressed as an easy one.
/// - `velocity_m_s` — finite with `|v| < c`. Signed; the sign carries direction and passes through to
///   the result, since `γ` is even in `v`.
///
/// # Returns
///
/// `None` if the mass is not positive-finite or `|velocity_m_s| ≥ c`. See the sub-layer's module docs
/// on why `None` rather than a fallback.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{relativistic_momentum, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest, no momentum.
/// assert_eq!(relativistic_momentum(1.0, 0.0), Some(0.0));
///
/// // At gamma = 2 the momentum is exactly twice the Newtonian mv.
/// let beta = 3.0_f64.sqrt() / 2.0;
/// let p = relativistic_momentum(1.0, beta * c).unwrap();
/// assert!((p / (1.0 * beta * c) - 2.0).abs() < 1.0e-12);
///
/// // Signed, and a massless particle is rejected rather than given zero.
/// assert!(relativistic_momentum(1.0, -0.5 * c).unwrap() < 0.0);
/// assert_eq!(relativistic_momentum(0.0, 0.5 * c), None);
/// ```
pub fn relativistic_momentum(mass_kg: f64, velocity_m_s: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let momentum = gamma * mass_kg * velocity_m_s;

    momentum.is_finite().then_some(momentum)
}

/// How much relativistic momentum exceeds the Newtonian `mv`: `p − mv = (γ−1)mv`.
///
/// The counterpart to [`clock_lag_seconds`](super::time_dilation::clock_lag_seconds) and
/// [`length_contraction_deficit`](super::length_contraction::length_contraction_deficit), and it exists
/// for the same reason: `relativistic_momentum(m, v) - m * v` subtracts two nearly-equal numbers at
/// every speed this crate simulates. Earth's orbital momentum exceeds its Newtonian value by five parts
/// per billion, so the naive difference keeps about seven digits — and none at all for a slow body.
///
/// Built from [`lorentz_factor_minus_one`], so no cancellation survives.
///
/// # Returns
///
/// The excess in `kg·m/s`, signed the same way as the velocity, on the same domain as
/// [`relativistic_momentum`]. Zero at rest.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{momentum_excess, SPEED_OF_LIGHT_M_S};
///
/// // At gamma = 2 the excess equals the Newtonian momentum itself.
/// let beta = 3.0_f64.sqrt() / 2.0;
/// let v = beta * SPEED_OF_LIGHT_M_S;
/// let excess = momentum_excess(1.0, v).unwrap();
/// assert!((excess / v - 1.0).abs() < 1.0e-12);
///
/// // A slow body still has an answer, where the naive subtraction gives exactly zero.
/// assert!(momentum_excess(1.0, 1.0).unwrap() > 0.0);
/// assert_eq!(momentum_excess(1.0, 0.0), Some(0.0));
/// ```
pub fn momentum_excess(mass_kg: f64, velocity_m_s: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return None;
    }

    let gamma_minus_one = lorentz_factor_minus_one(velocity_m_s)?;
    let excess = gamma_minus_one * mass_kg * velocity_m_s;

    excess.is_finite().then_some(excess)
}

/// Recover velocity from momentum: the inverse of [`relativistic_momentum`].
///
/// # The formula, and why it cannot fail
///
/// Inverting `p = γmv` gives, with `r = p/(mc)`:
///
/// ```text
///        c·r
/// v = ─────────
///      √(1 + r²)
/// ```
///
/// which in exact arithmetic is **always** below `c` in magnitude, for any finite `p` however large.
/// That is the speed limit expressed as arithmetic rather than as a constraint.
///
/// Contrast the Newtonian `v = p/m`, which happily returns superluminal answers — so this is one of the
/// few places where the relativistic formula is *better behaved* than the one it replaces rather than
/// merely more accurate.
///
/// # Two numerical guards, both load-bearing
///
/// **Overflow.** For `|r| > 1` the radicand is computed as `1/r² + 1` and the factor of `r` divided
/// out, rather than forming `r²`. Squaring directly would overflow for `|r| > 1.3e154` and return `NaN`
/// for momenta that are perfectly representable.
///
/// **Saturation at `c`.** Above `|r| ≈ 9.5e7`, `1 + 1/r²` rounds to exactly `1.0`, so the formula
/// returns exactly `c` — which would be a **superluminal answer for a massive body**, and worse, one
/// that every other function in this sub-layer then rejects: `lorentz_factor(c)` is `None`, so a
/// perfectly reasonable momentum would poison the next call. The result is therefore clamped to the
/// largest representable speed below `c`, `299_792_457.999_999_4 m/s`, whose `γ` is `4.7e7`.
///
/// So the promise is kept — this **never returns a speed at or above `c`** — but be clear about what it
/// costs: beyond `γβ ≈ 9.5e7` the returned velocity carries no information about how large the momentum
/// was. `f64` has run out of room below `c`, not this function. If you need to distinguish
/// ultrarelativistic momenta, work in `r = p/(mc)` or in energy and do not round-trip through velocity.
/// `the_saturation_point_is_clamped_rather_than_returning_c` pins the boundary.
///
/// # Returns
///
/// The velocity in `m/s`, or `None` if the mass is not positive-finite or the momentum is not finite.
/// Unlike every other function in this sub-layer there is **no domain restriction on the input**: every
/// finite momentum corresponds to a real subluminal velocity.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{relativistic_momentum, velocity_from_momentum, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Round-trips with `relativistic_momentum`.
/// let p = relativistic_momentum(2.0, 0.8 * c).unwrap();
/// let v = velocity_from_momentum(2.0, p).unwrap();
/// assert!((v / (0.8 * c) - 1.0).abs() < 1.0e-12);
///
/// // p = mc gives beta = 1/sqrt(2), a tidy check.
/// let v = velocity_from_momentum(1.0, c).unwrap();
/// assert!((v / c - 0.5_f64.sqrt()).abs() < 1.0e-12);
///
/// // No momentum, however absurd, reaches c.
/// let v = velocity_from_momentum(1.0, 1.0e30 * c).unwrap();
/// assert!(v < c);
/// ```
pub fn velocity_from_momentum(mass_kg: f64, momentum_kg_m_s: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 || !momentum_kg_m_s.is_finite() {
        return None;
    }

    // `r` is `p/(mc)`, which is also `γβ` — dimensionless, and the natural variable for this inverse.
    let r = momentum_kg_m_s / (mass_kg * SPEED_OF_LIGHT_M_S);

    if !r.is_finite() {
        return None;
    }

    let velocity = if r.abs() <= 1.0 {
        SPEED_OF_LIGHT_M_S * r / (1.0 + r * r).sqrt()
    } else {
        // Divide through by `r` first: the radicand stays near `1` and `r²` never forms, so a momentum
        // near `f64::MAX` returns a velocity a hair under `c` instead of `NaN`.
        let inverse = 1.0 / r;
        SPEED_OF_LIGHT_M_S * r.signum() / (inverse * inverse + 1.0).sqrt()
    };

    if !velocity.is_finite() {
        return None;
    }

    // The saturation guard. Once `1/r²` falls below `f64::EPSILON` the radicand is exactly `1.0` and the
    // line above yields exactly `±c`, which is not a velocity a massive body can have and is rejected by
    // every other function here. Step back to the largest representable subluminal speed instead: an
    // answer that is wrong in its last digit beats one that is wrong in kind.
    let ceiling = SPEED_OF_LIGHT_M_S * (1.0 - f64::EPSILON);

    Some(velocity.clamp(-ceiling, ceiling))
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/momentum_tests.rs"]
mod tests;
