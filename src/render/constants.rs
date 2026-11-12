//! Every constant the `render` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module.
//! Nothing about *time* is here: the frame-step cap and the clock scales belong to
//! [`crate::time::constants`], and the shipped scene's own scale is
//! [`crate::constants::TWO_BODY_TIME_SCALE`], because `core` builds the scene those
//! timings belong to.
//!
//! # Sections
//!
//! - **The background** — [`BACKGROUND_COLOR`], what the camera clears to.
//! - **Eclipse shadows** — [`MAX_ECLIPSE_OCCLUDERS`] and [`ECLIPSE_SHADER_PATH`], the analytic
//!   shadow a body casts on another.
//! - **Starlight** — the `STAR_*` values: a luminous prop lights the scene from its own
//!   position, with inverse-square falloff, instead of everything being lit by a fixed
//!   directional light.
//! - **Lighting** — where
//!   [`spawn_scene_entities`](crate::render::plugin::spawn_scene_entities) puts the
//!   light. The *camera* pose is per experiment and arrives via
//!   [`crate::experiments::catalog::ExperimentSpec`].
//! - **Mesh proxies** — [`LINE_VISUAL_THICKNESS_UNITS`], the one place a rendered
//!   shape differs from the model.
//! - **Motion trails** — the `TRAIL_*` values behind [`crate::render::trail`]. Length is in
//!   *distance*, not time, so it means the same thing at any frame rate.
//! - **Object placement** — how far in front of the camera the `B` key drops an object.
//! - **The camera's far plane** — [`CAMERA_FAR_UNITS`]. The camera's *pose* is per
//!   experiment and arrives in the spec; how far it can see is a property of the renderer,
//!   so it lives here.
//! - **The floor grid** — the `GRID_*` values behind [`crate::render::grid`], the tracing
//!   aid drawn on the ground plane. Two spacings: a fine one out to
//!   [`GRID_CORE_EXTENT_UNITS`] and a coarse one over the whole floor. Visual only: no grid
//!   constant affects physics.
//! - **Light streams** — the `LIGHT_STREAM_*` values behind [`crate::render::light_streams`]: where
//!   the rays start, how many, how often, how they are stepped, and how they are drawn.
//! - **The accretion disc** — the `DISC_*` and `ERGOSPHERE_*` values behind
//!   [`crate::render::accretion_disc`]: its size, accretion rate, inflow, plunge, and drawing.

pub use bevy::math::Vec3;

use crate::core::scene::Color;

// The frame-step cap and the default clock scale used to live here. They are time
// policy, not render policy, so they moved to `crate::time::constants` as
// `MAX_REAL_STEP_SECONDS` and `DEFAULT_TIME_SCALE` when `SimulationClock` took over
// the timestep (amendment A3).

// The startup camera pose used to live here, as a single constant. Framing is *per
// experiment* — a three-body ring 12 units across wants a different viewpoint than a
// two-body pair 30 units apart — so it moved to `experiments::constants` and reaches
// `spawn_scene_entities` through the `ExperimentSpec` resource (amendment A4).

/// Where the directional light is placed, in world units.
///
/// **Why this value.** PBR materials need a light source or every non-emissive body
/// renders as a uniform silhouette. For a *directional* light only the rotation
/// matters, and `looking_at(ZERO)` from `(4, 8, 4)` gives a slanted angle across the
/// ground plane — enough to shade the spheres' curvature, which is what makes them
/// read as spheres rather than discs.
///
/// **Used by.** [`crate::render::plugin::spawn_scene_entities`], once at startup.
///
/// **If you change it.** Only the *direction* from here to the origin has any
/// effect; the distance is irrelevant for a directional light. Aim it near-vertically
/// and the spheres flatten out; aim it near-horizontally and half the scene goes dark.
pub const LIGHT_POSITION_UNITS: Vec3 = Vec3::new(4.0, 8.0, 4.0);

/// Cross-section of the rod that stands in for a `Body::Line`, in world units.
///
/// **Why this value.** A line has no thickness, and a mesh must. `0.01` units reads
/// as a line at the scene's typical viewing distances while still being a real
/// cuboid the renderer can shade.
///
/// **Used by.** [`crate::render::plugin::spawn_scene_entities`], for the `Body::Line`
/// mesh proxy. `transform_from` sets `scale.x` to the line's length, so this is the
/// `y` and `z` extent.
///
/// **If you change it.** Visual only — and note it is deliberately **not** the
/// collision thickness. A line's hull is built from its own `width` field (see
/// [`crate::collision::tessellate::collision_mesh`]), so a rendered rod at `0.01`
/// and a `width` of `0.5` will look thin and collide thick. That divergence is why
/// the value is named for what it is.
pub const LINE_VISUAL_THICKNESS_UNITS: f32 = 0.01;

/// Distance between adjacent minor grid lines, in world units.
///
/// **Why this value.** `5` units against a `100`-unit floor gives 21 lines per direction:
/// dense enough that a body's motion is visible against them frame to frame, sparse enough
/// that the floor does not read as a solid sheet from the starting camera 45 units out. It
/// is also comfortably larger than the shipped bodies (`1.0`–`1.5` units), so a body never
/// spans a cell and its position within one is readable.
///
/// **Used by.** [`crate::render::grid::spawn_floor_grid`], which passes it to
/// [`floor_grid_segments`](crate::render::grid::floor_grid_segments).
///
/// **If you change it.** Line count is `2 · (2 · ⌊half_size/spacing⌋ + 1)`, so halving
/// this quadruples the entities — 162 at `2.5` units, which is still trivial but starts to
/// look like graph paper. Larger than the floor's half-size produces no grid at all, which
/// `floor_grid_segments` returns as an empty list rather than one line through the middle.
pub const GRID_SPACING_UNITS: f32 = 5.0;

/// How many minor spacings make a major line.
///
/// **Why this value.** Every fifth line, so a major line every `25` units — four cells
/// across a `100`-unit floor, which is the coarse scale you count in when judging "how far
/// did it go". Five is also how graph paper does it, and matching a convention people
/// already read is worth more here than any other value.
///
/// **Used by.** [`crate::render::grid::floor_grid_segments`], which classifies a line as
/// major when its index is a multiple of this.
///
/// **If you change it.** `1` makes every line major, which defeats having two weights;
/// larger than the number of lines per side leaves no major lines at all, which is legal
/// and just means a uniform grid. The classification is integer arithmetic on the line
/// index, so no value here can produce an off-by-one at the edges.
pub const GRID_MAJOR_EVERY: u32 = 5;

/// Cross-section of a minor grid line, in world units.
///
/// **Why this value.** `0.05` units is thin enough to read as a line rather than a stripe
/// from the starting camera distance, and thick enough not to alias into a dotted mess as
/// the camera moves — a one-pixel line with no anti-aliasing shimmers, which is precisely
/// the distraction a tracing aid must not add.
///
/// **Used by.** [`GridLineKind::thickness_units`](crate::render::grid::GridLineKind::thickness_units).
///
/// **If you change it.** These are rendered as scaled cuboids, so thickness costs nothing
/// — it is the same vertex count either way. Keep the three weights distinguishable:
/// minor < major < axis is what lets the grid be read at a glance.
pub const GRID_MINOR_THICKNESS_UNITS: f32 = 0.05;

