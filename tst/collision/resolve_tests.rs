//! Unit and property tests for the [`crate::collision::resolve`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/collision/resolve.rs` via `#[path]`, not as a standalone integration
//! test.
//!
//! The response is the half of body physics with laws attached, so the
//! conservation assertions go through
//! [`crate::physics_utils::conservation_laws`] rather than a hand-rolled
//! comparison — the checks and the simulation should agree with each other, the
//! same discipline `tst/core/integrator_tests.rs` follows. The last test in the
//! file is the end-to-end one: run the shipped scene until the spheres meet and
//! assert that they *do not pass through each other*, which is the whole point of
//! the layer.

use super::*;
use crate::collision::contact::contact_between_props;
use crate::constants::{TWO_BODY_TIME_SCALE, TWO_BODY_VISUAL_RADIUS_UNITS};
use crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS;
use crate::core::scene::{Body, Color, Pose, Quat, Scene};
use crate::physics_utils::conservation_laws::{kinetic_energy_is_conserved, momentum_is_conserved};
use crate::time::constants::MAX_REAL_STEP_SECONDS;
use crate::time::simulation::{SimulationClock, TimeStep};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/collision/proptest-regressions/resolve_tests.txt";

fn white() -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    }
}

fn ball(centre: Vec3, radius: f32) -> Body {
    Body::Sphere {
        pose: Pose {
            position: centre,
            rotation: Quat::IDENTITY,
        },
        radius,
        color: white(),
    }
}

fn floor() -> Body {
    Body::Plane {
        pose: Pose {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        },
        half_size: 20.0,
        color: white(),
    }
}

/// The `(mass_kg, velocity)` pairs along one axis that `conservation_laws` wants,
/// for the movable props only — an immovable prop is an external agent, not part
/// of the closed system.
/// A `TimeStep` from a plain number of seconds, for tests that are about durations
/// rather than about validation. Panics on an invalid duration, which is what a test
/// wants: an unusable literal here is a mistake in the test, not a case to handle.
fn seconds(value: f64) -> TimeStep {
    TimeStep::from_seconds(value).expect("test durations must be valid")
}

fn axis_bodies(props: &[Prop], axis: usize) -> Vec<(f64, f64)> {
    props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(|prop| (prop.mass_kg as f64, prop.velocity_units_s[axis] as f64))
        .collect()
}

#[test]
fn equal_masses_meeting_head_on_exchange_their_velocities() {
    // Overlapping along X, closing at 1 unit/s each.
    let mut props = vec![
        Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(1.0, 0.0, 0.0), 5.0),
        Prop::new(
            ball(Vec3::new(1.6, 0.0, 0.0), 1.0),
            Vec3::new(-1.0, 0.0, 0.0),
            5.0,
        ),
    ];

    assert_eq!(resolve_contacts(&mut props), 1);

    assert!(
        (props[0].velocity_units_s.x + 1.0).abs() < 1.0e-4,
        "the left ball must come back, got {}",
        props[0].velocity_units_s.x
    );
    assert!(
        (props[1].velocity_units_s.x - 1.0).abs() < 1.0e-4,
        "the right ball must come back, got {}",
        props[1].velocity_units_s.x
    );
}

#[test]
fn a_bounce_conserves_momentum_and_kinetic_energy() {
    let mut props = vec![
        Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(3.0, 0.0, 0.0), 2.0),
        Prop::new(
            ball(Vec3::new(1.5, 0.0, 0.0), 1.0),
            Vec3::new(-1.0, 0.0, 0.0),
            7.0,
        ),
    ];

    let before = axis_bodies(&props, 0);
    resolve_contacts(&mut props);
    let after = axis_bodies(&props, 0);

    assert!(
        momentum_is_conserved(&before, &after, 1.0e-6),
        "before {before:?}, after {after:?}"
    );
    assert!(
        kinetic_energy_is_conserved(&before, &after, 1.0e-6),
        "an elastic collision must conserve kinetic energy: before {before:?}, after {after:?}"
    );
}

