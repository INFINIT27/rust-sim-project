//! Tests for [`crate::spacecraft::config`], `#[path]`-included as a unit-test module of it, not an
//! integration test: the built-in craft, their limits, the fleet and its selection, and the kind of trip.

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
fn the_default_fleet_starts_on_the_crewed_transfer_and_selecting_the_next_cycles_round() {
    let mut fleet = Fleet::default();
    let names: Vec<_> = fleet.craft().iter().map(|c| c.name.clone()).collect();
    assert_eq!(names.len(), 4);
    assert_eq!(fleet.selected(), &Spacecraft::crewed_transfer());
    for step in 1..=4 {
        fleet.select_next();
        assert_eq!(fleet.selected().name, names[step % 4]);
        assert_eq!(fleet.selected_index(), step % 4);
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

// ---------------------------------------------------------------------------------------------
// The staged rocket
// ---------------------------------------------------------------------------------------------

#[test]
fn the_saturn_lander_is_four_stages_under_a_two_tonne_payload() {
    let craft = Spacecraft::saturn_v_lander();
    assert!(craft.is_valid());
    assert_eq!(craft.mass_kg, 2_150.0, "the payload alone");
    let Propulsion::Staged { stages, .. } = &craft.propulsion else {
        panic!("{:?}", craft.propulsion);
    };
    let names: Vec<_> = stages.iter().map(|stage| stage.name.as_str()).collect();
    assert_eq!(names, ["S-IC", "S-II", "S-IVB", "LM descent stage"]);
    // It launches at 1,360 times its payload.
    assert!((craft.launch_mass_kg() - 2_921_632.0).abs() < 1.0);
    assert_eq!(craft.prop().mass_kg, craft.launch_mass_kg() as f32);
}

#[test]
fn a_staged_craft_reports_its_first_engine_and_its_launch_acceleration() {
    let craft = Spacecraft::saturn_v_lander();
    assert_eq!(craft.engine().unwrap().name, "5 × F-1");
    let g = craft.acceleration_m_s2().unwrap() / STANDARD_GRAVITY_M_S2;
    assert!((g - 1.356).abs() < 0.005, "{g} g");
    assert!(craft.rocket().is_some());
}

#[test]
fn an_idealised_craft_has_no_rocket_and_launches_at_its_own_mass() {
    for craft in [
        Spacecraft::crewed_transfer(),
        Spacecraft::full_thrust_direct(),
        Spacecraft::coasting_probe(),
    ] {
        assert!(craft.rocket().is_none(), "{}", craft.name);
        assert_eq!(craft.launch_mass_kg(), craft.mass_kg);
    }
}

#[test]
fn the_real_engines_have_their_published_exhaust_speeds() {
    assert!((Engine::f1_cluster().exhaust_speed_m_s - 2_981.2).abs() < 0.1);
    assert!((Engine::j2().exhaust_speed_m_s - 4_128.6).abs() < 0.1);
    assert_eq!(
        Engine::j2_cluster().exhaust_speed_m_s,
        Engine::j2().exhaust_speed_m_s
    );
    assert!((Engine::j2_cluster().thrust_n / Engine::j2().thrust_n - 5.0).abs() < 1e-12);
    assert!((Engine::lunar_module_descent().exhaust_speed_m_s - 3_049.9).abs() < 0.1);
}

#[test]
fn a_staged_craft_needs_stages_propellant_and_a_limit() {
    let broken = |edit: &dyn Fn(&mut Vec<Stage>, &mut f64)| {
        let mut craft = Spacecraft::saturn_v_lander();
        if let Propulsion::Staged {
            stages,
            max_acceleration_m_s2,
        } = &mut craft.propulsion
        {
            edit(stages, max_acceleration_m_s2);
        }
        !craft.is_valid()
    };
    assert!(broken(&|stages, _| stages.clear()), "no stages");
    assert!(
        broken(&|stages, _| stages.iter_mut().for_each(|s| s.propellant_kg = 0.0)),
        "no propellant at all"
    );
    assert!(
        broken(&|stages, _| stages[1].dry_mass_kg = 0.0),
        "a massless stage"
    );
    assert!(broken(&|stages, _| stages[0].propellant_kg = -1.0));
    assert!(broken(&|stages, _| stages[2].engine.thrust_n = f64::NAN));
    assert!(broken(&|_, limit| *limit = 0.0), "no limit");
    // One empty stage among full ones is allowed: it is dropped at once.
    assert!(!broken(&|stages, _| stages[0].propellant_kg = 0.0));
}

// ---------------------------------------------------------------------------------------------
// The kind of trip
// ---------------------------------------------------------------------------------------------

#[test]
fn a_trip_lands_by_default_and_each_kind_has_a_name_and_a_label() {
    assert_eq!(Trip::default(), Trip::Land);
    assert_eq!(Trip::ALL, [Trip::Land, Trip::Return]);
    for trip in Trip::ALL {
        assert_eq!(Trip::from_name(trip.name()), Some(trip));
        assert!(!trip.label().is_empty());
    }
    assert_eq!(Trip::Land.name(), "land");
    assert_eq!(Trip::Return.name(), "return");
    assert_ne!(Trip::Land.label(), Trip::Return.label());
    assert_eq!(Trip::from_name("orbit"), None);
}

#[test]
fn a_fleet_carries_the_trip_the_next_launch_flies() {
    let mut fleet = Fleet::default();
    assert_eq!(fleet.trip(), Trip::Land, "the built-in fleet lands");
    fleet.set_trip(Trip::Return);
    assert_eq!(fleet.trip(), Trip::Return);
    // Choosing another craft keeps the trip.
    fleet.select_next();
    assert_eq!(fleet.trip(), Trip::Return);
    // A fleet built from a list lands until told otherwise.
    let fleet = Fleet::new(vec![Spacecraft::coasting_probe()]).unwrap();
    assert_eq!(fleet.trip(), Trip::Land);
}
