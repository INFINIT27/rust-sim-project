//! Spinning black holes: the Kerr solution, its orbits, its accretion disc, and motion near it.
//!
//! [`gravity`](super::gravity) is the Schwarzschild solution: a black hole that does not spin.
//! Every real black hole spins, and a spinning one drags space round with it. This module is the
//! Kerr solution, the exact spacetime of a spinning, uncharged black hole, described by its mass `M`
//! and its spin `a = J/(Mc)`, given here as the dimensionless `a/M` between `0` (Schwarzschild) and
//! `1` (the fastest a black hole can spin).
//!
//! # Units
//!
//! Everything about the shape of the spacetime is quoted in **gravitational radii**,
//! `r_g = GM/c²` (half the Schwarzschild radius), and **gravitational times**, `t_g = GM/c³`. In those
//! units every Kerr hole of a given spin is the same, and the formulas are the textbook ones with
//! `G = c = M = 1`. [`KerrBlackHole`] converts to metres and seconds for a real mass.
//!
//! # What spin changes
//!
//! | Quantity | `a = 0` | `a = 0.9` | `a = 1` |
//! | --- | --- | --- | --- |
//! | Horizon `r₊` | `2` | `1.44` | `1` |
//! | ISCO, prograde | `6` | `2.32` | `1` |
//! | ISCO, retrograde | `6` | `8.72` | `9` |
//! | Photon orbit, prograde | `3` | `1.56` | `1` |
//! | Photon orbit, retrograde | `3` | `3.91` | `4` |
//! | Disc efficiency `1 − E_ISCO` | `5.7%` | `15.6%` | `42.3%` |
//!
//! (radii in `r_g`). Matter orbiting **with** the spin can circle far closer in than around a
//! Schwarzschild hole before plunging, so it falls deeper into the well before it is lost and
//! radiates more of its rest energy on the way: this is why spinning holes power the brightest
//! quasars.
//!
//! # The ergosphere: where nothing can stand still
//!
//! Inside `r = 1 + √(1 − a² cos²θ)` (twice `r_g` at the equator, the horizon at the poles) space
//! is dragged round faster than light could move against it, so everything there, light included,
//! must co-rotate with the hole. That is "frame dragging" at its most extreme.
//!
//! # The accretion disc
//!
//! Gas orbiting the hole in its equatorial plane rubs against itself: faster inner rings drag on
//! slower outer ones. The friction carries angular momentum outward, so gas spirals slowly inward,
//! and the energy it releases is radiated from the disc's faces. The standard model of a thin disc
//! in the Kerr metric (Novikov–Thorne 1973, Page–Thorne 1974) gives the radiated flux at each radius
//! from conservation of energy and angular momentum alone, with no details of the friction needed:
//! [`disc_flux`]. It is zero at the ISCO (nothing inside it is held in orbit to rub), peaks a little
//! outside, and falls as `r⁻³` far out. The temperature follows from Stefan–Boltzmann.
//!
//! # Motion: Kerr–Schild coordinates
//!
//! [`kerr_schild_acceleration`] is the exact equation of motion for a test body or a ray of light,
//! in **Kerr–Schild** coordinates: Cartesian `x, y, z` with the spin along `+z`, and a time `t`
//! that agrees with a distant observer's far from the hole. Unlike the Boyer–Lindquist coordinates
//! most formulas are quoted in, these are regular at the horizon: matter and light cross it in a
//! finite time, so nothing has to be stopped just outside. A circular equatorial orbit of
//! Boyer–Lindquist radius `r` is a circle of Cartesian radius `√(r² + a²)`.
//!
//! The Boyer–Lindquist radius at a Kerr–Schild point is [`kerr_schild_radius`]; the horizon is
//! where it equals `r₊`, an oblate surface of equatorial radius `√(r₊² + a²)` and polar radius
//! `r₊`.

use super::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

/// The Stefan–Boltzmann constant, `σ`, in W·m⁻²·K⁻⁴ (exact since the 2019 SI).
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;

/// The proton mass, in kilograms (CODATA 2018). Sets the Eddington limit.
pub const PROTON_MASS_KG: f64 = 1.672_621_923_69e-27;

/// The Thomson cross-section of the electron, in m² (CODATA 2018). Sets the Eddington limit.
pub const THOMSON_CROSS_SECTION_M2: f64 = 6.652_458_732_1e-29;

/// Whether `spin` is a usable dimensionless spin: finite and in `[0, 1)`.
///
/// `1` itself is excluded: an extremal hole has a degenerate horizon, and its ISCO and photon
/// orbit coincide with the horizon in these coordinates.
fn valid_spin(spin: f64) -> bool {
    spin.is_finite() && (0.0..1.0).contains(&spin)
}

