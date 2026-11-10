//! Configuration files: the settings in `config/*.toml`, read once at startup and written back
//! when the menu saves a page ([`ConfigPlugin`]).
//!
//! Each file is optional, and so is each key in it. A missing file or key keeps the built-in
//! value, which is the owning layer's `Default` and the constants behind it, so the app runs the
//! same with no `config/` directory at all. A file that cannot be read, has a key this layer does
//! not know, or has a value out of range is an error: `main.rs` prints it and exits, the same as a
//! bad command-line argument, because a setting that silently did nothing is worse than a refusal.
//!
//! | File | Builds |
//! | --- | --- |
//! | `spacecraft.toml` | [`Fleet`], into the [`Mission`] |
//! | `camera.toml` | [`CameraSettings`] |
//! | `ui.toml` | [`CrosshairConfig`], [`CompassConfig`], [`ArrivalPopupConfig`] |
//! | `trails.toml` | [`TrailConfig`] |
//! | `performance.toml` | [`PerformanceSettings`], which the `RGA_*` variables override |
//! | `controls.toml` | [`KeyMap`] |
//!
//! # Where it sits
//!
//! Above every other layer, beside `main.rs`: it builds resources that `input`, `ui`, `render`,
//! `spacecraft`, and `performance` own, and none of them depends on it. Each plugin
//! `init_resource`s its own settings, which keeps a value inserted earlier, so
//! [`Config::insert_into`] must run before the plugins are added.

pub mod constants;
pub mod files;

use std::fmt;
use std::path::{Path, PathBuf};

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut, Resource, SystemParam};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::bindings::KeyMap;
use crate::config::constants::{
    CAMERA_FILE, CONFIG_DIR_NAME, CONFIG_DIR_VAR, CONTROLS_FILE, PERFORMANCE_FILE,
    SAVED_FILE_HEADER, SPACECRAFT_FILE, TRAILS_FILE, UI_FILE,
};
use crate::config::files::{
    CameraFile, ControlsFile, PerformanceFile, SpacecraftFile, TrailsFile, UiFile,
};
use crate::input::camera::CameraSettings;
use crate::performance::PerformanceSettings;
use crate::render::mission::Mission;
use crate::render::trail::TrailConfig;
use crate::spacecraft::Fleet;
use crate::ui::arrival::ArrivalPopupConfig;
use crate::ui::compass::CompassConfig;
use crate::ui::crosshair::CrosshairConfig;
use crate::ui::menu::{MenuRequests, SaveTarget};

/// A configuration file that could not be used: which file, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    /// The file.
    pub path: PathBuf,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Every setting the configuration files cover, as the resources the app runs on.
///
/// [`Default`] is the built-in configuration, the same as having no files.
///
/// # Examples
///
/// ```
/// use physics_sandbox::Config;
///
/// // A directory with no files in it is the built-in configuration.
/// let config = Config::load(std::path::Path::new("no/such/directory")).unwrap();
/// assert_eq!(config, Config::default());
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Config {
    /// From `spacecraft.toml`.
    pub fleet: Fleet,
    /// From `camera.toml`.
    pub camera: CameraSettings,
    /// From `ui.toml`'s `[crosshair]`.
    pub crosshair: CrosshairConfig,
    /// From `ui.toml`'s `[compass]`.
    pub compass: CompassConfig,
    /// From `ui.toml`'s `[arrival]`.
    pub arrival: ArrivalPopupConfig,
    /// From `trails.toml`.
    pub trails: TrailConfig,
    /// From `performance.toml`, before the environment overrides it.
    pub performance: PerformanceSettings,
    /// From `controls.toml`.
    pub keymap: KeyMap,
}

impl Config {
    /// Read every configuration file in `dir`. A missing file keeps its built-in values.
    pub fn load(dir: &Path) -> Result<Self, ConfigError> {
        let file = |name: &str| dir.join(name);
        let (crosshair, compass, arrival) =
            read::<UiFile, _>(&file(UI_FILE), UiFile::into_runtime)?;
        Ok(Self {
            fleet: read::<SpacecraftFile, _>(&file(SPACECRAFT_FILE), SpacecraftFile::into_runtime)?,
            camera: read::<CameraFile, _>(&file(CAMERA_FILE), CameraFile::into_runtime)?,
            crosshair,
            compass,
            arrival,
            trails: read::<TrailsFile, _>(&file(TRAILS_FILE), TrailsFile::into_runtime)?,
            performance: read::<PerformanceFile, _>(
                &file(PERFORMANCE_FILE),
                PerformanceFile::into_runtime,
            )?,
            keymap: read::<ControlsFile, _>(&file(CONTROLS_FILE), ControlsFile::into_runtime)?,
        })
    }

    /// Insert the resources into `app`. Call it before adding the plugins, which keep a resource
    /// that is already there. [`Config::performance`] is not inserted: it is needed before the
    /// `App` exists, to create the window, and goes to `PerformancePlugin` instead.
    pub fn insert_into(self, app: &mut App) {
        app.insert_resource(Mission::with_fleet(self.fleet))
            .insert_resource(self.camera)
            .insert_resource(self.crosshair)
            .insert_resource(self.compass)
            .insert_resource(self.arrival)
            .insert_resource(self.trails)
            .insert_resource(self.keymap);
    }
}

/// The directory the configuration was read from, which the menu's Save writes back to.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ConfigDir(pub PathBuf);

