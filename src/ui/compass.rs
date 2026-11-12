//! An orientation compass: the three world axes, drawn in the top-right corner.
//!
//! Flying a free camera around a scene, it is easy to lose which way is which — and once
//! you have, the grid's `±25`-unit squares tell you nothing, because you cannot say which
//! direction you are counting them in. The compass answers that continuously: three short
//! arms from a common origin, one per world axis, projected into the camera's own frame and
//! relabelled every frame as you turn.
//!
//! # How it stays correct as you move
//!
//! A world axis `d` seen from a camera with rotation `q` is `q⁻¹ · d` in view space. Bevy's
//! view space is right-handed with `−Z` forward, so:
//!
//! - `x` is right on screen, and maps straight to screen `+x`;
//! - `y` is up in view space but screen `y` grows **downward**, so it is negated;
//! - `z` is the depth: **positive is toward the viewer**, because the camera looks along
//!   `−Z`.
//!
//! So the screen arm is `(v.x, −v.y) · radius` and the third component becomes
//! [`CompassAxis::toward_viewer`], which the drawing code uses to fade an axis that points
//! away from you. That fade is what makes a flat drawing read as an orientation: without
//! it, an axis pointing at your eye and one pointing away look identical — both project to
//! a short arm — and the compass would be ambiguous exactly when you most need it.
//!
//! Translation is deliberately ignored. A compass shows *heading*, not position: it must
//! look the same from anywhere in the scene as long as you are facing the same way.
//!
//! # It is an observer aid, outside the model
//!
//! Like [`super::crosshair`] and [`crate::render::grid`], nothing here touches the `Scene`.
//! It reads the camera's `Transform` and paints; it holds no simulation state and it cannot
//! perturb one.
//!
//! # Colours match the floor grid
//!
//! Red for X and blue for Z, the same convention the grid's axis lines use, so the compass
//! and the lines on the floor agree about which is which. Y gets green — the axis with no
//! floor line to draw, since it points out of the floor, and the one the `Space`/`Ctrl` and
//! `E`/`Q` keys move along.

use bevy::app::{App, Plugin, Update};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Query, Res, Resource};
use bevy::math::{Quat, Vec2, Vec3};
use bevy::transform::components::Transform;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::ui::constants::{
    COMPASS_DIM_ALPHA, COMPASS_LABEL_SIZE_PX, COMPASS_MARGIN_PX, COMPASS_RADIUS_PX,
    COMPASS_THICKNESS_PX, COMPASS_X_COLOR_SRGBA, COMPASS_Y_COLOR_SRGBA, COMPASS_Z_COLOR_SRGBA,
};

/// Appearance and placement of the compass, in logical screen pixels.
///
/// All fields are `pub` and unvalidated on assignment, matching
/// [`CrosshairConfig`](super::crosshair::CrosshairConfig) and `FreeFlyController`;
/// [`compass_axes`] checks what it needs and degrades benignly.
#[derive(Resource, Debug, Copy, Clone, PartialEq)]
pub struct CompassConfig {
    /// Length of each arm, in logical pixels. Also the compass's radius, so the widget
    /// occupies `2 × radius` plus room for labels.
    pub radius_px: f32,
    /// Gap between the compass's bounding box and the top-right corner of the window.
    pub margin_px: f32,
    /// Arm stroke width in logical pixels.
    pub thickness_px: f32,
    /// Label height in logical pixels.
    pub label_size_px: f32,
}

impl Default for CompassConfig {
    /// The shipped compass: see the `COMPASS_*` constants for why each value.
    fn default() -> Self {
        Self {
            radius_px: COMPASS_RADIUS_PX,
            margin_px: COMPASS_MARGIN_PX,
            thickness_px: COMPASS_THICKNESS_PX,
            label_size_px: COMPASS_LABEL_SIZE_PX,
        }
    }
}

