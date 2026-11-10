//! A spacecraft in flight in the live scene: its phase, its true state, and its running report.
//!
//! [`Flight`] wraps the core [`Voyage`], which keeps the path, the correction schedule, and the
//! touchdown, and adds what the spacecraft does: which burn it is on, and what it has cost.
//!
//! - A **coasting** craft is the scene's to move, like every planet; [`Flight`] only passes the
//!   voyage its correction burns.
//! - A **powered** craft is flown here, before every substep ([`Flight::steer`]), in `f64` and
//!   relative to its target. The scene cannot carry it: near a landing it moves centimetres per
//!   substep, and `f32` positions `150` units from the origin change in steps of `15 km`, so every
//!   one of those moves would round away. Kept relative to the target, the craft also rides along
//!   when the target's own `f32` position jumps, so the approach stays smooth. The scene's copy is
//!   then set so the scene's step ends where the craft really is, for drawing and for the path.

use bevy::math::DVec3;

use crate::constants::METRES_PER_UNIT;
use crate::core::integrator::gravitational_acceleration;
use crate::core::scene::{Body, BodyId, Scene};
use crate::core::transfer::{physical_radius_m, Flyby, TransferPlan, Voyage};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::spacecraft::config::{Propulsion, Spacecraft};
use crate::spacecraft::constants::{
    BURN_DONE_M_S, CORRECTION_FIRST_FRACTION, CORRECTION_MIN_REMAINING_S, CRUISE_STEP_FRACTION,
    DEPARTURE_BURN_STEPS, LANDING_START_BRAKING_DISTANCES, LANDING_START_RADII,
    POWERED_MIN_SUBSTEP_S,
};
use crate::spacecraft::guidance::{
    braking_m_s2, match_velocity, segment_entry_m, steer, Law, Sighting,
};
use crate::spacecraft::plan::FlightPlan;
use crate::spacecraft::report::FlightReport;
use crate::time::simulation::TimeStep;

/// What a flight is doing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Phase {
    /// A coasting craft, moved by the scene.
    Coasting,
    /// A direct flight, steered at the target the whole way.
    Direct,
    /// A transfer's departure burn: full acceleration in a fixed `direction` until `remaining_m_s`
    /// of speed change is spent, in steps of at most `step_s`, as the planner flew it.
    Departure {
        direction: DVec3,
        remaining_m_s: f64,
        step_s: f64,
    },
    /// A transfer coasting on its arc.
    Cruise,
    /// A transfer's correction burn, toward a corrected velocity (scene frame, m/s).
    Correction { goal_velocity_m_s: DVec3 },
    /// A transfer braking to land.
    Landing,
    /// On the target's surface.
    Landed,
}

impl Phase {
    /// What the phase is called on the status line.
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Coasting => "coasting",
            Phase::Direct => "under power",
            Phase::Departure { .. } => "departure burn",
            Phase::Cruise => "cruising",
            Phase::Correction { .. } => "correction burn",
            Phase::Landing => "braking to land",
            Phase::Landed => "landed",
        }
    }
}

/// Another body through one live substep: moving uniformly, pulling, and solid if it is a sphere.
#[derive(Copy, Clone, Debug)]
struct Neighbour {
    position_m: DVec3,
    velocity_m_s: DVec3,
    mu: f64,
    radius_m: f64,
    solid: bool,
}

/// A spacecraft in flight. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// The path, schedule, and touchdown.
    pub voyage: Voyage,
    /// The craft flying.
    pub craft: Spacecraft,
    phase: Phase,
    report: FlightReport,
    /// A powered craft's true position relative to the target, in metres.
    relative_m: DVec3,
    /// A powered craft's true velocity relative to the target, in m/s.
    relative_velocity_m_s: DVec3,
    /// When a transfer last planned a correction, on the scene's clock.
    last_correction_s: Option<f64>,
}

fn to_metres(units: DVec3) -> DVec3 {
    units * METRES_PER_UNIT
}

fn to_units(metres: DVec3) -> DVec3 {
    metres / METRES_PER_UNIT
}

