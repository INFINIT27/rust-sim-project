//! Unit and property tests for the [`crate::input::camera`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/input/camera.rs`
//! via `#[path]`, not as a standalone integration test, so these tests keep
//! access to the module's private items and to `super::*`.

//! Unit + property tests for section 8.
//!
//! - Default-field check for `FreeFlyController` (pins task 8.1).
//! - Spot checks for `clamp_pitch` at concrete values (task 8.2).
//! - Property 12 test for `clamp_pitch` (task 8.3).
//! - Smoke test confirming the plugin wires up without panicking
//!   on a `MinimalPlugins` app (tasks 8.4–8.8).

use super::*;
use crate::input::constants::CAMERA_PRECISION_MULTIPLIER;
use crate::time::constants::MAX_REAL_STEP_SECONDS;
use crate::time::observer::ObserverClock;
use bevy::app::App;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use std::f32::consts::FRAC_PI_2;

/// `FreeFlyController::default` must be exactly the layer's constants.
///
/// This test used to pin task 8.1's literals (`move_speed = 5.0` and the rest). It now
/// pins the *constants* instead, because the numbers moved to `input/constants.rs` where
/// they are documented — and `move_speed` has since been raised to `20.0` for a scene
/// whose floor is twice as wide. Asserting the literal here would mean two places to edit
/// for one decision, and this is the place that would be forgotten.
#[test]
fn free_fly_controller_default_matches_its_constants() {
    let c = FreeFlyController::default();

    assert_eq!(c.move_speed, CAMERA_MOVE_SPEED_UNITS_S);
    assert_eq!(c.boost_mult, CAMERA_BOOST_MULTIPLIER);
    assert_eq!(c.look_sensitivity, LOOK_SENSITIVITY_RAD_PER_PX);
    assert_eq!(c.yaw, 0.0);
    assert_eq!(c.pitch, 0.0);
    assert!(!c.captured);
}

/// The camera has to be able to cross the world in a reasonable time, and not so fast
/// that a boosted frame jumps past a body.
///
/// A range rather than a value, so tuning the speed does not require editing a test — but
/// a *bound*, so a change that made the camera unusable at either extreme fails here. The
/// scene is 100 units across and the smallest body is 1 unit across.
#[test]
fn the_camera_speed_is_usable_for_this_scene() {
    let world_width = 100.0_f32;
    let crossing_seconds = world_width / CAMERA_MOVE_SPEED_UNITS_S;
    assert!(
        (2.0..=30.0).contains(&crossing_seconds),
        "crossing the world takes {crossing_seconds} s; aim for a few seconds"
    );

    // A boosted 60 Hz frame must not skip past the smallest body: the camera would appear
    // to teleport through it, which is disorienting when you are trying to fly *to* it.
    // The bound is the body's diameter, derived from the constant rather than assumed, so
    // shrinking the bodies tightens it automatically.
    let smallest_body_diameter =
        2.0 * crate::experiments::constants::THREE_BODY_VISUAL_RADIUS_UNITS;
    let boosted_frame_units = CAMERA_MOVE_SPEED_UNITS_S * CAMERA_BOOST_MULTIPLIER / 60.0;
    assert!(
        boosted_frame_units < smallest_body_diameter,
        "a boosted frame moves {boosted_frame_units} units, past a          {smallest_body_diameter}-unit body"
    );
}

// -----------------------------------------------------------------------
// Spot checks for `clamp_pitch` (task 8.2).
//
// These pin the three branches of the contract to concrete values so a
// regression shows up even if the property-test strategies below ever
// drift. Each test names the clause of Requirement 10.4 it covers.
// -----------------------------------------------------------------------

/// In-range inputs pass through unchanged.
#[test]
fn clamp_pitch_passes_through_in_range() {
    assert_eq!(clamp_pitch(0.0), 0.0);
    assert_eq!(clamp_pitch(0.5), 0.5);
    assert_eq!(clamp_pitch(-0.5), -0.5);
    // Boundary-adjacent values inside the clamp range.
    let inside_upper = FRAC_PI_2 - 2e-3;
    let inside_lower = -FRAC_PI_2 + 2e-3;
    assert_eq!(clamp_pitch(inside_upper), inside_upper);
    assert_eq!(clamp_pitch(inside_lower), inside_lower);
}

