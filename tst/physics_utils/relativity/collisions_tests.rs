//! Unit and property tests for [`crate::physics_utils::relativity::collisions`].
//!
//! Compiled into `src/physics_utils/relativity/collisions.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **Both solvers conserve `Σγmv` and `Σγmc²`**, with the totals computed in the test from
//!    `momentum.rs` and `energy.rs`, not from this module's own helpers.
//! 2. **The answer does not depend on the frame that solves it.** Boost, collide, compare with
//!    collide, boost. The Newtonian solver is run through the same test and fails it, which is the
//!    reason this module exists.
//! 3. **At everyday speeds both solvers agree with the Newtonian ones** in the parent layer.
//! 4. **A perfectly inelastic collision makes mass.**

use super::*;
use crate::physics_utils::conservation_laws::{
    elastic_collision_1d as newtonian_elastic,
    perfectly_inelastic_collision_1d as newtonian_inelastic,
};
use crate::physics_utils::relativity::energy::{relativistic_kinetic_energy, total_energy};
use crate::physics_utils::relativity::momentum::relativistic_momentum;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_collisions_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

/// `Σγmv` from `momentum.rs`.
fn momentum_of(bodies: &[(f64, f64)]) -> f64 {
    bodies
        .iter()
        .map(|&(m, v)| relativistic_momentum(m, v).expect("valid body"))
        .sum()
}

/// `Σγmc²` from `energy.rs`.
fn energy_of(bodies: &[(f64, f64)]) -> f64 {
    bodies
        .iter()
        .map(|&(m, v)| total_energy(m, v).expect("valid body"))
        .sum()
}

// -----------------------------------------------------------------------
// The totals and the centre-of-momentum frame.
// -----------------------------------------------------------------------

#[test]
fn the_totals_agree_with_momentum_and_energy() {
    let bodies = [(1.0, 0.6 * C), (3.0, -0.2 * C), (0.5, 0.95 * C)];
    let momentum = total_relativistic_momentum(&bodies).expect("valid");
    let energy = total_relativistic_energy(&bodies).expect("valid");

    assert!((momentum / momentum_of(&bodies) - 1.0).abs() < 1.0e-14);
    assert!((energy / energy_of(&bodies) - 1.0).abs() < 1.0e-14);
}

#[test]
fn the_centre_of_momentum_frame_has_no_momentum() {
    let bodies = [(1.0, 0.6 * C), (3.0, -0.2 * C), (0.5, 0.95 * C)];
    let frame = centre_of_momentum_velocity(&bodies).expect("valid");

    let inside: Vec<(f64, f64)> = bodies
        .iter()
        .map(|&(m, v)| (m, transform_velocity(v, frame).expect("valid")))
        .collect();

    // Relative to the momenta that cancel, which are of order m*c.
    assert!(momentum_of(&inside).abs() < 1.0e-13 * C);
}

#[test]
fn a_fast_light_body_outweighs_its_mass() {
    // 1 kg at 0.8c (gamma 5/3) against 1 kg at rest. Newton's centre of mass moves at 0.4c; the
    // relativistic one at (5/3 * 0.8)/(5/3 + 1) = 0.5c.
    let frame = centre_of_momentum_velocity(&[(1.0, 0.8 * C), (1.0, 0.0)]).expect("valid");
    assert!((frame / C - 0.5).abs() < 1.0e-14);
}

#[test]
fn a_system_moving_together_weighs_the_sum_of_its_parts() {
    let mass =
        system_invariant_mass(&[(1.0, 0.7 * C), (2.0, 0.7 * C), (4.0, 0.7 * C)]).expect("valid");
    assert!((mass - 7.0).abs() < 1.0e-13);

    let alone = system_invariant_mass(&[(3.0, -0.9 * C)]).expect("valid");
    assert!((alone - 3.0).abs() < 1.0e-13);
}

#[test]
fn colliding_beams_beat_a_fixed_target() {
    // Two protons, each with gamma = 7,247 (6.8 TeV). Head on, the available mass is 2*gamma*m:
    // 13.6 TeV. One such proton on a proton at rest gives sqrt(2*(gamma + 1))*m: 113 GeV.
    let gamma: f64 = 7_247.0;
    let beta = (1.0 - 1.0 / (gamma * gamma)).sqrt();
    let v = beta * C;

    let head_on = system_invariant_mass(&[(1.0, v), (1.0, -v)]).expect("valid");
    let fixed_target = system_invariant_mass(&[(1.0, v), (1.0, 0.0)]).expect("valid");

    assert!((head_on / (2.0 * gamma) - 1.0).abs() < 1.0e-6);
    assert!((fixed_target / (2.0 * (gamma + 1.0)).sqrt() - 1.0).abs() < 1.0e-6);
    assert!(head_on / fixed_target > 100.0);
}

