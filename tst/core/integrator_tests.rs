//! Unit and property tests for the [`crate::core::integrator`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/core/integrator.rs` via `#[path]`, not as a standalone integration test,
//! so these tests keep access to the module's private items and to `super::*`.
//!
//! The integrator composes four `physics_utils` functions and adds no formula of
//! its own, so the tests here are mostly about *composition*: that the force is
//! symmetric, that only the right props move, that the whole thing conserves
//! momentum, and that a circular orbit stays roughly circular. Where a
//! conservation law is asserted, it goes through
//! [`crate::physics_utils::conservation_laws`] rather than a hand-rolled
//! comparison — the checks and the simulation should agree with each other.

use super::*;
use crate::constants::{
    EARTH_MASS_KG, METRES_PER_UNIT, TWO_BODY_HEIGHT_UNITS, TWO_BODY_SEPARATION_UNITS,
};
use crate::core::constants::GRAVITY_SOFTENING_UNITS;
use crate::core::scene::{Body, Color, Pose, Quat, Scene};
use crate::physics_utils::conservation_laws::momentum_is_conserved;
use crate::time::simulation::TimeStep;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
const REGRESSION_SEEDS: &str = "tst/core/proptest-regressions/integrator_tests.txt";

/// A `TimeStep` from a plain number of seconds, for tests that are about durations
/// rather than about validation. Panics on an invalid duration, which is what a test
/// wants: an unusable literal here is a mistake in the test, not a case to handle.
fn seconds(value: f64) -> TimeStep {
    TimeStep::from_seconds(value).expect("test durations must be valid")
}

/// A unit sphere at `position`, for building props without repeating geometry.
fn ball_at(position: Vec3) -> Body {
    Body::Sphere {
        pose: Pose {
            position,
            rotation: Quat::IDENTITY,
        },
        radius: 1.0,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    }
}

/// Total momentum of a slice of props along one axis, as
/// `(mass_kg, velocity_units_s)` pairs in `f64` — the shape
/// `conservation_laws` wants.
fn axis_bodies(props: &[Prop], axis: usize) -> Vec<(f64, f64)> {
    props
        .iter()
        .filter(|p| !p.immovable)
        .map(|p| (p.mass_kg as f64, p.velocity_units_s[axis] as f64))
        .collect()
}

/// Masses large enough for gravity to do something at the new unit scale, small
/// enough that nothing overflows.
///
/// Raised from `1e9..1e15` when one unit became `1e9 m`: with separations in units
/// and accelerations scaled back down by the same factor, a `1e9 kg` pair produces
/// an acceleration around `1e-31 units/s²`. That is still a normal `f32`, but it
/// leaves no headroom, and planetary masses are what this scale is for.
fn testable_mass() -> impl Strategy<Value = f32> {
    1.0e20_f32..1.0e26_f32
}