#[test]
fn a_heavy_prop_is_barely_deflected_by_a_light_one() {
    let mut props = vec![
        Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
        Prop::new(ball(Vec3::new(1.5, 0.0, 0.0), 1.0), Vec3::ZERO, 1.0e6),
    ];

    resolve_contacts(&mut props);

    assert!(
        props[0].velocity_units_s.x < -0.99,
        "the light prop should bounce back at nearly its approach speed, got {}",
        props[0].velocity_units_s.x
    );
    assert!(
        props[1].velocity_units_s.x.abs() < 1.0e-3,
        "the heavy prop should hardly move, got {}",
        props[1].velocity_units_s.x
    );
}

#[test]
fn only_the_normal_component_is_touched_so_there_is_no_friction() {
    // Approaching along X, drifting along Z. The Z drift must survive untouched.
    let mut props = vec![
        Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(1.0, 0.0, 4.0), 3.0),
        Prop::new(
            ball(Vec3::new(1.5, 0.0, 0.0), 1.0),
            Vec3::new(-1.0, 0.0, 4.0),
            3.0,
        ),
    ];

    resolve_contacts(&mut props);

    assert!((props[0].velocity_units_s.z - 4.0).abs() < 1.0e-4);
    assert!((props[1].velocity_units_s.z - 4.0).abs() < 1.0e-4);
}

#[test]
fn a_wall_reflects_what_hits_it_and_never_moves() {
    // A ball sunk 0.1 into the floor, falling.
    let mut props = vec![
        Prop::new(
            ball(Vec3::new(0.0, 0.9, 0.0), 1.0),
            Vec3::new(0.0, -2.0, 0.0),
            4.0,
        ),
        Prop::immovable(floor()),
    ];

    assert_eq!(resolve_contacts(&mut props), 1);

    assert!(
        (props[0].velocity_units_s.y - 2.0).abs() < 1.0e-4,
        "the ball's fall must be reversed exactly, got {}",
        props[0].velocity_units_s.y
    );
    assert_eq!(
        props[1].velocity_units_s,
        Vec3::ZERO,
        "an immovable prop keeps its zero-velocity invariant"
    );
    assert_eq!(
        props[1].body.center(),
        Vec3::ZERO,
        "and it does not move out of the way either"
    );
    assert!(props[1].is_valid());
}

#[test]
fn a_ball_in_the_floor_is_lifted_clear_of_it() {
    let mut props = vec![
        Prop::new(
            ball(Vec3::new(0.0, 0.9, 0.0), 1.0),
            Vec3::new(0.0, -2.0, 0.0),
            4.0,
        ),
        Prop::immovable(floor()),
    ];

    resolve_contacts(&mut props);

    // The movable prop takes the whole correction: the wall's inverse mass is zero.
    assert!(
        props[0].body.center().y > 0.9,
        "the ball must be pushed up out of the floor, ended at {}",
        props[0].body.center().y
    );
    // And the overlap is gone, to within the touching tolerance.
    let remaining = crate::collision::contact::contact_between_props(&props[0], &props[1]);
    assert!(
        remaining.is_none_or(|contact| contact.depth <= 1.0e-3),
        "penetration survived resolution: {remaining:?}"
    );
}

#[test]
fn a_massless_prop_passes_straight_through() {
    let mut props = vec![
        Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(1.0, 0.0, 0.0), 0.0),
        Prop::new(ball(Vec3::new(1.5, 0.0, 0.0), 1.0), Vec3::ZERO, 5.0),
    ];
    let before = props.clone();

    assert_eq!(
        resolve_contacts(&mut props),
        0,
        "a movable massless prop takes no part in contact — see the module docs"
    );
    assert_eq!(props, before);
}

