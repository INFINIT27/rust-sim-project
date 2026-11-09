//! Motion trails: the last few units of each body's path, fading out behind it.
//!
//! A body's position tells you where it is; its trail tells you where it came from. That is
//! the difference between watching three points drift and *seeing* three orbits — which is
//! most of what the three-body experiment is for.
//!
//! # Length in distance, not time
//!
//! A trail is [`TRAIL_LENGTH_UNITS`] of **path**, not a number of frames or seconds. Under a
//! frame-rate-dependent variable timestep a fixed sample count would give a trail whose length
//! varied with the machine and with the clock scale — long during a fast pass, stubby when
//! paused-and-stepped. Measured in units it means the same thing everywhere: *this is the last
//! ten units the body travelled.*
//!
//! The tail is trimmed to **exactly** that length by sliding the last point along its segment,
//! so the trail does not pulse in and out by one sample's worth as new points arrive.
//!
//! # Fading is what makes it a trail rather than a wire
//!
//! Alpha runs from [`TRAIL_HEAD_ALPHA`] at the body to [`TRAIL_TAIL_ALPHA`] at the far end,
//! interpolated by *distance along the path*. Without the gradient the oldest position looks as
//! current as the newest and the trail reads as a static wire with no direction; with it, the
//! bright end is unambiguously "now".
//!
//! Each trail takes its **body's own colour**, so in the three-body ring the red body draws a
//! red trail. Following one orbit out of three is the whole reason that experiment colours its
//! bodies differently.
//!
//! # It is an observer aid, outside the model
//!
//! Like [`super::grid`] and [`crate::ui::compass`], nothing here touches the [`Scene`]. Trails
//! are *derived* from prop positions each frame into a resource this module owns; the
//! simulation cannot see them and they cannot perturb it. Recording continues while trails are
//! hidden, so pressing `T` again shows a full trail rather than one that starts growing from
//! nothing.
//!
//! # Sampled at the physics substeps, at a spacing sized to the orbit
//!
//! A trail draws the **absolute** path, the one the body really took through the world.
//! Two things have to be fine enough for that, and in the solar system neither was.
//!
//! - **Spacing.** A moon's loops around its planet are smaller than the solar system's
//!   `1.56`-unit spacing, so they fell between samples and the trail was a straight chord.
//!   [`moon_primary`] finds the moons (inside a heavier body's Hill sphere) and each one is
//!   sampled at [`TRAIL_MOON_SAMPLES_PER_ORBIT`] points per orbit instead.
//! - **Time.** One frame at the shipped clock is `11%` of Io's orbit, so a once-per-frame
//!   sample could never resolve the loop whatever the spacing. [`step_scene_system`] records
//!   after every physics substep through [`Scene::step_observed`], so the trail sees every
//!   position the integrator produced.
//!
//! [`step_scene_system`]: crate::render::plugin::step_scene_system
//!
//! # Drawn with gizmos, not entities
//!
//! [`draw_trails_system`] uses Bevy's immediate-mode [`Gizmos`], which is the right tool for
//! geometry that is different every frame: an entity-per-segment scheme would spawn and despawn
//! hundreds of entities per second to draw the same thing. The cost is that drawing cannot be
//! tested headlessly — gizmos need the render pipeline — so the geometry and the fade live in
//! [`TrailPath`], which is pure and carries the tests. See `tst/render/README.md`.

use std::collections::HashMap;

use bevy::app::{App, Plugin, Startup, Update};
use bevy::color::Color as BevyColor;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::gizmos::config::{GizmoConfigGroup, GizmoConfigStore};
use bevy::gizmos::gizmos::Gizmos;
use bevy::gizmos::AppGizmoBuilder;
use bevy::input::{keyboard::KeyCode, ButtonInput};
use bevy::math::Vec3;
use bevy::reflect::Reflect;