/// Positions within the scene bounds, in world units.
fn scene_position() -> impl Strategy<Value = Vec3> {
    (-20.0..20.0_f32, 0.0..20.0_f32, -20.0..20.0_f32).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

/// Timesteps a frame might plausibly produce, scaled.
fn small_dt() -> impl Strategy<Value = f32> {
    0.0..1.0e-3_f32
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1: the pairwise pull obeys Newton's third law.
    ///
    /// Two props of possibly very different masses feel forces of equal magnitude
    /// and opposite direction, so their *accelerations* are opposite and in
    /// inverse ratio to their masses. Asserted through the force rather than the
    /// acceleration, by scaling each acceleration back up by its own mass.
    #[test]
    fn the_pair_forces_are_equal_and_opposite(
        mass_a in testable_mass(),
        mass_b in testable_mass(),
        offset in 1.0..30.0_f32,
    ) {
        let props = [
            Prop::at_rest(ball_at(Vec3::ZERO), mass_a),
            Prop::at_rest(ball_at(Vec3::new(offset, 0.0, 0.0)), mass_b),
        ];

        let force_on_a = gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS) * mass_a;
        let force_on_b = gravitational_acceleration(&props, 1, GRAVITY_SOFTENING_UNITS) * mass_b;

        let scale = force_on_a.length().max(1.0e-30);
        prop_assert!(
            (force_on_a + force_on_b).length() / scale <= 1.0e-4,
            "forces {:?} and {:?} do not cancel", force_on_a, force_on_b
        );
    }

    /// Property 2: gravity always pulls, never pushes.
    ///
    /// The acceleration on a prop points *toward* the other prop. A sign error
    /// anywhere in the direction computation would make the pair fly apart, which
    /// no magnitude-only test would catch.
    #[test]
    fn gravity_pulls_toward_the_other_prop(
        mass_a in testable_mass(),
        mass_b in testable_mass(),
        position_b in scene_position(),
    ) {
        let position_a = Vec3::new(-30.0, 0.0, 0.0);
        let props = [
            Prop::at_rest(ball_at(position_a), mass_a),
            Prop::at_rest(ball_at(position_b), mass_b),
        ];

        let acceleration = gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS);
        let toward_b = (position_b - position_a).normalize();

        // Cosine of the angle between the acceleration and the direction to the
        // other prop must be ~1.
        prop_assert!(
            acceleration.normalize().dot(toward_b) > 0.999,
            "acceleration {:?} is not aimed at {:?}", acceleration, toward_b
        );
    }

    /// Property 3: acceleration falls off as the inverse square of separation.
    ///
    /// Doubling the distance quarters the acceleration — the defining shape of the
    /// law, and what a `/r` bug would break while still passing the direction and
    /// third-law properties.
    ///
    /// Restricted to separations well outside the softening length, where the
    /// softened law `a ∝ 1/(r² + ε²)` is asymptotically a pure inverse square. With
    /// `ε = 1` unit, `r ≥ 50` units keeps the deviation below `ε²/r² = 4e-4`. At
    /// `r ≈ ε` the property legitimately fails — it did, on an earlier run — and the
    /// softened regime has its own test
    /// (`softening_bounds_the_acceleration_at_tiny_separations`). Asserting the
    /// softened formula here as well would just restate the implementation.
    #[test]
    fn acceleration_obeys_the_inverse_square_law(
        mass in testable_mass(),
        separation in 50.0..200.0_f32,
    ) {
        let near = [
            Prop::at_rest(ball_at(Vec3::ZERO), mass),
            Prop::at_rest(ball_at(Vec3::new(separation, 0.0, 0.0)), mass),
        ];
        let far = [
            Prop::at_rest(ball_at(Vec3::ZERO), mass),
            Prop::at_rest(ball_at(Vec3::new(2.0 * separation, 0.0, 0.0)), mass),
        ];

        let a_near = gravitational_acceleration(&near, 0, GRAVITY_SOFTENING_UNITS).length();
        let a_far = gravitational_acceleration(&far, 0, GRAVITY_SOFTENING_UNITS).length();

        prop_assert!(
            ((a_far * 4.0) - a_near).abs() <= 1.0e-3 * a_near,
            "a(r) = {:e} but a(2r) * 4 = {:e}", a_near, a_far * 4.0
        );
    }

    /// Property 4: a step conserves total momentum.
    ///
    /// Internal forces come in equal and opposite pairs (Newton's third law), so
    /// no amount of mutual gravitation can move the system's centre of mass.
    /// Checked per axis through `conservation_laws::momentum_is_conserved`, so the
    /// simulation and the conservation checks have to agree with each other.
    ///
    /// The immovable props are excluded from the tally: they absorb momentum
    /// without moving, which is exactly what "infinite effective mass" means, and
    /// including them would make the law fail for a legitimate reason.
    #[test]
    fn a_step_conserves_momentum(
        mass_a in testable_mass(),
        mass_b in testable_mass(),
        position_b in scene_position(),
        dt in small_dt(),
    ) {
        let mut props = vec![
            Prop::at_rest(ball_at(Vec3::ZERO), mass_a),
            Prop::at_rest(ball_at(position_b + Vec3::new(0.0, 0.0, 30.0)), mass_b),
        ];

        let before: Vec<Vec<(f64, f64)>> = (0..3).map(|axis| axis_bodies(&props, axis)).collect();
        integrate_gravity(&mut props, seconds(f64::from(dt)), GRAVITY_SOFTENING_UNITS);
        let after: Vec<Vec<(f64, f64)>> = (0..3).map(|axis| axis_bodies(&props, axis)).collect();

        for axis in 0..3 {
            prop_assert!(
                momentum_is_conserved(&before[axis], &after[axis], 1.0e-6),
                "axis {} momentum changed over dt = {:e}", axis, dt
            );
        }
    }

    /// Property 5: immovable props never move, whatever is pulling on them.
    #[test]
    fn immovable_props_never_move(
        mass in testable_mass(),
        position in scene_position(),
        dt in small_dt(),
    ) {
        let wall = Prop::immovable_with_mass(ball_at(Vec3::ZERO), mass);
        let mut props = vec![wall.clone(), Prop::at_rest(ball_at(position + Vec3::X * 25.0), mass)];

        integrate_gravity(&mut props, seconds(f64::from(dt)), GRAVITY_SOFTENING_UNITS);

        prop_assert_eq!(&props[0], &wall, "an immovable prop must be untouched");
    }

    /// Property 6: every prop stays valid across a step.
    ///
    /// This is `Scene::step`'s Postcondition 5.1, and the reason
    /// `GRAVITY_SOFTENING_UNITS` exists: without it a near-coincident pair would
    /// produce an infinite force and a non-finite velocity.
    #[test]
    fn a_step_preserves_validity(
        mass_a in testable_mass(),
        mass_b in testable_mass(),
        position_a in scene_position(),
        position_b in scene_position(),
        dt in small_dt(),
    ) {
        let mut props = vec![
            Prop::at_rest(ball_at(position_a), mass_a),
            Prop::at_rest(ball_at(position_b), mass_b),
        ];

        integrate_gravity(&mut props, seconds(f64::from(dt)), GRAVITY_SOFTENING_UNITS);

        for prop in &props {
            prop_assert!(prop.is_valid(), "prop invalid after a step: {:?}", prop);
        }
    }

    /// Property 7: a zero timestep changes nothing at all.
    ///
    /// `Scene::step`'s Postcondition 5.2, asserted at the integrator level.
    #[test]
    fn a_zero_timestep_is_a_no_op(
        mass_a in testable_mass(),
        mass_b in testable_mass(),
        position_b in scene_position(),
    ) {
        let original = vec![
            Prop::at_rest(ball_at(Vec3::ZERO), mass_a),
            Prop::at_rest(ball_at(position_b + Vec3::X * 25.0), mass_b),
        ];
        let mut props = original.clone();

        integrate_gravity(&mut props, seconds(0.0), GRAVITY_SOFTENING_UNITS);

        prop_assert_eq!(&props, &original);
    }
}

