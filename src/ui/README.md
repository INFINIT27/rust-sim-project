# `src/ui/` — on-screen UI

> **Purpose:** Everything drawn in screen space with `bevy_egui`, its only importer: the debug
> overlay, crosshair, compass, `Esc` menu panel, `B` picker panel, and arrival pop-up.
> **Edit here to:** change what an on-screen panel shows, its layout, or how its numbers are
> formatted.
> **Not here:** the keys and input modes that open panels are in
> [`input/`](../input/README.md). World-space drawing (trails, grid, flight paths) is in
> [`render/`](../render/README.md).

The debug overlay, and the quarantine for the `bevy_egui` dependency.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer. |
| `constants.rs` | Every constant the layer reads: the overlay row budgets (`MAX_PROP_ROWS`, `MAX_DISTANCE_ROWS`), the crosshair defaults (`CROSSHAIR_*`), the compass's size, placement, and colours (`COMPASS_*`), the menu's layout (`MENU_*`), the picker's (`PLACEMENT_PANEL_*`), and the arrival pop-up's (`ARRIVAL_*`, `JOULES_PER_TONNE_OF_TNT`). The only lengths in the crate that are in pixels rather than world units. |
| `overlay.rs` | `DebugUiPlugin`, `debug_overlay_system` (one egui window: FPS, prop count, both clocks, camera pose, and a per-prop SI readout), plus the pure helpers `units_to_metres`, `prop_lines`, `distance_lines`, `clock_lines`, `human_duration`. |
| `crosshair.rs` | `CrosshairPlugin`, `crosshair_system`, `CrosshairConfig`, and the pure `crosshair_segments` geometry helper. Draws four short strokes with a gap at the centre of the view. |
| `hud.rs` | `HudPlugin`, `HudVisibility`, `toggle_hud_system`, and the run condition `hud_shown`. `H` hides the whole HUD (the debug panel and mission readout, the crosshair, the compass, and the `B` picker) and shows it again; each of those panels' systems runs only while `hud_shown`. The `Esc` menu and the arrival pop-up are not HUD, and stay. |
| `compass.rs` | `CompassPlugin`, `compass_system`, `CompassConfig`, `CompassAxis`, and the pure `compass_axes` projection. Draws the three world axes in the top-right corner, rotating with the camera. |
| `menu.rs` | `MenuPanelPlugin`, `menu_system`, `MenuRequests` and `SaveTarget` (how a page asks the `config` layer to save), `settings_rows`, `anti_aliasing_label`. The `Esc` menu: a first page (Settings, Configuration, Exit, a summary of the run), Settings (controls with rebinding, video) and Configuration (camera, interface, trails, spacecraft), each with Reset and Save. |
| `placement.rs` | `PlacementPanelPlugin`, `placement_panel_system`, `PlacementRow`, and the pure `placement_rows`. The `B` picker, bottom-centre: five interstellar objects with their masses. Non-interactive — the arrow keys drive it. |
| `arrival.rs` | `ArrivalPopupPlugin`, `arrival_popup_system`, `ArrivalPopupConfig` (its sizes, from `config/ui.toml`), and the pure `arrival_rows`, `duration_label`, `energy_label`. The pop-up when a spacecraft lands: route, time of travel, top speed (and fraction of light), Δv, engine time, peak g, energy (with its TNT equivalent), propellant, touchdown speed, corrections. Releases the cursor while open (`PopupState`); its title-bar ✕ or `Esc` dismisses it for that landing, and the next `R` also closes it. Drawn above the crosshair, which paints on egui's background layer. |

## Constants

Every tunable value this layer reads lives in `constants.rs` — none are declared in
an implementation file, per
[`.claude/steering/constants.md`](../../.claude/steering/constants.md). Each one
documents why it holds that value, who reads it, and what changes if it moves, so
this README does not restate them; open `constants.rs` to see the layer's whole
tuning surface at once.

Note these are the crate's only lengths measured in **pixels** rather than world
units, which is why the names say `_PX`.

## The compass: how a flat drawing shows a 3D orientation

