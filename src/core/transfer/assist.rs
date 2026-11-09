//! Gravity-assist routes: reaching a destination by flying close past planets on the way.
//!
//! # What a flyby is, to the search
//!
//! The craft never burns during a pass, so a flyby cannot change its speed *relative to the planet*:
//! it comes in at `v∞` and leaves at `v∞`, turned through an angle set by how close it passes and
//! on which side. But the planet is moving, so turning `v∞` changes the craft's velocity about the
//! Sun, and that is what flings it outward or inward. The search's only lever is therefore *where
//! beside the planet the craft passes*: a point in the plane across its approach, the aim point.
//!
//! # How a route is solved
//!
//! A route is a sequence: origin, up to [`TRANSFER_MAX_FLYBYS`] planets, destination. Every level
//! solves for the same two numbers, the launch azimuth and elevation, and only the goal changes:
//!
//! - The launch direction is scanned for paths that pass the first planet, as a direct departure
//!   scans for paths to its target.
//! - At each planet, a grid of aim points around it is solved for: "launch so that you pass here".
//!   Each solve starts from the angles of the one before, ring by ring along each spoke, and each
//!   is flown on to the next body of the route. The closest few go on to the next planet, or, at the
//!   last, are aimed at the destination's centre.
//!
//! Earlier flybys need no holding in place while a later one is aimed: the later pass is thousands
//! of times more sensitive to the launch angles than the earlier one, so the angle changes that move
//! it barely move the earlier pass. What does change, level by level, is that sensitivity, which is
//! why every aim here rescales its probes to what it measures ([`Adapt`]). An earlier design nested
//! one aim inside another per flyby and could not converge at the second, because each level
//! multiplied the error left by the one below.
//!
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// What every level of a route search shares, across the threads that search from each first
/// planet at once.
struct Search<'a> {
    ephemeris: &'a Ephemeris<'a>,
    /// Arrivals after this cannot win: the best found so far by any thread, or the horizon. An
    /// `f64`'s bits, so threads can share it without a lock.
    cutoff_bits: AtomicU64,
}

impl Search<'_> {
    fn cutoff_s(&self) -> f64 {
        f64::from_bits(self.cutoff_bits.load(Ordering::Relaxed))
    }

    /// Lower the cutoff to `arrival_s` if that is earlier, whichever thread found it.
    fn improve(&self, arrival_s: f64) {
        let mut current = self.cutoff_bits.load(Ordering::Relaxed);
        while arrival_s < f64::from_bits(current) {
            match self.cutoff_bits.compare_exchange_weak(
                current,
                arrival_s.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(seen) => current = seen,
            }
        }
    }
}

/// A body met during a route search: when, and how.
#[derive(Copy, Clone)]
struct Pass {
    body: usize,
    approach: Approach,
}

