//! Events, the Lorentz transformation, and the spacetime interval.
//!
//! An **event** is a place and a time: `(t, x)`. Every other module in this sub-layer is a special
//! case of what happens to an event when the frame changes:
//!
//! ```text
//! t' = γ(t − v·x/c²)        x' = γ(x − v·t)
//! ```
//!
//! - Two ticks of a clock at rest in `S` (`Δx = 0`) come out `Δt' = γΔt`: **time dilation**.
//! - The two ends of a rod at rest in `S'`, marked at one instant in `S` (`Δt = 0`), come out
//!   `Δx' = γΔx`: **length contraction**, read backwards.
//! - Two events at one instant in `S` but different places come out at *different* times in `S'`:
//!   the **relativity of simultaneity**, which has no Newtonian counterpart and is what the other
//!   two effects need in order not to contradict each other.
//!
//! # What does not change
//!
//! The interval `s² = (cΔt)² − Δx²` is the same in every frame. Its sign sorts every pair of events
//! into one of three kinds ([`Separation`]), and the kind is also the same in every frame:
//!
//! | `s²` | Kind | What every frame agrees on |
//! | --- | --- | --- |
//! | `> 0` | timelike | Which event came first. One can cause the other. |
//! | `= 0` | lightlike | A light signal just connects them. |
//! | `< 0` | spacelike | Nothing. The order in time depends on the frame, so neither can cause the other. |
//!
//! The sign convention here is `(+, −)`: positive for timelike, so that `√s²/c` is a proper time.
//!
//! # One axis
//!
//! `x` is the axis the frame moves along. For three space dimensions and a boost in any direction
//! see [`four_vector`](super::four_vector).

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::lorentz_factor;

/// How two events are separated: the sign of the interval between them.
///
/// The same in every inertial frame. See the module docs for what each kind means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separation {
    /// `(cΔt)² > Δx²`: a clock can travel between the two events, and their order is absolute.
    Timelike,
    /// `(cΔt)² = Δx²`: only light connects them.
    Lightlike,
    /// `(cΔt)² < Δx²`: no signal connects them, and their order depends on the frame.
    Spacelike,
}

/// An event's coordinates in `S'`, given its coordinates in `S`.
///
/// `S'` moves at `frame_velocity_m_s` along `+x` as measured in `S`, and the two origins coincide at
/// `t = t' = 0`. Returns `(t', x')` with `t' = γ(t − v·x/c²)` and `x' = γ(x − v·t)`.
///
/// # Returns
///
/// `None` if the frame is not strictly subluminal or an input is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::transform_event;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // A clock at the origin of S ticks at t = 1 s. A frame moving at 0.6c reads 1.25 s for that
/// // tick (time dilation), at a position 0.75 light-seconds behind it.
/// let (time, position) = transform_event(1.0, 0.0, 0.6 * c).unwrap();
/// assert!((time - 1.25).abs() < 1.0e-12);
/// assert!((position + 0.75 * c).abs() < 1.0e-3);
///
/// // A frame cannot move at c.
/// assert_eq!(transform_event(1.0, 0.0, c), None);
/// ```
pub fn transform_event(
    time_s: f64,
    position_m: f64,
    frame_velocity_m_s: f64,
) -> Option<(f64, f64)> {
    if !time_s.is_finite() || !position_m.is_finite() {
        return None;
    }

    let gamma = lorentz_factor(frame_velocity_m_s)?;
    let beta = frame_velocity_m_s / SPEED_OF_LIGHT_M_S;

    // `β·x/c` rather than `v·x/c²`: the same number, without forming `c²` under a product that may
    // already be large.
    let time = gamma * (time_s - beta * position_m / SPEED_OF_LIGHT_M_S);
    let position = gamma * (position_m - frame_velocity_m_s * time_s);

    (time.is_finite() && position.is_finite()).then_some((time, position))
}

/// An event's coordinates in `S`, given its coordinates in `S'`: the inverse of [`transform_event`].
///
/// The same transformation with the frame's velocity reversed, written as delegation so the two
/// cannot disagree about a sign.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::{
///     inverse_transform_event, transform_event,
/// };
///
/// let v = 0.8 * SPEED_OF_LIGHT_M_S;
/// let (time, position) = transform_event(3.0, 1.0e8, v).unwrap();
/// let (back_time, back_position) = inverse_transform_event(time, position, v).unwrap();
/// assert!((back_time - 3.0).abs() < 1.0e-12);
/// assert!((back_position - 1.0e8).abs() < 1.0e-3);
/// ```
pub fn inverse_transform_event(
    time_in_moving_frame_s: f64,
    position_in_moving_frame_m: f64,
    frame_velocity_m_s: f64,
) -> Option<(f64, f64)> {
    transform_event(
        time_in_moving_frame_s,
        position_in_moving_frame_m,
        -frame_velocity_m_s,
    )
}

/// The squared interval between two events, `s² = (cΔt)² − Δx²`, in square metres.
///
/// Positive for a timelike pair, zero for a lightlike one, negative for a spacelike one, and the
/// same in every inertial frame.
///
/// Computed as `(cΔt − Δx)(cΔt + Δx)` rather than as a difference of squares, so that a pair of
/// events very nearly connected by light keeps its sign and its leading digits.
///
/// # Returns
///
/// `None` if an input is not finite or the result overflows.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::interval_squared_m2;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // One second apart at the same place: (c * 1 s)^2.
/// assert_eq!(interval_squared_m2(1.0, 0.0), Some(c * c));
///
/// // A light signal: exactly zero.
/// assert_eq!(interval_squared_m2(2.0, 2.0 * c), Some(0.0));
///
/// // At the same instant, one metre apart: negative.
/// assert_eq!(interval_squared_m2(0.0, 1.0), Some(-1.0));
/// ```
pub fn interval_squared_m2(elapsed_s: f64, separation_m: f64) -> Option<f64> {
    if !elapsed_s.is_finite() || !separation_m.is_finite() {
        return None;
    }

    let light_path_m = SPEED_OF_LIGHT_M_S * elapsed_s;
    let interval = (light_path_m - separation_m) * (light_path_m + separation_m);

    interval.is_finite().then_some(interval)
}

