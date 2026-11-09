//! Kinematics: position, velocity, acceleration, and time under constant
//! acceleration.
//!
//! Scalars along a single caller-chosen axis, in SI units, with sign carrying
//! direction — the convention [`super::newtons_laws`] and
//! [`super::conservation_laws`] use. A caller working in 3-D applies these
//! componentwise.
//!
//! # What is here
//!
//! The seven constant-acceleration relations as free functions, plus
//! [`MotionState`], which bundles the three quantities into one value that can
//! [`advance`](MotionState::advance) itself. The struct delegates to the
//! functions rather than restating the arithmetic, so there is exactly one
//! implementation of each relation.
//!
//! | Want | Given | Function |
//! | --- | --- | --- |
//! | mean velocity | two positions, a duration | [`average_velocity`] |
//! | mean acceleration | two velocities, a duration | [`average_acceleration`] |
//! | final velocity | `v₀`, `a`, `t` | [`velocity_after`] |
//! | displacement | `v₀`, `a`, `t` | [`displacement`] |
//! | final position | `x₀`, `v₀`, `a`, `t` | [`position_after`] |
//! | final speed, no time | `v₀`, `a`, `s` | [`speed_from_displacement`] |
//! | time to a velocity | `v₀`, `v₁`, `a` | [`time_to_reach_velocity`] |
//!
//! # Constant acceleration is an assumption, not a guarantee
//!
//! Every relation here is the exact analytic solution for acceleration that does
//! not change over the interval. That makes them exact rather than approximate —
//! [`position_after`] over one second equals a thousand one-millisecond steps up
//! to float rounding, which is asserted as a property — but it also means a
//! caller whose acceleration varies must re-derive it and take short steps. The
//! functions cannot tell the difference and will not warn you.
//!
//! # Two functions return `Option`, and why
//!
//! [`speed_from_displacement`] and [`time_to_reach_velocity`] can be asked about
//! states that do not exist: a height above a projectile's apex, or the time at
//! which a coasting body reaches a different speed. Those are not precondition
//! violations — the inputs are perfectly valid — so they return `None` rather
//! than this layer's usual benign fallback. Reporting `0.0` would tell the caller
//! "immediately", which is a wrong answer rather than a cautious one. Every other
//! function here keeps the `debug_assert!`-plus-fallback contract.

use super::conservation_laws::{kinetic_energy, momentum};
use super::newtons_laws::net_force;

/// Mean velocity over an interval: `v̄ = (x₁ − x₀) / t`.
///
/// The definition of velocity, independent of any assumption about
/// acceleration. For constant acceleration it also equals the midpoint of the
/// initial and final velocities, `(v₀ + v₁)/2`, which is asserted as a property.
///
/// # Parameters
///
/// - `initial_position_m`, `final_position_m` — positions in metres. Finite.
/// - `elapsed_s` — duration in seconds. Finite and **strictly positive**: mean
///   velocity over zero time is undefined, not zero.
///
/// # Returns
///
/// Mean velocity in metres per second, signed along the caller's axis.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::average_velocity;
///
/// assert_eq!(average_velocity(0.0, 10.0, 2.0), 5.0);
/// assert_eq!(average_velocity(10.0, 0.0, 2.0), -5.0);
/// ```
#[inline]
pub fn average_velocity(initial_position_m: f64, final_position_m: f64, elapsed_s: f64) -> f64 {
    debug_assert!(
        initial_position_m.is_finite(),
        "average_velocity: initial_position_m must be finite, got {}",
        initial_position_m
    );
    debug_assert!(
        final_position_m.is_finite(),
        "average_velocity: final_position_m must be finite, got {}",
        final_position_m
    );
    debug_assert!(
        elapsed_s.is_finite() && elapsed_s > 0.0,
        "average_velocity: elapsed_s must be finite and strictly positive, got {}",
        elapsed_s
    );

    if !initial_position_m.is_finite()
        || !final_position_m.is_finite()
        || !elapsed_s.is_finite()
        || elapsed_s <= 0.0
    {
        return 0.0;
    }

    (final_position_m - initial_position_m) / elapsed_s
}

