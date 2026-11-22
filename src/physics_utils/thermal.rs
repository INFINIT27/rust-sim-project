//! Hot rock: the pressure heat puts into rock, and the smoothing kernel that turns a pressure into
//! a force between particles.
//!
//! A planet built from particles that collide through damped springs loses energy at every
//! contact. Left there, that energy leaves the simulation, and nothing is ever thrown off: the
//! impact's energy can only stop the planets, never push material apart. In a real giant impact
//! that energy becomes **heat**, the heated rock is at enormous pressure, and the pressure is what
//! expands the shocked material and throws debris into orbit and beyond. These are the laws for
//! that, in the form impact simulations use:
//!
//! - **The Tillotson equation of state** (Tillotson 1962, General Atomic report GA-3216), the
//!   standard for rock in impact simulations (Benz & Asphaug 1999, Icarus 142:5; Canup 2004,
//!   Icarus 168:433). Its pressure has a cold part, from compression, and a thermal part, from the
//!   internal energy. In this crate the contact springs already carry the cold part (they resist
//!   compression), so only the thermal part is used: [`tillotson_thermal_pressure`].
//! - **Smoothed-particle hydrodynamics** (Monaghan 1992, ARA&A 30:543): a pressure becomes a force
//!   between neighbouring particles through the gradient of a smoothing kernel, in a symmetric form
//!   that conserves momentum, angular momentum and energy exactly. [`cubic_spline_kernel`] and
//!   [`cubic_spline_gradient`] are the standard kernel.
//!
//! # What is simplified
//!
//! The density in the pressure is each particle's reference density, not one measured from its
//! neighbours: compression is the springs' job. So the thermal pressure is `Γ(u)·ρ₀·u`, the
//! Tillotson form at uncompressed density, where its compressed and expanded branches agree.
//! Hot rock that has expanded still pushes as if at its reference density; in exchange the
//! model needs no density summation, and stays a single pass over neighbouring pairs.

use std::f64::consts::PI;

/// The thermal pressure of rock at specific internal energy `internal_energy_j_kg` and density
/// `density_kg_m3`, by the Tillotson equation of state at uncompressed density: in pascals.
///
/// ```text
/// P = (a + b / (u/E₀ + 1)) · ρ · u
/// ```
///
/// `a` and `b` are dimensionless and `E₀` is in joules per kilogram. The bracket is the Grüneisen
/// parameter `Γ`: `a + b` for cool rock (`2` for basalt, the solid's value), falling to `a` (`0.5`)
/// once the internal energy is well above `E₀`, where the rock behaves as a gas. Zero for zero or
/// negative energy, and for a density that is not positive: heat cannot pull.
///
/// # Examples
///
/// ```
/// use physics_sandbox::tillotson_thermal_pressure;
///
/// // Basalt (a = 0.5, b = 1.5, E₀ = 487 MJ/kg) at 2,700 kg/m³ heated by 1 MJ/kg, about 1,000 K:
/// let pressure = tillotson_thermal_pressure(2700.0, 1.0e6, 0.5, 1.5, 4.87e8);
/// // Γ is close to its cool value of 2, so P ≈ 2 · ρ · u = 5.4 GPa.
/// assert!((pressure / 5.4e9 - 1.0).abs() < 0.01);
/// assert_eq!(tillotson_thermal_pressure(2700.0, 0.0, 0.5, 1.5, 4.87e8), 0.0);
/// ```
pub fn tillotson_thermal_pressure(
    density_kg_m3: f64,
    internal_energy_j_kg: f64,
    a: f64,
    b: f64,
    reference_energy_j_kg: f64,
) -> f64 {
    let valid = density_kg_m3 > 0.0 && internal_energy_j_kg > 0.0 && reference_energy_j_kg > 0.0;
    if !valid {
        return 0.0;
    }
    let gruneisen = a + b / (internal_energy_j_kg / reference_energy_j_kg + 1.0);
    gruneisen * density_kg_m3 * internal_energy_j_kg
}

