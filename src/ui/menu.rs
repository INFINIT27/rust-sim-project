//! The menu panel: what `Esc` puts on screen.
//!
//! Two halves in two layers, and this is the second one. [`crate::input::menu`] owns
//! whether the menu is open and the key that changes it; this module owns what it looks
//! like. Nothing here decides anything about input — it reads
//! [`crate::input::menu::MenuState`] and draws.
//!
//! # What it shows today, and how it grows
//!
//! Two sections: the **current settings** as a read-only list, and the **key bindings**
//! from [`KEY_BINDINGS`](crate::constants::KEY_BINDINGS). Both are produced by pure
//! functions — [`settings_rows`] and [`binding_rows`] — for the reason every egui module
//! here does the same: `EguiContexts::ctx_mut` needs a real window, so a system that
//! paints cannot be tested headlessly. Keeping the *content* pure moves everything worth
//! asserting out of the untestable part. See `tst/ui/README.md`.
//!
//! Adding an action later means adding a section, and the shape is already right: the panel
//! is a `Window` with interactive widgets allowed (see below), so a button that switches
//! experiments or a slider for the time scale drops in without revisiting the input gate.
//!
//! # This is the crate's first interactive widget, and that is a real change
//!
//! Every other module in this layer is careful *not* to claim input: the overlay uses
//! `ui.label` exclusively and the crosshair and compass paint onto a layer without
//! registering a widget at all, because Requirement 11.4 says the UI must not eat input
//! the camera needs. The menu has a button, so it does claim input — and the resolution is
//! not a `wants_pointer_input` check but the state itself: **while the menu is open, the
//! camera and the clock keys are inert anyway** (amendment A13). There is no contention to
//! arbitrate, because the two are never live at the same time.
//!
//! # It shows the settings; it does not own them
//!
//! Every value on the settings list is read from the resource that actually holds it — the
//! [`ExperimentSpec`], the [`SimulationClock`], the [`TrailConfig`]. The menu has no state
//! of its own beyond being open, so it cannot disagree with the app about what the app is
//! doing.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, ResMut};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::bindings::{bindings_in, BindingGroup};
use crate::constants::METRES_PER_UNIT;
use crate::experiments::catalog::ExperimentSpec;
use crate::input::menu::MenuState;
use crate::render::trail::TrailConfig;
use crate::time::simulation::SimulationClock;
use crate::ui::constants::{MENU_KEY_COLUMN_PX, MENU_SECTION_GAP_PX, MENU_WIDTH_PX};

/// One row of the settings list: a label and its current value.
///
/// A pair of owned `String`s rather than `&'static str`, because the values are formatted
/// numbers — the point of the list is that it shows what the app is doing *now*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsRow {
    /// What the setting is called.
    pub label: &'static str,
    /// Its current value, formatted for display.
    pub value: String,
}

/// The settings list: what is running, how fast, and what is being drawn.
///
/// Pure, so the whole content of the panel is testable without a window. Reads the same
/// resources the app runs on, so a row cannot disagree with reality.
///
/// # Why these rows
///
/// They are the answers to the questions the overlay does not already answer, plus the two
/// a user is most likely to have changed by accident with a stray keypress. `Time scale` and
/// `Simulation` are here because `]`, `[`, and `P` are easy to hit and their effect is
/// invisible in a still frame; `Trails` because `T` is a toggle with no other indicator;
/// `Seed` because it is what makes a run repeatable and it is otherwise only in the overlay.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{settings_rows, ExperimentId, SimulationClock, TrailConfig};
///
/// let spec = ExperimentId::TwoBody.build_default().spec;
/// let clock = SimulationClock::new(spec.time_scale);
/// let rows = settings_rows(&spec, &clock, &TrailConfig::default());
///
/// assert!(rows.iter().any(|row| row.label == "Experiment"));
/// assert!(rows.iter().any(|row| row.value == "Running"));
/// ```
pub fn settings_rows(
    spec: &ExperimentSpec,
    clock: &SimulationClock,
    trails: &TrailConfig,
) -> Vec<SettingsRow> {
    let row = |label: &'static str, value: String| SettingsRow { label, value };

    vec![
        row(
            "Experiment",
            format!("{} ({})", spec.id.name(), spec.id.slug()),
        ),
        row("Seed", format!("{}", spec.seed)),
        row(
            "Time scale",
            format!("{:.3e} simulated seconds per real second", clock.scale()),
        ),
        row(
            "Simulation",
            if clock.is_paused() {
                "Paused".to_string()
            } else {
                "Running".to_string()
            },
        ),
        row(
            "Elapsed",
            format!("{:.3e} simulated seconds", clock.elapsed_seconds()),
        ),
        row(
            "Trails",
            if trails.visible {
                format!("On, {} units of path", trails.length_units)
            } else {
                "Off".to_string()
            },
        ),
        row("Unit scale", format!("1 unit = {:.0e} m", METRES_PER_UNIT)),
    ]
}

