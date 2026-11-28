// The GPU twin of a substep of a scene made entirely of particles: gravity between every pair, the
// simulated contact (spring, dashpot, friction and spin), the thermal pressure of hot rock, and the
// heat the contacts make. It mirrors `collision::collision::push_contacts`, then
// `core::integrator::integrate_gravity_with` (Newtonian), then `ContactPass::heat`, which is what
// `tst/compute/particles_tests.rs` holds it to.
//
// Fluid particles (smoothed-particle hydrodynamics with the whole Tillotson equation of state, for
// fluid planets) mirror `measure_fluids` and `fluid_pair_forces` in the same file of the CPU.
//
// Time advances in ticks with individual, power-of-two steps per particle and a kick-drift-kick
// leapfrog (Springel 2005; see `Control`). One tick is these dispatches, in order
// (`compute::particles` encodes them):
//
//   tick                                 whether the tick runs; the tick counter
//   drift                                every particle, at its half-step velocity
//   refit_leaves, refit_level × depth   the tree's nodes from the particles  (tree only)
//   activate                             whose steps end now; everyone's predicted velocity
//   density_pairs or density_tree        fluid density, pressure, sound speed  (active fluids)
//   forces_pairs and/or forces_tree      accelerations and heating  (active particles)
//   close_kick                           the closing half kick; a fluid's smoothing and radius
//   limits                               each active particle's step limit
//   plan                                 at a macro step's end: the next one's tick and bins
//   open_kick                            each active particle's next step, and its half kick
//
// The CPU integrates with one global step (kick, then drift, heat at the mean velocities), so the
// two agree to the integrators' truncation error, not bit for bit.
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
const NEIGHBOURS_GRID: u32 = 16u;
// Barnes–Hut gravity by the cooperative walk (`gravity_group`) rather than in `forces_tree`.
const GRAVITY_GROUP: u32 = 32u;

struct Particle {
    // xyz: position, m. w: radius, m.
    position: vec4<f32>,
    // xyz: velocity, m/s. w: mass, kg.
    velocity: vec4<f32>,
    // xyz: spin, rad/s. w: heat, J/kg.
    spin: vec4<f32>,
    // x: elastic modulus, Pa. y: damping ratio, capped. z: friction. w: the prop's index in the
    // scene, as a float (exact to 16 million), so a read-back finds its prop after the GPU sorts.
    material: vec4<f32>,
    // Tillotson a, b, E0 (J/kg); E0 = 0 for a material heat does not push. w: E_cv (J/kg).
    thermal: vec4<f32>,
    // Tillotson ρ0 (kg/m³), A (Pa), B (Pa), E_iv (J/kg). Fluids only.
    eos: vec4<f32>,
    // Tillotson α, β (fluids only). z: the colour the particle is drawn in, `r + 256·g + 65536·b` of
    // its 8-bit channels (an integer, exact in f32). w: its world x, world units, for the renderer.
    eos_decay: vec4<f32>,
    // x: smoothing length this substep, m; 0 for a particle that is not a fluid. y: density, kg/m³,
    // as last measured. zw: its world y and z, world units, for the renderer.
    fluid: vec4<f32>,
}

// A particle's velocity and spin as a pair term sees them (`pair_heating`).
struct Motion {
    velocity: vec4<f32>,
    spin: vec4<f32>,
}

// Per-particle integrator state that the sort carries along with the particle. The particle record
// itself holds the velocity, spin and internal energy as they are at the current tick (predicted
// part-way through a step), which is what neighbours, the read-back and the renderer see; the
// half-step state the leapfrog integrates is here.
//
// - `kinematics[i]`: `a` = acceleration xyz (m/s²) and heating w (J/kg/s) at the particle's last
//   force evaluation; `b` = angular acceleration xyz (rad/s²) and w its current step, seconds;
//   `c` = its spin at the half step xyz (rad/s), w unused; `d` = its gravity xyz (m/s²) at its last
//   gravity evaluation and w the simulated time of it (s); `e` = xyz the rate the gravity was
//   changing at (the jerk, m/s³, from its last two evaluations) and w one once `d` holds a value.
//   `d` and `e` are used only when gravity skips ticks (`Params::gravity_interval`).
// - `timing[i]`: `a` = x its step limit (s), y its step in ticks, z the tick its step began, w one
//   when it is active this tick; `b` = its velocity at the half step xyz (m/s) and internal energy
//   w (J/kg) integrated to the half step; `c` its fluid state, carried through the sort only; `d`
//   xyz the part of its position finer than the position's f32 spacing, m (`drift`), w unused.
struct Slot {
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    d: vec4<f32>,
    e: vec4<f32>,
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
    // The quadrupole about the centre of mass, per unit mass (m²): `Σ (m/M)(3·x·xᵀ − |x|²·I)` for `x`
    // from the centre of mass. Per unit mass because the moment itself of a planet, `1e38 kg·m²`, is
    // at the edge of f32. `quad_diag.xyz`: xx, yy, zz; `quad_off.xyz`: xy, xz, yz.
    quad_diag: vec4<f32>,
    quad_off: vec4<f32>,
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
    // Ticks between gravity evaluations by the cooperative walk, a power of two; 1 is every tick.
    // See `gravity_due`.
    gravity_interval: u32,
    // The fluids' artificial viscosity: α, β, and ε (of h²); `SPH_VISCOSITY_*`.
    viscosity_alpha: f32,
    viscosity_beta: f32,
    viscosity_epsilon: f32,
    // The least sound speed, as a fraction of √(A/ρ0); `SPH_SOUND_SPEED_FLOOR_FRACTION`.
    sound_floor: f32,
    // A fluid's smoothing length over its spacing, and its bounds as fractions of the value at ρ0;
    // `SPH_SMOOTHING_FACTOR`, `SPH_MIN_SMOOTHING_FRACTION`, `SPH_MAX_SMOOTHING_FRACTION`.
    sph_factor: f32,
    smoothing_min: f32,
    smoothing_max: f32,
    // The frame's simulated seconds, added to the time left at the start of an advance.
    frame_dt: f32,
    // The step limits (`collision::resolve::substep_count` on the CPU): the most a particle may move
    // in a substep, m; the fraction of a particle pair's dynamical time; substeps per contact; and
    // the fluids' Courant factor.
    displacement_cap: f32,
    orbital_fraction: f32,
    contact_substeps: f32,
    courant: f32,
    // Where the particles' positions are measured from, in world units, and world units per metre:
    // `drift` writes each particle's world position for the renderer from them.
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    units_per_metre: f32,
    // The individual time steps: bins kept free below the shortest step at each macro step's start,
    // and the most bins (`GPU_TIME_BIN_HEADROOM`, `GPU_MAX_TIME_BINS`).
    bin_headroom: u32,
    max_bins: u32,
    // The most a particle's step may exceed a neighbour's (`GPU_NEIGHBOUR_STEP_RATIO`).
    neighbour_ratio: f32,
    // The cell list's buckets less one (a power of two less one), for the hash.
    grid_mask: u32,
}

// What the GPU decides for itself between ticks. Written by the host at an upload; then the kernels
// keep it.
//
// Time is a sequence of macro steps, each `2^bins` ticks of `dt`. A particle steps every `2^k`
// ticks for its own `k` (its bin), so it is active, evaluating forces and kicked, only on the ticks
// where its step ends; every particle is active at a macro step's boundary (Springel, V. (2005),
// The cosmological simulation code GADGET-2, MNRAS 364(4), 1105–1134,
// doi:10.1111/j.1365-2966.2005.09655.x, section 4.2, "Individual and adaptive timesteps").
struct Control {
    // Simulated seconds still to plan into macro steps in this advance (and carried from the last).
    time_left: f32,
    // The tick's length, seconds; zero when no macro step is in progress, which makes every kernel
    // return at once.
    dt: f32,
    // Fixed-step mode when positive: one bin, every tick this long.
    fixed_dt: f32,
    // Ticks run since the advance began.
    substeps: u32,
    // The shortest step any particle allows, as the bits of a positive f32 (which order as u32s).
    limit: atomic<u32>,
    // The particles' bounding box, as order-preserving u32s of the coordinates, for the sort.
    lo_x: atomic<u32>,
    lo_y: atomic<u32>,
    lo_z: atomic<u32>,
    hi_x: atomic<u32>,
    hi_y: atomic<u32>,
    hi_z: atomic<u32>,
    // Simulated seconds stepped since the upload: the clock is corrected from it when the GPU falls
    // behind.
    simulated: f32,
    // The longest step any particle allows, as `limit` is the shortest: their ratio sets the bins.
    limit_max: atomic<u32>,
    // The tick within the macro step, 0 to `2^bins`.
    tick: u32,
    // This macro step's bins: `2^bins` ticks.
    bins: u32,
    // One when this tick runs (a macro step is in progress, or the first evaluation after an upload).
    run: u32,
    // One when the macro step has ended and the next is still to be planned.
    boundary: u32,
    // One until the first macro step is planned: the first tick evaluates forces without moving.
    prime: u32,
    // The largest reach of any particle this tick, as the bits of a positive f32: the side of the
    // cell list's cells.
    reach_max: atomic<u32>,
    // Particles active, summed over the advance's ticks (`activate`), for the GPU pass timer.
    active_sum: atomic<u32>,
}

