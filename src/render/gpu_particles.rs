//! Drawing a scene of particles straight from the GPU physics buffer.
//!
//! While the physics keeps a scene's particles on the GPU ([`GpuParticles`]), they are drawn from
//! that same buffer: one instanced draw of a low-polygon sphere, one instance per particle, with no
//! entity per particle and nothing copied back to the CPU to be drawn. The physics' `drift` kernel
//! writes each particle's position in world units into the record, so the vertex shader
//! (`assets/shaders/gpu_particles.wgsl`) needs only the camera.
//!
//! The plumbing follows Bevy 0.14's `shader_instancing` example: an entity carrying the mesh and a
//! [`GpuParticleCloud`] (the buffer and how many particles it holds) is extracted to the render
//! world, queued into the transparent 3D phase, and drawn by `DrawGpuParticles` with the particle
//! buffer as its per-instance vertex buffer.
//!
//! When the GPU is lost the cloud draws nothing, and the mirror systems in [`super::plugin`] spawn
//! the usual entity per particle for the CPU fallback.

use bevy::{
    core_pipeline::core_3d::Transparent3d,
    ecs::{
        query::QueryItem,
        system::{lifetimeless::*, SystemParamItem},
    },
    pbr::{
        MeshPipeline, MeshPipelineKey, RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
    },
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        mesh::{GpuBufferInfo, GpuMesh, MeshVertexBufferLayoutRef},
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
        },
        render_resource::{
            Buffer, PipelineCache, RenderPipelineDescriptor, SpecializedMeshPipeline,
            SpecializedMeshPipelineError, SpecializedMeshPipelines, VertexAttribute,
            VertexBufferLayout, VertexFormat, VertexStepMode,
        },
        view::{ExtractedView, NoFrustumCulling},
        Render, RenderApp, RenderSet,
    },
};

use bevy::math::DVec3;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};

use crate::collision::aggregate::contact_clusters;
use crate::compute::particles::{can_step_on_gpu, GpuParticles};
use crate::core::scene::{Body, BodyId, Prop, Scene};
use crate::input::camera::CameraTarget;
use crate::render::constants::{
    CLUSTER_TARGET_MAX, CLUSTER_TARGET_MIN_PARTICLES, GPU_PARTICLES_SHADER_PATH,
    PARTICLE_MESH_SUBDIVISIONS,
};

/// Bytes per particle in the physics buffer: `FLOATS_PER_PARTICLE` (`32`) floats. Must match
/// `compute::particles`.
const PARTICLE_STRIDE_BYTES: u64 = 128; // ALGORITHM CONSTANT: the physics buffer's layout
/// Where in a particle record the renderer reads, bytes: position and radius (metres) at the start,
/// the colour code at float `26`, the world x at `27`, the world y and z at `30`. Must match
/// `particles.wgsl`'s `Particle`.
const RADIUS_OFFSET_BYTES: u64 = 0; // ALGORITHM CONSTANT: the physics buffer's layout
const COLOUR_OFFSET_BYTES: u64 = 26 * 4; // ALGORITHM CONSTANT: the physics buffer's layout
const WORLD_X_OFFSET_BYTES: u64 = 27 * 4; // ALGORITHM CONSTANT: the physics buffer's layout
const WORLD_YZ_OFFSET_BYTES: u64 = 30 * 4; // ALGORITHM CONSTANT: the physics buffer's layout

/// Whether the GPU draws `scene`'s particles itself: a GPU stepper that has not failed, and a scene
/// it can step. The mirror systems then spawn no entity for a particle.
pub fn gpu_draws_particles(gpu: Option<&GpuParticles>, scene: &Scene) -> bool {
    gpu.is_some_and(|gpu| !gpu.has_failed()) && can_step_on_gpu(scene)
}

/// Whether `prop` is one of the particles [`gpu_draws_particles`] covers.
pub fn is_gpu_drawn(prop: &Prop) -> bool {
    prop.material.is_particle() && !prop.is_luminous()
}

