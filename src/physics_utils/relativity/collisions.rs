//! Relativistic collisions along one axis.
//!
//! The counterpart of the parent layer's `elastic_collision_1d` and
//! `perfectly_inelastic_collision_1d` in
//! [`conservation_laws`](crate::physics_utils::conservation_laws). The conserved quantities are the
//! relativistic ones:
//!
//! ```text
//! Σ γmv    momentum, always
//! Σ γmc²   total energy, always
//! ```
//!
//! # What is different from the Newtonian solvers
//!
//! - **Energy is conserved in every collision, including a perfectly inelastic one.** Newtonian
//!   mechanics says kinetic energy is "lost" when two bodies stick. Here it is not lost: it is in the
//!   merged body, as mass. Two `1 kg` lumps meeting at `±0.6c` make a `2.5 kg` lump.
//!   [`perfectly_inelastic_collision_1d`] therefore returns a mass as well as a velocity.
//! - **"Elastic" means the masses do not change**, which is the same as saying kinetic energy is
//!   conserved separately.
//! - **The answer is the same whichever frame solves it.** Boost the bodies, collide them, and you
//!   get the boost of the collided bodies. The Newtonian solvers fail that test under a relativistic
//!   boost, which is the reason `p = mv` had to go (see [`momentum`](super::momentum)).
//!
//! # How the solvers work
//!
//! Both go through the **centre-of-momentum frame**, the frame in which the total momentum is zero.
//! Its velocity is `c²·Σp/ΣE` ([`centre_of_momentum_velocity`]). In that frame an elastic collision
//! on one axis simply reverses each body's velocity, and a perfectly inelastic one leaves the merged
//! body at rest. Transforming back gives the result, with no quadratic to solve and no wrong root to
//! pick.
//!
//! A body is a `(mass_kg, velocity_m_s)` tuple and a system is a slice of them, as in the parent
//! layer.

use super::constants::SPEED_OF_LIGHT_M_S;
use super::lorentz::lorentz_factor;
use super::velocity::{compose_velocities, transform_velocity};

/// `(Σγm, Σγmβ)` for a system: total energy over `c²` and total momentum over `c`, both in
/// kilograms. `None` for an empty system, a non-positive mass, or a speed not below `c`.
fn totals(bodies: &[(f64, f64)]) -> Option<(f64, f64)> {
    if bodies.is_empty() {
        return None;
    }

    let mut energy_kg = 0.0;
    let mut momentum_kg = 0.0;

    for &(mass_kg, velocity_m_s) in bodies {
        if !mass_kg.is_finite() || mass_kg <= 0.0 {
            return None;
        }

        let weighted = lorentz_factor(velocity_m_s)? * mass_kg;
        energy_kg += weighted;
        momentum_kg += weighted * (velocity_m_s / SPEED_OF_LIGHT_M_S);
    }

    (energy_kg.is_finite() && momentum_kg.is_finite()).then_some((energy_kg, momentum_kg))
}

/// Total relativistic momentum `Σγmv` of a system, in `kg·m/s`.
///
/// # Returns
///
/// `None` for an empty system, a mass that is not positive and finite, or a speed not below `c`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::collisions::total_relativistic_momentum;
///
/// let v = 0.6 * SPEED_OF_LIGHT_M_S;
///
/// // Equal and opposite: nothing.
/// assert_eq!(total_relativistic_momentum(&[(1.0, v), (1.0, -v)]), Some(0.0));
/// ```
pub fn total_relativistic_momentum(bodies: &[(f64, f64)]) -> Option<f64> {
    let (_, momentum_kg) = totals(bodies)?;
    let momentum = momentum_kg * SPEED_OF_LIGHT_M_S;

    momentum.is_finite().then_some(momentum)
}

/// Total energy `Σγmc²` of a system, rest energy included, in joules.
///
/// # Returns
///
/// `None` on the same domain as [`total_relativistic_momentum`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::collisions::total_relativistic_energy;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Two 1 kg bodies at +-0.6c: gamma is 1.25 each.
/// let energy = total_relativistic_energy(&[(1.0, 0.6 * c), (1.0, -0.6 * c)]).unwrap();
/// assert!((energy / (c * c) - 2.5).abs() < 1.0e-12);
/// ```
pub fn total_relativistic_energy(bodies: &[(f64, f64)]) -> Option<f64> {
    let (energy_kg, _) = totals(bodies)?;
    let energy = energy_kg * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S;

    energy.is_finite().then_some(energy)
}

/// The velocity of the frame in which a system's total momentum is zero: `c²·Σp/ΣE`.
///
/// The relativistic centre of mass velocity. Each body is weighted by `γm`, not by `m`, so a fast
/// light body counts for more than its mass suggests.
///
/// # Returns
///
/// Strictly subluminal. `None` on the same domain as [`total_relativistic_momentum`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::collisions::centre_of_momentum_velocity;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // A 1 kg body at 0.8c (gamma = 5/3) hitting a 1 kg body at rest. Newton says 0.4c.
/// let v = centre_of_momentum_velocity(&[(1.0, 0.8 * c), (1.0, 0.0)]).unwrap();
/// assert!((v / c - 0.5).abs() < 1.0e-12);
/// ```
pub fn centre_of_momentum_velocity(bodies: &[(f64, f64)]) -> Option<f64> {
    let (energy_kg, momentum_kg) = totals(bodies)?;
    let velocity = SPEED_OF_LIGHT_M_S * (momentum_kg / energy_kg);

    // Every body is subluminal, so the weighted mean is too. The check is against rounding only.
    (velocity.is_finite() && velocity.abs() < SPEED_OF_LIGHT_M_S).then_some(velocity)
}

