//! The key bindings, as data: which key does what, read by every input system, by `--help`, and by
//! the in-window menu.
//!
//! Every rebindable action is an [`Action`], described by a row of
//! [`crate::constants::ACTIONS`] (its label, group, and default key). The keys in use are a
//! [`KeyMap`] resource, which starts on the defaults, can be loaded from `config/controls.toml`,
//! and is changed from the menu's controls page. The systems that act on a key ask the `KeyMap`
//! (`keymap.just_pressed(&keys, Action::Pause)`) rather than naming a `KeyCode`, so what the menu
//! and `--help` show is what the keys do. The bindings that cannot change (the mouse, `Esc`, the
//! OS quit keys) are [`crate::constants::FIXED_BINDINGS`].
//!
//! # Why this is not in a layer
//!
//! `--help` ([`crate::experiments::selection::help_text`]) prints the bindings before any window
//! exists, the menu ([`crate::ui::menu`]) draws them, and `input/` and `render/` act on them. Those
//! layers may not import one another, so the bindings sit at the crate root next to
//! [`crate::constants`], on the same rule that module states: a thing read by more than one layer
//! belongs above all of them.

use std::borrow::Cow;

use bevy::ecs::system::{Res, Resource};
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;

use crate::constants::{ACTIONS, BINDABLE_KEYS, FIXED_BINDINGS};

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
    /// Adding objects and rockets to the running scene.
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
    /// `Window` last because quitting is the thing you need exactly once.
    pub const ALL: [Self; 6] = [
        Self::Camera,
        Self::Simulation,
        Self::View,
        Self::Place,
        Self::Menu,
        Self::Window,
    ];

    /// The heading the group is shown under.
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

/// A binding that cannot be changed: its keys as shown, and what they do.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct KeyBinding {
    /// The keys, as they should be shown: `"Left click"`, `"Cmd-Q"`.
    pub keys: &'static str,
    /// What they do, phrased as an action.
    pub action: &'static str,
    /// Which part of the app this belongs to.
    pub group: BindingGroup,
}

impl KeyBinding {
    /// Neither field is empty. A blank row renders as an unexplained gap.
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.keys.trim().is_empty() && !self.action.trim().is_empty()
    }
}

/// Something a key does that the user may rebind. Described by its row of
/// [`crate::constants::ACTIONS`], in the same order.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Action {
    /// Fly forward.
    MoveForward,
    /// Fly back.
    MoveBack,
    /// Fly left.
    MoveLeft,
    /// Fly right.
    MoveRight,
    /// Fly up, along world `+Y`.
    MoveUp,
    /// Fly down, along world `-Y`.
    MoveDown,
    /// A second key for flying up.
    MoveUpAlt,
    /// A second key for flying down.
    MoveDownAlt,
    /// Cycle the camera's speed mode.
    CycleSpeed,
    /// Lock onto the body at the crosshair, or release the lock.
    LockTarget,
    /// Pause or resume the simulation.
    Pause,
    /// Advance one frame while paused.
    SingleStep,
    /// Double the time scale.
    TimeFaster,
    /// Halve the time scale.
    TimeSlower,
    /// Multiply the time scale by ten.
    TimeDecadeFaster,
    /// Divide the time scale by ten.
    TimeDecadeSlower,
    /// Reset the time scale.
    TimeReset,
    /// Show or hide the motion trails.
    ToggleTrails,
    /// Open or close the object picker.
    TogglePicker,
    /// Move the picker's selection up.
    PickerUp,
    /// Move the picker's selection down.
    PickerDown,
    /// Place the picker's selection.
    PickerPlace,
    /// Pick the body at the crosshair as a rocket's origin or destination.
    Launch,
    /// Select the next spacecraft.
    NextCraft,
    /// Switch the black hole's disc between its shaded light and a flat test pattern.
    ToggleBlackHoleView,
    /// Show or hide the stars round the black hole.
    ToggleStars,
    /// Show or hide the HUD: the debug panel, crosshair, compass, mission readout, and picker.
    ToggleHud,
    /// Show or hide the outline of the black hole's event horizon.
    ToggleOutline,
}

/// The description of one [`Action`]: a row of [`crate::constants::ACTIONS`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ActionInfo {
    /// The action this row describes.
    pub action: Action,
    /// Its key in `config/controls.toml`.
    pub name: &'static str,
    /// What it does, as shown in `--help` and the menu.
    pub label: &'static str,
    /// Which part of the app it belongs to.
    pub group: BindingGroup,
    /// The key it is bound to until changed.
    pub default_key: KeyCode,
}

