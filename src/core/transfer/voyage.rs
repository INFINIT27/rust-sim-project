//! A craft in flight in the live scene: its correction schedule and its touchdown.

use super::*;

/// A craft in flight under the live scene: what it was told to do, what it has done, and
/// whether it has arrived.
///
/// Pure bookkeeping around the [`Scene`]: the craft is an ordinary [`Prop`], moved by
/// [`Scene::step`] like everything else. This only decides **when** to re-aim it (see
/// [`next_correction_at`](Self::next_correction_at)) and notices the substep it reaches the
/// target's surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Voyage {
    pub craft: BodyId,
    /// The body it left from.
    pub origin: BodyId,
    pub target: BodyId,
    /// Simulated time of launch, in seconds on the scene's clock.
    pub launched_at_s: f64,
    /// Simulated time the current plan reaches the target.
    pub arrival_at_s: f64,
    /// The path the current plan predicts, from the last burn on.
    pub planned_path_units: Vec<Vec3>,
    /// Where the craft has actually been.
    pub flown_path_units: Vec<Vec3>,
    /// Correction slots used, whether or not their burn converged.
    pub corrections: usize,
    /// The speed it left at, relative to the origin once clear of its gravity, in m/s.
    pub departure_speed_m_s: f64,
    /// Total speed change of every correction burn, in m/s.
    pub correction_delta_v_m_s: f64,
    /// Where on the target the craft touched down, as an offset from its centre in world units.
    pub touchdown_offset_units: Option<DVec3>,
    previous_offset_units: Option<DVec3>,
    /// The route's flybys, with `time_s` on the scene's clock rather than from the plan's start.
    pub flybys: Vec<Flyby>,
    /// The flybys as the launch plan had them, times from launch, for the status line's route.
    pub plan_flybys: Vec<Flyby>,
    last_correction_at_s: Option<f64>,
    path_spacing_units: f32,
    /// The craft's speed relative to the target at touchdown, in m/s, once it has landed.
    pub touchdown_speed_m_s: Option<f64>,
}

impl Voyage {
    /// Add `craft` to the scene at the plan's start, with the plan's velocity, and begin.
    ///
    /// The craft's own pose and velocity are overwritten; its shape, colour and mass are kept.
    pub fn launch(
        scene: &mut Scene,
        mut craft: Prop,
        origin: BodyId,
        target: BodyId,
        plan: TransferPlan,
        now_s: f64,
    ) -> Self {
        if let Body::Sphere { pose, .. } | Body::Cuboid { pose, .. } | Body::Plane { pose, .. } =
            &mut craft.body
        {
            *pose = Pose {
                position: plan.position_units,
                rotation: Quat::IDENTITY,
            };
        }
        craft.velocity_units_s = plan.velocity_units_s;
        let path_spacing_units = (scene.get(target).map_or(0.0, |target| {
            (target.body.center() - plan.position_units).length()
        }) / TRANSFER_PATH_POINTS) as f32;
        let craft = scene.add(craft);
        Voyage {
            origin,
            departure_speed_m_s: plan.departure_speed_m_s,
            path_spacing_units,
            craft,
            target,
            launched_at_s: now_s,
            arrival_at_s: now_s + plan.flight_seconds,
            flown_path_units: vec![plan.position_units.as_vec3()],
            planned_path_units: plan.path_units,
            corrections: 0,
            correction_delta_v_m_s: 0.0,
            touchdown_offset_units: None,
            previous_offset_units: None,
            flybys: absolute(&plan.flybys, now_s),
            plan_flybys: plan.flybys,
            last_correction_at_s: None,
            touchdown_speed_m_s: None,
        }
    }

    /// When the next correction burn is due, or `None` when every slot has been used.
    ///
    /// The first comes [`TRANSFER_FIRST_CORRECTION_S`] after launch, or at
    /// [`TRANSFER_FIRST_CORRECTION_FRACTION`] of a shorter flight, once the craft is clear of the
    /// departure arc, which is where the live integrator and the planner disagree most. The rest
    /// are at [`TRANSFER_CORRECTION_FRACTIONS`] of the way from launch to the current arrival, so
    /// they bunch up near the target, where a small error is a large fraction of its radius.
    ///
    /// A route with gravity assists also gets a burn shortly before each flyby, at
    /// [`TRANSFER_FLYBY_CORRECTION_LEADS_S`], and one just after, because a flyby multiplies whatever
    /// error the craft brings into it by thousands. The schedule is recomputed from the current
    /// plan each time, and the next burn is the first one after the last.
    pub fn next_correction_at(&self) -> Option<f64> {
        let flight = self.arrival_at_s - self.launched_at_s;
        let first = TRANSFER_FIRST_CORRECTION_S.min(TRANSFER_FIRST_CORRECTION_FRACTION * flight);
        let around_flybys = self.flybys.iter().flat_map(|flyby| {
            let leg = TRANSFER_FLYBY_LEAD_FRACTION * (flyby.time_s - self.launched_at_s);
            TRANSFER_FLYBY_CORRECTION_LEADS_S
                .iter()
                .map(move |lead| flyby.time_s - lead.min(leg))
                .chain([flyby.time_s + TRANSFER_FLYBY_CORRECTION_AFTER_S.min(leg)])
        });
        let after = self.last_correction_at_s.unwrap_or(self.launched_at_s);
        std::iter::once(self.launched_at_s + first)
            .chain(
                TRANSFER_CORRECTION_FRACTIONS
                    .iter()
                    .map(|f| self.launched_at_s + f * flight),
            )
            .chain(around_flybys)
            .filter(|&at| at > after && at < self.arrival_at_s)
            .min_by(f64::total_cmp)
    }

