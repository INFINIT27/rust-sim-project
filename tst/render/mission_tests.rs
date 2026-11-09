//! Tests for [`crate::render::mission`]. Picking is pure and tested directly; the flow from a pick
//! to a rocket flying in the scene is driven through a real `App`.

use super::*;
use crate::experiments::catalog::{two_body, Experiment};
use crate::experiments::solar_system::solar_system;
use crate::render::plugin::step_scene_system;
use bevy::app::App;
use bevy::MinimalPlugins;

fn app_for(experiment: Experiment) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(experiment.scene);
    app.insert_resource(SimulationClock::new(experiment.spec.time_scale));
    app.insert_resource(experiment.spec);
    app.insert_resource(experiment.names);
    app.add_systems(Update, step_scene_system);
    app.add_plugins(MissionPlugin);
    app
}

fn tick(app: &mut App) {
    app.world_mut()
        .resource_mut::<SimulationClock>()
        .advance(1.0 / 60.0);
    app.update();
}

fn named(app: &App, name: &str) -> BodyId {
    app.world()
        .resource::<BodyNames>()
        .0
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(id, _)| *id)
        .unwrap()
}

/// Mirror the two bodies as crosshair targets, and a camera that can be pointed at either.
fn spawn_targets(app: &mut App, bodies: &[BodyId]) {
    for &id in bodies {
        let at = app
            .world()
            .resource::<Scene>()
            .get(id)
            .unwrap()
            .body
            .center();
        app.world_mut().spawn((
            Transform::from_translation(at),
            MirroredBody(id),
            CameraTarget,
        ));
    }
    app.world_mut()
        .spawn((Camera3d::default(), Transform::default()));
}

fn aim_at(app: &mut App, id: BodyId) {
    let at = app
        .world()
        .resource::<Scene>()
        .get(id)
        .unwrap()
        .body
        .center();
    let mut cameras = app
        .world_mut()
        .query_filtered::<&mut Transform, With<Camera3d>>();
    let mut camera = cameras.single_mut(app.world_mut());
    *camera = Transform::from_translation(at + Vec3::new(0.0, 1.0, 2.0)).looking_at(at, Vec3::Y);
}

fn press_r(app: &mut App) {
    app.world_mut().resource_mut::<LaunchRequest>().request();
    tick(app);
}

fn press_g(app: &mut App) {
    app.world_mut()
        .resource_mut::<LaunchRequest>()
        .request_engine_switch();
    tick(app);
}

/// The built-in fleet's order: see `Fleet::default`.
const CREWED: usize = 0;
const DIRECT: usize = 1;
const COASTING: usize = 2;

/// Select craft `index` of the fleet directly. Not with `G`: a press is a frame, which would move
/// the planets away from crosshair targets already spawned. `G` itself has its own test.
fn select(app: &mut App, index: usize) {
    assert!(app
        .world_mut()
        .resource_mut::<Mission>()
        .fleet
        .select(index));
}

#[test]
fn the_first_pick_is_the_origin_the_second_the_destination() {
    let mut mission = Mission::default();
    mission.pick(BodyId(4));
    assert!(matches!(
        mission.phase,
        MissionPhase::Choosing { origin: BodyId(4) }
    ));
    mission.pick(BodyId(5));
    assert!(matches!(
        mission.phase,
        MissionPhase::Requested {
            origin: BodyId(4),
            target: BodyId(5)
        }
    ));
}

#[test]
fn picking_the_origin_again_cancels() {
    let mut mission = Mission::default();
    mission.pick(BodyId(4));
    mission.pick(BodyId(4));
    assert!(matches!(mission.phase, MissionPhase::Idle));
}

#[test]
fn a_new_pick_after_landing_starts_a_new_route() {
    let mut mission = Mission {
        phase: MissionPhase::NoPath {
            origin: BodyId(4),
            target: BodyId(5),
            reason: crate::spacecraft::NoFlight::InvalidRoute,
        },
        restore_scale: None,
        fleet: crate::spacecraft::Fleet::default(),
    };
    assert!(mission.is_picking());
    mission.pick(BodyId(5));
    assert!(matches!(
        mission.phase,
        MissionPhase::Choosing { origin: BodyId(5) }
    ));
}

