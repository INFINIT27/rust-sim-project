//! Tests for [`crate::spacecraft::rocket`], `#[path]`-included as a unit-test module of it, not an
//! integration test: the rocket equation in both directions, the tanks draining by it, staging,
//! the acceleration rising as the mass falls, and running dry. Whole flights of a rocket are flown
//! in `tst/spacecraft/plan_tests.rs`.

use super::*;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::spacecraft::Spacecraft;

fn engine(thrust_n: f64, exhaust_speed_m_s: f64) -> Engine {
    Engine {
        name: "test engine".to_string(),
        thrust_n,
        exhaust_speed_m_s,
    }
}

fn stage(dry_mass_kg: f64, propellant_kg: f64, thrust_n: f64, exhaust_speed_m_s: f64) -> Stage {
    Stage {
        name: "test stage".to_string(),
        dry_mass_kg,
        propellant_kg,
        engine: engine(thrust_n, exhaust_speed_m_s),
    }
}

/// One stage: 1 t of payload, 1 t of structure, 8 t of propellant, 3 km/s exhaust, 200 kN.
fn single() -> Rocket {
    Rocket::new(
        1_000.0,
        vec![stage(1_000.0, 8_000.0, 200_000.0, 3_000.0)],
        f64::INFINITY,
    )
}

/// Two stages under 1 t of payload.
fn two_stage() -> Rocket {
    Rocket::new(
        1_000.0,
        vec![
            stage(2_000.0, 20_000.0, 500_000.0, 3_000.0),
            stage(500.0, 4_000.0, 50_000.0, 4_000.0),
        ],
        f64::INFINITY,
    )
}

fn saturn() -> Rocket {
    Spacecraft::saturn_v_lander().rocket().unwrap()
}

// ---------------------------------------------------------------------------------------------
// The rocket equation
// ---------------------------------------------------------------------------------------------

#[test]
fn the_rocket_equation_is_exhaust_speed_times_the_log_of_the_mass_ratio() {
    // A mass ratio of e gives exactly the exhaust speed; e² gives twice it.
    let e = std::f64::consts::E;
    assert!((rocket_delta_v_m_s(3_000.0, e, 1.0) - 3_000.0).abs() < 1e-9);
    assert!((rocket_delta_v_m_s(3_000.0, e * e, 1.0) - 6_000.0).abs() < 1e-9);
    // No propellant burned, no speed change.
    assert_eq!(rocket_delta_v_m_s(3_000.0, 5.0, 5.0), 0.0);
}

#[test]
fn the_two_forms_of_the_rocket_equation_undo_each_other() {
    for delta_v in [1.0, 250.0, 3_000.0, 12_000.0] {
        let left = rocket_mass_after_kg(50_000.0, delta_v, 4_128.0);
        assert!(left < 50_000.0);
        assert!((rocket_delta_v_m_s(4_128.0, 50_000.0, left) - delta_v).abs() < 1e-8);
    }
}

#[test]
fn nonsense_gives_no_speed_change_and_no_mass_loss() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(rocket_delta_v_m_s(bad, 2.0, 1.0), 0.0);
        assert_eq!(rocket_delta_v_m_s(3_000.0, bad, 1.0), 0.0);
        assert_eq!(rocket_delta_v_m_s(3_000.0, 2.0, bad), 0.0);
    }
    // A final mass above the initial one is not a burn.
    assert_eq!(rocket_delta_v_m_s(3_000.0, 1.0, 2.0), 0.0);
    for bad in [0.0, -1.0, f64::NAN] {
        assert_eq!(rocket_mass_after_kg(1_000.0, bad, 3_000.0), 1_000.0);
        assert_eq!(rocket_mass_after_kg(1_000.0, 100.0, bad), 1_000.0);
    }
}

// ---------------------------------------------------------------------------------------------
// One stage
// ---------------------------------------------------------------------------------------------

#[test]
fn a_full_rocket_weighs_its_payload_structure_and_propellant() {
    let rocket = single();
    assert_eq!(rocket.mass_kg(), 10_000.0);
    assert_eq!(rocket.propellant_left_kg(), 8_000.0);
    assert!(!rocket.is_dry());
    assert_eq!(rocket.stage_number(), (1, 1));
    assert_eq!(rocket.stages_dropped(), 0);
}

#[test]
fn the_total_speed_change_is_the_rocket_equation_over_the_tank() {
    // 10 t down to 2 t at 3 km/s: 3000 · ln 5.
    let rocket = single();
    let expected = 3_000.0 * 5.0_f64.ln();
    assert!((rocket.delta_v_left_m_s() - expected).abs() < 1e-9);
}

