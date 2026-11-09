//! `β` and `γ`: the two numbers everything else in special relativity is written in terms of.
//!
//! `β = v/c` is a speed as a fraction of light speed, and `γ = 1/√(1 − β²)` is the factor by which
//! moving clocks slow, moving rulers shorten, and momentum exceeds `mv`. Both are dimensionless,
//! which is the point — they are what lets a statement about a satellite and a statement about a
//! particle accelerator be the same statement.
//!
//! # `γ` is `1.0` for almost everything in this crate
//!
//! | Motion | `v` | `β` | `γ − 1` |
//! | --- | --- | --- | --- |
//! | A fast walk | `2 m/s` | `6.7e-9` | `2.2e-17` |
//! | An airliner | `250 m/s` | `8.3e-7` | `3.5e-13` |
//! | A GPS satellite | `3,874 m/s` | `1.3e-5` | `8.3e-11` |
//! | The ISS | `7,660 m/s` | `2.6e-5` | `3.3e-10` |
//! | Earth around the Sun | `29,780 m/s` | `9.9e-5` | `4.9e-9` |
//! | Mercury around the Sun | `47,360 m/s` | `1.6e-4` | `1.2e-8` |
//!
//! So the solar system is Newtonian to about eight decimal places, and this module will not visibly
//! change `experiment_3`. That is the correct answer, not a disappointment — and it is exactly why
//! [`lorentz_factor_minus_one`] exists.

use super::constants::SPEED_OF_LIGHT_M_S;

/// A speed as a fraction of light speed: `β = v/c`.
///
/// Dimensionless, and signed — the sign carries direction along the caller's axis, the same
/// convention the rest of [`crate::physics_utils`] uses.
///
/// # Returns
///
/// `v/c`, or `None` if `velocity_m_s` is not finite. A `β` of magnitude `≥ 1` is **returned
/// normally**: this function is a unit conversion, and it is the callers that care whether the result
/// is physical. Rejecting it here would make it impossible to ask "how superluminal was that?" while
/// debugging.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{beta_from_velocity, SPEED_OF_LIGHT_M_S};
///
/// assert_eq!(beta_from_velocity(0.0), Some(0.0));
/// assert_eq!(beta_from_velocity(SPEED_OF_LIGHT_M_S), Some(1.0));
///
/// // Signed, and exact at c because c is exactly representable.
/// assert_eq!(beta_from_velocity(-SPEED_OF_LIGHT_M_S), Some(-1.0));
///
/// // Earth's orbital speed is about one ten-thousandth of light speed.
/// let beta = beta_from_velocity(29_780.0).unwrap();
/// assert!((beta - 9.93e-5).abs() < 1.0e-7);
/// ```
pub fn beta_from_velocity(velocity_m_s: f64) -> Option<f64> {
    if !velocity_m_s.is_finite() {
        return None;
    }

    Some(velocity_m_s / SPEED_OF_LIGHT_M_S)
}

/// The Lorentz factor `γ = 1/√(1 − v²/c²)`.
///
/// The single number that turns a Newtonian statement into a relativistic one. `γ ≥ 1` always, it is
/// `1` only at rest, and it diverges as `v → c`.
///
/// # Returns
///
/// `None` when the question has no answer — `|velocity_m_s| ≥ c`, or a non-finite input. See the
/// module docs for why this is `None` rather than a fallback value: `1.0` would claim there is no
/// relativistic effect, and `INFINITY` would turn into `NaN` one multiplication later.
///
/// Note that `|v| = c` **exactly** is rejected, not just `> c`. A massive object cannot reach `c`, and
/// `γ` genuinely does not exist there; light has no rest frame to dilate, so there is nothing this
/// could usefully return.
///
/// # Precision
///
/// For `β` below about `1e-8` — around `3 m/s` — the true `γ − 1` is smaller than `f64`'s resolution
/// beside `1.0`, so this returns exactly `1.0`. That is not an error, it is the best `f64` can say.
/// But it does mean **`lorentz_factor(v) - 1.0` is the wrong way to compute a small correction**: at
/// Earth's orbital speed it throws away half the significant digits, and at `1 m/s` it returns
/// exactly zero. Use [`lorentz_factor_minus_one`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{lorentz_factor, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest, exactly one.
/// assert_eq!(lorentz_factor(0.0), Some(1.0));
///
/// // The 3-4-5 triangle: beta = 0.6 gives gamma = 1.25 exactly.
/// let gamma = lorentz_factor(0.6 * c).unwrap();
/// assert!((gamma - 1.25).abs() < 1.0e-12);
///
/// // beta = 0.8 gives 5/3.
/// let gamma = lorentz_factor(0.8 * c).unwrap();
/// assert!((gamma - 5.0 / 3.0).abs() < 1.0e-12);
///
/// // At and beyond c there is no answer.
/// assert_eq!(lorentz_factor(c), None);
/// assert_eq!(lorentz_factor(2.0 * c), None);
/// ```
pub fn lorentz_factor(velocity_m_s: f64) -> Option<f64> {
    let beta = beta_from_velocity(velocity_m_s)?;

    if beta.abs() >= 1.0 {
        return None;
    }

    let gamma = 1.0 / (1.0 - beta * beta).sqrt();

    // `1 - beta*beta` is positive by the check above, so the root is real; but for `beta` a hair
    // under `1.0` the subtraction can still round to zero and hand back an infinity. Rejecting that
    // keeps the promise in the return docs — a `Some` is always a usable number.
    if gamma.is_finite() {
        Some(gamma)
    } else {
        None
    }
}