/// The particles to draw: the physics buffer and how many it holds. On the one entity that carries
/// the particle mesh; a `count` of zero draws nothing.
#[derive(Component, Clone, Default)]
pub struct GpuParticleCloud {
    /// The physics' particle buffer, when a scene is held on the GPU.
    pub buffer: Option<Buffer>,
    /// How many particles it holds.
    pub count: u32,
}

impl ExtractComponent for GpuParticleCloud {
    type QueryData = &'static GpuParticleCloud;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}

/// Draws a scene of particles from the GPU physics buffer. Added by [`super::plugin::ScenePlugin`]
/// when Bevy renders.
pub struct GpuParticleRenderPlugin;

impl Plugin for GpuParticleRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<GpuParticleCloud>::default())
            .add_systems(Startup, spawn_cloud)
            .add_systems(
                Update,
                (update_cloud, update_cluster_targets_system)
                    .after(super::plugin::step_scene_system),
            );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_render_command::<Transparent3d, DrawGpuParticles>()
                .init_resource::<SpecializedMeshPipelines<GpuParticlePipeline>>()
                .add_systems(Render, queue_cloud.in_set(RenderSet::QueueMeshes));
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<GpuParticlePipeline>();
        }
    }
}

/// The one entity the particles are drawn through: the particle mesh, never culled (its own bounds
/// say nothing about where the particles are).
fn spawn_cloud(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mesh = meshes.add(
        Sphere::new(1.0)
            .mesh()
            .ico(PARTICLE_MESH_SUBDIVISIONS)
            .unwrap_or_else(|_| Mesh::from(Sphere::new(1.0))),
    );
    commands.spawn((
        mesh,
        SpatialBundle::INHERITED_IDENTITY,
        GpuParticleCloud::default(),
        NoFrustumCulling,
    ));
}

/// Point the cloud at the physics buffer while the GPU holds the scene; draw nothing otherwise.
fn update_cloud(
    gpu: Option<Res<GpuParticles>>,
    scene: Res<Scene>,
    mut clouds: Query<&mut GpuParticleCloud>,
) {
    let held = gpu
        .as_deref()
        .filter(|gpu| gpu_draws_particles(Some(gpu), &scene) && gpu.holds(&scene))
        .and_then(GpuParticles::particle_buffer);
    for mut cloud in &mut clouds {
        match &held {
            Some((buffer, count, _)) => {
                let changed = cloud.count != *count
                    || cloud.buffer.as_ref().map(|b| b.id()) != Some(buffer.id());
                if changed {
                    cloud.buffer = Some(buffer.clone());
                    cloud.count = *count;
                }
            }
            None if cloud.count != 0 => {
                cloud.buffer = None;
                cloud.count = 0;
            }
            None => {}
        }
    }
}

/// The mesh pipeline, with the particle buffer as a second, per-instance vertex buffer and the
/// particle shader.
#[derive(Resource)]
struct GpuParticlePipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for GpuParticlePipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        Self {
            shader: world.load_asset(GPU_PARTICLES_SHADER_PATH),
            mesh_pipeline,
        }
    }
}

impl SpecializedMeshPipeline for GpuParticlePipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;
        descriptor.vertex.shader = self.shader.clone();
        // Locations 0–2 are the mesh's position, normal and uv.
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: PARTICLE_STRIDE_BYTES,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: RADIUS_OFFSET_BYTES,
                    shader_location: 3,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: COLOUR_OFFSET_BYTES,
                    shader_location: 4,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: WORLD_X_OFFSET_BYTES,
                    shader_location: 5,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: WORLD_YZ_OFFSET_BYTES,
                    shader_location: 6,
                },
            ],
        });
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = self.shader.clone();
        }
        Ok(descriptor)
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_cloud(
    draw_functions: Res<DrawFunctions<Transparent3d>>,
    pipeline: Res<GpuParticlePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<GpuParticlePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<GpuMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    clouds: Query<(Entity, &GpuParticleCloud)>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(Entity, &ExtractedView)>,
) {
    let draw = draw_functions.read().id::<DrawGpuParticles>();
    let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());
    for (view_entity, view) in &views {
        let Some(phase) = phases.get_mut(&view_entity) else {
            continue;
        };
        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        for (entity, cloud) in &clouds {
            if cloud.count == 0 || cloud.buffer.is_none() {
                continue;
            }
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(entity) else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());
            let Ok(id) = pipelines.specialize(&pipeline_cache, &pipeline, key, &mesh.layout) else {
                continue;
            };
            phase.add(Transparent3d {
                entity,
                pipeline: id,
                draw_function: draw,
                // One item for the whole cloud: it sorts as a whole against the scene's other
                // transparent items, which in a scene of particles there are none of.
                distance: 0.0,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::NONE,
            });
        }
    }
}

