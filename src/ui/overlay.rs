//! Debug UI overlay module (task 9.1).
//!
//! This module is the Bevy-side home for [`DebugUiPlugin`]: a small,
//! read-only `bevy_egui` overlay that shows FPS, the number of props
//! currently in `Scene`, and the active camera's `translation` and
//! `rotation` each frame. It is strictly diagnostic — Requirement 11.3
//! forbids it from mutating the `Scene`, any `FreeFlyController`, or
//! any `Transform`, and Requirement 11.4 forbids it from consuming or
//! routing keyboard or mouse input that the `FreeFlyCameraPlugin`
//! needs.
//!
//! # Why a standalone `overlay` module
//!
//! The `bevy_egui` dependency enters the crate here. Keeping it in its
//! own module keeps the `render` and `input` layers free of egui's
//! surface area, so a future refactor that
//! swaps the overlay for a different UI crate (e.g. a custom Bevy UI
//! panel) touches this file only.
//!
//! # FPS computation
//!
//! The task description calls for "a frame-time diagnostic computed
//! from `Time::delta_seconds()` or `FrameTimeDiagnosticsPlugin` if
//! already available". `FrameTimeDiagnosticsPlugin` introduces an
//! implicit plugin-ordering dependency on anyone using
//! `DebugUiPlugin`, so we go the simpler route and derive FPS from
//! `Time::delta_seconds()` directly — no extra plugin registration,
//! no cross-plugin coupling. A single-frame `1.0 / dt` reading is
//! noisy at the millisecond scale, but the overlay is a diagnostic,
//! not a benchmarking tool, so that tradeoff is acceptable here.
//!
//! # Safety rails
//!
//! - `Res<Scene>`, `Query<&Transform, …>`, and `Res<Time>` are all
//!   immutable borrows, so the system *cannot* mutate `Scene`, any
//!   `Transform`, or the clock (Requirement 11.3). `FreeFlyController`
//!   isn't queried at all, so it is trivially untouched.
//! - The system never calls any of egui's input-consumption APIs
//!   (`ctx.wants_keyboard_input`, `ctx.wants_pointer_input`, etc.) and
//!   only ever *draws* — egui's default behaviour is to claim input
//!   only when one of its widgets is actually interacted with, and
//!   this overlay uses `ui.label(..)` exclusively, which is
//!   non-interactive. That keeps WASD + mouse-look flowing through to
//!   `FreeFlyCameraPlugin` as Requirement 11.4 demands.

use bevy::app::{App, Plugin, Update};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::transform::components::Transform;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::Scene;
use crate::experiments::catalog::{BodyNames, ExperimentSpec};
use crate::input::camera::{CameraLock, CameraSpeed, FreeFlyController};
use crate::render::mission::Mission;
use crate::time::constants::{
    SECONDS_PER_DAY, SECONDS_PER_HOUR, SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MINUTE,
};
use crate::time::observer::ObserverClock;
use crate::time::simulation::SimulationClock;
use crate::ui::constants::{MAX_DISTANCE_ROWS, MAX_PROP_ROWS};

/// Bevy plugin that adds a read-only `bevy_egui` debug overlay to the
/// `App`.
///
/// Responsibilities (Requirement 11 / task 9.1):
///
/// 1. Ensure `bevy_egui::EguiPlugin` is present. Registered once via
///    the `App::is_plugin_added::<EguiPlugin>()` guard so that apps
///    which have already installed `EguiPlugin` (e.g. because another
///    plugin did it) don't fail with Bevy's "plugin already added"
///    panic.
/// 2. Register [`debug_overlay_system`] on the `Update` schedule.
///    That system draws exactly one `egui::Window` per frame showing
///    FPS, `scene.props.len()`, the camera's `translation`, and the
///    camera's `rotation`.
pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    /// Conditionally add `EguiPlugin`, then register the overlay
    /// system on `Update`.
    ///
    /// The `is_plugin_added` check matches task 9.1's explicit
    /// instruction: callers who already installed `EguiPlugin` won't
    /// trip Bevy's panic on duplicate plugin registration, while
    /// callers who haven't get it for free. Either way,
    /// `debug_overlay_system` runs once per `Update` tick.
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }
        app.add_systems(Update, (debug_overlay_system, mission_overlay_system));
    }
}

