//! The steering laws a powered craft flies by, recomputed before every substep from where it and
//! every body are now. Each is a feedback law, so gravity and drift are removed as they arise.
//!
//! # Landing (and the whole of a direct flight)
//!
//! [`steer`] wants the craft closing on the target at
//!
//! ```text
//! v_wanted = √( 2 · a_brake · s + v_touch² )
//! ```
//!
//! where `s` is the distance to the target's surface and `a_brake` the deceleration the craft can
//! count on: [`POWERED_BRAKE_FRACTION`] of what its engine gives beyond the target's surface
//! gravity. It is the speed from which braking at exactly `a_brake` all the way down reaches the
//! surface at `v_touch` ([`POWERED_TOUCHDOWN_SPEED_M_S`]), so a craft far away burns toward the
//! target, flips once it reaches that curve, and rides it down at a constant deceleration. With
//! `v_touch²` inside the root the curve stays finite in slope at the surface, so the last metres
//! are as easy for the substeps to follow as the rest, and nothing past the braking budget is ever
//! asked for.
//!
//! A direct flight lets the law speed the craft up, which is the burn toward the target. A
//! transfer's landing does not ([`Law::may_speed_up`]): the craft arrives on its coasting arc
//! already fast, and the law only steers it straight and brakes, never spending energy to go
//! faster.
//!
//! A body in the way is flown around ([`Law::avoid`]): while the straight line to the target passes
//! within [`POWERED_CLEARANCE_RADII`] of a body, the craft heads for a point beside it instead.
//!
//! # Swinging round instead of landing
//!
//! [`swing_by`] is not a feedback law but a single answer: the velocity a craft on a landing
//! approach should turn to so that it passes the body at a chosen distance, on the side that
//! sends it back toward where it came from. A return trip burns to it once, with the
//! velocity-matching burn below, and then coasts round.
//!
//! # Velocity-matching burns
//!
//! [`match_velocity`] burns toward a velocity, at up to the engine's acceleration: a transfer's
//! departure onto its arc, and its correction burns.

use bevy::math::DVec3;

use crate::spacecraft::constants::{
    POWERED_AIM_CLEARANCE_RADII, POWERED_BRAKE_FRACTION, POWERED_CLEARANCE_RADII,
    POWERED_MAX_SUBSTEP_S, POWERED_MIN_SUBSTEP_S, POWERED_SUBSTEP_FRACTION,
    POWERED_TOUCHDOWN_SPEED_M_S,
};

/// A body as the steering laws see it: where it is, how it moves, and how big it is. SI units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sighting {
    pub position_m: DVec3,
    pub velocity_m_s: DVec3,
    pub radius_m: f64,
}

/// What a steering law decided for one step.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Steering {
    /// The engine's acceleration for the step, in m/s². Never longer than the law allows.
    pub acceleration_m_s2: DVec3,
    /// The step to take, in seconds: at most what was offered.
    pub substep_s: f64,
}

/// How [`steer`] may fly the craft.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Law {
    /// The most acceleration the engine may give, in m/s².
    pub acceleration_m_s2: f64,
    /// The deceleration the law plans its braking curve with, in m/s²: see [`braking_m_s2`].
    pub braking_m_s2: f64,
    /// Whether the law may speed the craft up toward the target (a direct flight), or only steer
    /// and brake (a transfer's landing).
    pub may_speed_up: bool,
    /// Whether to fly around bodies in the way.
    pub avoid: bool,
}

/// The deceleration a craft with `acceleration_m_s2` of engine can plan to brake with when landing
/// on a body whose surface gravity is `surface_gravity_m_s2`: [`POWERED_BRAKE_FRACTION`] of what
/// is left after holding against that gravity. Zero or less means it cannot land softly there.
pub fn braking_m_s2(acceleration_m_s2: f64, surface_gravity_m_s2: f64) -> f64 {
    POWERED_BRAKE_FRACTION * (acceleration_m_s2 - surface_gravity_m_s2)
}

