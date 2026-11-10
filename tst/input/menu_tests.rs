//! Unit tests for the [`crate::input::menu`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/input/menu.rs` via `#[path]`,
//! not as a standalone integration test.
//!
//! These drive the real systems through a Bevy `App`, pressing keys by writing into
//! `ButtonInput<KeyCode>`, because the thing worth testing is the **binding and the gate** —
//! that `Esc` opens the menu, and that while it is open the camera and the clock stop
//! listening. A test that called `menu.open()` directly would assert nothing about either.
//!
//! `ButtonInput::clear` is called between frames the way Bevy's own input plugin does, so
//! `just_pressed` stays edge-triggered across our frames; without it one simulated press
//! would fire on every tick.

use super::*;
use bevy::app::{App, Update};
use bevy::MinimalPlugins;

/// An `App` with the menu's state and its `Esc` binding.
fn app_with_menu() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<MenuState>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_systems(Update, menu_toggle_system);
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

fn is_open(app: &App) -> bool {
    app.world().resource::<MenuState>().is_open()
}

// -----------------------------------------------------------------------
// The state itself.
// -----------------------------------------------------------------------

#[test]
fn the_app_starts_with_the_menu_closed() {
    // Launching into a menu would mean the experiment is hidden behind a wall of text
    // before the viewer has seen it run.
    assert!(!MenuState::default().is_open());
}

#[test]
fn open_and_close_are_idempotent() {
    // Both are called from more than one place — `close` from `Esc` and from the panel's
    // button — so neither may depend on the current state to be correct.
    let mut menu = MenuState::default();

    menu.open();
    menu.open();
    assert!(menu.is_open());

    menu.close();
    menu.close();
    assert!(!menu.is_open());
}

// -----------------------------------------------------------------------
// The `Esc` binding.
// -----------------------------------------------------------------------

#[test]
fn escape_opens_and_closes_the_menu() {
    let mut app = app_with_menu();
    assert!(!is_open(&app));

    press(&mut app, KeyCode::Escape);
    assert!(is_open(&app));

    press(&mut app, KeyCode::Escape);
    assert!(!is_open(&app));
}

#[test]
fn holding_escape_does_not_flutter_the_menu() {
    // Edge-triggered: a held key must open the menu once, not sixty times a second, which
    // would leave its state a coin toss decided by the frame count.
    let mut app = app_with_menu();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);

    for _ in 0..9 {
        app.update();
    }

    assert!(is_open(&app), "one press, one open");
}

#[test]
fn no_other_key_opens_the_menu() {
    let mut app = app_with_menu();

    for key in [
        KeyCode::KeyW,
        KeyCode::KeyP,
        KeyCode::KeyT,
        KeyCode::Space,
        KeyCode::Period,
        KeyCode::BracketLeft,
    ] {
        press(&mut app, key);
        assert!(!is_open(&app), "{key:?} must not open the menu");
    }
}

// -----------------------------------------------------------------------
// The gate: while the menu is open, only `Esc` is bound.
// -----------------------------------------------------------------------

/// An `App` with the menu, the camera, and the clock controls — the whole input layer.
fn app_with_input() -> App {
    use crate::input::camera::{free_fly_camera_system, FreeFlyController};
    use crate::input::controls::simulation_controls_system;
    use crate::time::observer::ObserverClock;
    use crate::time::simulation::SimulationClock;
    use bevy::core_pipeline::core_3d::Camera3d;
    use bevy::transform::components::Transform;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<MenuState>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<bevy::input::mouse::MouseButton>>();
    app.add_event::<bevy::input::mouse::MouseMotion>();
    app.insert_resource(SimulationClock::new(1000.0));

    // A clock that has actually ticked. Nothing advances the observer clock in an app this
    // small — `advance_clocks_system` lives in `ScenePlugin` — and a clock reading zero
    // makes `motion_step_seconds()` zero, so the camera would hold still whatever the gate
    // did and every movement assertion below would pass for the wrong reason.
    let mut clock = ObserverClock::new();
    clock.advance(1.0);
    app.insert_resource(clock);
    app.add_systems(
        Update,
        (
            menu_toggle_system,
            free_fly_camera_system,
            simulation_controls_system,
        )
            .chain(),
    );
    app.world_mut().spawn((
        Camera3d::default(),
        Transform::default(),
        FreeFlyController::default(),
    ));
    app
}

#[test]
fn the_camera_does_not_move_while_the_menu_is_open() {
    use bevy::core_pipeline::core_3d::Camera3d;
    use bevy::ecs::query::With;
    use bevy::math::Vec3;
    use bevy::transform::components::Transform;

    let mut app = app_with_input();

    // Open the menu, then hold `W` for several frames.
    press(&mut app, KeyCode::Escape);
    assert!(is_open(&app));

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    for _ in 0..10 {
        app.update();
    }

    let moved = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>()
        .single(app.world())
        .translation;

    assert_eq!(
        moved,
        Vec3::ZERO,
        "the camera must stand still behind a menu"
    );
}