/// The mass of a system taken as a whole: `√((ΣE)² − (Σp·c)²)/c²`.
///
/// At least the sum of the bodies' masses, and equal to it only when they all move together. The
/// excess is the kinetic energy available in the centre-of-momentum frame, over `c²`, and it is the
/// most a collision can turn into new mass. This is why colliders fire two beams at each other
/// rather than one at a fixed target.
///
/// # Returns
///
/// Kilograms. `None` on the same domain as [`total_relativistic_momentum`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::collisions::system_invariant_mass;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Head on at +-0.6c: all of both gammas counts.
/// let mass = system_invariant_mass(&[(1.0, 0.6 * c), (1.0, -0.6 * c)]).unwrap();
/// assert!((mass - 2.5).abs() < 1.0e-12);
///
/// // Moving together, the system's mass is just the sum.
/// let mass = system_invariant_mass(&[(1.0, 0.6 * c), (2.0, 0.6 * c)]).unwrap();
/// assert!((mass - 3.0).abs() < 1.0e-12);
/// ```
pub fn system_invariant_mass(bodies: &[(f64, f64)]) -> Option<f64> {
    let (energy_kg, momentum_kg) = totals(bodies)?;
    let momentum_kg = momentum_kg.abs();

    // `|Σγmβ| < Σγm` in exact arithmetic; `max` guards the last bit.
    let squared = (energy_kg - momentum_kg) * (energy_kg + momentum_kg);
    let mass = squared.max(0.0).sqrt();

    mass.is_finite().then_some(mass)
}

/// The velocities of two bodies after a head-on elastic collision.
///
/// Conserves `Σγmv` and `Σγmc²` with both masses unchanged. Of the two solutions of those equations
/// this returns the one in which the bodies actually collide (the other is "they pass through each
/// other", which is the input).
///
/// As in the Newtonian case, equal masses exchange velocities.
///
/// # Returns
///
/// `(velocity_a, velocity_b)` after the collision, both strictly subluminal. `None` if a mass is not
/// positive and finite or a speed is not below `c`. Unlike the parent layer's solver this does not
/// return the inputs for a bad argument; see the sub-layer's module docs on `None`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::collisions::elastic_collision_1d;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Equal masses swap velocities.
/// let (a, b) = elastic_collision_1d(1.0, 0.8 * c, 1.0, 0.0).unwrap();
/// assert!(a.abs() < 1.0e-6);
/// assert!((b / c - 0.8).abs() < 1.0e-12);
///
/// // A light body off a heavy one at rest bounces back at nearly its own speed.
/// let (a, _) = elastic_collision_1d(1.0, 0.5 * c, 1.0e12, 0.0).unwrap();
/// assert!((a / c + 0.5).abs() < 1.0e-9);
/// ```
pub fn elastic_collision_1d(
    mass_a_kg: f64,
    velocity_a_m_s: f64,
    mass_b_kg: f64,
    velocity_b_m_s: f64,
) -> Option<(f64, f64)> {
    let frame =
        centre_of_momentum_velocity(&[(mass_a_kg, velocity_a_m_s), (mass_b_kg, velocity_b_m_s)])?;

    // Into the centre-of-momentum frame, reverse, and back out.
    let bounce = |velocity_m_s: f64| {
        let inside = transform_velocity(velocity_m_s, frame)?;
        compose_velocities(-inside, frame)
    };

    let after_a = bounce(velocity_a_m_s)?;
    let after_b = bounce(velocity_b_m_s)?;

    (after_a.abs() < SPEED_OF_LIGHT_M_S && after_b.abs() < SPEED_OF_LIGHT_M_S)
        .then_some((after_a, after_b))
}

/// The single body left when two bodies collide and stick: its **mass** and its velocity.
///
/// The velocity is the centre-of-momentum velocity. The mass is the system's invariant mass, which
/// is **more than `mass_a_kg + mass_b_kg`** unless the two were already moving together: the kinetic
/// energy the Newtonian solver writes off as lost is here, as heat, and heat has mass.
///
/// # Returns
///
/// `(mass_kg, velocity_m_s)`. `None` on the same domain as [`elastic_collision_1d`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::collisions::perfectly_inelastic_collision_1d;
///
/// let c = SPEED_OF_LIGHT_M_S;
///
/// // Two 1 kg lumps at +-0.6c stop dead and weigh 2.5 kg.
/// let (mass, velocity) = perfectly_inelastic_collision_1d(1.0, 0.6 * c, 1.0, -0.6 * c).unwrap();
/// assert!((mass - 2.5).abs() < 1.0e-12);
/// assert_eq!(velocity, 0.0);
/// ```
pub fn perfectly_inelastic_collision_1d(
    mass_a_kg: f64,
    velocity_a_m_s: f64,
    mass_b_kg: f64,
    velocity_b_m_s: f64,
) -> Option<(f64, f64)> {
    let bodies = [(mass_a_kg, velocity_a_m_s), (mass_b_kg, velocity_b_m_s)];

    Some((
        system_invariant_mass(&bodies)?,
        centre_of_momentum_velocity(&bodies)?,
    ))
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/collisions_tests.rs"]
mod tests;
