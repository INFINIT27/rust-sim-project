//! The rocket's keys: `R` launches one, `G` switches its engine between powered and coasting.
//!
//! This layer only records the requests. [`MissionPlugin`](crate::render::mission::MissionPlugin)
//! in the render layer plans the transfer and flies it, the same split the object placer uses.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use crate::bindings::{keymap_or_default, Action, KeyMap};
use crate::input::menu::MenuState;

/// Whether a launch, or a switch of engine, has been asked for and not yet acted on.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchRequest {
    requested: bool,
    engine_switch: bool,
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

    /// Ask for the engine to switch between powered and coasting.
    #[inline]
    pub fn request_engine_switch(&mut self) {
        self.engine_switch = true;
    }

    /// Whether an engine switch is waiting, clearing the request. Taken like [`Self::take`].
    #[inline]
    pub fn take_engine_switch(&mut self) -> bool {
        std::mem::take(&mut self.engine_switch)
    }
}

/// Bevy system: `R` requests a launch and `G` an engine switch. Inert while the `Esc` menu is open,
/// like every other key.
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
    if keymap_or_default(&keymap).just_pressed(&keys, Action::NextCraft) {
        request.request_engine_switch();
    }
}

/// Plugin that inserts [`LaunchRequest`] and binds `R` and `G`.
pub struct LaunchInputPlugin;

impl Plugin for LaunchInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LaunchRequest>();
        app.init_resource::<MenuState>();
        app.add_systems(Update, launch_input_system);
    }
}
