# `src/input/` — input → camera motion

Turns keyboard, mouse, and window state into camera movement. Separate from
`render/` because it drives the camera entity's `Transform` directly and never
touches `Scene`.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer. |
| `constants.rs` | Every constant the layer reads: the `FreeFlyController` defaults (`CAMERA_MOVE_SPEED_UNITS_S`, `CAMERA_BOOST_MULTIPLIER`, `LOOK_SENSITIVITY_RAD_PER_PX`), the pitch clamp's `PITCH_LIMIT_MARGIN_RAD`, and `TIME_SCALE_KEY_FACTOR`. |
| `camera.rs` | `FreeFlyController` (per-entity tuning + yaw/pitch/captured state), the pure `clamp_pitch` helper, `free_fly_camera_system`, and `FreeFlyCameraPlugin`. Also the camera lock: `CameraTarget`, `CameraLock(Option<Orbit>)`, `toggle_camera_lock_system` (`F`), and `orbit_camera_system`, which places a locked camera on its orbit after scroll zoom in `PostUpdate`. Moves on **observer** time, so the camera is unaffected by the simulation being paused or rescaled. |
| `controls.rs` | `SimulationControlsPlugin`, `simulation_controls_system`. The clock keys: `P`, `.`, `]`, `[`, `\`. |
| `menu.rs` | `MenuState`, `menu_toggle_system`, `MenuInputPlugin`. Whether the `Esc` menu is open — the layer's **input mode** — and the key that toggles it. The panel itself is `ui/menu.rs`. |
| `mission.rs` | `LaunchRequest`, `launch_input_system`, `LaunchInputPlugin`. `R` asks to pick the body at the crosshair as a rocket's origin or destination, and `G` asks for the next spacecraft in the fleet; `render/mission.rs` does the picking and flies it. |
| `placement.rs` | `PlacementState`, `placement_input_system`, `PlacementInputPlugin`. The `B` picker: which interstellar object is selected, and whether a placement has been asked for. |

### Two families of movement axis

| Keys | Axis | Why |
| --- | --- | --- |
| `W`/`S`, `A`/`D` | Camera-**local** forward and right | Flying where you are looking |
| `Space`/`Left Ctrl` | World `±Y` | Pitching the view must not tilt vertical flight |
| `E`/`Q` | World `±Y` — **aliases** for the above | Muscle memory differs by tool: editors bind `Q`/`E`, shooters `Space`/`Ctrl` |

The world-axis bindings deliberately ignore the camera's orientation: a key whose direction
depended on the view would be useless for the one thing it is for.

`E`/`Q` were world `±Z` when introduced (amendment A6) and moved to `±Y` in A8. Two
consequences worth knowing:

- Holding `E` *and* `Space` is **one** unit of rise, not two — both sum into a single
  `delta` before normalisation. A test pins that, because applying them separately is the
  classic aliasing bug.
- **No key moves along world Z any more.** Sliding along Z means turning to face it, which
  changes what you are looking at as well as where you are. `no_key_moves_along_world_z_any_more`
  exists so that is visibly a decision rather than a regression.
| `controls.rs` | `SimulationControlsPlugin` and `simulation_controls_system`: `P` pause, `.` single-step, `[` / `]` slower/faster, `\\` reset the scale. Mutates the `SimulationClock` and nothing else. |

## The full key map

The source of truth is [`KEY_BINDINGS`](../constants.rs), which `--help` and the `Esc` menu
both render, so those two cannot drift apart. This copy can — check it against `--help`.

| Input                 | Action                                 |
| --------------------- | -------------------------------------- |
| Left-click the window | Capture cursor for mouse-look          |
| Mouse                 | Look around (while captured)           |
| `W` / `S`             | Move forward / backward                |
| `A` / `D`             | Strafe left / right                    |
| `Space` / `Left Ctrl` | Move up / down along world **Y**       |
| `E` / `Q`             | Up / down as well — aliases for the above |
| `C`                   | Cycle camera speed: normal (1x), fast (3x), slow (0.02x) |
| `F`                   | Lock onto the body nearest the crosshair, or release the lock |
| Mouse (while locked)  | Orbit the locked body, after a left click captures the cursor |
| Scroll (while locked) | Zoom in / out on the locked body       |
| `T`                   | Show / hide motion trails (on by default) |
| `B`                   | Open / close the object placer         |
| `Up` / `Down`         | Choose which object to place           |
| `Enter`               | Place the selected object in front of the camera |
| `R`                   | Rocket: `R` on the departure body, then `R` on the destination (`R` on the origin again cancels) |
| `G`                   | Next spacecraft for the next launch: crewed `3 g` transfer (default), full thrust, coasting probe |
| `Esc`                 | Open / close the menu (also releases the cursor) |
| `P`                   | Pause / resume the simulation           |
| `.`                   | Advance one frame while paused          |
| `]` / `[`             | Double / halve the time scale           |
| `\`                   | Reset the time scale to the experiment's own |
| `-`                   | Ten times slower: divide the time scale by 10 |
| `Cmd`-`Q` / `Alt`-`F4` / Ctrl-C | Quit — the window is full-screen and has no close button |

The camera keys keep working while the simulation is paused, because they run on the
observer clock (see [`src/time/README.md`](../time/README.md)).

## Constants

Every tunable value this layer reads lives in `constants.rs` — none are declared in
an implementation file, per
[`.claude/steering/constants.md`](../../.claude/steering/constants.md). Each one
documents why it holds that value, who reads it, and what changes if it moves, so
this README does not restate them; open `constants.rs` to see the layer's whole
tuning surface at once.

These govern *feel*, the one thing in this crate with no correct answer — but each still
has a stated reason and a documented way to be wrong. `CAMERA_MOVE_SPEED_UNITS_S` is
`20.0` units per second: the world is 100 units across, so that crosses it in five
seconds, and a `3×`-boosted 60 Hz frame still moves less than the smallest body's diameter
— both bounds asserted by `the_camera_speed_is_usable_for_this_scene`.

## The menu is an input mode, and it lives here

`menu.rs` holds `MenuState` — whether the `Esc` menu is open — and the `Esc` binding that
toggles it. The *panel* is drawn in `ui/menu.rs`; only the state and the key are here,
because what the state really encodes is **whether the app is accepting the
flying-and-observing bindings at all**. That is the same kind of fact as cursor capture,
which this layer already owns.

Putting it in `ui/` would mean `input/camera.rs`, `input/controls.rs`, and
`render/trail.rs` all importing the UI layer to ask whether to run — inverting the
dependency direction that already exists (`ui` and `render` read `input`, never the
reverse).

**One rule while the menu is open: only `Esc` is bound.** Camera movement, mouse-look, the
clock keys, and the trail toggle are all inert. One rule beats a per-key judgement — a menu
in which `W` still flies is a menu you cannot read, and "which keys should still work" is a
set nobody can remember. Each gated system takes `Option<Res<MenuState>>` and returns early;
absent state means closed, so a system added without the plugin still runs.

Two things the gate deliberately still does:

- **Releases the cursor**, in `free_fly_camera_system`, because a menu you cannot point at
  is not a menu — and because that system already owns the window's cursor state, so there
  is only ever one writer of it.
- **Drains `MouseMotion`**, every frame, before returning. This is not optional: buffered
  deltas would all land on the frame after the menu closed and snap the view. It has its own
  test.

`Esc` used to release the cursor directly; now it opens the menu and the release is a
consequence, so the key does what it always did and shows you something as well
(amendment A13). Opening the menu does **not** pause the simulation — `P` does that, and a
menu that also paused would silently change the run you opened it to inspect.

## Three speeds, as a mode rather than a hold

`C` cycles the camera through `normal → fast → slow → normal`. The multipliers are `1×`, `3×`
(`CAMERA_BOOST_MULTIPLIER`), and `0.02×` (`CAMERA_PRECISION_MULTIPLIER`). The asymmetry is not an
oversight — fast exists to cross a `4495`-unit solar system, slow to approach a body `0.0064` units
across.

This replaced two hold modifiers, `Left Shift` to boost and `C` to slow (amendment A26). Holding a key
for the length of a flight is tiring, and the two flights you most want a modifier for — crossing to
Neptune, creeping up on Earth — are exactly the longest ones. Shift is now unbound.

**The cost, and what pays for it.** A held key announces itself; a mode does not. So the debug overlay
carries a `cam` row naming the current mode and its multiplier. That row is not polish — without it a
user who tapped `C` once too often has a camera `150×` slower than they expect and nothing on screen
to say why. `overlay_tests` treats it as load-bearing, including that the three rows are mutually
distinguishable and that the multiplier column is not ragged.

Two consequences worth knowing. The modes are **mutually exclusive**, so the old combined `0.06×` is
no longer reachable — three speeds you can name beats nine you cannot. And the wrap from `fast` to
`slow` is a `150×` jump; that is inherent to cycling three states with one key, since some adjacent
pair has to be the extremes.

`C` rather than a modifier key for a reason that predates the toggle (amendment A25): on macOS
`Cmd`-`W` — the old slow modifier plus *forward* — is the system's close-window shortcut, so it could
shut the application mid-flight.

On the clock side, `[` and `]` halve and double while `-` and `+` divide and multiply by ten. A decade key exists because
the experiments' scales differ by decades (`1e4` Jovian, `1e6` solar, `2e7` ring), and getting between
them by halving takes nine presses. There is no `×10`: `\` resets to the experiment's own scale,
which is the way back.

## The object placer is deliberately *not* modal

`B` opens a picker; `Up`/`Down` choose; `Enter` places. Unlike the `Esc` menu, **every other
binding stays live**, because the object is placed in front of the camera and you have to be
able to fly and aim while choosing. None of its four keys collide with the camera or the clock,
which is what makes that safe, and a test asserts it.

`Enter` rather than a click, because left-click already captures the cursor — and does so *while
the picker is open*, since the picker is not modal. Overloading it would mean the first click
aims and the second places, which is exactly the kind of invisible state a UI should not have.

The request is a **flag**, not a call: `Enter` sets `place_requested` and this layer stops
there. `render/placement.rs` consumes it, works out where the camera is pointing, and adds the
prop — which is what keeps this layer's rule below intact.

The picker is still subordinate to the menu: opening the menu **closes** it, rather than merely
ignoring it, so dismissing the menu cannot reveal a forgotten picker with `Enter` armed.

## Rules and invariants

- **This layer never reads or writes `Scene`.** If a feature needs scene state,
  it belongs in `render/`.
- **`FreeFlyCameraPlugin` does not spawn the camera.** It only registers the
  system; the camera entity — `Camera3dBundle` + `FreeFlyController` — is
  spawned by `render/plugin.rs`'s `spawn_scene_entities`. The plugin is
  therefore inert without `ScenePlugin`. This is a known deviation from
  Requirement 10.1, acknowledged in task 8.8 of the original spec's task list.
- **Exactly one entity carries `(Camera3d, FreeFlyController, Transform)`.** The
  system uses `get_single_mut()` and returns early rather than panicking, so a
  second 3D camera degrades to "camera controls stop working" rather than a
  crash.
- **Drain `MouseMotion` on every code path, including early returns.** Buffered
  deltas would otherwise snap the view when capture is regained.
- **`yaw`/`pitch` are the source of truth once mouse-look runs.**
  `transform.rotation` is overwritten wholesale, never composed, so any external
  writer of camera rotation loses on the next captured frame.
- **`dt` scales translation only.** Mouse-look is deliberately `dt`-independent
  because mouse deltas are already per-frame displacements.
- **Movement is normalized before scaling,** so diagonal input is not faster;
  vertical movement uses world `Vec3::Y`, not the camera's local up.
- **`clamp_pitch` is total:** the `NaN` branch runs *before* `f32::clamp` so a
  `NaN` can never reach `Quat::from_euler`.
- **A new binding gets a row in `KEY_BINDINGS`, in the same change.** That table is what
  `--help` prints and what the menu draws, and nothing enforces the link — a key bound in
  code and forgotten there is simply undocumented.
- **This layer still never writes `Scene`, including for placement.** The placer records
  *what* and *whether*; the render layer decides *where* and does the adding.
- **A new gated system takes `Option<Res<MenuState>>`, not `Res<MenuState>`.** Half the
  tests in this crate build the smallest app that exercises one system, and a hard `Res`
  turns "I added one system" into a first-frame panic.

## Known defects, so you do not rediscover them

- The camera spawns with a `looking_at` orientation but the controller starts at
  `yaw = pitch = 0`, so the first mouse pixel after capture jumps the view
  ~54.7° and the demo scene leaves the frustum. Fix is to seed `yaw`/`pitch`
  from the spawn transform.
- Left-click is read with no egui-focus gate, so clicking the debug overlay
  grabs the cursor. On macOS `CursorGrabMode::Locked` is a display-global
  dissociation, which freezes the desktop pointer until Escape.
- The Requirement 10.9 capture-loss check reads `window.cursor.grab_mode`, which
  Bevy only ever writes *outward* to the OS. The branch is unreachable. The
  usable signal is `Window::focused` / the `WindowFocused` event.

## Where the tests are

`tst/input/camera_tests.rs` — 6 tests. Note the coverage is thin: only
`clamp_pitch` and a no-camera, no-window smoke test are exercised. Requirements
10.2–10.9 (capture, mouse-look, WASD, boost) are **unverified**, though they are
headlessly testable by spawning `(Camera3d, Transform, FreeFlyController)` plus
a `(Window, PrimaryWindow)` entity and pumping `ButtonInput`/`MouseMotion`.

## Adding a file here

- Keep the pure math (clamps, curves, sensitivity mapping) in free functions
  outside the system, as `clamp_pitch` is, so it is property-testable without an
  `App`.
- Anything that reads input must tolerate a missing window and a missing camera;
  headless tests run without either.
