//! Transfer planning: the path a craft takes from one body to another under the scene's own
//! gravity.
//!
//! Nothing here is a conic or a patched-conic approximation. A plan is found by **flying** the
//! craft through the scene's future:
//!
//! 1. Clone the scene, leave the craft out, and step the clone forward with the same integrator,
//!    softening, and substepping [`Scene::step`] uses, recording every body's position and velocity
//!    at [`TRANSFER_EPHEMERIS_SAMPLES`] evenly spaced times. That is the *ephemeris*. Contacts are
//!    not predicted.
//! 2. Fly the craft as a test particle through that ephemeris, pulled by every body in it, and
//!    record how close it comes to the target and when.
//! 3. Adjust the launch (or the correction burn) with Newton's method until the closest approach
//!    passes within [`TRANSFER_AIM_TOLERANCE_M`] of the target's centre.
//!
//! # Why a test particle is honest
//!
//! The craft weighs hundreds of kilograms and the lightest body it flies past is the Moon, at
//! `7.3e22 kg`. Its pull on any body is twenty orders of magnitude below that body's own
//! dynamics and below `f32` resolution, so leaving it out of the ephemeris changes nothing the
//! scene can represent. The craft itself is pulled by everything.
//!
//! # Why the live flight still needs corrections
//!
//! The planner integrates the craft in `f64` with steps sized to its distance from the nearest
//! body; the live scene integrates it in `f32` with whatever substep the frame gets. Over a
//! two-month flight those differ by thousands of kilometres, which is comparable to Mars. Real
//! missions have the same problem for different reasons and solve it the same way:
//! [`plan_correction`] re-aims from wherever the craft actually is.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bevy::math::DVec3;

use crate::constants::METRES_PER_UNIT;
use crate::core::constants::{
    COAST_SOLVE_MAX_STEP_M_S, COAST_SOLVE_PROBE_M_S, TRANSFER_ADAPTIVE_PROBE_FRACTION,
    TRANSFER_ADAPTIVE_STEP_FRACTION, TRANSFER_AIM_TOLERANCE_M, TRANSFER_ALONG_TRACK_FRACTION,
    TRANSFER_ASSIST_ONLY_HORIZON_S, TRANSFER_AZIMUTH_SAMPLES, TRANSFER_CANDIDATES,
    TRANSFER_CORRECTION_FRACTIONS, TRANSFER_CORRECTION_MAX_MISS_RADII,
    TRANSFER_EPHEMERIS_ORBIT_FRACTION, TRANSFER_EPHEMERIS_SAMPLES,
    TRANSFER_FIRST_CORRECTION_FRACTION, TRANSFER_FIRST_CORRECTION_S, TRANSFER_FLYBY_AIM_FRACTION,
    TRANSFER_FLYBY_ANGLES, TRANSFER_FLYBY_BAND_MARGIN, TRANSFER_FLYBY_CANDIDATES,
    TRANSFER_FLYBY_CORRECTION_AFTER_S, TRANSFER_FLYBY_CORRECTION_LEADS_S,
    TRANSFER_FLYBY_FIRST_PROBE_RAD, TRANSFER_FLYBY_FIRST_STEP_RAD, TRANSFER_FLYBY_LEAD_FRACTION,
    TRANSFER_FLYBY_MAX_TURN_RAD, TRANSFER_FLYBY_PASSED_FACTOR, TRANSFER_FLYBY_RADII,
    TRANSFER_FLYBY_SCAN_HILL_RADII, TRANSFER_FLYBY_SEEDS, TRANSFER_FLYBY_WINDOW_FRACTION,
    TRANSFER_HORIZON_FACTOR, TRANSFER_LINE_SEARCH_HALVINGS, TRANSFER_MAX_FLYBYS,
    TRANSFER_MAX_ITERATIONS, TRANSFER_MIN_EPHEMERIS_STEP_S, TRANSFER_MIN_HORIZON_S,
    TRANSFER_MIN_STEP_S, TRANSFER_PATH_POINTS, TRANSFER_REACH_MARGIN, TRANSFER_SPEED_FRACTIONS,
    TRANSFER_SPEED_REFINE_FRACTION, TRANSFER_SPEED_REFINE_ROUNDS, TRANSFER_STEP_FRACTION,
};
use crate::core::integrator::integrate_gravity;
use crate::core::scene::{moon_primary, Body, BodyId, Pose, Prop, Quat, Scene, Vec3};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::time::simulation::TimeStep;

