// The GPU twin of a substep of a scene made entirely of particles: gravity between every pair, the
// simulated contact (spring, dashpot, friction and spin), the thermal pressure of hot rock, and the
// heat the contacts make. It mirrors `collision::collision::push_contacts`, then
// `core::integrator::integrate_gravity_with` (Newtonian), then `ContactPass::heat`, which is what
// `tst/compute/particles_tests.rs` holds it to.
//
// One substep is these dispatches, in order (`compute::particles` encodes them):
//
//   refit_leaves, refit_level × depth   the tree's nodes from the particles  (tree only)
//   forces_pairs and/or forces_tree      accelerations from the start of the substep
//   kick                                 velocity and spin; their mean over the kick
//   heat_pairs or heat_tree              the heat, at the mean velocities
//   drift                                position at the new velocity; heat applied
//
// Everything is in SI, f32, positions relative to an origin near the particles. The particles are
// in Morton order, so the implicit tree over them (node n has children 2n and 2n+1, leaf L+l holds
// particles l·leaf_size up to the next leaf's) groups particles that are near each other.

// Must equal `GPU_WORKGROUP_SIZE` in `src/compute/constants.rs`; the host checks.
const WORKGROUP: u32 = 64u;
// Deeper than any tree the host builds (`log2` of the leaves, plus one per level).
const STACK: u32 = 64u;
const EMPTY: f32 = 3.0e38;
const PI: f32 = 3.14159265;

// Which methods `params.flags` asks for.
const GRAVITY_DIRECT: u32 = 1u;
const GRAVITY_TREE: u32 = 2u;
const NEIGHBOURS_PAIRS: u32 = 4u;
const NEIGHBOURS_TREE: u32 = 8u;

struct Particle {
    // xyz: position, m. w: radius, m.
    position: vec4<f32>,
    // xyz: velocity, m/s. w: mass, kg.
    velocity: vec4<f32>,
    // xyz: spin, rad/s. w: heat, J/kg.
    spin: vec4<f32>,
    // x: elastic modulus, Pa. y: damping ratio, capped. z: friction. w: unused.
    material: vec4<f32>,
    // Tillotson a, b, E0 (J/kg); E0 = 0 for a material heat does not push. w: unused.
    thermal: vec4<f32>,
}

struct Motion {
    velocity: vec4<f32>,
    spin: vec4<f32>,
}

struct Force {
    // xyz: acceleration, m/s². w: heating, J/kg/s.
    linear: vec4<f32>,
    // xyz: angular acceleration, rad/s². w: unused.
    angular: vec4<f32>,
}

struct Node {
    // xyz: centre of mass. w: mass; 0 for an empty node.
    com: vec4<f32>,
    // The box around every particle's reach in the node: what a neighbour search must open.
    lo: vec4<f32>,
    hi: vec4<f32>,
    // The box around the particles' centres: the size Barnes–Hut compares with the distance.
    centre_lo: vec4<f32>,
    centre_hi: vec4<f32>,
}

struct Params {
    dt: f32,
    count: u32,
    softening_squared: f32,
    gravitational_constant: f32,
    opening_angle: f32,
    leaves: u32,
    // A hot particle's smoothing length over its radius.
    smoothing_per_radius: f32,
    // The face-centred cubic packing fraction, for a particle's density.
    packing: f32,
    // How many contacts may share a slip; see `SOFT_CONTACT_MAX_NEIGHBOURS`.
    max_neighbours: f32,
    flags: u32,
    // Particles per leaf; leaf `l` holds particles `l·leaf_size ..`.
    leaf_size: u32,
    pad1: u32,
}

