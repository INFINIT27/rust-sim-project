//! The simulation clock and the [`TimeStep`] it hands out: the crate's single
//! definition of simulated time.
//!
//! # `TimeStep` is the currency, and that is the enforcement
//!
//! Every physics entry point in the crate — [`Scene::step`](crate::core::scene::Scene::step),
//! [`integrate_gravity`](crate::core::integrator::integrate_gravity),
//! [`substep_count`](crate::collision::resolve::substep_count) — takes a
//! `TimeStep`, never an `f32`. A `TimeStep` cannot be conjured from a float without
//! going through [`TimeStep::from_seconds`], which returns `Option` and rejects
//! anything non-finite or negative, so **the type is what enforces the interface**:
//! there is no way to advance the simulation by an unvalidated duration, and no
//! second place where a "dt" is decided.
//!
//! That replaces a scheme where a raw `f32` was clamped in a Bevy system, scaled
//! there, and handed down through four signatures that each re-checked it. The
//! checks are now in one constructor and the guards downstream are `debug_assert!`s
//! about an invariant the type already carries.
//!
//! # Simulated time is not observer time
//!
//! This clock runs at [`SimulationClock::scale`] simulated seconds per real second
//! — `5e6` in the shipped app, about a day of simulated time per frame. It is the
//! clock **and only clock** the physics sees. The camera and the overlay run on
//! [`crate::time::observer::ObserverClock`], which is wall time, so you can fly
//! around an experiment that is running five million times faster than you are.
//! Mixing them up is the defect this split exists to prevent; see
//! `src/time/README.md` for the table of which consumer reads which.
//!
//! # Time only moves forward
//!
//! `TimeStep` is a non-negative magnitude: there is no reverse arrow, and
//! [`Scene::step`](crate::core::scene::Scene::step)'s Requirement 5.5 precondition
//! (`dt >= 0`) is now a property of the type rather than an assertion.
//!
//! Reversibility was considered and deliberately deferred, because a signed step
//! that is not honoured is worse than no signed step — it would look like it worked
//! and quietly diverge. Making one meaningful needs three things this crate does not
//! have, and they are listed here so the next person does not have to rediscover
//! them:
//!
//! 1. **A reversible integrator.** Semi-implicit Euler is not its own inverse;
//!    stepping `−dt` does not undo `+dt`. Velocity Verlet or leapfrog would be
//!    required, and switching changes *forward* trajectories too — invalidating the
//!    orbit-drift measurements `core/integrator.rs` documents.
//! 2. **A contact policy for reverse.** The elastic impulse is time-symmetric, but
//!    the "only if approaching" gate is direction-dependent and positional
//!    de-penetration is a projection that destroys the information needed to invert
//!    it. Reverse through a bounce cannot retrace without disabling contact or
//!    recording a rewind history.
//! 3. **Two amendments** — Requirement 5.5 and Requirement 13.3 both currently
//!    specify that a negative duration is an error to be clamped away.
//!
//! The place to add the sign is this type, and nowhere else, which is most of the
//! value of having it.

use crate::time::constants::{
    DEFAULT_TIME_SCALE, MAX_REAL_STEP_SECONDS, MAX_TIME_SCALE, MIN_TIME_SCALE,
};

/// A validated interval of **simulated** time: finite, non-negative, in seconds.
///
/// The only thing that may advance the simulation. Physics functions take this
/// rather than a float so that "is this duration usable?" is answered once, at
/// construction, instead of at every call site.
///
/// `f64` internally, even though the scene's lengths are `f32`: a scaled step is
/// `8.3e4` seconds in the shipped app and total elapsed time reaches `2e8`, and the
/// closed-form relations in [`crate::physics_utils`] are all `f64`. Callers that
/// need an `f32` — the integrator, which multiplies it into `f32` velocities — ask
/// for [`seconds_f32`](Self::seconds_f32) explicitly, so the narrowing is visible
/// at the point it happens.
///
/// # Examples
///
/// ```
/// use physics_sandbox::TimeStep;
///
/// let step = TimeStep::from_seconds(0.5).expect("half a second is a valid step");
/// assert_eq!(step.seconds(), 0.5);
///
/// // Zero is legal — a paused clock hands out zero-length steps.
/// assert!(TimeStep::ZERO.is_zero());
///
/// // Nothing else is.
/// assert_eq!(TimeStep::from_seconds(-1.0), None);
/// assert_eq!(TimeStep::from_seconds(f64::NAN), None);
/// assert_eq!(TimeStep::from_seconds(f64::INFINITY), None);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Default)]
pub struct TimeStep(f64);

