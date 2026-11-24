//! Stepping a scene made of particles on the GPU.
//!
//! [`GpuParticles`] does what `Scene::step` does, for a scene whose every prop is a particle in
//! simulated contact ([`can_step_on_gpu`]): gravity, the contact force, friction and spin, the
//! thermal pressure of hot rock, the heat the contacts make, and the symplectic Euler kick and
//! drift, for every substep of a frame. The kernels are in `particles.wgsl`. The CPU's substep
//! count, laws and order are reproduced exactly, so the two paths are the same simulation at
//! different precision; the tests hold them together.
//!
//! # Algorithms
//!
//! How the GPU finds what acts on each particle is chosen at run time ([`GpuSolver`], set from the
//! menu), separately for the two jobs:
//!
//! | | Exact, O(N²) | Tree, O(N log N) |
//! | --- | --- | --- |
//! | **Gravity** ([`GravitySolver`]) | `Direct`: every pair, a tile at a time through workgroup memory | `BarnesHut`: distant groups as one point mass, within the opening angle `θ` |
//! | **Neighbours** ([`NeighbourSearch`]) | `AllPairs`: every pair tested for contact and pressure | `Tree`: only the particles whose tree nodes reach this one |
//!
//! Both trees are the same tree. Each frame the particles are sorted along a Morton (Z-order) curve
//! on the CPU, which puts particles that are near each other next to each other, and an implicit
//! binary tree is laid over that order: node `n` has children `2n` and `2n + 1`, and leaf `L + l` holds
//! the `l`-th bucket of `GPU_LEAF_SIZE` particles. Every substep the GPU refits it, one level per
//! dispatch from the leaves up: each
//! node's mass, centre of mass, the box around its particles' centres (for Barnes–Hut) and the box
//! around their reach (for the neighbour search). The shape stays fixed for
//! the frame, which is all the particles move between sorts; the boxes are always exact.
//!
//! # One upload, many substeps, one read
//!
//! A frame's substeps run on the GPU back to back. The scene is uploaded once at the start of the
//! frame and read back once at the end, so the traffic is two copies of the particles per frame
//! however many substeps there are. The read blocks until the GPU is done.
//!
//! # Precision: `f32`, relative to the particles
//!
//! GPUs are fast at `f32` and slow or unable at `f64`, and the scene keeps `f64` because an `f32`
//! position `150` units from the origin resolves only `15 km`. So positions are uploaded relative
//! to the particles' own centre, in metres: across the `1e8 m` a giant impact spans, `f32` resolves
//! about `6 m`, against particles hundreds of kilometres across that overlap by kilometres. The
//! scene's `f64` state is the master copy, re-based every frame, so the rounding never accumulates
//! across frames, only within one.
//!
//! Two runs, CPU and GPU, of a chaotic system part ways over time, as two CPU runs at different
//! step sizes would. Over a short run they agree closely; over a whole impact they agree on what
//! happens, which is what the tests check.
//!
//! # What it does not do
//!
//! - **Scenes with anything but particles** stay on the CPU: a planet that is a single sphere, a
//!   spacecraft, a black hole.
//! - **Higher multipoles.** The tree's groups pull as point masses (monopoles); a quadrupole term
//!   would allow a larger `θ` for the same error.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use bevy::render::render_resource::Buffer;

use crate::collision::collision::{contact_is_simulated, fluid_smoothing_m};
use crate::collision::constants::{
    MAX_SUBSTEPS, MAX_SUBSTEP_DISPLACEMENT_UNITS, MAX_SUBSTEP_DYNAMICAL_FRACTION,
    SOFT_CONTACT_MAX_DAMPING_RATIO, SOFT_CONTACT_MAX_NEIGHBOURS, SOFT_CONTACT_SUBSTEPS,
    SPH_COURANT, SPH_MAX_SMOOTHING_FRACTION, SPH_MIN_SMOOTHING_FRACTION, SPH_SMOOTHING_FACTOR,
    SPH_SOUND_SPEED_FLOOR_FRACTION, SPH_VISCOSITY_ALPHA, SPH_VISCOSITY_BETA, SPH_VISCOSITY_EPSILON,
};
use crate::compute::constants::{
    DEFAULT_OPENING_ANGLE, GPU_FRAME_TIME_TARGET_SECONDS, GPU_LEAF_SIZE, GPU_MAX_TIME_BINS,
    GPU_MIN_SUBSTEP_BUDGET, GPU_NEIGHBOUR_STEP_RATIO, GPU_READBACK_INTERVAL_FRAMES,
    GPU_TIME_BIN_HEADROOM, GPU_WORKGROUP_SIZE, MAX_OPENING_ANGLE, MIN_OPENING_ANGLE,
};
use crate::compute::device::GpuDevice;
use crate::compute::profiler::{GpuPhase, GpuProfile, GpuProfiler};
use crate::constants::METRES_PER_UNIT;
use crate::core::integrator::GravityModel;
use crate::core::scene::{Body, DVec3, Scene};
use crate::physics_utils::physics_constants::{FCC_PACKING_FRACTION, GRAVITATIONAL_CONSTANT};
use crate::physics_utils::soft_contact::damping_ratio_for_restitution;
use crate::time::simulation::TimeStep;

/// The kernels, compiled into the binary.
pub const PARTICLES_SHADER: &str = include_str!("particles.wgsl");

/// `f32`s per particle in the buffers: position and radius, velocity and mass, spin and heat,
/// material, thermal, the equation of state's cold and decay parameters, and the fluid state.
const FLOATS_PER_PARTICLE: usize = 32;

/// Bytes per particle, per force record, per integrator slot (four vec4s; `Slot` in the shader), and
/// per tree node.
const BYTES_PER_PARTICLE: u64 = (FLOATS_PER_PARTICLE * 4) as u64;
const BYTES_PER_FORCE: u64 = 32;
const BYTES_PER_SLOT: u64 = 64;
const BYTES_PER_NODE: u64 = 112;
/// Bytes per fluid state (density, pressure, sound speed, largest viscosity `μ`).
const BYTES_PER_SPH: u64 = 16;
/// Bytes per sort entry: a Morton key and a particle index.
const BYTES_PER_KEY: u64 = 8;
/// Bytes of the `Params` uniform; must match `particles.wgsl`.
const PARAMS_BYTES: u64 = 128;
/// Bytes of the `Control` record; must match `particles.wgsl`.
const CONTROL_BYTES: u64 = 80;

/// The stride between the per-level records the tree refit reads with a dynamic offset: the
/// alignment every device accepts for one.
const LEVEL_STRIDE: u64 = 256;

/// Bits of the `Params` flags; must match `particles.wgsl`.
const GRAVITY_DIRECT: u32 = 1;
const GRAVITY_TREE: u32 = 2;
const NEIGHBOURS_PAIRS: u32 = 4;
const NEIGHBOURS_TREE: u32 = 8;
const NEIGHBOURS_GRID: u32 = 16;
const GRAVITY_GROUP: u32 = 32;

/// How the GPU sums gravity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GravitySolver {
    /// Every pair, exactly: O(N²), what the CPU does.
    Direct,
    /// Barnes–Hut: distant groups of particles pull as one point mass. O(N log N), within the
    /// opening angle's error.
    BarnesHut,
}

/// How the GPU finds each particle's neighbours, for contact and pressure.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NeighbourSearch {
    /// Every pair tested: O(N²), what the CPU does.
    AllPairs,
    /// Only the particles whose tree nodes reach this one: O(N log N). Exact: no neighbour is
    /// missed, because every node's box covers its particles' reach.
    Tree,
    /// Only the particles in the 125 cells of a grid around this one, the grid rebuilt every tick
    /// on the GPU by a counting sort: O(N). Exact, since a cell is the largest reach.
    Grid,
}

/// The algorithms the GPU steps particles with. Chosen in the menu; applies from the next frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GpuSolver {
    pub gravity: GravitySolver,
    pub neighbours: NeighbourSearch,
    /// Barnes–Hut's opening angle `θ`; used only by [`GravitySolver::BarnesHut`].
    pub opening_angle: f32,
}

