# `src/time/` — one clock for the experiment, one for the observer

Every definition of duration in the crate. This layer exists because time used to
be *implicit*: a raw `f32` frame delta was clamped and scaled inside a Bevy system,
handed down through four physics signatures that each re-validated it, and read
independently by the camera and the overlay with nothing recording that those two
were deliberately on a different clock. There was no way to ask how long the
simulation had been running.

**No Bevy import at all** — not even `bevy::math`, since a duration is a scalar.
This is the only layer with no Bevy dependency whatsoever. The `Resource` impls that
let Bevy hold the clocks live in `render/plugin.rs`, the same place `Scene`'s does.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer, the two-clock split, and the import restrictions. |
| `constants.rs` | Every constant the layer reads: `MAX_REAL_STEP_SECONDS`, `DEFAULT_TIME_SCALE`, `MIN_TIME_SCALE`, `MAX_TIME_SCALE`, and the `SECONDS_PER_*` unit definitions the overlay's duration formatting reads. |
| `simulation.rs` | `TimeStep` — the validated interval physics takes — plus `SimulationClock` (scale, pause, elapsed) and `clamp_real_step`. |
| `observer.rs` | `ObserverClock`: wall time for the camera and the FPS readout. |

## The two clocks

| Clock | Runs at | Read by |
| --- | --- | --- |
| `SimulationClock` | `scale` simulated seconds per real second — `5e6` shipped | `Scene::step` and everything under it |
| `ObserverClock` | wall time | the free-fly camera, the overlay's FPS |

A paused experiment can still be flown around and inspected, because pausing stops
one clock and not the other. That is the *point* of the split, not a side effect:
`P` freezes the physics while `WASD` keeps working.

## `TimeStep` is the interface, and the type is the enforcement

```rust
pub struct TimeStep(f64);            // seconds, finite, >= 0

TimeStep::from_seconds(f64) -> Option<TimeStep>   // the only constructor from a float
clock.advance(real_dt) -> TimeStep                // the only producer in normal use
step.split(n) -> TimeStep                         // substeps keep the guarantee
```

Every physics entry point takes one:

| Function | Signature |
| --- | --- |
| `Scene::step` | `(&mut self, step: TimeStep)` |
| `integrate_gravity` | `(&mut [Prop], step: TimeStep)` |
| `substep_count` | `(&[Prop], step: TimeStep) -> u32` |

Because there is no other way to build one from a float, **there is no path by which
an unvalidated duration reaches the integrator**, and no second place where a "dt" is
decided. Requirement 5.5 (`dt >= 0`) went from a `debug_assert!` at the bottom of the
call stack to a property of the type, which also means it holds in release.

What this layer deliberately does **not** claim: the closed-form relations in
`physics_utils/`, which still take a plain `elapsed_s: f64`. They are scalar
mathematics, callable from a test with no clock in sight, and coupling them to a
simulation type would be the wrong kind of centralisation. The boundary is exact — a
`TimeStep` is the currency of *stepping the world*, and the `f64` it yields via
`seconds()` is what feeds the formulas inside a step.

## The pipeline

```text
Time::delta_seconds()                     f32, real seconds, variable
  │
  ▼  advance_clocks_system            (render/plugin.rs — the only Bevy timer read)
  ├─ ObserverClock::advance(raw)      → raw_step_seconds() for FPS
  │                                     motion_step_seconds() for the camera
  └─ SimulationClock::advance(raw)
       ├─ clamp_real_step(raw, MAX_REAL_STEP_SECONDS)    Requirement 13
       ├─ zero if paused (unless a single step is queued)
       ├─ × scale                                        → simulated seconds
       └─ += elapsed, store as last_step
  │
  ▼  step_scene_system
Scene::step(clock.last_step())
  └─ step.split(substep_count(..)) per substep → integrate_gravity + resolve_contacts
```

**Clamp before scale.** The cap is on the *real* interval, so the largest simulated
step is `max_real_step_seconds() × scale()` — `5e5` simulated seconds in the shipped
app, the figure the collision substep budget was sized against. Scaling first would
make the cap mean a different thing at every scale.

## Two step readings on the observer clock

`raw_step_seconds()` is the delta as the timer reported it — an FPS row must use it
or it would report the clamp instead of the truth. `motion_step_seconds()` is the
same delta capped, and camera motion must use it or a half-second stall would
teleport the viewer. Both are real time; they differ only in whether a hitch is
honoured.

## Time only moves forward

`TimeStep` is a non-negative magnitude. Reversibility was considered and deferred,
because a signed step that is not honoured is worse than none — it would look like it
worked and quietly diverge. Making one meaningful needs three things this crate does
not have:

1. **A reversible integrator.** Semi-implicit Euler is not its own inverse; `−dt` does
   not undo `+dt`. Velocity Verlet or leapfrog would be required, and switching
   changes *forward* trajectories too, invalidating the orbit-drift measurements
   `core/integrator.rs` documents.
2. **A contact policy for reverse.** The elastic impulse is time-symmetric, but the
   "only if approaching" gate is direction-dependent and positional de-penetration is
   a projection that destroys the information needed to invert it.
3. **Two amendments** — Requirements 5.5 and 13.3 both currently specify that a
   negative duration is an error to be clamped away.

The place to add the sign is `TimeStep`, and nowhere else. That is most of the value
of having the type.

## Rules and invariants

- **No Bevy import.** A duration is a scalar; if a change here needs `bevy::math`,
  the change is in the wrong layer.
- **`f64` throughout.** Elapsed simulated time reaches `2e8` seconds in the shipped
  scene, and accumulating that in `f32` would lose whole seconds. Narrowing happens
  at exactly one place, `TimeStep::seconds_f32`, named so the loss is visible.
- **`from_seconds` rejects; `clamp_real_step` sanitises.** A duration a caller states
  explicitly is a claim, so an invalid one is refused (`Option`). A frame delta comes
  from a timer that may hiccup, so Requirement 13.3 says to turn it into `0.0`. Two
  different jobs, deliberately two functions.
- **A single-step request is a boolean, not a counter.** A held key must not queue
  frames and then run the simulation away when released.
- **The scale is clamped, never rejected silently.** `set_scale` clamps into
  `[MIN_TIME_SCALE, MAX_TIME_SCALE]` and *ignores* a non-finite value rather than
  substituting a default — a clock many things read should not jump to real time
  because a caller's arithmetic produced `NaN`.

## Where the tests are

- `tst/time/simulation_tests.rs` — 4 properties, 20 unit tests. The `TimeStep` cases
  assert that nothing invalid can be built and that splitting preserves the
  guarantee; the clock cases assert clamp-before-scale, accumulation, pause,
  single-step, and scale clamping.
- `tst/time/observer_tests.rs` — 7 unit tests, including the raw-versus-capped split
  and a direct assertion that pausing the simulation leaves observer time running.

**Test durations are binary-exact fractions** (`1.0 / 64.0`, not `0.01`) wherever an
assertion is exact: `0.01f32 as f64` is `0.009999999776…`, and a test that ignores
that fails for reasons which have nothing to do with the clock.

## Adding a file here

- No Bevy. `f64` for durations. SI seconds, with the unit in every name.
- If it produces a duration that physics will consume, it must produce a `TimeStep`
  and not an `f64` — the type is the enforcement, and a second float-shaped producer
  would defeat it.
- A new clock needs its `Resource` impl in `render/plugin.rs` and its own row in the
  two-clock table above and in `mod.rs`.
