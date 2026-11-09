//! Newton's three laws of motion.
//!
//! All quantities are scalars along a single caller-chosen axis, in SI units,
//! with sign carrying direction — the same magnitude-only convention
//! [`super::physics_formulas::gravitational_constant_calculator`] uses. A
//! caller working in 3-D applies these componentwise.
//!
//! Every function is pure and total: preconditions are checked with
//! `debug_assert!` and a release build degrades to a documented benign value
//! rather than aborting the frame loop, matching
//! [`crate::core::scene`]'s convention.
//!
//! # How the three laws map onto this module
//!
//! | Law | Statement | Here |
//! | --- | --- | --- |
//! | First | A body under no net force keeps a constant velocity. | [`inertial_position`] |
//! | Second | `F = m·a`. | [`net_force`], [`acceleration_from_net_force`] |
//! | Third | Every action has an equal and opposite reaction. | [`reaction_force`] |
//!
//! The first law's *velocity* claim has no function of its own on purpose:
//! "velocity does not change" is represented by the absence of any velocity
//! update, and it falls out of the second law, since
//! `acceleration_from_net_force(0.0, m) == 0.0`. What the first law does buy us
//! computationally is the trajectory in [`inertial_position`].

/// Position of a body moving under no net force, after `elapsed_s`.
///
/// **Newton's first law.** With zero net force the velocity is constant, so the
/// trajectory is a straight line at uniform speed:
///
/// ```text
/// x(t) = x₀ + v·t
/// ```
///
/// This is exact, not an integration step: with no acceleration there is no
/// truncation error to accumulate, so a single call over a long `elapsed_s`
/// gives the same answer as many small steps (up to float rounding). A caller
/// under nonzero force must not use this; integrate the acceleration instead.
///
/// # Parameters
///
/// - `initial_position_m` — starting position in metres. Any finite value.
/// - `velocity_m_s` — constant velocity in metres per second. Any finite value;
///   the sign is the direction along the caller's axis.
/// - `elapsed_s` — elapsed time in seconds. Must be finite and non-negative,
///   matching `Scene::step`'s `dt` contract.
///
/// # Returns
///
/// The new position in metres. Returns `initial_position_m` unchanged when
/// `elapsed_s` is `0.0`.
///
/// # Preconditions and release behaviour
///
/// All three arguments finite, and `elapsed_s >= 0.0`. Violations fire a
/// `debug_assert!`; in release the function returns `initial_position_m`
/// unchanged, which is the only fallback that cannot teleport a body.
///
/// # Examples
///
/// ```
/// use physics_sandbox::inertial_position;
///
/// // 3 m/s for 2 s from the origin.
/// assert_eq!(inertial_position(0.0, 3.0, 2.0), 6.0);
///
/// // Negative velocity moves the other way along the axis.
/// assert_eq!(inertial_position(10.0, -4.0, 0.5), 8.0);
/// ```
#[inline]
pub fn inertial_position(initial_position_m: f64, velocity_m_s: f64, elapsed_s: f64) -> f64 {
    debug_assert!(
        initial_position_m.is_finite(),
        "inertial_position: initial_position_m must be finite, got {}",
        initial_position_m
    );
    debug_assert!(
        velocity_m_s.is_finite(),
        "inertial_position: velocity_m_s must be finite, got {}",
        velocity_m_s
    );
    debug_assert!(
        elapsed_s.is_finite() && elapsed_s >= 0.0,
        "inertial_position: elapsed_s must be finite and non-negative, got {}",
        elapsed_s
    );

    if !initial_position_m.is_finite()
        || !velocity_m_s.is_finite()
        || !elapsed_s.is_finite()
        || elapsed_s < 0.0
    {
        return initial_position_m;
    }

    initial_position_m + velocity_m_s * elapsed_s
}

/// The net force required to give a mass a given acceleration.
///
/// **Newton's second law**, in the form `F = m·a`.
///
/// # Parameters
///
/// - `mass_kg` — mass in kilograms. Must be finite and non-negative. Zero is
///   allowed and yields zero force: accelerating nothing costs nothing.
/// - `acceleration_m_s2` — acceleration in metres per second squared. Any
///   finite value; the sign is the direction along the caller's axis.
///
/// # Returns
///
/// The net force in newtons, with the same sign as `acceleration_m_s2`.
///
/// # Preconditions and release behaviour
///
/// Both arguments finite and `mass_kg >= 0.0`. Violations fire a
/// `debug_assert!`; in release the function returns `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::net_force;
///
/// // 2 kg at 3 m/s² needs 6 N.
/// assert_eq!(net_force(2.0, 3.0), 6.0);
///
/// // The first law as a special case: no acceleration, no force.
/// assert_eq!(net_force(2.0, 0.0), 0.0);
/// ```
#[inline]
pub fn net_force(mass_kg: f64, acceleration_m_s2: f64) -> f64 {
    debug_assert!(
        mass_kg.is_finite() && mass_kg >= 0.0,
        "net_force: mass_kg must be finite and non-negative, got {}",
        mass_kg
    );
    debug_assert!(
        acceleration_m_s2.is_finite(),
        "net_force: acceleration_m_s2 must be finite, got {}",
        acceleration_m_s2
    );

    if !mass_kg.is_finite() || mass_kg < 0.0 || !acceleration_m_s2.is_finite() {
        return 0.0;
    }

    mass_kg * acceleration_m_s2
}