/// Out-of-range inputs saturate at the open-interval bounds.
#[test]
fn clamp_pitch_saturates_out_of_range() {
    let upper = FRAC_PI_2 - 1e-3;
    let lower = -FRAC_PI_2 + 1e-3;
    assert_eq!(clamp_pitch(100.0), upper);
    assert_eq!(clamp_pitch(-100.0), lower);
    assert_eq!(clamp_pitch(FRAC_PI_2), upper);
    assert_eq!(clamp_pitch(-FRAC_PI_2), lower);
    assert_eq!(clamp_pitch(f32::INFINITY), upper);
    assert_eq!(clamp_pitch(f32::NEG_INFINITY), lower);
}

/// `NaN` collapses to `0.0` per the documented rule.
#[test]
fn clamp_pitch_nan_maps_to_zero() {
    assert_eq!(clamp_pitch(f32::NAN), 0.0);
}

// -----------------------------------------------------------------------
// Property 12: Camera pitch is clamped (task 8.3).
//
// **Validates: Requirements 10.4**
//
// For any arbitrary `pitch: f32` (including very large magnitudes,
// `NaN`, `+/-inf`), `clamp_pitch(pitch)` must lie strictly inside
// `(-π/2, π/2)`. `NaN` is handled by a documented rule
// (`clamp_pitch(NaN) == 0.0`), which the test encodes by asserting
// the output is finite and in the open interval in every case.
// -----------------------------------------------------------------------

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
///
/// The path is relative to the package root, which is Cargo's working directory
/// for tests. Commit the seeds it produces: they are the counterexamples this
/// suite has already paid for.
const REGRESSION_SEEDS: &str = "tst/input/proptest-regressions/camera_tests.txt";

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 12: Camera pitch is clamped.
    ///
    /// **Validates: Requirements 10.4**
    #[test]
    fn clamp_pitch_is_always_strictly_inside_open_half_pi_interval(
        pitch in any::<f32>(),
    ) {
        let out = clamp_pitch(pitch);

        // Finite: rules out `NaN` and `+/-inf` leaking through.
        prop_assert!(
            out.is_finite(),
            "clamp_pitch({}) = {} is not finite",
            pitch,
            out,
        );

        // Strictly inside (-π/2, π/2): the `1e-3` margin in
        // `clamp_pitch` guarantees this.
        prop_assert!(
            out > -FRAC_PI_2 && out < FRAC_PI_2,
            "clamp_pitch({}) = {} is not strictly inside (-π/2, π/2)",
            pitch,
            out,
        );

        // NaN has a defined rule: it maps to 0.0.
        if pitch.is_nan() {
            prop_assert_eq!(
                out,
                0.0,
                "clamp_pitch(NaN) must be 0.0, got {}",
                out,
            );
        }
    }
}

/// `FreeFlyCameraPlugin::build` (task 8.8) registers
/// `free_fly_camera_system` on `Update`. Running a single
/// `app.update()` tick on a minimal app with the plugin installed
/// must not panic — even without a primary window or camera
/// entity, the system bails out cleanly on the `get_single_mut`
/// branches and the `Option` window check.
#[test]
fn free_fly_camera_plugin_update_tick_does_not_panic() {
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ObserverClock>();
    app.add_plugins(FreeFlyCameraPlugin);

    // The system needs `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>`,
    // and `Events<MouseMotion>` resources to resolve its parameters
    // under MinimalPlugins. `InputPlugin` provides all three, but
    // MinimalPlugins doesn't include it, so initialize them
    // manually rather than pulling in a larger plugin set.
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.add_event::<MouseMotion>();

    // One tick should run without panicking, even though there's
    // no camera entity and no primary window. Both the
    // `get_single_mut` early-exit (no camera) and the `Option`
    // window handling (no primary window) are exercised here.
    app.update();
}