#[test]
fn two_walls_do_not_collide() {
    let mut props = vec![
        Prop::immovable(ball(Vec3::ZERO, 1.0)),
        Prop::immovable(ball(Vec3::new(1.0, 0.0, 0.0), 1.0)),
    ];
    let before = props.clone();

    assert_eq!(resolve_contacts(&mut props), 0);
    assert_eq!(props, before);
}

#[test]
fn a_pair_already_moving_apart_is_separated_but_not_reversed() {
    // Overlapping, but receding: reversing them again would trap them in a
    // contact they could never leave.
    let mut props = vec![
        Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(-1.0, 0.0, 0.0), 3.0),
        Prop::new(
            ball(Vec3::new(1.5, 0.0, 0.0), 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            3.0,
        ),
    ];

    assert_eq!(resolve_contacts(&mut props), 1);

    assert!(
        (props[0].velocity_units_s.x + 1.0).abs() < 1.0e-6,
        "velocity must be untouched, got {}",
        props[0].velocity_units_s.x
    );
    assert!(
        props[1].body.center().x > 1.5,
        "but the overlap is still removed"
    );
}

#[test]
fn props_that_are_nowhere_near_each_other_cost_nothing_and_change_nothing() {
    let mut props = vec![
        Prop::new(ball(Vec3::new(-20.0, 5.0, 0.0), 1.0), Vec3::X, 3.0),
        Prop::new(ball(Vec3::new(20.0, 5.0, 0.0), 1.0), -Vec3::X, 3.0),
        Prop::immovable(floor()),
    ];
    let before = props.clone();

    assert_eq!(resolve_contacts(&mut props), 0);
    assert_eq!(props, before);
}

#[test]
fn every_prop_stays_valid_through_a_resolution() {
    let mut props = vec![
        Prop::new(
            ball(Vec3::new(0.0, 0.9, 0.0), 1.0),
            Vec3::new(0.5, -2.0, 0.0),
            4.0,
        ),
        Prop::new(
            ball(Vec3::new(1.4, 0.9, 0.0), 1.0),
            Vec3::new(-0.5, -1.0, 0.0),
            4.0,
        ),
        Prop::immovable(floor()),
    ];

    resolve_contacts(&mut props);

    for prop in &props {
        assert!(prop.is_valid(), "{prop:?} is invalid after resolution");
    }
}

#[test]
fn substep_count_is_one_when_nothing_is_moving_far() {
    let props = vec![Prop::new(
        ball(Vec3::ZERO, 1.0),
        Vec3::new(0.1, 0.0, 0.0),
        1.0,
    )];

    assert_eq!(substep_count(&props, seconds(1.0)), 1);
    assert_eq!(
        substep_count(&props, seconds(0.0)),
        1,
        "a zero step needs no splitting"
    );
    assert_eq!(
        substep_count(&[], seconds(1.0)),
        1,
        "and neither does an empty scene"
    );
}

#[test]
fn substep_count_scales_with_the_distance_travelled() {
    let props = vec![Prop::new(
        ball(Vec3::ZERO, 1.0),
        Vec3::new(10.0, 0.0, 0.0),
        1.0,
    )];

    // 10 units in a second, budget 0.25 → 40 substeps.
    assert_eq!(substep_count(&props, seconds(1.0)), 40);
    // Half the time, half the substeps.
    assert_eq!(substep_count(&props, seconds(0.5)), 20);
}

#[test]
fn substep_count_is_capped_and_never_zero() {
    let props = vec![Prop::new(
        ball(Vec3::ZERO, 1.0),
        Vec3::new(1.0e9, 0.0, 0.0),
        1.0,
    )];

    assert_eq!(substep_count(&props, seconds(1.0)), MAX_SUBSTEPS);
    assert_eq!(substep_count(&props, TimeStep::ZERO), 1);
    assert!(substep_count(&props, seconds(1.0e-9)) >= 1);
}

