//! Free-fly camera module.
//!
//! Section 8 of the original spec's task list fills this module
//! in end-to-end: the `FreeFlyController` component (task 8.1), the
//! pure pitch-clamp helper [`clamp_pitch`] (task 8.2) plus its property
//! test (task 8.3), the combined cursor-capture / mouse-look / WASD
//! translation / capture-loss system [`free_fly_camera_system`]
//! (tasks 8.4–8.7), and finally the plugin wiring that registers that
//! system on `Update` (task 8.8).
//!
//! The entity that carries `(Camera3dBundle, FreeFlyController)` is
//! spawned by [`spawn_scene_entities`](crate::render::plugin) on
//! `Startup`, matching the design's startup sequence diagram.
//!
//! # Module layout
//!
//! - [`FreeFlyController`] — per-entity camera state (speeds, yaw,
//!   pitch, capture flag).
//! - [`clamp_pitch`] — pure `f32 → f32` helper that enforces
//!   Requirement 10.4's pitch bounds and gives Section 8's mouse-look
//!   system something property-testable to lean on.
//! - [`free_fly_camera_system`] — the single Bevy system that reads
//!   keyboard/mouse input, updates the primary window's cursor state,
//!   and rewrites the camera's `Transform` per frame.
//! - [`FreeFlyCameraPlugin`] — registers that system on `Update`.

use bevy::app::{App, Plugin, Update};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::component::Component;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, Resource};
use bevy::input::mouse::MouseMotion;
use bevy::input::{keyboard::KeyCode, mouse::MouseButton, ButtonInput};
use bevy::math::{EulerRot, Quat, Vec3};

use crate::time::observer::ObserverClock;

use crate::input::constants::{
    CAMERA_BOOST_MULTIPLIER, CAMERA_MOVE_SPEED_UNITS_S, CAMERA_PRECISION_MULTIPLIER,
    LOCK_MIN_DISTANCE_RADII, LOCK_PICK_MAX_ANGLE_RAD, LOCK_ZOOM_PIXELS_PER_LINE,
    LOCK_ZOOM_STEP_PER_LINE, LOOK_SENSITIVITY_RAD_PER_PX, PITCH_LIMIT_MARGIN_RAD,
};
use crate::input::menu::MenuState;
use crate::input::popup::PopupState;
use bevy::app::PostUpdate;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::Without;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::render::camera::Projection;
use bevy::transform::components::Transform;
use bevy::transform::TransformSystem;
use bevy::window::{CursorGrabMode, PrimaryWindow, Window};

/// Which of the three flight speeds the camera is in.
///
/// **Why a mode rather than a held key.** Holding a modifier for the whole of a long flight is
/// tiring, and the two things you most want to hold it for — crossing `4495` units to Neptune, and
/// creeping up on a `0.0064`-unit Earth — are exactly the flights that take longest. Amendment A26
/// replaced both hold modifiers with a cycle on `C`.
///
/// **The cost, stated plainly.** A held key tells you it is held; a mode does not. That is why
/// [`crate::ui::overlay::camera_speed_line`] puts the current mode on screen — with a toggle, the
/// readout is not a convenience, it is the only thing standing between the user and a camera that
/// has silently been `150×` too slow since some earlier keypress.
///
/// # The cycle
///
/// `Normal → Fast → Slow → Normal`, ascending and wrapping, which is the order
/// [`next`](Self::next) implements. From the default `Normal` that is one press to `Fast` and two to
/// `Slow`.
///
/// The wrap from `Fast` to `Slow` is a `150×` discontinuity, and that is inherent to cycling three
/// states with one key rather than a flaw in the ordering: some adjacent pair has to be the extremes.
/// Putting it between the two *deliberate* modes rather than next to `Normal` at least means an
/// accidental extra press lands somewhere obvious.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CameraSpeed {
    /// [`CAMERA_PRECISION_MULTIPLIER`] (`0.02×`) — for approaching a body at true scale.
    Slow,
    /// `1×`, the shipped [`CAMERA_MOVE_SPEED_UNITS_S`]. The default, so a fresh run behaves as it
    /// always did.
    #[default]
    Normal,
    /// [`CAMERA_BOOST_MULTIPLIER`] (`3×`) — for crossing a solar system.
    Fast,
}

