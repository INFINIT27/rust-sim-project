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
| `plugin_tests.rs` | 24 tests for `ScenePlugin`, `SceneConfig`, `MirroredBody`, and `clamp_dt`, including the lighting: a star lights its own scene, a starless one gets the directional light, and the black hole gets no directional light and no ambient fill. |
| `accretion_disc_tests.rs` | 18 tests: blackbody colours; the profile cold at the ISCO, hot just outside, cooler further out, and interpolating its own samples; the disc starting full; clumps orbiting at the Kerr rate, drifting in, plunging at the ISCO, swallowed and replaced; a paused clock moving nothing; every run building the same disc; the straight-line and along-a-path redshifts, blue on the side coming towards the camera and red from gravity face-on; an orbiting clump's streak trailing it along its orbit; the ergosphere's shape; the disc forming only round a spinning hole; the plugin headless. |
| `black_hole_view_tests.rs` | 19 tests (and an ignored timing of the splat): the material opaque with the stars and blended without; the shader writing its own depth; the uniform carrying Sgr A*'s horizon, photon orbit, ISCO, `r_g` scale, the disc's profile, the exposure, the plunge's `E` and `L`, and the view; the settings choosing the step, the step budget, and the stars; the ray-marcher switching off for the plain body; the sphere always holding the camera and inside the far plane; the hole's body found only in a scene with a disc; `V` switching shaded and pattern; the uniform rebuilding the twin's hole; the splat (nothing from an empty disc, a clump along its streak where the shader reads it, as much of the plane as the streaks cover, the simulated disc mostly covered with gaps); the gas texture wrapping and filtering; the shader's text mirroring its twin, its stars included; the outline off at the start, reaching the shader as a flag and changing nothing else, `O` flipping it except while the menu is open, and the shader's outline mirroring `horizon_measure` and its constants, with nothing thrown away before its `fwidth`. |
| `kerr_ray_tests.rs` | 31 tests of the `f32` ray-marcher twin: its metric pieces against the `f64` Kerr–Schild metric and their own finite differences; the starting momentum null and aimed at the camera; the null condition and `p_φ` holding to `10⁻⁴` in `f32` round the photon orbit; the same plane crossings as an `f64` coordinate-time tracer over the whole view; the far side arched over the hole and the underside below it; the shadow's edge-on edges and pole-on circle matching Bardeen's analytic shadow to `1%`; the face-on redshift equal to the orbit's time dilation, and the bent-path redshift matching the disc's own; the disc's light `g⁴` times its flux at `g T`; tone mapping; the shading table and blackbody; plunging gas unit, infalling, prograde, joining the orbit at the ISCO, and matching a clump dropped as the simulation drops them; the gas texture's coordinates; compositing through full and thin gas; the sky direction at escape already the final one; stars over the whole sky in equal shares, small, coloured, and steady; the sky behind the hole lensed into the picture; a few hundred steps per ray; unusable holes and cameras; the horizon's outline a `2 r_g` sphere without spin, squashed to `r₊` at the poles and `√(2 r₊)` round the equator with it, well inside the marched shadow, its measure changing evenly across the edge, and never met looking away. |
| `light_streams_tests.rs` | 17 tests (including one at spin `0.9`): each burst starts on the source sphere heading in, within the impact disc; the light comes from every octant; impact parameters fill the disc evenly (42% inside the critical 2.6 r_s); every run fires the same pattern; bursts fire at once and then on schedule; a paused clock moves nothing; rays keep light's speed in three dimensions; a ray at `2.5 r_s` is captured, one at `6 r_s` escapes bent towards the hole, one at `2.62 r_s` comes back the way it came; captured rays are removed and the count stays bounded; a streak is the ray's recent path, the same span of simulated time at any frame rate; the plugin runs headless. |
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
