//! Every constant the `compute` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module.
//!
//! # Sections
//!
//! - **The kernel** — [`GPU_WORKGROUP_SIZE`], the tile the particle shader works through.
//! - **The device** — [`GPU_PHYSICS_ENV`], the switch that turns the GPU path off for one run, and
//!   [`GPU_PROFILE_ENV`] with [`GPU_PROFILE_WINDOW_FRAMES`], the GPU pass timer.
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

/// The environment variable that turns the GPU pass timer on for one run: `RGA_GPU_PROFILE=on`.
///
/// **Why this name.** Beside [`GPU_PHYSICS_ENV`] and the other `RGA_*` switches. Off unless set,
/// because the timer splits each substep into several passes.
///
/// **Used by.** [`crate::compute::profiler::gpu_profile_enabled`].
///
/// **If you change it.** Update `src/compute/README.md` and `src/performance/README.md`, which name
/// it.
pub const GPU_PROFILE_ENV: &str = "RGA_GPU_PROFILE";

/// Timed frames the GPU pass timer averages over before the overlay's figures change.
///
/// **Why this value.** `30`: about a second of timed frames at the frame rates a large scene runs
/// at (the timer times every other frame or so, while the last frame's timestamps are read back),
/// long enough to smooth a frame's jitter and short enough to follow a collision as it develops.
///
/// **Used by.** [`crate::compute::profiler`].
///
/// **If you change it.** Only the overlay's smoothing changes; the benchmark's line averages over
/// its whole window.
pub const GPU_PROFILE_WINDOW_FRAMES: u32 = 30;

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

/// How many frames apart the GPU's particles are copied back into the CPU's scene, while a scene
/// is kept on the GPU.
///
/// **Why this value.** `15`, four times a second at `60` frames a second. The renderer draws the
/// GPU's own buffer, so the copy feeds only what reads the CPU's scene: the overlay's rows, the
/// impact analysis, a relaunch's fallback. Copying `110,000` particles (`14 MB`) and converting
/// them to `f64` every frame was a sizeable share of the frame (measured: `40–95 ms` a frame with
/// the upload and sort it replaced).
///
/// **Used by.** [`crate::compute::particles::GpuParticles::sync`].
///
/// **If you change it.** Smaller keeps the overlay fresher at the copy's cost; larger lets the CPU's
/// scene trail further behind what is drawn.
pub const GPU_READBACK_INTERVAL_FRAMES: u32 = 15;

/// The fewest substeps queued for one advance of a scene kept on the GPU.
///
/// **Why this value.** `1`. The GPU chooses each substep's length; the host queues as many as the
/// last advance took and a margin, but no more than fit [`GPU_FRAME_TIME_TARGET_SECONDS`]. A floor
/// above one would override that budget where a substep alone costs most of a frame (`50 ms` at
/// `110,000` particles), which is where it matters most.
///
/// **Used by.** [`crate::compute::particles::GpuParticles::advance`].
///
/// **If you change it.** Larger forces more substeps a frame than the time budget allows.
pub const GPU_MIN_SUBSTEP_BUDGET: u32 = 1;

/// The real time one frame's queued GPU substeps aim to fit in, seconds.
///
/// The clock asks for real time × the time scale each frame. When the GPU cannot step that much in
/// a frame, queuing every substep it needs makes the frame longer, the next frame's step longer
/// still, and the frame rate collapses: at `22,000` particles the app fell to `2` frames a second
/// (measured: `506 ms` a frame, about fifty substeps of `10 ms`). Instead the host queues only as
/// many substeps as fit this budget, from each substep's cost measured over recent frames, and the
/// simulation falls behind the clock, carrying at most a frame's time over and dropping the rest:
/// it runs in slow motion at a usable frame rate.
///
/// **Why this value.** `1/30 s`: thirty frames a second leaves the camera and the menu responsive.
///
/// **Used by.** [`crate::compute::particles::GpuParticles::advance`].
///
/// **If you change it.** Larger lets the simulation keep closer to its clock at a lower frame rate;
/// smaller keeps the frame rate higher and the simulation slower.
pub const GPU_FRAME_TIME_TARGET_SECONDS: f32 = 1.0 / 30.0;

/// Time bins kept below the shortest step at the start of each macro step of the GPU's individual
/// time steps.
///
/// Each particle steps in a power-of-two multiple of a tick (Springel 2005, section 4.2); the tick is
/// set at each macro step's start from the shortest step any particle then allows. A particle whose
/// allowed step shrinks during the macro step (one starting to fall into an impact) can only go down
/// to one tick, so the tick is the shortest step over `2^headroom`.
///
/// **Why this value.** `0`: the tick is the shortest step allowed at the macro step's start, and a
/// particle whose allowed step shrinks below it before the next macro step (never more than a frame
/// or two away) steps at one tick, a little over its limit; every limit has a safety factor (the
/// fluids' Courant `0.3`). Measured on an RTX 4070 SUPER, fluid `experiment_8` at `20,000`
/// particles a planet: `1` doubled the ticks (`8` to `15` a frame) and cost `30%` (direct) and
/// `20%` (tree) of the frame for no measured gain; with `0` the rubble giant impact keeps its energy
/// within `2.5e-3` of the binding energy with every algorithm.
///
/// **Used by.** `particles.wgsl`'s `plan`, through [`crate::compute::particles::GpuParticles`].
///
/// **If you change it.** Larger is safer against sudden collapses, at twice the ticks per bin.
pub const GPU_TIME_BIN_HEADROOM: u32 = 0;

/// The most time bins a macro step of the GPU's individual time steps spans: its longest step is at
/// most `2^GPU_MAX_TIME_BINS` ticks.
///
/// **Why this value.** `10`: a factor of `1,024` between the shortest and longest step covers a
/// planet's dense core against its far debris (measured on `experiment_8`: the step limits span
/// about two orders of magnitude) with room to spare, and keeps the tick counter far from overflow.
///
/// **Used by.** `particles.wgsl`'s `plan`, through [`crate::compute::particles::GpuParticles`].
///
/// **If you change it.** Larger lets slow particles step even less often; a macro step is never
/// longer than the time left in the frame, so it rarely binds.
pub const GPU_MAX_TIME_BINS: u32 = 10;

/// The most a particle's time step may exceed a neighbour's, in the GPU's individual time steps.
///
/// A particle in a long step answers its neighbours with the force from the start of that step;
/// against a neighbour in a short step (a contact, a shock) that is wrong for most of the step, and
/// the energy books break: measured on the rubble giant impact without a limit, `7.9%` of the
/// binding energy gained, against `3e-4` with every particle in one step. Saitoh & Makino's limiter
/// keeps neighbours' steps within this ratio, passively (a particle's step limit is at most this
/// times its neighbours') and by waking a particle whose neighbour finds it stepping longer.
///
/// Saitoh, T. R. & Makino, J. (2009), A necessary condition for individual time steps in SPH
/// simulations, *ApJ* 697(2), L99–L102, doi:10.1088/0004-637X/697/2/L99, section II, eqs. (1)–(2):
/// "We found f=4 to give good results", and larger values violate the Courant condition (read from
/// arXiv:0808.0773v2, 2026-11-22).
///
/// **Used by.** `particles.wgsl`'s `limit_neighbour` and `limits`, through
/// [`crate::compute::particles::GpuParticles`].
///
/// **If you change it.** Smaller keeps neighbours closer in step, at more evaluations; larger lets
/// stale forces back in.
pub const GPU_NEIGHBOUR_STEP_RATIO: f64 = 4.0;
