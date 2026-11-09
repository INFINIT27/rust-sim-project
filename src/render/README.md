# `src/render/` — model → Bevy ECS boundary

Where the GUI-free model crosses into Bevy. Everything Bevy-specific about
*drawing the scene* lives here, so `core/` never learns that a renderer exists.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer. |
| — | **Not here any more:** `SceneConfig`, and the clamp-and-scale that produced a `dt`. Both moved to `src/time/` (amendment A3); this layer adopts the clocks as ECS resources and calls them in order. |
| `constants.rs` | Every constant the layer reads: the light placement, the `Body::Line` mesh proxy's thickness, and the `GRID_*` values. The frame-step cap and clock scales moved to `time/constants.rs` (A3); the camera pose to `experiments/constants.rs` (A4). |
| `mission.rs` | `Mission`, `MissionPhase`, `pick_route_system`, `mission_system`, `draw_mission_system`, `MissionPlugin`. Picks an origin and destination at the crosshair with `R`. Runs **after** `step_scene_system`, so plans are made from the state the next frame steps from. Holds the `Fleet` `G` cycles. Plans the flight on a background task with the clock paused (`spacecraft::plan_flight`, by the selected craft's propulsion), adds the craft to the scene, flies a powered one before every substep (`Mission::steer`), runs correction burns (`spacecraft::plan_correction_for`), lands it, and draws its flown and planned path. The landed flight's report feeds the arrival pop-up. |
| `placement.rs` | `PlacementPlugin`, `place_object_system`, and the pure `placement_position`. Turns a `B`-picker request into a prop in the `Scene` — the only layer allowed to write one. |
| `eclipse.rs` | `BodyMaterial`, `EclipseExtension`, and the pure `star_visible_fraction`: analytic eclipse shadows, because a shadow map cannot resolve these bodies. |
| `trail.rs` | Motion trails: `TrailPath` (pure path + fade), `Trails`, `TrailConfig`, the record/toggle/draw systems, `TrailPlugin`. Drawn with gizmos; toggled by `T`. |
| `grid.rs` | The floor's tracing grid: `floor_grid_segments` (pure geometry), `spawn_floor_grid`, `FloorGridPlugin`, `FloorGridLine`, `GridLineKind`, `GridSegment`. **Render-only entities** — nothing here is a `Scene` prop. |
| `bridge.rs` | `transform_from(&Body) -> Transform`, the pure per-variant sizing math, plus `sync_scene_to_ecs_system`, which rewrites each mirrored entity's `Transform` every frame. |
| `plugin.rs` | `ScenePlugin`: adopts `Scene` as a `Resource`, spawns the world at `Startup` (`spawn_scene_entities`), reconciles the mirror every frame (`sync_mirror_entities_system`), and registers the per-frame `step` → `mirror` → `sync` chain. Also `MirroredBody`, `clamp_dt` (now a delegate to `time::clamp_real_step`), `advance_clocks_system`, `step_scene_system`, and the `Resource` impls for `Scene` and both clocks. |

## Constants

Every tunable value this layer reads lives in `constants.rs` — none are declared in
an implementation file, per
[`.claude/steering/constants.md`](../../.claude/steering/constants.md). Each one
documents why it holds that value, who reads it, and what changes if it moves, so
this README does not restate them; open `constants.rs` to see the layer's whole
tuning surface at once.

The shipped scene's clock scale is **not** here: `TWO_BODY_TIME_SCALE` describes the
scene `core/` builds, and both layers read it, so it lives in the crate-level
`src/constants.rs`.

### Shadows are computed, not sampled

`eclipse.rs` and `assets/shaders/eclipse.wgsl` draw the shadow one body casts on another — Io's black
dot crossing Jupiter, and the total eclipse when Io passes behind it.

**Shadow maps cannot do this**, and not by a little: at Jupiter's distance from the Sun, one texel of a
1024² cube face is 17× Jupiter's radius and 656× Io's; resolving Io's shadow would take ~671,000 texels
per face. So `shadows_enabled` is off and always will be. Instead, every body is a sphere and there is
one star, so "how much of the star can this point see" has a closed form — the overlap area of two
discs on the sky — which is exact at any scale.

The arithmetic exists **twice**, in Rust (`star_visible_fraction`, tested) and in WGSL (running). That
is managed rather than avoided: the Rust version is the reference, a test reads the shader as text and
pins its four case boundaries and shared constants, and **a change is not verified until someone has
looked at it**. See [`assets/README.md`](../../assets/README.md).

Two details:

- **Each body gets its own uniform**, so it can be excluded from its own occluder list. A sphere's
  surface is exactly at its own radius, and self-occlusion would make the terminator depend on
  floating-point noise rather than on the lighting.
- **A surface facing away from the star is never shadowed.** That exclusion has a cost: nothing
  stops an occluder's umbra at the receiver, so Io's shadow used to pass through Jupiter and darken
  its ambient-lit night side as well. `star_visible_fraction_all` and the shader's `star_visible`
  therefore take the surface normal and return full visibility when it points away from the star.
  There is no direct light there to shadow.
- **A miss is decided before any trigonometry.** This is the costliest shader in `experiment_3`, and
  from almost every fragment almost every occluder misses, so both sides first test
  `cos θ ≤ cos α cos β − sin α sin β` from divisions and square roots, and only an overlapping
  occluder pays for `asin` and `acos`. The test rejects only below a `1e-6` margin
  (`MISS_COSINE_MARGIN`). Without it, at tiny angles `cos(α + β)` rounds to exactly `1`, and an
  occluder squarely on the star would count as a miss. A test checks the shortcut against the
  original formula, bit for bit, over two million configurations.
- **`init_asset` and `MaterialPlugin` are both guarded** on the plugins that provide what they need —
  `AssetPlugin` and the render sub-app — because `MinimalPlugins` has neither. That is what lets a
  headless test register `Assets<BodyMaterial>` by hand and still exercise the whole spawn path.

### Space is dark, and a star lights its own scene

The camera clears to `BACKGROUND_COLOR`, a near-black — Bevy's default mid grey has almost no
contrast against the `4.6`-px Sun `experiment_3` opens with. Not pure black, so a window that failed
to render is still distinguishable from one that rendered empty space.

A prop with a positive `Prop::luminosity_solar` becomes a **point light at its own position**, and
the fixed directional light is spawned only when no prop in the scene has one — both at once would
wash out the terminator that lighting exists to produce. Nothing declares which; it follows from the
scene.

Two details worth knowing:

- **The light is a *child* of the body, not a component on it**, and that distinction was a real bug.
  Putting the `PointLight` on the mesh entity looks free — `PointLightBundle` adds only three
  components beyond `PbrBundle`, and transform propagation comes with it — and the scene goes dark the
  moment the camera looks away from the star. `extract_lights` skips any light whose entity has
  `ViewVisibility == false`, and `check_visibility` frustum-culls exactly those entities that **have
  an `Aabb`**: a mesh entity gets one from `calculate_bounds` and a light-only entity never does. A
  child has no bounds to cull against, and `InheritedVisibility` is unaffected by culling. The
  consequence is that `sync_mirror_entities_system` despawns **recursively**, or an absorbed star
  leaves its light behind.
- **The intensity is derived.** The Sun delivers `120,000 lux` at Earth's `149.6`-unit orbit and
  Bevy's illuminance is `intensity / (4π d²)`, so `STAR_LUMENS_PER_SOLAR_LUMINOSITY` is `3.375e10`.
  The falloff is right at every orbit, which means Mercury clips to white and Neptune is nearly
  black — both of which are true.

A star's material is also emissive (`STAR_EMISSIVE_SCALE`), because its light source is inside it and
reflected light alone would render its surface black.

### Sizes are never adjusted here

`transform_from` draws a `Body::Sphere` at its stored radius and nothing else. There is no
exaggeration and no minimum apparent size — amendment A17 removed the floor that briefly existed,
because lifting sub-pixel bodies to a visible minimum made every body on screen the same size,
including the Sun. A body smaller than a pixel is not drawn, the orbits are read from their trails,
and the camera is the instrument for choosing scale.

`CAMERA_FAR_UNITS` **is** here, though the camera's *pose* is not: where the camera starts is a
property of the experiment and arrives in the spec, while how far it can see is a property of
the renderer. It is 20,000 units rather than Bevy's default 1000, because the default would
clip away Saturn, Uranus, Neptune, and most of the floor in `experiment_3`. Reversed-Z depth
means the cost of a distant far plane is close to nothing — precision is set by the near plane.

`CAMERA_NEAR_UNITS` is `1e-4` units (100 km) rather than Bevy's default `0.1` (100,000 km).
Bodies are drawn at true size, so the only way to see a moon is to get close to it, and the
default near plane kept the camera lock 55 Io radii from Io. Bevy's infinite reversed-Z projection
stores `near / distance` in a 32-bit float, so the smaller near plane costs no measurable precision
anywhere in the scene. `plugin_tests::the_camera_lock_can_bring_io_close_enough_to_fill_the_view`
pins it.

## The mirror is reconciled every frame, not established once

`spawn_scene_entities` runs on `Startup`, which was sufficient for exactly as long as the scene
was fixed at build time. Two changes broke that in opposite directions: the `B` key **adds** a
prop, and a black hole **removes** one. So `sync_mirror_entities_system` now runs every frame,
spawning an entity for any prop without one and despawning any entity whose prop has gone.

Requirement 3.5's invariant — `count_entities_with::<MirroredBody>() == scene.props.len()` — is
therefore *maintained* rather than established. Without it, a placed object would be simulated,
pulling on everything, and drawn nowhere; an absorbed one would leave an entity frozen at a dead
prop's last position, because `sync_scene_to_ecs_system` deliberately leaves an unknown id's
transform alone (Requirement 9.2).

Order matters and the plugin fixes it: `step` → `sync_mirror_entities_system` →
`sync_scene_to_ecs_system`. The other way round, a new entity would spend its first frame at the
origin — a visible flash at the centre of the world, for an object placed forty units in front of
the viewer. Both properties have tests.

## The grid is scenery, not physics

`grid.rs` spawns the floor's tracing grid as **render-only entities**, marked
`FloorGridLine` and deliberately *not* `MirroredBody`. Three reasons, in order of how much
they matter:

1. `Scene.props` stays the list of things being *simulated*, so the overlay's prop count
   and per-prop readout keep meaning what they say — 42 grid lines would drown them.
2. Gravitation and contact are `O(n²)` over props; scenery in that loop costs real time to
   compute nothing.
3. The mirror invariant `count_entities_with::<MirroredBody>() == scene.props.len()` keeps
   holding, which the headless integration test asserts.

The grid **measures the floor** rather than assuming it: `spawn_floor_grid` reads the plane
prop's own `half_size` out of the `Scene` and grids all of it, so widening the floor widens the
grid with no second edit.

It does that at **two spacings**, because the shipped floor is 5000 units to the side and a
single 5-unit rule over it would be four thousand entities whose distant half is finer than the
pixels available to draw it:

| Tier | Spacing | Extent | Job |
| --- | --- | --- | --- |
| Axes | — | the whole floor | Where the origin is, from anywhere on the plane |
| Coarse | `GRID_COARSE_SPACING_UNITS` (100) | the whole floor | The plane is never bare |
| Fine | `GRID_SPACING_UNITS` (5) | `GRID_CORE_EXTENT_UNITS` (60) | Measuring the motion that actually happens |

~250 entities, with the resolution where the bodies are. `GridLayout` carries the four numbers
and `GridLayout::for_floor` derives them from a floor; a floor *smaller* than the core extent
gets fine lines that match it exactly, since a grid overhanging its own surface looks broken.

Two invariants the tests pin, because both failures are subtle on screen: the coarse spacing is
a whole multiple of the fine one, so the tiers line up instead of interleaving into a beat
pattern; and **no offset is ever drawn twice**, since two coincident lines read as one thicker
line and make the spacing look irregular. Lines are lifted `GRID_LIFT_UNITS` above the surface
because two coplanar surfaces z-fight, which flickers as the camera moves — that lift is
required, not decorative.

## Trails: ten units of path by default, fading out

`trail.rs` draws the last `TRAIL_LENGTH_UNITS` (10 — two fine grid cells) of each moving body's
path, brightest at the body and invisible at the far end, in the body's own colour, at
`TRAIL_LINE_WIDTH_PX` (6 px, three times Bevy's gizmo default). `T` toggles it; on by default.

Seven decisions worth knowing:

- **The head and the anchor are different points.** `points[0]` is the live head, always exactly
  where the body is; `points[1..]` are committed samples, and the spacing test measures from
  `points[1]` — **never from the head**, which is overwritten when a move is too small to
  commit. Comparing against the head destroys the anchor the comparison needs, and a body moving
  less than `TRAIL_MIN_SAMPLE_SPACING_UNITS` per frame then records *nothing at all*, however far
  it travels. Both shipped scenes are in exactly that range. That was a real bug; three tests and
  a property now cover the region it lived in.
- **An experiment may override the length, and the spacing follows.** Ten units is two grid
  cells — right for a scene 30 units across, and an invisible stub on an orbit 940 units around.
  An `ExperimentSpec` may set `trail_length_units`, which `apply_experiment_trail_length` reads
  once at `Startup`; `experiment_3` asks for 400 units. The **sample spacing is derived from the
  length** (`length / TRAIL_TARGET_SEGMENTS`, floored at `TRAIL_MIN_SAMPLE_SPACING_UNITS`), so the
  cost per body is flat whatever is asked for. Keeping the 0.05-unit default for a 400-unit trail
  would want 8000 samples, hit the 512 cap, and draw a trail a twentieth of its stated length —
  which reads as the length constant not working rather than the spacing.
- **Spacing is distance travelled, and turns earn extra samples.** The spacing test adds up how
  far the body has gone along its path since `points[1]`, not the straight line back to it. A
  straight-line test skips any turn that goes out and back within one spacing. Europa's path has
  near-cusps like that, and its trail missed them by `0.04` units. A sample is also committed once
  the path has turned `TRAIL_MAX_TURN_RADIANS` (`10°`), provided it has travelled
  `TRAIL_MIN_SPACING_FRACTION` (`1/16`) of a spacing. That way samples gather at bends, and a body
  jittering in place still cannot fill its buffer.
- **Trails are sampled at the physics substeps, and moons at their own spacing.** One solar-system
  frame is `11%` of Io's orbit, so `step_scene_system` records after every substep through
  `Scene::step_steered` (`step_observed` plus the mission's hook that flies a powered spacecraft
  before each substep), at `0.27 ms` a frame in release. `core::scene::moon_primary` finds the moons (inside a
  heavier body's Hill sphere, at most `MOON_MAX_MASS_RATIO` of its mass), and each is sampled
  `TRAIL_MOON_SAMPLES_PER_ORBIT` (`64`) times per orbit. At the scene's `1.56`-unit spacing a moon's
  loop fell between two samples. Points stay in world coordinates, so Io's trail shows its real
  backward loops, and Earth's Moon's trail is almost straight because its real path is.
- **Length is distance, not time.** Under a variable timestep a fixed sample count would give a
  trail whose length varied with the frame rate and the clock scale. The tail is trimmed to
  *exactly* 10 units by sliding the last point along its own segment, so the trail does not pulse
  in and out by a sample's worth as new points arrive.
- **The fade is what makes it a trail.** Alpha is interpolated by *distance along the path*, not
  by sample index — samples are unevenly spaced, since a body accelerating leaves longer gaps,
  and an index-based fade would make the fast part of a path fade faster than the slow part.
- **Gizmos, not entities.** The geometry changes every frame, so immediate-mode drawing is the
  right tool; an entity-per-segment scheme would spawn and despawn hundreds per second. The cost
  is that `draw_trails_system` cannot be tested headlessly, which is why the length and fade live
  in `TrailPath`, which is pure and carries most of the tests. Trails have their **own gizmo config group**
  (`TrailGizmos`) so their width is a trail setting rather than a global for every gizmo the app
  might later draw, and `TrailPlugin` registers the draw system **only** when a
  `GizmoConfigStore` exists — so a headless app records trails without pretending it can paint
  them.

Recording continues while trails are hidden, so `T` reads as a view setting rather than something
destructive. Like the grid, trails are **outside the model**: `Trails` is a render-layer resource
derived from prop positions, never state on `Prop`.

The `T` binding lives here rather than in `input/controls.rs` because it controls a render
setting, and putting it there would mean `input` depending on `render` — inverting the direction
that already exists. `trail_tests` checks it collides with neither the camera's keys nor the
clock's.

## Rules and invariants

- **Bevy imports are welcome here.** That is the point of the split: `core/` is
  restricted, this layer is not (Requirement 15.3).
- **The sizing contract spans three files and nothing couples them.** Validation
  lives in `core/scene.rs`, the scale math in `bridge.rs`, and the unit-mesh
  assumption in `plugin.rs`. `transform_from` supplies the *entire* scale
  (sphere `splat(radius)`, cuboid `2 × half_extents`, plane
  `(2 × half_size, 1, 2 × half_size)`, line `(length, 1, 1)`), which is only
  correct because `spawn_scene_entities` spawns unit-sized meshes. **Change one
  side and the scene silently mis-renders — no test will fail.**
- **The sync system must never mutate `Scene`.** Enforced at compile time by
  `Res<Scene>` rather than `ResMut` (Requirement 9.3). Keep it that way — and note
  that this is now the *only* guarantee, because `Scene::step` legitimately mutates
  every movable prop each tick, so no runtime test can distinguish sync's writes
  from the integrator's.
- **Ordering: `step_scene_system` then `sync_scene_to_ecs_system`,** via an
  `.after()` edge in `ScenePlugin::build` (Requirement 2.1). No test asserts
  this edge, so do not remove it on the strength of a green suite.
- **`transform_from` is total.** It must never panic, including for a degenerate
  `Line` where `from == to` (Requirement 14.2). The totality argument depends on
  Bevy's `glam_assert` feature being *off*; enabling it would break this.
- **The plugin ships `Scene::two_body_gravity()`, not `Scene::demo()`.** The
  container plane plus two Earth-mass spheres 30 units apart, released from rest.
  `demo()` remains the bare container that `two_body_gravity()` builds on.
- **`SceneConfig::time_scale` is why the simulation is visible at all.** The
  clamped frame delta is multiplied by it before reaching `Scene::step`. The shipped
  value is `TWO_BODY_TIME_SCALE = 5e6`, because two Earth masses `3e10 m` apart take
  about 6.5 years to fall together — at real time you would watch a still image. That
  scale makes one approach take ~41 seconds, at ~a day of simulated time per frame.
  The forces, masses, and formulas are untouched; only the amount of simulated time a
  frame represents changes. `Default` is `1.0` (real time), and the plugin overrides
  it deliberately.
- **The needed scale depends entirely on the scene, in either direction.** This
  astronomical pair needs a scale far *above* one because gravitation at that range is
  slow. An earlier metre-scale pair needed `4e-7`, far *below* one, because their
  orbital period was microseconds. Do not assume the constant should be small.
- **Scene lengths are world units, not metres.** `1 unit = 1e9 m`; see
  `core/README.md`. This layer never converts — `transform_from` passes units straight
  into `Transform`, so the render scale *is* the world scale, and only
  `core/integrator.rs` talks to SI.
- **Clamp first, then scale.** `max_dt` caps the *real* frame delta so a window
  drag cannot produce a huge step; `time_scale` then converts that real interval
  into simulated time. Reversing the order would let a stalled frame through.
- **`clamp_dt` order is load-bearing.** The `!is_finite() || < 0.0` test comes
  first so `NaN` — which fails every ordering comparison — lands in the `0.0`
  branch instead of slipping through a `min`.
- **Colours are linear-space throughout.** `to_bevy_color` uses `linear_rgba`
  with no gamma conversion, because `scene::Color` is already linear and
  `StandardMaterial` wants linear.

## Where the tests are

- `tst/render/bridge_tests.rs` — 9 tests: proptest properties over
  `transform_from` per variant, plus three headless `App` tests for the sync
  system.
- `tst/render/plugin_tests.rs` — 22 tests: plugin registration, entity-count
  invariant, `clamp_dt` branches, a `step_scene_system` smoke test, the camera's far plane
  against the floor's diagonal, and every catalog experiment spawned through the plugin.
- `tst/render/grid_tests.rs` — the pure layout, the tiering, and the spawned entities.
- `tst/render/trail_tests.rs` — 43 tests: `TrailPath` recording and fade (pure), the `T`
  binding, the headless-recording path, the per-experiment length plumbing, which bodies count as
  moons, and a moon's trail checked against every substep position the integrator produced.

## Adding a file here

- If it converts model data into a Bevy type, it belongs in `bridge.rs` or a new
  sibling — keep such functions pure and total so they stay testable without an
  `App`.
- If it registers systems or resources, it belongs in `plugin.rs`.
- Any new mesh must be unit-sized, or you must change `transform_from` in the
  same commit and say so in both files' docs.
- New `Update` systems that write `Transform` need an explicit ordering relation
  to `sync_scene_to_ecs_system`. Today the camera system gets away without one
  only because the camera entity carries no `MirroredBody`.