/// Mean acceleration over an interval: `ā = (v₁ − v₀) / t`.
///
/// The definition of acceleration. Pairs with
/// [`super::newtons_laws::net_force`] to get from an observed velocity change to
/// the force that caused it.
///
/// # Parameters
///
/// - `initial_velocity_m_s`, `final_velocity_m_s` — velocities in metres per
///   second. Finite.
/// - `elapsed_s` — duration in seconds. Finite and strictly positive.
///
/// # Returns
///
/// Mean acceleration in metres per second squared.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::average_acceleration;
///
/// // 0 to 60 m/s in 5 s.
/// assert_eq!(average_acceleration(0.0, 60.0, 5.0), 12.0);
///
/// // Braking gives a negative acceleration.
/// assert_eq!(average_acceleration(20.0, 0.0, 4.0), -5.0);
/// ```
#[inline]
pub fn average_acceleration(
    initial_velocity_m_s: f64,
    final_velocity_m_s: f64,
    elapsed_s: f64,
) -> f64 {
    debug_assert!(
        initial_velocity_m_s.is_finite(),
        "average_acceleration: initial_velocity_m_s must be finite, got {}",
        initial_velocity_m_s
    );
    debug_assert!(
        final_velocity_m_s.is_finite(),
        "average_acceleration: final_velocity_m_s must be finite, got {}",
        final_velocity_m_s
    );
    debug_assert!(
        elapsed_s.is_finite() && elapsed_s > 0.0,
        "average_acceleration: elapsed_s must be finite and strictly positive, got {}",
        elapsed_s
    );

    if !initial_velocity_m_s.is_finite()
        || !final_velocity_m_s.is_finite()
        || !elapsed_s.is_finite()
        || elapsed_s <= 0.0
    {
        return 0.0;
    }

    (final_velocity_m_s - initial_velocity_m_s) / elapsed_s
}

/// Velocity after accelerating for a time: `v = v₀ + a·t`.
///
/// # Parameters
///
/// - `initial_velocity_m_s` — starting velocity in metres per second. Finite.
/// - `acceleration_m_s2` — constant acceleration in metres per second squared.
///   Finite. Zero recovers Newton's first law: the velocity is unchanged.
/// - `elapsed_s` — duration in seconds. Finite and non-negative, matching
///   `Scene::step`'s `dt` contract.
///
/// # Returns
///
/// The velocity in metres per second at the end of the interval.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `initial_velocity_m_s` unchanged — the body coasts, which is the only fallback
/// that neither stops it dead nor launches it.
///
/// # Examples
///
/// ```
/// use physics_sandbox::velocity_after;
///
/// // Free fall from rest for 2 s (taking down as negative).
/// assert_eq!(velocity_after(0.0, -9.81, 2.0), -19.62);
///
/// // No acceleration, no change — Newton's first law.
/// assert_eq!(velocity_after(7.0, 0.0, 100.0), 7.0);
/// ```
#[inline]
pub fn velocity_after(initial_velocity_m_s: f64, acceleration_m_s2: f64, elapsed_s: f64) -> f64 {
    debug_assert!(
        initial_velocity_m_s.is_finite(),
        "velocity_after: initial_velocity_m_s must be finite, got {}",
        initial_velocity_m_s
    );
    debug_assert!(
        acceleration_m_s2.is_finite(),
        "velocity_after: acceleration_m_s2 must be finite, got {}",
        acceleration_m_s2
    );
    debug_assert!(
        elapsed_s.is_finite() && elapsed_s >= 0.0,
        "velocity_after: elapsed_s must be finite and non-negative, got {}",
        elapsed_s
    );

    if !initial_velocity_m_s.is_finite()
        || !acceleration_m_s2.is_finite()
        || !elapsed_s.is_finite()
        || elapsed_s < 0.0
    {
        return initial_velocity_m_s;
    }

    initial_velocity_m_s + acceleration_m_s2 * elapsed_s
}