impl GpuSolver {
    /// The exact methods, every pair for both jobs: exact, and the fastest at every detail up to
    /// some tens of thousands of particles. The tiled direct sum is so efficient on a GPU that
    /// the trees win only beyond that (measured on an RTX 4070 SUPER, `experiment_1`: `0.8 ms` a
    /// frame against `1.3 ms` at the default `220` particles, `84` against `114 ms` at `23,000`,
    /// and `2.2 s` against `0.85 s` at `114,000`). Switch to the trees in the menu for big runs.
    pub const DEFAULT: Self = Self::EXACT;

    /// Barnes–Hut gravity at `θ = 0.5` and the tree neighbour search: the methods that scale.
    pub const SCALABLE: Self = Self {
        gravity: GravitySolver::BarnesHut,
        neighbours: NeighbourSearch::Tree,
        opening_angle: DEFAULT_OPENING_ANGLE,
    };

    /// Exactly what the CPU does, every pair, both jobs: the reference the tests compare against.
    pub const EXACT: Self = Self {
        gravity: GravitySolver::Direct,
        neighbours: NeighbourSearch::AllPairs,
        opening_angle: DEFAULT_OPENING_ANGLE,
    };

    /// The opening angle within [`MIN_OPENING_ANGLE`] and [`MAX_OPENING_ANGLE`].
    pub fn is_valid(&self) -> bool {
        (MIN_OPENING_ANGLE..=MAX_OPENING_ANGLE).contains(&self.opening_angle)
    }

    /// Does either job use the tree?
    pub fn uses_tree(&self) -> bool {
        self.gravity == GravitySolver::BarnesHut || self.neighbours == NeighbourSearch::Tree
    }

    fn flags(&self) -> u32 {
        let gravity = match self.gravity {
            GravitySolver::Direct => GRAVITY_DIRECT,
            GravitySolver::BarnesHut => GRAVITY_TREE,
        };
        let neighbours = match self.neighbours {
            NeighbourSearch::AllPairs => NEIGHBOURS_PAIRS,
            NeighbourSearch::Tree => NEIGHBOURS_TREE,
            NeighbourSearch::Grid => NEIGHBOURS_GRID,
        };
        gravity | neighbours
    }
}

impl GravitySolver {
    /// Every solver, in the order the menu lists them.
    pub const ALL: [Self; 2] = [Self::Direct, Self::BarnesHut];

    /// The name in `collision.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::BarnesHut => "barnes-hut",
        }
    }

    /// The solver named `name` in `collision.toml`, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|solver| solver.name() == name)
    }

    /// What the menu shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Direct => "Direct: every pair, exact, O(N²)",
            Self::BarnesHut => "Barnes–Hut tree: O(N log N), within θ",
        }
    }
}

impl NeighbourSearch {
    /// Every search, in the order the menu lists them.
    pub const ALL: [Self; 3] = [Self::AllPairs, Self::Tree, Self::Grid];

    /// The name in `collision.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::AllPairs => "all-pairs",
            Self::Tree => "tree",
            Self::Grid => "grid",
        }
    }

    /// The search named `name` in `collision.toml`, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|search| search.name() == name)
    }

    /// What the menu shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::AllPairs => "All pairs: every pair tested, O(N²)",
            Self::Tree => "Tree: only nearby nodes, O(N log N)",
            Self::Grid => "Cell list: a grid rebuilt every tick, O(N)",
        }
    }
}

impl Default for GpuSolver {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Can [`GpuParticles`] step this scene?
///
/// True when it has at least two props and every one is a particle in simulated contact (see
/// [`contact_is_simulated`]), nothing absorbs, and gravity is Newtonian. Anything else is stepped
/// on the CPU.
pub fn can_step_on_gpu(scene: &Scene) -> bool {
    scene.props.len() >= 2
        && scene.gravity_model == GravityModel::Newtonian
        && scene
            .props
            .iter()
            .all(|prop| !prop.absorbs && contact_is_simulated(prop, prop))
}

/// The order a Morton (Z-order) curve visits `points` in: indices into `points`, sorted by the
/// interleaved bits of each point's position within their bounding box, 21 bits an axis.
///
/// Points that are near each other in space end up near each other in the order, which is what
/// lets an implicit binary tree over that order group them.
pub fn morton_order(points: &[DVec3]) -> Vec<usize> {
    let (lo, hi) = points.iter().fold(
        (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let span = (hi - lo).max_element().max(f64::MIN_POSITIVE);
    let scale = f64::from((1u32 << 21) - 1) / span;
    let spread = |value: u64| {
        let mut x = value & 0x1f_ffff;
        x = (x | (x << 32)) & 0x1f_0000_0000_ffff;
        x = (x | (x << 16)) & 0x1f_0000_ff00_00ff;
        x = (x | (x << 8)) & 0x100f_00f0_0f00_f00f;
        x = (x | (x << 4)) & 0x10c3_0c30_c30c_30c3;
        x = (x | (x << 2)) & 0x1249_2492_4924_9249;
        x
    };
    let code = |p: &DVec3| {
        let cell = ((*p - lo) * scale).max(DVec3::ZERO);
        spread(cell.x as u64) | (spread(cell.y as u64) << 1) | (spread(cell.z as u64) << 2)
    };
    let mut order: Vec<(u64, usize)> = points.iter().map(code).zip(0..).collect();
    order.sort_unstable();
    order.into_iter().map(|(_, index)| index).collect()
}

/// The buffers and bind group, sized for up to `capacity` particles (a power of two, which is also
/// the sort's padded length).
struct Buffers {
    capacity: usize,
    /// The particles: what every kernel steps and, in the app, what the renderer draws.
    particles: Buffer,
    /// Where the sort gathers them, copied back over `particles`.
    scratch: Buffer,
    /// What a blocking or periodic read-back copies the particles into.
    readback: Buffer,
    /// The GPU's own timing ([`Control`] in the shader).
    control: Buffer,
    /// What the timing is copied into to be read, a frame late, without waiting.
    control_readback: Buffer,
    /// The integrator's per-particle state (`kinematics`, `timing` in the shader), written at an
    /// upload, and where the sort gathers it.
    kinematics: Buffer,
    timing: Buffer,
    slot_scratch: Buffer,
    /// The cell list's buckets (a power of two), and its prefix sum's levels.
    buckets: usize,
    scan: Vec<(u32, u32)>,
    group: wgpu::BindGroup,
    // Kept alive for the bind group.
    _forces: Buffer,
    _nodes: Buffer,
    _sph: Buffer,
    _keys: Buffer,
    _grid: Buffer,
    _grid_cursor: Buffer,
    _grid_items: Buffer,
}

/// The compute pipelines, one per kernel.
struct Pipelines {
    forces_pairs: wgpu::ComputePipeline,
    forces_tree: wgpu::ComputePipeline,
    tick: wgpu::ComputePipeline,
    activate: wgpu::ComputePipeline,
    close_kick: wgpu::ComputePipeline,
    open_kick: wgpu::ComputePipeline,
    drift: wgpu::ComputePipeline,
    refit_leaves: wgpu::ComputePipeline,
    refit_level: wgpu::ComputePipeline,
    density_pairs: wgpu::ComputePipeline,
    density_tree: wgpu::ComputePipeline,
    begin: wgpu::ComputePipeline,
    limits: wgpu::ComputePipeline,
    limits_first: wgpu::ComputePipeline,
    plan: wgpu::ComputePipeline,
    bounds_reset: wgpu::ComputePipeline,
    bounds: wgpu::ComputePipeline,
    morton: wgpu::ComputePipeline,
    bitonic: wgpu::ComputePipeline,
    gather: wgpu::ComputePipeline,
    restore_sph: wgpu::ComputePipeline,
    grid_clear: wgpu::ComputePipeline,
    grid_count: wgpu::ComputePipeline,
    scan_up: wgpu::ComputePipeline,
    scan_down: wgpu::ComputePipeline,
    grid_cursor: wgpu::ComputePipeline,
    grid_scatter: wgpu::ComputePipeline,
    density_grid: wgpu::ComputePipeline,
    forces_grid: wgpu::ComputePipeline,
    gravity_group: wgpu::ComputePipeline,
}

/// A read-back in flight: nothing, mapping, or mapped and ready to read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Mapping {
    Idle,
    Pending,
    Ready,
}

/// A read-back's state, shared with the callback `wgpu` calls when the mapping is done.
#[derive(Clone)]
pub(crate) struct MapState(Arc<AtomicU8>);

impl MapState {
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicU8::new(0)))
    }
    pub(crate) fn get(&self) -> Mapping {
        match self.0.load(Ordering::Acquire) {
            1 => Mapping::Pending,
            2 => Mapping::Ready,
            _ => Mapping::Idle,
        }
    }
    pub(crate) fn set(&self, state: Mapping) {
        let value = match state {
            Mapping::Idle => 0,
            Mapping::Pending => 1,
            Mapping::Ready => 2,
        };
        self.0.store(value, Ordering::Release);
    }
}