/// The experiment row: which experiment is running, and the seed it was built from.
///
/// The seed is the reproducibility handle. A three-body run picks a fresh seed every
/// launch, so without it on screen an interesting outcome would be unrepeatable —
/// you would have watched something you could never show anyone. With it,
/// `--seed <value>` replays the same initial conditions exactly.
///
/// Pure and `String`-returning like the other row builders, so the formatting is
/// testable without egui.
///
/// ```text
/// exp   experiment_2 Three-body ring  seed 42
/// ```
pub fn experiment_line(spec: &ExperimentSpec) -> String {
    format!(
        "exp   {} {}  seed {}",
        spec.id.slug(),
        spec.id.name(),
        spec.seed
    )
}

/// The overlay's two-clock readout, as one line per clock plus a state line.
///
/// Pure and `String`-returning for the same reason as [`prop_lines`] and
/// [`distance_lines`]: the formatting is the part worth testing, and it should be
/// testable without standing up egui.
///
/// # Rows
///
/// ```text
/// sim   4.207e7 s  (486.9 d)   x5.000e6
/// real  8.42 s     60.1 fps
/// clock running
/// ```
///
/// - **sim** — total simulated time from
///   [`SimulationClock::elapsed_seconds`], in SI seconds and then in whichever human
///   unit fits ([`human_duration`]), followed by the clock's scale. This is the
///   quantity to compare against
///   [`Scene::two_body_free_fall_seconds`](crate::core::scene::Scene::two_body_free_fall_seconds):
///   when `sim` passes it, the shipped spheres have met.
/// - **real** — wall time from [`ObserverClock`], and the frame rate.
/// - **clock** — `running`, `paused`, `paused (step queued)`, or `held for a flight plan`, so the pause key's
///   effect is visible rather than inferred from the numbers having stopped.
///
/// Simulated seconds are shown in exponent form because they reach `1e8`; real
/// seconds are shown plainly because they will not.
pub fn clock_lines(simulation: &SimulationClock, observer: &ObserverClock) -> Vec<String> {
    let elapsed_sim = simulation.elapsed_seconds();

    let state = if simulation.is_held() {
        "held for a flight plan"
    } else if simulation.is_paused() {
        if simulation.single_step_pending() {
            "paused (step queued)"
        } else {
            "paused"
        }
    } else {
        "running"
    };

    vec![
        format!(
            "sim   {:.3e} s  ({})   x{:.3e}",
            elapsed_sim,
            human_duration(elapsed_sim),
            simulation.scale()
        ),
        format!(
            "real  {:.2} s     {:.1} fps",
            observer.elapsed_seconds(),
            observer.fps()
        ),
        format!("clock {state}"),
    ]
}

/// The camera's current speed mode, as one overlay row.
///
/// # Why this row exists at all
///
/// It is the counterweight to amendment A26. A held modifier announces itself — your finger is on
/// it — but a mode is invisible, so a user who cycled `C` once too often has a camera `150×` slower
/// than they expect and nothing on screen to explain it. This row is what makes the toggle usable
/// rather than merely fewer keystrokes.
///
/// The multiplier is printed alongside the name because "slow" does not say *how* slow, and the
/// three modes span `150×`.
///
/// ```text
/// cam   normal  x1.00
/// ```
///
/// # Examples
///
/// ```
/// use physics_sandbox::{camera_speed_line, CameraSpeed, FreeFlyController};
///
/// let controller = FreeFlyController::default();
///
/// assert_eq!(camera_speed_line(CameraSpeed::Normal, &controller), "cam   normal  x1.00");
/// assert_eq!(camera_speed_line(CameraSpeed::Fast, &controller), "cam   fast    x3.00");
/// assert_eq!(camera_speed_line(CameraSpeed::Slow, &controller), "cam   slow    x0.02");
/// ```
pub fn camera_speed_line(speed: CameraSpeed, controller: &FreeFlyController) -> String {
    // `{:<7}` pads the name so the multipliers line up in a column — the point of the row is
    // noticing at a glance that it is not `normal`, and a ragged left edge works against that.
    format!(
        "cam   {:<7} x{:.2}",
        speed.label(),
        speed.multiplier(controller)
    )
}

