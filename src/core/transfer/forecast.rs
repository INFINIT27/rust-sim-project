//! The planner's prediction and coasting solver, opened to the layers above `core`.
//!
//! The spacecraft layer decides *how* a craft flies (when it burns, how hard, toward what); this
//! module answers the orbital-mechanics questions it asks along the way: where will each body be,
//! what pulls on a point, and what launch velocity coasts from here to a target arriving around a
//! given time. [`Forecast`] wraps the [`Ephemeris`] the searches use, read-only, so the planner's
//! internals can change without breaking those callers.

use super::*;

/// The scene's predicted future, read-only: each body's position, velocity, gravity, and size at
/// any time up to the horizon. SI units throughout; bodies are addressed by index (see
/// [`Forecast::bodies`] and [`Forecast::index_of`]).
pub struct Forecast<'a> {
    ephemeris: Ephemeris<'a>,
}

impl<'a> Forecast<'a> {
    /// Predict `scene` for `horizon_s` seconds, without `leave_out`, with moon systems the flight
    /// from `origin` to `target` does not visit folded into their planets (as every plan does).
    /// `None` once `progress` is cancelled.
    pub fn new(
        scene: &Scene,
        leave_out: Option<BodyId>,
        target: BodyId,
        origin: Option<BodyId>,
        horizon_s: f64,
        progress: &'a PlanProgress,
    ) -> Option<Self> {
        Ephemeris::build(scene, leave_out, target, origin, horizon_s, progress)
            .map(|ephemeris| Forecast { ephemeris })
    }

    /// How far ahead the prediction reaches, in seconds.
    pub fn horizon_s(&self) -> f64 {
        self.ephemeris.horizon_s()
    }

    /// The scene id of every body in the prediction, by index.
    pub fn bodies(&self) -> &[BodyId] {
        &self.ephemeris.ids
    }

    /// The index of the body with scene id `id`, if it is in the prediction (a folded moon is not).
    pub fn index_of(&self, id: BodyId) -> Option<usize> {
        self.ephemeris.index_of(id)
    }

    /// The index of the destination the prediction was built for.
    pub fn target_index(&self) -> usize {
        self.ephemeris.target
    }

    /// The index of the most massive body: the Sun in the solar system, Jupiter in its moons.
    pub fn dominant_index(&self) -> usize {
        self.ephemeris.dominant()
    }

    /// Body `index`'s position at `t`, in metres.
    pub fn position_m(&self, index: usize, t: f64) -> DVec3 {
        self.ephemeris.position(index, t)
    }

    /// Body `index`'s velocity at `t`, in m/s: the rate of change of [`Forecast::position_m`], so a
    /// craft matching it really keeps pace (see `Ephemeris::position_rate`).
    pub fn velocity_m_s(&self, index: usize, t: f64) -> DVec3 {
        self.ephemeris.position_rate(index, t)
    }

    /// Body `index`'s `G · m`, in m³/s².
    pub fn mu_m3_s2(&self, index: usize) -> f64 {
        self.ephemeris.mu[index]
    }

    /// Body `index`'s physical radius, in metres (zero for anything but a sphere).
    pub fn radius_m(&self, index: usize) -> f64 {
        self.ephemeris.radius_m[index]
    }

    /// The gravitational acceleration at `position_m` and time `t`, in m/s², softened as the scene
    /// softens it.
    pub fn gravity_m_s2(&self, position_m: DVec3, t: f64) -> DVec3 {
        self.ephemeris.acceleration(position_m, t)
    }

    /// The scene's Plummer softening length, in metres: gravity at distance `d` from a body falls as
    /// `μ / (d² + ε²)`, not `μ / d²`.
    pub fn softening_m(&self) -> f64 {
        self.ephemeris.softening_m
    }
}

/// A coasting arc [`solve_coast`] or [`solve_coast_with`] found: the launch velocity, the predicted
/// flight, and how it arrives.
#[derive(Clone, Debug, PartialEq)]
pub struct Coast {
    /// The velocity solved for, in the scene's frame, in m/s: the launch velocity for
    /// [`solve_coast`], or whatever the launcher of [`solve_coast_with`] takes.
    pub velocity_m_s: DVec3,
    /// Where the coast starts, in metres, and how fast, in m/s: what the launcher made of
    /// `velocity_m_s`.
    pub start_m: DVec3,
    pub start_velocity_m_s: DVec3,
    /// The predicted flight from the start point to the target. `miss_m` is how far from the
    /// target's centre its closest approach is; a hit lands on the surface.
    pub plan: TransferPlan,
    /// The craft's velocity relative to the target as it arrives, in m/s: what a landing burn has
    /// to cancel.
    pub arrival_relative_velocity_m_s: DVec3,
}

/// The launch velocity, near `velocity_guess_m_s`, that coasts from `start_m` to `target`'s
/// centre, arriving within `arrival_window_s` (seconds from the start), under every body's
/// gravity in `forecast`.
///
/// The same Newton solve a correction burn uses, over all three velocity components, so it finds
/// the solution nearest the guess: a two-body estimate (a Lambert solution) is a good one. `None`
/// when it does not converge to within [`TRANSFER_AIM_TOLERANCE_M`] of the centre, or the
/// forecast's progress is cancelled.
pub fn solve_coast(
    forecast: &Forecast,
    start_m: DVec3,
    velocity_guess_m_s: DVec3,
    arrival_window_s: (f64, f64),
) -> Option<Coast> {
    solve_coast_with(
        forecast,
        velocity_guess_m_s,
        arrival_window_s,
        &|velocity| Some((start_m, velocity)),
    )
}

/// [`solve_coast`] for a launch that is not instantaneous: `launcher` turns the velocity being
/// solved for into the state the coast starts from (metres, m/s), and the solve finds the velocity,
/// near `velocity_guess_m_s`, whose coast reaches the target within `arrival_window_s`.
///
/// A burn at a few g out of a planet's gravity well takes minutes and climbs thousands of
/// kilometres, so a craft that ends it at the velocity an instant launch would need carries too
/// much energy. With a launcher that flies the burn, the solve aims the burn instead. The coast is
/// flown from `t = 0`, so the burn's own minutes are left out of the timing: a few hundred seconds
/// on arcs of days or months.
pub fn solve_coast_with(
    forecast: &Forecast,
    velocity_guess_m_s: DVec3,
    arrival_window_s: (f64, f64),
    launcher: &dyn Fn(DVec3) -> Option<(DVec3, DVec3)>,
) -> Option<Coast> {
    let ephemeris = &forecast.ephemeris;
    let velocity = |dv: &[f64; 3]| velocity_guess_m_s + DVec3::new(dv[0], dv[1], dv[2]);
    let launch = |dv: &[f64; 3]| launcher(velocity(dv));
    let goal = Goal::centre(ephemeris.target, arrival_window_s);
    let (dv, approach) = aim(
        ephemeris,
        [0.0; 3],
        [COAST_SOLVE_PROBE_M_S; 3],
        [COAST_SOLVE_MAX_STEP_M_S; 3],
        &goal,
        None,
        &launch,
    )?;
    let (r, v) = launch(&dv)?;
    let plan = finish(ephemeris, r, v, arrival_window_s, approach);
    Some(Coast {
        velocity_m_s: velocity(&dv),
        start_m: r,
        start_velocity_m_s: v,
        arrival_relative_velocity_m_s: approach.relative_velocity_m_s,
        plan,
    })
}
