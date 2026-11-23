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
//! - **The Tillotson equation of state**, the standard for rock and iron in impact simulations. Its
//!   pressure has a cold part, from compression, and a thermal part, from the internal energy.
//!   - [`tillotson_pressure`] is the whole equation, every branch, for fluid planets: the density
//!     is measured from the neighbours, and the pressure alone resists compression.
//!   - [`tillotson_thermal_pressure`] is its thermal part at uncompressed density, for the rubble
//!     planets, whose contact springs carry the cold part.
//! - **Smoothed-particle hydrodynamics**: a pressure becomes a force between neighbouring
//!   particles through the gradient of a smoothing kernel, in a symmetric form that conserves
//!   momentum, angular momentum and energy exactly. [`cubic_spline_kernel`] and
//!   [`cubic_spline_gradient`] are the standard kernel.
//!
//! # Sources
//!
//! - Tillotson, J. H. (1962). *Metallic equations of state for hypervelocity impact.* General
//!   Atomic report GA-3216, General Dynamics, San Diego, CA.
//! - Melosh, H. J. (1989). *Impact Cratering: A Geologic Process.* Oxford Monographs on Geology and
//!   Geophysics 11. Oxford University Press, New York. Appendix II: the branches of the Tillotson
//!   equation as [`tillotson_pressure`] writes them.
//! - Benz, W. & Asphaug, E. (1999). Catastrophic disruptions revisited. *Icarus* 142(1), 5–20.
//!   doi:10.1006/icar.1999.6204.
//! - The branch structure and the zero-pressure floor were checked against the SWIFT code's
//!   `src/equation_of_state/planetary/tillotson.h` (<https://github.com/SWIFTSIM/SWIFT>, read
//!   2026-11-22).
//! - Monaghan, J. J. & Lattanzio, J. C. (1985). A refined particle method for astrophysical
//!   problems. *Astronomy and Astrophysics* 149, 135–143: the cubic-spline kernel.
//! - Monaghan, J. J. (1992). Smoothed particle hydrodynamics. *Annual Review of Astronomy and
//!   Astrophysics* 30, 543–574. doi:10.1146/annurev.aa.30.090192.002551: the symmetric pressure
//!   force and the energy equation.
//!
//! # What is simplified
//!
//! - **No tension.** A pressure below zero is returned as zero, as in SWIFT's planetary equations of
//!   state: a fluid planet has no tensile strength, and negative pressures between particles cause
//!   the tensile instability of smoothed-particle hydrodynamics (particles clumping in pairs).
//! - **The thermal-only form** used by the rubble planets takes each particle's reference density:
//!   hot rock that has expanded still pushes as if at that density, so the model needs no density
//!   summation.

use std::f64::consts::PI;

/// The ten parameters of the Tillotson equation of state for one material, in SI.
///
/// The names follow Melosh (1989, appendix II); the energies, which SWIFT and WoMa call `u_0`,
/// `u_iv` and `u_cv`, are `E₀`, `E_iv` and `E_cv` here. Plain data with no invariant of its own
/// beyond [`TillotsonParameters::is_valid`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TillotsonParameters {
    /// `ρ₀`: the density at which the cold material is under no pressure, kg/m³.
    pub reference_density_kg_m3: f64,
    /// `A`: the linear coefficient of the cold compression curve, Pa.
    pub bulk_a_pa: f64,
    /// `B`: the quadratic coefficient of the cold compression curve, Pa.
    pub bulk_b_pa: f64,
    /// `a`: the Grüneisen parameter far above `E₀`. Dimensionless.
    pub a: f64,
    /// `b`: added to `a` for the Grüneisen parameter of the cool material. Dimensionless.
    pub b: f64,
    /// `E₀`: the energy over which the Grüneisen parameter falls from `a + b` to `a`, J/kg.
    pub reference_energy_j_kg: f64,
    /// `E_iv`: the energy at which expanded material begins to vaporise, J/kg.
    pub incipient_vaporisation_j_kg: f64,
    /// `E_cv`: the energy at which expanded material is completely vaporised, J/kg.
    pub complete_vaporisation_j_kg: f64,
    /// `α`: the decay rate of the thermal term's solid part in expanded vapour. Dimensionless.
    pub alpha: f64,
    /// `β`: the decay rate of the cold term in expanded vapour. Dimensionless.
    pub beta: f64,
}

