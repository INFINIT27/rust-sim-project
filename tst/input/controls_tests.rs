//! Unit tests for the [`crate::input::controls`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/input/controls.rs`
//! via `#[path]`, not as a standalone integration test.
//!
//! These drive the real system through a Bevy `App` with `MinimalPlugins`, pressing
//! keys by writing into `ButtonInput<KeyCode>`, because the thing worth testing is
//! the *binding* — that `P` pauses and `]` speeds up — and a test that called
//! `clock.toggle_paused()` directly would assert nothing about the mapping.
//!
//! `ButtonInput::clear` is called between frames the way Bevy's own input plugin
//! does, so `just_pressed` behaves as it does in the running app; without that, one
//! simulated press would fire on every tick.

use super::*;
use bevy::app::{App, Update};
use bevy::MinimalPlugins;

/// An `App` with the controls system and a clock at a known scale.
fn app_with_controls(scale: f32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(SimulationClock::new(scale));
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_systems(Update, simulation_controls_system);
    app
}

/// Press `key` for exactly one frame, the way the real input plugin would.
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    // Bevy's input plugin clears the just-pressed set at the start of each frame;
    // doing the same here keeps `just_pressed` edge-triggered across our frames.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
}

fn clock(app: &App) -> SimulationClock {
    *app.world().resource::<SimulationClock>()
}

#[test]
fn p_pauses_and_resumes() {
    let mut app = app_with_controls(1000.0);
    assert!(!clock(&app).is_paused());

    press(&mut app, KeyCode::KeyP);
    assert!(clock(&app).is_paused());

    press(&mut app, KeyCode::KeyP);
    assert!(!clock(&app).is_paused());
}

#[test]
fn holding_p_does_not_flutter_the_pause_state() {
    // Edge-triggered: a held key must pause once, not sixty times a second — which
    // would leave the state a coin toss decided by the frame count.
    let mut app = app_with_controls(1000.0);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyP);

    for _ in 0..9 {
        app.update();
    }

    assert!(clock(&app).is_paused(), "one press, one pause");
}

#[test]
fn period_queues_a_single_step() {
    let mut app = app_with_controls(1000.0);

    press(&mut app, KeyCode::KeyP);
    press(&mut app, KeyCode::Period);

    let queued = clock(&app);
    assert!(queued.is_paused());
    assert!(queued.single_step_pending());
}

#[test]
fn bracket_right_speeds_up_and_bracket_left_slows_down() {
    let mut app = app_with_controls(1000.0);

    press(&mut app, KeyCode::BracketRight);
    assert_eq!(clock(&app).scale(), 1000.0 * TIME_SCALE_KEY_FACTOR);

    press(&mut app, KeyCode::BracketLeft);
    assert_eq!(clock(&app).scale(), 1000.0);
}

#[test]
fn the_speed_keys_cannot_leave_the_usable_range() {
    let mut app = app_with_controls(1000.0);

    for _ in 0..80 {
        press(&mut app, KeyCode::BracketLeft);
    }
    let slow = clock(&app);
    assert_eq!(slow.scale(), crate::time::constants::MIN_TIME_SCALE);
    assert!(slow.is_valid());

    for _ in 0..200 {
        press(&mut app, KeyCode::BracketRight);
    }
    let fast = clock(&app);
    assert_eq!(fast.scale(), crate::time::constants::MAX_TIME_SCALE);
    assert!(fast.is_valid());
}

#[test]
fn backslash_resets_the_scale_to_the_shipped_value() {
    let mut app = app_with_controls(1000.0);

    press(&mut app, KeyCode::BracketRight);
    press(&mut app, KeyCode::BracketRight);
    assert_ne!(clock(&app).scale(), TWO_BODY_TIME_SCALE);

    press(&mut app, KeyCode::Backslash);
    assert_eq!(clock(&app).scale(), TWO_BODY_TIME_SCALE);
}

#[test]
fn the_controls_never_touch_elapsed_simulated_time() {
    // These keys change *when and how fast* the experiment advances. They must never
    // advance it themselves — only `advance_clocks_system` may do that.
    let mut app = app_with_controls(1000.0);

    for key in [
        KeyCode::KeyP,
        KeyCode::Period,
        KeyCode::BracketRight,
        KeyCode::BracketLeft,
        KeyCode::Backslash,
    ] {
        press(&mut app, key);
    }

    assert_eq!(clock(&app).elapsed_seconds(), 0.0);
}

#[test]
fn no_binding_collides_with_the_camera() {
    // The camera owns WASD, Q/E, Space, Left Ctrl, C, and Esc. A collision would make one key do
    // two things, which is the kind of bug that only shows up in the running app. Keep this list in
    // step with `free_fly_camera_system`, `cycle_camera_speed_system`, and
    // `toggle_camera_lock_system`.
    //
    // `ShiftLeft` is deliberately **not** here any more: A26 replaced the hold modifiers with the
    // `C` cycle, so Shift is unbound and is free for a future binding. Leaving it in would forbid
    // that for no reason.
    let camera_keys = [
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::KeyQ,
        KeyCode::KeyE,
        KeyCode::Space,
        KeyCode::ControlLeft,
        KeyCode::KeyC,
        KeyCode::KeyF,
        KeyCode::Escape,
    ];
    let control_keys = [
        KeyCode::KeyP,
        KeyCode::Period,
        KeyCode::BracketRight,
        KeyCode::BracketLeft,
        KeyCode::Backslash,
        KeyCode::Minus,
        KeyCode::Equal,
    ];

    for control in control_keys {
        assert!(
            !camera_keys.contains(&control),
            "{control:?} is bound by both the camera and the clock controls"
        );
    }
}

