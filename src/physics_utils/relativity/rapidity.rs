//! Rapidity, and motion under constant proper acceleration.
//!
//! # Rapidity: the speed that adds
//!
//! Velocities do not add ([`compose_velocities`](super::velocity::compose_velocities)), but the
//! **rapidity** `φ = atanh(v/c)` does: boosting by `φ₁` and then by `φ₂` along one axis is a boost
//! by `φ₁ + φ₂`. It runs from `−∞` to `+∞` as `v` runs from `−c` to `+c`, so the speed limit is not
//! a wall in rapidity, only an infinitely long road.
//!
//! Everything else is a hyperbolic function of it: `β = tanh φ`, `γ = cosh φ`, `γβ = sinh φ`, and
//! the Doppler factor is `e^(−φ)`. A Lorentz transformation is a rotation by a hyperbolic angle.
//!
//! # Constant proper acceleration: the relativistic `v = at`
//!
//! The parent layer's [`kinematics`](crate::physics_utils::kinematics) has `v = at`, which passes
//! `c` after `c/a` seconds (about `354` days at `1 g`). The relativistic version holds the
//! acceleration the *traveller* feels constant instead. The traveller's rapidity then grows linearly
//! with their own clock, `φ = aτ/c`, and with `k = a·t/c`:
//!
//! ```text
//! v(t) = a·t / √(1 + k²)          never reaches c
//! x(t) = (c²/a)·(√(1 + k²) − 1)   a hyperbola, hence "hyperbolic motion"
//! τ(t) = (c/a)·asinh(k)           the traveller's clock
//! ```
//!
//! All three reduce to `at`, `½at²`, and `t` while `k` is small. All start from rest at the origin.
//!
//! | Ship time at `1 g` | Earth time | Distance | Speed |
//! | --- | --- | --- | --- |
//! | `1 year` | `1.19 years` | `0.56 ly` | `0.77c` |
//! | `2 years` | `3.76 years` | `2.91 ly` | `0.97c` |
//! | `5 years` | `84.5 years` | `83.5 ly` | `0.99993c` |

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::beta_from_velocity;

/// The largest speed below `c` this module returns. See [`velocity_from_rapidity`].
fn subluminal_ceiling() -> f64 {
    SPEED_OF_LIGHT_M_S * (1.0 - f64::EPSILON)
}

/// Rapidity `φ = atanh(v/c)`. Dimensionless and signed like the velocity.
///
/// # Returns
///
/// `None` if `|velocity_m_s| ≥ c` or it is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::rapidity::rapidity;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// assert_eq!(rapidity(0.0), Some(0.0));
///
/// // beta = 0.6 is a rapidity of ln 2: the Doppler factor there is exactly one half.
/// let phi = rapidity(0.6 * c).unwrap();
/// assert!((phi - 2.0_f64.ln()).abs() < 1.0e-12);
///
/// assert_eq!(rapidity(c), None);
/// ```
pub fn rapidity(velocity_m_s: f64) -> Option<f64> {
    let beta = beta_from_velocity(velocity_m_s)?;

    if beta.abs() >= 1.0 {
        return None;
    }

    let phi = beta.atanh();

    phi.is_finite().then_some(phi)
}

/// The velocity with a given rapidity: `v = c·tanh φ`. The inverse of [`rapidity`].
///
/// # Saturation
///
/// In exact arithmetic the result is below `c` for every finite `φ`. In `f64`, `tanh φ` rounds to
/// exactly `1.0` above `φ ≈ 19`, so the result is clamped to the largest representable speed below
/// `c`, for the same reason
/// [`velocity_from_momentum`](super::momentum::velocity_from_momentum) is: `c` itself is not a speed
/// a massive body can have, and every other function here would reject it. Past that point the
/// velocity no longer says how large the rapidity was.
///
/// # Returns
///
/// `None` if `rapidity` is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::rapidity::{rapidity, velocity_from_rapidity};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Two boosts of 0.5c are one boost of 0.8c: the rapidities add.
/// let phi = rapidity(0.5 * c).unwrap();
/// let v = velocity_from_rapidity(phi + phi).unwrap();
/// assert!((v / c - 0.8).abs() < 1.0e-12);
///
/// // However large the rapidity, the speed stays below c.
/// assert!(velocity_from_rapidity(1.0e6).unwrap() < c);
/// ```
pub fn velocity_from_rapidity(rapidity: f64) -> Option<f64> {
    if !rapidity.is_finite() {
        return None;
    }

    let ceiling = subluminal_ceiling();
    let velocity = SPEED_OF_LIGHT_M_S * rapidity.tanh();

    Some(velocity.clamp(-ceiling, ceiling))
}

/// `k = a·t/c`, validated: the one dimensionless number hyperbolic motion depends on.
fn acceleration_parameter(proper_acceleration_m_s2: f64, coordinate_time_s: f64) -> Option<f64> {
    if !proper_acceleration_m_s2.is_finite()
        || !coordinate_time_s.is_finite()
        || coordinate_time_s < 0.0
    {
        return None;
    }

    let k = proper_acceleration_m_s2 * coordinate_time_s / SPEED_OF_LIGHT_M_S;

    k.is_finite().then_some(k)
}