// ---- Acceleration: hand-written cases ----

/// A lone prop feels nothing.
#[test]
fn a_lone_prop_has_no_acceleration() {
    let props = [Prop::at_rest(ball_at(Vec3::ZERO), EARTH_MASS_KG)];

    assert_eq!(
        gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS),
        Vec3::ZERO
    );
}

/// A massless prop is not accelerated, though it still moves.
///
/// The distinction the `Prop` data model exists to preserve: `a = F/m` is
/// undefined at zero mass, so the prop coasts rather than being deflected.
#[test]
fn a_massless_prop_is_not_accelerated_but_still_coasts() {
    let mut props = vec![
        Prop::new(ball_at(Vec3::ZERO), Vec3::new(1.0, 0.0, 0.0), 0.0),
        Prop::at_rest(ball_at(Vec3::new(5.0, 0.0, 0.0)), EARTH_MASS_KG),
    ];

    assert_eq!(
        gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS),
        Vec3::ZERO,
        "a massless prop cannot be accelerated"
    );

    integrate_gravity(&mut props, seconds(1.0), GRAVITY_SOFTENING_UNITS);

    assert_eq!(
        props[0].velocity_units_s,
        Vec3::new(1.0, 0.0, 0.0),
        "its velocity must be unchanged"
    );
    assert_eq!(
        props[0].body.center(),
        Vec3::new(1.0, 0.0, 0.0),
        "but it must still have travelled at that velocity"
    );
}

