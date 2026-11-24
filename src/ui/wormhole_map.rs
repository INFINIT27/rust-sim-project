//! The geometry view of the wormhole (`experiment_9`): its embedding diagram, with the camera on it.
//!
//! # What it shows
//!
//! Take one slice of space through the wormhole's centre: a plane, two dimensions instead of three.
//! Because space is curved, that slice cannot be drawn flat without stretching. Bent into a third
//! dimension it can: the surface drawn has exactly the slice's distances, its radius `r(ℓ)` and its
//! height `z(ℓ)` (James et al. 2015, arXiv:1502.03809, eq. 6). **The sheet is the space**; the gaps
//! beside it and inside the funnel are not places at all, and height is not a direction anyone can
//! go in. A circle on the sheet stands for a whole sphere.
//!
//! The slice is the one holding the camera and the direction it looks in, so the line from the
//! camera is exactly the light arriving at the crosshair, and the way the camera would fly: it
//! crosses the throat, or turns back.
//!
//! # How it is drawn
//!
//! [`WormholeMap`] and [`project`] are pure; [`wormhole_map_system`] paints them with egui, as the
//! compass is painted, and takes no input but [`Action::ToggleWormholeMap`] (`G`).

use bevy::app::{App, Plugin, Update};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Query, Res, ResMut, Resource};
use bevy::input::{keyboard::KeyCode, ButtonInput};
use bevy::math::{DVec3, Vec2, Vec3};
use bevy::transform::components::Transform;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::bindings::{keymap_or_default, Action, KeyMap};
use crate::core::scene::{Body, Scene};
use crate::input::menu::MenuState;
use crate::physics_utils::relativity::wormhole::{WormholePath, WormholeShape};
use crate::projects::interstellar::catalog::ExperimentSpec;
use crate::projects::interstellar::wormhole::{is_wormhole, WormholeSettings};
use crate::render::wormhole_camera::{WormholePlace, WormholeTraveller};
use crate::ui::constants::{
    WORMHOLE_MAP_CAMERA_SRGBA, WORMHOLE_MAP_CAPTION, WORMHOLE_MAP_FAR_SRGBA,
    WORMHOLE_MAP_HEIGHT_PX, WORMHOLE_MAP_MARGIN_PX, WORMHOLE_MAP_MERIDIANS,
    WORMHOLE_MAP_NEAR_SRGBA, WORMHOLE_MAP_PATH_STEPS, WORMHOLE_MAP_REACH_THROATS,
    WORMHOLE_MAP_RINGS, WORMHOLE_MAP_RING_SEGMENTS, WORMHOLE_MAP_TILT_RAD,
    WORMHOLE_MAP_TUNNEL_SRGBA, WORMHOLE_MAP_WIDTH_PX,
};

/// Whether the geometry view is shown, in the wormhole's experiment. On at start.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WormholeMapVisibility {
    /// `true` while it is drawn.
    pub visible: bool,
}

impl Default for WormholeMapVisibility {
    fn default() -> Self {
        Self { visible: true }
    }
}

/// The surface of one wormhole, out to some distance beyond its mouths. In throat radii.
#[derive(Clone, Debug, PartialEq)]
pub struct WormholeMap {
    /// `(ℓ, r, z)` from the near side's mouth outwards; the far side is its mirror image.
    profile: Vec<Vec3>,
}

impl WormholeMap {
    /// The surface of `shape` out to `reach` throat radii beyond its mouths, at `rings` radii a side.
    ///
    /// The radii are packed towards the mouth, where the surface turns.
    pub fn new(shape: &WormholeShape, reach: f64, rings: usize) -> Self {
        let throat = shape.throat();
        let mouth = shape.half_length();
        let scale = shape.scale(mouth).min(throat);
        let rings = rings.max(2);
        let growth = (1.0 + reach * throat / scale).ln();
        // dz/dℓ; over the tunnel it is 1.
        let rise = |l: f64| (1.0 - shape.slope(l).powi(2)).max(0.0).sqrt();
        let mut height = mouth;
        let mut last = mouth;
        let mut profile = Vec::with_capacity(rings + 1);
        for i in 0..=rings {
            let l = mouth + scale * ((growth * i as f64 / rings as f64).exp() - 1.0);
            // Simpson's rule, in four pieces, on from the last ring.
            let h = (l - last) / 8.0;
            for k in 0..4 {
                let a = last + 2.0 * h * f64::from(k);
                height += h / 3.0 * (rise(a) + 4.0 * rise(a + h) + rise(a + 2.0 * h));
            }
            last = l;
            profile.push(Vec3::new(
                (l / throat) as f32,
                (shape.radius(l) / throat) as f32,
                (height / throat) as f32,
            ));
        }
        Self { profile }
    }

