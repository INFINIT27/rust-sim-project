//! Closed-form physics formulas.
//!
//! Every function here is pure and total: it never allocates, never mutates
//! global state, and never panics in a release build. Preconditions are
//! documented and checked with `debug_assert!`, matching the convention
//! [`crate::core::scene`] uses — a debug build fails loudly on nonsense
//! input, a release build degrades to a benign value instead of aborting the
//! frame loop.

use super::physics_constants::GRAVITATIONAL_CONSTANT;

/// Calculate the gravitational attraction between two point masses.
///
/// Despite the name (kept to match the requested API), the value returned is
/// a **force in newtons**, not a constant: the gravitational constant itself
/// is [`GRAVITATIONAL_CONSTANT`]. This function evaluates Newton's law of
/// universal gravitation,
///
/// ```text
/// F = G * m_a * m_b / r²
/// ```
///
/// which is the magnitude of the attractive force each mass exerts on the
/// other. The force is mutual and equal in magnitude on both bodies
/// (Newton's third law), so there is a single return value rather than one
/// per mass, and it carries no direction — the caller supplies the geometry.
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — the two masses, in kilograms. Must be finite
///   and non-negative.
/// - `distance_m` — the separation between their centres of mass, in metres.
///   Must be finite and strictly positive.
///
/// # Returns
///
/// The force magnitude in newtons: always finite and non-negative for valid
/// input. Returns `0.0` when either mass is zero (no mass, no attraction).
///
/// # Preconditions and release behaviour
///
/// The precondition is `mass_a_kg >= 0.0`, `mass_b_kg >= 0.0`, `distance_m >
/// 0.0`, and all three finite. Violations fire a descriptive `debug_assert!`
/// in debug builds; in release builds the function returns `0.0` rather than
/// propagating `NaN`/`inf` into a caller's simulation state. `distance_m ==
/// 0.0` is rejected rather than returning infinity, because coincident point
/// masses are a modelling error, not a physical singularity worth
/// representing.
///
/// # Accuracy and evaluation order
///
/// The expression is grouped as `G * ((m_a / r) * (m_b / r))` rather than the
/// literal `G * m_a * m_b / (r * r)`. The two are algebraically identical, but
/// the grouping buys two properties that floating point does not give for
/// free:
///
/// - **No spurious overflow.** Each mass is scaled down by `r` *before* the
///   two are multiplied, so the intermediate stays near the magnitude of the
///   result. The naive form builds `m_a * m_b` first, which is `~1e55` for two
///   stellar masses and would overflow `f32` outright.
/// - **Exact symmetry in the two masses.** IEEE-754 multiplication is
///   commutative, so forming `(m_a / r) * (m_b / r)` before touching `G` makes
///   swapping the arguments bit-for-bit identical. Folding `G` into one side
///   first — `(G * m_a / r) * (m_b / r)` — does *not*: that rounds `G * m_a`
///   against whichever mass it lands on and disagrees by 1 ULP for lopsided
///   pairs, which is exactly what the symmetry property test caught.
///
/// Expect roughly five significant figures overall, limited by the
/// uncertainty in `G` itself rather than by `f64`.
///
/// # Examples
///
/// Two one-kilogram masses one metre apart attract with exactly `G` newtons:
///
/// ```
/// use physics_sandbox::{gravitational_constant_calculator, GRAVITATIONAL_CONSTANT};
///
/// let force = gravitational_constant_calculator(1.0, 1.0, 1.0);
/// assert_eq!(force, GRAVITATIONAL_CONSTANT);
/// ```
///
/// The Earth–Sun attraction is about `3.54e22` N:
///
/// ```
/// use physics_sandbox::gravitational_constant_calculator;
///
/// let force = gravitational_constant_calculator(5.972e24, 1.989e30, 1.496e11);
/// assert!((force - 3.54e22).abs() / 3.54e22 < 0.01);
/// ```
#[inline]
pub fn gravitational_constant_calculator(mass_a_kg: f64, mass_b_kg: f64, distance_m: f64) -> f64 {
    debug_assert!(
        mass_a_kg.is_finite() && mass_a_kg >= 0.0,
        "gravitational_constant_calculator: mass_a_kg must be finite and non-negative, got {}",
        mass_a_kg
    );
    debug_assert!(
        mass_b_kg.is_finite() && mass_b_kg >= 0.0,
        "gravitational_constant_calculator: mass_b_kg must be finite and non-negative, got {}",
        mass_b_kg
    );
    debug_assert!(
        distance_m.is_finite() && distance_m > 0.0,
        "gravitational_constant_calculator: distance_m must be finite and strictly positive, got {}",
        distance_m
    );

    // Release-build fallback for a violated precondition. The `is_finite`
    // tests come first so `NaN` — which fails every ordering comparison —
    // lands here rather than slipping through the sign checks, the same
    // ordering `crate::render::plugin::clamp_dt` relies on.
    if !mass_a_kg.is_finite()
        || !mass_b_kg.is_finite()
        || !distance_m.is_finite()
        || mass_a_kg < 0.0
        || mass_b_kg < 0.0
        || distance_m <= 0.0
    {
        return 0.0;
    }

    GRAVITATIONAL_CONSTANT * ((mass_a_kg / distance_m) * (mass_b_kg / distance_m))
}

#[cfg(test)]
#[path = "../../tst/physics_utils/physics_formulas_tests.rs"]
mod tests;
