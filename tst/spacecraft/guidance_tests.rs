//! Tests for [`crate::spacecraft::guidance`], `#[path]`-included as a unit-test module of it, not
//! an integration test: the landing law, velocity-matching burns, detours, and touchdown.

use super::*;

/// An Aerojet 260 at full thrust on `15 t`.
const FULL: f64 = 26.2e6 / 15_000.0;

fn still(position_m: DVec3, radius_m: f64) -> Sighting {
    Sighting {
        position_m,
        velocity_m_s: DVec3::ZERO,
        radius_m,
    }
}

/// The full-thrust direct law, landing on a body with no surface gravity.
fn direct() -> Law {
    Law {
        acceleration_m_s2: FULL,
        braking_m_s2: braking_m_s2(FULL, 0.0),
        may_speed_up: true,
        avoid: true,
    }
}

#[test]
fn the_braking_budget_is_a_share_of_what_is_left_after_holding_against_gravity() {
    assert!((braking_m_s2(29.42, 0.0) - POWERED_BRAKE_FRACTION * 29.42).abs() < 1e-12);
    assert!((braking_m_s2(29.42, 9.81) - POWERED_BRAKE_FRACTION * 19.61).abs() < 1e-12);
    // At 3 g, Jupiter's 24.8 m/s² leaves only a little; the Sun's 274 m/s² leaves nothing.
    assert!(braking_m_s2(29.42, 24.8) > 0.0);
    assert!(braking_m_s2(29.42, 274.0) < 0.0);
}

#[test]
fn far_from_the_target_a_direct_flight_burns_at_full_thrust_toward_it() {
    let target = still(DVec3::new(1.0e11, 0.0, 0.0), 3.4e6);
    let s = steer(DVec3::ZERO, DVec3::ZERO, &target, &[], &direct(), 60.0);
    assert!((s.acceleration_m_s2.length() - FULL).abs() < 1.0e-6);
    assert!(s.acceleration_m_s2.normalize().dot(DVec3::X) > 0.999_999);
    assert_eq!(s.substep_s, POWERED_MAX_SUBSTEP_S);
}

#[test]
fn a_landing_never_speeds_the_craft_up() {
    // Far away and slow, a direct flight would burn toward the target; a landing coasts.
    let target = still(DVec3::new(1.0e11, 0.0, 0.0), 3.4e6);
    let landing = Law {
        may_speed_up: false,
        avoid: false,
        ..direct()
    };
    let v = DVec3::new(1_000.0, 0.0, 0.0);
    let s = steer(DVec3::ZERO, v, &target, &[], &landing, 60.0);
    assert!(
        s.acceleration_m_s2.length() < 1.0e-6,
        "{:?}",
        s.acceleration_m_s2
    );
}

#[test]
fn a_landing_turns_a_sideways_drift_toward_the_target() {
    let target = still(DVec3::new(1.0e9, 0.0, 0.0), 3.4e6);
    let landing = Law {
        may_speed_up: false,
        avoid: false,
        ..direct()
    };
    let drifting = DVec3::new(5_000.0, 2_000.0, 0.0);
    let s = steer(DVec3::ZERO, drifting, &target, &[], &landing, 60.0);
    assert!(
        s.acceleration_m_s2.y < 0.0,
        "it cancels the drift: {:?}",
        s.acceleration_m_s2
    );
}

#[test]
fn a_craft_faster_than_it_can_stop_from_brakes_at_full_thrust() {
    // 1e9 m from the surface, braking at 0.8 a allows about 1,672 km/s; this craft does 3,000.
    let target = still(DVec3::new(1.0e9 + 3.4e6, 0.0, 0.0), 3.4e6);
    let s = steer(
        DVec3::ZERO,
        DVec3::new(3.0e6, 0.0, 0.0),
        &target,
        &[],
        &direct(),
        60.0,
    );
    assert!(
        s.acceleration_m_s2.x < -FULL + 1.0,
        "{:?}",
        s.acceleration_m_s2
    );
}

#[test]
fn on_its_braking_curve_the_craft_coasts_and_it_touches_down_at_ten_m_s() {
    let radius = 1.0e6;
    let law = direct();
    // 5 km up, the curve's speed is √(2 · a_brake · 5,000 + 10²).
    let on_curve = (2.0 * law.braking_m_s2 * 5_000.0 + 100.0_f64).sqrt();
    let target = still(DVec3::new(radius + 5_000.0, 0.0, 0.0), radius);
    let s = steer(
        DVec3::ZERO,
        DVec3::new(on_curve, 0.0, 0.0),
        &target,
        &[],
        &law,
        60.0,
    );
    assert!(
        s.acceleration_m_s2.length() < 1.0e-6,
        "{:?}",
        s.acceleration_m_s2
    );
    // At the surface only the touchdown speed is wanted.
    let at_surface = still(DVec3::new(radius, 0.0, 0.0), radius);
    let s = steer(
        DVec3::ZERO,
        DVec3::new(POWERED_TOUCHDOWN_SPEED_M_S, 0.0, 0.0),
        &at_surface,
        &[],
        &law,
        60.0,
    );
    assert!(s.acceleration_m_s2.length() < 1.0e-6);
}

