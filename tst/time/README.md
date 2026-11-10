# `tst/time/` — tests for the two clocks

> **Purpose:** Tests for [`src/time/`](../../src/time/README.md): `TimeStep` as a guarantee and
> both clocks as a policy (clamp, scale, pause, single-step).
> **Edit here to:** add or fix a clock or `TimeStep` test.
> **Not here:** the clocks, in `src/time/`.

Mirrors `src/time/`, one file per source file. Both are `#[path]`-included unit-test
modules compiled *inside* their subject, so they use `crate::…` paths and can reach
private fields (`elapsed_s`, `single_step_pending`).

## What lives here

| File | Subject | Shape of the tests |
| --- | --- | --- |
| `simulation_tests.rs` | `simulation.rs` | `TimeStep` as a *guarantee* (nothing invalid can be built, splitting preserves it) and `SimulationClock` as a *policy* (clamp, scale, accumulate, pause, single-step). 4 properties, 20 unit tests. |
| `observer_tests.rs` | `observer.rs` | Wall time, and the raw-versus-capped step split. 7 unit tests. |
| `proptest-regressions/` | — | Counterexample seeds proptest has already paid for. Committed. |

## Conventions specific to this directory

- **Binary-exact durations wherever an assertion is exact.** Frames are `1.0 / 64.0`
  and `1.0 / 32.0`, never `0.01` or `0.02`: `0.01f32 as f64` is `0.009999999776…`, so
  a test asserting `10.0` against a scaled `0.01` frame fails for reasons that have
  nothing to do with the clock. Every such case carries a comment saying so, because
  the next person will otherwise "fix" it back to a round number.
- **The clamp order is asserted, not assumed.** `the_clamp_applies_before_the_scale`
  exists because the two operations commute arithmetically but not semantically: the
  cap is on the *real* interval, so scaling first would make it mean a different thing
  at every scale.
- **Pause is tested for what it does to the *other* clock too.**
  `the_observer_clock_is_independent_of_the_simulation_clock` asserts the property the
  whole split exists for — a frozen experiment you can still fly around — as a
  running test rather than a claim in a README.

## What moved here from elsewhere

The FPS cases (`0.0` on the first tick, `0.0` rather than `inf` for a sub-microsecond
delta, `0.0` for a non-finite delta) came from `ui/overlay.rs`'s private
`fps_from_dt`, which `ObserverClock::fps` replaced. If you are looking for them in
`tst/ui/overlay_tests.rs`, that is why they are not there.

The `Scene::step` and `integrate_gravity` precondition tests that used to assert a
`debug_assert!` on a negative or `NaN` `dt` are **not** here either: that contract is
now the type's, so `tst/core/scene_tests.rs` and `tst/core/integrator_tests.rs`
assert that `TimeStep::from_seconds` rejects those values instead — which holds in
release too, where a `debug_assert!` does not.

## Adding a test file here

Follow `tst/README.md`: mirror the `src/` path, end the subject with a `#[path]`
stub, pin `REGRESSION_SEEDS` for every `proptest!` block. A new clock needs a test
that it cannot produce an invalid `TimeStep` from any input sequence — that is the
property every physics function downstream relies on instead of validating for
itself.
