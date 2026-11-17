//! Tests for [`crate::spacecraft::flight`], `#[path]`-included as a unit-test module of it, not an
//! integration test: phases and the correction schedule. Whole flights are flown in
//! `tst/spacecraft/plan_tests.rs`.

use super::*;
use crate::core::transfer::PlanProgress;
use crate::spacecraft::constants::{CORRECTION_FIRST_FRACTION, CORRECTION_MIN_REMAINING_S};
use crate::spacecraft::plan_flight;

fn launched(craft: Spacecraft) -> (Scene, Flight) {
    let experiment = crate::projects::interstellar::solar_system::solar_system();
    let id = |name: &str| {
        experiment
            .names
            .0
            .iter()
            .find(|(_, n)| *n == name)
            .unwrap()
            .0
    };
    let (earth, moon) = (id("Earth"), id("Moon"));
    let mut scene = experiment.scene;
    let plan = plan_flight(&scene, earth, moon, &craft, &PlanProgress::default()).unwrap();
    let flight = Flight::launch(&mut scene, craft, earth, moon, plan, 0.0);
    (scene, flight)
}

#[test]
fn each_propulsion_starts_in_its_own_phase() {
    let (_, crewed) = launched(Spacecraft::crewed_transfer());
    assert!(matches!(crewed.phase(), Phase::Departure { .. }));
    assert_eq!(crewed.phase().label(), "departure burn");
    let (_, direct) = launched(Spacecraft::full_thrust_direct());
    assert_eq!(direct.phase(), Phase::Direct);
    let (_, probe) = launched(Spacecraft::coasting_probe());
    assert_eq!(probe.phase(), Phase::Coasting);
    assert!(
        probe.report().delta_v_m_s > 0.0,
        "the booster's push is counted"
    );
}

#[test]
fn a_transfer_takes_no_correction_while_it_burns() {
    let (_, flight) = launched(Spacecraft::crewed_transfer());
    assert!(flight.next_correction_at().is_none());
}

#[test]
fn the_departure_burn_ends_in_a_cruise_and_spends_what_was_planned() {
    let (mut scene, mut flight) = launched(Spacecraft::crewed_transfer());
    let Phase::Departure { remaining_m_s, .. } = flight.phase() else {
        panic!("{:?}", flight.phase());
    };
    let step = TimeStep::from_seconds(10.0).unwrap();
    for _ in 0..1_000 {
        if flight.phase() == Phase::Cruise {
            break;
        }
        scene.step_steered(
            step,
            &mut flight,
            |f, s, sub| f.steer(s, sub),
            |f, s| f.observe(s),
        );
    }
    assert_eq!(flight.phase(), Phase::Cruise);
    let spent = flight.report().delta_v_m_s;
    assert!(
        (spent - remaining_m_s).abs() < 1.0,
        "{spent} of {remaining_m_s}"
    );
    let g = flight.report().peak_acceleration_m_s2 / 9.806_65;
    assert!((g - 3.0).abs() < 1e-9, "{g} g");
}

#[test]
fn a_cruise_corrects_first_early_then_halving_the_time_left() {
    let (mut scene, mut flight) = launched(Spacecraft::crewed_transfer());
    flight.phase = Phase::Cruise;
    let (launched_at, arrival) = (flight.voyage.launched_at_s, flight.voyage.arrival_at_s);
    let first = flight.next_correction_at().unwrap();
    assert!(
        (first - (launched_at + CORRECTION_FIRST_FRACTION * (arrival - launched_at))).abs() < 1e-6
    );

    flight.apply_correction(&mut scene, None, first);
    let second = flight.next_correction_at().unwrap();
    assert!((second - (first + 0.5 * (arrival - first))).abs() < 1e-6);

    // Within the last six hours, no more.
    flight.apply_correction(&mut scene, None, arrival - 0.5 * CORRECTION_MIN_REMAINING_S);
    assert!(flight.next_correction_at().is_none());
}