// -----------------------------------------------------------------------
// Translation: which key moves the camera which way.
//
// These are new with amendment A6, and they cover the pre-existing keys too — the
// movement directions had no tests at all before, only a smoke test that one tick does
// not panic. A camera whose keys move it along the wrong axis is the kind of defect that
// is obvious in the window and invisible in a test suite.
//
// The system tolerates a missing primary window (it skips only the cursor-state work), so
// these run under `MinimalPlugins` with a camera entity and no window at all.
// -----------------------------------------------------------------------

/// How far a single held key moves the camera in one frame of these tests.
///
/// `move_speed × motion_step_seconds()`, and the step is the *clamped* one: the observer
/// clock caps camera motion at `MAX_REAL_STEP_SECONDS` so a stalled frame cannot teleport
/// the viewer. Feeding it a one-second frame therefore yields a `0.1`-second step, not a
/// one-second one — which is the behaviour, and worth deriving here rather than writing a
/// literal that would silently mean something else if either constant moved.
fn step_units() -> f32 {
    CAMERA_MOVE_SPEED_UNITS_S * MAX_REAL_STEP_SECONDS
}

/// An `App` with the camera system, one camera entity at the origin, and a clock that has
/// seen a long frame — so every press moves the camera by exactly [`step_units`].
fn app_with_camera() -> App {
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    // Deliberately longer than the cap, so the clamp is part of what these tests exercise.
    let mut clock = ObserverClock::new();
    clock.advance(1.0);
    app.insert_resource(clock);

    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.add_event::<MouseMotion>();
    app.add_systems(Update, (free_fly_camera_system, cycle_camera_speed_system));

    // Looking down -Z from the origin: the default orientation, so local forward is -Z and
    // local right is +X. That keeps the local-axis expectations below readable.
    app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        FreeFlyController::default(),
    ));

    app
}

/// Hold `key` for one frame and return how far the camera moved.
fn movement_from(key: KeyCode) -> Vec3 {
    let mut app = app_with_camera();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    query.single(app.world()).translation
}

#[test]
fn e_moves_up_world_y_and_q_moves_down() {
    // Amendment A8 moved these from world Z to world Y. They are now *aliases* for
    // Space/ControlLeft rather than a third axis — see the next test.
    let speed = step_units();

    let with_e = movement_from(KeyCode::KeyE);
    assert!(
        (with_e - Vec3::new(0.0, speed, 0.0)).length() < 1.0e-4,
        "E must move +{speed} on world Y, moved {with_e:?}"
    );

    let with_q = movement_from(KeyCode::KeyQ);
    assert!(
        (with_q - Vec3::new(0.0, -speed, 0.0)).length() < 1.0e-4,
        "Q must move -{speed} on world Y, moved {with_q:?}"
    );
}

#[test]
fn q_and_e_are_aliases_for_space_and_control() {
    // Two bindings for one motion, because the muscle memory differs by tool. Asserting the
    // equivalence rather than the values, so the pairs cannot drift apart.
    assert_eq!(movement_from(KeyCode::KeyE), movement_from(KeyCode::Space));
    assert_eq!(
        movement_from(KeyCode::KeyQ),
        movement_from(KeyCode::ControlLeft)
    );
}

#[test]
fn an_alias_held_with_its_twin_does_not_double_the_speed() {
    // Both keys sum into one `delta` before normalisation, so holding E *and* Space is one
    // unit of rise. If they were applied separately this would be twice as fast, which is the
    // classic aliasing bug.
    let mut app = app_with_camera();
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyE);
        keys.press(KeyCode::Space);
    }
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    let moved = query.single(app.world()).translation;

    assert!(
        (moved - Vec3::new(0.0, step_units(), 0.0)).length() < 1.0e-4,
        "E + Space moved {moved:?}, expected one unit of rise"
    );
}

#[test]
fn no_key_moves_along_world_z_any_more() {
    // A consequence of A8 worth pinning: sliding along Z now means turning to face it. If a
    // Z binding is ever wanted back, this test is the reminder that it was removed on
    // purpose rather than lost.
    for key in [
        KeyCode::KeyQ,
        KeyCode::KeyE,
        KeyCode::Space,
        KeyCode::ControlLeft,
    ] {
        assert!(
            movement_from(key).z.abs() < 1.0e-6,
            "{key:?} moved along world Z"
        );
    }
}

