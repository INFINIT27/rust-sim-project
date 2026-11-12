//! Unit tests for `hud.rs`: the HUD's visibility, its run condition, and its key.

use bevy::app::App;
use bevy::ecs::system::RunSystemOnce;
use bevy::input::ButtonInput;
use bevy::MinimalPlugins;

use super::*;

#[test]
fn the_hud_starts_shown() {
    assert!(HudVisibility::default().visible);
}

#[test]
fn the_run_condition_follows_the_visibility_and_defaults_to_shown() {
    let mut app = App::new();
    assert!(
        app.world_mut().run_system_once(hud_shown),
        "no resource: shown"
    );
    app.insert_resource(HudVisibility { visible: false });
    assert!(!app.world_mut().run_system_once(hud_shown));
    app.insert_resource(HudVisibility { visible: true });
    assert!(app.world_mut().run_system_once(hud_shown));
}

#[test]
fn h_toggles_the_hud() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(HudPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyH);
    app.update();
    assert!(!app.world().resource::<HudVisibility>().visible, "hidden");

    // Held down, it does not flicker: the input system clears `just_pressed` after a frame.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.update();
    assert!(!app.world().resource::<HudVisibility>().visible);

    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(KeyCode::KeyH);
    keys.clear();
    keys.press(KeyCode::KeyH);
    app.update();
    assert!(
        app.world().resource::<HudVisibility>().visible,
        "shown again"
    );
}
