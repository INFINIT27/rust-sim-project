//! The placement picker: the list `B` puts on screen.
//!
//! Draws what [`crate::input::placement`] has selected. Like every other module in this
//! layer it only reads — the selection is changed by the arrow keys, not by clicking a row,
//! which is what keeps this panel out of the interactive-widget exception the `Esc` menu
//! needed (see [`crate::ui::menu`] and Requirement 11.4).
//!
//! # Why keys rather than clicks
//!
//! The picker is open **while you fly**: it is not modal, because the object is placed in
//! front of the camera and you have to be able to aim. So the cursor is still captured for
//! mouse-look, and there is nothing to click *with*. Arrow keys are the only input that makes
//! sense here, and that in turn keeps the panel non-interactive, which keeps this layer's
//! central rule intact for everything except the menu.
//!
//! # Placed bottom-centre, not in a corner
//!
//! The corners are taken — the debug overlay top-left, the compass top-right — and the centre
//! of the view has the crosshair, which is exactly where the object is about to appear. The
//! bottom edge is what is left, and it is where a HUD of this kind belongs anyway: close
//! enough to the crosshair to read both without moving your eyes, far enough not to cover the
//! thing you are aiming at.
//!
//! # What each row says
//!
//! The name, the mass in solar masses, and the object's own one-line note — because the
//! interesting thing about these objects is precisely the numbers: `1.4 M☉` in a `22 km` ball,
//! or an event horizon that is the one object in this crate drawn true to scale. A list of
//! five names alone would be a list of five names.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::Res;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::experiments::constants::STELLAR_OBJECTS;
use crate::input::menu::MenuState;
use crate::input::placement::PlacementState;
use crate::ui::constants::{PLACEMENT_PANEL_MARGIN_PX, PLACEMENT_PANEL_WIDTH_PX};

/// One row of the picker: the label, and whether it is the current selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementRow {
    /// The object's name.
    pub name: &'static str,
    /// Mass and the object's note, formatted for display.
    pub detail: String,
    /// Is this the row `Enter` would place?
    pub selected: bool,
}

/// A mass in solar masses, formatted for one line of a picker row.
///
/// Fixed-point below a thousand and scientific above it, because the catalog spans `0.122` to
/// `4.3e6` solar masses and neither format reads well across that whole range: `0.122` is
/// clearer than `1.220e-1`, and `4.297e6` is clearer — and 4 characters shorter — than
/// `4297134.238`. The switch keeps the longest row inside
/// [`PLACEMENT_PANEL_WIDTH_PX`](crate::ui::constants::PLACEMENT_PANEL_WIDTH_PX), which a test
/// checks.
fn solar_mass_label(solar_masses: f64) -> String {
    if solar_masses >= 1000.0 {
        format!("{solar_masses:.3e} M☉")
    } else {
        format!("{solar_masses:.3} M☉")
    }
}

/// The picker's rows, in catalog order, with the selection marked.
///
/// Pure, so the panel's whole content is testable without a window — the same split every
/// egui module here uses. See `tst/ui/README.md`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{placement_rows, PlacementState};
///
/// let mut placer = PlacementState::default();
/// placer.open();
/// placer.select_next();
///
/// let rows = placement_rows(&placer);
/// assert_eq!(rows.iter().filter(|row| row.selected).count(), 1);
/// assert!(rows[1].selected, "Down moved the selection to the second row");
/// ```
pub fn placement_rows(placer: &PlacementState) -> Vec<PlacementRow> {
    let selected = placer.selected_index();

    STELLAR_OBJECTS
        .iter()
        .enumerate()
        .map(|(index, object)| PlacementRow {
            name: object.name,
            detail: format!(
                "{} — {}",
                solar_mass_label(object.solar_masses()),
                object.note
            ),
            selected: index == selected,
        })
        .collect()
}

/// Paint the picker, when it is open.
///
/// Draws nothing while [`PlacementState::is_open`] is `false`, and nothing while the `Esc`
/// menu is open — the menu is modal and closes the picker anyway, but the check makes the
/// draw order irrelevant rather than load-bearing.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window already has an
/// egui context, so this system needs a real windowed app. See `tst/ui/README.md`.
pub fn placement_panel_system(
    mut contexts: EguiContexts,
    placer: Res<PlacementState>,
    menu: Option<Res<MenuState>>,
) {
    if !placer.is_open() || menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }

    let ctx = contexts.ctx_mut();

    egui::Window::new("Place an object")
        .default_width(PLACEMENT_PANEL_WIDTH_PX)
        .collapsible(false)
        .resizable(false)
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(0.0, -PLACEMENT_PANEL_MARGIN_PX),
        )
        .show(ctx, |ui| {
            for row in placement_rows(&placer) {
                // A leading marker rather than a highlight colour: the panel is drawn over a
                // scene of arbitrary brightness, and "which row is selected" must not depend
                // on what is behind it.
                let marker = if row.selected { "▸" } else { "  " };
                let text = egui::RichText::new(format!("{marker} {:<14} {}", row.name, row.detail));

                ui.label(if row.selected {
                    text.strong()
                } else {
                    text.weak()
                });
            }

            ui.separator();
            ui.label(
                egui::RichText::new("Up / Down to choose   Enter to place   B to cancel").weak(),
            );
        });
}

/// Draws the placement picker while `B` has it open.
///
/// Adds `EguiPlugin` if it is not already present, so this composes with the overlay, the
/// crosshair, the compass, and the menu in any order. Inserts [`PlacementState`] only so that
/// adding the panel without [`PlacementInputPlugin`](crate::input::placement::PlacementInputPlugin)
/// still runs.
pub struct PlacementPanelPlugin;

impl Plugin for PlacementPanelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }

        app.init_resource::<PlacementState>();
        app.add_systems(
            Update,
            placement_panel_system.run_if(crate::ui::hud::hud_shown),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/ui/placement_tests.rs"]
mod tests;