/// Plan the fastest route from `origin` to `target` that passes up to [`TRANSFER_MAX_FLYBYS`]
/// planets on the way, leaving at the cap, arriving before `cutoff_s`. `None` when no route is found
/// or the search was cancelled.
#[allow(clippy::too_many_arguments)]
pub(super) fn assisted_departure(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    max_departure_speed_m_s: f64,
    launch_altitude_m: f64,
    horizon_s: f64,
    cutoff_s: f64,
    progress: &PlanProgress,
) -> Option<TransferPlan> {
    let ephemeris = Ephemeris::build(scene, None, target, Some(origin), horizon_s, progress)?;
    let origin_index = ephemeris.index_of(origin)?;
    let dominant = ephemeris.dominant();
    let origin_primary = moon_primary(scene, origin).map(|(primary, _)| primary);

    // Flyby candidates: the origin's siblings, massive and solid, that are not the destination,
    // and whose orbits reach into the band the route crosses. A planet wholly outside it, like
    // Saturn on the way from Mercury to Jupiter, could only help by sending the craft past the
    // target and back, which is never the fastest way there.
    let orbit_range = |i: usize| orbit_range_m(&ephemeris, i, dominant);
    let (origin_low, origin_high) = orbit_range(origin_index);
    let (target_low, target_high) = orbit_range(ephemeris.target);
    let band = (
        origin_low.min(target_low) / TRANSFER_FLYBY_BAND_MARGIN,
        origin_high.max(target_high) * TRANSFER_FLYBY_BAND_MARGIN,
    );
    let candidates: Vec<usize> = (0..ephemeris.ids.len())
        .filter(|&i| i != origin_index && i != ephemeris.target && i != dominant)
        .filter(|&i| ephemeris.mu[i] > 0.0 && ephemeris.radius_m[i] > 0.0)
        .filter(|&i| moon_primary(scene, ephemeris.ids[i]).map(|(p, _)| p) == origin_primary)
        .filter(|&i| {
            let (low, high) = orbit_range(i);
            low <= band.1 && high >= band.0
        })
        .collect();

    let search = Search {
        ephemeris: &ephemeris,
        cutoff_bits: AtomicU64::new(cutoff_s.min(ephemeris.horizon_s()).to_bits()),
    };

    // Fewest flybys first: every route through one planet, and only if none arrives, routes through
    // two. A second flyby multiplies the work by a grid at the first, for a route that is rarely
    // faster when a one-flyby route exists. Real mission design escalates the same way.
    for flybys in 1..=TRANSFER_MAX_FLYBYS {
        let best = routes_with(
            &search,
            &candidates,
            origin_index,
            flybys,
            max_departure_speed_m_s,
            launch_altitude_m,
        );
        if best.is_some() || progress.is_cancelled() {
            return best.map(|mut plan| {
                plan.departure_speed_m_s = max_departure_speed_m_s;
                plan
            });
        }
    }
    None
}

/// The fastest route through exactly `flybys` planets from `candidates`, if any arrives. Each
/// first planet is searched on its own thread.
fn routes_with(
    search: &Search,
    candidates: &[usize],
    origin_index: usize,
    flybys: usize,
    max_departure_speed_m_s: f64,
    launch_altitude_m: f64,
) -> Option<TransferPlan> {
    std::thread::scope(|scope| {
        let workers: Vec<_> = candidates
            .iter()
            .map(|&first| {
                scope.spawn(move || {
                    routes_from(
                        search,
                        candidates,
                        origin_index,
                        first,
                        flybys,
                        max_departure_speed_m_s,
                        launch_altitude_m,
                    )
                })
            })
            .collect();
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok().flatten())
            .min_by(|a, b| a.flight_seconds.total_cmp(&b.flight_seconds))
    })
}

/// The fastest route through exactly `flybys` planets that passes `first` first.
#[allow(clippy::too_many_arguments)]
fn routes_from(
    search: &Search,
    candidates: &[usize],
    origin_index: usize,
    first: usize,
    flybys: usize,
    max_departure_speed_m_s: f64,
    launch_altitude_m: f64,
) -> Option<TransferPlan> {
    let ephemeris = search.ephemeris;
    let mut best: Option<TransferPlan> = None;
    {
        let radial = RadialLaunch::new(ephemeris, origin_index, first, launch_altitude_m)?;
        let launch = |angles: &[f64; 2]| Some(radial.state(max_departure_speed_m_s, angles));

        // Routes that start with this planet: straight on to the destination, or via one more
        // planet, which may be the origin itself coming round again.
        let onward: Vec<Vec<usize>> = if flybys == 1 {
            vec![vec![ephemeris.target]]
        } else {
            candidates
                .iter()
                .copied()
                .chain([origin_index])
                .filter(|&second| second != first)
                .map(|second| vec![second, ephemeris.target])
                .collect()
        };

        let seeds = scan_for(search, &launch, first);
        for (angles, pass) in seeds {
            for rest in &onward {
                let Some(plan) = through(search, &launch, angles, pass, rest, &[first]) else {
                    continue;
                };
                if best
                    .as_ref()
                    .is_none_or(|b| plan.flight_seconds < b.flight_seconds)
                {
                    search.improve(plan.flight_seconds);
                    best = Some(plan);
                }
            }
        }
    }
    best
}