#[test]
fn every_movement_key_uses_the_axis_it_is_documented_to_use() {
    let speed = step_units();

    // Camera-local: the entity looks down -Z, so forward is -Z and right is +X.
    assert!((movement_from(KeyCode::KeyW) - Vec3::new(0.0, 0.0, -speed)).length() < 1.0e-4);
    assert!((movement_from(KeyCode::KeyS) - Vec3::new(0.0, 0.0, speed)).length() < 1.0e-4);
    assert!((movement_from(KeyCode::KeyD) - Vec3::new(speed, 0.0, 0.0)).length() < 1.0e-4);
    assert!((movement_from(KeyCode::KeyA) - Vec3::new(-speed, 0.0, 0.0)).length() < 1.0e-4);

    // World: vertical, by either binding.
    assert!((movement_from(KeyCode::Space) - Vec3::new(0.0, speed, 0.0)).length() < 1.0e-4);
    assert!((movement_from(KeyCode::ControlLeft) - Vec3::new(0.0, -speed, 0.0)).length() < 1.0e-4);
    assert!((movement_from(KeyCode::KeyE) - Vec3::new(0.0, speed, 0.0)).length() < 1.0e-4);
    assert!((movement_from(KeyCode::KeyQ) - Vec3::new(0.0, -speed, 0.0)).length() < 1.0e-4);
}

#[test]
fn q_and_e_ignore_where_the_camera_is_looking() {
    // The point of a world-axis binding: pitching or yawing the view must not tilt it. If
    // this ever fails, someone has switched `Vec3::Z` for `transform.back()`.
    let mut app = app_with_camera();

    {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Transform, With<Camera3d>>();
        let mut transform = query.single_mut(app.world_mut());
        transform.rotation = Quat::from_euler(EulerRot::YXZ, 1.1, -0.7, 0.0);
    }

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    let moved = query.single(app.world()).translation;

    assert!(
        (moved - Vec3::new(0.0, step_units(), 0.0)).length() < 1.0e-4,
        "E moved {moved:?}; a rotated view must not change a world-axis binding"
    );
}

#[test]
fn opposite_keys_cancel_and_diagonals_do_not_exceed_the_speed() {
    let mut app = app_with_camera();
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyQ);
        keys.press(KeyCode::KeyE);
    }
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    assert_eq!(
        query.single(app.world()).translation,
        Vec3::ZERO,
        "Q and E together must cancel, not double"
    );

    // And a diagonal is normalised: holding three keys must not move the camera further
    // than holding one, which is what `delta.normalize()` is for.
    let mut app = app_with_camera();
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
        keys.press(KeyCode::KeyE);
    }
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    let diagonal = query.single(app.world()).translation.length();
    assert!(
        (diagonal - step_units()).abs() < 1.0e-3,
        "a three-key diagonal moved {diagonal}, expected {}",
        step_units()
    );
}

// -----------------------------------------------------------------------
// The three speed modes (amendment A26).
//
// `C` cycles `Normal -> Fast -> Slow -> Normal`. The hold modifiers these
// replaced are gone, and `the_old_hold_modifiers_do_nothing` says so.
// -----------------------------------------------------------------------

/// Press `C` `times` times, one app update per press, then return the controller's mode.
///
/// One update per press because the cycle is **edge**-triggered: pressing once and updating twice
/// must advance the mode once, and driving them together is what would hide a level-triggered bug.
fn mode_after(times: usize) -> CameraSpeed {
    let mut app = app_with_camera();

    for _ in 0..times {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);
        app.update();

        // Bevy's `InputPlugin` clears the just-pressed set at the start of every frame, and
        // `MinimalPlugins` does not include it — so without this the edge would persist and every
        // frame would look like a fresh press. The same reasoning as `controls_tests::press`.
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.release(KeyCode::KeyC);
        }
    }

    let mut controllers = app.world_mut().query::<&FreeFlyController>();
    controllers.single(app.world()).speed
}

/// How far one held movement key moves the camera in a frame, in the given mode.
fn movement_in(key: KeyCode, mode: CameraSpeed) -> f32 {
    let mut app = app_with_camera();

    {
        let mut controllers = app.world_mut().query::<&mut FreeFlyController>();
        let mut controller = controllers.single_mut(app.world_mut());
        controller.speed = mode;
    }

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    query.single(app.world()).translation.length()
}

