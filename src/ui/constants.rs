//! Every constant the `ui` layer reads: the overlay's row budgets and the
//! crosshair's appearance.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module. Two
//! groups, both about fitting an 800×600 window
//! ([`crate::constants::WINDOW_WIDTH_PX`]):
//!
//! - **Overlay** — [`MAX_PROP_ROWS`] and [`MAX_DISTANCE_ROWS`], where the panel stops
//!   listing and starts counting.
//! - **Crosshair** — the `CROSSHAIR_*` values behind `CrosshairConfig::default`.
//! - **Compass** — the `COMPASS_*` values behind `CompassConfig::default`, the top-right
//!   orientation gizmo.
//! - **Menu** — the `MENU_*` values behind the `Esc` panel's layout.
//! - **Placement picker** — the `PLACEMENT_PANEL_*` values behind the `B` panel.
//!
//! Everything here is in **logical screen pixels**, not world units — the only place
//! in the crate where a length is not in units.

/// Most props listed individually in the overlay before it starts summarising.
///
/// **Why this value.** The panel has to stay readable in an 800×600 window, and one
/// row per prop stops being useful long before it stops being possible. Eight rows
/// fits the panel without crowding the distance list below it. Anything beyond this
/// is reported as a count rather than dropped silently — a truncated list that looks
/// complete is worse than one that says so.
///
/// **Used by.** [`crate::ui::overlay::prop_lines`], which appends a summary row once
/// it is exceeded.
///
/// **If you change it.** The shipped scene has three props, so this only bites in
/// larger scenes. Raise it past what the window can show and the panel scrolls or
/// clips — which is worse than the summary line, because the summary tells the reader
/// something is missing.
pub const MAX_PROP_ROWS: usize = 8;

/// Most pairwise distances listed before the overlay starts summarising.
///
/// **Why this value.** Pair count grows as `n(n−1)/2`, so a scene reaches this at 6
/// props while [`MAX_PROP_ROWS`] is still comfortable. The two budgets are different
/// numbers because they are consumed at different rates, not by oversight.
///
/// **Used by.** [`crate::ui::overlay::distance_lines`].
///
/// **If you change it.** Same tradeoff as [`MAX_PROP_ROWS`], but the quadratic growth
/// means a small raise buys very little scene size: 12 rows covers 5 props, 20 covers
/// 6.
pub const MAX_DISTANCE_ROWS: usize = 12;

/// Default length of each crosshair stroke, in logical pixels.
///
/// **Why this value.** Long enough to locate the centre of the view at a glance,
/// short enough not to occlude what is being aimed at. Six pixels reads as a
/// crosshair rather than as a cross.
///
/// **Used by.** `CrosshairConfig::default`, consumed by
/// [`crate::ui::crosshair::crosshair_segments`].
///
/// **If you change it.** `0.0` draws nothing, which is the documented way to hide the
/// crosshair without removing the plugin. Large values start covering the scene, and
/// there is no upper clamp — `crosshair_segments` validates finiteness and sign only.
pub const CROSSHAIR_ARM_LENGTH_PX: f32 = 6.0;

/// Default gap between the exact centre of the view and the inner end of each
/// crosshair stroke, in logical pixels.
///
/// **Why this value.** The gap is the point of the design: it keeps the pixel the
/// crosshair points *at* visible. Three pixels is the smallest gap that stays legible
/// against a textured background.
///
/// **Used by.** `CrosshairConfig::default`, consumed by
/// [`crate::ui::crosshair::crosshair_segments`].
///
/// **If you change it.** `0.0` makes the four strokes meet, giving a solid cross that
/// hides its own target — legal, and tested, but it defeats the purpose. Beyond
/// roughly [`CROSSHAIR_ARM_LENGTH_PX`] the four strokes read as four separate marks
/// rather than one crosshair.
pub const CROSSHAIR_GAP_PX: f32 = 3.0;

/// Default crosshair stroke width, in logical pixels.
///
/// **Why this value.** One pixel is the thinnest line egui will draw crisply, and a
/// crosshair wants to be the thinnest thing that is still visible.
///
/// **Used by.** `CrosshairConfig::default`, consumed by
/// [`crate::ui::crosshair::crosshair_system`] when it strokes the segments.
///
/// **If you change it.** On a high-DPI display a logical pixel covers several
/// physical ones, so this stays visually consistent across scale factors. Thicker
/// strokes progressively occlude the centre of the view.
pub const CROSSHAIR_THICKNESS_PX: f32 = 1.0;

