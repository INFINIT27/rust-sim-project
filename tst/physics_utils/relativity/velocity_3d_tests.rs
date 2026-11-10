//! Unit and property tests for [`crate::physics_utils::relativity::velocity_3d`].
//!
//! Compiled into `src/physics_utils/relativity/velocity_3d.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **On one axis this is `velocity.rs`.** The 3-D formula is written separately, so agreeing with
//!    the scalar one along `x`, `y` and `z` checks both.
//! 2. **The speed of light is invariant in every direction**, not only along the boost.
//! 3. **Composition does not commute, and the angle between the two orders is the Wigner
//!    rotation.** Checked against the closed form for perpendicular boosts, which the implementation
//!    does not use.
//! 4. **Aberration agrees with transforming a photon's velocity**, and with Bradley's `20.5″`.

use super::*;
use crate::physics_utils::relativity::velocity::{compose_velocities, transform_velocity};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use std::f64::consts::PI;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_velocity_3d_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

/// `β` for a light ray built from sines and cosines: four roundings below `1`, because
/// `sin² + cos²` can come out a rounding above `1` and the function rightly rejects a speed above
/// `c`. The difference from `c` is `9e-16` relative, far below every tolerance here.
const LIGHT: f64 = 1.0 - 4.0 * f64::EPSILON;

/// A velocity of `beta·c` in the direction given by two angles.
fn velocity(beta: f64, polar: f64, azimuth: f64) -> [f64; 3] {
    [
        beta * C * polar.sin() * azimuth.cos(),
        beta * C * polar.sin() * azimuth.sin(),
        beta * C * polar.cos(),
    ]
}

fn scaled(vector: [f64; 3], factor: f64) -> [f64; 3] {
    vector.map(|component| component * factor)
}

// -----------------------------------------------------------------------
// Agreement with the one-axis module.
// -----------------------------------------------------------------------

#[test]
fn along_each_axis_it_is_the_scalar_formula() {
    for axis in 0..3 {
        for (object, frame) in [(0.5, 0.3), (-0.9, 0.6), (0.2, -0.95), (1.0, 0.7)] {
            let mut u = [0.0; 3];
            let mut v = [0.0; 3];
            u[axis] = object * C;
            v[axis] = frame * C;

            let result = transform_velocity_3d(u, v).expect("valid");
            let expected = transform_velocity(object * C, frame * C).expect("valid");

            assert!(
                (result[axis] - expected).abs() < 1.0e-12 * C,
                "axis {axis}: {} against {}",
                result[axis] / C,
                expected / C
            );
            for other in (0..3).filter(|&other| other != axis) {
                assert_eq!(result[other], 0.0, "nothing appears off the axis");
            }

            let composed = compose_velocities_3d(u, v).expect("valid");
            let expected = compose_velocities(object * C, frame * C).expect("valid");
            assert!((composed[axis] - expected).abs() < 1.0e-12 * C);
        }
    }
}

#[test]
fn a_frame_at_rest_changes_nothing() {
    let u = [0.1 * C, -0.2 * C, 0.3 * C];
    let result = transform_velocity_3d(u, [0.0; 3]).expect("valid");
    for axis in 0..3 {
        assert!((result[axis] - u[axis]).abs() < 1.0e-15 * C);
    }
}

#[test]
fn an_object_at_rest_recedes_at_the_frame_speed() {
    let v = [0.3 * C, 0.4 * C, -0.5 * C];
    let result = transform_velocity_3d([0.0; 3], v).expect("valid");
    for axis in 0..3 {
        assert!((result[axis] + v[axis]).abs() < 1.0e-15 * C);
    }
}

