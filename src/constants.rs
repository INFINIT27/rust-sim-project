//! Constants shared across layers, plus the ones the binary shell owns.
//!
//! Per `.claude/steering/constants.md`, a constant lives here when **more than one
//! layer reads it**, or when it belongs to [`main.rs`](../main.rs), which has no
//! layer directory of its own. Everything read by a single layer lives in that
//! layer's own `constants.rs`:
//!
//! | Layer | Its constants |
//! | --- | --- |
//! | [`crate::core`] | [`crate::core::constants`] |
//! | [`crate::collision`] | [`crate::collision::constants`] |
//! | [`crate::physics_utils`] | [`crate::physics_utils::physics_constants`] |
//! | [`crate::render`] | [`crate::render::constants`] |
//! | [`crate::input`] | [`crate::input::constants`] |
//! | [`crate::ui`] | [`crate::ui::constants`] |
//!
//! This file is deliberately short. A value that drifts down to a single reader
//! should move into that layer, and one that spreads to a second layer should move
//! up here — the location is a statement about scope, so keeping it accurate is
//! part of the work.
//!
//! # Sections
//!
//! - **The unit scale** — [`METRES_PER_UNIT`], the definition every length in the
//!   crate is expressed against.
//! - **The shipped scene** — the `TWO_BODY_*` values and [`EARTH_MASS_KG`], which
//!   [`crate::core::scene`] builds the scene from and [`crate::render::plugin`]
//!   frames the camera and scales the clock against.
//! - **The window** — what `main.rs` asks Bevy for: the title, the display mode, and
//!   the size the window takes when it is not full-screen.
//! - **The key bindings** — [`ACTIONS`] (each rebindable action and its default key),
//!   [`FIXED_BINDINGS`], and [`BINDABLE_KEYS`], which the input systems, `--help`, and the
//!   in-window menu all read through [`crate::bindings`]. Here because several layers need them
//!   and none may import another.

use bevy::window::WindowMode;

use bevy::input::keyboard::KeyCode;

use crate::bindings::{Action, ActionInfo, BindingGroup, KeyBinding};

/// How many metres one world unit represents: **1 unit = 10⁹ m**.
///
/// Every length in the crate — positions, `Aabb` bounds, radii, half-extents,
/// half-sizes, line endpoints, hull vertices — is in *world units*, and every
/// velocity is in *units per second*. Mass stays in kilograms and time in seconds:
/// **only length is scaled**, so a `dt` of `1.0` is one second of simulated time
/// whatever this value is.
///
/// **Why this value.** At astronomical scales `f32` runs out of room where it
/// matters: a separation of `3e10 m` stored directly has a representable spacing of
/// about `2000 m`, and the render pipeline would be pushing `1e10`-magnitude
/// transforms through a depth buffer. `1e9` keeps every stored number in the 1–100
/// range, which is `f32`'s comfortable interval, at a cost of one multiply per
/// gravitational pair. For reference at this scale: the scene's `±50`-unit bounds
/// span about `0.67 AU`, an Earth radius is `0.0064` units, and the Earth–Sun
/// distance is `150` units.
///
/// **Used by.** [`crate::core::integrator`], which is the crate's **only**
/// units↔SI conversion site — it scales separations up to metres to evaluate
/// gravitation and scales the resulting acceleration back down.
/// [`crate::core::scene::Prop::speed_m_s`] and [`crate::ui::overlay::units_to_metres`]
/// use it to *display* SI. Note [`crate::collision`] does **not**: contact is
/// scale-free, so the elastic solver takes units/s and returns units/s.
///
/// **If you change it.** Every stored length changes meaning, so the scene
/// constants below have to be rescaled with it or the shipped scene silently
/// becomes a different physical setup. Raising it shrinks stored numbers and
/// worsens `f32` precision at human scales; lowering it toward `1.0` makes the
/// scene SI-native and reintroduces the precision loss it exists to avoid. Nothing
/// in the code assumes a particular value, but every number in the two sections
/// below was chosen against this one.
pub const METRES_PER_UNIT: f64 = 1.0e9;

