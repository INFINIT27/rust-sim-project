# `src/compute/` — GPU physics

> **Purpose:** Stepping a scene made only of particles on the GPU: finding a device, the kernels
> (`particles.wgsl`), the switchable algorithms (exact pairs or a tree, for gravity and for
> neighbours), and the stepper that uploads the scene, keeps the scene on the GPU, advances it in individually stepped
> ticks, and copies it back now and then.
> **Edit here to:** change a GPU kernel, the algorithms, the tree, the workgroup or leaf size, how
> the device is chosen, or the `RGA_GPU_PHYSICS` switch.
> **Not here:** the CPU physics it mirrors is in [`collision/`](../collision/README.md) and
> [`core/`](../core/README.md); the Bevy plugin and resources that put it in the app are in
> [`render/`](../render/README.md) (`GpuPhysicsPlugin`, `PhysicsBackend`, the `GpuSolver`
> resource); the menu rows are in [`ui/`](../ui/README.md); the file is `collision.toml`.

The CPU physics adds up gravity and contact one pair at a time, in `f64`. For a planet built from
thousands of particles that sum over every pair is the whole cost, and it is exactly the work a GPU
does well. This layer runs it there for scenes made only of particles, with the same laws as the
CPU: gravity, the spring-dashpot contact, friction and spin, the thermal pressure of hot rock, the
smoothed-particle hydrodynamics of fluid planets (density, Tillotson pressure, artificial viscosity),
and the heat the contacts make. The app falls back to the CPU for everything else.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer and its import restrictions. |
| `constants.rs` | `GPU_WORKGROUP_SIZE` (`64`), `GPU_LEAF_SIZE` (`16`), the opening angle's default and range, `GPU_PHYSICS_ENV` (`RGA_GPU_PHYSICS`), the pass timer's `GPU_PROFILE_ENV` (`RGA_GPU_PROFILE`) and `GPU_PROFILE_WINDOW_FRAMES` (`30`), the residency and frame budget (`GPU_READBACK_INTERVAL_FRAMES`, `GPU_FRAME_TIME_TARGET_SECONDS`, `GPU_MIN_SUBSTEP_BUDGET`), and the individual time steps (`GPU_TIME_BIN_HEADROOM`, `GPU_MAX_TIME_BINS`, `GPU_NEIGHBOUR_STEP_RATIO`). |
| `device.rs` | `GpuDevice::from_render`: the physics on Bevy's own device, so the renderer can draw its buffer. `GpuDevice::request`: a device of its own (headless), or `None`; it logs and remembers a lost device or an error instead of letting `wgpu` panic. `gpu_physics_enabled`: the switch. |
| `particles.rs` | `GpuParticles`: compile the kernels; keep a scene on the GPU (`upload`, `holds`, `advance`, `sync`, `particle_buffer`, `simulated_seconds`); for tests, `step` a scene in one blocking call and `step_in` at a given fixed step count. `GpuSolver`, `GravitySolver`, `NeighbourSearch`. `can_step_on_gpu`. `morton_order`. |
| `profiler.rs` | The GPU pass timer, on with `RGA_GPU_PROFILE=on`: each group of kernels (`GpuPhase`: sort, restore, drift, refit, grid, density, pairs, gravity, tree, contacts, kick) as its own compute pass with a timestamp at each end (`wgpu::Features::TIMESTAMP_QUERY`), read back without waiting, and averaged a frame (`GpuProfile`): per phase, between passes, ticks run against substeps queued, and the fraction of particles active a tick (`activate`'s count, `Control::active_sum`). Shown in the debug overlay and printed by the benchmark. |
| `particles.wgsl` | The kernels: the tick and its timing (`tick`, `activate`, `limits`, `plan`, the kicks), the drift, the tree refit, the fluid density (both ways), the forces with their heating (tiled all-pairs, and the tree walk), and the GPU sort. |

## The algorithms

Chosen in `Esc` > Configuration > Collisions or `collision.toml`, applied from the next frame,
separately for the two jobs:

| | Exact, O(N²) | Tree, O(N log N) | Grid, O(N) |
| --- | --- | --- | --- |
| **Gravity** | `direct`: every pair, a tile of `64` particles at a time through workgroup memory | `barnes-hut`: distant groups by their monopole and quadrupole when their size is under `θ` times their distance, walked by each workgroup together | — |
| **Neighbours** (contact, pressure, heat) | `all-pairs`: every pair tested | `tree`: only the leaves whose box reaches the particle | `grid`: only the 125 cells of a grid around the particle, the grid rebuilt every tick |

Every neighbour search is exact: nothing within reach is missed.

**The tree.** At the start of each frame the GPU sorts the particles along a Morton (Z-order)
curve, which puts particles that are near each other next to each other, and lays an implicit binary
tree over that order: node `n` has children `2n` and `2n + 1`, and leaf `L + l` holds the `l`-th
bucket of `16` particles. Every tick the GPU refits it, a level per dispatch from the leaves up:
each node's mass, centre of mass, quadrupole about it (per unit mass, so a planet's `1e38 kg·m²`
fits in `f32`), the box around its particles' centres (Barnes–Hut's size) and the box around their
reach (what the neighbour search must open). The shape is fixed for the frame; the boxes and moments
are always exact. (Not a Karras radix tree: the splits are by index, not by Morton prefix. With the
re-sort every frame the boxes stay tight enough that it has not been worth it.)

