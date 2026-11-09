//! Every constant the `time` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module.
//! The shipped scene's clock scale is not here — that is
//! [`crate::constants::TWO_BODY_TIME_SCALE`], because it describes the *scene*
//! rather than the clock, and two layers read it.

/// Largest **real** interval one simulation step may represent, in seconds.
///
/// The clamp Requirement 13 specifies: a raw frame delta above this saturates
/// here before it is scaled into simulated time.
///
/// **Why this value.** Requirement 13.5 fixes it at `0.1`. It is the largest real
/// interval the integrator is trusted with: a dropped frame, a window drag, or a
/// debugger pause produces an arbitrarily large delta, and feeding that to a
/// first-order integrator makes props jump. Clamping is what keeps a hitch from
/// becoming a teleport.
///
/// **Used by.** [`crate::time::simulation::clamp_real_step`], and through it every
/// [`SimulationClock::advance`](crate::time::simulation::SimulationClock::advance).
/// [`crate::time::observer::ObserverClock`] uses it too, for camera motion only.
///
/// **If you change it.** The ceiling on a single simulated step is this **times the
/// clock's scale** — `0.1 × 5e6 = 5e5` simulated seconds in the shipped app, which
/// is the figure the collision substep budget was sized against. Raising it lets a
/// bad frame move props further in one step; lowering it makes the simulation lag
/// real time under load rather than jump, which is the safer failure.
pub const MAX_REAL_STEP_SECONDS: f32 = 0.1;

/// Default simulated seconds per real second: `1.0`, real time.
///
/// **Why this value.** Real time is the least surprising default for a clock with
/// no scene attached — a default that silently ran five million times fast would be
/// a trap. The shipped app overrides it with
/// [`crate::constants::TWO_BODY_TIME_SCALE`], because *that* scene needs it.
///
/// **Used by.** `SimulationClock::default`.
///
/// **If you change it.** Only affects a clock built without an explicit scale;
/// `ScenePlugin` always sets one, so the running app is unaffected.
pub const DEFAULT_TIME_SCALE: f32 = 1.0;

/// Slowest clock scale the observer controls will select.
///
/// **Why this value.** Below real time this scene is a still image — two Earth
/// masses take 6.5 years to fall together — so slow motion is not the tool you
/// want; **pause** is, and it is a separate control that does not have to travel
/// through the scale. Stopping the floor at `1.0` also keeps the meaning of the
/// scale unambiguous: it is a multiplier on real time, never a divisor.
///
/// **Used by.** [`SimulationClock::set_scale`](crate::time::simulation::SimulationClock::set_scale),
/// which clamps into `[MIN_TIME_SCALE, MAX_TIME_SCALE]`, and therefore by the
/// slower/faster key bindings in [`crate::input::controls`].
///
/// **If you change it.** Lowering it below `1.0` makes the slower key meaningful
/// for scenes at human scale — a metre-scale pair orbits in microseconds and wants
/// a scale far below one — at the cost of letting this scene be slowed to a
/// standstill without being paused, which reads as a freeze bug.
pub const MIN_TIME_SCALE: f32 = 1.0;

/// Fastest clock scale the observer controls will select.
///
/// **Why this value.** Derived, not picked: at `1e9` a single 60 Hz frame advances
/// `1.7e7` simulated seconds, which is about 8% of the shipped scene's `2.0e8 s`
/// infall. A first-order integrator taking twelve steps across the whole encounter
/// is not integrating, it is guessing, and the substep loop cannot rescue it —
/// [`crate::collision::constants::MAX_SUBSTEPS`] caps at 64 substeps per frame.
/// This is therefore the point past which the numbers stop meaning anything, which
/// is the right place to stop.
///
/// **Used by.** `SimulationClock::set_scale`, as the upper clamp.
///
/// **If you change it.** Raising it does not crash anything — the clamps and
/// validity guards all hold — it silently degrades accuracy, which is the failure
/// mode this constant exists to make deliberate rather than accidental.
pub const MAX_TIME_SCALE: f32 = 1.0e9;

/// Seconds in a minute.
///
/// **Why this value.** Definitional, not tunable: the SI minute is 60 seconds. It is a
/// named constant so that a duration conversion reads as arithmetic on units rather
/// than as a magic `60.0`, and so the chain below is built from one place.
///
/// **Used by.** [`crate::ui::overlay::human_duration`], which renders an elapsed time
/// in the largest unit that leaves it above one.
///
/// **If you change it.** Do not. It is a unit definition; a different value would make
/// every duration the overlay prints wrong.
pub const SECONDS_PER_MINUTE: f64 = 60.0;

/// Seconds in an hour. Definitional; see [`SECONDS_PER_MINUTE`].
///
/// **Used by.** [`crate::ui::overlay::human_duration`].
///
/// **If you change it.** Do not — a unit definition.
pub const SECONDS_PER_HOUR: f64 = 60.0 * SECONDS_PER_MINUTE;

/// Seconds in a day. Definitional; see [`SECONDS_PER_MINUTE`].
///
/// **Used by.** [`crate::ui::overlay::human_duration`].
///
/// **If you change it.** Do not — a unit definition. Note this is the 86,400-second
/// SI day, not a sidereal day; the scene's astronomical figures use the same
/// convention.
pub const SECONDS_PER_DAY: f64 = 24.0 * SECONDS_PER_HOUR;

/// Seconds in a **Julian** year: 365.25 days.
///
/// **Why this value.** The Julian year is the IAU convention for astronomical
/// intervals, and it is what the scene's "about 6.5 years to fall together" figure is
/// quoted in. Using a calendar year here would put the overlay and the docs into
/// quiet disagreement.
///
/// **Used by.** [`crate::ui::overlay::human_duration`].
///
/// **If you change it.** The only defensible alternatives are the tropical or sidereal
/// year, which differ by minutes — invisible against this scene, but the free-fall
/// figures in `core/scene.rs` and the root `README.md` are quoted against this one, so
/// change those in the same edit.
pub const SECONDS_PER_JULIAN_YEAR: f64 = 365.25 * SECONDS_PER_DAY;