/// What lives on the GPU between frames: one scene's particles, uploaded once and stepped there
/// until the scene is replaced.
struct Resident {
    /// The [`Scene::revision`] uploaded.
    revision: u64,
    count: usize,
    /// The point positions are relative to, metres: the particles' mean position at the upload.
    origin: DVec3,
    has_fluid: bool,
    /// The scene's gravitational softening, world units.
    softening_units: f32,
    /// Frames since the particles were last copied out for the CPU's scene.
    frames_since_readback: u32,
    particles_mapping: MapState,
    control_mapping: MapState,
    /// The last timing read back: substeps the last advance took, and time it left over.
    last_substeps: u32,
    last_time_left: f32,
    /// Real seconds a substep costs, averaged over recent frames: the frame's real length over the
    /// substeps it queued. Zero until measured.
    substep_cost_s: f32,
    /// Substeps the last advance queued.
    last_budget: u32,
    /// Simulated seconds stepped since the upload, as last read back.
    last_simulated: f32,
    /// Whether the first advance is still to come, which begins with a tick that evaluates the forces.
    unprimed: bool,
    /// Copies of the particles written into the CPU's scene since the upload, and the simulated
    /// seconds (since the upload) the last of them was taken at.
    syncs: u64,
    synced_at: f64,
}

/// The GPU particle stepper: a device, the compiled kernels, buffers that grow with the scene, and
/// the scene they hold between frames.
pub struct GpuParticles {
    gpu: GpuDevice,
    pipelines: Pipelines,
    layout: wgpu::BindGroupLayout,
    params: Buffer,
    levels: Buffer,
    buffers: Option<Buffers>,
    resident: Option<Resident>,
    /// The algorithms; read at every advance, so a change applies at once.
    pub solver: GpuSolver,
    /// Barnes–Hut gravity by the cooperative walk, a workgroup's particles at once (`gravity_group`),
    /// rather than each particle walking alone. On by default; off only to compare the two.
    pub cooperative: bool,
    /// Damping ratios already worked out, by restitution: the inverse is a bisection, and a
    /// scene's particles share a handful of materials.
    damping_ratios: Vec<(f32, f32)>,
    /// The GPU pass timer, while it is on ([`GpuParticles::set_profiling`]).
    profiler: Option<GpuProfiler>,
}

/// The deepest tree the level buffer has room for: `2^32` leaves, more than any buffer holds.
const MAX_LEVELS: u64 = 32;

/// The level buffer's records: one per tree level, then one per bitonic stage `(j, k)` at
/// [`sort_record`]. `32` sequence lengths and `32` partner distances each.
const LEVEL_RECORDS: u64 = MAX_LEVELS + MAX_LEVELS * MAX_LEVELS + MAX_SCAN_LEVELS;

/// The first of the records the cell list's prefix sum reads, one a level: the segment it scans.
const SCAN_RECORD: u64 = MAX_LEVELS + MAX_LEVELS * MAX_LEVELS;
/// The most levels the prefix sum can need: each level is `256` times shorter than the last.
const MAX_SCAN_LEVELS: u64 = 8;
/// Values one workgroup of the prefix sum scans: `GPU_WORKGROUP_SIZE` threads, four each.
const SCAN_BLOCK: usize = 4 * GPU_WORKGROUP_SIZE as usize;

/// The prefix sum's levels over `values` values, as (first, length) segments of one buffer, each
/// level's segment followed at once by the next's (the block totals); and the buffer's length.
fn scan_levels(values: usize) -> (Vec<(u32, u32)>, usize) {
    let mut levels = Vec::new();
    let mut first = 0;
    let mut length = values.div_ceil(SCAN_BLOCK) * SCAN_BLOCK;
    loop {
        levels.push((first as u32, length as u32));
        first += length;
        if length == SCAN_BLOCK {
            break;
        }
        length = (length / SCAN_BLOCK).div_ceil(SCAN_BLOCK) * SCAN_BLOCK;
    }
    (levels, first + SCAN_BLOCK)
}

/// The record a bitonic stage reads: partner distance `j = 2^log_j`, length `k = 2^log_k`.
fn sort_record(log_k: u32, log_j: u32) -> u32 {
    MAX_LEVELS as u32 + log_k * MAX_LEVELS as u32 + log_j
}

/// A kernel to dispatch: its pipeline, the items it runs over (a thread each), and the level record
/// it reads (see [`LEVEL_RECORDS`]).
type Dispatch<'a> = (&'a wgpu::ComputePipeline, usize, u32);

/// Dispatches the GPU pass timer times as one pass, and what they are.
type Stage<'a> = (GpuPhase, Vec<Dispatch<'a>>);

/// Encode `stages` in order: all in one compute pass labelled `label`, or, when `profiler` is timing
/// this frame and has room, each as its own pass with a timestamp at either end. The dispatches are
/// the same either way.
fn encode_stages(
    encoder: &mut wgpu::CommandEncoder,
    group: &wgpu::BindGroup,
    label: &str,
    stages: &[Stage<'_>],
    profiler: &mut Option<&mut GpuProfiler>,
) {
    fn run<'a>(
        pass: &mut wgpu::ComputePass<'a>,
        group: &'a wgpu::BindGroup,
        &(pipeline, items, record): &Dispatch<'a>,
    ) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[record * LEVEL_STRIDE as u32]);
        pass.dispatch_workgroups((items as u32).div_ceil(GPU_WORKGROUP_SIZE).max(1), 1, 1);
    }
    if let Some(profiler) = profiler.as_mut() {
        if profiler.has_room(stages.len()) {
            for (phase, dispatches) in stages {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(phase.name()),
                    timestamp_writes: Some(profiler.span(*phase)),
                });
                for dispatch in dispatches {
                    run(&mut pass, group, dispatch);
                }
            }
            return;
        }
        profiler.note_untimed();
    }
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: None,
    });
    for dispatch in stages.iter().flat_map(|(_, dispatches)| dispatches) {
        run(&mut pass, group, dispatch);
    }
}

/// The colour of a particle as the float the shader unpacks: 8 bits a channel, red lowest, an
/// integer under `2^24` and so exact in `f32` (bits stored raw could be a NaN a copy is free to
/// change).
fn colour_code(color: crate::core::scene::Color) -> f32 {
    let channel = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
    (channel(color.r) + 256 * channel(color.g) + 65536 * channel(color.b)) as f32
}

impl GpuParticles {
    /// Find a GPU and compile the kernels for it, or `None` when there is none. In the app the
    /// physics uses Bevy's device instead ([`GpuDevice::from_render`]); this is for tests.
    ///
    /// Not compiled into unit tests without the `gpu-tests` feature, so a GPU test that is not marked
    /// as one fails to compile there (see `tst/README.md`).
    #[cfg(any(not(test), feature = "gpu-tests"))]
    pub fn request() -> Option<Self> {
        GpuDevice::request().map(Self::new)
    }

