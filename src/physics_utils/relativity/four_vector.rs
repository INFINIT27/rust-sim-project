//! Four-vectors: the quantities that transform together under a boost in any direction.
//!
//! A four-vector is one time-like number and three space-like ones that mix under a change of frame
//! exactly the way `(ct, x, y, z)` does. The two used here:
//!
//! | Four-vector | Time part | Space part | Its invariant |
//! | --- | --- | --- | --- |
//! | Event ([`event_four_vector`]) | `c·t` | position | the interval `s²` |
//! | Four-momentum ([`four_momentum`]) | `E/c` | momentum | `(mc)²` |
//!
//! The time part is scaled by `c` so that all four components carry the same unit. Without that the
//! boost would need a different formula for each kind of four-vector.
//!
//! # The product that every frame agrees on
//!
//! [`FourVector::minkowski_product`] is `a·b = a_t·b_t − a_x·b_x − a_y·b_y − a_z·b_z`, signature
//! `(+, −, −, −)`, matching [`spacetime`](super::spacetime). A boost changes all four components and
//! leaves this number alone. Every invariant in this sub-layer is this product of something with
//! itself: the interval, and `E² − (pc)² = (mc²)²`.
//!
//! # The boost
//!
//! [`FourVector::boost`] is the `4×4` Lorentz transformation the sub-layer's module docs say a
//! second axis needs. With `β = v/c` as a vector:
//!
//! ```text
//! t' = γ(t − β·x)
//! x' = x + [ γ²/(γ+1)·(β·x) − γ·t ]·β
//! ```
//!
//! Written with `γ²/(γ+1)` instead of the textbook `(γ−1)/β²`, which is the same number but is
//! `0/0` for a frame at rest and loses digits for a slow one.

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::lorentz_factor;

/// One time-like and three space-like components, all in the same unit.
///
/// The type does not know what it holds. An event is in metres (`c·t` and position); a four-momentum
/// is in `kg·m/s` (`E/c` and momentum). Build one with [`event_four_vector`] or [`four_momentum`], or
/// directly when the components are already scaled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FourVector {
    /// The time-like component, already multiplied or divided by `c` to match `space`.
    pub time: f64,
    /// The three space-like components.
    pub space: [f64; 3],
}

impl FourVector {
    /// A four-vector from its components.
    pub fn new(time: f64, space: [f64; 3]) -> Self {
        Self { time, space }
    }

    /// Whether all four components are finite.
    pub fn is_finite(&self) -> bool {
        self.time.is_finite() && self.space.iter().all(|component| component.is_finite())
    }

    /// The Minkowski product `a_t·b_t − a·b`, the same in every inertial frame.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::physics_utils::relativity::four_vector::FourVector;
    ///
    /// let a = FourVector::new(5.0, [3.0, 0.0, 0.0]);
    /// let b = FourVector::new(2.0, [1.0, 1.0, 0.0]);
    /// assert_eq!(a.minkowski_product(&b), 7.0);
    /// ```
    pub fn minkowski_product(&self, other: &FourVector) -> f64 {
        self.time * other.time
            - self.space[0] * other.space[0]
            - self.space[1] * other.space[1]
            - self.space[2] * other.space[2]
    }

    /// The product of the vector with itself: `time² − |space|²`.
    ///
    /// For an event this is the squared interval from the origin, in `m²`. For a four-momentum it is
    /// `(mc)²`. Positive is timelike, zero lightlike, negative spacelike.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::physics_utils::relativity::four_vector::FourVector;
    ///
    /// assert_eq!(FourVector::new(5.0, [3.0, 0.0, 0.0]).invariant(), 16.0);
    /// assert_eq!(FourVector::new(1.0, [0.0, 1.0, 0.0]).invariant(), 0.0);
    /// ```
    pub fn invariant(&self) -> f64 {
        self.minkowski_product(self)
    }

    /// Component-wise sum. The total four-momentum of a system is the sum of its bodies'.
    pub fn plus(&self, other: &FourVector) -> FourVector {
        FourVector {
            time: self.time + other.time,
            space: [
                self.space[0] + other.space[0],
                self.space[1] + other.space[1],
                self.space[2] + other.space[2],
            ],
        }
    }