**The cooperative walk** (`gravity_group`). A workgroup's `64` particles are neighbours in Morton
order, so they walk the tree together, a level at a time, testing nodes against the box around all
`64` (if a node is far enough for the box it is far enough for each particle), and building shared
lists of far nodes and leaves that every thread then sums for its own particle, as Bonsai builds its
interaction lists with the threads of a group of particles (Bédorf, Gaburov & Portegies Zwart 2012,
*J. Comput. Phys.* 231, 2825, doi:10.1016/j.jcp.2011.12.024, sections 2.2 and 3.3; its quadrupoles,
section 3.2). The stricter opening test makes it about ten times more
accurate than each particle walking alone at the same `θ`, and it is faster. A tree level wider than
the shared frontier sends the workgroup back to the walk each thread does alone. (An earlier attempt
with one thread building the lists was slower than the per-thread walk: the list must be built in
parallel.)

**The cell list** (`grid_*`, `density_grid`, `forces_grid`). Cells are the largest reach; cells are
hashed into twice as many buckets as particles (the primes commonly attributed to Teschner et al.
2003, not checked against the paper's text), so the grid needs no
bounds. A counting sort builds it every tick: count each bucket, a multi-level exclusive prefix sum
(`scan_up`, `scan_down`), scatter the particle indices. A candidate counts only if it is in the cell
being visited, so two cells sharing a bucket are never counted twice.

**Force error against the direct sum** (`the_tree_s_forces_match_the_direct_sum`; median, 99th
percentile; `experiment_8`'s planets spread apart so only gravity acts):

| `θ` | Monopole, each particle alone | Quadrupole, each alone | Quadrupole, cooperative |
| --- | --- | --- | --- |
| `0.3` | `4.7e-4`, `1.5e-3` | `3.2e-5`, `9.0e-5` | `7.7e-7`, `2.1e-5` |
| `0.5` | `4.3e-3`, `2.1e-2` | `5.2e-4`, `2.9e-3` | `5.0e-5`, `1.0e-3` |
| `0.7` | `1.1e-2`, `4.2e-2` | `2.6e-3`, `1.7e-2` | `1.7e-4`, `3.6e-3` |
| `0.9` | `2.1e-2`, `7.0e-2` | `6.9e-3`, `4.5e-2` | `6.0e-4`, `3.0e-2` |

The cooperative walk at `θ = 0.9` is seven times more accurate (median) than the old default,
monopoles walked by each particle at `θ = 0.5`, and much faster; see Speed.

## Time: residency, ticks and individual steps

A scene of particles is **uploaded once** (`upload`, when `Scene::revision` changes) and lives on the
GPU: every frame the app calls `advance` (never waits), the renderer draws the particle buffer
itself (`render::gpu_particles`), and `sync` copies the particles back into the CPU's scene every
`GPU_READBACK_INTERVAL_FRAMES` frames for the overlay, trails and analysis. The GPU decides its own
timing in the `Control` record; the host reads it back a frame late.

Time advances in **ticks** with **individual power-of-two time steps** and a **kick-drift-kick
leapfrog** (Springel 2005, *MNRAS* 364, 1105, doi:10.1111/j.1365-2966.2005.09655.x, section 4.2):
a macro step is `2^bins` ticks, planned on the GPU (`plan`) at its start from the shortest and
longest step any particle allows (the CPU's `substep_count` criteria, per particle: displacement,
pair dynamical time, contact duration, the fluids' Courant step). Each particle steps every `2^k`
ticks for its own `k`; it is *active* (forces evaluated, kicked) only on the tick its step ends.
Neighbours' steps stay within `GPU_NEIGHBOUR_STEP_RATIO` (`4`) of each other by Saitoh & Makino's
limiter (*ApJ* 697, L99, doi:10.1088/0004-637X/697/2/L99, section II), passively and by waking a
particle in too long a step (its kick corrected as in Durier & Dalla Vecchia 2012, *MNRAS* 419, 465,
appendix D).

One tick, each its own submission (Windows resets a GPU whose one piece of work runs past about two
seconds):

```text
tick                                 does this tick run; the tick counter
drift                                every particle at its half-step velocity (exact: whole f32
                                     grains of the position, the rest kept beside it)
refit_leaves, refit_level × depth   (only when a tree is used)
activate                             whose steps end now; everyone's velocity predicted to now
density_pairs or density_tree        active fluids: density, pressure, sound speed
limits_first                         (first tick after an upload only) every step limit
forces_pairs and/or forces_tree      active particles: acceleration, angular acceleration, heating
close_kick                           the closing half kick; a fluid's smoothing length and radius
limits                               each active particle's step limit
plan                                 at a macro step's end: the next one's tick and bins
open_kick                            each active particle's next step and opening half kick; a
                                     woken particle's step shortened
```

At the start of each advance the particles are re-sorted along a Morton curve on the GPU (`bounds`,
`morton`, a bitonic sort, `gather`), carrying their integrator state with them. A frame never queues
more ticks than fit `GPU_FRAME_TIME_TARGET_SECONDS`; when the GPU cannot keep up the simulation runs
in slow motion and the clock shows the time simulated.

Three things learned building it, kept so they are not relearned:

- **The pair forces see the half-step velocities**, the heat the predicted ones. Friction's drag cap
  holds only if the force is evaluated from the velocity the kick then changes; from the predicted
  velocity a sticking contact overshoots and friction pumps energy in (`10%` of the binding energy
  in two hours of settling). Fluids use the predicted velocity, as GADGET-2 does.
- **The first tick after an upload computes the step limits before the forces** (`limits_first`):
  otherwise friction is uncapped for the first half kick (`0.6%` of the binding energy an hour,
  uploading every minute).
- **Kahan summation does not survive the GPU compiler**: `(x + s − x) − s` was folded to zero
  (measured: no effect). The drift instead moves the position by whole multiples of its own f32
  spacing and keeps the remainder. Without it a particle `2.7e8 m` from the origin did not move at
  all at fine steps.

The leapfrog is second order (`the_gpu_leapfrog_is_second_order_on_an_orbit`: errors `2,506`,
`626`, `39 m` at `8`, `16`, `64` steps, ratio `4.21`). The planets themselves converge only at
first order: the rubble's drag cap scales as `1/dt` by design, and the fluid's error falls by about
two per halving, most likely from the SPH terms' kinks (viscosity only on approach, pressure floored
at zero; not proven).

## Agreement with the CPU

The frozen CPU path integrates with one global step and symplectic Euler, whose velocities sit
`a·dt/2` from the leapfrog's, so the two no longer agree to the rounding (before 2026-11-22 they
did: `16 m` and `55 m`). Measured on an RTX 4070 SUPER:

- **Rubble, an hour of settling:** `740 m` at worst on `1,500 km` particles, velocities within
  `0.051 m/s`.
- **Fluid, ten minutes of settling:** `1.6 km` after the first minute, `7.5 km` at worst after ten,
  densities within `2e-3`, heat within `1.9e4 J/kg`, with either neighbour search.
- **`experiment_1` with every algorithm.** All four combinations meet at `14.00–14.03 h`, merge
  with nothing escaping, and hold energy (heat included) within `2.5e-3` of the binding energy.
  Momentum is left at `1e-6` of the impact's with the exact gravity and `2e-3` with Barnes–Hut.
- **`experiment_8`.** Both solvers keep the angular momentum to five digits through the impact.
  Barnes–Hut gained `0.33%` before the quadrupoles and the cooperative walk: its forces are not
  equal and opposite, and were then ten times less accurate.

## Speed

One frame (`33` simulated seconds) of fluid `experiment_8` kept on the GPU, physics only, every
tick of the frame done, measured on an RTX 4070 SUPER (`measure_resident_frames`):

| Gravity, neighbours | `20,000` per planet (`22,116` bodies) | `100,000` per planet |
| --- | --- | --- |
| Direct, all pairs | `55 ms` | `1.64 s` |
| Direct, grid | `63 ms` | `1.12 s` |
| Barnes–Hut `θ = 0.5`, tree | `73 ms` | `642 ms` |
| Barnes–Hut `θ = 0.7`, tree | `53 ms` | `357 ms` |
| Barnes–Hut `θ = 0.9`, tree | `45 ms` | `234 ms` |
| Barnes–Hut `θ = 0.7`, grid | `50 ms` | `314 ms` |
| Barnes–Hut `θ = 0.9`, grid | **`42 ms`** | **`210 ms`** |

Before the performance work of 2026-11-22 (global substeps chosen on the CPU, monopoles, each
particle walking alone) Barnes–Hut at `θ = 0.5` with the tree took `101 ms` and `667 ms`. Barnes–Hut
at `θ = 0.9` with the grid is `2.4` and `3.2` times faster, and seven times more accurate (median
force error `6.0e-4` against `4.3e-3`). The default stays the exact pair at `θ = 0.5` for now
(changing it changes the shipped `collision.toml`); for big runs choose Barnes–Hut, `θ = 0.7` to
`0.9`, and the grid.

**Where a frame goes** (`measure_gpu_profile`, the pass timer; same GPU, fluid `experiment_8` at
`200,000` a planet, `221,518` bodies, a frame at a time as the app runs it). The frame budget fits
only one tick a frame at this size, and `74%` of the particles are active in a tick, so individual
time steps save little here. Milliseconds of GPU time a tick:

| Phase | Barnes–Hut `θ = 0.5`, tree | `θ = 0.7`, grid | `θ = 0.9`, grid |
| --- | --- | --- | --- |
| gravity (`gravity_group`) | `84` | `37–41` | `15–17` |
| neighbours (`forces_tree` / `forces_grid`) | `15` | `17–19` | `18–19` |
| density | `10` | `13–25` | `24–25` |
| sort, drift, refit, grid, kick together | `1.4` | `1.6` | `1.5` |
| **frame** | **`110`** | **`69–87`** | **`60–63`** |

The density pass varied between `13` and `25 ms` from one process to the next; the other phases
held within `10%`. The sort, the drift, the tree refit and the cell list cost about `2%` together,
so a radix sort or ticks that skip the inactive particles would buy little. The time is in the three
loops over neighbours and tree nodes. At `20,000` a planet the frame fits `3.7` ticks (`26 ms`),
with gravity `42%`, the grid's neighbours `30%` and the density `26%`.

The cell list only pays once gravity is cheap: with direct gravity the `N²` tile loop is the frame
whether or not it also tests neighbours.

## What it does not do

| Missing | Consequence |
| --- | --- |
| Mixed scenes | A scene with any prop that is not a particle steps on the CPU. |
| Higher multipoles, a radix tree | Nodes pull as monopole and quadrupole; an octupole, or a Karras radix tree whose splits follow the Morton prefixes, are the next steps for accuracy and for clustered debris. |
| `f64` | `f32`, with positions relative to the particles' mean at the upload; the drift is exact to the position's f32 spacing (`8 m` at `1e8 m`), the rest kept beside it. |
| Per-tick cost in proportion to the active particles | Every tick still drifts every particle, refits the tree and rebuilds the grid over all of them; only the density and forces skip the inactive. Measured by the pass timer at `200,000` a planet: those passes are `2%` of the frame, so this is not worth doing yet. |

## Rules and invariants

- **The GPU path is the simulation for particle scenes** (decided 2026-11-22). Planets built from
  particles, and anything more demanding, need a GPU; environments without one are not supported
  for them. New physics and performance work is built here only, with no CPU twin, and tested on
  its own terms: conservation, convergence with detail, and published results. The CPU particle
  code in `collision/` is a frozen fallback for a lost device and for building scenes, not kept in
  step with the kernels. (Until 2026-11-22 the CPU path was the reference every kernel was held to.)
- **`WORKGROUP` in the shader equals `GPU_WORKGROUP_SIZE`.** `GpuParticles::new` asserts it.
- **Every product in a kernel is grouped so no intermediate overflows `f32`**: two planet-sized
  particle masses multiplied are `1e44`, past `f32`'s `3.4e38`.
- **A lost GPU is not a crash.** Every entry point returns `None` or `false` and leaves the scene
  as it was, and the caller falls back to the frozen CPU path, which keeps the app running but is
  not the same simulation.

## Import restrictions

`wgpu`, `bevy::tasks` (to block on the device), `bevy::log`, `bevy::render::renderer` and
`bevy::render::render_resource` (to share Bevy's device and hand the renderer a buffer), and the crate's `core`, `collision`,
`physics_utils`, `time` and `constants`. Nothing from `bevy::ecs` or the layers above.
