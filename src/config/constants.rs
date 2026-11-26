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

/// The spacecraft file: the fleet, which craft `R` launches, and the kind of trip.
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

/// The collisions file: how much detail impacts are simulated in.
///
/// **Used by.** [`crate::config::Config::load`], and the menu's Save on the collisions page.
///
/// **If you change it.** Rename the shipped file, or the built-in detail is used.
pub const COLLISION_FILE: &str = "collision.toml";

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

/// The wormhole file: its throat, its length, and how sharply its mouths flare.
///
/// **Why this value.** Named for what it sets, as the other files are.
///
/// **Used by.** [`crate::config::Config::load`], and the menu's Save on the wormhole page.
///
/// **If you change it.** Rename the shipped file, or the built-in wormhole is used.
pub const WORMHOLE_FILE: &str = "wormhole.toml";

/// The directory inside `config/` the menu saves to.
///
/// **Why this value.** The user's own settings, apart from the shipped ones. `.gitignore` names it,
/// so trying things out in the menu never changes a tracked file.
///
/// **Used by.** [`crate::config::saved_dir`].
///
/// **If you change it.** Change `.gitignore` with it; settings saved under the old name are no longer
/// read.
pub const SAVED_DIR_NAME: &str = "user";

/// The environment variable a relaunched run finds the previous run's settings by.
///
/// **Why this value.** The menu's Restart and Experiments page start the program again
/// ([`crate::config::relaunch`]), and the settings changed in the menu must survive that whether
/// or not they were saved. The old run writes every page to a fresh directory and names it here;
/// the new one reads that directory ahead of the saved and shipped ones, then deletes it. `RGA_`
/// like the crate's other variables.
///
/// **Used by.** [`crate::config::relaunch::relaunch_requested_system`], which sets it, and
/// [`crate::config::relaunch::take_relaunch_dir`], which reads it in `main.rs`.
///
/// **If you change it.** Both ends are in this crate, so nothing outside it notices.
pub const RELAUNCH_CONFIG_VAR: &str = "RGA_RELAUNCH_CONFIG";

/// How the directory a relaunch writes its settings to is named, in the system's temporary
/// directory.
///
/// **Why this value.** The new run deletes the directory once it has read it, so it must be sure
/// the directory is one a relaunch made: it deletes only a directory directly inside the temporary
/// directory whose name starts with this. A variable set by hand to a directory of the user's is
/// still read, and never deleted.
///
/// **Used by.** [`crate::config::relaunch`].
///
/// **If you change it.** Directories left by a crash under the old name are no longer cleaned up;
/// they are a few kilobytes each, in the temporary directory.
pub const RELAUNCH_DIR_PREFIX: &str = "physics-sandbox-relaunch-";
