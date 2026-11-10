# `src/` — crate sources

> **Purpose:** The crate root (`lib.rs`, `main.rs`), the files shared by every layer
> (`constants.rs`, `bindings.rs`), and the map of the ten layer directories.
> **Edit here to:** add a layer, change the window or app wiring, add a key binding, add a
> constant two layers read, or re-export a new public item.
> **Not here:** anything a single layer owns. Find the layer in the table below.

The `physics-sandbox` crate root and its ten layer directories. Every layer is a
module directory with its own `README.md`; this file explains how they relate
and where a new one would go.

## What lives here

| Path | Job |
| --- | --- |
| `lib.rs` | Crate root. Declares the ten layers and re-exports every public item at the crate root. |
| `constants.rs` | Constants read by **more than one layer** — the unit scale, the shipped scene's configuration, the key-binding table — plus the window values `main.rs` owns. |
| `bindings.rs` | The key bindings as *data*: `Action`, the `KeyMap` resource every input system reads keys through, `BindingGroup`, `KeyBinding`, key names, and the rows `--help` and the `Esc` menu both draw. Above every layer because several layers need it and none may import another. |
| `main.rs` | Binary entry point. Builds the `App`, loads `config/*.toml` (see `config/`) and inserts it before any plugin, configures the window, adds the plugins, calls `run()`. The window is **full-screen and borderless** (`WINDOW_MODE` in `constants.rs`, amendment A12), so it has no close button: quit with `Cmd`-`Q`, `Alt`-`F4`, or `Ctrl`-`C`. Set `WINDOW_MODE` to `WindowMode::Windowed` for an 800×600 window. |
| `core/` | GUI-free world model. Bevy-free apart from `bevy::math`. |
| `collision/` | Body physics: the triangle hull that bounds each prop, the contact test over those hulls, and the elastic response. |
| `render/` | The boundary that mirrors the model onto Bevy's ECS. |
| `input/` | Keyboard and mouse → camera motion, the clock keys, and the menu's input mode. |
| `ui/` | The `bevy_egui` overlay, crosshair, compass, and the `Esc` menu panel. |
| `physics_utils/` | Physical constants and closed-form formulas, in `f64`. |
| `time/` | The two clocks and the `TimeStep` every physics entry point takes. No Bevy import at all. |
| `experiments/` | The catalog of simulations, each one's settings, and the command-line selection that picks one. |
| `spacecraft/` | What a craft is, the fleet `G` cycles, planning a flight (the least-energy transfer by default), flying it, and its report. Above `core`, Bevy-free but for `bevy::math`. |
| `performance/` | The `RGA_*` switches that trade image quality for frame time (anti-aliasing, floor eclipse, vsync), the frame-time benchmark, and the measurements behind each default. |
| `config/` | Reads `config/*.toml` at startup into the resources the other layers own: the fleet, camera tuning, crosshair, compass, arrival pop-up, trails, performance switches. Above every other layer, beside `main.rs`. |

## Rules and invariants

- **Layers do not reach sideways through the crate root.** Inside the crate,
  refer to another layer by its real path (`crate::core::scene::Body`), not via
  the `lib.rs` re-export. The re-exports exist for consumers, and routing
  internal references through them hides the real dependency graph.
- **`time/` imports no Bevy at all**, and `core/`, `collision/`, and
  `physics_utils/` may import `bevy::math` and nothing else from Bevy — no
  `bevy::ecs`, no `bevy_egui`, no `wgpu` (Requirements 4.2 and 15.3). This is
  enforced by review, not by a lint; see `core/README.md`.
- **`lib.rs` re-exports everything public.** Consumers write
  `physics_sandbox::Scene`, never `physics_sandbox::core::scene::Scene`, so the module
  tree can be reshaped without a breaking change. A new public item that is not
  re-exported breaks that promise.
- **No constant is declared in an implementation file.** Each layer keeps its own
  in `<layer>/constants.rs` (`physics_utils/physics_constants.rs` for that one), and
  a value read by two layers moves up to `src/constants.rs`. Every constant states
  why it holds that value, who reads it, and what changes if it moves. The rule,
  including what counts as a constant and what is just arithmetic, is
  [`.claude/steering/constants.md`](../.claude/steering/constants.md); the audit is one
  grep, at the bottom of that file.
- **The frame pipeline is `advance_clocks` → `step` → `sync_scene_to_ecs` →
  render.** The ordering edges live in `render/plugin.rs`; nothing else may insert
  itself between them. Inside `step` the order is *integrate → resolve contacts*,
  repeated per substep.
- **Physics is advanced only by a `TimeStep`.** `Scene::step`,
  `integrate_gravity`, and `substep_count` take `crate::time::TimeStep`, never a
  float, and the only constructor from a float rejects anything invalid. A new
  physics entry point taking an `f32` duration breaks the guarantee that no
  unvalidated duration can reach the integrator — see `src/time/README.md`.
- **The app's identity comes from an experiment, not from constants.** Which scene,
  which clock scale, and where the camera starts are all on the `ExperimentSpec` that
  `ScenePlugin` inserts; nothing in `render/` may go back to hard-coding them. See
  `src/experiments/README.md`.
- **The camera and the overlay read `ObserverClock`, never the simulation clock.**
  Observer time is wall time so the experiment can be watched while paused or
  rescaled; a viewer-side system reading simulated time would freeze with it. Inside `step(dt)` the order is
  *integrate → resolve contacts*, repeated per substep; `core/scene.rs` owns that
  loop and `collision/` owns the second half of it.

## Where the tests are

`tst/`, mirroring this tree directory for directory. Each source file ends with

```rust
#[cfg(test)]
#[path = "../../tst/<layer>/<module>_tests.rs"]
mod tests;
```

so the tests compile *inside* their module — they keep access to private items
and `super::*` while the source file stays focused on implementation. See
`tst/README.md`.

## Adding a layer here

1. Create `src/<layer>/` with a `mod.rs`, a `README.md`
   (see `.claude/steering/source-directory-docs.md`), and a `constants.rs` if it has
   any tunable value (see `.claude/steering/constants.md`).
2. Create the mirror `tst/<layer>/` with its own `README.md`.
3. Declare `pub mod <layer>;` in `lib.rs` and re-export the layer's public items
   at the crate root.
4. Add the layer to the directory map in the root `README.md` and to the
   table above.
5. State the layer's import restrictions in its `README.md`. A layer that may
   import anything is not a layer.