impl CameraSpeed {
    /// The next mode in the cycle: `Normal → Fast → Slow → Normal`.
    ///
    /// Total and wrapping, so no sequence of presses can leave the camera in a state that is not one
    /// of the three.
    #[inline]
    pub fn next(self) -> Self {
        match self {
            Self::Normal => Self::Fast,
            Self::Fast => Self::Slow,
            Self::Slow => Self::Normal,
        }
    }

    /// The factor this mode multiplies [`FreeFlyController::move_speed`] by.
    ///
    /// Read from the controller's own fields rather than from the constants directly, so a controller
    /// built with custom multipliers is honoured — which is what the tests rely on.
    #[inline]
    pub fn multiplier(self, controller: &FreeFlyController) -> f32 {
        match self {
            Self::Slow => controller.precision_mult,
            Self::Normal => 1.0,
            Self::Fast => controller.boost_mult,
        }
    }

    /// A short lower-case label for the overlay: `slow`, `normal`, or `fast`.
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Normal => "normal",
            Self::Fast => "fast",
        }
    }
}

/// ECS component that holds the per-entity state driving the free-fly
/// camera controller.
///
/// Field semantics (from task 8.1 and Requirement 10):
///
/// - `move_speed`: translation speed in world units per second used by
///   the WASD/Space/Ctrl step (Requirement 10.5). Defaults to `5.0`.
/// - `boost_mult`: multiplier applied to `move_speed` while
///   the camera is in [`CameraSpeed::Fast`] (Requirement 10.5, amendment A26). Defaults to `3.0`.
/// - `look_sensitivity`: radians per pixel applied to raw mouse-motion
///   deltas when computing yaw/pitch updates (Requirement 10.2).
///   Defaults to `0.002`.
/// - `yaw`: current yaw in radians, rotating around `+Y`. Updated by
///   the mouse-look system (Requirement 10.2). Defaults to `0.0`.
/// - `pitch`: current pitch in radians, clamped into the open
///   interval `(-π/2 + 1e-3, π/2 - 1e-3)` (Requirement 10.4).
///   Defaults to `0.0`.
/// - `speed`: which of the three [`CameraSpeed`] modes is active, cycled by `C`
///   (amendment A26). Defaults to [`CameraSpeed::Normal`].
/// - `captured`: whether the cursor is currently captured for
///   mouse-look. Updated by the capture/release logic
///   (Requirements 10.7, 10.8). Defaults to `false`.
#[derive(Component, Debug, Copy, Clone, PartialEq)]
pub struct FreeFlyController {
    /// Translation speed in world units per second.
    pub move_speed: f32,
    /// Multiplier applied to `move_speed` in [`CameraSpeed::Fast`].
    pub boost_mult: f32,
    /// Multiplier applied to `move_speed` in [`CameraSpeed::Slow`].
    ///
    /// Far more aggressive than [`boost_mult`](Self::boost_mult), because the problems are not
    /// symmetrical: fast crosses a `4495`-unit solar system, slow approaches a body `0.0064` units
    /// across. Since A26 the two are **mutually exclusive modes** rather than modifiers that
    /// multiplied together, so the old combined `0.06×` is no longer reachable.
    pub precision_mult: f32,
    /// Which of the three speeds is currently active.
    ///
    /// Cycled by `C`. Defaults to [`CameraSpeed::Normal`], so a fresh run flies at the shipped
    /// speed.
    pub speed: CameraSpeed,
    /// Radians per pixel applied to raw mouse-motion deltas.
    pub look_sensitivity: f32,
    /// Current yaw in radians, around `+Y`.
    pub yaw: f32,
    /// Current pitch in radians, clamped to avoid gimbal flipping.
    pub pitch: f32,
    /// Whether the cursor is currently captured for mouse-look.
    pub captured: bool,
}

