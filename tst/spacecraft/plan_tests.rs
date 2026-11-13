//! Tests for [`crate::spacecraft::plan`], `#[path]`-included as a unit-test module of it, not an
//! integration test: planning each kind of flight, and flying it through the live scene.

use super::*;
use crate::experiments::solar_system::solar_system;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::spacecraft::{Flight, Phase, Spacecraft};
use crate::time::simulation::TimeStep;

fn pair(name_a: &str, name_b: &str) -> (Scene, BodyId, BodyId) {
    let experiment = solar_system();
    let id = |name: &str| {
        experiment
            .names
            .0
            .iter()
            .find(|(_, n)| *n == name)
            .unwrap_or_else(|| panic!("no body named {name}"))
            .0
    };
    let (a, b) = (id(name_a), id(name_b));
    (experiment.scene, a, b)
}

/// Plan `craft`'s flight between two named bodies.
fn plan(craft: &Spacecraft, from: &str, to: &str) -> (Scene, BodyId, BodyId, FlightPlan) {
    let (scene, origin, target) = pair(from, to);
    let plan = plan_flight(&scene, origin, target, craft, &PlanProgress::default())
        .unwrap_or_else(|e| panic!("{from} → {to}: {e:?}"));
    (scene, origin, target, plan)
}

/// Fly `plan` through the live scene, about 1,200 frames as the app's clock spreads a flight,
/// with correction burns when they are due, until the craft lands or three planned flight times
/// have passed.
fn fly(
    scene: Scene,
    craft: Spacecraft,
    origin: BodyId,
    target: BodyId,
    plan: FlightPlan,
) -> Flight {
    let frame_s = (plan.transfer.flight_seconds / 1_200.0).min(16_666.7);
    let frame = TimeStep::from_seconds(frame_s).unwrap();
    let mut scene = scene;
    let mut flight = Flight::launch(&mut scene, craft, origin, target, plan.clone(), 0.0);
    let mut now = 0.0;
    while !flight.has_landed() && now < 3.0 * plan.transfer.flight_seconds {
        if flight.next_correction_at().is_some_and(|at| now >= at) {
            let fix = plan_correction_for(
                &scene,
                &flight.craft,
                flight.voyage.craft,
                origin,
                target,
                flight.voyage.arrival_at_s - now,
                flight.next_flyby(now).as_ref(),
            );
            flight.apply_correction(&mut scene, fix, now);
        }
        scene.step_steered(
            frame,
            &mut flight,
            |flight, scene, substep| flight.steer(scene, substep),
            |flight, scene| flight.observe(scene),
        );
        now += frame_s;
    }
    flight.finish(now);
    flight
}

/// Plan and fly the crewed transfer between two named bodies, and check it landed softly at no
/// more than its 3 g, within a few percent of the planned time. Returns the flight and the planned
/// time of flight, in seconds.
fn fly_crewed(from: &str, to: &str) -> (Flight, f64) {
    let craft = Spacecraft::crewed_transfer();
    let (scene, origin, target, plan) = plan(&craft, from, to);
    let planned = plan.transfer.flight_seconds;
    let flight = fly(scene, craft.clone(), origin, target, plan);
    let report = flight.report();
    assert!(flight.has_landed(), "{from} → {to} never landed");
    let touchdown = report.touchdown_speed_m_s.unwrap();
    assert!(
        touchdown <= 11.0,
        "{from} → {to}: landed at {touchdown:.1} m/s"
    );
    assert!(
        report.peak_acceleration_m_s2 <= craft.acceleration_m_s2().unwrap() * (1.0 + 1e-9),
        "{from} → {to}: {} g",
        report.peak_acceleration_m_s2 / STANDARD_GRAVITY_M_S2
    );
    let travel = report.travel_time_s.unwrap();
    assert!(
        (travel - planned).abs() <= 0.05 * planned,
        "{from} → {to}: flew {:.1} d against {:.1} d planned",
        travel / 86_400.0,
        planned / 86_400.0
    );
    (flight, planned)
}

// ---------------------------------------------------------------------------------------------
// Routing and refusals
// ---------------------------------------------------------------------------------------------