impl Flight {
    /// Add `craft` to the scene at the start of `plan` and begin.
    pub fn launch(
        scene: &mut Scene,
        craft: Spacecraft,
        origin: BodyId,
        target: BodyId,
        plan: FlightPlan,
        now_s: f64,
    ) -> Self {
        let origin_velocity = scene
            .get(origin)
            .map_or(DVec3::ZERO, |prop| to_metres(prop.velocity_units_s));
        let (relative_m, relative_velocity_m_s) = scene.get(target).map_or_else(
            || (DVec3::ZERO, DVec3::ZERO),
            |target| {
                (
                    to_metres(plan.transfer.position_units - target.body.center()),
                    to_metres(plan.transfer.velocity_units_s - target.velocity_units_s),
                )
            },
        );
        let mut report = FlightReport::new(&craft.name, craft.engine().is_some());
        let phase = match (&craft.propulsion, plan.departure_burn_m_s) {
            (Propulsion::Coasting { .. }, _) => {
                // The booster's push, from rest relative to the origin to the launch velocity.
                report.record_impulse(
                    (to_metres(plan.transfer.velocity_units_s) - origin_velocity).length(),
                );
                Phase::Coasting
            }
            (Propulsion::Transfer { .. }, Some(burn)) => {
                let accel = craft.acceleration_m_s2().unwrap_or(f64::INFINITY);
                Phase::Departure {
                    direction: burn.normalize_or_zero(),
                    remaining_m_s: burn.length(),
                    step_s: burn.length() / accel / DEPARTURE_BURN_STEPS as f64,
                }
            }
            _ => Phase::Direct,
        };
        report.record_speed(to_metres(plan.transfer.velocity_units_s).length());
        let voyage = Voyage::launch(scene, craft.prop(), origin, target, plan.transfer, now_s);
        Flight {
            voyage,
            craft,
            phase,
            report,
            relative_m,
            relative_velocity_m_s,
            last_correction_s: None,
        }
    }

    /// What the flight is doing now.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The running report: complete once landed.
    pub fn report(&self) -> &FlightReport {
        &self.report
    }

    /// A powered craft's true position and velocity relative to its target, in metres and m/s
    /// (zero for a coasting craft, which the scene carries).
    pub fn relative_state(&self) -> (DVec3, DVec3) {
        (self.relative_m, self.relative_velocity_m_s)
    }

    /// Whether the craft is on the target's surface.
    pub fn has_landed(&self) -> bool {
        self.voyage.touchdown_offset_units.is_some()
    }

    /// When the next correction burn is due. A coasting craft keeps the voyage's schedule. A
    /// transfer, only while it cruises (never mid-burn or while landing), corrects first at
    /// [`CORRECTION_FIRST_FRACTION`] of its flight and then each time half of what was left has
    /// passed, until under [`CORRECTION_MIN_REMAINING_S`] is left: each correction removes the drift
    /// since the last, so halving the gap ends the cruise as close to the target as the last one
    /// can still be useful. `None` for a direct flight.
    pub fn next_correction_at(&self) -> Option<f64> {
        match self.phase {
            Phase::Coasting => self.voyage.next_correction_at(),
            Phase::Cruise => {
                let (launched, arrival) = (self.voyage.launched_at_s, self.voyage.arrival_at_s);
                let next = match self.last_correction_s {
                    None => launched + CORRECTION_FIRST_FRACTION * (arrival - launched),
                    Some(last) => last + 0.5 * (arrival - last),
                };
                (arrival - next >= CORRECTION_MIN_REMAINING_S).then_some(next)
            }
            _ => None,
        }
    }

    /// The next flyby still ahead of `now_s`, for a correction to aim at. See [`Voyage::next_flyby`].
    pub fn next_flyby(&self, now_s: f64) -> Option<Flyby> {
        self.voyage.next_flyby(now_s)
    }

    /// Put the craft on a correction from [`crate::core::transfer::plan_correction`], or count the
    /// slot as used when there is none. A coasting craft changes velocity at once; a transfer burns
    /// toward the new velocity at up to its acceleration.
    pub fn apply_correction(&mut self, scene: &mut Scene, fix: Option<TransferPlan>, now_s: f64) {
        let goal = fix.as_ref().map(|plan| to_metres(plan.velocity_units_s));
        let before = self.voyage.correction_delta_v_m_s;
        self.last_correction_s = Some(now_s);
        self.voyage.apply_correction(scene, fix, now_s);
        self.report.corrections = self.voyage.corrections;
        match (self.phase, goal) {
            (Phase::Coasting, _) => self
                .report
                .record_impulse(self.voyage.correction_delta_v_m_s - before),
            (Phase::Cruise, Some(goal_velocity_m_s)) => {
                self.phase = Phase::Correction { goal_velocity_m_s };
            }
            _ => {}
        }
    }

    /// Record one substep of the live scene: the path, the peak speed, and, for a coasting craft,
    /// touchdown. Called after the scene steps.
    pub fn observe(&mut self, scene: &Scene) {
        if self.phase == Phase::Coasting {
            self.voyage.observe(scene);
            if let Some(craft) = scene.get(self.voyage.craft) {
                self.report
                    .record_speed(to_metres(craft.velocity_units_s).length());
            }
        } else {
            self.voyage.record_path(scene);
        }
        if self.has_landed() {
            if self.report.touchdown_speed_m_s.is_none() {
                self.report.touchdown_speed_m_s = self.voyage.touchdown_speed_m_s;
            }
            self.phase = Phase::Landed;
        }
    }