#[test]
fn a_burn_drains_the_tank_by_the_rocket_equation() {
    let mut rocket = single();
    let burn = rocket.burn(1_000.0);

    assert!((burn.delta_v_m_s - 1_000.0).abs() < 1e-9);
    let expected_mass = 10_000.0 * (-1_000.0_f64 / 3_000.0).exp();
    assert!((rocket.mass_kg() - expected_mass).abs() < 1e-6);
    assert!((burn.propellant_kg - (10_000.0 - expected_mass)).abs() < 1e-6);
    // The exhaust carries ½ m v_e².
    assert!((burn.energy_j - 0.5 * burn.propellant_kg * 3_000.0 * 3_000.0).abs() < 1e-3);
    assert_eq!(burn.stages_dropped, 0);
}

#[test]
fn the_same_speed_change_costs_less_propellant_from_a_lighter_rocket() {
    let mut rocket = single();
    let first = rocket.burn(1_000.0).propellant_kg;
    let second = rocket.burn(1_000.0).propellant_kg;
    assert!(second < first, "{second} kg after {first} kg");
    // Each takes the same fraction of what the rocket weighed then.
    let fraction = 1.0 - (-1_000.0_f64 / 3_000.0).exp();
    assert!((first / 10_000.0 - fraction).abs() < 1e-12);
    assert!((second / (10_000.0 - first) - fraction).abs() < 1e-12);
}

#[test]
fn burning_in_pieces_ends_where_one_burn_does() {
    let mut whole = single();
    whole.burn(3_000.0);
    let mut pieces = single();
    for _ in 0..3_000 {
        pieces.burn(1.0);
    }
    assert!((whole.mass_kg() - pieces.mass_kg()).abs() < 1e-6);
}

#[test]
fn the_acceleration_rises_as_the_tank_empties() {
    let mut rocket = single();
    // 200 kN on 10 t.
    assert!((rocket.acceleration_m_s2() - 20.0).abs() < 1e-12);
    rocket.burn(2_000.0);
    let lighter = rocket.acceleration_m_s2();
    assert!((lighter - 200_000.0 / rocket.mass_kg()).abs() < 1e-12);
    assert!(lighter > 20.0);
}

#[test]
fn the_limit_caps_the_acceleration() {
    let mut rocket = Rocket::new(
        1_000.0,
        vec![stage(1_000.0, 8_000.0, 200_000.0, 3_000.0)],
        25.0,
    );
    assert!(
        (rocket.acceleration_m_s2() - 20.0).abs() < 1e-12,
        "under the limit at launch"
    );
    rocket.burn(2_000.0);
    assert_eq!(rocket.acceleration_m_s2(), 25.0, "held to it once lighter");
}

#[test]
fn a_full_thrust_burn_takes_the_propellant_over_the_flow_rate() {
    // 200 kN at 3 km/s burns 66.67 kg a second.
    let mut rocket = single();
    let burn = rocket.burn(1_000.0);
    assert!((burn.seconds - burn.propellant_kg * 3_000.0 / 200_000.0).abs() < 1e-9);
    // And it is quicker than the launch acceleration alone would suggest, because the
    // acceleration rose during it.
    assert!(burn.seconds < 1_000.0 / 20.0);
}

