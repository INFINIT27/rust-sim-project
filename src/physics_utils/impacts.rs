//! Planetary impacts: when two bodies held together by their own gravity collide, what is left.
//!
//! The collision solvers in [`super::conservation_laws`] answer how two bodies bounce. For two
//! planets the more important question is whether anything bounces at all. A planet is held
//! together by its gravity, not by the strength of its rock, so two planets that meet at a few
//! km/s melt, merge, and throw off debris. These functions are the measured scaling laws for that.
//!
//! # The quantities
//!
//! - [`mutual_escape_speed`]: the speed two bodies need to separate to infinity from contact. Two
//!   bodies that fall onto each other from rest meet at nearly this speed, and a pair that leaves
//!   contact slower than it is still bound and falls back together.
//! - [`specific_impact_energy`]: `Q_R`, the impact's kinetic energy in the centre-of-mass frame,
//!   per kilogram of the pair.
//! - [`catastrophic_disruption_threshold`]: `Q*_RD`, the `Q_R` that leaves half the mass in the
//!   largest remnant and disperses the rest.
//! - [`largest_remnant_fraction`]: the share of the total mass in the largest surviving body, from
//!   `Q_R / Q*_RD`. What is not in it is debris.
//!
//! # Sources
//!
//! Leinhardt & Stewart (2012), "Collisions between gravity-dominated bodies. I. Outcome regimes and
//! scaling laws", ApJ 745:79, and Stewart & Leinhardt (2012), ApJ 751:32, for planet-sized bodies.
//! Their law fits hundreds of numerical simulations of collisions from rubble piles to planets.
//!
//! # What is left out
//!
//! The angle. Leinhardt & Stewart reduce an oblique impact's energy to the share of the projectile
//! that actually strikes the target. These functions take the full head-on energy, so an oblique
//! impact here is treated as more destructive than it is. Hit-and-run (a grazing impact where both
//! bodies survive) is not decided here; the collision layer finds it from the speed the pair leaves
//! contact at.

use super::physics_constants::{
    DISRUPTION_COEFFICIENT, DISRUPTION_REFERENCE_DENSITY_KG_M3, DISRUPTION_VELOCITY_EXPONENT,
    GRAVITATIONAL_CONSTANT, SUPERCATASTROPHIC_EXPONENT, SUPERCATASTROPHIC_ONSET,
};

/// The speed two bodies in contact need to escape each other, in m/s.
///
/// ```text
/// v_esc = √( 2·G·(m₁ + m₂) / (R₁ + R₂) )
/// ```
///
/// From energy conservation for the pair's relative motion: the relative kinetic energy
/// `½·μ·v²` must pay the binding `G·m₁·m₂ / (R₁ + R₂)`, and the reduced mass `μ` cancels.
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — masses, kg. Finite and non-negative, not both zero.
/// - `radius_a_m`, `radius_b_m` — radii, m. Finite and non-negative, not both zero.
///
/// # Returns
///
/// The escape speed, m/s. In release, invalid inputs return `0.0`: nothing is bound.
///
/// # Examples
///
/// ```
/// use physics_sandbox::mutual_escape_speed;
///
/// // The Earth and the Moon touching: about 10 km/s.
/// let v = mutual_escape_speed(5.9722e24, 7.342e22, 6.371e6, 1.7374e6);
/// assert!((v - 9_976.0).abs() < 10.0);
/// ```
#[inline]
pub fn mutual_escape_speed(
    mass_a_kg: f64,
    mass_b_kg: f64,
    radius_a_m: f64,
    radius_b_m: f64,
) -> f64 {
    let total_mass = mass_a_kg + mass_b_kg;
    let contact = radius_a_m + radius_b_m;
    let valid = mass_a_kg >= 0.0
        && mass_b_kg >= 0.0
        && radius_a_m >= 0.0
        && radius_b_m >= 0.0
        && total_mass.is_finite()
        && contact.is_finite()
        && total_mass > 0.0
        && contact > 0.0;
    debug_assert!(
        valid,
        "mutual_escape_speed: need finite non-negative masses and radii, not all zero; got m = ({mass_a_kg}, {mass_b_kg}), R = ({radius_a_m}, {radius_b_m})"
    );
    if !valid {
        return 0.0;
    }

    (2.0 * GRAVITATIONAL_CONSTANT * total_mass / contact).sqrt()
}