/// Launch directions whose paths pass near `body`, as `(angles, pass)`, closest first. Each
/// direction is judged by its *first* pass within [`TRANSFER_FLYBY_SCAN_HILL_RADII`] Hill radii, so a longer horizon
/// adds later encounters without trading away the early ones that make fast routes.
fn scan_for(search: &Search, launch: &Launcher<2>, body: usize) -> Vec<([f64; 2], Pass)> {
    let ephemeris = search.ephemeris;
    let n = TRANSFER_AZIMUTH_SAMPLES;
    let step = std::f64::consts::TAU / n as f64;
    let cutoff = search.cutoff_s();
    // Wider than the Hill sphere where the pass will count: one step of the scan moves an encounter
    // an AU away by millions of kilometres, so a scan that only kept passes inside the Hill sphere
    // stepped over most of them. Newton's method pulls a near miss in to its aim point.
    let reach = TRANSFER_FLYBY_SCAN_HILL_RADII * ephemeris.hill_m[body];
    let scan: Vec<Option<Approach>> = (0..n)
        .map(|k| {
            let (r, v) = launch(&[step * k as f64, 0.0])?;
            let flight = fly(ephemeris, r, v, body, (0.0, cutoff), reach, false);
            (!flight.crashed).then_some(flight.approach).flatten()
        })
        .collect();
    let distance = |k: usize| scan[k % n].map_or(f64::INFINITY, |a| a.distance_m);
    let mut dips: Vec<usize> = (0..n)
        .filter(|&k| {
            distance(k) < reach
                && distance(k) <= distance(k + n - 1)
                && distance(k) <= distance(k + 1)
        })
        .collect();
    dips.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)));
    dips.truncate(TRANSFER_FLYBY_SEEDS);
    dips.into_iter()
        .filter_map(|k| scan[k].map(|approach| ([step * k as f64, 0.0], Pass { body, approach })))
        .collect()
}

/// Continue a route through `pass`: solve the launch for a grid of aim points beside its planet,
/// fly each on to the next body of `rest`, and take the closest few onward. `angles` is a launch
/// that meets `pass`.
fn through(
    search: &Search,
    launch: &Launcher<2>,
    angles: [f64; 2],
    pass: Pass,
    rest: &[usize],
    route: &[usize],
) -> Option<TransferPlan> {
    let ephemeris = search.ephemeris;
    let next = *rest.first()?;
    if ephemeris.progress.is_cancelled() || pass.approach.time_s >= search.cutoff_s() {
        return None;
    }
    if !within_reach_after(ephemeris, &pass, next) {
        return None;
    }

    let body_radius = ephemeris.radius_m[pass.body];
    let basis = miss_basis(pass.approach.relative_velocity_m_s);
    let t_pass = pass.approach.time_s;
    let pass_window = flyby_window(t_pass, ephemeris.step_s);
    let pass_at = |from: [f64; 2], point: [f64; 2]| {
        let goal = Goal {
            body: pass.body,
            window: pass_window,
            point: Some(AimPoint {
                basis,
                offset_m: point,
                hold_direction: false,
            }),
            tolerance_m: TRANSFER_FLYBY_AIM_FRACTION * body_radius,
        };
        aim(
            ephemeris,
            from,
            [TRANSFER_FLYBY_FIRST_PROBE_RAD; 2],
            [TRANSFER_FLYBY_FIRST_STEP_RAD; 2],
            &goal,
            Some(Adapt::MissAtLeast(body_radius)),
            launch,
        )
        .map(|(angles, _)| angles)
    };

    // The grid, ring by ring outward along each spoke, so every solve starts beside the last.
    let mut grid: Vec<([f64; 2], Approach)> = Vec::new();
    for spoke in 0..TRANSFER_FLYBY_ANGLES {
        let angle = std::f64::consts::TAU * spoke as f64 / TRANSFER_FLYBY_ANGLES as f64;
        let mut from = angles;
        for &radii in &TRANSFER_FLYBY_RADII {
            if ephemeris.progress.is_cancelled() {
                return None;
            }
            let point = [
                radii * body_radius * angle.cos(),
                radii * body_radius * angle.sin(),
            ];
            let Some(solved) = pass_at(from, point) else {
                continue;
            };
            from = solved;
            let Some((r, v)) = launch(&solved) else {
                continue;
            };
            let flight = fly(
                ephemeris,
                r,
                v,
                next,
                (t_pass, search.cutoff_s()),
                f64::INFINITY,
                false,
            );
            if let (false, Some(approach)) = (flight.crashed, flight.approach) {
                grid.push((solved, approach));
            }
        }
    }
    grid.sort_by(|a, b| a.1.distance_m.total_cmp(&b.1.distance_m));
    grid.truncate(TRANSFER_FLYBY_CANDIDATES);

    grid.into_iter()
        .filter_map(|(solved, approach)| {
            if rest.len() == 1 {
                arrive(search, launch, solved, t_pass, route)
            } else {
                let longer: Vec<usize> = route.iter().copied().chain([next]).collect();
                through(
                    search,
                    launch,
                    solved,
                    Pass {
                        body: next,
                        approach,
                    },
                    &rest[1..],
                    &longer,
                )
            }
        })
        .min_by(|a, b| a.flight_seconds.total_cmp(&b.flight_seconds))
}

