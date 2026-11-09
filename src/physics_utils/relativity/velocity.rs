//! Transforming a velocity between two frames, and composing two velocities.
//!
//! # The two directions, and why conflating them is the classic error
//!
//! With `S'` moving at `v` along `+x` as measured in `S`:
//!
//! ```text
//!             u − v                        u' + v
//! u'  =  ─────────────        u  =  ─────────────────
//!         1 − u·v/c²                  1 + u'·v/c²
//!
//! [`transform_velocity`]          [`compose_velocities`]
//! "what does S' measure?"         "what does S measure?"
//! ```
//!
//! They are the *same formula with the sign of `v` flipped*, which is exactly why they get swapped:
//! both are "the relativistic version of `u ± v`", both are symmetric-looking, and at low speeds both
//! reduce to Galilean addition, so a test at `1 m/s` cannot tell them apart. `transform_velocity`
//! *removes* the frame's motion; `compose_velocities` *adds* it.
//!
//! Pick by asking which frame the input velocity was measured in — not by which one reads better.
//!
//! # The two properties that make this relativity rather than arithmetic
//!
//! 1. **`c` is invariant.** Compose `c` with any subluminal `v` and you get `c` back:
//!    `(c + v)/(1 + v/c) = c(c + v)/(c + v) = c`. This is the second postulate, falling straight out of
//!    the formula — no special case anywhere in the code puts it there.
//!
//!    In **real** arithmetic that is exact. In `f64` it is not quite, and the gap is worth knowing:
//!    `c − v` and `1 − u·v/c²` each round independently, so the cancellation is only as good as the
//!    denominator. Measured over `10⁵` values of `β`, the result is **bit-identical to `c` for about
//!    65% of them** and the worst relative deviation is `1.9e-12` — roughly `8,600` ULP — reached as
//!    `β → 1`, where `1 − β²` loses most of its significant digits. So the tests here assert a
//!    relative tolerance of `1e-11`, not equality, and `light_speed_is_invariant_to_within_a_rounding`
//!    pins that bound so a future rewrite cannot quietly make it worse.
//! 2. **You cannot reach `c` by adding.** Two subluminal velocities compose to a subluminal velocity,
//!    always. `0.9c + 0.9c = 0.994_475c`, not `1.8c`. A property test drives this over a wide range of
//!    pairs, because it is the claim a reader is least likely to take on trust.
//!
//! # Only one axis
//!
//! These are the **collinear** formulas: the object's velocity is along the same axis as the boost. A
//! velocity with a transverse component transforms differently — the perpendicular part picks up its
//! own `γ` factor and the result is not obtainable by calling this per component. See the
//! sub-layer's module docs on why a general boost needs a `4×4` transformation rather than three
//! scalar calls.

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::is_subluminal;

