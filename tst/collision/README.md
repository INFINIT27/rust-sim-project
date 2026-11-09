# `tst/collision/` — tests for body physics

Mirrors `src/collision/`, one file per source file. Every file here is a
`#[path]`-included unit-test module compiled *inside* its subject, not a
standalone integration test, so it uses `crate::…` paths and can reach the
module's private items (`apply_contact`, `pair_can_interact`, `separate`).

## What lives here

| File | Subject | Shape of the tests |
| --- | --- | --- |
| `trimesh_tests.rs` | `trimesh.rs` | Winding, closedness, and inside/outside — on meshes built **by hand**, so a tessellation bug cannot make them pass. 3 properties, 16 unit tests. |
| `tessellate_tests.rs` | `tessellate.rs` | Every `Body` variant's hull is valid, closed, and outward-wound; the numbers the docs quote are pinned. 4 properties, 11 unit tests. |
| `contact_tests.rs` | `contact.rs` | Whether a pair overlaps, and what normal and depth come back. 4 properties, 13 unit tests. |
| `resolve_tests.rs` | `resolve.rs` | Conservation through a bounce, the three prop kinds, separation, substep counting, and the end-to-end scene check. 3 properties, 16 unit tests. |
| `absorb_tests.rs` | `absorb.rs` | 19 unit tests: what a merge conserves (mass exactly, momentum to tolerance, energy **not**), the bookkeeping around removal, and the cases where "who absorbs whom" is a decision. |
| `proptest-regressions/` | — | Counterexample seeds proptest has already paid for. Committed. |

## Conventions specific to this directory

- **Exact assertions go on boxes; bracketed ones go on spheres.** An axis-aligned
  box hull is the box, so its overlap depth has a closed-form answer and
  `box_overlap_depth_is_exact` asserts it to `1e-4`. A sphere hull is an
  inscribed icosphere, so sphere tests assert a direction and a bracket. Writing
  an exact depth assertion against a sphere would be asserting the tessellation,
  not the algorithm.
- **Conservation is asserted through `physics_utils`**, never by hand:
  `momentum_is_conserved` and `kinetic_energy_is_conserved` decide whether a
  bounce was legal. The checks and the simulation must agree with each other —
  the same discipline `tst/core/integrator_tests.rs` follows.
- **Facet artifacts are asserted, not hidden.** `contact_tests.rs` documents and
  tests the vertex-on-vertex graze at 1.98 radii, and
  `the_sphere_hull_undershoots_by_the_ratio_the_docs_quote` fails if the inradius
  ratio in the `SPHERE_SUBDIVISIONS` docs stops matching the mesh. A doc claim
  about geometry is a test here.

## The one test to run if you change anything

`resolve_tests::the_shipped_scene_bounces_instead_of_passing_through` is the
end-to-end check on the whole layer. It runs `Scene::two_body_gravity` through
`Scene::step` at the *coarsest* timestep the app can produce (`max_dt` × the
shipped time scale) for 1.4× the free-fall time, and asserts that the spheres

1. actually meet,
2. never close past 85% of their contact separation (the hull's documented
   undershoot, and nothing like the free pass they had before this layer),
3. separate again afterwards, and
4. **do not swap sides** — which is precisely the phasing-through the layer
   exists to prevent.

If a change to tessellation, contact, or response breaks the feature in a way the
unit tests tolerate, this is the test that notices.

## Adding a test file here

Follow `tst/README.md`: mirror the `src/` path, end the subject with a `#[path]`
stub, pin `REGRESSION_SEEDS` for every `proptest!` block, and keep the file
runnable with no window and no GPU. For a new `Body` variant, the minimum is a
`tessellate_tests.rs` case asserting its hull is valid, closed, and contains a
known interior point.