// A fluid particle's state for the substep, from `density_pairs` or `density_tree`.
struct Sph {
    // x: density, kg/m³. y: pressure, Pa. z: sound speed, m/s. w: the largest viscosity `|μ|` with
    // an approaching neighbour, m/s, for the Courant limit.
    state: vec4<f32>,
}

struct Level {
    first: u32,
    count: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> kinematics: array<Slot>;
@group(0) @binding(3) var<storage, read_write> timing: array<Slot>;
@group(0) @binding(4) var<storage, read_write> forces: array<Force>;
@group(0) @binding(5) var<storage, read_write> nodes: array<Node>;
@group(0) @binding(6) var<uniform> level: Level;
@group(0) @binding(7) var<storage, read_write> sph: array<Sph>;
// The sort's (Morton key, particle index) pairs, padded to a power of two.
@group(0) @binding(8) var<storage, read_write> keys: array<vec2<u32>>;
// Where the sort gathers the particles into their new order.
@group(0) @binding(9) var<storage, read_write> scratch: array<Particle>;
@group(0) @binding(10) var<storage, read_write> control: Control;
// Where the sort gathers `kinematics` (first half) and `timing` (second half).
@group(0) @binding(11) var<storage, read_write> slot_scratch: array<Slot>;
// The cell list (`grid_clear` .. `grid_scatter`): each bucket's start in `grid_items` after the prefix
// sum (with the sum's upper levels past the buckets), the write cursors, and the particle indices.
@group(0) @binding(12) var<storage, read_write> grid: array<atomic<u32>>;
@group(0) @binding(13) var<storage, read_write> grid_cursor_at: array<atomic<u32>>;
@group(0) @binding(14) var<storage, read_write> grid_items: array<u32>;

var<workgroup> tile: array<Particle, WORKGROUP>;
// The tile's half-step velocities and spins, what the pair forces are evaluated from (`moving`).
var<workgroup> tile_half: array<Motion, WORKGROUP>;
var<workgroup> tile_sph: array<Sph, WORKGROUP>;
// Set by any invocation of the workgroup whose particle is active, read back uniformly, so a
// kernel with barriers can skip a workgroup with nothing to do.
var<workgroup> workgroup_any: u32;
// The particle's own step, seconds, for the friction's drag cap in `pair_terms`.
var<private> my_step: f32;

// `running` for a kernel with barriers: the same for every invocation of the workgroup.
fn running_uniform(local: u32) -> bool {
    if local == 0u {
        workgroup_any = control.run;
    }
    return workgroupUniformLoad(&workgroup_any) == 1u;
}

// Particle `j`'s velocity and spin at the half step: what it is drifting with.
fn half_motion(j: u32) -> Motion {
    return Motion(timing[j].b, kinematics[j].c);
}

// `p` with the velocity and spin it is drifting with, for the pair forces. The dissipative ones (the
// dashpot, friction and its drag cap, the viscosity) must see the velocity the kick will change, as
// in symplectic Euler on the half-step velocities: evaluated from the predicted velocity, which
// already holds the last step's friction, a sticking contact overshoots and the friction pumps
// energy in (measured: 10% of a planet's binding energy in its first two hours of settling). The
// heat is still booked at the predicted velocity, near the mean over the step.
fn moving(p: Particle, m: Motion) -> Particle {
    if is_fluid(p) {
        return p;
    }
    var q = p;
    q.velocity = vec4<f32>(m.velocity.xyz, p.velocity.w);
    q.spin = vec4<f32>(m.spin.xyz, p.spin.w);
    return q;
}

// The step particle `i`'s forces this tick will act over, for the friction's drag cap: its step, or
// at the first evaluation after an upload the first tick (the shortest limit, from `limits_first`).
fn forces_step(i: u32) -> f32 {
    if control.prime == 1u {
        return max(bitcast<f32>(atomicLoad(&control.limit)), 1.0e-30);
    }
    return max(max(kinematics[i].b.w, control.dt), 1.0e-30);
}

// The time-step limiter (Saitoh, T. R. & Makino, J. (2009), A necessary condition for individual time
// steps in SPH simulations, ApJ 697(2), L99–L102, doi:10.1088/0004-637X/697/2/L99): active particle
// `i`, stepping `mine` seconds, meets neighbour `j`. If `j` is in a step more than `neighbour_ratio`
// times longer it is woken (its step ends next tick; `open_kick`), since it would otherwise answer a
// contact with a stale force for its whole step. Returns `j`'s step limit, for `i`'s passive limit.
// The flag is `forces[j].angular.w < 0`: an inactive particle's force record is otherwise unused.
fn limit_neighbour(j: u32, mine: f32) -> f32 {
    if !is_active(j) && kinematics[j].b.w > params.neighbour_ratio * mine {
        forces[j].angular.w = -1.0;
    }
    return timing[j].a.x;
}

// An active particle's neighbour limit into its force record, the least of what each force kernel
// found (both may run).
fn store_nearest(i: u32, nearest: f32) {
    let previous = forces[i].angular.w;
    if previous > 0.0 {
        forces[i].angular.w = min(previous, nearest);
    } else {
        forces[i].angular.w = nearest;
    }
}

// Whether particle `i`'s gravity is due: at the first evaluation after an upload, when none is
// remembered, and once the last is `gravity_interval` ticks old (of this tick's length). The age,
// not the tick within the macro step, because macro steps can be a couple of ticks long, and a
// boundary every other tick would make every interval two. The cooperative walk runs for a
// workgroup when any of its active particles is due, and then evaluates all of them; particles
// next to each other in Morton order are evaluated together and stay in step. On the ticks between,
// an active particle is kicked with its gravity
// predicted from its last two evaluations (`predicted_gravity`), after Grudić, M. Y. (2021),
// Accelerating self-gravitating hydrodynamics simulations with adaptive force updates, MNRAS 507(1),
// 1324 (doi:10.1093/mnras/stab2208; arXiv:2010.13792): gravity changes on its own, longer time scale
// than the fluid's, so evaluating it at every hydrodynamic step wastes most of those evaluations.
// Grudić predicts with the jerk the gravity solver evaluates; here it is the difference of the last
// two evaluations, and the interval is one for every particle rather than set by each one's tidal
// time. A particle whose own step is `gravity_interval` ticks or more is always evaluated: its last
// evaluation is at least a step old.
fn gravity_due(i: u32) -> bool {
    if params.gravity_interval <= 1u || control.prime == 1u {
        return true;
    }
    let k = kinematics[i];
    return k.e.w < 0.5
        || control.simulated - k.d.w >= (f32(params.gravity_interval) - 0.5) * control.dt;
}

// Whether the cooperative walk evaluated particle `i`'s gravity this tick (`remember_gravity` stamps
// it with this tick's time).
fn gravity_evaluated(k: Slot) -> bool {
    return k.e.w > 0.5 && k.d.w == control.simulated;
}

// Remember gravity `g` just evaluated for particle `i`, and the rate it is changing at from the last
// evaluation, for `predicted_gravity`. Nothing when gravity is evaluated every tick.
fn remember_gravity(i: u32, g: vec3<f32>) {
    if params.gravity_interval <= 1u {
        return;
    }
    let last = kinematics[i].d;
    let now = control.simulated;
    let elapsed = now - last.w;
    var jerk = vec3<f32>(0.0);
    if kinematics[i].e.w > 0.5 && elapsed > 0.0 {
        jerk = (g - last.xyz) / elapsed;
    }
    kinematics[i].d = vec4<f32>(g, now);
    kinematics[i].e = vec4<f32>(jerk, 1.0);
}

// Particle `i`'s gravity now, predicted from its slot `k` (`remember_gravity`): the last value moved
// on by the jerk for the time since.
fn predicted_gravity(k: Slot) -> vec3<f32> {
    if k.e.w < 0.5 {
        return vec3<f32>(0.0);
    }
    return k.d.xyz + k.e.xyz * (control.simulated - k.d.w);
}

// Whether this tick runs at all.
fn running() -> bool {
    return control.run == 1u;
}

// Whether particle `i` is active this tick: its step ends here.
fn is_active(i: u32) -> bool {
    return timing[i].a.w > 0.5;
}

// For a kernel with barriers: whether this tick runs and some particle of the workgroup is active,
// the same for every invocation (through workgroup memory), so returning on it keeps the barriers
// in uniform control flow.
fn workgroup_has_work(local: u32, i: u32, in_range: bool) -> bool {
    if local == 0u {
        workgroup_any = 0u;
    }
    workgroupBarrier();
    if in_range && running() && is_active(i) {
        workgroup_any = 1u;
    }
    return workgroupUniformLoad(&workgroup_any) == 1u;
}

fn has(flag: u32) -> bool {
    return (params.flags & flag) != 0u;
}

fn is_fluid(p: Particle) -> bool {
    return p.fluid.x > 0.0;
}

// A rubble particle with a thermal pressure (not a fluid, which has its own).
fn is_hot(p: Particle) -> bool {
    return p.thermal.z > 0.0 && !is_fluid(p);
}

fn reach(p: Particle) -> f32 {
    if is_fluid(p) {
        return p.fluid.x;
    }
    if is_hot(p) {
        return max(p.position.w, params.smoothing_per_radius * p.position.w);
    }
    return p.position.w;
}

// The cubic spline W(r, h).
fn kernel_value(distance: f32, smoothing: f32) -> f32 {
    let q = distance / smoothing;
    let sigma = 1.0 / (PI * smoothing * smoothing * smoothing);
    if q < 1.0 {
        return sigma * (1.0 - 1.5 * q * q + 0.75 * q * q * q);
    }
    if q < 2.0 {
        let s = 2.0 - q;
        return sigma * 0.25 * s * s * s;
    }
    return 0.0;
}

// The Tillotson pressure before its floor at zero; `unclamped_tillotson_pressure` on the CPU.
fn tillotson_raw(p: Particle, rho: f32, u_in: f32) -> f32 {
    let u = max(u_in, 0.0);
    let a = p.thermal.x;
    let b = p.thermal.y;
    let e0 = p.thermal.z;
    let e_cv = p.thermal.w;
    let rho0 = p.eos.x;
    let bulk_a = p.eos.y;
    let bulk_b = p.eos.z;
    let e_iv = p.eos.w;
    let eta = rho / rho0;
    let mu = eta - 1.0;
    let w = u / (e0 * eta * eta) + 1.0;
    let condensed = (a + b / w) * rho * u + bulk_a * mu + bulk_b * mu * mu;
    if eta >= 1.0 || u <= e_iv {
        return condensed;
    }
    let nu = 1.0 / eta - 1.0;
    let expanded = a * rho * u
        + (b * rho * u / w + bulk_a * mu * exp(-p.eos_decay.y * nu)) * exp(-p.eos_decay.x * nu * nu);
    if u >= e_cv {
        return expanded;
    }
    return ((u - e_iv) * expanded + (e_cv - u) * condensed) / (e_cv - e_iv);
}

// The Tillotson sound speed by finite differences, floored; `fluid_sound_speed` on the CPU. The
// step is larger than the CPU's `1e-4` because f32 carries seven digits, not sixteen.
fn tillotson_sound(p: Particle, rho: f32, u_in: f32) -> f32 {
    let u = max(u_in, 0.0);
    let step = 1.0e-3;
    let d_rho = rho * step;
    let dp_drho = (tillotson_raw(p, rho + d_rho, u) - tillotson_raw(p, rho - d_rho, u)) / (2.0 * d_rho);
    let d_u = step * (u + p.eos.w);
    let raw = tillotson_raw(p, rho, u);
    let dp_du = (tillotson_raw(p, rho, u + d_u) - raw) / d_u;
    let pressure = max(raw, 0.0);
    let c = sqrt(max(dp_drho + pressure / (rho * rho) * dp_du, 0.0));
    return max(c, params.sound_floor * sqrt(p.eos.y / p.eos.x));
}

// What a fluid neighbour adds to my density: `f·m·W` with Woolfson's factor `f = ρ0_me/ρ0_other`.
fn density_from(me: Particle, other: Particle, distance_squared: f32) -> f32 {
    if !is_fluid(other) {
        return 0.0;
    }
    let reach_sum = me.fluid.x + other.fluid.x;
    if distance_squared >= reach_sum * reach_sum {
        return 0.0;
    }
    return other.velocity.w * (me.eos.x / other.eos.x) * kernel_value(sqrt(distance_squared), 0.5 * reach_sum);
}

// The artificial viscosity's `|μ|` between two fluid neighbours (`fluid_courant_seconds`): zero
// unless they approach.
fn approach_mu(me: Particle, other: Particle, offset: vec3<f32>, distance_squared: f32) -> f32 {
    if !is_fluid(other) {
        return 0.0;
    }
    let reach_sum = me.fluid.x + other.fluid.x;
    if distance_squared >= reach_sum * reach_sum {
        return 0.0;
    }
    let approach = dot(me.velocity.xyz - other.velocity.xyz, -offset);
    if approach >= 0.0 {
        return 0.0;
    }
    let smoothing = 0.5 * reach_sum;
    return abs(smoothing * approach / (distance_squared + params.viscosity_epsilon * smoothing * smoothing));
}

// My density, pressure and sound speed from the measured density.
fn fluid_state(me: Particle, rho: f32) -> Sph {
    return Sph(vec4<f32>(rho, max(tillotson_raw(me, rho, me.spin.w), 0.0), tillotson_sound(me, rho, me.spin.w), 0.0));
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
    // Two fluids: m_other · (P_me/ρ_me² + Π/2), my heating per unit of v_me,other · ∇W_me.
    fluid_heating: f32,
}

fn pair_terms(
    me: Particle,
    other: Particle,
    offset: vec3<f32>,
    distance: f32,
    me_sph: Sph,
    other_sph: Sph,
) -> Pair {
    var out: Pair;
    out.acceleration = vec3<f32>(0.0);
    out.angular = vec3<f32>(0.0);
    out.dashpot = 0.0;
    out.friction_on_other = vec3<f32>(0.0);
    out.pressure = 0.0;
    out.gradient_me = vec3<f32>(0.0);
    out.fluid_heating = 0.0;
    let normal = offset / distance;
    out.normal = normal;
    out.arm_me = 0.0;
    out.arm_other = 0.0;
    if is_fluid(me) || is_fluid(other) {
        // `fluid_pair_forces` on the CPU. A fluid and a rubble particle feel only gravity.
        if !(is_fluid(me) && is_fluid(other)) {
            return out;
        }
        let smoothing = 0.5 * (me.fluid.x + other.fluid.x);
        let slope = kernel_slope(distance, smoothing);
        if slope >= 0.0 {
            return out;
        }
        let gradient_me = -normal * slope;
        let rho_me = me_sph.state.x;
        let rho_other = other_sph.state.x;
        let own_me = me_sph.state.y / (rho_me * rho_me);
        let own_other = other_sph.state.y / (rho_other * rho_other);
        // `v_me,other · r_me,other`, negative when they approach; r_me − r_other is −offset.
        let approach = dot(me.velocity.xyz - other.velocity.xyz, -offset);
        var viscous = 0.0;
        if approach < 0.0 {
            let mu = smoothing * approach
                / (distance * distance + params.viscosity_epsilon * smoothing * smoothing);
            let sound = 0.5 * (me_sph.state.z + other_sph.state.z);
            let rho = 0.5 * (rho_me + rho_other);
            viscous = (-params.viscosity_alpha * sound * mu + params.viscosity_beta * mu * mu) / rho;
        }
        let m_other = other.velocity.w;
        out.gradient_me = gradient_me;
        // The force on me is −m_me·m_other·(…)·∇W_me.
        out.acceleration = -gradient_me * (m_other * (own_me + own_other + viscous));
        out.fluid_heating = m_other * (own_me + 0.5 * viscous);
        return out;
    }
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
            let drag = min(friction * push, slip_mass * sliding_speed / (my_step * params.max_neighbours));
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

// The quadrupole's pull of `node`, at `to` from the particle to the node's centre of mass: the next
// term past the monopole of the multipole expansion of the potential, `Φ = −GM/r − G·rᵀQr/(2r⁵)`,
// whose gradient is `a = (GM/r⁴)·(−q·u + 2.5·(u·q·u)·u)` for `u` the unit vector to the node and `q`
// the quadrupole per unit mass. Grouped so nothing overflows or underflows f32 (`GM/r⁴` stays
// within range from a metre to `1e9 m`). Bonsai keeps the same two moments: "we use only monopole and
// quadrupole moments" (Bédorf, J., Gaburov, E. & Portegies Zwart, S. (2012), A sparse octree
// gravitational N-body code that runs entirely on the GPU processor, J. Comput. Phys. 231(7),
// 2825–2839, doi:10.1016/j.jcp.2011.12.024, section 3.2; read from arXiv:1106.1900, 2026-11-22). Unsoftened:
// only nodes far enough to be accepted get here.
fn quadrupole_from(to: vec3<f32>, distance_squared: f32, node: Node) -> vec3<f32> {
    let r = sqrt(distance_squared);
    let u = to / r;
    let d = node.quad_diag.xyz;
    let o = node.quad_off.xyz;
    let qu = vec3<f32>(
        d.x * u.x + o.x * u.y + o.y * u.z,
        o.x * u.x + d.y * u.y + o.z * u.z,
        o.y * u.x + o.z * u.y + d.z * u.z,
    );
    let inverse_squared = 1.0 / distance_squared;
    let strength = params.gravitational_constant * node.com.w * inverse_squared * inverse_squared;
    return strength * (2.5 * dot(u, qu) * u - qu);
}

fn gravity_from(me: Particle, offset: vec3<f32>, distance_squared: f32, mass: f32) -> vec3<f32> {
    let distance = sqrt(distance_squared);
    return offset
        * (params.gravitational_constant * mass / (distance * (distance_squared + params.softening_squared)));
}

// My share of the heat a pair makes, J/kg/s, at the mean velocities.
fn pair_heating(me: Particle, other: Particle, t: Pair, mean_me: Motion, mean_other: Motion) -> f32 {
    if is_fluid(me) {
        return t.fluid_heating * dot(mean_me.velocity.xyz - mean_other.velocity.xyz, t.gradient_me);
    }
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
    if !workgroup_has_work(local.x, i, in_range) {
        return;
    }
    let mine = in_range && is_active(i);
    var me: Particle;
    var me_moving: Particle;
    var me_sph: Sph;
    if in_range {
        me = particles[i];
        me_moving = moving(me, half_motion(i));
        me_sph = sph[i];
        my_step = forces_step(i);
    }
    var acceleration = vec3<f32>(0.0);
    var angular = vec3<f32>(0.0);
    var heating = 0.0;
    var nearest = 3.0e38;
    for (var base = 0u; base < params.count; base += WORKGROUP) {
        let loading = base + local.x;
        if loading < params.count {
            tile[local.x] = particles[loading];
            tile_half[local.x] = half_motion(loading);
            tile_sph[local.x] = sph[loading];
        }
        workgroupBarrier();
        if mine {
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
                    nearest = min(nearest, limit_neighbour(base + k, my_step));
                    let t = pair_terms(
                        me_moving,
                        moving(other, tile_half[k]),
                        offset,
                        sqrt(distance_squared),
                        me_sph,
                        tile_sph[k],
                    );
                    acceleration += t.acceleration;
                    angular += t.angular;
                    heating += pair_heating(
                        me,
                        other,
                        t,
                        Motion(me.velocity, me.spin),
                        Motion(other.velocity, other.spin),
                    );
                }
            }
        }
        workgroupBarrier();
    }
    if mine {
        let f = forces[i];
        forces[i].linear = vec4<f32>(f.linear.xyz + acceleration, f.linear.w + heating);
        forces[i].angular = vec4<f32>(f.angular.xyz + angular, f.angular.w);
        store_nearest(i, nearest);
    }
}

