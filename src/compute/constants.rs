//! Every constant the `compute` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module.
//!
//! # Sections
//!
//! - **The kernel** — [`GPU_WORKGROUP_SIZE`], the tile the particle shader works through.
//! - **The device** — [`GPU_PHYSICS_ENV`], the switch that turns the GPU path off for one run.
//! - **The solver** — [`DEFAULT_OPENING_ANGLE`] and its range, for the tree gravity.

/// How many particles one GPU workgroup handles, and so how many each tile of the N-body loop
/// loads into workgroup memory at once.
///
/// **Why this value.** `64`: a multiple of every vendor's SIMD width (32 on NVIDIA, 32 or 64 on AMD,
/// 8 to 32 on Intel and Apple), so no lane sits idle in a full workgroup, and a tile of `64`
/// particles is `3 KB` of workgroup memory, far under the `16 KB` WebGPU guarantees. Larger
/// workgroups reuse each tile across more particles but leave more lanes idle on the last,
/// partial workgroup and fit fewer workgroups per compute unit; `64` to `256` is the usual range
/// for tiled N-body.
///
/// **Used by.** [`crate::compute::particles`], to size the dispatch, and `particles.wgsl`'s
/// `WORKGROUP`, which must equal it; [`crate::compute::particles::GpuParticles::new`] checks.
///
/// **If you change it.** Change the shader's `WORKGROUP` to match. Above `256` some devices refuse
/// the pipeline.
pub const GPU_WORKGROUP_SIZE: u32 = 64;

/// The environment variable that turns the GPU physics off for one run: `RGA_GPU_PHYSICS=off`.
///
/// **Why this name.** Every other per-run switch is an `RGA_*` variable
/// (see [`crate::performance`]), and this sits beside them.
///
/// **Used by.** [`crate::compute::device::gpu_physics_enabled`].
///
/// **If you change it.** Update `config/README.md` and the root `README.md`, which name it.
pub const GPU_PHYSICS_ENV: &str = "RGA_GPU_PHYSICS";

/// The Barnes–Hut opening angle `θ` the GPU's tree gravity starts with: a group of particles is
/// treated as one point mass at its centre of mass when its size is less than `θ` times its
/// distance. Dimensionless.
///
/// **Why this value.** `0.5`, the usual choice since Barnes & Hut (1986, Nature 324:446): the force
/// on a particle is then within about a percent of the exact sum for clustered matter, at a cost
/// that grows as `N·log N` rather than `N²`. Smaller is more exact and slower.
///
/// **Used by.** [`crate::compute::particles::GpuSolver::DEFAULT`]; the menu changes it at run time
/// between [`MIN_OPENING_ANGLE`] and [`MAX_OPENING_ANGLE`].
///
/// **If you change it.** At `0` every node is opened and the tree reproduces the direct sum, slowly.
/// Above `1` a node can be accepted from inside its own box's reach, and the error grows quickly.
pub const DEFAULT_OPENING_ANGLE: f32 = 0.5;

/// The smallest opening angle the menu offers. See [`DEFAULT_OPENING_ANGLE`].
///
/// **Why this value.** `0.1`: the force error is then far below the `f32` the GPU works in, and the
/// tree is slower than the direct sum for any scene the crate builds, so smaller buys nothing.
///
/// **Used by.** The menu and [`crate::compute::particles::GpuSolver::is_valid`].
///
/// **If you change it.** Keep it above `0`; at `0` the tree opens every node.
pub const MIN_OPENING_ANGLE: f32 = 0.1;

/// The largest opening angle the menu offers. See [`DEFAULT_OPENING_ANGLE`].
///
/// **Why this value.** `1.0`, the top of the range in use (Barnes & Hut used up to `1`); above it
/// the monopole error grows faster than the time saved.
///
/// **Used by.** As [`MIN_OPENING_ANGLE`].
///
/// **If you change it.** Above `1` the force can be wrong by tens of percent near dense clumps.
pub const MAX_OPENING_ANGLE: f32 = 1.0;

/// How many particles each leaf of the GPU's tree holds.
///
/// **Why this value.** `16`. A tree walk visits nodes one at a time from global memory, which costs
/// far more per visit than the direct sum's loop over particles staged in workgroup memory; with one
/// particle a leaf, the walk spent its time on the near field, visiting each neighbour as its own
/// node, and was slower than the direct sum at every size measured up to `100,000` particles.
/// Buckets of `8` to `32` are usual in GPU tree codes (Bédorf et al. 2012, J. Comput. Phys.
/// 231:2825, use `16`).
///
/// **Used by.** [`crate::compute::particles`], and `particles.wgsl` through its `leaf_size`.
///
/// **If you change it.** A power of two keeps the leaves aligned with the particle buffer's
/// capacity. Larger leaves are cheaper to walk to and costlier to open.
pub const GPU_LEAF_SIZE: usize = 16;
