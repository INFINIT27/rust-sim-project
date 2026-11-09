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

#[cfg(test)]
#[path = "../../tst/physics_utils/physics_constants_tests.rs"]
mod tests;