/// The landing law (see the module docs): the thrust for the next step, and how long that step
/// should be.
///
/// `obstacles` are every other body the craft must not hit. `offered_s` is the longest step the
/// caller will take; the answer may be shorter, since the approach to a surface needs steps far
/// finer than gravity does.
pub fn steer(
    craft_position_m: DVec3,
    craft_velocity_m_s: DVec3,
    target: &Sighting,
    obstacles: &[Sighting],
    law: &Law,
    offered_s: f64,
) -> Steering {
    let to_target = target.position_m - craft_position_m;
    let distance = to_target.length();
    let surface_m = (distance - target.radius_m).max(0.0);
    let relative_velocity = craft_velocity_m_s - target.velocity_m_s;
    let relative_speed = relative_velocity.length();

    let curve_m_s = (2.0 * law.braking_m_s2.max(0.0) * surface_m
        + POWERED_TOUCHDOWN_SPEED_M_S * POWERED_TOUCHDOWN_SPEED_M_S)
        .sqrt();
    let closing_m_s = if law.may_speed_up {
        curve_m_s
    } else {
        curve_m_s.min(relative_speed.max(POWERED_TOUCHDOWN_SPEED_M_S))
    };
    let blocked = if law.avoid {
        blocking(craft_position_m, target.position_m, obstacles)
    } else {
        None
    };
    let heading = match blocked {
        Some(aim) => (aim - craft_position_m).normalize_or_zero(),
        None => to_target / distance.max(f64::MIN_POSITIVE),
    };
    let wanted = heading * closing_m_s;

    // Shorter steps the sooner the craft would reach the surface at its present speed, so the
    // approach, where speeds fall from thousands of km/s to metres per second, is resolved.
    // The floor shrinks for a strong engine: the curve tightens over its last metres on the scale
    // `v_touch / a_brake`, seven milliseconds at full thrust, which a fixed floor would overshoot.
    let time_to_surface = surface_m / relative_speed.max(POWERED_TOUCHDOWN_SPEED_M_S);
    let floor_s = POWERED_MIN_SUBSTEP_S.min(
        POWERED_SUBSTEP_FRACTION * POWERED_TOUCHDOWN_SPEED_M_S
            / law.braking_m_s2.max(f64::MIN_POSITIVE),
    );
    let substep_s = (POWERED_SUBSTEP_FRACTION * time_to_surface)
        .clamp(floor_s, POWERED_MAX_SUBSTEP_S)
        .min(offered_s);
    Steering {
        acceleration_m_s2: deadbeat(wanted - relative_velocity, law.acceleration_m_s2, substep_s),
        substep_s,
    }
}

/// Burn toward `goal_velocity_m_s` at up to `acceleration_m_s2`, against `gravity_m_s2`: the thrust
/// for the next step, and how long it should be (a [`POWERED_SUBSTEP_FRACTION`] of the burn left,
/// at most `offered_s`).
///
/// The gravity is cancelled over the step as well, so a burn ends even where a step's worth of
/// gravity is more than [`BURN_DONE_M_S`](crate::spacecraft::constants::BURN_DONE_M_S): at the
/// smallest step, Jupiter's `24.8 m/s²` would otherwise reopen a quarter of a m/s every step.
pub fn match_velocity(
    velocity_m_s: DVec3,
    goal_velocity_m_s: DVec3,
    gravity_m_s2: DVec3,
    acceleration_m_s2: f64,
    offered_s: f64,
) -> Steering {
    let error = goal_velocity_m_s - velocity_m_s;
    let burn_left_s = error.length() / acceleration_m_s2.max(f64::MIN_POSITIVE);
    let substep_s = (POWERED_SUBSTEP_FRACTION * burn_left_s)
        .clamp(POWERED_MIN_SUBSTEP_S, POWERED_MAX_SUBSTEP_S)
        .min(offered_s);
    Steering {
        acceleration_m_s2: deadbeat(
            error - gravity_m_s2 * substep_s,
            acceleration_m_s2,
            substep_s,
        ),
        substep_s,
    }
}

