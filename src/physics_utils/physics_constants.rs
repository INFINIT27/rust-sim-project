//! Physical constants, in SI base units.
//!
//! Each constant documents its unit and its source. Values are pinned by a
//! unit test so a careless edit cannot silently change the physics.

/// The universal (Newtonian) gravitational constant `G`.
///
/// Unit: cubic metres per kilogram per second squared
/// (`m³·kg⁻¹·s⁻²`), equivalently `N·m²·kg⁻²`.
///
/// Value: `6.674_30e-11`, the CODATA 2018 recommended value
/// (`6.674 30(15) × 10⁻¹¹`, relative standard uncertainty `2.2e-5`). `G` is
/// the least precisely known of the fundamental constants, so treat any
/// result derived from it as good to roughly five significant figures — the
/// tolerances in this module's tests are chosen with that in mind.
///
/// This is a true constant, not a computed quantity: it is the proportionality
/// factor in Newton's law of universal gravitation, which
/// [`super::physics_formulas::gravitational_constant_calculator`] evaluates.
pub const GRAVITATIONAL_CONSTANT: f64 = 6.674_30e-11;

/// Standard gravity `g₀`, the unit a "g" of acceleration is counted in.
///
/// Unit: metres per second squared (`m·s⁻²`).
///
/// Value: `9.806_65` **exactly**: a defined conventional value (3rd CGPM, 1901), not a measurement of
/// Earth's gravity, which varies from about `9.78` at the equator to `9.83` at the poles.
///
/// Used to state a powered rocket's acceleration in g, as
/// [`crate::render::mission`]'s status line does: an Aerojet 260 on `15 t` is `178 g`.
pub const STANDARD_GRAVITY_M_S2: f64 = 9.806_65;

/// The reference density in the catastrophic-disruption scaling law, `ρ₁`.
///
/// Unit: kilograms per cubic metre (`kg·m⁻³`).
///
/// Value: `1000` **exactly**, by definition: Leinhardt & Stewart (2012, ApJ 745:79, eq. 2) measure
/// every collision against the radius `R_C1` the combined mass would have at `1 g/cm³`, so that one
/// curve fits bodies of any composition. It is part of the law, not a density of anything.
///
/// Used by [`super::impacts::catastrophic_disruption_threshold`].
pub const DISRUPTION_REFERENCE_DENSITY_KG_M3: f64 = 1000.0;

/// The coefficient `c*` of the catastrophic-disruption scaling law. Dimensionless.
///
/// Value: `5`, Stewart & Leinhardt (2012, ApJ 751:32) for **planet-sized** bodies, which behave as
/// fluids held together by gravity. Their fit is `5 ± 2`. For small rubble-pile bodies the same
/// authors give `1.9 ± 0.3` (Leinhardt & Stewart 2012); this crate's collisions that make debris are
/// between planets and moons, so the planetary value is the one used.
///
/// Used by [`super::impacts::catastrophic_disruption_threshold`].
pub const DISRUPTION_COEFFICIENT: f64 = 5.0;

/// The velocity exponent `μ̄` of the catastrophic-disruption scaling law. Dimensionless.
///
/// Value: `1/3`, Stewart & Leinhardt (2012, ApJ 751:32) for planet-sized bodies: the limit in which
/// the impact's energy, not its momentum, sets the outcome. Leinhardt & Stewart (2012) give `0.36`
/// for small bodies. It sets how strongly an unequal pair of masses raises the threshold.
///
/// Used by [`super::impacts::catastrophic_disruption_threshold`].
pub const DISRUPTION_VELOCITY_EXPONENT: f64 = 1.0 / 3.0;

/// The specific impact energy, as a multiple of the catastrophic-disruption threshold, at which the
/// largest remnant stops following the linear law and enters the super-catastrophic regime.
/// Dimensionless.
///
/// Value: `1.8`, Leinhardt & Stewart (2012, ApJ 745:79, section 4.2): the linear "universal law"
/// leaves `10%` of the mass in the largest remnant there, and the power law
/// [`SUPERCATASTROPHIC_EXPONENT`] takes over from that point so the two meet.
///
/// Used by [`super::impacts::largest_remnant_fraction`].
pub const SUPERCATASTROPHIC_ONSET: f64 = 1.8;