`compass_axes(rotation, radius)` transforms each world axis into the camera's frame — `q⁻¹ ·
d` — and splits the result in two: the screen arm `(v.x, −v.y) · radius` (screen `y` grows
downward, hence the negation), and `toward_viewer = v.z`, which is **positive toward the
viewer** because Bevy's view space looks along `−Z`.

That third component is the whole trick. An axis pointing at your eye and one pointing away
project to the *same* zero-length arm, so without a depth cue the compass would be ambiguous
exactly when you are looking along an axis — the moment you most need it. `compass_system`
fades a receding arm to `COMPASS_DIM_ALPHA` and draws back-to-front, which is what makes the
three lines read as an orientation rather than as a shape.

Translation is deliberately absent from the signature: a compass shows **heading, not
position**, so it must look identical from anywhere in the scene at the same attitude. A test
pins that decision.

Colours match the floor grid — red X, blue Z — so the compass and the lines on the ground
agree. Y is green: it is the one axis with no line on the floor, because it points out of it.

## Rules and invariants

- **This is the only module in the crate that may import `bevy_egui`.** That is
  the layer's reason for existing: swapping the debug panel for a Bevy-native UI
  should be a one-directory change. If you find yourself importing egui
  elsewhere, move the code here instead.
- **The overlay is read-only.** Enforced at compile time by its parameter types:
  `Res<Scene>`, `Res<Time>`, `Query<&Transform, …>`, and `FreeFlyController` is
  not queried at all (Requirement 11.3). Keep it that way — a mutating overlay
  would make the debug UI part of the simulation.
- **The menu is the one interactive widget, and the state is what makes it safe.**
  `menu.rs` has a `Close` button, so it *does* claim pointer input — which the rule below
  otherwise forbids. The resolution is not a `wants_pointer_input` check but
  `MenuState`: while the menu is open the camera and the clock keys are inert anyway
  (amendment A13), so the two are never live at the same time and there is no contention to
  arbitrate. Any *other* module here adding a widget still needs the gate the rule
  describes.
- **The placement picker is driven by keys, not clicks, and that is what keeps it
  non-interactive.** It is open while you fly, so the cursor is captured for mouse-look and
  there is nothing to click with. `placement_rows` is pure and the panel is labels only, so the
  interactive-widget exception above applies to the menu and *only* the menu.
- **The menu shows settings; it does not own them.** Every row is read from the resource
  that holds it — the `ExperimentSpec`, the `SimulationClock`, the `TrailConfig` — so the
  panel cannot disagree with the app about what the app is doing. The bindings come from
  `crate::bindings::binding_rows`, the same rows `--help` prints.
- **Nothing else here may consume input the camera needs** (Requirement 11.4). The
  overlay holds because it uses `ui.label` exclusively — labels are
  non-interactive, so egui never claims focus. The crosshair holds more strongly:
  it paints onto the background layer (under every window, so centred panels cover it) via
  `Context::layer_painter` rather than registering a widget at all, so egui has nothing to
  give focus to. Adding any interactive widget to either breaks the guarantee. The two
  panels that do take clicks, the menu's Close button and the arrival pop-up's ✕, are
  safe because the camera stands down while `MenuState` or `PopupState` is open.
- **The panel's content is pure, because the painting cannot be tested.**
  `settings_rows` and `binding_rows` are plain functions over the resources, and they carry
  the tests; `menu_system` is kept thin enough to read. The same split the crosshair,
  compass, and grid use, and for the same reason — `EguiContexts::ctx_mut` needs a real
  window.
- **The readout is in SI units, and that conversion lives here.** Scene lengths and
  velocities are in world units (`1 unit = 1e9 m`); `units_to_metres` scales them for
  display. This is the crate's *second* conversion site after `core::integrator`, and
  it is a display concern only — the scene keeps working in units. The single most
  likely defect is a forgotten multiply, which prints a plausible-looking number nine
  orders of magnitude too small, so it has its own test.
- **Truncate loudly.** `prop_lines` and `distance_lines` cap at `MAX_PROP_ROWS` and
  `MAX_DISTANCE_ROWS` and append a `"... and N more"` row. Pair count grows as
  `n(n-1)/2`, so the distance list saturates first — six props already make 15 pairs.
  A truncated list that looks complete is worse than one that says so.
- **Scientific notation throughout, deliberately** — see *How to read the readout*
  below. The quantities span `1e-8 m/s` early in a free fall to `5.972e24 kg`;
  fixed-point would print either all zeros or an unreadable wall of digits.
- **Keep pure geometry and formatting out of the systems.** `fps_from_dt` and
  `crosshair_segments` both take and return plain numbers, so they are
  property-testable with no egui context. The systems are thin shells that convert
  those numbers into egui calls. Follow that split for anything new — it is the
  only reason this layer has meaningful test coverage at all.
- **Colour spaces are kept apart deliberately.** `CrosshairConfig::color` is an
  sRGB `[u8; 4]`, because that is what egui wants. `scene::Color` is *linear*
  RGBA for PBR materials. Passing one where the other is expected silently washes
  colours out, so neither type is reused for the other's job.
- **No explicit schedule ordering is needed.** `bevy_egui` 0.28 puts
  `InitContexts`/`ProcessInput`/`BeginFrame` in `PreUpdate` and `ProcessOutput`
  in `PostUpdate`, so anything on plain `Update` lands correctly between them.
- **`DebugUiPlugin` has a hard, undeclared dependency on `ScenePlugin`.**
  `Res<Scene>` is not `Option<Res<Scene>>`, so adding this plugin without a
  `Scene` in the world panics at system-param resolution.
- **`EguiContexts::ctx_mut()` needs a primary window with an egui context.**
  Under `MinimalPlugins` you must supply `Assets<Shader>`, `Assets<Image>`, the
  five input events, `RequestRedraw`, `AssetEvent<Image>`, and a
  `(Window, PrimaryWindow)` entity — see the note in the tests.

## How to read the readout

Every figure in the overlay is SI: **metres**, **kilograms**, **metres per second**.
Nothing is in world units, and nothing is in a scaled or "display" unit.

**The `e` is a decimal exponent, not Euler's number.** `7e2` means `7 × 10² = 700`,
not `7·e² ≈ 51.7`. Rust's `{:e}` also prints no `+` on positive exponents and no
padding, which is why zero shows as `0.000e0` rather than `0.000e+00`.

| Displayed | Means | Note |
| --- | --- | --- |
| `6.684e1 m/s` | 66.84 m/s | |
| `1.818e2 m/s` | 181.8 m/s | |
| `0.000e0 m/s` | 0 m/s | at rest |
| `5.972e24 kg` | 5.972 × 10²⁴ kg | one Earth mass |
| `3.0000e10 m` | 3 × 10¹⁰ m | ≈ 0.2 AU, the two-body starting separation |
| `1.5297e10 m` | 1.5297 × 10¹⁰ m | half that, plus the 3-unit height offset |

Row-by-row:

- `#0 Plane  m=0.000e0 kg  [immovable]` — id 0, a `Body::Plane`, zero mass, never
  integrated. Zero mass here means *gravitationally inert*, not "massless": see
  `core/README.md`.
