//! The black hole drawn on the GPU: every pixel's light followed along its exact Kerr path.
//!
//! A sphere centred on the hole carries [`BlackHoleMaterial`], whose fragment shader
//! (`assets/shaders/black_hole.wgsl`) follows the light reaching the camera through each pixel
//! backwards until it reaches the disc, falls into the hole, or comes from the sky. The shader is a
//! transcription of [`crate::render::kerr_ray`], its Rust twin, which is where the physics is
//! tested.
//!
//! # How the sphere is drawn
//!
//! Only its **back faces** are drawn, so every pixel that looks through the sphere gets exactly one
//! fragment. The sphere is at least [`BLACK_HOLE_VIEW_SPHERE_RG`] gravitational radii across, and
//! grows with the camera's distance so that it always holds the camera ([`sphere_radius_rg`]):
//! every pixel on the screen then looks through it, and the whole sky is lensed. The fragment's
//! position gives the pixel's direction, and the ray starts at the camera.
//!
//! # What it shows
//!
//! - **The shadow**, black.
//! - **The disc, as simulated.** Every frame the clumps are splatted into a gas texture
//!   ([`splat_gas`]) over the turn round the hole and the radius, each as a strip along its streak,
//!   and the shader reads how much of the plane the gas covers wherever the light crosses it. The
//!   gaps let the light through to the images behind ([`crate::render::kerr_ray::composite`]).
//! - **Its light, as the camera sees it** ([`crate::render::kerr_ray::disc_colour`]): a blackbody
//!   at `g T`, with `g` from the light's own momentum where it left the gas and the gas's motion,
//!   orbiting or plunging, and an intensity `g⁴` times the disc's flux there. That spans orders of
//!   magnitude, so the picture is exposed and tone-mapped like a photograph
//!   ([`crate::render::kerr_ray::tone_map`], [`BLACK_HOLE_EXPOSURE`](crate::render::constants::BLACK_HOLE_EXPOSURE)).
//! - **The stars**, scattered over the whole sky and seen through the lens
//!   ([`crate::render::kerr_ray::star_field`]): crowded towards the shadow's edge, and stretched
//!   into arcs round the hole within its Einstein ring.
//!
//! [`Action::ToggleBlackHoleView`] (`V`) switches the disc to a flat test pattern (rings every
//! `2 r_g`, spokes every `30°`, tinted by image order) that makes the lens's distortion easy to
//! read, and back. [`Action::ToggleStars`] (`N`) turns the stars off and on, and
//! [`Action::ToggleOutline`] (`O`) outlines the event horizon itself ([`BlackHoleOutline`], off by
//! default), to show where and how big the hole really is.
//!
//! # Depth
//!
//! The shader writes each pixel's depth itself: the depth of the first gas the light passed through,
//! of the hole's centre for the shadow, and the very back of the view for the sky. So bodies in
//! front of the disc hide it, and those beyond it (the S-stars, on the far side of their orbits)
//! show against the sky instead of being covered by it.
//!
//! # Settings
//!
//! [`PerformanceSettings`] carries the ray-marcher's quality (its step), the stars, and the
//! fallback: with the ray-marcher off, the hole is drawn as its plain body, a black ball.
//!
//! # Limits
//!
//! The rest of the scene is not lensed, and the sky is opaque: a body placed beyond the sphere, on
//! the far side of the hole, is hidden by the stars.

use bevy::app::{App, Plugin, Update};
use bevy::asset::{Asset, Assets, Handle};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{With, Without};
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, Resource};
use bevy::input::{keyboard::KeyCode, ButtonInput};
use bevy::math::{primitives::Sphere, UVec4, Vec3, Vec4};
use bevy::pbr::{
    Material, MaterialMeshBundle, MaterialPipeline, MaterialPipelineKey, MaterialPlugin,
    NotShadowCaster,
};
use bevy::reflect::TypePath;
use bevy::render::alpha::AlphaMode;
use bevy::render::mesh::{Mesh, MeshVertexBufferLayoutRef, Meshable};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, Face, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::render::texture::{
    Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor,
};
use bevy::render::view::Visibility;
use bevy::transform::components::{GlobalTransform, Transform};