#[test]
fn the_camera_starts_at_normal_speed() {
    // A fresh run must fly exactly as it did before A26 — the toggle adds a way to change speed, it
    // does not change the speed you start at.
    assert_eq!(FreeFlyController::default().speed, CameraSpeed::Normal);
    assert_eq!(mode_after(0), CameraSpeed::Normal);

    let moved = movement_in(KeyCode::KeyW, CameraSpeed::Normal);
    assert!(
        (moved - step_units()).abs() < 1.0e-4,
        "normal mode moved {moved}, expected the base {}",
        step_units()
    );
}

#[test]
fn c_cycles_normal_then_fast_then_slow_and_wraps() {
    // The order the docs promise, and the wrap. Three presses must return to where it started, or
    // the mode is reachable only by counting presses since launch.
    assert_eq!(mode_after(1), CameraSpeed::Fast);
    assert_eq!(mode_after(2), CameraSpeed::Slow);
    assert_eq!(mode_after(3), CameraSpeed::Normal);
    assert_eq!(mode_after(4), CameraSpeed::Fast);
}

#[test]
fn the_cycle_is_total_so_no_number_of_presses_escapes_the_three_modes() {
    // `next` is exhaustive over the enum, so this cannot fail — which is the point of asserting it:
    // a fourth variant added without a `next` arm would not compile, and a `next` that returned a
    // default on some path would fail here.
    let mut mode = CameraSpeed::Normal;

    for _ in 0..30 {
        mode = mode.next();
        assert!(matches!(
            mode,
            CameraSpeed::Slow | CameraSpeed::Normal | CameraSpeed::Fast
        ));
    }
}

#[test]
fn holding_c_does_not_race_through_the_modes() {
    // The bug a toggle invites, and the reason the cycle is its own edge-triggered system rather
    // than a branch inside the level-triggered movement system: `C` held for sixty frames must
    // advance the mode exactly once.
    let mut app = app_with_camera();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);
    app.update();

    // `clear()` without `release()` is exactly what holding a key looks like to Bevy: still
    // `pressed`, no longer `just_pressed`. Without the clear this test would pass for the wrong
    // reason under `MinimalPlugins`, since nothing else retires the edge.
    for _ in 0..59 {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
    }

    let mut controllers = app.world_mut().query::<&FreeFlyController>();
    assert_eq!(
        controllers.single(app.world()).speed,
        CameraSpeed::Fast,
        "a held C must advance the mode once, not once per frame"
    );
}

#[test]
fn each_mode_applies_its_own_multiplier() {
    let base = step_units();

    let cases = [
        (CameraSpeed::Slow, CAMERA_PRECISION_MULTIPLIER),
        (CameraSpeed::Normal, 1.0),
        (CameraSpeed::Fast, CAMERA_BOOST_MULTIPLIER),
    ];

    for (mode, factor) in cases {
        let moved = movement_in(KeyCode::KeyW, mode);
        let expected = base * factor;

        assert!(
            (moved - expected).abs() < 1.0e-4,
            "{mode:?} moved {moved}, expected {expected}"
        );
    }
}

#[test]
fn every_movement_key_obeys_the_mode() {
    // The multiplier is applied to the speed rather than to a direction, so no key can be left out.
    for mode in [CameraSpeed::Slow, CameraSpeed::Fast] {
        let expected = step_units() * mode.multiplier(&FreeFlyController::default());

        for key in [
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::KeyE,
            KeyCode::KeyQ,
            KeyCode::Space,
            KeyCode::ControlLeft,
        ] {
            let moved = movement_in(key, mode);

            assert!(
                (moved - expected).abs() < 1.0e-4,
                "{key:?} in {mode:?} moved {moved}, expected {expected}"
            );
        }
    }
}

