//! Flying a spacecraft between any two bodies in the running scene.
//!
//! `R` (see [`crate::input::mission`]) with the crosshair on a body picks it as the **origin**. A
//! second `R` on another body picks the **destination** and launches. `R` on the origin again
//! cancels. Picking uses the same crosshair test as the camera lock (`F`), so whatever `F` would
//! lock onto, `R` can fly between, including objects placed with `B`. `G` selects the next
//! spacecraft in the [`Mission`]'s [`Fleet`]; the crewed least-energy transfer is the default.
//!
//! A flight then goes:
//!
//! 1. **Plan.** [`plan_flight`] runs on a background task against a snapshot of the scene, with
//!    the simulation clock paused. It takes milliseconds for a direct flight, seconds for a
//!    transfer, and can take minutes for a coasting probe to an outer planet; `R` cancels it. The
//!    snapshot is then still the present when the plan comes back, and the camera stays live.
//! 2. **Fly.** [`step_scene_system`](crate::render::plugin::step_scene_system) hands every substep
//!    to [`Mission::steer`] before the scene steps (a powered craft is flown there, see
//!    [`Flight::steer`]) and to [`Mission::observe`] after (the path, the report, touchdown). A
//!    flight shorter than [`MISSION_MIN_WATCH_S`] at the current clock slows the clock to that, and
//!    the old scale comes back on arrival.
//! 3. **Correct.** At the times [`Flight::next_correction_at`] gives, [`plan_correction_for`] re-aims
//!    from where the craft really is, again on a paused clock. A coasting craft takes the burn at
//!    once; a transfer burns toward it at up to its acceleration.
//! 4. **Land.** At touchdown the craft leaves the scene, a ring rides on the target's surface where
//!    it came down, and the flight's [`FlightReport`](crate::spacecraft::FlightReport) is complete
//!    for the arrival pop-up ([`crate::ui::arrival`]). A craft is far below what `f32` can add to any
//!    body it lands on, so the landing is an inelastic merge with the arithmetic left out.

use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::color::{Alpha, Color as BevyColor, LinearRgba};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Query, Res, ResMut, Resource};
use bevy::gizmos::config::GizmoConfigStore;
use bevy::gizmos::gizmos::Gizmos;
use bevy::math::{Dir3, Vec3};
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use bevy::transform::components::Transform;

use crate::core::scene::{BodyId, Scene};
use crate::core::transfer::{Flyby, PlanProgress, TransferPlan};
use crate::experiments::catalog::{BodyNames, ExperimentSpec};
use crate::input::camera::{pick_lock_target, CameraTarget};
use crate::input::mission::LaunchRequest;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::render::constants::{
    MISSION_MARKER_ANGULAR_RADIUS, MISSION_MIN_WATCH_S, MISSION_PLANNED_PATH_ALPHA,
};
use crate::render::plugin::MirroredBody;
use crate::spacecraft::{
    plan_correction_for, plan_flight, Fleet, Flight, FlightPlan, NoFlight, Phase, Propulsion,
    Spacecraft,
};
use crate::time::simulation::{SimulationClock, TimeStep};

type PlanTask = Task<Result<FlightPlan, NoFlight>>;
type CorrectionTask = Task<Option<TransferPlan>>;

/// Where the current spacecraft is in its journey.
#[derive(Default)]
pub enum MissionPhase {
    /// No craft, and no origin picked.
    #[default]
    Idle,
    /// The origin is picked; the next `R` on another body picks the destination.
    Choosing { origin: BodyId },
    /// Both ends are picked; the plan starts this frame.
    Requested { origin: BodyId, target: BodyId },
    /// A flight is being planned; the clock is paused until it returns. `progress` is how a long
    /// search shows its horizon and how `R` stops it.
    Planning {
        origin: BodyId,
        target: BodyId,
        task: PlanTask,
        progress: Arc<PlanProgress>,
        was_paused: bool,
        /// The craft being planned for, selected when the plan started.
        craft: Spacecraft,
    },
    /// The craft is flying.
    Cruise(Flight),
    /// A correction burn is being planned; the clock is paused until it returns.
    Correcting {
        flight: Flight,
        task: CorrectionTask,
        was_paused: bool,
    },
    /// The craft is on the target's surface.
    Landed { flight: Flight, landed_after_s: f64 },
    /// There is no plan, and `reason` says why.
    NoPath {
        origin: BodyId,
        target: BodyId,
        reason: NoFlight,
    },
}

