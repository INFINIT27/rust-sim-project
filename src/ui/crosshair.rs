//! A small centre-screen crosshair.
//!
//! Marks the exact point the camera is looking at, which is what makes
//! mouse-look aiming legible and gives a fixed reference for judging whether a
//! simulated body is where you expected.
//!
//! # Shape
//!
//! Four separate strokes with a gap in the middle, rather than two crossing
//! lines:
//!
//! ```text
//!            │
//!            │        <- arm_length_px
//!
//!      ───      ───   <- gap_px on either side of the exact centre
//!
//!            │
//!            │
//! ```
//!
//! The gap leaves the pixel under the crosshair visible. A solid cross would
//! cover the one thing the crosshair exists to point at.
//!
//! # Structure, and why the geometry is a free function
//!
//! [`crosshair_segments`] computes the four strokes as plain `(f32, f32)` pairs
//! with no egui types involved, so the geometry is unit- and property-testable
//! with no egui context, no window, and no GPU — the same split [`super::overlay`]
//! uses for its private `fps_from_dt` helper. [`crosshair_system`] does nothing
//! but convert those numbers into egui calls.
//!
//! # It cannot steal input
//!
//! The crosshair is painted directly onto a foreground layer via
//! `Context::layer_painter` rather than as a widget, so egui never registers it
//! as interactive and it can never claim pointer or keyboard focus. That
//! preserves Requirement 11.4 — the camera keeps receiving the input it needs —
//! by construction rather than by convention.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::system::{Res, Resource};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::ui::constants::{
    CROSSHAIR_ARM_LENGTH_PX, CROSSHAIR_COLOR_SRGBA, CROSSHAIR_GAP_PX, CROSSHAIR_THICKNESS_PX,
};

/// Appearance of the centre-screen crosshair, in logical screen pixels.
///
/// All fields are `pub` and unvalidated on assignment, matching
/// `SceneConfig` and `FreeFlyController`; [`crosshair_segments`] checks them and
/// degrades benignly.
#[derive(Resource, Debug, Copy, Clone, PartialEq)]
pub struct CrosshairConfig {
    /// Length of each of the four strokes, in logical pixels. Must be finite and
    /// non-negative; `0.0` draws nothing.
    pub arm_length_px: f32,
    /// Distance from the exact centre to the inner end of each stroke, in
    /// logical pixels. Must be finite and non-negative; `0.0` makes the four
    /// strokes meet in the middle, giving a solid cross.
    pub gap_px: f32,
    /// Stroke width in logical pixels.
    pub thickness_px: f32,
    /// Stroke colour as sRGB `[r, g, b, a]`, straight (non-premultiplied) alpha.
    ///
    /// Deliberately not `egui::Color32` and not `scene::Color`: keeping it a
    /// plain byte array means this config carries no egui type and no
    /// colour-space ambiguity. `scene::Color` is *linear* RGBA for PBR
    /// materials, whereas egui wants sRGB — mixing the two silently washes
    /// colours out, so they are kept apart.
    pub color: [u8; 4],
}

impl Default for CrosshairConfig {
    /// A small, unobtrusive white crosshair: 6 px arms, 3 px gap, 1 px thick, at
    /// roughly 78% opacity so it reads against both the light floor and the dark
    /// background without hiding either.
    fn default() -> Self {
        Self {
            arm_length_px: CROSSHAIR_ARM_LENGTH_PX,
            gap_px: CROSSHAIR_GAP_PX,
            thickness_px: CROSSHAIR_THICKNESS_PX,
            color: CROSSHAIR_COLOR_SRGBA,
        }
    }
}