// What every particle of leaf `leaf` does to `me` (index `i`): gravity if `gravity`, neighbours'
// forces and heating if `near`.
struct Sum {
    acceleration: vec3<f32>,
    angular: vec3<f32>,
    heating: f32,
    // The least step limit of the neighbours met (`limit_neighbour`).
    nearest: f32,
}

fn leaf_forces(me: Particle, me_sph: Sph, i: u32, leaf: u32, gravity: bool, near: bool, sum: Sum) -> Sum {
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
            out.nearest = min(out.nearest, limit_neighbour(j, my_step));
            let t = pair_terms(
                moving(me, half_motion(i)),
                moving(other, half_motion(j)),
                offset,
                sqrt(distance_squared),
                me_sph,
                sph[j],
            );
            out.acceleration += t.acceleration;
            out.angular += t.angular;
            out.heating += pair_heating(
                me,
                other,
                t,
                Motion(me.velocity, me.spin),
                Motion(other.velocity, other.spin),
            );
        }
    }
    return out;
}

// ---------------------------------------------------------------------------------------------
// A tick of the individual time steps: kick-drift-kick leapfrog (Springel 2005, section 4).
//
// Each submission is one tick: `tick` → `drift` → (tree refit) → `activate` → density → forces →
// `close_kick` → `limits` → `plan` → `open_kick`. A particle's step is `2^k` ticks; it is active on
// the tick its step ends, when its forces are evaluated, its step closed with a half kick, a new
// step chosen and opened with another half kick. Between, it drifts at its half-step velocity and
// its neighbours see its velocity predicted from its last acceleration.
// ---------------------------------------------------------------------------------------------