/// Cross-section of a major grid line, in world units.
///
/// **Why this value.** Roughly `2.5×` a minor line, which is the smallest ratio that reads
/// as deliberate rather than as inconsistent rendering.
///
/// **Used by.** `GridLineKind::thickness_units`.
///
/// **If you change it.** See [`GRID_MINOR_THICKNESS_UNITS`].
pub const GRID_MAJOR_THICKNESS_UNITS: f32 = 0.12;

/// Cross-section of an axis line, in world units.
///
/// **Why this value.** Thickest of the three, because the two axis lines are the grid's
/// origin marker: they answer "where is `(0, 0)`" without counting cells.
///
/// **Used by.** `GridLineKind::thickness_units`.
///
/// **If you change it.** See [`GRID_MINOR_THICKNESS_UNITS`].
pub const GRID_AXIS_THICKNESS_UNITS: f32 = 0.2;

/// How far above the floor's surface the grid is drawn, in world units.
///
/// **Why this value.** Not aesthetic — **required**. Drawing the lines exactly at the
/// surface puts two coplanar surfaces in the depth buffer, and the winner varies per pixel
/// and per frame: the grid flickers and tears as the camera moves. `0.02` units is the
/// classic decal offset — large enough to win the depth test at any viewing distance in
/// this scene, small enough that the lines still read as painted on the floor rather than
/// floating above it.
///
/// **Used by.** [`crate::render::grid::floor_grid_segments`], applied to every segment's
/// `y`.
///
/// **If you change it.** Zero brings the z-fighting back. Large values make the grid
/// visibly hover, and — since the floor's collision slab hangs *below* the surface — a
/// grid lifted more than a body's radius would appear to pass through bodies resting on
/// the floor. Note the lines are visual only, so this never affects contact.
pub const GRID_LIFT_UNITS: f32 = 0.02;

/// Colour of a minor grid line, in linear RGBA.
///
/// **Why this value.** A little lighter than
/// [`DEMO_FLOOR_COLOR`](crate::core::constants::DEMO_FLOOR_COLOR) (`0.35` grey), so the
/// lines are legible against the floor without competing with the bodies for attention.
/// The grid is a reference, not a subject.
///
/// **Used by.** `GridLineKind::color`.
///
/// **If you change it.** Linear space, not sRGB — the materials are built with
/// `linear_rgba`. Channels outside `[0.0, 1.0]` are not rejected here (this is not a
/// `Scene` colour, so `Color::is_valid` never sees it), so keep them in range by hand.
pub const GRID_MINOR_COLOR: Color = Color {
    r: 0.46,
    g: 0.46,
    b: 0.50,
    a: 1.0,
};

/// Colour of a major grid line, in linear RGBA.
///
/// **Why this value.** Lighter than a minor line by about as much as a minor line is
/// lighter than the floor, so the three tiers step evenly and the eye can order them.
///
/// **Used by.** `GridLineKind::color`.
///
/// **If you change it.** See [`GRID_MINOR_COLOR`].
pub const GRID_MAJOR_COLOR: Color = Color {
    r: 0.62,
    g: 0.62,
    b: 0.68,
    a: 1.0,
};

/// Colour of the line along the **X** axis, in linear RGBA.
///
/// **Why this value.** A desaturated red. Red for X follows the convention every 3D tool
/// uses, and desaturated so the axes mark the origin without looking like a body — the
/// two-body spheres are red and blue, and a saturated red axis line would compete with
/// one of them.
///
/// **Used by.** `GridLineKind::color`.
///
/// **If you change it.** Keep it distinguishable from
/// [`GRID_AXIS_Z_COLOR`] and from the body colours in
/// [`crate::core::constants`] and [`crate::experiments::constants`].
pub const GRID_AXIS_X_COLOR: Color = Color {
    r: 0.62,
    g: 0.30,
    b: 0.30,
    a: 1.0,
};

/// Colour of the line along the **Z** axis, in linear RGBA.
///
/// **Why this value.** A desaturated blue, by the same convention and for the same reason
/// as [`GRID_AXIS_X_COLOR`]. (Blue is conventionally Z; Y is up and has no floor line to
/// draw.)
///
/// **Used by.** `GridLineKind::color`.
///
/// **If you change it.** See [`GRID_AXIS_X_COLOR`].
pub const GRID_AXIS_Z_COLOR: Color = Color {
    r: 0.30,
    g: 0.38,
    b: 0.62,
    a: 1.0,
};

/// Furthest the **fine** grid extends from the origin, in world units.
///
/// Beyond this the grid continues at [`GRID_COARSE_SPACING_UNITS`] instead of stopping — see
/// [`crate::render::grid::GridLayout`] for the three tiers and why they exist.
///
/// **Why this value.** `60` units covers the scene's `±50` bounds with a margin, which is the
/// region bodies actually move through and therefore the only place a 5-unit rule is worth
/// reading. Extending it to the whole `2000`-unit floor would be 1,602 line entities whose
/// distant half resolves to a grey moiré — denser than the pixels available to draw it.
///
/// **Used by.** [`crate::render::grid::grid_core_extent_units`], which takes the smaller of
/// this and the floor's own half-size — so a *small* floor still gets a fine grid that matches
/// it exactly, and only a large one is capped.
///
/// **If you change it.** Fine-line count is `2 · (2 · ⌊extent/spacing⌋ + 1)`, so this trades
/// entities for fine coverage linearly. Below the bounds' extent the fine grid stops inside
/// the region bodies move through, which is the one place it must not.
pub const GRID_CORE_EXTENT_UNITS: f32 = 60.0;

/// Spacing of the **coarse** grid, which spans the whole floor, in world units.
///
/// **Why this value.** `100` units is the coarsest rule that still gives the eye something to
/// judge distance by at the far end of the plane, and it divides the floor's `2000`-unit
/// half-size exactly, so the outermost line lands *on* the edge rather than short of it. Over
/// the shipped floor it costs 80 entities, against the 1,602 a 5-unit rule would need for the
/// same coverage — a twentieth of the cost for the part of the view where a 5-unit rule would
/// be sub-pixel anyway.
///
/// It is also a whole multiple of [`GRID_SPACING_UNITS`], which matters: a coarse line always
/// falls where a fine line would, so the two tiers line up instead of interleaving into a
/// beat pattern.
///
/// **Used by.** [`crate::render::grid::GridLayout::for_floor`], and through it every coarse
/// line [`crate::render::grid::floor_grid_segments`] emits.
///
/// **If you change it.** Keep it a whole multiple of `GRID_SPACING_UNITS` and, ideally, a
/// divisor of the floor's half-size. Coarse-line count is `2 · (2 · ⌊half_size/this⌋ + 1)`, so
/// halving it quadruples the count over a floor this size.
pub const GRID_COARSE_SPACING_UNITS: f32 = 100.0;

/// How much **path** a motion trail keeps, in world units.
///
/// **Why this value.** `10` units is two fine grid cells, so a trail's length can be read
/// straight off the floor, and about seven of the two-body spheres' diameters — long enough to
/// show the *curvature* of a path, which is what distinguishes an orbit from a fall. It was `5`,
/// which was legible in principle and too short to follow in practice.
///
/// **Used by.** `TrailConfig::default`, and through it every
/// [`TrailPath::record`](crate::render::trail::TrailPath::record).
///
/// **If you change it.** Measured in **distance, not time**, so it means the same thing at any
/// frame rate or clock scale. Longer trails cost samples in proportion
/// (`length / min_spacing`, capped by [`TRAIL_MAX_SAMPLES`] — check that cap is still above the
/// ratio) and start to obscure the bodies they belong to; much shorter and the fade has too
/// little length to read as a gradient.
pub const TRAIL_LENGTH_UNITS: f32 = 10.0;

