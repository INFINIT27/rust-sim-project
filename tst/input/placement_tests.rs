//! Unit tests for the [`crate::input::placement`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/input/placement.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! Driven through a real `App` for the same reason `menu_tests` is: what is worth testing is
//! the **binding** — that `B` opens the picker and `Down` moves the selection — and a test
//! calling `select_next()` directly would assert nothing about the keys.

use super::*;
use bevy::app::{App, Update};
use bevy::MinimalPlugins;

fn app_with_placer() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<PlacementState>();
    app.init_resource::<MenuState>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_systems(Update, placement_input_system);
    app
}

/// Press `key` for exactly one frame, the way the real input plugin would.
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
}

fn placer(app: &App) -> PlacementState {
    *app.world().resource::<PlacementState>()
}

// -----------------------------------------------------------------------
// The state.
// -----------------------------------------------------------------------

#[test]
fn the_picker_starts_closed_on_the_first_object() {
    let state = PlacementState::default();

    assert!(!state.is_open());
    assert_eq!(state.selected_index(), 0);
    assert!(!state.place_requested());
}

#[test]
fn the_selection_wraps_in_both_directions() {
    // Wrapping rather than stopping, because the list is five rows: at that length a wrap is
    // instantly legible and neither key is ever dead.
    let mut state = PlacementState::default();
    let last = STELLAR_OBJECTS.len() - 1;

    state.select_previous();
    assert_eq!(state.selected_index(), last);

    state.select_next();
    assert_eq!(state.selected_index(), 0);
}

#[test]
fn the_selection_is_always_a_real_object() {
    // `selected()` indexes the table, so an out-of-range index would panic in the middle of
    // drawing a panel. Walking the whole list twice covers every wrap.
    let mut state = PlacementState::default();

    for _ in 0..(STELLAR_OBJECTS.len() * 2 + 1) {
        assert!(state.selected().is_valid());
        state.select_next();
    }
}

#[test]
fn closing_cancels_a_pending_request() {
    // A request left armed across a close would place an object after the user had visibly
    // dismissed the thing that was going to place it.
    let mut state = PlacementState::default();
    state.open();
    state.request_place();
    assert!(state.place_requested());

    state.close();
    assert!(!state.place_requested());
}

#[test]
fn a_request_needs_an_open_picker() {
    // `Enter` is a key someone might press for any reason; without a visible selection the
    // request has no meaning.
    let mut state = PlacementState::default();
    state.request_place();

    assert!(!state.place_requested());
}

#[test]
fn reopening_keeps_the_previous_selection() {
    // Placing three neutron stars in a row should not mean re-selecting each time.
    let mut state = PlacementState::default();
    state.open();
    state.select_next();
    state.select_next();
    let chosen = state.selected_index();

    state.close();
    state.open();

    assert_eq!(state.selected_index(), chosen);
}

// -----------------------------------------------------------------------
// The bindings.
// -----------------------------------------------------------------------

#[test]
fn b_opens_and_closes_the_picker() {
    let mut app = app_with_placer();
    assert!(!placer(&app).is_open());

    press(&mut app, KeyCode::KeyB);
    assert!(placer(&app).is_open());

    press(&mut app, KeyCode::KeyB);
    assert!(!placer(&app).is_open());
}

#[test]
fn holding_b_does_not_flutter_the_picker() {
    let mut app = app_with_placer();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    for _ in 0..9 {
        app.update();
    }

    assert!(placer(&app).is_open(), "one press, one open");
}

#[test]
fn the_arrow_keys_move_the_selection_only_while_open() {
    let mut app = app_with_placer();

    // Closed: the arrows do nothing, so they stay free for anything else to bind later.
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(placer(&app).selected_index(), 0);

    press(&mut app, KeyCode::KeyB);
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(placer(&app).selected_index(), 1);

    press(&mut app, KeyCode::ArrowUp);
    assert_eq!(placer(&app).selected_index(), 0);
}

#[test]
fn enter_requests_a_placement_only_while_open() {
    let mut app = app_with_placer();

    press(&mut app, KeyCode::Enter);
    assert!(!placer(&app).place_requested());

    press(&mut app, KeyCode::KeyB);
    press(&mut app, KeyCode::Enter);
    assert!(placer(&app).place_requested());
}

#[test]
fn nothing_but_b_opens_the_picker() {
    let mut app = app_with_placer();

    for key in [
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Enter,
        KeyCode::KeyP,
        KeyCode::KeyT,
    ] {
        press(&mut app, key);
        assert!(!placer(&app).is_open(), "{key:?} must not open the picker");
    }
}

// -----------------------------------------------------------------------
// Interaction with the menu.
// -----------------------------------------------------------------------

#[test]
fn the_menu_closes_the_picker_and_makes_b_inert() {
    // *Only `Esc` is bound* is a rule with no exceptions (amendment A13). The picker is also
    // closed rather than merely ignored: leaving it open underneath a modal menu would mean
    // dismissing the menu revealed a picker the user had forgotten about, with `Enter` armed.
    let mut app = app_with_placer();

    press(&mut app, KeyCode::KeyB);
    assert!(placer(&app).is_open());

    app.world_mut().resource_mut::<MenuState>().open();
    app.update();
    assert!(!placer(&app).is_open(), "the menu closes the picker");

    press(&mut app, KeyCode::KeyB);
    assert!(!placer(&app).is_open(), "B is inert while the menu is open");
}

#[test]
fn the_picker_works_again_once_the_menu_closes() {
    let mut app = app_with_placer();

    app.world_mut().resource_mut::<MenuState>().open();
    app.update();
    app.world_mut().resource_mut::<MenuState>().close();

    press(&mut app, KeyCode::KeyB);
    assert!(placer(&app).is_open());
}

#[test]
fn the_picker_runs_without_a_menu_at_all() {
    // `Option<Res<MenuState>>`: absent state means no menu, which means nothing is blocking.
    // Half the tests in this crate build the smallest app that exercises one system.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<PlacementState>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_systems(Update, placement_input_system);

    press(&mut app, KeyCode::KeyB);
    assert!(placer(&app).is_open());
}

#[test]
fn the_plugin_registers_the_bindings() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_plugins(PlacementInputPlugin);

    press(&mut app, KeyCode::KeyB);
    assert!(placer(&app).is_open());
}

#[test]
fn no_placement_key_collides_with_the_camera_or_the_clock() {
    // The picker is deliberately **not** modal — you have to be able to fly and aim while it
    // is open — so unlike the menu it shares the frame with every other binding. That only
    // works if none of its keys are already taken.
    let camera_and_clock = [
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::KeyQ,
        KeyCode::KeyE,
        KeyCode::Space,
        KeyCode::ControlLeft,
        KeyCode::ShiftLeft,
        KeyCode::Escape,
        KeyCode::KeyP,
        KeyCode::Period,
        KeyCode::BracketRight,
        KeyCode::BracketLeft,
        KeyCode::Backslash,
        KeyCode::KeyT,
    ];

    for key in [
        KeyCode::KeyB,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Enter,
    ] {
        assert!(
            !camera_and_clock.contains(&key),
            "{key:?} is bound twice, and the picker is not modal"
        );
    }
}