// One thread: whether this tick runs, and the tick counter.
@compute @workgroup_size(1)
fn tick() {
    control.run = 0u;
    atomicStore(&control.reach_max, 0u);
    if control.prime == 1u {
        // The first evaluation after an upload: forces at the start, nothing moved.
        control.run = 1u;
        return;
    }
    if control.boundary == 1u || control.dt <= 0.0 {
        // Between macro steps, waiting for time to plan the next.
        return;
    }
    control.run = 1u;
    control.tick = control.tick + 1u;
    control.substeps = control.substeps + 1u;
    control.simulated = control.simulated + control.dt;
    if control.tick >= (1u << control.bins) {
        control.boundary = 1u;
    }
}

// The gap between `x` and the next f32 up in magnitude: `2^(e − 23)` for `x` in `[2^e, 2^(e+1))`. Tiny
// near zero, where any offset is representable.
fn spacing(x: f32) -> f32 {
    let exponent = bitcast<u32>(abs(x)) & 0x7f800000u;
    if exponent <= (24u << 23u) {
        return 1.0e-30;
    }
    return bitcast<f32>(exponent - (23u << 23u));
}

// Every particle moves at its half-step velocity for one tick; and where the renderer draws it.
@compute @workgroup_size(WORKGROUP)
fn drift(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() {
        return;
    }
    var p = particles[i];
    // A tick's move is often below the f32 spacing of a position far from the origin (one metre ten
    // thousand kilometres out, thirty-two at a quarter of a million), and added plainly it rounds
    // away tick after tick: a slow particle never moves. So the move accumulates in a small offset
    // beside the position (`timing.d`), and the position moves by whole multiples of its own spacing,
    // which is exact. (Kahan's compensated sum, `(x + s − x) − s`, is the textbook cure, but GPU
    // compilers fold it to zero; WGSL has no `precise` to stop them. Measured: no effect.)
    var t = timing[i];
    let offset = t.d.xyz + t.b.xyz * control.dt;
    let grain = vec3<f32>(spacing(p.position.x), spacing(p.position.y), spacing(p.position.z));
    let whole = trunc(offset / grain) * grain;
    t.d = vec4<f32>(offset - whole, 0.0);
    timing[i] = t;
    p.position = vec4<f32>(p.position.xyz + whole, p.position.w);
    // Where the renderer draws it, in world units: f32 resolves metres there, near the origin.
    let world = vec3<f32>(params.origin_x, params.origin_y, params.origin_z)
        + p.position.xyz * params.units_per_metre;
    p.eos_decay.w = world.x;
    p.fluid.z = world.y;
    p.fluid.w = world.z;
    particles[i] = p;
    atomicMax(&control.reach_max, bitcast<u32>(max(reach(p), 0.0)));
}

// Which particles' steps end this tick, and every particle's velocity, spin and internal energy
// predicted to this tick from its half-step state and its last acceleration: `v½ + a·(t − t½)`. For
// an active particle that is the end of its step with the old acceleration (the prediction GADGET-2
// gives its SPH particles: Springel 2005, section 4.3); `close_kick` replaces it with the new one.
@compute @workgroup_size(WORKGROUP)
fn activate(
    @builtin(global_invocation_id) global: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    let i = global.x;
    if i < params.count && running() && activate_one(i) {
        atomicAdd(&workgroup_active, 1u);
    }
    // The workgroup's count goes to `control.active_sum` once, not once a particle.
    workgroupBarrier();
    if local == 0u {
        let in_group = atomicLoad(&workgroup_active);
        if in_group > 0u {
            atomicAdd(&control.active_sum, in_group);
        }
    }
}

// The workgroup's active particles this tick, for `activate`.
var<workgroup> workgroup_active: atomic<u32>;

// `activate` for particle `i`: whether its step ends this tick.
fn activate_one(i: u32) -> bool {
    var p = particles[i];
    let k = kinematics[i];
    var t = timing[i];
    let ticks = t.a.y;
    let begin = t.a.z;
    let ends = control.prime == 1u || control.boundary == 1u
        || f32(control.tick) - begin >= ticks - 0.5;
    let since_half = (f32(control.tick) - begin - 0.5 * ticks) * control.dt;
    p.velocity = vec4<f32>(t.b.xyz + k.a.xyz * since_half, p.velocity.w);
    var heat = t.b.w;
    if is_hot(p) || is_fluid(p) {
        heat = max(heat + k.a.w * since_half, 0.0);
    }
    p.spin = vec4<f32>(k.c.xyz + k.b.xyz * since_half, heat);
    particles[i] = p;
    t.a.w = select(0.0, 1.0, ends);
    timing[i] = t;
    return ends;
}

// An active particle: the closing half kick of its step with the forces just evaluated, leaving its
// velocity, spin and energy at the end of the step; and, for a fluid, its smoothing length and drawn
// radius from the density measured this tick.
@compute @workgroup_size(WORKGROUP)
fn close_kick(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() || !is_active(i) {
        return;
    }
    var p = particles[i];
    var f = forces[i];
    var k = kinematics[i];
    var t = timing[i];
    // A tick the cooperative walk skipped this particle: the gravity it would have added, predicted.
    if params.gravity_interval > 1u && !gravity_evaluated(k) {
        f.linear = vec4<f32>(f.linear.xyz + predicted_gravity(k), f.linear.w);
    }
    let half = 0.5 * k.b.w;
    let velocity = t.b.xyz + f.linear.xyz * half;
    var heat = t.b.w;
    if is_hot(p) || is_fluid(p) {
        heat = max(0.0, heat + f.linear.w * half);
    }
    let spin = k.c.xyz + f.angular.xyz * half;
    p.velocity = vec4<f32>(velocity, p.velocity.w);
    p.spin = vec4<f32>(spin, heat);
    if is_fluid(p) {
        // The density this tick measured sets the smoothing length (`fluid_smoothing_m`) and the
        // radius it is drawn at (`fluid_radius_m`, never larger than at the reference density).
        let rho = sph[i].state.x;
        let mass = p.velocity.w;
        let reference = params.sph_factor * pow(mass / p.eos.x, 1.0 / 3.0);
        let smoothing = clamp(
            params.sph_factor * pow(mass / rho, 1.0 / 3.0),
            params.smoothing_min * reference,
            params.smoothing_max * reference,
        );
        let radius = pow(mass * params.packing / (4.18879020 * max(rho, p.eos.x)), 1.0 / 3.0);
        p.position = vec4<f32>(p.position.xyz, radius);
        p.fluid.x = smoothing;
        p.fluid.y = rho;
    }
    particles[i] = p;
    k.a = f.linear;
    k.b = vec4<f32>(f.angular.xyz, k.b.w);
    // The least step limit among its neighbours, for `limits` (until `open_kick` reuses `c.w`).
    k.c = vec4<f32>(spin, select(3.0e38, f.angular.w, f.angular.w > 0.0));
    kinematics[i] = k;
    t.b = vec4<f32>(velocity, heat);
    timing[i] = t;
    forces[i] = Force(vec4<f32>(0.0), vec4<f32>(0.0));
}

