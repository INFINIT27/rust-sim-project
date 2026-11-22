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

use std::sync::mpsc;

use crate::collision::collision::contact_is_simulated;
use crate::collision::constants::{
    SOFT_CONTACT_MAX_DAMPING_RATIO, SOFT_CONTACT_MAX_NEIGHBOURS, SPH_SMOOTHING_FACTOR,
};
use crate::collision::resolve::substep_count;
use crate::compute::constants::{
    DEFAULT_OPENING_ANGLE, GPU_LEAF_SIZE, GPU_WORKGROUP_SIZE, MAX_OPENING_ANGLE, MIN_OPENING_ANGLE,
};
use crate::compute::device::GpuDevice;
use crate::constants::METRES_PER_UNIT;
use crate::core::integrator::GravityModel;
use crate::core::scene::{Body, DVec3, Scene};
use crate::physics_utils::physics_constants::{FCC_PACKING_FRACTION, GRAVITATIONAL_CONSTANT};
use crate::physics_utils::soft_contact::damping_ratio_for_restitution;
use crate::time::simulation::TimeStep;

/// The kernels, compiled into the binary.
pub const PARTICLES_SHADER: &str = include_str!("particles.wgsl");

/// `f32`s per particle in the buffers: position and radius, velocity and mass, spin and heat,
/// material, thermal.
const FLOATS_PER_PARTICLE: usize = 20;

/// Bytes per particle, per motion record (velocity and spin), per force record, and per tree node.
const BYTES_PER_PARTICLE: u64 = (FLOATS_PER_PARTICLE * 4) as u64;
const BYTES_PER_MOTION: u64 = 32;
const BYTES_PER_NODE: u64 = 80;

/// The stride between the per-level records the tree refit reads with a dynamic offset: the
/// alignment every device accepts for one.
const LEVEL_STRIDE: u64 = 256;

/// Bits of the `Params` flags; must match `particles.wgsl`.
const GRAVITY_DIRECT: u32 = 1;
const GRAVITY_TREE: u32 = 2;
const NEIGHBOURS_PAIRS: u32 = 4;
const NEIGHBOURS_TREE: u32 = 8;

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
    pub const ALL: [Self; 2] = [Self::AllPairs, Self::Tree];

    /// The name in `collision.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::AllPairs => "all-pairs",
            Self::Tree => "tree",
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

/// The buffers and bind group, sized for up to `capacity` particles.
struct Buffers {
    capacity: usize,
    particles: wgpu::Buffer,
    readback: wgpu::Buffer,
    group: wgpu::BindGroup,
    // Kept alive for the bind group.
    _motion: wgpu::Buffer,
    _mean: wgpu::Buffer,
    _forces: wgpu::Buffer,
    _nodes: wgpu::Buffer,
}

/// The compute pipelines, one per kernel.
struct Pipelines {
    forces_pairs: wgpu::ComputePipeline,
    forces_tree: wgpu::ComputePipeline,
    kick: wgpu::ComputePipeline,
    heat_pairs: wgpu::ComputePipeline,
    heat_tree: wgpu::ComputePipeline,
    drift: wgpu::ComputePipeline,
    refit_leaves: wgpu::ComputePipeline,
    refit_level: wgpu::ComputePipeline,
}

/// The GPU particle stepper: a device, the compiled kernels, and buffers that grow with the scene.
pub struct GpuParticles {
    gpu: GpuDevice,
    pipelines: Pipelines,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    levels: wgpu::Buffer,
    buffers: Option<Buffers>,
    /// The algorithms; [`step`](Self::step) reads them every call, so a change applies at once.
    pub solver: GpuSolver,
    /// Damping ratios already worked out, by restitution: the inverse is a bisection, and a
    /// scene's particles share a handful of materials.
    damping_ratios: Vec<(f32, f32)>,
}

/// The deepest tree the level buffer has room for: `2^32` leaves, more than any buffer holds.
const MAX_LEVELS: u64 = 32;