// -----------------------------------------------------------------------
// Elastic.
// -----------------------------------------------------------------------

#[test]
fn equal_masses_exchange_velocities() {
    let (a, b) = elastic_collision_1d(2.0, 0.9 * C, 2.0, -0.3 * C).expect("valid");
    assert!((a / C + 0.3).abs() < 1.0e-13, "a = {}c", a / C);
    assert!((b / C - 0.9).abs() < 1.0e-13, "b = {}c", b / C);
}

#[test]
fn a_head_on_collision_in_the_centre_of_momentum_frame_just_reverses() {
    // 3 kg at +0.5c has gamma*m*v = 3 * 0.5/sqrt(0.75). Give 1 kg the opposite momentum.
    let momentum = relativistic_momentum(3.0, 0.5 * C).expect("valid");
    let other = crate::physics_utils::relativity::momentum::velocity_from_momentum(1.0, -momentum)
        .expect("valid");

    let (a, b) = elastic_collision_1d(3.0, 0.5 * C, 1.0, other).expect("valid");
    assert!((a / C + 0.5).abs() < 1.0e-13);
    assert!((b / other + 1.0).abs() < 1.0e-13);
}

#[test]
fn a_heavy_wall_reflects_and_barely_moves() {
    let (ball, wall) = elastic_collision_1d(1.0, 0.5 * C, 1.0e15, 0.0).expect("valid");
    assert!((ball / C + 0.5).abs() < 1.0e-12);
    assert!(wall > 0.0 && wall < 1.0e-12 * C);
}

#[test]
fn a_moving_wall_does_not_add_twice_its_speed() {
    // Newton: a ball at rest hit by a wall at v leaves at 2v. At v = 0.6c that is 1.2c. The
    // relativistic answer is the wall's velocity composed with itself: 0.882c.
    let (wall, ball) = elastic_collision_1d(1.0e15, 0.6 * C, 1.0, 0.0).expect("valid");
    let expected = compose_velocities(0.6 * C, 0.6 * C).expect("valid");

    assert!(
        (ball / expected - 1.0).abs() < 1.0e-12,
        "ball = {}c",
        ball / C
    );
    assert!((ball / C - 15.0 / 17.0).abs() < 1.0e-12);
    assert!((wall / C - 0.6).abs() < 1.0e-12);

    let (_, newtonian_ball) = newtonian_elastic(1.0e15, 0.6 * C, 1.0, 0.0);
    assert!(newtonian_ball > C, "the solver this replaces says 1.2c");
}

#[test]
fn bodies_moving_together_are_left_alone() {
    let (a, b) = elastic_collision_1d(1.0, 0.4 * C, 5.0, 0.4 * C).expect("valid");
    assert!((a / C - 0.4).abs() < 1.0e-14);
    assert!((b / C - 0.4).abs() < 1.0e-14);
}

// -----------------------------------------------------------------------
// Perfectly inelastic.
// -----------------------------------------------------------------------

#[test]
fn two_lumps_at_point_six_c_make_two_and_a_half() {
    let (mass, velocity) =
        perfectly_inelastic_collision_1d(1.0, 0.6 * C, 1.0, -0.6 * C).expect("valid");
    assert!((mass - 2.5).abs() < 1.0e-14);
    assert_eq!(velocity, 0.0);
}

#[test]
fn the_extra_mass_is_the_kinetic_energy_that_disappeared() {
    // In the centre-of-momentum frame the merged body is at rest, so all the kinetic energy there
    // has become mass: (M - m1 - m2) c^2 = KE1 + KE2.
    let (mass, _) = perfectly_inelastic_collision_1d(2.0, 0.8 * C, 3.0, -0.4 * C).expect("valid");
    let frame = centre_of_momentum_velocity(&[(2.0, 0.8 * C), (3.0, -0.4 * C)]).expect("valid");

    let kinetic = relativistic_kinetic_energy(2.0, transform_velocity(0.8 * C, frame).unwrap())
        .expect("valid")
        + relativistic_kinetic_energy(3.0, transform_velocity(-0.4 * C, frame).unwrap())
            .expect("valid");

    assert!(((mass - 5.0) * C * C / kinetic - 1.0).abs() < 1.0e-13);
}