    /// Compile the kernels on `gpu`.
    ///
    /// # Panics
    ///
    /// If the shader's workgroup size is not [`GPU_WORKGROUP_SIZE`], which is a mistake in this
    /// crate rather than something a user can cause.
    pub fn new(gpu: GpuDevice) -> Self {
        assert!(
            PARTICLES_SHADER.contains(&format!("const WORKGROUP: u32 = {GPU_WORKGROUP_SIZE}u;")),
            "particles.wgsl's WORKGROUP must equal GPU_WORKGROUP_SIZE ({GPU_WORKGROUP_SIZE})"
        );
        let device = gpu.wgpu();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particles"),
            source: wgpu::ShaderSource::Wgsl(PARTICLES_SHADER.into()),
        });
        let entry =
            |binding: u32, ty: wgpu::BufferBindingType, dynamic: bool| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty,
                    has_dynamic_offset: dynamic,
                    min_binding_size: None,
                },
                count: None,
            };
        let storage = wgpu::BufferBindingType::Storage { read_only: false };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particles"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform, false),
                entry(1, storage, false),
                entry(2, storage, false),
                entry(3, storage, false),
                entry(4, storage, false),
                entry(5, storage, false),
                entry(6, wgpu::BufferBindingType::Uniform, true),
                entry(7, storage, false),
                entry(8, storage, false),
                entry(9, storage, false),
                entry(10, storage, false),
                entry(11, storage, false),
                entry(12, storage, false),
                entry(13, storage, false),
                entry(14, storage, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particles"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |entry_point: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            })
        };
        let pipelines = Pipelines {
            forces_pairs: pipeline("forces_pairs"),
            forces_tree: pipeline("forces_tree"),
            tick: pipeline("tick"),
            activate: pipeline("activate"),
            close_kick: pipeline("close_kick"),
            open_kick: pipeline("open_kick"),
            drift: pipeline("drift"),
            refit_leaves: pipeline("refit_leaves"),
            refit_level: pipeline("refit_level"),
            density_pairs: pipeline("density_pairs"),
            density_tree: pipeline("density_tree"),
            begin: pipeline("begin"),
            limits: pipeline("limits"),
            limits_first: pipeline("limits_first"),
            plan: pipeline("plan"),
            bounds_reset: pipeline("bounds_reset"),
            bounds: pipeline("bounds"),
            morton: pipeline("morton"),
            bitonic: pipeline("bitonic"),
            gather: pipeline("gather"),
            restore_sph: pipeline("restore_sph"),
            grid_clear: pipeline("grid_clear"),
            grid_count: pipeline("grid_count"),
            scan_up: pipeline("scan_up"),
            scan_down: pipeline("scan_down"),
            grid_cursor: pipeline("grid_cursor"),
            grid_scatter: pipeline("grid_scatter"),
            density_grid: pipeline("density_grid"),
            forces_grid: pipeline("forces_grid"),
            gravity_group: pipeline("gravity_group"),
        };
        let params = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // One record per tree level, `first` and `count`, each at its own aligned offset: level `d`
        // of the implicit tree is nodes `2^d .. 2^(d+1)`. Then one per bitonic stage: `(j, k)`.
        let mut records = vec![0u8; (LEVEL_RECORDS * LEVEL_STRIDE) as usize];
        let mut write = |index: u64, first: u32, count: u32| {
            let at = (index * LEVEL_STRIDE) as usize;
            records[at..at + 4].copy_from_slice(&first.to_le_bytes());
            records[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        };
        for depth in 0..MAX_LEVELS as u32 {
            let first = 1u32.checked_shl(depth).unwrap_or(0);
            write(u64::from(depth), first, first);
        }
        for log_k in 0..MAX_LEVELS as u32 {
            for log_j in 0..MAX_LEVELS as u32 {
                let j = 1u32.checked_shl(log_j).unwrap_or(0);
                let k = 1u32.checked_shl(log_k).unwrap_or(0);
                write(u64::from(sort_record(log_k, log_j)), j, k);
            }
        }
        let levels = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tree levels and sort stages"),
            size: LEVEL_RECORDS * LEVEL_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&levels, 0, &records);
        Self {
            gpu,
            pipelines,
            layout,
            params,
            levels,
            buffers: None,
            resident: None,
            solver: GpuSolver::DEFAULT,
            cooperative: true,
            damping_ratios: Vec::new(),
            profiler: None,
        }
    }

    /// The GPU this runs on, for display.
    pub fn describe(&self) -> String {
        self.gpu.describe()
    }

    /// Turn the GPU pass timer on or off (see [`crate::compute::profiler`]). Returns whether it is
    /// on: `false` after asking for it when the device cannot write timestamps.
    pub fn set_profiling(&mut self, on: bool) -> bool {
        if !on {
            self.profiler = None;
        } else if self.profiler.is_none() {
            self.profiler = GpuProfiler::new(self.gpu.wgpu(), &self.gpu.queue);
        }
        self.profiler.is_some()
    }

    /// Whether the GPU pass timer is on.
    pub fn profiling(&self) -> bool {
        self.profiler.is_some()
    }

    /// The timer's means over its last full window of frames, while it is on. All zero until the
    /// first window fills.
    pub fn profile(&self) -> Option<&GpuProfile> {
        self.profiler.as_ref().map(GpuProfiler::recent)
    }

    /// The timer's means since [`reset_profile`](Self::reset_profile) (or since it was turned on).
    pub fn profile_since_reset(&self) -> Option<GpuProfile> {
        self.profiler.as_ref().map(GpuProfiler::total)
    }

    /// Start the timer's running means again, as the benchmark does when its warm-up ends.
    pub fn reset_profile(&mut self) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.reset();
        }
    }

    /// Has the GPU been lost or reported an error? The app then falls back to the CPU.
    pub fn has_failed(&self) -> bool {
        self.gpu.has_failed()
    }

    // -----------------------------------------------------------------------------------------
    // Stepping a scene in one call: upload, step, wait, read back. For tests and measurements.
    // -----------------------------------------------------------------------------------------

    /// Advance `scene` by `step` on the GPU, each substep as long as the particles allow (chosen
    /// on the GPU, as `collision::resolve::substep_count` would), with the algorithms in
    /// [`solver`](Self::solver), and return how many substeps it took.
    ///
    /// `None`, with the scene untouched, when [`can_step_on_gpu`] says no or the GPU failed. A
    /// zero step does nothing and returns `Some(0)`. Blocks until the GPU is done.
    pub fn step(&mut self, scene: &mut Scene, step: TimeStep) -> Option<u32> {
        if !can_step_on_gpu(scene) || self.gpu.has_failed() {
            return None;
        }
        if step.is_zero() || !step.is_valid() {
            return Some(0);
        }
        self.guarded(|this| {
            if !this.upload(scene) {
                return None;
            }
            let mut taken = 0;
            let mut first = true;
            // Whole budgets until the step is done: nothing is carried over in a single call.
            loop {
                let seconds = if first { step.seconds() as f32 } else { 0.0 };
                this.encode_advance(seconds, MAX_SUBSTEPS, first);
                first = false;
                let (substeps, time_left) = this.read_control_blocking()?;
                taken += substeps;
                if time_left <= 0.0 || substeps == 0 {
                    break;
                }
            }
            this.read_back_blocking(scene).then_some(taken)
        })
        .flatten()
    }

    /// Advance `scene` by `step` in exactly `substeps` equal substeps. `false`, with the scene
    /// untouched, when the scene cannot be stepped here or the read-back failed. Blocks.
    pub fn step_in(&mut self, scene: &mut Scene, step: TimeStep, substeps: u32) -> bool {
        if !can_step_on_gpu(scene) || substeps == 0 || self.gpu.has_failed() {
            return false;
        }
        self.guarded(|this| {
            if !this.upload(scene) {
                return false;
            }
            let seconds = step.seconds() as f32;
            this.write_control(seconds / substeps as f32);
            this.encode_advance(seconds, substeps, true);
            this.read_back_blocking(scene)
        })
        .unwrap_or(false)
    }

    /// Run `work`, catching the panic a submission to a lost device raises (`wgpu` panics from
    /// inside a submission, past its error handler) and marking the device failed, so every later
    /// call returns at once and the app carries on on the CPU.
    fn guarded<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> Option<T> {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&mut *self)));
        match outcome {
            Ok(value) if !self.gpu.has_failed() => Some(value),
            Ok(_) => None,
            Err(_) => {
                bevy::log::error!(
                    "physics GPU failed during a step; stepping on the CPU from now on"
                );
                self.gpu.failed.store(true, Ordering::Relaxed);
                self.resident = None;
                None
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Keeping a scene on the GPU: upload once, advance every frame, read back now and then.
    // -----------------------------------------------------------------------------------------

    /// Does the GPU hold `scene`'s particles (the same [`Scene::revision`])?
    pub fn holds(&self, scene: &Scene) -> bool {
        self.resident
            .as_ref()
            .is_some_and(|resident| resident.revision == scene.revision())
    }

    /// Copy `scene`'s particles to the GPU, replacing whatever it held. `false` when the scene
    /// cannot be stepped here.
    pub fn upload(&mut self, scene: &Scene) -> bool {
        if !can_step_on_gpu(scene) || self.gpu.has_failed() {
            return false;
        }
        let count = scene.props.len();
        self.ensure_capacity(count);
        let positions: Vec<DVec3> = scene
            .props
            .iter()
            .map(|prop| prop.body.center() * METRES_PER_UNIT)
            .collect();
        let origin = positions.iter().sum::<DVec3>() / count as f64;
        let mut floats = Vec::with_capacity(count * FLOATS_PER_PARTICLE);
        for (index, prop) in scene.props.iter().enumerate() {
            let Body::Sphere { radius, color, .. } = &prop.body else {
                return false;
            };
            let radius_m =
                f64::from(prop.contact_radius_units.unwrap_or(*radius)) * METRES_PER_UNIT;
            let position = positions[index] - origin;
            let velocity = prop.velocity_units_s * METRES_PER_UNIT;
            let zeta = self.damping_ratio(prop.material.restitution);
            let thermal = prop.material.thermal;
            // A fluid's smoothing length from its last measured density, as the CPU's
            // `Particle::of` works it out; zero marks a particle that is not a fluid.
            let (smoothing, density) = match (prop.material.fluid, thermal) {
                (true, Some(eos)) => {
                    let eos = eos.parameters();
                    let density = if prop.density_kg_m3 > 0.0 {
                        prop.density_kg_m3
                    } else {
                        eos.reference_density_kg_m3
                    };
                    (
                        fluid_smoothing_m(f64::from(prop.mass_kg), density, &eos),
                        density,
                    )
                }
                _ => (0.0, 0.0),
            };
            floats.extend_from_slice(&[
                position.x as f32,
                position.y as f32,
                position.z as f32,
                radius_m as f32,
                velocity.x as f32,
                velocity.y as f32,
                velocity.z as f32,
                prop.mass_kg,
                prop.spin_rad_s.x as f32,
                prop.spin_rad_s.y as f32,
                prop.spin_rad_s.z as f32,
                prop.heat_j_kg as f32,
                prop.material.modulus_pa.unwrap_or(0.0),
                zeta,
                prop.material.friction,
                index as f32,
                thermal.map_or(0.0, |t| t.a),
                thermal.map_or(0.0, |t| t.b),
                thermal.map_or(0.0, |t| t.reference_energy_j_kg),
                thermal.map_or(0.0, |t| t.complete_vaporisation_j_kg),
                thermal.map_or(0.0, |t| t.reference_density_kg_m3),
                thermal.map_or(0.0, |t| t.bulk_a_pa),
                thermal.map_or(0.0, |t| t.bulk_b_pa),
                thermal.map_or(0.0, |t| t.incipient_vaporisation_j_kg),
                thermal.map_or(0.0, |t| t.alpha),
                thermal.map_or(0.0, |t| t.beta),
                colour_code(*color),
                prop.body.center().x as f32,
                smoothing as f32,
                density as f32,
                prop.body.center().y as f32,
                prop.body.center().z as f32,
            ]);
        }
        let Some(buffers) = self.buffers.as_ref() else {
            return false;
        };
        let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.gpu.queue.write_buffer(&buffers.particles, 0, &bytes);
        // The integrator starts with no acceleration and no step; its half-step state is the
        // particle's own (the first tick evaluates the forces without moving: `Control::prime`).
        let mut kinematics = Vec::with_capacity(count * 16);
        let mut timing = Vec::with_capacity(count * 16);
        for prop in &scene.props {
            let velocity = prop.velocity_units_s * METRES_PER_UNIT;
            let spin = prop.spin_rad_s;
            kinematics.extend_from_slice(&[0.0; 8]);
            // `c.w`: its neighbours' least step limit, unknown until its first evaluation.
            kinematics.extend_from_slice(&[spin.x as f32, spin.y as f32, spin.z as f32, 3.0e38]);
            kinematics.extend_from_slice(&[0.0; 4]);
            // No step limit known yet: it constrains no neighbour until its first evaluation.
            timing.extend_from_slice(&[3.0e38, 0.0, 0.0, 0.0]);
            timing.extend_from_slice(&[
                velocity.x as f32,
                velocity.y as f32,
                velocity.z as f32,
                prop.heat_j_kg as f32,
            ]);
            timing.extend_from_slice(&[0.0; 4]);
            timing.extend_from_slice(&[0.0; 4]);
        }
        let le =
            |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|f| f.to_le_bytes()).collect() };
        self.gpu
            .queue
            .write_buffer(&buffers.kinematics, 0, &le(&kinematics));
        self.gpu
            .queue
            .write_buffer(&buffers.timing, 0, &le(&timing));
        self.resident = Some(Resident {
            revision: scene.revision(),
            count,
            origin,
            has_fluid: scene.props.iter().any(|prop| prop.material.fluid),
            softening_units: scene.softening_units,
            frames_since_readback: 0,
            particles_mapping: MapState::new(),
            control_mapping: MapState::new(),
            last_substeps: 0,
            last_time_left: 0.0,
            substep_cost_s: 0.0,
            last_budget: 0,
            last_simulated: 0.0,
            unprimed: true,
            syncs: 0,
            synced_at: 0.0,
        });
        self.write_control(0.0);
        self.write_params(0.0);
        true
    }

    /// Advance the held scene by `step` of simulated time, without waiting for the GPU: each
    /// substep as long as the particles allow, chosen on the GPU. `real_seconds` is how long the
    /// last frame really took, from which each substep's real cost is estimated.
    ///
    /// As many substeps are queued as the last advance needed and a margin, but no more than fit
    /// [`GPU_FRAME_TIME_TARGET_SECONDS`] of real time. Time the GPU does not get through is carried
    /// into the next advance (up to a frame's worth) and the rest dropped: a frame too hard to
    /// finish slows the simulation, never steps it unstably and never stalls the frame rate.
    /// `false` when nothing is held or the GPU failed.
    pub fn advance(&mut self, step: TimeStep, real_seconds: f32) -> bool {
        if self.resident.is_none() || self.gpu.has_failed() {
            return false;
        }
        self.guarded(|this| {
            this.poll_control();
            let Some(resident) = this.resident.as_mut() else {
                return false;
            };
            // The last frame's real length over the substeps it queued: what one costs, averaged.
            if resident.last_budget > 0 && real_seconds.is_finite() && real_seconds > 0.0 {
                let cost = real_seconds / resident.last_budget as f32;
                resident.substep_cost_s = if resident.substep_cost_s > 0.0 {
                    0.8 * resident.substep_cost_s + 0.2 * cost
                } else {
                    cost
                };
            }
            let last = resident.last_substeps;
            let wanted = if resident.last_time_left > 0.0 {
                last.saturating_mul(2)
            } else {
                last + last / 4 + 2
            };
            let affordable = if resident.substep_cost_s > 0.0 {
                (GPU_FRAME_TIME_TARGET_SECONDS / resident.substep_cost_s) as u32
            } else {
                MAX_SUBSTEPS
            };
            let budget = wanted
                .min(affordable)
                .clamp(GPU_MIN_SUBSTEP_BUDGET, MAX_SUBSTEPS);
            resident.last_budget = budget;
            this.encode_advance(step.seconds() as f32, budget, true);
            this.request_control();
            true
        })
        .unwrap_or(false)
    }

    /// Bring `scene` up to date with the GPU's particles when a read-back has arrived, and start
    /// another every [`GPU_READBACK_INTERVAL_FRAMES`] frames. Never waits. Returns whether the
    /// scene was written. The scene trails the GPU by the read-back's few frames, which is what
    /// the overlay and the analysis read; the renderer draws the GPU's own buffer.
    pub fn sync(&mut self, scene: &mut Scene) -> bool {
        if !self.holds(scene) || self.gpu.has_failed() {
            return false;
        }
        self.guarded(|this| {
            this.gpu.wgpu().poll(wgpu::Maintain::Poll);
            let (Some(resident), Some(buffers)) = (this.resident.as_mut(), this.buffers.as_ref())
            else {
                return false;
            };
            let mut written = false;
            if resident.particles_mapping.get() == Mapping::Ready {
                let length = resident.count as u64 * BYTES_PER_PARTICLE;
                let values = read_mapped(&buffers.readback, length + CONTROL_BYTES);
                let (particles, control) = values.split_at(length as usize / 4);
                write_back(particles, resident.origin, scene);
                resident.particles_mapping.set(Mapping::Idle);
                resident.syncs += 1;
                resident.synced_at = f64::from(control[11]);
                written = true;
            }
            resident.frames_since_readback += 1;
            if resident.particles_mapping.get() == Mapping::Idle
                && resident.frames_since_readback >= GPU_READBACK_INTERVAL_FRAMES
            {
                resident.frames_since_readback = 0;
                let length = resident.count as u64 * BYTES_PER_PARTICLE;
                let mut encoder =
                    this.gpu
                        .wgpu()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("particle read-back"),
                        });
                encoder.copy_buffer_to_buffer(&buffers.particles, 0, &buffers.readback, 0, length);
                // The timing with it, for the simulated time the copy was taken at.
                encoder.copy_buffer_to_buffer(
                    &buffers.control,
                    0,
                    &buffers.readback,
                    length,
                    CONTROL_BYTES,
                );
                this.gpu.queue.submit(Some(encoder.finish()));
                map_async(
                    &buffers.readback,
                    length + CONTROL_BYTES,
                    &resident.particles_mapping,
                );
            }
            written
        })
        .unwrap_or(false)
    }

    /// The particle buffer the renderer draws, how many particles it holds, and the point their
    /// positions are relative to (metres), when a scene is held. Each particle is
    /// `FLOATS_PER_PARTICLE` (`32`) floats: position and radius first (metres), and its colour code
    /// (see `particles.wgsl`) at float `26`.
    pub fn particle_buffer(&self) -> Option<(Buffer, u32, DVec3)> {
        let resident = self.resident.as_ref()?;
        let buffers = self.buffers.as_ref()?;
        Some((
            buffers.particles.clone(),
            resident.count as u32,
            resident.origin,
        ))
    }

    /// How many copies of the particles [`sync`](Self::sync) has written into the CPU's scene since
    /// the upload, and the simulated seconds since the upload the last was taken at (exact: the
    /// timing is copied with it). For what follows the GPU between copies, such as the camera's
    /// lock targets.
    pub fn last_sync(&self) -> Option<(u64, f64)> {
        self.resident
            .as_ref()
            .map(|resident| (resident.syncs, resident.synced_at))
    }

    /// Simulated seconds the GPU has stepped the held scene since it was uploaded, as last read back
    /// (a frame late). Less than the clock asked for when the GPU fell behind; the app sets the
    /// clock from it so the time shown is the time simulated.
    pub fn simulated_seconds(&self) -> Option<f64> {
        self.resident
            .as_ref()
            .map(|resident| f64::from(resident.last_simulated))
    }

    /// Wait for the GPU to finish what has been queued, and return the timing of the last advance:
    /// the substeps it took and the time it left over. For measurements; the app never waits.
    pub fn finish(&mut self) -> Option<(u32, f32)> {
        self.gpu.wgpu().poll(wgpu::Maintain::Wait);
        self.poll_control();
        let resident = self.resident.as_ref()?;
        Some((resident.last_substeps, resident.last_time_left))
    }

    /// Forget the held scene (a lost device, or a scene that can no longer be stepped here).
    pub fn release(&mut self) {
        self.resident = None;
    }

    // -----------------------------------------------------------------------------------------
    // Encoding.
    // -----------------------------------------------------------------------------------------

    /// The `Params` uniform for the held scene and the current solver, with `frame_dt` seconds to
    /// add at the next advance. The layout must match `Params` in `particles.wgsl`.
    fn write_params(&self, frame_dt: f32) {
        let Some(resident) = self.resident.as_ref() else {
            return;
        };
        let count = resident.count;
        let leaves = count.div_ceil(GPU_LEAF_SIZE).next_power_of_two();
        let softening_m = f64::from(resident.softening_units.max(0.0)) * METRES_PER_UNIT;
        let smoothing_per_radius = SPH_SMOOTHING_FACTOR
            * (4.0 * std::f64::consts::PI / (3.0 * FCC_PACKING_FRACTION)).cbrt();
        let solver = if self.solver.is_valid() {
            self.solver
        } else {
            GpuSolver::DEFAULT
        };
        let mut bytes = Vec::with_capacity(PARAMS_BYTES as usize);
        let float = |bytes: &mut Vec<u8>, value: f64| {
            bytes.extend_from_slice(&(value as f32).to_le_bytes());
        };
        let word = |bytes: &mut Vec<u8>, value: u32| bytes.extend_from_slice(&value.to_le_bytes());
        float(&mut bytes, 0.0); // dt: unused, the GPU chooses each substep's
        word(&mut bytes, count as u32);
        float(&mut bytes, softening_m * softening_m);
        float(&mut bytes, GRAVITATIONAL_CONSTANT);
        float(&mut bytes, f64::from(solver.opening_angle));
        word(&mut bytes, leaves as u32);
        float(&mut bytes, smoothing_per_radius);
        float(&mut bytes, FCC_PACKING_FRACTION);
        float(&mut bytes, SOFT_CONTACT_MAX_NEIGHBOURS);
        let group = if self.cooperative && solver.gravity == GravitySolver::BarnesHut {
            GRAVITY_GROUP
        } else {
            0
        };
        word(&mut bytes, solver.flags() | group);
        word(&mut bytes, GPU_LEAF_SIZE as u32);
        word(&mut bytes, 0);
        for value in [
            SPH_VISCOSITY_ALPHA,
            SPH_VISCOSITY_BETA,
            SPH_VISCOSITY_EPSILON,
            SPH_SOUND_SPEED_FLOOR_FRACTION,
            SPH_SMOOTHING_FACTOR,
            SPH_MIN_SMOOTHING_FRACTION,
            SPH_MAX_SMOOTHING_FRACTION,
            f64::from(frame_dt),
            f64::from(MAX_SUBSTEP_DISPLACEMENT_UNITS) * METRES_PER_UNIT,
            f64::from(MAX_SUBSTEP_DYNAMICAL_FRACTION),
            SOFT_CONTACT_SUBSTEPS,
            SPH_COURANT,
            resident.origin.x / METRES_PER_UNIT,
            resident.origin.y / METRES_PER_UNIT,
            resident.origin.z / METRES_PER_UNIT,
            1.0 / METRES_PER_UNIT,
        ] {
            float(&mut bytes, value);
        }
        word(&mut bytes, GPU_TIME_BIN_HEADROOM);
        word(&mut bytes, GPU_MAX_TIME_BINS);
        float(&mut bytes, GPU_NEIGHBOUR_STEP_RATIO);
        word(
            &mut bytes,
            self.buffers.as_ref().map_or(0, |b| b.buckets as u32 - 1),
        );
        debug_assert_eq!(bytes.len() as u64, PARAMS_BYTES);
        self.gpu.queue.write_buffer(&self.params, 0, &bytes);
    }

    /// Reset the GPU's timing: nothing left to step, and fixed-step mode at `fixed_dt` seconds a
    /// substep when positive.
    fn write_control(&self, fixed_dt: f32) {
        let Some(buffers) = self.buffers.as_ref() else {
            return;
        };
        let mut bytes = Vec::with_capacity(CONTROL_BYTES as usize);
        bytes.extend_from_slice(&0.0f32.to_le_bytes()); // time_left
        bytes.extend_from_slice(&0.0f32.to_le_bytes()); // dt
        bytes.extend_from_slice(&fixed_dt.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // substeps
        bytes.extend_from_slice(&f32::MAX.to_bits().to_le_bytes()); // limit
        bytes.resize(17 * 4, 0); // bounds, simulated, limit_max, tick, bins, run, boundary
        bytes.extend_from_slice(&1u32.to_le_bytes()); // prime: evaluate the forces first
        bytes.resize(CONTROL_BYTES as usize, 0);
        self.gpu.queue.write_buffer(&buffers.control, 0, &bytes);
    }

    /// Queue one advance of `seconds`: the sort (when `sort`), the frame's time added to what is
    /// left, then `substeps` substeps, each its own submission so no single piece of GPU work runs
    /// long (Windows resets a GPU whose one piece of work runs past about two seconds).
    fn encode_advance(&mut self, seconds: f32, substeps: u32, sort: bool) {
        let (Some(resident), Some(buffers)) = (self.resident.as_ref(), self.buffers.as_ref())
        else {
            return;
        };
        // The first advance after an upload starts with a tick that evaluates the forces without
        // moving anything (`Control::prime`).
        let substeps = substeps + u32::from(resident.unprimed);
        let count = resident.count;
        let has_fluid = resident.has_fluid;
        // The solver can change between frames.
        self.write_params(seconds);
        let solver = if self.solver.is_valid() {
            self.solver
        } else {
            GpuSolver::DEFAULT
        };
        let leaves = count.div_ceil(GPU_LEAF_SIZE).next_power_of_two();
        let depth = leaves.trailing_zeros();
        let padded = buffers.capacity;
        let buckets = buffers.buckets;
        let scan = buffers.scan.clone();
        let p = &self.pipelines;
        let group = &buffers.group;
        let device = self.gpu.wgpu();
        let queue = &self.gpu.queue;
        let mut profiler = self.profiler.as_mut();
        if let Some(profiler) = profiler.as_mut() {
            profiler.begin_frame();
        }

        // The sort and the start of the advance.
        let mut start: Vec<Dispatch> = vec![(&p.begin, 1, 0)];
        if sort {
            start.push((&p.bounds_reset, 1, 0));
            start.push((&p.bounds, count, 0));
            start.push((&p.morton, padded, 0));
            let log_n = padded.trailing_zeros();
            for log_k in 1..=log_n {
                for log_j in (0..log_k).rev() {
                    start.push((&p.bitonic, padded, sort_record(log_k, log_j)));
                }
            }
            start.push((&p.gather, count, 0));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("particle advance"),
        });
        encode_stages(
            &mut encoder,
            group,
            "begin",
            &[(GpuPhase::Sort, start)],
            &mut profiler,
        );
        if sort {
            let length = count as u64 * BYTES_PER_PARTICLE;
            encoder.copy_buffer_to_buffer(&buffers.scratch, 0, &buffers.particles, 0, length);
            let slots = count as u64 * BYTES_PER_SLOT;
            encoder.copy_buffer_to_buffer(&buffers.slot_scratch, 0, &buffers.kinematics, 0, slots);
            encoder.copy_buffer_to_buffer(&buffers.slot_scratch, slots, &buffers.timing, 0, slots);
            encode_stages(
                &mut encoder,
                group,
                "restore fluid states",
                &[(GpuPhase::Restore, vec![(&p.restore_sph, count, 0)])],
                &mut profiler,
            );
        }
        queue.submit(Some(encoder.finish()));

        // One tick, the same every substep (the order is in `particles.wgsl`'s header), in the
        // groups the timer times.
        let mut tick: Vec<Stage> =
            vec![(GpuPhase::Drift, vec![(&p.tick, 1, 0), (&p.drift, count, 0)])];
        if solver.uses_tree() {
            let mut refit = vec![(&p.refit_leaves, leaves, 0)];
            for level in (0..depth).rev() {
                refit.push((&p.refit_level, 1 << level, level));
            }
            tick.push((GpuPhase::Refit, refit));
        }
        if solver.neighbours == NeighbourSearch::Grid {
            let mut grid = vec![(&p.grid_clear, buckets + 1, 0), (&p.grid_count, count, 0)];
            for (level, &(_, length)) in scan.iter().enumerate() {
                let record = (SCAN_RECORD as usize + level) as u32;
                grid.push((&p.scan_up, length as usize / 4, record));
            }
            for (level, &(_, length)) in scan.iter().enumerate().rev().skip(1) {
                let record = (SCAN_RECORD as usize + level) as u32;
                grid.push((&p.scan_down, length as usize / 4, record));
            }
            grid.push((&p.grid_cursor, buckets, 0));
            grid.push((&p.grid_scatter, count, 0));
            tick.push((GpuPhase::Grid, grid));
        }
        let mut density = vec![(&p.activate, count, 0)];
        if has_fluid {
            density.push(match solver.neighbours {
                NeighbourSearch::AllPairs => (&p.density_pairs, count, 0),
                NeighbourSearch::Tree => (&p.density_tree, count, 0),
                NeighbourSearch::Grid => (&p.density_grid, count, 0),
            });
        }
        density.push((&p.limits_first, count, 0));
        tick.push((GpuPhase::Density, density));
        if solver.gravity == GravitySolver::Direct || solver.neighbours == NeighbourSearch::AllPairs
        {
            tick.push((GpuPhase::PairForces, vec![(&p.forces_pairs, count, 0)]));
        }
        let cooperative = self.cooperative && solver.gravity == GravitySolver::BarnesHut;
        if cooperative {
            tick.push((GpuPhase::GravityWalk, vec![(&p.gravity_group, count, 0)]));
        }
        if solver.neighbours == NeighbourSearch::Tree
            || (solver.gravity == GravitySolver::BarnesHut && !cooperative)
        {
            tick.push((GpuPhase::TreeWalk, vec![(&p.forces_tree, count, 0)]));
        }
        if solver.neighbours == NeighbourSearch::Grid {
            tick.push((GpuPhase::GridContacts, vec![(&p.forces_grid, count, 0)]));
        }
        tick.push((
            GpuPhase::Kick,
            vec![
                (&p.close_kick, count, 0),
                (&p.limits, count, 0),
                (&p.plan, 1, 0),
                (&p.open_kick, count, 0),
            ],
        ));

        for _ in 0..substeps {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("particle substep"),
            });
            encode_stages(
                &mut encoder,
                group,
                "particle substep",
                &tick,
                &mut profiler,
            );
            queue.submit(Some(encoder.finish()));
        }
        if let Some(profiler) = profiler {
            profiler.end_frame(device, queue);
        }
        if let Some(resident) = self.resident.as_mut() {
            resident.unprimed = false;
        }
    }

    /// Copy the timing out to be read on a later frame, if the last copy has been read.
    fn request_control(&mut self) {
        let (Some(resident), Some(buffers)) = (self.resident.as_ref(), self.buffers.as_ref())
        else {
            return;
        };
        if resident.control_mapping.get() != Mapping::Idle {
            return;
        }
        let mut encoder = self
            .gpu
            .wgpu()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("timing read-back"),
            });
        encoder.copy_buffer_to_buffer(
            &buffers.control,
            0,
            &buffers.control_readback,
            0,
            CONTROL_BYTES,
        );
        self.gpu.queue.submit(Some(encoder.finish()));
        map_async(
            &buffers.control_readback,
            CONTROL_BYTES,
            &resident.control_mapping,
        );
    }

    /// Take a timing read-back that has arrived, without waiting.
    fn poll_control(&mut self) {
        self.gpu.wgpu().poll(wgpu::Maintain::Poll);
        let (Some(resident), Some(buffers)) = (self.resident.as_mut(), self.buffers.as_ref())
        else {
            return;
        };
        if resident.control_mapping.get() == Mapping::Ready {
            let values = read_mapped(&buffers.control_readback, CONTROL_BYTES);
            resident.last_time_left = time_left(&values);
            resident.last_substeps = values[3].to_bits();
            resident.last_simulated = values[11];
            resident.control_mapping.set(Mapping::Idle);
            if let Some(profiler) = self.profiler.as_mut() {
                // `Control::active_sum`: particles active, summed over the advance's ticks.
                profiler.add_advance(
                    resident.last_budget,
                    resident.last_substeps,
                    values[19].to_bits(),
                    resident.count,
                );
            }
        }
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.poll();
        }
    }

    /// Read the timing now, waiting for the GPU: the substeps the last advance took and the time
    /// it left. `None` when the read failed.
    fn read_control_blocking(&mut self) -> Option<(u32, f32)> {
        let buffers = self.buffers.as_ref()?;
        let mut encoder = self
            .gpu
            .wgpu()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("timing read-back"),
            });
        encoder.copy_buffer_to_buffer(
            &buffers.control,
            0,
            &buffers.control_readback,
            0,
            CONTROL_BYTES,
        );
        self.gpu.queue.submit(Some(encoder.finish()));
        let state = MapState::new();
        map_async(&buffers.control_readback, CONTROL_BYTES, &state);
        self.gpu.wgpu().poll(wgpu::Maintain::Wait);
        if state.get() != Mapping::Ready || self.gpu.has_failed() {
            return None;
        }
        let values = read_mapped(&buffers.control_readback, CONTROL_BYTES);
        Some((values[3].to_bits(), time_left(&values)))
    }

    /// Read the particles now, waiting for the GPU, into `scene`. `false` when the read failed.
    fn read_back_blocking(&mut self, scene: &mut Scene) -> bool {
        let (Some(resident), Some(buffers)) = (self.resident.as_ref(), self.buffers.as_ref())
        else {
            return false;
        };
        let length = resident.count as u64 * BYTES_PER_PARTICLE;
        let mut encoder = self
            .gpu
            .wgpu()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("particle read-back"),
            });
        encoder.copy_buffer_to_buffer(&buffers.particles, 0, &buffers.readback, 0, length);
        self.gpu.queue.submit(Some(encoder.finish()));
        let state = MapState::new();
        map_async(&buffers.readback, length, &state);
        self.gpu.wgpu().poll(wgpu::Maintain::Wait);
        if state.get() != Mapping::Ready || self.gpu.has_failed() {
            return false;
        }
        let values = read_mapped(&buffers.readback, length);
        write_back(&values, resident.origin, scene);
        true
    }

    /// The capped damping ratio for `restitution`, worked out once per material.
    fn damping_ratio(&mut self, restitution: f32) -> f32 {
        if let Some((_, zeta)) = self
            .damping_ratios
            .iter()
            .find(|(known, _)| known.to_bits() == restitution.to_bits())
        {
            return *zeta;
        }
        let zeta = damping_ratio_for_restitution(f64::from(restitution))
            .min(SOFT_CONTACT_MAX_DAMPING_RATIO) as f32;
        self.damping_ratios.push((restitution, zeta));
        zeta
    }

    /// Make sure the buffers hold at least `count` particles, growing them (to the next power of
    /// two, which is also the sort's padded length) if not.
    fn ensure_capacity(&mut self, count: usize) {
        if self
            .buffers
            .as_ref()
            .is_some_and(|buffers| buffers.capacity >= count)
        {
            return;
        }
        let capacity = count.next_power_of_two().max(GPU_WORKGROUP_SIZE as usize);
        let device = &self.gpu.device;
        let storage = |label, size, extra: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC
                    | extra,
                mapped_at_creation: false,
            })
        };
        let none = wgpu::BufferUsages::empty();
        let n = capacity as u64;
        // `VERTEX` so the renderer can draw the particles straight from the buffer.
        let particles = storage(
            "particles",
            n * BYTES_PER_PARTICLE,
            wgpu::BufferUsages::VERTEX,
        );
        let scratch = storage("particles sorted", n * BYTES_PER_PARTICLE, none);
        let kinematics = storage("particle kinematics", n * BYTES_PER_SLOT, none);
        let timing = storage("particle timing", n * BYTES_PER_SLOT, none);
        let slot_scratch = storage("integrator sorted", 2 * n * BYTES_PER_SLOT, none);
        let forces = storage("particle forces", n * BYTES_PER_FORCE, none);
        // Twice as many buckets as particles keeps collisions between cells rare.
        let buckets = (2 * capacity).next_power_of_two();
        let (scan, scan_length) = scan_levels(buckets + 1);
        let grid = storage("cell list starts", scan_length as u64 * 4, none);
        let grid_cursor = storage("cell list cursors", buckets as u64 * 4, none);
        let grid_items = storage("cell list items", n * 4, none);
        assert!(
            scan.len() as u64 <= MAX_SCAN_LEVELS,
            "too many particles for the cell list"
        );
        for (level, &(first, length)) in scan.iter().enumerate() {
            let mut record = [0u8; 8];
            record[..4].copy_from_slice(&first.to_le_bytes());
            record[4..].copy_from_slice(&length.to_le_bytes());
            self.gpu.queue.write_buffer(
                &self.levels,
                (SCAN_RECORD + level as u64) * LEVEL_STRIDE,
                &record,
            );
        }
        let leaves = (capacity / GPU_LEAF_SIZE).max(1);
        let nodes = storage("tree nodes", 2 * leaves as u64 * BYTES_PER_NODE, none);
        let sph = storage("fluid states", n * BYTES_PER_SPH, none);
        let keys = storage("sort keys", n * BYTES_PER_KEY, none);
        let control = storage("timing", CONTROL_BYTES, none);
        let mappable = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let readback = mappable(
            "particles read-back",
            n * BYTES_PER_PARTICLE + CONTROL_BYTES,
        );
        let control_readback = mappable("timing read-back", CONTROL_BYTES);
        fn whole(binding: u32, buffer: &Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }
        }
        let group = self
            .gpu
            .wgpu()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("particles"),
                layout: &self.layout,
                entries: &[
                    whole(0, &self.params),
                    whole(1, &particles),
                    whole(2, &kinematics),
                    whole(3, &timing),
                    whole(4, &forces),
                    whole(5, &nodes),
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.levels,
                            offset: 0,
                            size: wgpu::BufferSize::new(8),
                        }),
                    },
                    whole(7, &sph),
                    whole(8, &keys),
                    whole(9, &scratch),
                    whole(10, &control),
                    whole(11, &slot_scratch),
                    whole(12, &grid),
                    whole(13, &grid_cursor),
                    whole(14, &grid_items),
                ],
            });
        self.buffers = Some(Buffers {
            capacity,
            particles,
            scratch,
            readback,
            control,
            control_readback,
            kinematics,
            timing,
            slot_scratch,
            buckets,
            scan,
            group,
            _forces: forces,
            _nodes: nodes,
            _sph: sph,
            _keys: keys,
            _grid: grid,
            _grid_cursor: grid_cursor,
            _grid_items: grid_items,
        });
        // New buffers hold nothing.
        self.resident = None;
    }
}

