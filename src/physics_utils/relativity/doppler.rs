//! The relativistic Doppler effect: how the frequency of light changes with the source's motion.
//!
//! The classical Doppler effect is about a wave being chased or fled. The relativistic one multiplies
//! that by time dilation, because the source's clock (its oscillation) runs slow as well:
//!
//! ```text
//!  f_observed        1                   θ = 0    approaching   √((1+β)/(1−β))
//! ──────────── = ─────────────           θ = 90°  crossing      1/γ   (time dilation alone)
//!  f_source      γ(1 − β·cos θ)          θ = 180° receding      √((1−β)/(1+β))
//! ```
//!
//! # The sign convention, stated once
//!
//! The one-axis functions take a **`recession_velocity_m_s`: positive when source and observer are
//! moving apart**, negative when closing. Positive velocity means a lower frequency, a longer
//! wavelength, and a positive redshift, which is the convention astronomy uses.
//!
//! Only the *relative* velocity appears. There is no medium, so "the source moves" and "the observer
//! moves" are the same situation. Sound has two formulas; light has one.
//!
//! # Two things with no classical counterpart
//!
//! - **The transverse shift.** A source crossing the line of sight is neither approaching nor
//!   receding, and classically its frequency is unchanged. Relativistically it is lowered by `1/γ`.
//!   The Ives–Stilwell experiment measured exactly this.
//! - **The factor is `e^(−φ)`**, with `φ` the [`rapidity`](super::rapidity). Doppler factors
//!   therefore multiply when velocities compose, which is the same statement as "rapidities add".

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::beta_from_velocity;

/// `β` for a strictly subluminal speed, or `None`.
fn subluminal_beta(velocity_m_s: f64) -> Option<f64> {
    let beta = beta_from_velocity(velocity_m_s)?;

    (beta.abs() < 1.0).then_some(beta)
}

/// The ratio `f_observed / f_source` for motion along the line of sight: `√((1 − β)/(1 + β))`.
///
/// Below `1` when the two are moving apart (`recession_velocity_m_s > 0`), above `1` when closing.
/// Wavelengths scale by the reciprocal.
///
/// # Returns
///
/// `None` if `|recession_velocity_m_s| ≥ c` or it is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::doppler::doppler_factor;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// assert_eq!(doppler_factor(0.0), Some(1.0));
///
/// // Receding at 0.6c halves the frequency; approaching at 0.6c doubles it.
/// assert!((doppler_factor(0.6 * c).unwrap() - 0.5).abs() < 1.0e-12);
/// assert!((doppler_factor(-0.6 * c).unwrap() - 2.0).abs() < 1.0e-12);
/// ```
pub fn doppler_factor(recession_velocity_m_s: f64) -> Option<f64> {
    let beta = subluminal_beta(recession_velocity_m_s)?;
    let factor = ((1.0 - beta) / (1.0 + beta)).sqrt();

    (factor.is_finite() && factor > 0.0).then_some(factor)
}

/// The frequency an observer measures for light emitted at `source_frequency_hz`, for motion along
/// the line of sight.
///
/// # Returns
///
/// Hertz, or `None` if the frequency is not positive and finite or the speed is not strictly
/// subluminal.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::doppler::observed_frequency;
///
/// // A 100 MHz beacon on a ship leaving at 0.8c is received at a third of that.
/// let received = observed_frequency(1.0e8, 0.8 * SPEED_OF_LIGHT_M_S).unwrap();
/// assert!((received - 1.0e8 / 3.0).abs() < 1.0e-3);
/// ```
pub fn observed_frequency(source_frequency_hz: f64, recession_velocity_m_s: f64) -> Option<f64> {
    if !source_frequency_hz.is_finite() || source_frequency_hz <= 0.0 {
        return None;
    }

    let frequency = source_frequency_hz * doppler_factor(recession_velocity_m_s)?;

    frequency.is_finite().then_some(frequency)
}

/// Redshift `z = λ_observed/λ_source − 1` for motion along the line of sight.
///
/// Positive when receding (redshift), negative when approaching (blueshift), and never below `−1`.
/// At low speed `z ≈ β`.
///
/// Like every "difference" quantity in this sub-layer it is not computed by subtracting: at Earth's
/// orbital speed `z` is `1e-4` and `1/doppler_factor − 1` would lose four digits, and at walking pace
/// all of them. With `s = √(1 − β²)` the stable form is `z = (β + β²/(1 + s))/s`.
///
/// # Returns
///
/// `None` if `|recession_velocity_m_s| ≥ c` or it is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::doppler::redshift;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Receding at 0.6c doubles every wavelength: z = 1.
/// assert!((redshift(0.6 * c).unwrap() - 1.0).abs() < 1.0e-12);
///
/// // At 30 km/s, z is beta to four digits.
/// let z = redshift(30_000.0).unwrap();
/// assert!((z / (30_000.0 / c) - 1.0).abs() < 1.0e-4);
/// ```
pub fn redshift(recession_velocity_m_s: f64) -> Option<f64> {
    let beta = subluminal_beta(recession_velocity_m_s)?;
    let root = (1.0 - beta * beta).sqrt();
    let z = (beta + beta * beta / (1.0 + root)) / root;

    z.is_finite().then_some(z)
}

