//! The relativity sub-layer's constants module.
//!
//! One constant, and it is a different *kind* of thing from
//! [`GRAVITATIONAL_CONSTANT`](crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT):
//! `G` is measured and is the least precisely known fundamental constant, while `c` is **defined**
//! and carries no uncertainty at all. That difference is why the tolerances here are float-epsilon
//! rather than the five significant figures the gravitation tests settle for.

/// The speed of light in vacuum, `c`.
///
/// Unit: metres per second (`m·s⁻¹`).
///
/// **Why this value.** `299_792_458` **exactly**, with no uncertainty — since 1983 the metre has been
/// *defined* as the distance light travels in `1/299_792_458` of a second, so this number is not a
/// measurement that could be refined. It is a unit conversion between the second and the metre.
/// Writing it as `2.998e8` would be a needless loss of eight exact digits.
///
/// **The digit string invites a double-take, so: this is metres per second.** `299_792_458 m/s` is
/// `2.998e8 m/s`, which is the same speed as the more commonly quoted `3e5 km/s` — the decimal point
/// moves three places, the unit does not change. Written in km/s this constant would need one
/// (`299_792.458`), so as an integer it can only be m/s.
///
/// Three cross-checks, if the exponent ever looks wrong again: light reaches the Moon in `1.28 s`,
/// reaches Earth from the Sun in `8.32 min`, and laps Earth's equator `7.5` times a second. The first
/// two are pinned by `constants_tests`.
///
/// It is exactly representable in `f64` (it is well under `2^53`), so `SPEED_OF_LIGHT_M_S` and
/// `SPEED_OF_LIGHT_M_S * 1.0` and any integer arithmetic on it agree bit for bit.
///
/// **Used by.** Every function in [`crate::physics_utils::relativity`] — as the limit speed in
/// [`lorentz_factor`](crate::physics_utils::relativity::lorentz::lorentz_factor), as the `c²` in the
/// velocity transformation's denominator, and as the boundary the domain checks compare against.
///
/// **If you change it.** Do not. It is definitional, and changing it would silently redefine the
/// metre for this crate while leaving `G` — whose units contain metres — describing the old one, so
/// gravitation and relativity would stop agreeing about length. If you want a scenario where light is
/// slow enough to see the effects, scale the *velocities* up instead, which is what
/// `beta_from_velocity` exists to make readable.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/constants_tests.rs"]
mod tests;