#[test]
fn r_on_two_bodies_plans_with_the_clock_paused_then_flies_a_rocket_between_them() {
    let mut app = app_for(solar_system());
    let (earth, moon) = (named(&app, "Earth"), named(&app, "Moon"));
    spawn_targets(&mut app, &[earth, moon]);
    let props_before = app.world().resource::<Scene>().props.len();
    let scale_before = app.world().resource::<SimulationClock>().scale();

    aim_at(&mut app, earth);
    press_r(&mut app);
    let names = app.world().resource::<BodyNames>().clone();
    let status = app
        .world()
        .resource::<Mission>()
        .status_line(0.0, &names)
        .unwrap();
    assert!(status.starts_with("rocket from Earth"), "{status}");

    aim_at(&mut app, moon);
    press_r(&mut app);
    assert!(matches!(
        app.world().resource::<Mission>().phase,
        MissionPhase::Planning { .. }
    ));
    assert!(
        app.world().resource::<SimulationClock>().is_paused(),
        "the planets wait for the plan"
    );

    let started = std::time::Instant::now();
    while matches!(
        app.world().resource::<Mission>().phase,
        MissionPhase::Planning { .. }
    ) {
        assert!(
            started.elapsed().as_secs() < 300,
            "the plan never came back"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        tick(&mut app);
    }

    let MissionPhase::Cruise(flight) = &app.world().resource::<Mission>().phase else {
        panic!("expected the rocket to be flying");
    };
    assert_eq!((flight.voyage.origin, flight.voyage.target), (earth, moon));
    let craft = flight.voyage.craft;
    assert_eq!(
        app.world().resource::<Scene>().props.len(),
        props_before + 1
    );
    let clock = app.world().resource::<SimulationClock>();
    assert!(!clock.is_paused(), "the clock runs again");
    assert!(
        clock.scale() < scale_before,
        "a few-day transfer to the Moon slows a 1e6 clock so it can be watched"
    );

    let launched_at = app
        .world()
        .resource::<Scene>()
        .get(craft)
        .unwrap()
        .body
        .center();
    // The launch point is where the plan put it: 200 km above Earth's surface, Earth as it is now.
    // A plan made from a snapshot the scene has since stepped past would put it where Earth was.
    let earth_now = app.world().resource::<Scene>().get(earth).unwrap();
    let altitude_m = f64::from((launched_at - earth_now.body.center()).length())
        * crate::constants::METRES_PER_UNIT
        - f64::from(
            earth_now
                .contact_radius_units
                .unwrap_or(match earth_now.body {
                    crate::core::scene::Body::Sphere { radius, .. } => radius,
                    _ => 0.0,
                }),
        ) * crate::constants::METRES_PER_UNIT;
    assert!(
        (altitude_m - 200_000.0).abs() < 50_000.0,
        "the rocket starts 200 km above Earth, not {:.0} km",
        altitude_m / 1e3
    );

    tick(&mut app);
    let moved_to = app
        .world()
        .resource::<Scene>()
        .get(craft)
        .unwrap()
        .body
        .center();
    assert!(moved_to != launched_at, "the integrator moves the rocket");

    let status = app
        .world()
        .resource::<Mission>()
        .status_line(0.0, &names)
        .unwrap();
    assert!(
        status.starts_with("rocket Earth → Moon: departure burn"),
        "{status}"
    );
}

#[test]
fn r_with_nothing_at_the_crosshair_picks_nothing() {
    let mut app = app_for(solar_system());
    let earth = named(&app, "Earth");
    spawn_targets(&mut app, &[earth]);
    press_r(&mut app);

    assert!(matches!(
        app.world().resource::<Mission>().phase,
        MissionPhase::Idle
    ));
    assert!(
        !app.world_mut().resource_mut::<LaunchRequest>().take(),
        "the press was consumed"
    );
}

#[test]
fn an_experiment_without_a_craft_never_launches() {
    let mut experiment = two_body();
    experiment.spec.launches = false;
    let mut app = app_for(experiment);
    app.world_mut().resource_mut::<Mission>().phase = MissionPhase::Requested {
        origin: BodyId(1),
        target: BodyId(2),
    };
    tick(&mut app);

    assert!(matches!(
        app.world().resource::<Mission>().phase,
        MissionPhase::Requested { .. }
    ));
    assert!(!app.world().resource::<SimulationClock>().is_paused());
}

#[test]
fn there_is_no_status_line_before_anything_is_picked() {
    assert_eq!(
        Mission::default().status_line(0.0, &BodyNames::default()),
        None
    );
}

#[test]
fn a_rocket_flown_through_the_app_lands_where_it_was_aimed() {
    let mut app = app_for(solar_system());
    let (earth, moon) = (named(&app, "Earth"), named(&app, "Moon"));
    spawn_targets(&mut app, &[earth, moon]);
    aim_at(&mut app, earth);
    press_r(&mut app);
    aim_at(&mut app, moon);
    press_r(&mut app);

    let started = std::time::Instant::now();
    loop {
        match &app.world().resource::<Mission>().phase {
            MissionPhase::Landed { flight, .. } => {
                assert_eq!(flight.voyage.target, moon);
                let report = flight.report();
                let touchdown = report.touchdown_speed_m_s.expect("it touched down");
                assert!(touchdown <= 15.0, "landed at {touchdown:.1} m/s");
                assert!(
                    report.travel_time_s.is_some(),
                    "the report has the flight time"
                );
                break;
            }
            MissionPhase::Planning { .. } | MissionPhase::Correcting { .. } => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            MissionPhase::Cruise(_) => {}
            _ => panic!("the voyage ended without landing"),
        }
        assert!(started.elapsed().as_secs() < 300, "never landed");
        tick(&mut app);
    }
    assert_eq!(
        app.world().resource::<SimulationClock>().scale(),
        experiment_scale(),
        "the clock is back to its own scale after the short hop"
    );
}

fn experiment_scale() -> f32 {
    solar_system().spec.time_scale
}

#[test]
fn r_while_the_search_runs_cancels_it_and_restarts_the_clock() {
    let mut app = app_for(solar_system());
    // The long search is the coasting one; a powered plan comes back before `R` could cancel it.
    select(&mut app, COASTING);
    let (earth, neptune) = (named(&app, "Earth"), named(&app, "Neptune"));
    spawn_targets(&mut app, &[earth, neptune]);
    aim_at(&mut app, earth);
    press_r(&mut app);
    aim_at(&mut app, neptune);
    press_r(&mut app);
    assert!(app.world().resource::<SimulationClock>().is_paused());

    press_r(&mut app);
    let started = std::time::Instant::now();
    while !matches!(app.world().resource::<Mission>().phase, MissionPhase::Idle) {
        assert!(started.elapsed().as_secs() < 60, "the search never stopped");
        std::thread::sleep(std::time::Duration::from_millis(5));
        tick(&mut app);
    }
    assert!(!app.world().resource::<SimulationClock>().is_paused());
    assert_eq!(
        app.world().resource::<Scene>().props.len(),
        solar_system().scene.props.len(),
        "no rocket was launched"
    );
}

#[test]
fn an_out_of_reach_target_says_why_and_what_it_would_take() {
    let experiment = solar_system();
    let names = experiment.names.clone();
    let id = |name: &str| names.0.iter().find(|(_, n)| *n == name).unwrap().0;
    let (mercury, jupiter) = (id("Mercury"), id("Jupiter"));
    let reach = crate::core::transfer::coasting_reach(
        &experiment.scene,
        mercury,
        jupiter,
        crate::spacecraft::constants::PROBE_MAX_DEPARTURE_SPEED_M_S,
    )
    .unwrap();
    let mission = Mission {
        phase: MissionPhase::NoPath {
            origin: mercury,
            target: jupiter,
            reason: crate::spacecraft::NoFlight::OutOfReach(reach),
        },
        restore_scale: None,
        fleet: crate::spacecraft::Fleet::default(),
    };
    let status = mission.status_line(0.0, &names).unwrap();
    assert!(
        status.contains("no gravity-assist route") && status.contains("17.5 km/s"),
        "{status}"
    );
}

#[test]
fn g_cycles_through_the_fleet_and_the_crewed_transfer_is_the_default() {
    let mut app = app_for(solar_system());
    let earth = named(&app, "Earth");
    spawn_targets(&mut app, &[earth]);
    let names = app.world().resource::<BodyNames>().clone();
    let fleet = crate::spacecraft::Fleet::default();
    assert_eq!(
        app.world().resource::<Mission>().fleet.selected_index(),
        CREWED,
        "the crewed transfer is the default"
    );

    aim_at(&mut app, earth);
    press_r(&mut app);
    for step in 0..=fleet.craft().len() {
        let expected = &fleet.craft()[step % fleet.craft().len()].name;
        let status = app
            .world()
            .resource::<Mission>()
            .status_line(0.0, &names)
            .unwrap();
        assert!(
            status.contains(&format!("({expected}; G switches)")),
            "{status}"
        );
        press_g(&mut app);
    }
}

#[test]
fn a_coasting_probe_still_cruises_and_takes_correction_burns() {
    let mut app = app_for(solar_system());
    let (earth, moon) = (named(&app, "Earth"), named(&app, "Moon"));
    spawn_targets(&mut app, &[earth, moon]);
    select(&mut app, COASTING);
    aim_at(&mut app, earth);
    press_r(&mut app);
    aim_at(&mut app, moon);
    press_r(&mut app);
    let started = std::time::Instant::now();
    while matches!(
        app.world().resource::<Mission>().phase,
        MissionPhase::Planning { .. }
    ) {
        assert!(
            started.elapsed().as_secs() < 300,
            "the plan never came back"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        tick(&mut app);
    }
    let names = app.world().resource::<BodyNames>().clone();
    let MissionPhase::Cruise(flight) = &app.world().resource::<Mission>().phase else {
        panic!("expected the rocket to be flying");
    };
    assert_eq!(flight.phase(), crate::spacecraft::Phase::Coasting);
    assert!(
        flight.craft.engine().is_none(),
        "a coasting probe has no engine"
    );
    assert!(
        flight.next_correction_at().is_some(),
        "it schedules corrections"
    );
    let status = app
        .world()
        .resource::<Mission>()
        .status_line(0.0, &names)
        .unwrap();
    assert!(
        status.starts_with("rocket Earth → Moon: cruising"),
        "{status}"
    );
}

#[test]
fn a_full_thrust_craft_flies_mercury_to_jupiter_and_lands_softly() {
    // The flight a coasting probe cannot make: no direct path, and no assist within 12 years.
    let mut app = app_for(solar_system());
    let (mercury, jupiter) = (named(&app, "Mercury"), named(&app, "Jupiter"));
    spawn_targets(&mut app, &[mercury, jupiter]);
    select(&mut app, DIRECT);
    aim_at(&mut app, mercury);
    press_r(&mut app);
    aim_at(&mut app, jupiter);
    press_r(&mut app);

    let started = std::time::Instant::now();
    loop {
        match &app.world().resource::<Mission>().phase {
            MissionPhase::Landed {
                flight,
                landed_after_s,
            } => {
                assert_eq!(flight.voyage.target, jupiter);
                let touchdown = flight.report().touchdown_speed_m_s.unwrap();
                assert!(touchdown <= 11.0, "landed at {touchdown:.1} m/s");
                assert!(
                    (11.0 * 3_600.0..15.0 * 3_600.0).contains(landed_after_s),
                    "landed after {:.2} h",
                    landed_after_s / 3_600.0
                );
                break;
            }
            MissionPhase::Planning { .. } => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            MissionPhase::Cruise(flight) => {
                assert_eq!(flight.phase(), crate::spacecraft::Phase::Direct);
            }
            _ => panic!("the flight ended without landing"),
        }
        assert!(started.elapsed().as_secs() < 300, "never landed");
        tick(&mut app);
    }
}