mod assist;
mod departure;
mod ephemeris;
mod flight;
mod forecast;
mod voyage;

use self::assist::*;
use self::departure::*;
pub use self::ephemeris::PlanProgress;
use self::ephemeris::*;
use self::flight::*;
pub use self::forecast::{solve_coast, solve_coast_with, Coast, Forecast};
#[cfg(test)]
use self::voyage::segment_entry;
pub use self::voyage::Voyage;

/// A solved trajectory: where the craft starts, how fast, and where it will go.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferPlan {
    /// Where the craft starts, in world units.
    pub position_units: Vec3,
    /// The craft's velocity in the scene's frame, in units per second.
    pub velocity_units_s: Vec3,
    /// The predicted path from the start to the closest approach, in world units.
    pub path_units: Vec<Vec3>,
    /// Seconds from the start to the closest approach.
    pub flight_seconds: f64,
    /// Predicted distance from the target's centre at closest approach, in metres.
    pub miss_m: f64,
    /// For a departure, the speed the planner chose to leave at: relative to the origin once clear
    /// of its gravity, at most the craft's cap. `0.0` for a correction.
    pub departure_speed_m_s: f64,
    /// Every planet the path passes close to on the way, in order: aimed for, or met by chance.
    pub flybys: Vec<Flyby>,
    /// The fastest the craft goes on the way, in the scene's frame, in m/s. For a powered flight
    /// this can be a sizeable fraction of light speed, where the Newtonian model this simulation
    /// uses is measurably wrong.
    pub peak_speed_m_s: f64,
}

/// A close pass of a body on the way to the destination: inside its Hill sphere, where its gravity
/// bends the path. Aimed for, this is a gravity assist.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Flyby {
    /// The body passed.
    pub body: BodyId,
    /// Seconds from the start of the plan to the closest point of the pass.
    pub time_s: f64,
    /// Distance from the body's centre at the closest point, in metres.
    pub closest_m: f64,
    /// The craft's speed about the system's dominant body after the pass, less before it, in m/s.
    /// Positive is a boost.
    pub speed_change_m_s: f64,
    /// Craft minus body at the closest point, in metres: where beside the body the craft passes,
    /// which is what a correction burn re-aims for before the flyby.
    pub offset_m: DVec3,
    /// Craft minus body velocity at the closest point, in m/s.
    pub relative_velocity_m_s: DVec3,
}