impl TillotsonParameters {
    /// Every parameter finite; `ρ₀`, `A`, `E₀` positive; `B`, `a`, `b`, `α`, `β` non-negative; and
    /// `0 < E_iv < E_cv`.
    pub fn is_valid(&self) -> bool {
        let finite = [
            self.reference_density_kg_m3,
            self.bulk_a_pa,
            self.bulk_b_pa,
            self.a,
            self.b,
            self.reference_energy_j_kg,
            self.incipient_vaporisation_j_kg,
            self.complete_vaporisation_j_kg,
            self.alpha,
            self.beta,
        ]
        .iter()
        .all(|value| value.is_finite());
        finite
            && self.reference_density_kg_m3 > 0.0
            && self.bulk_a_pa > 0.0
            && self.bulk_b_pa >= 0.0
            && self.a >= 0.0
            && self.b >= 0.0
            && self.reference_energy_j_kg > 0.0
            && self.alpha >= 0.0
            && self.beta >= 0.0
            && self.incipient_vaporisation_j_kg > 0.0
            && self.complete_vaporisation_j_kg > self.incipient_vaporisation_j_kg
    }
}

/// The pressure of a material at density `density_kg_m3` and specific internal energy
/// `internal_energy_j_kg`, by the whole Tillotson equation of state, in pascals. Never negative.
///
/// With `η = ρ/ρ₀`, `μ = η − 1`, `ν = 1/η − 1` and `w = u/(E₀·η²) + 1` (Melosh 1989, appendix II):
///
/// ```text
/// condensed  P_c = (a + b/w)·ρ·u + A·μ + B·μ²                                ρ ≥ ρ₀, or u ≤ E_iv
/// expanded   P_e = a·ρ·u + (b·ρ·u/w + A·μ·e^(−β·ν))·e^(−α·ν²)                ρ < ρ₀ and u ≥ E_cv
/// between    P   = ((u − E_iv)·P_e + (E_cv − u)·P_c) / (E_cv − E_iv)          ρ < ρ₀, E_iv < u < E_cv
/// ```
///
/// The condensed branch is the cold compression curve `A·μ + B·μ²` plus a thermal term with
/// Grüneisen parameter `a + b/w`; the expanded branch lets vapour behave as a gas. A negative
/// result (cold material stretched below `ρ₀`) is returned as zero: see the module's notes on
/// tension. A density that is not positive and finite, or an energy that is not finite, gives
/// zero; a negative energy counts as zero.
///
/// At `ρ = ρ₀` the condensed branch is [`tillotson_thermal_pressure`] exactly.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{tillotson_pressure, TillotsonParameters};
///
/// let basalt = TillotsonParameters {
///     reference_density_kg_m3: 2700.0,
///     bulk_a_pa: 2.67e10,
///     bulk_b_pa: 2.67e10,
///     a: 0.5,
///     b: 1.5,
///     reference_energy_j_kg: 4.87e8,
///     incipient_vaporisation_j_kg: 4.72e6,
///     complete_vaporisation_j_kg: 1.82e7,
///     alpha: 5.0,
///     beta: 5.0,
/// };
/// // Cold and uncompressed: no pressure.
/// assert_eq!(tillotson_pressure(2700.0, 0.0, &basalt), 0.0);
/// // Cold and compressed by 10%: A·0.1 + B·0.01.
/// let p = tillotson_pressure(2970.0, 0.0, &basalt);
/// assert!((p / (2.67e9 + 2.67e8) - 1.0).abs() < 1e-9);
/// // Cold and stretched: no tension.
/// assert_eq!(tillotson_pressure(2000.0, 0.0, &basalt), 0.0);
/// ```
pub fn tillotson_pressure(
    density_kg_m3: f64,
    internal_energy_j_kg: f64,
    material: &TillotsonParameters,
) -> f64 {
    let valid =
        density_kg_m3.is_finite() && density_kg_m3 > 0.0 && internal_energy_j_kg.is_finite();
    debug_assert!(
        material.is_valid(),
        "tillotson_pressure: invalid material {material:?}"
    );
    if !valid || !material.is_valid() {
        return 0.0;
    }
    unclamped_tillotson_pressure(density_kg_m3, internal_energy_j_kg.max(0.0), material).max(0.0)
}

