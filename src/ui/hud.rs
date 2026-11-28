//! Showing and hiding the HUD as a whole.
//!
//! The HUD is everything drawn over the view to read it by: the debug panel and the mission
//! readout ([`super::overlay`]), the crosshair ([`super::crosshair`]), the compass
//! ([`super::compass`]), and the `B` picker ([`super::placement`]). [`Action::ToggleHud`] (`H`)
//! hides all of it at once, for a clean view or a screenshot, and shows it again.
//!
//! The debug panel is also off until asked for: [`DebugPanel`] starts hidden, and the `Esc`
//! menu's main page turns it on for the session. It is drawn only while both are on.
//!
//! Each of those panels' systems runs only while [`hud_shown`]. The `Esc` menu and the arrival
//! pop-up are not HUD: the menu is how settings are changed, and the pop-up holds the cursor until
//! it is dismissed, so hiding either would leave the user stuck.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::input::{keyboard::KeyCode, ButtonInput};

use crate::bindings::{keymap_or_default, Action, KeyMap};
use crate::input::menu::MenuState;

/// Whether the HUD is shown. On at start.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudVisibility {
    /// `true` while the HUD is drawn.
    pub visible: bool,
}

impl Default for HudVisibility {
    fn default() -> Self {
        HudVisibility { visible: true }
    }
}

/// Run condition for the HUD's systems: true while the HUD is shown, and in an app without
/// [`HudPlugin`] (so each panel still works on its own, as the headless tests add them).
pub fn hud_shown(hud: Option<Res<HudVisibility>>) -> bool {
    hud.is_none_or(|hud| hud.visible)
}

/// Whether the debug panel is shown. Off at start; the `Esc` menu's "Show the debug panel" box
/// turns it on for the rest of the session. Never saved: every launch starts with it off.
///
/// Separate from [`HudVisibility`] because the two answer different questions. The panel is a
/// diagnostic most runs do not need on screen, so it is opt-in; `H` still hides it along with the
/// rest of the HUD while it is on.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebugPanel {
    /// `true` while the debug panel is drawn (and the HUD is shown).
    pub visible: bool,
}

/// Run condition for the debug panel: true only while [`DebugPanel::visible`] is set. With no
/// resource it is false, so the panel is off unless something has asked for it.
pub fn debug_panel_shown(panel: Option<Res<DebugPanel>>) -> bool {
    panel.is_some_and(|panel| panel.visible)
}

/// Bevy system that shows or hides the HUD on [`Action::ToggleHud`], except while the menu is open
/// (where only `Esc` is bound).
pub fn toggle_hud_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    mut hud: ResMut<HudVisibility>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::ToggleHud) {
        hud.visible = !hud.visible;
    }
}

/// Adds [`HudVisibility`] and its `H` toggle.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudVisibility>();
        app.add_systems(Update, toggle_hud_system);
    }
}

#[cfg(test)]
#[path = "../../tst/ui/hud_tests.rs"]
mod tests;