#[test]
fn the_transverse_velocity_is_divided_by_gamma() {
    // The effect with no one-axis counterpart. A frame moving along x at 0.8c (gamma 5/3) sees a
    // body moving along y at 0.3c moving along y at 0.18c, although the frame has no y motion.
    let result = transform_velocity_3d([0.0, 0.3 * C, 0.0], [0.8 * C, 0.0, 0.0]).expect("valid");
    assert!((result[0] / C + 0.8).abs() < 1.0e-14);
    assert!((result[1] / C - 0.18).abs() < 1.0e-14);
    assert_eq!(result[2], 0.0);
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn a_frame_may_not_move_at_light_speed_but_an_object_may() {
    let light = [0.0, C, 0.0];
    assert!(transform_velocity_3d(light, [0.5 * C, 0.0, 0.0]).is_some());
    assert_eq!(transform_velocity_3d([0.0; 3], light), None);
    assert_eq!(compose_velocities_3d([0.0; 3], light), None);

    // A frame whose components are each below c but whose speed is not.
    assert_eq!(
        transform_velocity_3d([0.0; 3], [0.7 * C, 0.7 * C, 0.7 * C]),
        None
    );
    assert_eq!(
        transform_velocity_3d([0.7 * C, 0.7 * C, 0.7 * C], [0.0; 3]),
        None,
        "nor an object"
    );
}

#[test]
fn non_finite_components_have_no_answer() {
    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(transform_velocity_3d([bad, 0.0, 0.0], [0.0; 3]), None);
        assert_eq!(transform_velocity_3d([0.0; 3], [0.0, bad, 0.0]), None);
        assert_eq!(wigner_rotation_angle([0.0, 0.0, bad], [0.0; 3]), None);
        assert_eq!(aberration_angle(bad, 0.5 * C), None);
        assert_eq!(aberration_angle(1.0, bad), None);
    }
    assert_eq!(
        wigner_rotation_angle([C, 0.0, 0.0], [0.0, 0.5 * C, 0.0]),
        None
    );
    assert_eq!(
        wigner_rotation_angle([0.0, 0.5 * C, 0.0], [C, 0.0, 0.0]),
        None
    );
}

// -----------------------------------------------------------------------
// The Wigner rotation.
// -----------------------------------------------------------------------

#[test]
fn composition_in_two_directions_does_not_commute() {
    // 0.6c along x and 0.6c along y. One order gives (0.6, 0.48)c, the other (0.48, 0.6)c.
    let along_x = [0.6 * C, 0.0, 0.0];
    let along_y = [0.0, 0.6 * C, 0.0];

    let one_way = compose_velocities_3d(along_y, along_x).expect("valid");
    let other_way = compose_velocities_3d(along_x, along_y).expect("valid");

    assert!((one_way[0] / C - 0.6).abs() < 1.0e-14);
    assert!((one_way[1] / C - 0.48).abs() < 1.0e-14);
    assert!((other_way[0] / C - 0.48).abs() < 1.0e-14);
    assert!((other_way[1] / C - 0.6).abs() < 1.0e-14);
    assert!((speed(one_way) / speed(other_way) - 1.0).abs() < 1.0e-14);
}

#[test]
fn the_wigner_angle_matches_the_closed_form_for_perpendicular_boosts() {
    // tan(eps) = g1*g2*b1*b2 / (g1 + g2). Not the formula the implementation uses.
    for (first, second) in [
        (0.1, 0.1),
        (0.6, 0.6),
        (0.3, 0.9),
        (0.99, 0.5),
        (0.999, 0.999),
    ] {
        let gamma = |beta: f64| 1.0 / (1.0 - beta * beta).sqrt();
        let expected =
            (gamma(first) * gamma(second) * first * second / (gamma(first) + gamma(second))).atan();

        let angle =
            wigner_rotation_angle([first * C, 0.0, 0.0], [0.0, 0.0, second * C]).expect("valid");
        assert!(
            (angle - expected).abs() < 1.0e-12,
            "betas {first}, {second}: {angle} against {expected}"
        );
    }
}

#[test]
fn the_wigner_angle_at_orbital_speed_is_half_beta_squared() {
    // Two perpendicular boosts at Earth's orbital speed: beta^2/2 = 4.9e-9 rad, a milli-arcsecond.
    // An acos-based angle would return zero here.
    let v = 29_780.0;
    let angle = wigner_rotation_angle([v, 0.0, 0.0], [0.0, v, 0.0]).expect("valid");
    let expected = 0.5 * (v / C) * (v / C);
    assert!((angle / expected - 1.0).abs() < 1.0e-6, "got {angle:e}");
}

