//! The key bindings, as data — one table that both `--help` and the in-window menu read.
//!
//! Every key the app responds to is a row of [`crate::constants::KEY_BINDINGS`]. Nothing
//! here reads or writes input: this module is the *description* of the bindings, not their
//! implementation. The
//! systems that actually bind the keys live in the layers that own what the key changes —
//! `input/camera.rs` for movement, `input/controls.rs` for the clock, `render/trail.rs`
//! for `T`, `input/menu.rs` for `Esc`.
//!
//! # Why the table is not in a layer
//!
//! Two very different consumers need the same list:
//!
//! - [`help_text`](crate::experiments::selection::help_text), in the `experiments` layer,
//!   which prints it to a terminal before any window exists;
//! - [`ui::menu`](crate::ui::menu), which draws it on screen.
//!
//! Neither layer may import the other, so a table owned by either would have to be
//! duplicated in the other — and a duplicated key list drifts the first time a binding
//! changes, in the one place a user goes to *find out* what the bindings are. So it sits
//! at the crate root next to [`crate::constants`], on the same rule that module states: a
//! thing read by more than one layer belongs above all of them.
//!
//! # This is documentation, and documentation can lie
//!
//! Nothing checks at compile time that a row here matches a `just_pressed` call over in
//! `input/`. What is checked is the other direction: the key-collision test in
//! `tst/input/controls_tests.rs` asserts no two *implementations* claim one key, and
//! `tst/bindings_tests.rs` asserts every group is non-empty, no key string appears twice,
//! and every row is renderable. A binding added in code and forgotten here will simply be
//! undocumented — so add the row in the same change, which is what the layer READMEs say.

use crate::constants::KEY_BINDINGS;

/// Which part of the app a binding belongs to.
///
/// The grouping is by *what the key changes*, which is also how the bindings are laid out
/// on screen and in `--help`: someone looking for "how do I go faster" is thinking about
/// the simulation, not about which module owns the keypress.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BindingGroup {
    /// Moving and aiming the camera.
    Camera,
    /// The simulation clock: pause, step, speed.
    Simulation,
    /// Observer aids that are drawn but change nothing: trails.
    View,
    /// Adding interstellar objects to the running scene.
    Place,
    /// The menu itself.
    Menu,
    /// The OS window: leaving the app.
    Window,
}

impl BindingGroup {
    /// Every group, in the order they are displayed.
    ///
    /// `Camera` first because it is what a new viewer needs to do anything at all, and
    /// `Window` last because quitting is the thing you need exactly once — but it is on
    /// the list, because the window is full-screen and has no close button (amendment
    /// A12).
    pub const ALL: [Self; 6] = [
        Self::Camera,
        Self::Simulation,
        Self::View,
        Self::Place,
        Self::Menu,
        Self::Window,
    ];

    /// The heading shown above the group.
    #[inline]
    pub fn title(&self) -> &'static str {
        match self {
            Self::Camera => "Camera",
            Self::Simulation => "Simulation",
            Self::View => "View",
            Self::Place => "Place an object",
            Self::Menu => "Menu",
            Self::Window => "Window",
        }
    }
}

/// One binding: the keys, and what they do.
///
/// `keys` is a display string rather than a `KeyCode`, deliberately. It has to say things
/// no single `KeyCode` can — `"W / S"` is two keys with opposite effects, `"Space / Left
/// Ctrl"` names one axis, `"Cmd-Q / Alt-F4"` is platform-dependent and not ours to bind at
/// all. Making this a `KeyCode` would buy a compile-time link to the implementation for
/// the simple rows and force a second, stringly-typed mechanism for the rest.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct KeyBinding {
    /// The keys, as they should be shown: `"W / S"`, `"Left Shift"`, `"]"`.
    pub keys: &'static str,
    /// What they do, phrased as an action: `"Fly forward / back"`.
    pub action: &'static str,
    /// Which part of the app this belongs to.
    pub group: BindingGroup,
}

impl KeyBinding {
    /// Validation_Rules for `KeyBinding`: neither field is empty.
    ///
    /// A blank row renders as a gap with no explanation, which is worse than an absent
    /// row — the reader cannot tell whether the key does nothing or the label is missing.
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.keys.trim().is_empty() && !self.action.trim().is_empty()
    }
}

/// The bindings in `group`, in table order.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{bindings_in, BindingGroup};
///
/// let menu: Vec<_> = bindings_in(BindingGroup::Menu).collect();
///
/// assert_eq!(menu.len(), 1, "one key opens the menu");
/// assert_eq!(menu[0].keys, "Esc");
/// ```
pub fn bindings_in(group: BindingGroup) -> impl Iterator<Item = &'static KeyBinding> {
    KEY_BINDINGS
        .iter()
        .filter(move |binding| binding.group == group)
}

/// Width of the key column, in characters: the longest `keys` string in the table.
///
/// Measured rather than declared, so a new binding with a long key string widens the
/// column instead of overflowing it. Used by [`binding_help_text`] for the terminal
/// layout; the on-screen menu uses egui's own grid and does not need it.
pub fn key_column_width() -> usize {
    KEY_BINDINGS
        .iter()
        .map(|binding| binding.keys.chars().count())
        .max()
        .unwrap_or(0)
}

/// The bindings as a plain-text block, grouped, for `--help`.
///
/// Indented two levels to match the rest of the usage text. Ends with a newline, so a
/// caller can append directly.
///
/// # Examples
///
/// ```
/// use physics_sandbox::binding_help_text;
///
/// let text = binding_help_text();
///
/// assert!(text.contains("Camera:"));
/// assert!(text.contains("Esc"));
/// assert!(text.ends_with('\n'));
/// ```
pub fn binding_help_text() -> String {
    let width = key_column_width();
    let mut text = String::new();

    for group in BindingGroup::ALL {
        let mut rows = bindings_in(group).peekable();
        if rows.peek().is_none() {
            continue;
        }

        text.push_str("    ");
        text.push_str(group.title());
        text.push_str(":\n");

        for binding in rows {
            text.push_str("        ");
            text.push_str(binding.keys);
            for _ in binding.keys.chars().count()..width {
                text.push(' ');
            }
            text.push_str("   ");
            text.push_str(binding.action);
            text.push('\n');
        }
    }

    text
}

#[cfg(test)]
#[path = "../tst/bindings_tests.rs"]
mod tests;