#[test]
fn a_transfer_is_computed_about_the_planet_for_its_own_moon_and_the_sun_otherwise() {
    let experiment = solar_system();
    let id = |name: &str| {
        experiment
            .names
            .0
            .iter()
            .find(|(_, n)| *n == name)
            .unwrap()
            .0
    };
    let scene = &experiment.scene;
    let (sun, earth, moon, mars) = (id("Sun"), id("Earth"), id("Moon"), id("Mars"));
    assert_eq!(central_body(scene, earth, moon), Some(earth));
    assert_eq!(central_body(scene, moon, earth), Some(earth));
    assert_eq!(central_body(scene, earth, mars), Some(sun));
    assert_eq!(central_body(scene, moon, mars), Some(sun));
}

#[test]
fn a_flight_refuses_the_same_body_twice_and_an_invalid_craft() {
    let (scene, earth, _) = pair("Earth", "Mars");
    let progress = PlanProgress::default();
    let craft = Spacecraft::crewed_transfer();
    assert_eq!(
        plan_flight(&scene, earth, earth, &craft, &progress),
        Err(NoFlight::InvalidRoute)
    );
    let broken = Spacecraft {
        mass_kg: 0.0,
        ..craft
    };
    let (_, _, mars) = pair("Earth", "Mars");
    assert_eq!(
        plan_flight(&scene, earth, mars, &broken, &progress),
        Err(NoFlight::InvalidRoute)
    );
}