#[test]
fn slow_is_much_further_from_normal_than_fast_is() {
    // The two are deliberately asymmetric: fast exists to cross a 4495-unit solar system, slow to
    // approach a body 0.0064 units across. A symmetrical pair would leave the slow end useless.
    let slow = movement_in(KeyCode::KeyW, CameraSpeed::Slow);
    let fast = movement_in(KeyCode::KeyW, CameraSpeed::Fast);
    let plain = step_units();

    assert!(slow < plain, "slow must be slower than the base speed");
    assert!(fast > plain, "and fast faster");
    assert!(
        (plain / slow) > 10.0 * (fast / plain),
        "the slow end should be far more aggressive: {}x down against {}x up",
        plain / slow,
        fast / plain
    );
}

#[test]
fn the_old_hold_modifiers_do_nothing() {
    // A26 removed them rather than leaving them working alongside the modes. Named so the removal
    // is on the record: holding `Shift` was the previous way to boost and is deep muscle memory, so
    // "it silently does nothing now" is a real behaviour worth being able to fail on.
    //
    // `KeyC` is deliberately absent — it is the cycle key, and pressing it is not a no-op.
    let expected = step_units();

    for modifier in [
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::SuperLeft,
        KeyCode::AltLeft,
    ] {
        let mut app = app_with_camera();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyW);
            keys.press(modifier);
        }
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, With<Camera3d>>();
        let moved = query.single(app.world()).translation.length();

        assert!(
            (moved - expected).abs() < 1.0e-4,
            "{modifier:?} still changed the speed: moved {moved}, expected the base {expected}"
        );
    }
}

#[test]
fn cycling_the_mode_moves_nothing_by_itself() {
    // `C` is not a direction, and it sits beside `WASD` — so it will be pressed by a hand resting
    // on the movement keys, and a `C` that nudged the camera would be found by feel.
    let mut app = app_with_camera();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>();
    assert_eq!(query.single(app.world()).translation, Vec3::ZERO);
}
#[test]
fn the_controller_default_carries_the_precision_multiplier() {
    assert_eq!(
        FreeFlyController::default().precision_mult,
        CAMERA_PRECISION_MULTIPLIER
    );
}

const LOCK_TOLERANCE: f32 = 1.0e-3;

fn lock_app() -> (App, Entity) {
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let mut clock = ObserverClock::new();
    clock.advance(1.0);
    app.insert_resource(clock);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.add_event::<MouseMotion>();
    app.add_event::<MouseWheel>();
    app.add_systems(
        Update,
        (toggle_camera_lock_system, free_fly_camera_system).chain(),
    );
    app.add_systems(PostUpdate, orbit_camera_system);

    app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        FreeFlyController::default(),
        CameraLock::default(),
    ));
    let ahead = app
        .world_mut()
        .spawn((CameraTarget, Transform::from_xyz(1.0, 0.0, -10.0)))
        .id();
    app.world_mut()
        .spawn((CameraTarget, Transform::from_xyz(10.0, 0.0, 0.0)));
    (app, ahead)
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

fn scroll(app: &mut App, lines: f32) {
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: lines,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

fn camera_state(app: &mut App) -> (Transform, CameraLock) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&Transform, &CameraLock), With<Camera3d>>();
    let (transform, lock) = query.single(app.world());
    (*transform, *lock)
}

fn assert_faces(transform: &Transform, point: Vec3) {
    let towards = (point - transform.translation).normalize();
    assert!(
        transform.forward().as_vec3().dot(towards) > 1.0 - LOCK_TOLERANCE,
        "camera at {:?} looking {:?} does not face {point:?}",
        transform.translation,
        transform.forward()
    );
}

fn entity_at(x: f32) -> Entity {
    Entity::from_raw(x as u32)
}

#[test]
fn the_lock_picks_the_candidate_nearest_the_crosshair() {
    let picked = pick_lock_target(
        Vec3::ZERO,
        Vec3::NEG_Z,
        [
            (entity_at(1.0), Vec3::new(2.0, 0.0, -10.0)),
            (entity_at(2.0), Vec3::new(0.5, 0.0, -100.0)),
        ],
    );
    assert_eq!(picked, Some(entity_at(2.0)), "angle decides, not distance");
}