/// The exponent of the largest remnant's mass against impact energy in the super-catastrophic
/// regime. Dimensionless.
///
/// Value: `−1.5`, Leinhardt & Stewart (2012, ApJ 745:79, eq. 44), fitted to laboratory and numerical
/// disruptions: `M_lr/M_tot = (0.1 / 1.8^η)·(Q_R/Q*_RD)^η`.
///
/// Used by [`super::impacts::largest_remnant_fraction`].
pub const SUPERCATASTROPHIC_EXPONENT: f64 = -1.5;

/// The fraction of space equal spheres fill when packed as a face-centred cubic lattice,
/// `π / (3·√2)`. Dimensionless.
///
/// Value: `0.7405`, exact (Kepler's conjecture, proved by Hales 2005: no packing of equal spheres
/// is denser). The planets built from particles are packed this way
/// ([`crate::collision::aggregate`]), so a particle stands for its sphere plus its share of the gaps.
///
/// Used by [`super::thermal::particle_density`].
pub const FCC_PACKING_FRACTION: f64 = std::f64::consts::PI / (3.0 * std::f64::consts::SQRT_2);

// ---------------------------------------------------------------------------------------------
// The Tillotson equation of state: basalt and iron.
//
// Sources, cited in full once here and by short form on each constant:
//
// - **Tillotson 1962**: Tillotson, J. H. (1962). *Metallic equations of state for hypervelocity
//   impact.* General Atomic report GA-3216, General Dynamics, San Diego, CA. The equation itself.
// - **Melosh 1989**: Melosh, H. J. (1989). *Impact Cratering: A Geologic Process.* Oxford Monographs
//   on Geology and Geophysics 11. Oxford University Press, New York. Appendix II: the equation's
//   branches and the iron parameter set.
// - **Benz & Asphaug 1999**: Benz, W. & Asphaug, E. (1999). Catastrophic disruptions revisited.
//   *Icarus* 142(1), 5–20. doi:10.1006/icar.1999.6204. Table II: the basalt parameter set.
// - **SWIFT**: the SWIFT simulation code, `src/equation_of_state/planetary/tillotson.h`, functions
//   `set_Til_basalt` and `set_Til_iron` (SI units). https://github.com/SWIFTSIM/SWIFT, read
//   2026-11-22.
// - **WoMa**: the WoMa planet-building code, `woma/eos/tillotson.py` (SI units).
//   https://github.com/srbonilla/WoMa, read 2026-11-22.
//
// **Which were checked.** Every value below was read from SWIFT and WoMa, which agree on every
// digit. Both are implementations; WoMa's documentation attributes the basalt set to
// Benz & Asphaug 1999 and the iron set to Melosh 1989. The papers themselves were not re-read
// here.
// ---------------------------------------------------------------------------------------------

/// The Tillotson parameter `a` for basalt. Dimensionless.
///
/// Value: `0.5` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa; see the source list
/// above). It is the Grüneisen parameter of rock heated far past [`BASALT_TILLOTSON_E0_J_KG`],
/// where it behaves as a gas.
///
/// Used by the planetary materials in `src/projects/interstellar/constants.rs`, through
/// [`super::thermal::tillotson_thermal_pressure`] and [`super::thermal::tillotson_pressure`].
pub const BASALT_TILLOTSON_A: f64 = 0.5;

/// The Tillotson parameter `b` for basalt. Dimensionless.
///
/// Value: `1.5` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa). With
/// [`BASALT_TILLOTSON_A`] it gives cool rock a Grüneisen parameter of `2`.
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_B: f64 = 1.5;

/// The Tillotson reference energy `E₀` for basalt, in joules per kilogram.
///
/// Value: `4.87e8` (`487 MJ/kg`) (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa):
/// the internal energy over which the Grüneisen parameter falls from `a + b` towards `a`. It is
/// far larger than iron's [`IRON_TILLOTSON_E0_J_KG`], as both codes have it.
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_E0_J_KG: f64 = 4.87e8;