#[test]
fn a_bullet_into_a_block_at_rest() {
    // 1 kg at 0.8c into 1 kg at rest. E = (5/3 + 1), p = 4/3: v = 0.5c, M = sqrt(64/9 - 16/9) =
    // 2.309 kg. Newton says 0.4c and 2 kg.
    let (mass, velocity) = perfectly_inelastic_collision_1d(1.0, 0.8 * C, 1.0, 0.0).expect("valid");
    assert!((velocity / C - 0.5).abs() < 1.0e-14);
    assert!((mass - (48.0_f64 / 9.0).sqrt()).abs() < 1.0e-14);
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn bad_bodies_have_no_collision() {
    for bad_mass in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(elastic_collision_1d(bad_mass, 0.0, 1.0, 0.0), None);
        assert_eq!(elastic_collision_1d(1.0, 0.0, bad_mass, 0.0), None);
        assert_eq!(
            perfectly_inelastic_collision_1d(bad_mass, 0.0, 1.0, 0.0),
            None
        );
        assert_eq!(total_relativistic_momentum(&[(bad_mass, 0.0)]), None);
    }
    for bad_velocity in [C, -C, 2.0 * C, f64::NAN] {
        assert_eq!(elastic_collision_1d(1.0, bad_velocity, 1.0, 0.0), None);
        assert_eq!(
            perfectly_inelastic_collision_1d(1.0, 0.0, 1.0, bad_velocity),
            None
        );
        assert_eq!(total_relativistic_energy(&[(1.0, bad_velocity)]), None);
        assert_eq!(centre_of_momentum_velocity(&[(1.0, bad_velocity)]), None);
        assert_eq!(system_invariant_mass(&[(1.0, bad_velocity)]), None);
    }
}

#[test]
fn an_empty_system_has_no_totals() {
    assert_eq!(total_relativistic_momentum(&[]), None);
    assert_eq!(total_relativistic_energy(&[]), None);
    assert_eq!(centre_of_momentum_velocity(&[]), None);
    assert_eq!(system_invariant_mass(&[]), None);
}

// -----------------------------------------------------------------------
// Frame independence: the reason `p = mv` had to go.
// -----------------------------------------------------------------------