/// Default crosshair colour, as **sRGB** `[r, g, b, a]` with straight
/// (non-premultiplied) alpha.
///
/// **Why this value.** White at `200/255` ≈ 78% opacity reads against both the light
/// floor and the dark background without hiding either. A fully opaque crosshair
/// dominates a mostly-empty scene.
///
/// **Used by.** `CrosshairConfig::default`, converted to `egui::Color32` in
/// [`crate::ui::crosshair::crosshair_system`].
///
/// **If you change it.** Note the colour space: this is **sRGB**, because egui wants
/// sRGB, whereas [`crate::core::scene::Color`] is *linear* RGBA for PBR materials.
/// Copying a value between the two silently washes colours out, which is why this is
/// a plain byte array rather than either of the crate's colour types.
pub const CROSSHAIR_COLOR_SRGBA: [u8; 4] = [255, 255, 255, 200];

/// Length of each compass arm, in logical pixels — also the compass's radius.
///
/// **Why this value.** `28` px makes the three arms readable at a glance without the widget
/// competing with the scene: with labels it occupies roughly a 75 px square in the corner of
/// an 800×600 window, about 1.5% of the area. Small enough to ignore, large enough that a
/// 20° change in heading is visible.
///
/// **Used by.** `CompassConfig::default`, consumed by
/// [`crate::ui::compass::compass_axes`] and by the anchoring in
/// [`crate::ui::compass::compass_system`].
///
/// **If you change it.** The compass is inset by `margin + radius`, so it stays clear of the
/// window edge automatically. Very large values start to overlap the debug overlay, which is
/// anchored top-*left* — they would have to be enormous in an 800-px-wide window.
pub const COMPASS_RADIUS_PX: f32 = 28.0;

/// Gap between the compass's bounding box and the top-right corner, in logical pixels.
///
/// **Why this value.** `16` px reads as deliberate placement rather than as something that
/// fell against the edge, and leaves room for the labels of an arm pointing at the corner.
///
/// **Used by.** `CompassConfig::default`, consumed by `compass_system`'s anchoring.
///
/// **If you change it.** Zero puts the compass's bounding box flush with the corner, where
/// an outward-pointing label can be clipped. Nothing enforces an upper bound; a large
/// margin simply moves it toward the middle of the window.
pub const COMPASS_MARGIN_PX: f32 = 16.0;

/// Compass arm stroke width, in logical pixels.
///
/// **Why this value.** `2` px rather than the crosshair's `1`: the compass is read as three
/// distinct coloured directions, and a one-pixel line carries too little colour to identify
/// at a glance.
///
/// **Used by.** `CompassConfig::default`, consumed by `compass_system`.
///
/// **If you change it.** Cosmetic. Thick arms start to obscure their own labels, which sit
/// just beyond each tip.
pub const COMPASS_THICKNESS_PX: f32 = 2.0;

/// Compass label height, in logical pixels.
///
/// **Why this value.** `13` px is egui's comfortable body size — the labels are single
/// characters (`X`, `Y`, `Z`) and need to be legible against the 3D scene behind them, not
/// merely present.
///
/// **Used by.** `CompassConfig::default`. Also sets how far past each tip its label is
/// pushed, so the two scale together.
///
/// **If you change it.** Small values become unreadable against a busy background; large
/// ones make the labels dominate the arms they annotate.
pub const COMPASS_LABEL_SIZE_PX: f32 = 13.0;

/// Alpha multiplier for a compass axis that points **away** from the viewer.
/// Dimensionless.
///
/// **Why this value.** Load-bearing, not decorative: an axis pointing at your eye and one
/// pointing away both project to a short arm, so without a depth cue the compass is
/// ambiguous exactly when you most need it — looking along an axis. `0.35` is a large enough
/// drop to read instantly as "behind" while leaving the arm visible.
///
/// **Used by.** [`crate::ui::compass::compass_system`], applied to an axis whose
/// [`toward_viewer`](crate::ui::compass::CompassAxis::toward_viewer) is negative.
///
/// **If you change it.** `1.0` removes the cue and makes the compass ambiguous; `0.0` hides
/// receding axes entirely, which is a defensible alternative design but loses the "there is
/// an axis there, going away" information.
pub const COMPASS_DIM_ALPHA: f32 = 0.35;

