//! Every constant the `input` layer reads: the free-fly camera's feel.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module.
//! Three of these four are the `FreeFlyController::default()` field values, pinned
//! by task 8.1; the fourth is the pitch clamp's margin.
//!
//! These govern *feel*, which is the one thing in this crate that has no correct
//! answer — but each still has a reason, and each has a way to be wrong.

/// Default camera speed, in world units per second.
///
/// **Why this value.** The scene is `100` units across and the floor now spans all of
/// it, so `20` units per second crosses it in five seconds — a pace that gets you from
/// one side of an experiment to the other without the trip becoming the activity.
/// [`CAMERA_BOOST_MULTIPLIER`] triples it for longer hauls, and mouse-look is
/// unaffected, so precise aiming does not get harder as travel gets faster.
///
/// Task 8.1 originally pinned `5.0`. That was chosen against a scene whose floor was
/// `50` units across, and it took 20 seconds to cross the world — slow enough that
/// looking at the far side of a three-body ring was a chore.
///
/// **Used by.** `FreeFlyController::default`, read every frame by
/// [`crate::input::camera::free_fly_camera_system`].
///
/// **If you change it.** Units per second, so it is tied to the scene's scale: at
/// [`crate::constants::METRES_PER_UNIT`] this is `2e10 m/s`, some sixty times *c*,
/// which is a reminder that the camera is not a physical object and is not
/// integrated. Too high and a boosted frame overshoots a `1.5`-unit sphere between
/// frames — at `20` with a `3×` boost, a 60 Hz frame moves one unit, so a body is still
/// several frames wide. Too low and crossing the scene is tedious.
pub const CAMERA_MOVE_SPEED_UNITS_S: f32 = 20.0;

/// Multiplier applied to [`CAMERA_MOVE_SPEED_UNITS_S`] while `Left Shift` is held.
/// Dimensionless.
///
/// **Why this value.** Pinned by task 8.1. `3×` is a large enough jump to be worth
/// reaching for and small enough to stay controllable — the boost is for traversing
/// the scene, not for leaving it.
///
/// **Used by.** `FreeFlyController::default`, applied in
/// [`crate::input::camera::free_fly_camera_system`] on the same frame the modifier is
/// read.
///
/// **If you change it.** Multiplies the concern in
/// [`CAMERA_MOVE_SPEED_UNITS_S`]: at very high multipliers a held `Shift` moves the
/// camera further per frame than the scene is wide. Values below `1.0` turn the
/// modifier into a brake, which is a legitimate design but contradicts the name.
pub const CAMERA_BOOST_MULTIPLIER: f32 = 3.0;

/// Default mouse-look sensitivity, in radians of rotation per pixel of motion.
///
/// **Why this value.** Pinned by task 8.1. `0.002 rad/px` means a 500-pixel drag
/// sweeps one radian, so a full turn takes about three window-widths of travel —
/// close to the range typical first-person applications use.
///
/// **Used by.** `FreeFlyController::default`, multiplied into the accumulated mouse
/// delta by [`crate::input::camera::free_fly_camera_system`] each frame.
///
/// **If you change it.** It scales raw device pixels, so a high-DPI mouse or a
/// display-scaling change alters the effective sensitivity without this value moving.
/// Large values make pitch hit its clamp ([`PITCH_LIMIT_MARGIN_RAD`]) from a small
/// flick; a negative value inverts the look axis, which is a real preference and
/// would work.
pub const LOOK_SENSITIVITY_RAD_PER_PX: f32 = 0.002;

/// How far short of straight up and straight down the camera's pitch stops, in
/// radians.
///
/// The pitch clamp is `(−π/2 + margin, π/2 − margin)`.
///
/// **Why this value.** Requirement 10.4 specifies an **open** interval, so the margin
/// is what keeps the clamped value strictly inside `(−π/2, π/2)`. At exactly `±π/2`
/// the yaw axis and the view direction align, the rotation basis becomes degenerate,
/// and yaw stops having a defined effect — gimbal lock. `1e-3 rad` is about `0.06°`:
/// far too small to see, far too large to be lost in `f32`.
///
/// **Used by.** [`crate::input::camera::clamp_pitch`], on every frame with mouse
/// motion.
///
/// **If you change it.** Zero re-admits the degenerate pose Requirement 10.4 excludes.
/// Values large enough to notice (say above `0.05`) stop the camera visibly short of
/// vertical, which reads as the look control sticking.
pub const PITCH_LIMIT_MARGIN_RAD: f32 = 1.0e-3;

/// Factor the clock scale is multiplied or divided by, per press of the
/// faster/slower keys. Dimensionless.
///
/// **Why this value.** Doubling is the coarsest step that still feels like a
/// control rather than a jump, and the shipped scale of `5e6` is reachable from
/// [`MIN_TIME_SCALE`](crate::time::constants::MIN_TIME_SCALE) in 23 presses — enough
/// resolution to find a speed, few enough presses to cross the range. A power of two
/// also keeps the scale exactly representable in `f32` for as long as the starting
/// value is, so repeated presses cannot accumulate rounding drift.
///
/// **Used by.** [`crate::input::controls::simulation_controls_system`], on
/// `just_pressed` of `]` (multiply) and `[` (divide).
///
/// **If you change it.** The clock clamps the result into
/// `[MIN_TIME_SCALE, MAX_TIME_SCALE]`, so no value here can produce an unusable
/// scale. Below `1.0` it would invert the two keys, which is why the system divides
/// rather than accepting a reciprocal constant; at `1.0` both keys become no-ops.
pub const TIME_SCALE_KEY_FACTOR: f32 = 2.0;