/// `Q_R`: the impact's kinetic energy in the centre-of-mass frame, per kilogram of the pair, in J/kg.
///
/// ```text
/// Q_R = ½ · μ · v² / (m₁ + m₂),    μ = m₁·m₂ / (m₁ + m₂)
/// ```
///
/// Only the relative motion can do damage, which is why the reduced mass appears; dividing by the
/// total mass makes it a figure that can be compared across bodies of any size.
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — kg, finite and strictly positive.
/// - `impact_speed_m_s` — the relative speed at contact, m/s. Its sign does not matter.
///
/// # Returns
///
/// `Q_R` in J/kg, `≥ 0`. In release, invalid inputs return `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::specific_impact_energy;
///
/// // Two equal bodies at 2 m/s: μ = m/2, so Q_R = ½·(m/2)·4 / 2m = 0.5 J/kg.
/// assert!((specific_impact_energy(3.0, 3.0, 2.0) - 0.5).abs() < 1e-12);
/// ```
#[inline]
pub fn specific_impact_energy(mass_a_kg: f64, mass_b_kg: f64, impact_speed_m_s: f64) -> f64 {
    let total_mass = mass_a_kg + mass_b_kg;
    let valid = mass_a_kg > 0.0
        && mass_b_kg > 0.0
        && total_mass.is_finite()
        && impact_speed_m_s.is_finite();
    debug_assert!(
        valid,
        "specific_impact_energy: need finite positive masses and a finite speed; got m = ({mass_a_kg}, {mass_b_kg}), v = {impact_speed_m_s}"
    );
    if !valid {
        return 0.0;
    }

    // Grouped as mass fractions so astronomical masses never multiply together.
    let reduced_fraction = (mass_a_kg / total_mass) * (mass_b_kg / total_mass);
    0.5 * reduced_fraction * impact_speed_m_s * impact_speed_m_s
}

/// `Q*_RD`: the specific impact energy that leaves half the total mass in the largest remnant, in
/// J/kg, for a collision between gravity-dominated bodies.
///
/// Leinhardt & Stewart (2012, ApJ 745:79), eqs. 28 and 23:
///
/// ```text
/// Q*_RD,γ=1 = c* · (4/5)·π·ρ₁·G · R_C1²                  equal masses
/// Q*_RD     = Q*_RD,γ=1 · ( (γ + 1)² / (4γ) )^(2/(3μ̄) − 1)    γ = m_small / m_large
/// ```
///
/// where `R_C1` is the radius of the **combined** mass at the reference density
/// `ρ₁ = 1000 kg/m³`. The first line is the gravitational binding of the combined body, which is why
/// it grows as the square of its size: big bodies are much harder to disperse. The second raises the
/// threshold for unequal pairs, because a small projectile couples its energy into a large target
/// less efficiently than an equal partner does.
///
/// The constants are the planet-sized values: [`DISRUPTION_COEFFICIENT`] `c* = 5` and
/// [`DISRUPTION_VELOCITY_EXPONENT`] `μ̄ = 1/3`, from Stewart & Leinhardt (2012, ApJ 751:32).
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — kg, finite and strictly positive. Order does not matter.
///
/// # Returns
///
/// `Q*_RD` in J/kg, `> 0`. In release, invalid inputs return `f64::INFINITY`: an impact that cannot
/// be judged disperses nothing.
///
/// # Examples
///
/// ```
/// use physics_sandbox::catastrophic_disruption_threshold;
///
/// // The Earth struck by a Mars-mass body: about 3.2e8 J/kg.
/// let q = catastrophic_disruption_threshold(5.9722e24, 6.4171e23);
/// assert!((q / 3.24e8 - 1.0).abs() < 0.01);
/// ```
pub fn catastrophic_disruption_threshold(mass_a_kg: f64, mass_b_kg: f64) -> f64 {
    let total_mass = mass_a_kg + mass_b_kg;
    let valid = mass_a_kg > 0.0 && mass_b_kg > 0.0 && total_mass.is_finite();
    debug_assert!(
        valid,
        "catastrophic_disruption_threshold: need finite positive masses, got ({mass_a_kg}, {mass_b_kg})"
    );
    if !valid {
        return f64::INFINITY;
    }

    let combined_radius_m = (3.0 * total_mass
        / (4.0 * std::f64::consts::PI * DISRUPTION_REFERENCE_DENSITY_KG_M3))
        .cbrt();
    let equal_mass_threshold = DISRUPTION_COEFFICIENT
        * 0.8
        * std::f64::consts::PI
        * DISRUPTION_REFERENCE_DENSITY_KG_M3
        * GRAVITATIONAL_CONSTANT
        * combined_radius_m
        * combined_radius_m;

    let ratio = mass_a_kg.min(mass_b_kg) / mass_a_kg.max(mass_b_kg);
    let exponent = 2.0 / (3.0 * DISRUPTION_VELOCITY_EXPONENT) - 1.0;
    let mass_ratio_factor = ((ratio + 1.0) * (ratio + 1.0) / (4.0 * ratio)).powf(exponent);

    equal_mass_threshold * mass_ratio_factor
}

