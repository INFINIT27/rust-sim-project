//! Showing and hiding the HUD as a whole.
//!
//! The HUD is everything drawn over the view to read it by: the debug panel and the mission
//! readout ([`super::overlay`]), the crosshair ([`super::crosshair`]), the compass
//! ([`super::compass`]), and the `B` picker ([`super::placement`]). [`Action::ToggleHud`] (`H`)
//! hides all of it at once, for a clean view or a screenshot, and shows it again.
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