/// `γ − 1`, computed without catastrophic cancellation.
///
/// # Why this is a separate function
///
/// Every relativistic *correction* is proportional to `γ − 1`, and at the speeds this crate actually
/// simulates that quantity is between `1e-8` and `1e-17` — so subtracting `1.0` from a number very
/// close to `1.0` destroys most of it. Measured, at Earth's orbital speed of `29,780 m/s`:
///
/// | | |
/// | --- | --- |
/// | `lorentz_factor(v) - 1.0` | `4.933_759_623_071_410e-9` |
/// | this function | `4.933_759_646_368_286e-9` |
/// | relative disagreement | `4.7e-9` — half the digits gone |
///
/// At `1 m/s` the naive form returns **exactly zero** while the true value is `5.6e-18`.
///
/// # How
///
/// Multiply through by the conjugate:
///
/// ```text
///                 1                  β²
/// γ − 1 =  ───────────── − 1  =  ───────────
///           √(1 − β²)            s·(1 + s)
/// ```
///
/// where `s = √(1 − β²)`. No subtraction of nearly-equal quantities survives, so the result is
/// accurate to full `f64` precision for every `β` in range.
///
/// # Returns
///
/// `None` on the same domain as [`lorentz_factor`]: `|velocity_m_s| ≥ c`, or non-finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{lorentz_factor, lorentz_factor_minus_one, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Agrees with the naive form where the naive form still has digits to spare.
/// let stable = lorentz_factor_minus_one(0.6 * c).unwrap();
/// assert!((stable - 0.25).abs() < 1.0e-12);
///
/// // And has an answer where the naive form has none. A walking pace:
/// let tiny = lorentz_factor_minus_one(1.0).unwrap();
/// assert!(tiny > 0.0, "the true value is 5.6e-18");
/// assert_eq!(lorentz_factor(1.0).unwrap() - 1.0, 0.0, "the naive form underflows");
/// ```
pub fn lorentz_factor_minus_one(velocity_m_s: f64) -> Option<f64> {
    let beta = beta_from_velocity(velocity_m_s)?;

    if beta.abs() >= 1.0 {
        return None;
    }

    let root = (1.0 - beta * beta).sqrt();
    let value = (beta * beta) / (root * (1.0 + root));

    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

/// Whether a speed is one a massive object could actually have: `|v| < c`.
///
/// Exists so callers can ask the domain question without reaching for `lorentz_factor(..).is_some()`,
/// which reads as a computation and allocates a thought about `γ` that the caller did not need.
///
/// A non-finite speed is **not** subluminal — `NaN` compares false against everything, and returning
/// `true` for it would be the one answer that lets a bad value through a guard.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{is_subluminal, SPEED_OF_LIGHT_M_S};
///
/// assert!(is_subluminal(0.0));
/// assert!(is_subluminal(-0.999 * SPEED_OF_LIGHT_M_S));
///
/// // c itself is not: no massive object reaches it, and light has no rest frame.
/// assert!(!is_subluminal(SPEED_OF_LIGHT_M_S));
/// assert!(!is_subluminal(f64::NAN));
/// assert!(!is_subluminal(f64::INFINITY));
/// ```
pub fn is_subluminal(velocity_m_s: f64) -> bool {
    velocity_m_s.is_finite() && velocity_m_s.abs() < SPEED_OF_LIGHT_M_S
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/lorentz_tests.rs"]
mod tests;
