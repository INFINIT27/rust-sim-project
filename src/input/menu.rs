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

use crate::bindings::{is_bindable, Action, KeyMap};
use crate::input::popup::PopupState;

/// Which page of the menu is showing.
///
/// The menu always opens on [`Main`](Self::Main), the page with the way into everything else.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum MenuPage {
    /// The first page: Settings, Configuration, Exit, and what the run is doing.
    #[default]
    Main,
    /// The controls and video settings.
    Settings(SettingsTab),
    /// The runtime configuration from `config/*.toml`.
    Configuration(ConfigTab),
}

/// The tabs of the Settings page.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum SettingsTab {
    /// Every key binding, and rebinding them.
    #[default]
    Controls,
    /// Anti-aliasing, vsync, and the floor's eclipse shadows.
    Video,
}

/// The tabs of the Configuration page, one per configuration file the menu edits.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ConfigTab {
    /// `camera.toml`.
    #[default]
    Camera,
    /// `ui.toml`: the crosshair, the compass, and the arrival pop-up.
    Interface,
    /// `trails.toml`.
    Trails,
    /// `spacecraft.toml`.
    Spacecraft,
}

/// Whether the menu is open, which page it shows, and whether it is waiting for a key to rebind.
///
/// The fields are private and changed only through named calls, so "who opened the menu" is
/// always a call rather than an assignment — the same discipline
/// [`SimulationClock`](crate::time::simulation::SimulationClock) uses for its scale.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{Action, MenuPage, MenuState};
///
/// // Closed by default: the app opens into the experiment, not into a menu.
/// let mut menu = MenuState::default();
/// assert!(!menu.is_open());
///
/// menu.toggle();
/// assert!(menu.is_open());
/// assert_eq!(menu.page(), MenuPage::Main, "the menu always opens on its first page");
///
/// menu.start_rebinding(Action::Pause);
/// assert_eq!(menu.rebinding(), Some(Action::Pause));
///
/// menu.close();
/// assert!(!menu.is_open());
/// assert_eq!(menu.rebinding(), None, "closing abandons a rebind");
/// ```
#[derive(Resource, Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct MenuState {
    open: bool,
    page: MenuPage,
    rebinding: Option<Action>,
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

    /// Open the menu on its first page. Idempotent only in that it is open afterwards: opening
    /// always goes back to [`MenuPage::Main`].
    #[inline]
    pub fn open(&mut self) {
        self.open = true;
        self.page = MenuPage::Main;
        self.rebinding = None;
    }

    /// Close the menu, abandoning any rebind. Idempotent.
    ///
    /// Does **not** re-capture the cursor: you get it back by clicking, the same way you
    /// captured it the first time. Re-capturing automatically would hide the cursor
    /// somewhere the user did not ask for it to go.
    #[inline]
    pub fn close(&mut self) {
        self.open = false;
        self.rebinding = None;
    }

    /// Open the menu (on its first page) or close it.
    #[inline]
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// The page showing.
    #[inline]
    pub fn page(&self) -> MenuPage {
        self.page
    }

    /// Show `page`, abandoning any rebind.
    #[inline]
    pub fn show(&mut self, page: MenuPage) {
        self.page = page;
        self.rebinding = None;
    }

    /// The action waiting for a key, if a rebind is in progress.
    #[inline]
    pub fn rebinding(&self) -> Option<Action> {
        self.rebinding
    }

    /// Wait for the next key press and bind `action` to it (see [`menu_toggle_system`]).
    #[inline]
    pub fn start_rebinding(&mut self, action: Action) {
        self.rebinding = Some(action);
    }

    /// Stop waiting for a key.
    #[inline]
    pub fn cancel_rebinding(&mut self) {
        self.rebinding = None;
    }
}

/// Bevy system for the menu's keys: `Esc` opens and closes the menu, or closes an open pop-up
/// instead; and while a rebind is waiting, the next key press is the new key.
///
/// While a [`PopupState`] pop-up is open and the menu is closed, `Esc` closes the pop-up and
/// does not open the menu: `Esc` means "get me out of whatever this is", and the pop-up is what
/// is in front. The next `Esc` opens the menu as usual.
///
/// While [`MenuState::rebinding`] is set, the first bindable key pressed
/// ([`crate::bindings::is_bindable`]) is bound to that action through [`KeyMap::set`], which
/// swaps if another action had it. `Esc` cancels the rebind rather than closing the menu, and
/// is never itself bound, so the menu can always be reached.
///
/// Edge-triggered (`just_pressed`), like every other binding in the crate: a held `Esc`
/// must not flicker the menu sixty times a second.
///
/// Ordering matters, and the plugin arranges it: this runs **before**
/// [`free_fly_camera_system`](super::camera::free_fly_camera_system), so opening the menu
/// takes effect on the same frame as the keypress rather than the next one. Registered the
/// other way round, one frame of movement would leak through after `Esc`.
pub fn menu_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuState>,
    popup: Option<ResMut<PopupState>>,
    keymap: Option<ResMut<KeyMap>>,
) {
    if let Some(action) = menu.rebinding() {
        if keys.just_pressed(KeyCode::Escape) {
            menu.cancel_rebinding();
        } else if let Some(key) = keys
            .get_just_pressed()
            .copied()
            .find(|&key| is_bindable(key))
        {
            if let Some(mut keymap) = keymap {
                keymap.set(action, key);
            }
            menu.cancel_rebinding();
        }
        return;
    }
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if let Some(mut popup) = popup {
        if popup.is_open() && !menu.is_open() {
            popup.close();
            return;
        }
    }
    menu.toggle();
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
        app.init_resource::<PopupState>();
        app.init_resource::<KeyMap>();
        app.add_systems(
            Update,
            menu_toggle_system.before(super::camera::free_fly_camera_system),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/input/menu_tests.rs"]
mod tests;