/// Displacement over an interval: `s = v₀·t + ½·a·t²`.
///
/// Note this is *net* displacement, not distance travelled: a body thrown up and
/// caught again has zero displacement and a nonzero path length. Nothing here
/// computes path length.
///
/// # Parameters
///
/// - `initial_velocity_m_s` — starting velocity in metres per second. Finite.
/// - `acceleration_m_s2` — constant acceleration in metres per second squared.
///   Finite.
/// - `elapsed_s` — duration in seconds. Finite and non-negative.
///
/// # Returns
///
/// Net displacement in metres, signed along the caller's axis.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `0.0`, i.e. the body did not move.
///
/// # Examples
///
/// ```
/// use physics_sandbox::displacement;
///
/// // Free fall from rest for 2 s: ½ · 9.81 · 4 = 19.62 m down.
/// assert_eq!(displacement(0.0, -9.81, 2.0), -19.62);
///
/// // Thrown up at 9.81 m/s, back where it started after 2 s.
/// assert_eq!(displacement(9.81, -9.81, 2.0), 0.0);
/// ```
#[inline]
pub fn displacement(initial_velocity_m_s: f64, acceleration_m_s2: f64, elapsed_s: f64) -> f64 {
    debug_assert!(
        initial_velocity_m_s.is_finite(),
        "displacement: initial_velocity_m_s must be finite, got {}",
        initial_velocity_m_s
    );
    debug_assert!(
        acceleration_m_s2.is_finite(),
        "displacement: acceleration_m_s2 must be finite, got {}",
        acceleration_m_s2
    );
    debug_assert!(
        elapsed_s.is_finite() && elapsed_s >= 0.0,
        "displacement: elapsed_s must be finite and non-negative, got {}",
        elapsed_s
    );

    if !initial_velocity_m_s.is_finite()
        || !acceleration_m_s2.is_finite()
        || !elapsed_s.is_finite()
        || elapsed_s < 0.0
    {
        return 0.0;
    }

    initial_velocity_m_s * elapsed_s + 0.5 * acceleration_m_s2 * (elapsed_s * elapsed_s)
}

/// Position after an interval: `x = x₀ + v₀·t + ½·a·t²`.
///
/// [`super::newtons_laws::inertial_position`] is the `a == 0.0` special case, and
/// the two agree exactly there — asserted as a property, since that identity is
/// the seam between Newton's first law and general kinematics.
///
/// # Parameters
///
/// - `initial_position_m` — starting position in metres. Finite.
/// - `initial_velocity_m_s` — starting velocity in metres per second. Finite.
/// - `acceleration_m_s2` — constant acceleration in metres per second squared.
///   Finite.
/// - `elapsed_s` — duration in seconds. Finite and non-negative.
///
/// # Returns
///
/// The position in metres at the end of the interval.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `initial_position_m` unchanged, so a bad input leaves the body where it was
/// rather than teleporting it to the origin.
///
/// # Examples
///
/// ```
/// use physics_sandbox::position_after;
///
/// // Dropped from rest at the origin, after 2 s of free fall.
/// assert_eq!(position_after(0.0, 0.0, -9.81, 2.0), -19.62);
///
/// // Zero time never moves anything.
/// assert_eq!(position_after(5.0, 3.0, -9.81, 0.0), 5.0);
/// ```
#[inline]
pub fn position_after(
    initial_position_m: f64,
    initial_velocity_m_s: f64,
    acceleration_m_s2: f64,
    elapsed_s: f64,
) -> f64 {
    debug_assert!(
        initial_position_m.is_finite(),
        "position_after: initial_position_m must be finite, got {}",
        initial_position_m
    );
    debug_assert!(
        initial_velocity_m_s.is_finite(),
        "position_after: initial_velocity_m_s must be finite, got {}",
        initial_velocity_m_s
    );
    debug_assert!(
        acceleration_m_s2.is_finite(),
        "position_after: acceleration_m_s2 must be finite, got {}",
        acceleration_m_s2
    );
    debug_assert!(
        elapsed_s.is_finite() && elapsed_s >= 0.0,
        "position_after: elapsed_s must be finite and non-negative, got {}",
        elapsed_s
    );

    if !initial_position_m.is_finite()
        || !initial_velocity_m_s.is_finite()
        || !acceleration_m_s2.is_finite()
        || !elapsed_s.is_finite()
        || elapsed_s < 0.0
    {
        return initial_position_m;
    }

    initial_position_m + displacement(initial_velocity_m_s, acceleration_m_s2, elapsed_s)
}