#[test]
fn a_3_g_craft_cannot_lift_off_from_or_land_on_the_sun() {
    // The Sun's surface gravity is 28 g. Jupiter's 2.5 g is under 3 g, so it is allowed.
    let craft = Spacecraft::crewed_transfer();
    let progress = PlanProgress::default();
    let (scene, sun, earth) = pair("Sun", "Earth");
    for (from, to, body) in [(sun, earth, sun), (earth, sun, sun)] {
        match plan_flight(&scene, from, to, &craft, &progress) {
            Err(NoFlight::TooMuchGravity {
                body: blocked,
                gravity_m_s2,
                ..
            }) => {
                assert_eq!(blocked, body);
                assert!((gravity_m_s2 / STANDARD_GRAVITY_M_S2 - 28.0).abs() < 1.0);
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_cancelled_plan_says_so() {
    let (scene, earth, mars) = pair("Earth", "Mars");
    let progress = PlanProgress::default();
    progress.cancel();
    for craft in [
        Spacecraft::crewed_transfer(),
        Spacecraft::full_thrust_direct(),
    ] {
        assert_eq!(
            plan_flight(&scene, earth, mars, &craft, &progress),
            Err(NoFlight::Cancelled)
        );
    }
}

#[test]
fn a_coasting_probe_is_planned_by_the_coasting_planner() {
    let (_, _, _, plan) = plan(&Spacecraft::coasting_probe(), "Earth", "Moon");
    assert!(plan.departure_burn_m_s.is_none());
    assert!(plan.transfer.departure_speed_m_s > 0.0);
    assert!(
        plan.transfer.departure_speed_m_s
            <= crate::spacecraft::constants::PROBE_MAX_DEPARTURE_SPEED_M_S + 1e-6
    );
}

// ---------------------------------------------------------------------------------------------
// The least-energy transfer
// ---------------------------------------------------------------------------------------------

#[test]
fn the_departure_burn_climbs_and_ends_with_the_speed_it_was_given() {
    // From rest 200 km above Earth, straight up at 3 g for 11 km/s: gravity takes some of it.
    let mu = 3.986e14;
    let start = DVec3::new(6.571e6, 0.0, 0.0);
    let burn = DVec3::new(11_000.0, 0.0, 0.0);
    let (r, v) = departure_burn(mu, start, burn, 3.0 * STANDARD_GRAVITY_M_S2).unwrap();
    assert!(r.x > start.x + 1.0e6, "it climbed to {:.0} km", r.x / 1e3);
    // About 3 km/s goes to holding the craft up against gravity while it burns.
    assert!(v.x < 11_000.0 && v.x > 7_000.0, "{v:?}");
}

#[test]
fn the_departure_burn_changes_smoothly_with_the_burn_it_is_given() {
    // The planner differentiates it, so a centimetre per second more must move it continuously.
    let mu = 3.986e14;
    let start = DVec3::new(6.571e6, 0.0, 0.0);
    let accel = 3.0 * STANDARD_GRAVITY_M_S2;
    let at =
        |dv: f64| departure_burn(mu, start, DVec3::new(11_000.0 + dv, 0.0, 0.0), accel).unwrap();
    let (r0, v0) = at(0.0);
    let (r1, v1) = at(0.01);
    let (r2, v2) = at(0.02);
    // Equal steps in, equal steps out, to well within the step itself.
    assert!(((r2 - r1) - (r1 - r0)).length() < 1.0e-3);
    assert!(((v2 - v1) - (v1 - v0)).length() < 1.0e-6);
}

#[test]
fn a_departure_burn_that_cannot_climb_falls_back() {
    // 1.1 m/s² of thrust against Earth's 9.2 m/s² at 200 km: it falls.
    let mu = 3.986e14;
    let start = DVec3::new(6.571e6, 0.0, 0.0);
    assert!(departure_burn(mu, start, DVec3::new(11_000.0, 0.0, 0.0), 1.1).is_none());
}

#[test]
fn the_crewed_transfer_to_mars_is_planned_onto_a_coasting_arc() {
    let craft = Spacecraft::crewed_transfer();
    let (_, _, _, plan) = plan(&craft, "Earth", "Mars");
    let burn = plan
        .departure_burn_m_s
        .expect("a transfer departs with a burn");
    // Escaping Earth from rest takes at least its 11 km/s escape speed; the whole trip, launched
    // now rather than at a launch window, well under what a direct flight would take.
    assert!(burn.length() > 11_000.0, "{burn:?}");
    assert!(
        (15_000.0..60_000.0).contains(&plan.delta_v_estimate_m_s),
        "{} km/s",
        plan.delta_v_estimate_m_s / 1e3
    );
    let days = plan.transfer.flight_seconds / 86_400.0;
    assert!((100.0..1_000.0).contains(&days), "{days} d");
}

#[test]
fn the_crewed_transfer_lands_on_the_moon_in_days_at_no_more_than_3_g() {
    let (flight, _) = fly_crewed("Earth", "Moon");
    let report = flight.report();
    let days = report.travel_time_s.unwrap() / 86_400.0;
    assert!((2.0..8.0).contains(&days), "{days} d");
    assert!(report.corrections >= 1, "the cruise is corrected");
    assert!(report.energy_j.unwrap() > 0.0 && report.propellant_kg.unwrap() > 0.0);
}

#[test]
fn the_crewed_transfer_lands_back_on_earth_from_the_moon_on_time() {
    // Within 1%, not just the 5% every crewed flight gets: flown uncorrected, this one lands
    // within 0.1% of the plan, so anything more is the corrections adding error. They once did:
    // their prediction left the origin out, folded the Moon into Earth, and burned 436 and then
    // 818 m/s against a pull that was not there, landing 5% late (and failing on Linux CI).
    let (flight, planned) = fly_crewed("Moon", "Earth");
    let travel = flight.report().travel_time_s.unwrap();
    assert!(
        (travel - planned).abs() <= 0.01 * planned,
        "flew {:.3} d against {:.3} d planned",
        travel / 86_400.0,
        planned / 86_400.0
    );
}

/// The crewed transfer to the far planets, flown live. About three minutes in release.
#[test]
#[ignore = "flies months-to-years transfers live; run with --release -- --ignored crewed_transfers"]
fn crewed_transfers_land_across_the_solar_system() {
    for (from, to) in [
        ("Earth", "Mars"),
        ("Mars", "Earth"),
        ("Earth", "Venus"),
        ("Mercury", "Jupiter"),
        ("Earth", "Neptune"),
    ] {
        fly_crewed(from, to);
    }
}

// ---------------------------------------------------------------------------------------------
// The full-thrust direct flight
// ---------------------------------------------------------------------------------------------

/// Plan and fly the full-thrust direct flight between two named bodies.
fn fly_direct(from: &str, to: &str) -> (FlightPlan, Flight) {
    let craft = Spacecraft::full_thrust_direct();
    let (scene, origin, target, plan) = plan(&craft, from, to);
    let flight = fly(scene, craft, origin, target, plan.clone());
    (plan, flight)
}

fn assert_landed_softly(plan: &FlightPlan, flight: &Flight) {
    let report = flight.report();
    let touchdown = report.touchdown_speed_m_s.expect("the rocket lands");
    assert!(touchdown <= 11.0, "touched down at {touchdown:.1} m/s");
    assert_eq!(
        report.corrections, 0,
        "a direct flight takes no corrections"
    );
    let planned = plan.transfer.peak_speed_m_s;
    assert!(
        (report.max_speed_m_s - planned).abs() < 0.01 * planned,
        "live peak {} against planned {planned}",
        report.max_speed_m_s
    );
}

#[test]
fn a_full_thrust_flight_from_mercury_to_jupiter_takes_13_hours() {
    // Out of a coasting probe's reach. Under full thrust it is half a day at up to 12% of light.
    let (plan, flight) = fly_direct("Mercury", "Jupiter");
    let hours = plan.transfer.flight_seconds / 3_600.0;
    assert!((12.0..14.0).contains(&hours), "{hours} h");
    let c = crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
    assert!((0.10..0.14).contains(&(plan.transfer.peak_speed_m_s / c)));
    assert_eq!(flight.phase(), Phase::Landed);
    assert_landed_softly(&plan, &flight);
}

#[test]
fn a_full_thrust_flight_lands_on_the_moon_within_half_an_hour() {
    let (plan, flight) = fly_direct("Earth", "Moon");
    assert!(plan.transfer.flight_seconds < 1_800.0);
    assert_landed_softly(&plan, &flight);
}

#[test]
fn a_full_thrust_flight_steers_around_the_sun_when_it_is_in_the_way() {
    // Mars moved to the far side of the Sun from Earth, so the straight line runs through the Sun.
    let experiment = solar_system();
    let id = |name: &str| {
        experiment
            .names
            .0
            .iter()
            .find(|(_, n)| *n == name)
            .unwrap()
            .0
    };
    let (sun, earth, mars) = (id("Sun"), id("Earth"), id("Mars"));
    let mut scene = experiment.scene;
    let sun_at = scene.get(sun).unwrap().body.center();
    let earth_at = scene.get(earth).unwrap().body.center();
    let index = scene.iter().position(|(id, _)| id == mars).unwrap();
    if let crate::core::scene::Body::Sphere { pose, .. } = &mut scene.props[index].body {
        pose.position = sun_at - (earth_at - sun_at) * 1.52;
    }
    scene.props[index].velocity_units_s = -scene.props[index].velocity_units_s;
    let sun_radius = physical_radius_m(scene.get(sun).unwrap());

    let craft = Spacecraft::full_thrust_direct();
    let plan = plan_flight(&scene, earth, mars, &craft, &PlanProgress::default()).unwrap();
    let closest = plan
        .transfer
        .path_units
        .iter()
        .map(|p| to_metres(p.as_dvec3() - sun_at).length())
        .fold(f64::INFINITY, f64::min);
    assert!(
        closest > crate::spacecraft::constants::POWERED_CLEARANCE_RADII * sun_radius,
        "the planned path passes {:.2} solar radii from the Sun",
        closest / sun_radius
    );
    let flight = fly(scene, craft, earth, mars, plan.clone());
    assert_landed_softly(&plan, &flight);
}

/// Every pair of the Sun, the eight planets, and the Moon, flown live at full thrust.
#[test]
#[ignore = "flies all 90 pairs live; run with --release -- --ignored every_pair"]
fn every_pair_in_the_solar_system_can_be_flown_at_full_thrust() {
    let names = [
        "Sun", "Mercury", "Venus", "Earth", "Moon", "Mars", "Jupiter", "Saturn", "Uranus",
        "Neptune",
    ];
    for a in names {
        for b in names.iter().filter(|b| **b != a) {
            let (_, flight) = fly_direct(a, b);
            let touchdown = flight.report().touchdown_speed_m_s.unwrap_or(f64::INFINITY);
            // The Sun's 274 m/s² surface gravity adds a few m/s in the last instant.
            assert!(
                touchdown <= 13.5,
                "{a} → {b}: touched down at {touchdown:.1} m/s"
            );
        }
    }
}

#[test]
fn a_transfer_correction_with_no_time_left_is_declined_not_a_crash() {
    // A fast frame can step past the arrival time while a correction is due, which hands the
    // planner a negative time to arrival. It used to panic in the prediction it builds.
    let (mut scene, earth, moon) = pair("Earth", "Moon");
    let craft = scene.add(Spacecraft::crewed_transfer().prop());
    for remaining in [-10.0, 0.0] {
        assert!(plan_transfer_correction(&scene, craft, earth, moon, remaining).is_none());
    }
}

#[test]
fn a_forecast_that_fails_is_not_reported_as_a_cancel() {
    let progress = PlanProgress::default();
    assert_eq!(forecast_failure(&progress), NoFlight::NoForecast);
    progress.cancel();
    assert_eq!(forecast_failure(&progress), NoFlight::Cancelled);
}