// An inactive particle a neighbour woke (`limit_neighbour`): its step now ends next tick, and its
// half-step velocity, spin and energy are corrected for the shorter step, as if it had been opened
// that long (Durier, F. & Dalla Vecchia, C. (2012), Implementation of feedback in smoothed particle
// hydrodynamics: towards concordance of methods, MNRAS 419(1), 465–478,
// doi:10.1111/j.1365-2966.2011.19712.x, appendix D: they re-kick with the acceleration from the
// start of the step, as here, and also move the step's start onto the hierarchy; here only its end
// moves, to the next tick). The ticks it has drifted already used the old half-step velocity, an
// error of order `a·Δt²` per wake. The trigger is Saitoh & Makino's (an active neighbour in a step
// more than `f = 4` times shorter, their section II); the correction is not theirs.
fn wake(i: u32) {
    if forces[i].angular.w >= 0.0 {
        return;
    }
    forces[i].angular.w = 0.0;
    var t = timing[i];
    var k = kinematics[i];
    let elapsed = f32(control.tick) + 1.0 - t.a.z;
    if elapsed >= t.a.y {
        return;
    }
    let step = elapsed * control.dt;
    let delta = 0.5 * (step - k.b.w);
    var heat = t.b.w;
    if is_hot(particles[i]) || is_fluid(particles[i]) {
        heat = max(0.0, heat + k.a.w * delta);
    }
    t.b = vec4<f32>(t.b.xyz + k.a.xyz * delta, heat);
    k.c = vec4<f32>(k.c.xyz + k.b.xyz * delta, k.c.w);
    k.b.w = step;
    t.a.y = elapsed;
    kinematics[i] = k;
    timing[i] = t;
}

// An active particle: its next step, the longest power of two ticks within its limit that starts on
// its own bin's grid, and the opening half kick of that step. The record keeps the end-of-step
// values, which are this tick's.
@compute @workgroup_size(WORKGROUP)
fn open_kick(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || control.dt <= 0.0 || control.boundary == 1u {
        return;
    }
    if !is_active(i) {
        wake(i);
        return;
    }
    var t = timing[i];
    let macro_ticks = 1u << control.bins;
    let wanted = u32(clamp(t.a.x / control.dt, 1.0, f32(macro_ticks)));
    // The largest power of two not above `wanted`.
    var ticks = 1u << (31u - countLeadingZeros(wanted));
    // A step must start on its own bin's grid: no longer than the lowest set bit of the tick.
    if control.tick != 0u {
        ticks = min(ticks, control.tick & (~control.tick + 1u));
    }
    let step = f32(ticks) * control.dt;
    let p = particles[i];
    var k = kinematics[i];
    let half = 0.5 * step;
    var heat = p.spin.w;
    if is_hot(p) || is_fluid(p) {
        heat = max(0.0, heat + k.a.w * half);
    }
    t.b = vec4<f32>(p.velocity.xyz + k.a.xyz * half, heat);
    k.c = vec4<f32>(p.spin.xyz + k.b.xyz * half, 0.0);
    k.b.w = step;
    kinematics[i] = k;
    t.a = vec4<f32>(t.a.x, f32(ticks), f32(control.tick), 0.0);
    timing[i] = t;
}

// Each fluid particle's density from every fluid particle (`measure_fluids`), then its pressure and
// sound speed, into `sph`.
@compute @workgroup_size(WORKGROUP)
fn density_pairs(
    @builtin(global_invocation_id) global: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let i = global.x;
    let in_range = i < params.count;
    if !workgroup_has_work(local.x, i, in_range) {
        return;
    }
    let mine = in_range && is_active(i);
    var me: Particle;
    if in_range {
        me = particles[i];
    }
    var rho = 0.0;
    var mu = 0.0;
    if mine && is_fluid(me) {
        rho = me.velocity.w * kernel_value(0.0, me.fluid.x);
    }
    for (var base = 0u; base < params.count; base += WORKGROUP) {
        let loading = base + local.x;
        if loading < params.count {
            tile[local.x] = particles[loading];
        }
        workgroupBarrier();
        if mine && is_fluid(me) {
            let span = min(WORKGROUP, params.count - base);
            for (var k = 0u; k < span; k++) {
                if base + k == i {
                    continue;
                }
                let offset = tile[k].position.xyz - me.position.xyz;
                let distance_squared = dot(offset, offset);
                rho += density_from(me, tile[k], distance_squared);
                mu = max(mu, approach_mu(me, tile[k], offset, distance_squared));
            }
        }
        workgroupBarrier();
    }
    if mine && is_fluid(me) {
        var state = fluid_state(me, rho);
        state.state.w = mu;
        sph[i] = state;
    }
}

// `density_pairs` through the tree: only the leaves whose box reaches the particle.
@compute @workgroup_size(WORKGROUP)
fn density_tree(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() || !is_active(i) {
        return;
    }
    let me = particles[i];
    if !is_fluid(me) {
        return;
    }
    let my_reach = reach(me);
    var rho = me.velocity.w * kernel_value(0.0, me.fluid.x);
    var mu = 0.0;
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
                rho += density_from(me, other, distance_squared);
                mu = max(mu, approach_mu(me, other, offset, distance_squared));
            }
            continue;
        }
        if top + 2u <= STACK {
            stack[top] = 2u * n;
            stack[top + 1u] = 2u * n + 1u;
            top += 2u;
        }
    }
    var state = fluid_state(me, rho);
    state.state.w = mu;
    sph[i] = state;
}

@compute @workgroup_size(WORKGROUP)
fn refit_leaves(@builtin(global_invocation_id) global: vec3<u32>) {
    let leaf = global.x;
    if leaf >= params.leaves || !running() {
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
    // A third pass for the quadrupole about it.
    var diag = vec3<f32>(0.0);
    var off = vec3<f32>(0.0);
    for (var j = first; j < last; j++) {
        let p = particles[j];
        let x = p.position.xyz - weighted;
        let w = p.velocity.w / mass;
        diag += w * (3.0 * x * x - vec3<f32>(dot(x, x)));
        off += w * 3.0 * vec3<f32>(x.x * x.y, x.x * x.z, x.y * x.z);
    }
    node.quad_diag = vec4<f32>(diag, 0.0);
    node.quad_off = vec4<f32>(off, 0.0);
    nodes[params.leaves + leaf] = node;
}

@compute @workgroup_size(WORKGROUP)
fn refit_level(@builtin(global_invocation_id) global: vec3<u32>) {
    if global.x >= level.count || !running() {
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
        // The children's quadrupoles moved to the new centre (the parallel-axis theorem).
        let wl = left.com.w / mass;
        let wr = right.com.w / mass;
        let dl = left.com.xyz - centre;
        let dr = right.com.xyz - centre;
        let shift_diag = wl * (3.0 * dl * dl - vec3<f32>(dot(dl, dl)))
            + wr * (3.0 * dr * dr - vec3<f32>(dot(dr, dr)));
        let shift_off = wl * 3.0 * vec3<f32>(dl.x * dl.y, dl.x * dl.z, dl.y * dl.z)
            + wr * 3.0 * vec3<f32>(dr.x * dr.y, dr.x * dr.z, dr.y * dr.z);
        node.quad_diag = vec4<f32>(wl * left.quad_diag.xyz + wr * right.quad_diag.xyz + shift_diag, 0.0);
        node.quad_off = vec4<f32>(wl * left.quad_off.xyz + wr * right.quad_off.xyz + shift_off, 0.0);
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
    if i >= params.count || !running() || !is_active(i) {
        return;
    }
    let me = particles[i];
    let me_sph = sph[i];
    my_step = forces_step(i);
    let my_reach = reach(me);
    let gravity = has(GRAVITY_TREE) && !has(GRAVITY_GROUP);
    let near = has(NEIGHBOURS_TREE);
    let theta_squared = params.opening_angle * params.opening_angle;
    var sum = Sum(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, 3.0e38);
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
                sum.acceleration += gravity_from(me, to, distance_squared, node.com.w)
                    + quadrupole_from(to, distance_squared, node);
                continue;
            }
        }
        if n >= params.leaves {
            // Near enough to open, or overlapping: every particle of the leaf, one by one. A leaf
            // that does not overlap is only here for its gravity.
            sum = leaf_forces(me, me_sph, i, n - params.leaves, gravity, near && overlaps, sum);
            continue;
        }
        if top + 2u <= STACK {
            stack[top] = 2u * n;
            stack[top + 1u] = 2u * n + 1u;
            top += 2u;
        }
    }
    let f = forces[i];
    forces[i].linear = vec4<f32>(f.linear.xyz + sum.acceleration, f.linear.w + sum.heating);
    forces[i].angular = vec4<f32>(f.angular.xyz + sum.angular, f.angular.w);
    store_nearest(i, sum.nearest);
}