impl Default for FreeFlyController {
    /// Defaults pinned by task 8.1: `move_speed = 5.0`,
    /// `boost_mult = 3.0`, `look_sensitivity = 0.002`, `yaw = 0.0`,
    /// `pitch = 0.0`, `captured = false`.
    #[inline]
    fn default() -> Self {
        Self {
            move_speed: CAMERA_MOVE_SPEED_UNITS_S,
            boost_mult: CAMERA_BOOST_MULTIPLIER,
            precision_mult: CAMERA_PRECISION_MULTIPLIER,
            speed: CameraSpeed::Normal,
            look_sensitivity: LOOK_SENSITIVITY_RAD_PER_PX,
            yaw: 0.0,
            pitch: 0.0,
            captured: false,
        }
    }
}

/// The tuning every new camera's [`FreeFlyController`] starts with: speeds and look
/// sensitivity, but none of its per-frame state (yaw, pitch, speed mode, capture).
///
/// A resource so that it can come from `config/camera.toml` (see `src/config/`) rather than only
/// from [`crate::input::constants`]. `spawn_scene_entities` builds the camera's controller from
/// it with [`controller`](Self::controller), and falls back to the defaults when it is absent.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{CameraSettings, FreeFlyController};
///
/// let settings = CameraSettings { move_speed: 50.0, ..CameraSettings::default() };
/// let controller = settings.controller();
/// assert_eq!(controller.move_speed, 50.0);
/// assert_eq!(controller.boost_mult, FreeFlyController::default().boost_mult);
/// ```
#[derive(Resource, Debug, Copy, Clone, PartialEq)]
pub struct CameraSettings {
    /// Translation speed in world units per second.
    pub move_speed: f32,
    /// What `Shift` multiplies `move_speed` by.
    pub boost_mult: f32,
    /// What the precision speed mode multiplies `move_speed` by.
    pub precision_mult: f32,
    /// Mouse-look sensitivity, in radians per pixel.
    pub look_sensitivity: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        let controller = FreeFlyController::default();
        Self {
            move_speed: controller.move_speed,
            boost_mult: controller.boost_mult,
            precision_mult: controller.precision_mult,
            look_sensitivity: controller.look_sensitivity,
        }
    }
}

impl CameraSettings {
    /// A fresh controller with this tuning and the default per-frame state.
    pub fn controller(&self) -> FreeFlyController {
        FreeFlyController {
            move_speed: self.move_speed,
            boost_mult: self.boost_mult,
            precision_mult: self.precision_mult,
            look_sensitivity: self.look_sensitivity,
            ..FreeFlyController::default()
        }
    }
}

/// Clamp `pitch` into the open interval `(-π/2 + 1e-3, π/2 - 1e-3)`.
///
/// Contract (Requirement 10.4 / task 8.2):
///
/// - Inputs already in `[-π/2 + 1e-3, π/2 - 1e-3]` are returned
///   unchanged.
/// - Inputs below `-π/2 + 1e-3` saturate at `-π/2 + 1e-3`.
/// - Inputs above `π/2 - 1e-3` saturate at `π/2 - 1e-3`.
/// - `NaN` collapses to `0.0` so downstream code never sees a `NaN`
///   pitch (the mouse-look system feeds this into
///   `Quat::from_euler`, which would otherwise poison the camera
///   rotation for the rest of the run).
///
/// The `1e-3` margin is what keeps the post-clamp value *strictly*
/// inside `(-π/2, π/2)`, matching Requirement 10.4's "open interval"
/// wording. `f32::clamp` already defines `clamp(lower, upper)` when
/// `lower <= upper` and both are finite, both of which hold here by
/// construction.
///
/// The `NaN` branch runs first so that `f32::clamp`'s `NaN`-propagation
/// behaviour never has a chance to leak out; this is the "defined rule
/// that the test encodes" called out in task 8.3.
#[inline]
pub fn clamp_pitch(pitch: f32) -> f32 {
    if pitch.is_nan() {
        return 0.0;
    }
    let upper = std::f32::consts::FRAC_PI_2 - PITCH_LIMIT_MARGIN_RAD;
    let lower = -std::f32::consts::FRAC_PI_2 + PITCH_LIMIT_MARGIN_RAD;
    pitch.clamp(lower, upper)
}