/// The density of a particle that stands for its share of a body packed as a face-centred cubic
/// lattice: its mass over the volume of its lattice cell, `m·φ / (4/3·π·r³)` with `φ` the packing
/// fraction ([`FCC_PACKING_FRACTION`](super::physics_constants::FCC_PACKING_FRACTION)).
///
/// A planet built from `n` such particles has `n` cells filling its volume, so each particle's
/// density is the planet's mean density.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{particle_density, FCC_PACKING_FRACTION};
///
/// let radius = 1.0_f64;
/// let mass = 1000.0 * (4.0 / 3.0) * std::f64::consts::PI / FCC_PACKING_FRACTION;
/// assert!((particle_density(mass, radius) - 1000.0).abs() < 1e-9);
/// ```
pub fn particle_density(mass_kg: f64, radius_m: f64) -> f64 {
    use super::physics_constants::FCC_PACKING_FRACTION;
    mass_kg * FCC_PACKING_FRACTION / (4.0 / 3.0 * PI * radius_m.powi(3))
}

/// The moment of inertia of a uniform sphere about any axis through its centre, `2/5·m·r²`, in
/// kilogram square metres.
///
/// # Examples
///
/// ```
/// use physics_sandbox::sphere_moment_of_inertia;
///
/// assert_eq!(sphere_moment_of_inertia(5.0, 2.0), 8.0);
/// ```
pub fn sphere_moment_of_inertia(mass_kg: f64, radius_m: f64) -> f64 {
    0.4 * mass_kg * radius_m * radius_m
}

/// The cubic-spline smoothing kernel `W(r, h)` in three dimensions (Monaghan & Lattanzio 1985),
/// in inverse cubic metres: how much a particle at distance `r` counts towards a quantity smoothed
/// over length `h`. Zero from `r = 2h` out, and normalised so its integral over space is `1`.
///
/// ```text
/// q = r/h,   σ = 1/(π·h³)
/// W = σ·(1 − 3/2·q² + 3/4·q³)     0 ≤ q < 1
/// W = σ·(2 − q)³/4                1 ≤ q < 2
/// ```
///
/// # Examples
///
/// ```
/// use physics_sandbox::cubic_spline_kernel;
///
/// let h = 1.0;
/// assert!((cubic_spline_kernel(0.0, h) - 1.0 / std::f64::consts::PI).abs() < 1e-12);
/// assert_eq!(cubic_spline_kernel(2.0, h), 0.0);
/// ```
pub fn cubic_spline_kernel(distance_m: f64, smoothing_m: f64) -> f64 {
    if smoothing_m <= 0.0 || distance_m.is_nan() {
        return 0.0;
    }
    let q = distance_m.abs() / smoothing_m;
    let sigma = 1.0 / (PI * smoothing_m.powi(3));
    if q < 1.0 {
        sigma * (1.0 - 1.5 * q * q + 0.75 * q * q * q)
    } else if q < 2.0 {
        sigma * 0.25 * (2.0 - q).powi(3)
    } else {
        0.0
    }
}

/// The slope `dW/dr` of [`cubic_spline_kernel`], in inverse metres to the fourth: never positive,
/// zero at `r = 0` and from `r = 2h` out.
///
/// The gradient of the kernel at particle `i` from particle `j` is this times the unit vector from
/// `j` to `i`, which is what a pressure force between them is built on.
///
/// # Examples
///
/// ```
/// use physics_sandbox::cubic_spline_gradient;
///
/// let h = 1.0;
/// assert_eq!(cubic_spline_gradient(0.0, h), 0.0);
/// assert!(cubic_spline_gradient(1.0, h) < 0.0);
/// assert_eq!(cubic_spline_gradient(2.5, h), 0.0);
/// ```
pub fn cubic_spline_gradient(distance_m: f64, smoothing_m: f64) -> f64 {
    if smoothing_m <= 0.0 || distance_m.is_nan() {
        return 0.0;
    }
    let q = distance_m.abs() / smoothing_m;
    let sigma = 1.0 / (PI * smoothing_m.powi(4));
    if q < 1.0 {
        sigma * (-3.0 * q + 2.25 * q * q)
    } else if q < 2.0 {
        sigma * -0.75 * (2.0 - q).powi(2)
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "../../tst/physics_utils/thermal_tests.rs"]
mod tests;