use crate::bindings::{keymap_or_default, Action, KeyMap};
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{BodyId, Scene};
use crate::input::menu::MenuState;
use crate::performance::PerformanceSettings;
use crate::physics_utils::relativity::kerr::KerrBlackHole;
use crate::render::accretion_disc::{
    advance_accretion_disc_system, disc_source, AccretionDisc, DiscParticle, DiscProfile,
};
use crate::render::constants::{
    BLACK_HOLE_SHADER_PATH, BLACK_HOLE_VIEW_CAMERA_MARGIN, BLACK_HOLE_VIEW_SPHERE_RG,
    CAMERA_FAR_UNITS, DISC_CLUMP_WIDTH_RG, DISC_GAS_TEXTURE_HEIGHT, DISC_GAS_TEXTURE_WIDTH,
};
use crate::render::kerr_ray::{texture_coordinates, texture_radius, DiscShading, RayHole};
use crate::render::plugin::MirroredBody;

pub use uniform::BlackHoleUniform;

/// Wrapper module for [`BlackHoleUniform`], for the same `dead_code` reason as
/// [`EclipseUniform`](crate::render::eclipse::EclipseUniform)'s: `#[derive(ShaderType)]` emits
/// layout checks nothing calls.
mod uniform {
    #![allow(dead_code)]

    use bevy::math::{UVec4, Vec4};
    use bevy::render::render_resource::ShaderType;

    use crate::render::constants::KERR_RAY_PROFILE_SAMPLES;

    /// The hole, its disc, and the march's settings, as the shader reads them. All `vec4`s, so the
    /// layout is the same on every driver.
    #[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
    pub struct BlackHoleUniform {
        /// `xyz`: the hole's centre in world units. `w`: world units per `r_g`.
        pub centre: Vec4,
        /// Spin, horizon, trapped radius (the prograde photon orbit), ISCO; radii in `r_g`.
        pub hole: Vec4,
        /// `x`: the disc's outer edge. `y`: the step fraction. `z`: the sphere's radius. In `r_g`.
        pub disc: Vec4,
        /// `x`: the most steps per ray. `y`: the most plane crossings. `z`: what to draw, `0` for
        /// the shaded disc and `1` for the test pattern. `w`: `1` for stars on the sky.
        pub limits: UVec4,
        /// `x`: the exposure. `z`, `w`: the energy and angular momentum of the ISCO orbit, which
        /// plunging gas keeps.
        pub shading: Vec4,
        /// The disc's temperature (`x`, kelvin) and relative flux (`y`), from the ISCO to the outer
        /// edge, uniformly in `√(r − r_ISCO)`.
        pub profile: [Vec4; KERR_RAY_PROFILE_SAMPLES],
    }

    impl Default for BlackHoleUniform {
        fn default() -> Self {
            BlackHoleUniform {
                centre: Vec4::ZERO,
                hole: Vec4::ZERO,
                disc: Vec4::ZERO,
                limits: UVec4::ZERO,
                shading: Vec4::ZERO,
                profile: [Vec4::ZERO; KERR_RAY_PROFILE_SAMPLES],
            }
        }
    }
}

impl BlackHoleUniform {
    /// The uniform for the hole and disc of `profile`, at world position `centre`, drawn as `view`
    /// with `settings`' quality and stars, on a sphere `sphere_rg` gravitational radii across, with
    /// the event horizon outlined when `outline` is on.
    ///
    /// # Returns
    ///
    /// `None` for an unusable hole.
    pub fn new(
        profile: &DiscProfile,
        centre: Vec3,
        view: BlackHoleView,
        settings: &PerformanceSettings,
        sphere_rg: f32,
        outline: BlackHoleOutline,
    ) -> Option<Self> {
        let mut hole = RayHole::new(profile.spin, profile.outer_rg)?;
        hole.step_fraction = settings.black_hole_quality.step_fraction();
        hole.max_steps = settings.black_hole_quality.max_steps();
        let units_per_rg = KerrBlackHole::new(profile.mass_kg, profile.spin)?
            .gravitational_radius_m()
            / METRES_PER_UNIT;
        let shading = DiscShading::new(&hole, profile);
        let mode = u32::from(view == BlackHoleView::Pattern);
        let stars = u32::from(settings.black_hole_stars);
        Some(BlackHoleUniform {
            centre: centre.extend(units_per_rg as f32),
            hole: Vec4::new(hole.spin, hole.horizon, hole.trapped, hole.isco),
            disc: Vec4::new(hole.outer, hole.step_fraction, sphere_rg, 0.0),
            limits: UVec4::new(hole.max_steps, hole.max_crossings, mode, stars),
            shading: Vec4::new(
                shading.exposure,
                f32::from(u8::from(outline.0)),
                hole.isco_energy,
                hole.isco_angular_momentum,
            ),
            profile: shading.profile.map(|sample| sample.extend(0.0).extend(0.0)),
        })
    }