/// The camera's lock state: `off`, or the orbit distance in metres.
///
/// The lock silently makes WASD inert, so like the speed mode it needs a readout.
///
/// ```
/// use physics_sandbox::{camera_lock_line, CameraLock};
///
/// assert_eq!(camera_lock_line(&CameraLock::default()), "lock  off");
/// ```
pub fn camera_lock_line(lock: &CameraLock) -> String {
    match lock.0 {
        None => "lock  off".to_string(),
        Some(orbit) => format!(
            "lock  orbit {:.2e} m",
            units_to_metres(f64::from(orbit.distance))
        ),
    }
}

/// A duration in seconds, rendered in the largest unit that leaves it above one.
///
/// Seconds up to a minute, then minutes, hours, days, years — because `4.2e7 s`
/// tells you nothing about whether the experiment is a minute or a decade in, and
/// "486.9 d" tells you immediately. Years use 365.25 days, the Julian year, matching
/// the convention the astronomical figures in the scene's docs use.
///
/// Non-finite or negative input yields `"-"` rather than a misleading number.
pub fn human_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "-".to_string();
    }

    let (value, unit) = if seconds >= SECONDS_PER_JULIAN_YEAR {
        (seconds / SECONDS_PER_JULIAN_YEAR, "y")
    } else if seconds >= SECONDS_PER_DAY {
        (seconds / SECONDS_PER_DAY, "d")
    } else if seconds >= SECONDS_PER_HOUR {
        (seconds / SECONDS_PER_HOUR, "h")
    } else if seconds >= SECONDS_PER_MINUTE {
        (seconds / SECONDS_PER_MINUTE, "min")
    } else {
        (seconds, "s")
    };

    format!("{value:.1} {unit}")
}

/// Convert a world-unit length into metres.
///
/// The overlay is the crate's second units-to-SI conversion site, after
/// `core::integrator`. It is a *display* conversion: the scene keeps working in
/// units, and only the strings the user reads are in metres. Returned as `f64`
/// because a scene-sized length in metres — `3e10` and up — has no useful
/// precision left in `f32`.
#[inline]
pub fn units_to_metres(units: f64) -> f64 {
    units * METRES_PER_UNIT
}

/// One row per prop: id, shape, mass in kg, speed and velocity in m/s.
///
/// Every figure is SI. Mass is already in kilograms; lengths and velocities are
/// scaled from world units by [`units_to_metres`]. Immovable props are tagged and
/// show no velocity, because theirs is always zero by invariant.
///
/// # Notation
///
/// Values are formatted in scientific notation throughout. That is not laziness:
/// the quantities in play span from `1e-8 m/s` early in a free fall to `1e24 kg`,
/// and fixed-point would print either all zeros or an unreadable wall of digits.
///
/// **The `e` is a decimal exponent, not Euler's number.** `7e2` reads as
/// `7 × 10² = 700`, not as `7·e² ≈ 51.7`. So `6.684e1 m/s` is 66.84 m/s and
/// `5.972e24 kg` is one Earth mass.
///
/// Rust's `{:e}` prints no `+` on a positive exponent and does not pad it, so zero
/// appears as `0.000e0` rather than `0.000e+00`. That is why an at-rest prop's row
/// looks slightly odd instead of simply reading `0`.
///
/// # Truncation
///
/// At most [`MAX_PROP_ROWS`] props are listed; if there are more, a final row says
/// how many were omitted.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{prop_lines, Scene};
///
/// // The shipped scene: floor plus two spheres.
/// let lines = prop_lines(&Scene::two_body_gravity());
///
/// assert_eq!(lines.len(), 3);
/// assert!(lines[0].contains("Plane") && lines[0].contains("immovable"));
/// assert!(lines[1].contains("Sphere") && lines[1].contains("5.972e24 kg"));
/// ```
pub fn prop_lines(scene: &Scene) -> Vec<String> {
    let mut lines: Vec<String> = scene
        .iter()
        .take(MAX_PROP_ROWS)
        .map(|(id, prop)| {
            let head = format!(
                "#{} {}  m={:.3e} kg",
                id.0,
                prop.body.kind_name(),
                prop.mass_kg
            );

            if prop.immovable {
                format!("{}  [immovable]", head)
            } else {
                let velocity = prop.velocity_units_s;
                format!(
                    "{}  |v|={:.3e} m/s  v=({:.2e}, {:.2e}, {:.2e})",
                    head,
                    prop.speed_m_s(),
                    units_to_metres(velocity.x),
                    units_to_metres(velocity.y),
                    units_to_metres(velocity.z),
                )
            }
        })
        .collect();

    let omitted = scene.props.len().saturating_sub(MAX_PROP_ROWS);
    if omitted > 0 {
        lines.push(format!("... and {} more prop(s)", omitted));
    }

    lines
}

