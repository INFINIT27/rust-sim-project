# `tst/ui/` — tests for `src/ui/`

> **Purpose:** Tests for [`src/ui/`](../../src/ui/README.md): the overlay's SI readout, the
> crosshair and compass geometry, and the content of the menu, picker, and arrival pop-up.
> **Edit here to:** add or fix a test of an on-screen panel.
> **Not here:** the panels and how to read them, in `src/ui/`.

Tests for the egui overlay, the crosshair, the compass, and the `Esc` menu. See
`src/ui/README.md` for the overlay's invariants and its undeclared dependency on
`ScenePlugin`.

## What lives here

| File | Job |
| --- | --- |
| `overlay_tests.rs` | 14 tests: three `fps_from_dt` branch tests, a `DebugUiPlugin: Plugin` compile check, and 10 covering the SI readout. |
| `crosshair_tests.rs` | 18 tests: 6 proptest properties over `crosshair_segments` plus layout, config, and colour cases. |
| `hud_tests.rs` | 3 tests: the HUD starting shown; the run condition following the visibility (and showing with no resource); `H` hiding it, not flickering while held, and showing it again. |
| `compass_tests.rs` | The pure `compass_axes` projection: screen mapping, the depth cue's sign, and the yaw/pitch derivations spelled out in comments. |
| `menu_tests.rs` | 13 tests over the panel's pure content: every settings row present, labelled, and tracking the live resource it reads; the menu listing `Esc`; save requests taken once with their outcome reported; an unchanged edit not marking its resource changed; every anti-aliasing mode labelled; and one crude width check on `MENU_WIDTH_PX`. |
| `placement_tests.rs` | 9 tests over `placement_rows`: every object gets a row, exactly one is marked, the masses shown are the catalog's, and the longest row fits the panel. |
| `arrival_tests.rs` | 6 tests over the arrival pop-up's pure content: duration, energy and mass formatting, a crewed landing's rows, no engine rows for a coasting probe, and when the pop-up opens and closes (once per landing). |
| `proptest-regressions/` | Committed proptest failure seeds. Tool-owned; no `README.md` required. |

## Nothing here calls a system that paints

`EguiContexts::ctx_mut()` needs the primary window to already have an egui context, which
needs a real window. So every module in this layer keeps its *content* in pure functions and
the tests live there: `fps_from_dt`, `prop_lines`, `crosshair_segments`, `compass_axes`,
`settings_rows`, `binding_rows`. What that leaves uncovered is layout — whether two grids
line up, whether a panel fits its width — and `cargo run --release` is the only thing that
answers those.

## What is covered

**Overlay.** The pure `fps_from_dt` helper — normal frame time, zero and
sub-threshold `dt`, and non-finite `dt` — plus a compile-level check that
`DebugUiPlugin` satisfies `impl Plugin`, which is all `main.rs`'s `add_plugins`
call needs.

**The SI readout.** `units_to_metres`, `prop_lines`, and `distance_lines` are pure, so
they are fully covered with no egui context. What the tests actually guard is the
*unit conversion*: a forgotten multiply would print a 30-unit separation as `30 m`
instead of `3e10 m`, which looks plausible and would be easy to miss by eye. Hence
explicit pins — `units_to_metres(TWO_BODY_SEPARATION_UNITS) == 3.0e10`, a prop moving
at 1 unit/s reading `1e9 m/s`, and the sphere pair reading exactly
`#1-#2: 3.0000e10 m`. Also covered: each unordered pair appears once and never in
reverse, a lone prop has no pairs, both lists truncate *loudly* with a stated count,
and `the_readout_follows_the_simulation` steps the scene and asserts the numbers
changed — without it every other test would still pass on a readout frozen at the
starting state.

**Crosshair.** All of the crosshair's real logic is in `crosshair_segments`, which
takes and returns plain `f32` pairs, so it is fully covered with no egui context.
Properties: every stroke is exactly `arm_length_px` long (independent of the gap);
the shape is symmetric about both axes; the strokes are axis-aligned; nothing
intrudes into the gap, so the pixel being pointed at stays visible; every
coordinate is finite; and moving the centre translates the shape rigidly. Unit
tests pin the documented layout exactly, the zero-gap and zero-arm degenerate
cases, a gap larger than the arms staying well-formed, the left/right/up/down
return order, the default appearance field by field, and that `to_egui_color` uses
straight rather than premultiplied alpha.

### A note on tolerances in the crosshair properties

These are `f32` screen coordinates, and the properties measure offsets by
round-tripping through an absolute position — computing `centre − offset` and then
subtracting `centre` again. At a coordinate of 9000 px, `f32` spacing is already
~1e-3, so the recovered offset can differ from the original by that much even
though the implementation is symmetric by construction. Two properties failed on
first run for exactly this reason. The fix was `magnitude_epsilon`, which scales
the tolerance with the coordinate; do not replace it with a fixed epsilon, which is
either vacuous near the origin or impossible at the edge of a large window.

## Known gap, and the correction to the reason recorded in the file

`debug_overlay_system` is **never executed by any test**. The header comment in
`overlay_tests.rs` attributes this to `EguiPlugin` needing a render backend. That
diagnosis is wrong: `ExtractResourcePlugin`, `ExtractComponentPlugin`, and
`EguiPlugin::finish` all no-op cleanly without a `RenderApp`. The actual panic is
`load_internal_asset!` in `EguiPlugin::build` requiring an `Assets<Shader>`
resource that `MinimalPlugins` does not provide.

The overlay *is* headlessly testable. The full prerequisite list, verified by
running it:

- `init_resource::<Assets<Shader>>()` and `init_resource::<Assets<Image>>()`
- `add_event` for `CursorMoved`, `MouseButtonInput`, `MouseWheel`,
  `KeyboardInput`, `TouchInput` (read by `process_input_system`)
- `add_event::<RequestRedraw>()` and `add_event::<AssetEvent<Image>>()`
- a spawned `(Window, PrimaryWindow)` entity, so `setup_new_windows_system`
  attaches the `EguiContext` that `ctx_mut()` looks for

With those in place the observable assertions are: `EguiRenderOutput.paint_jobs`
is non-empty after a tick (Requirement 11.2 — exactly one window drew),
`Scene::bodies` and the camera `Transform` are unchanged (11.3), and
`wants_keyboard_input()` / `wants_pointer_input()` are both `false` (11.4 — the
non-consumption property the source currently asserts only in prose).

## Adding a test here

If you add the setup above, correct the header comment in the same change rather
than leaving a superseded explanation in place.
