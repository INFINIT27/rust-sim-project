# `tst/experiments/` — tests for the experiment catalog

Mirrors `src/experiments/`, one file per source file. All six are `#[path]`-included
unit-test modules compiled *inside* their subject, so they use `crate::…` paths and can
reach private helpers.

## What lives here

| File | Subject | Shape of the tests |
| --- | --- | --- |
| `catalog_tests.rs` | `catalog.rs` | Catalog hygiene over `ExperimentId::ALL`, the three-body premise checked geometrically, and a 400-frame run of every experiment. 2 properties, 19 unit tests. |
| `solar_system_tests.rs` | `solar_system.rs` | The scene checked against published astronomy, true sizes asserted where two exaggerations used to be, one simulated Earth year through the real integrator, and the Earth-Moon-Sun geometry put through the shadow shader's own arithmetic. 35 unit tests. |
| `objects_tests.rs` | `objects.rs` | 20 unit tests: masses and radii against published figures, a black hole's radius recomputed as `2GM/c²`, and its capture radius as the tidal disruption radius. |
| `jovian_tests.rs` | `jovian.rs` | 27 unit tests: published moon data, the Laplace resonance derived from the distances, and two real integrations — one Io period, and ten. |
| `scale_check_tests.rs` | `scale_check.rs` | 13 unit tests: the size ratios against published figures, the layout, readability in pixels, and that the scene is provably static. |
| `selection_tests.rs` | `selection.rs` | Every accepted argument form and every error variant. 15 unit tests. |
| `random_tests.rs` | `random.rs` | Determinism, range, coarse spread, the zero seed. 2 properties, 9 unit tests. |
| `proptest-regressions/` | — | Counterexample seeds proptest has already paid for. Committed. |

## Conventions specific to this directory

- **Hygiene tests iterate `ExperimentId::ALL`.** Reachability, spec validity, camera
  framing, nothing starting in contact, and the run-without-exploding check all loop over
  the catalog, so **a new experiment is covered the moment it is added** rather than when
  someone remembers to write tests for it. Prefer extending one of those loops to adding
  a parallel test for a single experiment.
- **A premise that is a geometric claim gets a geometric test.** "On a circle", "evenly
  spaced", "exactly tangential", "inside the speed window" are the whole content of
  `experiment_2`, and none of them are visible from reading the builder without redoing
  the trigonometry. `every_ring_velocity_is_exactly_tangential` is the one to look at
  first if the ring ever stops behaving.
- **Derivations are recomputed, not copied.**
  `the_circular_speed_matches_the_documented_derivation` evaluates `√(G·m/(√3·R))` from
  first principles in `f64` and also checks the `~138 m/s` figure the docs quote, so the
  implementation and the documentation cannot drift together.
- **No test asserts a trajectory.** A three-body system is chaotic; pinning where a body
  ends up after N frames would be pinning the integrator's error, and the test would break
  on any accuracy improvement. `every_experiment_runs_without_going_invalid` asserts only
  validity and finiteness.
- **A claim to be real is checked against a source, not against the code.**
  `solar_system_tests.rs` compares orbital speeds to published `km/s` figures and periods to
  published day counts, so a mistyped exponent in the constants table fails even though the
  scene it produces is self-consistent. A test that only compared the scene to the constants
  that built it would pass with the mass of the Sun wrong.
- **A departure that has been *removed* gets a test that names it.** There are no size
  exaggerations left, so the tests that used to guard them now assert the opposite and each says
  what it replaced — `the_sun_is_now_correctly_proportioned_against_jupiter`,
  `every_body_is_stored_at_its_true_size`,
  `a_black_hole_captures_at_its_tidal_radius_not_its_horizon`. "There is no longer a lie here" is
  only meaningful if the lie is on the record.
- **A limit that forced a design change gets a test that states the limit.** `jovian_tests` asserts
  that the crate-wide softening leaves under `20%` of Newton's force at Io's orbit, and that the old
  `157×` Jupiter was more than `25×` Io's orbital radius. Neither tests the Jovian system — they test
  the *reasons* `Scene::softening_units` and `Prop::contact_radius_units` exist, so a future
  simplification that removes either fails with the argument attached rather than with a mystery.
- **An instrument gets tested as an instrument.** `scale_check_tests` is grouped by the three things
  a ruler has to get right: that it measures the real quantity (radii identical to the experiment it
  checks), that it is readable (every body over `3` px), and that it cannot move (`120` worst-case
  frames through the real integrator, then whole-scene equality). "Is this scene correct" is the wrong
  question for `experiment_5`; "would you trust a measurement taken with it" is the right one.
- **Randomness is tested for what it promises.** Determinism, range, and a ten-bucket
  spread — not statistical quality, which a hand-rolled generator does not claim. See
  `src/experiments/random.rs` on when to reach for `rand` instead.

## Note on `random_tests.rs`'s imports

It deliberately does *not* `use super::*`: `proptest::prelude` re-exports a `Rng` **trait**
from `rand`, and two globs offering that name is an ambiguity error. The explicit
`use super::Rng;` names the one the file is about.

## Adding a test file here

Follow `tst/README.md`: mirror the `src/` path, end the subject with a `#[path]` stub, pin
`REGRESSION_SEEDS` for every `proptest!` block, and keep it runnable with no window and no
GPU.