/// The velocity after accelerating from rest for `coordinate_time_s`, at an acceleration that is
/// constant as the traveller feels it: `v = a·t/√(1 + (a·t/c)²)`.
///
/// # Parameters
///
/// - `proper_acceleration_m_s2`: what an accelerometer on board reads. Signed; finite.
/// - `coordinate_time_s`: time in the frame the body started at rest in. Finite, not negative.
///
/// # Returns
///
/// A velocity strictly below `c` in magnitude (clamped as in [`velocity_from_rapidity`] once `f64`
/// can no longer tell it from `c`), or `None` for an input outside the ranges above.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::rapidity::velocity_under_proper_acceleration;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // After c/a seconds Newton says c. The answer is c/sqrt(2).
/// let v = velocity_under_proper_acceleration(10.0, c / 10.0).unwrap();
/// assert!((v / c - 0.5_f64.sqrt()).abs() < 1.0e-12);
///
/// // A car: 3 m/s^2 for 10 s is 30 m/s, as it always was.
/// let v = velocity_under_proper_acceleration(3.0, 10.0).unwrap();
/// assert!((v - 30.0).abs() < 1.0e-9);
/// ```
pub fn velocity_under_proper_acceleration(
    proper_acceleration_m_s2: f64,
    coordinate_time_s: f64,
) -> Option<f64> {
    let k = acceleration_parameter(proper_acceleration_m_s2, coordinate_time_s)?;

    // `k/√(1+k²)` with `hypot`, which does not overflow for a large `k`.
    let ceiling = subluminal_ceiling();
    let velocity = SPEED_OF_LIGHT_M_S * k / 1.0_f64.hypot(k);

    Some(velocity.clamp(-ceiling, ceiling))
}

/// The distance covered from rest after `coordinate_time_s` of constant proper acceleration:
/// `x = (c²/a)·(√(1 + (a·t/c)²) − 1)`.
///
/// Computed as `c·t·k/(1 + √(1 + k²))`, which is the same quantity with the subtraction multiplied
/// out. The textbook form subtracts `1` from a number that is `1` to sixteen digits at any everyday
/// acceleration, and divides by `a`, which fails at `a = 0`. This form does neither.
///
/// # Returns
///
/// Metres, signed like the acceleration, always less than `c·t` in magnitude. `None` on the same
/// domain as [`velocity_under_proper_acceleration`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::rapidity::distance_under_proper_acceleration;
///
/// // A car again: half a t squared.
/// let x = distance_under_proper_acceleration(3.0, 10.0).unwrap();
/// assert!((x - 150.0).abs() < 1.0e-9);
///
/// // No acceleration, no distance.
/// assert_eq!(distance_under_proper_acceleration(0.0, 10.0), Some(0.0));
/// ```
pub fn distance_under_proper_acceleration(
    proper_acceleration_m_s2: f64,
    coordinate_time_s: f64,
) -> Option<f64> {
    let k = acceleration_parameter(proper_acceleration_m_s2, coordinate_time_s)?;

    let distance = SPEED_OF_LIGHT_M_S * coordinate_time_s * (k / (1.0 + 1.0_f64.hypot(k)));

    distance.is_finite().then_some(distance)
}

/// The time the traveller's own clock shows after `coordinate_time_s` of constant proper
/// acceleration: `τ = (c/a)·asinh(a·t/c)`.
///
/// Grows only logarithmically once the traveller is fast, which is why a `1 g` ship crosses the
/// galaxy in about twelve years of its own time.
///
/// # Returns
///
/// Seconds, between `0` and `coordinate_time_s`. `None` on the same domain as
/// [`velocity_under_proper_acceleration`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::rapidity::proper_time_under_proper_acceleration;
///
/// // Without acceleration the two clocks agree.
/// assert_eq!(proper_time_under_proper_acceleration(0.0, 100.0), Some(100.0));
///
/// // With it, the traveller's reads less.
/// let tau = proper_time_under_proper_acceleration(9.81, 1.0e8).unwrap();
/// assert!(tau < 1.0e8);
/// ```
pub fn proper_time_under_proper_acceleration(
    proper_acceleration_m_s2: f64,
    coordinate_time_s: f64,
) -> Option<f64> {
    let k = acceleration_parameter(proper_acceleration_m_s2, coordinate_time_s)?;

    // `(c/a)·asinh(k) = t·asinh(k)/k`, and the ratio tends to `1` as `k` tends to `0`.
    let proper_time = if k == 0.0 {
        coordinate_time_s
    } else {
        coordinate_time_s * (k.asinh() / k)
    };

    proper_time.is_finite().then_some(proper_time)
}

/// The time that passes in the starting frame while the traveller's clock advances
/// `proper_time_s`: `t = (c/a)·sinh(a·τ/c)`. The inverse of
/// [`proper_time_under_proper_acceleration`].
///
/// # Returns
///
/// Seconds, at least `proper_time_s`. `None` if an input is not finite, `proper_time_s` is negative,
/// or the result overflows (`sinh` does so once `a·τ/c` passes about `710`).
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::rapidity::{
///     coordinate_time_under_proper_acceleration, proper_time_under_proper_acceleration,
/// };
///
/// let year = 31_557_600.0;
/// let earth = coordinate_time_under_proper_acceleration(9.806_65, 2.0 * year).unwrap();
/// assert!((earth / year - 3.76).abs() < 0.01, "two ship years at 1 g is 3.76 Earth years");
///
/// let ship = proper_time_under_proper_acceleration(9.806_65, earth).unwrap();
/// assert!((ship / year - 2.0).abs() < 1.0e-9);
/// ```
pub fn coordinate_time_under_proper_acceleration(
    proper_acceleration_m_s2: f64,
    proper_time_s: f64,
) -> Option<f64> {
    if !proper_acceleration_m_s2.is_finite() || !proper_time_s.is_finite() || proper_time_s < 0.0 {
        return None;
    }

    let phi = proper_acceleration_m_s2 * proper_time_s / SPEED_OF_LIGHT_M_S;

    let coordinate_time = if phi == 0.0 {
        proper_time_s
    } else {
        proper_time_s * (phi.sinh() / phi)
    };

    coordinate_time.is_finite().then_some(coordinate_time)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/rapidity_tests.rs"]
mod tests;