/// Saves the configuration when the menu asks: the menu (in `ui`) cannot write files of a layer
/// above it, so it posts a [`SaveTarget`] in [`MenuRequests`] and [`save_requested_system`] writes
/// that page's file and reports back.
pub struct ConfigPlugin {
    dir: PathBuf,
}

impl ConfigPlugin {
    /// Save into `dir`, normally [`config_dir`].
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConfigDir(self.dir.clone()))
            .init_resource::<MenuRequests>()
            .add_systems(Update, save_requested_system);
    }
}

/// Every resource a page of the menu can save. Each is optional, so an app without one of the
/// plugins still saves the rest; a missing one saves its defaults.
#[derive(SystemParam)]
pub struct Configured<'w> {
    keymap: Option<Res<'w, KeyMap>>,
    performance: Option<Res<'w, PerformanceSettings>>,
    camera: Option<Res<'w, CameraSettings>>,
    crosshair: Option<Res<'w, CrosshairConfig>>,
    compass: Option<Res<'w, CompassConfig>>,
    arrival: Option<Res<'w, ArrivalPopupConfig>>,
    trails: Option<Res<'w, TrailConfig>>,
    mission: Option<Res<'w, Mission>>,
}

/// The value of an optional resource, or its default.
fn or_default<T: Resource + Clone + Default>(resource: &Option<Res<T>>) -> T {
    resource.as_deref().cloned().unwrap_or_default()
}

/// Write the page the menu asked to save, if it asked, and put the outcome in
/// [`MenuRequests::status`] for the menu to show.
pub fn save_requested_system(
    dir: Res<ConfigDir>,
    mut requests: ResMut<MenuRequests>,
    configured: Configured,
) {
    let Some(target) = requests.take_save() else {
        return;
    };
    let c = &configured;
    let dir = &dir.0;
    let result = match target {
        SaveTarget::Controls => write_file(
            dir,
            CONTROLS_FILE,
            &ControlsFile::from_runtime(&or_default(&c.keymap)),
        ),
        SaveTarget::Video => write_file(
            dir,
            PERFORMANCE_FILE,
            &PerformanceFile::from_runtime(&or_default(&c.performance)),
        ),
        SaveTarget::Camera => write_file(
            dir,
            CAMERA_FILE,
            &CameraFile::from_runtime(&or_default(&c.camera)),
        ),
        SaveTarget::Interface => write_file(
            dir,
            UI_FILE,
            &UiFile::from_runtime(
                &or_default(&c.crosshair),
                &or_default(&c.compass),
                &or_default(&c.arrival),
            ),
        ),
        SaveTarget::Trails => write_file(
            dir,
            TRAILS_FILE,
            &TrailsFile::from_runtime(&or_default(&c.trails)),
        ),
        SaveTarget::Spacecraft => {
            let fleet = c
                .mission
                .as_ref()
                .map_or_else(Fleet::default, |mission| mission.fleet.clone());
            write_file(dir, SPACECRAFT_FILE, &SpacecraftFile::from_runtime(&fleet))
        }
    };
    requests.set_status(match result {
        Ok(path) => format!("Saved {}", path.display()),
        Err(error) => format!("Could not save: {error}"),
    });
}

/// Write `file` as TOML to `dir/name`, after [`SAVED_FILE_HEADER`], creating `dir` if needed.
/// Returns the path written.
pub fn write_file<F: Serialize>(dir: &Path, name: &str, file: &F) -> Result<PathBuf, ConfigError> {
    let path = dir.join(name);
    let error = |message: String| ConfigError {
        path: path.clone(),
        message,
    };
    let text = toml::to_string(file).map_err(|e| error(e.to_string()))?;
    std::fs::create_dir_all(dir).map_err(|e| error(e.to_string()))?;
    std::fs::write(&path, format!("{SAVED_FILE_HEADER}{text}"))
        .map_err(|e| error(e.to_string()))?;
    Ok(path)
}

/// Parse `text` as a configuration file of type `F` and convert it with `into_runtime`. Pure, so
/// a file's contents can be tested without writing it to disk.
pub fn parse<F, T>(text: &str, into_runtime: fn(F) -> Result<T, String>) -> Result<T, String>
where
    F: DeserializeOwned,
{
    let file: F = toml::from_str(text).map_err(|error| error.to_string())?;
    into_runtime(file)
}

/// Read the file at `path` as type `F`, or its defaults when it does not exist.
fn read<F, T>(path: &Path, into_runtime: fn(F) -> Result<T, String>) -> Result<T, ConfigError>
where
    F: DeserializeOwned + Default,
{
    let error = |message: String| ConfigError {
        path: path.to_path_buf(),
        message,
    };
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text, into_runtime).map_err(error),
        Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {
            into_runtime(F::default()).map_err(error)
        }
        Err(other) => Err(error(other.to_string())),
    }
}

/// The configuration directory: [`CONFIG_DIR_VAR`] when it is set, otherwise `config/` in the
/// crate root when that exists (so `cargo run` and a binary run from `target/` both find it, the
/// same rule `main.rs` uses for `assets/`), otherwise `config/` in the working directory.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_VAR) {
        return PathBuf::from(dir);
    }
    let in_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join(CONFIG_DIR_NAME);
    if in_crate.is_dir() {
        in_crate
    } else {
        PathBuf::from(CONFIG_DIR_NAME)
    }
}

#[cfg(test)]
#[path = "../../tst/config/config_tests.rs"]
mod tests;
