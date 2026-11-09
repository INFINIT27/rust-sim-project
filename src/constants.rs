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
//! - **The key bindings** — [`KEY_BINDINGS`], the one table `--help` and the in-window
//!   menu both read. Data rather than a tunable, and here for the same reason as
//!   everything else in this file: two layers need it and neither may import the other.

use bevy::window::WindowMode;

use crate::bindings::{BindingGroup, KeyBinding};

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

/// Every key the app responds to: the keys, what they do, and which part of the app they
/// belong to.
///
/// **Why these values.** They are the bindings themselves, chosen where each was
/// introduced and documented in the module that implements it — this is the *display*
/// copy, and the only one. `--help` prints it before a window exists and
/// [`crate::ui::menu`] draws it on screen; those live in layers that may not import each
/// other, so a table owned by either would be duplicated in the other and would drift the
/// first time a binding changed. See [`crate::bindings`] for the types and the rendering.
///
/// The order within a group is the order shown, and it is deliberate: the pair that reads
/// as one control (`W / S`, `] / [`) sits on one row, and the rows run from what a new
/// viewer needs first to what they need once.
///
/// **Used by.** [`crate::bindings::bindings_in`] and
/// [`crate::bindings::binding_help_text`], and through them
/// [`crate::experiments::selection::help_text`] and [`crate::ui::menu::menu_system`].
///
/// **If you change it.** Adding a row documents a key; it does not *bind* one — the
/// binding is a `just_pressed` call in the layer that owns the setting, and the two are
/// kept in step by hand. So add the row in the same change as the binding. Two rows
/// claiming the same key fail `tst/bindings_tests.rs`, and two *implementations* claiming
/// one key fail `controls_tests::no_binding_collides_with_the_camera`, which is the check
/// that actually matters.
pub const KEY_BINDINGS: [KeyBinding; 24] = [
    KeyBinding {
        keys: "W / S",
        action: "Fly forward / back, relative to the view",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "A / D",
        action: "Strafe left / right",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Space / Left Ctrl",
        action: "Rise / descend along world Y",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "E / Q",
        action: "Rise / descend — aliases, for hands that prefer them",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "C",
        action: "Cycle camera speed: normal, fast, slow",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Left click",
        action: "Capture the cursor for mouse-look",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Mouse",
        action: "Look around, or orbit when locked, while captured",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "F",
        action: "Lock onto the body at the crosshair, or release it",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "Scroll",
        action: "Zoom in / out on the locked body",
        group: BindingGroup::Camera,
    },
    KeyBinding {
        keys: "P",
        action: "Pause or resume the simulation (the camera keeps working)",
        group: BindingGroup::Simulation,
    },
    KeyBinding {
        keys: ".",
        action: "Advance exactly one frame while paused",
        group: BindingGroup::Simulation,
    },
    KeyBinding {
        keys: "] / [",
        action: "Double / halve the time scale",
        group: BindingGroup::Simulation,
    },
    KeyBinding {
        keys: "\\",
        action: "Reset the time scale to this experiment's own",
        group: BindingGroup::Simulation,
    },
    KeyBinding {
        keys: "+ / -",
        action: "Ten times faster / slower: multiply / divide by 10",
        group: BindingGroup::Simulation,
    },
    KeyBinding {
        keys: "T",
        action: "Show or hide the motion trails",
        group: BindingGroup::View,
    },
    KeyBinding {
        keys: "B",
        action: "Open or close the object placer",
        group: BindingGroup::Place,
    },
    KeyBinding {
        keys: "Up / Down",
        action: "Choose which object to place",
        group: BindingGroup::Place,
    },
    KeyBinding {
        keys: "Enter",
        action: "Place the selected object in front of the camera",
        group: BindingGroup::Place,
    },
    KeyBinding {
        keys: "R",
        action: "Rocket: R on the departure body, then R on the destination",
        group: BindingGroup::Place,
    },
    KeyBinding {
        keys: "G",
        action: "Next spacecraft (crewed 3 g, full thrust, probe)",
        group: BindingGroup::Place,
    },
    KeyBinding {
        keys: "Esc",
        action: "Open or close this menu; also releases the cursor",
        group: BindingGroup::Menu,
    },
    KeyBinding {
        keys: "Cmd-Q",
        action: "Quit (macOS)",
        group: BindingGroup::Window,
    },
    KeyBinding {
        keys: "Alt-F4",
        action: "Quit (Windows / Linux)",
        group: BindingGroup::Window,
    },
    KeyBinding {
        keys: "Ctrl-C",
        action: "Quit from the launching terminal",
        group: BindingGroup::Window,
    },
];