/// One row per pair of props: centre-to-centre distance in metres.
///
/// Each unordered pair appears once, in `(lower id, higher id)` order. The
/// distance is between body *centres* — for a `Plane` that is its origin, not the
/// nearest point on its surface, so the floor's distances are informative rather
/// than a contact measure.
///
/// # Truncation
///
/// Pair count grows as `n(n-1)/2`. At most [`MAX_DISTANCE_ROWS`] pairs are listed;
/// if there are more, a final row says how many were omitted.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{distance_lines, Scene, TWO_BODY_SEPARATION_UNITS};
///
/// let lines = distance_lines(&Scene::two_body_gravity());
///
/// // Three props means three pairs.
/// assert_eq!(lines.len(), 3);
///
/// // The two spheres start 30 units = 3e10 m apart.
/// assert!(lines.iter().any(|line| line.starts_with("#1-#2: 3.0000e10 m")));
/// ```
pub fn distance_lines(scene: &Scene) -> Vec<String> {
    let props: Vec<_> = scene.iter().collect();

    let mut lines = Vec::new();
    let mut pairs_seen = 0usize;

    for (index, (id_a, prop_a)) in props.iter().enumerate() {
        for (id_b, prop_b) in props.iter().skip(index + 1) {
            pairs_seen += 1;
            if lines.len() >= MAX_DISTANCE_ROWS {
                continue;
            }

            let separation_units = (prop_b.body.center() - prop_a.body.center()).length();
            lines.push(format!(
                "#{}-#{}: {:.4e} m",
                id_a.0,
                id_b.0,
                units_to_metres(separation_units)
            ));
        }
    }

    let omitted = pairs_seen.saturating_sub(lines.len());
    if omitted > 0 {
        lines.push(format!("... and {} more pair(s)", omitted));
    }

    lines
}