    /// The same four-vector as measured in a frame moving at `frame_velocity_m_s`.
    ///
    /// A pure boost with no rotation: the axes of the two frames are parallel. Two boosts in
    /// different directions do **not** make a pure boost
    /// (see [`wigner_rotation_angle`](super::velocity_3d::wigner_rotation_angle)).
    ///
    /// # Returns
    ///
    /// `None` if the frame's speed is not strictly below `c`, or a component of either argument is
    /// not finite.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
    /// use physics_sandbox::physics_utils::relativity::four_vector::FourVector;
    ///
    /// let c = SPEED_OF_LIGHT_M_S;
    ///
    /// // A tick at the origin, one unit of ct later. From a frame moving along y at 0.6c it is
    /// // 1.25 units later and 0.75 units behind, and the interval is untouched.
    /// let tick = FourVector::new(1.0, [0.0, 0.0, 0.0]);
    /// let seen = tick.boost([0.0, 0.6 * c, 0.0]).unwrap();
    /// assert!((seen.time - 1.25).abs() < 1.0e-12);
    /// assert!((seen.space[1] + 0.75).abs() < 1.0e-12);
    /// assert!((seen.invariant() - tick.invariant()).abs() < 1.0e-12);
    /// ```
    pub fn boost(&self, frame_velocity_m_s: [f64; 3]) -> Option<FourVector> {
        if !self.is_finite() || frame_velocity_m_s.iter().any(|v| !v.is_finite()) {
            return None;
        }

        let frame_speed = frame_velocity_m_s[0]
            .hypot(frame_velocity_m_s[1])
            .hypot(frame_velocity_m_s[2]);
        let gamma = lorentz_factor(frame_speed)?;
        let beta = frame_velocity_m_s.map(|component| component / SPEED_OF_LIGHT_M_S);

        let along = beta[0] * self.space[0] + beta[1] * self.space[1] + beta[2] * self.space[2];
        let weight = gamma * gamma / (gamma + 1.0) * along - gamma * self.time;

        let boosted = FourVector {
            time: gamma * (self.time - along),
            space: [
                self.space[0] + weight * beta[0],
                self.space[1] + weight * beta[1],
                self.space[2] + weight * beta[2],
            ],
        };

        boosted.is_finite().then_some(boosted)
    }
}

/// The four-vector of an event: `(c·t, position)`, in metres.
///
/// # Returns
///
/// `None` if a component is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::four_vector::event_four_vector;
///
/// let event = event_four_vector(1.0, [0.0, 2.0, 0.0]).unwrap();
/// assert_eq!(event.time, SPEED_OF_LIGHT_M_S);
/// assert_eq!(event.space, [0.0, 2.0, 0.0]);
/// ```
pub fn event_four_vector(time_s: f64, position_m: [f64; 3]) -> Option<FourVector> {
    let event = FourVector::new(SPEED_OF_LIGHT_M_S * time_s, position_m);

    event.is_finite().then_some(event)
}

/// The four-momentum of a massive body: `(E/c, p) = γm·(c, v)`, in `kg·m/s`.
///
/// Its invariant is `(mc)²` whatever the velocity, and the sum of several bodies' four-momenta is
/// what a collision or a decay conserves.
///
/// For light, which has no `γ`, build the vector directly: `FourVector::new(E/c, direction·E/c)`.
///
/// # Returns
///
/// `None` if the mass is not positive and finite, the speed is not strictly below `c`, or a
/// component is not finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::four_vector::four_momentum;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // 2 kg at 0.6c along z: gamma = 1.25, so E/c = 2.5c and p = 1.5c.
/// let p = four_momentum(2.0, [0.0, 0.0, 0.6 * c]).unwrap();
/// assert!((p.time / c - 2.5).abs() < 1.0e-12);
/// assert!((p.space[2] / c - 1.5).abs() < 1.0e-12);
///
/// // The invariant is (mc)^2.
/// assert!((p.invariant() / (c * c) - 4.0).abs() < 1.0e-12);
/// ```
pub fn four_momentum(mass_kg: f64, velocity_m_s: [f64; 3]) -> Option<FourVector> {
    if !mass_kg.is_finite() || mass_kg <= 0.0 || velocity_m_s.iter().any(|v| !v.is_finite()) {
        return None;
    }

    let speed = velocity_m_s[0]
        .hypot(velocity_m_s[1])
        .hypot(velocity_m_s[2]);
    let weighted = lorentz_factor(speed)? * mass_kg;

    let momentum = FourVector {
        time: weighted * SPEED_OF_LIGHT_M_S,
        space: velocity_m_s.map(|component| weighted * component),
    };

    momentum.is_finite().then_some(momentum)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/four_vector_tests.rs"]
mod tests;