/// Colour of the compass's **X** arm, as sRGB `[r, g, b, a]` with straight alpha.
///
/// **Why this value.** Red for X, matching
/// [`GRID_AXIS_X_COLOR`](crate::render::constants::GRID_AXIS_X_COLOR) so the compass and the
/// line on the floor agree about which axis is which. Brighter than the grid's version
/// because this is drawn over the scene rather than lit as part of it.
///
/// **Used by.** [`crate::ui::compass::compass_axes`].
///
/// **If you change it.** Note the colour space: **sRGB** here, because egui wants sRGB,
/// whereas the grid's axis colours are *linear* for PBR. The two cannot be copied between
/// each other — see [`CROSSHAIR_COLOR_SRGBA`]. Keep the three arms mutually
/// distinguishable, including for the most common colour-vision deficiencies: the
/// red/green pair is the risky one, which is part of why the arms are labelled at all.
pub const COMPASS_X_COLOR_SRGBA: [u8; 4] = [226, 96, 88, 255];

/// Colour of the compass's **Y** arm, as sRGB `[r, g, b, a]`.
///
/// **Why this value.** Green for Y, the usual convention. Y is the one axis with no line on
/// the floor to match — it points out of it — so this colour is the only place the vertical
/// axis is named by colour.
///
/// **Used by.** `compass_axes`.
///
/// **If you change it.** See [`COMPASS_X_COLOR_SRGBA`].
pub const COMPASS_Y_COLOR_SRGBA: [u8; 4] = [120, 210, 130, 255];

/// Colour of the compass's **Z** arm, as sRGB `[r, g, b, a]`.
///
/// **Why this value.** Blue for Z, matching
/// [`GRID_AXIS_Z_COLOR`](crate::render::constants::GRID_AXIS_Z_COLOR), so the compass arm and
/// the line on the floor name the same axis. No key moves along Z (see amendment A8), which
/// makes seeing where Z has gone the compass's job alone.
///
/// **Used by.** `compass_axes`.
///
/// **If you change it.** See [`COMPASS_X_COLOR_SRGBA`].
pub const COMPASS_Z_COLOR_SRGBA: [u8; 4] = [110, 150, 235, 255];

/// Width the menu panel opens at, in logical pixels.
///
/// **Why this value.** Set by its widest content: the key-bindings table, whose longest
/// action reads "Pause or resume the simulation (the camera keeps working)" — 57
/// characters, about `400` px at egui's default body size. `560` px fits that beside a
/// [`MENU_KEY_COLUMN_PX`]-wide key column with margin, and leaves the settings values room
/// to sit beside their labels rather than wrapping under them.
///
/// **Used by.** [`crate::ui::menu::menu_system`], as the `egui::Window`'s
/// `default_width`. The window is not resizable, so this is the width.
///
/// **If you change it.** Narrower and the long action strings wrap, which turns a scannable
/// two-column table into ragged prose. Much wider and the panel starts to cover the scene it
/// is drawn over — the menu is centred, so it grows into the middle of the view. Note the
/// window is full-screen (amendment A12), so there is plenty of room either way; the
/// constraint is legibility, not space.
pub const MENU_WIDTH_PX: f32 = 560.0;

/// Minimum width of the menu's left column — the key names and the setting labels — in
/// logical pixels.
///
/// **Why this value.** `140` px is enough for the longest key string (`Space / Left Ctrl`)
/// and the longest setting label (`Time scale`), so both grids line up on the same edge and
/// the panel reads as one table rather than two. Without a floor, egui sizes each grid to
/// its own content and the two columns land in different places.
///
/// **Used by.** [`crate::ui::menu::menu_system`], as `min_col_width` on both grids.
///
/// **If you change it.** Too small and the key column wraps mid-binding, which makes
/// `Space / Left Ctrl` look like two rows. Too large and the actions are pushed away from
/// the keys they describe, which is the thing a two-column table exists to avoid.
pub const MENU_KEY_COLUMN_PX: f32 = 140.0;