    /// World units per `r_g`.
    pub fn units_per_rg(&self) -> f32 {
        self.centre.w
    }

    /// The hole as the twin sees it, rebuilt from the uniform's numbers.
    pub fn ray_hole(&self) -> RayHole {
        RayHole {
            spin: self.hole.x,
            horizon: self.hole.y,
            trapped: self.hole.z,
            isco: self.hole.w,
            outer: self.disc.x,
            step_fraction: self.disc.y,
            max_steps: self.limits.x,
            max_crossings: self.limits.y,
            isco_energy: self.shading.z,
            isco_angular_momentum: self.shading.w,
        }
    }
}

/// The material that ray-marches the hole: [`BlackHoleUniform`] at `@group(2) @binding(0)`, and
/// the gas texture at bindings `1` (texture) and `2` (sampler).
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct BlackHoleMaterial {
    /// Rewritten when the hole or the camera moves ([`sync_black_hole_view_system`]).
    #[uniform(0)]
    pub uniform: BlackHoleUniform,
    /// How much of the plane the gas covers, over (turn round the hole, radius): [`splat_gas`],
    /// rewritten every frame by [`update_gas_texture_system`].
    #[texture(1)]
    #[sampler(2)]
    pub gas: Handle<Image>,
}

impl Material for BlackHoleMaterial {
    fn fragment_shader() -> ShaderRef {
        BLACK_HOLE_SHADER_PATH.into()
    }

    /// Opaque while the sky has stars: every pixel is then covered, and an opaque material writes
    /// the depth the shader gives it, so trails drawn later are hidden behind the
    /// disc and the shadow where they pass behind them. Blended with the stars off, so the sky
    /// pixels can be discarded and the gaps in the gas show the scene's background.
    fn alpha_mode(&self) -> AlphaMode {
        if self.uniform.limits.w == 1 {
            AlphaMode::Opaque
        } else {
            AlphaMode::Blend
        }
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

/// How much of each texel of the gas texture the simulation's clumps cover, as bytes (`0` none,
/// `255` all), [`DISC_GAS_TEXTURE_WIDTH`] × [`DISC_GAS_TEXTURE_HEIGHT`], rows by radius.
///
/// Each clump is a streak along its recent path ([`DiscParticle::streak_coordinates`]), here a
/// strip [`DISC_CLUMP_WIDTH_RG`] wide. Each streak is cut into pieces no longer than half a texel,
/// and each piece's area is shared between the four nearest texel centres (wrapping round the hole)
/// as a fraction of each texel's own area in the plane. Strips overlap at random, so the fraction
/// `τ` of a texel they cover adds up as an optical depth, and what is seen through is `e^{−τ}`: the
/// coverage is `1 − e^{−τ}`.
///
/// The clumps are shared between up to eight threads, each splatting into its own buffer; the
/// buffers are added in order, so every run gives the same bytes.
pub fn splat_gas(hole: &RayHole, profile: &DiscProfile, particles: &[DiscParticle]) -> Vec<u8> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |p| p.get())
        .clamp(1, 8);
    let chunk = particles.len().div_ceil(threads).max(1);
    let depth = std::thread::scope(|scope| {
        let handles: Vec<_> = particles
            .chunks(chunk)
            .map(|share| scope.spawn(move || splat_depth(hole, profile, share)))
            .collect();
        let mut total = vec![0.0f32; DISC_GAS_TEXTURE_WIDTH * DISC_GAS_TEXTURE_HEIGHT];
        for handle in handles {
            if let Ok(part) = handle.join() {
                for (sum, value) in total.iter_mut().zip(part) {
                    *sum += value;
                }
            }
        }
        total
    });
    depth
        .into_iter()
        .map(|tau| ((1.0 - (-tau).exp()) * 255.0).round() as u8)
        .collect()
}

