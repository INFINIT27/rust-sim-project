//! General relativity's closed-form results around one spherical, non-rotating mass.
//!
//! Everything else in this sub-layer is special relativity: flat spacetime, no gravity. This module
//! is the part of general relativity that reduces to a formula. It is **not** a solver. Nothing here
//! bends a trajectory or steps an orbit; each function is a number that falls out of the
//! Schwarzschild solution, the exact description of spacetime outside a spherical mass.
//!
//! # The one length that sets the scale
//!
//! The Schwarzschild radius `r_s = 2GM/c²` ([`schwarzschild_radius`]). Every effect here is a
//! function of `r_s/r`, and that ratio is tiny everywhere in the solar system:
//!
//! | Body | `r_s` | `r_s / radius` |
//! | --- | --- | --- |
//! | Earth | `8.87 mm` | `1.4e-9` |
//! | The Sun | `2.95 km` | `4.2e-6` |
//! | A neutron star (`1.4 M☉`, `12 km`) | `4.1 km` | `0.34` |
//! | A black hole | — | `1` at the horizon |
//!
//! # What is exact and what is first-order
//!
//! | Function | Validity |
//! | --- | --- |
//! | [`schwarzschild_radius`], [`gravitational_time_dilation_factor`], [`gravitational_rate_deficit`], [`gravitational_redshift`] | Exact outside a spherical non-rotating mass, for clocks at rest. |
//! | [`perihelion_precession_per_orbit`], [`light_deflection_angle`], [`orbiting_clock_rate_offset`] | First order in `r_s/r`. Good to about `r_s/r` itself: parts per million at the Sun's surface, useless near a black hole. |
//!
//! # Radius means the Schwarzschild coordinate `r`
//!
//! Defined so that a circle at `r` has circumference `2πr`. In the solar system it is the ordinary
//! distance from the centre to within `r_s`.
//!
//! # Conventions
//!
//! Masses in kilograms, as in the rest of [`crate::physics_utils`], with
//! [`GRAVITATIONAL_CONSTANT`]. Note that `G` is known to five digits while the product `GM` for the
//! Sun and Earth is known to ten, so results here carry `G`'s uncertainty (`2e-5` relative).
//! `None` for a question with no answer, as everywhere in this sub-layer.

use super::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

/// Whether a value is finite and strictly positive.
fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// The Schwarzschild radius `r_s = 2GM/c²`, in metres.
///
/// The radius of the event horizon of a non-rotating black hole of this mass, and for anything else
/// the length that measures how relativistic its gravity is.
///
/// # Returns
///
/// `None` if the mass is not positive and finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::schwarzschild_radius;
///
/// // The Sun: 2.95 km.
/// let radius = schwarzschild_radius(1.988_47e30).unwrap();
/// assert!((radius - 2_953.0).abs() < 1.0);
///
/// // Earth: just under nine millimetres.
/// let radius = schwarzschild_radius(5.972_2e24).unwrap();
/// assert!((radius - 8.87e-3).abs() < 1.0e-5);
/// ```
pub fn schwarzschild_radius(mass_kg: f64) -> Option<f64> {
    if !positive(mass_kg) {
        return None;
    }

    // Divide by `c` twice rather than by `c²`, keeping the intermediate values moderate.
    let radius = 2.0 * GRAVITATIONAL_CONSTANT * mass_kg / SPEED_OF_LIGHT_M_S / SPEED_OF_LIGHT_M_S;

    positive(radius).then_some(radius)
}

/// `r_s/r`, for a radius strictly outside the horizon. `None` at or inside it.
fn compactness(mass_kg: f64, radius_m: f64) -> Option<f64> {
    if !positive(radius_m) {
        return None;
    }

    let ratio = schwarzschild_radius(mass_kg)? / radius_m;

    (ratio < 1.0).then_some(ratio)
}

/// How fast a clock at rest at `radius_m` runs, compared with one far away: `√(1 − r_s/r)`.
///
/// Between `0` and `1`. Clocks lower in a gravitational field run slower. The far-away clock's
/// elapsed time is multiplied by this to get the deep clock's.
///
/// # Returns
///
/// `None` if the mass or radius is not positive and finite, or `radius_m ≤ r_s`: at the horizon the
/// factor is zero and inside it no clock can stay at rest.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::{
///     gravitational_time_dilation_factor, schwarzschild_radius,
/// };
///
/// let mass = 1.988_47e30;
/// let horizon = schwarzschild_radius(mass).unwrap();
///
/// // At 4/3 of the horizon radius clocks run at half speed.
/// let factor = gravitational_time_dilation_factor(mass, horizon * 4.0 / 3.0).unwrap();
/// assert!((factor - 0.5).abs() < 1.0e-12);
///
/// assert_eq!(gravitational_time_dilation_factor(mass, horizon), None);
/// ```
pub fn gravitational_time_dilation_factor(mass_kg: f64, radius_m: f64) -> Option<f64> {
    let ratio = compactness(mass_kg, radius_m)?;

    Some((1.0 - ratio).sqrt())
}

