# `tst/core/` — tests for `src/core/`

> **Purpose:** Tests for [`src/core/`](../../src/core/README.md): `Scene` and its value types,
> gravity integration, and transfer planning, including one opt-in Earth→Mars voyage.
> **Edit here to:** add or fix a world-model, integrator, or transfer test, or commit a new
> proptest seed.
> **Not here:** the code and the invariants under test, in `src/core/`.

Unit and property tests for the GUI-free world model. See `src/core/README.md`
for what is under test and the invariants these tests exist to pin.

## What lives here

| File | Job |
| --- | --- |
| `scene_tests.rs` | 65 unit tests + 9 proptest properties over `Scene`, `Prop`, `Body`, `Pose`, `Color`, `Aabb`, `Scene::remove`, and the `step(dt)` seam. |
| `integrator_tests.rs` | 13 unit tests + 7 proptest properties over mutual gravitation and the integration step. |
| `transfer_tests.rs` | Plans reach the target's centre at `15.4 km/s` v∞; Earth→Moon and Io→Europa are flown through the live integrator and land; moon folding conserves mass and momentum; the segment-entry test; and one opt-in (`--release -- --ignored`) Earth→Mars voyage. |
| `proptest-regressions/` | Committed proptest failure seeds. Tool-owned; no `README.md` required. |

## What is covered

- **Properties:** body construction preserves validation; `add` is additive and
  id-stable; `step` preserves validity and bounds; `step(0.0)` is a no-op;
  `step` preserves the exact set of resolvable `BodyId`s.
- **Validation boundaries:** one test per edge, each paired with a positive
  sanity case so a trivially-true `is_valid` cannot pass — zero/negative/NaN/inf
  radii, a zero `half_extents` component, non-finite line endpoints, negative
  width, out-of-range and NaN colour channels, non-unit and zero quaternions,
  `Aabb` with `min > max`.
- **`Scene::demo` content:** exactly one prop, and it is an immovable ground
  `Plane` at the origin with identity rotation; that plane's footprint fits inside `bounds`;
  bounds valid with strictly positive extent; every body valid. The exact count is
  asserted deliberately — the guarantee is *emptiness*, so a prop added back to the
  startup scene must fail a test rather than slip in. Requirements 3.2 and 12.1
  originally required one of each variant; see amendment A1 in
  the original requirements spec.
- **Debug-vs-release contract:** `should_panic` tests for `add(invalid)`,
  `step(-1.0)`, and `step(NaN)`, gated `#[cfg(debug_assertions)]`.

### The gravitation integrator

Properties: pairwise forces are equal and opposite (Newton's third law, asserted on
the *forces* by scaling each acceleration by its own mass); gravity always pulls
toward the other prop, never away; acceleration obeys the inverse square law;
a step conserves total momentum per axis, checked through
`conservation_laws::momentum_is_conserved` so the simulation and the conservation
checks have to agree; immovable props never move; every prop stays valid across a
step; and `dt == 0.0` leaves the scene byte-identical.

Unit tests: a lone prop feels nothing and travels straight; a massless prop is not
accelerated but still coasts; the ground plane pulls on nothing while
`immovable_with_mass` does attract; coincident props yield no acceleration rather
than a `NaN`; softening bounds the acceleration at tiny separations; two props
released from rest fall together with a stationary barycentre; the shipped scene
falls together monotonically **along one axis only**; and the simulation stays finite
and bounded straight through the encounter.

**`the_shipped_scene_falls_together_along_one_axis` is the test that pins the absence
of angular velocity.** It asserts `y` and `z` stay *exactly* at their starting values
for both spheres, position and velocity alike. Any leak into another axis would mean
a rotation nobody asked for. It also asserts the approach is monotone at every one of
2500 steps, which catches an integrator that overshoots.

The companion `the_simulation_survives_the_encounter` deliberately claims **nothing**
about the post-contact trajectory — there is no collision response and the timestep is
far too coarse through closest approach — only that the result is finite, valid, and
bounded, which is what softening buys.

**Two calibrations exist because of failures.** The inverse-square property is
restricted to separations of at least 50 units, because Plummer softening makes the
law legitimately `a ∝ 1/(r² + ε²)`; near `ε` the deviation is large enough that the
property failed on an earlier run. Asserting the softened formula instead would just
restate the implementation, so the softened regime has its own bounded-magnitude test.
And `testable_mass()` was raised to `1e20..1e26 kg` when one unit became `1e9 m` — at
the old `1e9 kg` a pair produces accelerations around `1e-31 units/s²`, which is a
normal `f32` but leaves no headroom.

### `Prop` attributes

**`Prop` attributes** — 13 unit tests and 4 properties. The unit tests pin each
constructor, that the velocity vector carries both speed and heading (against a
3-4-5 triple, so the length is exact), that a prop at rest has `heading() == None`,
and every `is_valid` rejection: non-finite velocity, negative/non-finite/infinite
mass, an immovable prop carrying a velocity, and an invalid inner body. Two tests
exist specifically to pin the distinction the data model was designed around:
`zero_mass_means_massless_not_static` and `an_immovable_prop_is_not_massless`.
Properties: every prop the strategies build is valid; `speed`/`heading` agree with
the velocity vector and the heading is unit length; and `responds_to_forces` /
`is_massless` are exactly their stated conjunctions, never a shortcut that
conflates zero mass with immovability.


## Known gaps, so you do not assume coverage you do not have

- Every strategy generates only *valid* values, so an `is_valid` that returned
  `true` unconditionally would be caught only by the hand-written negative unit
  tests, not by any property.
- The "`step` preserves X" properties are **vacuous while `step` is a no-op**.
  They are regression guards for when real physics lands, not live properties.
- `dt` is only ever sampled in `[0.0, 0.1]` (the `clamp_dt` range), so the seam
  is untested for the larger `dt` a non-Bevy caller could pass.
- Nothing tests the layer's Bevy-purity rule; it is review-enforced only.

## Adding a test here

Match the existing shape: a proptest property for anything that should hold for
all inputs, plus a named unit test per boundary with a positive counterpart. Keep
`#[cfg(debug_assertions)]` on anything asserting a `debug_assert!` panic.
