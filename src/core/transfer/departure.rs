//! The departure search at one horizon: the speed ladder, the direction scan, and refinement.

use super::*;

/// A launch straight up from `launch_altitude_m` above a body, parameterised by azimuth and
/// elevation in a scan plane that contains the direction to some other body.
#[derive(Copy, Clone, Debug)]
pub(super) struct RadialLaunch {
    position: DVec3,
    velocity: DVec3,
    radius: f64,
    mu: f64,
    towards: DVec3,
    across: DVec3,
    normal: DVec3,
}

impl RadialLaunch {
    /// From body `origin` at the ephemeris's start, scanning around the plane of `toward`'s
    /// direction and its velocity relative to `origin`. When that velocity is along the line of
    /// sight any plane through it will do. `None` for an origin with no surface.
    pub(super) fn new(
        ephemeris: &Ephemeris,
        origin: usize,
        toward: usize,
        launch_altitude_m: f64,
    ) -> Option<Self> {
        let (position, velocity) = ephemeris.states[0][origin];
        let (toward_position, toward_velocity) = ephemeris.states[0][toward];
        if ephemeris.radius_m[origin] <= 0.0 {
            return None;
        }
        let towards = (toward_position - position).try_normalize()?;
        let normal = [toward_velocity - velocity, DVec3::Y, DVec3::X]
            .into_iter()
            .find_map(|other| towards.cross(other).try_normalize())?;
        Some(RadialLaunch {
            position,
            velocity,
            radius: ephemeris.radius_m[origin] + launch_altitude_m,
            mu: ephemeris.mu[origin],
            towards,
            across: normal.cross(towards),
            normal,
        })
    }

    /// The craft's state leaving at `v_infinity` in direction `(azimuth, elevation)`. Leaving
    /// radially at `v∞` from radius `r` takes `√(v∞² + 2μ/r)` there; the origin's gravity takes
    /// the difference back on the way out.
    pub(super) fn state(&self, v_infinity: f64, params: &[f64; 2]) -> (DVec3, DVec3) {
        let (azimuth, elevation) = (params[0], params[1]);
        let direction = (self.towards * azimuth.cos() + self.across * azimuth.sin())
            * elevation.cos()
            + self.normal * elevation.sin();
        let speed = (v_infinity.powi(2) + 2.0 * self.mu / self.radius).sqrt();
        (
            self.position + direction * self.radius,
            self.velocity + direction * speed,
        )
    }
}