/// The optical depth `τ` the clumps of `particles` add to each texel ([`splat_gas`]).
fn splat_depth(hole: &RayHole, profile: &DiscProfile, particles: &[DiscParticle]) -> Vec<f32> {
    let (width, height) = (DISC_GAS_TEXTURE_WIDTH, DISC_GAS_TEXTURE_HEIGHT);
    // Each row's texel area, in r_g²: r dr dφ, with r = r₊ + span y².
    let span = hole.outer - hole.horizon;
    let area: Vec<f32> = (0..height)
        .map(|j| {
            let y = (j as f32 + 0.5) / height as f32;
            let r = texture_radius(hole, y);
            let dr = 2.0 * span * y / height as f32;
            (r * dr * std::f32::consts::TAU / width as f32).max(1.0e-6)
        })
        .collect();
    let mut depth = vec![0.0f32; width * height];
    let mut deposit = |r: f32, phi: f32, covered: f32| {
        let uv = texture_coordinates(hole, r, phi);
        let fx = uv.x * width as f32 - 0.5;
        let fy = (uv.y * height as f32 - 0.5).clamp(0.0, (height - 1) as f32);
        let (i0, j0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - i0, fy - j0);
        let i0 = (i0 as i64).rem_euclid(width as i64) as usize;
        let i1 = (i0 + 1) % width;
        let j0 = j0 as usize;
        let j1 = (j0 + 1).min(height - 1);
        for (i, j, w) in [
            (i0, j0, (1.0 - tx) * (1.0 - ty)),
            (i1, j0, tx * (1.0 - ty)),
            (i0, j1, (1.0 - tx) * ty),
            (i1, j1, tx * ty),
        ] {
            depth[j * width + i] += covered * w / area[j];
        }
    };

    for particle in particles {
        let points: Vec<(f32, f32)> = particle
            .streak_coordinates(profile)
            .into_iter()
            .map(|(r, phi)| (r as f32, phi as f32))
            .collect();
        if let [(r, phi)] = points[..] {
            deposit(r, phi, DISC_CLUMP_WIDTH_RG * DISC_CLUMP_WIDTH_RG);
            continue;
        }
        for pair in points.windows(2) {
            let ((r0, phi0), (r1, phi1)) = (pair[0], pair[1]);
            // The short way round.
            let turn = (phi1 - phi0 + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let r_mid = 0.5 * (r0 + r1);
            let length = (r1 - r0).hypot(r_mid * turn);
            let (a, b) = (
                texture_coordinates(hole, r0, phi0),
                texture_coordinates(hole, r1, phi1),
            );
            let texels = (turn.abs() / std::f32::consts::TAU * width as f32)
                .max((b.y - a.y).abs() * height as f32);
            let pieces = ((2.0 * texels).ceil() as usize).clamp(1, 256);
            for k in 0..pieces {
                let t = (k as f32 + 0.5) / pieces as f32;
                deposit(
                    r0 + t * (r1 - r0),
                    phi0 + t * turn,
                    DISC_CLUMP_WIDTH_RG * length / pieces as f32,
                );
            }
        }
    }
    depth
}

/// A gas texture holding `data` ([`splat_gas`]): one byte a texel, filtered linearly, wrapping
/// round the hole and clamped at the horizon and the edge.
pub fn gas_image(data: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: DISC_GAS_TEXTURE_WIDTH as u32,
            height: DISC_GAS_TEXTURE_HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
    image
}

/// How the disc is drawn. `V` switches between them.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlackHoleView {
    /// The simulated gas, shaded by its temperature and redshift.
    #[default]
    Shaded,
    /// A flat test pattern on the disc, for reading the lens's distortion.
    Pattern,
}

impl BlackHoleView {
    /// The view `V` switches to.
    pub fn next(self) -> BlackHoleView {
        match self {
            BlackHoleView::Shaded => BlackHoleView::Pattern,
            BlackHoleView::Pattern => BlackHoleView::Shaded,
        }
    }
}

/// Whether the hole's event horizon is outlined, in
/// [`BLACK_HOLE_OUTLINE_COLOUR`](crate::render::constants::BLACK_HOLE_OUTLINE_COLOUR), so the hole
/// itself can be picked out. Off by default; [`Action::ToggleOutline`] (`O`) flips it.
///
/// The outline is the horizon where it really is, its oblate spheroid seen along straight sight
/// lines ([`crate::render::kerr_ray::horizon_measure`]), drawn over everything else. Light bends, so
/// the horizon is *seen* filling the whole shadow round it, about `2.5` times wider; the outline
/// shows the hole's true size and place within it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlackHoleOutline(pub bool);

/// Marks the sphere the hole is ray-marched on, with the uniform it was built from.
#[derive(Component, Clone, Copy, Debug)]
pub struct BlackHoleSphere {
    /// What its material holds.
    pub uniform: BlackHoleUniform,
}