/// Speed after a given displacement, without knowing the time:
/// `|v| = √(v₀² + 2·a·s)`.
///
/// The "timeless" kinematic relation, obtained by eliminating `t`. Useful when
/// the question is about a place rather than a moment — impact speed at the
/// ground, muzzle speed at the end of a barrel.
///
/// # This returns a speed, not a velocity
///
/// The relation only determines `v²`, so its solution is `±√(…)` and the sign is
/// genuinely not recoverable from these three inputs: a body can arrive at the
/// same displacement travelling either way. The non-negative root is returned. For
/// motion that never reverses direction — the common case — the velocity is
/// `speed * displacement_m.signum()`. If the motion may reverse, use
/// [`time_to_reach_velocity`] and [`velocity_after`] instead, which do carry the
/// sign.
///
/// # Parameters
///
/// - `initial_velocity_m_s` — starting velocity in metres per second. Finite;
///   only its magnitude affects the result.
/// - `acceleration_m_s2` — constant acceleration in metres per second squared.
///   Finite.
/// - `displacement_m` — net displacement in metres, signed. Finite.
///
/// # Returns
///
/// `Some(speed)` in metres per second, always non-negative. `None` when
/// `v₀² + 2·a·s < 0`, meaning the displacement is never reached — asking how fast
/// a ball is moving above the top of its arc, for instance. `None` is also
/// returned in release builds when a precondition is violated.
///
/// # Preconditions and release behaviour
///
/// All three arguments finite. Violations fire a `debug_assert!`; in release the
/// function returns `None`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::speed_from_displacement;
///
/// // Dropped from rest, 19.62 m down at 9.81 m/s²: 19.62 m/s.
/// let speed = speed_from_displacement(0.0, -9.81, -19.62).unwrap();
/// assert!((speed - 19.62).abs() < 1.0e-12);
///
/// // Thrown up at 10 m/s, asked about a height it cannot reach.
/// assert_eq!(speed_from_displacement(10.0, -9.81, 100.0), None);
/// ```
#[inline]
pub fn speed_from_displacement(
    initial_velocity_m_s: f64,
    acceleration_m_s2: f64,
    displacement_m: f64,
) -> Option<f64> {
    debug_assert!(
        initial_velocity_m_s.is_finite(),
        "speed_from_displacement: initial_velocity_m_s must be finite, got {}",
        initial_velocity_m_s
    );
    debug_assert!(
        acceleration_m_s2.is_finite(),
        "speed_from_displacement: acceleration_m_s2 must be finite, got {}",
        acceleration_m_s2
    );
    debug_assert!(
        displacement_m.is_finite(),
        "speed_from_displacement: displacement_m must be finite, got {}",
        displacement_m
    );

    if !initial_velocity_m_s.is_finite()
        || !acceleration_m_s2.is_finite()
        || !displacement_m.is_finite()
    {
        return None;
    }

    let speed_squared =
        initial_velocity_m_s * initial_velocity_m_s + 2.0 * acceleration_m_s2 * displacement_m;

    if speed_squared < 0.0 {
        return None;
    }

    Some(speed_squared.sqrt())
}

/// Time to accelerate from one velocity to another: `t = (v₁ − v₀) / a`.
///
/// # Parameters
///
/// - `initial_velocity_m_s` — starting velocity in metres per second. Finite.
/// - `target_velocity_m_s` — velocity to reach, in metres per second. Finite.
/// - `acceleration_m_s2` — constant acceleration in metres per second squared.
///   Finite. Zero is a valid input, not an error — see below.
///
/// # Returns
///
/// `Some(t)` in seconds, always non-negative. `None` when the target is never
/// reached going forward in time:
///
/// - **Zero acceleration** and a different target: a coasting body never changes
///   speed. Returns `None` rather than dividing by zero. If the target equals the
///   current velocity, it is already there, so `Some(0.0)`.
/// - **Wrong direction**: accelerating away from the target — braking when asked
///   when you will speed up — puts the solution in the past. Returns `None`.
///
/// `None` is also returned in release builds when a precondition is violated.
///
/// # Preconditions and release behaviour
///
/// All three arguments finite. Violations fire a `debug_assert!`; in release the
/// function returns `None`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::time_to_reach_velocity;
///
/// // 0 to 60 m/s at 12 m/s²: 5 s.
/// assert_eq!(time_to_reach_velocity(0.0, 60.0, 12.0), Some(5.0));
///
/// // Already there.
/// assert_eq!(time_to_reach_velocity(5.0, 5.0, 0.0), Some(0.0));
///
/// // Coasting never gets there.
/// assert_eq!(time_to_reach_velocity(5.0, 9.0, 0.0), None);
///
/// // Accelerating the wrong way.
/// assert_eq!(time_to_reach_velocity(0.0, 10.0, -2.0), None);
/// ```
#[inline]
pub fn time_to_reach_velocity(
    initial_velocity_m_s: f64,
    target_velocity_m_s: f64,
    acceleration_m_s2: f64,
) -> Option<f64> {
    debug_assert!(
        initial_velocity_m_s.is_finite(),
        "time_to_reach_velocity: initial_velocity_m_s must be finite, got {}",
        initial_velocity_m_s
    );
    debug_assert!(
        target_velocity_m_s.is_finite(),
        "time_to_reach_velocity: target_velocity_m_s must be finite, got {}",
        target_velocity_m_s
    );
    debug_assert!(
        acceleration_m_s2.is_finite(),
        "time_to_reach_velocity: acceleration_m_s2 must be finite, got {}",
        acceleration_m_s2
    );

    if !initial_velocity_m_s.is_finite()
        || !target_velocity_m_s.is_finite()
        || !acceleration_m_s2.is_finite()
    {
        return None;
    }

    let velocity_change = target_velocity_m_s - initial_velocity_m_s;

    if velocity_change == 0.0 {
        // Already at the target, whatever the acceleration.
        return Some(0.0);
    }

    if acceleration_m_s2 == 0.0 {
        // Coasting: the velocity never changes, so the target is unreachable.
        return None;
    }

    let elapsed_s = velocity_change / acceleration_m_s2;

    // A negative time means the target lies in the past: the body is
    // accelerating away from it.
    if elapsed_s < 0.0 {
        return None;
    }

    Some(elapsed_s)
}

