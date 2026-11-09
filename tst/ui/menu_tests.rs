//! Unit tests for the [`crate::ui::menu`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/ui/menu.rs` via `#[path]`,
//! not as a standalone integration test.
//!
//! # Why nothing here calls `menu_system`
//!
//! `EguiContexts::ctx_mut()` requires the primary window to already have an egui context,
//! which needs a real window — the same limitation the overlay, crosshair, and compass
//! tests live with (see `tst/ui/README.md`). So the panel's *content* is produced by pure
//! functions and everything worth asserting is asserted against those: [`settings_rows`]
//! and [`binding_rows`] carry the tests, and the painting is kept thin enough to read.
//!
//! What that does not cover is the layout — whether the two grids line up, whether the
//! panel fits its width. `cargo run --release` is the only thing that answers those.

use super::*;
use crate::experiments::catalog::ExperimentId;
use crate::render::trail::TrailConfig;
use crate::time::simulation::SimulationClock;

/// The spec, clock, and trail config for a default two-body run.
fn shipped() -> (ExperimentSpec, SimulationClock, TrailConfig) {
    let spec = ExperimentId::TwoBody.build_default().spec;
    let clock = SimulationClock::new(spec.time_scale);
    (spec, clock, TrailConfig::default())
}

fn value_of(rows: &[SettingsRow], label: &str) -> String {
    rows.iter()
        .find(|row| row.label == label)
        .unwrap_or_else(|| panic!("no `{label}` row"))
        .value
        .clone()
}

// -----------------------------------------------------------------------
// The settings list.
// -----------------------------------------------------------------------

#[test]
fn the_settings_list_names_the_running_experiment_and_its_seed() {
    // The seed is what makes a run repeatable, so it has to be somewhere you can read it
    // without knowing to look at the overlay's `exp` row.
    let (spec, clock, trails) = shipped();
    let rows = settings_rows(&spec, &clock, &trails);

    let experiment = value_of(&rows, "Experiment");
    assert!(experiment.contains(spec.id.name()));
    assert!(experiment.contains(spec.id.slug()));
    assert_eq!(value_of(&rows, "Seed"), format!("{}", spec.seed));
}

#[test]
fn every_row_has_a_label_and_a_value() {
    let (spec, clock, trails) = shipped();

    for row in settings_rows(&spec, &clock, &trails) {
        assert!(!row.label.trim().is_empty());
        assert!(
            !row.value.trim().is_empty(),
            "`{}` has no value, which draws as an unexplained gap",
            row.label
        );
    }
}

#[test]
fn no_two_rows_share_a_label() {
    // Two rows with one label is a reader's worst case: both look authoritative and only
    // one is the setting they were looking for.
    let (spec, clock, trails) = shipped();
    let rows = settings_rows(&spec, &clock, &trails);

    for (index, row) in rows.iter().enumerate() {
        for other in &rows[index + 1..] {
            assert_ne!(row.label, other.label, "`{}` appears twice", row.label);
        }
    }
}

#[test]
fn the_list_reports_the_pause_state_rather_than_assuming_it() {
    // `P` is easy to hit by accident and its effect is invisible in a still frame, which is
    // the whole reason this row exists.
    let (spec, mut clock, trails) = shipped();
    assert_eq!(
        value_of(&settings_rows(&spec, &clock, &trails), "Simulation"),
        "Running"
    );

    clock.toggle_paused();
    assert_eq!(
        value_of(&settings_rows(&spec, &clock, &trails), "Simulation"),
        "Paused"
    );
}

#[test]
fn the_list_follows_the_clock_scale() {
    // Not a constant: `]` and `[` change it at runtime, and the menu must show what the
    // clock actually holds rather than what the experiment asked for.
    let (spec, mut clock, trails) = shipped();
    let before = value_of(&settings_rows(&spec, &clock, &trails), "Time scale");

    clock.multiply_scale(2.0);
    let after = value_of(&settings_rows(&spec, &clock, &trails), "Time scale");

    assert_ne!(before, after, "the row must track the live scale");
}

