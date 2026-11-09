# `src/core/` — GUI-free world model

The pure-Rust representation of the scene: what exists, where it is, and how
big it is. No renderer, no ECS, no window. This is the layer a physics engine
would drive.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer and states the Bevy-free rule. |
| `constants.rs` | Every constant the layer reads: `ROTATION_UNIT_TOLERANCE`, `GRAVITY_SOFTENING_UNITS`, and the starter scene's bounds, floor size, and colours. |
| `integrator.rs` | The gravitational half of the `Scene::step` seam: mutual gravitation, composed entirely from `physics_utils` functions, and the crate's only units↔SI conversion site. `gravitational_acceleration`, `integrate_gravity`. The contact half lives in `src/collision/`. |
| `transfer.rs` | Planning a craft's path between any two bodies under the scene's own gravity: `plan_departure`, `plan_correction`, `TransferPlan`, and `Voyage`, which tracks a live flight's correction schedule and finds the substep it touches down. Predicts by stepping a clone of the scene, not with conics. Chooses the departure speed and direction that arrive first, under the craft's cap, with no time limit; `PlanProgress` reports and cancels the search. |
| `scene.rs` | `moon_primary` (which planet a body orbits as a moon, by Hill sphere), `Scene` container, `Prop` (a body plus its velocity and mass), and the value types `Pose`, `Color`, `Aabb`, `BodyId`, `Body` (`Sphere`/`Cuboid`/`Line`/`Plane`), plus the `step(dt)` physics seam, `step_observed`, which calls an observer after each substep, and `step_steered`, which also calls a controller before each. |

## Units: 1 world unit = 10⁹ metres

Every length in the scene — positions, `Aabb` bounds, radii, half-extents,
half-sizes, line endpoints — is in **world units**, and every velocity is in
**units per second**. `METRES_PER_UNIT` (in the crate-level `src/constants.rs`, since
more than one layer reads it) is the single place that
says what a unit means, and `integrator.rs` is the only place that converts:
separations up to metres to evaluate gravitation, acceleration back down before it
touches a velocity.

**Mass stays in kilograms and time stays in seconds** — only length is scaled. A
`dt` of `1.0` is one second of simulated time regardless of the unit.

Why not store metres: at `3e10 m` the spacing between representable `f32` values is
already ~2 km, and the render pipeline would be pushing `1e10`-magnitude transforms
through a depth buffer. Keeping stored numbers in the 1–100 range costs one multiply
per pair.

For reference at this scale: the `±50`-unit bounds span about 0.67 AU, an Earth
radius is 0.0064 units, and the Earth–Sun distance is 150 units.

