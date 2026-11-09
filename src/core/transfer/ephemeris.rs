//! The predicted future a plan is flown through: [`Ephemeris`], the moon folding that keeps it
//! cheap, and the [`PlanProgress`] a long build checks.

use super::*;

/// The scene's predicted future, sampled at a fixed interval, with the craft left out.
///
/// Carries the search's [`PlanProgress`] so every flight through it can notice a cancel. A long
/// search spends minutes inside flights and Newton iterations, and a cancel that only the
/// ephemeris build checked was measured waiting `142 s` for them.
pub(super) struct Ephemeris<'a> {
    pub(super) progress: &'a PlanProgress,
    /// Seconds between samples: the horizon over [`TRANSFER_EPHEMERIS_SAMPLES`], so a six-hour hop
    /// to the Moon and a decade to Neptune cost the same memory.
    pub(super) step_s: f64,
    /// `states[k][i]` is body `i`'s position (m) and velocity (m/s) at `k · step_s`.
    pub(super) states: Vec<Vec<(DVec3, DVec3)>>,
    /// `G · m` per body, in m³/s². Zero for massless and immovable-massless props.
    pub(super) mu: Vec<f64>,
    /// Physical radius per sphere, in metres; zero for anything else.
    pub(super) radius_m: Vec<f64>,
    pub(super) softening_m: f64,
    /// The index of the destination.
    pub(super) target: usize,
    /// The scene id of each body, by index.
    pub(super) ids: Vec<BodyId>,
    /// Each body's Hill radius at the start, in metres: where its gravity outweighs its primary's
    /// tide, and so where a pass of it is a flyby. Infinite for the dominant body.
    pub(super) hill_m: Vec<f64>,
}

impl<'a> Ephemeris<'a> {
    /// Predict `scene` for `horizon_s`, without `leave_out`, with every moon system the flight
    /// does not visit folded into one body — see [`fold_distant_moons`].
    ///
    /// `None` as soon as `progress` is cancelled, which it checks every few hundred samples: a
    /// century's ephemeris takes long enough that a cancel must not wait for it to finish.
    pub(super) fn build(
        scene: &Scene,
        leave_out: Option<BodyId>,
        target: BodyId,
        origin: Option<BodyId>,
        horizon_s: f64,
        progress: &'a PlanProgress,
    ) -> Option<Self> {
        let mut future = scene.clone();
        if let Some(id) = leave_out {
            future.remove(id)?;
        }
        fold_distant_moons(&mut future, origin, target);
        let target = future.iter().position(|(id, _)| id == target)?;
        // The horizon over the sample count, but never so long that the fastest orbit left in
        // the prediction is under-sampled: past that, Hermite interpolation stops following the
        // curve and the step is capped instead, so a long horizon costs memory rather than truth.
        let step_s = (horizon_s / TRANSFER_EPHEMERIS_SAMPLES as f64)
            .min(TRANSFER_EPHEMERIS_ORBIT_FRACTION * shortest_dynamical_time_s(&future))
            .max(TRANSFER_MIN_EPHEMERIS_STEP_S);
        let step = TimeStep::from_seconds(step_s)?;
        let samples = (horizon_s / step_s).ceil() as usize + 1;

        let snapshot = |scene: &Scene| -> Vec<(DVec3, DVec3)> {
            scene
                .props
                .iter()
                .map(|prop| {
                    (
                        to_metres(prop.body.center()),
                        to_metres(prop.velocity_units_s),
                    )
                })
                .collect()
        };

        let ids: Vec<BodyId> = future.iter().map(|(id, _)| id).collect();
        let hill_m = hill_radii_m(&future);

        let mut states = Vec::with_capacity(samples);
        states.push(snapshot(&future));
        for sample in 1..samples {
            if sample % 256 == 0 && progress.is_cancelled() {
                return None;
            }
            // The gravitational half of `Scene::step`, substepped by the same rule, without the
            // contact half: hull tessellation is over nine tenths of a step's cost and no body in
            // a transfer scene touches another, so it would be paid for nothing.
            let substeps = crate::collision::resolve::substep_count(&future.props, step);
            let substep = step.split(substeps);
            for _ in 0..substeps {
                integrate_gravity(&mut future.props, substep, future.softening_units);
            }
            states.push(snapshot(&future));
        }

        Some(Ephemeris {
            progress,
            mu: future
                .props
                .iter()
                .map(|prop| GRAVITATIONAL_CONSTANT * f64::from(prop.mass_kg))
                .collect(),
            radius_m: future.props.iter().map(physical_radius_m).collect(),
            softening_m: f64::from(future.softening_units) * METRES_PER_UNIT,
            step_s,
            states,
            target,
            ids,
            hill_m,
        })
    }