pub(super) fn departure_within(
    scene: &Scene,
    origin: BodyId,
    target: BodyId,
    max_departure_speed_m_s: f64,
    launch_altitude_m: f64,
    horizon_s: f64,
    progress: &PlanProgress,
) -> Option<TransferPlan> {
    let ephemeris = Ephemeris::build(scene, None, target, Some(origin), horizon_s, progress)?;
    let radial = RadialLaunch::new(
        &ephemeris,
        ephemeris.index_of(origin)?,
        ephemeris.target,
        launch_altitude_m,
    )?;
    let launcher = |v_infinity: f64| {
        move |params: &[f64; 2]| -> Option<(DVec3, DVec3)> {
            Some(radial.state(v_infinity, params))
        }
    };

    let n = TRANSFER_AZIMUTH_SAMPLES;
    let step = std::f64::consts::TAU / n as f64;

    // Every launch direction at one speed, then each dip refined into a solution. Flights stop at
    // `cutoff`, the earliest arrival found so far: a path that arrives later cannot win, and
    // flying it to the end of a decades-long horizon is where nearly all the time went.
    let solve_at = |v_infinity: f64, cutoff: f64| -> Vec<Solution> {
        let launch = launcher(v_infinity);
        let scan: Vec<Option<Approach>> = (0..n)
            .map(|k| {
                if progress.is_cancelled() {
                    return None;
                }
                let (r, v) = launch(&[step * k as f64, 0.0])?;
                let flight = fly(
                    &ephemeris,
                    r,
                    v,
                    ephemeris.target,
                    (0.0, cutoff),
                    f64::INFINITY,
                    false,
                );
                if flight.crashed {
                    None
                } else {
                    flight.approach
                }
            })
            .collect();
        let distance = |k: usize| scan[k % n].map_or(f64::INFINITY, |a| a.distance_m);
        let mut dips: Vec<usize> = (0..n)
            .filter(|&k| {
                distance(k).is_finite()
                    && distance(k) <= distance(k + n - 1)
                    && distance(k) <= distance(k + 1)
            })
            .collect();
        dips.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)));
        dips.truncate(TRANSFER_CANDIDATES);
        dips.into_iter()
            .filter_map(|k| {
                let guess = scan[k]?;
                let window = (
                    0.5 * guess.time_s,
                    (1.5 * guess.time_s + ephemeris.step_s).min(cutoff),
                );
                refine(
                    &ephemeris,
                    &launch,
                    [step * k as f64, 0.0],
                    window,
                    step,
                    v_infinity,
                )
            })
            .collect()
    };

    // The speed ladder: the fastest arrival is usually at the cap, but not always — from a bad
    // geometry a slower departure can fall onto an orbit that meets the target sooner than any fast
    // one. Each rung is a full scan.
    let mut best: Option<Solution> = None;
    for fraction in TRANSFER_SPEED_FRACTIONS {
        let cutoff = best.map_or(f64::INFINITY, |b| b.approach.time_s);
        for candidate in solve_at(fraction * max_departure_speed_m_s, cutoff) {
            if candidate.approach.time_s < cutoff
                && best.is_none_or(|b| candidate.approach.time_s < b.approach.time_s)
            {
                best = Some(candidate);
            }
        }
    }
    let mut best = best?;

    // Then a hill climb on speed from the best rung, each trial seeded from the best solution's
    // angles, since a small change of speed moves the aim a little.
    let mut nudge = TRANSFER_SPEED_REFINE_FRACTION * max_departure_speed_m_s;
    for _ in 0..TRANSFER_SPEED_REFINE_ROUNDS {
        for v_infinity in [best.v_infinity_m_s - nudge, best.v_infinity_m_s + nudge] {
            if v_infinity <= 0.0 || v_infinity > max_departure_speed_m_s || progress.is_cancelled()
            {
                continue;
            }
            let window = (0.5 * best.approach.time_s, best.approach.time_s);
            let launch = launcher(v_infinity);
            if let Some(candidate) =
                refine(&ephemeris, &launch, best.params, window, step, v_infinity)
            {
                if candidate.approach.time_s < best.approach.time_s {
                    best = candidate;
                }
            }
        }
        nudge *= 0.5;
    }

    let (r, v) = launcher(best.v_infinity_m_s)(&best.params)?;
    let mut plan = finish(&ephemeris, r, v, best.window, best.approach);
    plan.departure_speed_m_s = best.v_infinity_m_s;
    Some(plan)
}

/// One refined departure: launch angles, the speed they were solved at, and the arrival.
#[derive(Copy, Clone)]
pub(super) struct Solution {
    pub(super) params: [f64; 2],
    pub(super) v_infinity_m_s: f64,
    pub(super) approach: Approach,
    pub(super) window: (f64, f64),
}

/// Newton's method from `start` on the two launch angles at a fixed speed.
pub(super) fn refine(
    ephemeris: &Ephemeris,
    launch: &Launcher<2>,
    start: [f64; 2],
    window: (f64, f64),
    scan_step: f64,
    v_infinity_m_s: f64,
) -> Option<Solution> {
    aim(
        ephemeris,
        start,
        [1.0e-6, 1.0e-6],
        [0.5 * scan_step, 0.5 * scan_step],
        &Goal::centre(ephemeris.target, window),
        None,
        launch,
    )
    .map(|(params, approach)| Solution {
        params,
        v_infinity_m_s,
        approach,
        window,
    })
}