- `#1 Sphere  m=5.972e24 kg  |v|=6.684e1 m/s  v=(6.68e1, 0.00e0, 0.00e0)` — id 1,
  one Earth mass, moving at 66.84 m/s along +X. `|v|` is the magnitude;
  `v` is the component vector, so the two agree when motion is on one axis.
- `#1-#2: 3.0000e10 m` — the distance between the *centres* of props 1 and 2. For a
  `Plane` that is its origin, not the nearest point on its surface.

The ids are `BodyId` values from `Scene::add`, so they are stable for a prop's
lifetime and are not row indices.

## Where the tests are

- `tst/ui/overlay_tests.rs` — 14 tests: `fps_from_dt`, a `DebugUiPlugin: Plugin`
  compile check, and the SI readout (unit conversion, per-prop rows, pairwise
  distances, loud truncation, and that the readout actually tracks the simulation
  rather than printing the starting state forever).
- `tst/ui/crosshair_tests.rs` — 18 tests: 6 proptest properties over
  `crosshair_segments` (stroke length, symmetry, axis alignment, nothing inside
  the gap, finiteness, rigid translation) plus hand-written layout, config, and
  colour cases.

Neither `debug_overlay_system` nor `crosshair_system` is **ever executed by the
suite** — all the crosshair coverage is of its pure geometry helper. The comment
in `overlay_tests.rs` blames the render backend; the real blocker is the missing
`Assets<Shader>` resource, and both systems *are* headlessly testable once that
and the items listed above are supplied.

## Adding a file here

- Keep pure display math (formatting, smoothing, unit conversion) in free
  functions like `fps_from_dt` so it is testable without an egui context.
- Read state, never write it. Anything that mutates simulation state from a UI
  control belongs behind an event that a `render/` system consumes.