/// How far a body must move before a new trail sample is taken, in world units.
///
/// **Why this value.** `0.05` units gives up to 100 samples over a `5`-unit trail, which is
/// smooth enough that a curve reads as a curve rather than a chain of chords. It also stops a
/// nearly-stationary body from filling its buffer with coincident points, at which stage the
/// trail's *length* would stop meaning anything — the failure this constant exists to prevent.
///
/// **Used by.** `TrailConfig::default`, consumed by `TrailPath::record`.
///
/// **If you change it.** Sample count is `length / this`, so halving it doubles the work per
/// frame and the memory per body. Note the head is always moved to the body's exact position
/// even when no new sample is taken, so a large spacing makes the trail *chunkier* but never
/// makes it lag behind the body.
pub const TRAIL_MIN_SAMPLE_SPACING_UNITS: f32 = 0.05;

/// Hard cap on stored samples per trail.
///
/// **Why this value.** `512` is comfortably above the ~200 a `10`-unit trail at the shipped
/// spacing needs, so it never bites in normal operation — it exists so a mis-set
/// [`TRAIL_MIN_SAMPLE_SPACING_UNITS`] of zero cannot grow the buffer without bound. Doubling the
/// trail's length doubled the requirement, which is why this moved with it.
///
/// **Used by.** `TrailConfig::default`, consumed by `TrailPath::record`.
///
/// **If you change it.** A cap below `length / min_spacing` silently shortens trails, which
/// looks like a bug in the length rather than in the cap. Below `2` no trail can be drawn at
/// all, so `record` floors it there.
pub const TRAIL_MAX_SAMPLES: usize = 512;

/// How many segments a trail is aimed at when an experiment asks for its own length.
/// Dimensionless.
///
/// **Why this value.** [`TRAIL_MIN_SAMPLE_SPACING_UNITS`] is the right spacing for a
/// `10`-unit trail and hopeless for a `400`-unit one: at `0.05` units a solar-system trail
/// would want `8000` samples and be truncated to [`TRAIL_MAX_SAMPLES`] at a twentieth of
/// its stated length — the trail would silently stop being as long as it says.
/// [`apply_experiment_trail_length`](crate::render::trail::apply_experiment_trail_length)
/// therefore derives the spacing as `length / this`, so **cost per body is flat whatever
/// length an experiment asks for**.
///
/// `256` segments is smooth enough that a full circle reads as a circle (a `1.4°` chord),
/// and it sits at half of `TRAIL_MAX_SAMPLES`, leaving the cap as the backstop it is meant
/// to be rather than the thing that decides the answer.
///
/// **Used by.** `apply_experiment_trail_length`, when
/// [`ExperimentSpec::trail_length_units`](crate::experiments::catalog::ExperimentSpec::trail_length_units)
/// is `Some`. An experiment that does not ask keeps `TRAIL_MIN_SAMPLE_SPACING_UNITS`
/// exactly, so the shipped two- and three-body trails are unchanged by this.
///
/// **If you change it.** It must stay at or below `TRAIL_MAX_SAMPLES` or the cap starts
/// truncating trails again. Note the derived spacing is floored at
/// `TRAIL_MIN_SAMPLE_SPACING_UNITS`, so a short requested length cannot make the sampling
/// finer than the default.
pub const TRAIL_TARGET_SEGMENTS: usize = 256;

/// How far a trail's path may turn between two samples before a new one is taken, in radians.
///
/// **Why this value.** A sample spacing alone cuts across a sharp turn by up to half a
/// spacing. Europa's absolute path has near-cusps where it almost stops and turns back, and at
/// its spacing the trail missed them by `0.03` units. Committing a sample every `10°` of turn
/// bounds that error at about `length × angle / 8`, under `0.002` units for Europa, and costs
/// samples only where the path bends.
///
/// **Used by.** [`crate::render::trail::TrailPath::record`].
///
/// **If you change it.** Smaller angles add samples at every bend, and a moon's trail can then
/// reach [`TRAIL_MAX_SAMPLES`] and be cut short. Much larger and cusps get flattened again.
pub const TRAIL_MAX_TURN_RADIANS: f32 = 10.0 * std::f32::consts::PI / 180.0;

/// The shortest a segment can be when it is committed for turning rather than for distance, as
/// a fraction of the sample spacing. Dimensionless.
///
/// **Why this value.** Without a floor a body jittering in place would turn through large
/// angles over no distance and fill its buffer with coincident samples, which is the failure
/// [`TRAIL_MIN_SAMPLE_SPACING_UNITS`] exists to prevent. `1/16` of a spacing still lets a cusp
/// collect a dozen samples.
///
/// **Used by.** [`crate::render::trail::TrailPath::record`], with [`TRAIL_MAX_TURN_RADIANS`].
///
/// **If you change it.** At `1.0` turning never commits early and cusps are cut again. Near
/// `0.0` jitter can fill the buffer.
pub const TRAIL_MIN_SPACING_FRACTION: f32 = 1.0 / 16.0;

/// How many trail samples a moon gets per orbit around its primary. Dimensionless.
///
/// **Why this value.** A moon's absolute path is its planet's path plus a loop, or a wave
/// when the moon is slower than its planet, one orbit across. At the solar system's
/// `1.56`-unit spacing that loop falls between two samples and vanishes. `64` samples per
/// orbit is a `5.6°` chord, smooth enough for Io's loops to read as loops. The trail keeps the
/// usual [`TRAIL_TARGET_SEGMENTS`] budget, so it covers `4` orbits' worth of distance, about
/// `10.6` units for Io and `9.7` for Earth's Moon.
///
/// **Used by.** [`crate::render::trail::Trails::record_from`], for bodies that
/// [`crate::core::scene::moon_primary`] identifies as moons.
///
/// **If you change it.** Raising it shortens a moon's trail in proportion, because the
/// segment budget is fixed. Below about `16` a loop turns into a visible polygon.
pub const TRAIL_MOON_SAMPLES_PER_ORBIT: usize = 64;

/// Near clip distance of the scene camera, in world units.
///
/// **Why this value.** Bodies are drawn at their true size, so inspecting one means getting very
/// close to it. Bevy's default `0.1` units is `100,000 km`, which is `55×` Io's radius and `16×`
/// Earth's: the camera lock could never get nearer than that, and Io stayed a small disc. `1e-4`
/// units is `100 km`, so the lock's closest orbit, `1.5` radii plus this, lets Io fill the view.
///
/// **The cost is nothing measurable.** Bevy 0.14 builds the projection with
/// `perspective_infinite_reverse_rh` into a 32-bit float depth buffer, so the stored depth is
/// `near / distance`. Floating point keeps its relative precision all the way down, so Neptune at
/// `4495` units stores `2.2e-8` with the same relative resolution as a body at `1` unit.
///
/// **Used by.** [`spawn_scene_entities`](crate::render::plugin::spawn_scene_entities), as the
/// camera's `PerspectiveProjection::near`, and through it the camera lock's closest orbit
/// ([`crate::input::camera::orbit_distance_bounds`]).
///
/// **If you change it.** Larger keeps the camera farther from small bodies and clips anything
/// nearer than it, which is what made the moons hard to inspect. Smaller than about `1e-6` starts to
/// clip the near side of geometry through precision loss in the view transform itself, which is
/// `f32`.
pub const CAMERA_NEAR_UNITS: f32 = 1.0e-4;