/// The outer event horizon's Boyer–Lindquist radius, `r₊ = 1 + √(1 − a²)`, in `r_g`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::horizon_radius;
///
/// assert_eq!(horizon_radius(0.0), Some(2.0));
/// assert!((horizon_radius(0.6).unwrap() - 1.8).abs() < 1.0e-12);
/// ```
pub fn horizon_radius(spin: f64) -> Option<f64> {
    valid_spin(spin).then(|| 1.0 + (1.0 - spin * spin).sqrt())
}

/// The ergosphere's outer boundary at polar angle `polar_angle_rad` from the spin axis,
/// `r = 1 + √(1 − a² cos²θ)`, in `r_g` (Boyer–Lindquist).
///
/// `2 r_g` at the equator for every spin; at the poles it touches the horizon.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::{ergosphere_radius, horizon_radius};
/// use std::f64::consts::FRAC_PI_2;
///
/// assert_eq!(ergosphere_radius(0.9, FRAC_PI_2), Some(2.0));
/// assert_eq!(ergosphere_radius(0.9, 0.0), horizon_radius(0.9));
/// ```
pub fn ergosphere_radius(spin: f64, polar_angle_rad: f64) -> Option<f64> {
    if !valid_spin(spin) || !polar_angle_rad.is_finite() {
        return None;
    }
    let cos = polar_angle_rad.cos();
    Some(1.0 + (1.0 - spin * spin * cos * cos).sqrt())
}

/// The radius of the innermost stable circular orbit in the equatorial plane, in `r_g`
/// (Bardeen, Press & Teukolsky 1972).
///
/// `prograde` is an orbit in the direction of the spin. `6` for a non-spinning hole; prograde it
/// falls to `1` as the spin approaches `1`, retrograde it rises to `9`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::isco_radius;
///
/// assert!((isco_radius(0.0, true).unwrap() - 6.0).abs() < 1.0e-12);
/// assert!((isco_radius(0.9, true).unwrap() - 2.3209).abs() < 1.0e-4);
/// assert!((isco_radius(0.9, false).unwrap() - 8.7173).abs() < 1.0e-4);
/// ```
pub fn isco_radius(spin: f64, prograde: bool) -> Option<f64> {
    if !valid_spin(spin) {
        return None;
    }
    let a = spin;
    let z1 = 1.0 + (1.0 - a * a).cbrt() * ((1.0 + a).cbrt() + (1.0 - a).cbrt());
    let z2 = (3.0 * a * a + z1 * z1).sqrt();
    let root = ((3.0 - z1) * (3.0 + z1 + 2.0 * z2)).sqrt();
    Some(if prograde {
        3.0 + z2 - root
    } else {
        3.0 + z2 + root
    })
}

/// The radius of the circular photon orbit in the equatorial plane, in `r_g`:
/// `r = 2(1 + cos(⅔ arccos(∓a)))`.
///
/// `3` for a non-spinning hole; prograde it falls towards `1`, retrograde it rises towards `4`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::photon_orbit_radius;
///
/// assert!((photon_orbit_radius(0.0, true).unwrap() - 3.0).abs() < 1.0e-12);
/// assert!(photon_orbit_radius(0.9, true).unwrap() < 3.0);
/// assert!(photon_orbit_radius(0.9, false).unwrap() > 3.0);
/// ```
pub fn photon_orbit_radius(spin: f64, prograde: bool) -> Option<f64> {
    if !valid_spin(spin) {
        return None;
    }
    let signed = if prograde { -spin } else { spin };
    Some(2.0 * (1.0 + (2.0 / 3.0 * signed.acos()).cos()))
}

/// The angular velocity of the horizon, `Ω_H = a/(2 r₊)`, in radians per `t_g`.
///
/// The rate at which anything at the horizon is dragged round: the hole's own rotation rate, as far
/// as an outside observer can tell.
pub fn horizon_angular_velocity(spin: f64) -> Option<f64> {
    Some(spin / (2.0 * horizon_radius(spin)?))
}

/// The angular velocity at which space is dragged round at Boyer–Lindquist radius `r` in the
/// equatorial plane, `ω = 2a/(r³ + a²r + 2a²)`, in radians per `t_g`.
///
/// The rate at which a body with no angular momentum of its own is carried round. Equals
/// [`horizon_angular_velocity`] at the horizon and falls as `2a/r³` far away.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::{
///     frame_dragging_angular_velocity, horizon_angular_velocity, horizon_radius,
/// };
///
/// let r_plus = horizon_radius(0.9).unwrap();
/// let at_horizon = frame_dragging_angular_velocity(0.9, r_plus).unwrap();
/// assert!((at_horizon - horizon_angular_velocity(0.9).unwrap()).abs() < 1.0e-12);
/// ```
pub fn frame_dragging_angular_velocity(spin: f64, radius: f64) -> Option<f64> {
    if !valid_spin(spin) || !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let a = spin;
    Some(2.0 * a / (radius * radius * radius + a * a * radius + 2.0 * a * a))
}