#[test]
fn an_immovable_props_speed_does_not_drive_the_substep_count() {
    // An immovable prop does not travel, whatever is in its velocity field —
    // `is_valid` forbids the combination, but the count must not depend on that.
    let mut wall = Prop::immovable(ball(Vec3::ZERO, 1.0));
    wall.velocity_units_s = Vec3::new(1000.0, 0.0, 0.0);

    assert_eq!(substep_count(&[wall], seconds(1.0)), 1);
}

#[test]
fn the_shipped_scene_bounces_instead_of_passing_through() {
    // The end-to-end check on the whole layer: run the shipped two-body infall
    // through `Scene::step` until well past the meeting time and assert the pair
    // never interpenetrates deeply and does come back out.
    //
    // Driven through a real `SimulationClock` at the shipped scale, fed the app's
    // worst-case frame delta (`MAX_REAL_STEP_SECONDS`), so the test exercises the
    // whole time path — clamp, scale, `TimeStep` — rather than a hand-made duration.
    // That is also the coarsest step the running app can produce, i.e. the hardest
    // case for contact detection.
    let mut scene = Scene::two_body_gravity();
    let mut clock = SimulationClock::new(TWO_BODY_TIME_SCALE);
    let step_seconds = f64::from(MAX_REAL_STEP_SECONDS) * f64::from(TWO_BODY_TIME_SCALE);
    let steps = (1.4 * Scene::two_body_free_fall_seconds() / step_seconds).ceil() as usize;

    let contact_separation = 2.0 * TWO_BODY_VISUAL_RADIUS_UNITS;
    let mut peak_substeps = 0_u32;
    let mut closest = f32::INFINITY;
    let mut separation_after_contact: Option<f32> = None;
    let mut touched = false;

    for _ in 0..steps {
        // Pins what the two substep criteria each contribute. The **displacement** rule never
        // fires for this scene: at the coarsest step the app can produce, the pair never moves
        // more than `MAX_SUBSTEP_DISPLACEMENT_UNITS` in one frame, which is the claim that
        // constant's docs make.
        //
        // The **dynamical-time** rule does fire, near contact, and that is the point of it. The
        // pair is most tightly bound at its closest approach, where `t_dyn` has fallen from
        // `1.8e8` s at the `30`-unit start to `5.8e6` s at the `3`-unit contact separation — so
        // the final approach was being integrated an order of magnitude too coarsely and the
        // displacement rule could not see it. The count is recorded and checked after the loop
        // rather than pinned here, because it varies across the encounter by design.
        let step = clock.advance(MAX_REAL_STEP_SECONDS);
        let substeps = substep_count(&scene.props, step);
        peak_substeps = peak_substeps.max(substeps);

        let travel = scene.props[1].speed().max(scene.props[2].speed()) * step.seconds_f32();
        assert!(
            travel <= MAX_SUBSTEP_DISPLACEMENT_UNITS,
            "the displacement criterion should never fire for this scene, but travel was {travel}"
        );

        scene.step(step);

        let offset = scene.props[2].body.center() - scene.props[1].body.center();
        let separation = offset.length();
        closest = closest.min(separation);

        // The bounce is detected by the pair **separating**, not by a sampled proximity.
        // Gravitation alone can only close this pair — they start at rest — so a positive
        // relative radial velocity is possible *only* after a contact has reversed it. That is
        // a stronger claim than "they were once within 3 units", and it is frame-rate
        // independent: with the dynamical criterion now taking 12 substeps through the
        // encounter, the whole approach-bounce-recede sequence can happen inside one frame and
        // a once-per-frame distance sample misses it entirely. It did, which is how this was
        // found.
        let closing_speed = (scene.props[2].velocity_units_s - scene.props[1].velocity_units_s)
            .dot(offset.normalize_or_zero());

        if closing_speed > 0.0 {
            touched = true;
        }
        if touched {
            separation_after_contact =
                Some(separation.max(separation_after_contact.unwrap_or(0.0)));
        }

        for prop in &scene.props {
            assert!(prop.is_valid(), "{prop:?} went invalid mid-flight");
        }
    }

    assert!(
        touched,
        "the pair never began separating, so no bounce happened — gravitation alone cannot \
         reverse a pair released from rest"
    );

    // The dynamical-time criterion's whole contribution to this scene, in one number. It stays
    // at `1` for the long infall and peaks at `12` through the encounter, where `t_dyn` is at
    // its shortest — so the close approach now gets twelve times the resolution it used to, and
    // the bounce is no longer the coarsest-integrated part of the experiment. Bounded well under
    // `MAX_SUBSTEPS`, so nothing is being silently capped.
    assert_eq!(
        peak_substeps, 12,
        "the encounter's substep count is what the dynamical criterion buys this scene"
    );
    assert!(peak_substeps < MAX_SUBSTEPS);

    // The clock makes this checkable for the first time: the pair must have met
    // within the free-fall time the scene's own constants predict. Before
    // `SimulationClock` existed there was no elapsed simulated time to compare
    // against, so the docs' "meets after ~2.0e8 s" claim was untestable.
    let predicted = Scene::two_body_free_fall_seconds();
    assert!(
        clock.elapsed_seconds() >= predicted,
        "ran {:.3e} s of simulated time, less than the predicted {:.3e} s free fall",
        clock.elapsed_seconds(),
        predicted
    );
    assert!(clock.is_valid());

    // The hulls are inscribed icospheres, so the centres can close to
    // `2 · inradius` before contact is found: 6.6% inside the nominal contact
    // distance, plus a little for the step that carries them in. Nothing like the
    // free pass they had before this layer existed, where they crossed to the far
    // side.
    let floor = 0.85 * contact_separation;
    assert!(
        closest >= floor,
        "the spheres interpenetrated to {closest} units, past the {floor} allowed \
         by the hull's undershoot — they are phasing through"
    );

    let recovered = separation_after_contact.expect("the pair must separate again after bouncing");
    assert!(
        recovered > contact_separation,
        "an elastic bounce must carry them back apart, reached only {recovered}"
    );

    // And they came out the way they went in: neither sphere crossed the other.
    assert!(
        scene.props[1].body.center().x < scene.props[2].body.center().x,
        "the spheres swapped sides, which is exactly the phasing-through this layer prevents"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Whatever the masses and approach speeds, a bounce between two movable props
    /// conserves both momentum and kinetic energy along the contact normal. This is
    /// the law the response is built from, asserted over the whole input space
    /// rather than at one point.
    #[test]
    fn a_bounce_always_conserves_momentum_and_energy(
        mass_a in 0.5_f32..1000.0,
        mass_b in 0.5_f32..1000.0,
        speed_a in 0.1_f32..50.0,
        speed_b in 0.1_f32..50.0,
    ) {
        let mut props = vec![
            Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(speed_a, 0.0, 0.0), mass_a),
            Prop::new(ball(Vec3::new(1.5, 0.0, 0.0), 1.0), Vec3::new(-speed_b, 0.0, 0.0), mass_b),
        ];

        let before = axis_bodies(&props, 0);
        prop_assert_eq!(resolve_contacts(&mut props), 1);
        let after = axis_bodies(&props, 0);

        prop_assert!(momentum_is_conserved(&before, &after, 1.0e-5));
        prop_assert!(kinetic_energy_is_conserved(&before, &after, 1.0e-5));
        // And they are now leaving, which is what makes the bounce a bounce.
        prop_assert!(props[0].velocity_units_s.x < props[1].velocity_units_s.x);
    }

    /// Resolution never invents a non-finite value and never breaks a prop's
    /// Validation_Rules, however deep the overlap it is handed.
    #[test]
    fn resolution_preserves_validity_at_any_depth(offset in 0.01_f32..1.99, mass in 0.5_f32..100.0) {
        let mut props = vec![
            Prop::new(ball(Vec3::ZERO, 1.0), Vec3::new(1.0, 0.0, 0.0), mass),
            Prop::new(ball(Vec3::new(offset, 0.0, 0.0), 1.0), Vec3::new(-1.0, 0.0, 0.0), mass),
            Prop::immovable(floor()),
        ];

        resolve_contacts(&mut props);

        for prop in &props {
            prop_assert!(prop.is_valid());
        }
    }

    /// Separation always moves the pair apart along the normal, never together:
    /// the centres are further apart after resolution than before.
    #[test]
    fn separation_increases_the_gap(offset in 0.2_f32..1.9) {
        let mut props = vec![
            Prop::at_rest(ball(Vec3::ZERO, 1.0), 4.0),
            Prop::at_rest(ball(Vec3::new(offset, 0.0, 0.0), 1.0), 4.0),
        ];
        let before = (props[1].body.center() - props[0].body.center()).length();

        resolve_contacts(&mut props);

        let after = (props[1].body.center() - props[0].body.center()).length();
        prop_assert!(after >= before);
    }
}