/// The Tillotson reference density `ρ₀` of basalt, in kilograms per cubic metre: the density at
/// which cold basalt is under no pressure.
///
/// Value: `2700` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_RHO0_KG_M3: f64 = 2700.0;

/// The Tillotson bulk modulus `A` of basalt, in pascals: the linear term of the cold compression
/// curve, `P = A·μ + B·μ²` with `μ = ρ/ρ₀ − 1`.
///
/// Value: `2.67e10` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_BULK_A_PA: f64 = 2.67e10;

/// The Tillotson nonlinear compression coefficient `B` of basalt, in pascals: the quadratic term
/// of the cold compression curve.
///
/// Value: `2.67e10` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_BULK_B_PA: f64 = 2.67e10;

/// The specific internal energy at which expanded basalt **begins** to vaporise, `E_iv`, in joules
/// per kilogram: below it, expanded basalt follows the condensed branch of the equation.
///
/// Value: `4.72e6` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_EIV_J_KG: f64 = 4.72e6;

/// The specific internal energy at which expanded basalt is **completely** vaporised, `E_cv`, in
/// joules per kilogram: above it, expanded basalt follows the expanded (gas-like) branch.
///
/// Value: `1.82e7` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_ECV_J_KG: f64 = 1.82e7;

/// The Tillotson parameter `α` of basalt, dimensionless: how fast the thermal term's solid-like
/// part dies away as vapour expands.
///
/// Value: `5` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_ALPHA: f64 = 5.0;

/// The Tillotson parameter `β` of basalt, dimensionless: how fast the cold term dies away as vapour
/// expands.
///
/// Value: `5` (Benz & Asphaug 1999, table II; checked in SWIFT and WoMa).
///
/// Used as [`BASALT_TILLOTSON_A`].
pub const BASALT_TILLOTSON_BETA: f64 = 5.0;

/// The Tillotson parameter `a` for iron. Dimensionless.
///
/// Value: `0.5` (Melosh 1989, appendix II; checked in SWIFT and WoMa; see the source list
/// above).
///
/// Used by the planetary iron material in `src/projects/interstellar/constants.rs`, through
/// [`super::thermal::tillotson_pressure`].
pub const IRON_TILLOTSON_A: f64 = 0.5;

/// The Tillotson parameter `b` for iron. Dimensionless.
///
/// Value: `1.5` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_B: f64 = 1.5;

/// The Tillotson reference energy `E₀` for iron, in joules per kilogram.
///
/// Value: `9.5e6` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_E0_J_KG: f64 = 9.5e6;

/// The Tillotson reference density `ρ₀` of iron, in kilograms per cubic metre.
///
/// Value: `7800` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_RHO0_KG_M3: f64 = 7800.0;

/// The Tillotson bulk modulus `A` of iron, in pascals.
///
/// Value: `1.28e11` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_BULK_A_PA: f64 = 1.28e11;

/// The Tillotson nonlinear compression coefficient `B` of iron, in pascals.
///
/// Value: `1.05e11` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_BULK_B_PA: f64 = 1.05e11;

/// The specific internal energy at which expanded iron begins to vaporise, `E_iv`, in joules per
/// kilogram.
///
/// Value: `2.4e6` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_EIV_J_KG: f64 = 2.4e6;

/// The specific internal energy at which expanded iron is completely vaporised, `E_cv`, in joules
/// per kilogram.
///
/// Value: `8.67e6` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_ECV_J_KG: f64 = 8.67e6;

/// The Tillotson parameter `α` of iron. Dimensionless.
///
/// Value: `5` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_ALPHA: f64 = 5.0;

/// The Tillotson parameter `β` of iron. Dimensionless.
///
/// Value: `5` (Melosh 1989; checked in SWIFT and WoMa).
///
/// Used as [`IRON_TILLOTSON_A`].
pub const IRON_TILLOTSON_BETA: f64 = 5.0;

#[cfg(test)]
#[path = "../../tst/physics_utils/physics_constants_tests.rs"]
mod tests;
