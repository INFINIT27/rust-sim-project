//! The rocket's key: `R` launches one. Which spacecraft it launches, and whether it lands or
//! loops round and returns, are chosen on the `Esc` menu's spacecraft page, not by a key.
//!
//! This layer only records the request. [`MissionPlugin`](crate::render::mission::MissionPlugin)
//! in the render layer plans the transfer and flies it, the same split the object placer uses.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use crate::bindings::{keymap_or_default, Action, KeyMap};
use crate::input::menu::MenuState;

/// Whether a launch has been asked for and not yet acted on.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchRequest {
    requested: bool,
}

impl LaunchRequest {
    /// Ask for a launch.
    #[inline]
    pub fn request(&mut self) {
        self.requested = true;
    }

    /// Whether a launch is waiting, clearing the request.
    ///
    /// Taken by the consumer rather than cleared each frame, so a hitch cannot drop a press.
    #[inline]
    pub fn take(&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }
}

/// Bevy system: `R` requests a launch. Inert while the `Esc` menu is open, like every other key.
pub fn launch_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    mut request: ResMut<LaunchRequest>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::Launch) {
        request.request();
    }
}

/// Plugin that inserts [`LaunchRequest`] and binds `R`.
pub struct LaunchInputPlugin;

impl Plugin for LaunchInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LaunchRequest>();
        app.init_resource::<MenuState>();
        app.add_systems(Update, launch_input_system);
    }
}
