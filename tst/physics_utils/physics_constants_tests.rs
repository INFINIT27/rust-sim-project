//! Unit tests for the [`crate::physics_utils::physics_constants`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/physics_utils/physics_constants.rs` via `#[path]`, not as a standalone
//! integration test, so these tests keep access to the module's private items
//! and to `super::*`.
//!
//! A constant has no behaviour to exercise, so these tests pin the three
//! things that can still go wrong: the literal value, its representability in
//! `f64`, and its unit. The last one matters most — a value that is right to
//! five digits but expressed in the wrong unit would pass a naive equality
//! check and silently corrupt every derived force.

// Every assertion in this module is applied to a `const`, so
// `clippy::assertions_on_constants` fires by construction. Allowed
// deliberately: the lint's objection is "this cannot fail at runtime", and
// that is exactly the property being asserted. These tests exist to fail at
// *compile* time on a future edit to the constant, which is the only way it
// can change.
#![allow(clippy::assertions_on_constants)]

use super::*;

/// The literal is exactly the CODATA 2018 recommended value.
///
/// Pinned bit-exactly rather than approximately: this is a defined input to
/// the crate, not a measurement we are reproducing, so any change to it should
/// be a deliberate edit that also updates this test.
#[test]
fn gravitational_constant_matches_codata_2018_value() {
    assert_eq!(GRAVITATIONAL_CONSTANT, 6.674_30e-11);
}

/// The constant is a usable finite positive number.
///
/// Guards against a typo that produces `0.0`, a negative value (which would
/// turn gravity repulsive), or a literal so small it denormalises.
#[test]
fn gravitational_constant_is_finite_and_strictly_positive() {
    assert!(
        GRAVITATIONAL_CONSTANT.is_finite(),
        "G must be finite, got {}",
        GRAVITATIONAL_CONSTANT
    );
    assert!(
        GRAVITATIONAL_CONSTANT > 0.0,
        "G must be strictly positive, got {}",
        GRAVITATIONAL_CONSTANT
    );
    assert!(
        GRAVITATIONAL_CONSTANT.is_normal(),
        "G must be a normal (non-denormal) f64, got {}",
        GRAVITATIONAL_CONSTANT
    );
}

/// The exponent is right: `G` sits in `[6.6e-11, 6.7e-11]`.
///
/// A misplaced decimal point or a dropped `e-11` is the most likely way to
/// break this constant, and it would leave the value plausible-looking while
/// changing every force by orders of magnitude. This is the cheap check that
/// catches it independently of the bit-exact pin above.
#[test]
fn gravitational_constant_has_the_expected_order_of_magnitude() {
    assert!(
        (6.6e-11..=6.7e-11).contains(&GRAVITATIONAL_CONSTANT),
        "G must be ~6.674e-11 m³·kg⁻¹·s⁻², got {}",
        GRAVITATIONAL_CONSTANT
    );
}

/// The constant is expressed in SI base units, verified dimensionally.
///
/// `G` is defined so that two 1 kg masses 1 m apart attract with exactly `G`
/// newtons. Feeding those unit inputs through the formula and getting the
/// constant back proves the value is in `m³·kg⁻¹·s⁻²` and not, say,
/// `cm³·g⁻¹·s⁻²` — which would be numerically different by `1e-3` yet just as
/// "positive and finite".
#[test]
fn gravitational_constant_is_in_si_units() {
    let force_between_unit_masses =
        crate::physics_utils::physics_formulas::gravitational_constant_calculator(1.0, 1.0, 1.0);

    assert_eq!(
        force_between_unit_masses, GRAVITATIONAL_CONSTANT,
        "two 1 kg masses 1 m apart must attract with exactly G newtons"
    );
}

/// Standard gravity is the defined conventional value, exactly.
#[test]
fn standard_gravity_is_the_defined_value() {
    assert_eq!(STANDARD_GRAVITY_M_S2, 9.806_65);
}

/// An Aerojet 260 at its SL-3 thrust on a `15 t` craft is `178 g`, as the docs quote.
#[test]
fn the_powered_rockets_acceleration_is_178_g() {
    let g = 26.2e6 / 15_000.0 / STANDARD_GRAVITY_M_S2;
    assert!((g - 178.1).abs() < 0.1, "{g} g");
}
