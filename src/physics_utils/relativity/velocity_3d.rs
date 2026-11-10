//! Velocities in three dimensions: the general transformation, the Wigner rotation, and aberration.
//!
//! [`velocity`](super::velocity) handles an object moving along the same axis as the frame. This
//! module handles any direction. With `S'` moving at `v` as measured in `S`, and `γ` the Lorentz
//! factor of `v`:
//!
//! ```text
//!           1         ⎡  u            γ    (u·v)   ⎤
//! u' = ─────────── · ⎢ ─── − v + ─────── · ───── v ⎥
//!       1 − u·v/c²    ⎣  γ          γ + 1    c²    ⎦
//! ```
//!
//! The part of `u` along `v` transforms as in one dimension. The part across it is divided by
//! `γ(1 − u·v/c²)`: it changes even though the frame has no motion in that direction, because the
//! two frames disagree about time.
//!
//! # Composition in 3-D does not commute
//!
//! On one axis `u ⊕ v = v ⊕ u`. In general the two have the **same speed but different
//! directions**. The angle between them is the **Wigner rotation**: two boosts in different
//! directions are not one boost, they are one boost and a rotation. [`wigner_rotation_angle`] returns
//! that angle. For an orbiting body the rotations accumulate into Thomas precession.
//!
//! This is why the functions in [`velocity`](super::velocity) must not be applied per component.
//!
//! # Aberration
//!
//! A light ray's direction also depends on the frame: [`aberration_angle`]. Moving towards a
//! source of light tilts its apparent direction forwards, the way vertical rain slants for a moving
//! car. For Earth's orbital speed this is `20.5″`, which Bradley measured in 1727.
//!
//! A velocity here is a `[f64; 3]` in metres per second.

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::lorentz_factor;

