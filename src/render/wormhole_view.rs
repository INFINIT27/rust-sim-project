//! The wormhole drawn on the GPU: every pixel's light followed to the sky it came from.
//!
//! A sphere centred on the camera carries [`WormholeMaterial`], whose fragment shader
//! (`assets/shaders/wormhole.wgsl`) is a transcription of [`crate::render::wormhole_ray`], its
//! tested Rust twin. Only the sphere's back faces are drawn, so there is one fragment per pixel, and
//! the shader writes the very back of the view as its depth: the skies are behind everything.
//!
//! # What it shows
//!
//! - **The near side's sky**, the same stars as the black hole's, bent round the mouth, with its
//!   Einstein ring.
//! - **The far side's sky** through the mouth: other stars, and a nebula, so the two cannot be taken
//!   for one another. From the far side the two change places.
//! - **The bodies of the scene**, as spheres the light meets on its way: bent round the mouth, and seen
//!   through it when they are on the other side.
//! - **In the tunnel**, both at once: the far sky ahead, the near one behind.
//!
//! [`Action::ToggleBlackHoleView`](crate::bindings::Action::ToggleBlackHoleView) (`V`) draws lines of
//! latitude and longitude instead, orange for the near sky and blue for the far one, and
//! [`Action::ToggleStars`](crate::bindings::Action::ToggleStars) (`N`) turns the stars off. The
//! quality, stars and on/off switches are the black hole's, in
//! [`PerformanceSettings`]. Off draws the mouth as a plain ball.
//!
//! # Limits
//!
//! At most [`WORMHOLE_MAX_BODIES`] spheres are drawn, the nearest along the wormhole. They cast no shadows
//! on one another. Trails are not drawn, as a straight line is not where light would show one.

use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::asset::{Asset, Assets, Handle};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, Resource};
use bevy::math::{primitives::Sphere, DVec3, UVec4, Vec3, Vec4};
use bevy::pbr::{
    Material, MaterialMeshBundle, MaterialPipeline, MaterialPipelineKey, MaterialPlugin,
    NotShadowCaster, StandardMaterial,
};
use bevy::reflect::TypePath;
use bevy::render::mesh::{Mesh, MeshVertexBufferLayoutRef, Meshable};
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
};
use bevy::render::view::Visibility;
use bevy::transform::components::Transform;
use bevy::transform::TransformSystem;

use crate::core::scene::{Body, Prop, Scene};

use crate::input::camera::{orbit_camera_system, CameraTarget};
use crate::performance::PerformanceSettings;
use crate::physics_utils::relativity::wormhole::WormholeShape;
use crate::projects::interstellar::catalog::ExperimentSpec;
use crate::projects::interstellar::constants::WORMHOLE_COLOR;
use crate::projects::interstellar::wormhole::{is_wormhole, WormholeSettings};
use crate::render::black_hole_view::{hide_hole_body_system, ray_marched, BlackHoleView};
use crate::render::constants::{
    KERR_RAY_MAX_STEPS, KERR_RAY_STEP_FRACTION, WORMHOLE_EXPOSURE, WORMHOLE_MAX_BODIES,
    WORMHOLE_RAY_MAX_STEPS, WORMHOLE_RAY_STEP_FRACTION, WORMHOLE_SHADER_PATH,
    WORMHOLE_VIEW_SPHERE_UNITS,
};
use crate::render::plugin::{step_scene_system, sync_mirror_entities_system, MirroredBody};
use crate::render::wormhole_camera::{wormhole_camera_system, WormholePlace, WormholeTraveller};
use crate::render::wormhole_ray::RayWormhole;

pub use uniform::WormholeUniform;

/// The wormhole's shape is an ECS resource, so the menu and `config/wormhole.toml` can set it.
impl Resource for WormholeSettings {}

/// Wrapper module for [`WormholeUniform`], for the same `dead_code` reason as
/// [`EclipseUniform`](crate::render::eclipse::EclipseUniform)'s.
mod uniform {
    #![allow(dead_code)]

    use bevy::math::{UVec4, Vec4};
    use bevy::render::render_resource::ShaderType;

    use crate::render::constants::WORMHOLE_MAX_BODIES;

