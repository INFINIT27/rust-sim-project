//! `ScenePlugin` module: wires the pure-Rust `Scene` model into a Bevy
//! `App` as a resource and registers the per-frame systems that
//! (a) advance the two clocks, (b) step the scene by the simulated interval one
//! of them produced, and (c) mirror the `Scene`'s bodies onto ECS entities via
//! the sibling `bridge` module.
//!
//! # This module no longer decides what a timestep is
//!
//! It used to: a raw `f32` frame delta was clamped and scaled here, inside
//! `step_scene_system`, against a `SceneConfig` resource this module owned. Both
//! now live in [`crate::time`] — [`SimulationClock`] holds the scale, the pause
//! state, and the total elapsed simulated time, and hands out a validated
//! [`TimeStep`](crate::time::simulation::TimeStep). This module's job is reduced to
//! *adopting* those clocks as ECS
//! resources and calling them in the right order, which is the only part that
//! genuinely needs Bevy.
//!
//! `SceneConfig` is gone with it; see amendment A3 in
//! the original requirements spec. [`clamp_dt`] remains as
//! Requirement 13's named helper and delegates to [`clamp_real_step`].
//!
//! Unlike the `core::scene` module, this module is on the Bevy side of the
//! render boundary (Requirement 15.3) and may import freely from `bevy`.
//!
//! Public surface:
//!
//! - [`MirroredBody`] — an ECS `Component` that links a spawned entity
//!   back to the `Scene`'s `BodyId`, so [`sync_scene_to_ecs_system`]
//!   can look up the current `Body` for each mirrored entity.
//! - [`ScenePlugin`] — the `bevy::app::Plugin` implementation that inserts
//!   `Scene::two_body_gravity()`, a [`SimulationClock`], and an
//!   [`ObserverClock`]; registers [`spawn_scene_entities`] on `Startup`; and
//!   registers [`advance_clocks_system`], [`step_scene_system`], and
//!   [`sync_scene_to_ecs_system`] on `Update` in that order (Requirement 2.1).
//! - [`advance_clocks_system`] — the one place a Bevy frame delta enters the
//!   crate, feeding both clocks.
//! - [`spawn_scene_entities`] — the startup system that seeds the
//!   world with a camera (carrying a `FreeFlyController`), a
//!   directional light, and one `PbrBundle` per body in `Scene`
//!   (Requirement 3.5).

use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::{AssetApp, AssetServer, Assets, Handle};
use bevy::color::{Color as BevyColor, LinearRgba};
use bevy::core_pipeline::core_3d::Camera3dBundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, Resource};
use bevy::hierarchy::{BuildChildren, DespawnRecursiveExt};
use bevy::math::primitives::{Cuboid, Plane3d, Sphere};
use bevy::math::{UVec4, Vec3, Vec4};
use bevy::pbr::{
    DirectionalLightBundle, MaterialMeshBundle, MaterialPlugin, PointLight, PointLightBundle,
    StandardMaterial,
};
use bevy::render::camera::ClearColor;
use bevy::render::camera::PerspectiveProjection;
use bevy::render::mesh::{Mesh, Meshable};
use bevy::time::Time;
use bevy::transform::components::Transform;

use crate::core::scene::{Body, BodyId, Color, Prop, Scene};
use crate::experiments::catalog::{BodyNames, Experiment, ExperimentSpec};
use crate::experiments::selection::Selection;
use crate::input::camera::{CameraLock, CameraTarget, FreeFlyController};
use crate::performance::PerformanceSettings;
use crate::render::bridge::{sync_scene_to_ecs_system, transform_from};
use crate::render::constants::{
    BACKGROUND_COLOR, CAMERA_FAR_UNITS, CAMERA_NEAR_UNITS, LIGHT_POSITION_UNITS,
    LINE_VISUAL_THICKNESS_UNITS, MAX_ECLIPSE_OCCLUDERS, STAR_EMISSIVE_SCALE,
    STAR_LIGHT_RANGE_UNITS, STAR_LUMENS_PER_SOLAR_LUMINOSITY,
};
use crate::render::eclipse::{BodyMaterial, EclipseExtension, EclipseUniform};
use crate::render::mission::Mission;
use crate::render::trail::{TrailConfig, Trails};
use crate::time::observer::ObserverClock;
use crate::time::simulation::{clamp_real_step, SimulationClock};