/// Bevy system that drives the free-fly camera each frame.
///
/// Combines the four pieces called out by tasks 8.4–8.7 into a single
/// `Update`-schedule system (matching the design's pseudocode):
///
/// 0. **The menu gate** (amendment A13): while
///    [`MenuState::is_open`](crate::input::menu::MenuState::is_open) or
///    [`PopupState::is_open`](crate::input::popup::PopupState::is_open),
///    the cursor is released and the system returns without capturing,
///    looking, or moving — after draining `MouseMotion`, which is what
///    stops buffered deltas snapping the view when the menu closes.
/// 1. **Cursor capture** (task 8.4, Requirements 10.7 / 10.8):
///    left-click captures the cursor (grab mode locked, cursor hidden,
///    `controller.captured = true`). *Release* is now a consequence of
///    the menu opening rather than a branch here: `Esc` opens the menu
///    and step 0 drops the grab.
/// 2. **Capture-loss handling** (task 8.7, Requirement 10.9): if the
///    OS has already dropped the window's grab mode to `None` while
///    `controller.captured` is still `true`, treat the frame as if no
///    mouse motion occurred — skip the yaw/pitch update entirely so
///    `yaw` and `pitch` remain unchanged until capture is re-acquired.
/// 3. **Mouse-look** (task 8.5, Requirements 10.2–10.4): while
///    `controller.captured`, accumulate this frame's `MouseMotion`
///    deltas into `(dx, dy)`, update
///    `yaw -= dx * look_sensitivity` and
///    `pitch -= dy * look_sensitivity`, clamp pitch via
///    [`clamp_pitch`], and rewrite `transform.rotation` to
///    `Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)`.
/// 4. **WASD + Space + Ctrl translation at the current speed mode** (task 8.6,
///    Requirements 10.5–10.6, amendment A26): build a local-axis delta from held
///    keys (W/S along forward, D/A along right, Space/ControlLeft
///    along world-up `+Y`), multiply by
///    `move_speed * controller.speed.multiplier(controller) * dt`,
///    where the direction is the sum of the held keys' axes: `W`/`S` and
///    `A`/`D` in the camera's local frame, and `Space`/`ControlLeft` plus
///    their `E`/`Q` aliases along world `±Y` (amendments A6, A8)
///    after normalization, and add to `transform.translation`. If no
///    movement key is held, `delta == Vec3::ZERO` and the translation
///    step leaves `transform.translation` untouched.
///
/// # Note on the `MouseMotion` reader
///
/// `EventReader<MouseMotion>` is drained every frame regardless of
/// `controller.captured` so that leftover deltas from frames where
/// capture was released (or lost) don't suddenly snap the camera when
/// the user re-captures. The *consumption* is unconditional; the
/// *application* to yaw/pitch is gated by the capture check.
pub fn free_fly_camera_system(
    clock: Res<ObserverClock>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    // The two panels that stand the camera down; one parameter so the system stays within
    // clippy's argument limit.
    (menu, popup): (Option<Res<MenuState>>, Option<Res<PopupState>>),
    mut mouse_motion: EventReader<MouseMotion>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut camera: Query<
        (&mut FreeFlyController, &mut Transform, Option<&CameraLock>),
        With<Camera3d>,
    >,
) {
    // Resolve the unique camera entity. `get_single_mut` is tolerant of
    // the "no camera yet" case (e.g. during the first frame before
    // `spawn_scene_entities` has run under some plugin orderings) —
    // bail out silently rather than panic.
    let Ok((mut controller, mut transform, lock)) = camera.get_single_mut() else {
        // Still drain mouse motion so stale deltas don't pile up.
        mouse_motion.clear();
        return;
    };

    // Resolve the primary window. Headless tests that add this plugin
    // without a `WindowPlugin` won't have one; in that case we skip the
    // cursor-state updates and the capture-loss check but still process
    // input and movement so the system is usable in a pure ECS test.
    let mut primary_window = windows.get_single_mut().ok();

    // ------------------------------------------------------------------
    // 0. The menu gate (amendment A13).
    //
    // While the menu is open the camera accepts nothing: no capture, no look, no
    // movement. The cursor is released here rather than in `input/menu.rs` because this
    // system owns the window's cursor state and two writers of one field is how that
    // field ends up depending on system order. Written unconditionally rather than only
    // on the opening frame, so the state is correct even if the menu was opened by
    // something other than a keypress.
    //
    // `mouse_motion` is drained before returning, and that is not optional: buffered
    // deltas would snap the view the instant the menu closed. It is the same reason the
    // no-camera path above drains, and it has its own test.
    //
    // `Option<Res<..>>` rather than `Res<..>`, so a bare `add_systems(Update,
    // free_fly_camera_system)` still runs: absent state means no menu, which means closed.
    // The same reasoning as `apply_experiment_trail_length`'s optional spec — most tests in
    // this crate build the smallest app that exercises one system.
    //
    // An open pop-up (`input/popup.rs`) closes the gate too, for the same reason: it has a close
    // button to point at, and a click on that button must not capture the cursor.
    // ------------------------------------------------------------------
    let popup_open = popup.as_ref().is_some_and(|popup| popup.is_open());
    if popup_open || menu.as_ref().is_some_and(|menu| menu.is_open()) {
        controller.captured = false;
        if let Some(window) = primary_window.as_mut() {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
        }
        mouse_motion.clear();
        return;
    }

    // ------------------------------------------------------------------
    // 1. Cursor capture (task 8.4, Requirements 10.7, 10.8).
    //
    // Release is no longer a branch here: `Esc` opens the menu (see `input/menu.rs`),
    // and the gate above releases the cursor because the menu is open. The key does what
    // it always did; it just shows you something as well.
    // ------------------------------------------------------------------
    if mouse_buttons.just_pressed(MouseButton::Left) {
        controller.captured = true;
        if let Some(window) = primary_window.as_mut() {
            window.cursor.grab_mode = CursorGrabMode::Locked;
            window.cursor.visible = false;
        }
    }

    // ------------------------------------------------------------------
    // 2. Capture-loss handling (task 8.7, Requirement 10.9).
    //
    // If the OS has already released the grab while we still think the
    // cursor is captured, treat this frame as "no mouse motion": skip
    // the yaw/pitch update so the camera doesn't spin from a single
    // accumulated delta when focus returns. We don't flip
    // `controller.captured` — the user can re-capture with a left
    // click, which is exactly the recovery path in the design's
    // "Scenario 5: Cursor capture lost".
    // ------------------------------------------------------------------
    let os_released_capture = primary_window
        .as_ref()
        .map(|w| w.cursor.grab_mode == CursorGrabMode::None)
        .unwrap_or(false);
    let apply_mouse_look = controller.captured && !os_released_capture;

    // ------------------------------------------------------------------
    // 3. Mouse-look (task 8.5, Requirements 10.2, 10.3, 10.4).
    //
    // Always drain `mouse_motion` — see module-level note — but only
    // fold the accumulated delta into yaw/pitch when `apply_mouse_look`
    // is true. When it isn't, we leave `yaw`, `pitch`, and
    // `transform.rotation` untouched this frame (Requirements 10.3 and
    // 10.9).
    // ------------------------------------------------------------------
    let (dx, dy) = {
        let mut dx = 0.0_f32;
        let mut dy = 0.0_f32;
        for ev in mouse_motion.read() {
            dx += ev.delta.x;
            dy += ev.delta.y;
        }
        (dx, dy)
    };

    if apply_mouse_look {
        controller.yaw -= dx * controller.look_sensitivity;
        controller.pitch -= dy * controller.look_sensitivity;
        controller.pitch = clamp_pitch(controller.pitch);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, controller.yaw, controller.pitch, 0.0);
    }

    // ------------------------------------------------------------------
    // 4. WASD + Space/Ctrl + Q/E translation with shift boost (task 8.6,
    //    Requirements 10.5, 10.6, amendments A6 and A8).
    //
    // Two families of axis, deliberately:
    //
    // - **Camera-local** — `W`/`S` along `forward`, `A`/`D` along `right`.
    //   `Transform::forward()` and `Transform::right()` return `Dir3` in
    //   Bevy 0.14, which coerces to `Vec3` via `Dir3::as_vec3`.
    // - **World** — `Space`/`ControlLeft` and `E`/`Q`, both along `±Y`.
    //   These deliberately ignore where the camera is looking: pitching
    //   the view must not tilt vertical flight.
    //
    // `E`/`Q` are **aliases** for `Space`/`ControlLeft`, not a third axis
    // (amendment A8). Two bindings for one motion because the muscle
    // memory differs by tool — editors tend to use `Q`/`E`, shooters
    // `Space`/`Ctrl` — and there is no reason to make someone relearn.
    // Note the consequence: **no key moves along world Z.** Sliding along
    // Z means turning to face it, which changes what you are looking at
    // as well as where you are.
    // ------------------------------------------------------------------
    // While locked, `orbit_camera_system` owns the camera's position, so flying would only be
    // overwritten later in the frame.
    if lock.is_some_and(CameraLock::is_locked) {
        return;
    }

    let forward: Vec3 = transform.forward().as_vec3();
    let right: Vec3 = transform.right().as_vec3();

    let mut delta = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        delta += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        delta -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        delta += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        delta -= right;
    }
    if keys.pressed(KeyCode::Space) {
        delta += Vec3::Y;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        delta -= Vec3::Y;
    }
    // World `±Y`, the same axis as `Space`/`ControlLeft` above. Summed into the
    // same `delta`, so holding `E` and `Space` together is one unit of rise rather
    // than two — `delta.normalize()` sees a single direction, not a doubled one.
    if keys.pressed(KeyCode::KeyE) {
        delta += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyQ) {
        delta -= Vec3::Y;
    }

    if delta != Vec3::ZERO {
        // One mode, one factor. `C` cycles it (amendment A26); nothing here reads a held key, which
        // is the whole point — holding a modifier for the length of a flight to Neptune is exactly
        // the case the modifiers were for and exactly the case they were worst at.
        //
        // Mutually exclusive rather than multiplied: the old scheme let `Shift` and `C` stack to
        // `0.06×`, a speed nobody asked for and nobody could name. Three speeds you can say out loud
        // beats nine you cannot.
        let speed = controller.move_speed * controller.speed.multiplier(&controller);
        // `motion_step_seconds`, not the raw delta: a stalled frame reporting half a
        // second would move the camera two and a half units in one tick, which reads as
        // a teleport. This is the observer's clock, so it is unaffected by the
        // simulation being paused or rescaled — you can fly around a frozen experiment.
        transform.translation += delta.normalize() * speed * clock.motion_step_seconds();
    }
}