/// Simulated seconds an advance still has to step, from a read-back `Control`: what is left to plan,
/// and the rest of a macro step in progress (`2^bins − tick` ticks of `dt`).
fn time_left(values: &[f32]) -> f32 {
    let planned = if values[16].to_bits() == 0 && values[1] > 0.0 {
        let ticks = (1u32 << values[14].to_bits()).saturating_sub(values[13].to_bits());
        ticks as f32 * values[1]
    } else {
        0.0
    };
    values[0] + planned
}

/// Ask `wgpu` to map the first `length` bytes of `buffer` for reading, recording the outcome in
/// `state`.
pub(crate) fn map_async(buffer: &wgpu::Buffer, length: u64, state: &MapState) {
    state.set(Mapping::Pending);
    let done = state.clone();
    buffer
        .slice(..length)
        .map_async(wgpu::MapMode::Read, move |outcome| {
            done.set(if outcome.is_ok() {
                Mapping::Ready
            } else {
                Mapping::Idle
            });
        });
}

/// The first `length` bytes of a mapped `buffer` as floats, then unmap it.
fn read_mapped(buffer: &Buffer, length: u64) -> Vec<f32> {
    let values = {
        let view = buffer.slice(..length).get_mapped_range();
        view.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    };
    buffer.unmap();
    values
}