impl GpuParticles {
    /// Find a GPU and compile the kernels for it, or `None` when there is no GPU.
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
        let device = &gpu.device;
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
            kick: pipeline("kick"),
            heat_pairs: pipeline("heat_pairs"),
            heat_tree: pipeline("heat_tree"),
            drift: pipeline("drift"),
            refit_leaves: pipeline("refit_leaves"),
            refit_level: pipeline("refit_level"),
        };
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle params"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // One record per tree level, `first` and `count`, each at its own aligned offset. Level `d`
        // of the implicit tree is nodes `2^d .. 2^(d+1)`, so the records never change.
        let mut records = vec![0u8; (MAX_LEVELS * LEVEL_STRIDE) as usize];
        for depth in 0..MAX_LEVELS as usize {
            let first = 1u32.checked_shl(depth as u32).unwrap_or(0);
            let at = depth * LEVEL_STRIDE as usize;
            records[at..at + 4].copy_from_slice(&first.to_le_bytes());
            records[at + 4..at + 8].copy_from_slice(&first.to_le_bytes());
        }
        let levels = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tree levels"),
            size: MAX_LEVELS * LEVEL_STRIDE,
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
            solver: GpuSolver::DEFAULT,
            damping_ratios: Vec::new(),
        }
    }

    /// The GPU this runs on, for display.
    pub fn describe(&self) -> String {
        self.gpu.describe()
    }

    /// Advance `scene` by `step` on the GPU, in as many substeps as [`substep_count`] asks for,
    /// with the algorithms in [`solver`](Self::solver), and return that count.
    ///
    /// `None`, with the scene untouched, when [`can_step_on_gpu`] says no or the GPU could not
    /// hand the result back; the caller then steps on the CPU instead. A zero step does nothing and
    /// returns `Some(0)`.
    pub fn step(&mut self, scene: &mut Scene, step: TimeStep) -> Option<u32> {
        if !can_step_on_gpu(scene) || self.gpu.has_failed() {
            return None;
        }
        if step.is_zero() || !step.is_valid() {
            return Some(0);
        }
        let substeps = substep_count(&scene.props, step);
        self.step_in(scene, step, substeps).then_some(substeps)
    }

    /// Advance `scene` by `step` in exactly `substeps` substeps. `false`, with the scene untouched,
    /// when the scene cannot be stepped here or the read-back failed.
    ///
    /// A GPU that is lost part-way (reset by the driver, or unplugged) makes `wgpu` panic from inside
    /// a submission, past its error handler; that panic is caught here, the device is marked as
    /// failed, and every later call returns at once, so the app carries on on the CPU. The scene is
    /// written only after a successful read-back, so it is untouched.
    pub fn step_in(&mut self, scene: &mut Scene, step: TimeStep, substeps: u32) -> bool {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_substeps(scene, step, substeps)
        }));
        match outcome {
            Ok(done) => done,
            Err(_) => {
                bevy::log::error!(
                    "physics GPU failed during a step; stepping on the CPU from now on"
                );
                self.gpu
                    .failed
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                false
            }
        }
    }

    fn run_substeps(&mut self, scene: &mut Scene, step: TimeStep, substeps: u32) -> bool {
        if !can_step_on_gpu(scene) || substeps == 0 || self.gpu.has_failed() {
            return false;
        }
        let count = scene.props.len();
        self.ensure_capacity(count);

        // The particles in Morton order, relative to their mean position.
        let positions: Vec<DVec3> = scene
            .props
            .iter()
            .map(|prop| prop.body.center() * METRES_PER_UNIT)
            .collect();
        let origin = positions.iter().sum::<DVec3>() / count as f64;
        let order = morton_order(&positions);
        let mut floats = Vec::with_capacity(count * FLOATS_PER_PARTICLE);
        for &index in &order {
            let prop = &scene.props[index];
            let Body::Sphere { radius, .. } = &prop.body else {
                return false;
            };
            let radius_m =
                f64::from(prop.contact_radius_units.unwrap_or(*radius)) * METRES_PER_UNIT;
            let position = positions[index] - origin;
            let velocity = prop.velocity_units_s * METRES_PER_UNIT;
            let zeta = self.damping_ratio(prop.material.restitution);
            let thermal = prop.material.thermal;
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
                0.0,
                thermal.map_or(0.0, |t| t.a),
                thermal.map_or(0.0, |t| t.b),
                thermal.map_or(0.0, |t| t.reference_energy_j_kg),
                0.0,
            ]);
        }
        let Some(buffers) = self.buffers.as_ref() else {
            return false;
        };
        let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        let queue = &self.gpu.queue;
        queue.write_buffer(&buffers.particles, 0, &bytes);

        // The tree's leaves: buckets of `GPU_LEAF_SIZE` particles, rounded up to a power of two.
        let leaves = count.div_ceil(GPU_LEAF_SIZE).next_power_of_two();
        let dt = (step.seconds() / f64::from(substeps)) as f32;
        let softening_m = f64::from(scene.softening_units.max(0.0)) * METRES_PER_UNIT;
        let smoothing_per_radius = SPH_SMOOTHING_FACTOR
            * (4.0 * std::f64::consts::PI / (3.0 * FCC_PACKING_FRACTION)).cbrt();
        let solver = if self.solver.is_valid() {
            self.solver
        } else {
            GpuSolver::DEFAULT
        };
        let mut params = Vec::with_capacity(48);
        params.extend_from_slice(&dt.to_le_bytes());
        params.extend_from_slice(&(count as u32).to_le_bytes());
        params.extend_from_slice(&((softening_m * softening_m) as f32).to_le_bytes());
        params.extend_from_slice(&(GRAVITATIONAL_CONSTANT as f32).to_le_bytes());
        params.extend_from_slice(&solver.opening_angle.to_le_bytes());
        params.extend_from_slice(&(leaves as u32).to_le_bytes());
        params.extend_from_slice(&(smoothing_per_radius as f32).to_le_bytes());
        params.extend_from_slice(&(FCC_PACKING_FRACTION as f32).to_le_bytes());
        params.extend_from_slice(&(SOFT_CONTACT_MAX_NEIGHBOURS as f32).to_le_bytes());
        params.extend_from_slice(&solver.flags().to_le_bytes());
        params.extend_from_slice(&(GPU_LEAF_SIZE as u32).to_le_bytes());
        params.extend_from_slice(&0u32.to_le_bytes());
        queue.write_buffer(&self.params, 0, &params);

        // Every substep, each its own submission, then copy the last state out. One submission per
        // substep keeps every piece of GPU work short: Windows resets a GPU whose single piece of
        // work runs for more than about two seconds, and a frame at `200,000` particles, submitted
        // whole, was reset (measured). Submitting costs microseconds; the queue runs them in order.
        fn run<'a>(
            pass: &mut wgpu::ComputePass<'a>,
            group: &'a wgpu::BindGroup,
            pipeline: &'a wgpu::ComputePipeline,
            items: usize,
            level: u32,
        ) {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[level * LEVEL_STRIDE as u32]);
            pass.dispatch_workgroups((items as u32).div_ceil(GPU_WORKGROUP_SIZE), 1, 1);
        }
        let p = &self.pipelines;
        let group = &buffers.group;
        let depth = leaves.trailing_zeros();
        let length = count as u64 * BYTES_PER_PARTICLE;
        for substep in 0..substeps {
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("particle substep"),
                    });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("particle substep"),
                    timestamp_writes: None,
                });
                let mut dispatch =
                    |pipeline, items, level| run(&mut pass, group, pipeline, items, level);
                if solver.uses_tree() {
                    dispatch(&p.refit_leaves, leaves, 0);
                    for level in (0..depth).rev() {
                        dispatch(&p.refit_level, 1 << level, level);
                    }
                }
                if solver.gravity == GravitySolver::Direct
                    || solver.neighbours == NeighbourSearch::AllPairs
                {
                    dispatch(&p.forces_pairs, count, 0);
                }
                if solver.uses_tree() {
                    dispatch(&p.forces_tree, count, 0);
                }
                dispatch(&p.kick, count, 0);
                match solver.neighbours {
                    NeighbourSearch::AllPairs => dispatch(&p.heat_pairs, count, 0),
                    NeighbourSearch::Tree => dispatch(&p.heat_tree, count, 0),
                }
                dispatch(&p.drift, count, 0);
            }
            if substep + 1 == substeps {
                encoder.copy_buffer_to_buffer(&buffers.particles, 0, &buffers.readback, 0, length);
            }
            queue.submit(Some(encoder.finish()));
        }

        // Wait for it and read it.
        let slice = buffers.readback.slice(..length);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |outcome| {
            let _ = sender.send(outcome);
        });
        self.gpu.device.poll(wgpu::Maintain::Wait);
        if !matches!(receiver.recv(), Ok(Ok(()))) || self.gpu.has_failed() {
            return false;
        }
        let values: Vec<f32> = {
            let view = slice.get_mapped_range();
            view.chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        };
        buffers.readback.unmap();

        // Back into the scene, in `f64`, undoing the Morton order.
        for (&index, particle) in order.iter().zip(values.chunks_exact(FLOATS_PER_PARTICLE)) {
            let prop = &mut scene.props[index];
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
        }
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
    /// two, which is also the tree's leaf count) if not.
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
        let storage = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        let n = capacity as u64;
        let particles = storage("particles", n * BYTES_PER_PARTICLE);
        let motion = storage("particle motion", n * BYTES_PER_MOTION);
        let mean = storage("particle mean motion", n * BYTES_PER_MOTION);
        let forces = storage("particle forces", n * BYTES_PER_MOTION);
        let leaves = (capacity / GPU_LEAF_SIZE).max(1);
        let nodes = storage("tree nodes", 2 * leaves as u64 * BYTES_PER_NODE);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles readback"),
            size: n * BYTES_PER_PARTICLE,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particles"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particles.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: motion.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: mean.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: forces.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: nodes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.levels,
                        offset: 0,
                        size: wgpu::BufferSize::new(8),
                    }),
                },
            ],
        });
        self.buffers = Some(Buffers {
            capacity,
            particles,
            readback,
            group,
            _motion: motion,
            _mean: mean,
            _forces: forces,
            _nodes: nodes,
        });
    }
}

#[cfg(test)]
#[path = "../../tst/compute/particles_tests.rs"]
mod tests;
