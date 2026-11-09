//! Keyboard control of the **simulation clock**: pause, single-step, and speed.
//!
//! The camera in [`super::camera`] moves you around the experiment; this module
//! changes how fast the experiment runs, or stops it. Both are input, and they read
//! different clocks — which is the point:
//! [`SimulationClock`](crate::time::simulation::SimulationClock) is what these keys
//! touch, and pausing it leaves [`ObserverClock`](crate::time::observer::ObserverClock)
//! running, so a frozen experiment can still be flown around and inspected.
//!
//! # Bindings
//!
//! | Key | Action |
//! | --- | --- |
//! | `P` | Pause / resume simulated time |
//! | `.` | Advance exactly one frame while paused |
//! | `]` | Faster: multiply the clock scale by [`TIME_SCALE_KEY_FACTOR`] |
//! | `-` | Ten times slower: divide the clock scale by [`TIME_SCALE_DECADE_FACTOR`] |
//! | `[` | Slower: divide it by the same factor |
//! | `\` | Reset the scale to the shipped [`TWO_BODY_TIME_SCALE`] |
//!
//! None of these collide with the camera, which owns `WASD`, `Space`, `Left Ctrl`,
//! `Left Shift`, `Esc`, and the mouse. `P` rather than `Space` for pause precisely
//! because `Space` is already "move up" — the conventional pause key would have
//! silently fought the camera.
//!
//! # Why these are edge-triggered
//!
//! Every binding uses `just_pressed`, not `pressed`. A held `]` must not multiply the
//! scale sixty times a second, and a held `.` must not run the simulation while
//! "paused" — that would make the pause state a lie. Single-stepping is a boolean
//! request on the clock rather than a counter for the same reason; see
//! [`SimulationClock::request_single_step`](crate::time::simulation::SimulationClock::request_single_step).
//!
//! [`TWO_BODY_TIME_SCALE`]: crate::constants::TWO_BODY_TIME_SCALE

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut};
use bevy::input::{keyboard::KeyCode, ButtonInput};

use crate::constants::TWO_BODY_TIME_SCALE;
use crate::input::constants::{TIME_SCALE_DECADE_FACTOR, TIME_SCALE_KEY_FACTOR};
use crate::input::menu::MenuState;
use crate::time::simulation::SimulationClock;

/// Plugin that registers [`simulation_controls_system`] on `Update`.
///
/// Add it after [`ScenePlugin`](crate::render::plugin::ScenePlugin), which is what
/// inserts the [`SimulationClock`] this system mutates. The system is otherwise
/// order-independent: it changes the clock's *configuration*, and the next tick's
/// [`advance_clocks_system`](crate::render::plugin::advance_clocks_system) reads the
/// new value — so a keypress takes effect on the following frame, which is
/// indistinguishable from immediate at 60 Hz.
pub struct SimulationControlsPlugin;

impl Plugin for SimulationControlsPlugin {
    fn build(&self, app: &mut App) {
        // See `FreeFlyCameraPlugin`: the gate's resource, so this plugin runs standalone.
        app.init_resource::<MenuState>();
        app.add_systems(Update, simulation_controls_system);
    }
}

/// Bevy system that maps the control keys onto the simulation clock.
///
/// Reads only `ButtonInput<KeyCode>` and mutates only the [`SimulationClock`]. It
/// touches neither the `Scene` nor any `Transform`, so it cannot perturb the
/// experiment it is controlling — the only thing it can change is *when* the
/// experiment advances and how fast.
///
/// # Ordering with the camera
///
/// Both this and `free_fly_camera_system` run on `Update` with no edge between them.
/// They share no state: this one writes the clock and reads the keyboard, the camera
/// writes a `Transform` and reads the keyboard and the observer clock. Bevy may run
/// them in either order or in parallel, and the result is the same.
pub fn simulation_controls_system(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<MenuState>>,
    mut clock: ResMut<SimulationClock>,
) {
    // While the menu is open, only `Esc` is bound (amendment A13). Returning early rather
    // than filtering key by key keeps that one rule true of the whole layer: a menu in
    // which `[` still rescaled the clock would change the run you opened it to inspect.
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }

    if keys.just_pressed(KeyCode::KeyP) {
        clock.toggle_paused();
    }

    if keys.just_pressed(KeyCode::Period) {
        // Queues one frame. Has no effect unless the clock is paused, and does not
        // accumulate — see the clock's docs for why that is deliberate.
        clock.request_single_step();
    }

    if keys.just_pressed(KeyCode::BracketRight) {
        clock.multiply_scale(TIME_SCALE_KEY_FACTOR);
    }

    if keys.just_pressed(KeyCode::Minus) {
        // A whole order of magnitude in one press. `[` halves, which is the right granularity for
        // finding a pace and nine presses too slow for changing regime — the scales in this crate
        // differ by decades (`1e4` for the Jovian system, `1e6` for the solar system, `2e7` for the
        // ring), so a decade key is the one that matches them.
        clock.multiply_scale(1.0 / TIME_SCALE_DECADE_FACTOR);
    }

    // `Equal` is the **physical** key that carries `+` on a US layout — Bevy's `KeyCode` is a
    // position, not a character, so there is no `KeyCode::Plus` to bind and `Equal` is what a user
    // pressing `+` presses. It fires with or without `Shift`, which is a small kindness rather than a
    // laxity: `=` and `+` are the same key and nobody means a different thing by them.
    //
    // `NumpadAdd` as well, because a numeric keypad's `+` is unambiguously this action and a user who
    // reaches for it is not going to guess that only the top row counts. `-` predates this and is
    // bound only to `Minus`; that asymmetry is a gap, not a decision.
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        clock.multiply_scale(TIME_SCALE_DECADE_FACTOR);
    }

    if keys.just_pressed(KeyCode::BracketLeft) {
        clock.multiply_scale(1.0 / TIME_SCALE_KEY_FACTOR);
    }

    if keys.just_pressed(KeyCode::Backslash) {
        clock.set_scale(TWO_BODY_TIME_SCALE);
    }

    debug_assert!(
        clock.is_valid(),
        "simulation_controls_system: clock left its Validation_Rules: {:?}",
        *clock
    );
}

#[cfg(test)]
#[path = "../../tst/input/controls_tests.rs"]
mod tests;
