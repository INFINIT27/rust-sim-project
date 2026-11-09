//! Relativistic energy and work: `E = γmc²`, `KE = (γ−1)mc²`, and `W = ΔKE`.
//!
//! # The work–energy theorem survives, unlike `F = ma`
//!
//! This is the headline. [`dynamics`](super::dynamics) is a story about a Newtonian law breaking: `F = ma`
//! splits into `γ³ma` and `γma` and force stops being parallel to acceleration. Energy is the opposite
//! story — **`W = ΔKE` holds unchanged**. Only the expression for `KE` is different.
//!
//! ```text
//! E  = γmc²          total energy
//! E₀ = mc²           rest energy — present even at rest, which is new
//! KE = E − E₀ = (γ−1)mc²
//! W  = ΔKE           unchanged in form from Newtonian mechanics
//! ```
//!
//! A useful consequence, and one that ties this module to [`dynamics`](super::dynamics): **a transverse
//! force does no work.** It changes direction, not speed, so `γ` is unchanged and `ΔKE = 0`. Only the
//! longitudinal component of a force ever changes a body's energy — which is why a synchrotron's bending
//! magnets cost nothing in beam energy while its accelerating cavities cost everything.
//!
//! # `mc²` is not a formula for kinetic energy
//!
//! `E₀ = mc²` is the energy a body has **at rest**, and it is enormous: `1 kg` is `8.99e16 J`, about
//! `21.5` megatons of TNT. Nothing in this module releases it — mass is conserved here, and converting
//! rest energy into anything else requires nuclear physics this crate does not have. `mc²` appears
//! because it is the natural unit of energy for a body of mass `m`, and every quantity below is most
//! usefully read as a multiple of it:
//!
//! | To reach | `KE / mc²` |
//! | --- | --- |
//! | `β = 0.1` | `0.005` |
//! | `β = 0.5` | `0.155` |
//! | `β = 0.866` | `1.000` — as much again as it already had |
//! | `β = 0.99` | `6.089` |
//! | `β = 0.999` | `21.37` |
//!
//! The column has no upper bound, which is the speed limit stated as economics: **reaching `c` costs
//! infinite energy.**
//!
//! # Why `(γ−1)mc²` must use the stable `γ−1`
//!
//! Harder than anywhere else in this sub-layer, because `c²` is `8.99e16` and multiplies whatever error
//! `γ−1` carries. Measured, for a `1 kg` body, against the Newtonian `½mv²` it must reduce to:
//!
//! | `v` | `½mv²` | `(γ−1)mc²` stable | naive `γ − 1.0` |
//! | --- | --- | --- | --- |
//! | `1 m/s` | `0.5 J` | `0.5 J` | **`0 J`** |
//! | `10 m/s` | `50 J` | `50 J` | **`59.87 J`** — `20%` high |
//! | `1 km/s` | `5.0e5 J` | `5.0e5 J` | `5.0001e5 J` |
//!
//! So the naive form is not merely imprecise at everyday speeds, it is **wrong by tens of percent and
//! then silently zero**. [`lorentz_factor_minus_one`] is what
//! makes this module usable below relativistic speed at all.

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::{lorentz_factor, lorentz_factor_minus_one};

/// Rest energy `E₀ = mc²`, in joules.
///
/// The energy a body has by virtue of existing. Note what this is *not*: a quantity this crate can
/// release, convert, or spend. Mass is conserved throughout, and nothing here turns `mc²` into anything.
///
/// # Returns
///
/// `None` if `mass_kg` is not finite and strictly positive — the same domain as the rest of this
/// sub-layer, and for the same reason: a massless particle has no rest frame, so `mc²` is not zero for
/// it, it is inapplicable.
///
/// # Examples
///
/// ```
/// use physics_sandbox::rest_energy;
///
/// // A kilogram is 8.99e16 J, about 21.5 megatons of TNT.
/// let joules = rest_energy(1.0).unwrap();
/// assert!((joules / 8.987_551_787e16 - 1.0).abs() < 1.0e-9);
/// assert!((joules / 4.184e15 - 21.48).abs() < 0.01, "in megatons");
///
/// // Linear in mass, and undefined for a massless body.
/// assert_eq!(rest_energy(2.0).unwrap(), 2.0 * rest_energy(1.0).unwrap());
/// assert_eq!(rest_energy(0.0), None);
/// ```
pub fn rest_energy(mass_kg: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return None;
    }

    let energy = mass_kg * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S;

    energy.is_finite().then_some(energy)
}