/// Multiplier applied to the camera's speed while a **precision modifier** is held.
/// Dimensionless.
///
/// **Why this value.** The mirror image of [`CAMERA_BOOST_MULTIPLIER`], and it has to be far more
/// aggressive than that constant is, because the two problems are not symmetrical. Boost exists to
/// cross a `4495`-unit solar system; precision exists to approach a body `0.0064` units across, and
/// since amendment A17 those are the real numbers.
///
/// At `0.02` — a fiftieth — the base `20` units/s becomes `0.4`, which crosses the Earth in `16 ms`
/// and takes `3.5` seconds to pass the Sun. That is the slowest speed still fast enough to be
/// useful, and it is `1500×` slower than boosted flight, which is roughly the dynamic range the
/// scenes need.
///
/// **Used by.** [`crate::input::camera::free_fly_camera_system`], multiplied into the speed while `C`
/// is held. **Multiplicative with boost**, so holding both gives
/// `0.02 × 3 = 0.06` — the simplest rule, and the only one that does not need a precedence order
/// nobody can see.
///
/// **If you change it.** `1.0` disables the modifier without removing the binding. Below about
/// `0.001` the camera stops responding in a way that reads as an input bug rather than as fine
/// control. Note this is still not slow enough to inspect a planet comfortably at true scale — a
/// per-experiment camera speed is the real fix and is recorded as a known limitation of A15.
pub const CAMERA_PRECISION_MULTIPLIER: f32 = 0.02;

/// Factor the `-` and `+` keys divide and multiply the simulation's clock scale by. Dimensionless.
///
/// **Why this value.** `[` and `]` halve and double, which is the right granularity for finding a
/// pace and a slow way to change regime: getting from the solar system's `1e6` down to something
/// that resolves a close encounter is more than nine presses. `10` is one press per order of
/// magnitude, which is how the scales in this crate actually differ — `1e4` for the Jovian system,
/// `1e6` for the solar system, `2e7` for the three-body ring.
///
/// **One factor, both directions.** `-` divides by it and `+` multiplies by it, so the two are exact
/// inverses and `-` then `+` returns to the scale you started from. A separate constant for the
/// up direction would allow that to stop being true, which is the only thing making a decade key
/// worth having: it is how you go and look at something and then come back.
///
/// **Used by.** [`crate::input::controls::simulation_controls_system`], on `-` and `+`.
///
/// **If you change it.** The clock clamps at both ends —
/// [`MIN_TIME_SCALE`](crate::time::constants::MIN_TIME_SCALE) and
/// [`MAX_TIME_SCALE`](crate::time::constants::MAX_TIME_SCALE) — so no value here can stop time or run
/// it unboundedly fast; it only changes how few presses it takes to reach either limit. Note the
/// clamps make the round trip lossy **at** a limit: `+` at the ceiling does nothing, so the matching
/// `-` lands a decade below where you were. `\` is the exact way back.
pub const TIME_SCALE_DECADE_FACTOR: f32 = 10.0;

/// How far off the crosshair a body may be and still be picked by the lock key, in radians.
///
/// **Why this value.** `0.26` rad is about `15°`, a third of Bevy's default `45°` vertical field of
/// view. Bodies in the solar-system scenes are a few pixels wide, so demanding the crosshair sit on
/// one would make the lock unusable. Anything wider and pressing `F` while looking at empty space
/// grabs a body you were not looking at.
///
/// **Used by.** [`crate::input::camera::pick_lock_target`].
///
/// **If you change it.** Above `π` every body qualifies and the key always locks onto the nearest
/// in angle, wherever it is. At `0` nothing can be picked.
pub const LOCK_PICK_MAX_ANGLE_RAD: f32 = 0.26;

/// Factor the orbit distance is multiplied by per line of scroll toward the body. Dimensionless.
///
/// **Why this value.** Zoom is multiplicative so one notch feels the same around a `0.0064`-unit
/// Earth as around a `1.5`-unit sphere. `0.85` closes `15%` per notch, which crosses an order of
/// magnitude in about fourteen notches: quick enough to go from a planet's orbit to its surface,
/// slow enough to stop where you meant to.
///
/// **Used by.** [`crate::input::camera::zoom_distance`]. Scrolling away divides by it, so in then
/// out returns exactly to where you were.
///
/// **If you change it.** Must stay in `(0, 1)`. At `1` the wheel does nothing; above it the wheel
/// is reversed.
pub const LOCK_ZOOM_STEP_PER_LINE: f32 = 0.85;

/// Pixels of scroll that count as one line, for trackpads and other pixel-unit wheels.
///
/// **Why this value.** Platforms report a trackpad swipe in pixels and a mouse notch in lines. At
/// `50`, a short two-finger swipe moves about as far as one notch of a wheel.
///
/// **Used by.** [`crate::input::camera::scroll_lines`].
///
/// **If you change it.** Lower makes trackpad zoom more sensitive. A mouse wheel is unaffected.
pub const LOCK_ZOOM_PIXELS_PER_LINE: f32 = 50.0;

/// Closest the orbiting camera may come to a body, in multiples of its drawn radius, before the
/// near plane is added on.
///
/// **Why this value.** At `1.5` radii the body fills most of a `45°` view without the camera ever
/// being inside it. The near plane is added on top in [`crate::input::camera::orbit_distance_bounds`],
/// because a body smaller than the near plane would otherwise be clipped away just as you reach it.
///
/// **If you change it.** Must exceed `1`. At `1` or below the camera can enter the body.
pub const LOCK_MIN_DISTANCE_RADII: f32 = 1.5;