/// Mass of the Earth, in kilograms — the mass of each sphere in the shipped scene.
///
/// **Why this value.** The CODATA/IAU figure `5.972e24 kg`. A real mass rather
/// than a round number so the scene's timings can be checked against published
/// physics rather than against itself.
///
/// **Used by.** [`crate::core::scene::Scene::two_body_gravity`] for both spheres,
/// and [`crate::core::scene::Scene::two_body_free_fall_seconds`], which derives the
/// encounter time from it.
///
/// **If you change it.** Free-fall time scales as `1/√m`, so doubling the mass
/// makes the pair meet `√2` sooner and
/// [`TWO_BODY_TIME_SCALE`] no longer produces a ~40-second approach. This is a
/// *mass* only: the spheres are drawn at [`TWO_BODY_VISUAL_RADIUS_UNITS`], which is
/// nothing like an Earth radius, because gravitation here treats every prop as a
/// point mass at its centre.
pub const EARTH_MASS_KG: f32 = 5.972e24;

/// Starting centre-to-centre separation of the two spheres, in world units —
/// `30` units is `3e10 m`, about `0.2 AU`.
///
/// **Why this value.** Free-fall time scales as `d^{3/2}`, so a wide start buys a
/// long approach to watch: this separation gives roughly `2.0e8 s` (about 6.5
/// years) of simulated infall, and the interesting part of the acceleration curve
/// is the last third of it.
///
/// **Used by.** [`crate::core::scene::Scene::two_body_gravity`], which places the
/// spheres at `∓half` of it, and
/// [`crate::core::scene::Scene::two_body_free_fall_seconds`].
///
/// **If you change it.** The approach duration moves as `d^{3/2}`, so
/// [`TWO_BODY_TIME_SCALE`] needs adjusting to keep the ~40-second framing. Widen it
/// past the camera's field of view — set up in [`crate::render::constants`] — and
/// the spheres start off screen; narrow it toward
/// `2 · TWO_BODY_VISUAL_RADIUS_UNITS` and they begin already in contact.
pub const TWO_BODY_SEPARATION_UNITS: f32 = 30.0;

/// Height of the two spheres above the floor, in world units.
///
/// **Why this value.** High enough that the spheres are clearly above the ground
/// plane rather than embedded in it — `3` units is twice their radius — and low
/// enough to stay in a frame that also shows the floor for scale.
///
/// **Used by.** [`crate::core::scene::Scene::two_body_gravity`] for both spheres'
/// `y`, and [`crate::render::plugin::spawn_scene_entities`], which aims the camera
/// at this height so the pair sits on the view's centre line.
///
/// **If you change it.** The camera's look-at target follows it automatically, but
/// the floor does not: drop it below [`TWO_BODY_VISUAL_RADIUS_UNITS`] and the
/// spheres intersect the ground slab, which — now that contact is resolved — means
/// they are pushed up out of it on the first step rather than falling cleanly.
pub const TWO_BODY_HEIGHT_UNITS: f32 = 3.0;

/// Radius of each sphere in the shipped scene, in world units.
///
/// **Why this value.** Visibility, not physics: at `1.5` units this is roughly 235
/// Earth radii, which is absurd as a body but necessary to see anything from a
/// camera 45 units away. Gravitation treats each prop as a point mass at its
/// centre, so the radius plays no part in the attraction.
///
/// **Used by.** [`crate::core::scene::Scene::two_body_gravity`], and — since the
/// `collision` layer arrived — every contact between the spheres: their hulls are
/// built from this radius, so it is now **where the bodies stop**.
///
/// **If you change it.** It sets the contact separation (`2 ×` this), so the
/// spheres meet earlier or later in the infall. Shrink it below
/// [`crate::collision::constants::MAX_SUBSTEP_DISPLACEMENT_UNITS`] and a sphere can
/// cross its own diameter within one substep, which is the tunnelling case the
/// substep loop exists to prevent; the hull's inscribed-icosphere undershoot
/// (6.6%) is also a fixed *fraction* of it, so a small sphere has a proportionally
/// small error.
pub const TWO_BODY_VISUAL_RADIUS_UNITS: f32 = 1.5;

/// Simulated seconds per real second for the shipped scene: the clock scale.
///
/// **Why this value.** Two Earth masses `3e10 m` apart take about **6.5 years**
/// (`≈2.04e8 s`) to fall together, and at real time you would watch a still image.
/// `2.04e8 / 5.0e6 ≈ 41` seconds of watching. At 60 fps that is a simulated step of
/// `≈8.3e4 s` — roughly a day per frame — giving about 2500 integration steps over
/// the approach, which is enough for the first-order integrator to trace a smooth
/// curve.
///
/// **Used by.** [`crate::render::plugin::ScenePlugin`], which builds the
/// [`SimulationClock`](crate::time::simulation::SimulationClock) with it, and
/// [`crate::input::controls`], whose reset key restores it. The frame delta is clamped
/// *first* and scaled second,
/// so this multiplies the already-capped
/// [`crate::time::constants::MAX_REAL_STEP_SECONDS`].
///
/// **If you change it.** Raise it and the approach is quicker but each step is
/// coarser, so the integrator's first-order error grows and — past roughly `3e7` —
/// props begin moving far enough per frame that
/// [`crate::collision::constants::MAX_SUBSTEP_DISPLACEMENT_UNITS`] starts splitting
/// frames into many substeps, each a full force evaluation. Lower it and the scene
/// is more accurate and far duller. Nothing physical changes either way: masses,
/// forces, and formulas are untouched, only how much simulated time a frame
/// represents.
pub const TWO_BODY_TIME_SCALE: f32 = 5.0e6;