/// The key bindings, grouped and in display order: `(group title, rows)`.
///
/// A thin regrouping of [`KEY_BINDINGS`](crate::constants::KEY_BINDINGS) into the shape the
/// panel draws. Pure, and it exists so a test can assert the panel would show every binding
/// — which is the one thing that can actually be wrong about a list of labels.
///
/// Empty groups are dropped rather than drawn as a bare heading.
///
/// # Examples
///
/// ```
/// use physics_sandbox::binding_rows;
///
/// let groups = binding_rows();
///
/// assert_eq!(groups[0].0, "Camera", "movement comes first");
/// assert!(groups.iter().all(|(_, rows)| !rows.is_empty()));
/// ```
pub fn binding_rows() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    BindingGroup::ALL
        .into_iter()
        .filter_map(|group| {
            let rows: Vec<_> = bindings_in(group)
                .map(|binding| (binding.keys, binding.action))
                .collect();

            (!rows.is_empty()).then_some((group.title(), rows))
        })
        .collect()
}

/// Paint the menu, when it is open.
///
/// Draws nothing at all while [`MenuState::is_open`] is `false`, which is the common case —
/// so the cost of the menu existing is one `bool` check per frame.
///
/// # The `Close` button, and why it may take input
///
/// It is the crate's only interactive widget. That is safe here, and only here, because the
/// camera and clock bindings are already inert while the menu is open — see the module docs
/// and amendment A13. It closes the menu by calling
/// [`MenuState::close`](crate::input::menu::MenuState::close), the same path `Esc` takes.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window already has an
/// egui context, so this system needs a real windowed app. See `tst/ui/README.md`.
pub fn menu_system(
    mut contexts: EguiContexts,
    mut menu: ResMut<MenuState>,
    spec: Res<ExperimentSpec>,
    clock: Res<SimulationClock>,
    trails: Res<TrailConfig>,
) {
    if !menu.is_open() {
        return;
    }

    let ctx = contexts.ctx_mut();
    let mut close_requested = false;

    egui::Window::new("Menu")
        .default_width(MENU_WIDTH_PX)
        .collapsible(false)
        .resizable(false)
        // Anchored to the centre: this is a modal panel, not a tool window, and the corners
        // are taken by the overlay (top-left) and the compass (top-right).
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.heading("Settings");
            egui::Grid::new("menu_settings")
                .num_columns(2)
                .min_col_width(MENU_KEY_COLUMN_PX)
                .show(ui, |ui| {
                    for row in settings_rows(&spec, &clock, &trails) {
                        ui.label(row.label);
                        ui.label(row.value);
                        ui.end_row();
                    }
                });

            ui.add_space(MENU_SECTION_GAP_PX);
            ui.separator();
            ui.heading("Key bindings");

            for (title, rows) in binding_rows() {
                ui.add_space(0.5 * MENU_SECTION_GAP_PX);
                ui.label(egui::RichText::new(title).strong());
                egui::Grid::new(format!("menu_bindings_{title}"))
                    .num_columns(2)
                    .min_col_width(MENU_KEY_COLUMN_PX)
                    .show(ui, |ui| {
                        for (keys, action) in rows {
                            ui.label(egui::RichText::new(keys).monospace());
                            ui.label(action);
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(MENU_SECTION_GAP_PX);
            ui.separator();
            if ui.button("Close (Esc)").clicked() {
                close_requested = true;
            }
        });

    // Applied after the closure so the `ResMut` borrow does not have to be threaded through
    // it. One frame of delay is not possible here — this is still the same system run.
    if close_requested {
        menu.close();
    }
}

/// Draws the menu when `Esc` has opened it.
///
/// Inserts nothing: [`MenuState`] is owned by
/// [`MenuInputPlugin`](crate::input::menu::MenuInputPlugin), and this plugin
/// `init_resource`s it only so that adding the panel without the input half still runs. Adds
/// `EguiPlugin` if it is not already present, so this composes with the overlay, the
/// crosshair, and the compass in any order.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use physics_sandbox::{MenuInputPlugin, MenuPanelPlugin, ScenePlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(ScenePlugin::default())
///     .add_plugins(MenuInputPlugin)
///     .add_plugins(MenuPanelPlugin)
///     .run();
/// ```
pub struct MenuPanelPlugin;

impl Plugin for MenuPanelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }

        app.init_resource::<MenuState>();
        app.add_systems(Update, menu_system);
    }
}

#[cfg(test)]
#[path = "../../tst/ui/menu_tests.rs"]
mod tests;