/// An immovable, zero-mass prop is gravitationally inert.
///
/// This is the ground plane. A floor that tugged at everything above it would be
/// a surprise, and `Prop::immovable` sets `mass_kg` to `0.0` precisely so it does
/// not.
#[test]
fn the_ground_plane_pulls_on_nothing() {
    let scene = Scene::demo();
    let floor = scene.props[0].clone();
    assert!(floor.immovable && floor.mass_kg == 0.0);

    let props = [
        floor,
        Prop::at_rest(ball_at(Vec3::new(0.0, 3.0, 0.0)), 1.0e12),
    ];

    assert_eq!(
        gravitational_acceleration(&props, 1, GRAVITY_SOFTENING_UNITS),
        Vec3::ZERO,
        "an inert floor must not accelerate anything"
    );
}

/// An immovable prop *with* a mass is a fixed attractor.
///
/// The counterpart to the test above, and the reason `immovable_with_mass` exists:
/// immovability governs motion, mass governs the field.
#[test]
fn an_immovable_prop_with_mass_still_attracts() {
    let props = [
        Prop::immovable_with_mass(ball_at(Vec3::ZERO), EARTH_MASS_KG),
        Prop::at_rest(ball_at(Vec3::new(10.0, 0.0, 0.0)), 1.0),
    ];

    let acceleration = gravitational_acceleration(&props, 1, GRAVITY_SOFTENING_UNITS);

    assert!(acceleration.x < 0.0, "the satellite must be pulled inward");
    assert_eq!(acceleration.y, 0.0);
    assert_eq!(acceleration.z, 0.0);
}

/// Coincident props are skipped rather than producing a `NaN` direction.
#[test]
fn coincident_props_produce_no_acceleration() {
    let props = [
        Prop::at_rest(ball_at(Vec3::ZERO), EARTH_MASS_KG),
        Prop::at_rest(ball_at(Vec3::ZERO), EARTH_MASS_KG),
    ];

    let acceleration = gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS);

    assert_eq!(acceleration, Vec3::ZERO);
    assert!(acceleration.is_finite());
}

/// Softening caps the force instead of letting it diverge.
///
/// At a separation far below `GRAVITY_SOFTENING_UNITS` the acceleration must stay
/// finite and bounded by the `r = 0` limit `G·m/ε²`, rather than growing without
/// limit as a bare inverse square would.
#[test]
fn softening_bounds_the_acceleration_at_tiny_separations() {
    let mass = EARTH_MASS_KG;
    let tiny = GRAVITY_SOFTENING_UNITS * 1.0e-3;
    let props = [
        Prop::at_rest(ball_at(Vec3::ZERO), mass),
        Prop::at_rest(ball_at(Vec3::new(tiny, 0.0, 0.0)), mass),
    ];

    let acceleration = gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS).length();

    // The r = 0 limit is G*m/eps^2, in metres per second squared, converted back
    // into units per second squared the same way the integrator does.
    let softening_m = GRAVITY_SOFTENING_UNITS as f64 * METRES_PER_UNIT;
    let softened_limit =
        (6.674_30e-11_f64 * mass as f64 / (softening_m * softening_m) / METRES_PER_UNIT) as f32;

    assert!(acceleration.is_finite(), "must not diverge");
    assert!(
        acceleration <= softened_limit * 1.001,
        "acceleration {:e} exceeds the softened limit {:e}",
        acceleration,
        softened_limit
    );
}

// ---- Integration: hand-written cases ----

/// A single prop with a velocity travels in a straight line.
///
/// With nothing to attract it, the integrator reduces to Newton's first law.
#[test]
fn a_lone_moving_prop_travels_in_a_straight_line() {
    let mut props = vec![Prop::new(
        ball_at(Vec3::ZERO),
        Vec3::new(2.0, 0.0, -1.0),
        5.0,
    )];

    integrate_gravity(&mut props, seconds(3.0), GRAVITY_SOFTENING_UNITS);

    assert_eq!(props[0].body.center(), Vec3::new(6.0, 0.0, -3.0));
    assert_eq!(
        props[0].velocity_units_s,
        Vec3::new(2.0, 0.0, -1.0),
        "no force means no change in velocity"
    );
}