/// Bind the camera and the mesh, then draw the mesh once per particle from the physics buffer.
type DrawGpuParticles = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    DrawParticleInstances,
);

struct DrawParticleInstances;

impl<P: PhaseItem> RenderCommand<P> for DrawParticleInstances {
    type Param = (SRes<RenderAssets<GpuMesh>>, SRes<RenderMeshInstances>);
    type ViewQuery = ();
    type ItemQuery = Read<GpuParticleCloud>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        cloud: Option<&'w GpuParticleCloud>,
        (meshes, render_mesh_instances): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.entity())
        else {
            return RenderCommandResult::Failure;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Failure;
        };
        let Some((buffer, count)) = cloud.and_then(|c| c.buffer.as_ref().map(|b| (b, c.count)))
        else {
            return RenderCommandResult::Failure;
        };
        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, buffer.slice(..u64::from(count) * PARTICLE_STRIDE_BYTES));
        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count: indices,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                pass.draw_indexed(0..*indices, 0, 0..count);
            }
            GpuBufferInfo::NonIndexed => {
                pass.draw(0..gpu_mesh.vertex_count, 0..count);
            }
        }
        RenderCommandResult::Success
    }
}

/// One body of a scene of particles the GPU draws, as something the camera can lock onto with `F`.
///
/// There is no entity per particle while the GPU draws them, so each body (a contact cluster: a
/// planet, the merged body, a clump of debris) gets one invisible [`CameraTarget`] at its centre of
/// mass, scaled to its extent (the camera orbits no closer than that). Rebuilt from each copy of the
/// particles the CPU receives, and moved between copies at the body's velocity, so a locked camera
/// follows smoothly. Keyed by the body's lowest [`BodyId`], so a lock survives the rebuild; when
/// two bodies merge, the one whose key goes releases the lock, as an absorbed body does.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ClusterTarget {
    /// The lowest id among the body's particles.
    pub key: BodyId,
    /// Its centre of mass at `at`, world units.
    pub centre: Vec3,
    /// Its centre of mass's velocity, world units per simulated second.
    pub velocity: Vec3,
    /// When `centre` was, simulated seconds since the GPU took the scene.
    pub at: f64,
    /// The distance from its centre of mass to its outermost particle's far side, world units.
    pub radius: f32,
}