#[test]
fn a_throttled_burn_takes_the_speed_change_over_the_limit() {
    let mut rocket = Rocket::new(
        1_000.0,
        vec![stage(1_000.0, 8_000.0, 200_000.0, 3_000.0)],
        5.0,
    );
    let burn = rocket.burn(1_000.0);
    assert!((burn.seconds - 200.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------------
// Running dry
// ---------------------------------------------------------------------------------------------

#[test]
fn a_rocket_gives_only_the_speed_change_its_tank_holds() {
    let mut rocket = single();
    let all = rocket.delta_v_left_m_s();
    let burn = rocket.burn(all + 5_000.0);

    assert!((burn.delta_v_m_s - all).abs() < 1e-9);
    assert!((burn.propellant_kg - 8_000.0).abs() < 1e-9);
    assert!(rocket.is_dry());
    assert_eq!(rocket.propellant_left_kg(), 0.0);
    assert_eq!(rocket.mass_kg(), 2_000.0, "the payload and the empty stage");
    assert_eq!(rocket.acceleration_m_s2(), 0.0);
    assert_eq!(rocket.delta_v_left_m_s(), 0.0);
}

#[test]
fn a_dry_rocket_burns_nothing() {
    let mut rocket = single();
    rocket.burn(f64::INFINITY);
    let burn = rocket.burn(100.0);
    assert_eq!(burn, Burn::default());
}

#[test]
fn a_burn_of_nothing_does_nothing() {
    let mut rocket = single();
    for nothing in [0.0, -5.0, f64::NAN] {
        assert_eq!(rocket.burn(nothing), Burn::default());
        assert_eq!(rocket.plan_burn(nothing), 0.0);
    }
    assert_eq!(rocket.mass_kg(), 10_000.0);
}

#[test]
fn the_last_stage_is_kept_when_it_empties() {
    let mut rocket = two_stage();
    let burn = rocket.burn(f64::INFINITY);
    assert_eq!(
        burn.stages_dropped, 1,
        "the first is dropped, the last is the craft"
    );
    assert_eq!(rocket.stages_dropped(), 1);
    assert_eq!(rocket.mass_kg(), 1_500.0);
    assert_eq!(rocket.stage_number(), (2, 2));
}

#[test]
fn a_plan_carries_on_past_dry_at_the_last_acceleration() {
    let mut rocket = single();
    let all = rocket.delta_v_left_m_s();
    let to_dry = rocket.clone().burn(all).seconds;
    // 200 kN on the 2 t left is 100 m/s².
    let seconds = rocket.plan_burn(all + 1_000.0);
    assert!((seconds - (to_dry + 10.0)).abs() < 1e-6, "{seconds} s");
}

// ---------------------------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------------------------

#[test]
fn the_total_speed_change_is_the_sum_over_the_stages_each_carrying_what_is_above() {
    let rocket = two_stage();
    // First stage: 27.5 t down to 7.5 t. Second: 5.5 t down to 1.5 t.
    let expected = 3_000.0 * (27_500.0_f64 / 7_500.0).ln() + 4_000.0 * (5_500.0_f64 / 1_500.0).ln();
    assert!((rocket.delta_v_left_m_s() - expected).abs() < 1e-8);
}

#[test]
fn an_empty_stage_is_dropped_and_the_next_one_lights() {
    let mut rocket = two_stage();
    let first = 3_000.0 * (27_500.0_f64 / 7_500.0).ln();

    let burn = rocket.burn(first);
    assert_eq!(burn.stages_dropped, 0, "empty, but not yet dropped");
    assert!((rocket.mass_kg() - 7_500.0).abs() < 1e-6);

    // The next burn drops the empty stage: 2 t of structure gone at once.
    let burn = rocket.burn(1.0);
    assert_eq!(burn.stages_dropped, 1);
    assert_eq!(rocket.stage_number(), (2, 2));
    assert!(rocket.mass_kg() < 5_500.0 && rocket.mass_kg() > 5_490.0);
    assert_eq!(rocket.engine().unwrap().exhaust_speed_m_s, 4_000.0);
}

#[test]
fn a_burn_across_a_staging_uses_each_stage_for_its_share() {
    let mut rocket = two_stage();
    let first = 3_000.0 * (27_500.0_f64 / 7_500.0).ln();
    let burn = rocket.burn(first + 1_000.0);

    assert!((burn.delta_v_m_s - (first + 1_000.0)).abs() < 1e-9);
    assert_eq!(burn.stages_dropped, 1);
    let second_burned = 5_500.0 * (1.0 - (-1_000.0_f64 / 4_000.0).exp());
    assert!((burn.propellant_kg - (20_000.0 + second_burned)).abs() < 1e-6);
    assert!((rocket.propellant_left_kg() - (4_000.0 - second_burned)).abs() < 1e-6);
}

#[test]
fn staging_beats_one_stage_with_the_same_propellant_and_structure() {
    // The reason rockets stage: the same 24 t of propellant and 2.5 t of structure, in one piece.
    let lumped = Rocket::new(
        1_000.0,
        vec![stage(2_500.0, 24_000.0, 500_000.0, 3_000.0)],
        f64::INFINITY,
    );
    let staged = Rocket::new(
        1_000.0,
        vec![
            stage(2_000.0, 20_000.0, 500_000.0, 3_000.0),
            stage(500.0, 4_000.0, 50_000.0, 3_000.0),
        ],
        f64::INFINITY,
    );
    assert!(staged.delta_v_left_m_s() > lumped.delta_v_left_m_s() + 500.0);
}

#[test]
fn the_acceleration_drops_when_a_weaker_stage_lights() {
    let mut rocket = two_stage();
    let first = 3_000.0 * (27_500.0_f64 / 7_500.0).ln();
    rocket.burn(first - 1.0);
    let before = rocket.acceleration_m_s2();
    rocket.burn(2.0);
    let after = rocket.acceleration_m_s2();
    assert!(before > 60.0, "500 kN on 7.5 t: {before}");
    assert!(after < 10.0, "50 kN on 5.5 t: {after}");
}

#[test]
fn the_last_stage_acceleration_is_its_thrust_on_its_full_mass() {
    let rocket = two_stage();
    assert!((rocket.last_stage_acceleration_m_s2() - 50_000.0 / 5_500.0).abs() < 1e-12);
}

// ---------------------------------------------------------------------------------------------
// The built-in Saturn V lander
// ---------------------------------------------------------------------------------------------

#[test]
fn the_saturn_stack_weighs_what_its_parts_add_up_to() {
    let rocket = saturn();
    assert!(
        (rocket.mass_kg() - 2_921_632.0).abs() < 1.0,
        "{}",
        rocket.mass_kg()
    );
    assert_eq!(rocket.stage_number(), (1, 4));
    // Propellant is 93.5% of it.
    let fraction = rocket.propellant_left_kg() / rocket.mass_kg();
    assert!((fraction - 0.935).abs() < 0.002, "{fraction}");
}

#[test]
fn the_saturn_stages_give_the_speed_changes_the_constants_document() {
    let mut rocket = saturn();
    let expected_km_s = [4.01, 5.29, 6.82, 3.32];
    let mut total = 0.0;
    for (index, expected) in expected_km_s.into_iter().enumerate() {
        // Burn the current stage to empty: everything it has, and no more.
        let mass = rocket.mass_kg();
        let stage = rocket.stage().unwrap().clone();
        let delta_v = rocket_delta_v_m_s(
            stage.engine.exhaust_speed_m_s,
            mass,
            mass - stage.propellant_kg,
        );
        assert!(
            (delta_v / 1_000.0 - expected).abs() < 0.01,
            "stage {}: {delta_v} m/s",
            index + 1
        );
        total += delta_v;
        rocket.burn(delta_v);
        // Light the next one.
        rocket.burn(1e-9);
    }
    assert!((total / 1_000.0 - 19.44).abs() < 0.01, "{total} m/s in all");
    assert!((saturn().delta_v_left_m_s() - total).abs() < 1.0);
}

#[test]
fn the_saturn_stack_lifts_off_at_1_35_g_and_is_held_to_3() {
    let mut rocket = saturn();
    let g = rocket.acceleration_m_s2() / STANDARD_GRAVITY_M_S2;
    assert!((g - 1.355).abs() < 0.005, "{g} g");

    // By the end of the first stage the thrust could give over 5 g; the crew limit holds it.
    rocket.burn(3_900.0);
    assert!((rocket.acceleration_m_s2() / STANDARD_GRAVITY_M_S2 - 3.0).abs() < 1e-12);

    // The second stage lights under one g.
    rocket.burn(200.0);
    assert_eq!(rocket.stage_number(), (2, 4));
    let g = rocket.acceleration_m_s2() / STANDARD_GRAVITY_M_S2;
    assert!(g < 1.0 && g > 0.8, "{g} g");
}

#[test]
fn the_lander_stage_can_hold_itself_up_at_the_moon() {
    // 45 kN on the descent stage and the empty ascent stage is 3.6 m/s², against the Moon's 1.62.
    let acceleration = saturn().last_stage_acceleration_m_s2();
    assert!((acceleration - 3.623).abs() < 0.01, "{acceleration}");
}

// ---------------------------------------------------------------------------------------------
// Looking ahead to the weakest stage
// ---------------------------------------------------------------------------------------------

#[test]
fn the_weakest_acceleration_within_a_stage_is_the_present_one() {
    let rocket = two_stage();
    let now = rocket.acceleration_m_s2();
    assert_eq!(rocket.weakest_acceleration_m_s2(0.0), now);
    assert_eq!(rocket.weakest_acceleration_m_s2(1_000.0), now);
}

#[test]
fn the_weakest_acceleration_across_a_staging_is_the_weaker_stage_as_it_lights() {
    let rocket = two_stage();
    let first = 3_000.0 * (27_500.0_f64 / 7_500.0).ln();
    let weakest = rocket.weakest_acceleration_m_s2(first + 100.0);
    assert!((weakest - 50_000.0 / 5_500.0).abs() < 1e-6, "{weakest}");
}

#[test]
fn a_look_ahead_past_dry_is_judged_on_the_stages_there_are() {
    let rocket = two_stage();
    let weakest = rocket.weakest_acceleration_m_s2(1.0e9);
    assert!((weakest - 50_000.0 / 5_500.0).abs() < 1e-6, "{weakest}");

    let mut dry = two_stage();
    dry.burn(f64::INFINITY);
    assert_eq!(dry.weakest_acceleration_m_s2(100.0), 0.0);
}