/// Far clip distance of the scene camera, in world units.
///
/// **Why this value.** Bevy's default perspective projection clips at `1000` units, which
/// was invisible while every scene was `30` units across and is fatal to the solar system:
/// Saturn starts `1433` units out and Neptune `4495`, so with the default they would
/// simply not be drawn — and neither would most of the `4000`-unit floor. `20,000` units
/// covers the whole system from anywhere a viewer can fly to, with room to spare.
///
/// The cost is normally depth precision, and here it is close to nothing: Bevy uses a
/// reversed-Z depth buffer, where resolution is set overwhelmingly by the **near** plane
/// ([`CAMERA_NEAR_UNITS`]) rather than by the far one.
///
/// **Used by.** [`spawn_scene_entities`](crate::render::plugin::spawn_scene_entities), as
/// the camera's `PerspectiveProjection::far`.
///
/// **If you change it.** Lowering it below an experiment's extent makes distant bodies
/// vanish rather than shrink, which reads as a physics bug and is not one — the props are
/// still there, and the overlay still lists them. Raising it far higher starts to cost
/// depth precision on near geometry even with reversed Z.
pub const CAMERA_FAR_UNITS: f32 = 20_000.0;

/// Alpha of a trail at the body end. Dimensionless.
///
/// **Why this value.** Fully opaque: the newest position should read as strongly as the body
/// itself, since the bright end is what says "this way is now".
///
/// **Used by.** `TrailConfig::default`, consumed by
/// [`TrailPath::segments`](crate::render::trail::TrailPath::segments).
///
/// **If you change it.** Lowering it makes the whole trail fainter; the *gradient* is what
/// carries the direction, so keep a clear gap between this and [`TRAIL_TAIL_ALPHA`].
pub const TRAIL_HEAD_ALPHA: f32 = 1.0;

/// Alpha of a trail at the far end. Dimensionless.
///
/// **Why this value.** Zero, so the trail ends by *disappearing* rather than by stopping. A
/// visible tail end reads as a fixed-length wire dragged behind the body — the eye latches onto
/// the endpoint and follows it instead of the body.
///
/// **Used by.** `TrailConfig::default`, consumed by `TrailPath::segments`.
///
/// **If you change it.** Values above zero give the trail a definite end. Nothing clamps the
/// relationship between the two ends, so setting this *above* the head alpha inverts the fade
/// and makes the oldest position the brightest — legal, and wrong.
pub const TRAIL_TAIL_ALPHA: f32 = 0.0;

/// Width of a trail's line, in **logical screen pixels**.
///
/// **Why this value.** `6` px is three times Bevy's `2`-px gizmo default. At `2` px a trail was
/// technically present and practically invisible: a one-or-two-pixel line against a lit 3D scene,
/// fading to transparent along its length, is below the threshold where the eye picks out a
/// direction. Three times that reads as a stroke.
///
/// This is the one trail dimension in **pixels rather than world units** — gizmo lines are
/// screen-space, so a trail is the same width whether the body is 10 units away or 200. That is
/// the right behaviour for an annotation (it stays legible at any distance) and the wrong one for
/// a physical object, which is part of why trails are drawn as gizmos and bodies as meshes.
///
/// **Used by.** [`crate::render::trail::TrailPlugin`], which writes it into the
/// [`TrailGizmos`](crate::render::trail::TrailGizmos) group's config at startup. The group is
/// separate from Bevy's default one so that trail thickness is a *trail* setting rather than a
/// global for every gizmo the app might later draw.
///
/// **If you change it.** Nothing clamps it. Much thicker and a trail obscures the body it belongs
/// to and swamps the grid lines it is measured against; much thinner and it disappears, which is
/// the failure this value exists to fix.
pub const TRAIL_LINE_WIDTH_PX: f32 = 6.0;

/// Minimum distance in front of the camera at which a placed object appears, in world units.
///
/// **Why this value.** `40` units puts an object far enough away to see whole — a `0.30`-unit
/// neutron star is still a visible dot, and a `12.7`-unit black hole fits in the view — while
/// staying close enough that it is unmistakably *the thing you just placed* rather than
/// something in the distance. It is also inside the two-body scene's own scale (`30` units
/// across), so placing an object into `experiment_1` puts it in play immediately rather than
/// on the far side of the world.
///
/// **Used by.** [`crate::render::placement::placement_position`], as a **floor**: the actual
/// distance is the larger of this and [`PLACEMENT_RADII_CLEARANCE`] times the object's own
/// drawn radius, because the catalog spans radii from `0.30` to `16.8` units and one fixed
/// distance cannot serve both.
///
/// **If you change it.** Smaller values put big objects around the camera's ears — hence the
/// radius-based floor, which is what actually protects against that. Much larger and a placed
/// object arrives outside the framing of the experiment you placed it in, which reads as
/// nothing having happened.
pub const PLACEMENT_DISTANCE_UNITS: f32 = 40.0;

/// How many of its own radii in front of the camera a placed object must be. Dimensionless.
///
/// **Why this value.** `3` radii means an object always subtends less than about `40°` of the
/// view, so you see a sphere rather than a wall. It is what makes placement work for the red
/// dwarf (`16.8` units drawn, so placed `50` out) using the same rule as the neutron star
/// (`0.30` units, so the `40`-unit floor applies instead).
///
/// **Used by.** `placement_position`.
///
/// **If you change it.** Below `1.0` the camera can end up inside the object it just placed,
/// which is disorienting and — since the camera does not collide — leaves you looking at the
/// inside of a sphere with no way to tell what happened.
pub const PLACEMENT_RADII_CLEARANCE: f32 = 3.0;

/// What the camera clears the frame to, in linear RGBA — the colour of empty space.
///
/// **Why this value.** Nearly black, with a trace of blue. Bevy's default clear colour is a mid
/// grey (`0.25` in each channel), which is fine for a lit demo scene and wrong for a simulation of
/// space: a `4.6`-px Sun against mid grey has almost no contrast, and since amendment A17 a `4.6`-px
/// Sun is what `experiment_3` opens with. Against this, a dim body is visible at the edge of what a
/// display can show.
///
/// Not pure black, deliberately. `0.004` is below the first step of an 8-bit sRGB ramp on most
/// displays, so it reads as black — but it is distinguishable from a window that has failed to
/// render at all, which is a diagnostic worth keeping for free. The faint blue is the same
/// convention every astronomy image uses.
///
/// **Used by.** [`crate::render::plugin::ScenePlugin`], as Bevy's `ClearColor` resource.
///
/// **If you change it.** It is the backdrop every other colour is judged against: the grid lines
/// ([`GRID_MINOR_COLOR`]), the trails, and the crosshair were all chosen to read against a dark
/// field. Lightening it materially costs the faintest bodies first, which are the ones hardest to
/// see already.
pub const BACKGROUND_COLOR: Color = Color {
    r: 0.004,
    g: 0.005,
    b: 0.012,
    a: 1.0,
};

