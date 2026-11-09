//! Lambert's problem: the two-body orbit that goes from one point to another in a given time.
//!
//! The least-energy search asks it thousands of times (for every time of flight, the departure
//! and arrival velocity of the arc about the central body), which is why it is solved in closed
//! form here rather than flown. The full flight, with every planet pulling, is then solved from
//! the best arc's estimate by [`crate::core::transfer::solve_coast`].
//!
//! The method is the universal-variable formulation (Bate, Mueller & White, *Fundamentals of
//! Astrodynamics*, §5.3; Curtis, *Orbital Mechanics for Engineering Students*, Algorithm 5.2),
//! single revolution, solved for the universal variable `z` by bisection, which cannot fail to
//! converge on the single-revolution branch where the time of flight rises monotonically with `z`.

use bevy::math::DVec3;

// ALGORITHM CONSTANT: where the series for the Stumpff functions takes over from their closed
// forms, which lose all precision to cancellation as `z` approaches zero. Not tunable.
const STUMPFF_SERIES_BELOW: f64 = 1.0e-6;
// ALGORITHM CONSTANT: the most negative `z` (the most hyperbolic arc) tried. `cosh(√1e4) ≈ 1e43`
// is still far from overflow, and an arc that needs more is faster than any engine here can
// reach. Not tunable.
const Z_MIN: f64 = -1.0e4;
// ALGORITHM CONSTANT: bisection steps. Each halves the bracket, so 200 resolve `z` to the last bit
// of an `f64` over any bracket here. Not tunable.
const BISECTION_STEPS: usize = 200;

/// The Stumpff function `C(z)`.
fn stumpff_c(z: f64) -> f64 {
    if z.abs() < STUMPFF_SERIES_BELOW {
        0.5 - z / 24.0 + z * z / 720.0
    } else if z > 0.0 {
        (1.0 - z.sqrt().cos()) / z
    } else {
        ((-z).sqrt().cosh() - 1.0) / -z
    }
}

/// The Stumpff function `S(z)`.
fn stumpff_s(z: f64) -> f64 {
    if z.abs() < STUMPFF_SERIES_BELOW {
        1.0 / 6.0 - z / 120.0 + z * z / 5040.0
    } else if z > 0.0 {
        let s = z.sqrt();
        (s - s.sin()) / s.powi(3)
    } else {
        let s = (-z).sqrt();
        (s.sinh() - s) / s.powi(3)
    }
}

/// The velocities at both ends of the single-revolution arc about a body of `mu` (m³/s²) that goes
/// from `r1` to `r2` (metres, relative to that body) in `tof_s` seconds.
///
/// The arc goes round the body the way `normal` points (right-handed): the short way when the
/// short way agrees with it, the long way otherwise. Pass the orbital angular momentum of the
/// origin, and the arc is prograde.
///
/// `None` for a time of flight no single-revolution arc can make (too short even on a hyperbola),
/// or when the two points are in line with the body, where the plane of the arc is undefined.
pub fn lambert(r1: DVec3, r2: DVec3, tof_s: f64, mu: f64, normal: DVec3) -> Option<(DVec3, DVec3)> {
    let (n1, n2) = (r1.length(), r2.length());
    if !(tof_s > 0.0 && mu > 0.0 && n1 > 0.0 && n2 > 0.0) {
        return None;
    }
    let cos_dtheta = (r1.dot(r2) / (n1 * n2)).clamp(-1.0, 1.0);
    let short_way = r1.cross(r2).dot(normal) >= 0.0;
    let sin_dtheta = (1.0 - cos_dtheta * cos_dtheta).sqrt() * if short_way { 1.0 } else { -1.0 };
    if sin_dtheta.abs() < 1.0e-9 {
        return None;
    }
    let a = sin_dtheta * (n1 * n2 / (1.0 - cos_dtheta)).sqrt();

    let y = |z: f64| n1 + n2 + a * (z * stumpff_s(z) - 1.0) / stumpff_c(z).sqrt();
    // The time of flight of the arc with universal variable `z`, or `None` where `y < 0`, which no
    // arc reaches.
    let time_of = |z: f64| {
        let yz = y(z);
        (yz >= 0.0)
            .then(|| ((yz / stumpff_c(z)).powf(1.5) * stumpff_s(z) + a * yz.sqrt()) / mu.sqrt())
    };

    // Bisect on the single-revolution branch, `z` in [Z_MIN, 4π²), where the time of flight rises
    // with `z` and runs to infinity at the top.
    let (mut lo, mut hi) = (Z_MIN, 4.0 * std::f64::consts::PI.powi(2) - 1.0e-9);
    if time_of(lo).is_some_and(|t| t > tof_s) {
        return None;
    }
    for _ in 0..BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        match time_of(mid) {
            Some(t) if t > tof_s => hi = mid,
            _ => lo = mid,
        }
    }
    let z = 0.5 * (lo + hi);
    let yz = y(z);
    if yz.is_nan() || yz <= 0.0 {
        return None;
    }

    // Lagrange coefficients.
    let f = 1.0 - yz / n1;
    let g = a * (yz / mu).sqrt();
    let g_dot = 1.0 - yz / n2;
    if g.abs() < f64::MIN_POSITIVE {
        return None;
    }
    let v1 = (r2 - r1 * f) / g;
    let v2 = (r2 * g_dot - r1) / g;
    (v1.is_finite() && v2.is_finite()).then_some((v1, v2))
}

#[cfg(test)]
#[path = "../../tst/spacecraft/lambert_tests.rs"]
mod tests;