/// Relativistic kinetic energy `KE = (γ−1)mc²`, in joules.
///
/// Named `relativistic_` to sit beside the parent layer's Newtonian
/// [`kinetic_energy`](crate::physics_utils::conservation_laws::kinetic_energy) (`½mv²`) without
/// shadowing it, since both are re-exported at the crate root and a caller should have to say which one
/// they mean.
///
/// # Computed from `γ−1`, necessarily
///
/// Via [`lorentz_factor_minus_one`], never by subtracting `1.0` from `γ`. See the module docs for the
/// measurement: at `10 m/s` the naive form is `20%` high, and at `1 m/s` it returns exactly zero.
///
/// # Returns
///
/// The kinetic energy in joules — always `≥ 0`, and `0` exactly at rest — or `None` if the mass is not
/// positive-finite or `|velocity_m_s| ≥ c`. The sign of the velocity is irrelevant, as with every `γ`
/// quantity.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{relativistic_kinetic_energy, rest_energy, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest, none.
/// assert_eq!(relativistic_kinetic_energy(1.0, 0.0), Some(0.0));
///
/// // At gamma = 2 the kinetic energy equals the rest energy exactly.
/// let ke = relativistic_kinetic_energy(1.0, 3.0_f64.sqrt() / 2.0 * c).unwrap();
/// assert!((ke / rest_energy(1.0).unwrap() - 1.0).abs() < 1.0e-12);
///
/// // At everyday speeds it is 1/2 m v^2 — which the naive `gamma - 1.0` cannot reproduce.
/// let ke = relativistic_kinetic_energy(1.0, 10.0).unwrap();
/// assert!((ke - 50.0).abs() < 1.0e-9, "got {ke}, expected 1/2 * 1 * 10^2");
///
/// // Direction does not matter.
/// assert_eq!(
///     relativistic_kinetic_energy(1.0, 0.5 * c),
///     relativistic_kinetic_energy(1.0, -0.5 * c)
/// );
/// ```
pub fn relativistic_kinetic_energy(mass_kg: f64, velocity_m_s: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return None;
    }

    let gamma_minus_one = lorentz_factor_minus_one(velocity_m_s)?;
    let energy = gamma_minus_one * mass_kg * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S;

    energy.is_finite().then_some(energy)
}

/// Total energy `E = γmc²`, in joules: rest energy plus kinetic.
///
/// # Returns
///
/// The total energy in joules — never below [`rest_energy`] — or `None` on the same domain as
/// [`relativistic_kinetic_energy`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{relativistic_kinetic_energy, rest_energy, total_energy, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
/// let v = 0.6 * c; // gamma = 1.25
///
/// // The three quantities are one equation: E = E0 + KE.
/// let total = total_energy(1.0, v).unwrap();
/// let rest = rest_energy(1.0).unwrap();
/// let kinetic = relativistic_kinetic_energy(1.0, v).unwrap();
/// assert!((total - (rest + kinetic)).abs() / total < 1.0e-12);
///
/// // And at rest the total is just the rest energy.
/// assert_eq!(total_energy(1.0, 0.0), rest_energy(1.0));
/// ```
pub fn total_energy(mass_kg: f64, velocity_m_s: f64) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let energy = gamma * mass_kg * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S;

    energy.is_finite().then_some(energy)
}

/// The work `W = ΔKE = (γ₂ − γ₁)mc²` needed to go from `initial_velocity_m_s` to `final_velocity_m_s`.
///
/// **The work–energy theorem is unchanged by relativity** — this is `ΔKE`, exactly as it would be in
/// Newtonian mechanics. Only the expression for `KE` differs. That is worth stating because the
/// neighbouring module is about a Newtonian law that *does* break.
///
/// Negative when the body is slowed: work is done *by* it rather than on it.
///
/// # Numerical note, and the one case it cannot help
///
/// Computed as the difference of two [`relativistic_kinetic_energy`] values, each built from the stable
/// `γ−1`, rather than by subtracting two `γ`s. That is exact when starting from rest — the common case —
/// and accurate whenever the two speeds differ appreciably.
///
/// It **does** lose digits when the two speeds are nearly equal, because then the answer is genuinely
/// the small difference of two large numbers. That is inherent to the question rather than to the
/// method: asking for the work over an infinitesimal speed change in `f64` has no good answer, and the
/// right move is to integrate `longitudinal_force × distance` instead.
///
/// # Returns
///
/// The work in joules, or `None` if the mass is not positive-finite or either speed is not strictly
/// subluminal. Zero when the two speeds are equal in magnitude — including when they are opposite in
/// sign, since turning a body around takes no *net* work.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{rest_energy, work_done, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Accelerating 1 kg from rest to gamma = 2 costs exactly its rest energy.
/// let w = work_done(1.0, 0.0, 3.0_f64.sqrt() / 2.0 * c).unwrap();
/// assert!((w / rest_energy(1.0).unwrap() - 1.0).abs() < 1.0e-12);
///
/// // Slowing down returns the energy: work is negative and exactly opposite.
/// let back = work_done(1.0, 3.0_f64.sqrt() / 2.0 * c, 0.0).unwrap();
/// assert!((back + w).abs() / w < 1.0e-12);
///
/// // Reversing direction at the same speed is free — no net work.
/// let turn = work_done(1.0, 0.5 * c, -0.5 * c).unwrap();
/// assert_eq!(turn, 0.0);
///
/// // At everyday speeds it is the Newtonian 1/2 m (v2^2 - v1^2).
/// let w = work_done(2.0, 3.0, 5.0).unwrap();
/// assert!((w - (0.5 * 2.0 * (25.0 - 9.0))).abs() < 1.0e-9, "got {w}");
/// ```
pub fn work_done(mass_kg: f64, initial_velocity_m_s: f64, final_velocity_m_s: f64) -> Option<f64> {
    let before = relativistic_kinetic_energy(mass_kg, initial_velocity_m_s)?;
    let after = relativistic_kinetic_energy(mass_kg, final_velocity_m_s)?;

    let work = after - before;

    work.is_finite().then_some(work)
}

