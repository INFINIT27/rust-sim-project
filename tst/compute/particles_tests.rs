//! Tests for [`crate::compute::particles`] that open no GPU, `#[path]`-included into it. The tests
//! that run the kernels are in `particles_gpu_tests.rs`.

use super::*;
use crate::core::scene::Material;
use crate::projects::interstellar::giant_impact::giant_impact;

#[test]
fn the_shader_s_workgroup_is_the_constant() {
    assert!(PARTICLES_SHADER.contains(&format!("const WORKGROUP: u32 = {GPU_WORKGROUP_SIZE}u;")));
}

#[test]
fn only_scenes_made_entirely_of_particles_go_to_the_gpu() {
    let mut scene = giant_impact().scene;
    assert!(can_step_on_gpu(&scene));

    let mut with_a_rigid_body = scene.clone();
    with_a_rigid_body.props[0].material = Material::ELASTIC;
    assert!(!can_step_on_gpu(&with_a_rigid_body));

    let mut with_an_absorber = scene.clone();
    with_an_absorber.props[0].absorbs = true;
    assert!(!can_step_on_gpu(&with_an_absorber));

    scene.gravity_model = GravityModel::Schwarzschild;
    assert!(!can_step_on_gpu(&scene));
}