#[test]
fn a_placed_black_hole_and_neutron_star_saturate_the_substep_cap_before_contact() {
    // Pins the measurement `MAX_SUBSTEPS`' docs now quote, from the scenario a user hit: a
    // `10 M☉` black hole that has swallowed the Sun (`2.19e31 kg`) closing on a `1.4 M☉` neutron
    // star, at the solar system's own clock.
    //
    // The claim is that the cap is reached *outside* the hole's `1.499`-unit capture radius, so the
    // final approach of any such encounter is under-resolved. That is a real accuracy limit and it
    // is invisible to every conservation check in the crate — see
    // `momentum_is_conserved_even_when_the_step_is_far_too_large`.
    use crate::collision::constants::MAX_SUBSTEPS;

    let pair = |separation_units: f32| {
        let sphere = |x: f32| Body::Sphere {
            pose: Pose {
                position: Vec3::new(x, 0.0, 0.0),
                rotation: Quat::IDENTITY,
            },
            // True radii: both are far smaller than the separations here, so the substep count is
            // set by the dynamical criterion rather than by displacement against a hull.
            radius: 1.0e-5,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        };

        vec![
            Prop::new(sphere(0.0), Vec3::ZERO, 2.188e31),
            Prop::new(sphere(separation_units), Vec3::ZERO, 2.8e30),
        ]
    };

    // One solar-system frame: `1e6` seconds of simulated time per second of wall clock at 60 Hz.
    let frame = TimeStep::from_seconds(1.0e6 / 60.0).expect("a valid frame");

    // Comfortably resolved where the objects are placed — 40 units in front of the camera.
    let at_placement = substep_count(&pair(40.0), frame);
    assert!(
        at_placement < MAX_SUBSTEPS,
        "a freshly placed pair should not saturate the cap, got {at_placement}"
    );

    // Saturated well before contact. `5.22` units is where the two criteria cross; assert on
    // either side of it rather than on the number itself, so the test states the *fact* rather
    // than reproducing the arithmetic that produced it.
    assert!(
        substep_count(&pair(6.0), frame) < MAX_SUBSTEPS,
        "the cap should not yet be reached at 6 units"
    );
    assert_eq!(
        substep_count(&pair(4.5), frame),
        MAX_SUBSTEPS,
        "the cap should be saturated by 4.5 units"
    );

    // And still saturated at the capture radius, which is the consequence that matters: the
    // encounter is already under-resolved by the time the hole can eat.
    assert_eq!(
        substep_count(&pair(1.499), frame),
        MAX_SUBSTEPS,
        "the final approach to the capture radius is capped"
    );
}