/// Luminous power of a one-solar-luminosity star, in **lumens**.
///
/// **Why this value.** Derived, not chosen: the Sun delivers about `120,000 lux` at Earth's orbit,
/// and Bevy treats a point light's `intensity` as lumens with an illuminance of
/// `intensity / (4π · d²)` at distance `d`. Earth orbits at `149.6` units, so
///
/// ```text
/// intensity = 120_000 · 4π · 149.6² ≈ 3.375e10 lumens
/// ```
///
/// That makes the *illuminance* on every body physically right at its own orbital distance, which is
/// the point: Mercury is `6.6×` over-exposed and Neptune receives `1/900` of Earth's light, exactly
/// as they do. A star's light is scaled from here by its
/// [`luminosity_solar`](crate::core::scene::Prop::luminosity_solar), so a `0.0017 L☉` red dwarf is
/// `600×` dimmer than the Sun rather than merely a different colour.
///
/// **Used by.** `spawn_mirror` in [`crate::render::plugin`], for every prop with a positive luminosity.
///
/// **If you change it.** The whole scene's exposure moves with it, and the falloff does not — the
/// outer planets are dark because they are `77×` further out than Mercury, and no intensity fixes
/// that without blowing out the inner ones. If the scene is uniformly too dark or too bright, this
/// is the number; if the *range* between near and far bodies is the problem, that range is real.
pub const STAR_LUMENS_PER_SOLAR_LUMINOSITY: f32 = 3.375e10;

/// How far a star's light reaches, in world units.
///
/// **Why this value.** Bevy's `PointLight::range` is a hard cut-off used to cluster lights, not a
/// physical falloff — a fragment beyond it is unlit regardless of how bright the source is. So it
/// has to exceed the scene, and `20,000` units matches [`CAMERA_FAR_UNITS`]: anything the camera can
/// see, the Sun can light. Neptune is at `4495`.
///
/// **Used by.** `spawn_mirror`.
///
/// **If you change it.** Too small and the outer solar system goes abruptly black at a distance with
/// no physical meaning, which is the one lighting artefact that would look like a bug rather than
/// like space. There is no cost to it being generous: the falloff is inverse-square either way, so a
/// large range does not brighten anything.
pub const STAR_LIGHT_RANGE_UNITS: f32 = 20_000.0;

/// How bright a star's own surface is drawn, as a multiple of its colour. Dimensionless.
///
/// **Why this value.** A star is lit from *inside* — the point light sits at its centre — so with
/// only reflected light its surface would render **black**, which is the one thing a star must not
/// be. Its material is therefore emissive, at `4×` its base colour: above `1.0` so it saturates to a
/// bright disc under tonemapping, and not so far above that the whole frame blooms out.
///
/// **Used by.** `spawn_mirror`, as the emissive term on a luminous prop's material.
///
/// **If you change it.** Purely cosmetic, and independent of
/// [`STAR_LUMENS_PER_SOLAR_LUMINOSITY`]: this is how bright the star *looks*, that is how much it
/// *lights other things*. They are separate numbers because Bevy models them separately, and
/// keeping them in step is a judgement rather than a calculation.
pub const STAR_EMISSIVE_SCALE: f32 = 4.0;

/// How many occluding bodies the eclipse shader can test against.
///
/// **Why this value.** `16` covers every scene in the catalog: `experiment_3` is the largest at `14`
/// props, of which `13` can occlude — the floor cannot, being flat. It is also the natural size for a
/// WGSL uniform array, which cannot be dynamically sized, and `16 × 16` bytes is a `256`-byte
/// uniform, which is nothing.
///
/// **Used by.** [`crate::render::eclipse::EclipseUniform`] and the matching `array` in
/// `assets/shaders/eclipse.wgsl`. **Both must agree**: the shader hard-codes `16` because WGSL has no
/// way to import a Rust constant, and `eclipse_tests` pins the pair.
///
/// **If you change it.** The shader's `MAX_OCCLUDERS` and its array length must change with it.
/// Occluders past this count are **silently ignored** — the update system takes the nearest ones to
/// the star, so what is dropped is the least likely to be casting a shadow on anything, but it is
/// still a limit rather than a graceful degradation.
pub const MAX_ECLIPSE_OCCLUDERS: usize = 16;

/// Where the eclipse fragment shader lives, relative to `assets/`.
///
/// **Why this value.** Bevy's `AssetPlugin` resolves a `ShaderRef::Path` against the `assets/`
/// directory, and this is the crate's only shader — so it is the only reason `assets/` exists at all.
///
/// **Used by.** `MaterialExtension::fragment_shader` for
/// [`EclipseExtension`](crate::render::eclipse::EclipseExtension).
///
/// **If you change it.** The file must move with it, and note the failure mode: a missing shader is a
/// **runtime** error, not a compile error. The app logs an asset-not-found and draws the bodies
/// untextured or not at all, which is why `cargo run` is part of verifying a change here.
pub const ECLIPSE_SHADER_PATH: &str = "shaders/eclipse.wgsl";

// ---------------------------------------------------------------------------------------------
// Rocket transfer — `crate::render::mission`
// ---------------------------------------------------------------------------------------------

/// Opacity of the part of a rocket's path that is still only predicted.
///
/// **Why this value.** `0.35`: the flown path is drawn opaque, so the plan reads as a fainter line
/// running on ahead of it, and the eye can tell where the rocket has been from where it is going.
///
/// **Used by.** [`crate::render::mission::draw_mission_system`].
///
/// **If you change it.** Near `1.0` the two halves are indistinguishable; near `0.0` the plan
/// disappears against the orbit trails.
pub const MISSION_PLANNED_PATH_ALPHA: f32 = 0.35;

/// Angular radius of the marker drawn where a rocket has landed, in radians.
///
/// **Why this value.** About `0.4°`. A lander on Mars is far below a pixel at any useful distance,
/// so the marker is a ring sized by its distance from the camera, which keeps it the same size on
/// screen at every zoom.
///
/// **Used by.** [`crate::render::mission::draw_mission_system`].
///
/// **If you change it.** Larger hides Mars behind the marker when zoomed out; smaller loses it.
pub const MISSION_MARKER_ANGULAR_RADIUS: f32 = 0.007;

/// The shortest a rocket's flight may take to watch, in real seconds.
///
/// **Why this value.** Twenty seconds. At the solar system's clock of `1e6`, Earth to the Moon is
/// `0.03 s`, under two frames, so it would be over before it was seen. Its integration would also
/// be too coarse to land, since the live scene steps a whole frame between correction checks. On
/// launch the clock is slowed until the flight lasts at least this long, and restored on arrival.
/// Earth to Mars takes `42 s` at the shipped clock and is left alone.
///
/// **Used by.** [`crate::render::mission::mission_system`].
///
/// **If you change it.** Shorter and short hops land less reliably. Longer and every flight under
/// it is slowed further.
pub const MISSION_MIN_WATCH_S: f64 = 20.0;