/// [`tillotson_pressure`] before the floor at zero: negative for cold material stretched below
/// `ρ₀`. What [`tillotson_sound_speed`] differentiates, so that the stiffness of the material is
/// seen on both sides of `ρ₀`, where the floor would make a central difference read half of it.
/// The caller has checked the inputs.
fn unclamped_tillotson_pressure(rho: f64, u: f64, material: &TillotsonParameters) -> f64 {
    let eta = rho / material.reference_density_kg_m3;
    let mu = eta - 1.0;
    let w = u / (material.reference_energy_j_kg * eta * eta) + 1.0;
    let condensed = (material.a + material.b / w) * rho * u
        + material.bulk_a_pa * mu
        + material.bulk_b_pa * mu * mu;
    if eta >= 1.0 || u <= material.incipient_vaporisation_j_kg {
        condensed
    } else {
        let nu = 1.0 / eta - 1.0;
        let expanded = material.a * rho * u
            + (material.b * rho * u / w + material.bulk_a_pa * mu * (-material.beta * nu).exp())
                * (-material.alpha * nu * nu).exp();
        if u >= material.complete_vaporisation_j_kg {
            expanded
        } else {
            let (iv, cv) = (
                material.incipient_vaporisation_j_kg,
                material.complete_vaporisation_j_kg,
            );
            ((u - iv) * expanded + (cv - u) * condensed) / (cv - iv)
        }
    }
}

/// The adiabatic sound speed of a material at density `density_kg_m3` and specific internal energy
/// `internal_energy_j_kg`, by the Tillotson equation of state, in metres per second.
///
/// ```text
/// c² = (∂P/∂ρ)_u + (P/ρ²)·(∂P/∂u)_ρ
/// ```
///
/// the sound speed of any equation of state written as `P(ρ, u)`, since along an adiabat
/// `du = (P/ρ²)·dρ`. The two partial derivatives are taken by finite differences of the pressure
/// before its floor at zero, which keeps one formula for every branch (and the same in the GPU
/// kernel) and sees the material's stiffness on both sides of `ρ₀`; `P` itself is the floored
/// pressure. A caller that needs a minimum signal speed (deep in vapour, where `c²` can fall to
/// zero) applies its own floor.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{tillotson_sound_speed, TillotsonParameters};
///
/// let basalt = TillotsonParameters {
///     reference_density_kg_m3: 2700.0,
///     bulk_a_pa: 2.67e10,
///     bulk_b_pa: 2.67e10,
///     a: 0.5,
///     b: 1.5,
///     reference_energy_j_kg: 4.87e8,
///     incipient_vaporisation_j_kg: 4.72e6,
///     complete_vaporisation_j_kg: 1.82e7,
///     alpha: 5.0,
///     beta: 5.0,
/// };
/// // Cold, uncompressed rock: c² = A/ρ₀, about 3.1 km/s.
/// let c = tillotson_sound_speed(2700.0, 0.0, &basalt);
/// assert!((c / (2.67e10_f64 / 2700.0).sqrt() - 1.0).abs() < 1e-3, "{c}");
/// ```
pub fn tillotson_sound_speed(
    density_kg_m3: f64,
    internal_energy_j_kg: f64,
    material: &TillotsonParameters,
) -> f64 {
    /// The relative step of the finite differences. Not tunable: small enough that the error of a
    /// central difference (its square, `1e-8`) is negligible and large enough that `f64` rounding
    /// in the pressure (`1e-16` of it, divided by the step) is too.
    const STEP: f64 = 1.0e-4; // ALGORITHM CONSTANT: finite-difference step
    let valid =
        density_kg_m3.is_finite() && density_kg_m3 > 0.0 && internal_energy_j_kg.is_finite();
    if !valid || !material.is_valid() {
        return 0.0;
    }
    let raw = |rho: f64, u: f64| unclamped_tillotson_pressure(rho, u, material);
    let rho = density_kg_m3;
    let u = internal_energy_j_kg.max(0.0);
    let d_rho = rho * STEP;
    let dp_drho = (raw(rho + d_rho, u) - raw(rho - d_rho, u)) / (2.0 * d_rho);
    // Forward in `u`, which cannot go below zero.
    let d_u = STEP * (u + material.incipient_vaporisation_j_kg);
    let dp_du = (raw(rho, u + d_u) - raw(rho, u)) / d_u;
    let pressure = raw(rho, u).max(0.0);
    (dp_drho + pressure / (rho * rho) * dp_du).max(0.0).sqrt()
}