/// A circular orbit in the equatorial plane: its angular velocity, and the energy and angular
/// momentum per unit rest mass of the body on it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CircularOrbit {
    /// Angular velocity as a distant observer measures it, `dφ/dt`, in radians per `t_g`. Positive
    /// for prograde.
    pub angular_velocity: f64,
    /// Energy per unit rest energy, `E/(mc²)`. Below `1` for a bound orbit.
    pub energy: f64,
    /// Angular momentum per unit mass, in `r_g·c`. Positive for prograde.
    pub angular_momentum: f64,
}

/// The circular equatorial orbit at Boyer–Lindquist radius `r` (Bardeen, Press & Teukolsky 1972):
/// `Ω = ±1/(r^{3/2} ± a)` and the matching energy and angular momentum.
///
/// # Returns
///
/// `None` if no circular orbit for a massive body exists there (at or inside the photon orbit for
/// that direction), or an input is not usable. Orbits between the photon orbit and the ISCO exist
/// but are unstable.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::{circular_orbit, isco_radius};
///
/// // Schwarzschild at 6 r_g: E = √(8/9).
/// let orbit = circular_orbit(0.0, 6.0, true).unwrap();
/// assert!((orbit.energy - (8.0_f64 / 9.0).sqrt()).abs() < 1.0e-12);
///
/// // Far out it is Kepler's.
/// let far = circular_orbit(0.9, 1.0e6, true).unwrap();
/// assert!((far.angular_velocity * 1.0e9 - 1.0).abs() < 1.0e-5);
/// ```
pub fn circular_orbit(spin: f64, radius: f64, prograde: bool) -> Option<CircularOrbit> {
    if !valid_spin(spin) || !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let a = if prograde { spin } else { -spin };
    let sqrt_r = radius.sqrt();
    let r_three_halves = radius * sqrt_r;
    let denominator_squared = r_three_halves - 3.0 * sqrt_r + 2.0 * a;
    if denominator_squared <= 0.0 {
        return None;
    }
    let denominator = radius.powf(0.75) * denominator_squared.sqrt();
    let sign = if prograde { 1.0 } else { -1.0 };

    let orbit = CircularOrbit {
        angular_velocity: sign / (r_three_halves + a),
        energy: (r_three_halves - 2.0 * sqrt_r + a) / denominator,
        angular_momentum: sign * (radius * radius - 2.0 * a * sqrt_r + a * a) / denominator,
    };

    (orbit.energy.is_finite() && orbit.angular_momentum.is_finite()).then_some(orbit)
}

/// The fraction of rest energy a thin disc radiates, `1 − E_ISCO`, for prograde accretion.
///
/// `5.72%` for a non-spinning hole, `15.6%` at `a = 0.9`, `32.1%` at `a = 0.998` (Thorne's limit for
/// a hole spun up by its own disc). Compare hydrogen fusion's `0.7%`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::radiative_efficiency;
///
/// assert!((radiative_efficiency(0.0).unwrap() - 0.0572).abs() < 1.0e-4);
/// assert!((radiative_efficiency(0.998).unwrap() - 0.321).abs() < 1.0e-3);
/// ```
pub fn radiative_efficiency(spin: f64) -> Option<f64> {
    let isco = isco_radius(spin, true)?;
    Some(1.0 - circular_orbit(spin, isco, true)?.energy)
}

