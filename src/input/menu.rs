//! The menu's **state** and the `Esc` key that toggles it.
//!
//! Two halves, in two layers. This one owns the fact *that* the menu is open and the key
//! that changes it; [`crate::ui::menu`] owns what it looks like. The split is not
//! bureaucratic — [`MenuState`] is read by three systems in two other layers to decide
//! whether to accept input at all, and none of them should have to depend on `bevy_egui`
//! to ask "is the menu open?".
//!
//! # Why the state lives in `input`
//!
//! Because what it really encodes is an **input mode**: while the menu is open, the app is
//! not accepting the flying-and-observing bindings. That is the same kind of fact as
//! cursor capture, which this layer already owns. Putting it in `ui` would mean
//! `input/camera.rs`, `input/controls.rs`, and `render/trail.rs` all importing the UI
//! layer to find out whether to run — inverting the dependency direction that already
//! exists (`ui` and `render` read `input`, never the reverse).
//!
//! # One rule while the menu is open: only `Esc` is bound
//!
//! Every other binding is inert — camera movement, mouse-look, the clock keys, the trail
//! toggle. One rule is worth more than a per-key judgement here: a menu where `W` still
//! flies and `[` still changes the clock is a menu you cannot read, and the alternative
//! (deciding case by case which keys "should" still work) produces a set nobody can
//! remember. The gate is an early return in each of the three systems, documented at the
//! point of the check.
//!
//! Two deliberate exceptions to "inert":
//!
//! - **The cursor is released** when the menu opens, because a menu you cannot point at is
//!   not a menu. That is done by `free_fly_camera_system`, which already owns the window's
//!   cursor state — this module never touches the window.
//! - **`MouseMotion` is still drained** every frame while the menu is open. It has to be:
//!   the events would otherwise pile up and snap the view the moment the menu closed. This
//!   is the one thing the gate must not skip, and it has a test.
//!
//! # The simulation keeps running
//!
//! Opening the menu does **not** pause the experiment. `P` pauses; a menu that also paused
//! would make "what was the state of the run when I opened this?" unanswerable, and it
//! would silently change what you are observing. The clock is a property of the
//! experiment, not of the UI.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::input::{keyboard::KeyCode, ButtonInput};

/// Whether the menu is open.
///
/// The field is private and changed only through [`open`](Self::open),
/// [`close`](Self::close), and [`toggle`](Self::toggle), so "who opened the menu" is
/// always a named call rather than an assignment — the same discipline
/// [`SimulationClock`](crate::time::simulation::SimulationClock) uses for its scale.
///
/// # Examples
///
/// ```
/// use physics_sandbox::MenuState;
///
/// // Closed by default: the app opens into the experiment, not into a menu.
/// let mut menu = MenuState::default();
/// assert!(!menu.is_open());
///
/// menu.toggle();
/// assert!(menu.is_open());
///
/// menu.close();
/// assert!(!menu.is_open());
/// ```
#[derive(Resource, Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct MenuState {
    open: bool,
}

impl MenuState {
    /// Is the menu open?
    ///
    /// The question every gated system asks. `true` means **only `Esc` is bound** — see
    /// the module docs.
    #[inline]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the menu. Idempotent.
    #[inline]
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Close the menu. Idempotent.
    ///
    /// Does **not** re-capture the cursor: you get it back by clicking, the same way you
    /// captured it the first time. Re-capturing automatically would hide the cursor
    /// somewhere the user did not ask for it to go.
    #[inline]
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Flip the menu open or closed.
    #[inline]
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

/// Bevy system that opens and closes the menu on `Esc`.
///
/// Edge-triggered (`just_pressed`), like every other binding in the crate: a held `Esc`
/// must not flicker the menu sixty times a second, which would leave its state a coin toss
/// decided by the frame count.
///
/// # What happened to "`Esc` releases the cursor"
///
/// It still does, as a *consequence*. Before the menu existed, `free_fly_camera_system`
/// watched `Esc` itself and dropped the cursor grab. Now `Esc` opens the menu and the
/// camera system releases the cursor because the menu is open — so the key does what it
/// always did, plus shows you something. Requirement 10.8 is amended by A13 accordingly.
///
/// Ordering matters, and the plugin arranges it: this runs **before**
/// [`free_fly_camera_system`](super::camera::free_fly_camera_system), so opening the menu
/// takes effect on the same frame as the keypress rather than the next one. Registered the
/// other way round, one frame of movement would leak through after `Esc`.
pub fn menu_toggle_system(keys: Res<ButtonInput<KeyCode>>, mut menu: ResMut<MenuState>) {
    if keys.just_pressed(KeyCode::Escape) {
        menu.toggle();
    }
}

/// Plugin that inserts [`MenuState`] and registers the `Esc` binding.
///
/// Drawing the menu is [`MenuPanelPlugin`](crate::ui::menu::MenuPanelPlugin)'s job, in the
/// `ui` layer. This plugin is useful without it: `MenuState` alone makes every gated
/// system stop accepting input, which is exactly what a test wants.
///
/// The plugins whose systems are *gated* by `MenuState` each `init_resource` it themselves,
/// so adding `FreeFlyCameraPlugin` without this one still compiles and runs — the menu
/// simply never opens, because nothing is watching `Esc`.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use physics_sandbox::{MenuInputPlugin, MenuPanelPlugin, ScenePlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(ScenePlugin::default())
///     .add_plugins(MenuInputPlugin)
///     .add_plugins(MenuPanelPlugin)
///     .run();
/// ```
pub struct MenuInputPlugin;

impl Plugin for MenuInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuState>();
        app.add_systems(
            Update,
            menu_toggle_system.before(super::camera::free_fly_camera_system),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/input/menu_tests.rs"]
mod tests;