/// Plan the fastest launch from the surface of `origin` to `target`, leaving at any speed up to
/// `max_departure_speed_m_s` relative to `origin` once clear of its gravity.
///
/// Any two bodies will do: a planet and its moon, two of Jupiter's moons, Earth and Neptune, or an
/// object placed with `B`. The craft starts `launch_altitude_m` above the origin's surface and
/// leaves radially, so the free choices are the launch **direction** and **speed**.
///
/// # How the search goes
///
/// For each speed in [`TRANSFER_SPEED_FRACTIONS`] of the cap, directions are scanned around the
/// plane of the two bodies' relative motion, which contains the target and, in a coplanar system,
/// is the system's own plane. Each dip in the closest-approach distance is refined in both angles
/// with Newton's method until it passes through the target's centre. The earliest arrival over all
/// speeds is then improved by a short hill climb on speed. The chosen speed is on the plan as
/// [`TransferPlan::departure_speed_m_s`].
///
/// How far ahead to look depends entirely on the pair: hours for the Moon, decades for Neptune.
/// The search starts at [`TRANSFER_HORIZON_FACTOR`] times the straight-line time at the cap and
/// **doubles the horizon until a path turns up**, with no upper limit. The first horizon that holds
/// any path holds the fastest one, since every faster path would fit in it too.
///
/// Because a target may be unreachable under the cap, the search can run forever. `progress` shows
/// how far out it has got and is how a caller stops it; every flight and every Newton iteration
/// checks it.
///
/// # When it does not search at all
///
/// When both bodies orbit the same primary, a two-body energy bound is checked first: the farthest
/// from that primary the craft can coast at the cap (see [`coasting_reach`]) against the closest the
/// target's orbit ever comes. If the target is beyond reach, no direct path exists at any horizon
/// and the search would run forever, so [`NoTransfer::OutOfReach`] comes back at once. Mercury to
/// Jupiter at `15.4 km/s` is the case that found this: a craft leaving Mercury coasts out to `3.05 AU`,
/// and Jupiter is never inside `5.2`.
pub fn plan_departure(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    max_departure_speed_m_s: f64,
    launch_altitude_m: f64,
    progress: &PlanProgress,
) -> Result<TransferPlan, NoTransfer> {
    if origin == target || max_departure_speed_m_s <= 0.0 {
        return Err(NoTransfer::InvalidRoute);
    }
    let origin_prop = scene.get(origin).ok_or(NoTransfer::InvalidRoute)?;
    let target_prop = scene.get(target).ok_or(NoTransfer::InvalidRoute)?;
    if physical_radius_m(origin_prop) <= 0.0 {
        return Err(NoTransfer::InvalidRoute);
    }
    // Direct paths are bounded by the craft's energy; gravity assists can add to it, and are tried
    // whenever origin and target orbit the same primary, which is when there are siblings to pass.
    let reach = coasting_reach(scene, origin, target, max_departure_speed_m_s);
    let direct_possible =
        reach.is_none_or(|reach| reach.target_closest_m <= TRANSFER_REACH_MARGIN * reach.reach_m);
    let assists_possible = TRANSFER_MAX_FLYBYS > 0 && reach.is_some();
    if !direct_possible && !assists_possible {
        return Err(NoTransfer::OutOfReach(
            reach.ok_or(NoTransfer::InvalidRoute)?,
        ));
    }
    let separation = to_metres(target_prop.body.center() - origin_prop.body.center()).length();

    let mut horizon = (TRANSFER_HORIZON_FACTOR * separation / max_departure_speed_m_s)
        .max(TRANSFER_MIN_HORIZON_S);
    loop {
        if progress.is_cancelled() || !horizon.is_finite() {
            return Err(NoTransfer::Cancelled);
        }
        // With no direct path the only hope is an assist, and an assist search has no energy bound
        // to say when to give up, so it stops at a horizon instead. A direct search needs no such
        // limit: `coasting_reach` already ruled out the targets it could chase forever.
        if !direct_possible && horizon > TRANSFER_ASSIST_ONLY_HORIZON_S {
            return Err(NoTransfer::OutOfReach(
                reach.ok_or(NoTransfer::InvalidRoute)?,
            ));
        }
        progress.set_horizon_s(horizon);
        let direct = direct_possible
            .then(|| {
                departure_within(
                    scene,
                    origin,
                    target,
                    max_departure_speed_m_s,
                    launch_altitude_m,
                    horizon,
                    progress,
                )
            })
            .flatten();
        // A route with flybys has to beat the direct one to be worth flying, so its flights stop
        // at the direct arrival.
        let cutoff = direct.as_ref().map_or(horizon, |plan| plan.flight_seconds);
        let assisted = assists_possible
            .then(|| {
                assisted_departure(
                    scene,
                    origin,
                    target,
                    max_departure_speed_m_s,
                    launch_altitude_m,
                    horizon,
                    cutoff,
                    progress,
                )
            })
            .flatten();
        let fastest = [direct, assisted]
            .into_iter()
            .flatten()
            .min_by(|a, b| a.flight_seconds.total_cmp(&b.flight_seconds));
        if let Some(plan) = fastest {
            return Ok(plan);
        }
        horizon *= 2.0;
    }
}

/// The farthest from a body of gravitational parameter `mu` that something at distance `r` moving at
/// `speed` can coast: `−μ/E`, twice its semi-major axis, or infinity once `E ≥ 0`.
pub(super) fn coasting_limit_m(r: f64, speed: f64, mu: f64) -> f64 {
    let energy = 0.5 * speed * speed - mu / r;
    if energy >= 0.0 {
        f64::INFINITY
    } else {
        -mu / energy
    }
}

/// The closest an orbit with position `r` and velocity `v` comes to a body of parameter `mu`.
pub(super) fn periapsis_m(r: DVec3, v: DVec3, mu: f64) -> f64 {
    let energy = 0.5 * v.length_squared() - mu / r.length();
    let h = r.cross(v).length();
    let eccentricity = (1.0 + 2.0 * energy * h * h / (mu * mu)).max(0.0).sqrt();
    h * h / (mu * (1.0 + eccentricity))
}