/// How big the ray-marched sphere must be, in `r_g`, for a camera `camera_rg` gravitational radii
/// from the hole: at least [`BLACK_HOLE_VIEW_SPHERE_RG`], and big enough to hold the camera with
/// room to spare ([`BLACK_HOLE_VIEW_CAMERA_MARGIN`]), so every pixel looks through it and the whole
/// sky is lensed; but inside the camera's far plane, `far_rg`.
pub fn sphere_radius_rg(camera_rg: f32, far_rg: f32) -> f32 {
    (BLACK_HOLE_VIEW_SPHERE_RG.max(BLACK_HOLE_VIEW_CAMERA_MARGIN * camera_rg)).min(0.9 * far_rg)
}

/// Whether the hole is ray-marched: the ray-marcher is on in `settings` (no settings resource means
/// the shipped default). Off is the fallback, the hole's plain body.
pub fn ray_marched(settings: Option<&PerformanceSettings>) -> bool {
    settings.map_or(PerformanceSettings::default().black_hole, |s| s.black_hole)
}

/// Bevy system that keeps one ray-marched sphere on the scene's spinning black hole: spawned when
/// there is one and its disc has a profile, moved, resized round the camera ([`sphere_radius_rg`]),
/// and updated with it, rebuilt if the hole changes size, hidden while the ray-marcher is off
/// ([`ray_marched`]), and removed when there is no hole.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn sync_black_hole_view_system(
    mut commands: Commands,
    scene: Res<Scene>,
    view: Res<BlackHoleView>,
    outline: Option<Res<BlackHoleOutline>>,
    settings: Option<Res<PerformanceSettings>>,
    disc: Res<AccretionDisc>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BlackHoleMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut spheres: Query<(
        Entity,
        &mut BlackHoleSphere,
        &Handle<BlackHoleMaterial>,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let settings = settings.map_or_else(PerformanceSettings::default, |s| *s);
    let outline = outline.map_or_else(BlackHoleOutline::default, |o| *o);
    let wanted = disc_source(&scene).and_then(|(mass_kg, spin, centre)| {
        let profile = disc.profile()?;
        if profile.mass_kg != mass_kg || profile.spin != spin {
            return None;
        }
        let units_per_rg =
            (KerrBlackHole::new(mass_kg, spin)?.gravitational_radius_m() / METRES_PER_UNIT) as f32;
        let camera = cameras
            .iter()
            .next()
            .map_or(0.0, |camera| camera.translation().distance(centre));
        let sphere_rg = sphere_radius_rg(camera / units_per_rg, CAMERA_FAR_UNITS / units_per_rg);
        BlackHoleUniform::new(profile, centre, *view, &settings, sphere_rg, outline)
    });
    let visibility = if ray_marched(Some(&settings)) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let mut kept = false;
    for (entity, mut sphere, handle, mut transform, mut shown) in &mut spheres {
        match wanted {
            Some(uniform) if !kept && sphere.uniform.units_per_rg() == uniform.units_per_rg() => {
                kept = true;
                if sphere.uniform != uniform {
                    sphere.uniform = uniform;
                    *transform = sphere_transform(&uniform);
                    if let Some(material) = materials.get_mut(handle) {
                        material.uniform = uniform;
                    }
                }
                if *shown != visibility {
                    *shown = visibility;
                }
            }
            _ => commands.entity(entity).despawn(),
        }
    }

    if let (Some(uniform), false) = (wanted, kept) {
        commands.spawn((
            MaterialMeshBundle {
                mesh: meshes.add(Sphere::new(1.0).mesh().uv(64, 32)),
                material: materials.add(BlackHoleMaterial {
                    uniform,
                    gas: images.add(gas_image(vec![
                        0;
                        DISC_GAS_TEXTURE_WIDTH
                            * DISC_GAS_TEXTURE_HEIGHT
                    ])),
                }),
                transform: sphere_transform(&uniform),
                visibility,
                ..Default::default()
            },
            NotShadowCaster,
            BlackHoleSphere { uniform },
        ));
    }
}

/// Where the unit sphere goes for `uniform`: on the hole, scaled to the sphere's radius.
pub fn sphere_transform(uniform: &BlackHoleUniform) -> Transform {
    Transform::from_translation(uniform.centre.truncate())
        .with_scale(Vec3::splat(uniform.disc.z * uniform.units_per_rg()))
}

