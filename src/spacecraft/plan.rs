//! Planning a spacecraft's flight from one body to a landing on another, by its propulsion.
//!
//! - A **coasting** craft is planned by [`plan_departure`]: the fastest coasting path at up to its
//!   launch speed, gravity assists included.
//! - A **direct** craft is planned by flying its steering law ([`crate::spacecraft::guidance`])
//!   through the scene's predicted future.
//! - A **transfer** craft flies the least-energy orbital transfer, found in two stages. First a
//!   two-body search: for each time of flight, the Lambert arc about the body the transfer is
//!   computed about (the Sun, or a planet for a trip to its own moon), and its energy, counted as
//!   the speed change to leave the origin's surface onto it plus the speed change to land from it.
//!   Then the cheapest arcs are solved in full, every body pulling, by
//!   [`crate::core::transfer::solve_coast`], until one converges.
//!
//! Energy is counted as speed change (Δv) because, for a fixed exhaust speed and mass, a burn's
//! energy is proportional to it (see [`FlightReport`](crate::spacecraft::FlightReport)). The
//! departure is *now*, when `R` is pressed: the search finds the cheapest arc from where the
//! planets are, not the cheapest launch window.

use bevy::math::{DVec3, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{moon_primary, BodyId, Scene};
use crate::core::transfer::{
    physical_radius_m, plan_departure, solve_coast, solve_coast_with, Forecast, NoTransfer,
    PlanProgress, Reach, TransferPlan,
};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::spacecraft::config::{Propulsion, Spacecraft};
use crate::spacecraft::constants::{
    DEPARTURE_BURN_STEPS, ORBIT_SEARCH_CANDIDATES, ORBIT_SEARCH_DIRECTIONS,
    ORBIT_SEARCH_MAX_FRACTION, ORBIT_SEARCH_MIN_FRACTION, ORBIT_SEARCH_REFINE_STEPS,
    ORBIT_SEARCH_SAMPLES, ORBIT_SOLVE_WINDOW_FRACTION, POWERED_BRAKE_FRACTION,
    POWERED_CANCEL_CHECK_STEPS, POWERED_HORIZON_FACTOR,
};
use crate::spacecraft::guidance::{braking_m_s2, segment_entry_m, steer, Law, Sighting};
use crate::spacecraft::lambert::lambert;

/// A planned flight: what the craft will do, and what it is expected to cost.
#[derive(Clone, Debug, PartialEq)]
pub struct FlightPlan {
    /// The predicted flight. It starts with the craft at rest relative to the origin (a coasting
    /// craft starts at its launch velocity), and its path runs to the landing.
    pub transfer: TransferPlan,
    /// For a transfer, the departure burn: a speed change (m/s), flown at full acceleration in this
    /// fixed direction from rest above the origin (see [`departure_burn`]).
    pub departure_burn_m_s: Option<DVec3>,
    /// The speed change the flight is expected to take, in m/s, counting burns as instantaneous.
    /// The live flight's finite burns fight gravity for a while, so it takes somewhat more.
    pub delta_v_estimate_m_s: f64,
}

/// Why there is no flight.
#[derive(Clone, Debug, PartialEq)]
pub enum NoFlight {
    /// The same body twice, a missing one, a body with no surface, or an invalid craft.
    InvalidRoute,
    /// The search was cancelled through its [`PlanProgress`].
    Cancelled,
    /// A coasting craft cannot reach the target at its launch speed (see
    /// [`crate::core::transfer::coasting_reach`]).
    OutOfReach(Reach),
    /// A direct flight would hit this body on the way, despite steering around it.
    Blocked(BodyId),
    /// A direct flight had not landed by the end of its horizon.
    NoArrival,
    /// The craft's engine cannot beat this body's surface gravity: it can neither lift off from it
    /// nor land on it softly.
    TooMuchGravity {
        body: BodyId,
        gravity_m_s2: f64,
        acceleration_m_s2: f64,
    },
    /// No coasting arc to the target converged in the full solve.
    NoArc,
}

impl From<NoTransfer> for NoFlight {
    fn from(reason: NoTransfer) -> Self {
        match reason {
            NoTransfer::InvalidRoute => NoFlight::InvalidRoute,
            NoTransfer::Cancelled => NoFlight::Cancelled,
            NoTransfer::OutOfReach(reach) => NoFlight::OutOfReach(reach),
        }
    }
}

/// Plan `craft`'s flight from `origin` to a landing on `target`, by its propulsion (see the module
/// docs). The scene is not changed.
pub fn plan_flight(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    craft: &Spacecraft,
    progress: &PlanProgress,
) -> Result<FlightPlan, NoFlight> {
    if origin == target || !craft.is_valid() {
        return Err(NoFlight::InvalidRoute);
    }
    match &craft.propulsion {
        Propulsion::Coasting {
            max_departure_speed_m_s,
        } => {
            let transfer = plan_departure(
                scene,
                origin,
                target,
                *max_departure_speed_m_s,
                craft.launch_altitude_m,
                progress,
            )?;
            Ok(FlightPlan {
                delta_v_estimate_m_s: transfer.departure_speed_m_s,
                departure_burn_m_s: None,
                transfer,
            })
        }
        Propulsion::Direct { .. } => {
            let accel = craft.acceleration_m_s2().ok_or(NoFlight::InvalidRoute)?;
            check_gravity(scene, origin, target, accel, craft.launch_altitude_m)?;
            plan_direct(
                scene,
                origin,
                target,
                accel,
                craft.launch_altitude_m,
                progress,
            )
        }
        Propulsion::Transfer { .. } => {
            let accel = craft.acceleration_m_s2().ok_or(NoFlight::InvalidRoute)?;
            check_gravity(scene, origin, target, accel, craft.launch_altitude_m)?;
            plan_transfer(
                scene,
                origin,
                target,
                accel,
                craft.launch_altitude_m,
                progress,
            )
        }
    }
}

/// A body's `G · m` (m³/s²) and physical radius (m) from the scene.
fn mu_and_radius(scene: &Scene, id: BodyId) -> Option<(f64, f64)> {
    let prop = scene.get(id)?;
    Some((
        GRAVITATIONAL_CONSTANT * f64::from(prop.mass_kg),
        physical_radius_m(prop),
    ))
}

/// The gravity at a body's surface, or at `altitude_m` above it, in m/s².
pub fn surface_gravity_m_s2(scene: &Scene, id: BodyId, altitude_m: f64) -> Option<f64> {
    let (mu, radius) = mu_and_radius(scene, id)?;
    let r = radius + altitude_m;
    (r > 0.0).then(|| mu / (r * r))
}

/// Whether an engine of `accel` can lift off from `origin` (at the launch altitude) and land
/// softly on `target`: it must beat the origin's gravity, and leave a braking budget at the
/// target's surface (see [`braking_m_s2`]).
fn check_gravity(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    accel: f64,
    launch_altitude_m: f64,
) -> Result<(), NoFlight> {
    let (Some((_, origin_radius)), Some((_, target_radius))) =
        (mu_and_radius(scene, origin), mu_and_radius(scene, target))
    else {
        return Err(NoFlight::InvalidRoute);
    };
    if origin_radius <= 0.0 || target_radius <= 0.0 {
        return Err(NoFlight::InvalidRoute);
    }
    let too_much = |body: BodyId, gravity_m_s2: f64| NoFlight::TooMuchGravity {
        body,
        gravity_m_s2,
        acceleration_m_s2: accel,
    };
    let lift = surface_gravity_m_s2(scene, origin, launch_altitude_m).unwrap_or(0.0);
    if lift >= accel {
        return Err(too_much(origin, lift));
    }
    let land = surface_gravity_m_s2(scene, target, 0.0).unwrap_or(0.0);
    if braking_m_s2(accel, land) <= 0.0 {
        return Err(too_much(target, land));
    }
    Ok(())
}

fn to_metres(units: Vec3) -> DVec3 {
    units.as_dvec3() * METRES_PER_UNIT
}

fn to_units(metres: DVec3) -> Vec3 {
    (metres / METRES_PER_UNIT).as_vec3()
}

// ---------------------------------------------------------------------------------------------
// Direct flight
// ---------------------------------------------------------------------------------------------

/// A direct flight: the steering law flown through the scene's predicted future, for the path, the
/// arrival time, the peak speed, and anything in the way.
///
/// The craft starts `launch_altitude_m` above `origin`, on the side facing the target, moving with
/// `origin`. The live flight steers itself with the same law, so it needs no corrections.
fn plan_direct(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    accel: f64,
    launch_altitude_m: f64,
    progress: &PlanProgress,
) -> Result<FlightPlan, NoFlight> {
    let (Some(origin_prop), Some(target_prop)) = (scene.get(origin), scene.get(target)) else {
        return Err(NoFlight::InvalidRoute);
    };
    let origin_radius = physical_radius_m(origin_prop);
    let origin_centre = to_metres(origin_prop.body.center());
    let target_centre = to_metres(target_prop.body.center());
    let outward = (target_centre - origin_centre).normalize_or_zero();
    if outward == DVec3::ZERO {
        return Err(NoFlight::InvalidRoute);
    }
    let start = origin_centre + outward * (origin_radius + launch_altitude_m);
    let start_velocity = to_metres(origin_prop.velocity_units_s);
    let law = Law {
        acceleration_m_s2: accel,
        braking_m_s2: braking_m_s2(
            accel,
            surface_gravity_m_s2(scene, target, 0.0).unwrap_or(0.0),
        ),
        may_speed_up: true,
        avoid: true,
    };

    // A straight burn-and-brake over the gap takes 2√(d / a); the horizon allows for detours and
    // the target's motion.
    let gap = (target_centre - start).length();
    let straight_s = 2.0 * (gap / (POWERED_BRAKE_FRACTION * accel)).sqrt();
    let horizon = POWERED_HORIZON_FACTOR * straight_s;
    let forecast = Forecast::new(scene, None, target, Some(origin), horizon, progress)
        .ok_or(NoFlight::Cancelled)?;
    let target_index = forecast.target_index();
    let origin_index = forecast.index_of(origin);
    let sighting = |body: usize, t: f64| Sighting {
        position_m: forecast.position_m(body, t),
        velocity_m_s: forecast.velocity_m_s(body, t),
        radius_m: forecast.radius_m(body),
    };

    let spacing = gap / crate::core::constants::TRANSFER_PATH_POINTS;
    let (mut r, mut v, mut t) = (start, start_velocity, 0.0);
    let mut path = vec![to_units(r)];
    let mut peak_speed = v.length();
    let mut delta_v = 0.0;
    let mut steps: u64 = 0;
    loop {
        if steps.is_multiple_of(POWERED_CANCEL_CHECK_STEPS) && progress.is_cancelled() {
            return Err(NoFlight::Cancelled);
        }
        steps += 1;
        if t >= horizon {
            return Err(NoFlight::NoArrival);
        }

        let goal = sighting(target_index, t);
        let obstacles: Vec<Sighting> = (0..forecast.bodies().len())
            .filter(|&i| i != target_index)
            .map(|i| sighting(i, t))
            .collect();
        let steering = steer(r, v, &goal, &obstacles, &law, horizon - t);
        let dt = steering.substep_s;

        let gravity = forecast.gravity_m_s2(r, t);
        let v_next = v + (steering.acceleration_m_s2 + gravity) * dt;
        let r_next = r + v_next * dt;
        let t_next = t + dt;
        delta_v += steering.acceleration_m_s2.length() * dt;

        // Touchdown, judged on the segment relative to the target as the live flight judges it.
        let goal_next = sighting(target_index, t_next);
        if let Some(entry) = segment_entry_m(
            r - goal.position_m,
            r_next - goal_next.position_m,
            goal.radius_m,
        ) {
            path.push(to_units(goal_next.position_m + entry));
            return Ok(FlightPlan {
                transfer: TransferPlan {
                    position_units: to_units(start),
                    velocity_units_s: to_units(start_velocity),
                    path_units: path,
                    flight_seconds: t_next,
                    miss_m: 0.0,
                    departure_speed_m_s: 0.0,
                    flybys: Vec::new(),
                    peak_speed_m_s: peak_speed,
                },
                departure_burn_m_s: None,
                delta_v_estimate_m_s: delta_v,
            });
        }
        // Any other body hit on the way. The origin only counts once the craft has left it.
        for (i, &id) in forecast.bodies().iter().enumerate() {
            let radius = forecast.radius_m(i);
            if i == target_index || radius <= 0.0 || (Some(i) == origin_index && t == 0.0) {
                continue;
            }
            if (r_next - forecast.position_m(i, t_next)).length() < radius {
                return Err(NoFlight::Blocked(id));
            }
        }

        (r, v, t) = (r_next, v_next, t_next);
        peak_speed = peak_speed.max(v.length());
        if (r - to_metres(*path.last().unwrap_or(&Vec3::ZERO))).length() >= spacing {
            path.push(to_units(r));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Least-energy transfer
// ---------------------------------------------------------------------------------------------

/// The body a transfer between `origin` and `target` is computed about: the planet, for a trip
/// between it and its own moon or between two of its moons; the scene's dominant body otherwise.
pub fn central_body(scene: &Scene, origin: BodyId, target: BodyId) -> Option<BodyId> {
    let primary = |id: BodyId| moon_primary(scene, id).map(|(p, _)| p);
    let dominant = scene
        .iter()
        .filter(|(_, prop)| !prop.immovable)
        .max_by(|(_, a), (_, b)| a.mass_kg.total_cmp(&b.mass_kg))
        .map(|(id, _)| id)?;
    Some(match (primary(origin), primary(target)) {
        (_, Some(p)) if p == origin => origin,
        (Some(p), _) if p == target => target,
        (Some(a), Some(b)) if a == b => a,
        _ => dominant,
    })
}

/// One two-body arc the search scored: when it arrives, which surface direction it uses where the
/// origin or target is the central body, what it costs, and where and how fast the craft leaves.
#[derive(Copy, Clone, Debug)]
struct Arc {
    tof_s: f64,
    direction: usize,
    delta_v_m_s: f64,
    /// The arrival's share of `delta_v_m_s`: what landing from the arc costs.
    delta_v_in_m_s: f64,
    launch_m: DVec3,
    launch_velocity_m_s: DVec3,
}

/// The least-energy transfer (see the module docs).
fn plan_transfer(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    accel: f64,
    launch_altitude_m: f64,
    progress: &PlanProgress,
) -> Result<FlightPlan, NoFlight> {
    let central = central_body(scene, origin, target).ok_or(NoFlight::InvalidRoute)?;
    let (Some((mu_c, _)), Some((mu_o, radius_o)), Some((mu_t, radius_t))) = (
        mu_and_radius(scene, central),
        mu_and_radius(scene, origin),
        mu_and_radius(scene, target),
    ) else {
        return Err(NoFlight::InvalidRoute);
    };
    let at = |id: BodyId| {
        scene
            .get(id)
            .map(|p| (to_metres(p.body.center()), to_metres(p.velocity_units_s)))
            .ok_or(NoFlight::InvalidRoute)
    };
    let ((c0, vc0), (o0, vo0), (t0, vt0)) = (at(central)?, at(origin)?, at(target)?);
    let launch_r = radius_o + launch_altitude_m;

    // The Hohmann time between the two orbits about the central body sets the search's scale.
    let r_o = if origin == central {
        launch_r
    } else {
        (o0 - c0).length()
    };
    let r_t = if target == central {
        radius_t
    } else {
        (t0 - c0).length()
    };
    let semi_major = 0.5 * (r_o + r_t);
    let hohmann_s = std::f64::consts::PI * (semi_major.powi(3) / mu_c).sqrt();
    let (t_min, t_max) = (
        ORBIT_SEARCH_MIN_FRACTION * hohmann_s,
        ORBIT_SEARCH_MAX_FRACTION * hohmann_s,
    );

    let horizon = t_max * (1.0 + ORBIT_SOLVE_WINDOW_FRACTION) * 1.05;
    let forecast = Forecast::new(scene, None, target, Some(origin), horizon, progress)
        .ok_or(NoFlight::Cancelled)?;
    let (Some(ci), Some(ti)) = (forecast.index_of(central), forecast.index_of(target)) else {
        return Err(NoFlight::InvalidRoute);
    };

    // Prograde: the way the origin goes round the central body (or the target, if the origin is
    // the central body).
    let normal = {
        let n = if origin == central {
            (t0 - c0).cross(vt0 - vc0)
        } else {
            (o0 - c0).cross(vo0 - vc0)
        };
        if n.length_squared() > 0.0 {
            n.normalize()
        } else {
            DVec3::Y
        }
    };
    // Landing on the central body itself (a moon to its planet): where on its surface the arc
    // arrives is searched too, in the transfer plane.
    let reference = {
        let toward = (o0 - t0) - normal * (o0 - t0).dot(normal);
        if toward.length_squared() > 0.0 {
            toward.normalize()
        } else {
            normal.any_orthonormal_vector()
        }
    };
    let surface_direction = |k: usize| {
        let angle = std::f64::consts::TAU * k as f64 / ORBIT_SEARCH_DIRECTIONS as f64;
        reference * angle.cos() + normal.cross(reference) * angle.sin()
    };
    let directions = if target == central {
        ORBIT_SEARCH_DIRECTIONS
    } else {
        1
    };

    // The two-body cost of the arc that leaves the origin now and arrives after `tof_s`, landing
    // in surface direction `k` when the target is the central body.
    let cost = |tof_s: f64, k: usize| -> Option<Arc> {
        let c_t = forecast.position_m(ci, tof_s);
        let vc_t = forecast.velocity_m_s(ci, tof_s);
        let r2 = if target == central {
            surface_direction(k) * radius_t
        } else {
            forecast.position_m(ti, tof_s) - c_t
        };
        let (v1, v2) = lambert(o0 - c0, r2, tof_s, mu_c, normal)?;

        // Leaving: from rest relative to the origin at the launch altitude, the burn also climbs
        // out of the origin's well, so the launch speed is √(v∞² + 2μ/r), straight up along the
        // escape direction, from the side the craft escapes toward.
        let v_inf = v1 - (vo0 - vc0);
        let delta_v_out = (v_inf.length_squared() + 2.0 * mu_o / launch_r).sqrt();
        let direction = v_inf.normalize_or_zero();
        // Arriving: everything left relative to the target at its surface has to be braked away.
        let delta_v_in = if target == central {
            v2.length()
        } else {
            let v_inf = v2 - (forecast.velocity_m_s(ti, tof_s) - vc_t);
            (v_inf.length_squared() + 2.0 * mu_t / radius_t).sqrt()
        };
        let delta_v_m_s = delta_v_out + delta_v_in;
        delta_v_m_s.is_finite().then_some(Arc {
            tof_s,
            direction: k,
            delta_v_m_s,
            delta_v_in_m_s: delta_v_in,
            launch_m: o0 + direction * launch_r,
            launch_velocity_m_s: vo0 + direction * delta_v_out,
        })
    };

    // Leaving the central body itself (a planet to its moon): straight up, on the slowest climb
    // that reaches the target's distance, aimed at where the target will be when it tops out. That
    // is the cheapest way up from rest, and a burn sideways at a few g from rest would fall back
    // before it could build up orbital speed. The climb's time depends on how far away the target
    // is then, which barely changes, so a few rounds settle it.
    let ascent = || -> Option<Arc> {
        let mut tof_s = hohmann_s;
        let mut arc = None;
        for _ in 0..ASCENT_ROUNDS {
            let to_target = forecast.position_m(ti, tof_s) - forecast.position_m(ci, tof_s);
            let top = to_target.length();
            if top <= launch_r {
                return None;
            }
            // A radial ellipse with its apex at `top` has semi-major axis `top / 2`; the time from
            // `launch_r` up to the apex follows from Kepler's equation with eccentric anomaly E,
            // `r = a (1 − cos E)`, `t = √(a³/μ) (E − sin E)`.
            let a = 0.5 * top;
            let e0 = (1.0 - launch_r / a).clamp(-1.0, 1.0).acos();
            let climb_s = (a.powi(3) / mu_c).sqrt() * (std::f64::consts::PI - (e0 - e0.sin()));
            let speed = (2.0 * mu_c * (1.0 / launch_r - 1.0 / top)).sqrt();
            let direction = to_target.normalize();
            // At the apex the craft is at rest relative to the central body; the target passes at
            // its own speed.
            let passing = forecast.velocity_m_s(ti, climb_s) - forecast.velocity_m_s(ci, climb_s);
            let delta_v_in = (passing.length_squared() + 2.0 * mu_t / radius_t).sqrt();
            arc = Some(Arc {
                tof_s: climb_s,
                direction: 0,
                delta_v_m_s: speed + delta_v_in,
                delta_v_in_m_s: delta_v_in,
                launch_m: c0 + direction * launch_r,
                launch_velocity_m_s: vc0 + direction * speed,
            });
            if (climb_s - tof_s).abs() < 1.0 {
                break;
            }
            tof_s = climb_s;
        }
        arc
    };

    let mut minima: Vec<Arc> = if origin == central {
        ascent().into_iter().collect()
    } else {
        let best_at = |tof_s: f64| {
            (0..directions)
                .filter_map(|k| cost(tof_s, k))
                .min_by(|a, b| a.delta_v_m_s.total_cmp(&b.delta_v_m_s))
        };
        // Log-spaced samples of the time of flight, then every local minimum, refined.
        let ratio = (t_max / t_min).powf(1.0 / (ORBIT_SEARCH_SAMPLES - 1) as f64);
        let samples: Vec<(f64, Option<Arc>)> = (0..ORBIT_SEARCH_SAMPLES)
            .map(|i| {
                let tof = t_min * ratio.powi(i as i32);
                (tof, best_at(tof))
            })
            .collect();
        let value = |i: usize| samples[i].1.map_or(f64::INFINITY, |arc| arc.delta_v_m_s);
        (0..samples.len())
            .filter(|&i| {
                let here = value(i);
                here.is_finite()
                    && (i == 0 || here <= value(i - 1))
                    && (i + 1 == samples.len() || here <= value(i + 1))
            })
            .filter_map(|i| {
                // Golden-section refinement between the neighbouring samples, same direction.
                let k = samples[i].1?.direction;
                let (mut a, mut b) = (
                    samples[i.saturating_sub(1)].0,
                    samples[(i + 1).min(samples.len() - 1)].0,
                );
                let phi = 0.5 * (5.0_f64.sqrt() - 1.0);
                let f = |tof: f64| cost(tof, k).map_or(f64::INFINITY, |arc| arc.delta_v_m_s);
                for _ in 0..ORBIT_SEARCH_REFINE_STEPS {
                    let (c, d) = (b - phi * (b - a), a + phi * (b - a));
                    if f(c) < f(d) {
                        b = d;
                    } else {
                        a = c;
                    }
                }
                cost(0.5 * (a + b), k).or(samples[i].1)
            })
            .collect()
    };
    if progress.is_cancelled() {
        return Err(NoFlight::Cancelled);
    }
    minima.sort_by(|a, b| a.delta_v_m_s.total_cmp(&b.delta_v_m_s));

    // Fly the cheapest arcs in full until one converges onto the target.
    for arc in minima.iter().take(ORBIT_SEARCH_CANDIDATES) {
        if progress.is_cancelled() {
            return Err(NoFlight::Cancelled);
        }
        let window = (
            arc.tof_s * (1.0 - ORBIT_SOLVE_WINDOW_FRACTION),
            (arc.tof_s * (1.0 + ORBIT_SOLVE_WINDOW_FRACTION)).min(forecast.horizon_s()),
        );
        // The solve is for the departure burn itself: a speed change in a fixed direction, flown
        // at full acceleration from rest above the origin exactly as the live flight flies it, so
        // the coast starts where the real burn ends. It starts from the burn, in the instant
        // launch's direction, that leaves the origin with the same orbital energy: the same coast,
        // near enough, as the instant launch, so the solve begins where that one converges. A
        // burn short of it falls back and crashes before the arrival window, which no solve
        // recovers from.
        let launch_rel = arc.launch_m - o0;
        let instant = arc.launch_velocity_m_s - vo0;
        let Some(guess) = energy_matched_burn(mu_o, launch_rel, instant, accel) else {
            continue;
        };
        let launcher = |burn: DVec3| {
            departure_burn(mu_o, launch_rel, burn, accel).map(|(r, v)| (o0 + r, vo0 + v))
        };
        let Some(coast) = solve_coast_with(&forecast, guess, window, &launcher) else {
            continue;
        };
        if coast.plan.miss_m > radius_t {
            continue;
        }
        let delta_v_out = coast.velocity_m_s.length();
        // The landing is estimated from the two-body arc. The full coast is recorded through the
        // target to its closest approach to the centre, deep in the softened core, where the speed
        // is no guide to the speed at the surface.
        let delta_v_in = arc.delta_v_in_m_s;
        let mut transfer = coast.plan;
        transfer.path_units.insert(0, to_units(arc.launch_m));
        transfer.position_units = to_units(arc.launch_m);
        transfer.velocity_units_s = to_units(vo0);
        transfer.departure_speed_m_s = delta_v_out;
        return Ok(FlightPlan {
            transfer,
            departure_burn_m_s: Some(coast.velocity_m_s),
            delta_v_estimate_m_s: delta_v_out + delta_v_in,
        });
    }
    Err(NoFlight::NoArc)
}

/// The correction burn for `craft` in flight as `craft_id` in `scene`, by its propulsion: a coasting
/// craft's trim from [`crate::core::transfer::plan_correction`] (aiming at `via` if a flyby is ahead),
/// a transfer's from [`plan_transfer_correction`]. `None` for a direct flight, which needs none.
pub fn plan_correction_for(
    scene: &Scene,
    craft: &Spacecraft,
    craft_id: BodyId,
    target: BodyId,
    expected_arrival_s: f64,
    via: Option<&crate::core::transfer::Flyby>,
) -> Option<TransferPlan> {
    match craft.propulsion {
        Propulsion::Coasting { .. } => {
            crate::core::transfer::plan_correction(scene, craft_id, target, expected_arrival_s, via)
        }
        Propulsion::Transfer { .. } => {
            plan_transfer_correction(scene, craft_id, target, expected_arrival_s)
        }
        Propulsion::Direct { .. } => None,
    }
}

/// A correction for a transfer in flight: the velocity that coasts from where `craft` is now in
/// `scene` to `target`'s centre, arriving within [`ORBIT_SOLVE_WINDOW_FRACTION`] of
/// `expected_arrival_s` from now. The returned plan's `velocity_units_s` is the new velocity; the
/// flight burns toward it at up to its acceleration. `None` when no coast converges, or the one
/// that does misses the target, and the craft should carry on.
///
/// The same full solve the departure was planned with ([`solve_coast`]), not the coasting probe's
/// [`crate::core::transfer::plan_correction`]: that one is built for trims of a few m/s, while a
/// transfer arrives at its first correction with the drift its finite departure burn left, which
/// on the long arc to Jupiter needs more than its steps reach.
pub fn plan_transfer_correction(
    scene: &Scene,
    craft: BodyId,
    target: BodyId,
    expected_arrival_s: f64,
) -> Option<TransferPlan> {
    let prop = scene.get(craft)?;
    let (start, velocity) = (
        to_metres(prop.body.center()),
        to_metres(prop.velocity_units_s),
    );
    let late = expected_arrival_s * (1.0 + ORBIT_SOLVE_WINDOW_FRACTION);
    let progress = PlanProgress::default();
    let forecast = Forecast::new(scene, Some(craft), target, None, late, &progress)?;
    let window = (
        expected_arrival_s * (1.0 - ORBIT_SOLVE_WINDOW_FRACTION),
        late.min(forecast.horizon_s()),
    );
    let coast = solve_coast(&forecast, start, velocity, window)?;
    (coast.plan.miss_m <= forecast.radius_m(forecast.target_index())).then_some(coast.plan)
}

/// Fly a departure burn of `burn` (a speed change, m/s) from rest at `start_rel` (metres from the
/// origin's centre): thrust at `accel` in the burn's direction for `|burn| / accel` seconds,
/// under the origin's gravity alone, in [`DEPARTURE_BURN_STEPS`] equal steps. Returns the state
/// relative to the origin when the burn is done; `None` if the craft falls back toward the surface.
///
/// The burn is open-loop and its steps fixed in number, so where it ends changes smoothly with
/// `burn`, which is what lets [`solve_coast_with`] differentiate it. The live flight flies the same
/// burn the same way ([`crate::spacecraft::Flight`]).
pub fn departure_burn(
    mu: f64,
    start_rel: DVec3,
    burn: DVec3,
    accel: f64,
) -> Option<(DVec3, DVec3)> {
    let speed = burn.length();
    if !(speed.is_finite() && accel > 0.0) {
        return None;
    }
    let thrust = burn.normalize_or_zero() * accel;
    let dt = speed / accel / DEPARTURE_BURN_STEPS as f64;
    let floor = start_rel.length() * DEPARTURE_FLOOR_FRACTION;
    let (mut r, mut v) = (start_rel, DVec3::ZERO);
    for _ in 0..DEPARTURE_BURN_STEPS {
        let d = r.length();
        if d < floor {
            return None;
        }
        v += (thrust - r * (mu / (d * d * d))) * dt;
        r += v * dt;
    }
    Some((r, v))
}

/// The departure burn along `instant` (a speed change an instant launch from `start_rel` would
/// need) that ends with the same orbital energy about the origin as that instant launch, found by
/// bisection on its size: [`departure_burn`] climbs while it burns, so it needs more than
/// `|instant|` to make up for what the origin's gravity takes. `None` when no burn up to
/// [`ENERGY_MATCH_MAX_FACTOR`] times the instant one does.
fn energy_matched_burn(mu: f64, start_rel: DVec3, instant: DVec3, accel: f64) -> Option<DVec3> {
    let direction = instant.normalize_or_zero();
    let goal = 0.5 * instant.length_squared() - mu / start_rel.length();
    let energy = |size: f64| {
        departure_burn(mu, start_rel, direction * size, accel)
            .map(|(r, v)| 0.5 * v.length_squared() - mu / r.length())
    };
    let (mut lo, mut hi) = (instant.length(), ENERGY_MATCH_MAX_FACTOR * instant.length());
    if energy(hi).is_none_or(|e| e < goal) {
        return None;
    }
    for _ in 0..ENERGY_MATCH_STEPS {
        let mid = 0.5 * (lo + hi);
        if energy(mid).is_some_and(|e| e >= goal) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(direction * hi)
}

// ALGORITHM CONSTANT: the largest burn [`energy_matched_burn`] tries, as a multiple of the instant
// launch. Gravity takes about a third of a 3 g burn from rest near Earth and most of one near
// Jupiter, where the craft only just climbs; three times covers both. Not tunable.
const ENERGY_MATCH_MAX_FACTOR: f64 = 3.0;
// ALGORITHM CONSTANT: bisection steps of [`energy_matched_burn`]. Forty halve the bracket to a
// trillionth of the burn, far below the solve's own probe. Not tunable.
const ENERGY_MATCH_STEPS: usize = 40;

// ALGORITHM CONSTANT: how far below its start, as a fraction of its distance from the origin's
// centre, a departure burn may sink before it counts as falling back in. A burn that climbs never
// comes close; one that sinks to 0.97 of its start has lost almost all of a 200 km launch altitude
// above Earth (6,571 km to 6,374 km), so it is falling, not climbing. Not tunable.
const DEPARTURE_FLOOR_FRACTION: f64 = 0.97;
// ALGORITHM CONSTANT: rounds of the direct ascent's fixed point (climb time from the target's
// distance, the target's distance at that time). It moves by under a second after two or three,
// since the target's distance changes by a fraction of a percent over the climb. Not tunable.
const ASCENT_ROUNDS: usize = 8;

#[cfg(test)]
#[path = "../../tst/spacecraft/plan_tests.rs"]
mod tests;