/// The radius of the sphere the light rays set off from, in horizon radii.
///
/// **Why this value.** Far enough out that a ray starts nearly straight (the bend accumulated beyond
/// `12 r_s` is a few degrees at most), and close enough that the whole sphere fits the experiment's
/// opening view: `152` units for Sgr A*, framed by
/// `experiments::constants::BLACK_HOLE_FRAMED_RADIUS_UNITS`, kept in step by hand.
///
/// **Used by.** [`crate::render::light_streams::LightStreams::emit`].
///
/// **If you change it.** Change that framing too, and keep it below [`LIGHT_STREAM_ESCAPE_RS`].
pub const LIGHT_STREAM_SOURCE_DISTANCE_RS: f32 = 12.0;

/// The widest a ray is aimed off the hole's centre, in horizon radii.
///
/// **Why this value.** The critical impact parameter is `3√3/2 = 2.6 r_s`: inside it light is
/// captured, just outside it light whirls round the photon sphere. Offsets are spread evenly over a
/// disc of radius `4 r_s`, so `(2.6/4)² = 42%` of the rays fall in and the rest bend round,
/// many of them hard.
///
/// **Used by.** [`crate::render::light_streams::LightStreams::emit`].
///
/// **If you change it.** Larger means more rays that only bend a little.
pub const LIGHT_STREAM_MAX_IMPACT_RS: f32 = 4.0;

/// How many rays set off each time the emitter fires.
///
/// **Why this value.** With [`LIGHT_STREAM_EMISSION_INTERVAL_S`] it sets the density: `48` every
/// `10 s` keeps about three thousand rays in flight, enough to fill every direction round the hole
/// while staying cheap to step and draw.
///
/// **Used by.** [`crate::render::light_streams::LightStreams::emit`].
///
/// **If you change it.** Cost is linear in it.
pub const LIGHT_STREAM_RAYS: u32 = 48;

/// Simulated seconds between bursts of rays.
///
/// **Why this value.** At the experiment's `100×` clock a burst sets off every tenth of a second,
/// a continuous stream. See [`LIGHT_STREAM_RAYS`].
///
/// **Used by.** [`crate::render::light_streams::LightStreams::advance`].
///
/// **If you change it.** Smaller means more rays in flight.
pub const LIGHT_STREAM_EMISSION_INTERVAL_S: f64 = 10.0;

/// The seed for where the rays set off from and how they are aimed.
///
/// **Why this value.** Arbitrary. Fixed so a run is the same every time and the tests can rely on
/// it.
///
/// **Used by.** [`crate::render::light_streams::LightStreams`].
///
/// **If you change it.** A different but equivalent pattern of rays.
pub const LIGHT_STREAM_SEED: u64 = 0x5167_A57A;

/// The radius, in horizon radii, at which a falling ray is counted as captured.
///
/// **Why this value.** The same as the experiment's capture radius: where light from the hole's
/// neighbourhood is redshifted tenfold. In the distant observer's time a ray never reaches the
/// horizon; it slows and creeps towards it, and this is where it is let go.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Must be above `1`. Closer to `1` and captured rays linger longer.
pub const LIGHT_STREAM_CAPTURE_RS: f64 = 1.0101;

/// The radius, in horizon radii, beyond which an outbound ray is removed.
///
/// **Why this value.** A little past the source sphere, so a ray that bends round and leaves is
/// seen leaving.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Keep it above [`LIGHT_STREAM_SOURCE_DISTANCE_RS`].
pub const LIGHT_STREAM_ESCAPE_RS: f64 = 13.5;

/// The longest a ray is kept, in simulated seconds, whatever it is doing.
///
/// **Why this value.** A crossing takes about `1000 s`; a ray whirling near the photon sphere can
/// take several times that. `5000` lets it finish and bounds the rare one that would circle for a
/// very long time.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Visual only.
pub const LIGHT_STREAM_MAX_AGE_S: f64 = 5000.0;

/// Each ray's step, as a fraction of its light-travel time to the hole's centre, `r/c`.
///
/// **Why this value.** Steps shrink near the hole, where the path bends hardest. `0.02` is `1.3 s`
/// at the photon sphere, which keeps the classical fourth-order Runge–Kutta steps accurate through
/// a whirl round it (the tests check rays at `2.5`, `2.62`, and `6 r_s` against what they must do).
/// Scaling by `r` rather than by the distance to the horizon matters: the latter made rays creeping
/// towards the horizon take steps of a hundredth of a second, and cost ten times as much.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Larger is cheaper and less accurate near the hole.
pub const LIGHT_STREAM_STEP_FRACTION: f64 = 0.02;

/// The longest step a ray takes, in simulated seconds.
///
/// **Why this value.** Far from the hole the fractional rule allows steps of tens of seconds; `2 s`
/// keeps the streak's samples and the bending smooth there too.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Cost is inverse in it.
pub const LIGHT_STREAM_MAX_STEP_S: f64 = 2.0;

/// The most steps a ray takes in one frame.
///
/// **Why this value.** A bound on cost at a very fast clock: past it the steps grow rather than
/// multiply. At the experiment's clock a ray takes far fewer.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Must be at least `1`.
pub const LIGHT_STREAM_MAX_STEPS_PER_FRAME: u32 = 2000;

/// Simulated seconds between the points of a ray's streak.
///
/// **Why this value.** With [`LIGHT_STREAM_TRAIL_SAMPLES`] it sets the streak's length: `16` points
/// `4 s` apart is `64 s` of travel, about `1.5 r_s` far out. Sampled by simulated time, so the
/// streak is the same length at any frame rate.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Visual only.
pub const LIGHT_STREAM_SAMPLE_INTERVAL_S: f64 = 4.0;

/// How many earlier points each ray's streak keeps.
///
/// **Why this value.** See [`LIGHT_STREAM_SAMPLE_INTERVAL_S`]. Shorter than a lone beam's would be,
/// because with three thousand rays in flight long streaks would fill the view.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Drawing cost is linear in it.
pub const LIGHT_STREAM_TRAIL_SAMPLES: u32 = 16;

/// The colour of the light streams at their heads, linear RGB with alpha.
///
/// **Why this value.** A warm white, bright against the black sky. Each streak fades from this to
/// transparent at its tail, and the slight transparency keeps a dense crowd of streaks from
/// saturating into a solid blob.
///
/// **Used by.** [`crate::render::light_streams`].
///
/// **If you change it.** Visual only.
pub const LIGHT_STREAM_COLOR: Color = Color {
    r: 1.0,
    g: 0.9,
    b: 0.65,
    a: 0.85,
};

/// The width of the light streams, in screen pixels.
///
/// **Why this value.** Thin enough that thousands of streaks stay separate; wide enough to read.
///
/// **Used by.** [`crate::render::light_streams::LightStreamsPlugin`], on its own gizmo group.
///
/// **If you change it.** Visual only.
pub const LIGHT_STREAM_LINE_WIDTH_PX: f32 = 2.0;

/// How many clumps of gas make up the accretion disc.
///
/// **Why this value.** Dense enough that the disc reads as a continuous sheet of gas rather than
/// separate streaks from the opening view, and that its lensed images, where fewer clumps land per
/// unit of screen, read as continuous too.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Cost is linear in it.
pub const DISC_PARTICLES: u32 = 10_000;