use crate::core::scene::moon_primary;
use crate::core::scene::{BodyId, Color, Scene};
use crate::experiments::catalog::ExperimentSpec;
use crate::input::menu::MenuState;
use crate::render::constants::{
    TRAIL_HEAD_ALPHA, TRAIL_LENGTH_UNITS, TRAIL_LINE_WIDTH_PX, TRAIL_MAX_SAMPLES,
    TRAIL_MAX_TURN_RADIANS, TRAIL_MIN_SAMPLE_SPACING_UNITS, TRAIL_MIN_SPACING_FRACTION,
    TRAIL_MOON_SAMPLES_PER_ORBIT, TRAIL_TAIL_ALPHA, TRAIL_TARGET_SEGMENTS,
};

/// The gizmo config group trails are drawn in.
///
/// A group of their own rather than Bevy's default one, so
/// [`TRAIL_LINE_WIDTH_PX`](crate::render::constants::TRAIL_LINE_WIDTH_PX) is a **trail** setting:
/// anything else this app later draws with gizmos gets its own width instead of inheriting the one
/// chosen to make trails legible.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct TrailGizmos;

/// One drawable piece of a trail: a segment, and the alpha at each end.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TrailSegment {
    /// The end nearer the body, in world units.
    pub from: Vec3,
    /// The end further from the body, in world units.
    pub to: Vec3,
    /// Alpha at [`from`](Self::from): closer to [`TRAIL_HEAD_ALPHA`].
    pub from_alpha: f32,
    /// Alpha at [`to`](Self::to): closer to [`TRAIL_TAIL_ALPHA`].
    pub to_alpha: f32,
}

/// One body's recent path, newest point first.
///
/// A deque rather than a `Vec`: points are pushed at the head and dropped from the tail every
/// frame, which is `O(1)` at both ends here and `O(n)` for a `Vec` at one of them.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{TrailPath, Vec3};
///
/// let mut path = TrailPath::default();
///
/// // Walk 8 units along X, sampling every 1 unit, keeping only the last 5.
/// for step in 0..=8 {
///     path.record(Vec3::new(step as f32, 0.0, 0.0), 5.0, 0.5, 64);
/// }
///
/// // Exactly 5 units of path survive, and the head is where the body is.
/// assert!((path.length_units() - 5.0).abs() < 1.0e-4);
/// assert_eq!(path.head(), Some(Vec3::new(8.0, 0.0, 0.0)));
///
/// // Brightest at the body, invisible at the far end.
/// let segments = path.segments(1.0, 0.0);
/// assert_eq!(segments.first().map(|s| s.from_alpha), Some(1.0));
/// assert!(segments.last().unwrap().to_alpha < 1.0e-4);
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrailPath {
    /// Newest first. Front is the body's current position.
    points: std::collections::VecDeque<Vec3>,
    /// Distance the body has travelled along its path from the newest committed sample
    /// (`points[1]`) to the live head (`points[0]`). See [`TrailPath::record`].
    head_travel_units: f32,
    /// How far the path has turned between the newest committed sample and the head, in
    /// radians, and the direction of its most recent movement.
    head_turn_radians: f32,
    heading: Option<Vec3>,
}