    /// The index of the body with the most mass: the Sun, in the solar system.
    pub(super) fn dominant(&self) -> usize {
        (0..self.mu.len())
            .max_by(|&a, &b| self.mu[a].total_cmp(&self.mu[b]))
            .unwrap_or(0)
    }

    /// The index of the body with scene id `id`, if it is in the prediction.
    pub(super) fn index_of(&self, id: BodyId) -> Option<usize> {
        self.ids.iter().position(|&candidate| candidate == id)
    }

    /// Body `body`'s velocity at `t`, interpolated linearly between samples.
    pub(super) fn velocity(&self, body: usize, t: f64) -> DVec3 {
        let x = (t / self.step_s).clamp(0.0, (self.states.len() - 1) as f64);
        let k = (x.floor() as usize).min(self.states.len() - 2);
        self.states[k][body]
            .1
            .lerp(self.states[k + 1][body].1, x - k as f64)
    }

    /// How fast body `body`'s interpolated [`position`](Self::position) is changing at `t`: the
    /// derivative of the same cubic, in m/s.
    ///
    /// Not [`velocity`](Self::velocity). The stored velocities come from the integrator, which
    /// updates velocity before position, so they lead the path the positions trace by about half a
    /// step's worth of acceleration. A steering law that matches a target's velocity has to match
    /// how its position actually moves, or it closes on a point that keeps receding: a powered
    /// landing on the Moon stalled `1.5 km` up, closing at the `25 m/s` it was told to.
    pub(super) fn position_rate(&self, body: usize, t: f64) -> DVec3 {
        let x = (t / self.step_s).clamp(0.0, (self.states.len() - 1) as f64);
        let k = (x.floor() as usize).min(self.states.len() - 2);
        let s = x - k as f64;
        let ((p0, v0), (p1, v1)) = (self.states[k][body], self.states[k + 1][body]);
        let s2 = s * s;
        (p0 * (6.0 * s2 - 6.0 * s) + p1 * (6.0 * s - 6.0 * s2)) / self.step_s
            + v0 * (3.0 * s2 - 4.0 * s + 1.0)
            + v1 * (3.0 * s2 - 2.0 * s)
    }

    pub(super) fn horizon_s(&self) -> f64 {
        (self.states.len() - 1) as f64 * self.step_s
    }

    /// Body `body`'s position at `t`, by cubic Hermite interpolation between the samples either
    /// side. Using the velocities as well as the positions makes the error fall as the fourth
    /// power of the step, which is what lets one sample a day still put Earth where it is to well
    /// under a kilometre.
    pub(super) fn position(&self, body: usize, t: f64) -> DVec3 {
        let x = (t / self.step_s).clamp(0.0, (self.states.len() - 1) as f64);
        let k = (x.floor() as usize).min(self.states.len() - 2);
        let s = x - k as f64;
        let ((p0, v0), (p1, v1)) = (self.states[k][body], self.states[k + 1][body]);
        let (s2, s3) = (s * s, s * s * s);
        p0 * (2.0 * s3 - 3.0 * s2 + 1.0)
            + v0 * (self.step_s * (s3 - 2.0 * s2 + s))
            + p1 * (3.0 * s2 - 2.0 * s3)
            + v1 * (self.step_s * (s3 - s2))
    }

    /// The gravitational acceleration at `r` and time `t`, in m/s², with the scene's Plummer
    /// softening — the same law `gravitational_acceleration` applies to a prop.
    pub(super) fn acceleration(&self, r: DVec3, t: f64) -> DVec3 {
        let eps2 = self.softening_m * self.softening_m;
        let mut a = DVec3::ZERO;
        for (body, &mu) in self.mu.iter().enumerate() {
            if mu <= 0.0 {
                continue;
            }
            let offset = self.position(body, t) - r;
            let d2 = offset.length_squared();
            if d2 > 0.0 {
                a += offset / d2.sqrt() * (mu / (d2 + eps2));
            }
        }
        a
    }
}

/// The shortest local dynamical time `√(r³ / G(m + M))` of any body about what it orbits, in
/// seconds: its moon's primary when it is a moon, the scene's most massive body otherwise.
/// `f64::INFINITY` for a scene with nothing in orbit.
pub(super) fn shortest_dynamical_time_s(scene: &Scene) -> f64 {
    let Some((dominant, _)) = scene
        .iter()
        .filter(|(_, prop)| !prop.immovable)
        .max_by(|(_, a), (_, b)| a.mass_kg.total_cmp(&b.mass_kg))
    else {
        return f64::INFINITY;
    };
    scene
        .iter()
        .filter(|(id, prop)| *id != dominant && !prop.immovable && prop.mass_kg > 0.0)
        .filter_map(|(id, prop)| {
            let primary = moon_primary(scene, id).map_or(dominant, |(primary, _)| primary);
            let primary_prop = scene.get(primary)?;
            let r = to_metres(prop.body.center() - primary_prop.body.center()).length();
            let mu = GRAVITATIONAL_CONSTANT * f64::from(prop.mass_kg + primary_prop.mass_kg);
            (r > 0.0 && mu > 0.0).then(|| (r.powi(3) / mu).sqrt())
        })
        .fold(f64::INFINITY, f64::min)
}