struct Level {
    first: u32,
    count: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> motion: array<Motion>;
@group(0) @binding(3) var<storage, read_write> mean: array<Motion>;
@group(0) @binding(4) var<storage, read_write> forces: array<Force>;
@group(0) @binding(5) var<storage, read_write> nodes: array<Node>;
@group(0) @binding(6) var<uniform> level: Level;

var<workgroup> tile: array<Particle, WORKGROUP>;
var<workgroup> tile_mean: array<Motion, WORKGROUP>;

fn has(flag: u32) -> bool {
    return (params.flags & flag) != 0u;
}

fn is_hot(p: Particle) -> bool {
    return p.thermal.z > 0.0;
}

fn reach(p: Particle) -> f32 {
    if is_hot(p) {
        return max(p.position.w, params.smoothing_per_radius * p.position.w);
    }
    return p.position.w;
}

fn inertia(p: Particle) -> f32 {
    return 0.4 * p.velocity.w * p.position.w * p.position.w;
}

fn density(p: Particle) -> f32 {
    let r = p.position.w;
    return p.velocity.w * params.packing / (4.18879020 * r * r * r);
}

// Tillotson's thermal pressure at the reference density, over the density squared: `P/ρ²`.
fn pressure_over_density_squared(p: Particle) -> f32 {
    let heat = p.spin.w;
    if !is_hot(p) || heat <= 0.0 {
        return 0.0;
    }
    let rho = density(p);
    let gruneisen = p.thermal.x + p.thermal.y / (heat / p.thermal.z + 1.0);
    return gruneisen * heat / rho;
}

// dW/dr of the cubic spline.
fn kernel_slope(distance: f32, smoothing: f32) -> f32 {
    let q = distance / smoothing;
    let h2 = smoothing * smoothing;
    let sigma = 1.0 / (PI * h2 * h2);
    if q < 1.0 {
        return sigma * (-3.0 * q + 2.25 * q * q);
    }
    if q < 2.0 {
        let s = 2.0 - q;
        return sigma * -0.75 * s * s;
    }
    return 0.0;
}

// What a neighbour does to `me`, with `me` as the CPU's `a` and `other` as its `b`.
struct Pair {
    // On me: acceleration, m/s², and angular acceleration, rad/s².
    acceleration: vec3<f32>,
    angular: vec3<f32>,
    // For the heat pass.
    normal: vec3<f32>,
    arm_me: f32,
    arm_other: f32,
    dashpot: f32,
    friction_on_other: vec3<f32>,
    // m_other · (P/ρ² of both), and ∇W at me.
    pressure: f32,
    gradient_me: vec3<f32>,
}

fn pair_terms(me: Particle, other: Particle, offset: vec3<f32>, distance: f32) -> Pair {
    var out: Pair;
    out.acceleration = vec3<f32>(0.0);
    out.angular = vec3<f32>(0.0);
    out.dashpot = 0.0;
    out.friction_on_other = vec3<f32>(0.0);
    out.pressure = 0.0;
    out.gradient_me = vec3<f32>(0.0);
    let normal = offset / distance;
    out.normal = normal;
    let r_me = me.position.w;
    let r_other = other.position.w;
    let m_me = me.velocity.w;
    let m_other = other.velocity.w;
    out.arm_me = distance * (r_me / (r_me + r_other));
    out.arm_other = distance - out.arm_me;
    var force_on_other = vec3<f32>(0.0);
    var torque_me = vec3<f32>(0.0);

    let overlap = r_me + r_other - distance;
    if overlap > 0.0 && me.material.x > 0.0 && other.material.x > 0.0 {
        // Every product grouped so no intermediate overflows f32: two planet-sized particle masses
        // multiplied are 1e44.
        let modulus = me.material.x * (other.material.x / (me.material.x + other.material.x));
        let reduced_radius = r_me * (r_other / (r_me + r_other));
        let stiffness = modulus * reduced_radius;
        let reduced_mass = m_me * (m_other / (m_me + m_other));
        let zeta = max(me.material.y, other.material.y);
        let damping = 2.0 * zeta * sqrt(reduced_mass) * sqrt(stiffness);

        let normal_speed = dot(other.velocity.xyz - me.velocity.xyz, normal);
        let push = max(0.0, stiffness * overlap - damping * normal_speed);
        force_on_other = normal * push;
        out.dashpot = stiffness * overlap - push;

        let surface_me = me.velocity.xyz + cross(me.spin.xyz, normal * out.arm_me);
        let surface_other = other.velocity.xyz + cross(other.spin.xyz, -normal * out.arm_other);
        let slip = surface_other - surface_me;
        let sliding = slip - normal * dot(slip, normal);
        let sliding_speed = length(sliding);
        let friction = sqrt(me.material.z * other.material.z);
        if friction > 0.0 && sliding_speed > 0.0 {
            let inverse = 1.0 / m_me + 1.0 / m_other
                + out.arm_me * (out.arm_me / inertia(me))
                + out.arm_other * (out.arm_other / inertia(other));
            let slip_mass = 1.0 / inverse;
            let drag = min(friction * push, slip_mass * sliding_speed / (params.dt * params.max_neighbours));
            let friction_on_other = -sliding / sliding_speed * drag;
            out.friction_on_other = friction_on_other;
            force_on_other += friction_on_other;
            torque_me = cross(normal * out.arm_me, -friction_on_other);
        }
    }
    out.acceleration = -force_on_other / m_me;
    out.angular = torque_me / inertia(me);

    if is_hot(me) && is_hot(other) {
        let smoothing = 0.5 * params.smoothing_per_radius * (r_me + r_other);
        let slope = kernel_slope(distance, smoothing);
        let both = pressure_over_density_squared(me) + pressure_over_density_squared(other);
        if slope < 0.0 && both > 0.0 {
            let gradient_me = -normal * slope;
            out.gradient_me = gradient_me;
            out.pressure = m_other * both;
            // The force on me is −m_me·m_other·(…)·∇W.
            out.acceleration -= gradient_me * out.pressure;
        }
    }
    return out;
}

fn gravity_from(me: Particle, offset: vec3<f32>, distance_squared: f32, mass: f32) -> vec3<f32> {
    let distance = sqrt(distance_squared);
    return offset
        * (params.gravitational_constant * mass / (distance * (distance_squared + params.softening_squared)));
}

// My share of the heat a pair makes, J/kg/s, at the mean velocities.
fn pair_heating(me: Particle, other: Particle, t: Pair, mean_me: Motion, mean_other: Motion) -> f32 {
    if !is_hot(me) {
        return 0.0;
    }
    var dissipated = t.dashpot * dot(mean_other.velocity.xyz - mean_me.velocity.xyz, t.normal);
    let surface_me = mean_me.velocity.xyz + cross(mean_me.spin.xyz, t.normal * t.arm_me);
    let surface_other = mean_other.velocity.xyz + cross(mean_other.spin.xyz, -t.normal * t.arm_other);
    dissipated -= dot(t.friction_on_other, surface_other - surface_me);
    var share = 1.0;
    if is_hot(other) {
        share = 0.5;
    }
    var heating = share * dissipated / me.velocity.w;
    heating += 0.5 * t.pressure * dot(mean_me.velocity.xyz - mean_other.velocity.xyz, t.gradient_me);
    return heating;
}

fn neighbours(me: Particle, other: Particle, distance_squared: f32) -> bool {
    let reach_sum = reach(me) + reach(other);
    return distance_squared < reach_sum * reach_sum;
}

@compute @workgroup_size(WORKGROUP)
fn forces_pairs(
    @builtin(global_invocation_id) global: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let i = global.x;
    let in_range = i < params.count;
    var me: Particle;
    if in_range {
        me = particles[i];
    }
    var acceleration = vec3<f32>(0.0);
    var angular = vec3<f32>(0.0);
    for (var base = 0u; base < params.count; base += WORKGROUP) {
        let loading = base + local.x;
        if loading < params.count {
            tile[local.x] = particles[loading];
        }
        workgroupBarrier();
        if in_range {
            let span = min(WORKGROUP, params.count - base);
            for (var k = 0u; k < span; k++) {
                if base + k == i {
                    continue;
                }
                let other = tile[k];
                let offset = other.position.xyz - me.position.xyz;
                let distance_squared = dot(offset, offset);
                if distance_squared <= 0.0 {
                    continue;
                }
                if has(GRAVITY_DIRECT) {
                    acceleration += gravity_from(me, offset, distance_squared, other.velocity.w);
                }
                if has(NEIGHBOURS_PAIRS) && neighbours(me, other, distance_squared) {
                    let t = pair_terms(me, other, offset, sqrt(distance_squared));
                    acceleration += t.acceleration;
                    angular += t.angular;
                }
            }
        }
        workgroupBarrier();
    }
    if in_range {
        forces[i].linear = vec4<f32>(forces[i].linear.xyz + acceleration, 0.0);
        forces[i].angular = vec4<f32>(forces[i].angular.xyz + angular, 0.0);
    }
}

// What every particle of leaf `leaf` does to `me` (index `i`): gravity if `gravity`, neighbours'
// forces if `near`. Adds into `acceleration` and `angular`.
struct Sum {
    acceleration: vec3<f32>,
    angular: vec3<f32>,
}

fn leaf_forces(me: Particle, i: u32, leaf: u32, gravity: bool, near: bool, sum: Sum) -> Sum {
    var out = sum;
    let first = leaf * params.leaf_size;
    let last = min(first + params.leaf_size, params.count);
    for (var j = first; j < last; j++) {
        if j == i {
            continue;
        }
        let other = particles[j];
        let offset = other.position.xyz - me.position.xyz;
        let distance_squared = dot(offset, offset);
        if distance_squared <= 0.0 {
            continue;
        }
        if gravity {
            out.acceleration += gravity_from(me, offset, distance_squared, other.velocity.w);
        }
        if near && neighbours(me, other, distance_squared) {
            let t = pair_terms(me, other, offset, sqrt(distance_squared));
            out.acceleration += t.acceleration;
            out.angular += t.angular;
        }
    }
    return out;
}

@compute @workgroup_size(WORKGROUP)
fn kick(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    let p = particles[i];
    let f = forces[i];
    let velocity = p.velocity.xyz + f.linear.xyz * params.dt;
    let spin = p.spin.xyz + f.angular.xyz * params.dt;
    motion[i] = Motion(vec4<f32>(velocity, 0.0), vec4<f32>(spin, 0.0));
    mean[i] = Motion(
        vec4<f32>(0.5 * (p.velocity.xyz + velocity), 0.0),
        vec4<f32>(0.5 * (p.spin.xyz + spin), 0.0),
    );
    forces[i] = Force(vec4<f32>(0.0), vec4<f32>(0.0));
}

@compute @workgroup_size(WORKGROUP)
fn heat_pairs(
    @builtin(global_invocation_id) global: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let i = global.x;
    let in_range = i < params.count;
    var me: Particle;
    var mine: Motion;
    if in_range {
        me = particles[i];
        mine = mean[i];
    }
    var heating = 0.0;
    for (var base = 0u; base < params.count; base += WORKGROUP) {
        let loading = base + local.x;
        if loading < params.count {
            tile[local.x] = particles[loading];
            tile_mean[local.x] = mean[loading];
        }
        workgroupBarrier();
        if in_range && is_hot(me) {
            let span = min(WORKGROUP, params.count - base);
            for (var k = 0u; k < span; k++) {
                if base + k == i {
                    continue;
                }
                let other = tile[k];
                let offset = other.position.xyz - me.position.xyz;
                let distance_squared = dot(offset, offset);
                if distance_squared <= 0.0 || !neighbours(me, other, distance_squared) {
                    continue;
                }
                let t = pair_terms(me, other, offset, sqrt(distance_squared));
                heating += pair_heating(me, other, t, mine, tile_mean[k]);
            }
        }
        workgroupBarrier();
    }
    if in_range {
        forces[i].linear.w = heating;
    }
}

@compute @workgroup_size(WORKGROUP)
fn drift(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    var p = particles[i];
    let m = motion[i];
    p.velocity = vec4<f32>(m.velocity.xyz, p.velocity.w);
    p.position = vec4<f32>(p.position.xyz + m.velocity.xyz * params.dt, p.position.w);
    var heat = p.spin.w;
    if is_hot(p) {
        heat = max(0.0, heat + forces[i].linear.w * params.dt);
    }
    p.spin = vec4<f32>(m.spin.xyz, heat);
    particles[i] = p;
    forces[i].linear.w = 0.0;
}

@compute @workgroup_size(WORKGROUP)
fn refit_leaves(@builtin(global_invocation_id) global: vec3<u32>) {
    let leaf = global.x;
    if leaf >= params.leaves {
        return;
    }
    var node: Node;
    node.com = vec4<f32>(0.0);
    node.lo = vec4<f32>(EMPTY);
    node.hi = vec4<f32>(-EMPTY);
    node.centre_lo = vec4<f32>(EMPTY);
    node.centre_hi = vec4<f32>(-EMPTY);
    let first = leaf * params.leaf_size;
    let last = min(first + params.leaf_size, params.count);
    var weighted = vec3<f32>(0.0);
    var mass = 0.0;
    for (var j = first; j < last; j++) {
        let p = particles[j];
        let r = reach(p);
        mass += p.velocity.w;
        node.lo = min(node.lo, vec4<f32>(p.position.xyz - vec3<f32>(r), 0.0));
        node.hi = max(node.hi, vec4<f32>(p.position.xyz + vec3<f32>(r), 0.0));
        node.centre_lo = min(node.centre_lo, vec4<f32>(p.position.xyz, 0.0));
        node.centre_hi = max(node.centre_hi, vec4<f32>(p.position.xyz, 0.0));
    }
    // A second pass for the centre of mass, each mass a fraction of the total so nothing
    // overflows f32.
    for (var j = first; j < last; j++) {
        let p = particles[j];
        weighted += p.position.xyz * (p.velocity.w / mass);
    }
    if mass > 0.0 {
        node.com = vec4<f32>(weighted, mass);
    }
    nodes[params.leaves + leaf] = node;
}

@compute @workgroup_size(WORKGROUP)
fn refit_level(@builtin(global_invocation_id) global: vec3<u32>) {
    if global.x >= level.count {
        return;
    }
    let n = level.first + global.x;
    let left = nodes[2u * n];
    let right = nodes[2u * n + 1u];
    let mass = left.com.w + right.com.w;
    var node: Node;
    if mass > 0.0 {
        let centre = left.com.xyz * (left.com.w / mass) + right.com.xyz * (right.com.w / mass);
        node.com = vec4<f32>(centre, mass);
    } else {
        node.com = vec4<f32>(0.0);
    }
    node.lo = min(left.lo, right.lo);
    node.hi = max(left.hi, right.hi);
    node.centre_lo = min(left.centre_lo, right.centre_lo);
    node.centre_hi = max(left.centre_hi, right.centre_hi);
    nodes[n] = node;
}
@compute @workgroup_size(WORKGROUP)
fn forces_tree(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    let me = particles[i];
    let my_reach = reach(me);
    let gravity = has(GRAVITY_TREE);
    let near = has(NEIGHBOURS_TREE);
    let theta_squared = params.opening_angle * params.opening_angle;
    var sum = Sum(vec3<f32>(0.0), vec3<f32>(0.0));
    var stack: array<u32, STACK>;
    stack[0] = 1u;
    var top = 1u;
    while top > 0u {
        top -= 1u;
        let n = stack[top];
        let node = nodes[n];
        if node.com.w <= 0.0 {
            continue;
        }
        let nearest = clamp(me.position.xyz, node.lo.xyz, node.hi.xyz);
        let gap = me.position.xyz - nearest;
        let overlaps = dot(gap, gap) < my_reach * my_reach;
        if !overlaps {
            if !gravity {
                continue;
            }
            let extent = node.centre_hi.xyz - node.centre_lo.xyz;
            let size = max(extent.x, max(extent.y, extent.z));
            let to = node.com.xyz - me.position.xyz;
            let distance_squared = dot(to, to);
            if size * size < theta_squared * distance_squared {
                sum.acceleration += gravity_from(me, to, distance_squared, node.com.w);
                continue;
            }
        }
        if n >= params.leaves {
            // Near enough to open, or overlapping: every particle of the leaf, one by one. A leaf
            // that does not overlap is only here for its gravity.
            sum = leaf_forces(me, i, n - params.leaves, gravity, near && overlaps, sum);
            continue;
        }
        if top + 2u <= STACK {
            stack[top] = 2u * n;
            stack[top + 1u] = 2u * n + 1u;
            top += 2u;
        }
    }
    forces[i].linear = vec4<f32>(forces[i].linear.xyz + sum.acceleration, 0.0);
    forces[i].angular = vec4<f32>(forces[i].angular.xyz + sum.angular, 0.0);
}

@compute @workgroup_size(WORKGROUP)
fn heat_tree(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    let me = particles[i];
    if !is_hot(me) {
        forces[i].linear.w = 0.0;
        return;
    }
    let mine = mean[i];
    let my_reach = reach(me);
    var heating = 0.0;
    var stack: array<u32, STACK>;
    stack[0] = 1u;
    var top = 1u;
    while top > 0u {
        top -= 1u;
        let n = stack[top];
        let node = nodes[n];
        if node.com.w <= 0.0 {
            continue;
        }
        let nearest = clamp(me.position.xyz, node.lo.xyz, node.hi.xyz);
        let gap = me.position.xyz - nearest;
        if dot(gap, gap) >= my_reach * my_reach {
            continue;
        }
        if n >= params.leaves {
            let first = (n - params.leaves) * params.leaf_size;
            let last = min(first + params.leaf_size, params.count);
            for (var j = first; j < last; j++) {
                if j == i {
                    continue;
                }
                let other = particles[j];
                let offset = other.position.xyz - me.position.xyz;
                let distance_squared = dot(offset, offset);
                if distance_squared <= 0.0 || !neighbours(me, other, distance_squared) {
                    continue;
                }
                let t = pair_terms(me, other, offset, sqrt(distance_squared));
                heating += pair_heating(me, other, t, mine, mean[j]);
            }
            continue;
        }
        if top + 2u <= STACK {
            stack[top] = 2u * n;
            stack[top + 1u] = 2u * n + 1u;
            top += 2u;
        }
    }
    forces[i].linear.w = heating;
}
