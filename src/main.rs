//! `physics-sandbox` binary entry point.
//!
//! Parses the command line into an [`experiments::Command`](physics_sandbox::Command) —
//! run an experiment, list the catalog, or print usage — and for a run, hands the
//! chosen [`Selection`](physics_sandbox::Selection) to `ScenePlugin`. The parse itself
//! lives in `experiments::selection` so it is testable without launching a window;
//! this file only decides what to do with the result.
//!
//! Wires a Bevy [`App`] with `DefaultPlugins` (configured with a **borderless
//! full-screen** primary window titled `"physics-sandbox"` — see
//! [`WINDOW_MODE`](physics_sandbox::constants::WINDOW_MODE) and amendment A12) plus the
//! app-level plugins in the
//! order mandated by the design: `ScenePlugin`, `FreeFlyCameraPlugin`, and
//! `DebugUiPlugin`, followed by `CrosshairPlugin`. The crosshair goes last
//! because `DebugUiPlugin` installs `EguiPlugin` first and both plugins guard
//! against double-adding it; the order is otherwise immaterial, since the two
//! draw to different egui layers. Any Bevy initialization panic (e.g. the wgpu backend
//! fails to create a window) is allowed to propagate out of `app.run()` so
//! the process exits non-zero with the Bevy diagnostic on stderr, per
//! Requirement 1.3.

use std::path::PathBuf;

use bevy::prelude::*;
use physics_sandbox::constants::{WINDOW_HEIGHT_PX, WINDOW_MODE, WINDOW_TITLE, WINDOW_WIDTH_PX};
use physics_sandbox::{
    catalog_lines, config_dir, help_text, parse_args, AccretionDiscPlugin, ArrivalPopupPlugin,
    BlackHoleViewPlugin, Command, CompassPlugin, Config, ConfigPlugin, CrosshairPlugin,
    DebugUiPlugin, FloorGridPlugin, FreeFlyCameraPlugin, HudPlugin, LaunchInputPlugin,
    MenuInputPlugin, MenuPanelPlugin, MissionPlugin, PerformancePlugin, PerformanceSettings,
    PlacementInputPlugin, PlacementPanelPlugin, PlacementPlugin, ScenePlugin, Selection,
    SimulationControlsPlugin, TrailPlugin,
};

fn main() {
    // `skip(1)` drops the program name. A parse error is a usage mistake, not a crash:
    // print it and the usage text to stderr and exit non-zero, which is what a shell
    // expects.
    let selection = match parse_args(std::env::args().skip(1)) {
        Ok(Command::Run(selection)) => selection,
        Ok(Command::List) => {
            for line in catalog_lines() {
                println!("{line}");
            }
            return;
        }
        Ok(Command::Help) => {
            print!("{}", help_text());
            return;
        }
        Err(error) => {
            eprintln!("error: {error}\n");
            eprint!("{}", help_text());
            std::process::exit(2);
        }
    };

    // The configuration files (`config/*.toml`; see `src/config/`). Same contract again: a bad
    // file stops the run, naming the file and the key, rather than running on a setting that
    // silently did nothing.
    let dir = config_dir();
    let config = match Config::load(&dir) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };

    // Same contract as a bad argument: a misspelt switch stops the run rather than quietly
    // benchmarking a configuration nobody asked for. The variables override
    // `config/performance.toml` for this run.
    let performance = match PerformanceSettings::from_env_over(config.performance) {
        Ok(performance) => performance,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };

    point_bevy_at_the_assets_directory();
    run(selection, dir, config, performance);
}

