//! Whether a dismissible pop-up is showing, such as the report shown when a spacecraft lands.
//!
//! A pop-up is something you read and then close, so while one is open the camera releases the
//! cursor and stands down, exactly as it does for the `Esc` menu: you can point at the pop-up's
//! close button, and a click on it does not capture the cursor for mouse-look. `Esc` closes the
//! pop-up before it opens the menu (see [`menu_toggle_system`](super::menu::menu_toggle_system)).
//!
//! Unlike the menu, a pop-up leaves the other keys bound. The arrival pop-up's own hint says `R`
//! picks the next route, and the clock keys have no reason to stop.
//!
//! The state lives here, beside [`MenuState`](super::menu::MenuState), for the same reason: the
//! camera reads it to decide whether to accept input, and it should not have to depend on the UI
//! toolkit to ask. [`crate::ui::arrival`] opens it when it starts showing a landing, and closes it
//! when the pop-up's ✕ is clicked or the landing is over.

use bevy::ecs::system::Resource;

/// Whether a dismissible pop-up is open.
///
/// Changed only through [`open`](Self::open) and [`close`](Self::close), like
/// [`MenuState`](super::menu::MenuState).
///
/// # Examples
///
/// ```
/// use physics_sandbox::PopupState;
///
/// let mut popup = PopupState::default();
/// assert!(!popup.is_open());
///
/// popup.open();
/// assert!(popup.is_open());
///
/// popup.close();
/// assert!(!popup.is_open());
/// ```
#[derive(Resource, Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct PopupState {
    open: bool,
}

impl PopupState {
    /// Is a pop-up open?
    #[inline]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the pop-up. Idempotent.
    #[inline]
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Close the pop-up. Idempotent. Like closing the menu, it does not re-capture the cursor:
    /// a click in the scene does that.
    #[inline]
    pub fn close(&mut self) {
        self.open = false;
    }
}