/// The fraction by which a clock at rest at `radius_m` runs slow: `1 − √(1 − r_s/r)`.
///
/// The gravitational counterpart of
/// [`lorentz_factor_minus_one`](super::lorentz::lorentz_factor_minus_one), and it exists for the
/// same reason. On Earth's surface this is `7e-10`, and `1.0 - gravitational_time_dilation_factor`
/// keeps only seven digits of it. Computed as `x/(1 + √(1 − x))` with `x = r_s/r`, which subtracts
/// nothing.
///
/// For a weak field it is `GM/(rc²)`, the Newtonian potential over `c²`.
///
/// # Returns
///
/// A fraction in `(0, 1)`: seconds lost per second of far-away time. `None` on the same domain as
/// [`gravitational_time_dilation_factor`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::gravitational_rate_deficit;
///
/// // A clock on Earth's surface loses 60 microseconds a day to Earth's gravity, against a clock
/// // far from Earth.
/// let deficit = gravitational_rate_deficit(5.972_2e24, 6.371e6).unwrap();
/// assert!((deficit * 86_400.0 * 1.0e6 - 60.1).abs() < 0.1);
/// ```
pub fn gravitational_rate_deficit(mass_kg: f64, radius_m: f64) -> Option<f64> {
    let ratio = compactness(mass_kg, radius_m)?;

    Some(ratio / (1.0 + (1.0 - ratio).sqrt()))
}

/// Gravitational redshift `z = λ_observed/λ_emitted − 1` for light sent between two clocks at rest
/// at different radii.
///
/// `1 + z = √((1 − r_s/r_observer)/(1 − r_s/r_emitter))`. Positive when the light climbs out
/// (emitter deeper: a redshift), negative when it falls in (a blueshift). Computed without
/// subtracting nearly-equal numbers, so the `2.5e-15` of a laboratory experiment survives.
///
/// # Returns
///
/// `None` if the mass or a radius is not positive and finite, or either radius is at or inside the
/// horizon.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::gravitational_redshift;
///
/// // Light leaving the Sun's surface for a distant observer: 2.12 parts per million.
/// let z = gravitational_redshift(1.988_47e30, 6.957e8, 1.0e20).unwrap();
/// assert!((z - 2.12e-6).abs() < 1.0e-8);
///
/// // The same radius at both ends: none.
/// assert_eq!(gravitational_redshift(1.988_47e30, 6.957e8, 6.957e8), Some(0.0));
/// ```
pub fn gravitational_redshift(
    mass_kg: f64,
    emitter_radius_m: f64,
    observer_radius_m: f64,
) -> Option<f64> {
    let at_emitter = compactness(mass_kg, emitter_radius_m)?;
    let at_observer = compactness(mass_kg, observer_radius_m)?;
    let horizon = schwarzschild_radius(mass_kg)?;

    // With `a = 1 − x_obs` and `b = 1 − x_emit`: `z = √(a/b) − 1 = (a − b)/(b·(√(a/b) + 1))`, and
    // `a − b = x_emit − x_obs = r_s·(r_obs − r_emit)/(r_emit·r_obs)`. The radii are subtracted
    // directly, so two radii a few metres apart do not cancel to nothing.
    let a = 1.0 - at_observer;
    let b = 1.0 - at_emitter;
    let difference =
        horizon * ((observer_radius_m - emitter_radius_m) / emitter_radius_m / observer_radius_m);
    let z = difference / (b * ((a / b).sqrt() + 1.0));

    z.is_finite().then_some(z)
}

/// How far an orbit's closest point advances each revolution: `Δφ = 6πGM/(c²·a·(1 − e²))`.
///
/// A Newtonian orbit around one mass is a closed ellipse. In general relativity the ellipse turns
/// slowly in its own plane, in the direction of the orbit. For Mercury this is the `43″` per century
/// that Newtonian gravity could not account for, and its explanation was the theory's first success.
///
/// First order in `r_s/a`.
///
/// # Parameters
///
/// - `mass_kg`: the central mass. The orbiting body is taken to be much lighter.
/// - `semi_major_axis_m`: positive and finite.
/// - `eccentricity`: in `[0, 1)`. A parabola or hyperbola does not come back and has no perihelion
///   to advance.
///
/// # Returns
///
/// Radians per orbit, positive. `None` for an input outside those ranges.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::perihelion_precession_per_orbit;
///
/// // Mercury: 0.1035 arcseconds per orbit, and 415.2 orbits per century.
/// let per_orbit = perihelion_precession_per_orbit(1.988_47e30, 5.790_9e10, 0.205_63).unwrap();
/// let arcseconds_per_century = per_orbit.to_degrees() * 3_600.0 * 415.2;
/// assert!((arcseconds_per_century - 42.98).abs() < 0.05);
/// ```
pub fn perihelion_precession_per_orbit(
    mass_kg: f64,
    semi_major_axis_m: f64,
    eccentricity: f64,
) -> Option<f64> {
    if !positive(semi_major_axis_m)
        || !eccentricity.is_finite()
        || !(0.0..1.0).contains(&eccentricity)
    {
        return None;
    }

    // `6πGM/c² = 3π·r_s`, and `a(1 − e²)` is the orbit's semi-latus rectum.
    let horizon = schwarzschild_radius(mass_kg)?;
    let semi_latus_rectum = semi_major_axis_m * (1.0 - eccentricity) * (1.0 + eccentricity);
    let precession = 3.0 * std::f64::consts::PI * horizon / semi_latus_rectum;

    precession.is_finite().then_some(precession)
}