/// Whether two events are timelike, lightlike, or spacelike separated.
///
/// Compares `|cΔt|` with `|Δx|` directly rather than testing the sign of [`interval_squared_m2`],
/// so the answer does not depend on a product that may underflow.
///
/// # Returns
///
/// `None` if an input is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::{classify_separation, Separation};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// assert_eq!(classify_separation(1.0, 0.5 * c), Some(Separation::Timelike));
/// assert_eq!(classify_separation(1.0, -c), Some(Separation::Lightlike));
/// assert_eq!(classify_separation(1.0, 2.0 * c), Some(Separation::Spacelike));
/// ```
pub fn classify_separation(elapsed_s: f64, separation_m: f64) -> Option<Separation> {
    if !elapsed_s.is_finite() || !separation_m.is_finite() {
        return None;
    }

    let light_path_m = (SPEED_OF_LIGHT_M_S * elapsed_s).abs();
    let distance_m = separation_m.abs();

    Some(if light_path_m > distance_m {
        Separation::Timelike
    } else if light_path_m < distance_m {
        Separation::Spacelike
    } else {
        Separation::Lightlike
    })
}

/// The time a clock reads if it travels uniformly from one event to the other: `τ = √(s²)/c`.
///
/// This is the proper time of [`time_dilation`](super::time_dilation), reached from the two events
/// instead of from a speed. It is the longest time any clock can record between them: a clock that
/// takes a detour reads less, which is the twin paradox.
///
/// # Returns
///
/// Seconds, never negative; zero for a lightlike pair. `None` for a spacelike pair, because no clock
/// can be at both events, and `None` for a non-finite input.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::proper_time_between;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // A ship covers 4 light-seconds in 5 seconds (0.8c). Its own clock reads 3 seconds.
/// let tau = proper_time_between(5.0, 4.0 * c).unwrap();
/// assert!((tau - 3.0).abs() < 1.0e-12);
///
/// // Nothing travels between spacelike events.
/// assert_eq!(proper_time_between(1.0, 2.0 * c), None);
/// ```
pub fn proper_time_between(elapsed_s: f64, separation_m: f64) -> Option<f64> {
    if classify_separation(elapsed_s, separation_m)? == Separation::Spacelike {
        return None;
    }

    // Work in seconds, `(Δt − Δx/c)(Δt + Δx/c)`, so `c²` is never formed.
    let light_time_s = separation_m / SPEED_OF_LIGHT_M_S;
    let squared = (elapsed_s - light_time_s) * (elapsed_s + light_time_s);

    // The classification above was made on the unrounded comparison; a product a rounding below
    // zero for a lightlike pair is still zero.
    let proper_time = squared.max(0.0).sqrt();

    proper_time.is_finite().then_some(proper_time)
}

/// The distance between two events in the frame where they happen at the same instant: `√(−s²)`.
///
/// The counterpart of [`proper_time_between`] for a spacelike pair. A rod's rest length is the
/// proper distance between its ends.
///
/// # Returns
///
/// Metres, never negative; zero for a lightlike pair. `None` for a timelike pair, because no frame
/// sees those two events at once, and `None` for a non-finite input.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::proper_distance_between;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // 5 light-seconds apart and 3 seconds apart: 4 light-seconds in the frame where they coincide
/// // in time.
/// let distance = proper_distance_between(3.0, 5.0 * c).unwrap();
/// assert!((distance / c - 4.0).abs() < 1.0e-12);
///
/// assert_eq!(proper_distance_between(1.0, 0.0), None);
/// ```
pub fn proper_distance_between(elapsed_s: f64, separation_m: f64) -> Option<f64> {
    if classify_separation(elapsed_s, separation_m)? == Separation::Timelike {
        return None;
    }

    let light_path_m = SPEED_OF_LIGHT_M_S * elapsed_s;
    let squared = (separation_m - light_path_m) * (separation_m + light_path_m);
    let distance = squared.max(0.0).sqrt();

    distance.is_finite().then_some(distance)
}

/// How far apart in time `S'` finds two events that `S` finds simultaneous: `Δt' = −γ·v·Δx/c²`.
///
/// The relativity of simultaneity as a number. `separation_m` is the second event's position minus
/// the first's, in `S`. A negative result means `S'` finds the second event (the one further along
/// its direction of motion) **earlier**.
///
/// # Returns
///
/// Seconds, or `None` if the frame is not strictly subluminal or an input is not finite. Zero when
/// the frame is at rest or the events are at the same place.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::spacetime::simultaneity_offset_s;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Two flashes one light-second apart, simultaneous on the platform. On a train passing at
/// // 0.6c the front flash happens 0.75 s before the rear one.
/// let offset = simultaneity_offset_s(c, 0.6 * c).unwrap();
/// assert!((offset + 0.75).abs() < 1.0e-12);
/// ```
pub fn simultaneity_offset_s(separation_m: f64, frame_velocity_m_s: f64) -> Option<f64> {
    transform_event(0.0, separation_m, frame_velocity_m_s).map(|(time, _)| time)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/spacetime_tests.rs"]
mod tests;