/// The last leg: aim the launch at the destination's centre, from angles whose path passes every
/// planet of `route`.
///
/// The aim is free to move the launch anywhere that hits the destination, and from some starts it
/// slides onto a path that skips the flybys altogether: a direct path, found the long way round.
/// So the result counts only if it still passes every planet of the route, in order.
fn arrive(
    search: &Search,
    launch: &Launcher<2>,
    angles: [f64; 2],
    after_s: f64,
    route: &[usize],
) -> Option<TransferPlan> {
    let ephemeris = search.ephemeris;
    let window = (after_s, search.cutoff_s());
    let goal = Goal::centre(ephemeris.target, window);
    let solved = aim(
        ephemeris,
        angles,
        [TRANSFER_FLYBY_FIRST_PROBE_RAD; 2],
        [TRANSFER_FLYBY_FIRST_STEP_RAD; 2],
        &goal,
        Some(Adapt::MissAtLeast(ephemeris.radius_m[ephemeris.target])),
        launch,
    );
    let (angles, approach) = solved?;
    let (r, v) = launch(&angles)?;
    let plan = finish(ephemeris, r, v, window, approach);
    let mut passed = plan.flybys.iter().map(|flyby| flyby.body);
    let keeps_route = route
        .iter()
        .all(|&body| passed.any(|id| id == ephemeris.ids[body]));
    keeps_route.then_some(plan)
}

/// The nearest and farthest body `i`'s orbit comes to `dominant`, from its state at the start.
fn orbit_range_m(ephemeris: &Ephemeris, i: usize, dominant: usize) -> (f64, f64) {
    let (r, v) = ephemeris.states[0][i];
    let (r0, v0) = ephemeris.states[0][dominant];
    let (r, v) = (r - r0, v - v0);
    let mu = ephemeris.mu[dominant];
    let low = periapsis_m(r, v, mu);
    let energy = 0.5 * v.length_squared() - mu / r.length();
    let high = if energy >= 0.0 {
        f64::INFINITY
    } else {
        // Apoapsis = 2a − periapsis, with 2a = −μ/E.
        -mu / energy - low
    };
    (low, high)
}

/// Whether, after `pass`, the craft could coast out to `next` at all: the reach bound from the
/// planet it just passed, at the speed it passes it with. An unpowered flyby keeps `v∞` relative
/// to the planet, so this is the most the pass can leave it with.
fn within_reach_after(ephemeris: &Ephemeris, pass: &Pass, next: usize) -> bool {
    let dominant = ephemeris.dominant();
    let mu = ephemeris.mu[dominant];
    let t = pass.approach.time_s;
    let relative = |body: usize| {
        (
            ephemeris.position(body, t) - ephemeris.position(dominant, t),
            ephemeris.velocity(body, t) - ephemeris.velocity(dominant, t),
        )
    };
    let (r_pass, v_pass) = relative(pass.body);
    let (r_next, v_next) = relative(next);
    let reach = coasting_limit_m(
        r_pass.length(),
        v_pass.length() + pass.approach.relative_velocity_m_s.length(),
        mu,
    );
    periapsis_m(r_next, v_next, mu) <= TRANSFER_REACH_MARGIN * reach
}