    /// The wormhole, the camera's place in it, and the march's settings, as the shader reads them.
    #[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
    pub struct WormholeUniform {
        /// `xyz`: the camera's label. `w`: its `ℓ`, in throat radii.
        pub camera: Vec4,
        /// Half-length, flare (both in throat radii), step fraction, exposure.
        pub shape: Vec4,
        /// `x`: the most steps per ray. `y`: `1` for the test pattern. `z`: `1` for stars. `w`: how many bodies.
        pub limits: UVec4,
        /// Each body: `xyz` its label, `w` its `ℓ` in throat radii.
        pub places: [Vec4; WORMHOLE_MAX_BODIES],
        /// `xyz`: the direction of its star, in the label frame at it. `w`: its radius.
        pub lights: [Vec4; WORMHOLE_MAX_BODIES],
        /// `rgb`: its colour. `a`: `1` if it glows.
        pub colours: [Vec4; WORMHOLE_MAX_BODIES],
    }
}

impl WormholeUniform {
    /// The uniform for a camera at `place` in `shape`, drawn as `view` with `settings`' quality and
    /// stars.
    pub fn new(
        shape: &WormholeShape,
        place: &WormholePlace,
        view: BlackHoleView,
        settings: &PerformanceSettings,
    ) -> Self {
        let ray = ray_wormhole(shape, settings);
        let throat = shape.throat();
        Self {
            camera: place.n.as_vec3().extend((place.l / throat) as f32),
            shape: Vec4::new(
                ray.half_length,
                ray.flare,
                ray.step_fraction,
                WORMHOLE_EXPOSURE,
            ),
            limits: UVec4::new(
                ray.max_steps,
                u32::from(view == BlackHoleView::Pattern),
                u32::from(settings.black_hole_stars),
                0,
            ),
            ..Self::default()
        }
    }
}

/// The wormhole as the ray-marcher sees it: `shape` in throat radii, at `settings`' quality (the
/// same halving and doubling as the black hole's).
pub fn ray_wormhole(shape: &WormholeShape, settings: &PerformanceSettings) -> RayWormhole {
    let throat = shape.throat();
    let flare = match *shape {
        WormholeShape::Dneg { flare, .. } => flare,
        // The shader draws the Dneg shape only.
        WormholeShape::Ellis { throat } => throat,
    };
    let quality = settings.black_hole_quality;
    let coarser = quality.step_fraction() / KERR_RAY_STEP_FRACTION;
    let longer = f64::from(quality.max_steps()) / f64::from(KERR_RAY_MAX_STEPS);
    RayWormhole {
        step_fraction: WORMHOLE_RAY_STEP_FRACTION * coarser,
        max_steps: (f64::from(WORMHOLE_RAY_MAX_STEPS) * longer) as u32,
        ..RayWormhole::new(
            (shape.half_length() / throat) as f32,
            (flare / throat) as f32,
        )
    }
}

/// The material that ray-marches the wormhole: [`WormholeUniform`] at `@group(2) @binding(0)`.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct WormholeMaterial {
    /// Rewritten when the camera moves or a setting changes ([`sync_wormhole_view_system`]).
    #[uniform(0)]
    pub uniform: WormholeUniform,
}

impl Material for WormholeMaterial {
    fn fragment_shader() -> ShaderRef {
        WORMHOLE_SHADER_PATH.into()
    }