// ---------------------------------------------------------------------------------------------
// The GPU's own timing: each macro step and its bins chosen from the particles, on the GPU.
// ---------------------------------------------------------------------------------------------

// The start of an advance: add the frame's time to what is left to plan (time the GPU could not get
// through is carried over, up to two frames' worth in all), and count ticks from zero. A macro step
// in progress carries on across advances.
@compute @workgroup_size(1)
fn begin() {
    // An advance that adds no time (a blocking step's later rounds) leaves what is left alone.
    if params.frame_dt > 0.0 {
        if control.fixed_dt > 0.0 {
            control.time_left = params.frame_dt;
        } else {
            control.time_left = min(control.time_left + params.frame_dt, 2.0 * params.frame_dt);
        }
    }
    control.substeps = 0u;
    atomicStore(&control.active_sum, 0u);
}

// An active particle's step limit (`collision::resolve::substep_count` on the CPU): it moves at
// most `displacement_cap`; a pair of it orbits in at least `1/orbital_fraction` steps; a rubble
// contact lasts `contact_substeps` steps; a fluid particle obeys Monaghan's Courant condition. At a
// macro step's end, when every particle is active, the shortest and longest go to `plan`.
@compute @workgroup_size(WORKGROUP)
fn limits(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() || !is_active(i) {
        return;
    }
    step_limit(i);
}

// `limits` before the forces of the first evaluation after an upload, which has no step yet: the
// friction's drag cap needs the step its force will act over (the shortest limit: `forces_step`),
// and the limiter needs the neighbours' limits. Without it the first half kick used uncapped
// friction (measured: 0.6% of the binding energy gained an hour, re-uploading every minute).
@compute @workgroup_size(WORKGROUP)
fn limits_first(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || control.prime == 0u {
        return;
    }
    step_limit(i);
}

fn step_limit(i: u32) {
    let p = particles[i];
    let mass = p.velocity.w;
    let r = p.position.w;
    var limit = 3.0e38;
    let speed = length(p.velocity.xyz);
    if speed > 0.0 {
        limit = min(limit, params.displacement_cap / speed);
    }
    if mass > 0.0 && r > 0.0 {
        let d = 2.0 * r;
        // √((2r)³ / (G·2m)), grouped so nothing overflows.
        let dynamical = d * sqrt(d / (params.gravitational_constant * (2.0 * mass)));
        limit = min(limit, params.orbital_fraction * dynamical);
    }
    if p.material.x > 0.0 && r > 0.0 {
        // A particle against a copy of itself: stiffness E·r/4, reduced mass m/2.
        let stiffness = p.material.x * (0.25 * r);
        let duration = PI * sqrt((0.5 * mass) / stiffness);
        limit = min(limit, duration / params.contact_substeps);
    }
    if is_fluid(p) {
        let s = sph[i].state;
        let signal = s.z * (1.0 + 1.2 * params.viscosity_alpha) + 1.2 * params.viscosity_beta * s.w;
        limit = min(limit, params.courant * p.fluid.x / signal);
    }
    // The passive half of the limiter (Saitoh & Makino 2009): within a ratio of its neighbours'.
    limit = min(limit, params.neighbour_ratio * kinematics[i].c.w);
    limit = clamp(limit, 1.0e-30, 3.0e38);
    timing[i].a.x = limit;
    if control.boundary == 1u || control.prime == 1u {
        atomicMin(&control.limit, bitcast<u32>(limit));
        atomicMax(&control.limit_max, bitcast<u32>(limit));
    }
}

// One thread, every tick: at a macro step's end (or the first evaluation after an upload), the next
// macro step from what is left to plan. The tick is the shortest limit over `2^headroom`, so a
// particle whose limit shrinks during the step still has a bin; the bins span up to the longest
// limit, at most `max_bins`, and no further than the time left. Nothing left: no tick runs until
// the next advance adds time.
@compute @workgroup_size(1)
fn plan() {
    if control.prime == 0u && control.boundary == 0u {
        return;
    }
    let max_bins = params.max_bins;
    control.prime = 0u;
    if control.time_left <= 0.0 {
        control.dt = 0.0;
        control.boundary = 1u;
        return;
    }
    var dt: f32;
    var bins = 0u;
    if control.fixed_dt > 0.0 {
        dt = min(control.fixed_dt, control.time_left);
    } else {
        let shortest = bitcast<f32>(atomicLoad(&control.limit));
        let longest = bitcast<f32>(atomicLoad(&control.limit_max));
        let headroom = min(params.bin_headroom, max_bins);
        // floor(log2(longest / shortest)), within the bins left above the headroom.
        while bins + headroom < max_bins && shortest * f32(1u << (bins + 1u)) <= longest {
            bins += 1u;
        }
        bins += headroom;
        dt = shortest / f32(1u << headroom);
        while bins > 0u && dt * f32(1u << bins) > control.time_left {
            bins -= 1u;
        }
        dt = min(dt, control.time_left);
    }
    let span = dt * f32(1u << bins);
    if span >= control.time_left {
        control.time_left = 0.0;
    } else {
        control.time_left = control.time_left - span;
    }
    control.dt = dt;
    control.bins = bins;
    control.tick = 0u;
    control.boundary = 0u;
    atomicStore(&control.limit, 0x7f7fffffu);
    atomicStore(&control.limit_max, 0u);
}

// ---------------------------------------------------------------------------------------------
// The cell list: a neighbour search from a uniform grid rebuilt every tick, O(N).
//
// Cells are the largest reach, so every neighbour of a particle (nearer than the sum of the two
// reaches) is within two cells: the 125 around it. (Cells of twice the reach and the 27 around
// visit `216` reach-cubed of space for a neighbourhood of `34`; these visit `125`. Measured on an
// RTX 4070 SUPER: `63` against `70 ms` a frame at `20,000` particles a planet.) Cells are hashed into `grid_mask + 1` buckets (Teschner, M., Heidelberger, B.,
// Müller, M., Pomeranets, D. & Gross, M. (2003), Optimized spatial hashing for collision detection
// of deformable objects, Proc. Vision, Modeling, and Visualization 2003, 47–54. The three primes
// are the ones commonly attributed to that paper; its text could not be extracted to check them), so the grid needs no bounds and debris can fly anywhere. Built by a counting sort: count
// each bucket, an exclusive prefix sum over the counts (`scan_up`, `scan_down`), then scatter each
// particle's index into its bucket's run of `grid_items`.
// ---------------------------------------------------------------------------------------------

var<workgroup> scan_tile: array<u32, WORKGROUP>;

// The side of a grid cell this tick, m: the largest reach, from `drift`.
fn cell_size() -> f32 {
    return max(bitcast<f32>(atomicLoad(&control.reach_max)), 1.0e-30);
}

fn cell_of(position: vec3<f32>, size: f32) -> vec3<i32> {
    return vec3<i32>(floor(position / size));
}

fn bucket_of(cell: vec3<i32>) -> u32 {
    let c = bitcast<vec3<u32>>(cell);
    return ((c.x * 73856093u) ^ (c.y * 19349663u) ^ (c.z * 83492791u)) & params.grid_mask;
}

// Every bucket's count to zero, and the sentinel past the last.
@compute @workgroup_size(WORKGROUP)
fn grid_clear(@builtin(global_invocation_id) global: vec3<u32>) {
    let b = global.x;
    if b > params.grid_mask + 1u || !running() {
        return;
    }
    atomicStore(&grid[b], 0u);
}

@compute @workgroup_size(WORKGROUP)
fn grid_count(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() {
        return;
    }
    let b = bucket_of(cell_of(particles[i].position.xyz, cell_size()));
    atomicAdd(&grid[b], 1u);
}

// One level of the prefix sum, upward: each workgroup scans 256 values of the segment
// `level.first .. + level.count` in place (exclusive), and writes their total just past the
// segment, where the next level scans the totals.
@compute @workgroup_size(WORKGROUP)
fn scan_up(
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    if !running_uniform(local.x) {
        return;
    }
    let base = level.first + group.x * 4u * WORKGROUP + local.x * 4u;
    let a = atomicLoad(&grid[base]);
    let b = atomicLoad(&grid[base + 1u]);
    let c = atomicLoad(&grid[base + 2u]);
    let d = atomicLoad(&grid[base + 3u]);
    let mine = a + b + c + d;
    scan_tile[local.x] = mine;
    workgroupBarrier();
    // Hillis–Steele inclusive scan of the 64 threads' sums.
    for (var stride = 1u; stride < WORKGROUP; stride *= 2u) {
        var add = 0u;
        if local.x >= stride {
            add = scan_tile[local.x - stride];
        }
        workgroupBarrier();
        scan_tile[local.x] += add;
        workgroupBarrier();
    }
    let before = scan_tile[local.x] - mine;
    atomicStore(&grid[base], before);
    atomicStore(&grid[base + 1u], before + a);
    atomicStore(&grid[base + 2u], before + a + b);
    atomicStore(&grid[base + 3u], before + a + b + c);
    if local.x == WORKGROUP - 1u {
        atomicStore(&grid[level.first + level.count + group.x], scan_tile[local.x]);
    }
}

