//! Moving rulers are short: `L = L₀/γ`.
//!
//! # The `γ` goes the other way from time dilation, and that is the whole trap
//!
//! Time dilation *multiplies* by `γ`; length contraction *divides* by it.
//!
//! ```text
//! t = γ·τ        (a moving clock's interval, stretched)
//! L = L₀ / γ     (a moving rod's length, shrunk)
//! ```
//!
//! Both say "the moving thing is diminished", but only if you keep straight which quantity belongs to
//! whom: proper time is the *smallest* time, while proper length is the *largest* length.
//! `time_dilation_and_length_contraction_are_not_the_same_function` exists purely to fail if these two
//! ever get wired to the same helper.
//!
//! # Only along the direction of motion
//!
//! A rod moving lengthwise shortens. The same rod's *width* is unchanged — transverse dimensions are
//! untouched by a boost. So a moving sphere is an oblate spheroid, not a smaller sphere, and a moving
//! cube is a slab. [`contracted_length`] takes a scalar and it is the caller's job to have resolved
//! their length along the boost axis first; there is no way for this function to check that, which is
//! why it is said here rather than assumed.
//!
//! # It is not an optical effect
//!
//! The contraction is what a frame *measures*, using simultaneous readings of both ends. What a camera
//! at one point would *photograph* is different, because light from the far end left earlier —
//! a fast-moving cube looks rotated (Penrose–Terrell), not squashed. This module computes the
//! measurement, and the crate has no relativistic renderer.
//!
//! # Real numbers
//!
//! | Object | `v` | Contraction |
//! | --- | --- | --- |
//! | A `1 m` rod at `β = √3/2` | `0.866c` | `0.500 m` — halved |
//! | Earth's diameter, orbital motion | `29,780 m/s` | shorter by `6.3 cm` |
//! | The ISS, `109 m` long | `7,660 m/s` | shorter by `36 nm` |
//!
//! Which is the same conclusion the `γ` table reaches: nothing in this crate's scenes is
//! measurably contracted, and `experiment_3` is right to ignore it.

use super::lorentz::{lorentz_factor, lorentz_factor_minus_one};

/// The length `L = L₀/γ` of an object whose rest length is `proper_length_m`, moving at
/// `velocity_m_s` **along its measured dimension**.
///
/// # Parameters
///
/// - `proper_length_m` — the length in the object's own rest frame, in metres. Must be finite and
///   non-negative: a negative length is not a direction, it is a mistake, and unlike a velocity there
///   is no sign convention that would rescue it.
/// - `velocity_m_s` — speed in our frame, finite with `|v| < c`. The **sign is irrelevant**, as with
///   time dilation: `γ` depends on `v²`.
///
/// # Returns
///
/// The contracted length in metres — never greater than `proper_length_m` — or `None` if
/// `|velocity_m_s| ≥ c`, or either argument is not finite, or `proper_length_m` is negative.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{contracted_length, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest, unchanged.
/// assert_eq!(contracted_length(1.0, 0.0), Some(1.0));
///
/// // gamma = 2 at beta = sqrt(3)/2: a metre rod measures half a metre.
/// let length = contracted_length(1.0, 3.0_f64.sqrt() / 2.0 * c).unwrap();
/// assert!((length - 0.5).abs() < 1.0e-12);
///
/// // Direction does not matter, only speed.
/// assert_eq!(contracted_length(1.0, 0.5 * c), contracted_length(1.0, -0.5 * c));
///
/// // Always shorter, never longer.
/// assert!(contracted_length(1.0, 0.9 * c).unwrap() < 1.0);
/// ```
pub fn contracted_length(proper_length_m: f64, velocity_m_s: f64) -> Option<f64> {
    if !proper_length_m.is_finite() || proper_length_m < 0.0 {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let contracted = proper_length_m / gamma;

    contracted.is_finite().then_some(contracted)
}

/// The rest length `L₀ = γL` of an object measured as `contracted_length_m` while moving at
/// `velocity_m_s`.
///
/// The inverse of [`contracted_length`]: "we measured this moving thing as `L` — how long is it
/// really?" The answer is always at least `contracted_length_m`.
///
/// # Returns
///
/// The proper length in metres, on the same domain as [`contracted_length`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{contracted_length, proper_length, SPEED_OF_LIGHT_M_S};
///
/// let v = 3.0_f64.sqrt() / 2.0 * SPEED_OF_LIGHT_M_S;
///
/// // gamma = 2: something we measure as half a metre is a metre at rest.
/// let rest = proper_length(0.5, v).unwrap();
/// assert!((rest - 1.0).abs() < 1.0e-12);
///
/// // Round-trips with `contracted_length`.
/// let back = contracted_length(rest, v).unwrap();
/// assert!((back - 0.5).abs() < 1.0e-12);
/// ```
pub fn proper_length(contracted_length_m: f64, velocity_m_s: f64) -> Option<f64> {
    if !contracted_length_m.is_finite() || contracted_length_m < 0.0 {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let proper = contracted_length_m * gamma;

    proper.is_finite().then_some(proper)
}

/// How much shorter a moving object measures: `L₀ − L = L₀·(γ−1)/γ`.
///
/// The counterpart to [`clock_lag_seconds`](super::time_dilation::clock_lag_seconds), and it exists for
/// the same reason: `proper_length_m - contracted_length(..)` is a subtraction of two nearly-equal
/// numbers at every speed in this crate. Earth's diameter contracts by `6.3 cm` out of `12,742 km` —
/// a ratio of `4.9e-9`, so the naive difference keeps about seven digits, and for a slow object none.
///
/// # Returns
///
/// The shortfall in metres, always `≥ 0`, or `None` on the same domain as [`contracted_length`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::length_contraction_deficit;
///
/// // Earth's diameter, at Earth's orbital speed: a few centimetres.
/// let deficit = length_contraction_deficit(12_742_000.0, 29_780.0).unwrap();
/// assert!((deficit - 0.0629).abs() < 0.001, "got {deficit} m");
///
/// // A walking pace still has an answer where the naive subtraction would give zero.
/// assert!(length_contraction_deficit(1.0, 1.0).unwrap() > 0.0);
///
/// // At rest, no contraction.
/// assert_eq!(length_contraction_deficit(1.0, 0.0), Some(0.0));
/// ```
pub fn length_contraction_deficit(proper_length_m: f64, velocity_m_s: f64) -> Option<f64> {
    if !proper_length_m.is_finite() || proper_length_m < 0.0 {
        return None;
    }

    let gamma_minus_one = lorentz_factor_minus_one(velocity_m_s)?;
    let gamma = 1.0 + gamma_minus_one;
    let deficit = proper_length_m * gamma_minus_one / gamma;

    deficit.is_finite().then_some(deficit)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/length_contraction_tests.rs"]
mod tests;