/// One axis of the compass, already projected into screen space.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CompassAxis {
    /// `"X"`, `"Y"`, or `"Z"`.
    pub label: &'static str,
    /// Offset of the arm's tip from the compass centre, in logical pixels, with `y`
    /// growing **downward** as screen coordinates do.
    pub tip_px: Vec2,
    /// How much this axis points at the viewer, in `[-1, 1]`: `1.0` straight out of the
    /// screen, `-1.0` straight into it, `0.0` flat across the view.
    ///
    /// The sign is what disambiguates an axis coming toward you from one going away —
    /// both of which project to a short arm — and the drawing code fades the negative
    /// case. Named for what it means rather than `z`, because view-space `z` being
    /// *toward* the viewer surprises everyone once.
    pub toward_viewer: f32,
    /// The axis's colour as sRGB `[r, g, b, a]`, straight alpha.
    pub color_srgba: [u8; 4],
}

impl CompassAxis {
    /// Is this axis pointing away from the viewer?
    ///
    /// Exactly at zero counts as *not* away: an axis lying flat across the view is fully
    /// visible and should be drawn at full strength.
    #[inline]
    pub fn points_away(&self) -> bool {
        self.toward_viewer < 0.0
    }
}

/// The three world axes as seen by a camera with rotation `rotation`, projected onto a
/// compass of radius `radius_px`.
///
/// Pure so the projection is testable without a window, a GPU, or egui — the same split
/// [`crosshair_segments`](super::crosshair::crosshair_segments) and
/// [`floor_grid_segments`](crate::render::grid::floor_grid_segments) use.
///
/// # Returns
///
/// Always three axes, in `X, Y, Z` order — a fixed order so a caller can index them and a
/// test can name them. Each tip is at most `radius_px` from the centre, and exactly
/// `radius_px` when the axis lies flat across the view.
///
/// A non-finite or negative `radius_px`, or a non-finite rotation, yields three zero-length
/// arms rather than `NaN` positions: the compass collapses to a dot, which is visibly wrong
/// without being able to poison a painter.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{compass_axes, Quat};
///
/// // A camera with no rotation looks down −Z, so world X is to the right, world Y is up
/// // (negative screen y), and world Z points at the viewer.
/// let [x, y, z] = compass_axes(Quat::IDENTITY, 30.0);
///
/// assert_eq!(x.label, "X");
/// assert!((x.tip_px.x - 30.0).abs() < 1.0e-3);
/// assert!((y.tip_px.y + 30.0).abs() < 1.0e-3, "screen y grows downward");
/// assert!(z.toward_viewer > 0.99, "Z comes out of the screen");
/// assert!(!z.points_away());
/// ```
pub fn compass_axes(rotation: Quat, radius_px: f32) -> [CompassAxis; 3] {
    debug_assert!(
        radius_px.is_finite() && radius_px > 0.0,
        "compass_axes: radius_px must be finite and positive, got {}",
        radius_px
    );

    let usable_radius = if radius_px.is_finite() && radius_px > 0.0 {
        radius_px
    } else {
        0.0
    };

    // A non-finite rotation cannot be inverted usefully. Falling back to identity keeps
    // the compass drawable and merely wrong, rather than filling the painter with `NaN`.
    let view_from_world = if rotation.is_finite() && rotation.length_squared() > 0.0 {
        rotation.normalize().inverse()
    } else {
        Quat::IDENTITY
    };

    let project = |label: &'static str, axis: Vec3, color_srgba: [u8; 4]| {
        let view = view_from_world * axis;

        CompassAxis {
            label,
            // Screen `y` grows downward, so the view-space `y` is negated. Without this
            // the compass would be mirrored vertically, which reads as correct until you
            // pitch the camera.
            tip_px: Vec2::new(view.x, -view.y) * usable_radius,
            toward_viewer: view.z,
            color_srgba,
        }
    };

    [
        project("X", Vec3::X, COMPASS_X_COLOR_SRGBA),
        project("Y", Vec3::Y, COMPASS_Y_COLOR_SRGBA),
        project("Z", Vec3::Z, COMPASS_Z_COLOR_SRGBA),
    ]
}