/// The position, velocity, and acceleration of a body along one axis.
///
/// The composable form of this module: rather than threading three numbers
/// through every call, hold one `MotionState` and [`advance`](Self::advance) it.
/// Every method delegates to the free functions above or to the other
/// `physics_utils` modules, so there is exactly one implementation of each
/// relation and no arithmetic to keep in sync.
///
/// This is a *value*, not a handle: [`advance`](Self::advance) returns a new
/// state rather than mutating in place, which keeps it usable in the pure,
/// GUI-free style the rest of the layer follows and makes before/after
/// comparisons — exactly what the conservation checks need — the natural thing to
/// write.
///
/// # Fields are public and unvalidated
///
/// All three fields are `pub` so a caller can build a state with a struct
/// literal, and nothing validates them on assignment. Use
/// [`is_valid`](Self::is_valid) where it matters; the methods `debug_assert!` it
/// themselves and degrade benignly in release. This mirrors
/// [`crate::core::scene`], where validation is a predicate rather than an
/// enforced invariant.
///
/// # Examples
///
/// ```
/// use physics_sandbox::MotionState;
///
/// // A body dropped from 100 m.
/// let start = MotionState {
///     position_m: 100.0,
///     velocity_m_s: 0.0,
///     acceleration_m_s2: -9.81,
/// };
///
/// let after_two_seconds = start.advance(2.0);
/// // 100 − ½·9.81·2² = 80.38 m, to within float rounding at this magnitude.
/// assert!((after_two_seconds.position_m - 80.38).abs() < 1.0e-12);
/// assert_eq!(after_two_seconds.velocity_m_s, -19.62);
///
/// // Acceleration is carried over unchanged: it is the caller's to update.
/// assert_eq!(after_two_seconds.acceleration_m_s2, -9.81);
/// ```
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MotionState {
    /// Position along the caller's axis, in metres.
    pub position_m: f64,
    /// Velocity along the caller's axis, in metres per second. Signed.
    pub velocity_m_s: f64,
    /// Acceleration along the caller's axis, in metres per second squared.
    /// Held constant by [`advance`](Self::advance); the caller re-derives it from
    /// forces each step.
    pub acceleration_m_s2: f64,
}

impl Default for MotionState {
    /// At rest at the origin, with no acceleration.
    fn default() -> Self {
        Self {
            position_m: 0.0,
            velocity_m_s: 0.0,
            acceleration_m_s2: 0.0,
        }
    }
}

impl MotionState {
    /// Construct a state from its three components.
    ///
    /// Equivalent to the struct literal; provided so callers can build one in
    /// expression position without naming the fields.
    #[inline]
    pub const fn new(position_m: f64, velocity_m_s: f64, acceleration_m_s2: f64) -> Self {
        Self {
            position_m,
            velocity_m_s,
            acceleration_m_s2,
        }
    }