/// Two props released from rest fall toward each other symmetrically.
#[test]
fn two_props_released_from_rest_fall_together() {
    let mass = EARTH_MASS_KG;
    let mut props = vec![
        Prop::at_rest(ball_at(Vec3::new(-5.0, 0.0, 0.0)), mass),
        Prop::at_rest(ball_at(Vec3::new(5.0, 0.0, 0.0)), mass),
    ];

    // A day of simulated time: at this scale, gravitation needs a big `dt` to move
    // anything measurable.
    integrate_gravity(&mut props, seconds(86_400.0), GRAVITY_SOFTENING_UNITS);

    assert!(
        props[0].velocity_units_s.x > 0.0,
        "left prop must move right"
    );
    assert!(
        props[1].velocity_units_s.x < 0.0,
        "right prop must move left"
    );
    assert!(
        (props[0].velocity_units_s.x + props[1].velocity_units_s.x).abs()
            < 1.0e-6 * props[0].velocity_units_s.x.abs(),
        "equal masses must approach at equal speeds"
    );

    // The midpoint — the barycentre — has not moved.
    let midpoint = 0.5 * (props[0].body.center() + props[1].body.center());
    assert!(
        midpoint.length() < 1.0e-5,
        "the barycentre drifted to {:?}",
        midpoint
    );
}

/// The shipped scene falls together from rest, and the motion is purely radial.
///
/// Integrated over 90% of the analytic free-fall time in 2500 steps — the same
/// resolution the shipped time scale gives at 60 fps. Stopping short of contact
/// keeps the test in the regime where the timestep is adequate; the encounter
/// itself is covered by the stability test below.
///
/// The `y` and `z` assertions are the point of this test as much as the approach
/// is: with no tangential velocity there is no angular momentum, so the spheres
/// must stay exactly on the X axis. Any leak into another axis would mean the
/// integrator or the scene had introduced a rotation nobody asked for.
#[test]
fn the_shipped_scene_falls_together_along_one_axis() {
    let mut scene = Scene::two_body_gravity();

    let separation_of =
        |scene: &Scene| (scene.props[2].body.center() - scene.props[1].body.center()).length();

    let start_separation = separation_of(&scene);
    assert!(
        (start_separation - TWO_BODY_SEPARATION_UNITS).abs() < 1.0e-3,
        "expected a {}-unit start, got {}",
        TWO_BODY_SEPARATION_UNITS,
        start_separation
    );
    assert_eq!(scene.props[1].speed(), 0.0, "the pair must start at rest");
    assert_eq!(scene.props[2].speed(), 0.0, "the pair must start at rest");

    let steps = 2500;
    let dt = (0.9 * Scene::two_body_free_fall_seconds() / steps as f64) as f32;

    let mut previous = start_separation;
    for step in 0..steps {
        scene.step(seconds(f64::from(dt)));

        let current = separation_of(&scene);
        assert!(
            current <= previous,
            "separation grew from {} to {} at step {} — the approach must be monotone",
            previous,
            current,
            step
        );
        previous = current;
    }

    // Closed a long way, but has not met yet: the analytic answer at 90% of the
    // fall time is ~10.6 units, so anything in this window is the right shape.
    let end_separation = separation_of(&scene);
    assert!(
        end_separation > 5.0 && end_separation < 0.5 * TWO_BODY_SEPARATION_UNITS,
        "expected to close to roughly a third of the start, got {}",
        end_separation
    );

    for index in [1, 2] {
        let centre = scene.props[index].body.center();
        let velocity = scene.props[index].velocity_units_s;

        // Purely radial: no angular velocity was given, so none may appear.
        assert_eq!(
            centre.y, TWO_BODY_HEIGHT_UNITS,
            "prop {} left its starting height",
            index
        );
        assert_eq!(centre.z, 0.0, "prop {} acquired motion along Z", index);
        assert_eq!(velocity.y, 0.0, "prop {} acquired velocity along Y", index);
        assert_eq!(velocity.z, 0.0, "prop {} acquired velocity along Z", index);
    }

    // Equal masses, so equal and opposite speeds and a stationary barycentre.
    let velocity_a = scene.props[1].velocity_units_s.x;
    let velocity_b = scene.props[2].velocity_units_s.x;
    assert!(velocity_a > 0.0, "the left sphere must move right");
    assert!(velocity_b < 0.0, "the right sphere must move left");
    assert!(
        (velocity_a + velocity_b).abs() < 1.0e-4 * velocity_a.abs(),
        "equal masses must close at equal speeds, got {} and {}",
        velocity_a,
        velocity_b
    );

    let barycentre = 0.5 * (scene.props[1].body.center() + scene.props[2].body.center());
    assert!(
        (barycentre.x).abs() < 1.0e-3,
        "the barycentre drifted to x = {}",
        barycentre.x
    );

    for prop in &scene.props {
        assert!(
            prop.is_valid(),
            "prop invalid during the approach: {:?}",
            prop
        );
    }
}

