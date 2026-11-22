# `src/compute/` — GPU physics

> **Purpose:** Stepping a scene made only of particles on the GPU: finding a device, the kernels
> (`particles.wgsl`), the switchable algorithms (exact pairs or a tree, for gravity and for
> neighbours), and the stepper that uploads the scene, runs a frame's substeps and reads it back.
> **Edit here to:** change a GPU kernel, the algorithms, the tree, the workgroup or leaf size, how
> the device is chosen, or the `RGA_GPU_PHYSICS` switch.
> **Not here:** the CPU physics it mirrors is in [`collision/`](../collision/README.md) and
> [`core/`](../core/README.md); the Bevy plugin and resources that put it in the app are in
> [`render/`](../render/README.md) (`GpuPhysicsPlugin`, `PhysicsBackend`, the `GpuSolver`
> resource); the menu rows are in [`ui/`](../ui/README.md); the file is `collision.toml`.

The CPU physics adds up gravity and contact one pair at a time, in `f64`. For a planet built from
thousands of particles that sum over every pair is the whole cost, and it is exactly the work a GPU
does well. This layer runs it there for scenes made only of particles, with the same laws as the
CPU: gravity, the spring-dashpot contact, friction and spin, the thermal pressure of hot rock, and
the heat the contacts make. The app falls back to the CPU for everything else.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer and its import restrictions. |
| `constants.rs` | `GPU_WORKGROUP_SIZE` (`64`), `GPU_LEAF_SIZE` (`16`), the opening angle's default and range, and `GPU_PHYSICS_ENV` (`RGA_GPU_PHYSICS`). |
| `device.rs` | `GpuDevice::request`: the best GPU, or `None`; it logs and remembers a lost device or an error instead of letting `wgpu` panic. `gpu_physics_enabled`: the switch. |
| `particles.rs` | `GpuParticles`: compile the kernels, `step` a scene with the `GpuSolver` it holds, `step_in` at a given substep count. `GpuSolver`, `GravitySolver`, `NeighbourSearch`. `can_step_on_gpu`. `morton_order`. |
| `particles.wgsl` | The kernels: the tree refit, the forces (tiled all-pairs, and the tree walk), the kick, the heat (both ways), and the drift. |

## The algorithms

Chosen in `Esc` > Configuration > Collisions or `collision.toml`, applied from the next frame,
separately for the two jobs:

| | Exact, O(N²) | Tree, O(N log N) |
| --- | --- | --- |
| **Gravity** | `direct`: every pair, a tile of `64` particles at a time through workgroup memory | `barnes-hut`: distant groups as one point mass when their size is under `θ` times their distance |
| **Neighbours** (contact, pressure, heat) | `all-pairs`: every pair tested | `tree`: only the leaves whose box reaches the particle; exact, nothing is missed |

**The tree.** Each frame the CPU sorts the particles along a Morton (Z-order) curve, which puts
particles that are near each other next to each other, and lays an implicit binary tree over that
order: node `n` has children `2n` and `2n + 1`, and leaf `L + l` holds the `l`-th bucket of `16`
particles. Every substep the GPU refits it, a level per dispatch from the leaves up: each node's
mass, centre of mass, the box around its particles' centres (Barnes–Hut's size) and the box around
their reach (what the neighbour search must open). The shape is fixed for the frame; the boxes are
always exact.

## A substep

```text
refit_leaves, refit_level × depth   (only when a tree is used)
forces_pairs and/or forces_tree      acceleration and angular acceleration from the start state
kick                                 new velocity and spin; their mean over the substep
heat_pairs or heat_tree              the heat, from the pair forces at the mean velocities
drift                                position at the new velocity; heat applied
```

The same order as the CPU: contact forces, gravity, then the heat booked at the mean of the start
and end velocities, which is what keeps the energy books exact (see `collision/README.md`). Each
substep is its own submission: Windows resets a GPU whose single piece of work runs past about two
seconds, which a whole frame at `200,000` particles did.

## Agreement with the CPU