    /// Back faces only: one fragment per pixel.
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// Marks the sphere the wormhole is ray-marched on, with the uniform its material holds.
#[derive(Component, Clone, Copy, Debug)]
pub struct WormholeSphere {
    /// What its material holds.
    pub uniform: WormholeUniform,
}

/// Bevy system that keeps one ray-marched sphere round the camera in the wormhole experiment, with
/// the camera's place in its uniform; hidden while the ray-marcher is off.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn sync_wormhole_view_system(
    mut commands: Commands,
    spec: Option<Res<ExperimentSpec>>,
    traveller: Res<WormholeTraveller>,
    shape: Res<WormholeSettings>,
    scene: Res<Scene>,
    view: Option<Res<BlackHoleView>>,
    settings: Option<Res<PerformanceSettings>>,
    cameras: Query<&Transform, (With<Camera3d>, Without<WormholeSphere>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WormholeMaterial>>,
    mut spheres: Query<(
        Entity,
        &mut WormholeSphere,
        &Handle<WormholeMaterial>,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let wanted = spec
        .filter(|spec| is_wormhole(spec))
        .and(traveller.place)
        .zip(cameras.iter().next());
    let Some((place, camera)) = wanted else {
        for (entity, ..) in &spheres {
            commands.entity(entity).despawn();
        }
        return;
    };
    let settings = settings.map_or_else(PerformanceSettings::default, |s| *s);
    let view = view.map_or_else(BlackHoleView::default, |v| *v);
    let uniform =
        WormholeUniform::new(&shape.shape(), &place, view, &settings).with_bodies(&scene, &place);
    let visibility = if ray_marched(Some(&settings)) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let placed = Transform::from_translation(camera.translation)
        .with_scale(Vec3::splat(WORMHOLE_VIEW_SPHERE_UNITS));

    if spheres.is_empty() {
        commands.spawn((
            MaterialMeshBundle {
                mesh: meshes.add(Sphere::new(1.0).mesh().uv(64, 32)),
                material: materials.add(WormholeMaterial { uniform }),
                transform: placed,
                visibility,
                ..Default::default()
            },
            NotShadowCaster,
            WormholeSphere { uniform },
        ));
        return;
    }
    for (_, mut sphere, handle, mut transform, mut shown) in &mut spheres {
        if *transform != placed {
            *transform = placed;
        }
        if sphere.uniform != uniform {
            sphere.uniform = uniform;
            if let Some(material) = materials.get_mut(handle) {
                material.uniform = uniform;
            }
        }
        if *shown != visibility {
            *shown = visibility;
        }
    }
}

/// Marks the mouth itself: what the `F` key locks onto, and the ball drawn when the ray-marcher is
/// off. Not a body of the scene, so bodies pass through it.
#[derive(Component, Clone, Copy, Debug)]
pub struct WormholeMouth;

impl WormholeUniform {
    /// Put the spheres of `scene` in it: the nearest [`WORMHOLE_MAX_BODIES`] to `camera` along the
    /// wormhole, each lit by the nearest luminous body on its own side.
    pub fn with_bodies(mut self, scene: &Scene, camera: &WormholePlace) -> Self {
        let Some(space) = &scene.wormhole else {
            return self;
        };
        let throat = space.shape.throat();
        let mut placed: Vec<(WormholePlace, &Prop)> = scene
            .iter()
            .filter(|(_, prop)| matches!(prop.body, Body::Sphere { .. }))
            .filter_map(|(id, prop)| Some((space.place(id)?, prop)))
            .collect();
        placed.sort_by(|(a, _), (b, _)| (a.l - camera.l).abs().total_cmp(&(b.l - camera.l).abs()));
        placed.truncate(WORMHOLE_MAX_BODIES);
        for (i, (place, prop)) in placed.iter().enumerate() {
            let Body::Sphere { radius, color, .. } = prop.body else {
                continue;
            };
            let at = place.position(&space.shape);
            let towards = placed
                .iter()
                .filter(|(star, lamp)| {
                    lamp.is_luminous()
                        && !std::ptr::eq(*lamp, *prop)
                        && is_far(star) == is_far(place)
                })
                .map(|(star, _)| star.position(&space.shape) - at)
                .min_by(|a, b| a.length().total_cmp(&b.length()));
            let star = towards.map_or(DVec3::ZERO, |way| {
                place.frame().inverse() * way.normalize_or_zero()
            });
            self.places[i] = place.n.as_vec3().extend((place.l / throat) as f32);
            self.lights[i] = star.as_vec3().extend((f64::from(radius) / throat) as f32);
            self.colours[i] = Vec4::new(
                color.r,
                color.g,
                color.b,
                f32::from(u8::from(prop.is_luminous())),
            );
        }
        self.limits.w = placed.len() as u32;
        self
    }
}

/// Whether a place is on the far side of the middle.
fn is_far(place: &WormholePlace) -> bool {
    place.l < 0.0
}

/// Bevy system that keeps the wormhole of the scene the shape of the settings, before it is stepped.
pub fn reshape_wormhole_system(shape: Res<WormholeSettings>, mut scene: ResMut<Scene>) {
    let wanted = shape.shape();
    let differs = scene
        .wormhole
        .as_ref()
        .is_some_and(|space| space.shape != wanted);
    if differs {
        scene.reshape_wormhole(wanted);
    }
}

/// Bevy system that decides, in a wormhole, how each body is shown. While the wormhole is ray-marched
/// the shader draws every body, bent as its light is, so their meshes are hidden; with the ray-marcher
/// off only those on the side the camera is on are. And only bodies outside the mouth on that side
/// can be locked onto: the others are not anywhere that the camera can orbit.
#[allow(clippy::type_complexity)]
pub fn sync_wormhole_bodies_system(
    mut commands: Commands,
    scene: Res<Scene>,
    traveller: Res<WormholeTraveller>,
    settings: Option<Res<PerformanceSettings>>,
    mut bodies: Query<
        (Entity, &MirroredBody, &mut Visibility, Has<CameraTarget>),
        Without<WormholeSphere>,
    >,
) {
    let Some(space) = &scene.wormhole else {
        return;
    };
    let marched = ray_marched(settings.as_deref());
    let camera_far = traveller.place.is_some_and(|place| is_far(&place));
    for (entity, body, mut visibility, lockable) in &mut bodies {
        // A body added this frame has no place until it is stepped; it is on the near side.
        let place = space.place(body.0);
        let same_side = place.is_some_and(|place| is_far(&place)) == camera_far;
        let wanted = if marched || !same_side {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        let outside = place.is_none_or(|place| place.l.abs() > space.shape.half_length());
        let reachable = same_side && outside;
        if reachable && !lockable {
            commands.entity(entity).insert(CameraTarget);
        } else if !reachable && lockable {
            commands.entity(entity).remove::<CameraTarget>();
        }
    }
}

/// Bevy system that keeps one [`WormholeMouth`] at the centre in the wormhole experiment, as large as
/// the throat, drawn only while the ray-marcher is off.
pub fn sync_wormhole_mouth_system(
    mut commands: Commands,
    spec: Option<Res<ExperimentSpec>>,
    shape: Res<WormholeSettings>,
    settings: Option<Res<PerformanceSettings>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mouths: Query<(&mut Transform, &mut Visibility), With<WormholeMouth>>,
) {
    if !spec.is_some_and(|spec| is_wormhole(&spec)) {
        return;
    }
    let scale = Vec3::splat(shape.shape().throat() as f32);
    let visibility = if ray_marched(settings.as_deref()) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    if mouths.is_empty() {
        commands.spawn((
            MaterialMeshBundle {
                mesh: meshes.add(Sphere::new(1.0).mesh().uv(64, 32)),
                material: materials.add(StandardMaterial {
                    base_color: bevy::color::Color::linear_rgb(
                        WORMHOLE_COLOR.r,
                        WORMHOLE_COLOR.g,
                        WORMHOLE_COLOR.b,
                    ),
                    ..Default::default()
                }),
                transform: Transform::from_scale(scale),
                visibility,
                ..Default::default()
            },
            NotShadowCaster,
            CameraTarget,
            WormholeMouth,
        ));
    }
    for (mut transform, mut shown) in &mut mouths {
        if transform.scale != scale {
            transform.scale = scale;
        }
        if *shown != visibility {
            *shown = visibility;
        }
    }
}

/// Draws the wormhole experiment by ray-marching it on the GPU, and flies the camera through it.
///
/// Add it after [`BlackHoleViewPlugin`](super::black_hole_view::BlackHoleViewPlugin), whose view and
/// star keys it shares. Without the render app (headless) it only moves the camera.
pub struct WormholeViewPlugin;

impl Plugin for WormholeViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WormholeSettings>();
        app.init_resource::<WormholeTraveller>();
        app.add_systems(Update, reshape_wormhole_system.before(step_scene_system));
        app.add_systems(
            PostUpdate,
            wormhole_camera_system
                .after(orbit_camera_system)
                .before(TransformSystem::TransformPropagate),
        );
        if !app.is_plugin_added::<bevy::render::RenderPlugin>() {
            return;
        }
        app.add_plugins(MaterialPlugin::<WormholeMaterial>::default());
        app.add_systems(
            Update,
            (
                sync_wormhole_bodies_system
                    .after(hide_hole_body_system)
                    .after(sync_mirror_entities_system),
                sync_wormhole_mouth_system,
            ),
        );
        app.add_systems(
            PostUpdate,
            sync_wormhole_view_system
                .after(wormhole_camera_system)
                .before(TransformSystem::TransformPropagate),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/render/wormhole_view_tests.rs"]
mod tests;