/// Bevy plugin that wires the free-fly camera into an `App`.
///
/// Task 8.8 fills this in: register [`free_fly_camera_system`] on the
/// `Update` schedule. The camera entity itself — a `Camera3dBundle`
/// carrying a `FreeFlyController` — is spawned by
/// [`spawn_scene_entities`](crate::render::plugin::spawn_scene_entities)
/// on `Startup`, matching the design's startup sequence.
/// Cycle [`FreeFlyController::speed`] on each press of `C`.
///
/// # Why this is its own system
///
/// `free_fly_camera_system` reads held keys and runs every frame; this reads an **edge** and must
/// fire exactly once per press. Mixing the two would mean one system that is both level- and
/// edge-triggered, which is the shape that produces "the mode advanced twice" bugs.
///
/// # The menu gate
///
/// Returns early while the `Esc` menu is open, the same rule the rest of the layer follows
/// (amendment A13): a menu in which `C` still changed the camera's speed would change the run you
/// opened it to inspect. `Option<Res<..>>` so a test app without the menu resource still runs it.
pub fn cycle_camera_speed_system(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<MenuState>>,
    mut controllers: Query<&mut FreeFlyController>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        return;
    }

    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }

    for mut controller in &mut controllers {
        controller.speed = controller.speed.next();
    }
}