impl TrailPath {
    /// Record `point` as the body's current position, then trim to `max_length_units`.
    ///
    /// # Parameters
    ///
    /// - `point` — where the body is now. Ignored if non-finite: a `NaN` position would poison
    ///   every length and alpha downstream, and a dropped sample merely shortens the trail.
    /// - `max_length_units` — how much path to keep.
    /// - `min_spacing_units` — how far the body must have travelled along its path since the
    ///   last sample for a new one to be taken. Without it a nearly-stationary body fills the
    ///   buffer with coincident points, and the trail's length stops meaning anything.
    /// - `max_samples` — hard cap on stored points, so a pathological `min_spacing` cannot grow
    ///   the buffer without bound.
    ///
    /// # The head and the anchor are different points, and that is the whole trick
    ///
    /// `points[0]` is the **live head**: always exactly where the body is, so the trail cannot
    /// lag behind it. `points[1..]` are **committed samples**, and the spacing test is measured
    /// from `points[1]` — the newest committed one — never from the head.
    ///
    /// Measuring against the head is the obvious implementation and it is wrong, which this
    /// method got wrong first: the head is *overwritten* when a move is too small to commit, so
    /// comparing against it destroys the very anchor the comparison needs. A body moving less
    /// than `min_spacing_units` per frame then never commits anything at all — the path stays one
    /// point long forever, however far the body travels. Both shipped scenes are in exactly that
    /// range (the two-body pair moves ~`0.029` units per 60 Hz frame, the three-body ring
    /// ~`0.046`, against a `0.05` spacing), so the bug read as "trails barely appear".
    ///
    /// # Spacing is distance travelled, not straight-line distance
    ///
    /// The test is how far the body has gone **along its path** since the anchor. Measuring
    /// the straight line from the anchor instead loses any turn that goes out and back inside
    /// one spacing. Europa's world-coordinate path has near-cusps like that, and the trail cut
    /// straight across them, `0.04` units from where Europa actually went.
    ///
    /// Spacing alone still cuts a sharp turn by up to half a spacing, so a sample is also
    /// committed once the path has turned [`TRAIL_MAX_TURN_RADIANS`] since the anchor, provided
    /// it has travelled [`TRAIL_MIN_SPACING_FRACTION`] of a spacing. Samples gather where the
    /// path bends and stay sparse where it is straight.
    pub fn record(
        &mut self,
        point: Vec3,
        max_length_units: f32,
        min_spacing_units: f32,
        max_samples: usize,
    ) {
        if !point.is_finite() {
            return;
        }

        let spacing = min_spacing_units.max(0.0);

        match (self.points.front().copied(), self.points.get(1).copied()) {
            // Nothing recorded yet: this position is the whole path.
            (None, _) => {
                self.points.push_front(point);
                self.head_travel_units = 0.0;
                self.head_turn_radians = 0.0;
                self.heading = None;
            }

            // A position was recorded but there is no committed sample behind it yet. Any
            // movement at all promotes it: `[live, previous]` is the first drawable segment, and
            // waiting for a full spacing here would leave a just-launched body trailless.
            (Some(previous), None) => {
                if previous != point {
                    self.points.push_front(point);
                    self.head_travel_units = previous.distance(point);
                    self.head_turn_radians = 0.0;
                    self.heading = (point - previous).try_normalize();
                }
            }

            // The usual case. `anchor` is the newest *committed* sample, which the head slides
            // away from until it has earned a sample of its own.
            (Some(head), Some(_)) => {
                let step = head.distance(point);
                let direction = (point - head).try_normalize();
                let turn = match (self.heading, direction) {
                    (Some(before), Some(now)) => before.angle_between(now),
                    _ => 0.0,
                };
                let travel = self.head_travel_units + step;
                let turned = self.head_turn_radians + turn;
                if direction.is_some() {
                    self.heading = direction;
                }

                if travel >= spacing
                    || (turned >= TRAIL_MAX_TURN_RADIANS
                        && travel >= spacing * TRAIL_MIN_SPACING_FRACTION)
                {
                    // Commit: the old head becomes a permanent sample and `point` is the new head.
                    self.points.push_front(point);
                    self.head_travel_units = step;
                    self.head_turn_radians = 0.0;
                } else if let Some(head) = self.points.front_mut() {
                    // Slide the head to the body. The anchor at index 1 is untouched, so the
                    // next frame's comparison still measures from a fixed point.
                    *head = point;
                    self.head_travel_units = travel;
                    self.head_turn_radians = turned;
                }
            }
        }

        while self.points.len() > max_samples.max(2) {
            self.points.pop_back();
        }

        self.trim_to(max_length_units);
    }