/// Vertical gap between the menu's sections, in logical pixels.
///
/// **Why this value.** `8` px reads as a deliberate break between "Settings", "Key
/// bindings", and the close button without the panel becoming airy. Group headings inside
/// the bindings list use **half** this, so the hierarchy is visible in the spacing itself:
/// sections are further apart than the groups within one.
///
/// **Used by.** [`crate::ui::menu::menu_system`], via `add_space`.
///
/// **If you change it.** Purely cosmetic. At `0.0` the separators still divide the sections,
/// so nothing becomes ambiguous — it just gets tight.
pub const MENU_SECTION_GAP_PX: f32 = 8.0;

/// Width the placement picker opens at, in logical pixels.
///
/// **Why this value.** Set by its widest row: a name, a mass in solar masses, and the object's
/// note — about 80 characters, or `560` px at egui's default body size. Masses above a
/// thousand solar masses are shown in scientific notation precisely so that row fits; see
/// `solar_mass_label`. Matches [`MENU_WIDTH_PX`] so the two panels read as parts of one
/// interface rather than two unrelated boxes.
///
/// **Used by.** [`crate::ui::placement::placement_panel_system`].
///
/// **If you change it.** Narrower and the notes wrap under the names, which turns a scannable
/// list into a paragraph. The rows are laid out with a monospace-padded name column, so the
/// alignment survives any width that does not wrap.
pub const PLACEMENT_PANEL_WIDTH_PX: f32 = 560.0;

/// Gap between the placement picker and the bottom edge of the window, in logical pixels.
///
/// **Why this value.** `48` px clears the bottom edge without pushing the list up into the
/// crosshair at the centre of the view — which is exactly where the object is about to
/// appear, and therefore the one place a HUD must not cover.
///
/// **Used by.** `placement_panel_system`, as the anchor offset.
///
/// **If you change it.** Zero puts the panel flush with the bottom edge, where a window
/// shadow can be clipped. Large values walk it up toward the crosshair.
pub const PLACEMENT_PANEL_MARGIN_PX: f32 = 48.0;

/// Width the arrival pop-up opens at, in logical pixels.
///
/// **Why this value.** `440` px fits its widest row, a label and an energy with its TNT equivalent
/// ("Energy used" and "8.31 TJ (1.99 kt of TNT)"), on one line at egui's default body size.
///
/// **Used by.** [`crate::ui::arrival::arrival_popup_system`].
///
/// **If you change it.** Narrower and the long values wrap; wider only leaves space.
pub const ARRIVAL_POPUP_WIDTH_PX: f32 = 440.0;

/// Width of the arrival pop-up's label column, in logical pixels.
///
/// **Why this value.** `190` px fits its longest label, "Engine time at full thrust", so every value
/// starts in one column.
///
/// **Used by.** [`crate::ui::arrival::arrival_popup_system`].
///
/// **If you change it.** Narrower and the values stagger; wider pushes them off the right edge.
pub const ARRIVAL_LABEL_COLUMN_PX: f32 = 190.0;

/// The energy of one tonne of TNT, in joules, for the arrival pop-up's comparison.
///
/// **Why this value.** `4.184 GJ`, the defined value of the "ton of TNT" energy unit (one gigacalorie
/// at `4.184 J` per thermochemical calorie). A familiar scale for energies that are otherwise just
/// powers of ten.
///
/// **Used by.** [`crate::ui::arrival::energy_label`].
///
/// **If you change it.** Every TNT comparison scales with it; it is a definition, so don't.
pub const JOULES_PER_TONNE_OF_TNT: f64 = 4.184e9;

/// The arrival pop-up's last line, telling the viewer how it goes away.
///
/// **Why this value.** The pop-up has no button (a click would capture the cursor for mouse-look),
/// so it says what closes it: the next `R`, which starts a new route.
///
/// **Used by.** [`crate::ui::arrival::arrival_popup_system`].
///
/// **If you change it.** Keep it true to how the pop-up closes; it is the only place that says.
pub const ARRIVAL_HINT: &str = "R picks the next departure body and closes this.";