/// The bodies of `scene` worth locking onto: its contact clusters of at least
/// [`CLUSTER_TARGET_MIN_PARTICLES`], the [`CLUSTER_TARGET_MAX`] most massive, at simulated time `at`.
pub fn cluster_targets(scene: &Scene, at: f64) -> Vec<ClusterTarget> {
    let ids: Vec<BodyId> = scene.iter().map(|(id, _)| id).collect();
    let mut bodies: Vec<(f64, ClusterTarget)> = contact_clusters(&scene.props)
        .into_iter()
        .filter(|members| members.len() >= CLUSTER_TARGET_MIN_PARTICLES)
        .filter_map(|members| {
            let mass: f64 = members
                .iter()
                .map(|&i| f64::from(scene.props[i].mass_kg))
                .sum();
            if mass <= 0.0 {
                return None;
            }
            let weighted = |of: &dyn Fn(&Prop) -> DVec3| -> DVec3 {
                members
                    .iter()
                    .map(|&i| of(&scene.props[i]) * (f64::from(scene.props[i].mass_kg) / mass))
                    .sum()
            };
            let centre = weighted(&|prop: &Prop| prop.body.center());
            let velocity = weighted(&|prop: &Prop| prop.velocity_units_s);
            let radius = members
                .iter()
                .map(|&i| {
                    let prop = &scene.props[i];
                    let own = match prop.body {
                        Body::Sphere { radius, .. } => f64::from(radius),
                        _ => 0.0,
                    };
                    (prop.body.center() - centre).length() + own
                })
                .fold(0.0, f64::max);
            let key = members.iter().map(|&i| ids[i]).min()?;
            Some((
                mass,
                ClusterTarget {
                    key,
                    centre: centre.as_vec3(),
                    velocity: velocity.as_vec3(),
                    at,
                    radius: radius as f32,
                },
            ))
        })
        .collect();
    bodies.sort_by(|a, b| b.0.total_cmp(&a.0));
    bodies.truncate(CLUSTER_TARGET_MAX);
    bodies.into_iter().map(|(_, body)| body).collect()
}

/// The lock targets being worked out in the background, and what they were built from.
#[derive(Default)]
pub struct ClusterTargetWork {
    /// The `(scene revision, copies)` the targets in the world, or in `task`, were built from.
    built_from: Option<(u64, u64)>,
    /// Finding the contact clusters takes `31 ms` at `22,000` particles (measured, release), too
    /// long for a frame, so it runs on the async pool on a copy of the scene.
    task: Option<Task<Vec<ClusterTarget>>>,
}

/// Keep one lock target per body while the GPU draws the scene's particles (see [`ClusterTarget`]),
/// and none otherwise: the mirror entities are the targets then.
pub fn update_cluster_targets_system(
    mut commands: Commands,
    gpu: Option<Res<GpuParticles>>,
    scene: Res<Scene>,
    mut targets: Query<(Entity, &mut ClusterTarget, &mut Transform)>,
    mut work: Local<ClusterTargetWork>,
) {
    let held = gpu
        .as_deref()
        .filter(|gpu| gpu_draws_particles(Some(gpu), &scene) && gpu.holds(&scene));
    let Some((gpu, (syncs, synced_at))) = held.and_then(|gpu| Some((gpu, gpu.last_sync()?))) else {
        for (entity, ..) in &targets {
            commands.entity(entity).despawn();
        }
        *work = ClusterTargetWork::default();
        return;
    };
    // Rebuilt when the GPU took a new scene (the scene is then the upload itself) or a new copy of
    // the particles has arrived. A rebuild still running when the next copy comes finishes first.
    let stamp = (scene.revision(), syncs);
    if work.built_from != Some(stamp) && work.task.is_none() {
        work.built_from = Some(stamp);
        let copy = scene.clone();
        work.task = Some(
            AsyncComputeTaskPool::get().spawn(async move { cluster_targets(&copy, synced_at) }),
        );
    }
    if let Some(mut task) = work.task.take() {
        match block_on(future::poll_once(&mut task)) {
            Some(mut fresh) => {
                for (entity, mut target, _) in &mut targets {
                    match fresh.iter().position(|body| body.key == target.key) {
                        Some(at) => *target = fresh.swap_remove(at),
                        None => commands.entity(entity).despawn(),
                    }
                }
                for body in fresh {
                    commands.spawn((
                        body,
                        CameraTarget,
                        TransformBundle::from_transform(
                            Transform::from_translation(body.centre)
                                .with_scale(Vec3::splat(body.radius)),
                        ),
                    ));
                }
            }
            None => work.task = Some(task),
        }
    }
    let now = gpu.simulated_seconds().unwrap_or(synced_at);
    for (_, target, mut transform) in &mut targets {
        transform.translation = target.centre + target.velocity * (now - target.at) as f32;
        transform.scale = Vec3::splat(target.radius);
    }
}

#[cfg(test)]
#[path = "../../tst/render/gpu_particles_tests.rs"]
mod tests;
