//! One trial flight through an ephemeris, and the Newton aiming loop built on it.
//!
//! A flight is aimed at a [`Goal`]: a body, a time window to meet it in, and either its centre or
//! a point beside it. The centre is how a craft lands. A point beside a planet is how it flies past
//! on a chosen side at a chosen distance, which decides how far that planet's gravity bends it and
//! so where it goes next.

use super::*;

/// The craft's closest approach to one body over one flight.
#[derive(Copy, Clone, Debug)]
pub(super) struct Approach {
    pub(super) time_s: f64,
    pub(super) distance_m: f64,
    /// Craft minus body at closest approach, in metres.
    pub(super) offset_m: DVec3,
    /// Craft minus body velocity at closest approach, in m/s.
    pub(super) relative_velocity_m_s: DVec3,
}

/// A launch from `N` parameters to the craft's starting state, or `None` when it cannot be made: a
/// launch direction, a correction burn, or a launcher that itself solves an inner aim.
pub(super) type Launcher<'a, const N: usize> = dyn Fn(&[f64; N]) -> Option<(DVec3, DVec3)> + 'a;

/// A point beside a body, in a fixed basis perpendicular to the craft's approach.
#[derive(Copy, Clone, Debug)]
pub(super) struct AimPoint {
    pub(super) basis: (DVec3, DVec3),
    pub(super) offset_m: [f64; 2],
    /// Also require the pass to approach along the basis's own direction, within
    /// [`TRANSFER_FLYBY_MAX_TURN_RAD`]. The offset is only compared inside the basis's plane, so
    /// without this a different encounter that approaches from another direction can sit on the
    /// point in that plane and count as a hit. Corrections set it, because they must find the
    /// pass they were planned around. The assist search does not, because its aim points are new
    /// passes, not a planned one.
    pub(super) hold_direction: bool,
}

impl AimPoint {
    /// How far a pass's offset strays from the basis's plane, beyond what
    /// [`TRANSFER_FLYBY_MAX_TURN_RAD`] of turn allows, in metres. Zero when `hold_direction` is off.
    ///
    /// At closest approach the offset is perpendicular to the pass's own approach, so a pass that
    /// approaches along the basis's direction has none of its offset along that direction. One that
    /// approaches turned by `θ` has `distance · sin θ` of it there.
    pub(super) fn off_plane_m(&self, approach: &Approach) -> f64 {
        if !self.hold_direction {
            return 0.0;
        }
        let planned_direction = self.basis.0.cross(self.basis.1);
        let along = approach.offset_m.dot(planned_direction).abs();
        (along - approach.distance_m * TRANSFER_FLYBY_MAX_TURN_RAD.sin()).max(0.0)
    }
}

/// Where an aim is trying to put the craft.
#[derive(Copy, Clone, Debug)]
pub(super) struct Goal {
    /// The ephemeris index of the body.
    pub(super) body: usize,
    /// Simulated seconds after launch within which the closest approach counts.
    pub(super) window: (f64, f64),
    /// `None` for the centre; otherwise the point beside the body to pass through.
    pub(super) point: Option<AimPoint>,
    /// How close to the goal counts as converged, in metres.
    pub(super) tolerance_m: f64,
}

impl Goal {
    /// The body's centre, to [`TRANSFER_AIM_TOLERANCE_M`]: a landing.
    pub(super) fn centre(body: usize, window: (f64, f64)) -> Self {
        Goal {
            body,
            window,
            point: None,
            tolerance_m: TRANSFER_AIM_TOLERANCE_M,
        }
    }
}

/// A close pass of a body during a flight: inside its Hill sphere, after having been outside it.
#[derive(Copy, Clone, Debug)]
pub(super) struct Encounter {
    pub(super) body: usize,
    pub(super) time_s: f64,
    pub(super) distance_m: f64,
    /// Craft minus body at the closest point, in metres.
    pub(super) offset_m: DVec3,
    /// Craft minus body velocity there, in m/s.
    pub(super) relative_velocity_m_s: DVec3,
}

