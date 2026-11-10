//! Constants for the `config` layer: where the configuration files are, and what they are called.
//!
//! The values *inside* the files have their defaults in each owning layer's own constants file
//! (`input`, `ui`, `render`, `spacecraft`, `performance`). This file only says where to look.

/// The environment variable that points the app at a configuration directory.
///
/// **Why this value.** Prefixed like every other variable the app reads (`RGA_AA` and
/// the rest in `src/performance/`), so `env | grep RGA_` lists them all.
///
/// **Used by.** [`crate::config::config_dir`].
///
/// **If you change it.** Update `config/README.md`, which tells people to set it.
pub const CONFIG_DIR_VAR: &str = "RGA_CONFIG";

/// The configuration directory's name, looked for in the crate root and then the working directory.
///
/// **Why this value.** A plain `config/` beside `assets/`, the other runtime-loaded directory.
///
/// **Used by.** [`crate::config::config_dir`].
///
/// **If you change it.** Rename the shipped directory to match, or the app silently runs on defaults.
pub const CONFIG_DIR_NAME: &str = "config";

/// The spacecraft file: the fleet `G` cycles through.
///
/// **Used by.** [`crate::config::Config::load`].
///
/// **If you change it.** Rename the shipped file, or the built-in fleet is used.
pub const SPACECRAFT_FILE: &str = "spacecraft.toml";

/// The camera file: movement speed, speed-mode multipliers, and look sensitivity.
///
/// **Used by.** [`crate::config::Config::load`].
///
/// **If you change it.** Rename the shipped file, or the built-in tuning is used.
pub const CAMERA_FILE: &str = "camera.toml";

/// The on-screen UI file: the crosshair, the compass, and the arrival pop-up.
///
/// **Used by.** [`crate::config::Config::load`].
///
/// **If you change it.** Rename the shipped file, or the built-in sizes are used.
pub const UI_FILE: &str = "ui.toml";

/// The motion-trails file.
///
/// **Used by.** [`crate::config::Config::load`].
///
/// **If you change it.** Rename the shipped file, or the built-in trails are used.
pub const TRAILS_FILE: &str = "trails.toml";

/// The performance file: anti-aliasing, the floor's eclipse shadows, and vsync. The
/// `RGA_*` environment variables override it for one run.
///
/// **Used by.** [`crate::config::Config::load`].
///
/// **If you change it.** Rename the shipped file, or the built-in switches are used.
pub const PERFORMANCE_FILE: &str = "performance.toml";

/// The controls file: which key each rebindable action is on.
///
/// **Used by.** [`crate::config::Config::load`], and the menu's Save on the controls page.
///
/// **If you change it.** Rename the shipped file, or the default keys are used.
pub const CONTROLS_FILE: &str = "controls.toml";

/// The first lines of a file the menu saves.
///
/// **Why this value.** A saved file is generated, so the comments a hand-written one carries are
/// not kept; this says so, and where the keys are explained, so the next person to open it is not
/// surprised.
///
/// **Used by.** [`crate::config::save_requested_system`].
///
/// **If you change it.** Keep every line a TOML comment, or saved files stop loading.
pub const SAVED_FILE_HEADER: &str = "# Saved from the in-app menu, which does not keep comments.\n\
# Every key is optional; see config/README.md for what each one does.\n\n";