#[test]
fn the_newtonian_solver_gives_a_different_answer_in_a_different_frame() {
    // Collide in S, then view the result from S'. Or view the bodies from S' first, then collide.
    // Physics requires the same answer. The Newtonian solver, fed relativistically transformed
    // velocities, does not give it; the relativistic one does.
    let (mass_a, velocity_a, mass_b, velocity_b) = (1.0, 0.6 * C, 3.0, -0.2 * C);
    let frame = 0.5 * C;
    let into = |v: f64| transform_velocity(v, frame).expect("valid");

    let (newton_a, _) = newtonian_elastic(mass_a, velocity_a, mass_b, velocity_b);
    let (newton_a_boosted_first, _) =
        newtonian_elastic(mass_a, into(velocity_a), mass_b, into(velocity_b));
    assert!(
        (into(newton_a) - newton_a_boosted_first).abs() > 0.01 * C,
        "the Newtonian solver should disagree with itself across frames"
    );

    let (a, b) = elastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b).expect("valid");
    let (a_boosted_first, b_boosted_first) =
        elastic_collision_1d(mass_a, into(velocity_a), mass_b, into(velocity_b)).expect("valid");
    assert!((into(a) - a_boosted_first).abs() < 1.0e-12 * C);
    assert!((into(b) - b_boosted_first).abs() < 1.0e-12 * C);
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// **An elastic collision conserves relativistic momentum and total energy.** The momentum
    /// tolerance is relative to `Σ|γmv|`, since the two momenta may cancel.
    #[test]
    fn an_elastic_collision_conserves_momentum_and_energy(
        mass_a in 0.01_f64..100.0,
        mass_b in 0.01_f64..100.0,
        beta_a in -0.99_f64..0.99,
        beta_b in -0.99_f64..0.99,
    ) {
        let before = [(mass_a, beta_a * C), (mass_b, beta_b * C)];
        let (after_a, after_b) =
            elastic_collision_1d(mass_a, beta_a * C, mass_b, beta_b * C).expect("valid");
        let after = [(mass_a, after_a), (mass_b, after_b)];

        // A velocity near c amplifies its own rounding into the momentum by gamma^2 (up to ~50 on
        // the way in, and the outgoing bodies can be faster than either incoming one).
        let scale = energy_of(&before);
        prop_assert!(
            (energy_of(&after) - energy_of(&before)).abs() <= 1.0e-10 * scale,
            "energy {} -> {}", energy_of(&before), energy_of(&after)
        );
        prop_assert!(
            (momentum_of(&after) - momentum_of(&before)).abs() <= 1.0e-10 * scale / C,
            "momentum {} -> {}", momentum_of(&before), momentum_of(&after)
        );
    }

    /// The bodies do not pass through each other: the relative velocity changes sign.
    #[test]
    fn an_elastic_collision_reverses_the_approach(
        mass_a in 0.01_f64..100.0,
        mass_b in 0.01_f64..100.0,
        beta_a in -0.99_f64..0.99,
        beta_b in -0.99_f64..0.99,
    ) {
        prop_assume!((beta_a - beta_b).abs() > 1.0e-6);
        let (after_a, after_b) =
            elastic_collision_1d(mass_a, beta_a * C, mass_b, beta_b * C).expect("valid");

        prop_assert!(after_a.abs() < C && after_b.abs() < C);
        prop_assert!(
            (after_a - after_b).signum() == -(beta_a - beta_b).signum(),
            "approach {} became {}", (beta_a - beta_b) * C, after_a - after_b
        );
    }

    /// **A perfectly inelastic collision conserves momentum and total energy too**, and the merged
    /// body is never lighter than the two that made it.
    #[test]
    fn an_inelastic_collision_conserves_both_and_gains_mass(
        mass_a in 0.01_f64..100.0,
        mass_b in 0.01_f64..100.0,
        beta_a in -0.99_f64..0.99,
        beta_b in -0.99_f64..0.99,
    ) {
        let before = [(mass_a, beta_a * C), (mass_b, beta_b * C)];
        let (mass, velocity) =
            perfectly_inelastic_collision_1d(mass_a, beta_a * C, mass_b, beta_b * C).expect("valid");
        let after = [(mass, velocity)];

        let scale = energy_of(&before);
        prop_assert!((energy_of(&after) - energy_of(&before)).abs() <= 1.0e-11 * scale);
        prop_assert!((momentum_of(&after) - momentum_of(&before)).abs() <= 1.0e-11 * scale / C);
        prop_assert!(mass >= (mass_a + mass_b) * (1.0 - 1.0e-12));
    }

    /// **The elastic solver gives the same collision in every frame.**
    #[test]
    fn the_elastic_result_is_frame_independent(
        mass_a in 0.01_f64..100.0,
        mass_b in 0.01_f64..100.0,
        beta_a in -0.9_f64..0.9,
        beta_b in -0.9_f64..0.9,
        frame in -0.9_f64..0.9,
    ) {
        let into = |v: f64| transform_velocity(v, frame * C).expect("valid");

        let (a, b) = elastic_collision_1d(mass_a, beta_a * C, mass_b, beta_b * C).expect("valid");
        let (a_first, b_first) =
            elastic_collision_1d(mass_a, into(beta_a * C), mass_b, into(beta_b * C)).expect("valid");

        // Compared in rapidity, where a boost is a shift and the tolerance means the same thing at
        // every speed.
        let phi = |v: f64| (v / C).atanh();
        prop_assert!((phi(into(a)) - phi(a_first)).abs() < 1.0e-9);
        prop_assert!((phi(into(b)) - phi(b_first)).abs() < 1.0e-9);
    }

    /// **At everyday speeds both solvers are the Newtonian ones.** Up to `1 km/s` the relativistic
    /// corrections are below `β² = 1.1e-11`.
    #[test]
    fn slow_collisions_are_newtonian(
        mass_a in 0.01_f64..100.0,
        mass_b in 0.01_f64..100.0,
        velocity_a in -1_000.0_f64..1_000.0,
        velocity_b in -1_000.0_f64..1_000.0,
    ) {
        let scale = velocity_a.abs().max(velocity_b.abs()).max(1.0);

        let (a, b) = elastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b).expect("valid");
        let (newton_a, newton_b) = newtonian_elastic(mass_a, velocity_a, mass_b, velocity_b);
        prop_assert!((a - newton_a).abs() < 1.0e-9 * scale, "{} against {}", a, newton_a);
        prop_assert!((b - newton_b).abs() < 1.0e-9 * scale, "{} against {}", b, newton_b);

        let (mass, velocity) =
            perfectly_inelastic_collision_1d(mass_a, velocity_a, mass_b, velocity_b).expect("valid");
        let newton = newtonian_inelastic(mass_a, velocity_a, mass_b, velocity_b);
        prop_assert!((velocity - newton).abs() < 1.0e-9 * scale);
        prop_assert!((mass / (mass_a + mass_b) - 1.0).abs() < 1.0e-10);
    }
}