/// Register `Scene` as a Bevy `Resource` from the Bevy side of the
/// render boundary.
///
/// `Resource` is a marker trait (`trait Resource: Send + Sync + 'static`),
/// so implementing it externally keeps `scene::Scene` free of any
/// `bevy::ecs` reference per Requirement 4.2 / task 3.1's "Ensure Scene
/// references no `bevy::ecs`, `bevy_egui`, or `wgpu` types" constraint.
/// The `step_scene_system` and `sync_scene_to_ecs_system` signatures
/// mandated by tasks 7.4–7.5 take `Res<Scene>` / `ResMut<Scene>`, both
/// of which require `Scene: Resource`, so the impl has to live
/// somewhere — right here, on the Bevy side, is the natural home.
impl Resource for Scene {}

/// ECS component that marks an entity as a render-time mirror of a body
/// in the pure-Rust `Scene`.
///
/// Each `PbrBundle` spawned by `spawn_scene_entities` (task 7.6) is
/// tagged with a `MirroredBody(id)` whose inner `BodyId` is the same id
/// that `Scene::add` returned when the body was inserted. The per-frame
/// `sync_scene_to_ecs_system` (task 7.5) iterates entities with this
/// component, resolves the current `Body` via `Scene::get`, and rewrites
/// the entity's `Transform` through `bridge::transform_from`.
///
/// Keeping this link as a thin `BodyId` wrapper — rather than, say, a
/// borrow of the `Body` itself — is what lets `sync_scene_to_ecs_system`
/// take `Res<Scene>` (immutable) while still driving per-frame transform
/// updates (Requirement 9.3).
#[derive(Component, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MirroredBody(pub BodyId);

/// The two clocks are ECS resources, so Bevy can hold them.
///
/// `Resource` is a Bevy trait and [`crate::time`] imports no Bevy at all, so these
/// impls live here — the same reasoning as `impl Resource for Scene` above, and the
/// same place.
impl Resource for SimulationClock {}

impl Resource for ObserverClock {}

/// The running experiment's settings, so the startup camera and the overlay can read
/// *which* experiment this is rather than assume. See
/// [`crate::experiments`].
impl Resource for ExperimentSpec {}

/// The running experiment's body names, for the rocket's status and anything else that labels a
/// body. Placing an object with `B` adds its name.
impl Resource for BodyNames {}

/// Bevy plugin that wires the pure-Rust `Scene` into an `App`.
///
/// Task 7.1 introduced the struct; task 7.7 fills in `build` with the
/// full resource + system registration:
///
/// 1. Build the selected experiment and insert its `Scene`, its [`ExperimentSpec`], a
///    [`SimulationClock`] at *that experiment's* scale, and an [`ObserverClock`], all
///    as resources (Requirement 3.1, amendments A3 and A4).
/// 2. Register [`spawn_scene_entities`] on the `Startup` schedule
///    (Requirement 3.5).
/// 3. Register the `Update` chain [`advance_clocks_system`] →
///    [`step_scene_system`] → [`sync_scene_to_ecs_system`], so each runs exactly
///    once per tick in the sequence mandated by Requirement 2.1.
pub struct ScenePlugin {
    /// Which experiment to run, and the seed for its randomised initial conditions.
    ///
    /// `Default` is `experiment_1` with no fixed seed, so `ScenePlugin::default()`
    /// behaves exactly as the old unit-struct `ScenePlugin` did.
    pub selection: Selection,
}

impl Default for ScenePlugin {
    #[inline]
    fn default() -> Self {
        Self {
            selection: Selection::default(),
        }
    }
}

impl ScenePlugin {
    /// A plugin that runs `selection`.
    ///
    /// The seed is resolved *here*, once, at build time — not per frame and not per
    /// system — so the `ExperimentSpec` records the exact number the scene was built
    /// from and `--seed <that number>` reproduces it.
    #[inline]
    pub fn new(selection: Selection) -> Self {
        Self { selection }
    }
}