/// The spacecraft, if any, what it is doing, and the fleet the next one is chosen from.
#[derive(Resource, Default)]
pub struct Mission {
    pub phase: MissionPhase,
    /// The clock scale before a short flight slowed it, to put back on arrival.
    pub restore_scale: Option<f32>,
    /// The spacecraft `R` can launch; `G` selects the next. Built-in by default (see
    /// [`Fleet::default`]); anything that edits it, such as a settings menu, changes the next launch.
    pub fleet: Fleet,
}

impl Mission {
    /// A mission with nothing flying and `fleet` to launch from, such as the fleet in
    /// `config/spacecraft.toml`.
    pub fn with_fleet(fleet: Fleet) -> Self {
        Self {
            fleet,
            ..Self::default()
        }
    }

    /// Show the mission one substep of the live scene, after it steps. Called by
    /// `step_scene_system`.
    pub fn observe(&mut self, scene: &Scene) {
        if let MissionPhase::Cruise(flight) = &mut self.phase {
            flight.observe(scene);
        }
    }

    /// Fly a powered craft through one substep of the live scene, before it steps. Called by
    /// `step_scene_system` through [`Scene::step_steered`]; does nothing for a coasting one.
    pub fn steer(&mut self, scene: &mut Scene, substep: TimeStep) {
        if let MissionPhase::Cruise(flight) = &mut self.phase {
            flight.steer(scene, substep);
        }
    }

    /// The flight in progress or just landed, if any.
    pub fn flight(&self) -> Option<&Flight> {
        match &self.phase {
            MissionPhase::Cruise(flight)
            | MissionPhase::Correcting { flight, .. }
            | MissionPhase::Landed { flight, .. } => Some(flight),
            _ => None,
        }
    }

    /// Whether `R` picks a body right now. False while a craft is being planned or flown, so a
    /// stray press cannot abandon a flight halfway.
    pub fn is_picking(&self) -> bool {
        matches!(
            self.phase,
            MissionPhase::Idle
                | MissionPhase::Choosing { .. }
                | MissionPhase::Landed { .. }
                | MissionPhase::NoPath { .. }
        )
    }

    /// Advance the pick with the body `R` was pressed on.
    pub fn pick(&mut self, body: BodyId) {
        self.phase = match std::mem::take(&mut self.phase) {
            MissionPhase::Choosing { origin } if origin == body => MissionPhase::Idle,
            MissionPhase::Choosing { origin } => MissionPhase::Requested {
                origin,
                target: body,
            },
            _ => MissionPhase::Choosing { origin: body },
        };
    }

    /// One line for the overlay describing the mission, or `None` before anything is picked.
    pub fn status_line(&self, now_s: f64, names: &BodyNames) -> Option<String> {
        let route = |origin: BodyId, target: BodyId| {
            format!("{} → {}", names.label(origin), names.label(target))
        };
        // The whole route a flight flies, with each planet it passes for an assist in between.
        let flight_route = |flight: &Flight| {
            std::iter::once(flight.voyage.origin)
                .chain(flight.voyage.plan_flybys.iter().map(|flyby| flyby.body))
                .chain([flight.voyage.target])
                .map(|id| names.label(id))
                .collect::<Vec<_>>()
                .join(" → ")
        };
        let days = span;
        let flying = |flight: &Flight, doing: &str| {
            let voyage = &flight.voyage;
            let report = flight.report();
            let progress = format!(
                "{} of {}",
                days(now_s - voyage.launched_at_s),
                days(voyage.arrival_at_s - voyage.launched_at_s)
            );
            match flight.phase() {
                Phase::Coasting => format!(
                    "rocket {}: {doing}, {progress}, left at {:.1} km/s, {} burns ({:.1} m/s)",
                    flight_route(flight),
                    voyage.departure_speed_m_s / 1_000.0,
                    voyage.corrections,
                    voyage.correction_delta_v_m_s
                ),
                phase => format!(
                    "rocket {}: {}, {progress}, Δv {:.2} km/s so far, peak {:.1} km/s ({:.2}% of c)",
                    flight_route(flight),
                    if doing == "planning a correction" {
                        doing
                    } else {
                        phase.label()
                    },
                    report.delta_v_m_s / 1_000.0,
                    report.max_speed_m_s / 1_000.0,
                    100.0 * report.max_speed_m_s / SPEED_OF_LIGHT_M_S
                ),
            }
        };
        let selected = &self.fleet.selected().name;
        Some(match &self.phase {
            MissionPhase::Idle => return None,
            MissionPhase::Choosing { origin } => format!(
                "rocket from {} ({selected}; G switches): aim at the destination and press R (R on \
                 {0} cancels)",
                names.label(*origin)
            ),
            MissionPhase::Requested { origin, target } => {
                format!(
                    "rocket {}: planning the departure…",
                    route(*origin, *target)
                )
            }
            MissionPhase::Planning {
                origin,
                target,
                progress,
                craft,
                ..
            } => match craft.propulsion {
                Propulsion::Coasting { .. } => format!(
                    "rocket {}: searching flights of up to {} (R cancels)",
                    route(*origin, *target),
                    span(progress.horizon_s())
                ),
                _ => format!(
                    "rocket {}: planning the {} flight (R cancels)",
                    route(*origin, *target),
                    craft.name
                ),
            },
            MissionPhase::Cruise(flight) => flying(flight, "cruising"),
            MissionPhase::Correcting { flight, .. } => flying(flight, "planning a correction"),
            MissionPhase::Landed {
                flight,
                landed_after_s,
            } => match flight.report().touchdown_speed_m_s {
                Some(touchdown) if flight.craft.engine().is_some() => format!(
                    "rocket {}: landed after {} at {:.0} m/s, Δv {:.2} km/s",
                    flight_route(flight),
                    days(*landed_after_s),
                    touchdown,
                    flight.report().delta_v_m_s / 1_000.0
                ),
                _ => format!(
                    "rocket {}: landed after {}, {} burns ({:.1} m/s)",
                    flight_route(flight),
                    days(*landed_after_s),
                    flight.voyage.corrections,
                    flight.voyage.correction_delta_v_m_s
                ),
            },
            MissionPhase::NoPath {
                origin,
                target,
                reason,
            } => format!(
                "rocket {}: {}",
                route(*origin, *target),
                why_not(reason, names, *origin, *target)
            ),
        })
    }
}