#[test]
fn the_camera_moves_again_once_the_menu_closes() {
    use bevy::core_pipeline::core_3d::Camera3d;
    use bevy::ecs::query::With;
    use bevy::math::Vec3;
    use bevy::transform::components::Transform;

    // The gate has to be a gate, not a switch that stays off: the same input that did
    // nothing while the menu was open must work again the moment it closes.
    let mut app = app_with_input();

    press(&mut app, KeyCode::Escape);
    press(&mut app, KeyCode::Escape);
    assert!(!is_open(&app));

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    for _ in 0..10 {
        app.update();
    }

    let moved = app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>()
        .single(app.world())
        .translation;

    assert_ne!(
        moved,
        Vec3::ZERO,
        "closing the menu must give the keys back"
    );
}

#[test]
fn the_clock_keys_do_nothing_while_the_menu_is_open() {
    use crate::time::simulation::SimulationClock;

    let mut app = app_with_input();
    let before = *app.world().resource::<SimulationClock>();

    press(&mut app, KeyCode::Escape);
    for key in [
        KeyCode::KeyP,
        KeyCode::Period,
        KeyCode::BracketRight,
        KeyCode::BracketLeft,
        KeyCode::Backslash,
    ] {
        press(&mut app, key);
    }

    let after = *app.world().resource::<SimulationClock>();
    assert_eq!(
        after.scale(),
        before.scale(),
        "`]` must not rescale the run you opened the menu to inspect"
    );
    assert_eq!(after.is_paused(), before.is_paused(), "`P` must be inert");
    assert!(!after.single_step_pending(), "`.` must be inert");
}

#[test]
fn the_trail_toggle_does_nothing_while_the_menu_is_open() {
    use crate::core::scene::Scene;
    use crate::render::trail::{toggle_trails_system, TrailConfig};

    // `T` is gated in the render layer rather than here, so it gets its own check: the menu
    // *shows* the trail setting, and a key that changed it underneath the reading would be
    // worse than a key that does nothing.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<MenuState>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TrailConfig>();
    app.insert_resource(Scene::demo());
    app.add_systems(Update, (menu_toggle_system, toggle_trails_system).chain());

    let visible_before = app.world().resource::<TrailConfig>().visible;

    press(&mut app, KeyCode::Escape);
    press(&mut app, KeyCode::KeyT);

    assert_eq!(
        app.world().resource::<TrailConfig>().visible,
        visible_before,
        "`T` must be inert while the menu is open"
    );

    // And working again afterwards.
    press(&mut app, KeyCode::Escape);
    press(&mut app, KeyCode::KeyT);
    assert_ne!(
        app.world().resource::<TrailConfig>().visible,
        visible_before
    );
}

#[test]
fn mouse_motion_is_still_drained_while_the_menu_is_open() {
    use bevy::input::mouse::MouseMotion;
    use bevy::math::Vec2;

    // The one thing the gate must *not* skip. Buffered deltas would all apply on the frame
    // after the menu closed, snapping the view — the same failure the no-camera path
    // already guards against, and the reason the gate drains before returning.
    let mut app = app_with_input();
    press(&mut app, KeyCode::Escape);

    for _ in 0..5 {
        app.world_mut().send_event(MouseMotion {
            delta: Vec2::new(50.0, 50.0),
        });
        app.update();
    }

    // Close the menu, capture the cursor, and run a frame with no new motion. If the
    // events had piled up, this frame would rotate the camera.
    press(&mut app, KeyCode::Escape);
    {
        use crate::input::camera::FreeFlyController;
        let mut query = app.world_mut().query::<&mut FreeFlyController>();
        for mut controller in query.iter_mut(app.world_mut()) {
            controller.captured = true;
        }
    }
    app.update();

    let yaw = {
        use crate::input::camera::FreeFlyController;
        app.world_mut()
            .query::<&FreeFlyController>()
            .single(app.world())
            .yaw
    };

    assert_eq!(yaw, 0.0, "stale mouse deltas must not survive the menu");
}

#[test]
fn opening_the_menu_releases_the_cursor() {
    use crate::input::camera::FreeFlyController;

    // `Esc` released the cursor before the menu existed and must still do it, or the menu
    // is something you cannot point at. The window itself is absent under `MinimalPlugins`,
    // so what is checked here is the controller's own flag — the part that is testable
    // headlessly.
    let mut app = app_with_input();

    {
        let mut query = app.world_mut().query::<&mut FreeFlyController>();
        for mut controller in query.iter_mut(app.world_mut()) {
            controller.captured = true;
        }
    }

    press(&mut app, KeyCode::Escape);

    let captured = app
        .world_mut()
        .query::<&FreeFlyController>()
        .single(app.world())
        .captured;

    assert!(!captured, "opening the menu must release the cursor");
}

