//! The observer clock: wall time, for everything that watches the experiment
//! rather than takes part in it.
//!
//! The camera and the debug overlay run on this. The physics runs on
//! [`SimulationClock`](crate::time::simulation::SimulationClock), which in the
//! shipped app is five million times faster. Keeping them as two types, rather than
//! two readings of one timer, is what makes the distinction impossible to get wrong
//! by accident:
//!
//! | Consumer | Clock | Why |
//! | --- | --- | --- |
//! | `Scene` and everything under it | `SimulationClock` | Scaled: the experiment's own time |
//! | Free-fly camera | `ObserverClock` | You move at a human pace regardless of the scene's clock |
//! | Debug overlay FPS | `ObserverClock` | Frames per *real* second is the only useful reading |
//! | Overlay's simulated-time row | reads `SimulationClock` | Displays the other clock; does not drive anything |
//!
//! Before this split both of these pulled `Res<Time>` independently and nothing
//! recorded that the camera was deliberately unscaled — it was true, load-bearing,
//! and undocumented.
//!
//! # Two step readings, and why
//!
//! [`raw_step_seconds`](ObserverClock::raw_step_seconds) is the frame delta as the
//! timer reported it, which is what an FPS readout must use or it would report the
//! clamp instead of the truth.
//! [`motion_step_seconds`](ObserverClock::motion_step_seconds) is the same delta
//! capped at [`MAX_REAL_STEP_SECONDS`], which is what camera motion must use or a
//! half-second stall would teleport the viewer across the scene. Both are real
//! time; they differ only in whether a hitch is honoured.

use crate::time::constants::MAX_REAL_STEP_SECONDS;
use crate::time::simulation::clamp_real_step;

/// Wall-clock time, for the camera and the overlay.
///
/// # Examples
///
/// ```
/// use physics_sandbox::ObserverClock;
///
/// // 1/64 s is exactly representable in both `f32` and `f64`, so the elapsed-time
/// // assertion can be exact.
/// let mut clock = ObserverClock::default();
/// clock.advance(1.0 / 64.0);
///
/// assert_eq!(clock.raw_step_seconds(), 0.015625);
/// assert_eq!(clock.elapsed_seconds(), 0.015625);
/// assert_eq!(clock.fps(), 64.0);
///
/// // A stall is reported honestly for FPS, but capped for camera motion.
/// clock.advance(0.5);
/// assert_eq!(clock.raw_step_seconds(), 0.5);
/// assert_eq!(clock.motion_step_seconds(), 0.1);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ObserverClock {
    /// Total real time observed, in seconds. `f64` because a long session
    /// accumulates more than `f32` can hold without losing milliseconds.
    elapsed_s: f64,
    /// The last frame delta as reported, sanitised only for finiteness.
    raw_step_s: f32,
}

impl ObserverClock {
    /// A clock at zero elapsed.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a real frame delta.
    ///
    /// Non-finite and negative deltas become `0.0` — the same sanitisation
    /// [`clamp_real_step`] applies, minus the cap, since the raw reading is the
    /// point of this field.
    pub fn advance(&mut self, real_step_s: f32) {
        let sanitised = if real_step_s.is_finite() && real_step_s > 0.0 {
            real_step_s
        } else {
            0.0
        };

        self.raw_step_s = sanitised;
        self.elapsed_s += f64::from(sanitised);
    }

    /// The last frame delta in real seconds, uncapped. Use for FPS.
    #[inline]
    pub fn raw_step_seconds(&self) -> f32 {
        self.raw_step_s
    }

    /// The last frame delta capped at [`MAX_REAL_STEP_SECONDS`]. Use for motion.
    ///
    /// Anything that multiplies a speed by a duration wants this one: a stalled
    /// frame reporting `0.5 s` would move a `5`-unit-per-second camera two and a
    /// half units in a single frame, which reads as a teleport.
    #[inline]
    pub fn motion_step_seconds(&self) -> f32 {
        clamp_real_step(self.raw_step_s, MAX_REAL_STEP_SECONDS)
    }

    /// Total real time observed, in seconds.
    ///
    /// Paired with
    /// [`SimulationClock::elapsed_seconds`](crate::time::simulation::SimulationClock::elapsed_seconds),
    /// this is what lets the overlay show both clocks side by side — and their ratio
    /// is the effective time scale actually achieved, which is not the configured
    /// scale whenever frames are being clamped.
    #[inline]
    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_s
    }

    /// Frames per real second, from the last frame's raw delta.
    ///
    /// `0.0` rather than `inf` when the delta is unusably small — the very first
    /// tick reports `0.0`, and one frame of `inf FPS` in a readout is worse than one
    /// frame of `0.0`.
    #[inline]
    pub fn fps(&self) -> f32 {
        if !self.raw_step_s.is_finite() || self.raw_step_s <= 1.0e-6 {
            return 0.0;
        }
        1.0 / self.raw_step_s
    }

    /// Validation_Rules for `ObserverClock`: elapsed finite and non-negative, last
    /// step finite and non-negative.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.elapsed_s.is_finite()
            && self.elapsed_s >= 0.0
            && self.raw_step_s.is_finite()
            && self.raw_step_s >= 0.0
    }
}

#[cfg(test)]
#[path = "../../tst/time/observer_tests.rs"]
mod tests;