impl TimeStep {
    /// A zero-length step: legal, and what a paused clock produces.
    ///
    /// Physics treats it as a no-op — [`Scene::step`](crate::core::scene::Scene::step)
    /// returns immediately, which is what keeps its Postcondition 5.2 (a zero step
    /// leaves the scene byte-identical) true now that contact resolution, which is a
    /// function of position rather than of duration, runs inside the step.
    pub const ZERO: Self = Self(0.0);

    /// A step of `seconds` simulated seconds, or `None` if that is not a duration.
    ///
    /// Rejects non-finite and negative values. This is the **only** way to build a
    /// `TimeStep` from a float, so it is the choke point the whole interface rests
    /// on: reject here and no downstream function has to defend itself.
    ///
    /// Returns `Option` rather than clamping, because a caller holding a `NaN`
    /// duration has a bug worth surfacing, and silently substituting `0.0` would
    /// hide it. The one place that *does* want the lenient behaviour — a Bevy frame
    /// delta, which Requirement 13.3 says must become `0.0` — gets it from
    /// [`clamp_real_step`] before it ever reaches here.
    #[inline]
    pub fn from_seconds(seconds: f64) -> Option<Self> {
        if seconds.is_finite() && seconds >= 0.0 {
            Some(Self(seconds))
        } else {
            None
        }
    }

    /// The step's duration in simulated seconds.
    #[inline]
    pub fn seconds(&self) -> f64 {
        self.0
    }

    /// The step's duration narrowed to `f32`, for arithmetic against `f32` scene
    /// state.
    ///
    /// Named for the narrowing rather than hiding it: a step of `5e5` seconds has
    /// plenty of `f32` relative precision, but the cast is a real loss of exactness
    /// and the call sites that do it should read as if they know.
    #[inline]
    pub fn seconds_f32(&self) -> f32 {
        self.0 as f32
    }

    /// Is this step exactly zero length?
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0.0
    }

    /// Validation_Rules for `TimeStep`: finite and non-negative.
    ///
    /// Always true for a value built through [`from_seconds`](Self::from_seconds) —
    /// which is the point. It exists so the `debug_assert!`s downstream can state
    /// the invariant they rely on rather than re-deriving it, and so the guarantee
    /// survives a future `impl` that adds arithmetic.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.0.is_finite() && self.0 >= 0.0
    }

    /// This step divided into `parts` equal substeps.
    ///
    /// How [`Scene::step`](crate::core::scene::Scene::step) subdivides a frame for
    /// collision: the substep is still a `TimeStep`, so a substep cannot be
    /// smuggled past the same validation the frame went through.
    ///
    /// `parts == 0` yields [`ZERO`](Self::ZERO) rather than dividing by zero — a
    /// request for no parts is a request for no time. The result is exact for
    /// `parts` a power of two and correctly rounded otherwise; `parts` substeps do
    /// not necessarily re-sum to the original step, and nothing depends on their
    /// doing so.
    #[inline]
    pub fn split(&self, parts: u32) -> Self {
        if parts == 0 {
            return Self::ZERO;
        }
        Self(self.0 / f64::from(parts))
    }
}