/// The simulation stays finite and valid straight through the encounter.
///
/// Runs three free-fall times, so the spheres meet, whip through the softened core,
/// and separate again. **No claim is made about the trajectory after contact**:
/// there is no collision response, and the timestep is far too coarse through
/// closest approach for the numbers to mean anything. What is asserted is that the
/// simulation degrades into something bounded and finite rather than into `NaN` or
/// an escape to infinity — which is exactly what `GRAVITY_SOFTENING_UNITS` buys.
#[test]
fn the_simulation_survives_the_encounter() {
    let mut scene = Scene::two_body_gravity();

    let steps = 7500;
    let dt = (3.0 * Scene::two_body_free_fall_seconds() / steps as f64) as f32;

    for _ in 0..steps {
        scene.step(seconds(f64::from(dt)));
    }

    for prop in &scene.props {
        assert!(
            prop.is_valid(),
            "prop invalid after the encounter: {:?}",
            prop
        );
        assert!(
            prop.body.center().is_finite() && prop.velocity_units_s.is_finite(),
            "prop went non-finite: {:?}",
            prop
        );
        assert!(
            prop.body.center().length() < 1.0e4,
            "prop escaped to {:?} — softening should keep the encounter bounded",
            prop.body.center()
        );
    }
}

/// The floor is untouched by the simulation.
#[test]
fn the_container_plane_is_untouched_by_the_simulation() {
    let mut scene = Scene::two_body_gravity();
    let floor_before = scene.props[0].clone();

    for _ in 0..100 {
        scene.step(seconds(1.0e-9));
    }

    assert_eq!(scene.props[0], floor_before);
}

// ---- Precondition violations ----

/// A negative duration can no longer reach the integrator: it is rejected where a
/// `TimeStep` is built, so the old `debug_assert!` on `dt < 0.0` has nothing left to
/// catch. Asserting the rejection is strictly stronger — it holds in release too.
#[test]
fn a_negative_timestep_cannot_be_constructed() {
    assert_eq!(TimeStep::from_seconds(-1.0), None);

    // And the legal zero-length step leaves every prop exactly where it was.
    let mut props = vec![Prop::at_rest(ball_at(Vec3::ZERO), 1.0)];
    let before = props.clone();
    integrate_gravity(&mut props, TimeStep::ZERO, GRAVITY_SOFTENING_UNITS);
    assert_eq!(props, before);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "gravitational_acceleration: target")]
fn an_out_of_bounds_target_panics_in_debug() {
    let props = [Prop::at_rest(ball_at(Vec3::ZERO), 1.0)];
    let _ = gravitational_acceleration(&props, 7, GRAVITY_SOFTENING_UNITS);
}

/// In release, an invalid timestep leaves the scene alone and an out-of-bounds
/// target yields no acceleration.
#[test]
#[cfg(not(debug_assertions))]
fn precondition_violations_are_inert_in_release() {
    let original = vec![Prop::new(ball_at(Vec3::ZERO), Vec3::X, 1.0)];

    // The invalid-duration half of this test is gone: a negative or non-finite
    // duration cannot be built into a `TimeStep`, so there is no release fallback
    // left to exercise (amendment A3). The zero-length step is the nearest thing —
    // legal, and required to be inert.
    let mut props = original.clone();
    integrate_gravity(&mut props, TimeStep::ZERO, GRAVITY_SOFTENING_UNITS);
    assert_eq!(props, original);

    // An out-of-bounds target still has a release fallback, and still must not panic.
    assert_eq!(
        gravitational_acceleration(&original, 7, GRAVITY_SOFTENING_UNITS),
        Vec3::ZERO
    );
}