/// Title of the application window.
///
/// **Why this value.** The crate name, which is what a window list should show.
///
/// **Used by.** `main.rs`, in the `WindowPlugin` it hands to `DefaultPlugins`.
///
/// **If you change it.** Cosmetic. No test asserts it.
pub const WINDOW_TITLE: &str = "physics-sandbox";

/// How the primary window is displayed: **full-screen, borderless**.
///
/// **Why this value.** The app is something you look *through* rather than at — a free-fly
/// camera, a crosshair at the centre of the view, and mouse-look that captures the cursor.
/// An 800×600 window spent most of the screen on the desktop behind it, and at that size
/// a `1`-unit planet on a `150`-unit orbit is two or three pixels across. Full-screen is
/// the difference between seeing an orbit and inferring one.
///
/// **Borderless** rather than [`WindowMode::Fullscreen`] deliberately: borderless keeps the
/// display's current resolution and refresh rate, so there is no mode switch on launch, no
/// black flicker, and no other window getting rearranged when the app exits. Exclusive
/// full-screen buys nothing here — the app is not trying to own the GPU's scanout.
///
/// **Used by.** `main.rs`, as the primary window's `mode`. Amends Requirement 1.1, which
/// fixed an 800×600 window; recorded as amendment A12.
///
/// **If you change it.** [`WindowMode::Windowed`] restores the old behaviour exactly, at
/// [`WINDOW_WIDTH_PX`] × [`WINDOW_HEIGHT_PX`]. Note what full-screen costs: there is no
/// title bar to close, so quitting is `Cmd`-`Q` (macOS), `Alt`-`F4` (Windows/Linux), or
/// `Ctrl`-`C` in the launching terminal — `Esc` only releases the captured cursor. And the
/// aspect ratio becomes the display's rather than 4:3; every camera framing in the crate is
/// derived from the *vertical* field of view, which does not change, and a wider display
/// only adds horizontal margin — so nothing gets cropped.
pub const WINDOW_MODE: WindowMode = WindowMode::BorderlessFullscreen;

/// Width of the window when it is **not** full-screen, in physical pixels.
///
/// **Why this value.** `800 × 600` is the size Requirement 1 fixed, kept as the windowed
/// fallback: small enough to open comfortably on any display, and the aspect ratio every
/// camera-framing constant in the crate was originally derived against. It has no effect
/// while [`WINDOW_MODE`] is full-screen, because the display's own resolution is used —
/// but it is what the window becomes the moment that constant changes, so it is a live
/// value rather than dead weight.
///
/// **Used by.** `main.rs`, as the primary window's resolution.
///
/// **If you change it.** The camera's framing depends on the *aspect ratio*, not the pixel
/// count: Bevy's default projection has a fixed vertical field of view, so a window
/// narrower than 4:3 crops the scene horizontally. See
/// [`crate::experiments::constants::TWO_BODY_CAMERA_POSITION_UNITS`] for the framing that
/// assumes it.
pub const WINDOW_WIDTH_PX: f32 = 800.0;

/// Height of the window when it is **not** full-screen, in physical pixels.
///
/// **Why this value.** The `600` of `800 × 600` — see [`WINDOW_WIDTH_PX`].
///
/// **Used by.** `main.rs`, as the primary window's resolution.
///
/// **If you change it.** Together with [`WINDOW_WIDTH_PX`] it sets the windowed aspect
/// ratio, which is what the camera framing assumes. The overlay, crosshair, and compass are
/// laid out in pixels and anchored to edges, so all three reposition themselves at any size
/// — including full-screen — and none needs adjusting.
pub const WINDOW_HEIGHT_PX: f32 = 600.0;

