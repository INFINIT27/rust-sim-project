//! Tests for [`crate::config::relaunch`], `#[path]`-included as a unit-test module of it.
//!
//! Starting the program again is not tested here: a test that spawned the binary would open a
//! full-screen window. What is tested is what that start depends on: every setting changed in
//! the menu, saved or not, reaches the new run, and the directory carrying them is deleted only
//! when a relaunch made it. That the arguments select the same run again is
//! `to_args_parses_back_to_the_same_selection`, in the selection tests.

use super::*;
use crate::bindings::{Action, KeyMap};
use crate::compute::particles::GpuSolver;
use crate::config::Config;
use crate::performance::{AntiAliasing, PerformanceSettings};
use bevy::app::App;
use bevy::ecs::system::RunSystemOnce;
use bevy::input::keyboard::KeyCode;

/// A fresh, empty directory under the target directory, for files written by a test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("relaunch-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A configuration with a setting changed on every page that has one easy to change, as a run
/// whose menu had been used would have.
fn edited() -> (Config, PerformanceSettings) {
    let mut config = Config::default();
    config.camera.move_speed = 9.0;
    config.trails.visible = !config.trails.visible;
    config.collision.debris_fragments = 0;
    config.solver = GpuSolver::SCALABLE;
    let mut keymap = KeyMap::default();
    keymap.set(Action::Pause, KeyCode::KeyK);
    config.keymap = keymap;
    let performance = PerformanceSettings {
        anti_aliasing: AntiAliasing::Fxaa,
        vsync: false,
        ..PerformanceSettings::default()
    };
    (config, performance)
}

#[test]
fn every_setting_the_running_app_has_reaches_the_relaunched_one() {
    let (config, performance) = edited();
    assert_ne!(config, Config::default(), "the test must change something");

    let mut app = App::new();
    config.clone().insert_into(&mut app);
    app.insert_resource(performance);

    let dir = scratch("every_setting");
    let written = dir.clone();
    app.world_mut()
        .run_system_once(move |configured: Configured| write_all(&written, &configured))
        .unwrap();

    // The relaunched run reads this directory ahead of the others, so what it gets is exactly
    // what was written.
    let reloaded = Config::load(&dir).unwrap();
    assert_eq!(reloaded.camera, config.camera);
    assert_eq!(reloaded.trails, config.trails);
    assert_eq!(reloaded.collision, config.collision);
    assert_eq!(reloaded.solver, config.solver);
    assert_eq!(reloaded.keymap, config.keymap);
    assert_eq!(reloaded.fleet, config.fleet);
    assert_eq!(reloaded.performance, performance);
    assert_eq!(
        Config {
            performance,
            ..config
        },
        reloaded,
        "a setting was lost on the way to the relaunched run"
    );
}

#[test]
fn a_directory_a_relaunch_made_is_recognised_and_deleted() {
    let dir = fresh_relaunch_dir();
    assert!(is_relaunch_dir(&dir), "{}", dir.display());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("camera.toml"), "").unwrap();

    discard_relaunch_dir(&dir);
    assert!(!dir.exists(), "{} was not deleted", dir.display());
}

#[test]
fn a_directory_of_the_users_is_never_deleted() {
    // Named like a relaunch's, but not in the temporary directory.
    let theirs = scratch(&format!("{RELAUNCH_DIR_PREFIX}mine"));
    assert!(!is_relaunch_dir(&theirs), "{}", theirs.display());
    discard_relaunch_dir(&theirs);
    assert!(theirs.exists());

    // In the temporary directory, but not named like one.
    let elsewhere = std::env::temp_dir().join("physics-sandbox-not-a-relaunch");
    assert!(!is_relaunch_dir(&elsewhere));

    // Inside a relaunch's directory rather than the directory itself.
    assert!(!is_relaunch_dir(&fresh_relaunch_dir().join("nested")));
}

#[test]
fn two_relaunches_never_share_a_directory() {
    assert_ne!(fresh_relaunch_dir(), fresh_relaunch_dir());
}
