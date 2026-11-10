//! The object placer: `B` to open, `Up`/`Down` to choose, `Enter` to place.
//!
//! This module holds the **state and the keys**; [`crate::ui::placement`] draws the picker
//! and [`crate::render::placement`] does the placing. Three layers for one feature, and the
//! split is the same one the `Esc` menu uses, for the same reasons: the state is input, the
//! panel is UI, and **only `render` may touch the [`Scene`](crate::core::scene::Scene)** —
//! which is what placing an object means.
//!
//! # Why it is not modal, unlike the menu
//!
//! The `Esc` menu makes every other binding inert, because it is something you *read*. The
//! placer is something you **aim**: the object appears in front of the camera, so flying and
//! looking have to keep working while it is open, or you could only ever place things where
//! you already were. `Up`/`Down`/`Enter` collide with nothing the camera uses, so both can
//! be live at once.
//!
//! It is still gated by the menu — while the `Esc` menu is open, `B` does nothing, because
//! *only `Esc` is bound* is a rule with no exceptions (amendment A13).
//!
//! # Why `Enter` rather than a click
//!
//! Left-click already captures the cursor for mouse-look, and it does so *while* the picker
//! is open, since the picker is not modal. Overloading it would mean the first click aims
//! and the second places, which is exactly the kind of state nobody can see. `Enter` is
//! unambiguous and it is next to the arrow keys already doing the selecting.
//!
//! # The request is a flag, not a call
//!
//! `Enter` sets [`PlacementState::place_requested`] and this layer stops there.
//! `render/placement.rs` consumes the flag, works out where the camera is pointing, and adds
//! the prop. That indirection is what keeps this layer's rule — *never reads or writes
//! `Scene`* — intact, and it is one frame, which at 60 Hz is indistinguishable from
//! immediate.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::input::{keyboard::KeyCode, ButtonInput};

use crate::bindings::{keymap_or_default, Action, KeyMap};
use crate::experiments::constants::STELLAR_OBJECTS;
use crate::experiments::objects::StellarObject;
use crate::input::menu::MenuState;

/// Whether the placement picker is open, which object is selected, and whether a placement
/// has been asked for.
///
/// The fields are private and changed only through named methods, like
/// [`crate::input::menu::MenuState`] — so "who moved the selection" is always a
/// call rather than an assignment, and `selected` cannot be set out of range.
///
/// # Examples
///
/// ```
/// use physics_sandbox::PlacementState;
///
/// let mut placer = PlacementState::default();
/// assert!(!placer.is_open());
///
/// placer.toggle();
/// assert!(placer.is_open());
/// assert_eq!(placer.selected().name, "Red dwarf", "the first row, to begin with");
///
/// // The selection wraps rather than stopping, so holding one key cycles the list.
/// placer.select_previous();
/// assert_eq!(placer.selected().name, "Sgr A*");
/// ```
#[derive(Resource, Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct PlacementState {
    open: bool,
    selected: usize,
    place_requested: bool,
}

impl PlacementState {
    /// Is the picker open?
    #[inline]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the picker. Idempotent, and it keeps the previous selection: placing three
    /// neutron stars in a row should not mean re-selecting each time.
    #[inline]
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Close the picker, cancelling any pending request.
    ///
    /// Cancelling matters: a request set on the frame the picker closes would place an
    /// object after the user had visibly dismissed the thing that was going to place it.
    #[inline]
    pub fn close(&mut self) {
        self.open = false;
        self.place_requested = false;
    }

    /// Flip the picker open or closed.
    #[inline]
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Index of the selected row, always in range for
    /// [`crate::experiments::constants::STELLAR_OBJECTS`].
    #[inline]
    pub fn selected_index(&self) -> usize {
        // Defensive: `selected` is only ever moved by the two methods below, both of which
        // wrap, so this can only differ if the table shrank between builds.
        self.selected.min(STELLAR_OBJECTS.len().saturating_sub(1))
    }

    /// The selected object.
    #[inline]
    pub fn selected(&self) -> &'static StellarObject {
        &STELLAR_OBJECTS[self.selected_index()]
    }

    /// Move the selection one row down the list, wrapping at the end.
    ///
    /// Wrapping rather than stopping because the list is five rows: a wrap is instantly
    /// legible at that length, and it means neither key is ever dead.
    #[inline]
    pub fn select_next(&mut self) {
        let count = STELLAR_OBJECTS.len();
        if count > 0 {
            self.selected = (self.selected_index() + 1) % count;
        }
    }

    /// Move the selection one row up the list, wrapping at the start.
    #[inline]
    pub fn select_previous(&mut self) {
        let count = STELLAR_OBJECTS.len();
        if count > 0 {
            self.selected = (self.selected_index() + count - 1) % count;
        }
    }

    /// Has a placement been asked for and not yet carried out?
    #[inline]
    pub fn place_requested(&self) -> bool {
        self.place_requested
    }

    /// Ask for the selected object to be placed.
    ///
    /// Has no effect while the picker is closed — the request is meaningless without a
    /// visible selection, and `Enter` is a key someone might press for any reason.
    #[inline]
    pub fn request_place(&mut self) {
        if self.open {
            self.place_requested = true;
        }
    }

    /// Clear the request, called by whoever carried it out.
    ///
    /// Separate from [`request_place`](Self::request_place) so the consumer — in the render
    /// layer — is the one that decides the request is finished. A request that cleared itself
    /// after a frame could be dropped by a hitch.
    #[inline]
    pub fn clear_request(&mut self) {
        self.place_requested = false;
    }
}

/// Bevy system that drives the picker from the keyboard.
///
/// | Key | Effect |
/// | --- | --- |
/// | `B` | Open or close the picker |
/// | `Up` / `Down` | Move the selection, wrapping |
/// | `Enter` | Place the selected object in front of the camera |
/// | `Esc` | Opens the menu, which closes the picker |
///
/// Every binding is edge-triggered (`just_pressed`), for the reason every binding in this
/// crate is: a held `Down` must not cycle the list sixty times a second.
///
/// # Interaction with the menu
///
/// Inert while the `Esc` menu is open, and the picker is **closed** when the menu opens —
/// two overlapping panels with two different selection models is a UI nobody can read, and
/// `Esc` is the key that means "get me out of whatever this is".
pub fn placement_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    mut placer: ResMut<PlacementState>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        // Not merely ignored: closed. The menu is drawn over the picker, so leaving it open
        // underneath would mean dismissing the menu revealed a picker the user had forgotten
        // about — with `Enter` armed.
        if placer.is_open() {
            placer.close();
        }
        return;
    }

    if keymap_or_default(&keymap).just_pressed(&keys, Action::TogglePicker) {
        placer.toggle();
    }

    if !placer.is_open() {
        return;
    }

    if keymap_or_default(&keymap).just_pressed(&keys, Action::PickerDown) {
        placer.select_next();
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::PickerUp) {
        placer.select_previous();
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::PickerPlace) {
        placer.request_place();
    }
}

/// Plugin that inserts [`PlacementState`] and registers the picker's keys.
///
/// The *placing* is [`PlacementPlugin`](crate::render::placement::PlacementPlugin) in the
/// render layer, and the picker is drawn by
/// [`PlacementPanelPlugin`](crate::ui::placement::PlacementPanelPlugin). This plugin alone
/// gives you a selection nothing acts on, which is what a headless test wants.
pub struct PlacementInputPlugin;

impl Plugin for PlacementInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlacementState>();
        app.init_resource::<MenuState>();
        app.add_systems(Update, placement_input_system);
    }
}

#[cfg(test)]
#[path = "../../tst/input/placement_tests.rs"]
mod tests;