fn posed(position: Vec3, rotation: Quat) -> Pose {
    Pose { position, rotation }
}

fn slab(position: Vec3, rotation: Quat, half_size: f32) -> Body {
    Body::Plane {
        pose: posed(position, rotation),
        half_size,
        color: white(),
    }
}

fn crate_box(position: Vec3, rotation: Quat, half_extents: Vec3) -> Body {
    Body::Cuboid {
        pose: posed(position, rotation),
        half_extents,
        color: white(),
    }
}

/// What `resolve_contacts` does to a pair with no broad phase at all: the narrow phase on the
/// real hulls, then the response. The broad phase may only skip work, never change this.
fn resolved_by_the_narrow_phase_alone(mut props: Vec<Prop>) -> Vec<Prop> {
    if let Some(contact) = contact_between_props(&props[0], &props[1]) {
        apply_contact(&mut props, 0, 1, &contact);
    }
    props
}

#[test]
fn a_body_far_above_a_floor_wider_than_its_orbit_is_ruled_out_before_any_hull() {
    // The solar system's case: the floor's bounding sphere holds every orbit, so only the box
    // test can tell that a moon 3 units up is nowhere near it.
    let floor = Prop::immovable(slab(Vec3::ZERO, Quat::IDENTITY, DEMO_FLOOR_HALF_SIZE_UNITS));
    let moon = Prop::new(ball(Vec3::new(1200.0, 3.0, -800.0), 0.07), Vec3::X, 1.0);

    assert!(
        bounding_spheres_overlap(&moon, &floor),
        "the scenario must be one a bounding sphere cannot reject"
    );
    assert!(!may_touch(&moon, &floor));
    assert!(!may_touch(&floor, &moon));

    let mut props = vec![moon, floor];
    let before = props.clone();
    assert_eq!(resolve_contacts(&mut props), 0);
    assert_eq!(props, before);
}