/// Clamp a raw real-time frame delta into `[0.0, max_real_step_s]`.
///
/// Requirement 13, in one function:
///
/// - a delta inside the interval passes through unchanged (13.1);
/// - a larger finite delta saturates at `max_real_step_s` (13.2);
/// - a negative or non-finite delta becomes `0.0` (13.3);
/// - the result is therefore always finite and in range (13.4).
///
/// This is the lenient counterpart to [`TimeStep::from_seconds`]: a frame delta
/// comes from a timer that may hiccup, so it is *sanitised*, whereas a duration a
/// caller states explicitly is *rejected* if invalid.
///
/// # Preconditions and release behaviour
///
/// `max_real_step_s` finite and strictly positive. Violations fire a
/// `debug_assert!`; in release the function returns `0.0`, which stalls the
/// simulation rather than stepping it by an unbounded amount.
///
/// # Examples
///
/// ```
/// use physics_sandbox::clamp_real_step;
///
/// assert_eq!(clamp_real_step(0.016, 0.1), 0.016);
/// assert_eq!(clamp_real_step(2.5, 0.1), 0.1);
/// assert_eq!(clamp_real_step(-1.0, 0.1), 0.0);
/// assert_eq!(clamp_real_step(f32::NAN, 0.1), 0.0);
/// ```
#[inline]
pub fn clamp_real_step(raw_s: f32, max_real_step_s: f32) -> f32 {
    debug_assert!(
        max_real_step_s.is_finite() && max_real_step_s > 0.0,
        "clamp_real_step: max_real_step_s must be finite and strictly positive, got {}",
        max_real_step_s
    );

    if !max_real_step_s.is_finite() || max_real_step_s <= 0.0 {
        return 0.0;
    }

    // Finiteness first: `NaN` fails every comparison, so a sign test alone would let
    // it through. Same ordering the rest of the crate uses.
    if !raw_s.is_finite() || raw_s <= 0.0 {
        return 0.0;
    }

    raw_s.min(max_real_step_s)
}

/// The clock that owns simulated time.
///
/// Holds the scale, the pause state, the total elapsed simulated time, and the step
/// most recently handed out. [`advance`](Self::advance) is the single place a real
/// frame delta becomes simulated duration, and the only producer of a
/// [`TimeStep`] in normal operation.
///
/// # What it fixed
///
/// Before this type, the crate had no notion of "how long has the simulation run" —
/// simulated time existed only as a series of anonymous deltas, so the claim that
/// the shipped spheres meet after `2.0e8 s` could not be checked against anything.
/// [`elapsed_seconds`](Self::elapsed_seconds) is that missing quantity, and the
/// overlay now shows it.
///
/// # Examples
///
/// ```
/// use physics_sandbox::SimulationClock;
///
/// // A clock running at 1000 simulated seconds per real second. The frame delta
/// // below is 1/64 s, which is exactly representable in both `f32` and `f64`, so
/// // these assertions can be exact rather than approximate.
/// let mut clock = SimulationClock::new(1000.0);
/// let frame = 1.0 / 64.0;
///
/// let step = clock.advance(frame);
/// assert_eq!(step.seconds(), 15.625);
/// assert_eq!(clock.elapsed_seconds(), 15.625);
///
/// // Pausing stops simulated time without stopping the caller.
/// clock.set_paused(true);
/// assert!(clock.advance(frame).is_zero());
/// assert_eq!(clock.elapsed_seconds(), 15.625);
///
/// // ... and a single-step request lets exactly one frame through.
/// clock.request_single_step();
/// assert_eq!(clock.advance(frame).seconds(), 15.625);
/// assert!(clock.advance(frame).is_zero());
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SimulationClock {
    /// Total simulated time this clock has handed out, in seconds.
    elapsed_s: f64,
    /// Simulated seconds per real second. Clamped into
    /// `[MIN_TIME_SCALE, MAX_TIME_SCALE]` by [`set_scale`](Self::set_scale).
    scale: f32,
    /// Ceiling on the *real* interval one step may represent, in seconds.
    max_real_step_s: f32,
    /// When set, [`advance`](Self::advance) yields `TimeStep::ZERO` and elapsed time
    /// stops.
    paused: bool,
    /// Set by [`request_single_step`](Self::request_single_step): lets exactly one
    /// frame through while paused, then clears itself.
    single_step_pending: bool,
    /// Set by [`hold`](Self::hold) while something that needs the scene to stand still, a
    /// flight plan, is being computed. Separate from `paused`, which is the user's: `P` cannot
    /// release a hold, and a pause set during one survives it.
    held: bool,
    /// The step the last [`advance`](Self::advance) returned, so consumers that did
    /// not call it — the scene-stepping system, the overlay — can read it without a
    /// second source of truth.
    last_step: TimeStep,
}