/// The flux a thin accretion disc radiates from each face at Boyer–Lindquist radius `r`, per unit
/// accretion rate: `F / (Ṁc²/r_g²)`, dimensionless (Page & Thorne 1974).
///
/// ```text
///              1     dΩ/dr       r
/// F̂(r) = − ──── · ────────── · ∫    (E − ΩL) (dL/dr) dr
///            4πr   (E − ΩL)²    r_ISCO
/// ```
///
/// with `E`, `L`, `Ω` those of prograde circular orbits ([`circular_orbit`]). Zero at the ISCO, a
/// peak a little outside it, and `3/(8π r³)` far out, the Newtonian thin disc. The integral is
/// evaluated by Simpson's rule and the derivatives by central differences.
///
/// # Returns
///
/// `None` inside the ISCO (the model has no disc there) or for unusable inputs.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::{disc_flux, isco_radius};
///
/// let isco = isco_radius(0.9, true).unwrap();
/// assert_eq!(disc_flux(0.9, isco), Some(0.0));
/// assert!(disc_flux(0.9, 2.0 * isco).unwrap() > 0.0);
///
/// // Far out, the Newtonian 3/(8πr³).
/// let far = disc_flux(0.9, 1.0e6).unwrap();
/// assert!((far / (3.0 / (8.0 * std::f64::consts::PI * 1.0e18)) - 1.0).abs() < 0.01);
/// ```
pub fn disc_flux(spin: f64, radius: f64) -> Option<f64> {
    let isco = isco_radius(spin, true)?;
    if !radius.is_finite() || radius < isco {
        return None;
    }
    if radius == isco {
        return Some(0.0);
    }

    let orbit = |r: f64| circular_orbit(spin, r, true);
    let derivative = |f: &dyn Fn(f64) -> Option<f64>, r: f64| -> Option<f64> {
        let h = 1.0e-5 * r;
        Some((f(r + h)? - f(r - h)?) / (2.0 * h))
    };
    let angular_momentum = |r: f64| orbit(r).map(|o| o.angular_momentum);
    let angular_velocity = |r: f64| orbit(r).map(|o| o.angular_velocity);

    // The integrand vanishes like √(r − r_ISCO) at the lower end (dL/dr does), so Simpson's rule on
    // a grid uniform in √(r − r_ISCO) keeps its accuracy. With s = √(r − r_ISCO), dr = 2s ds.
    let steps = 400;
    let span = (radius - isco).sqrt();
    let integrand = |s: f64| -> Option<f64> {
        if s == 0.0 {
            return Some(0.0);
        }
        // Keep the derivative's stencil outside the ISCO.
        let r = isco + s * s;
        let o = orbit(r)?;
        let dl = derivative(&angular_momentum, r.max(isco * (1.0 + 2.0e-5)))?;
        Some((o.energy - o.angular_velocity * o.angular_momentum) * dl * 2.0 * s)
    };

    let h = span / f64::from(steps);
    let mut sum = integrand(0.0)? + integrand(span)?;
    for i in 1..steps {
        let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
        sum += weight * integrand(h * f64::from(i))?;
    }
    let integral = sum * h / 3.0;

    let o = orbit(radius)?;
    let d_omega = derivative(&angular_velocity, radius)?;
    let binding = o.energy - o.angular_velocity * o.angular_momentum;
    let flux = -d_omega / (binding * binding) * integral / (4.0 * std::f64::consts::PI * radius);

    flux.is_finite().then_some(flux.max(0.0))
}

/// A spinning black hole of a real mass: the Kerr quantities above, in SI units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct KerrBlackHole {
    /// The mass, in kilograms.
    pub mass_kg: f64,
    /// The dimensionless spin `a/M`, in `[0, 1)`.
    pub spin: f64,
}

impl KerrBlackHole {
    /// A hole of `mass_kg` and dimensionless `spin`, or `None` if the mass is not positive and
    /// finite or the spin is not in `[0, 1)`.
    pub fn new(mass_kg: f64, spin: f64) -> Option<Self> {
        (mass_kg.is_finite() && mass_kg > 0.0 && valid_spin(spin)).then_some(Self { mass_kg, spin })
    }

    /// The gravitational radius `r_g = GM/c²`, in metres.
    pub fn gravitational_radius_m(&self) -> f64 {
        GRAVITATIONAL_CONSTANT * self.mass_kg / (SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S)
    }

    /// The gravitational time `t_g = GM/c³`, in seconds.
    pub fn gravitational_time_s(&self) -> f64 {
        self.gravitational_radius_m() / SPEED_OF_LIGHT_M_S
    }

    /// The Eddington luminosity, `4πGMm_pc/σ_T`, in watts: where radiation pressure on ionised
    /// hydrogen balances gravity.
    pub fn eddington_luminosity_w(&self) -> f64 {
        4.0 * std::f64::consts::PI
            * GRAVITATIONAL_CONSTANT
            * self.mass_kg
            * PROTON_MASS_KG
            * SPEED_OF_LIGHT_M_S
            / THOMSON_CROSS_SECTION_M2
    }

    /// The accretion rate, in kg/s, at which a thin disc radiates `eddington_fraction` of the
    /// Eddington luminosity: `Ṁ = λ L_Edd / (η c²)`.
    pub fn accretion_rate_kg_s(&self, eddington_fraction: f64) -> Option<f64> {
        if !eddington_fraction.is_finite() || eddington_fraction <= 0.0 {
            return None;
        }
        let efficiency = radiative_efficiency(self.spin)?;
        Some(
            eddington_fraction * self.eddington_luminosity_w()
                / (efficiency * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S),
        )
    }

    /// The flux a thin disc fed at `accretion_rate_kg_s` radiates from each face at
    /// Boyer–Lindquist radius `radius_rg`, in W/m². See [`disc_flux`].
    pub fn disc_flux_w_m2(&self, radius_rg: f64, accretion_rate_kg_s: f64) -> Option<f64> {
        let r_g = self.gravitational_radius_m();
        let scale = accretion_rate_kg_s * SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S / (r_g * r_g);
        Some(disc_flux(self.spin, radius_rg)? * scale)
    }

    /// The disc's surface temperature at `radius_rg`, in kelvin: `(F/σ)^{1/4}`, as an observer
    /// riding with the gas would measure it.
    pub fn disc_temperature_k(&self, radius_rg: f64, accretion_rate_kg_s: f64) -> Option<f64> {
        Some((self.disc_flux_w_m2(radius_rg, accretion_rate_kg_s)? / STEFAN_BOLTZMANN).powf(0.25))
    }
}