    /// Note how long the flight took, at the time it is seen to have landed.
    pub fn finish(&mut self, now_s: f64) {
        self.report.travel_time_s = Some(now_s - self.voyage.launched_at_s);
    }

    /// Before one substep: fly a powered craft through it, and put it in the scene where it ends up.
    /// Meant as the controller of [`Scene::step_steered`]. Does nothing for a coasting craft, a
    /// landed one, or one whose craft or target is gone.
    pub fn steer(&mut self, scene: &mut Scene, substep: TimeStep) {
        if matches!(self.phase, Phase::Coasting | Phase::Landed) || self.has_landed() {
            return;
        }
        let (Some(accel), Some(engine)) = (self.craft.acceleration_m_s2(), self.craft.engine())
        else {
            return;
        };
        let exhaust = engine.exhaust_speed_m_s;
        let mass = self.craft.mass_kg;
        let (Some(craft_index), Some(target_index)) = (
            scene.iter().position(|(id, _)| id == self.voyage.craft),
            scene.iter().position(|(id, _)| id == self.voyage.target),
        ) else {
            return;
        };
        let softening = scene.softening_units;
        let softening_m = f64::from(softening) * METRES_PER_UNIT;
        let dt = substep.seconds();

        // Every other body as the scene's semi-implicit step will move it: from where it is, at
        // its velocity after this substep's gravity.
        let moving = |index: usize| {
            let prop = &scene.props[index];
            let pull = gravitational_acceleration(&scene.props, index, softening);
            (
                to_metres(prop.body.center()),
                to_metres(prop.velocity_units_s + pull * dt),
            )
        };
        let target_prop = &scene.props[target_index];
        let target_radius = physical_radius_m(target_prop);
        let target_mu = GRAVITATIONAL_CONSTANT * f64::from(target_prop.mass_kg);
        let (target_start, target_velocity) = moving(target_index);
        let others: Vec<Neighbour> = scene
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != craft_index && *index != target_index)
            .map(|(index, (_, prop))| {
                let (position_m, velocity_m_s) = moving(index);
                Neighbour {
                    position_m,
                    velocity_m_s,
                    mu: GRAVITATIONAL_CONSTANT * f64::from(prop.mass_kg),
                    radius_m: physical_radius_m(prop),
                    solid: matches!(prop.body, Body::Sphere { .. }),
                }
            })
            .collect();
        let gravity = |r: DVec3, tau: f64, target_at: Option<DVec3>| {
            let mut a = DVec3::ZERO;
            let mut pull = |centre: DVec3, mu: f64| {
                let offset = centre - r;
                let d2 = offset.length_squared();
                if mu > 0.0 && d2 > 0.0 {
                    a += offset / d2.sqrt() * (mu / (d2 + softening_m * softening_m));
                }
            };
            for other in &others {
                pull(other.position_m + other.velocity_m_s * tau, other.mu);
            }
            if let Some(centre) = target_at {
                pull(centre, target_mu);
            }
            a
        };
        let surface_gravity = if target_radius > 0.0 {
            target_mu / (target_radius * target_radius)
        } else {
            0.0
        };
        let braking = braking_m_s2(accel, surface_gravity);

