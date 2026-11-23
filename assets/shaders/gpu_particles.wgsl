// Particles drawn straight from the GPU physics buffer (`src/render/gpu_particles.rs`): a unit
// sphere mesh, one instance per particle, placed at the world position the physics' `drift` kernel
// wrote into the particle's record and scaled to its radius.
//
// Shading is a light at the camera with a little ambient: a cloud of tens of thousands of spheres
// reads by its shape, and the scene's lights are not bound in this pass.

#import bevy_pbr::mesh_view_bindings::view

// World units per metre: `1 / METRES_PER_UNIT` in `src/constants.rs`. The radius arrives in metres.
const UNITS_PER_METRE: f32 = 1.0e-9;
// The share of light a face turned away from the camera still gets.
const AMBIENT: f32 = 0.3;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // The particle: position (metres, relative to the physics' origin; unused here) and radius.
    @location(3) position_radius: vec4<f32>,
    // Its colour, `r + 256·g + 65536·b` of 8-bit channels.
    @location(4) colour_code: f32,
    // Its world position, world units, written by `drift`.
    @location(5) world_x: f32,
    @location(6) world_yz: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) colour: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let centre = vec3<f32>(vertex.world_x, vertex.world_yz);
    let world = centre + vertex.position * (vertex.position_radius.w * UNITS_PER_METRE);
    var out: VertexOutput;
    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    let code = u32(vertex.colour_code);
    out.colour = vec3<f32>(
        f32(code & 255u),
        f32((code >> 8u) & 255u),
        f32((code >> 16u) & 255u),
    ) / 255.0;
    out.normal = vertex.normal;
    out.world_position = world;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let to_eye = normalize(view.world_position - in.world_position);
    let lit = AMBIENT + (1.0 - AMBIENT) * max(dot(normalize(in.normal), to_eye), 0.0);
    return vec4<f32>(in.colour * lit, 1.0);
}