#[test]
fn the_list_reports_the_trail_setting_both_ways() {
    // `T` is a toggle with no other on-screen indicator, so this row is the only way to
    // tell "trails are off" from "nothing is moving".
    let (spec, clock, mut trails) = shipped();

    let on = value_of(&settings_rows(&spec, &clock, &trails), "Trails");
    assert!(on.starts_with("On"));
    assert!(
        on.contains(&format!("{}", trails.length_units)),
        "an experiment can change the length, so show it: {on:?}"
    );

    trails.visible = false;
    assert_eq!(
        value_of(&settings_rows(&spec, &clock, &trails), "Trails"),
        "Off"
    );
}

#[test]
fn the_list_states_the_unit_scale() {
    // Every length in the app is in world units, and the grid is measured in them. Without
    // this row the numbers in the overlay have no stated meaning.
    let (spec, clock, trails) = shipped();
    let value = value_of(&settings_rows(&spec, &clock, &trails), "Unit scale");

    assert!(value.contains("1 unit"));
    assert!(
        value.contains('m'),
        "the scale has to name a real unit: {value:?}"
    );
}

#[test]
fn the_list_describes_every_experiment_in_the_catalog() {
    // The menu is per-run, so it must work for whichever experiment is running — including
    // one added later. This is the loop that makes that automatic.
    for id in ExperimentId::ALL {
        let spec = id.build_default().spec;
        let clock = SimulationClock::new(spec.time_scale);
        let rows = settings_rows(&spec, &clock, &TrailConfig::default());

        assert!(
            value_of(&rows, "Experiment").contains(id.slug()),
            "{id:?} is not named in its own menu"
        );
    }
}

// -----------------------------------------------------------------------
// The bindings table.
// -----------------------------------------------------------------------

#[test]
fn the_panel_would_show_every_binding() {
    // The one thing that can actually be wrong about a list of labels: a dropped row.
    let drawn: usize = binding_rows().iter().map(|(_, rows)| rows.len()).sum();

    assert_eq!(drawn, crate::constants::KEY_BINDINGS.len());
}

#[test]
fn the_groups_are_titled_and_never_empty() {
    for (title, rows) in binding_rows() {
        assert!(!title.trim().is_empty());
        assert!(!rows.is_empty(), "`{title}` would draw as a bare heading");
    }
}

#[test]
fn the_groups_are_in_display_order() {
    let titles: Vec<&str> = binding_rows().into_iter().map(|(title, _)| title).collect();
    let expected: Vec<&str> = crate::bindings::BindingGroup::ALL
        .into_iter()
        .map(|group| group.title())
        .collect();

    assert_eq!(titles, expected);
}

#[test]
fn the_menu_tells_you_how_to_close_it() {
    // A modal panel that does not say how to dismiss it is a trap — especially this one,
    // since the window is full-screen with no chrome to fall back on.
    let escape = binding_rows()
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .find(|(keys, _)| *keys == "Esc");

    assert!(escape.is_some(), "the panel must list its own key");
}

#[test]
fn the_panel_is_wide_enough_for_its_longest_line() {
    use crate::ui::constants::{MENU_KEY_COLUMN_PX, MENU_WIDTH_PX};

    // A crude but real check on the one layout number a test can reason about: at egui's
    // default body size a character is roughly 7 px wide, so the longest action has to fit
    // in what is left of the panel after the key column. Wrong, and the table wraps into
    // ragged prose — which `MENU_WIDTH_PX` was chosen to prevent.
    let longest = crate::constants::KEY_BINDINGS
        .iter()
        .map(|binding| binding.action.chars().count())
        .max()
        .unwrap_or(0);

    let available = MENU_WIDTH_PX - MENU_KEY_COLUMN_PX;
    assert!(
        (longest as f32) * 7.0 <= available * 1.05,
        "the longest action is {longest} characters but only {available} px are left for it"
    );
}