/// Why [`plan_departure`] returned no plan.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NoTransfer {
    /// The bodies are the same, one is missing, the origin has no surface, or the cap is not
    /// positive.
    InvalidRoute,
    /// The search was cancelled through its [`PlanProgress`].
    Cancelled,
    /// No direct path exists at this speed, by the energy bound in [`coasting_reach`].
    OutOfReach(Reach),
}

/// How far a craft can coast from its origin compared with where the target can be, about the
/// primary both orbit. Distances in metres, speeds in m/s.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Reach {
    /// The farthest the craft can get from the primary at the cap: `−μ/E` at the most energetic
    /// departure, or infinity once that departure escapes.
    pub reach_m: f64,
    /// The closest the target's orbit comes to the primary.
    pub target_closest_m: f64,
    /// About the departure speed that would reach that closest point, for a tangential launch from
    /// a circular-ish origin orbit.
    pub needed_speed_m_s: f64,
}

/// The two-body energy bound on how far from a common primary a craft can coast, or `None` when the
/// origin and target do not orbit the same primary.
///
/// The most energetic departure adds the whole cap to the origin's own orbital velocity. Its
/// specific energy `E = v²/2 − μ/r` fixes the semi-major axis, and no orbit with that energy reaches
/// past `2a = −μ/E` from the primary, whatever its direction. This is exact for two bodies; other
/// planets can change the craft's energy only by flying close to them, which is a gravity assist
/// and which [`plan_departure`] does not aim for. [`TRANSFER_REACH_MARGIN`] covers the incidental
/// perturbations.
///
/// Only the outward bound is checked. Falling inward to a small orbit is limited by angular
/// momentum, not energy, and every inward pair in the solar system is within the cap.
pub fn coasting_reach(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    max_departure_speed_m_s: f64,
) -> Option<Reach> {
    let primary_of = |id| moon_primary(scene, id).map(|(primary, _)| primary);
    let primary = match (primary_of(origin), primary_of(target)) {
        (Some(a), Some(b)) if a == b => a,
        (None, None) => scene
            .iter()
            .filter(|(_, prop)| !prop.immovable)
            .max_by(|(_, a), (_, b)| a.mass_kg.total_cmp(&b.mass_kg))
            .map(|(id, _)| id)?,
        _ => return None,
    };
    if primary == origin || primary == target {
        return None;
    }
    let primary_prop = scene.get(primary)?;
    let mu = GRAVITATIONAL_CONSTANT * f64::from(primary_prop.mass_kg);
    let relative = |id: BodyId| {
        let prop = scene.get(id)?;
        Some((
            to_metres(prop.body.center() - primary_prop.body.center()),
            to_metres(prop.velocity_units_s - primary_prop.velocity_units_s),
        ))
    };
    let (r_origin, v_origin) = relative(origin)?;
    let (r_target, v_target) = relative(target)?;

    let r = r_origin.length();
    let reach_m = coasting_limit_m(r, v_origin.length() + max_departure_speed_m_s, mu);
    let target_closest_m = periapsis_m(r_target, v_target, mu);

    // Vis-viva for the ellipse from `r` out to the target's periapsis, less the origin's speed.
    let needed_speed_m_s = if target_closest_m > r {
        let a = 0.5 * (r + target_closest_m);
        (mu * (2.0 / r - 1.0 / a)).sqrt() - v_origin.length()
    } else {
        0.0
    };

    Some(Reach {
        reach_m,
        target_closest_m,
        needed_speed_m_s,
    })
}