/// The four strokes of the crosshair, as `[start, end]` point pairs.
///
/// Returned in a fixed order — **left, right, up, down** — so tests and future
/// callers can index them meaningfully.
///
/// # Coordinates
///
/// Logical screen pixels in egui's convention: `x` grows right and `y` grows
/// **down**. The shape is symmetric about both axes, so the `y` direction does
/// not affect appearance; it matters only if a caller reasons about which pair is
/// "up".
///
/// # Geometry
///
/// Each stroke runs from `gap_px` away from the centre to
/// `gap_px + arm_length_px` away. Defining the arm as a *length* measured from
/// the end of the gap — rather than as an outer radius — means no combination of
/// non-negative `gap_px` and `arm_length_px` can produce a reversed or
/// zero-crossing segment.
///
/// # Parameters
///
/// - `center_x`, `center_y` — the centre of the crosshair, in pixels. Finite.
/// - `arm_length_px` — length of each stroke. Finite and non-negative.
/// - `gap_px` — clear space between the centre and each stroke. Finite and
///   non-negative.
///
/// # Returns
///
/// Four `[start, end]` pairs. When `arm_length_px` is `0.0` every pair is
/// degenerate (start equals end) and egui draws nothing.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release every segment
/// collapses to the origin `(0.0, 0.0)`, drawing nothing rather than smearing a
/// `NaN`-positioned stroke across the frame.
///
/// # Examples
///
/// ```
/// use physics_sandbox::crosshair_segments;
///
/// // 4 px arms with a 2 px gap, centred at (100, 50).
/// let [left, right, up, down] = crosshair_segments(100.0, 50.0, 4.0, 2.0);
///
/// assert_eq!(left, [(94.0, 50.0), (98.0, 50.0)]);
/// assert_eq!(right, [(102.0, 50.0), (106.0, 50.0)]);
/// assert_eq!(up, [(100.0, 44.0), (100.0, 48.0)]);
/// assert_eq!(down, [(100.0, 52.0), (100.0, 56.0)]);
/// ```
pub fn crosshair_segments(
    center_x: f32,
    center_y: f32,
    arm_length_px: f32,
    gap_px: f32,
) -> [[(f32, f32); 2]; 4] {
    debug_assert!(
        center_x.is_finite() && center_y.is_finite(),
        "crosshair_segments: centre must be finite, got ({}, {})",
        center_x,
        center_y
    );
    debug_assert!(
        arm_length_px.is_finite() && arm_length_px >= 0.0,
        "crosshair_segments: arm_length_px must be finite and non-negative, got {}",
        arm_length_px
    );
    debug_assert!(
        gap_px.is_finite() && gap_px >= 0.0,
        "crosshair_segments: gap_px must be finite and non-negative, got {}",
        gap_px
    );

    if !center_x.is_finite()
        || !center_y.is_finite()
        || !arm_length_px.is_finite()
        || arm_length_px < 0.0
        || !gap_px.is_finite()
        || gap_px < 0.0
    {
        return [[(0.0, 0.0); 2]; 4];
    }

    let inner = gap_px;
    let outer = gap_px + arm_length_px;

    [
        // Left: outer edge inward to the gap.
        [(center_x - outer, center_y), (center_x - inner, center_y)],
        // Right: gap outward to the outer edge.
        [(center_x + inner, center_y), (center_x + outer, center_y)],
        // Up (smaller y in egui's downward-y space).
        [(center_x, center_y - outer), (center_x, center_y - inner)],
        // Down.
        [(center_x, center_y + inner), (center_x, center_y + outer)],
    ]
}

/// Convert an sRGB `[r, g, b, a]` byte array to egui's colour type.
///
/// Uses `from_rgba_unmultiplied` because [`CrosshairConfig::color`] is documented
/// as straight alpha. Passing premultiplied bytes here would darken the stroke as
/// its alpha fell.
#[inline]
fn to_egui_color(rgba: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// Paint the crosshair at the centre of the primary window, once per frame.
///
/// Reads [`CrosshairConfig`], derives the geometry from
/// [`crosshair_segments`], and strokes it onto a foreground egui layer. Draws
/// nothing when the screen rect is degenerate — a zero-sized or minimised window
/// — which keeps Requirement 14.1 satisfied.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window
/// already has an egui context, so this system needs a real windowed app or the
/// full headless egui setup. See `tst/ui/README.md`.
pub fn crosshair_system(config: Res<CrosshairConfig>, mut contexts: EguiContexts) {
    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();

    // A minimised or zero-sized window has nothing to centre on.
    if !(screen.width() > 0.0 && screen.height() > 0.0) {
        return;
    }

    let center = screen.center();
    let segments = crosshair_segments(center.x, center.y, config.arm_length_px, config.gap_px);

    // `layer_painter` paints directly rather than registering a widget, so the
    // crosshair cannot claim pointer or keyboard focus from the camera.
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("physics_sandbox_crosshair"),
    ));
    let stroke = egui::Stroke::new(config.thickness_px, to_egui_color(config.color));

    for [(start_x, start_y), (end_x, end_y)] in segments {
        painter.line_segment(
            [egui::pos2(start_x, start_y), egui::pos2(end_x, end_y)],
            stroke,
        );
    }
}

/// Draws a small crosshair at the centre of the view.
///
/// Inserts [`CrosshairConfig::default()`] and registers [`crosshair_system`] on
/// `Update`. Adds `EguiPlugin` only if it is not already present, so this plugin
/// composes with [`super::overlay::DebugUiPlugin`] and with a host app that
/// installed egui itself.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use physics_sandbox::{CrosshairPlugin, ScenePlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(ScenePlugin::default())
///     .add_plugins(CrosshairPlugin)
///     .run();
/// ```
#[derive(Debug, Default, Copy, Clone)]
pub struct CrosshairPlugin;

impl Plugin for CrosshairPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }

        app.init_resource::<CrosshairConfig>();
        app.add_systems(Update, crosshair_system);
    }
}

#[cfg(test)]
#[path = "../../tst/ui/crosshair_tests.rs"]
mod tests;