impl Default for SimulationClock {
    /// A real-time clock: [`DEFAULT_TIME_SCALE`], the Requirement 13.5 step
    /// ceiling, running, at zero elapsed.
    #[inline]
    fn default() -> Self {
        Self::new(DEFAULT_TIME_SCALE)
    }
}

impl SimulationClock {
    /// A running clock at `scale` simulated seconds per real second, starting at
    /// zero elapsed simulated time.
    ///
    /// `scale` is clamped into `[MIN_TIME_SCALE, MAX_TIME_SCALE]`, and a non-finite
    /// value falls back to [`DEFAULT_TIME_SCALE`] — a clock is a piece of
    /// infrastructure many things read, so it refuses to exist in an unusable state
    /// rather than propagating one.
    #[inline]
    pub fn new(scale: f32) -> Self {
        Self {
            elapsed_s: 0.0,
            scale: sanitise_scale(scale),
            max_real_step_s: MAX_REAL_STEP_SECONDS,
            paused: false,
            single_step_pending: false,
            held: false,
            last_step: TimeStep::ZERO,
        }
    }

    /// Turn a real frame delta into simulated duration, and record it.
    ///
    /// The crate's one conversion from observer time to simulated time:
    ///
    /// 1. sanitise and cap the real delta ([`clamp_real_step`]);
    /// 2. zero it if paused — unless a single step was requested, which is consumed
    ///    here;
    /// 3. multiply by [`scale`](Self::scale);
    /// 4. add to [`elapsed_seconds`](Self::elapsed_seconds) and store as
    ///    [`last_step`](Self::last_step).
    ///
    /// Clamp *before* scale, deliberately: the cap is on the real interval a step
    /// may represent, so the largest simulated step is
    /// `max_real_step_seconds() × scale()`. Scaling first would make the cap mean a
    /// different thing at every scale.
    ///
    /// # Returns
    ///
    /// The step to hand to [`Scene::step`](crate::core::scene::Scene::step).
    /// `TimeStep::ZERO` when paused or held, when the delta was unusable, or when the scale
    /// is zero.
    pub fn advance(&mut self, real_step_s: f32) -> TimeStep {
        let real = clamp_real_step(real_step_s, self.max_real_step_s);

        let effective_real = if self.held {
            0.0
        } else if self.paused {
            if self.single_step_pending {
                self.single_step_pending = false;
                real
            } else {
                0.0
            }
        } else {
            // A step request while running is meaningless, and leaving it set would
            // fire the moment the user next paused.
            self.single_step_pending = false;
            real
        };

        // In `f64` before it is stored: at scale `5e6` a `0.1 s` frame is `5e5`
        // simulated seconds, and elapsed time reaches `2e8` over the shipped scene —
        // accumulating that in `f32` would start losing whole seconds.
        let simulated = f64::from(effective_real) * f64::from(self.scale);
        let step = TimeStep::from_seconds(simulated).unwrap_or(TimeStep::ZERO);

        self.elapsed_s += step.seconds();
        self.last_step = step;
        step
    }

    /// The step the last [`advance`](Self::advance) produced.
    #[inline]
    pub fn last_step(&self) -> TimeStep {
        self.last_step
    }