/// Why there is no flight, for the status line.
fn why_not(reason: &NoFlight, names: &BodyNames, origin: BodyId, target: BodyId) -> String {
    const AU_M: f64 = 1.495_978_707e11;
    match reason {
        NoFlight::OutOfReach(reach) => format!(
            "out of reach: no direct path, and no gravity-assist route within 12 years. Leaving {} \
             at the cap, a rocket coasts out to {:.2} AU at most, and {} never comes inside \
             {:.2} AU. A direct path needs about {:.1} km/s",
            names.label(origin),
            reach.reach_m / AU_M,
            names.label(target),
            reach.target_closest_m / AU_M,
            reach.needed_speed_m_s / 1_000.0
        ),
        NoFlight::InvalidRoute => {
            format!("{} has no surface to launch from", names.label(origin))
        }
        NoFlight::Cancelled => "cancelled".to_string(),
        NoFlight::Blocked(body) => format!(
            "no powered path: {} is in the way even when steering around it",
            names.label(*body)
        ),
        NoFlight::NoArrival => "no powered path: the rocket never settles onto the target".into(),
        NoFlight::TooMuchGravity {
            body,
            gravity_m_s2,
            acceleration_m_s2,
        } => format!(
            "{}'s surface gravity is {:.1} g, more than this craft's {:.1} g can lift off from or land \
             against (G picks another craft)",
            names.label(*body),
            gravity_m_s2 / STANDARD_GRAVITY_M_S2,
            acceleration_m_s2 / STANDARD_GRAVITY_M_S2
        ),
        NoFlight::NoArc => format!(
            "no coasting arc to {} converged from where the planets are now (G picks another craft)",
            names.label(target)
        ),
    }
}

/// A duration for the status line: hours under two days, days under two years, then years.
pub fn span(seconds: f64) -> String {
    const DAY_S: f64 = 86_400.0;
    const YEAR_S: f64 = 365.25 * DAY_S;
    if seconds < 2.0 * DAY_S {
        format!("{:.1} h", seconds / 3_600.0)
    } else if seconds < 2.0 * YEAR_S {
        format!("{:.0} d", seconds / DAY_S)
    } else {
        format!("{:.1} y", seconds / YEAR_S)
    }
}