#[test]
fn the_lock_ignores_candidates_far_off_the_crosshair_or_at_the_eye() {
    let picked = pick_lock_target(
        Vec3::ZERO,
        Vec3::NEG_Z,
        [
            (entity_at(1.0), Vec3::new(10.0, 0.0, 0.0)),
            (entity_at(2.0), Vec3::new(0.0, 0.0, 5.0)),
            (entity_at(3.0), Vec3::ZERO),
        ],
    );
    assert_eq!(picked, None);
}

#[test]
fn yaw_pitch_towards_inverts_the_camera_rotation() {
    for direction in [
        Vec3::NEG_Z,
        Vec3::X,
        Vec3::new(-1.0, 0.5, 2.0),
        Vec3::new(0.3, -0.9, -0.1),
    ] {
        let (yaw, pitch) = yaw_pitch_towards(direction);
        let forward = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
        assert!(
            (forward - direction.normalize()).length() < LOCK_TOLERANCE,
            "{direction:?} came back as {forward:?}"
        );
    }
}

#[test]
fn yaw_pitch_towards_straight_up_stays_inside_the_pitch_clamp() {
    let (_, pitch) = yaw_pitch_towards(Vec3::Y);
    assert_eq!(pitch, clamp_pitch(FRAC_PI_2));
}

#[test]
fn zooming_in_then_out_returns_to_the_same_distance() {
    let there = zoom_distance(10.0, 3.0, 0.0, 100.0);
    assert!(there < 10.0, "positive scroll zooms in");
    let back = zoom_distance(there, -3.0, 0.0, 100.0);
    assert!((back - 10.0).abs() < LOCK_TOLERANCE, "came back to {back}");
}

#[test]
fn zoom_stays_within_its_bounds() {
    assert_eq!(zoom_distance(10.0, 1000.0, 2.0, 50.0), 2.0);
    assert_eq!(zoom_distance(10.0, -1000.0, 2.0, 50.0), 50.0);
}

#[test]
fn the_minimum_orbit_clears_the_body_and_the_near_plane() {
    let (min, max) = orbit_distance_bounds(2.0, 0.1, 1000.0);
    assert!(
        min - 2.0 > 0.1,
        "the body's surface must lie beyond the near plane"
    );
    assert_eq!(max, 500.0);
}

#[test]
fn pixel_scroll_is_converted_to_lines() {
    let lines = scroll_lines([
        MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: Entity::PLACEHOLDER,
        },
        MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: LOCK_ZOOM_PIXELS_PER_LINE * 2.0,
            window: Entity::PLACEHOLDER,
        },
    ]);
    assert!((lines - 3.0).abs() < LOCK_TOLERANCE);
}

#[test]
fn f_locks_onto_the_body_in_front_and_faces_it() {
    let (mut app, ahead) = lock_app();
    tap(&mut app, KeyCode::KeyF);

    let (transform, lock) = camera_state(&mut app);
    let orbit = lock.0.expect("F must lock onto the body ahead");
    assert_eq!(orbit.target, ahead);
    assert!((orbit.distance - Vec3::new(1.0, 0.0, -10.0).length()).abs() < LOCK_TOLERANCE);
    assert!(
        transform.translation.length() < LOCK_TOLERANCE,
        "locking does not move the camera"
    );
    assert_faces(&transform, Vec3::new(1.0, 0.0, -10.0));
}

#[test]
fn f_with_nothing_near_the_crosshair_does_nothing() {
    let (mut app, ahead) = lock_app();
    app.world_mut().despawn(ahead);
    tap(&mut app, KeyCode::KeyF);
    assert!(!camera_state(&mut app).1.is_locked());
}

#[test]
fn a_second_f_releases_the_lock() {
    let (mut app, _) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    tap(&mut app, KeyCode::KeyF);
    assert!(!camera_state(&mut app).1.is_locked());
}

#[test]
fn the_locked_camera_follows_a_moving_target() {
    let (mut app, ahead) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    let distance = camera_state(&mut app).1 .0.unwrap().distance;

    let moved = Vec3::new(30.0, 4.0, -12.0);
    app.world_mut()
        .get_mut::<Transform>(ahead)
        .unwrap()
        .translation = moved;
    app.update();

    let (transform, _) = camera_state(&mut app);
    assert!(((transform.translation - moved).length() - distance).abs() < LOCK_TOLERANCE);
    assert_faces(&transform, moved);
}