#[test]
fn the_step_shrinks_toward_the_surface_and_never_exceeds_the_offer() {
    let target = still(DVec3::new(1.0e6 + 10_000.0, 0.0, 0.0), 1.0e6);
    let s = steer(
        DVec3::ZERO,
        DVec3::new(100.0, 0.0, 0.0),
        &target,
        &[],
        &direct(),
        60.0,
    );
    assert!((s.substep_s - POWERED_SUBSTEP_FRACTION * 100.0).abs() < 1.0e-9);
    let s = steer(DVec3::ZERO, DVec3::ZERO, &target, &[], &direct(), 0.5);
    assert!(s.substep_s <= 0.5);
}

#[test]
fn a_strong_engine_gets_finer_steps_at_the_very_end() {
    // At 178 g the curve tightens over v_touch / a_brake, about 7 ms, so its floor is far finer.
    let radius = 1.0e6;
    let at_surface = still(DVec3::new(radius, 0.0, 0.0), radius);
    let v = DVec3::new(POWERED_TOUCHDOWN_SPEED_M_S, 0.0, 0.0);
    let strong = steer(DVec3::ZERO, v, &at_surface, &[], &direct(), 60.0);
    assert!(
        strong.substep_s < POWERED_MIN_SUBSTEP_S,
        "{}",
        strong.substep_s
    );
    let gentle = Law {
        acceleration_m_s2: 29.42,
        braking_m_s2: braking_m_s2(29.42, 0.0),
        ..direct()
    };
    let crewed = steer(DVec3::ZERO, v, &at_surface, &[], &gentle, 60.0);
    // Each floor is a fiftieth of `v_touch / a_brake`, capped at the fixed floor.
    let floor = |law: &Law| {
        POWERED_MIN_SUBSTEP_S
            .min(POWERED_SUBSTEP_FRACTION * POWERED_TOUCHDOWN_SPEED_M_S / law.braking_m_s2)
    };
    assert!((crewed.substep_s - floor(&gentle)).abs() < 1e-12);
    assert!((strong.substep_s - floor(&direct())).abs() < 1e-12);
    assert!(strong.substep_s < crewed.substep_s);
}

#[test]
fn a_velocity_matching_burn_converges_even_against_jupiters_gravity() {
    // Hold 1 km/s against 24.8 m/s² at 3 g: with the gravity fed forward, the error closes.
    let gravity = DVec3::new(0.0, -24.8, 0.0);
    let goal = DVec3::new(1_000.0, 0.0, 0.0);
    let mut v = DVec3::ZERO;
    for _ in 0..100_000 {
        if (goal - v).length() <= crate::spacecraft::constants::BURN_DONE_M_S {
            return;
        }
        let s = match_velocity(v, goal, gravity, 29.42, f64::INFINITY);
        assert!(s.acceleration_m_s2.length() <= 29.42 + 1e-9);
        v += (s.acceleration_m_s2 + gravity) * s.substep_s;
    }
    panic!("the burn never finished: {v:?}");
}

#[test]
fn a_body_on_the_line_is_flown_around_and_a_clear_line_is_not() {
    let sun = still(DVec3::new(5.0e10, 1.0e8, 0.0), 7.0e8);
    let to = DVec3::new(1.0e11, 0.0, 0.0);
    let aim = blocking(DVec3::ZERO, to, &[sun]).expect("the Sun is on the line");
    let from_sun = aim - sun.position_m;
    assert!((from_sun.length() - POWERED_AIM_CLEARANCE_RADII * sun.radius_m).abs() < 1.0);
    assert!(
        from_sun.y < 0.0,
        "it passes on the side the line already does"
    );

    let far = still(DVec3::new(5.0e10, 1.0e10, 0.0), 7.0e8);
    assert!(blocking(DVec3::ZERO, to, &[far]).is_none());
    let behind = still(DVec3::new(-5.0e10, 0.0, 0.0), 7.0e8);
    assert!(
        blocking(DVec3::ZERO, to, &[behind]).is_none(),
        "only bodies between count"
    );
}

#[test]
fn the_touchdown_test_finds_the_entry_point() {
    let entry = segment_entry_m(DVec3::new(3.0, 0.0, 0.0), DVec3::new(-3.0, 0.0, 0.0), 1.0)
        .expect("the segment crosses the sphere");
    assert!((entry - DVec3::X).length() < 1.0e-12);
    assert!(segment_entry_m(DVec3::new(3.0, 2.0, 0.0), DVec3::new(-3.0, 2.0, 0.0), 1.0).is_none());
    assert!(segment_entry_m(DVec3::new(3.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 0.0), 1.0).is_none());
}