/// Bevy system: `G` selects the next spacecraft, and an `R` press picks the body nearest the
/// crosshair as the origin or destination, or cancels a plan that is still being searched for.
///
/// Presses with nothing near the crosshair, or while a craft is flying, are dropped.
pub fn pick_route_system(
    mut request: ResMut<LaunchRequest>,
    mut mission: ResMut<Mission>,
    camera: Query<&Transform, With<Camera3d>>,
    bodies: Query<(Entity, &Transform, &MirroredBody), With<CameraTarget>>,
) {
    if request.take_engine_switch() {
        mission.fleet.select_next();
    }
    if !request.take() {
        return;
    }
    if let MissionPhase::Planning { progress, .. } = &mission.phase {
        progress.cancel();
        return;
    }
    if !mission.is_picking() {
        return;
    }
    let Ok(eye) = camera.get_single() else {
        return;
    };
    let candidates = bodies
        .iter()
        .map(|(entity, transform, _)| (entity, transform.translation));
    let picked = pick_lock_target(eye.translation, eye.forward().as_vec3(), candidates)
        .and_then(|entity| bodies.get(entity).ok())
        .map(|(_, _, body)| body.0);
    if let Some(body) = picked {
        mission.pick(body);
    }
}

/// Bevy system that advances the mission by one frame: start or finish a plan, schedule a
/// correction, or land.
///
/// Runs after `step_scene_system`, so every plan is made from, and applied to, the state the next
/// frame will step from. See `MissionPlugin` for why the other order launches from the wrong place.
pub fn mission_system(
    spec: Res<ExperimentSpec>,
    mut scene: ResMut<Scene>,
    mut clock: ResMut<SimulationClock>,
    mut mission: ResMut<Mission>,
) {
    if !spec.launches {
        return;
    }
    let now = clock.elapsed_seconds();
    let Mission {
        phase,
        restore_scale,
        fleet,
    } = &mut *mission;

    *phase = match std::mem::take(phase) {
        MissionPhase::Requested { origin, target } => {
            let progress = Arc::new(PlanProgress::default());
            let craft = fleet.selected().clone();
            MissionPhase::Planning {
                origin,
                target,
                task: spawn_plan(&scene, craft.clone(), origin, target, Arc::clone(&progress)),
                progress,
                was_paused: pause(&mut clock),
                craft,
            }
        }
        MissionPhase::Planning {
            origin,
            target,
            mut task,
            progress,
            was_paused,
            craft,
        } => match block_on(future::poll_once(&mut task)) {
            None => MissionPhase::Planning {
                origin,
                target,
                task,
                progress,
                was_paused,
                craft,
            },
            Some(plan) => {
                clock.set_paused(was_paused);
                match plan {
                    Ok(plan) => {
                        *restore_scale = slow_for(&mut clock, plan.transfer.flight_seconds);
                        MissionPhase::Cruise(Flight::launch(
                            &mut scene, craft, origin, target, plan, now,
                        ))
                    }
                    Err(NoFlight::Cancelled) => MissionPhase::Idle,
                    Err(reason) => MissionPhase::NoPath {
                        origin,
                        target,
                        reason,
                    },
                }
            }
        },
        MissionPhase::Cruise(mut flight) if flight.has_landed() => {
            flight.finish(now);
            scene.remove(flight.voyage.craft);
            restore(&mut clock, restore_scale);
            MissionPhase::Landed {
                landed_after_s: now - flight.voyage.launched_at_s,
                flight,
            }
        }
        MissionPhase::Cruise(flight)
            if scene.get(flight.voyage.craft).is_none()
                || scene.get(flight.voyage.target).is_none() =>
        {
            scene.remove(flight.voyage.craft);
            restore(&mut clock, restore_scale);
            MissionPhase::Idle
        }
        MissionPhase::Cruise(flight) if flight.next_correction_at().is_some_and(|at| now >= at) => {
            MissionPhase::Correcting {
                task: spawn_correction(
                    &scene,
                    flight.craft.clone(),
                    flight.voyage.craft,
                    flight.voyage.target,
                    flight.voyage.arrival_at_s - now,
                    flight.next_flyby(now),
                ),
                was_paused: pause(&mut clock),
                flight,
            }
        }
        MissionPhase::Correcting {
            mut flight,
            mut task,
            was_paused,
        } => match block_on(future::poll_once(&mut task)) {
            None => MissionPhase::Correcting {
                flight,
                task,
                was_paused,
            },
            Some(fix) => {
                clock.set_paused(was_paused);
                flight.apply_correction(&mut scene, fix, now);
                MissionPhase::Cruise(flight)
            }
        },
        other => other,
    };
}

/// Pause the clock for a plan, returning whether it was already paused so it can be restored.
fn pause(clock: &mut SimulationClock) -> bool {
    let was_paused = clock.is_paused();
    clock.set_paused(true);
    was_paused
}