/// Re-aim a craft already in flight: the smallest velocity change that puts its closest approach
/// to `target` through the target's centre, with the arrival expected about `expected_arrival_s`
/// seconds from now.
///
/// With `via`, a flyby still ahead (its time counted from now), the burn aims for that flyby's
/// planned point beside the planet instead. Aiming straight at the destination from before a flyby
/// finds the cheapest burn that hits it, and that is often one that skips the flyby, cancelling the
/// gravity assist the route was planned around. Once the flyby is behind, corrections aim at the
/// destination again.
///
/// Via a flyby, the pass must also be the planned one, approaching from within
/// [`TRANSFER_FLYBY_MAX_TURN_RAD`] of the planned direction (see `AimPoint::hold_direction`).
///
/// The returned plan's `velocity_units_s` is the craft's **new** velocity; the burn is its
/// difference from the current one. `None` when the craft or target is missing, no correction
/// converges, or the one that converges no longer reaches the destination: its predicted miss is
/// more than [`TRANSFER_CORRECTION_MAX_MISS_RADII`] of the destination's radius. In every case the
/// craft should simply carry on, since a burn onto a path that misses only adds error for the
/// next correction to remove.
pub fn plan_correction(
    scene: &Scene,
    craft: BodyId,
    target: BodyId,
    expected_arrival_s: f64,
    via: Option<&Flyby>,
) -> Option<TransferPlan> {
    let craft_prop = scene.get(craft)?;
    let r0 = to_metres(craft_prop.body.center());
    let v0 = to_metres(craft_prop.velocity_units_s);
    let horizon = (1.5 * expected_arrival_s).max(TRANSFER_MIN_HORIZON_S);
    let never_cancelled = PlanProgress::default();
    let ephemeris = Ephemeris::build(scene, Some(craft), target, None, horizon, &never_cancelled)?;
    let window = (0.5 * expected_arrival_s, horizon);

    let launch = |dv: &[f64; 3]| Some((r0, v0 + DVec3::new(dv[0], dv[1], dv[2])));
    let flyby = via.and_then(|flyby| Some((flyby, ephemeris.index_of(flyby.body)?)));
    let (goal, adapt) = match flyby {
        Some((flyby, body)) => {
            let basis = miss_basis(flyby.relative_velocity_m_s);
            let radius = ephemeris.radius_m[body];
            let goal = Goal {
                body,
                window: flyby_window(flyby.time_s, ephemeris.step_s),
                point: Some(AimPoint {
                    basis,
                    offset_m: [flyby.offset_m.dot(basis.0), flyby.offset_m.dot(basis.1)],
                    hold_direction: true,
                }),
                tolerance_m: TRANSFER_FLYBY_AIM_FRACTION * radius,
            };
            (goal, Some(Adapt::MissAtLeast(radius)))
        }
        None => (Goal::centre(ephemeris.target, window), None),
    };
    let (dv, approach) = aim(
        &ephemeris, [0.0; 3], [0.01; 3], [50.0; 3], &goal, adapt, &launch,
    )?;
    let (r, v) = launch(&dv)?;
    // Via a flyby, the arrival can only come after it: an approach to the destination found before
    // the pass is not the arrival, and taking it as one dropped the flyby from the plan.
    let arrival_window = flyby.map_or(window, |(flyby, _)| (flyby.time_s, horizon));
    let plan = finish(&ephemeris, r, v, arrival_window, approach);
    let reach_m = TRANSFER_CORRECTION_MAX_MISS_RADII * ephemeris.radius_m[ephemeris.target];
    (plan.miss_m <= reach_m).then_some(plan)
}

/// The window in which a pass planned for `time_s` counts: within [`TRANSFER_FLYBY_WINDOW_FRACTION`]
/// of that time either side. Wide enough for the drift a correction removes, narrow enough to
/// exclude the planet's next encounter, which for Ganymede comes a week later and once captured a
/// correction meant for the one before.
pub(super) fn flyby_window(time_s: f64, step_s: f64) -> (f64, f64) {
    let half = TRANSFER_FLYBY_WINDOW_FRACTION * time_s + step_s;
    ((time_s - half).max(0.0), time_s + half)
}

/// The radius a prop physically interacts at, in metres: its contact radius when it has one,
/// else its sphere radius, else zero.
pub fn physical_radius_m(prop: &Prop) -> f64 {
    let units = match (prop.contact_radius_units, &prop.body) {
        (Some(radius), _) => radius,
        (None, Body::Sphere { radius, .. }) => *radius,
        _ => 0.0,
    };
    f64::from(units) * METRES_PER_UNIT
}

fn to_metres(units: Vec3) -> DVec3 {
    units.as_dvec3() * METRES_PER_UNIT
}

fn to_units(metres: DVec3) -> Vec3 {
    (metres / METRES_PER_UNIT).as_vec3()
}

#[cfg(test)]
#[path = "../../../tst/core/transfer_tests.rs"]
mod tests;