/// Every rebindable action: its name in `config/controls.toml`, what it does, which part of the
/// app it belongs to, and the key it starts on.
///
/// **Why these values.** They are the bindings themselves, chosen where each was introduced and
/// documented in the module that implements it. The order within a group is the order shown in
/// `--help` and the menu, from what a new viewer needs first to what they need once, and it must
/// match [`Action::ALL`] (a test checks), because [`Action::info`] indexes this table.
///
/// **Used by.** [`KeyMap::default`](crate::bindings::KeyMap), and through it every input system
/// that reads a key, `--help` ([`crate::bindings::binding_help_text`]), and the menu's controls
/// page.
///
/// **If you change it.** Changing a default key rebinds it for everyone who has not saved their
/// own `config/controls.toml`. Two actions on one key fail `tst/bindings_tests.rs`. Adding an
/// action means a variant in [`Action`], a row here, and a `KeyMap` call in the system that acts.
pub const ACTIONS: [ActionInfo; 28] = [
    ActionInfo {
        action: Action::MoveForward,
        name: "move_forward",
        label: "Fly forward",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyW,
    },
    ActionInfo {
        action: Action::MoveBack,
        name: "move_back",
        label: "Fly back",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyS,
    },
    ActionInfo {
        action: Action::MoveLeft,
        name: "move_left",
        label: "Fly left",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyA,
    },
    ActionInfo {
        action: Action::MoveRight,
        name: "move_right",
        label: "Fly right",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyD,
    },
    ActionInfo {
        action: Action::MoveUp,
        name: "move_up",
        label: "Fly up (world up)",
        group: BindingGroup::Camera,
        default_key: KeyCode::Space,
    },
    ActionInfo {
        action: Action::MoveDown,
        name: "move_down",
        label: "Fly down (world down)",
        group: BindingGroup::Camera,
        default_key: KeyCode::ControlLeft,
    },
    ActionInfo {
        action: Action::MoveUpAlt,
        name: "move_up_alt",
        label: "Fly up, second key",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyE,
    },
    ActionInfo {
        action: Action::MoveDownAlt,
        name: "move_down_alt",
        label: "Fly down, second key",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyQ,
    },
    ActionInfo {
        action: Action::CycleSpeed,
        name: "cycle_speed",
        label: "Cycle camera speed (normal, fast, slow)",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyC,
    },
    ActionInfo {
        action: Action::LockTarget,
        name: "lock_target",
        label: "Lock onto the body at the crosshair, or release it",
        group: BindingGroup::Camera,
        default_key: KeyCode::KeyF,
    },
    ActionInfo {
        action: Action::Pause,
        name: "pause",
        label: "Pause / resume the simulation",
        group: BindingGroup::Simulation,
        default_key: KeyCode::KeyP,
    },
    ActionInfo {
        action: Action::SingleStep,
        name: "single_step",
        label: "Advance one frame while paused",
        group: BindingGroup::Simulation,
        default_key: KeyCode::Period,
    },
    ActionInfo {
        action: Action::TimeFaster,
        name: "time_faster",
        label: "Double the time scale",
        group: BindingGroup::Simulation,
        default_key: KeyCode::BracketRight,
    },
    ActionInfo {
        action: Action::TimeSlower,
        name: "time_slower",
        label: "Halve the time scale",
        group: BindingGroup::Simulation,
        default_key: KeyCode::BracketLeft,
    },
    ActionInfo {
        action: Action::TimeDecadeFaster,
        name: "time_ten_times_faster",
        label: "Time scale x10",
        group: BindingGroup::Simulation,
        default_key: KeyCode::Equal,
    },
    ActionInfo {
        action: Action::TimeDecadeSlower,
        name: "time_ten_times_slower",
        label: "Time scale /10",
        group: BindingGroup::Simulation,
        default_key: KeyCode::Minus,
    },
    ActionInfo {
        action: Action::TimeReset,
        name: "time_reset",
        label: "Reset the time scale",
        group: BindingGroup::Simulation,
        default_key: KeyCode::Backslash,
    },
    ActionInfo {
        action: Action::ToggleTrails,
        name: "toggle_trails",
        label: "Show / hide motion trails",
        group: BindingGroup::View,
        default_key: KeyCode::KeyT,
    },
    ActionInfo {
        action: Action::TogglePicker,
        name: "toggle_picker",
        label: "Open / close the object picker",
        group: BindingGroup::Place,
        default_key: KeyCode::KeyB,
    },
    ActionInfo {
        action: Action::PickerUp,
        name: "picker_up",
        label: "Picker: previous object",
        group: BindingGroup::Place,
        default_key: KeyCode::ArrowUp,
    },
    ActionInfo {
        action: Action::PickerDown,
        name: "picker_down",
        label: "Picker: next object",
        group: BindingGroup::Place,
        default_key: KeyCode::ArrowDown,
    },
    ActionInfo {
        action: Action::PickerPlace,
        name: "picker_place",
        label: "Picker: place it in front of the camera",
        group: BindingGroup::Place,
        default_key: KeyCode::Enter,
    },
    ActionInfo {
        action: Action::Launch,
        name: "launch",
        label: "Rocket: on the departure body, then on the destination",
        group: BindingGroup::Place,
        default_key: KeyCode::KeyR,
    },
    ActionInfo {
        action: Action::NextCraft,
        name: "next_craft",
        label: "Next spacecraft in the fleet",
        group: BindingGroup::Place,
        default_key: KeyCode::KeyG,
    },
    ActionInfo {
        action: Action::ToggleBlackHoleView,
        name: "toggle_black_hole_view",
        label: "Black hole disc: shaded / test pattern",
        group: BindingGroup::View,
        default_key: KeyCode::KeyV,
    },
    ActionInfo {
        action: Action::ToggleStars,
        name: "toggle_stars",
        label: "Show / hide the stars round the black hole",
        group: BindingGroup::View,
        default_key: KeyCode::KeyN,
    },
    ActionInfo {
        action: Action::ToggleHud,
        name: "toggle_hud",
        label: "Show / hide the HUD (debug panel, crosshair, compass)",
        group: BindingGroup::View,
        default_key: KeyCode::KeyH,
    },
    ActionInfo {
        action: Action::ToggleOutline,
        name: "toggle_black_hole_outline",
        label: "Show / hide the outline of the black hole's event horizon",
        group: BindingGroup::View,
        default_key: KeyCode::KeyO,
    },
];