    /// How far the surface goes: the largest `|ℓ|` on it.
    pub fn reach(&self) -> f32 {
        self.profile[self.profile.len() - 1].x
    }

    /// Its largest radius, and its greatest height above the middle.
    pub fn size(&self) -> (f32, f32) {
        let edge = self.profile[self.profile.len() - 1];
        (edge.y, edge.z)
    }

    /// The radius and height at `l`, which beyond the surface's edge is the edge.
    pub fn at(&self, l: f32) -> (f32, f32) {
        let away = l.abs();
        let mouth = self.profile[0];
        if away <= mouth.x {
            return (mouth.y, l);
        }
        let i = self
            .profile
            .partition_point(|point| point.x < away)
            .clamp(1, self.profile.len() - 1);
        let (a, b) = (self.profile[i - 1], self.profile[i]);
        let t = ((away - a.x) / (b.x - a.x)).clamp(0.0, 1.0);
        let point = a.lerp(b, t);
        (point.y, point.z.copysign(l))
    }

    /// The point of the surface at `l`, `angle` round it: `y` up, the near side's end at the top.
    pub fn point(&self, l: f32, angle: f32) -> Vec3 {
        let (r, z) = self.at(l);
        Vec3::new(r * angle.cos(), z, r * angle.sin())
    }

    /// The `ℓ` of every ring drawn, from the far edge to the near one.
    pub fn rings(&self) -> Vec<f32> {
        let far = self.profile.iter().rev().map(|point| -point.x);
        let near = self.profile.iter().map(|point| point.x);
        far.chain(near).collect()
    }
}

/// Where `point` is drawn, in throat radii (`x` right, `y` up), and how far it is towards the viewer:
/// a parallel projection from `tilt` radians above the level.
pub fn project(point: Vec3, tilt: f32) -> (Vec2, f32) {
    let (sin, cos) = tilt.sin_cos();
    (
        Vec2::new(point.x, point.y * cos - point.z * sin),
        point.z * cos + point.y * sin,
    )
}

/// How far beyond the mouths the surface should go to hold the camera: at least the first of
/// [`WORMHOLE_MAP_REACH_THROATS`], at most the second.
pub fn reach_for(shape: &WormholeShape, place: &WormholePlace) -> f64 {
    let outside = (place.l.abs() - shape.half_length()).max(0.0) / shape.throat();
    let (least, most) = WORMHOLE_MAP_REACH_THROATS;
    (1.2 * outside).clamp(least, most)
}

/// Where the camera's label is round the diagram, radians.
pub fn angle_of(place: &WormholePlace) -> f64 {
    place.n.z.atan2(place.n.x)
}

/// The straightest path from `place` along `direction` (label frame, unit), as `(ℓ, angle)` points in
/// throat radii, to the surface's edge at `reach` or for `steps` points.
pub fn path_points(
    shape: &WormholeShape,
    place: &WormholePlace,
    direction: DVec3,
    reach: f32,
    steps: usize,
) -> Vec<Vec2> {
    let throat = shape.throat();
    let p = direction.dot(place.n);
    let across = direction - p * place.n;
    let start_angle = angle_of(place);
    let round = DVec3::new(-start_angle.sin(), 0.0, start_angle.cos());
    let way = if across.dot(round) < 0.0 { -1.0 } else { 1.0 };
    let (mut path, impact) = WormholePath::start(shape, place.l, across.length().atan2(p));
    let mut points = Vec::with_capacity(steps);
    for _ in 0..steps {
        let l = (path.l / throat) as f32;
        points.push(Vec2::new(l, (start_angle + way * path.angle) as f32));
        if l.abs() >= reach && path.l * path.p > 0.0 {
            break;
        }
        // Even along the path, so a path circling the throat shows as one.
        path = path.advance(shape, impact, 0.04 * throat, 0.05);
    }
    points
}