/// Marks an entity the camera may lock onto.
///
/// The render layer attaches it to every sphere and cuboid it draws, so this layer can pick a target
/// without reading `Scene`. The floor plane does not get one: orbiting a `100`-unit slab is not
/// inspecting anything.
#[derive(Component, Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct CameraTarget;

/// Bodies the camera may lock onto. `Without<Camera3d>` lets the camera's own `Transform` be
/// borrowed mutably in the same system.
type TargetFilter = (With<CameraTarget>, Without<Camera3d>);

/// What the camera is orbiting while it is locked.
///
/// Only the distance is stored. The direction is [`FreeFlyController::yaw`] and
/// [`FreeFlyController::pitch`], so mouse-look orbits with no extra input handling, and unlocking
/// continues free flight from the same view.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Orbit {
    /// The [`CameraTarget`] entity being orbited.
    pub target: Entity,
    /// Distance from the target's centre to the camera, in world units.
    pub distance: f32,
}

/// The camera's lock state. `None` is free flight.
#[derive(Component, Debug, Default, Copy, Clone, PartialEq)]
pub struct CameraLock(pub Option<Orbit>);

impl CameraLock {
    /// Whether the camera is orbiting a target.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.0.is_some()
    }
}

/// The candidate closest in angle to the view direction, if it is within
/// [`LOCK_PICK_MAX_ANGLE_RAD`] of it.
///
/// `forward` must be normalised. A candidate sitting exactly at `eye` has no direction and is
/// skipped.
pub fn pick_lock_target(
    eye: Vec3,
    forward: Vec3,
    candidates: impl IntoIterator<Item = (Entity, Vec3)>,
) -> Option<Entity> {
    candidates
        .into_iter()
        .filter_map(|(entity, centre)| {
            let to_centre = (centre - eye).try_normalize()?;
            let angle = forward.dot(to_centre).clamp(-1.0, 1.0).acos();
            (angle <= LOCK_PICK_MAX_ANGLE_RAD).then_some((entity, angle))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity)
}

/// The `(yaw, pitch)` whose view direction is `direction`, the inverse of
/// `Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z`.
///
/// Pitch goes through [`clamp_pitch`], so a target straight above or below the camera is looked at
/// from just short of vertical rather than flipping the view.
pub fn yaw_pitch_towards(direction: Vec3) -> (f32, f32) {
    let d = direction.normalize_or_zero();
    let yaw = (-d.x).atan2(-d.z);
    let pitch = clamp_pitch(d.y.clamp(-1.0, 1.0).asin());
    (yaw, pitch)
}

/// Total scroll this frame in lines. Positive is toward the screen, which zooms in.
pub fn scroll_lines(events: impl IntoIterator<Item = MouseWheel>) -> f32 {
    events
        .into_iter()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / LOCK_ZOOM_PIXELS_PER_LINE,
        })
        .sum()
}