/// The velocity to turn to, relative to a body, so that a craft heading for it swings round it at
/// `periapsis_m` from its centre instead of hitting it, on the side that sends it back toward
/// home.
///
/// Everything is relative to the body: the craft at `relative_m` moving at
/// `relative_velocity_m_s`, home at `home_m`, and `body_velocity_m_s` the body's own velocity
/// relative to home. `mu` is the body's `G · m`. The speed is kept and only the heading changes,
/// so the turn costs the least it can.
///
/// The heading follows from two-body motion about the body. Energy gives the speed at closest
/// approach, `v_p² = v² − 2μ/d + 2μ/r_p`; angular momentum is the same there as here,
/// `r_p · v_p = d · v · sin φ`; so `φ` is the angle to hold between the velocity and the line to
/// the centre.
///
/// The craft can pass on either side. A pass bends its path toward the body by
/// `δ = 2 · asin(1 / e)`, with `e = 1 + r_p · v∞² / μ`, so each side leaves in a different
/// direction, and the side chosen is the one whose departure, seen from home, has the most speed
/// toward home. Round the Moon that is the far side, the loop a free-return flight makes.
///
/// Returns the velocity unchanged for a craft already inside `periapsis_m`, or at rest.
pub fn swing_by(
    relative_m: DVec3,
    relative_velocity_m_s: DVec3,
    mu: f64,
    periapsis_m: f64,
    home_m: DVec3,
    body_velocity_m_s: DVec3,
) -> DVec3 {
    let (distance, speed) = (relative_m.length(), relative_velocity_m_s.length());
    if !(distance > periapsis_m && periapsis_m > 0.0 && speed > 0.0) {
        return relative_velocity_m_s;
    }
    let inward = -relative_m / distance;
    let pass_speed_squared = speed * speed - 2.0 * mu / distance + 2.0 * mu / periapsis_m;
    let sin_heading =
        (periapsis_m * pass_speed_squared.max(0.0).sqrt() / (distance * speed)).min(1.0);
    let cos_heading = (1.0 - sin_heading * sin_heading).sqrt();

    // The two sides lie in the plane through the body, the craft and home; failing that (home
    // dead ahead or behind), in the plane of the body's own motion.
    // A direction counts as off the line of approach when at least a millionth of it is.
    let across = |v: DVec3| {
        let off = v - inward * v.dot(inward);
        (off.length() > 1e-6 * v.length()).then(|| off.normalize())
    };
    let side = across(home_m)
        .or_else(|| across(body_velocity_m_s))
        .unwrap_or_else(|| inward.any_orthonormal_vector());

    // How far the pass turns the path, and so which way the craft leaves on each side.
    let excess_squared = speed * speed - 2.0 * mu / distance;
    let (turn, excess) = if excess_squared > 0.0 {
        let eccentricity = 1.0 + periapsis_m * excess_squared / mu.max(f64::MIN_POSITIVE);
        (2.0 * (1.0 / eccentricity).asin(), excess_squared.sqrt())
    } else {
        // Bound to the body: it comes back the way it went in.
        (std::f64::consts::PI, 0.0)
    };
    let home_direction = home_m.normalize_or_zero();
    let candidate = |sign: f64| {
        let side = side * sign;
        let heading = inward * cos_heading + side * sin_heading;
        // Passing on `side` puts the body on the other side of the path, so the path bends away
        // from `side`.
        let toward = (side - heading * side.dot(heading)).normalize_or_zero();
        let leaving = (heading * turn.cos() - toward * turn.sin()) * excess;
        let score = (leaving + body_velocity_m_s).dot(home_direction);
        (heading * speed, score)
    };
    let (plus, minus) = (candidate(1.0), candidate(-1.0));
    if minus.1 > plus.1 {
        minus.0
    } else {
        plus.0
    }
}

/// The acceleration that removes `error` (m/s) in one step of `substep_s`, capped at `limit_m_s2`.
fn deadbeat(error: DVec3, limit_m_s2: f64, substep_s: f64) -> DVec3 {
    let mut command = error / substep_s.max(f64::MIN_POSITIVE);
    let magnitude = command.length();
    if magnitude > limit_m_s2 {
        command *= limit_m_s2 / magnitude;
    }
    if command.is_finite() {
        command
    } else {
        DVec3::ZERO
    }
}

/// Where to head instead of the target, if a body sits in the way: a point beside the nearest
/// body that the straight line from `from` to `to` passes within [`POWERED_CLEARANCE_RADII`] of,
/// [`POWERED_AIM_CLEARANCE_RADII`] from its centre on the side the line already passes. `None` when
/// the way is clear.
pub fn blocking(from: DVec3, to: DVec3, obstacles: &[Sighting]) -> Option<DVec3> {
    let line = to - from;
    let length_squared = line.length_squared();
    if length_squared <= 0.0 {
        return None;
    }
    obstacles
        .iter()
        .filter(|body| body.radius_m > 0.0)
        .filter_map(|body| {
            let along = (body.position_m - from).dot(line) / length_squared;
            if along <= 0.0 || along >= 1.0 {
                return None;
            }
            let closest = from + line * along;
            let offset = closest - body.position_m;
            (offset.length() < POWERED_CLEARANCE_RADII * body.radius_m).then(|| {
                let side = if offset.length_squared() > 0.0 {
                    offset.normalize()
                } else {
                    line.any_orthonormal_vector()
                };
                (
                    along,
                    body.position_m + side * POWERED_AIM_CLEARANCE_RADII * body.radius_m,
                )
            })
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, aim)| aim)
}

#[cfg(test)]
#[path = "../../tst/spacecraft/guidance_tests.rs"]
mod tests;