/// A vector's length, without forming a square that could overflow.
fn norm(vector: [f64; 3]) -> f64 {
    vector[0].hypot(vector[1]).hypot(vector[2])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn is_finite(vector: [f64; 3]) -> bool {
    vector.iter().all(|component| component.is_finite())
}

/// The speed of a 3-D velocity, in metres per second.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::velocity_3d::speed;
///
/// assert_eq!(speed([3.0, 4.0, 0.0]), 5.0);
/// ```
pub fn speed(velocity_m_s: [f64; 3]) -> f64 {
    norm(velocity_m_s)
}

/// The velocity of an object as measured in `S'`, given its velocity in `S`, for a frame moving in
/// any direction.
///
/// The 3-D form of [`transform_velocity`](super::velocity::transform_velocity), with the same domain
/// rule: the object may move at up to and including `c`, the frame must be strictly slower.
///
/// # Returns
///
/// `None` if the frame's speed is not below `c`, the object's speed exceeds `c`, or a component is
/// not finite.
///
/// An object given at exactly `c` comes back at `c` to within a few parts in `1e12`, not exactly;
/// see [`velocity`](super::velocity) for the measurement.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::velocity_3d::transform_velocity_3d;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // An object moving along y at 0.5c, seen from a frame moving along x at 0.6c (gamma = 1.25).
/// // It recedes along x at 0.6c, and its y velocity is divided by gamma: 0.4c.
/// let u = transform_velocity_3d([0.0, 0.5 * c, 0.0], [0.6 * c, 0.0, 0.0]).unwrap();
/// assert!((u[0] / c + 0.6).abs() < 1.0e-12);
/// assert!((u[1] / c - 0.4).abs() < 1.0e-12);
/// assert_eq!(u[2], 0.0);
/// ```
pub fn transform_velocity_3d(
    object_velocity_m_s: [f64; 3],
    frame_velocity_m_s: [f64; 3],
) -> Option<[f64; 3]> {
    if !is_finite(object_velocity_m_s) || !is_finite(frame_velocity_m_s) {
        return None;
    }

    // Work in units of `c` throughout, so every intermediate value is of order one.
    let u = object_velocity_m_s.map(|component| component / SPEED_OF_LIGHT_M_S);
    let v = frame_velocity_m_s.map(|component| component / SPEED_OF_LIGHT_M_S);

    // Compared in metres per second, like the one-axis check, so a caller's own `speed(u) <= c`
    // test and this one cannot disagree by a rounding.
    if norm(object_velocity_m_s) > SPEED_OF_LIGHT_M_S {
        return None;
    }

    let gamma = lorentz_factor(norm(frame_velocity_m_s))?;
    let along = dot(u, v);
    let denominator = 1.0 - along;

    if denominator <= 0.0 {
        return None;
    }

    let parallel_weight = gamma / (gamma + 1.0) * along - 1.0;
    let mut result = [0.0; 3];

    for axis in 0..3 {
        let beta = (u[axis] / gamma + parallel_weight * v[axis]) / denominator;
        result[axis] = beta * SPEED_OF_LIGHT_M_S;
    }

    is_finite(result).then_some(result)
}

/// The object's velocity in `S`, given its velocity in `S'`: the relativistic sum `u' ⊕ v`.
///
/// The inverse of [`transform_velocity_3d`], by reversing the frame. **The order of the arguments
/// matters**: swapping them gives a vector of the same length pointing somewhere else, unless the two
/// are parallel. See the module docs.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::velocity_3d::{compose_velocities_3d, speed};
///
/// let c = SPEED_OF_LIGHT_M_S;
/// let along_x = [0.6 * c, 0.0, 0.0];
/// let along_y = [0.0, 0.6 * c, 0.0];
///
/// let one_way = compose_velocities_3d(along_y, along_x).unwrap();
/// let other_way = compose_velocities_3d(along_x, along_y).unwrap();
///
/// // The same speed (0.768c, not the 0.849c of adding at right angles)...
/// assert!(((speed(one_way) / c).powi(2) - 0.5904).abs() < 1.0e-12);
/// assert!((speed(one_way) - speed(other_way)).abs() < 1.0e-6);
///
/// // ...in different directions.
/// assert!((one_way[0] - other_way[0]).abs() > 0.1 * c);
/// ```
pub fn compose_velocities_3d(
    object_velocity_in_moving_frame_m_s: [f64; 3],
    frame_velocity_m_s: [f64; 3],
) -> Option<[f64; 3]> {
    transform_velocity_3d(
        object_velocity_in_moving_frame_m_s,
        frame_velocity_m_s.map(|component| -component),
    )
}

/// The Wigner rotation angle for two boosts: the angle between `a ⊕ b` and `b ⊕ a`.
///
/// Zero when the two velocities are parallel or either is zero, and it approaches the angle between
/// the two velocities as both approach `c`. For two perpendicular velocities it has the closed form
/// `tan ε = γ₁γ₂β₁β₂/(γ₁ + γ₂)`, which the tests check this against.
///
/// At everyday speeds it is about `β₁β₂/2` for perpendicular velocities: `5e-9` radians for two
/// boosts at Earth's orbital speed.
///
/// # Returns
///
/// Radians in `[0, π]`. `None` if either speed is not strictly below `c` or a component is not
/// finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::velocity_3d::wigner_rotation_angle;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Parallel boosts are one boost: no rotation.
/// let angle = wigner_rotation_angle([0.6 * c, 0.0, 0.0], [0.3 * c, 0.0, 0.0]).unwrap();
/// assert!(angle.abs() < 1.0e-12);
///
/// // Perpendicular boosts at 0.6c each: tan = (1.25 * 0.6)^2 / 2.5, which is 12.7 degrees.
/// let angle = wigner_rotation_angle([0.6 * c, 0.0, 0.0], [0.0, 0.6 * c, 0.0]).unwrap();
/// assert!((angle.tan() - 0.225).abs() < 1.0e-12);
/// ```
pub fn wigner_rotation_angle(
    first_velocity_m_s: [f64; 3],
    second_velocity_m_s: [f64; 3],
) -> Option<f64> {
    // A frame must be strictly subluminal, and here both velocities take a turn as the frame.
    lorentz_factor(norm(first_velocity_m_s))?;
    lorentz_factor(norm(second_velocity_m_s))?;

    let one_way = compose_velocities_3d(first_velocity_m_s, second_velocity_m_s)?;
    let other_way = compose_velocities_3d(second_velocity_m_s, first_velocity_m_s)?;

    // In units of `c`, so the products below are of order one.
    let a = one_way.map(|component| component / SPEED_OF_LIGHT_M_S);
    let b = other_way.map(|component| component / SPEED_OF_LIGHT_M_S);

    // `atan2(|a×b|, a·b)` rather than `acos`: accurate for a small angle, where `acos` is not.
    let angle = norm(cross(a, b)).atan2(dot(a, b));

    angle.is_finite().then_some(angle)
}

/// The direction of a light ray in `S'`, given its direction in `S`: `cos θ' = (cos θ − β)/(1 − β·cos θ)`.
///
/// Both angles are between the ray's **direction of travel** and the frame's velocity. For a frame moving
/// in the positive direction the result is at least `angle_rad`: in the moving frame every ray is swung towards the direction opposite
/// to the frame's motion, which means the *sources* of the light appear crowded towards the
/// direction the observer is heading.
///
/// # Parameters
///
/// - `angle_rad`: in `[0, π]`, measured in `S`.
/// - `frame_velocity_m_s`: the speed of `S'` along the reference direction. Signed; `|v| < c`.
///
/// # Returns
///
/// Radians in `[0, π]`. `None` if the angle is outside `[0, π]`, the frame is not strictly
/// subluminal, or an input is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::velocity_3d::aberration_angle;
/// use std::f64::consts::PI;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Light crossing at right angles in S. From a frame moving at 0.6c it travels at 126.87 degrees
/// // to the motion: cos = -0.6.
/// let angle = aberration_angle(PI / 2.0, 0.6 * c).unwrap();
/// assert!((angle.cos() + 0.6).abs() < 1.0e-12);
///
/// // A ray along the motion stays along it.
/// assert_eq!(aberration_angle(0.0, 0.6 * c), Some(0.0));
/// ```
pub fn aberration_angle(angle_rad: f64, frame_velocity_m_s: f64) -> Option<f64> {
    if !angle_rad.is_finite() || !(0.0..=std::f64::consts::PI).contains(&angle_rad) {
        return None;
    }

    let gamma = lorentz_factor(frame_velocity_m_s)?;
    let beta = frame_velocity_m_s / SPEED_OF_LIGHT_M_S;

    // `atan2(sin θ/γ, cos θ − β)`: the two components of the transformed ray, up to a common
    // positive factor. Accurate near `0` and `π`, where an `acos` of the quotient loses digits.
    let angle = (angle_rad.sin() / gamma).atan2(angle_rad.cos() - beta);

    angle.is_finite().then_some(angle)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/velocity_3d_tests.rs"]
mod tests;
