//! Tests for [`crate::spacecraft::report`], `#[path]`-included as a unit-test module of it, not an
//! integration test: what a burn costs.

use super::*;

const MASS: f64 = 15_000.0;
const EXHAUST: f64 = 2_579.2;
const LIMIT: f64 = 29.42;

#[test]
fn a_burn_adds_its_speed_change_energy_and_propellant() {
    let mut report = FlightReport::new("test", true);
    report.record_burn(LIMIT, 100.0, MASS, EXHAUST, LIMIT);
    let delta_v = LIMIT * 100.0;
    assert!((report.delta_v_m_s - delta_v).abs() < 1e-9);
    assert!((report.burn_time_s - 100.0).abs() < 1e-9);
    assert_eq!(report.peak_acceleration_m_s2, LIMIT);
    // Exhaust power F · v_e / 2 for 100 s, and F / v_e of propellant per second.
    let force = MASS * LIMIT;
    assert!((report.energy_j.unwrap() - 0.5 * force * EXHAUST * 100.0).abs() < 1.0);
    assert!((report.propellant_kg.unwrap() - force / EXHAUST * 100.0).abs() < 1e-6);
}

#[test]
fn gentle_trims_count_for_their_share_of_full_thrust_not_their_duration() {
    let mut report = FlightReport::new("test", true);
    report.record_burn(LIMIT / 1_000.0, 3_600.0, MASS, EXHAUST, LIMIT);
    assert!(
        (report.burn_time_s - 3.6).abs() < 1e-9,
        "{}",
        report.burn_time_s
    );
}

#[test]
fn a_craft_with_no_engine_has_no_energy_or_propellant() {
    let mut report = FlightReport::new("probe", false);
    report.record_impulse(15_400.0);
    report.record_impulse(-3.0);
    report.record_impulse(f64::NAN);
    assert_eq!(report.delta_v_m_s, 15_400.0);
    assert!(report.energy_j.is_none() && report.propellant_kg.is_none());
}

#[test]
fn the_fastest_speed_is_kept() {
    let mut report = FlightReport::new("test", true);
    for speed in [10.0, 300.0, 20.0, f64::NAN] {
        report.record_speed(speed);
    }
    assert_eq!(report.max_speed_m_s, 300.0);
}

#[test]
fn nothing_is_counted_for_no_thrust() {
    let mut report = FlightReport::new("test", true);
    report.record_burn(0.0, 100.0, MASS, EXHAUST, LIMIT);
    report.record_burn(LIMIT, 0.0, MASS, EXHAUST, LIMIT);
    assert_eq!(report.delta_v_m_s, 0.0);
    assert_eq!(report.energy_j, Some(0.0));
}