/// Each body's Hill radius `a · (m / 3M)^⅓` about what it orbits (its moon primary, else the
/// dominant body), in metres. Infinite for the dominant body and for massless ones' primaries.
fn hill_radii_m(scene: &Scene) -> Vec<f64> {
    let dominant = scene
        .iter()
        .filter(|(_, prop)| !prop.immovable)
        .max_by(|(_, a), (_, b)| a.mass_kg.total_cmp(&b.mass_kg))
        .map(|(id, _)| id);
    scene
        .iter()
        .map(|(id, prop)| {
            let Some(dominant) = dominant.filter(|&d| d != id && prop.mass_kg > 0.0) else {
                return f64::INFINITY;
            };
            let primary = moon_primary(scene, id).map_or(dominant, |(primary, _)| primary);
            let Some(primary_prop) = scene.get(primary) else {
                return f64::INFINITY;
            };
            let a = to_metres(prop.body.center() - primary_prop.body.center()).length();
            a * (f64::from(prop.mass_kg) / (3.0 * f64::from(primary_prop.mass_kg))).cbrt()
        })
        .collect()
}

/// A departure search's progress, shared with whoever is waiting for it.
///
/// The search has no time limit — it doubles its horizon until a path turns up — so the waiter
/// needs two things: to see how far out it has got, and to stop it. Atomics rather than a
/// channel because both are a single number read or written whenever convenient.
#[derive(Debug, Default)]
pub struct PlanProgress {
    pub(super) cancelled: AtomicBool,
    pub(super) horizon_bits: AtomicU64,
}

impl PlanProgress {
    /// Ask the search to stop. It returns `None` at its next check.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Whether [`cancel`](Self::cancel) has been called.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// The horizon the search is working on now, in seconds; `0.0` before it starts.
    pub fn horizon_s(&self) -> f64 {
        f64::from_bits(self.horizon_bits.load(Ordering::Relaxed))
    }

    pub(super) fn set_horizon_s(&self, horizon_s: f64) {
        self.horizon_bits
            .store(horizon_s.to_bits(), Ordering::Relaxed);
    }
}

/// Fold every moon whose system the flight does not visit into its primary, conserving mass and
/// momentum: the primary becomes one body at the system's barycentre, moving at its velocity.
///
/// From outside a moon system its pull is that of its total mass at its barycentre, to within the
/// tidal term, so for a craft that never enters it nothing changes. What does change is the cost:
/// Io orbits in `1.77` days and forces the integrator to substep every `170 s`, which for a decade's
/// prediction is tens of millions of substeps spent on a body the craft passes `4 AU` away from.
/// The live scene still has every moon, and the correction burns absorb the difference.
///
/// Kept: the origin and target themselves, the origin's own moons (the craft leaves through them),
/// and the siblings of an origin or target that is itself a moon. So a departure from Earth keeps
/// the Moon, a hop from Io to Europa keeps all four Galilean moons, and Earth to Jupiter folds them:
/// the craft aims at Jupiter's centre, and the correction burns see the moons as it closes.
pub(super) fn fold_distant_moons(scene: &mut Scene, origin: Option<BodyId>, target: BodyId) {
    let primary_of =
        |scene: &Scene, id: BodyId| moon_primary(scene, id).map(|(primary, _)| primary);
    let involved: Vec<BodyId> = origin.into_iter().chain([target]).collect();
    let systems: Vec<BodyId> = origin
        .into_iter()
        .chain(involved.iter().filter_map(|&id| primary_of(scene, id)))
        .collect();

    let folds: Vec<(BodyId, BodyId)> = scene
        .iter()
        .map(|(id, _)| id)
        .filter(|id| !involved.contains(id))
        .filter_map(|id| primary_of(scene, id).map(|primary| (id, primary)))
        .filter(|(_, primary)| !systems.contains(primary))
        .collect();

    for (moon_id, primary_id) in folds {
        let Some(moon) = scene.remove(moon_id) else {
            continue;
        };
        let Some(index) = scene.iter().position(|(id, _)| id == primary_id) else {
            continue;
        };
        let primary = &mut scene.props[index];
        let moon_mass = f64::from(moon.mass_kg);
        let total = moon_mass + f64::from(primary.mass_kg);
        let weight = (moon_mass / total) as f32;
        let shift = (moon.body.center() - primary.body.center()) * weight;
        primary.velocity_units_s += (moon.velocity_units_s - primary.velocity_units_s) * weight;
        primary.body.translate(shift);
        primary.mass_kg = total as f32;
    }
}