/// The velocity of an object as measured in `S'`, given its velocity in `S`.
///
/// `S'` moves at `frame_velocity_m_s` along `+x` as measured in `S`. This **removes** the frame's
/// motion: `u' = (u − v)/(1 − u·v/c²)`.
///
/// # Parameters
///
/// - `object_velocity_m_s` — the object's velocity in `S`. May be up to and **including** `±c`: a
///   photon is a legitimate thing to transform, and the answer is the point of the exercise.
/// - `frame_velocity_m_s` — `S'`'s velocity in `S`. Must be **strictly** subluminal, because a frame
///   moving at `c` is not a frame. This asymmetry with the parameter above is deliberate and is the
///   whole reason `c` can be shown to be invariant.
///
/// # Returns
///
/// The velocity in `S'`, or `None` if the frame speed is not strictly subluminal, the object speed
/// exceeds `c` in magnitude, either is not finite, or the denominator vanishes.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{transform_velocity, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // A stationary frame changes nothing.
/// assert_eq!(transform_velocity(0.5 * c, 0.0), Some(0.5 * c));
///
/// // An object at rest in S, seen from a frame moving at 0.5c, recedes at 0.5c.
/// let u = transform_velocity(0.0, 0.5 * c).unwrap();
/// assert!((u + 0.5 * c).abs() < 1.0e-6);
///
/// // Light is light in every frame, to within a rounding — see the module docs on why this is a
/// // tolerance rather than equality.
/// let photon = transform_velocity(c, 0.5 * c).unwrap();
/// assert!((photon / c - 1.0).abs() < 1.0e-11, "got {}c", photon / c);
///
/// let backward = transform_velocity(-c, 0.9 * c).unwrap();
/// assert!((backward / c + 1.0).abs() < 1.0e-11);
///
/// // A frame cannot move at c.
/// assert_eq!(transform_velocity(0.0, c), None);
/// ```
pub fn transform_velocity(object_velocity_m_s: f64, frame_velocity_m_s: f64) -> Option<f64> {
    // The object may sit exactly at `c`; the frame may not. See the parameter docs.
    if !object_velocity_m_s.is_finite()
        || object_velocity_m_s.abs() > SPEED_OF_LIGHT_M_S
        || !is_subluminal(frame_velocity_m_s)
    {
        return None;
    }

    // Divide inside the products rather than forming `u·v` and then `c²`: `u·v` reaches `9e16` for two
    // near-light velocities, which is fine in `f64`, but `(u/c)·(v/c)` is a pair of numbers in `[-1, 1]`
    // and cannot overflow at any input this function accepts. The same reasoning
    // `gravitational_constant_calculator` uses for `G·(m₁/d)·(m₂/d)`.
    let beta_product =
        (object_velocity_m_s / SPEED_OF_LIGHT_M_S) * (frame_velocity_m_s / SPEED_OF_LIGHT_M_S);
    let denominator = 1.0 - beta_product;

    // Zero only if both are exactly `±c` with the same sign, which the frame check already excludes —
    // but the guard stays, because an infinity escaping from here would be indistinguishable from a
    // real velocity two operations later.
    if denominator == 0.0 || !denominator.is_finite() {
        return None;
    }

    let result = (object_velocity_m_s - frame_velocity_m_s) / denominator;

    result.is_finite().then_some(result)
}

/// The relativistic sum of two velocities: the object's velocity in `S`, given its velocity in `S'`.
///
/// `u = (u' + v)/(1 + u'·v/c²)`. This **adds** the frame's motion, and is the inverse of
/// [`transform_velocity`] — the same function with `frame_velocity_m_s` negated.
///
/// # The result is never superluminal
///
/// Given `|u'| ≤ c` and `|v| < c`, the result satisfies `|u| ≤ c`, with equality only when `|u'| = c`.
/// This is not enforced by clamping; it is what the formula does. In `f64` the `|u'| = c` case can land
/// a few parts in `1e12` either side of `c` — see the module docs — so a caller using this as a guard
/// should compare with a tolerance rather than `<`.
///
/// # Returns
///
/// The velocity in `S`, on the same domain as [`transform_velocity`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{compose_velocities, transform_velocity, SPEED_OF_LIGHT_M_S};
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Half light speed plus half light speed is four fifths, not all of it.
/// let sum = compose_velocities(0.5 * c, 0.5 * c).unwrap();
/// assert!((sum / c - 0.8).abs() < 1.0e-12);
///
/// // Two near-light velocities still fall short.
/// let sum = compose_velocities(0.9 * c, 0.9 * c).unwrap();
/// assert!(sum < c, "0.9c + 0.9c is 0.9945c");
///
/// // Light stays light, to within a rounding.
/// let photon = compose_velocities(c, 0.5 * c).unwrap();
/// assert!((photon / c - 1.0).abs() < 1.0e-11);
///
/// // Inverse of `transform_velocity`: composing then removing the same frame is a round trip.
/// let there = compose_velocities(0.3 * c, 0.6 * c).unwrap();
/// let back = transform_velocity(there, 0.6 * c).unwrap();
/// assert!((back - 0.3 * c).abs() < 1.0e-3);
///
/// // At everyday speeds it is Galilean addition to a part in 1e17.
/// let slow = compose_velocities(3.0, 4.0).unwrap();
/// assert!((slow - 7.0).abs() < 1.0e-12);
/// ```
pub fn compose_velocities(
    object_velocity_in_moving_frame_m_s: f64,
    frame_velocity_m_s: f64,
) -> Option<f64> {
    // Literally `transform_velocity` with the frame reversed. Written as delegation rather than as a
    // second copy of the algebra, so the two can never disagree about a sign — which, given that the
    // module docs call sign confusion the classic error here, is not a hypothetical risk.
    transform_velocity(object_velocity_in_moving_frame_m_s, -frame_velocity_m_s)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/velocity_tests.rs"]
mod tests;