/// Bevy system that splats the simulation's clumps into the gas texture every frame
/// ([`splat_gas`]) while the hole is ray-marched, so the ray-marched disc is the simulated one.
pub fn update_gas_texture_system(
    settings: Option<Res<PerformanceSettings>>,
    disc: Res<AccretionDisc>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BlackHoleMaterial>>,
    spheres: Query<(&BlackHoleSphere, &Handle<BlackHoleMaterial>)>,
) {
    if !ray_marched(settings.as_deref()) {
        return;
    }
    let Some(profile) = disc.profile() else {
        return;
    };
    for (sphere, handle) in &spheres {
        let data = splat_gas(&sphere.uniform.ray_hole(), profile, disc.particles());
        let Some(material) = materials.get_mut(handle) else {
            continue;
        };
        // Touching the material too makes Bevy rebuild its bind group with the new texture.
        if let Some(image) = images.get_mut(&material.gas) {
            image.data = data;
        }
    }
}

/// The body the disc forms around: the heaviest absorbing prop, as
/// [`stream_source`](crate::render::light_streams::stream_source) picks it, in a scene with a disc.
pub fn hole_body(scene: &Scene) -> Option<BodyId> {
    disc_source(scene)?;
    scene
        .iter()
        .filter(|(_, prop)| prop.absorbs && prop.mass_kg > 0.0)
        .max_by(|(_, a), (_, b)| a.mass_kg.total_cmp(&b.mass_kg))
        .map(|(id, _)| id)
}

/// Bevy system that hides the hole's own body while the hole is ray-marched. Its mesh is an opaque
/// ball that would cover the near side of the ray-marched disc; the shader draws the true shadow
/// in its place. With the ray-marcher off it is shown, as the fallback.
pub fn hide_hole_body_system(
    scene: Res<Scene>,
    settings: Option<Res<PerformanceSettings>>,
    mut bodies: Query<(&MirroredBody, &mut Visibility), Without<BlackHoleSphere>>,
) {
    let hole = hole_body(&scene);
    let marched = ray_marched(settings.as_deref());
    for (body, mut visibility) in &mut bodies {
        let wanted = if marched && Some(body.0) == hole {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Bevy system that switches [`BlackHoleView`] on [`Action::ToggleBlackHoleView`], except while the
/// menu is open.
pub fn toggle_black_hole_view_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    mut view: ResMut<BlackHoleView>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::ToggleBlackHoleView) {
        *view = view.next();
    }
}

/// Bevy system that shows or hides the stars round the black hole on [`Action::ToggleStars`],
/// except while the menu is open. It flips [`PerformanceSettings::black_hole_stars`], the same
/// switch as the menu's, so the two always agree; with the stars off, the sky is the scene's
/// background.
pub fn toggle_stars_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    settings: Option<ResMut<PerformanceSettings>>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }
    let Some(mut settings) = settings else {
        return;
    };
    if keymap_or_default(&keymap).just_pressed(&keys, Action::ToggleStars) {
        settings.black_hole_stars = !settings.black_hole_stars;
    }
}

/// Bevy system that shows or hides the outline of the hole's event horizon ([`BlackHoleOutline`]) on
/// [`Action::ToggleOutline`], except while the menu is open.
pub fn toggle_outline_system(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Option<Res<KeyMap>>,
    menu: Option<Res<MenuState>>,
    mut outline: ResMut<BlackHoleOutline>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }
    if keymap_or_default(&keymap).just_pressed(&keys, Action::ToggleOutline) {
        outline.0 = !outline.0;
    }
}

/// Draws the scene's spinning black hole by ray-marching it on the GPU.
///
/// Add it after [`AccretionDiscPlugin`](super::accretion_disc::AccretionDiscPlugin), whose disc it
/// draws. Needs the render app; without one (headless) it only adds [`BlackHoleView`] and
/// [`BlackHoleOutline`].
pub struct BlackHoleViewPlugin;

impl Plugin for BlackHoleViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BlackHoleView>();
        app.init_resource::<BlackHoleOutline>();
        if !app.is_plugin_added::<bevy::render::RenderPlugin>() {
            return;
        }
        app.add_plugins(MaterialPlugin::<BlackHoleMaterial>::default());
        app.add_systems(
            Update,
            (
                toggle_black_hole_view_system,
                toggle_stars_system,
                toggle_outline_system,
                sync_black_hole_view_system,
                hide_hole_body_system,
                update_gas_texture_system,
            )
                .chain()
                .after(advance_accretion_disc_system),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/render/black_hole_view_tests.rs"]
mod tests;