#[test]
fn scrolling_zooms_toward_the_target() {
    let (mut app, _) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    let before = camera_state(&mut app).1 .0.unwrap().distance;

    scroll(&mut app, 2.0);

    let (transform, lock) = camera_state(&mut app);
    let after = lock.0.unwrap().distance;
    assert!((after - before * LOCK_ZOOM_STEP_PER_LINE.powi(2)).abs() < LOCK_TOLERANCE);
    let target = Vec3::new(1.0, 0.0, -10.0);
    assert!(((transform.translation - target).length() - after).abs() < LOCK_TOLERANCE);
}

#[test]
fn scrolling_cannot_put_the_camera_inside_the_target() {
    let (mut app, _) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    scroll(&mut app, 1000.0);
    let distance = camera_state(&mut app).1 .0.unwrap().distance;
    assert!(
        distance > 1.0,
        "a unit-scale target was entered at distance {distance}"
    );
}

#[test]
fn mouse_motion_orbits_the_target_at_a_fixed_distance() {
    let (mut app, _) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    let (before, lock) = camera_state(&mut app);
    let distance = lock.0.unwrap().distance;

    let mut query = app.world_mut().query::<&mut FreeFlyController>();
    query.single_mut(app.world_mut()).captured = true;
    app.world_mut().send_event(MouseMotion {
        delta: bevy::math::Vec2::new(300.0, -120.0),
    });
    app.update();

    let (after, _) = camera_state(&mut app);
    let target = Vec3::new(1.0, 0.0, -10.0);
    assert!(
        (after.translation - before.translation).length() > 1.0,
        "the camera must swing round"
    );
    assert!(((after.translation - target).length() - distance).abs() < LOCK_TOLERANCE);
    assert_faces(&after, target);
}

#[test]
fn flight_keys_are_inert_while_locked() {
    let (mut app, _) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    let before = camera_state(&mut app).0.translation;

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.update();

    assert!((camera_state(&mut app).0.translation - before).length() < LOCK_TOLERANCE);
}

#[test]
fn a_despawned_target_releases_the_lock_and_leaves_the_camera_in_place() {
    let (mut app, ahead) = lock_app();
    tap(&mut app, KeyCode::KeyF);
    let before = camera_state(&mut app).0.translation;

    app.world_mut().despawn(ahead);
    app.update();

    let (transform, lock) = camera_state(&mut app);
    assert!(!lock.is_locked());
    assert!((transform.translation - before).length() < LOCK_TOLERANCE);
}

#[test]
fn the_plugin_registers_the_lock_systems() {
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(ObserverClock::new());
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.add_event::<MouseMotion>();
    app.add_plugins(FreeFlyCameraPlugin);
    app.update();
}

#[test]
fn an_open_popup_makes_the_speed_key_inert_too() {
    use crate::input::popup::PopupState;

    // The pop-up stands the camera down like the menu does, for every camera binding, not only
    // capture and movement.
    let mut app = app_with_camera();
    app.init_resource::<PopupState>();
    app.world_mut().resource_mut::<PopupState>().open();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);
    app.update();

    let speed = app
        .world_mut()
        .query::<&FreeFlyController>()
        .single(app.world())
        .speed;
    assert_eq!(speed, CameraSpeed::Normal);
}

#[test]
fn changed_camera_settings_reach_the_flying_camera_without_resetting_its_view() {
    let mut app = app_with_camera();
    app.init_resource::<CameraSettings>();
    app.add_systems(Update, apply_camera_settings_system);
    app.update();
    {
        let mut query = app.world_mut().query::<&mut FreeFlyController>();
        let mut controller = query.single_mut(app.world_mut());
        controller.yaw = 1.25;
    }

    app.world_mut().resource_mut::<CameraSettings>().move_speed = 77.0;
    app.update();

    let controller = *app
        .world_mut()
        .query::<&FreeFlyController>()
        .single(app.world());
    assert_eq!(controller.move_speed, 77.0);
    assert_eq!(
        controller.yaw, 1.25,
        "the view is the camera's, not the settings'"
    );
}