/// The speed a body reaches after `work_j` of work is done on it: the inverse of [`work_done`].
///
/// # The formula, and why it is not the obvious one
///
/// With `g = KE_after/(mc²)`, which is `γ−1` after the work:
///
/// ```text
///      c·√(g(g + 2))
/// v = ───────────────
///          1 + g
/// ```
///
/// rather than the textbook `v = c√(1 − 1/γ²)`. The two are algebraically identical, but the textbook
/// form computes `1 − 1/γ²` where `γ` is a hair above `1`, which cancels catastrophically: at
/// `KE = 1e-17 mc²` it returns exactly zero while this form returns `β = 4.47e-9`.
///
/// **The result is always subluminal, for any finite work**, which is the speed limit expressed once
/// more — this time as economics. There is no amount of energy that makes this return `c`.
///
/// # Returns
///
/// The final speed in `m/s`, **always non-negative** — energy is a scalar and cannot say which way the
/// body ended up going, so the caller keeps track of direction. `None` if the mass is not
/// positive-finite, either input is not finite, the initial speed is not subluminal, or the work is
/// negative enough to leave the body with less than zero kinetic energy.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{rest_energy, velocity_after_work, work_done, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // One rest energy of work takes a body from rest to gamma = 2, i.e. beta = sqrt(3)/2.
/// let v = velocity_after_work(1.0, 0.0, rest_energy(1.0).unwrap()).unwrap();
/// assert!((v / c - 3.0_f64.sqrt() / 2.0).abs() < 1.0e-12);
///
/// // Round-trips with `work_done`.
/// let w = work_done(1.0, 0.2 * c, 0.8 * c).unwrap();
/// let reached = velocity_after_work(1.0, 0.2 * c, w).unwrap();
/// assert!((reached / (0.8 * c) - 1.0).abs() < 1.0e-9);
///
/// // No finite amount of work reaches c.
/// let v = velocity_after_work(1.0, 0.0, 1.0e30 * rest_energy(1.0).unwrap()).unwrap();
/// assert!(v < c);
///
/// // Taking out more energy than the body has is not a question with an answer.
/// assert_eq!(velocity_after_work(1.0, 0.0, -1.0), None);
/// ```
pub fn velocity_after_work(mass_kg: f64, initial_velocity_m_s: f64, work_j: f64) -> Option<f64> {
    if !work_j.is_finite() {
        return None;
    }

    let before = relativistic_kinetic_energy(mass_kg, initial_velocity_m_s)?;
    let rest = rest_energy(mass_kg)?;
    let after = before + work_j;

    // Removing more energy than the body has is not a slower body, it is a contradiction.
    if !after.is_finite() || after < 0.0 {
        return None;
    }

    // `g` is `γ−1` after the work, and the whole point of carrying `KE` rather than `γ` through this
    // module is that `g` is accurate all the way down to `1e-17`.
    let g = after / rest;

    if !g.is_finite() {
        return None;
    }

    // `β = √(g(g+2))/(1+g)`, not `√(1 − 1/γ²)`: algebraically the same and numerically not, since the
    // latter subtracts two numbers that agree to sixteen digits whenever `g` is small.
    //
    // Two branches, because that expression is only well-conditioned at one end. Below `g = 1` it is
    // used as written. Above it, `g²` would overflow for `g > 1.3e154` — so numerator and denominator are
    // both divided by `g` first, which is algebraically identical and keeps every intermediate near `1`.
    // Without that split, `1e200` rest-energies of work returned `None` instead of "essentially `c`",
    // which is the same overflow that `velocity_from_momentum` has to dodge.
    let beta = if g <= 1.0 {
        (g * (g + 2.0)).sqrt() / (1.0 + g)
    } else {
        let inverse = 1.0 / g;
        (1.0 + 2.0 * inverse).sqrt() / (1.0 + inverse)
    };

    if !beta.is_finite() {
        return None;
    }

    // The same saturation guard `velocity_from_momentum` needs, for the same reason: for very large `g`
    // the expression rounds to exactly `1`, and a velocity of exactly `c` is one every other function
    // here rejects. Step back to the largest representable subluminal speed.
    let ceiling = 1.0 - f64::EPSILON;

    Some(SPEED_OF_LIGHT_M_S * beta.min(ceiling))
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/energy_tests.rs"]
mod tests;