    /// The next flyby still ahead of `now_s`, with its time counted from `now_s`: what a correction
    /// burn now should aim for, if anything. See [`plan_correction`].
    pub fn next_flyby(&self, now_s: f64) -> Option<Flyby> {
        self.flybys
            .iter()
            .filter(|flyby| flyby.time_s > now_s)
            .min_by(|a, b| a.time_s.total_cmp(&b.time_s))
            .map(|flyby| Flyby {
                time_s: flyby.time_s - now_s,
                ..*flyby
            })
    }

    /// Put the craft on a correction plan from [`plan_correction`], or count the slot as used when
    /// there is none.
    pub fn apply_correction(&mut self, scene: &mut Scene, plan: Option<TransferPlan>, now_s: f64) {
        self.corrections += 1;
        self.last_correction_at_s = Some(now_s);
        let Some(plan) = plan else {
            return;
        };
        let Some(index) = scene.iter().position(|(id, _)| id == self.craft) else {
            return;
        };
        let burn = plan.velocity_units_s - scene.props[index].velocity_units_s;
        self.correction_delta_v_m_s += burn.length() * METRES_PER_UNIT;
        scene.props[index].velocity_units_s = plan.velocity_units_s;
        self.arrival_at_s = now_s + plan.flight_seconds;
        self.planned_path_units = plan.path_units;
        // Flybys already behind the craft are not in the new plan; keep them for the record.
        self.flybys.retain(|flyby| flyby.time_s <= now_s);
        self.flybys.extend(absolute(&plan.flybys, now_s));
    }

    /// Record one substep of the live scene: extend the flown path and look for touchdown.
    ///
    /// Touchdown is tested on the straight segment between consecutive substeps, relative to the
    /// target, so a craft that crosses the target's whole disc within one substep is still caught
    /// at the point it entered.
    pub fn observe(&mut self, scene: &Scene) {
        if self.touchdown_offset_units.is_some() {
            return;
        }
        let (Some(craft), Some(target)) = (scene.get(self.craft), scene.get(self.target)) else {
            return;
        };
        let craft_position = craft.body.center();
        let target_position = target.body.center();
        let offset = craft_position - target_position;
        let radius = physical_radius_m(target) / METRES_PER_UNIT;

        if let Some(previous) = self.previous_offset_units {
            if let Some(entry) = segment_entry(previous, offset, radius) {
                self.touchdown_offset_units = Some(entry);
                self.touchdown_speed_m_s = Some(
                    (craft.velocity_units_s - target.velocity_units_s).length() * METRES_PER_UNIT,
                );
                self.flown_path_units
                    .push((target_position + entry).as_vec3());
                return;
            }
        }
        self.previous_offset_units = Some(offset);
        self.record_point(craft_position);
    }

    /// Record one substep of the live scene on the flown path only, without looking for touchdown:
    /// for a craft whose controller finds its own touchdown more precisely than the scene's `f32`
    /// positions can (see `spacecraft::Flight`). It then sets `touchdown_offset_units` itself.
    pub fn record_path(&mut self, scene: &Scene) {
        if self.touchdown_offset_units.is_some() {
            return;
        }
        if let Some(craft) = scene.get(self.craft) {
            self.record_point(craft.body.center());
        }
    }

    fn record_point(&mut self, craft_position: DVec3) {
        let craft_position = craft_position.as_vec3();
        let moved = self
            .flown_path_units
            .last()
            .is_none_or(|last| (craft_position - *last).length() >= self.path_spacing_units);
        if moved {
            self.flown_path_units.push(craft_position);
        }
    }
}

/// The first point at which the segment `from → to` enters the sphere of `radius` about the
/// origin, or `None` if it never does.
pub(super) fn segment_entry(from: DVec3, to: DVec3, radius: f64) -> Option<DVec3> {
    if from.length() <= radius {
        return Some(from);
    }
    let d = to - from;
    let (a, b, c) = (
        d.dot(d),
        2.0 * from.dot(d),
        from.dot(from) - radius * radius,
    );
    let discriminant = b * b - 4.0 * a * c;
    if a <= 0.0 || discriminant < 0.0 {
        return None;
    }
    let s = (-b - discriminant.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&s).then(|| from + d * s)
}

/// `flybys` with their times moved from seconds after the plan starts to the scene's clock.
fn absolute(flybys: &[Flyby], plan_start_s: f64) -> Vec<Flyby> {
    flybys
        .iter()
        .map(|flyby| Flyby {
            time_s: plan_start_s + flyby.time_s,
            ..*flyby
        })
        .collect()
}