// -----------------------------------------------------------------------
// Kerr–Schild coordinates and the equation of motion.
// -----------------------------------------------------------------------

/// The Boyer–Lindquist radius `r` at Kerr–Schild point `position` (in `r_g`), the positive root of
/// `r⁴ − (ρ² − a²) r² − a² z² = 0` with `ρ² = x² + y² + z²`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::kerr_schild_radius;
///
/// // Without spin it is the distance from the centre.
/// assert!((kerr_schild_radius(0.0, [3.0, 4.0, 0.0]) - 5.0).abs() < 1.0e-12);
/// // With spin, a circle of radius √(r² + a²) in the equatorial plane has radius r.
/// assert!((kerr_schild_radius(0.6, [(4.0_f64 + 0.36).sqrt(), 0.0, 0.0]) - 2.0).abs() < 1.0e-12);
/// ```
pub fn kerr_schild_radius(spin: f64, position: [f64; 3]) -> f64 {
    let [x, y, z] = position;
    let a2 = spin * spin;
    let rho2 = x * x + y * y + z * z;
    let b = rho2 - a2;
    let r2 = 0.5 * (b + (b * b + 4.0 * a2 * z * z).sqrt());
    r2.sqrt()
}

type Metric = [[f64; 4]; 4];

/// The Kerr–Schild metric `g_μν = η_μν + f k_μ k_ν` at `position` (in `r_g`, `M = 1`), with
/// `f = 2r³/(r⁴ + a²z²)` and `k = (1, (rx + ay)/(r² + a²), (ry − ax)/(r² + a²), z/r)`. Index `0` is
/// time.
pub fn kerr_schild_metric(spin: f64, position: [f64; 3]) -> Metric {
    let [x, y, z] = position;
    let a = spin;
    let r = kerr_schild_radius(spin, position);
    let r2 = r * r;
    let f = 2.0 * r2 * r / (r2 * r2 + a * a * z * z);
    let k = [
        1.0,
        (r * x + a * y) / (r2 + a * a),
        (r * y - a * x) / (r2 + a * a),
        z / r,
    ];

    let mut g = [[0.0; 4]; 4];
    for (mu, row) in g.iter_mut().enumerate() {
        for (nu, entry) in row.iter_mut().enumerate() {
            let eta = if mu != nu {
                0.0
            } else if mu == 0 {
                -1.0
            } else {
                1.0
            };
            *entry = eta + f * k[mu] * k[nu];
        }
    }
    g
}

/// The inverse of a 4×4 symmetric metric, by Gauss–Jordan elimination with partial pivoting.
#[cfg(test)]
fn invert(matrix: Metric) -> Option<Metric> {
    let mut a = matrix;
    let mut inverse = [[0.0; 4]; 4];
    for (i, row) in inverse.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for column in 0..4 {
        let pivot =
            (column..4).max_by(|&p, &q| a[p][column].abs().total_cmp(&a[q][column].abs()))?;
        if a[pivot][column].abs() < 1.0e-300 {
            return None;
        }
        a.swap(column, pivot);
        inverse.swap(column, pivot);
        let scale = a[column][column];
        for k in 0..4 {
            a[column][k] /= scale;
            inverse[column][k] /= scale;
        }
        for row in 0..4 {
            if row != column {
                let factor = a[row][column];
                for k in 0..4 {
                    a[row][k] -= factor * a[column][k];
                    inverse[row][k] -= factor * inverse[column][k];
                }
            }
        }
    }
    Some(inverse)
}

/// `V^T g V` for the four-vector `V = (1, v)`.
fn norm(g: &Metric, v: [f64; 3]) -> f64 {
    let full = [1.0, v[0], v[1], v[2]];
    let mut total = 0.0;
    for mu in 0..4 {
        for nu in 0..4 {
            total += g[mu][nu] * full[mu] * full[nu];
        }
    }
    total
}