fn colour([r, g, b, a]: [u8; 4], strength: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(r, g, b, (f32::from(a) * strength) as u8)
}

/// Paints the geometry view in the bottom-right corner, in the wormhole's experiment.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window already has an egui
/// context.
pub fn wormhole_map_system(
    mut contexts: EguiContexts,
    shown: Res<WormholeMapVisibility>,
    spec: Option<Res<ExperimentSpec>>,
    settings: Option<Res<WormholeSettings>>,
    traveller: Option<Res<WormholeTraveller>>,
    scene: Option<Res<Scene>>,
    camera: Query<&Transform, With<Camera3d>>,
) {
    if !shown.visible || !spec.is_some_and(|spec| is_wormhole(&spec)) {
        return;
    }
    let (Some(settings), Some(place), Ok(camera)) = (
        settings,
        traveller.and_then(|traveller| traveller.place),
        camera.get_single(),
    ) else {
        return;
    };
    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();
    let panel = egui::Rect::from_min_size(
        screen.right_bottom()
            - egui::vec2(
                WORMHOLE_MAP_WIDTH_PX + WORMHOLE_MAP_MARGIN_PX,
                WORMHOLE_MAP_HEIGHT_PX + WORMHOLE_MAP_MARGIN_PX,
            ),
        egui::vec2(WORMHOLE_MAP_WIDTH_PX, WORMHOLE_MAP_HEIGHT_PX),
    );
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("physics_sandbox_wormhole_map"),
    ));
    painter.rect_filled(panel, 6.0, egui::Color32::from_black_alpha(200));

    let shape = settings.shape();
    let map = WormholeMap::new(&shape, reach_for(&shape, &place), WORMHOLE_MAP_RINGS);
    let (radius, height) = map.size();
    let (sin, cos) = WORMHOLE_MAP_TILT_RAD.sin_cos();
    // A line of text above and below, and a margin all round.
    let area = panel.shrink2(egui::vec2(14.0, 34.0));
    let scale =
        (area.width() / (2.0 * radius)).min(area.height() / (2.0 * (height * cos + radius * sin)));
    let to_screen = |l: f32, angle: f32| {
        let (flat, depth) = project(map.point(l, angle), WORMHOLE_MAP_TILT_RAD);
        (
            area.center() + egui::vec2(flat.x, -flat.y) * scale,
            // The back of the surface fainter than the front.
            0.35 + 0.65 * (0.5 + 0.5 * depth / (radius * cos + height * sin)).clamp(0.0, 1.0),
        )
    };
    let mouth = (shape.half_length() / shape.throat()) as f32;
    let tint = |l: f32| {
        if l.abs() <= mouth {
            WORMHOLE_MAP_TUNNEL_SRGBA
        } else if l > 0.0 {
            WORMHOLE_MAP_NEAR_SRGBA
        } else {
            WORMHOLE_MAP_FAR_SRGBA
        }
    };

    let rings = map.rings();
    let turn = std::f32::consts::TAU;
    for &l in &rings {
        let width = if l.abs() <= mouth { 2.0_f32 } else { 0.8 };
        for i in 0..WORMHOLE_MAP_RING_SEGMENTS {
            let angle = |i: usize| turn * i as f32 / WORMHOLE_MAP_RING_SEGMENTS as f32;
            let ((a, shade), (b, _)) = (to_screen(l, angle(i)), to_screen(l, angle(i + 1)));
            let stroke = egui::Stroke::new(width, colour(tint(l), 0.6 * shade));
            painter.line_segment([a, b], stroke);
        }
    }
    for i in 0..WORMHOLE_MAP_MERIDIANS {
        let angle = turn * i as f32 / WORMHOLE_MAP_MERIDIANS as f32;
        for pair in rings.windows(2) {
            let ((a, shade), (b, _)) = (to_screen(pair[0], angle), to_screen(pair[1], angle));
            let stroke = egui::Stroke::new(
                0.8_f32,
                colour(tint(0.5 * (pair[0] + pair[1])), 0.6 * shade),
            );
            painter.line_segment([a, b], stroke);
        }
    }

    // The bodies, by how far along they are and which way round (their height is left out).
    for (id, prop) in scene.iter().flat_map(|scene| scene.iter()) {
        let space = scene.as_ref().and_then(|scene| scene.wormhole.as_ref());
        let (Some(at), Body::Sphere { color, .. }) =
            (space.and_then(|space| space.place(id)), &prop.body)
        else {
            continue;
        };
        let l = (at.l / shape.throat()) as f32;
        if l.abs() <= map.reach() {
            let (dot, _) = to_screen(l, angle_of(&at) as f32);
            let shade = bevy::color::Color::linear_rgb(color.r, color.g, color.b).to_srgba();
            let byte = |x: f32| (255.0 * x) as u8;
            painter.circle_filled(
                dot,
                3.0,
                colour(
                    [byte(shade.red), byte(shade.green), byte(shade.blue), 255],
                    1.0,
                ),
            );
        }
    }

    // The light at the crosshair, and the camera it arrives at.
    let forward = place.frame().inverse() * camera.forward().as_vec3().as_dvec3();
    let path = path_points(
        &shape,
        &place,
        forward,
        map.reach(),
        WORMHOLE_MAP_PATH_STEPS,
    );
    let path: Vec<egui::Pos2> = path
        .iter()
        .filter(|point| point.x.abs() <= map.reach())
        .map(|point| to_screen(point.x, point.y).0)
        .collect();
    for pair in path.windows(2) {
        let stroke = egui::Stroke::new(1.8_f32, colour(WORMHOLE_MAP_CAMERA_SRGBA, 0.9));
        painter.line_segment([pair[0], pair[1]], stroke);
    }
    let l = (place.l / shape.throat()) as f32;
    let (dot, _) = to_screen(l, angle_of(&place) as f32);
    if l.abs() <= map.reach() {
        painter.circle_filled(dot, 5.0, colour(WORMHOLE_MAP_CAMERA_SRGBA, 1.0));
    } else {
        // Further out than the diagram goes: held at its edge, hollow.
        let stroke = egui::Stroke::new(1.8_f32, colour(WORMHOLE_MAP_CAMERA_SRGBA, 1.0));
        painter.circle_stroke(dot, 5.0, stroke);
    }

    let font = egui::FontId::proportional(12.0);
    painter.text(
        panel.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        "near side",
        font.clone(),
        colour(WORMHOLE_MAP_NEAR_SRGBA, 1.0),
    );
    painter.text(
        panel.right_top() + egui::vec2(-10.0, 8.0),
        egui::Align2::RIGHT_TOP,
        format!(
            "l = {l:.2}   r = {:.2}",
            shape.radius(place.l) / shape.throat()
        ),
        font.clone(),
        colour(WORMHOLE_MAP_CAMERA_SRGBA, 1.0),
    );
    painter.text(
        panel.left_bottom() + egui::vec2(10.0, -8.0),
        egui::Align2::LEFT_BOTTOM,
        "far side",
        font.clone(),
        colour(WORMHOLE_MAP_FAR_SRGBA, 1.0),
    );
    painter.text(
        panel.right_bottom() + egui::vec2(-10.0, -8.0),
        egui::Align2::RIGHT_BOTTOM,
        WORMHOLE_MAP_CAPTION,
        font,
        egui::Color32::GRAY,
    );
}

/// Bevy system that shows or hides the geometry view on [`Action::ToggleWormholeMap`], except while
/// the menu is open.
pub fn toggle_wormhole_map_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    mut shown: ResMut<WormholeMapVisibility>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::ToggleWormholeMap) {
        shown.visible = !shown.visible;
    }
}

/// Draws the wormhole's geometry view, and its `G` toggle. Part of the HUD.
pub struct WormholeMapPlugin;

impl Plugin for WormholeMapPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }
        app.init_resource::<WormholeMapVisibility>();
        app.add_systems(
            Update,
            (
                toggle_wormhole_map_system,
                wormhole_map_system.run_if(crate::ui::hud::hud_shown),
            )
                .chain(),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/ui/wormhole_map_tests.rs"]
mod tests;
