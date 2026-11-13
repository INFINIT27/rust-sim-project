//! Unit and property tests for [`crate::physics_utils::relativity::four_vector`].
//!
//! Compiled into `src/physics_utils/relativity/four_vector.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **A boost leaves the Minkowski product alone**, for any two vectors and any direction.
//! 2. **Along `x` the boost is `transform_event`**, the one-axis transformation written separately
//!    in `spacetime.rs`.
//! 3. **A boosted four-momentum is the four-momentum of the transformed velocity.** This ties the
//!    boost to `velocity_3d.rs`, which uses a different formula, and it is the statement that energy
//!    and momentum transform together.
//! 4. **Total four-momentum gives the system's mass**, agreeing with `collisions.rs`.

use super::*;
use crate::physics_utils::relativity::collisions::system_invariant_mass;
use crate::physics_utils::relativity::energy::total_energy;
use crate::physics_utils::relativity::momentum::relativistic_momentum;
use crate::physics_utils::relativity::spacetime::{interval_squared_m2, transform_event};
use crate::physics_utils::relativity::velocity_3d::transform_velocity_3d;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use std::f64::consts::PI;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_four_vector_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

fn velocity(beta: f64, polar: f64, azimuth: f64) -> [f64; 3] {
    [
        beta * C * polar.sin() * azimuth.cos(),
        beta * C * polar.sin() * azimuth.sin(),
        beta * C * polar.cos(),
    ]
}

/// The sum of the absolute values of the four components: the scale rounding is relative to.
fn size(vector: &FourVector) -> f64 {
    vector.time.abs() + vector.space.iter().map(|c| c.abs()).sum::<f64>()
}

// -----------------------------------------------------------------------
// The product.
// -----------------------------------------------------------------------

#[test]
fn the_product_has_one_plus_and_three_minuses() {
    let unit_time = FourVector::new(1.0, [0.0; 3]);
    assert_eq!(unit_time.invariant(), 1.0);
    for axis in 0..3 {
        let mut space = [0.0; 3];
        space[axis] = 1.0;
        assert_eq!(FourVector::new(0.0, space).invariant(), -1.0, "axis {axis}");
        assert_eq!(
            FourVector::new(0.0, space).minkowski_product(&unit_time),
            0.0
        );
    }
}

#[test]
fn the_product_is_symmetric() {
    let a = FourVector::new(2.0, [1.0, -3.0, 0.5]);
    let b = FourVector::new(-1.0, [4.0, 2.0, 7.0]);
    assert_eq!(a.minkowski_product(&b), b.minkowski_product(&a));
    assert_eq!(a.minkowski_product(&b), -2.0 - 4.0 + 6.0 - 3.5);
}

#[test]
fn an_events_invariant_is_the_interval() {
    let event = event_four_vector(2.0, [1.0e8, 0.0, 0.0]).expect("finite");
    let interval = interval_squared_m2(2.0, 1.0e8).expect("finite");
    assert!((event.invariant() / interval - 1.0).abs() < 1.0e-15);
}

#[test]
fn plus_adds_component_by_component() {
    let a = FourVector::new(1.0, [2.0, 3.0, 4.0]);
    let b = FourVector::new(10.0, [20.0, 30.0, 40.0]);
    assert_eq!(a.plus(&b), FourVector::new(11.0, [22.0, 33.0, 44.0]));
}

// -----------------------------------------------------------------------
// The boost.
// -----------------------------------------------------------------------

#[test]
fn a_frame_at_rest_changes_nothing() {
    let vector = FourVector::new(3.0, [1.0, -2.0, 5.0]);
    assert_eq!(vector.boost([0.0; 3]), Some(vector));
}

#[test]
fn along_x_the_boost_is_the_one_axis_transformation() {
    for beta in [-0.9, -0.3, 0.1, 0.6, 0.99] {
        let (time_s, position_m) = (3.0, 2.0e8);
        let event = event_four_vector(time_s, [position_m, 7.0, -11.0]).expect("finite");
        let boosted = event.boost([beta * C, 0.0, 0.0]).expect("subluminal");
        let (expected_time, expected_position) =
            transform_event(time_s, position_m, beta * C).expect("subluminal");

        let scale = size(&event) / (1.0 - beta * beta).sqrt();
        assert!(
            (boosted.time - C * expected_time).abs() < 1.0e-14 * scale,
            "beta {beta}"
        );
        assert!(
            (boosted.space[0] - expected_position).abs() < 1.0e-14 * scale,
            "beta {beta}"
        );
        // The components across the boost are untouched, exactly.
        assert_eq!(boosted.space[1], 7.0);
        assert_eq!(boosted.space[2], -11.0);
    }
}