/// The pieces of the Kerr–Schild metric `g = η + f k k` at `position`, and their spatial
/// derivatives: `(f, k, ∂_j f, ∂_j k_μ)`, all analytic.
///
/// `∂_j r` comes from differentiating `r⁴ − (ρ² − a²) r² − a² z² = 0`:
/// `∂_j r = (r² x_j + a² z δ_jz) / (r (2r² − ρ² + a²))`.
#[allow(clippy::type_complexity)]
fn metric_pieces(spin: f64, position: [f64; 3]) -> (f64, [f64; 4], [f64; 3], [[f64; 4]; 3]) {
    let [x, y, z] = position;
    let a = spin;
    let a2 = a * a;
    let r = kerr_schild_radius(spin, position);
    let r2 = r * r;
    let rho2 = x * x + y * y + z * z;

    let mut dr = [0.0; 3];
    let denominator = r * (2.0 * r2 - rho2 + a2);
    for (j, entry) in dr.iter_mut().enumerate() {
        let z_term = if j == 2 { a2 * z } else { 0.0 };
        *entry = (r2 * position[j] + z_term) / denominator;
    }

    // f = 2r³ / (r⁴ + a²z²)
    let q = r2 * r2 + a2 * z * z;
    let f = 2.0 * r2 * r / q;
    let mut df = [0.0; 3];
    for (j, entry) in df.iter_mut().enumerate() {
        let dz = if j == 2 { 1.0 } else { 0.0 };
        let dq = 4.0 * r2 * r * dr[j] + 2.0 * a2 * z * dz;
        *entry = (6.0 * r2 * dr[j] * q - 2.0 * r2 * r * dq) / (q * q);
    }

    // k = (1, (r x + a y)/(r² + a²), (r y − a x)/(r² + a²), z/r)
    let s = r2 + a2;
    let num_x = r * x + a * y;
    let num_y = r * y - a * x;
    let k = [1.0, num_x / s, num_y / s, z / r];
    let mut dk = [[0.0; 4]; 3];
    for j in 0..3 {
        let delta = |i: usize| if i == j { 1.0 } else { 0.0 };
        let ds = 2.0 * r * dr[j];
        let d_num_x = dr[j] * x + r * delta(0) + a * delta(1);
        let d_num_y = dr[j] * y + r * delta(1) - a * delta(0);
        dk[j][1] = (d_num_x * s - num_x * ds) / (s * s);
        dk[j][2] = (d_num_y * s - num_y * ds) / (s * s);
        dk[j][3] = (delta(2) * r - z * dr[j]) / r2;
    }

    (f, k, df, dk)
}

/// The coordinate acceleration `d²x/dt²` of a free body or ray in geometric units (`r_g`, `t_g`,
/// `c = 1`), from the geodesic equation written with coordinate time:
/// `ẍⁱ = −Γⁱ_αβ VᵅVᵝ + Γ⁰_αβ VᵅVᵝ vⁱ`, `V = (1, v)`.
///
/// Analytic throughout. With `g = η + f k k`, every derivative of the metric is a derivative of `f`
/// or `k` ([`metric_pieces`]), and the inverse metric is `η − f k^μ k^ν` with `k^μ = (−1, k_x, k_y,
/// k_z)` (`k` is null in flat space), so no matrix is inverted. The metric is stationary, so no
/// time derivatives appear.
fn geometric_acceleration(spin: f64, x: [f64; 3], v: [f64; 3]) -> Option<[f64; 3]> {
    let (f, k, df, dk) = metric_pieces(spin, x);
    let full = [1.0, v[0], v[1], v[2]];

    let k_v: f64 = (0..4).map(|mu| k[mu] * full[mu]).sum();
    let mut dk_v = [0.0; 3];
    for j in 0..3 {
        dk_v[j] = (1..4).map(|mu| dk[j][mu] * full[mu]).sum();
    }

    // Γ_{ν αβ} VᵅVᵝ with the first index lowered:
    //   Σ_j Vʲ ∂_j g_νβ Vᵝ  −  ½ ∂_ν g_αβ VᵅVᵝ   (the second only for spatial ν).
    let mut lowered = [0.0; 4];
    for (nu, entry) in lowered.iter_mut().enumerate() {
        let mut first = 0.0;
        for j in 0..3 {
            let dk_nu = if nu == 0 { 0.0 } else { dk[j][nu] };
            first += full[j + 1] * (df[j] * k[nu] * k_v + f * (dk_nu * k_v + k[nu] * dk_v[j]));
        }
        let second = if nu == 0 {
            0.0
        } else {
            let j = nu - 1;
            0.5 * (df[j] * k_v * k_v + 2.0 * f * k_v * dk_v[j])
        };
        *entry = first - second;
    }

    // Raise with g^μν = η^μν − f k^μ k^ν.
    let k_up = [-1.0, k[1], k[2], k[3]];
    let k_up_lowered: f64 = (0..4).map(|nu| k_up[nu] * lowered[nu]).sum();
    let mut raised = [0.0; 4];
    for mu in 0..4 {
        let eta = if mu == 0 { -lowered[0] } else { lowered[mu] };
        raised[mu] = eta - f * k_up[mu] * k_up_lowered;
    }

    let acceleration = [
        -raised[1] + raised[0] * v[0],
        -raised[2] + raised[0] * v[1],
        -raised[3] + raised[0] * v[2],
    ];
    acceleration
        .iter()
        .all(|c| c.is_finite())
        .then_some(acceleration)
}