impl Action {
    /// Every action, in the order of [`crate::constants::ACTIONS`].
    pub const ALL: [Action; 28] = [
        Action::MoveForward,
        Action::MoveBack,
        Action::MoveLeft,
        Action::MoveRight,
        Action::MoveUp,
        Action::MoveDown,
        Action::MoveUpAlt,
        Action::MoveDownAlt,
        Action::CycleSpeed,
        Action::LockTarget,
        Action::Pause,
        Action::SingleStep,
        Action::TimeFaster,
        Action::TimeSlower,
        Action::TimeDecadeFaster,
        Action::TimeDecadeSlower,
        Action::TimeReset,
        Action::ToggleTrails,
        Action::TogglePicker,
        Action::PickerUp,
        Action::PickerDown,
        Action::PickerPlace,
        Action::Launch,
        Action::NextCraft,
        Action::ToggleBlackHoleView,
        Action::ToggleStars,
        Action::ToggleHud,
        Action::ToggleOutline,
    ];

    /// This action's row of [`crate::constants::ACTIONS`].
    #[inline]
    pub fn info(self) -> &'static ActionInfo {
        &ACTIONS[self as usize]
    }

    /// The action whose `config/controls.toml` name is `name`.
    pub fn from_name(name: &str) -> Option<Action> {
        ACTIONS
            .iter()
            .find(|info| info.name == name)
            .map(|info| info.action)
    }
}

/// The key each [`Action`] is bound to. One key per action and one action per key:
/// [`set`](Self::set) swaps when the new key is already taken.
///
/// # Examples
///
/// ```
/// use bevy::input::keyboard::KeyCode;
/// use physics_sandbox::{Action, KeyMap};
///
/// let mut keys = KeyMap::default();
/// assert_eq!(keys.key(Action::Pause), KeyCode::KeyP);
///
/// // `K` is free, so pause simply moves to it.
/// keys.set(Action::Pause, KeyCode::KeyK);
/// assert_eq!(keys.key(Action::Pause), KeyCode::KeyK);
///
/// // `T` belongs to the trails, so they take pause's old key.
/// let displaced = keys.set(Action::Pause, KeyCode::KeyT);
/// assert_eq!(displaced, Some(Action::ToggleTrails));
/// assert_eq!(keys.key(Action::ToggleTrails), KeyCode::KeyK);
/// ```
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct KeyMap {
    keys: [KeyCode; 28],
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            keys: ACTIONS.map(|info| info.default_key),
        }
    }
}

impl KeyMap {
    /// The key `action` is bound to.
    #[inline]
    pub fn key(&self, action: Action) -> KeyCode {
        self.keys[action as usize]
    }

    /// Bind `action` to `key`. If another action had `key`, it takes `action`'s old key instead,
    /// so no key ever does two things; that action is returned.
    pub fn set(&mut self, action: Action, key: KeyCode) -> Option<Action> {
        let old = self.key(action);
        let displaced = self.action_for(key).filter(|&other| other != action);
        if let Some(other) = displaced {
            self.keys[other as usize] = old;
        }
        self.keys[action as usize] = key;
        displaced
    }

    /// The action bound to `key`, if any.
    pub fn action_for(&self, key: KeyCode) -> Option<Action> {
        Action::ALL
            .into_iter()
            .find(|&action| self.key(action) == key)
    }

    /// The defaults with `overrides` applied as given, for reading a saved file: unlike
    /// [`set`](Self::set) nothing is swapped, so two actions ending on one key is an error naming
    /// both, rather than a silent reshuffle of what the file said.
    pub fn with_overrides(
        overrides: impl IntoIterator<Item = (Action, KeyCode)>,
    ) -> Result<Self, String> {
        let mut map = Self::default();
        for (action, key) in overrides {
            map.keys[action as usize] = key;
        }
        for (index, &first) in Action::ALL.iter().enumerate() {
            for &second in &Action::ALL[index + 1..] {
                if map.key(first) == map.key(second) {
                    return Err(format!(
                        "`{}` and `{}` are both on {}",
                        first.info().name,
                        second.info().name,
                        key_name(map.key(first))
                    ));
                }
            }
        }
        Ok(map)
    }

    /// Every action back on its default key.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Is `action`'s key held?
    #[inline]
    pub fn pressed(&self, input: &ButtonInput<KeyCode>, action: Action) -> bool {
        input.pressed(self.key(action))
    }

    /// Was `action`'s key pressed this frame?
    #[inline]
    pub fn just_pressed(&self, input: &ButtonInput<KeyCode>, action: Action) -> bool {
        input.just_pressed(self.key(action))
    }
}