/// Bevy `Update` system that renders the debug overlay.
///
/// Parameter choices (Requirement 11.3):
///
/// - `contexts: EguiContexts` must be mutable — `EguiContexts::ctx_mut`
///   returns an `&mut egui::Context`, which is egui's handle for
///   issuing per-frame UI. Mutating egui's context is not the same as
///   mutating game state; `Scene`, every `Transform`, and every
///   `FreeFlyController` are all untouched, which is what
///   Requirement 11.3 cares about.
/// - `scene: Res<Scene>` — immutable borrow, so `Scene::props`
///   cannot be mutated from this system.
/// - `time: Res<Time>` — immutable borrow; we only read
///   `delta_seconds()` to compute FPS.
/// - `camera: Query<&Transform, With<Camera3d>>` — immutable borrow of
///   the camera's `Transform`. `With<Camera3d>` filters to the unique
///   3D camera entity spawned by `spawn_scene_entities`. Using
///   `Query<&Transform>` (rather than `Query<&mut Transform>`) is
///   what formally gives us Requirement 11.3's "no Transform
///   mutation" guarantee at compile time.
///
/// The overlay itself uses a single `egui::Window::new("Debug").show`
/// call per frame, which is exactly the "one egui Window per frame"
/// called out in Requirement 11.2. All four UI rows use `ui.label(..)`,
/// which is non-interactive — egui won't capture keyboard or mouse
/// focus for a label, so WASD and mouse-look pass through to
/// `FreeFlyCameraPlugin` as Requirement 11.4 requires.
///
/// # Graceful-degradation notes
///
/// - If the camera entity is absent (e.g. startup hasn't run yet in
///   some test configurations), `camera.get_single()` returns `Err`
///   and we fall back to `"(no camera)"` strings for translation and
///   rotation rather than panicking. This mirrors the same defensive
///   pattern used in `free_fly_camera_system`.
pub fn debug_overlay_system(
    mut contexts: EguiContexts,
    scene: Res<Scene>,
    spec: Res<ExperimentSpec>,
    simulation: Res<SimulationClock>,
    observer: Res<ObserverClock>,
    camera: Query<&Transform, With<Camera3d>>,
    controllers: Query<(&FreeFlyController, Option<&CameraLock>)>,
) {
    // FPS comes from the *observer* clock's raw delta: frames per real second is the
    // only useful reading, and it must not be affected by the simulation being paused
    // or rescaled.
    let fps = observer.fps();
    let prop_count = scene.props.len();
    let clock_rows = clock_lines(&simulation, &observer);
    let experiment_row = experiment_line(&spec);

    // The camera's speed mode. `None` when there is no controller yet — the same graceful
    // degradation the transform rows below use, and for the same reason: this system has no
    // ordering dependency on the plugin that spawns the camera.
    let controller = controllers.get_single().ok();
    let speed_row =
        controller.map(|(controller, _)| camera_speed_line(controller.speed, controller));
    let lock_row = controller.and_then(|(_, lock)| lock).map(camera_lock_line);

    // Resolve the unique camera transform. Falling back to a
    // placeholder string keeps the overlay alive across startup
    // ordering quirks (e.g. the very first tick before
    // `spawn_scene_entities` has run) instead of panicking on a
    // missing entity — important because `DebugUiPlugin` has no
    // ordering dependency on `ScenePlugin` today.
    let camera_lines: (String, String) = match camera.get_single() {
        Ok(transform) => {
            let t = transform.translation;
            let r = transform.rotation;
            (
                format!("translation: ({:.3}, {:.3}, {:.3})", t.x, t.y, t.z),
                format!("rotation: ({:.3}, {:.3}, {:.3}, {:.3})", r.x, r.y, r.z, r.w),
            )
        }
        Err(_) => (
            "translation: (no camera)".to_string(),
            "rotation: (no camera)".to_string(),
        ),
    };

    // One `egui::Window` per frame (Requirement 11.2). `ui.label`
    // rows are non-interactive, so egui never claims input focus
    // for them — WASD + mouse-look keep flowing to the camera
    // plugin (Requirement 11.4).
    egui::Window::new("Debug").show(contexts.ctx_mut(), |ui| {
        // Which experiment, and the seed that produced it — so an interesting run can
        // be reproduced with `--seed <that number>`.
        ui.label(experiment_row);
        ui.label(format!("FPS: {:.1}", fps));
        ui.label(format!("props: {}", prop_count));

        // The two clocks, side by side. This is the row that makes the split legible:
        // the experiment's elapsed time is not the observer's, and their ratio is the
        // scale actually being achieved.
        ui.separator();
        for line in &clock_rows {
            ui.label(line);
        }
        ui.separator();
        // Before the transform rows: which speed mode `C` has left the camera in. A mode is
        // invisible in a way a held key is not, so this row is what makes A26's toggle usable —
        // see `camera_speed_line`.
        if let Some(row) = speed_row {
            ui.label(row);
        }
        if let Some(row) = lock_row {
            ui.label(row);
        }
        ui.label(camera_lines.0);
        ui.label(camera_lines.1);

        // Per-prop state and pairwise separations, all in SI units. Built by the
        // pure helpers above so the formatting is testable without an egui context.
        ui.separator();
        ui.label("props (SI units)");
        for line in prop_lines(&scene) {
            ui.label(line);
        }

        ui.separator();
        ui.label("centre-to-centre distances");
        for line in distance_lines(&scene) {
            ui.label(line);
        }
    });
}

/// Bevy system that shows the rocket's progress in a small window of its own, once one has been
/// launched. Separate from [`debug_overlay_system`] so that window stays about the scene.
pub fn mission_overlay_system(
    mut contexts: EguiContexts,
    mission: Option<Res<Mission>>,
    names: Option<Res<BodyNames>>,
    simulation: Res<SimulationClock>,
) {
    let now = simulation.elapsed_seconds();
    let names = names.as_deref().cloned().unwrap_or_default();
    let Some(row) = mission.and_then(|mission| mission.status_line(now, &names)) else {
        return;
    };
    egui::Window::new("Rocket").show(contexts.ctx_mut(), |ui| {
        ui.label(row);
    });
}

#[cfg(test)]
#[path = "../../tst/ui/overlay_tests.rs"]
mod tests;