/// The closest and farthest the camera may orbit a body of drawn radius `radius`.
///
/// The near plane is added to the minimum so the camera can never get close enough to clip the body
/// it is looking at. The maximum is half the far plane, which keeps the target itself and most of
/// what is behind it in view.
pub fn orbit_distance_bounds(radius: f32, near: f32, far: f32) -> (f32, f32) {
    let min = radius * LOCK_MIN_DISTANCE_RADII + near;
    (min, (0.5 * far).max(min))
}

/// `distance` after `lines` of scroll, kept within `[min, max]`.
pub fn zoom_distance(distance: f32, lines: f32, min: f32, max: f32) -> f32 {
    (distance * LOCK_ZOOM_STEP_PER_LINE.powf(lines)).clamp(min, max)
}

/// Lock onto the [`CameraTarget`] nearest the crosshair on `F`, or release the lock if one is held.
///
/// Locking turns the camera to face the target from where it already is. Pressing `F` with nothing
/// near the crosshair does nothing.
pub fn toggle_camera_lock_system(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<MenuState>>,
    targets: Query<(Entity, &Transform), TargetFilter>,
    mut camera: Query<(&mut FreeFlyController, &mut CameraLock, &Transform), With<Camera3d>>,
) {
    if menu.as_ref().is_some_and(|menu| menu.is_open()) || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Ok((mut controller, mut lock, transform)) = camera.get_single_mut() else {
        return;
    };

    if lock.is_locked() {
        lock.0 = None;
        return;
    }

    let eye = transform.translation;
    let candidates = targets.iter().map(|(entity, t)| (entity, t.translation));
    let Some(target) = pick_lock_target(eye, transform.forward().as_vec3(), candidates) else {
        return;
    };
    let Ok((_, target_transform)) = targets.get(target) else {
        return;
    };

    let offset = target_transform.translation - eye;
    (controller.yaw, controller.pitch) = yaw_pitch_towards(offset);
    lock.0 = Some(Orbit {
        target,
        distance: offset.length(),
    });
}