    /// Drop and shorten from the tail until the path is at most `max_length_units` long.
    ///
    /// The final step slides the tail point *along its own segment* so the length is exactly the
    /// maximum rather than somewhere within one sample of it. That is what stops the trail
    /// pulsing shorter and longer as samples arrive.
    fn trim_to(&mut self, max_length_units: f32) {
        if !max_length_units.is_finite() || max_length_units <= 0.0 {
            self.points.clear();
            return;
        }

        // Walk from the head, accumulating length, and cut where the budget runs out.
        let mut used = 0.0_f32;
        let mut keep = 1_usize;
        let mut cut_at: Option<Vec3> = None;

        for (near, far) in self
            .points
            .iter()
            .copied()
            .zip(self.points.iter().copied().skip(1))
        {
            let span = near.distance(far);

            if used + span >= max_length_units {
                let remaining = max_length_units - used;
                // `span` is at least `remaining` here, and `remaining > 0` unless the budget was
                // already exhausted exactly — in which case the tail lands on `near`.
                let fraction = if span > 0.0 { remaining / span } else { 0.0 };
                cut_at = Some(near + (far - near) * fraction);
                break;
            }

            used += span;
            keep += 1;
        }

        self.points.truncate(keep);

        if let Some(tail) = cut_at {
            self.points.push_back(tail);
        }
    }

    /// The body's current position, or `None` for an empty path.
    #[inline]
    pub fn head(&self) -> Option<Vec3> {
        self.points.front().copied()
    }

    /// Total path length in world units.
    pub fn length_units(&self) -> f32 {
        self.points
            .iter()
            .zip(self.points.iter().skip(1))
            .map(|(near, far)| near.distance(*far))
            .sum()
    }

    /// How many points are stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Is there nothing to draw?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.len() < 2
    }

    /// Forget the path.
    #[inline]
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Validation_Rules for `TrailPath`: every stored point is finite.
    ///
    /// `record` refuses a non-finite point, so this holds by construction; it exists so the
    /// drawing system can assert what it relies on before handing values to a painter.
    pub fn is_valid(&self) -> bool {
        self.points.iter().all(|point| point.is_finite())
    }

    /// The path as drawable segments, with alpha interpolated by distance from the head.
    ///
    /// `head_alpha` applies at the body and `tail_alpha` at the far end of the *stored* path —
    /// so a trail that has not yet grown to full length fades over the length it has, rather
    /// than being uniformly bright until it does. Returns an empty `Vec` for a path with fewer
    /// than two points.
    pub fn segments(&self, head_alpha: f32, tail_alpha: f32) -> Vec<TrailSegment> {
        let total = self.length_units();
        if self.is_empty() || !total.is_finite() || total <= 0.0 {
            return Vec::new();
        }

        let alpha_at = |distance_from_head: f32| {
            let fraction = (distance_from_head / total).clamp(0.0, 1.0);
            head_alpha + (tail_alpha - head_alpha) * fraction
        };

        let mut segments = Vec::with_capacity(self.points.len().saturating_sub(1));
        let mut travelled = 0.0_f32;

        for (near, far) in self.points.iter().zip(self.points.iter().skip(1)) {
            let span = near.distance(*far);
            let from_alpha = alpha_at(travelled);
            travelled += span;

            segments.push(TrailSegment {
                from: *near,
                to: *far,
                from_alpha,
                to_alpha: alpha_at(travelled),
            });
        }

        segments
    }
}

/// Every body's trail, keyed by [`BodyId`].
///
/// An ECS resource owned by this module rather than state on `Prop`: a trail is something the
/// *viewer* wants, and putting it in the `Scene` would mean the physics model carried a
/// rendering concern — and that `Scene`'s equality, cloning, and validity all had to account
/// for it.
#[derive(Resource, Clone, Debug, Default)]
pub struct Trails {
    paths: HashMap<BodyId, TrailPath>,
}

impl Trails {
    /// The trail for `id`, if one has been recorded.
    #[inline]
    pub fn path(&self, id: BodyId) -> Option<&TrailPath> {
        self.paths.get(&id)
    }

