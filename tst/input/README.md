# `tst/input/` — tests for `src/input/`

> **Purpose:** Tests for [`src/input/`](../../src/input/README.md): the camera, the clock
> keys, the `Esc` menu gate, and the `B` picker's keys.
> **Edit here to:** add or fix a test of a key or of camera movement.
> **Not here:** the controller, its known defects, and the key map, in `src/input/`.

Tests for the free-fly camera, the clock keys, and the menu's input mode. See
`src/input/README.md` for the invariants and for the three known defects in the controller
itself.

## What lives here

| File | Job |
| --- | --- |
| `camera_tests.rs` | `FreeFlyController::default()` field pins, `clamp_pitch` spot checks and a property, the movement axes driven through the real system, and one plugin smoke test. |
| `controls_tests.rs` | 18 tests, including a rebound key doing the action and the old key not: every clock key driven through the real system, edge-triggering, the scale clamps, and the no-collision check against the camera's keys. |
| `menu_tests.rs` | 20 tests: the menu always opening on its first page, a rebind taking the next bindable key (`Esc` cancels, an unbindable key is ignored), `Esc` opening and closing the menu, edge-triggering, and **the gate** — that the camera, the clock keys, and `T` all go inert while the menu is open and work again when it closes, that the cursor is released but not re-grabbed, that `MouseMotion` is still drained, and that `Esc` closes an open pop-up before it opens the menu and a click behind the pop-up does not capture the cursor. |
| `placement_tests.rs` | 16 tests: `B` opening the picker, the arrows wrapping, `Enter` arming a request only while open, the menu closing it — and that none of its four keys collide, since unlike the menu the picker is **not** modal. |

## What is covered

- Every `Default` field value, pinned individually so a tuning change is a
  deliberate edit.
- `clamp_pitch` in range, saturating, and on `NaN`, plus a property asserting the
  output is always finite and strictly inside `(-π/2, π/2)` for any `f32`.
- One `MinimalPlugins` tick that does not panic.

## Known gaps — this is the crate's weakest-covered area

The plugin smoke test deliberately spawns **no camera entity and no window**, so
it exercises only the two early-return branches. Requirements 10.2–10.9 — cursor
capture and release, capture-loss suppression, the yaw/pitch math and its
`captured == false` gating, WASD normalization, shift boost, and `dt` scaling —
are **entirely unexecuted**.

None of that is a limitation of the harness. All of it is reachable headlessly by
spawning `(Camera3d::default(), Transform::default(), FreeFlyController::default())`
and `(Window::default(), PrimaryWindow)` under `MinimalPlugins`, then
`init_resource`-ing `ButtonInput<KeyCode>` / `ButtonInput<MouseButton>`, adding
the `MouseMotion` event, and pumping input between `app.update()` calls —
`MinimalPlugins` omits `InputPlugin`, which is the only reason the existing test
does that setup at all.

## The gate's tests need a clock that has ticked

`menu_tests::app_with_input` advances an `ObserverClock` before inserting it. That is
load-bearing, not incidental: nothing advances the observer clock in an app this small —
`advance_clocks_system` lives in `ScenePlugin` — and a clock reading zero makes
`motion_step_seconds()` zero, so the camera holds still whatever the gate does. Without
that line, `the_camera_does_not_move_while_the_menu_is_open` passes for the wrong reason,
and `the_camera_moves_again_once_the_menu_closes` is what catches it.

## Adding a test here

Prefer driving the real system over testing helpers in isolation: the helpers are
already covered and the system is not. Assert on `FreeFlyController` state
(`yaw`, `pitch`, `captured`) and on `Window::cursor` alongside the resulting
`Transform`, so a test failure names which of the four jobs regressed.
