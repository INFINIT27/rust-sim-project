//! The pop-up that appears when a spacecraft lands: how long it took, how fast it went, and what it
//! cost.
//!
//! It reads the landed flight's [`FlightReport`] from the
//! [`Mission`] and shows it for as long as the craft sits landed: the next `R` starts picking a new
//! route, which ends that phase and closes the pop-up. It is deliberately not interactive. A click
//! anywhere in the window captures the cursor for mouse-look (see `input/camera.rs`), so a button
//! here would grab the cursor as it was pressed, and while flying the cursor is usually captured
//! already. The rows are built by [`arrival_rows`], a pure function, so they can be tested without
//! a window.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::Res;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::core::scene::BodyId;
use crate::experiments::catalog::BodyNames;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::render::mission::{Mission, MissionPhase};
use crate::spacecraft::FlightReport;
use crate::ui::constants::{
    ARRIVAL_HINT, ARRIVAL_LABEL_COLUMN_PX, ARRIVAL_POPUP_WIDTH_PX, JOULES_PER_TONNE_OF_TNT,
};

/// A duration in the largest units that read naturally: `"4 d 14 h"`, `"6 h 12 min"`,
/// `"17 y 3 d"`, `"42 s"`.
pub fn duration_label(seconds: f64) -> String {
    const MINUTE: f64 = 60.0;
    const HOUR: f64 = 3_600.0;
    const DAY: f64 = 86_400.0;
    const YEAR: f64 = 365.25 * DAY;
    let s = seconds.max(0.0);
    if s >= YEAR {
        format!(
            "{:.0} y {:.0} d",
            (s / YEAR).floor(),
            (s % YEAR / DAY).floor()
        )
    } else if s >= DAY {
        format!(
            "{:.0} d {:.0} h",
            (s / DAY).floor(),
            (s % DAY / HOUR).floor()
        )
    } else if s >= HOUR {
        format!(
            "{:.0} h {:.0} min",
            (s / HOUR).floor(),
            (s % HOUR / MINUTE).floor()
        )
    } else if s >= MINUTE {
        format!(
            "{:.0} min {:.0} s",
            (s / MINUTE).floor(),
            (s % MINUTE).floor()
        )
    } else {
        format!("{s:.0} s")
    }
}

/// An energy with an SI prefix and its equivalent in TNT: `"8.31 TJ (1.99 kt of TNT)"`.
pub fn energy_label(joules: f64) -> String {
    const PREFIXES: [(f64, &str); 6] = [
        (1.0e18, "EJ"),
        (1.0e15, "PJ"),
        (1.0e12, "TJ"),
        (1.0e9, "GJ"),
        (1.0e6, "MJ"),
        (1.0e3, "kJ"),
    ];
    let energy = PREFIXES
        .iter()
        .find(|(scale, _)| joules >= *scale)
        .map_or(format!("{joules:.0} J"), |(scale, unit)| {
            format!("{:.2} {unit}", joules / scale)
        });
    let tonnes = joules / JOULES_PER_TONNE_OF_TNT;
    let tnt = if tonnes >= 1.0e6 {
        format!("{:.2} Mt", tonnes / 1.0e6)
    } else if tonnes >= 1.0e3 {
        format!("{:.2} kt", tonnes / 1.0e3)
    } else {
        format!("{tonnes:.2} t")
    };
    format!("{energy} ({tnt} of TNT)")
}

/// A mass in kilograms or tonnes: `"620 kg"`, `"12,400 t"`.
fn mass_label(kg: f64) -> String {
    if kg >= 1_000.0 {
        format!("{} t", thousands((kg / 1_000.0).round()))
    } else {
        format!("{kg:.0} kg")
    }
}

/// An integer-valued number with thousands separators.
fn thousands(value: f64) -> String {
    let digits = format!("{:.0}", value.abs());
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if value < 0.0 {
        format!("-{out}")
    } else {
        out
    }
}

/// The pop-up's rows for a flight from `origin` to `target` that ended with `report`: a label and
/// a value each, in display order. Rows the flight has no number for are left out: a craft with no
/// modelled engine (a coasting probe, whose report has no energy) gets no engine rows.
pub fn arrival_rows(
    report: &FlightReport,
    origin: BodyId,
    target: BodyId,
    names: &BodyNames,
) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        ("Spacecraft", report.craft_name.clone()),
        (
            "Route",
            format!("{} → {}", names.label(origin), names.label(target)),
        ),
    ];
    if let Some(time) = report.travel_time_s {
        rows.push(("Time of travel", duration_label(time)));
    }
    rows.push((
        "Maximum speed",
        format!(
            "{:.1} km/s ({:.3}% of light)",
            report.max_speed_m_s / 1_000.0,
            100.0 * report.max_speed_m_s / SPEED_OF_LIGHT_M_S
        ),
    ));
    rows.push((
        "Speed change (Δv)",
        format!("{:.2} km/s", report.delta_v_m_s / 1_000.0),
    ));
    if report.energy_j.is_some() {
        rows.push((
            "Engine time at full thrust",
            duration_label(report.burn_time_s),
        ));
        rows.push((
            "Peak acceleration",
            format!(
                "{:.2} g",
                report.peak_acceleration_m_s2 / STANDARD_GRAVITY_M_S2
            ),
        ));
    }
    if let Some(energy) = report.energy_j {
        rows.push(("Energy used", energy_label(energy)));
    }
    if let Some(propellant) = report.propellant_kg {
        rows.push(("Propellant burned", mass_label(propellant)));
    }
    if let Some(touchdown) = report.touchdown_speed_m_s {
        rows.push(("Touchdown speed", format!("{touchdown:.1} m/s")));
    }
    rows.push(("Correction burns", report.corrections.to_string()));
    rows
}

/// Paint the arrival pop-up while the mission's craft is landed.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window already has an egui
/// context, so this system needs a real windowed app. See `tst/ui/README.md`.
pub fn arrival_popup_system(
    mut contexts: EguiContexts,
    mission: Option<Res<Mission>>,
    names: Option<Res<BodyNames>>,
) {
    let Some(mission) = mission else {
        return;
    };
    let MissionPhase::Landed { flight, .. } = &mission.phase else {
        return;
    };
    let names = names.map(|names| names.clone()).unwrap_or_default();
    let rows = arrival_rows(
        flight.report(),
        flight.voyage.origin,
        flight.voyage.target,
        &names,
    );
    let ctx = contexts.ctx_mut();
    egui::Window::new(format!("Arrived at {}", names.label(flight.voyage.target)))
        .default_width(ARRIVAL_POPUP_WIDTH_PX)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Grid::new("arrival_report")
                .num_columns(2)
                .min_col_width(ARRIVAL_LABEL_COLUMN_PX)
                .show(ui, |ui| {
                    for (label, value) in &rows {
                        ui.label(*label);
                        ui.label(value);
                        ui.end_row();
                    }
                });
            ui.separator();
            ui.label(ARRIVAL_HINT);
        });
}

/// Plugin that shows the arrival pop-up. Needs [`MissionPlugin`](crate::render::mission::MissionPlugin)
/// for the flight it reports on.
pub struct ArrivalPopupPlugin;

impl Plugin for ArrivalPopupPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }
        app.add_systems(Update, arrival_popup_system);
    }
}

#[cfg(test)]
#[path = "../../tst/ui/arrival_tests.rs"]
mod tests;