/// The acceleration a net force produces on a mass.
///
/// **Newton's second law**, rearranged to `a = F / m`. This is the form an
/// integrator calls: forces are what the simulation accumulates, acceleration
/// is what it needs.
///
/// # Parameters
///
/// - `net_force_n` — the net force in newtons. Any finite value.
/// - `mass_kg` — mass in kilograms. Must be finite and **strictly positive**.
///   Unlike [`net_force`], zero is rejected here: `F / 0` has no physical
///   answer, and a massless body being pushed is a modelling error rather than
///   a body with infinite acceleration.
///
/// # Returns
///
/// The acceleration in metres per second squared, with the same sign as
/// `net_force_n`.
///
/// # Preconditions and release behaviour
///
/// Both arguments finite and `mass_kg > 0.0`. Violations fire a
/// `debug_assert!`; in release the function returns `0.0`, so a bad mass leaves
/// the body coasting rather than launching it to infinity.
///
/// # Examples
///
/// ```
/// use physics_sandbox::acceleration_from_net_force;
///
/// // 6 N on 2 kg gives 3 m/s².
/// assert_eq!(acceleration_from_net_force(6.0, 2.0), 3.0);
///
/// // Newton's first law: no net force, no acceleration.
/// assert_eq!(acceleration_from_net_force(0.0, 2.0), 0.0);
/// ```
#[inline]
pub fn acceleration_from_net_force(net_force_n: f64, mass_kg: f64) -> f64 {
    debug_assert!(
        net_force_n.is_finite(),
        "acceleration_from_net_force: net_force_n must be finite, got {}",
        net_force_n
    );
    debug_assert!(
        mass_kg.is_finite() && mass_kg > 0.0,
        "acceleration_from_net_force: mass_kg must be finite and strictly positive, got {}",
        mass_kg
    );

    if !net_force_n.is_finite() || !mass_kg.is_finite() || mass_kg <= 0.0 {
        return 0.0;
    }

    net_force_n / mass_kg
}

/// The reaction to an action force: equal in magnitude, opposite in direction.
///
/// **Newton's third law.** If body A pushes body B with `action_force_n`, then B
/// pushes A with the negation. Along a single axis that is exactly a sign flip,
/// which is why this function is one line — the content of the law is the
/// *pairing*, not the arithmetic, and the value of having it as a named function
/// is that a caller applying a force to one body has an obvious, greppable
/// reminder to apply the counterpart to the other.
///
/// Note what the law does *not* say: the two forces act on **different bodies**,
/// so they never cancel in a single body's net force. They cancel only in the
/// total for a closed system — which is precisely why momentum is conserved
/// (see [`super::conservation_laws::momentum_is_conserved`]).
///
/// # Parameters
///
/// - `action_force_n` — the action force in newtons. Any finite value.
///
/// # Returns
///
/// The reaction force in newtons: `-action_force_n`. Note that `0.0` maps to
/// `-0.0`, which compares equal to `0.0` under `==`.
///
/// # Preconditions and release behaviour
///
/// `action_force_n` must be finite. A violation fires a `debug_assert!`; in
/// release the function returns `0.0`, because the only safe reaction to an
/// unrepresentable action is none.
///
/// # Examples
///
/// ```
/// use physics_sandbox::reaction_force;
///
/// assert_eq!(reaction_force(5.0), -5.0);
/// assert_eq!(reaction_force(-2.5), 2.5);
///
/// // Equal and opposite: the pair always sums to zero.
/// let action = 12.75;
/// assert_eq!(action + reaction_force(action), 0.0);
/// ```
#[inline]
pub fn reaction_force(action_force_n: f64) -> f64 {
    debug_assert!(
        action_force_n.is_finite(),
        "reaction_force: action_force_n must be finite, got {}",
        action_force_n
    );

    if !action_force_n.is_finite() {
        return 0.0;
    }

    -action_force_n
}

#[cfg(test)]
#[path = "../../tst/physics_utils/newtons_laws_tests.rs"]
mod tests;