    /// How many bodies are being trailed.
    #[inline]
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Is nothing being trailed?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Forget every trail.
    #[inline]
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Record the current position of every **movable** prop in `scene`.
    ///
    /// Immovable props are skipped: the floor does not move, and a trail on it would be a
    /// single point drawn every frame forever.
    ///
    /// A moon (see [`moon_primary`]) is sampled at [`TRAIL_MOON_SAMPLES_PER_ORBIT`] points per
    /// orbit around its primary, and its trail is [`TRAIL_TARGET_SEGMENTS`] of those samples
    /// long, capped at the scene's trail length. Every point is in world coordinates.
    ///
    /// Cheap enough to call after every physics substep, which is what
    /// [`crate::render::plugin::step_scene_system`] does.
    ///
    /// Trails for ids no longer in the scene are dropped, so a scene that loses a prop does not
    /// leave a trail hanging in the air. Nothing removes props today; this is what keeps that
    /// true when something does.
    pub fn record_from(&mut self, scene: &Scene, config: &TrailConfig) {
        let mut seen = Vec::with_capacity(scene.props.len());

        for (id, prop) in scene.iter() {
            if prop.immovable {
                continue;
            }

            seen.push(id);
            let (length_units, spacing_units) = match moon_primary(scene, id) {
                None => (config.length_units, config.min_spacing_units),
                Some((_, orbit_radius_units)) => {
                    let spacing = std::f32::consts::TAU * orbit_radius_units
                        / TRAIL_MOON_SAMPLES_PER_ORBIT as f32;
                    let length = config
                        .length_units
                        .min(spacing * TRAIL_TARGET_SEGMENTS as f32);
                    (length, spacing)
                }
            };
            self.paths.entry(id).or_default().record(
                prop.body.center(),
                length_units,
                spacing_units,
                config.max_samples,
            );
        }

        self.paths.retain(|id, _| seen.contains(id));
    }

    /// Every trail with its `BodyId`, for the drawing system.
    pub fn iter(&self) -> impl Iterator<Item = (BodyId, &TrailPath)> + '_ {
        self.paths.iter().map(|(id, path)| (*id, path))
    }
}

/// Whether trails are shown, and how they are shaped.
///
/// All fields are `pub` and unvalidated on assignment, matching the crate's other config
/// resources; the recording and drawing code checks what it needs and degrades benignly.
#[derive(Resource, Debug, Copy, Clone, PartialEq)]
pub struct TrailConfig {
    /// Are trails drawn? Toggled by `T`, on by default.
    ///
    /// Recording continues while this is `false`, so re-enabling shows a full trail immediately
    /// rather than one that grows from nothing — a toggle that lost history would make the key
    /// feel destructive rather than like a view setting.
    pub visible: bool,
    /// How much **path** each trail keeps, in world units.
    pub length_units: f32,
    /// How far a body must move before a new sample is taken, in world units.
    pub min_spacing_units: f32,
    /// Hard cap on stored samples per body.
    pub max_samples: usize,
    /// Alpha at the body end.
    pub head_alpha: f32,
    /// Alpha at the far end.
    pub tail_alpha: f32,
}

impl Default for TrailConfig {
    /// Visible, [`TRAIL_LENGTH_UNITS`] long, fading from [`TRAIL_HEAD_ALPHA`] to
    /// [`TRAIL_TAIL_ALPHA`].
    fn default() -> Self {
        Self {
            visible: true,
            length_units: TRAIL_LENGTH_UNITS,
            min_spacing_units: TRAIL_MIN_SAMPLE_SPACING_UNITS,
            max_samples: TRAIL_MAX_SAMPLES,
            head_alpha: TRAIL_HEAD_ALPHA,
            tail_alpha: TRAIL_TAIL_ALPHA,
        }
    }
}

/// Bevy system that records every movable prop's position into [`Trails`], once per frame.
///
/// [`crate::render::plugin::step_scene_system`] already records after every substep when
/// trails exist. This pass catches the frames it does not cover (a paused clock, or a scene
/// stepped by something else), and repeating the final position changes nothing.
///
/// Runs unconditionally, including while trails are hidden — see [`TrailConfig::visible`].
pub fn record_trails_system(
    scene: Res<Scene>,
    config: Res<TrailConfig>,
    mut trails: ResMut<Trails>,
) {
    trails.record_from(&scene, &config);
}