/// The bindings that cannot be changed: the mouse, `Esc`, and the operating system's quit keys.
///
/// **Why these values.** The mouse is not a key. `Esc` opens the menu where keys are rebound, so
/// rebinding it could lock you out of the menu; it also cancels a rebind in progress. The quit
/// keys belong to the OS, not to the app.
///
/// **Used by.** [`crate::bindings::binding_rows`], so `--help` and the menu list them after the
/// rebindable actions of their group.
///
/// **If you change it.** These rows only describe; the bindings are in the systems themselves.
pub const FIXED_BINDINGS: [KeyBinding; 7] = [
    KeyBinding {
        keys: "Left click",
        action: "Capture the cursor for mouse-look",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Mouse",
        action: "Look around (while captured)",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Scroll",
        action: "Zoom in / out on the locked body",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Esc",
        action: "Close a pop-up, or open or close the menu",
        group: BindingGroup::Menu,
    },
    KeyBinding {
        keys: "Cmd-Q",
        action: "Quit (macOS)",
        group: BindingGroup::Window,
    },
    KeyBinding {
        keys: "Alt-F4",
        action: "Quit (Windows, Linux)",
        group: BindingGroup::Window,
    },
    KeyBinding {
        keys: "Ctrl-C",
        action: "Quit from the terminal",
        group: BindingGroup::Window,
    },
];

/// The keys an action may be rebound to.
///
/// **Why these values.** Letters, digits, punctuation, arrows, the editing block, function keys,
/// the modifiers, and the number pad: every key a keyboard reliably reports. Left out: `Esc`,
/// which is the menu's (see [`FIXED_BINDINGS`]), and the OS keys (`Super`, media keys), which the
/// OS often keeps for itself.
///
/// **Used by.** The menu's rebind capture (a key outside this list is ignored), and
/// [`crate::bindings::parse_key`], which reads `config/controls.toml`.
///
/// **If you change it.** A key removed here can no longer be bound, and a saved file naming it
/// fails to load.
pub const BINDABLE_KEYS: [KeyCode; 96] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Backquote,
    KeyCode::Minus,
    KeyCode::Equal,
    KeyCode::BracketLeft,
    KeyCode::BracketRight,
    KeyCode::Backslash,
    KeyCode::Semicolon,
    KeyCode::Quote,
    KeyCode::Comma,
    KeyCode::Period,
    KeyCode::Slash,
    KeyCode::Space,
    KeyCode::Enter,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Insert,
    KeyCode::Delete,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
    KeyCode::NumpadAdd,
    KeyCode::NumpadSubtract,
    KeyCode::NumpadMultiply,
    KeyCode::NumpadDivide,
    KeyCode::NumpadDecimal,
    KeyCode::NumpadEnter,
    KeyCode::CapsLock,
];