#[test]
fn a_slow_boost_is_galilean() {
    // 30 m/s for one second: x' = x - vt to first order, and ct is unchanged to 1e-14 relative.
    // The (gamma - 1)/beta^2 form would compute 0/0-ish garbage here; gamma^2/(gamma + 1) does not.
    let event = event_four_vector(1.0, [100.0, 0.0, 0.0]).expect("finite");
    let boosted = event.boost([30.0, 0.0, 0.0]).expect("subluminal");
    assert!((boosted.space[0] - 70.0).abs() < 1.0e-6);
    assert!((boosted.time / C - 1.0).abs() < 1.0e-12);
}

#[test]
fn a_frame_at_light_speed_and_bad_components_have_no_boost() {
    let vector = FourVector::new(1.0, [0.0; 3]);
    assert_eq!(vector.boost([C, 0.0, 0.0]), None);
    assert_eq!(vector.boost([0.7 * C, 0.7 * C, 0.7 * C]), None);
    assert_eq!(vector.boost([f64::NAN, 0.0, 0.0]), None);
    assert_eq!(FourVector::new(f64::NAN, [0.0; 3]).boost([0.0; 3]), None);
    assert_eq!(
        FourVector::new(0.0, [0.0, f64::INFINITY, 0.0]).boost([0.0; 3]),
        None
    );
    assert_eq!(event_four_vector(f64::NAN, [0.0; 3]), None);
    assert_eq!(event_four_vector(0.0, [0.0, 0.0, f64::INFINITY]), None);
}

// -----------------------------------------------------------------------
// Four-momentum.
// -----------------------------------------------------------------------

#[test]
fn the_four_momentum_holds_energy_and_momentum() {
    let (mass, v) = (2.5, 0.8 * C);
    let p = four_momentum(mass, [0.0, v, 0.0]).expect("valid");

    let energy = total_energy(mass, v).expect("valid");
    let momentum = relativistic_momentum(mass, v).expect("valid");
    assert!((p.time * C / energy - 1.0).abs() < 1.0e-15);
    assert!((p.space[1] / momentum - 1.0).abs() < 1.0e-15);
    assert_eq!(p.space[0], 0.0);
    assert_eq!(p.space[2], 0.0);
}

#[test]
fn a_body_at_rest_has_only_a_time_component() {
    let p = four_momentum(3.0, [0.0; 3]).expect("valid");
    assert_eq!(p, FourVector::new(3.0 * C, [0.0; 3]));
}

#[test]
fn boosting_into_the_rest_frame_leaves_only_the_rest_energy() {
    let v = velocity(0.9, 1.1, 2.3);
    let p = four_momentum(4.0, v).expect("valid");
    let at_rest = p.boost(v).expect("subluminal");

    assert!((at_rest.time / (4.0 * C) - 1.0).abs() < 1.0e-13);
    for (axis, component) in at_rest.space.iter().enumerate() {
        assert!(component.abs() < 1.0e-13 * size(&p), "axis {axis}");
    }
}

#[test]
fn a_photon_is_built_directly_and_stays_lightlike() {
    // E/c = 1, moving along x. From a frame chasing it at 0.6c it is redshifted by the Doppler
    // factor 1/2, and still lightlike.
    let photon = FourVector::new(1.0, [1.0, 0.0, 0.0]);
    let chased = photon.boost([0.6 * C, 0.0, 0.0]).expect("subluminal");
    assert!((chased.time - 0.5).abs() < 1.0e-15);
    assert!((chased.space[0] - 0.5).abs() < 1.0e-15);
    assert!(chased.invariant().abs() < 1.0e-15);

    // From a frame moving across it, the transverse Doppler factor in reverse: gamma.
    let crossed = photon.boost([0.0, 0.6 * C, 0.0]).expect("subluminal");
    assert!((crossed.time - 1.25).abs() < 1.0e-15);
}

#[test]
fn massless_and_bad_bodies_have_no_four_momentum() {
    assert_eq!(four_momentum(0.0, [0.0; 3]), None);
    assert_eq!(four_momentum(-1.0, [0.0; 3]), None);
    assert_eq!(four_momentum(f64::NAN, [0.0; 3]), None);
    assert_eq!(four_momentum(1.0, [C, 0.0, 0.0]), None);
    assert_eq!(four_momentum(1.0, [0.0, f64::NAN, 0.0]), None);
}

#[test]
fn two_photons_have_mass_together() {
    // Back to back, E/c = 1 each: total (2, 0, 0, 0), invariant 4. At right angles: (2, 1, 1, 0),
    // invariant 2. Parallel: (2, 2, 0, 0), invariant 0, and no mass.
    let along_x = FourVector::new(1.0, [1.0, 0.0, 0.0]);
    let against_x = FourVector::new(1.0, [-1.0, 0.0, 0.0]);
    let along_y = FourVector::new(1.0, [0.0, 1.0, 0.0]);

    assert_eq!(along_x.plus(&against_x).invariant(), 4.0);
    assert_eq!(along_x.plus(&along_y).invariant(), 2.0);
    assert_eq!(along_x.plus(&along_x).invariant(), 0.0);
}