pub(super) struct Flight {
    pub(super) approach: Option<Approach>,
    pub(super) path_units: Vec<Vec3>,
    /// `(time, position, velocity)` at each path point, kept with the path.
    pub(super) samples: Vec<(f64, DVec3, DVec3)>,
    pub(super) encounters: Vec<Encounter>,
    pub(super) crashed: bool,
}

/// Fly a test particle from `r0`, `v0` (SI) through the ephemeris, tracking its closest approach
/// to body `toward` inside `window` seconds, counting only points nearer than `within_m`.
///
/// With a finite `within_m` this finds the *first* pass inside that distance rather than the
/// closest pass of the whole window: the flight stops once it is well past the first one.
///
/// A flight ends at the window's close, on reaching the destination (the ephemeris target), or on
/// hitting any other body. With `keep_path` it also records the path, its states, and every close
/// pass on the way.
pub(super) fn fly(
    ephemeris: &Ephemeris,
    r0: DVec3,
    v0: DVec3,
    toward: usize,
    window: (f64, f64),
    within_m: f64,
    keep_path: bool,
) -> Flight {
    let horizon = ephemeris.horizon_s().min(window.1);
    let destination = ephemeris.target;
    let toward_radius = ephemeris.radius_m[toward];
    let bodies = ephemeris.mu.len();

    let (mut r, mut v, mut t) = (r0, v0, 0.0);
    let mut previous_offset = r - ephemeris.position(toward, 0.0);
    let spacing = (r - ephemeris.position(destination, 0.0)).length() / TRANSFER_PATH_POINTS;
    let mut approach: Option<Approach> = None;
    let mut flight = Flight {
        approach: None,
        path_units: Vec::new(),
        samples: Vec::new(),
        encounters: Vec::new(),
        crashed: false,
    };
    let mut last_point = r;
    if keep_path {
        flight.path_units.push(to_units(r));
        flight.samples.push((t, r, v));
    }
    // Per body: whether the craft has been outside its Hill sphere yet, and the closest it has
    // come since. A pass only counts once the craft has been away, so leaving the origin is not
    // a flyby of it and coming back to it is.
    let mut left = vec![false; bodies];
    let mut nearest_pass: Vec<Option<Encounter>> = vec![None; bodies];

    let mut steps: u32 = 0;
    while t < horizon {
        steps = steps.wrapping_add(1);
        if steps.is_multiple_of(1024) && ephemeris.progress.is_cancelled() {
            flight.crashed = true;
            return flight;
        }
        let nearest = (0..bodies)
            .filter(|&body| ephemeris.mu[body] > 0.0)
            .map(|body| (ephemeris.position(body, t) - r).length())
            .fold(f64::INFINITY, f64::min);
        let dt = (TRANSFER_STEP_FRACTION * nearest / v.length().max(1.0))
            .clamp(
                TRANSFER_MIN_STEP_S,
                ephemeris.step_s.max(TRANSFER_MIN_STEP_S),
            )
            .min(horizon - t);

        // Symplectic Euler, the order `integrate_gravity` uses: accelerate from the current
        // configuration, then move at the new velocity.
        v += ephemeris.acceleration(r, t) * dt;
        r += v * dt;
        t += dt;

        let offset = r - ephemeris.position(toward, t);
        let relative_velocity = (offset - previous_offset) / dt;

        // Closest point of the step's straight relative segment to the body's centre, so the
        // approach is found to sub-step precision rather than at whichever step lands nearest.
        let segment = offset - previous_offset;
        let s = if segment.length_squared() > 0.0 {
            (-previous_offset.dot(segment) / segment.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let closest = previous_offset + segment * s;
        let closest_time = t - dt + s * dt;
        if closest_time >= window.0
            && closest.length() < within_m
            && approach.is_none_or(|a| closest.length() < a.distance_m)
        {
            approach = Some(Approach {
                time_s: closest_time,
                distance_m: closest.length(),
                offset_m: closest,
                relative_velocity_m_s: relative_velocity,
            });
        }
        previous_offset = offset;

        if keep_path {
            if (r - last_point).length() >= spacing {
                flight.path_units.push(to_units(r));
                flight.samples.push((t, r, v));
                last_point = r;
            }
            for body in 0..bodies {
                let offset = r - ephemeris.position(body, t);
                let distance = offset.length();
                if distance > ephemeris.hill_m[body] {
                    left[body] = true;
                } else if left[body] && nearest_pass[body].is_none_or(|e| distance < e.distance_m) {
                    nearest_pass[body] = Some(Encounter {
                        body,
                        time_s: t,
                        distance_m: distance,
                        offset_m: offset,
                        relative_velocity_m_s: v - ephemeris.velocity(body, t),
                    });
                }
            }
        }

        let crashed = (0..bodies).any(|body| {
            body != destination
                && ephemeris.radius_m[body] > 0.0
                && (ephemeris.position(body, t) - r).length() < ephemeris.radius_m[body]
        });
        if crashed {
            flight.approach = approach;
            flight.crashed = true;
            return flight;
        }

        // Past the goal and well clear of it: nothing later can be a better first meeting. At the
        // destination that is a landing; at a flyby body it is the far side of the pass.
        if let Some(a) = approach {
            let landed = toward == destination && a.distance_m <= toward_radius;
            let passed = toward != destination
                && t > a.time_s
                && offset.length() > TRANSFER_FLYBY_PASSED_FACTOR * a.distance_m.max(toward_radius);
            if (landed && offset.length() > 4.0 * toward_radius) || passed {
                break;
            }
        }
    }

    if keep_path {
        if let Some(a) = approach {
            flight
                .path_units
                .push(to_units(ephemeris.position(toward, a.time_s) + a.offset_m));
        }
        flight.encounters = nearest_pass
            .into_iter()
            .flatten()
            .filter(|e| e.body != destination && ephemeris.mu[e.body] > 0.0)
            .collect();
    }
    flight.approach = approach;
    flight
}

/// The closest-approach offset projected onto the plane perpendicular to the arrival velocity,
/// in the basis `(e1, e2)`. Zero means a dead-centre hit; this is what Newton's method drives to
/// zero, because it is smooth where the raw distance has a kink.
pub(super) fn miss_vector(approach: &Approach, basis: (DVec3, DVec3)) -> [f64; 2] {
    [
        approach.offset_m.dot(basis.0),
        approach.offset_m.dot(basis.1),
    ]
}

pub(super) fn miss_basis(relative_velocity: DVec3) -> (DVec3, DVec3) {
    let u = relative_velocity.normalize_or_zero();
    let reference = if u.cross(DVec3::Y).length_squared() > 1.0e-6 {
        DVec3::Y
    } else {
        DVec3::X
    };
    let e1 = u.cross(reference).normalize();
    (e1, u.cross(e1).normalize())
}

/// How [`aim`] rescales its probe and step limit each iteration, from the sensitivity it measured.
#[derive(Copy, Clone, Debug)]
pub(super) enum Adapt {
    /// To the current distance from the goal, but never below this many metres: a flyby planet's
    /// radius, so that close to an aim point a step cannot carry the craft through the planet.
    MissAtLeast(f64),
}

/// Newton's method on the 2-D miss from `goal` over `N` launch parameters, taking the
/// minimum-norm step when `N > 2`. Returns the converged parameters and their approach.
///
/// `launch` may fail — a launcher that itself solves an inner aim does when that aim does — and
/// a failed launch counts as a failed flight.
pub(super) fn aim<const N: usize>(
    ephemeris: &Ephemeris,
    start: [f64; N],
    probe: [f64; N],
    max_step: [f64; N],
    goal: &Goal,
    adapt: Option<Adapt>,
    launch: &Launcher<N>,
) -> Option<([f64; N], Approach)> {
    let (mut probe, mut max_step) = (probe, max_step);
    let flight_at = |params: &[f64; N]| {
        let (r, v) = launch(params)?;
        let flight = fly(
            ephemeris,
            r,
            v,
            goal.body,
            goal.window,
            f64::INFINITY,
            false,
        );
        if flight.crashed {
            None
        } else {
            flight.approach
        }
    };
    let norm = |m: [f64; 2]| m[0].hypot(m[1]);
    let basis_of = |approach: &Approach| {
        goal.point
            .map_or_else(|| miss_basis(approach.relative_velocity_m_s), |p| p.basis)
    };
    let residual = |approach: &Approach, basis: (DVec3, DVec3)| {
        let m = miss_vector(approach, basis);
        match goal.point {
            Some(point) => [m[0] - point.offset_m[0], m[1] - point.offset_m[1]],
            None => m,
        }
    };
    // How far from the goal a flight is, for convergence and the line search. The cross-track miss
    // Newton drives counts in full. The along-track part counts only past
    // `TRANSFER_ALONG_TRACK_FRACTION` of the pass distance: a flight cut off by its window before the
    // true closest approach sits on the line through the goal while still far short of it, and that
    // must not pass for a hit. But the closest point is found by a straight segment across one step,
    // which near a planet is off along the track by kilometres, and a metre's tolerance must not be
    // asked of that.
    let distance_from_goal = |approach: &Approach, basis: (DVec3, DVec3)| {
        let across = norm(residual(approach, basis));
        let along = approach
            .offset_m
            .dot(approach.relative_velocity_m_s.normalize_or_zero())
            .abs();
        let allowed = (TRANSFER_ALONG_TRACK_FRACTION * approach.distance_m).max(goal.tolerance_m);
        let off_plane = goal.point.map_or(0.0, |point| point.off_plane_m(approach));
        across.hypot((along - allowed).max(0.0)).hypot(off_plane)
    };

    let mut params = start;
    for _ in 0..TRANSFER_MAX_ITERATIONS {
        if ephemeris.progress.is_cancelled() {
            return None;
        }
        let nominal = flight_at(&params)?;
        let basis = basis_of(&nominal);
        let m0 = residual(&nominal, basis);
        let nominal_distance = distance_from_goal(&nominal, basis);
        if nominal_distance <= goal.tolerance_m {
            return Some((params, nominal));
        }

        // Finite-difference Jacobian, 2 × N.
        let mut jacobian = [[0.0; N]; 2];
        for column in 0..N {
            let mut nudged = params;
            nudged[column] += probe[column];
            let m = residual(&flight_at(&nudged)?, basis);
            for row in 0..2 {
                jacobian[row][column] = (m[row] - m0[row]) / probe[column];
            }
        }

        // Rescale to what was just measured: probe for a change of a fixed small fraction of the
        // scale, and step no further than moves the miss by a fixed fraction of it. Behind a flyby
        // the miss moves a million times further per radian than on a direct path, and no single
        // probe suits both.
        if let Some(adapt) = adapt {
            let length = match adapt {
                Adapt::MissAtLeast(floor) => nominal_distance.max(floor),
            };
            for column in 0..N {
                let sensitivity = jacobian[0][column].hypot(jacobian[1][column]);
                if sensitivity.is_finite() && sensitivity > 0.0 {
                    probe[column] = TRANSFER_ADAPTIVE_PROBE_FRACTION * length / sensitivity;
                    max_step[column] = TRANSFER_ADAPTIVE_STEP_FRACTION * length / sensitivity;
                }
            }
        }

        // Δ = −Jᵀ (J Jᵀ)⁻¹ m₀: the exact Newton step when N = 2, the smallest one that zeroes the
        // linearised miss when N = 3.
        let jjt = |a: usize, b: usize| (0..N).map(|k| jacobian[a][k] * jacobian[b][k]).sum::<f64>();
        let (a, b, d) = (jjt(0, 0), jjt(0, 1), jjt(1, 1));
        let det = a * d - b * b;
        if det.abs() < f64::MIN_POSITIVE || !det.is_finite() {
            return None;
        }
        let y = [(d * m0[0] - b * m0[1]) / det, (a * m0[1] - b * m0[0]) / det];

        // Damped so one bad linearisation far from the solution cannot throw the aim away, then
        // halved until the step actually gets closer: near a strong gravity well, or where the
        // closest approach jumps between encounters, the full Newton step can overshoot forever.
        let mut delta = [0.0; N];
        let mut scale: f64 = 1.0;
        for k in 0..N {
            delta[k] = -(jacobian[0][k] * y[0] + jacobian[1][k] * y[1]);
            if delta[k].abs() > max_step[k] {
                scale = scale.min(max_step[k] / delta[k].abs());
            }
        }
        let improved = (0..TRANSFER_LINE_SEARCH_HALVINGS).find_map(|halving| {
            let s = scale * 0.5_f64.powi(halving as i32);
            let mut candidate = params;
            for k in 0..N {
                candidate[k] += delta[k] * s;
            }
            flight_at(&candidate)
                .filter(|next| distance_from_goal(next, basis) < nominal_distance)
                .map(|_| candidate)
        });
        params = improved?;
    }
    None
}

/// Fly the solved launch once more, keeping its path, and turn it into a plan. Every close pass on
/// the way becomes a [`Flyby`], with the speed it gained or lost about the system's dominant body.
pub(super) fn finish(
    ephemeris: &Ephemeris,
    r: DVec3,
    v: DVec3,
    window: (f64, f64),
    approach: Approach,
) -> TransferPlan {
    let flight = fly(
        ephemeris,
        r,
        v,
        ephemeris.target,
        window,
        f64::INFINITY,
        true,
    );
    let approach = flight.approach.unwrap_or(approach);
    let dominant = ephemeris.dominant();
    let flybys = flight
        .encounters
        .iter()
        .filter(|e| e.time_s < approach.time_s && e.body != dominant)
        .map(|e| Flyby {
            body: ephemeris.ids[e.body],
            time_s: e.time_s,
            closest_m: e.distance_m,
            speed_change_m_s: speed_change(ephemeris, &flight.samples, e, dominant),
            offset_m: e.offset_m,
            relative_velocity_m_s: e.relative_velocity_m_s,
        })
        .collect();
    let peak_speed_m_s = flight
        .samples
        .iter()
        .filter(|(t, _, _)| *t <= approach.time_s)
        .map(|(_, _, velocity)| velocity.length())
        .fold(v.length(), f64::max);
    TransferPlan {
        position_units: to_units(r),
        velocity_units_s: to_units(v),
        path_units: flight.path_units,
        flight_seconds: approach.time_s,
        miss_m: approach.distance_m,
        departure_speed_m_s: 0.0,
        flybys,
        peak_speed_m_s,
    }
}

/// The craft's speed about `dominant` just after leaving `encounter`'s Hill sphere, less its speed
/// just before entering it: what the pass gave it.
fn speed_change(
    ephemeris: &Ephemeris,
    samples: &[(f64, DVec3, DVec3)],
    encounter: &Encounter,
    dominant: usize,
) -> f64 {
    let outside = |&&(t, r, _): &&(f64, DVec3, DVec3)| {
        (ephemeris.position(encounter.body, t) - r).length() > ephemeris.hill_m[encounter.body]
    };
    let speed = |&(t, _, v): &(f64, DVec3, DVec3)| {
        let dominant_velocity = ephemeris.velocity(dominant, t);
        (v - dominant_velocity).length()
    };
    let before = samples
        .iter()
        .filter(|s| s.0 < encounter.time_s)
        .rfind(outside);
    let after = samples
        .iter()
        .filter(|s| s.0 > encounter.time_s)
        .find(outside);
    match (before, after) {
        (Some(before), Some(after)) => speed(after) - speed(before),
        _ => 0.0,
    }
}