Nothing *displayed* is in units: the debug overlay converts everything to SI before
printing it, and `src/ui/README.md` has the reading guide (including the reminder
that the `e` in `6.684e1` is a decimal exponent, not Euler's number). So a figure in
the code is in units and a figure on screen is in metres — do not compare them
directly.

**Velocity is `velocity_units_s`, not `velocity_m_s`.** That rename was forced by the
unit change: `position += velocity · dt` is only dimensionally sound when both use
the same length unit. `Prop::speed()` returns units/s; `Prop::speed_m_s()` gives the
SI reading, which is the one to check when you want to know whether a simulated speed
is physically sensible — one unit per second is `1e9 m/s`, over three times *c*.

## `Body` is what a thing looks like; `Prop` is what it does

`Body` carries geometry and colour only. `Prop` pairs a `Body` with the
simulation attributes:

| Field | Meaning |
| --- | --- |
| `body` | Shape and colour. The only part the render bridge sees. |
| `velocity_units_s` | One vector: **direction is the heading, magnitude is the speed** in world units per second. `Vec3::ZERO` means at rest. |
| `mass_kg` | Mass in kilograms. Finite, non-negative. |
| `immovable` | When set, never integrated and never moved by a force. |

`Scene.props` is a `Vec<Prop>`; `Scene::add`, `get`, and `iter` all deal in
props. `transform_from` still takes a `&Body`, so the render layer stays unaware
that dynamics exist.

### Mass, masslessness, and immovability are three different things

This is the distinction the type exists to preserve, so do not collapse it:

| `immovable` | `mass_kg` | Meaning |
| --- | --- | --- |
| `false` | `> 0.0` | Ordinary dynamic prop. Forces accelerate it. |
| `false` | `0.0` | **Massless but moving** — travels at its velocity, but `F = m·a` cannot accelerate it. Reserved for photon-like props. |
| `true` | any | **Immovable.** Never integrated, infinite effective mass in a collision. |

Most physics engines overload `mass == 0` to mean infinite mass. This one does
**not** — immovability has its own field precisely so that zero mass stays
available to mean genuinely massless. For an immovable prop, `mass_kg` carries no
simulation meaning at all, which is why `Prop::is_massless()` is `false` for one
even though `Prop::immovable()` stores `0.0` there.

Two contracts worth knowing before you write a `Prop` literal:

- **An immovable prop must have `Vec3::ZERO` velocity.** `is_valid` rejects the
  combination rather than silently ignoring the velocity, because an author who
  set both meant something and deserves a loud failure rather than a wall that
  mysteriously never moves.
- **`heading()` returns `Option<Vec3>`.** A stationary prop has no direction, and
  normalising `Vec3::ZERO` is undefined. `speed()` is the magnitude.

Units are `f32` here, matching glam and the render side. `physics_utils` works in
`f64`, so conversion happens at that call boundary — not in this layer.

## The startup scene is a bare container

`Scene::demo()` returns **one immovable ground plane and nothing else** — a
`Body::Plane` at the origin, identity rotation, `half_size` 5000 units (a 1e13 m
square), wrapped in `Prop::immovable`. It is immovable rather than zero-mass so a future integrator
skips it without a special case. It is the surface simulated bodies
rest on and fall toward, and it is deliberately free of props so that anything else
visible in a running app is something a simulation put there.

The original spec (Requirements 3.2 and 12.1) called for one of each of the four
variants; the cube, sphere, and line were removed on purpose. See amendment A1 in
the original requirements spec. `Body` still has all four variants and
the render bridge still draws all four — only the startup content changed.

That footprint is **100× the XZ extent of `bounds`** (±50), deliberately: the floor is meant
to read as *unbounded*, and it has to reach past the outermost thing an experiment puts on it —
Neptune, at 4495 units in `experiment_3`. A body ejected from the world therefore no longer
leaves the floor in the same instant and falls into empty space. Note an experiment whose scene
is larger than `bounds` widens its own `Scene::bounds`, as the solar system does; the floor is
shared, the bounds are not.

It is **not** a mathematical half-space — a true infinite plane would need the collision layer
to special-case a half-space test, since an unbounded surface cannot be a closed triangle
hull. What the size buys is the behaviour of an infinite floor at none of that cost; a prop
launched past 5000 units would still find the edge.

Cost is not the concern the size suggests: the plane's collision hull is a slab spanning
`y ∈ [−1, 0]`, so a prop above the floor is rejected by the broad phase on the **vertical**
axis alone, however wide the footprint gets.

The render layer's tracing grid measures itself against this floor — see
`src/render/grid.rs` — and covers **all** of it, at two spacings: coarse lines every 100 units
to the edge, fine lines every 5 out to 60. A single 5-unit rule over 10,000 units would be four
thousand entities whose distant half is finer than the pixels available to draw it.

## `Scene::step` now integrates *and* collides

The seam is live, and it has two halves per substep:

```text
step(step: TimeStep):
  n = substep_count(props, step)        collision/resolve.rs
  repeat n times:
      integrate_gravity(props, step.split(n))   core/integrator.rs
      resolve_contacts(props)                   collision/resolve.rs
```

**`step_observed` is the same step with an observer.** It calls a closure with the scene
after every substep, and `step` is `step_observed` with an empty closure. Trails use it
(`render/plugin.rs`) because one solar-system frame is `11%` of Io's orbit, so positions
read once per frame cannot draw Io's path. The observer is a closure, so `Scene` still
knows nothing about rendering.

**`step_steered` adds a controller before each substep.** The substeps are exactly
`step_observed`'s, so the rest of the scene moves as it would without one. A powered rocket
uses it: its `spacecraft::Flight` keeps the craft's true state in `f64` (the scene's `f32` cannot carry the
centimetre moves of a landing) and writes it into the scene before gravity runs. Both callbacks
are handed one shared `state`, so a controller that both steers and records is not captured
mutably by two closures at once.