#[test]
fn the_box_test_judges_a_sphere_by_its_contact_radius() {
    let floor = Prop::immovable(slab(Vec3::ZERO, Quat::IDENTITY, 20.0));
    // Drawn ten units across and hanging into the floor, physically 0.07 units and well clear.
    let drawn_big = || ball(Vec3::new(3.0, 5.0, 0.0), 10.0);

    let honest = Prop::new(drawn_big(), Vec3::ZERO, 1.0).with_contact_radius(0.07);
    assert!(!may_touch(&honest, &floor));

    let as_drawn = Prop::new(drawn_big(), Vec3::ZERO, 1.0);
    assert!(contact_between_props(&as_drawn, &floor).is_some());
    assert!(may_touch(&as_drawn, &floor));
}

#[test]
fn a_sphere_touching_a_slab_or_box_collides_exactly_as_the_narrow_phase_says() {
    let tilt = Quat::from_rotation_z(0.6) * Quat::from_rotation_x(0.3);
    let spin = Quat::from_rotation_y(0.9) * Quat::from_rotation_x(-0.4);
    let cases = [
        (
            "resting in the floor",
            ball(Vec3::new(0.0, 0.9, 0.0), 1.0),
            slab(Vec3::ZERO, Quat::IDENTITY, 20.0),
        ),
        (
            "over a corner",
            ball(Vec3::new(20.5, -0.3, 20.5), 1.0),
            slab(Vec3::ZERO, Quat::IDENTITY, 20.0),
        ),
        (
            "under a corner",
            ball(Vec3::new(20.3, -1.4, -20.3), 1.0),
            slab(Vec3::ZERO, Quat::IDENTITY, 20.0),
        ),
        (
            "in a tilted floor",
            ball(tilt * Vec3::new(3.0, 0.8, -4.0), 1.0),
            slab(Vec3::ZERO, tilt, 20.0),
        ),
        (
            "over a tilted corner",
            ball(tilt * Vec3::new(20.4, -0.5, 19.8), 1.0),
            slab(Vec3::ZERO, tilt, 20.0),
        ),
        (
            "on a spun box's edge",
            ball(
                Vec3::new(5.0, 2.0, -1.0) + spin * Vec3::new(1.6, 0.9, 0.0),
                0.5,
            ),
            crate_box(Vec3::new(5.0, 2.0, -1.0), spin, Vec3::new(1.5, 0.75, 2.0)),
        ),
    ];

    for (name, sphere, boxed) in cases {
        let props = vec![
            Prop::new(sphere, Vec3::new(0.0, -2.0, 0.0), 4.0),
            Prop::immovable(boxed),
        ];
        assert!(
            contact_between_props(&props[0], &props[1]).is_some(),
            "{name}: the case must be a real contact"
        );

        let expected = resolved_by_the_narrow_phase_alone(props.clone());
        let mut actual = props;
        assert_eq!(resolve_contacts(&mut actual), 1, "{name}");
        assert_eq!(actual, expected, "{name}");
    }
}

