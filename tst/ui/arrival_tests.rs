//! Tests for [`crate::ui::arrival`], `#[path]`-included as a unit-test module of it, not an
//! integration test: the pop-up's formatting and its rows, without a window.

use super::*;
use crate::spacecraft::FlightReport;

#[test]
fn durations_read_in_their_two_largest_units() {
    assert_eq!(duration_label(42.4), "42 s");
    assert_eq!(duration_label(372.0), "6 min 12 s");
    assert_eq!(duration_label(6.0 * 3_600.0 + 12.0 * 60.0), "6 h 12 min");
    assert_eq!(duration_label(4.0 * 86_400.0 + 14.5 * 3_600.0), "4 d 14 h");
    assert_eq!(
        duration_label(17.0 * 365.25 * 86_400.0 + 3.5 * 86_400.0),
        "17 y 3 d"
    );
    assert_eq!(duration_label(-5.0), "0 s");
}

#[test]
fn energies_carry_an_si_prefix_and_a_tnt_equivalent() {
    assert_eq!(energy_label(8.31e12), "8.31 TJ (1.99 kt of TNT)");
    assert_eq!(energy_label(4.184e9), "4.18 GJ (1.00 t of TNT)");
    assert_eq!(energy_label(4.184e15), "4.18 PJ (1.00 Mt of TNT)");
    assert_eq!(energy_label(12.0), "12 J (0.00 t of TNT)");
}

#[test]
fn masses_switch_to_tonnes_with_separators() {
    assert_eq!(mass_label(620.4), "620 kg");
    assert_eq!(mass_label(12_400_000.0), "12,400 t");
    assert_eq!(thousands(1_234_567.0), "1,234,567");
}

/// Earth and Mars, and the solar system's names.
fn earth_and_mars() -> (BodyId, BodyId, BodyNames) {
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
    (id("Earth"), id("Mars"), experiment.names)
}

#[test]
fn a_crewed_landing_reports_time_speed_energy_and_propellant() {
    let report = FlightReport {
        craft_name: "Crewed transfer (3 g)".into(),
        travel_time_s: Some(516.6 * 86_400.0),
        max_speed_m_s: 35_800.0,
        delta_v_m_s: 34_660.0,
        burn_time_s: 1_180.0,
        peak_acceleration_m_s2: 3.0 * STANDARD_GRAVITY_M_S2,
        energy_j: Some(6.7e11),
        propellant_kg: Some(201_000.0),
        touchdown_speed_m_s: Some(10.1),
        corrections: 11,
    };
    let (earth, mars, names) = earth_and_mars();
    let rows = arrival_rows(&report, earth, mars, &names);
    let value = |label: &str| {
        rows.iter()
            .find(|(l, _)| *l == label)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("no row {label}: {rows:?}"))
    };
    assert_eq!(value("Route"), "Earth → Mars");
    assert_eq!(value("Time of travel"), "1 y 151 d");
    assert_eq!(value("Maximum speed"), "35.8 km/s (0.012% of light)");
    assert_eq!(value("Speed change (Δv)"), "34.66 km/s");
    assert_eq!(value("Peak acceleration"), "3.00 g");
    assert_eq!(value("Energy used"), "670.00 GJ (160.13 t of TNT)");
    assert_eq!(value("Propellant burned"), "201 t");
    assert_eq!(value("Touchdown speed"), "10.1 m/s");
    assert_eq!(value("Correction burns"), "11");
}

#[test]
fn a_coasting_probe_has_no_engine_rows() {
    let report = FlightReport::new("Coasting probe (Voyager-like)", false);
    let (earth, mars, names) = earth_and_mars();
    let labels: Vec<_> = arrival_rows(&report, earth, mars, &names)
        .iter()
        .map(|(l, _)| *l)
        .collect();
    for engine_row in ["Energy used", "Propellant burned", "Peak acceleration"] {
        assert!(!labels.contains(&engine_row), "{labels:?}");
    }
    assert!(labels.contains(&"Speed change (Δv)"));
}

#[test]
fn the_popup_opens_once_per_landing_and_closes_when_it_ends() {
    let mars = (BodyId(4), 7_u64);
    let mars_again = (BodyId(4), 9_u64);
    assert_eq!(popup_change(None, Some(mars)), PopupChange::Open);
    assert_eq!(
        popup_change(Some(mars), Some(mars)),
        PopupChange::Keep,
        "a pop-up dismissed with Esc or ✕ stays closed for the rest of its landing"
    );
    assert_eq!(popup_change(Some(mars), None), PopupChange::Close);
    assert_eq!(popup_change(None, None), PopupChange::Keep);
    assert_eq!(
        popup_change(Some(mars), Some(mars_again)),
        PopupChange::Open,
        "a second trip to the same body is a new landing"
    );
}