**`step` takes a `TimeStep`, not an `f32`.** The type is validated on construction —
finite, non-negative — so Requirement 5.5's precondition is a property of the argument
rather than a `debug_assert!` inside the method, and it holds in release too. The
clock that produces it, and the reason there is exactly one, are in
`src/time/README.md` (amendment A3).

`integrate_gravity` gives every prop a mutual gravitational pull and advances each
movable prop's velocity and position. It composes four `physics_utils` functions
and adds no formula of its own: `gravitational_constant_calculator`,
`acceleration_from_net_force`, `velocity_after`, `displacement`.

`resolve_contacts` then makes props **solid**: any two whose triangle hulls
overlap bounce perfectly elastically and are pushed apart. A `Body`'s radius or
half-extents are no longer only a rendering detail — they are where the body
stops. The layer that owns all of that is `src/collision/`, and its `README.md` is
the place to read before touching it; what matters from here is:

- **A zero-length step returns before either half runs.** That is Postcondition
  5.2, not an optimisation: integration is genuinely a no-op at zero, but contact
  resolution is a function of *position* and would separate an already-overlapping
  pair however small the step. A paused clock hands out `TimeStep::ZERO` every
  frame, so this is also what makes pausing free.
- **Substeps exist so contact cannot be missed.** Contact is tested at substep
  boundaries, so `substep_count` splits `dt` until nothing travels further than a
  quarter unit per substep. For the shipped scene the answer is always `1` — the
  pair closes at `0.029` units per frame — so the loop is free today and load-bearing
  for faster scenes.
- **The startup floor is part of the physics now.** `Prop::immovable(Body::Plane)`
  is an infinite-mass surface that reflects whatever lands on it, and its hull is a
  one-unit-thick slab hanging *below* the visible quad so bodies rest exactly where
  the floor is drawn.

Three independent questions decide what happens to a prop, matching the attribute
table above:

| Question | Condition |
| --- | --- |
| Does it move? | `!immovable` |
| Can a force accelerate it? | `responds_to_forces()` — movable **and** `mass_kg > 0.0` |
| Does it pull on others? | Always, in proportion to `mass_kg` — even when immovable |

That last row is why `Prop::immovable_with_mass` exists: immovability governs
*motion*, mass governs the *field*, so an immovable prop with a mass is a fixed
attractor. `Prop::immovable` sets `mass_kg` to `0.0`, which is what makes the ground
plane gravitationally inert.

`Scene::two_body_gravity()` builds on it, adding two Earth-mass spheres 30 units
apart **released from rest**. They are given no tangential velocity, so the motion is
purely radial: no orbit, no angular momentum. `Scene::two_body_free_fall_seconds()`
derives how long they take to meet — about 6.5 years of simulated time — from the
scene's own constants.

They now **bounce** rather than passing through: equal masses meeting head-on
exchange velocities, so they leave as fast as they arrived, climb back out against
their mutual pull, and fall again, indefinitely. Successive bounces are not
identical — the integrator is first-order and the positional de-penetration is not
momentum conserving, so the amplitude drifts a little at each encounter.