    /// Total simulated time handed out, in seconds.
    ///
    /// The quantity to compare against
    /// [`Scene::two_body_free_fall_seconds`](crate::core::scene::Scene::two_body_free_fall_seconds):
    /// when this passes that, the shipped spheres have met.
    #[inline]
    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_s
    }

    /// Simulated seconds per real second.
    #[inline]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Set the scale, clamped into `[MIN_TIME_SCALE, MAX_TIME_SCALE]`.
    ///
    /// A non-finite value is **ignored** — it leaves the clock as it was rather than
    /// substituting a default, because a caller computing a scale that came out
    /// `NaN` is better served by the clock not changing than by it jumping to real
    /// time.
    #[inline]
    pub fn set_scale(&mut self, scale: f32) {
        if scale.is_finite() {
            self.scale = scale.clamp(MIN_TIME_SCALE, MAX_TIME_SCALE);
        }
    }

    /// Multiply the scale by `factor`, clamped into the same range.
    ///
    /// What the faster/slower key bindings call. A non-finite or non-positive factor
    /// is ignored: zero would silently freeze the simulation in a way that looks
    /// like a hang rather than a pause, and negative would mean reverse time, which
    /// this crate does not implement (see the module docs).
    #[inline]
    pub fn multiply_scale(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.set_scale(self.scale * factor);
        }
    }

    /// Is simulated time stopped?
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Stop or resume simulated time.
    ///
    /// Pausing does not stop the app: the camera, the overlay, and rendering all run
    /// on [`ObserverClock`](crate::time::observer::ObserverClock), so a paused
    /// experiment can still be flown around and inspected. That is the whole point
    /// of keeping the two clocks apart.
    #[inline]
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Hold the clock still, whatever the user's pause, while a flight plan is being computed: a
    /// plan is made from the scene as it is now, so the scene must not move until it is applied.
    /// `P` still flips [`is_paused`](Self::is_paused), which takes effect once the hold is
    /// released.
    #[inline]
    pub fn hold(&mut self) {
        self.held = true;
    }

    /// Release a [`hold`](Self::hold). The clock is paused afterwards only if the user paused it.
    #[inline]
    pub fn release(&mut self) {
        self.held = false;
    }

    /// Is the clock held for a plan?
    #[inline]
    pub fn is_held(&self) -> bool {
        self.held
    }

    /// Flip the pause state.
    #[inline]
    pub fn toggle_paused(&mut self) {
        self.paused = !self.paused;
    }

    /// Let exactly one frame of simulated time through on the next
    /// [`advance`](Self::advance), then re-pause.
    ///
    /// Frame-by-frame inspection. Has no effect unless the clock is paused when the
    /// next `advance` happens, and does not accumulate: pressing the key ten times
    /// while paused advances one frame, not ten, because the flag is a boolean
    /// rather than a counter. That is deliberate — a queue of pending steps would
    /// let a held key run the simulation away.
    #[inline]
    pub fn request_single_step(&mut self) {
        self.single_step_pending = true;
    }

    /// Is a single step queued?
    #[inline]
    pub fn single_step_pending(&self) -> bool {
        self.single_step_pending
    }

    /// The ceiling on the real interval one step may represent, in seconds.
    ///
    /// Requirement 13.5's `0.1`. Multiply by [`scale`](Self::scale) for the largest
    /// simulated step this clock can produce.
    #[inline]
    pub fn max_real_step_seconds(&self) -> f32 {
        self.max_real_step_s
    }

    /// Validation_Rules for `SimulationClock`: elapsed time finite and
    /// non-negative, scale finite and within `[MIN_TIME_SCALE, MAX_TIME_SCALE]`,
    /// step ceiling finite and strictly positive, and the last step valid.
    ///
    /// Every constructor and mutator maintains this; it exists so a system that
    /// receives the clock as a resource can assert what it is relying on.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.elapsed_s.is_finite()
            && self.elapsed_s >= 0.0
            && self.scale.is_finite()
            && (MIN_TIME_SCALE..=MAX_TIME_SCALE).contains(&self.scale)
            && self.max_real_step_s.is_finite()
            && self.max_real_step_s > 0.0
            && self.last_step.is_valid()
    }
}

/// Clamp a scale into the usable range, falling back to
/// [`DEFAULT_TIME_SCALE`] for a non-finite input.
#[inline]
fn sanitise_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(MIN_TIME_SCALE, MAX_TIME_SCALE)
    } else {
        DEFAULT_TIME_SCALE
    }
}

#[cfg(test)]
#[path = "../../tst/time/simulation_tests.rs"]
mod tests;
