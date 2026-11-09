//! Tests for [`crate::spacecraft::config`], `#[path]`-included as a unit-test module of it, not an
//! integration test: the built-in craft, their limits, and the fleet `G` cycles.

use super::*;

#[test]
fn the_crewed_craft_is_held_to_3_g_though_its_engine_could_give_178() {
    let craft = Spacecraft::crewed_transfer();
    let engine = craft.engine().unwrap();
    assert!((engine.thrust_n / craft.mass_kg / STANDARD_GRAVITY_M_S2 - 178.1).abs() < 0.1);
    let g = craft.acceleration_m_s2().unwrap() / STANDARD_GRAVITY_M_S2;
    assert!((g - 3.0).abs() < 1e-12, "{g} g");
    assert!(matches!(craft.propulsion, Propulsion::Transfer { .. }));
}

#[test]
fn the_full_thrust_craft_is_not_limited_and_the_probe_has_no_engine() {
    let full = Spacecraft::full_thrust_direct();
    let g = full.acceleration_m_s2().unwrap() / STANDARD_GRAVITY_M_S2;
    assert!((g - 178.1).abs() < 0.1, "{g} g");
    let probe = Spacecraft::coasting_probe();
    assert!(probe.engine().is_none() && probe.acceleration_m_s2().is_none());
}

#[test]
fn an_engine_too_weak_for_its_limit_gives_only_what_it_has() {
    let mut craft = Spacecraft::crewed_transfer();
    craft.mass_kg = 26.2e6 / 10.0; // 10 m/s² of thrust against a 29.4 m/s² limit.
    assert!((craft.acceleration_m_s2().unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn the_aerojet_exhaust_speed_is_its_263_s_specific_impulse() {
    let engine = Engine::aerojet_260();
    assert!((engine.exhaust_speed_m_s - 2_579.2).abs() < 0.1);
}

#[test]
fn every_built_in_craft_is_valid_and_a_broken_one_is_not() {
    for craft in Fleet::default().craft() {
        assert!(craft.is_valid(), "{}", craft.name);
    }
    let mut broken = Spacecraft::crewed_transfer();
    broken.launch_altitude_m = -1.0;
    assert!(!broken.is_valid());
    let mut broken = Spacecraft::coasting_probe();
    broken.propulsion = Propulsion::Coasting {
        max_departure_speed_m_s: f64::NAN,
    };
    assert!(!broken.is_valid());
}

#[test]
fn the_default_fleet_starts_on_the_crewed_transfer_and_g_cycles_round() {
    let mut fleet = Fleet::default();
    let names: Vec<_> = fleet.craft().iter().map(|c| c.name.clone()).collect();
    assert_eq!(names.len(), 3);
    assert_eq!(fleet.selected(), &Spacecraft::crewed_transfer());
    for step in 1..=3 {
        fleet.select_next();
        assert_eq!(fleet.selected().name, names[step % 3]);
        assert_eq!(fleet.selected_index(), step % 3);
    }
}

#[test]
fn a_fleet_must_have_a_valid_craft_and_selection_stays_in_range() {
    assert!(Fleet::new(Vec::new()).is_none());
    let mut broken = Spacecraft::crewed_transfer();
    broken.mass_kg = 0.0;
    assert!(Fleet::new(vec![broken]).is_none());
    let mut fleet = Fleet::new(vec![Spacecraft::coasting_probe()]).unwrap();
    assert!(!fleet.select(1));
    assert!(fleet.select(0));
    fleet.select_next();
    assert_eq!(fleet.selected_index(), 0, "one craft wraps to itself");
}

#[test]
fn the_craft_prop_has_its_mass_and_size() {
    let craft = Spacecraft::crewed_transfer();
    let prop = craft.prop();
    assert_eq!(prop.mass_kg, 15_000.0);
    match prop.body {
        Body::Sphere { radius, .. } => {
            assert!((f64::from(radius) * METRES_PER_UNIT - craft.radius_m).abs() < 1e-6)
        }
        other => panic!("{other:?}"),
    }
}