/// The accretion disc's outer edge, in gravitational radii `r_g = GM/c²`.
///
/// **Why this value.** `20 r_g` (`127` units for Sgr A*) takes in the hot inner disc, where nearly
/// all the light is made (the flux falls as `r⁻³`), and stays inside the sphere the light streams
/// start from.
///
/// **Used by.** [`crate::render::accretion_disc::DiscProfile`].
///
/// **If you change it.** Larger reaches cooler, dimmer gas; it is kept at least `1.5` times the
/// ISCO.
pub const DISC_OUTER_RADIUS_RG: f32 = 20.0;

/// The disc's accretion rate, as a fraction of the Eddington rate.
///
/// **Why this value.** `0.1`, typical of a bright quasar and inside the range where a thin disc is
/// the right model (above about `0.3` the disc puffs up; below about `0.01` it becomes a hot, faint
/// flow). For Sgr A* at spin `0.9` the inner disc then reaches a few hundred thousand kelvin and
/// glows in the ultraviolet. The real Sgr A* is fed about a billion times more slowly.
///
/// **Used by.** [`crate::render::accretion_disc::DiscProfile`].
///
/// **If you change it.** The temperature goes as its fourth root.
pub const DISC_EDDINGTON_FRACTION: f64 = 0.1;

/// How many orbits it takes the gas to drift inward by a factor `e` in radius.
///
/// **Why this value.** Sped up, on purpose: a real thin disc's gas takes thousands of orbits to
/// cross the disc, which would make the drift invisible. At `3` a clump released at the outer edge
/// reaches the ISCO in about seven of its own orbits, slowly enough that the disc still reads as
/// orbiting and fast enough that the plunge can be seen to happen.
///
/// **Used by.** [`crate::render::accretion_disc`]. The one non-physical rate there.
///
/// **If you change it.** Smaller drifts faster.
pub const DISC_INFLOW_ORBITS_PER_EFOLD: f64 = 3.0;

/// The fraction of the circular speed a clump keeps as it crosses the ISCO and starts to fall.
///
/// **Why this value.** At the ISCO a circular orbit is marginally stable; a `0.5%` loss is the nudge
/// that tips it in, and keeps the start of the plunge a slow whirl, as it is in a real disc.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Smaller drops the gas in more steeply.
pub const DISC_PLUNGE_SPEED_FACTOR: f64 = 0.995;

/// A plunging clump's step, as a fraction of its light-travel time to the hole's centre.
///
/// **Why this value.** The same rule and value as the light streams use.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Larger is cheaper and less accurate.
pub const DISC_PLUNGE_STEP_FRACTION: f64 = 0.02;

/// Simulated seconds between the points of a plunging clump's streak.
///
/// **Why this value.** A plunge from the ISCO takes a few hundred seconds; `2 s` samples it finely.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Visual only.
pub const DISC_PLUNGE_SAMPLE_INTERVAL_S: f64 = 2.0;

/// How many earlier points a plunging clump's streak keeps.
///
/// **Why this value.** `12` points `2 s` apart: a streak about as long as an orbiting clump's arc.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Visual only.
pub const DISC_PLUNGE_TRAIL_SAMPLES: u32 = 12;

/// How many radii the disc's temperature and flux are tabulated at.
///
/// **Why this value.** The Page–Thorne flux needs an integral per radius, so it is computed once per
/// hole and interpolated. `256` samples, spaced closer near the ISCO where the profile is steep,
/// keep the interpolation far below anything visible.
///
/// **Used by.** [`crate::render::accretion_disc::DiscProfile`].
///
/// **If you change it.** Cost is paid once, when the disc is built.
pub const DISC_PROFILE_SAMPLES: u32 = 256;

/// The seed for where the disc's clumps start.
///
/// **Why this value.** Arbitrary; fixed so every run is the same.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** A different but equivalent disc.
pub const DISC_SEED: u64 = 0xD15C_0A11;

/// The length of an orbiting clump's streak, as an angle along its orbit, in radians.
///
/// **Why this value.** About `17°`: long enough to show which way the disc turns, short enough that
/// the disc reads as clumps rather than rings.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Visual only.
pub const DISC_STREAK_ARC_RAD: f64 = 0.3;

/// How many points an orbiting clump's arc is drawn through.
///
/// **Why this value.** Enough for a smooth arc of [`DISC_STREAK_ARC_RAD`].
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Drawing cost is linear in it. At least `2`.
pub const DISC_STREAK_POINTS: u32 = 5;

/// The colour of the ergosphere's outline, linear RGB with alpha.
///
/// **Why this value.** A faint violet: present but behind everything else.
///
/// **Used by.** [`crate::render::accretion_disc`].
///
/// **If you change it.** Visual only.
pub const ERGOSPHERE_COLOR: Color = Color {
    r: 0.6,
    g: 0.45,
    b: 1.0,
    a: 0.35,
};

/// How many meridians of the ergosphere's outline are drawn, besides its equator.
///
/// **Why this value.** `6`, one every `60°`: enough to show its shape, flattened at the poles where
/// it meets the horizon.
///
/// **Used by.** [`crate::render::accretion_disc::ergosphere_outline`].
///
/// **If you change it.** Visual only.
pub const ERGOSPHERE_MERIDIANS: u32 = 6;

/// How many segments the ergosphere's equator is drawn with (its meridians use half as many).
///
/// **Why this value.** Smooth at the opening view's distance.
///
/// **Used by.** [`crate::render::accretion_disc::ergosphere_outline`].
///
/// **If you change it.** Visual only.
pub const ERGOSPHERE_SEGMENTS: u32 = 96;

/// Each step of the GPU ray-marcher (and its Rust twin), as a fraction of the light's distance from
/// the hole.
///
/// **Why this value.** Steps then shrink near the hole, where light bends hardest, and grow far out,
/// where it barely bends, so a ray from the camera to the disc and back out takes a few hundred
/// steps. At `3%` the twin's plane crossings agree with the `f64` tracer to well under a percent,
/// and the null condition holds to `10⁻⁴` even for light that swings round the photon orbit
/// (`kerr_ray_tests`). Every test also passes at `6%`, so there is room to trade accuracy for speed
/// in the performance settings.
///
/// **Used by.** [`crate::render::kerr_ray`].
///
/// **If you change it.** The cost per pixel is inversely proportional to it; the accuracy tests in
/// `kerr_ray_tests` say how far it can grow.
pub const KERR_RAY_STEP_FRACTION: f32 = 0.03;

/// The most steps one ray of the GPU ray-marcher (and its Rust twin) takes before it is abandoned.
///
/// **Why this value.** A ray that reaches the disc or the sky takes a few hundred steps
/// ([`KERR_RAY_STEP_FRACTION`]); only light skimming the photon orbit, circling many times before it
/// escapes or falls in, needs more, and those pixels are a hairline at the shadow's edge.
///
/// **Used by.** [`crate::render::kerr_ray`].
///
/// **If you change it.** Lower, and the hairline at the shadow's edge goes dark sooner; higher costs
/// only those pixels.
pub const KERR_RAY_MAX_STEPS: u32 = 2000;

/// How many crossings of the disc's plane a ray of the GPU ray-marcher (and its Rust twin) records:
/// image orders `0` to `2`.
///
/// **Why this value.** Order `0` is the disc arched over the hole, `1` its underside below the
/// hole, `2` a hairline ring at the shadow's edge; each is about `e^π ≈ 23` times thinner than the
/// one before, so a third is the last a pixel can show.
///
/// **Used by.** [`crate::render::kerr_ray`].
///
/// **If you change it.** Each further order adds a ring too thin to see; fewer drops the rings.
pub const KERR_RAY_MAX_CROSSINGS: usize = 3;

