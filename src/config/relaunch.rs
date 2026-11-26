//! Starting another run from the menu: Restart, and a run picked on the Experiments page.
//!
//! The menu posts a [`Selection`] in [`MenuRequests`]; [`relaunch_requested_system`] starts the
//! program again with that selection's arguments ([`Selection::to_args`]) and closes this one.
//!
//! # Why a new process
//!
//! A run is built once, at startup, by two dozen plugins: the scene and its clocks, the mirrored
//! entities, the GPU's particle buffers, trails, the black hole's and wormhole's views, the
//! mission, each with startup systems and per-system state. Rebuilding all of that in place would
//! mean every one of them learning to tear itself down, and the first one that forgot would leave
//! stale state in the new run. Starting the program again gets the same clean start a launch from
//! the shell does, and costs a second of window and GPU set-up.
//!
//! # The settings come with it, saved or not
//!
//! The menu changes the running settings as you edit them, and Save only writes them to disk. So
//! that a restart does not quietly undo an unsaved edit, every page is written to a fresh
//! directory in the system's temporary directory (named [`RELAUNCH_DIR_PREFIX`]…), and the new run
//! is told where with [`RELAUNCH_CONFIG_VAR`]. `main.rs` reads that directory ahead of the saved
//! and shipped ones ([`take_relaunch_dir`]), then deletes it ([`discard_relaunch_dir`]). A setting
//! that applies "from the next launch", such as the particle count, therefore applies on Restart.
//!
//! The command-line environment comes along too, since the new process inherits it: the
//! `RGA_*` switches still override `performance.toml`, as they did for this run.
//!
//! # What does not
//!
//! The run's own state: the clock, the bodies' positions, spacecraft in flight, placed objects.
//! Restart starts the experiment from the beginning, with the same seed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::app::AppExit;
use bevy::ecs::event::EventWriter;
use bevy::ecs::system::ResMut;

use super::{write_page, ConfigError, Configured};
use crate::config::constants::{RELAUNCH_CONFIG_VAR, RELAUNCH_DIR_PREFIX};
use crate::projects::interstellar::selection::Selection;
use crate::ui::menu::{MenuRequests, SaveTarget};

/// Write every page of the running configuration into `dir`, as Save would one at a time.
pub fn write_all(dir: &Path, configured: &Configured) -> Result<(), ConfigError> {
    for target in SaveTarget::ALL {
        write_page(dir, target, configured)?;
    }
    Ok(())
}

/// A new directory name for a relaunch's settings: in the system's temporary directory, named
/// with [`RELAUNCH_DIR_PREFIX`], this process's id, the time and a count, so two relaunches never
/// share one. (The count because Windows' clock can return the same time to two calls in a row.)
/// Not created; [`write_all`] creates it.
pub fn fresh_relaunch_dir() -> PathBuf {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let count = COUNT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    std::env::temp_dir().join(format!(
        "{RELAUNCH_DIR_PREFIX}{}-{nanos}-{count}",
        std::process::id()
    ))
}

/// Is `dir` one a relaunch made, and so safe to delete? Only a directory directly inside the
/// system's temporary directory whose name starts with [`RELAUNCH_DIR_PREFIX`].
///
/// The check is what keeps [`RELAUNCH_CONFIG_VAR`], set by hand to a directory of the user's, from
/// deleting it.
pub fn is_relaunch_dir(dir: &Path) -> bool {
    let named = dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(RELAUNCH_DIR_PREFIX));
    named && dir.parent() == Some(std::env::temp_dir().as_path())
}

/// The directory the run that started this one left its settings in, if it was started by a
/// relaunch. Clears [`RELAUNCH_CONFIG_VAR`], so a program this one starts does not inherit it.
///
/// For `main.rs`, before it loads the configuration.
pub fn take_relaunch_dir() -> Option<PathBuf> {
    let dir = std::env::var_os(RELAUNCH_CONFIG_VAR)?;
    std::env::remove_var(RELAUNCH_CONFIG_VAR);
    Some(PathBuf::from(dir))
}

/// Delete `dir` once its settings are read, if a relaunch made it ([`is_relaunch_dir`]).
/// A directory that cannot be deleted is left in the temporary directory, which is not worth
/// stopping the run over.
pub fn discard_relaunch_dir(dir: &Path) {
    if is_relaunch_dir(dir) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Start the program again with `selection`, carrying the running configuration. Returns once
/// the new process has started; closing this one is the caller's job.
fn relaunch(selection: &Selection, configured: &Configured) -> Result<(), String> {
    let dir = fresh_relaunch_dir();
    let started = write_all(&dir, configured)
        .map_err(|error| error.to_string())
        .and_then(|()| std::env::current_exe().map_err(|error| error.to_string()))
        .and_then(|exe| {
            Command::new(exe)
                .args(selection.to_args())
                .env(RELAUNCH_CONFIG_VAR, &dir)
                .spawn()
                .map_err(|error| error.to_string())
        });
    if started.is_err() {
        discard_relaunch_dir(&dir);
    }
    started.map(|_| ())
}

/// Start the run the menu asked for, if it asked, and close this one.
///
/// If the new process cannot be started, this one keeps running and the menu says why
/// ([`MenuRequests::status`]).
pub fn relaunch_requested_system(
    mut requests: ResMut<MenuRequests>,
    configured: Configured,
    mut exit: EventWriter<AppExit>,
) {
    let Some(selection) = requests.take_relaunch() else {
        return;
    };
    match relaunch(&selection, &configured) {
        Ok(()) => {
            requests.set_status(format!("Starting {}…", selection.id.name()));
            exit.send(AppExit::Success);
        }
        Err(error) => {
            requests.set_status(format!("Could not start {}: {error}", selection.id.name()));
        }
    }
}

#[cfg(test)]
#[path = "../../tst/config/relaunch_tests.rs"]
mod tests;