/// The app's [`KeyMap`], or the defaults when there is none. Systems take the key map as an
/// `Option` so a test app that adds one system without its plugin still runs.
pub fn keymap_or_default<'a>(keymap: &'a Option<Res<KeyMap>>) -> Cow<'a, KeyMap> {
    match keymap {
        Some(keymap) => Cow::Borrowed(&**keymap),
        None => Cow::Owned(KeyMap::default()),
    }
}

/// Whether `key` may be bound to an action: one of [`crate::constants::BINDABLE_KEYS`].
#[inline]
pub fn is_bindable(key: KeyCode) -> bool {
    BINDABLE_KEYS.contains(&key)
}

/// `key`'s name in `config/controls.toml`: Bevy's own (`"KeyW"`, `"BracketRight"`).
pub fn key_name(key: KeyCode) -> String {
    format!("{key:?}")
}

/// The bindable key named `name` by [`key_name`], or `None`.
pub fn parse_key(name: &str) -> Option<KeyCode> {
    BINDABLE_KEYS.into_iter().find(|&key| key_name(key) == name)
}

/// `key` as a person would write it: `"W"`, `"1"`, `"Space"`, `"Left Ctrl"`, `"]"`, `"Up"`.
pub fn key_label(key: KeyCode) -> String {
    let name = key_name(key);
    if let Some(letter) = name.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return digit.to_string();
    }
    if let Some(arrow) = name.strip_prefix("Arrow") {
        return arrow.to_string();
    }
    if let Some(pad) = name.strip_prefix("Numpad") {
        return format!("Numpad {pad}");
    }
    let side = |stem: &str, label: &str| {
        name.strip_prefix(stem)
            .map(|side| format!("{side} {label}"))
    };
    if let Some(label) = side("Control", "Ctrl")
        .or_else(|| side("Shift", "Shift"))
        .or_else(|| side("Alt", "Alt"))
    {
        return label;
    }
    match key {
        KeyCode::Backquote => "`",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Backslash => "\\",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::PageUp => "Page Up",
        KeyCode::PageDown => "Page Down",
        KeyCode::CapsLock => "Caps Lock",
        _ => return name,
    }
    .to_string()
}

/// One row of the bindings as shown: the key, what it does, its group, and, when it can be
/// rebound, which action it is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingRow {
    /// The key as shown, from [`key_label`] or a fixed row.
    pub keys: String,
    /// What it does.
    pub action: &'static str,
    /// Which part of the app it belongs to.
    pub group: BindingGroup,
    /// The action, when the row can be rebound; `None` for a fixed row.
    pub rebind: Option<Action>,
}

/// Every binding under `keymap`, grouped in [`BindingGroup::ALL`] order: within a group, the
/// rebindable actions in table order, then the fixed rows.
pub fn binding_rows(keymap: &KeyMap) -> Vec<BindingRow> {
    BindingGroup::ALL
        .into_iter()
        .flat_map(|group| {
            let actions = ACTIONS
                .iter()
                .filter(move |info| info.group == group)
                .map(move |info| BindingRow {
                    keys: key_label(keymap.key(info.action)),
                    action: info.label,
                    group,
                    rebind: Some(info.action),
                });
            let fixed = FIXED_BINDINGS
                .iter()
                .filter(move |binding| binding.group == group)
                .map(move |binding| BindingRow {
                    keys: binding.keys.to_string(),
                    action: binding.action,
                    group,
                    rebind: None,
                });
            actions.chain(fixed)
        })
        .collect()
}

/// The bindings as a plain-text block, grouped, for `--help`. The default keys: `--help` runs
/// before any `config/controls.toml` is read.
///
/// Indented two levels to match the rest of the usage text. Ends with a newline.
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
    let rows = binding_rows(&KeyMap::default());
    let width = rows
        .iter()
        .map(|row| row.keys.chars().count())
        .max()
        .unwrap_or(0);
    let mut text = String::new();

    for group in BindingGroup::ALL {
        let mut in_group = rows.iter().filter(|row| row.group == group).peekable();
        if in_group.peek().is_none() {
            continue;
        }

        text.push_str("    ");
        text.push_str(group.title());
        text.push_str(":\n");

        for row in in_group {
            text.push_str("        ");
            text.push_str(&row.keys);
            for _ in row.keys.chars().count()..width {
                text.push(' ');
            }
            text.push_str("   ");
            text.push_str(row.action);
            text.push('\n');
        }
    }

    text
}

#[cfg(test)]
#[path = "../tst/bindings_tests.rs"]
mod tests;