/// Write particles read back from the GPU into `scene`, each to the prop whose index it carries,
/// in `f64`, positions relative to `origin` (metres).
fn write_back(values: &[f32], origin: DVec3, scene: &mut Scene) {
    for particle in values.chunks_exact(FLOATS_PER_PARTICLE) {
        let index = particle[15] as usize;
        let Some(prop) = scene.props.get_mut(index) else {
            continue;
        };
        let position_m = origin
            + DVec3::new(
                f64::from(particle[0]),
                f64::from(particle[1]),
                f64::from(particle[2]),
            );
        prop.body
            .translate(position_m / METRES_PER_UNIT - prop.body.center());
        prop.velocity_units_s = DVec3::new(
            f64::from(particle[4]),
            f64::from(particle[5]),
            f64::from(particle[6]),
        ) / METRES_PER_UNIT;
        prop.spin_rad_s = DVec3::new(
            f64::from(particle[8]),
            f64::from(particle[9]),
            f64::from(particle[10]),
        );
        prop.heat_j_kg = f64::from(particle[11]).max(0.0);
        if prop.material.fluid {
            prop.density_kg_m3 = f64::from(particle[29]).max(0.0);
            if let Body::Sphere { radius, .. } = &mut prop.body {
                *radius = (f64::from(particle[3]) / METRES_PER_UNIT) as f32;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tst/compute/particles_tests.rs"]
mod tests;

/// The tests that open a GPU: see the `gpu-tests` feature in `Cargo.toml`.
#[cfg(all(test, feature = "gpu-tests"))]
#[path = "../../tst/compute/particles_gpu_tests.rs"]
mod gpu_tests;