Two things to preserve:

- **The velocity update comes before the position update.** That ordering is what
  makes the step symplectic. The explicit ordering (move at the old velocity,
  including `½·a·dt²`) was tried first and, measured on a circular test orbit, grew
  its radius by 5.2% per revolution; the symplectic ordering held it to under 0.05%.
  Both orderings pass every conservation and validity test — the difference only
  shows up in long-horizon trajectory shape — so do not "simplify" it back. (The
  shipped scene is now a radial infall rather than an orbit, so no current test
  reproduces that measurement; the monotone-approach assertion in
  `the_shipped_scene_falls_together_along_one_axis` is the nearest tripwire.)
- **`GRAVITY_SOFTENING_UNITS` is load-bearing, not a fudge.** Point masses have a
  `1/r²` singularity; without softening a near-miss produces an infinite force, a
  non-finite velocity, and a violation of `Scene::step`'s Postcondition 5.1 for
  reasons unrelated to the caller. The pairwise force is evaluated at
  `√(r² + ε²)`, which is standard Plummer softening and needs no new formula — it
  is just a different argument to the existing one.

## Constants

Every tunable value this layer reads lives in `constants.rs` — none are declared in
an implementation file, per
[`.claude/steering/constants.md`](../../.claude/steering/constants.md). Each one
documents why it holds that value, who reads it, and what changes if it moves, so
this README does not restate them; open `constants.rs` to see the layer's whole
tuning surface at once.

The scene-scale values this layer *builds from* — `EARTH_MASS_KG`, the `TWO_BODY_*`
numbers — are **not** here: `render/` reads them too, so they live in the
crate-level `src/constants.rs`.

## Rules and invariants

- **`bevy::math` only.** Modules here may reference `bevy::math` — which
  re-exports glam's `Vec3` and `Quat` — and nothing else from Bevy. No
  `bevy::ecs`, no `bevy_egui`, no `wgpu` (Requirements 4.2 and 15.3). This is
  why `impl Resource for Scene` lives in `render/plugin.rs` and not here.
  **Nothing enforces this but review** — there is no lint and no test for it.
- **Validation is advisory, not enforced.** `is_valid()` is a pure predicate.
  The only enforcement is `debug_assert!` in `Scene::add` and `Scene::step`, so
  a release build accepts an invalid `Body` silently (Requirement 14.4). No code
  path downstream may assume validity in release.
- **Quaternions arrive normalized.** `Pose::is_valid` *checks*
  `|len - 1.0| <= 1e-4`; nothing here ever calls `normalize()`.
- **`ids` and `bodies` are parallel vectors.** `ids[i]` is the id of
  `bodies[i]`, and `next_id` exceeds every id ever issued. Note that `bodies` is
  `pub` while `ids` is private, so external mutation of `bodies` can desync them
  and make `Scene::get` panic — treat `bodies` as read-only from outside.
- **Iteration order is insertion order** for both `iter()` and `bodies`
  (Requirement 4.7).
- **Strictness asymmetry**, worth memorising: sphere `radius`, every
  `half_extents` component, and `half_size` must be strictly `> 0.0`;
  `Line::width` may be exactly `0.0`; `Aabb` allows `min == max`.

## Where the tests are

`tst/core/scene_tests.rs` — 46 unit tests plus 5 proptest properties, all
runnable with no window and no GPU.

## Adding a file here

- It must compile with no Bevy import other than `bevy::math`. If it needs
  `bevy::ecs`, it belongs in `render/`.
- Give every new type an `is_valid()` predicate following the existing pattern,
  and guard mutating entry points with a descriptive `debug_assert!` rather than
  returning a `Result` — the crate's contract is loud in debug, benign in
  release.
- Document units, handedness, and the up-axis for anything geometric. The
  existing types assume glam/Bevy right-handed Y-up but do not say so, which is
  a known gap; do not extend it.
