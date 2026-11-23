//! Tests for [`crate::render::gpu_particles`], `#[path]`-included into it.
//!
//! The draw itself needs a window and cannot run headless; these check which scenes the GPU draws
//! and that the shader's constants match the physics buffer's layout.

use super::*;
use crate::core::scene::{CollisionSettings, PlanetModel};
use crate::projects::interstellar::giant_impact::giant_impact_with;

#[test]
fn without_a_gpu_stepper_the_mirror_entities_draw_the_particles() {
    let scene = giant_impact_with(&CollisionSettings {
        particles_per_body: 50,
        planet_model: PlanetModel::Rubble,
        ..CollisionSettings::default()
    })
    .scene;
    assert!(!gpu_draws_particles(None, &scene));
    assert!(scene.props.iter().all(is_gpu_drawn));
}

#[test]
fn the_record_offsets_match_the_physics_layout() {
    // Floats 26 (colour), 27 (world x) and 30–31 (world y, z) of a 32-float record.
    assert_eq!(PARTICLE_STRIDE_BYTES, 32 * 4);
    assert_eq!(COLOUR_OFFSET_BYTES, 26 * 4);
    assert_eq!(WORLD_X_OFFSET_BYTES, 27 * 4);
    assert_eq!(WORLD_YZ_OFFSET_BYTES, 30 * 4);
    let shader = std::fs::read_to_string("assets/shaders/gpu_particles.wgsl").unwrap();
    assert!(shader.contains("const UNITS_PER_METRE: f32 = 1.0e-9;"));
    assert_eq!(crate::constants::METRES_PER_UNIT, 1.0e9);
}

#[test]
fn each_planet_of_a_particle_scene_is_one_lock_target() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    let scene = moon_forming_impact_with(&CollisionSettings {
        particles_per_body: 200,
        planet_model: PlanetModel::Fluid,
        ..CollisionSettings::default()
    })
    .scene;
    let targets = cluster_targets(&scene, 12.0);
    assert_eq!(targets.len(), 2, "{targets:?}");
    // The most massive first: the proto-Earth, keyed by the first id it was given.
    let first_id = scene.iter().map(|(id, _)| id).min().unwrap();
    assert_eq!(targets[0].key, first_id);
    for target in &targets {
        assert_eq!(target.at, 12.0);
        // A planet of particles is thousands of kilometres across.
        let radius_km = f64::from(target.radius) * crate::constants::METRES_PER_UNIT / 1e3;
        assert!((2_000.0..10_000.0).contains(&radius_km), "{radius_km} km");
    }
    let apart = (targets[0].centre - targets[1].centre).length();
    assert!(
        apart > targets[0].radius + targets[1].radius,
        "the planets start apart"
    );
}

/// How long building the lock targets takes at the detail of a big run (it happens at each copy of
/// the particles, every `GPU_READBACK_INTERVAL_FRAMES` frames). Run with
/// `cargo test --release -- --ignored measure_cluster_targets --nocapture`.
#[test]
#[ignore = "measurement"]
fn measure_cluster_targets() {
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    let scene = moon_forming_impact_with(&CollisionSettings {
        particles_per_body: 20_000,
        planet_model: PlanetModel::Fluid,
        ..CollisionSettings::default()
    })
    .scene;
    let start = std::time::Instant::now();
    let targets = cluster_targets(&scene, 0.0);
    eprintln!(
        "MEASURE cluster targets at {} bodies: {} targets in {:.1} ms",
        scene.props.len(),
        targets.len(),
        start.elapsed().as_secs_f64() * 1e3
    );
}

/// The plugin itself, in a headless app, puts a lock target on each planet of a scene the GPU holds:
/// the `F` key has something to lock onto. (A test of the system's registration, which a
/// formatting-sensitive edit once silently dropped.)
#[test]
fn the_plugin_gives_a_particle_scene_lock_targets() {
    use crate::compute::particles::GpuParticles;
    use crate::core::scene::{CollisionSettings, PlanetModel};
    use crate::projects::interstellar::moon_forming::moon_forming_impact_with;
    let mut gpu = GpuParticles::request().expect("a GPU");
    let scene = moon_forming_impact_with(&CollisionSettings {
        particles_per_body: 200,
        planet_model: PlanetModel::Fluid,
        debris_fragments: 0,
    })
    .scene;
    assert!(gpu.upload(&scene));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .insert_resource(scene)
        .insert_resource(gpu)
        .add_plugins(GpuParticleRenderPlugin);
    let mut found = 0;
    for _ in 0..200 {
        app.update();
        found = app
            .world_mut()
            .query::<(&ClusterTarget, &CameraTarget)>()
            .iter(app.world())
            .count();
        if found > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(found, 2, "one lock target a planet");
}