    /// A body at rest at `position_m`, with no acceleration.
    #[inline]
    pub const fn at_rest(position_m: f64) -> Self {
        Self::new(position_m, 0.0, 0.0)
    }

    /// Are all three components finite?
    ///
    /// A pure predicate, in the style of `crate::core::scene`'s `is_valid`
    /// methods: nothing enforces it, and the methods here check it themselves.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.position_m.is_finite()
            && self.velocity_m_s.is_finite()
            && self.acceleration_m_s2.is_finite()
    }

    /// The body's speed: the magnitude of its velocity, in metres per second.
    #[inline]
    pub fn speed(&self) -> f64 {
        self.velocity_m_s.abs()
    }

    /// Advance the state by `dt_s`, holding acceleration constant.
    ///
    /// Position and velocity come from [`position_after`] and
    /// [`velocity_after`]; `acceleration_m_s2` is copied over unchanged, because
    /// deciding the next step's acceleration means summing forces, which is the
    /// caller's job (see [`super::newtons_laws::acceleration_from_net_force`]).
    ///
    /// Because both relations are exact analytic solutions for constant
    /// acceleration, advancing in many small steps agrees with one large step up
    /// to float rounding — asserted as a property. Step size therefore affects
    /// accuracy only through how well constant acceleration models the caller's
    /// actual forces.
    ///
    /// # Parameters
    ///
    /// - `dt_s` — duration in seconds. Finite and non-negative, matching
    ///   `Scene::step`.
    ///
    /// # Returns
    ///
    /// The new state. Returns `*self` unchanged when `dt_s` is `0.0`, and also in
    /// release builds when `self` is invalid or `dt_s` violates its
    /// precondition — leaving the body where it was rather than resetting it.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::MotionState;
    ///
    /// let coasting = MotionState::new(0.0, 3.0, 0.0);
    /// assert_eq!(coasting.advance(2.0), MotionState::new(6.0, 3.0, 0.0));
    ///
    /// // Zero time is a no-op.
    /// assert_eq!(coasting.advance(0.0), coasting);
    /// ```
    #[inline]
    pub fn advance(&self, dt_s: f64) -> Self {
        debug_assert!(
            self.is_valid(),
            "MotionState::advance: every component must be finite, got {:?}",
            self
        );
        debug_assert!(
            dt_s.is_finite() && dt_s >= 0.0,
            "MotionState::advance: dt_s must be finite and non-negative, got {}",
            dt_s
        );

        if !self.is_valid() || !dt_s.is_finite() || dt_s < 0.0 {
            return *self;
        }

        Self {
            position_m: position_after(
                self.position_m,
                self.velocity_m_s,
                self.acceleration_m_s2,
                dt_s,
            ),
            velocity_m_s: velocity_after(self.velocity_m_s, self.acceleration_m_s2, dt_s),
            acceleration_m_s2: self.acceleration_m_s2,
        }
    }

    /// The body's linear momentum for a given mass, in kilogram-metres per
    /// second.
    ///
    /// Delegates to [`super::conservation_laws::momentum`]. Mass is a parameter
    /// rather than a field because a `MotionState` describes motion, not a body —
    /// the same trajectory can be shared by bodies of different mass, and
    /// `Scene`'s bodies own their own mass.
    #[inline]
    pub fn momentum(&self, mass_kg: f64) -> f64 {
        momentum(mass_kg, self.velocity_m_s)
    }

    /// The body's kinetic energy for a given mass, in joules.
    ///
    /// Delegates to [`super::conservation_laws::kinetic_energy`].
    #[inline]
    pub fn kinetic_energy(&self, mass_kg: f64) -> f64 {
        kinetic_energy(mass_kg, self.velocity_m_s)
    }

    /// The net force a given mass must be under to have this acceleration, in
    /// newtons.
    ///
    /// Delegates to [`super::newtons_laws::net_force`] — Newton's second law read
    /// backwards, from an observed acceleration to the force responsible.
    #[inline]
    pub fn net_force(&self, mass_kg: f64) -> f64 {
        net_force(mass_kg, self.acceleration_m_s2)
    }
}

#[cfg(test)]
#[path = "../../tst/physics_utils/kinematics_tests.rs"]
mod tests;
