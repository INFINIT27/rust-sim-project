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