impl Plugin for ScenePlugin {
    /// Wire the pure-Rust `Scene` into the Bevy `App`.
    ///
    /// Resources are inserted with `insert_resource` (not
    /// `init_resource`) so the `Scene::demo()` content is identical
    /// across rebuilds — relying on a `Default` impl for `Scene` would
    /// either require exposing the empty-`Scene` constructor publicly
    /// (breaking the `Scene::demo`-only contract) or would ship an
    /// empty world instead of the demo content. The `SimulationClock` is likewise
    /// inserted with an explicit scale rather than defaulted: `Default` is real
    /// time, which for these scenes is a still image. The scale comes from the
    /// experiment's spec, because the value that makes one experiment watchable makes
    /// another a blur.
    ///
    /// The three `Update` systems are added together with explicit `.after` edges
    /// on the *systems* rather than on a system set, keeping the ordering local and
    /// self-evident. The order is load-bearing: the clocks must advance before the
    /// scene steps by what they produced, and the scene must step before the ECS is
    /// synced to it.
    fn build(&self, app: &mut App) {
        let Experiment { scene, spec, names } =
            self.selection.id.build(self.selection.resolve_seed());

        debug_assert!(
            spec.is_valid(),
            "ScenePlugin::build: experiment spec fails Validation_Rules: {:?}",
            spec
        );

        app.insert_resource(scene);
        app.insert_resource(SimulationClock::new(spec.time_scale));
        app.insert_resource(ObserverClock::new());
        app.insert_resource(spec);
        app.insert_resource(names);
        // The colour of empty space. Bevy's default is a mid grey, which a 4.6-px Sun has almost
        // no contrast against — and since amendment A17 that is what `experiment_3` opens with.
        app.insert_resource(ClearColor(to_bevy_color(BACKGROUND_COLOR)));

        // `MaterialPlugin` needs the render app, which arrives with `DefaultPlugins` and not with
        // `MinimalPlugins` — the same guard `TrailPlugin` uses for its gizmo group. Without it a
        // headless test would panic on a missing render sub-app; with the asset collection
        // initialised either way, `spawn_scene_entities` still runs and still mirrors every prop.
        // Both guarded on the plugins that provide what they need, which `MinimalPlugins` does not:
        // `init_asset` wants the `AssetServer` from `AssetPlugin`, and `MaterialPlugin` wants the
        // render sub-app. This is the same shape of guard `TrailPlugin` uses for its gizmo group, and
        // it is what lets a headless test register `Assets<BodyMaterial>` by hand and still exercise
        // the whole spawn path.
        if app.world().get_resource::<AssetServer>().is_some() {
            app.init_asset::<BodyMaterial>();
        }
        if app.is_plugin_added::<bevy::render::RenderPlugin>() {
            app.add_plugins(MaterialPlugin::<BodyMaterial>::default());
        }

        app.add_systems(Startup, spawn_scene_entities);
        app.add_systems(
            Update,
            (
                advance_clocks_system,
                step_scene_system.after(advance_clocks_system),
                // The mirror is reconciled *before* transforms are synced, so a prop that
                // appeared this frame is drawn in the right place on its first frame rather
                // than at the origin. Props can now come and go — see
                // `sync_mirror_entities_system`.
                sync_mirror_entities_system.after(step_scene_system),
                sync_scene_to_ecs_system.after(sync_mirror_entities_system),
                // After the transforms, so the occluder positions the shader gets are the ones that
                // were drawn this frame rather than last frame's.
                update_eclipse_occluders_system.after(sync_scene_to_ecs_system),
            ),
        );
    }
}

/// Convert our linear-space `Color` into Bevy's `bevy::color::Color`.
///
/// Our `Color` holds four `f32` channels in linear-space RGBA (see
/// Requirement 6.4 / 7.6), which is exactly what
/// [`bevy::color::Color::linear_rgba`] accepts, so this is a
/// component-wise copy — no gamma conversion, no channel reordering.
/// Isolating the conversion in one `#[inline]` helper keeps the
/// spawn-loop below readable and makes it obvious that `StandardMaterial`
/// receives a *linear* color (the PBR pipeline expects linear-space
/// inputs; handing it an sRGB-encoded value would wash the scene out).
#[inline]
fn to_bevy_color(c: Color) -> BevyColor {
    BevyColor::linear_rgba(c.r, c.g, c.b, c.a)
}