// One level of the prefix sum, downward: each block of 256 adds its block's offset, which the level
// above has by now scanned.
@compute @workgroup_size(WORKGROUP)
fn scan_down(
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    if !running() {
        return;
    }
    let offset = atomicLoad(&grid[level.first + level.count + group.x]);
    let base = level.first + group.x * 4u * WORKGROUP + local.x * 4u;
    for (var k = 0u; k < 4u; k++) {
        atomicAdd(&grid[base + k], offset);
    }
}

// Each bucket's write cursor at its start.
@compute @workgroup_size(WORKGROUP)
fn grid_cursor(@builtin(global_invocation_id) global: vec3<u32>) {
    let b = global.x;
    if b > params.grid_mask || !running() {
        return;
    }
    atomicStore(&grid_cursor_at[b], atomicLoad(&grid[b]));
}

@compute @workgroup_size(WORKGROUP)
fn grid_scatter(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() {
        return;
    }
    let b = bucket_of(cell_of(particles[i].position.xyz, cell_size()));
    let slot = atomicAdd(&grid_cursor_at[b], 1u);
    grid_items[slot] = i;
}

// `density_pairs` from the cell list: only the particles in the 125 cells around.
@compute @workgroup_size(WORKGROUP)
fn density_grid(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() || !is_active(i) {
        return;
    }
    let me = particles[i];
    if !is_fluid(me) {
        return;
    }
    let size = cell_size();
    let home = cell_of(me.position.xyz, size);
    var rho = me.velocity.w * kernel_value(0.0, me.fluid.x);
    var mu = 0.0;
    for (var dz = -2; dz <= 2; dz++) {
        for (var dy = -2; dy <= 2; dy++) {
            for (var dx = -2; dx <= 2; dx++) {
                let cell = home + vec3<i32>(dx, dy, dz);
                let b = bucket_of(cell);
                let end = atomicLoad(&grid[b + 1u]);
                for (var k = atomicLoad(&grid[b]); k < end; k++) {
                    let j = grid_items[k];
                    if j == i {
                        continue;
                    }
                    let other = particles[j];
                    // A bucket holds every cell that hashes to it: only this cell's particles.
                    if any(cell_of(other.position.xyz, size) != cell) {
                        continue;
                    }
                    let offset = other.position.xyz - me.position.xyz;
                    let distance_squared = dot(offset, offset);
                    rho += density_from(me, other, distance_squared);
                    mu = max(mu, approach_mu(me, other, offset, distance_squared));
                }
            }
        }
    }
    var state = fluid_state(me, rho);
    state.state.w = mu;
    sph[i] = state;
}

// The neighbours' forces and heating from the cell list; the gravity comes from `forces_pairs` or
// `forces_tree`.
@compute @workgroup_size(WORKGROUP)
fn forces_grid(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count || !running() || !is_active(i) {
        return;
    }
    let me = particles[i];
    let me_moving = moving(me, half_motion(i));
    let me_sph = sph[i];
    my_step = forces_step(i);
    let size = cell_size();
    let home = cell_of(me.position.xyz, size);
    var acceleration = vec3<f32>(0.0);
    var angular = vec3<f32>(0.0);
    var heating = 0.0;
    var nearest = 3.0e38;
    for (var dz = -2; dz <= 2; dz++) {
        for (var dy = -2; dy <= 2; dy++) {
            for (var dx = -2; dx <= 2; dx++) {
                let cell = home + vec3<i32>(dx, dy, dz);
                let b = bucket_of(cell);
                let end = atomicLoad(&grid[b + 1u]);
                for (var k = atomicLoad(&grid[b]); k < end; k++) {
                    let j = grid_items[k];
                    if j == i {
                        continue;
                    }
                    let other = particles[j];
                    if any(cell_of(other.position.xyz, size) != cell) {
                        continue;
                    }
                    let offset = other.position.xyz - me.position.xyz;
                    let distance_squared = dot(offset, offset);
                    if distance_squared <= 0.0 || !neighbours(me, other, distance_squared) {
                        continue;
                    }
                    nearest = min(nearest, limit_neighbour(j, my_step));
                    let t = pair_terms(
                        me_moving,
                        moving(other, half_motion(j)),
                        offset,
                        sqrt(distance_squared),
                        me_sph,
                        sph[j],
                    );
                    acceleration += t.acceleration;
                    angular += t.angular;
                    heating += pair_heating(
                        me,
                        other,
                        t,
                        Motion(me.velocity, me.spin),
                        Motion(other.velocity, other.spin),
                    );
                }
            }
        }
    }
    let f = forces[i];
    forces[i].linear = vec4<f32>(f.linear.xyz + acceleration, f.linear.w + heating);
    forces[i].angular = vec4<f32>(f.angular.xyz + angular, f.angular.w);
    store_nearest(i, nearest);
}

// ---------------------------------------------------------------------------------------------
// The cooperative tree walk: Barnes–Hut gravity for a workgroup at once.
//
// The particles are in Morton order, so a workgroup's 64 are near one another. Its threads walk the
// tree together, a level at a time, testing each node against the box around all 64 particles
// (Barnes–Hut's opening angle, from the nearest point of the box: if a node is far enough for the
// box it is far enough for every particle in it). Far nodes go into a shared list, leaves to open
// into another, and the nodes to open into the next level; then every thread sums both lists for its
// own particle. The walk and its memory reads are shared instead of repeated by each thread, as in
// Bonsai (Bédorf, J., Gaburov, E. & Portegies Zwart, S. (2012), J. Comput. Phys. 231(7), 2825–2839,
// doi:10.1016/j.jcp.2011.12.024): "we do not traverse a single particle, but rather a group of
// particles" of up to 64, the interaction lists filled by the group's threads (section 2.2), a cell
// opened for the whole group by its smallest distance to the group (section 3.3, eq. 1).
// A level too wide for the shared frontier sends the workgroup to the walk each thread does alone.
// ---------------------------------------------------------------------------------------------

const GROUP_FAR: u32 = 1024u;
const GROUP_LEAVES: u32 = 256u;
const GROUP_FRONTIER: u32 = 512u;

var<workgroup> group_far: array<u32, GROUP_FAR>;
var<workgroup> group_leaves: array<u32, GROUP_LEAVES>;
var<workgroup> group_frontier: array<u32, GROUP_FRONTIER>;
var<workgroup> group_next: array<u32, GROUP_FRONTIER>;
var<workgroup> group_far_count: atomic<u32>;
var<workgroup> group_leaf_count: atomic<u32>;
var<workgroup> group_next_count: atomic<u32>;
// Plain copies of the counts, written by thread 0, read uniformly.
var<workgroup> group_frontier_size: u32;
var<workgroup> group_decision: u32;
var<workgroup> group_box_lo: array<vec3<f32>, WORKGROUP>;
var<workgroup> group_box_hi: array<vec3<f32>, WORKGROUP>;

const DECIDE_FLUSH_FAR: u32 = 1u;
const DECIDE_FLUSH_LEAVES: u32 = 2u;
const DECIDE_OVERFLOW: u32 = 4u;

// The pull of the far nodes listed so far on `me`.
fn sum_far(me: Particle, count: u32) -> vec3<f32> {
    var acceleration = vec3<f32>(0.0);
    for (var k = 0u; k < count; k++) {
        let node = nodes[group_far[k]];
        let to = node.com.xyz - me.position.xyz;
        let distance_squared = dot(to, to);
        acceleration += gravity_from(me, to, distance_squared, node.com.w)
            + quadrupole_from(to, distance_squared, node);
    }
    return acceleration;
}

// The pull of every particle of the leaves listed so far on `me` (index `i`).
fn sum_leaves(me: Particle, i: u32, count: u32) -> vec3<f32> {
    var acceleration = vec3<f32>(0.0);
    for (var k = 0u; k < count; k++) {
        let first = (group_leaves[k] - params.leaves) * params.leaf_size;
        let last = min(first + params.leaf_size, params.count);
        for (var j = first; j < last; j++) {
            if j == i {
                continue;
            }
            let other = particles[j];
            let offset = other.position.xyz - me.position.xyz;
            let distance_squared = dot(offset, offset);
            if distance_squared > 0.0 {
                acceleration += gravity_from(me, offset, distance_squared, other.velocity.w);
            }
        }
    }
    return acceleration;
}

// Barnes–Hut gravity on `me` (index `i`), walked by this thread alone: the fallback.
fn gravity_walk(me: Particle, i: u32) -> vec3<f32> {
    let theta_squared = params.opening_angle * params.opening_angle;
    var acceleration = vec3<f32>(0.0);
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
        let extent = node.centre_hi.xyz - node.centre_lo.xyz;
        let size = max(extent.x, max(extent.y, extent.z));
        let to = node.com.xyz - me.position.xyz;
        let distance_squared = dot(to, to);
        if size * size < theta_squared * distance_squared {
            acceleration += gravity_from(me, to, distance_squared, node.com.w)
                + quadrupole_from(to, distance_squared, node);
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
                let d2 = dot(offset, offset);
                if d2 > 0.0 {
                    acceleration += gravity_from(me, offset, d2, other.velocity.w);
                }
            }
            continue;
        }
        if top + 2u <= STACK {
            stack[top] = 2u * n;
            stack[top + 1u] = 2u * n + 1u;
            top += 2u;
        }
    }
    return acceleration;
}