/// Slow the clock so a flight of `flight_s` simulated seconds lasts at least
/// [`MISSION_MIN_WATCH_S`] real ones, returning the scale to restore, if it changed.
fn slow_for(clock: &mut SimulationClock, flight_s: f64) -> Option<f32> {
    let scale = clock.scale();
    let slowest = (flight_s / MISSION_MIN_WATCH_S) as f32;
    (slowest < scale && slowest > 0.0).then(|| {
        clock.set_scale(slowest);
        scale
    })
}

fn restore(clock: &mut SimulationClock, restore_scale: &mut Option<f32>) {
    if let Some(scale) = restore_scale.take() {
        clock.set_scale(scale);
    }
}

fn spawn_plan(
    scene: &Scene,
    craft: Spacecraft,
    origin: BodyId,
    target: BodyId,
    progress: Arc<PlanProgress>,
) -> PlanTask {
    let scene = scene.clone();
    AsyncComputeTaskPool::get()
        .spawn(async move { plan_flight(&scene, origin, target, &craft, &progress) })
}

fn spawn_correction(
    scene: &Scene,
    craft: Spacecraft,
    craft_id: BodyId,
    target: BodyId,
    expected_arrival_s: f64,
    via: Option<Flyby>,
) -> CorrectionTask {
    let scene = scene.clone();
    AsyncComputeTaskPool::get().spawn(async move {
        plan_correction_for(
            &scene,
            &craft,
            craft_id,
            target,
            expected_arrival_s,
            via.as_ref(),
        )
    })
}

/// Bevy system that draws the mission: a ring on a picked origin, the craft's path while it flies
/// (planned path faint, flown path solid), and a ring where it landed.
pub fn draw_mission_system(
    mission: Res<Mission>,
    spec: Res<ExperimentSpec>,
    scene: Res<Scene>,
    camera: Query<&Transform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    if !spec.launches {
        return;
    }
    let c = mission
        .flight()
        .map_or(mission.fleet.selected().color, |flight| flight.craft.color);
    let color = BevyColor::LinearRgba(LinearRgba::new(c.r, c.g, c.b, c.a));
    let eye = camera.get_single().ok().map(|eye| eye.translation);
    let mut ring = |spot: Vec3| {
        if let Some(eye) = eye {
            let radius = (spot - eye).length() * MISSION_MARKER_ANGULAR_RADIUS;
            let facing = Dir3::new(eye - spot).unwrap_or(Dir3::Z);
            gizmos.circle(spot, facing, radius, color);
        }
    };

    let voyage = match &mission.phase {
        MissionPhase::Choosing { origin }
        | MissionPhase::Requested { origin, .. }
        | MissionPhase::Planning { origin, .. } => {
            if let Some(body) = scene.get(*origin) {
                ring(body.body.center().as_vec3());
            }
            return;
        }
        _ => match mission.flight() {
            Some(flight) => &flight.voyage,
            None => return,
        },
    };

    let landing = voyage
        .touchdown_offset_units
        .zip(scene.get(voyage.target))
        .map(|(offset, target)| (target.body.center() + offset).as_vec3());
    if let Some(spot) = landing {
        ring(spot);
    }

    let rocket = scene
        .get(voyage.craft)
        .map(|prop| prop.body.center().as_vec3());
    gizmos.linestrip(voyage.flown_path_units.iter().copied().chain(rocket), color);
    if landing.is_none() {
        gizmos.linestrip(
            voyage.planned_path_units.iter().copied(),
            color.with_alpha(MISSION_PLANNED_PATH_ALPHA),
        );
    }
}

/// Plugin that flies the experiment's spacecraft. Needs [`LaunchInputPlugin`] for the keys.
///
/// [`LaunchInputPlugin`]: crate::input::mission::LaunchInputPlugin
pub struct MissionPlugin;

impl Plugin for MissionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Mission>();
        app.init_resource::<LaunchRequest>();
        app.init_resource::<BodyNames>();
        // After the step, never before it. The clock computes this frame's step before anything
        // in `Update` can pause it, so a plan snapshotted *before* the step would be one frame stale
        // by the time the scene had moved: at the solar system's clock, Earth goes `500,000 km` in
        // that frame, and a craft placed at the stale launch point falls into an orbit of its own.
        // Snapshotting after the step takes the exact state the next frame starts from, and the
        // pause then holds it there.
        app.add_systems(
            Update,
            (pick_route_system, mission_system)
                .chain()
                .after(crate::render::plugin::step_scene_system),
        );
        if app.world().get_resource::<GizmoConfigStore>().is_some() {
            app.add_systems(Update, draw_mission_system.after(mission_system));
        }
    }
}

#[cfg(test)]
#[path = "../../tst/render/mission_tests.rs"]
mod tests;