/// [`geometric_acceleration`] with the Christoffel symbols from central differences of the
/// metric and a numerically inverted metric: slow, and kept so the tests can check the analytic
/// version against something written independently.
#[cfg(test)]
fn numerical_acceleration(spin: f64, x: [f64; 3], v: [f64; 3]) -> Option<[f64; 3]> {
    let g = kerr_schild_metric(spin, x);
    let g_inverse = invert(g)?;
    let full = [1.0, v[0], v[1], v[2]];

    // ∂_j g, for the three spatial directions.
    let r = kerr_schild_radius(spin, x).max(1.0);
    let h = 1.0e-5 * r;
    let mut dg = [[[0.0; 4]; 4]; 4]; // dg[j][mu][nu], j = 0 unused
    for j in 1..4 {
        let mut plus = x;
        let mut minus = x;
        plus[j - 1] += h;
        minus[j - 1] -= h;
        let gp = kerr_schild_metric(spin, plus);
        let gm = kerr_schild_metric(spin, minus);
        for mu in 0..4 {
            for nu in 0..4 {
                dg[j][mu][nu] = (gp[mu][nu] - gm[mu][nu]) / (2.0 * h);
            }
        }
    }

    // Γ_{ν αβ} VᵅVᵝ (first index lowered) = Σ_α Vᵅ (∂_α g_νβ Vᵝ) − ½ ∂_ν g_αβ VᵅVᵝ.
    let mut lowered = [0.0; 4];
    for (nu, entry) in lowered.iter_mut().enumerate() {
        let mut first = 0.0;
        for j in 1..4 {
            for beta in 0..4 {
                first += full[j] * dg[j][nu][beta] * full[beta];
            }
        }
        let mut second = 0.0;
        if nu > 0 {
            for alpha in 0..4 {
                for beta in 0..4 {
                    second += dg[nu][alpha][beta] * full[alpha] * full[beta];
                }
            }
        }
        *entry = first - 0.5 * second;
    }

    let mut raised = [0.0; 4];
    for mu in 0..4 {
        for nu in 0..4 {
            raised[mu] += g_inverse[mu][nu] * lowered[nu];
        }
    }

    let acceleration = [
        -raised[1] + raised[0] * v[0],
        -raised[2] + raised[0] * v[1],
        -raised[3] + raised[0] * v[2],
    ];
    acceleration
        .iter()
        .all(|c| c.is_finite())
        .then_some(acceleration)
}