/// Place a locked camera on its orbit: facing the target along yaw and pitch, at the orbit
/// distance, after this frame's scroll.
///
/// Runs in `PostUpdate`, after every `Update` system has moved the bodies, so the camera follows
/// this frame's position rather than lagging a frame behind it. Releases the lock if the target
/// has been despawned, which is what absorption does to a body, leaving the camera where it is.
///
/// The camera keeps following while the `Esc` menu is open, because following is not input, but
/// scroll is ignored there.
pub fn orbit_camera_system(
    menu: Option<Res<MenuState>>,
    mut wheel: EventReader<MouseWheel>,
    targets: Query<&Transform, TargetFilter>,
    mut camera: Query<
        (
            &FreeFlyController,
            &mut CameraLock,
            &mut Transform,
            Option<&Projection>,
        ),
        With<Camera3d>,
    >,
) {
    let lines = scroll_lines(wheel.read().copied());
    let lines = if menu.as_ref().is_some_and(|menu| menu.is_open()) {
        0.0
    } else {
        lines
    };

    let Ok((controller, mut lock, mut transform, projection)) = camera.get_single_mut() else {
        return;
    };
    let Some(orbit) = lock.0.as_mut() else {
        return;
    };
    let Ok(target) = targets.get(orbit.target) else {
        lock.0 = None;
        return;
    };

    let (near, far) = match projection {
        Some(Projection::Perspective(p)) => (p.near, p.far),
        Some(Projection::Orthographic(o)) => (o.near, o.far),
        None => (0.0, f32::MAX),
    };
    // A sphere's scale is its radius on every axis. A cuboid's is its full edge lengths, so the
    // largest of them is a bound on its corners' distance from the centre.
    let (min, max) = orbit_distance_bounds(target.scale.max_element(), near, far);
    orbit.distance = zoom_distance(orbit.distance, lines, min, max);

    transform.rotation = Quat::from_euler(EulerRot::YXZ, controller.yaw, controller.pitch, 0.0);
    transform.translation = target.translation - transform.forward().as_vec3() * orbit.distance;
}

pub struct FreeFlyCameraPlugin;

impl Plugin for FreeFlyCameraPlugin {
    /// Register `free_fly_camera_system` on `Update` (task 8.8 /
    /// Requirement 10.1).
    fn build(&self, app: &mut App) {
        // The gate's resource, so the camera system's `Res<MenuState>` resolves even
        // without `MenuInputPlugin`. Idempotent, and the same call the other two gated
        // plugins make: adding any one of them in isolation must still run.
        app.init_resource::<crate::input::menu::MenuState>();
        app.init_resource::<CameraSettings>();
        app.init_resource::<PopupState>();
        app.add_event::<MouseWheel>();
        app.add_systems(
            Update,
            (
                toggle_camera_lock_system.before(free_fly_camera_system),
                free_fly_camera_system,
                cycle_camera_speed_system,
            ),
        );
        app.add_systems(
            PostUpdate,
            orbit_camera_system.before(TransformSystem::TransformPropagate),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/input/camera_tests.rs"]
mod tests;