/// The share of the total mass left in the largest remnant after an impact of specific energy
/// `Q_R`, against a disruption threshold `Q*_RD`. Dimensionless, in `(0, 1]`.
///
/// Leinhardt & Stewart (2012, ApJ 745:79), the universal law (eq. 5) and, past
/// [`SUPERCATASTROPHIC_ONSET`], the super-catastrophic power law (eq. 44):
///
/// ```text
/// M_lr / M_tot = 1 − Q_R / (2·Q*_RD)                          Q_R/Q*_RD < 1.8
/// M_lr / M_tot = (0.1 / 1.8^η) · (Q_R / Q*_RD)^η,  η = −1.5     Q_R/Q*_RD ≥ 1.8
/// ```
///
/// The two meet at `0.1` at the onset. At `Q_R = Q*_RD` it is `½`, which is what defines `Q*_RD`.
///
/// # Parameters
///
/// - `specific_energy_j_kg` — `Q_R`, from [`specific_impact_energy`]. Finite and `≥ 0`.
/// - `disruption_threshold_j_kg` — `Q*_RD`, from [`catastrophic_disruption_threshold`]. Positive;
///   infinite is allowed and means nothing is dispersed.
///
/// # Returns
///
/// The fraction, in `(0, 1]`. `1` means a perfect merge. In release, invalid inputs return `1.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::largest_remnant_fraction;
///
/// assert_eq!(largest_remnant_fraction(0.0, 1.0e8), 1.0);
/// assert_eq!(largest_remnant_fraction(1.0e8, 1.0e8), 0.5);
/// assert!((largest_remnant_fraction(1.8e8, 1.0e8) - 0.1).abs() < 1e-12);
/// ```
pub fn largest_remnant_fraction(specific_energy_j_kg: f64, disruption_threshold_j_kg: f64) -> f64 {
    let valid = specific_energy_j_kg.is_finite()
        && specific_energy_j_kg >= 0.0
        && disruption_threshold_j_kg > 0.0
        && !disruption_threshold_j_kg.is_nan();
    debug_assert!(
        valid,
        "largest_remnant_fraction: need Q_R finite and >= 0 and Q*_RD > 0, got ({specific_energy_j_kg}, {disruption_threshold_j_kg})"
    );
    if !valid {
        return 1.0;
    }

    let ratio = specific_energy_j_kg / disruption_threshold_j_kg;
    if ratio < SUPERCATASTROPHIC_ONSET {
        1.0 - 0.5 * ratio
    } else {
        let onset_fraction = 1.0 - 0.5 * SUPERCATASTROPHIC_ONSET;
        onset_fraction * (ratio / SUPERCATASTROPHIC_ONSET).powf(SUPERCATASTROPHIC_EXPONENT)
    }
}

#[cfg(test)]
#[path = "../../tst/physics_utils/impacts_tests.rs"]
mod tests;