/// Make `assets/` findable however the binary was started.
///
/// # The bug this fixes
///
/// Bevy resolves its asset root **relative to the executable**, so `./target/release/physics-sandbox`
/// looks for `target/release/assets/shaders/eclipse.wgsl`, does not find it, and logs
/// `Path not found`. `cargo run` happens to work because it sets the working directory to the crate
/// root — so the eclipse shader loaded for every test and every launch during development and failed
/// only when the binary was run directly. That is the worst shape a path bug can have.
///
/// Worse, it fails *quietly* from the window's point of view: a material whose fragment shader never
/// loads still draws, so the symptom is "the shadows stopped working" with the reason in a log line
/// nobody was reading.
///
/// # Why an environment variable
///
/// `BEVY_ASSET_ROOT` is the documented override and `AssetPlugin` reads it at build time, so setting
/// it here — before `App::new` — needs no change to the plugin configuration and cannot disagree with
/// it. The alternative, `AssetPlugin { file_path }`, would bake one absolute path into the binary.
///
/// Only set when [`CARGO_MANIFEST_DIR`](std::env) actually contains an `assets` directory, and never
/// when the caller has already set the variable: an explicit choice by whoever ran the program
/// outranks a compile-time guess, and a binary copied away from its source tree should fall back to
/// Bevy's own exe-relative behaviour rather than to a path that no longer exists.
fn point_bevy_at_the_assets_directory() {
    if std::env::var_os("BEVY_ASSET_ROOT").is_some() {
        return;
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    if root.join("assets").is_dir() {
        std::env::set_var("BEVY_ASSET_ROOT", root);
    }
}

/// Build and run the app for `selection`, with the settings in `config` and the switches in
/// `performance`.
///
/// Split from `main` so the argument handling above reads as a decision and this reads
/// as the wiring it always was.
fn run(selection: Selection, dir: PathBuf, config: Config, performance: PerformanceSettings) {
    let mut app = App::new();
    // Before any plugin: each one `init_resource`s its settings, which keeps a resource that is
    // already there, so inserting the configured ones first is what makes them take effect.
    config.insert_into(&mut app);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: WINDOW_TITLE.into(),
            present_mode: performance.present_mode(),
            // Full-screen on launch. `resolution` still matters: it is the size the
            // window takes if `WINDOW_MODE` is ever set back to `Windowed`, and it is
            // the aspect ratio every camera-framing constant was derived against.
            mode: WINDOW_MODE,
            resolution: (WINDOW_WIDTH_PX, WINDOW_HEIGHT_PX).into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(ScenePlugin::new(selection))
    // The tracing grid goes after `ScenePlugin`, which inserts the `Scene` the grid
    // measures its floor from.
    .add_plugins(FloorGridPlugin)
    // Motion trails, after `ScenePlugin` for the same reason as the grid: it reads the
    // `Scene` that plugin inserts.
    .add_plugins(TrailPlugin)
    // The HUD as a whole: `H` hides the debug panel, crosshair, compass, and picker.
    .add_plugins(HudPlugin)
    .add_plugins(AccretionDiscPlugin)
    // The hole ray-marched on the GPU, after the disc, whose CPU drawing it switches off.
    .add_plugins(BlackHoleViewPlugin)
    .add_plugins(FreeFlyCameraPlugin)
    .add_plugins(DebugUiPlugin)
    .add_plugins(CrosshairPlugin)
    // The orientation compass, top-right. Like the crosshair, it guards against
    // double-adding `EguiPlugin`, so plugin order among the egui users is free.
    .add_plugins(CompassPlugin)
    // Clock control (pause, single-step, speed) goes after `ScenePlugin`, which is
    // what inserts the `SimulationClock` these keys mutate.
    .add_plugins(SimulationControlsPlugin)
    // The `Esc` menu, in two halves: the input mode and the key binding, then the panel
    // that draws it. `MenuInputPlugin` goes before the camera's plugin only for
    // readability — it declares its own ordering against `free_fly_camera_system`, so
    // the gate closes on the same frame as the keypress whatever order these are added.
    .add_plugins(MenuInputPlugin)
    .add_plugins(MenuPanelPlugin)
    // The object placer, in three halves for the same reason the menu is in two: the
    // keys are input, the picker is UI, and only `render` may write to the `Scene`.
    .add_plugins(PlacementInputPlugin)
    .add_plugins(PlacementPanelPlugin)
    .add_plugins(PlacementPlugin)
    .add_plugins(LaunchInputPlugin)
    .add_plugins(MissionPlugin)
    // The report shown when a spacecraft lands, after the mission that flies it.
    .add_plugins(ArrivalPopupPlugin)
    // Writes a page of the menu to its file in `config/` when the menu asks.
    .add_plugins(ConfigPlugin::new(dir))
    // Anti-aliasing, the floor-eclipse switch, and the benchmark; see `src/performance/`.
    .add_plugins(PerformancePlugin::new(performance))
    .run();
}