@compute @workgroup_size(WORKGROUP)
fn gravity_group(
    @builtin(global_invocation_id) global: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let i = global.x;
    let in_range = i < params.count;
    // When no active particle of the workgroup is due the whole workgroup returns: `close_kick`
    // predicts their gravity instead.
    if !workgroup_has_work(local.x, i, in_range && gravity_due(i)) {
        return;
    }
    let mine = in_range && is_active(i);
    var me: Particle;
    if in_range {
        me = particles[i];
        group_box_lo[local.x] = me.position.xyz;
        group_box_hi[local.x] = me.position.xyz;
    } else {
        group_box_lo[local.x] = vec3<f32>(EMPTY);
        group_box_hi[local.x] = vec3<f32>(-EMPTY);
    }
    workgroupBarrier();
    // The box around the workgroup's particles, by a tree reduction.
    for (var stride = WORKGROUP / 2u; stride > 0u; stride /= 2u) {
        if local.x < stride {
            group_box_lo[local.x] = min(group_box_lo[local.x], group_box_lo[local.x + stride]);
            group_box_hi[local.x] = max(group_box_hi[local.x], group_box_hi[local.x + stride]);
        }
        workgroupBarrier();
    }
    let lo = group_box_lo[0];
    let hi = group_box_hi[0];
    if local.x == 0u {
        group_frontier[0] = 1u;
        group_frontier_size = 1u;
        atomicStore(&group_far_count, 0u);
        atomicStore(&group_leaf_count, 0u);
    }
    let theta_squared = params.opening_angle * params.opening_angle;
    var acceleration = vec3<f32>(0.0);
    var overflow = false;
    loop {
        let size = workgroupUniformLoad(&group_frontier_size);
        if size == 0u {
            break;
        }
        // Make room before the level adds to the lists: each node adds at most one far node, one
        // leaf, or two nodes to the next level.
        if local.x == 0u {
            var decision = 0u;
            if atomicLoad(&group_far_count) + size > GROUP_FAR {
                decision |= DECIDE_FLUSH_FAR;
            }
            if atomicLoad(&group_leaf_count) + size > GROUP_LEAVES {
                decision |= DECIDE_FLUSH_LEAVES;
            }
            if 2u * size > GROUP_FRONTIER {
                decision |= DECIDE_OVERFLOW;
            }
            group_decision = decision;
        }
        let decision = workgroupUniformLoad(&group_decision);
        if (decision & DECIDE_OVERFLOW) != 0u {
            overflow = true;
            break;
        }
        if (decision & DECIDE_FLUSH_FAR) != 0u {
            if mine {
                acceleration += sum_far(me, min(atomicLoad(&group_far_count), GROUP_FAR));
            }
            workgroupBarrier();
            if local.x == 0u {
                atomicStore(&group_far_count, 0u);
            }
        }
        if (decision & DECIDE_FLUSH_LEAVES) != 0u {
            if mine {
                acceleration += sum_leaves(me, i, min(atomicLoad(&group_leaf_count), GROUP_LEAVES));
            }
            workgroupBarrier();
            if local.x == 0u {
                atomicStore(&group_leaf_count, 0u);
            }
        }
        if local.x == 0u {
            atomicStore(&group_next_count, 0u);
        }
        workgroupBarrier();
        for (var k = local.x; k < size; k += WORKGROUP) {
            let n = group_frontier[k];
            let node = nodes[n];
            if node.com.w <= 0.0 {
                continue;
            }
            let extent = node.centre_hi.xyz - node.centre_lo.xyz;
            let side = max(extent.x, max(extent.y, extent.z));
            let gap = node.com.xyz - clamp(node.com.xyz, lo, hi);
            if side * side < theta_squared * dot(gap, gap) {
                group_far[atomicAdd(&group_far_count, 1u)] = n;
            } else if n >= params.leaves {
                group_leaves[atomicAdd(&group_leaf_count, 1u)] = n;
            } else {
                let slot = atomicAdd(&group_next_count, 2u);
                group_next[slot] = 2u * n;
                group_next[slot + 1u] = 2u * n + 1u;
            }
        }
        workgroupBarrier();
        let next = atomicLoad(&group_next_count);
        for (var k = local.x; k < next; k += WORKGROUP) {
            group_frontier[k] = group_next[k];
        }
        if local.x == 0u {
            group_frontier_size = next;
        }
        workgroupBarrier();
    }
    if mine {
        if overflow {
            acceleration = gravity_walk(me, i);
        } else {
            acceleration += sum_far(me, min(atomicLoad(&group_far_count), GROUP_FAR));
            acceleration += sum_leaves(me, i, min(atomicLoad(&group_leaf_count), GROUP_LEAVES));
        }
        let f = forces[i];
        forces[i].linear = vec4<f32>(f.linear.xyz + acceleration, f.linear.w);
        remember_gravity(i, acceleration);
    }
}

// ---------------------------------------------------------------------------------------------
// The sort: particles along a Morton curve, on the GPU, so the tree over them groups neighbours.
// ---------------------------------------------------------------------------------------------

// An f32 as a u32 that orders as the float does, for atomic min and max.
fn ordered(value: f32) -> u32 {
    let bits = bitcast<u32>(value);
    if (bits & 0x80000000u) != 0u {
        return ~bits;
    }
    return bits | 0x80000000u;
}

fn unordered(bits: u32) -> f32 {
    if (bits & 0x80000000u) != 0u {
        return bitcast<f32>(bits & 0x7fffffffu);
    }
    return bitcast<f32>(~bits);
}

@compute @workgroup_size(1)
fn bounds_reset() {
    atomicStore(&control.lo_x, 0xffffffffu);
    atomicStore(&control.lo_y, 0xffffffffu);
    atomicStore(&control.lo_z, 0xffffffffu);
    atomicStore(&control.hi_x, 0u);
    atomicStore(&control.hi_y, 0u);
    atomicStore(&control.hi_z, 0u);
}

@compute @workgroup_size(WORKGROUP)
fn bounds(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    let p = particles[i].position;
    atomicMin(&control.lo_x, ordered(p.x));
    atomicMin(&control.lo_y, ordered(p.y));
    atomicMin(&control.lo_z, ordered(p.z));
    atomicMax(&control.hi_x, ordered(p.x));
    atomicMax(&control.hi_y, ordered(p.y));
    atomicMax(&control.hi_z, ordered(p.z));
}

// The lowest 10 bits of `v`, spread to every third bit.
fn spread(v: u32) -> u32 {
    var x = v & 0x3ffu;
    x = (x | (x << 16u)) & 0x030000ffu;
    x = (x | (x << 8u)) & 0x0300f00fu;
    x = (x | (x << 4u)) & 0x030c30c3u;
    x = (x | (x << 2u)) & 0x09249249u;
    return x;
}

// Each particle's Morton key within the bounding box, 10 bits an axis; the padding past the last
// particle sorts to the end.
@compute @workgroup_size(WORKGROUP)
fn morton(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= arrayLength(&keys) {
        return;
    }
    if i >= params.count {
        keys[i] = vec2<u32>(0xffffffffu, i);
        return;
    }
    let lo = vec3<f32>(
        unordered(atomicLoad(&control.lo_x)),
        unordered(atomicLoad(&control.lo_y)),
        unordered(atomicLoad(&control.lo_z)),
    );
    let hi = vec3<f32>(
        unordered(atomicLoad(&control.hi_x)),
        unordered(atomicLoad(&control.hi_y)),
        unordered(atomicLoad(&control.hi_z)),
    );
    let span = max(max(hi.x - lo.x, hi.y - lo.y), max(hi.z - lo.z, 1.0e-30));
    let cell = clamp((particles[i].position.xyz - lo) / span * 1023.0, vec3<f32>(0.0), vec3<f32>(1023.0));
    let c = vec3<u32>(cell);
    keys[i] = vec2<u32>(spread(c.x) | (spread(c.y) << 1u) | (spread(c.z) << 2u), i);
}

// One compare-and-swap stage of a bitonic sort (Batcher 1968): `level.first` is the partner
// distance `j`, `level.count` the sequence length `k`.
@compute @workgroup_size(WORKGROUP)
fn bitonic(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    let j = level.first;
    let k = level.count;
    let partner = i ^ j;
    if i >= arrayLength(&keys) || partner <= i {
        return;
    }
    let a = keys[i];
    let b = keys[partner];
    let ascending = (i & k) == 0u;
    let out_of_order = (a.x > b.x) || (a.x == b.x && a.y > b.y);
    if out_of_order == ascending {
        keys[i] = b;
        keys[partner] = a;
    }
}

// The particles into their sorted order, in `scratch`, and their integrator state with them, in
// `slot_scratch` (kinematics, then timing); the host copies both back.
@compute @workgroup_size(WORKGROUP)
fn gather(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    let source = keys[i].y;
    scratch[i] = particles[source];
    slot_scratch[i] = kinematics[source];
    // The fluid state rides in `timing`'s unused `c`: an inactive particle's pressure is read by its
    // active neighbours until it is next measured.
    var t = timing[source];
    t.c = sph[source].state;
    slot_scratch[params.count + i] = t;
}

// After the sort's copy back: each particle's fluid state from where `gather` carried it.
@compute @workgroup_size(WORKGROUP)
fn restore_sph(@builtin(global_invocation_id) global: vec3<u32>) {
    let i = global.x;
    if i >= params.count {
        return;
    }
    sph[i].state = timing[i].c;
}