/// A rotation about a random axis, knocked off unit length by as much as `Pose::is_valid`
/// allows so the hull is stretched by `|q|²`.
fn rotation_strategy() -> impl Strategy<Value = Quat> {
    (
        (-1.0_f32..1.0, -1.0_f32..1.0, -1.0_f32..1.0),
        -std::f32::consts::PI..std::f32::consts::PI,
        -9.0e-5_f32..9.0e-5,
    )
        .prop_map(|((x, y, z), angle, unit_error)| {
            let axis = Vec3::new(x, y, z).try_normalize().unwrap_or(Vec3::Y);
            Quat::from_axis_angle(axis, angle) * (1.0 + unit_error)
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2048,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// The broad phase is conservative: whenever the narrow phase finds the real hulls touching,
    /// the pair was let through. Covers sphere and cuboid against plane and cuboid, at any pose,
    /// with the other prop placed around the box's surface so roughly half the cases touch, and
    /// far from the origin so rounding at large coordinates is exercised too.
    #[test]
    fn the_broad_phase_lets_through_every_pair_the_narrow_phase_finds_touching(
        boxed_is_plane in any::<bool>(),
        other_is_sphere in any::<bool>(),
        boxed_rotation in rotation_strategy(),
        other_rotation in rotation_strategy(),
        extents in (0.2_f32..4.0, 0.2_f32..4.0, 0.2_f32..4.0),
        size in 0.1_f32..3.0,
        contact_radius in prop::option::of(0.05_f32..3.0),
        reach in (-1.3_f32..1.3, -1.3_f32..1.3, -1.3_f32..1.3),
        far in -3000.0_f32..3000.0,
    ) {
        let origin = far * Vec3::new(1.0, 0.3, -0.7);
        let (boxed, box_extent) = if boxed_is_plane {
            (slab(origin, boxed_rotation, extents.0), Vec3::new(extents.0, 0.5, extents.0))
        } else {
            let half_extents = Vec3::new(extents.0, extents.1, extents.2);
            (crate_box(origin, boxed_rotation, half_extents), half_extents)
        };

        let reach = Vec3::new(reach.0, reach.1, reach.2) * (box_extent + Vec3::splat(size));
        let centre = origin + boxed_rotation * reach;
        let other = if other_is_sphere {
            let sphere = Prop::new(ball(centre, size), Vec3::ZERO, 1.0);
            match contact_radius {
                Some(radius) => sphere.with_contact_radius(radius),
                None => sphere,
            }
        } else {
            let half_extents = Vec3::new(size, 0.7 * size, 1.3 * size);
            Prop::new(crate_box(centre, other_rotation, half_extents), Vec3::ZERO, 1.0)
        };
        let boxed = Prop::immovable(boxed);

        if contact_between_props(&other, &boxed).is_some() {
            prop_assert!(may_touch(&other, &boxed));
            prop_assert!(may_touch(&boxed, &other));
        }
    }
}