/// Startup system that seeds the Bevy world with the camera, a
/// directional light, and one `PbrBundle` per body in `Scene`.
///
/// Contract (Requirement 3.5):
///
/// - Spawns exactly **one** `Camera3dBundle` entity, positioned at
///   `(5, 5, 5)` and looking at the origin, with a
///   [`FreeFlyController::default()`] component attached so task 8's
///   camera systems have an entity to drive.
/// - Spawns **at least one** `DirectionalLightBundle` so the scene has
///   visible 3D shading (the PBR default material is matte-white and
///   would render as a uniform silhouette without a light).
/// - For every `(id, body)` in `scene.iter()`, spawns a `PbrBundle`
///   tagged with [`MirroredBody(id)`], whose `mesh`, `material`, and
///   `transform` are derived from `body`. After this system runs,
///   `count_entities_with::<MirroredBody>() == scene.props.len()`
///   (Requirement 3.5), which is what the headless test in task 7.9
///   asserts.
///
/// Mesh choices match the `transform_from` conventions so scales from
/// the render bridge compose correctly (Requirements 8.3–8.6):
///
/// - `Sphere` uses a unit-radius `Sphere::new(1.0)` mesh; the bridge
///   multiplies scale by `radius`.
/// - `Cuboid` uses a unit `Cuboid::new(1.0, 1.0, 1.0)` mesh (full
///   extents `1×1×1`); the bridge multiplies scale by `2.0 *
///   half_extents`, so the resulting rendered box has size
///   `2 * half_extents` — exactly the half-extents convention in
///   Requirement 6.1.
/// - `Plane` uses a unit `Plane3d` mesh (1×1 quad on `+Y`); the bridge
///   scales `x` and `z` by `2.0 * half_size`, producing a
///   `2 * half_size` square quad — the same convention as `Cuboid`
///   above.
/// - `Line` uses a thin `Cuboid` (`1.0 × 0.01 × 0.01`) as a renderable
///   proxy: the bridge's line case sets `scale.x = |to - from|`,
///   rotates `+X` onto the direction vector, and translates to the
///   midpoint, so the resulting rod visually approximates a line of
///   the requested length. Bevy 0.14 has no first-class line mesh
///   primitive that works with `PbrBundle`, and standing up a gizmo
///   line system here would pull in rendering concerns this task
///   doesn't own; the thin-rod approximation keeps `spawn_scene_entities`
///   purely a bundle-spawner while still producing something visible.
pub fn spawn_scene_entities(
    mut commands: Commands,
    scene: Res<Scene>,
    spec: Res<ExperimentSpec>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
) {
    // Camera: framed on the two-body pair, which now starts 30 units apart.
    //
    // The spheres sit at x = ±15, so the view needs a half-width of at least
    // ~17 units. Bevy's default FOV is 45 degrees vertical, giving a vertical
    // half-angle of 22.5 degrees, so the camera needs to be at least
    // 17 / tan(22.5 deg) ≈ 41 units away. (0, 18, 42) looking at (0, 3, 0) is
    // ~44.6 units out, leaving margin for the spheres to swing past each other
    // without leaving the frame.
    // Attaching `FreeFlyController::default()` here — rather than in
    // a separate system — is what lets task 8.1's controller defaults
    // take effect without any additional wiring (Requirement 10.1).
    commands.spawn((
        Camera3dBundle {
            // Framing comes from the experiment, not from a constant: each scene
            // has its own extent and its own interesting viewing angle.
            transform: Transform::from_translation(spec.camera_position_units)
                .looking_at(spec.camera_target_units, Vec3::Y),
            // Everything else is Bevy's default projection, except the clip planes. The
            // default far plane of 1000 units would delete Saturn, Uranus, Neptune, and most
            // of the floor from `experiment_3`, and the default near plane of 0.1 units kept
            // the camera 55 Io radii from Io. See `CAMERA_FAR_UNITS` and `CAMERA_NEAR_UNITS`.
            projection: PerspectiveProjection {
                near: CAMERA_NEAR_UNITS,
                far: CAMERA_FAR_UNITS,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        },
        FreeFlyController::default(),
        CameraLock::default(),
    ));

    // One directional light: PBR materials need a light source to
    // shade against, otherwise every non-emissive body renders as a
    // uniform silhouette against the clear color. The exact position
    // is chosen so the light casts at a slanted angle across the
    // ground plane (it's a directional light, so only the rotation
    // matters for lighting direction, but `looking_at` is the
    // convenient way to aim it).
    // Only when the scene has no star of its own. A scene *with* one is lit from where the star
    // actually is — see `spawn_mirror` — and adding a fixed directional light on top would wash out
    // the terminator that lighting is there to produce. The two-body pair, the three-body ring, the
    // Jovian system, and the scale-check row have no star, so they keep this.
    if !scene.props.iter().any(Prop::is_luminous) {
        commands.spawn(DirectionalLightBundle {
            transform: Transform::from_translation(LIGHT_POSITION_UNITS)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        });
    }

    // Mirror every prop in `scene` as a PbrBundle entity tagged with
    // `MirroredBody(id)`. Mesh + material are built per-variant so the
    // `transform_from` scale convention holds (see doc comment above).
    //
    // Only `prop.body` is read here: a prop's velocity and mass are simulation
    // state with no visual representation, so the render world never sees them.
    for (id, prop) in scene.iter() {
        spawn_mirror(&mut commands, &mut meshes, &mut materials, id, prop);
    }
}

/// The unit-sized mesh and the colour a `Body` is drawn with.
///
/// Every mesh is **unit-sized** and scaled by [`transform_from`], which is the convention
/// the whole bridge rests on: the model owns the size, the transform applies it, and a body
/// whose radius changes at runtime — a black hole that has just eaten something — needs no
/// new mesh.
///
/// Extracted from `spawn_scene_entities` when props became able to appear *after* startup, so
/// the startup spawn and [`sync_mirror_entities_system`] cannot drift apart in how they build
/// an entity.
fn mesh_and_color(body: &Body) -> (Mesh, Color) {
    match body {
        Body::Sphere { color, .. } => {
            // Unit-radius sphere mesh; scale is applied by
            // `transform_from` via `Vec3::splat(radius)`.
            (Mesh::from(Sphere::new(1.0)), *color)
        }
        Body::Cuboid { color, .. } => {
            // Unit cuboid (1×1×1 full extents); `transform_from`
            // scales by `2.0 * half_extents`.
            (Mesh::from(Cuboid::new(1.0, 1.0, 1.0)), *color)
        }
        Body::Plane { color, .. } => {
            // Unit plane (1×1 quad on +Y); `transform_from` scales
            // x and z by `2.0 * half_size`.
            (Mesh::from(Plane3d::default().mesh().size(1.0, 1.0)), *color)
        }
        Body::Line { color, .. } => {
            // Thin rod proxy for a line. `transform_from` sets
            // `scale.x` to the line's length, so the resulting
            // visual width and depth are `0.01` world units,
            // which reads as a line at typical scene distances.
            (
                Mesh::from(Cuboid::new(
                    1.0,
                    LINE_VISUAL_THICKNESS_UNITS,
                    LINE_VISUAL_THICKNESS_UNITS,
                )),
                *color,
            )
        }
    }
}

/// Spawn the mirror entity for one prop: a `PbrBundle` tagged with [`MirroredBody`].
///
/// The single place an entity enters the mirror, called both by `spawn_scene_entities` at
/// startup and by [`sync_mirror_entities_system`] for a prop that appeared later.
fn spawn_mirror(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<BodyMaterial>,
    id: BodyId,
    prop: &Prop,
) {
    let body = &prop.body;
    let (mesh, color) = mesh_and_color(body);

    let mut material = StandardMaterial::from(to_bevy_color(color));

    // A star is lit from *inside* — its point light sits at its own centre — so with only reflected
    // light its surface would render black, which is the one thing a star must not be. The emissive
    // term is what makes it a disc rather than a silhouette.
    if prop.is_luminous() {
        material.emissive = LinearRgba::rgb(
            color.r * STAR_EMISSIVE_SCALE,
            color.g * STAR_EMISSIVE_SCALE,
            color.b * STAR_EMISSIVE_SCALE,
        );
    }

    let mut entity = commands.spawn((
        MaterialMeshBundle::<BodyMaterial> {
            mesh: meshes.add(mesh),
            // The eclipse extension starts with no star and no occluders, which draws exactly as
            // plain PBR would. `update_eclipse_occluders_system` fills it in on the first frame.
            material: materials.add(BodyMaterial {
                base: material,
                extension: EclipseExtension::default(),
            }),
            transform: transform_from(body),
            ..Default::default()
        },
        MirroredBody(id),
    ));

    if matches!(body, Body::Sphere { .. } | Body::Cuboid { .. }) {
        entity.insert(CameraTarget);
    }

    // The light is a **child** of the body, not a component on it, and that distinction is the whole
    // of a bug worth remembering.
    //
    // The obvious thing is to put the `PointLight` on the mesh entity: `PointLightBundle` adds only
    // three components beyond `PbrBundle`, and then `sync_scene_to_ecs_system` carries the light
    // with the star for free. It works, and it goes dark the moment you look away from the star.
    //
    // `extract_lights` skips any light whose entity has `ViewVisibility == false`, and
    // `check_visibility` frustum-culls exactly those entities that **have an `Aabb`** — which a mesh
    // entity gets from `calculate_bounds` and a light-only entity never does. So a light sharing an
    // entity with a mesh inherits the mesh's culling, and the Sun leaving the frame took the
    // sunlight with it.
    //
    // A child gets the parent's translation through transform propagation and its own
    // `ViewVisibility`, which is never culled because it has no bounds to cull against. Note
    // `InheritedVisibility` is *not* affected by frustum culling, so the child stays lit while the
    // parent mesh is off screen.
    if prop.is_luminous() {
        let color = to_bevy_color(body_color(body));
        let intensity = prop.luminosity_solar * STAR_LUMENS_PER_SOLAR_LUMINOSITY;

        entity.with_children(|star| {
            star.spawn(PointLightBundle {
                point_light: PointLight {
                    color,
                    intensity,
                    range: STAR_LIGHT_RANGE_UNITS,
                    // Shadows off: a cube shadow map spanning a 20,000-unit range would have a texel
                    // wider than a planet, so it would cost a great deal and resolve nothing.
                    shadows_enabled: false,
                    ..Default::default()
                },
                // Identity: the parent's transform puts it at the star's centre. The parent's
                // *scale* propagates too and is harmless — a point light has no size.
                ..Default::default()
            });
        });
    }
}

/// Rewrite every body's eclipse uniform: where the star is, and which spheres could shadow it.
///
/// Runs after the transform sync so the occluder positions are this frame's. One uniform per body,
/// because a body must not be listed as its own occluder — a sphere's surface is exactly at its own
/// radius, and including it would make the terminator depend on floating-point noise rather than on
/// the lighting.
///
/// # Why the work is per frame and why that is cheap
///
/// The occluders move, so the list is not cacheable. It is `n` bodies × `n−1` occluders of `Vec4`
/// writes — `182` for the solar system — against a frame that already costs `98` substeps of
/// gravitation. The GPU cost is a re-upload of `256` bytes per material.
///
/// # A scene with no star
///
/// Leaves `star.w` at `0.0`, which the shader reads as "nothing to eclipse" and returns full
/// visibility for without touching the occluder loop. That is four of the five experiments, all lit by
/// a directional light, and it costs them one comparison per fragment.
///
/// # The floor
///
/// With [`PerformanceSettings::floor_eclipse`] off, a `Plane` gets the same no-star uniform and skips
/// the loop. The floor is nearly every shaded pixel of the solar system, so that is a third of its
/// frame time, for at most a `1.3%` penumbra; see `src/performance/README.md`.
pub fn update_eclipse_occluders_system(
    scene: Res<Scene>,
    settings: Option<Res<PerformanceSettings>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    bodies: Query<(&MirroredBody, &Handle<BodyMaterial>)>,
) {
    let floor_eclipse = settings.map_or_else(
        || PerformanceSettings::default().floor_eclipse,
        |settings| settings.floor_eclipse,
    );

    // The star, if this scene has one. The first luminous prop wins: a scene with two stars would
    // cast shadows from only one of them, which is a limitation recorded in A22 rather than a bug to
    // find later.
    let star = scene
        .iter()
        .find(|(_, prop)| prop.is_luminous())
        .and_then(|(id, prop)| match &prop.body {
            Body::Sphere { radius, .. } => Some((id, prop.body.center(), *radius)),
            _ => None,
        });

    let Some((star_id, star_centre, star_radius)) = star else {
        return;
    };

    // Every sphere that is neither the star nor flat. Collected once and filtered per body, rather
    // than rebuilt n times.
    let occluders: Vec<(BodyId, Vec4)> = scene
        .iter()
        .filter(|(id, _)| *id != star_id)
        .filter_map(|(id, prop)| match &prop.body {
            Body::Sphere { radius, .. } if *radius > 0.0 => {
                Some((id, prop.body.center().extend(*radius)))
            }
            _ => None,
        })
        .collect();

    for (mirrored, handle) in &bodies {
        let Some(material) = materials.get_mut(handle) else {
            continue;
        };

        if !floor_eclipse
            && matches!(
                scene.get(mirrored.0).map(|p| &p.body),
                Some(Body::Plane { .. })
            )
        {
            material.extension.uniform = EclipseUniform::default();
            continue;
        }

        let mut uniform = EclipseUniform {
            star: star_centre.extend(star_radius),
            count: UVec4::ZERO,
            spheres: [Vec4::ZERO; MAX_ECLIPSE_OCCLUDERS],
        };

        let mut written = 0;
        for (id, sphere) in &occluders {
            if *id == mirrored.0 || written >= MAX_ECLIPSE_OCCLUDERS {
                continue;
            }
            uniform.spheres[written] = *sphere;
            written += 1;
        }
        uniform.count.x = written as u32;

        material.extension.uniform = uniform;
    }
}

/// A body's own colour, whichever variant it is.
///
/// `mesh_and_color` returns it alongside a mesh; this is the same value without building geometry,
/// for the cases that only need the colour.
fn body_color(body: &Body) -> Color {
    match body {
        Body::Sphere { color, .. }
        | Body::Cuboid { color, .. }
        | Body::Plane { color, .. }
        | Body::Line { color, .. } => *color,
    }
}

/// Keep the ECS mirror in step with a `Scene` that can gain and lose props.
///
/// Spawns an entity for every prop with no mirror, and despawns every mirror whose prop has
/// gone. Runs after `step_scene_system`, so a prop absorbed this frame loses its entity in
/// the same frame it left the scene.
///
/// # Why this system had to exist
///
/// `spawn_scene_entities` runs once, on `Startup`, which was sufficient for exactly as long
/// as the scene was fixed at build time. Two changes broke that in opposite directions:
/// **placing** an object with `B` adds a prop, and **absorption** removes one. Without this
/// system the first would be invisible — simulated, pulling on everything, drawn nowhere —
/// and the second would leave an entity frozen at the last position of a prop that no longer
/// exists, which `sync_scene_to_ecs_system` deliberately leaves untouched (Requirement 9.2).
///
/// So the mirror invariant that Requirement 3.5 states for startup —
/// `count_entities_with::<MirroredBody>() == scene.props.len()` — is now maintained *every
/// frame* rather than established once. That is a strengthening, and
/// `plugin_tests::the_mirror_follows_props_appearing_and_disappearing` is what holds it.
pub fn sync_mirror_entities_system(
    mut commands: Commands,
    scene: Res<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mirrored: Query<(Entity, &MirroredBody)>,
) {
    // One pass over the entities, so the two directions below are both O(n) against a
    // scene of a handful of props rather than O(n²).
    let mut live: Vec<BodyId> = Vec::with_capacity(mirrored.iter().len());

    for (entity, mirrored_body) in &mirrored {
        let id = mirrored_body.0;
        if scene.get(id).is_some() {
            live.push(id);
        } else {
            // The prop is gone — absorbed, in every case that exists today. Despawn rather
            // than hide: a hidden entity is state that has to be found again later, and the
            // id will never come back (`Scene::remove` documents why).
            //
            // **Recursively**, because a luminous prop carries its light as a child. A plain
            // `despawn` would leave an orphaned `PointLight` still lighting the scene from wherever
            // the star was when it was swallowed.
            commands.entity(entity).despawn_recursive();
        }
    }

    for (id, prop) in scene.iter() {
        if !live.contains(&id) {
            spawn_mirror(&mut commands, &mut meshes, &mut materials, id, prop);
        }
    }
}

/// Clamp a raw frame-delta into the closed interval `[0.0, max_dt]`.
///
/// Contract (Requirements 13.1, 13.2, 13.3, 13.4):
///
/// - `raw` negative or not finite (`NaN`, `+inf`, `-inf`) → `0.0` (13.3).
/// - `raw > max_dt` → `max_dt` (13.2).
/// - Otherwise `raw` is returned unchanged (13.1).
/// - The returned value is always finite and lies in `[0.0, max_dt]`
///   (13.4), provided `max_dt` satisfies its own precondition below.
///
/// # Preconditions
///
/// `max_dt > 0.0` and `max_dt.is_finite()` (Requirement 13.5). A
/// `debug_assert!` catches violations in debug builds; release builds
/// pass through the caller-provided value and the postconditions still
/// hold so long as the precondition is met.
///
/// The ordering of the branches matters: the finite/negative check
/// comes first so that a `NaN` input — which fails every numeric
/// comparison — lands in the `0.0` branch rather than silently
/// propagating through `max_dt.min(raw)` and returning `NaN`.
///
/// # This is now a delegate
///
/// The implementation moved to [`clamp_real_step`] in [`crate::time`], where the
/// simulation clock can reach it without depending on the render layer. This
/// function remains because Requirement 13 names it and a body of tests pins it;
/// the two are the same function and there is only one implementation.
#[inline]
pub fn clamp_dt(raw: f32, max_dt: f32) -> f32 {
    clamp_real_step(raw, max_dt)
}

/// Bevy system that advances both clocks from the frame delta, once per tick.
///
/// **The only place a Bevy timer enters the crate.** It reads
/// `time.delta_seconds()` and hands it to:
///
/// - the [`ObserverClock`], which keeps it as real time for the camera and the FPS
///   readout; and
/// - the [`SimulationClock`], which clamps it, scales it into simulated time,
///   accumulates the total, and stores the resulting
///   [`TimeStep`](crate::time::simulation::TimeStep) for
///   [`step_scene_system`] to read.
///
/// Splitting this out of `step_scene_system` is what lets systems that must not
/// step the scene — the camera, the overlay, the pause controls — still see time.
/// It runs first in the `Update` chain, so every consumer in the same tick sees the
/// same two readings.
pub fn advance_clocks_system(
    time: Res<Time>,
    mut simulation: ResMut<SimulationClock>,
    mut observer: ResMut<ObserverClock>,
) {
    let raw = time.delta_seconds();

    observer.advance(raw);
    simulation.advance(raw);

    debug_assert!(
        simulation.is_valid() && observer.is_valid(),
        "advance_clocks_system: a clock left its Validation_Rules: {:?}, {:?}",
        *simulation,
        *observer
    );
}

/// Bevy system that advances the `Scene` by a clamped `dt` exactly once
/// per `Update` tick.
///
/// Contract (Requirements 2.1, 2.2, 13.1, 13.2, 13.3, 13.4):
///
/// - Reads the current frame delta via `time.delta_seconds()`.
/// - Runs it through [`clamp_dt`] against `config.max_dt`, so the value
///   actually passed to `Scene::step` is always finite and in
///   `[0.0, config.max_dt]` (Requirement 13.4), independent of the
///   raw timer value.
/// - Calls `scene.step(dt)` exactly once (Requirement 2.2). The
///   plugin registers this system once per `Update` schedule, so the
///   full per-frame contract of Requirement 2.1 (one `step` then one
///   `sync`) is enforced by the plugin wiring in task 7.7, not by
///   this function.
///
/// It no longer computes the timestep — [`advance_clocks_system`] did that, and
/// this system reads [`SimulationClock::last_step`]. `Scene` therefore never learns
/// about `bevy::time::Time`, which is what preserves its GUI-free contract, and the
/// clamp-and-scale rule lives in one testable place in [`crate::time`] rather than
/// inline in a Bevy system.
///
/// When the clock is paused the step is
/// [`TimeStep::ZERO`](crate::time::simulation::TimeStep::ZERO) and `Scene::step` returns
/// immediately, so a paused simulation costs nothing while the camera keeps running.
///
/// When [`TrailPlugin`](crate::render::trail::TrailPlugin) is installed, trails are recorded
/// after every substep rather than once per frame, so they follow the path the integrator
/// computed between frames. When a powered rocket is flying, the mission flies it before every
/// substep, since its engine and its `f64` state are not the scene's to integrate. See
/// [`Scene::step_steered`] and `Mission::steer`.
pub fn step_scene_system(
    clock: Res<SimulationClock>,
    mut scene: ResMut<Scene>,
    trails: Option<ResMut<Trails>>,
    trail_config: Option<Res<TrailConfig>>,
    mut mission: Option<ResMut<Mission>>,
) {
    // The mission flies a powered rocket before each substep (see `Mission::steer`), and both
    // trails and mission see the scene after it.
    let mut observers = (trails.zip(trail_config), mission.as_mut());
    scene.step_steered(
        clock.last_step(),
        &mut observers,
        |(_, mission), scene, substep| {
            if let Some(mission) = mission.as_mut() {
                mission.steer(scene, substep);
            }
        },
        |(trails, mission), scene| {
            if let Some((trails, config)) = trails.as_mut() {
                trails.record_from(scene, config);
            }
            if let Some(mission) = mission.as_mut() {
                mission.observe(scene);
            }
        },
    );
}

#[cfg(test)]
#[path = "../../tst/render/plugin_tests.rs"]
mod tests;