/// Where the black hole's ray-marching fragment shader lives, relative to `assets/`.
///
/// **Why this value.** Beside `eclipse.wgsl`, the only other shader.
///
/// **Used by.** [`BlackHoleMaterial`](crate::render::black_hole_view::BlackHoleMaterial).
///
/// **If you change it.** The file must move with it. A missing shader is a runtime error: the app
/// logs an asset-not-found and the hole is not drawn.
pub const BLACK_HOLE_SHADER_PATH: &str = "shaders/black_hole.wgsl";

/// The radius, in gravitational radii, of the sphere the black hole is ray-marched on.
///
/// **Why this value.** Light is followed only inside it; outside it, it is taken to go straight.
/// At `100 r_g` that ignores a bend of about `4/100` rad over a whole path, and it contains the disc
/// (`20 r_g`) and the opening camera (`53 r_g`) with room to spare. Steps grow with distance from
/// the hole, so a larger sphere costs only a few more steps per pixel.
///
/// **Used by.** [`crate::render::black_hole_view`].
///
/// **If you change it.** Smaller starts rays where the bending is not yet negligible; it must stay
/// well outside the disc's edge, [`DISC_OUTER_RADIUS_RG`].
pub const BLACK_HOLE_VIEW_SPHERE_RG: f32 = 100.0;

/// How many samples of the disc's temperature and flux the GPU ray-marcher (and its Rust twin)
/// carries, from the ISCO to the outer edge.
///
/// **Why this value.** Spaced uniformly in `√(r − r_ISCO)`, as [`DISC_PROFILE_SAMPLES`] are, `64`
/// reproduce the profile to well under a percent (`kerr_ray_tests`), and at one `vec4` each they
/// are `1 KiB` of uniform, far inside the `16 KiB` every GPU allows.
///
/// **Used by.** [`crate::render::kerr_ray::DiscShading`], and `PROFILE_SAMPLES` in
/// `assets/shaders/black_hole.wgsl`, which must match (a test checks).
///
/// **If you change it.** Change the shader's `PROFILE_SAMPLES` and its array length with it.
pub const KERR_RAY_PROFILE_SAMPLES: usize = 64;

/// The gas texture's width: texels round the hole.
///
/// **Why this value.** `512` texels round the hole are `0.7°` each, `0.12 r_g` at `10 r_g`, finer
/// than a clump's streak ([`DISC_STREAK_ARC_RAD`] is `0.3` rad, `25` texels), so each streak stays a
/// streak. With [`DISC_GAS_TEXTURE_HEIGHT`] it is `64 KiB`, uploaded every frame.
///
/// **Used by.** [`crate::render::black_hole_view::splat_gas`].
///
/// **If you change it.** Coarser blurs the clumps into a smooth sheet; finer costs upload and
/// splatting time and shows each streak's edges.
pub const DISC_GAS_TEXTURE_WIDTH: usize = 512;

/// The gas texture's height: texels from the horizon to the disc's edge, uniformly in `√(r − r₊)`.
///
/// **Why this value.** `128` rows put about `40` inside `5 r_g`, where the gas plunges and the disc
/// is brightest, and rows `0.2 r_g` apart at `10 r_g`.
///
/// **Used by.** [`crate::render::black_hole_view::splat_gas`].
///
/// **If you change it.** As [`DISC_GAS_TEXTURE_WIDTH`].
pub const DISC_GAS_TEXTURE_HEIGHT: usize = 128;

/// How wide a strip of the plane each clump's streak covers in the ray-marched disc, in `r_g`.
///
/// **Why this value.** It sets how much of the disc the gas covers. The clumps are spread so that
/// at `10 r_g` there are about `9` per `r_g²`, each with a streak `3 r_g` long; at `0.05 r_g` wide
/// they cover a texel `1.3` times over on average, so about `70%` of the disc is gas
/// (`1 − e^{−1.3}`) and the rest is gaps through which the images behind show.
///
/// **Used by.** [`crate::render::black_hole_view::splat_gas`].
///
/// **If you change it.** Wider makes the disc a solid sheet; narrower, a sparse web of streaks.
pub const DISC_CLUMP_WIDTH_RG: f32 = 0.05;

/// How much bigger than the camera's distance from the hole the ray-marched sphere is made.
///
/// **Why this value.** The sphere must hold the camera, so that every pixel looks through it and the
/// whole sky, stars included, is lensed; `1.25` keeps its far side well clear of the camera's near
/// side as the camera moves within a frame.
///
/// **Used by.** [`crate::render::black_hole_view::sphere_radius_rg`].
///
/// **If you change it.** Below `1` the camera leaves the sphere and the sky outside it is the
/// scene's background, unlensed and starless.
pub const BLACK_HOLE_VIEW_CAMERA_MARGIN: f32 = 1.25;

/// The exposure the ray-marched black hole is shown at: what the light is multiplied by before
/// tone mapping, `1 − e^{−exposure · I}`, with `I = 1` the disc's peak flux seen without any shift.
///
/// **Why this value.** The disc's light spans more than a thousandfold: `g⁴` beaming makes the inner
/// edge coming towards the camera several times its peak flux, and the flux falls as `r⁻³` to the
/// outer edge. At `6`, the outer disc (`1%` of the peak) shows at about `6%` grey, the peak is near
/// white, and the beamed side saturates smoothly rather than clipping.
///
/// **Used by.** [`crate::render::kerr_ray::DiscShading`], and through the uniform the shader.
///
/// **If you change it.** Lower shows the inner disc's structure and loses the outer disc; higher
/// washes the inner disc out to white.
pub const BLACK_HOLE_EXPOSURE: f32 = 6.0;

/// The colour the black hole's event horizon is outlined in while the outline is on, linear RGB.
///
/// **Why this value.** A warm amber, bright enough to stand out against the black shadow it sits in, but distinct from the disc's white-hot inner edge and the stars' white points, so the line
/// reads as a marker, not as light from the scene.
///
/// **Used by.** [`crate::render::black_hole_view::BlackHoleOutline`]; the shader's
/// `OUTLINE_COLOUR` mirrors it.
///
/// **If you change it.** Change `OUTLINE_COLOUR` in `assets/shaders/black_hole.wgsl` to match; a
/// test checks the two agree.
pub const BLACK_HOLE_OUTLINE_COLOUR: [f32; 3] = [1.0, 0.45, 0.1];

/// How wide the outline of the black hole's event horizon is drawn, in pixels, measured across it.
///
/// **Why this value.** The edge is where [`crate::render::kerr_ray::horizon_measure`] is zero; the
/// shader draws the line where it is within this many pixels' worth of its own change of zero, so
/// the line is the same width all the way round however the horizon is squashed. `2` reads clearly
/// at a glance and stays a thin line on a horizon only a few dozen pixels across.
///
/// **Used by.** The shader (`OUTLINE_WIDTH_PX` mirrors it), through
/// [`crate::render::black_hole_view::BlackHoleOutline`].
///
/// **If you change it.** Change `OUTLINE_WIDTH_PX` in `assets/shaders/black_hole.wgsl` too; a test
/// checks they agree.
pub const BLACK_HOLE_OUTLINE_WIDTH_PX: f32 = 2.0;