#[test]
fn closing_the_menu_does_not_recapture_the_cursor() {
    use crate::input::camera::FreeFlyController;

    // You get the cursor back by clicking, as you did the first time. Grabbing it back
    // automatically would hide the pointer somewhere the user did not ask it to go.
    let mut app = app_with_input();

    press(&mut app, KeyCode::Escape);
    press(&mut app, KeyCode::Escape);

    let captured = app
        .world_mut()
        .query::<&FreeFlyController>()
        .single(app.world())
        .captured;

    assert!(!captured);
}

#[test]
fn the_plugin_registers_the_binding() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_plugins(MenuInputPlugin);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();

    assert!(
        is_open(&app),
        "adding the plugin must be enough to make Esc work"
    );
}

#[test]
fn the_gated_plugins_each_insert_the_state_themselves() {
    use crate::input::camera::FreeFlyCameraPlugin;
    use crate::input::controls::SimulationControlsPlugin;
    use crate::time::simulation::SimulationClock;

    // Each gated system takes `Res<MenuState>`, which panics if nothing inserted it. So
    // every plugin that owns one inserts it — otherwise adding the camera without the menu
    // would crash on the first frame, which is exactly how most of the render tests are
    // built.
    let mut camera_only = App::new();
    camera_only.add_plugins(MinimalPlugins);
    camera_only.add_plugins(FreeFlyCameraPlugin);
    assert!(camera_only.world().get_resource::<MenuState>().is_some());

    let mut controls_only = App::new();
    controls_only.add_plugins(MinimalPlugins);
    controls_only.insert_resource(SimulationClock::new(1.0));
    controls_only.add_plugins(SimulationControlsPlugin);
    assert!(controls_only.world().get_resource::<MenuState>().is_some());
}

#[test]
fn escape_closes_an_open_popup_before_it_opens_the_menu() {
    let mut app = app_with_menu();
    app.init_resource::<PopupState>();
    app.world_mut().resource_mut::<PopupState>().open();

    press(&mut app, KeyCode::Escape);
    assert!(
        !app.world().resource::<PopupState>().is_open(),
        "Esc must close the pop-up in front"
    );
    assert!(!is_open(&app), "and must not open the menu behind it");

    press(&mut app, KeyCode::Escape);
    assert!(is_open(&app), "the next Esc opens the menu as usual");
}

#[test]
fn a_click_behind_an_open_popup_does_not_capture_the_cursor() {
    use crate::input::camera::FreeFlyController;
    use bevy::input::mouse::MouseButton;

    // The click that presses a pop-up's ✕ must not hand the cursor to mouse-look.
    let mut app = app_with_input();
    app.init_resource::<PopupState>();
    app.world_mut().resource_mut::<PopupState>().open();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    let captured = app
        .world_mut()
        .query::<&FreeFlyController>()
        .single(app.world())
        .captured;
    assert!(!captured, "an open pop-up keeps the cursor free");
}

#[test]
fn the_menu_always_opens_on_its_first_page() {
    let mut app = app_with_menu();
    press(&mut app, KeyCode::Escape);
    app.world_mut()
        .resource_mut::<MenuState>()
        .show(MenuPage::Settings(SettingsTab::Video));
    press(&mut app, KeyCode::Escape);
    assert!(!is_open(&app));
    press(&mut app, KeyCode::Escape);
    assert_eq!(
        app.world().resource::<MenuState>().page(),
        MenuPage::Main,
        "Esc brings out the first page, not the last one shown"
    );
}

/// An app with the menu open, waiting to rebind `action`.
fn app_rebinding(action: Action) -> App {
    let mut app = app_with_menu();
    app.init_resource::<KeyMap>();
    press(&mut app, KeyCode::Escape);
    app.world_mut()
        .resource_mut::<MenuState>()
        .start_rebinding(action);
    app
}

#[test]
fn the_next_key_pressed_while_rebinding_becomes_the_key() {
    let mut app = app_rebinding(Action::Pause);
    press(&mut app, KeyCode::KeyK);
    assert_eq!(
        app.world().resource::<KeyMap>().key(Action::Pause),
        KeyCode::KeyK
    );
    let menu = app.world().resource::<MenuState>();
    assert_eq!(menu.rebinding(), None, "one key, then the rebind is done");
    assert!(menu.is_open(), "and the menu stays open");
}

#[test]
fn escape_while_rebinding_cancels_it_and_keeps_the_menu_open() {
    let mut app = app_rebinding(Action::Pause);
    press(&mut app, KeyCode::Escape);
    assert_eq!(
        app.world().resource::<KeyMap>().key(Action::Pause),
        KeyCode::KeyP
    );
    let menu = app.world().resource::<MenuState>();
    assert_eq!(menu.rebinding(), None);
    assert!(menu.is_open());
}

#[test]
fn a_key_that_cannot_be_bound_is_ignored_while_rebinding() {
    let mut app = app_rebinding(Action::Pause);
    press(&mut app, KeyCode::SuperLeft);
    assert_eq!(
        app.world().resource::<MenuState>().rebinding(),
        Some(Action::Pause),
        "still waiting"
    );
    assert_eq!(
        app.world().resource::<KeyMap>().key(Action::Pause),
        KeyCode::KeyP
    );
}
