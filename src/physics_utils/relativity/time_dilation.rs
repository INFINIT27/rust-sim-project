//! Moving clocks run slow: `t = γτ`.
//!
//! # Which one is bigger, settled once
//!
//! **Proper time `τ`** is what a clock reads that is *present at both events* — carried along with the
//! moving object, at rest in its own frame. **Coordinate time `t`** is what a clock reads in the frame
//! the object is moving through. Since `γ ≥ 1`:
//!
//! ```text
//! t = γτ  ≥  τ
//! ```
//!
//! The moving clock accumulates *less* time. Proper time is the shortest time between two events, and
//! that is the whole content of the result — every sign confusion about time dilation is a confusion
//! about which of the two clocks was at both events.
//!
//! # The reciprocity that is not a paradox
//!
//! Each observer sees the *other's* clock running slow, and both are right, because they disagree
//! about which pairs of events are simultaneous. Nothing here resolves the twin paradox: that needs an
//! observer who accelerates, which breaks the symmetry and is outside this module's inertial-frame
//! premise. These functions answer "how much time passed" for a *given* pair of frames, not "who is
//! older when they meet".
//!
//! # Real numbers
//!
//! | Clock | `v` | SR drift |
//! | --- | --- | --- |
//! | GPS satellite | `3,874 m/s` | loses `7.2 µs/day` |
//! | ISS | `7,660 m/s` | loses `28.2 µs/day` |
//! | Earth's orbital motion | `29,780 m/s` | loses `0.156 s/year` |
//!
//! The GPS figure is the one worth remembering: it is a real engineering correction, and the satellites
//! would be useless within hours without it. Note that the *net* GPS correction is about `+38 µs/day`,
//! because general-relativistic gravitational time dilation runs the other way and is larger. This
//! module computes only the `−7.2` part; the crate has no GR.

use super::lorentz::{lorentz_factor, lorentz_factor_minus_one};

/// Coordinate time `t = γτ` for a clock moving at `velocity_m_s` that measured `proper_time_s`.
///
/// "This clock aboard the moving object ticked `proper_time_s`. How long did that take by *our*
/// clocks?" The answer is always at least `proper_time_s`.
///
/// # Parameters
///
/// - `proper_time_s` — the interval the moving clock read, in seconds. Must be finite and
///   non-negative, matching this crate's `dt` contract everywhere else.
/// - `velocity_m_s` — the clock's speed in our frame. Must be finite with `|v| < c`. The **sign is
///   irrelevant**: `γ` depends on `v²`, so a clock receding dilates exactly as one approaching does.
///
/// # Returns
///
/// The dilated interval in seconds, or `None` if `|velocity_m_s| ≥ c` or either argument is not finite
/// or `proper_time_s` is negative. Zero in gives zero out for any valid speed.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{dilated_time, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // At rest nothing happens.
/// assert_eq!(dilated_time(10.0, 0.0), Some(10.0));
///
/// // gamma = 2 at beta = sqrt(3)/2: one second aboard is two seconds here.
/// let t = dilated_time(1.0, 3.0_f64.sqrt() / 2.0 * c).unwrap();
/// assert!((t - 2.0).abs() < 1.0e-12);
///
/// // Direction does not matter, only speed.
/// assert_eq!(dilated_time(1.0, 0.5 * c), dilated_time(1.0, -0.5 * c));
///
/// // The moving clock always reads less than ours.
/// assert!(dilated_time(1.0, 0.9 * c).unwrap() > 1.0);
/// ```
pub fn dilated_time(proper_time_s: f64, velocity_m_s: f64) -> Option<f64> {
    if !proper_time_s.is_finite() || proper_time_s < 0.0 {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let dilated = gamma * proper_time_s;

    dilated.is_finite().then_some(dilated)
}

/// Proper time `τ = t/γ` for a clock moving at `velocity_m_s` while `coordinate_time_s` passed here.
///
/// The inverse of [`dilated_time`], and the direction you want when asking "our clocks say a day
/// passed — what did the satellite's clock read?" The answer is always at most `coordinate_time_s`.
///
/// # Returns
///
/// The proper interval in seconds, on the same domain as [`dilated_time`]. Never greater than
/// `coordinate_time_s`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{dilated_time, proper_time, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // gamma = 2: two seconds here is one aboard.
/// let tau = proper_time(2.0, 3.0_f64.sqrt() / 2.0 * c).unwrap();
/// assert!((tau - 1.0).abs() < 1.0e-12);
///
/// // Exactly inverse to `dilated_time`, round-tripping to float tolerance.
/// let back = dilated_time(tau, 3.0_f64.sqrt() / 2.0 * c).unwrap();
/// assert!((back - 2.0).abs() < 1.0e-12);
///
/// // The moving clock never reads more than ours.
/// assert!(proper_time(1.0, 0.9 * c).unwrap() < 1.0);
/// ```
pub fn proper_time(coordinate_time_s: f64, velocity_m_s: f64) -> Option<f64> {
    if !coordinate_time_s.is_finite() || coordinate_time_s < 0.0 {
        return None;
    }

    let gamma = lorentz_factor(velocity_m_s)?;
    let proper = coordinate_time_s / gamma;

    proper.is_finite().then_some(proper)
}

/// How far a moving clock falls behind ours over `coordinate_time_s`: `t − τ = t·(γ−1)/γ`.
///
/// # Why this is not `coordinate_time_s - proper_time(..)`
///
/// Because that subtraction is the cancellation [`lorentz_factor_minus_one`] exists to avoid, and at
/// every speed this crate simulates it is catastrophic. A GPS satellite's drift over one day is
/// `7.2 µs` against a `86,400 s` day — a ratio of `8.3e-11`, so the naive difference of two `f64`
/// day-lengths keeps about five significant digits and, at a walking pace, exactly zero.
///
/// This is the function to use for any drift, offset, or accumulated-error question. It is built from
/// `γ−1` and never subtracts two nearly-equal times.
///
/// # Returns
///
/// The lag in seconds — always `≥ 0`, since the moving clock is the one that falls behind — or `None`
/// on the same domain as [`dilated_time`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::clock_lag_seconds;
///
/// // A GPS satellite at 3874 m/s over one day: the published figure is about 7.2 microseconds.
/// let lag = clock_lag_seconds(86_400.0, 3_874.0).unwrap();
/// assert!((lag * 1.0e6 - 7.2).abs() < 0.1, "got {} us", lag * 1.0e6);
///
/// // At a walking pace there is still an answer, where the naive subtraction gives zero.
/// assert!(clock_lag_seconds(86_400.0, 1.0).unwrap() > 0.0);
///
/// // At rest, no lag at all.
/// assert_eq!(clock_lag_seconds(86_400.0, 0.0), Some(0.0));
/// ```
pub fn clock_lag_seconds(coordinate_time_s: f64, velocity_m_s: f64) -> Option<f64> {
    if !coordinate_time_s.is_finite() || coordinate_time_s < 0.0 {
        return None;
    }

    // `t − τ = t − t/γ = t·(γ−1)/γ`. Both factors are computed without subtracting nearly-equal
    // quantities: `γ−1` comes from the stable form, and dividing by `γ` is exact to a rounding.
    let gamma_minus_one = lorentz_factor_minus_one(velocity_m_s)?;
    let gamma = 1.0 + gamma_minus_one;
    let lag = coordinate_time_s * gamma_minus_one / gamma;

    lag.is_finite().then_some(lag)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/time_dilation_tests.rs"]
mod tests;