        let (mut relative, mut relative_velocity) = (self.relative_m, self.relative_velocity_m_s);
        let mut tau = 0.0;
        while tau < dt && self.voyage.touchdown_offset_units.is_none() {
            let target_now = target_start + target_velocity * tau;
            let craft_now = target_now + relative;
            let craft_velocity = target_velocity + relative_velocity;
            let goal = Sighting {
                position_m: target_now,
                velocity_m_s: target_velocity,
                radius_m: target_radius,
            };
            let left = dt - tau;

            // A cruise turns into the landing once the craft is within a margin of its braking
            // distance, or within a few of the target's radii, whichever is further: there the
            // landing law takes over the aiming too, as terminal guidance, and removes what drift
            // the last correction left before the target's gravity amplifies it.
            if self.phase == Phase::Cruise {
                let surface_m = (relative.length() - target_radius).max(0.0);
                let braking_distance =
                    relative_velocity.length_squared() / (2.0 * braking.max(f64::MIN_POSITIVE));
                if surface_m
                    <= (LANDING_START_BRAKING_DISTANCES * braking_distance)
                        .max(LANDING_START_RADII * target_radius)
                {
                    self.phase = Phase::Landing;
                }
            }

            let (thrust, h) = match self.phase {
                Phase::Direct | Phase::Landing => {
                    let obstacles: Vec<Sighting> = others
                        .iter()
                        .filter(|other| other.solid)
                        .map(|other| Sighting {
                            position_m: other.position_m + other.velocity_m_s * tau,
                            velocity_m_s: other.velocity_m_s,
                            radius_m: other.radius_m,
                        })
                        .collect();
                    let law = Law {
                        acceleration_m_s2: accel,
                        braking_m_s2: braking,
                        may_speed_up: self.phase == Phase::Direct,
                        avoid: self.phase == Phase::Direct,
                    };
                    let s = steer(craft_now, craft_velocity, &goal, &obstacles, &law, left);
                    (s.acceleration_m_s2, s.substep_s)
                }
                Phase::Departure {
                    direction,
                    remaining_m_s,
                    step_s,
                } => {
                    // A micrometre per second left is rounding, not burn.
                    if remaining_m_s <= 1.0e-6 {
                        self.phase = Phase::Cruise;
                        continue;
                    }
                    let h = left.min(step_s).min(remaining_m_s / accel);
                    self.phase = Phase::Departure {
                        direction,
                        remaining_m_s: remaining_m_s - accel * h,
                        step_s,
                    };
                    (direction * accel, h)
                }
                Phase::Correction { goal_velocity_m_s } => {
                    if (goal_velocity_m_s - craft_velocity).length() <= BURN_DONE_M_S {
                        self.phase = Phase::Cruise;
                        continue;
                    }
                    let pull = gravity(craft_now, tau, Some(target_now));
                    let s = match_velocity(craft_velocity, goal_velocity_m_s, pull, accel, left);
                    (s.acceleration_m_s2, s.substep_s)
                }
                Phase::Cruise => {
                    // A step a small fraction of the shortest dynamical time √(d³/μ) to any body:
                    // about a minute just after leaving a planet, as long as the scene's own
                    // substep in deep space.
                    let dynamical = |d: f64, mu: f64| {
                        if mu > 0.0 {
                            (d.powi(3) / mu).sqrt()
                        } else {
                            f64::INFINITY
                        }
                    };
                    let shortest = others
                        .iter()
                        .map(|other| {
                            let at = other.position_m + other.velocity_m_s * tau;
                            dynamical((craft_now - at).length(), other.mu)
                        })
                        .fold(dynamical(relative.length(), target_mu), f64::min);
                    let h = (CRUISE_STEP_FRACTION * shortest).max(POWERED_MIN_SUBSTEP_S);
                    (DVec3::ZERO, left.min(h))
                }
                Phase::Coasting | Phase::Landed => break,
            };
            let h = h.min(left).max(f64::MIN_POSITIVE);
            self.report
                .record_burn(thrust.length(), h, mass, exhaust, accel);

            // The craft's acceleration relative to the target: thrust, plus the pull on the craft
            // (the target's included) less the pull on the target.
            let differential =
                gravity(craft_now, tau, Some(target_now)) - gravity(target_now, tau, None);
            relative_velocity += (thrust + differential) * h;
            let next = relative + relative_velocity * h;
            match segment_entry_m(relative, next, target_radius) {
                Some(entry) => {
                    relative = entry;
                    self.voyage.touchdown_offset_units = Some(to_units(entry));
                    let speed = relative_velocity.length();
                    self.voyage.touchdown_speed_m_s = Some(speed);
                    self.report.touchdown_speed_m_s = Some(speed);
                    relative_velocity = DVec3::ZERO;
                }
                None => relative = next,
            }
            tau += h;
            self.report
                .record_speed((target_velocity + relative_velocity).length());
        }
        self.relative_m = relative;
        self.relative_velocity_m_s = relative_velocity;

        // Put the scene's copy where the scene's own step will carry it to the true end state:
        // the step accelerates it by gravity, then moves it at the new velocity.
        let end_position = to_units(target_start + target_velocity * dt + relative);
        let end_velocity = to_units(target_velocity + relative_velocity);
        let craft = &mut scene.props[craft_index];
        if let Body::Sphere { pose, .. } | Body::Cuboid { pose, .. } | Body::Plane { pose, .. } =
            &mut craft.body
        {
            pose.position = end_position - end_velocity * dt;
        }
        let pull = gravitational_acceleration(&scene.props, craft_index, softening);
        scene.props[craft_index].velocity_units_s = end_velocity - pull * dt;
    }
}

#[cfg(test)]
#[path = "../../tst/spacecraft/flight_tests.rs"]
mod tests;
