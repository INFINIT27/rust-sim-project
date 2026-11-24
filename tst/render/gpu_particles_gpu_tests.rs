//! Tests for [`crate::render::gpu_particles`] that open a GPU, `#[path]`-included into it when the
//! `gpu-tests` feature is on.

use super::*;

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