#[test]
fn a_correction_turns_a_cruise_into_a_burn_toward_its_velocity() {
    let (mut scene, mut flight) = launched(Spacecraft::crewed_transfer());
    flight.phase = Phase::Cruise;
    let mut fix = flight.voyage.planned_path_units.clone();
    fix.truncate(1);
    let plan = crate::core::transfer::TransferPlan {
        position_units: DVec3::ZERO,
        velocity_units_s: DVec3::new(1.0e-5, 0.0, 0.0),
        path_units: fix,
        flight_seconds: 1.0e5,
        miss_m: 0.0,
        departure_speed_m_s: 0.0,
        flybys: Vec::new(),
        peak_speed_m_s: 0.0,
    };
    flight.apply_correction(&mut scene, Some(plan), 1.0);
    match flight.phase() {
        Phase::Correction { goal_velocity_m_s } => {
            assert!((goal_velocity_m_s.x - 1.0e4).abs() < 1.0)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(flight.report().corrections, 1);
}

// ---------------------------------------------------------------------------------------------
// A rocket that carries its propellant
// ---------------------------------------------------------------------------------------------

/// Step `flight` in ten-second frames until `done` says so, or `frames` have passed.
fn step_until(
    scene: &mut Scene,
    flight: &mut Flight,
    frames: usize,
    done: impl Fn(&Flight) -> bool,
) {
    let step = TimeStep::from_seconds(10.0).unwrap();
    for _ in 0..frames {
        if done(flight) {
            break;
        }
        scene.step_steered(
            step,
            flight,
            |f, s, sub| f.steer(s, sub),
            |f, s| f.observe(s),
        );
    }
}

#[test]
fn a_rocket_launches_into_a_departure_burn_with_full_tanks() {
    let (scene, flight) = launched(Spacecraft::saturn_v_lander());
    assert!(matches!(flight.phase(), Phase::Departure { .. }));
    let rocket = flight.rocket().unwrap();
    assert_eq!(rocket.stage_number(), (1, 4));
    assert_eq!(flight.report().propellant_loaded_kg, Some(2_733_848.0));
    assert_eq!(flight.report().propellant_left_kg, Some(2_733_848.0));
    // The scene's copy weighs the whole stack.
    let prop = scene.get(flight.voyage.craft).unwrap();
    assert_eq!(prop.mass_kg, rocket.mass_kg() as f32);
    // An idealised craft has no rocket.
    let (_, crewed) = launched(Spacecraft::crewed_transfer());
    assert!(crewed.rocket().is_none());
}

#[test]
fn a_rockets_departure_burn_drains_its_tanks_by_the_rocket_equation_and_drops_stages() {
    let (mut scene, mut flight) = launched(Spacecraft::saturn_v_lander());
    let Phase::Departure { remaining_m_s, .. } = flight.phase() else {
        panic!("{:?}", flight.phase());
    };
    step_until(&mut scene, &mut flight, 1_000, |f| {
        f.phase() == Phase::Cruise
    });
    assert_eq!(flight.phase(), Phase::Cruise);

    // It spent the burn it planned.
    let report = flight.report().clone();
    assert!(
        (report.delta_v_m_s - remaining_m_s).abs() < 1.0,
        "{} of {remaining_m_s}",
        report.delta_v_m_s
    );

    // What is left in the tanks is what a fresh rocket has left after the same speed change.
    let mut fresh = Spacecraft::saturn_v_lander().rocket().unwrap();
    fresh.burn(report.delta_v_m_s);
    let rocket = flight.rocket().unwrap();
    assert!(
        (rocket.mass_kg() / fresh.mass_kg() - 1.0).abs() < 1e-9,
        "{} kg against {} kg",
        rocket.mass_kg(),
        fresh.mass_kg()
    );
    let accounted = report.propellant_kg.unwrap() + rocket.propellant_left_kg();
    assert!((accounted - 2_733_848.0).abs() < 1e-3, "{accounted} kg");

    // The first two stages are gone, and the scene's copy weighs what is left.
    assert_eq!(report.stages_dropped, 2);
    assert_eq!(rocket.stage_number(), (3, 4));
    let prop = scene.get(flight.voyage.craft).unwrap();
    assert_eq!(prop.mass_kg, rocket.mass_kg() as f32);

    // Never more than the crew limit, and the limit was reached as the first stage lightened.
    let g = report.peak_acceleration_m_s2 / 9.806_65;
    assert!((g - 3.0).abs() < 1e-9, "{g} g");
    // The engines ran for longer than the same burn at a constant 3 g would take.
    assert!(report.burn_time_s > remaining_m_s / (3.0 * 9.806_65));
}

#[test]
fn a_rocket_accelerates_harder_as_it_burns() {
    let (mut scene, mut flight) = launched(Spacecraft::saturn_v_lander());
    let at_launch = flight.rocket().unwrap().acceleration_m_s2();
    step_until(&mut scene, &mut flight, 6, |_| false);
    let after_a_minute = flight.rocket().unwrap().acceleration_m_s2();
    assert!(
        after_a_minute > 1.15 * at_launch,
        "{at_launch} then {after_a_minute} m/s²"
    );
}

/// The Saturn lander cut down to its first stage with a fiftieth of its propellant: far too
/// little for the trip.
fn short_of_propellant() -> Spacecraft {
    let mut craft = Spacecraft::saturn_v_lander();
    if let Propulsion::Staged { stages, .. } = &mut craft.propulsion {
        stages.truncate(1);
        stages[0].propellant_kg *= 0.02;
    }
    craft
}

#[test]
fn a_rocket_that_runs_dry_stops_burning_and_glides() {
    let (mut scene, mut flight) = launched(short_of_propellant());
    let carried = flight.rocket().unwrap().delta_v_left_m_s();
    assert!(matches!(flight.phase(), Phase::Departure { .. }));

    step_until(&mut scene, &mut flight, 200, |f| f.phase() == Phase::Dry);
    assert_eq!(flight.phase(), Phase::Dry);
    assert_eq!(flight.phase().label(), "out of propellant, gliding");

    // It gave exactly what its tank held, and no more however long it flies on.
    let report = flight.report().clone();
    assert!(report.ran_dry);
    assert_eq!(report.propellant_left_kg, Some(0.0));
    assert!(
        (report.delta_v_m_s - carried).abs() < 1e-6,
        "{} of {carried}",
        report.delta_v_m_s
    );
    assert!(flight.rocket().unwrap().is_dry());

    let before = flight.relative_state();
    step_until(&mut scene, &mut flight, 5, |f| f.has_landed());
    assert_eq!(
        flight.report().delta_v_m_s,
        report.delta_v_m_s,
        "no engine, no burn"
    );
    assert_ne!(
        flight.relative_state(),
        before,
        "but it still moves, under gravity"
    );
}

#[test]
fn a_dry_rocket_takes_no_corrections() {
    let (mut scene, mut flight) = launched(short_of_propellant());
    step_until(&mut scene, &mut flight, 200, |f| f.phase() == Phase::Dry);
    assert!(flight.next_correction_at().is_none());

    // Handed one anyway, it stays as it is: there is nothing to burn toward it with.
    let plan = crate::core::transfer::TransferPlan {
        position_units: DVec3::ZERO,
        velocity_units_s: DVec3::new(1.0e-5, 0.0, 0.0),
        path_units: vec![flight.voyage.planned_path_units[0]],
        flight_seconds: 1.0e5,
        miss_m: 0.0,
        departure_speed_m_s: 0.0,
        flybys: Vec::new(),
        peak_speed_m_s: 0.0,
    };
    flight.apply_correction(&mut scene, Some(plan), 1.0);
    assert_eq!(flight.phase(), Phase::Dry);
}
