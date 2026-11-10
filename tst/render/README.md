# `tst/render/` — tests for `src/render/`

> **Purpose:** Tests for [`src/render/`](../../src/render/README.md): the ECS mirror and
> sizing, the plugin's pipeline, missions driven through a headless `App`, eclipses, the grid,
> and trails.
> **Edit here to:** add or fix a test of drawing, mirroring, or mission handling.
> **Not here:** the render layer and its sizing contract, in `src/render/`.

Unit and property tests for the model → ECS boundary. See
`src/render/README.md` for the invariants under test, especially the three-file
sizing contract.

## What lives here

| File | Job |
| --- | --- |
| `bridge_tests.rs` | 9 tests for `transform_from` and `sync_scene_to_ecs_system`. |
| `plugin_tests.rs` | 11 tests for `ScenePlugin`, `SceneConfig`, `MirroredBody`, and `clamp_dt`. |
| `mission_tests.rs` | 7 tests: the origin/destination/cancel picks, and a headless `App` where `R` on Earth then on the Moon pauses the clock, plans, slows the clock for the short hop, and flies a rocket; nothing at the crosshair picks nothing; no craft, no launch. |

## What is covered

- **`transform_from`:** a property asserting finite translation/scale and
  unit-length rotation for any valid body; per-variant properties pinning the
  pose pass-through and each scale formula; a unit test for the degenerate
  `from == to` line.
- **`sync_scene_to_ecs_system`:** overwrites the `Transform` of a resolvable id,
  leaves an unresolvable id's `Transform` untouched, and does not mutate `Scene`
  — each in a `MinimalPlugins` app with the asset collections inserted by hand.
- **`ScenePlugin`:** inserts both resources; one tick yields one `MirroredBody`
  entity per body; `Scene` unchanged across 5 ticks.
- **`clamp_dt`:** spot checks for all three branches plus a property asserting
  the output is finite, in `[0, max_dt]`, and bit-exactly equal to a
  re-derivation of the spec branches.

## Known gaps, so you do not assume coverage you do not have

- **The `step` → `sync` ordering edge is asserted by nothing.** The plugin test
  concedes this in a comment; deleting `.after(step_scene_system)` leaves the
  suite green.
- **`render_bridge_does_not_mutate_scene` no longer asserts what its name says.**
  It used to compare `props` before and after five ticks, which only worked because
  `Scene::step` was a no-op. Now the integrator moves every movable prop every
  tick, so the test asserts the structural invariants a rogue write would break —
  prop count, id set and order, bounds, the immovable floor, and every prop's
  validity. Sync's actual mutation-freedom is compile-enforced by `Res<Scene>` and
  no runtime test can add to it.
- **The line's rotation clause is never checked.** Only unit magnitude is
  asserted, which the identity quaternion satisfies, so an implementation that
  dropped `from_rotation_arc` entirely would still pass.
- `sync_overwrites_transform_for_resolvable_ids` asserts translation and scale
  but never rotation, and uses a single entity with an identity quaternion. No
  test mixes resolvable and unresolvable entities in one world.
- The line proptest recomputes its expected midpoint and length with the *same*
  float expressions as the implementation, so its `1e-5` tolerances never bind.
- The old "Scene unchanged across a tick" assertions have been rewritten now that
  `Scene::step` really integrates; see the note above. They were vacuous before and
  would be simply false today.
- The camera and directional-light spawns are untested — only `MirroredBody`
  cardinality is checked.

## `eclipse_tests.rs` tests what it can, and says what it cannot

The eclipse **geometry** is pure Rust and is tested exhaustively, including against three answers known
independently of this code: Io's shadow on Jupiter is total and 2.6% of the disc wide, Jupiter's shadow
on Io is total, and Mercury transiting the Sun dims it by 0.003% — a figure from the transit literature.

The **shader** cannot be executed: no GPU in a test run and no WGSL interpreter. So
`the_shader_mirrors_the_reference_thresholds` reads `assets/shaders/eclipse.wgsl` as *text* and asserts
its four case boundaries and its two shared constants are the ones Rust uses. That catches a threshold
edited on one side only, which is the divergence that would actually happen, and it catches nothing
else. Verifying pixels is a human job, and [`assets/README.md`](../../assets/README.md) says where to
look.

## Adding a test here

The headless-app recipe (`MinimalPlugins`, `init_resource::<Assets<Mesh>>`,
`init_resource::<Assets<StandardMaterial>>`, then `ScenePlugin`) is currently
copy-pasted in three places in `plugin_tests.rs` and once in
`tst/headless_app.rs`. Prefer factoring it out over adding a fifth copy.