/// The acceleration of a free body or ray of light near a spinning black hole, in Kerr–Schild
/// coordinates and time, in metres per second squared.
///
/// `offset_m` is the position relative to the hole with the spin along `+z`; `velocity_m_s` is
/// `dx/dt`. The body is a test particle. The same equation serves matter and light: what decides
/// which is the velocity it is given ([`kerr_schild_light_velocity`] for light). With `spin = 0`
/// this is the Schwarzschild solution in Kerr–Schild coordinates, which shares its spatial paths
/// with [`schwarzschild_acceleration`](super::gravity::schwarzschild_acceleration) but not its
/// time: here a body falling in crosses the horizon in a finite time.
///
/// # Returns
///
/// `None` for an unusable mass or spin, a non-finite input, or a point at or inside the horizon
/// (Boyer–Lindquist `r ≤ r₊`).
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::kerr::{
///     circular_orbit, kerr_schild_acceleration, KerrBlackHole,
/// };
///
/// let hole = KerrBlackHole::new(8.5467e36, 0.9).unwrap();
/// let (r_g, t_g) = (hole.gravitational_radius_m(), hole.gravitational_time_s());
///
/// // A prograde circular orbit at r = 4 r_g is a circle of radius √(r² + a²), traversed at Ω.
/// let r = 4.0;
/// let omega = circular_orbit(0.9, r, true).unwrap().angular_velocity / t_g;
/// let radius_m = (r * r + 0.81_f64).sqrt() * r_g;
/// let a = kerr_schild_acceleration(8.5467e36, 0.9, [radius_m, 0.0, 0.0], [0.0, omega * radius_m, 0.0])
///     .unwrap();
/// assert!((a[0] / (-omega * omega * radius_m) - 1.0).abs() < 1.0e-6);
/// ```
pub fn kerr_schild_acceleration(
    mass_kg: f64,
    spin: f64,
    offset_m: [f64; 3],
    velocity_m_s: [f64; 3],
) -> Option<[f64; 3]> {
    let hole = KerrBlackHole::new(mass_kg, spin)?;
    if offset_m.iter().chain(&velocity_m_s).any(|c| !c.is_finite()) {
        return None;
    }
    let r_g = hole.gravitational_radius_m();
    let x = offset_m.map(|c| c / r_g);
    if kerr_schild_radius(spin, x) <= horizon_radius(spin)? {
        return None;
    }
    let v = velocity_m_s.map(|c| c / SPEED_OF_LIGHT_M_S);
    let a = geometric_acceleration(spin, x, v)?;
    let scale = SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S / r_g;
    Some(a.map(|c| c * scale))
}

/// The velocity of a ray of light at `offset_m` heading along `direction`, in Kerr–Schild
/// coordinates and time, in metres per second: the speed along `direction` at which `(1, v)` is a
/// null vector of the metric.
///
/// Far from the hole this is `c`. Near it the speed depends on direction: outgoing light is slowed,
/// ingoing light is not, and inside the ergosphere light moving against the spin can barely make
/// headway.
///
/// # Returns
///
/// `None` for unusable inputs, a zero direction, a point at or inside the horizon, or a direction
/// in which light cannot move there (possible inside the ergosphere).
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
/// use physics_sandbox::physics_utils::relativity::kerr::kerr_schild_light_velocity;
///
/// let far = kerr_schild_light_velocity(8.5467e36, 0.9, [1.0e18, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
/// assert!((far[1] / SPEED_OF_LIGHT_M_S - 1.0).abs() < 1.0e-6);
/// ```
pub fn kerr_schild_light_velocity(
    mass_kg: f64,
    spin: f64,
    offset_m: [f64; 3],
    direction: [f64; 3],
) -> Option<[f64; 3]> {
    let hole = KerrBlackHole::new(mass_kg, spin)?;
    if offset_m.iter().chain(&direction).any(|c| !c.is_finite()) {
        return None;
    }
    let length = direction[0].hypot(direction[1]).hypot(direction[2]);
    if length <= 0.0 || !length.is_finite() {
        return None;
    }
    let d = direction.map(|c| c / length);
    let r_g = hole.gravitational_radius_m();
    let x = offset_m.map(|c| c / r_g);
    if kerr_schild_radius(spin, x) <= horizon_radius(spin)? {
        return None;
    }

    // g00 + 2 s (g0i dⁱ) + s² (g_ij dⁱ dʲ) = 0, for the positive root s.
    let g = kerr_schild_metric(spin, x);
    let mut quadratic = 0.0;
    let mut linear = 0.0;
    for i in 0..3 {
        linear += g[0][i + 1] * d[i];
        for j in 0..3 {
            quadratic += g[i + 1][j + 1] * d[i] * d[j];
        }
    }
    let constant = g[0][0];
    let discriminant = linear * linear - quadratic * constant;
    if discriminant < 0.0 {
        return None;
    }
    let speed = (-linear + discriminant.sqrt()) / quadratic;
    if speed.is_nan() || speed <= 0.0 {
        return None;
    }
    let velocity = d.map(|c| c * speed * SPEED_OF_LIGHT_M_S);
    velocity.iter().all(|c| c.is_finite()).then_some(velocity)
}

/// `g_μν VᵘVᵛ` for `V = (1, v/c)` at `offset_m`: negative for a body slower than light there,
/// zero for light. In geometric units, so dimensionless.
///
/// For a body, `1/√(−this)` is `dt/dτ`, the factor by which its clock runs slow against
/// coordinate time: gravitational time dilation and the Doppler effect together.
pub fn kerr_schild_norm(
    mass_kg: f64,
    spin: f64,
    offset_m: [f64; 3],
    velocity_m_s: [f64; 3],
) -> Option<f64> {
    let hole = KerrBlackHole::new(mass_kg, spin)?;
    let r_g = hole.gravitational_radius_m();
    let x = offset_m.map(|c| c / r_g);
    let v = velocity_m_s.map(|c| c / SPEED_OF_LIGHT_M_S);
    let value = norm(&kerr_schild_metric(spin, x), v);
    value.is_finite().then_some(value)
}

/// The position, in Kerr–Schild coordinates and `r_g`, of the point at Boyer–Lindquist radius
/// `radius` and azimuth `azimuth_rad` in the equatorial plane: `(r cos φ + a sin φ, r sin φ − a cos φ, 0)`.
///
/// The Kerr–Schild azimuth of a point differs from its Boyer–Lindquist one by a radius-dependent
/// twist; on a circle of fixed `r` that twist is a constant, so a circular orbit is still a
/// circle traversed at `Ω`.
pub fn equatorial_point(spin: f64, radius: f64, azimuth_rad: f64) -> [f64; 3] {
    let (sin, cos) = azimuth_rad.sin_cos();
    [radius * cos + spin * sin, radius * sin - spin * cos, 0.0]
}

/// The Boyer–Lindquist radius and azimuth, in `r_g` and radians in `(−π, 3π/2]`, of a point in the
/// equatorial plane given in Kerr–Schild coordinates and `r_g`: the inverse of
/// [`equatorial_point`]. Any `z` is ignored.
///
/// Since `x + iy = (r − ia) e^{iφ}`, the azimuth is `atan2(y, x) + atan2(a, r)`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::physics_utils::relativity::kerr::{equatorial_coordinates, equatorial_point};
///
/// let (r, phi) = equatorial_coordinates(0.9, equatorial_point(0.9, 5.0, 1.0));
/// assert!((r - 5.0).abs() < 1.0e-12 && (phi - 1.0).abs() < 1.0e-12);
/// ```
pub fn equatorial_coordinates(spin: f64, position: [f64; 3]) -> (f64, f64) {
    let radius = kerr_schild_radius(spin, [position[0], position[1], 0.0]);
    let azimuth = position[1].atan2(position[0]) + spin.atan2(radius);
    (radius, azimuth)
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/kerr_tests.rs"]
mod tests;