/// Convert an sRGB byte array into egui's colour type, scaling alpha by `alpha_scale`.
///
/// The scale is how an axis pointing away from the viewer is faded; `1.0` leaves the colour
/// as authored.
fn to_egui_color(rgba: [u8; 4], alpha_scale: f32) -> egui::Color32 {
    let alpha = (f32::from(rgba[3]) * alpha_scale.clamp(0.0, 1.0)).round() as u8;
    egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], alpha)
}

/// Paint the compass in the top-right corner, once per frame.
///
/// Reads the camera's rotation, derives the arms from [`compass_axes`], and strokes them
/// onto a foreground egui layer with a label at each tip.
///
/// # Draw order
///
/// Axes are drawn back to front — sorted by [`CompassAxis::toward_viewer`] — so an arm
/// coming toward you overlaps one going away rather than the reverse. With three arms from
/// a shared origin the overlap is small, but getting it backwards makes the compass read
/// inside-out at certain angles.
///
/// Draws nothing when there is no camera, or when the window is zero-sized or minimised,
/// which keeps Requirement 14.1 satisfied.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window already has an
/// egui context, so this system needs a real windowed app. See `tst/ui/README.md`.
pub fn compass_system(
    config: Res<CompassConfig>,
    camera: Query<&Transform, With<Camera3d>>,
    mut contexts: EguiContexts,
) {
    // No camera means no orientation to show — during the first frame under some plugin
    // orderings, or in a test app with no scene.
    let Ok(transform) = camera.get_single() else {
        return;
    };

    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();

    if !(screen.width() > 0.0 && screen.height() > 0.0) {
        return;
    }

    // Anchored to the top-right corner, inset by the margin plus the radius so the whole
    // compass — not its centre — clears the edge. The debug overlay is a top-*left*
    // window, so the two do not fight for the same pixels.
    let inset = config.margin_px + config.radius_px;
    let centre = egui::pos2(screen.right() - inset, screen.top() + inset);

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("physics_sandbox_compass"),
    ));

    let mut axes = compass_axes(transform.rotation, config.radius_px);
    // Back to front: least toward the viewer first.
    axes.sort_by(|left, right| left.toward_viewer.total_cmp(&right.toward_viewer));

    for axis in axes {
        let alpha_scale = if axis.points_away() {
            COMPASS_DIM_ALPHA
        } else {
            1.0
        };
        let color = to_egui_color(axis.color_srgba, alpha_scale);
        let tip = egui::pos2(centre.x + axis.tip_px.x, centre.y + axis.tip_px.y);

        painter.line_segment([centre, tip], egui::Stroke::new(config.thickness_px, color));

        // The label sits just beyond the tip, pushed out along the arm so it never covers
        // the line it belongs to. An axis pointing almost straight at the viewer has a
        // near-zero arm and no direction to push along, so its label lands on the centre —
        // which is the honest place for it.
        let label_offset = if axis.tip_px.length() > 1.0 {
            axis.tip_px.normalize() * (0.35 * config.label_size_px + config.thickness_px)
        } else {
            Vec2::ZERO
        };

        painter.text(
            egui::pos2(tip.x + label_offset.x, tip.y + label_offset.y),
            egui::Align2::CENTER_CENTER,
            axis.label,
            egui::FontId::proportional(config.label_size_px),
            color,
        );
    }
}

/// Draws a three-axis orientation compass in the top-right corner.
///
/// Inserts [`CompassConfig::default()`] and registers [`compass_system`] on `Update`. Adds
/// `EguiPlugin` only if it is not already present, so this composes with
/// [`DebugUiPlugin`](super::overlay::DebugUiPlugin) and
/// [`CrosshairPlugin`](super::crosshair::CrosshairPlugin), and with a host app that
/// installed egui itself.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use physics_sandbox::{CompassPlugin, ScenePlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(ScenePlugin::default())
///     .add_plugins(CompassPlugin)
///     .run();
/// ```
pub struct CompassPlugin;

impl Plugin for CompassPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }

        app.init_resource::<CompassConfig>();
        app.add_systems(Update, compass_system.run_if(crate::ui::hud::hud_shown));
    }
}

#[cfg(test)]
#[path = "../../tst/ui/compass_tests.rs"]
mod tests;