#[test]
fn parallel_and_trivial_boosts_do_not_rotate() {
    let v = [0.3 * C, 0.4 * C, 0.0];
    assert!(
        wigner_rotation_angle(v, scaled(v, 1.5))
            .expect("valid")
            .abs()
            < 1.0e-14
    );
    assert!(
        wigner_rotation_angle(v, scaled(v, -0.5))
            .expect("valid")
            .abs()
            < 1.0e-14
    );
    assert_eq!(wigner_rotation_angle(v, [0.0; 3]), Some(0.0));
    assert_eq!(wigner_rotation_angle([0.0; 3], v), Some(0.0));
}

#[test]
fn near_light_speed_the_wigner_angle_approaches_the_angle_between_the_boosts() {
    let beta = 0.999_999_9;
    let angle = wigner_rotation_angle([beta * C, 0.0, 0.0], [0.0, beta * C, 0.0]).expect("valid");
    assert!(
        (angle - PI / 2.0).abs() < 1.0e-3,
        "got {} degrees",
        angle.to_degrees()
    );
    assert!(angle < PI / 2.0);
}

// -----------------------------------------------------------------------
// Aberration.
// -----------------------------------------------------------------------

#[test]
fn bradleys_stellar_aberration_is_twenty_arcseconds() {
    // Light arriving at right angles to Earth's orbital motion. Its direction of travel shifts by
    // atan(gamma*beta) = 20.49 arcseconds: the constant of aberration.
    let angle = aberration_angle(PI / 2.0, 29_780.0).expect("valid");
    let shift_arcseconds = (angle - PI / 2.0).to_degrees() * 3_600.0;
    assert!(
        (shift_arcseconds - 20.49).abs() < 0.01,
        "got {shift_arcseconds}"
    );
}

#[test]
fn rays_along_the_motion_keep_their_direction() {
    for beta in [-0.9, -0.1, 0.0, 0.1, 0.9] {
        assert_eq!(aberration_angle(0.0, beta * C), Some(0.0));
        let backward = aberration_angle(PI, beta * C).expect("valid");
        assert!((backward - PI).abs() < 1.0e-15);
    }
}

#[test]
fn a_frame_at_rest_does_not_aberrate() {
    for angle in [0.0, 0.3, PI / 2.0, 2.5, PI] {
        let result = aberration_angle(angle, 0.0).expect("valid");
        assert!((result - angle).abs() < 1.0e-15);
    }
}

#[test]
fn the_sky_crowds_forward() {
    // Rays coming *from* ahead travel at more than 90 degrees to the motion. Half the sky (all rays
    // travelling at theta > 90 degrees in S) is, at 0.9c, squeezed into the rays travelling at more
    // than acos(-0.9) = 154 degrees: a cone of 26 degrees around the direction of motion.
    let edge = aberration_angle(PI / 2.0, 0.9 * C).expect("valid");
    assert!((edge.cos() + 0.9).abs() < 1.0e-14);
    assert!(((PI - edge).to_degrees() - 25.84).abs() < 0.01);
}