/// The specific internal energy of material compressed from `ρ₀` to `density_kg_m3` with no heat
/// added: the **cold isentrope** through the reference state, in joules per kilogram.
///
/// ```text
/// du/dρ = P(ρ, u) / ρ²,    u(ρ₀) = 0
/// ```
///
/// integrated by fourth-order Runge–Kutta in `steps` equal steps of density. Compression does work
/// `P·dV = (P/ρ²)·dρ` per kilogram, and with the Tillotson equation that work also raises the
/// pressure, through its thermal term. A fluid planet built at zero temperature has this energy at
/// each depth, so that when its material expands it does work back out of it rather than out of
/// nothing. This is what WoMa calls the cold internal energy (`woma/eos/tillotson.py`,
/// <https://github.com/srbonilla/WoMa>, read 2026-11-22).
///
/// Zero at or below `ρ₀` (where the cold pressure is floored at zero and no work is done), and for
/// an invalid density, material, or zero steps.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{tillotson_cold_energy, tillotson_pressure, TillotsonParameters};
///
/// let basalt = TillotsonParameters {
///     reference_density_kg_m3: 2700.0,
///     bulk_a_pa: 2.67e10,
///     bulk_b_pa: 2.67e10,
///     a: 0.5,
///     b: 1.5,
///     reference_energy_j_kg: 4.87e8,
///     incipient_vaporisation_j_kg: 4.72e6,
///     complete_vaporisation_j_kg: 1.82e7,
///     alpha: 5.0,
///     beta: 5.0,
/// };
/// assert_eq!(tillotson_cold_energy(2700.0, &basalt, 100), 0.0);
/// // Slightly compressed: u ≈ A·μ²/(2ρ₀), the elastic energy of the linear term.
/// let u = tillotson_cold_energy(2700.0 * 1.001, &basalt, 100);
/// let elastic = 2.67e10 * 1.0e-6 / (2.0 * 2700.0);
/// assert!((u / elastic - 1.0).abs() < 0.01, "{u} against {elastic}");
/// ```
pub fn tillotson_cold_energy(
    density_kg_m3: f64,
    material: &TillotsonParameters,
    steps: u32,
) -> f64 {
    let rho0 = material.reference_density_kg_m3;
    let valid = density_kg_m3.is_finite() && material.is_valid() && steps > 0;
    if !valid || density_kg_m3 <= rho0 {
        return 0.0;
    }
    let slope =
        |rho: f64, u: f64| unclamped_tillotson_pressure(rho, u.max(0.0), material) / (rho * rho);
    let d_rho = (density_kg_m3 - rho0) / f64::from(steps);
    let mut u = 0.0;
    for step in 0..steps {
        let rho = rho0 + f64::from(step) * d_rho;
        let k1 = slope(rho, u);
        let k2 = slope(rho + 0.5 * d_rho, u + 0.5 * d_rho * k1);
        let k3 = slope(rho + 0.5 * d_rho, u + 0.5 * d_rho * k2);
        let k4 = slope(rho + d_rho, u + d_rho * k3);
        u += d_rho / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
    }
    u.max(0.0)
}

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