#[test]
fn the_controls_plugin_registers_its_system() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(SimulationClock::new(1000.0));
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_plugins(SimulationControlsPlugin);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyP);
    app.update();

    assert!(
        app.world().resource::<SimulationClock>().is_paused(),
        "adding the plugin must be enough to make the keys work"
    );
}

#[test]
fn minus_divides_the_scale_by_ten() {
    // A whole decade in one press. `[` halves, which takes nine presses to cross the gap between
    // the solar system's `1e6` and the Jovian system's `1e4` — and those decades are how the
    // scales in this crate actually differ.
    let mut app = app_with_controls(1.0e6);

    press(&mut app, KeyCode::Minus);
    assert_eq!(clock(&app).scale(), 1.0e5);

    press(&mut app, KeyCode::Minus);
    assert_eq!(clock(&app).scale(), 1.0e4);
}

#[test]
fn minus_cannot_take_the_scale_below_the_usable_floor() {
    let mut app = app_with_controls(1000.0);

    for _ in 0..20 {
        press(&mut app, KeyCode::Minus);
    }

    let slow = clock(&app);
    assert_eq!(slow.scale(), crate::time::constants::MIN_TIME_SCALE);
    assert!(
        slow.is_valid(),
        "no number of presses may leave an invalid clock"
    );
}

#[test]
fn the_decade_key_and_the_halving_key_are_independent() {
    // Both act on the same scale and neither resets the other, so they compose — which is what
    // makes `-` a coarse control rather than a mode.
    let mut app = app_with_controls(1.0e6);

    press(&mut app, KeyCode::Minus);
    press(&mut app, KeyCode::BracketLeft);
    assert_eq!(clock(&app).scale(), 5.0e4);

    press(&mut app, KeyCode::BracketRight);
    assert_eq!(clock(&app).scale(), 1.0e5);
}

#[test]
fn the_decade_key_never_advances_time() {
    let mut app = app_with_controls(1.0e6);

    press(&mut app, KeyCode::Minus);

    assert_eq!(clock(&app).elapsed_seconds(), 0.0);
}

#[test]
fn plus_multiplies_the_scale_by_ten() {
    // The mirror of `minus_divides_the_scale_by_ten`. Bound to `Equal`, which is the physical key
    // that carries `+`: Bevy's `KeyCode` is a position, so there is no `KeyCode::Plus`.
    let mut app = app_with_controls(1.0e4);

    press(&mut app, KeyCode::Equal);
    assert_eq!(clock(&app).scale(), 1.0e5);

    press(&mut app, KeyCode::Equal);
    assert_eq!(clock(&app).scale(), 1.0e6);
}

#[test]
fn the_numeric_keypads_plus_does_the_same_thing() {
    // A keypad `+` is unambiguously this action, and a user who reaches for it should not have to
    // discover that only the top row counts.
    let mut app = app_with_controls(1.0e4);

    press(&mut app, KeyCode::NumpadAdd);
    assert_eq!(clock(&app).scale(), 1.0e5);
}

#[test]
fn plus_and_minus_are_exact_inverses() {
    // The one property that makes a decade key worth having: you can speed up to watch something,
    // then come back to precisely the pace you were at. Exact equality rather than a tolerance,
    // because `x * 10 / 10` is exact in binary floating point for these values — if it ever is not,
    // the two directions have stopped sharing `TIME_SCALE_DECADE_FACTOR` and that is the bug.
    let mut app = app_with_controls(1.0e6);

    press(&mut app, KeyCode::Equal);
    press(&mut app, KeyCode::Equal);
    press(&mut app, KeyCode::Minus);
    press(&mut app, KeyCode::Minus);

    assert_eq!(clock(&app).scale(), 1.0e6);
}

#[test]
fn plus_cannot_take_the_scale_above_the_usable_ceiling() {
    // The counterpart to `minus_cannot_take_the_scale_below_the_usable_floor`, and the reason the
    // round trip above is only exact away from a limit: `+` at the ceiling does nothing, so the
    // matching `-` lands a decade below where it started. `\` is the exact way back.
    let mut app = app_with_controls(1000.0);

    for _ in 0..40 {
        press(&mut app, KeyCode::Equal);
    }

    let fast = clock(&app);
    assert_eq!(fast.scale(), crate::time::constants::MAX_TIME_SCALE);
    assert!(
        fast.is_valid(),
        "no number of presses may leave an invalid clock"
    );

    press(&mut app, KeyCode::Minus);
    assert!(
        clock(&app).scale() < crate::time::constants::MAX_TIME_SCALE,
        "the trip back down is lossy at the ceiling, which is what `\\` is for"
    );
}

#[test]
fn plus_never_advances_time() {
    let mut app = app_with_controls(1.0e6);

    press(&mut app, KeyCode::Equal);

    assert_eq!(clock(&app).elapsed_seconds(), 0.0);
}