Measured on an RTX 4070 SUPER:

- **Exact solver, short run.** Two planets settling for an hour at the same substeps: the furthest
  any particle drifts from its CPU twin is tens of metres on `1,500 km` particles, and velocities
  agree within `1 m/s`.
- **`experiment_1` with every algorithm.** All four combinations meet at `14.00 h`, merge with
  nothing escaping, and hold energy (heat included) within `0.06%` of the binding energy, as the
  CPU does. Momentum is left at `3e-7` of the impact's with the exact gravity and `3e-3` with
  Barnes–Hut.
- **`experiment_8`.** The exact solver keeps the angular momentum to four digits over twelve hours;
  Barnes–Hut gains `0.33%`, because its forces are not equal and opposite (a particle sees a
  distant group as one mass, the group sees the particle through other groupings).

## Speed

One frame of `experiment_1` (`33` simulated seconds), physics only:

| Particles per planet (bodies) | Direct + all pairs | Direct + tree | Barnes–Hut + all pairs | Barnes–Hut + tree |
| --- | --- | --- | --- | --- |
| `200` (`220`) | **`0.8 ms`** | `1.5 ms` | `1.2 ms` | `1.3 ms` |
| `2,000` (`2,210`) | **`3.8 ms`** | `9.2 ms` | `8.4 ms` | `9.6 ms` |
| `20,000` (`22,892`) | **`84 ms`** | `94 ms` | `164 ms` | `114 ms` |
| `100,000` (`114,236`) | `2.24 s` | `1.60 s` | `2.73 s` | **`0.85 s`** |
| `200,000` (`228,784`) | — | — | `5.0 s` | **`2.3 s`** |

The tiled direct sum is so efficient on a GPU (about `2e11` pair checks a second here) that the
trees, which walk nodes one at a time from global memory, win only past a few tens of thousands of
particles. Above that the gap grows as `N²` against `N log N`. So the default is the exact pair,
and the menu offers the trees for big runs. In the running app `experiment_1` keeps up with its
`2000×` clock to about `5,000` particles per planet (`68` frames a second); past that the clock
falls behind. At `100,000` it runs, at a frame every few seconds.

Two attempts that did not help, kept here so they are not tried blind again: one particle per leaf
(the walk then spent its time visiting the near field one node at a time; buckets of `16` cut the
`100,000`-particle frame from `2.05 s` to `0.80 s`), and a walk shared by each workgroup, with one
thread building the interaction list (slower at every size: `138 ms` against `115 ms` at `23,000`).

## What it does not do

| Missing | Consequence |
| --- | --- |
| Mixed scenes | A scene with any prop that is not a particle steps on the CPU. |
| Higher multipoles | Tree nodes pull as point masses; a quadrupole would allow a larger `θ` for the same error. |
| A cooperative tree walk | Each particle walks alone, so threads diverge. Production codes (Bonsai) share the walk across a warp with a list built in parallel; that is the next step if the trees must win sooner. |
| `f64` | `f32`, with positions relative to the particles (`6 m` across a `1e8 m` scene). |
| Overlap with rendering | The read-back blocks until the GPU is done. |

## Rules and invariants

- **The CPU path is the reference.** A change to a law in `collision/` or `core/` that a kernel
  mirrors changes the kernel too, and `tst/compute/particles_tests.rs` holds them together.
- **`WORKGROUP` in the shader equals `GPU_WORKGROUP_SIZE`.** `GpuParticles::new` asserts it.
- **Every product in a kernel is grouped so no intermediate overflows `f32`**: two planet-sized
  particle masses multiplied are `1e44`, past `f32`'s `3.4e38`.
- **No GPU, or a lost one, is not an error.** Every entry point returns `None` or `false` and leaves
  the scene as it was, and the caller steps on the CPU.

## Import restrictions

`wgpu`, `bevy::tasks` (to block on the device), `bevy::log`, and the crate's `core`, `collision`,
`physics_utils`, `time` and `constants`. Nothing from `bevy::ecs` or the layers above.