/// Bevy system that toggles trail visibility on `T`.
///
/// Edge-triggered (`just_pressed`), so holding the key does not flicker the setting sixty times
/// a second — the same discipline the clock controls in [`crate::input::controls`] follow.
///
/// # Why the binding lives in the render layer
///
/// `T` controls a *render* setting, and this module owns it. Putting the binding in
/// `input/controls.rs` — which owns the clock keys — would mean `input` depending on `render`,
/// inverting the direction that already exists (`render` reads `input`'s `FreeFlyController`).
/// `controls_tests` asserts that no binding collides across the two.
pub fn toggle_trails_system(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<MenuState>>,
    mut config: ResMut<TrailConfig>,
) {
    // While the menu is open, only `Esc` is bound (amendment A13) — the menu *shows* the
    // trail setting, so a `T` that changed it underneath the reading would be worse than a
    // `T` that does nothing.
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }

    if keys.just_pressed(KeyCode::KeyT) {
        config.visible = !config.visible;
    }
}

/// Startup system that lets the running experiment choose its own trail length.
///
/// A trail length only means something against the size of the path it marks: the default
/// [`TRAIL_LENGTH_UNITS`] (`10`) is a third of the two-body scene's width and about a
/// hundredth of one Earth orbit. An experiment that says so in
/// [`ExperimentSpec::trail_length_units`] gets its own length here; one that says `None`
/// keeps the default, which is why the two- and three-body trails are untouched by this.
///
/// # The spacing has to move with it
///
/// Sample spacing is **derived**, not left alone: `length / `[`TRAIL_TARGET_SEGMENTS`],
/// floored at [`TRAIL_MIN_SAMPLE_SPACING_UNITS`]. Keeping the default `0.05`-unit spacing
/// for a `400`-unit trail would ask for `8000` samples, hit the
/// [`TRAIL_MAX_SAMPLES`] cap at `512`, and silently draw a trail a twentieth of its stated
/// length — a bug that would look like the length constant not working. Deriving it keeps
/// the cost per body flat at whatever length is asked for.
///
/// # Why a system rather than a different default
///
/// [`TrailConfig`] is a resource with a `Default`, and `TrailPlugin` may be added before or
/// after `ScenePlugin`. Reading the spec at `Startup` — after every plugin's `build` has
/// run, so the resource is certainly there if it is coming at all — avoids depending on
/// plugin order. `Option<Res<..>>` covers the case where there is no experiment at all,
/// which is what most of the render tests are.
pub fn apply_experiment_trail_length(
    spec: Option<Res<ExperimentSpec>>,
    mut config: ResMut<TrailConfig>,
) {
    let Some(spec) = spec else {
        return;
    };
    let Some(length_units) = spec.trail_length_units else {
        return;
    };

    debug_assert!(
        length_units.is_finite() && length_units > 0.0,
        "apply_experiment_trail_length: trail length must be finite and positive, got {}",
        length_units
    );

    if !length_units.is_finite() || length_units <= 0.0 {
        return;
    }

    config.length_units = length_units;
    config.min_spacing_units =
        (length_units / TRAIL_TARGET_SEGMENTS as f32).max(TRAIL_MIN_SAMPLE_SPACING_UNITS);
}

/// Bevy system that draws each trail as a fading gizmo polyline.
///
/// Immediate-mode: nothing is retained between frames, which is what makes per-frame geometry
/// cheap. Each segment is a `line_gradient` from its brighter end to its dimmer one, in the
/// body's own colour.
///
/// Draws nothing when [`TrailConfig::visible`] is `false`.
///
/// Lines are [`TRAIL_LINE_WIDTH_PX`](crate::render::constants::TRAIL_LINE_WIDTH_PX) wide, set on
/// the [`TrailGizmos`] group by [`TrailPlugin`]. That width is in **screen pixels**, so a trail
/// stays equally legible whether its body is ten units away or two hundred — right for an
/// annotation, wrong for an object, which is part of why bodies are meshes and trails are not.
///
/// # Not headless-testable
///
/// `Gizmos` resolves against the render pipeline's config store, which `MinimalPlugins` does not
/// provide — so this system needs a real windowed app, exactly like the egui drawers, and
/// [`TrailPlugin`] does not register it when there is no store. The geometry it draws is
/// [`TrailPath::segments`], which is pure and tested; this function is deliberately thin enough
/// to read as obviously correct.
pub fn draw_trails_system(
    config: Res<TrailConfig>,
    scene: Res<Scene>,
    trails: Res<Trails>,
    mut gizmos: Gizmos<TrailGizmos>,
) {
    if !config.visible {
        return;
    }

    for (id, path) in trails.iter() {
        debug_assert!(
            path.is_valid(),
            "draw_trails_system: trail for {:?} holds a non-finite point",
            id
        );

        // The body's own colour, so one orbit can be followed out of three. A prop that has
        // left the scene keeps no trail (see `Trails::record_from`), so a missing colour here
        // means the id was removed this very frame.
        let Some(color) = scene.get(id).map(|prop| body_color(&prop.body)) else {
            continue;
        };

        for segment in path.segments(config.head_alpha, config.tail_alpha) {
            gizmos.line_gradient(
                segment.from,
                segment.to,
                with_alpha(color, segment.from_alpha),
                with_alpha(color, segment.to_alpha),
            );
        }
    }
}