/// The line-of-sight velocity that produces a redshift `z`: the inverse of [`redshift`].
///
/// `β = z(z + 2)/(z² + 2z + 2)`. Positive `z` gives a recession velocity; a blueshift gives a
/// negative one.
///
/// This is the *special-relativistic* reading of a redshift: a source moving through flat space. The
/// redshifts of distant galaxies come from the expansion of space and do not convert to a velocity
/// this way.
///
/// # Returns
///
/// `None` if `redshift` is not finite or is `≤ −1` (a wavelength cannot shrink to nothing). The
/// result is strictly subluminal, clamped to the largest representable speed below `c` once `z` is
/// so large that `f64` cannot tell the two apart.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::doppler::velocity_from_redshift;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// assert!((velocity_from_redshift(1.0).unwrap() / c - 0.6).abs() < 1.0e-12);
/// assert_eq!(velocity_from_redshift(0.0), Some(0.0));
/// assert_eq!(velocity_from_redshift(-1.0), None);
/// ```
pub fn velocity_from_redshift(redshift: f64) -> Option<f64> {
    if !redshift.is_finite() || redshift <= -1.0 {
        return None;
    }

    // For a large `z` divide through by `z²` first, so `z²` never forms.
    let beta = if redshift.abs() <= 1.0 {
        redshift * (redshift + 2.0) / (redshift * redshift + 2.0 * redshift + 2.0)
    } else {
        let inverse = 1.0 / redshift;
        (1.0 + 2.0 * inverse) / (1.0 + 2.0 * inverse + 2.0 * inverse * inverse)
    };

    let ceiling = SPEED_OF_LIGHT_M_S * (1.0 - f64::EPSILON);
    let velocity = (SPEED_OF_LIGHT_M_S * beta).clamp(-ceiling, ceiling);

    velocity.is_finite().then_some(velocity)
}

/// The ratio `f_observed / f_source` for a source crossing the line of sight at right angles (as
/// the observer sees it): `1/γ = √(1 − β²)`.
///
/// Pure time dilation. Always a lowering of frequency, whichever way the source is crossing, so it
/// is even in the speed.
///
/// # Returns
///
/// `None` if `|speed_m_s| ≥ c` or it is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::doppler::transverse_doppler_factor;
///
/// let factor = transverse_doppler_factor(0.6 * SPEED_OF_LIGHT_M_S).unwrap();
/// assert!((factor - 0.8).abs() < 1.0e-12);
/// ```
pub fn transverse_doppler_factor(speed_m_s: f64) -> Option<f64> {
    let beta = subluminal_beta(speed_m_s)?;

    Some((1.0 - beta * beta).sqrt())
}

/// The ratio `f_observed / f_source` for a source moving at any angle to the line of sight:
/// `1/(γ(1 − β·cos θ))`.
///
/// # Parameters
///
/// - `speed_m_s`: the source's speed in the observer's frame. Not negative, below `c`.
/// - `angle_rad`: the angle, **measured in the observer's frame**, between the source's velocity and
///   the direction from the source to the observer at the moment of emission. `0` is heading straight
///   for the observer, `π` straight away, `π/2` crossing.
///
/// The frame the angle is measured in matters: the two frames disagree about it
/// ([aberration](super::velocity_3d::aberration_angle)), and a formula quoted with the source-frame
/// angle looks different.
///
/// # Returns
///
/// `None` if the speed is negative, not strictly subluminal, or an input is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::doppler::doppler_factor_at_angle;
/// use std::f64::consts::PI;
///
/// let v = 0.6 * SPEED_OF_LIGHT_M_S;
///
/// assert!((doppler_factor_at_angle(v, 0.0).unwrap() - 2.0).abs() < 1.0e-12);
/// assert!((doppler_factor_at_angle(v, PI / 2.0).unwrap() - 0.8).abs() < 1.0e-12);
/// assert!((doppler_factor_at_angle(v, PI).unwrap() - 0.5).abs() < 1.0e-12);
/// ```
pub fn doppler_factor_at_angle(speed_m_s: f64, angle_rad: f64) -> Option<f64> {
    if !angle_rad.is_finite() || speed_m_s < 0.0 {
        return None;
    }

    let beta = subluminal_beta(speed_m_s)?;
    let factor = (1.0 - beta * beta).sqrt() / (1.0 - beta * angle_rad.cos());

    (factor.is_finite() && factor > 0.0).then_some(factor)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/doppler_tests.rs"]
mod tests;