#[test]
fn angles_outside_zero_to_pi_are_rejected() {
    assert_eq!(aberration_angle(-0.1, 0.5 * C), None);
    assert_eq!(aberration_angle(PI + 0.1, 0.5 * C), None);
    assert_eq!(aberration_angle(1.0, C), None);
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// **Light moves at `c` in every frame, whatever its direction.**
    #[test]
    fn light_speed_is_invariant_in_every_direction(
        polar in 0.0_f64..PI,
        azimuth in 0.0_f64..(2.0 * PI),
        frame_beta in 0.0_f64..0.99,
        frame_polar in 0.0_f64..PI,
        frame_azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let light = velocity(LIGHT, polar, azimuth);

        let seen = transform_velocity_3d(light, velocity(frame_beta, frame_polar, frame_azimuth))
            .expect("valid");
        // 1 - u.v/c^2 is as small as 0.01 here and carries the rounding.
        prop_assert!((speed(seen) / C - 1.0).abs() < 1.0e-12, "light at {}c", speed(seen) / C);
    }

    /// Two subluminal velocities compose to a subluminal velocity, and both orders give the same
    /// speed.
    #[test]
    fn composition_stays_subluminal_and_its_speed_commutes(
        first_beta in 0.0_f64..0.99,
        first_polar in 0.0_f64..PI,
        first_azimuth in 0.0_f64..(2.0 * PI),
        second_beta in 0.0_f64..0.99,
        second_polar in 0.0_f64..PI,
        second_azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let first = velocity(first_beta, first_polar, first_azimuth);
        let second = velocity(second_beta, second_polar, second_azimuth);

        let one_way = compose_velocities_3d(first, second).expect("valid");
        let other_way = compose_velocities_3d(second, first).expect("valid");

        prop_assert!(speed(one_way) < C);
        // Compared as gamma, the invariant the two orders share: g = g1*g2*(1 + b1.b2).
        let gamma = |v: [f64; 3]| 1.0 / (1.0 - (speed(v) / C).powi(2)).sqrt();
        prop_assert!((gamma(one_way) / gamma(other_way) - 1.0).abs() < 1.0e-9);

        let dot = (first[0] * second[0] + first[1] * second[1] + first[2] * second[2]) / (C * C);
        let expected = gamma(first) * gamma(second) * (1.0 + dot);
        prop_assert!((gamma(one_way) / expected - 1.0).abs() < 1.0e-9);
    }

    /// Composing and then transforming by the same frame is a round trip.
    #[test]
    fn compose_then_transform_is_a_round_trip(
        object_beta in 0.0_f64..0.99,
        object_polar in 0.0_f64..PI,
        object_azimuth in 0.0_f64..(2.0 * PI),
        frame_beta in 0.0_f64..0.99,
        frame_polar in 0.0_f64..PI,
        frame_azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let object = velocity(object_beta, object_polar, object_azimuth);
        let frame = velocity(frame_beta, frame_polar, frame_azimuth);

        let there = compose_velocities_3d(object, frame).expect("valid");
        let back = transform_velocity_3d(there, frame).expect("valid");

        // Both steps divide by something as small as 1 - 0.99^2.
        for axis in 0..3 {
            prop_assert!(
                (back[axis] - object[axis]).abs() < 1.0e-9 * C,
                "axis {}: {} against {}", axis, back[axis] / C, object[axis] / C
            );
        }
    }

    /// **Aberration is what the velocity transformation does to a photon.** The ray travels in the
    /// x-y plane at `angle` to x; the frame moves along x.
    #[test]
    fn aberration_agrees_with_transforming_a_photon(
        angle in 0.0_f64..PI,
        beta in -0.99_f64..0.99,
    ) {
        let ray = [LIGHT * C * angle.cos(), LIGHT * C * angle.sin(), 0.0];

        let seen = transform_velocity_3d(ray, [beta * C, 0.0, 0.0]).expect("valid");
        let from_velocity = seen[1].atan2(seen[0]);
        let from_formula = aberration_angle(angle, beta * C).expect("valid");

        prop_assert!(
            (from_velocity - from_formula).abs() < 1.0e-10,
            "{} against {}", from_velocity, from_formula
        );
    }

    /// Aberration by a frame and then by the reverse frame returns the ray, and a forward-moving
    /// frame never decreases the angle.
    #[test]
    fn aberration_reverses(
        angle in 0.0_f64..PI,
        beta in 0.0_f64..0.99,
    ) {
        let there = aberration_angle(angle, beta * C).expect("valid");
        prop_assert!(there >= angle - 1.0e-15);

        let back = aberration_angle(there, -beta * C).expect("valid");
        prop_assert!((back - angle).abs() < 1.0e-10, "{} came back as {}", angle, back);
    }
}