/// A body's colour, whichever variant it is.
fn body_color(body: &crate::core::scene::Body) -> Color {
    use crate::core::scene::Body;

    match body {
        Body::Sphere { color, .. }
        | Body::Cuboid { color, .. }
        | Body::Line { color, .. }
        | Body::Plane { color, .. } => *color,
    }
}

/// The scene colour as a Bevy colour with `alpha` substituted.
///
/// Linear space, matching [`Color`]'s contract and `to_bevy_color` in
/// [`super::plugin`] — gizmos take the same `bevy::color::Color`, so an sRGB value here would
/// wash out against the bodies its trail belongs to.
fn with_alpha(color: Color, alpha: f32) -> BevyColor {
    BevyColor::linear_rgba(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
}

/// Draws a fading trail behind every moving body, toggled with `T`.
///
/// Inserts [`Trails`] and [`TrailConfig`], and registers the record → draw pair plus the toggle
/// on `Update`. Recording is ordered **after** the scene has been stepped and synced, so a
/// trail's head is the position the body was drawn at this frame rather than the previous one.
///
/// Add it after [`ScenePlugin`](super::plugin::ScenePlugin), which inserts the `Scene` this
/// reads.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use physics_sandbox::{ScenePlugin, TrailPlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(ScenePlugin::default())
///     .add_plugins(TrailPlugin)
///     .run();
/// ```
pub struct TrailPlugin;

impl Plugin for TrailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Trails>();
        app.init_resource::<TrailConfig>();
        // See `FreeFlyCameraPlugin`: the gate's resource, so this plugin runs standalone.
        app.init_resource::<MenuState>();

        app.add_systems(Startup, apply_experiment_trail_length);

        app.add_systems(
            Update,
            (
                toggle_trails_system,
                record_trails_system.after(super::bridge::sync_scene_to_ecs_system),
            ),
        );

        // Gizmos need `GizmoPlugin`, which arrives with `DefaultPlugins` and not with
        // `MinimalPlugins`. Without it, initialising the group panics and the draw system cannot
        // resolve its `Gizmos` parameter — so recording and the toggle are registered
        // unconditionally and only *drawing* depends on a render pipeline being present. That is
        // what lets a headless app hold trails without pretending it can paint them, and it is
        // covered by `the_plugin_records_trails_without_a_render_pipeline`.
        if app.world().get_resource::<GizmoConfigStore>().is_some() {
            app.init_gizmo_group::<TrailGizmos>();

            // Bound separately and scoped: `resource_mut` returns a guard, and chaining
            // `config_mut` off it would drop the guard while the borrow is still live.
            {
                let mut store = app.world_mut().resource_mut::<GizmoConfigStore>();
                let (gizmo_config, _) = store.config_mut::<TrailGizmos>();
                gizmo_config.line_width = TRAIL_LINE_WIDTH_PX;
            }

            app.add_systems(Update, draw_trails_system.after(record_trails_system));
        }
    }
}

#[cfg(test)]
#[path = "../../tst/render/trail_tests.rs"]
mod tests;
