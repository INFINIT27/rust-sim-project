//! Tests for [`crate::spacecraft::flight`], `#[path]`-included as a unit-test module of it, not an
//! integration test: phases and the correction schedule. Whole flights are flown in
//! `tst/spacecraft/plan_tests.rs`.

use super::*;
use crate::core::transfer::PlanProgress;
use crate::spacecraft::constants::{CORRECTION_FIRST_FRACTION, CORRECTION_MIN_REMAINING_S};
use crate::spacecraft::plan_flight;

fn launched(craft: Spacecraft) -> (Scene, Flight) {
    let experiment = crate::experiments::solar_system::solar_system();
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