/// The angle by which a mass bends a passing ray of light: `α = 4GM/(c²·b)`.
///
/// `impact_parameter_m` is how close the ray would have passed to the centre had it not been bent.
/// The result is **twice** what Newtonian gravity gives for a particle moving at `c`. Eddington's
/// 1919 eclipse expedition measured the factor of two.
///
/// First order in `r_s/b`: for a ray grazing the Sun the neglected terms are a few millionths of the
/// answer. It is not valid for a ray passing near a black hole.
///
/// # Returns
///
/// Radians, positive. `None` if the mass or impact parameter is not positive and finite, or the
/// impact parameter is not outside the Schwarzschild radius.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::light_deflection_angle;
///
/// // Starlight grazing the Sun: 1.75 arcseconds.
/// let angle = light_deflection_angle(1.988_47e30, 6.957e8).unwrap();
/// assert!((angle.to_degrees() * 3_600.0 - 1.751).abs() < 0.001);
/// ```
pub fn light_deflection_angle(mass_kg: f64, impact_parameter_m: f64) -> Option<f64> {
    let ratio = compactness(mass_kg, impact_parameter_m)?;

    Some(2.0 * ratio)
}

/// How much faster a clock in a circular orbit runs than a clock at rest on the ground, as a
/// fraction: `(GM/c²)·(1/R − 3/(2r))`.
///
/// Two effects with opposite signs. The orbiting clock is higher, so it runs **fast** by
/// `(GM/c²)(1/R − 1/r)`. It is also moving at `v² = GM/r`, so it runs **slow** by `v²/2c²`, which is
/// the [`time_dilation`](super::time_dilation) of the rest of this sub-layer. Their sum is this
/// function.
///
/// | Orbit | Gravity | Speed | Net |
/// | --- | --- | --- | --- |
/// | ISS, `400 km` | `+3.5 µs/day` | `−28.2 µs/day` | `−24.7 µs/day` |
/// | GPS, `20,200 km` | `+45.7 µs/day` | `−7.2 µs/day` | `+38.5 µs/day` |
///
/// The sign changes at `r = 1.5·R`: below about `3,200 km` altitude a satellite's clock runs slow,
/// above it fast. GPS satellites have their clocks set `38.5 µs/day` slow on the ground so that
/// they keep ground time in orbit.
///
/// First order, and it leaves out the ground clock's own motion with Earth's rotation (under
/// `0.1 µs/day`) and Earth's oblateness.
///
/// # Returns
///
/// A signed fraction: seconds gained (positive) or lost per second. `None` if an input is not
/// positive and finite, or the orbit is below the ground (`orbit_radius_m < ground_radius_m`).
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::gravity::orbiting_clock_rate_offset;
///
/// let earth_mass = 5.972_2e24;
/// let earth_radius = 6.371e6;
///
/// // GPS: +38.5 microseconds a day.
/// let offset = orbiting_clock_rate_offset(earth_mass, earth_radius, 2.656e7).unwrap();
/// assert!((offset * 86_400.0 * 1.0e6 - 38.5).abs() < 0.1);
///
/// // The ISS: slow, not fast.
/// assert!(orbiting_clock_rate_offset(earth_mass, earth_radius, 6.771e6).unwrap() < 0.0);
/// ```
pub fn orbiting_clock_rate_offset(
    mass_kg: f64,
    ground_radius_m: f64,
    orbit_radius_m: f64,
) -> Option<f64> {
    if !positive(ground_radius_m) || !positive(orbit_radius_m) || orbit_radius_m < ground_radius_m {
        return None;
    }

    // `GM/c² = r_s/2`.
    let half_horizon = 0.5 * schwarzschild_radius(mass_kg)?;
    let offset = half_horizon * (1.0 / ground_radius_m - 1.5 / orbit_radius_m);

    offset.is_finite().then_some(offset)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/gravity_tests.rs"]
mod tests;