#[test]
fn the_total_four_momentum_gives_the_systems_mass() {
    let bodies = [(1.0, 0.6 * C), (3.0, -0.2 * C), (0.5, 0.95 * C)];
    let total = bodies
        .iter()
        .map(|&(m, v)| four_momentum(m, [v, 0.0, 0.0]).expect("valid"))
        .fold(FourVector::new(0.0, [0.0; 3]), |sum, p| sum.plus(&p));

    let from_four_vector = total.invariant().sqrt() / C;
    let from_collisions = system_invariant_mass(&bodies).expect("valid");
    assert!((from_four_vector / from_collisions - 1.0).abs() < 1.0e-13);
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// **A boost in any direction preserves the Minkowski product of any two vectors.**
    #[test]
    fn a_boost_preserves_the_product(
        a in (-10.0_f64..10.0, -10.0_f64..10.0, -10.0_f64..10.0, -10.0_f64..10.0),
        b in (-10.0_f64..10.0, -10.0_f64..10.0, -10.0_f64..10.0, -10.0_f64..10.0),
        beta in 0.0_f64..0.99,
        polar in 0.0_f64..PI,
        azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let first = FourVector::new(a.0, [a.1, a.2, a.3]);
        let second = FourVector::new(b.0, [b.1, b.2, b.3]);
        let frame = velocity(beta, polar, azimuth);

        let before = first.minkowski_product(&second);
        let after = first
            .boost(frame)
            .expect("subluminal")
            .minkowski_product(&second.boost(frame).expect("subluminal"));

        // The boosted components are up to 2*gamma times larger and their products cancel.
        let scale = size(&first) * size(&second) / (1.0 - beta * beta);
        prop_assert!(
            (after - before).abs() <= 1.0e-13 * scale + 1.0e-300,
            "product {} became {}", before, after
        );
    }

    /// Boosting by a velocity and then by its reverse is a round trip.
    #[test]
    fn the_reverse_boost_undoes_the_boost(
        a in (-10.0_f64..10.0, -10.0_f64..10.0, -10.0_f64..10.0, -10.0_f64..10.0),
        beta in 0.0_f64..0.99,
        polar in 0.0_f64..PI,
        azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let vector = FourVector::new(a.0, [a.1, a.2, a.3]);
        let frame = velocity(beta, polar, azimuth);
        let back = vector
            .boost(frame)
            .expect("subluminal")
            .boost(frame.map(|component| -component))
            .expect("subluminal");

        let scale = size(&vector) / (1.0 - beta * beta);
        prop_assert!((back.time - vector.time).abs() <= 1.0e-13 * scale + 1.0e-300);
        for axis in 0..3 {
            prop_assert!((back.space[axis] - vector.space[axis]).abs() <= 1.0e-13 * scale + 1.0e-300);
        }
    }

    /// **The boost of a body's four-momentum is the four-momentum of its transformed velocity.**
    /// Energy and momentum transform together, and the 4-D boost agrees with the 3-D velocity
    /// formula in `velocity_3d.rs`.
    #[test]
    fn boosting_the_momentum_matches_transforming_the_velocity(
        mass in 0.01_f64..100.0,
        object_beta in 0.0_f64..0.95,
        object_polar in 0.0_f64..PI,
        object_azimuth in 0.0_f64..(2.0 * PI),
        frame_beta in 0.0_f64..0.95,
        frame_polar in 0.0_f64..PI,
        frame_azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let object = velocity(object_beta, object_polar, object_azimuth);
        let frame = velocity(frame_beta, frame_polar, frame_azimuth);

        let boosted = four_momentum(mass, object).expect("valid").boost(frame).expect("subluminal");
        let seen = transform_velocity_3d(object, frame).expect("valid");
        let direct = four_momentum(mass, seen).expect("valid");

        // gamma of the transformed velocity amplifies that velocity's rounding by gamma^2; both
        // gammas are at most 3.2 here, so the transformed one is at most ~20.
        let scale = size(&direct);
        prop_assert!((boosted.time - direct.time).abs() <= 1.0e-10 * scale);
        for axis in 0..3 {
            prop_assert!(
                (boosted.space[axis] - direct.space[axis]).abs() <= 1.0e-10 * scale,
                "axis {}: {} against {}", axis, boosted.space[axis], direct.space[axis]
            );
        }
    }

    /// The invariant of a four-momentum is `(mc)²` at every velocity.
    #[test]
    fn the_four_momentum_invariant_is_the_mass(
        mass in 0.01_f64..100.0,
        beta in 0.0_f64..0.99,
        polar in 0.0_f64..PI,
        azimuth in 0.0_f64..(2.0 * PI),
    ) {
        let p = four_momentum(mass, velocity(beta, polar, azimuth)).expect("valid");
        let expected = (mass * C).powi(2);
        // time^2 and |space|^2 are each gamma^2 times the answer.
        prop_assert!((p.invariant() / expected - 1.0).abs() < 1.0e-13 / (1.0 - beta * beta));
    }
}
