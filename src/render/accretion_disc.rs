//! The accretion disc: gas orbiting a spinning black hole in its equatorial plane, heated by its own
//! friction until it glows.
//!
//! # What is modelled, and how faithfully
//!
//! - **Orbits.** Each clump of gas circles at the exact Kerr angular velocity for its radius,
//!   `Ω = 1/(r^{3/2} + a)` ([`circular_orbit`]), prograde: the spin drags the disc round with it,
//!   and inner rings lap outer ones.
//! - **Inflow.** Friction between rings carries angular momentum out, so gas spirals in. The real
//!   inflow is slow (a thin disc's gas takes thousands of orbits to cross it); here it is sped up to
//!   [`DISC_INFLOW_ORBITS_PER_EFOLD`] orbits per factor `e` in radius, so the drift can be watched.
//!   This is the one non-physical rate in the module.
//! - **The plunge.** Inside the ISCO no orbit is stable. A clump reaching it falls in along a real
//!   path, stepped with [`kerr_schild_acceleration`], whirling round with the spin as it drops, and
//!   is replaced by a new clump at the outer edge.
//! - **Heat.** The Page–Thorne flux ([`disc_flux`]) at each radius, for an accretion rate of
//!   [`DISC_EDDINGTON_FRACTION`] of the Eddington limit, gives the temperature a thermometer riding
//!   with the gas would read ([`DiscProfile`]), and the redshift of its light along any path
//!   ([`DiscParticle::redshift_factor_along`]).
//!
//! Inside the ISCO the Page–Thorne model radiates nothing. Plunging gas is drawn anyway, at the ISCO's
//! temperature and fading as it falls, so the plunge can be seen.
//!
//! # How it is drawn
//!
//! Not here. [`crate::render::black_hole_view`] ray-marches the hole on the GPU and splats these
//! clumps into the texture its shader reads, every frame: each pixel's light is followed along its
//! exact Kerr path to the gas it left, which is shaded by its temperature as the camera sees it.
//!
//! # The ergosphere
//!
//! The region inside which nothing can stay still, everything being dragged round with the hole.
//! [`ergosphere_outline`] gives its outline (its equator and a few meridians); it is not drawn at
//! present.
//!
//! # Scope
//!
//! Render-only, like the light streams: the gas has no mass, does not collide, and lights nothing.
//! There is one disc, around the most massive black hole of a relativistic scene
//! ([`stream_source`]), in the plane perpendicular to the spin (world `+Y`).
//!
//! # What a real Sgr A* looks like
//!
//! Not this. Sgr A* is fed at around a billionth of its Eddington rate, and its gas forms a hot,
//! faint, puffed-up flow rather than a thin disc. This is Sgr A* as it would be if it were fed like a
//! quasar, at a tenth of Eddington, which is the regime the thin-disc model describes.

use std::collections::VecDeque;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::math::Vec3;

use crate::constants::METRES_PER_UNIT;
use crate::core::integrator::kerr_schild_to_world;
use crate::core::scene::Scene;
use crate::experiments::random::Rng;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::kerr::{
    circular_orbit, disc_flux, equatorial_coordinates, equatorial_point, ergosphere_radius,
    horizon_radius, isco_radius, kerr_schild_acceleration, kerr_schild_norm, kerr_schild_radius,
    KerrBlackHole,
};
use crate::render::constants::{
    DISC_EDDINGTON_FRACTION, DISC_INFLOW_ORBITS_PER_EFOLD, DISC_OUTER_RADIUS_RG, DISC_PARTICLES,
    DISC_PLUNGE_SAMPLE_INTERVAL_S, DISC_PLUNGE_SPEED_FACTOR, DISC_PLUNGE_STEP_FRACTION,
    DISC_PLUNGE_TRAIL_SAMPLES, DISC_PROFILE_SAMPLES, DISC_SEED, DISC_STREAK_ARC_RAD,
    DISC_STREAK_POINTS, ERGOSPHERE_MERIDIANS, ERGOSPHERE_SEGMENTS,
};
use crate::render::light_streams::stream_source;
use crate::time::simulation::SimulationClock;

// -----------------------------------------------------------------------
// Blackbody colour.
// -----------------------------------------------------------------------

/// The colour of a blackbody at `temperature_k`, as linear RGB scaled so its brightest channel is
/// `1`.
///
/// A fit to the CIE colour of Planck's spectrum (Tanner Helland's), valid from `1,000` to `40,000`
/// K; temperatures outside are clamped. Red below about `3,000 K`, white near `6,500 K`, blue-white
/// above `10,000 K`, and barely changing beyond `30,000 K`, since a very hot body's visible light is
/// all on the Rayleigh–Jeans tail.
///
/// # Examples
///
/// ```
/// use physics_sandbox::blackbody_rgb;
///
/// let [r, g, b] = blackbody_rgb(2_000.0);
/// assert!(r > g && g > b, "a candle is orange");
///
/// let [r, _, b] = blackbody_rgb(20_000.0);
/// assert!(b > r, "a hot star is blue");
/// ```
pub fn blackbody_rgb(temperature_k: f64) -> [f32; 3] {
    let t = temperature_k.clamp(1_000.0, 40_000.0) / 100.0;

    let red = if t <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (t - 60.0).powf(-0.133_204_759_2)
    };
    let green = if t <= 66.0 {
        99.470_802_586_1 * t.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (t - 60.0).powf(-0.075_514_849_2)
    };
    let blue = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (t - 10.0).ln() - 305.044_792_730_7
    };

    let to_linear = |channel: f64| {
        let c = (channel / 255.0).clamp(0.0, 1.0);
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let rgb = [to_linear(red), to_linear(green), to_linear(blue)];
    let brightest = rgb.iter().copied().fold(0.0_f64, f64::max).max(1.0e-12);
    rgb.map(|c| (c / brightest) as f32)
}

// -----------------------------------------------------------------------
// The disc's profile.
// -----------------------------------------------------------------------

/// The disc's radiation, tabulated once per hole: temperature and relative flux against radius.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscProfile {
    /// The hole's mass, in kilograms.
    pub mass_kg: f64,
    /// The hole's spin.
    pub spin: f64,
    /// The ISCO, the disc's inner edge, in `r_g`.
    pub isco_rg: f64,
    /// The disc's outer edge, in `r_g`.
    pub outer_rg: f64,
    /// `r_g`, in metres.
    pub gravitational_radius_m: f64,
    /// `t_g`, in seconds.
    pub gravitational_time_s: f64,
    /// Sample radii, in `r_g`, from the ISCO to the outer edge.
    pub radii_rg: Vec<f64>,
    /// The temperature at each sample radius, in kelvin.
    pub temperature_k: Vec<f64>,
    /// The flux at each sample radius over the peak flux.
    pub relative_flux: Vec<f64>,
}

impl DiscProfile {
    /// The profile of a disc around a hole of `mass_kg` and `spin`, fed at
    /// [`DISC_EDDINGTON_FRACTION`] of Eddington, out to [`DISC_OUTER_RADIUS_RG`].
    pub fn new(mass_kg: f64, spin: f64) -> Option<Self> {
        let hole = KerrBlackHole::new(mass_kg, spin)?;
        let isco = isco_radius(spin, true)?;
        let outer = f64::from(DISC_OUTER_RADIUS_RG).max(isco * 1.5);
        let accretion = hole.accretion_rate_kg_s(DISC_EDDINGTON_FRACTION)?;

        let samples = DISC_PROFILE_SAMPLES.max(2) as usize;
        let mut radii = Vec::with_capacity(samples);
        let mut temperature = Vec::with_capacity(samples);
        let mut flux = Vec::with_capacity(samples);
        // Denser near the ISCO, where the profile changes fastest: uniform in √(r − r_ISCO).
        let span = (outer - isco).sqrt();
        for i in 0..samples {
            let s = span * i as f64 / (samples - 1) as f64;
            let r = isco + s * s;
            radii.push(r);
            temperature.push(hole.disc_temperature_k(r, accretion)?);
            flux.push(disc_flux(spin, r)?);
        }
        let peak = flux.iter().copied().fold(0.0_f64, f64::max).max(1.0e-300);

        Some(DiscProfile {
            mass_kg,
            spin,
            isco_rg: isco,
            outer_rg: outer,
            gravitational_radius_m: hole.gravitational_radius_m(),
            gravitational_time_s: hole.gravitational_time_s(),
            relative_flux: flux.iter().map(|f| f / peak).collect(),
            radii_rg: radii,
            temperature_k: temperature,
        })
    }

    /// Linear interpolation of a tabulated column at `radius_rg`, clamped to the disc.
    fn lookup(&self, column: &[f64], radius_rg: f64) -> f64 {
        let r = radius_rg.clamp(self.isco_rg, self.outer_rg);
        let index = self
            .radii_rg
            .partition_point(|&sample| sample < r)
            .clamp(1, self.radii_rg.len() - 1);
        let (r0, r1) = (self.radii_rg[index - 1], self.radii_rg[index]);
        let t = if r1 > r0 { (r - r0) / (r1 - r0) } else { 0.0 };
        column[index - 1] + t * (column[index] - column[index - 1])
    }

    /// The temperature of the gas at `radius_rg`, in kelvin, as it would measure itself.
    pub fn temperature_at(&self, radius_rg: f64) -> f64 {
        self.lookup(&self.temperature_k, radius_rg)
    }

    /// The flux at `radius_rg` over the disc's peak flux.
    pub fn relative_flux_at(&self, radius_rg: f64) -> f64 {
        self.lookup(&self.relative_flux, radius_rg)
    }

    /// The hottest temperature in the disc, in kelvin.
    pub fn peak_temperature_k(&self) -> f64 {
        self.temperature_k.iter().copied().fold(0.0, f64::max)
    }
}

// -----------------------------------------------------------------------
// The gas.
// -----------------------------------------------------------------------

/// A clump that has crossed the ISCO and is falling in.
#[derive(Clone, Debug, PartialEq)]
pub struct Plunge {
    /// Position relative to the hole, Kerr–Schild axes (spin along `+z`), metres.
    pub offset_m: [f64; 3],
    /// Coordinate velocity, metres per second.
    pub velocity_m_s: [f64; 3],
    /// Earlier positions, oldest first.
    pub trail_m: VecDeque<[f64; 3]>,
    since_sample_s: f64,
}

/// One clump of gas in the disc.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscParticle {
    /// Boyer–Lindquist radius while orbiting, in `r_g`.
    pub radius_rg: f64,
    /// Azimuth while orbiting, in radians, counter-clockwise about the spin axis.
    pub azimuth_rad: f64,
    /// `Some` once the clump has crossed the ISCO and is falling in.
    pub plunge: Option<Plunge>,
}

/// The position and velocity of a clump on a circular orbit, in Kerr–Schild axes, metres and m/s.
fn orbit_state(profile: &DiscProfile, radius_rg: f64, azimuth_rad: f64) -> ([f64; 3], [f64; 3]) {
    let spin = profile.spin;
    let omega = circular_orbit(spin, radius_rg, true).map_or(0.0, |o| o.angular_velocity);
    let (sin, cos) = azimuth_rad.sin_cos();
    let r_g = profile.gravitational_radius_m;
    let position = equatorial_point(spin, radius_rg, azimuth_rad).map(|c| c * r_g);
    // d/dφ of (r cos φ + a sin φ, r sin φ − a cos φ), times dφ/dt.
    let rate = omega / profile.gravitational_time_s;
    let velocity = [
        (-radius_rg * sin + spin * cos) * r_g * rate,
        (radius_rg * cos + spin * sin) * r_g * rate,
        0.0,
    ];
    (position, velocity)
}

fn add(a: [f64; 3], b: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2]]
}

fn rk4(profile: &DiscProfile, x: [f64; 3], v: [f64; 3], dt: f64) -> Option<([f64; 3], [f64; 3])> {
    let accel = |x, v| kerr_schild_acceleration(profile.mass_kg, profile.spin, x, v);
    let a1 = accel(x, v)?;
    let (x2, v2) = (add(x, v, 0.5 * dt), add(v, a1, 0.5 * dt));
    let a2 = accel(x2, v2)?;
    let (x3, v3) = (add(x, v2, 0.5 * dt), add(v, a2, 0.5 * dt));
    let a3 = accel(x3, v3)?;
    let (x4, v4) = (add(x, v3, dt), add(v, a3, dt));
    let a4 = accel(x4, v4)?;
    let mut nx = x;
    let mut nv = v;
    for i in 0..3 {
        nx[i] += dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]);
        nv[i] += dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]);
    }
    Some((nx, nv))
}

impl DiscParticle {
    /// The clump's Boyer–Lindquist radius, in `r_g`, orbiting or falling.
    pub fn current_radius_rg(&self, profile: &DiscProfile) -> f64 {
        match &self.plunge {
            None => self.radius_rg,
            Some(plunge) => kerr_schild_radius(
                profile.spin,
                plunge.offset_m.map(|c| c / profile.gravitational_radius_m),
            ),
        }
    }

    /// The clump's position and velocity, Kerr–Schild axes, metres and m/s.
    pub fn state(&self, profile: &DiscProfile) -> ([f64; 3], [f64; 3]) {
        match &self.plunge {
            None => orbit_state(profile, self.radius_rg, self.azimuth_rad),
            Some(plunge) => (plunge.offset_m, plunge.velocity_m_s),
        }
    }

    /// Advance by `dt_s`. Returns `false` once the clump has crossed the horizon.
    fn advance(&mut self, profile: &DiscProfile, dt_s: f64) -> bool {
        if self.plunge.is_none() {
            let Some(orbit) = circular_orbit(profile.spin, self.radius_rg, true) else {
                return false;
            };
            let omega = orbit.angular_velocity / profile.gravitational_time_s;
            self.azimuth_rad = (self.azimuth_rad + omega * dt_s).rem_euclid(std::f64::consts::TAU);
            let decay = omega * dt_s / (std::f64::consts::TAU * DISC_INFLOW_ORBITS_PER_EFOLD);
            self.radius_rg *= (-decay).exp();

            if self.radius_rg <= profile.isco_rg {
                self.radius_rg = profile.isco_rg;
                let (position, velocity) = orbit_state(profile, profile.isco_rg, self.azimuth_rad);
                self.plunge = Some(Plunge {
                    offset_m: position,
                    velocity_m_s: velocity.map(|c| c * DISC_PLUNGE_SPEED_FACTOR),
                    trail_m: VecDeque::with_capacity(DISC_PLUNGE_TRAIL_SAMPLES as usize),
                    since_sample_s: 0.0,
                });
            }
            return true;
        }

        let horizon = horizon_radius(profile.spin).unwrap_or(2.0) * 1.001;
        let plunge = self.plunge.as_mut().expect("checked above");
        let mut remaining = dt_s;
        while remaining > 0.0 {
            let r_bl = kerr_schild_radius(
                profile.spin,
                plunge.offset_m.map(|c| c / profile.gravitational_radius_m),
            );
            if r_bl <= horizon {
                return false;
            }
            let distance = plunge.offset_m[0]
                .hypot(plunge.offset_m[1])
                .hypot(plunge.offset_m[2]);
            let h = (DISC_PLUNGE_STEP_FRACTION * distance / SPEED_OF_LIGHT_M_S).min(remaining);
            match rk4(profile, plunge.offset_m, plunge.velocity_m_s, h) {
                Some((x, v)) => {
                    plunge.offset_m = x;
                    plunge.velocity_m_s = v;
                }
                None => return false,
            }
            remaining -= h;
            plunge.since_sample_s += h;
            if plunge.since_sample_s >= DISC_PLUNGE_SAMPLE_INTERVAL_S {
                plunge.since_sample_s -= DISC_PLUNGE_SAMPLE_INTERVAL_S;
                plunge.trail_m.push_back(plunge.offset_m);
                if plunge.trail_m.len() > DISC_PLUNGE_TRAIL_SAMPLES as usize {
                    plunge.trail_m.pop_front();
                }
            }
        }
        true
    }

    /// The Boyer–Lindquist radius and azimuth (`r_g`, radians) of each point of the clump's
    /// streak, oldest first: an arc of [`DISC_STREAK_ARC_RAD`] trailing it along its orbit while it
    /// orbits, its recent path once it plunges. The GPU ray-marcher splats these into its gas texture.
    pub fn streak_coordinates(&self, profile: &DiscProfile) -> Vec<(f64, f64)> {
        match &self.plunge {
            None => {
                let points = DISC_STREAK_POINTS.max(2);
                (0..points)
                    .map(|k| {
                        let back =
                            DISC_STREAK_ARC_RAD * f64::from(points - 1 - k) / f64::from(points - 1);
                        (self.radius_rg, self.azimuth_rad - back)
                    })
                    .collect()
            }
            Some(plunge) => plunge
                .trail_m
                .iter()
                .chain(std::iter::once(&plunge.offset_m))
                .map(|offset| {
                    equatorial_coordinates(
                        profile.spin,
                        offset.map(|c| c / profile.gravitational_radius_m),
                    )
                })
                .collect(),
        }
    }

    /// The redshift factor for light that left the clump with `emission = p_i/(−p_t)` and reached a
    /// camera where `√(−g_tt)` is `camera_lapse`: `1/(lapse · u^t (1 − emission·v/c))`.
    ///
    /// Exact along any path, bent or not. For light that travelled straight, `emission` is the
    /// unit vector towards the camera and the lapse is `1`, which is
    /// [`DiscParticle::redshift_factor`].
    pub fn redshift_factor_along(
        &self,
        profile: &DiscProfile,
        emission: [f64; 3],
        camera_lapse: f64,
    ) -> f64 {
        let (position, velocity) = self.state(profile);
        let Some(norm) = kerr_schild_norm(profile.mass_kg, profile.spin, position, velocity) else {
            return 1.0;
        };
        if norm >= 0.0 {
            return 1.0;
        }
        let u_t = 1.0 / (-norm).sqrt();
        let toward = (0..3).map(|i| emission[i] * velocity[i]).sum::<f64>() / SPEED_OF_LIGHT_M_S;
        let g = 1.0 / (camera_lapse * u_t * (1.0 - toward));
        if g.is_finite() && g > 0.0 {
            g
        } else {
            1.0
        }
    }

    /// The redshift factor `g = ν_seen/ν_emitted` for a camera at `camera_offset_m` (Kerr–Schild
    /// axes, relative to the hole): `1/(u^t (1 − n·v/c))`, with `n` the direction to the camera.
    ///
    /// `u^t` carries the gravitational redshift and time dilation, `1 − n·v/c` the Doppler effect.
    /// It treats the light's path as straight, which light bending near the hole is not.
    pub fn redshift_factor(&self, profile: &DiscProfile, camera_offset_m: [f64; 3]) -> f64 {
        let (position, _) = self.state(profile);
        let to_camera = [
            camera_offset_m[0] - position[0],
            camera_offset_m[1] - position[1],
            camera_offset_m[2] - position[2],
        ];
        let distance = to_camera[0].hypot(to_camera[1]).hypot(to_camera[2]);
        if distance <= 0.0 {
            return 1.0;
        }
        self.redshift_factor_along(profile, to_camera.map(|c| c / distance), 1.0)
    }
}

/// The disc: every clump, the profile they glow by, and the random state that places new ones.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct AccretionDisc {
    particles: Vec<DiscParticle>,
    profile: Option<DiscProfile>,
    rng: Rng,
    plunged: u64,
}

impl Default for AccretionDisc {
    fn default() -> Self {
        AccretionDisc {
            particles: Vec::new(),
            profile: None,
            rng: Rng::new(DISC_SEED),
            plunged: 0,
        }
    }
}

/// A uniform random number in `[0, 1)`.
fn unit(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

impl AccretionDisc {
    /// The clumps.
    pub fn particles(&self) -> &[DiscParticle] {
        &self.particles
    }

    /// The profile, once the disc has been built.
    pub fn profile(&self) -> Option<&DiscProfile> {
        self.profile.as_ref()
    }

    /// How many clumps have fallen through the horizon.
    pub fn plunged(&self) -> u64 {
        self.plunged
    }

    /// Remove the disc.
    pub fn clear(&mut self) {
        *self = AccretionDisc::default();
    }

    /// A new clump at the outer edge, at a random azimuth.
    fn fresh(&mut self, profile: &DiscProfile) -> DiscParticle {
        DiscParticle {
            radius_rg: profile.outer_rg,
            azimuth_rad: std::f64::consts::TAU * unit(&mut self.rng),
            plunge: None,
        }
    }

    /// Build the disc for a hole of `mass_kg` and `spin`: the profile, and [`DISC_PARTICLES`]
    /// clumps spread over it.
    ///
    /// Clumps are placed with the steady-state density of the inflow: the drift speed falls as
    /// `r Ω ∝ r^{-1/2}`, so in a steady flow the number per unit radius rises as `r^{1/2}`.
    fn build(&mut self, mass_kg: f64, spin: f64) {
        let Some(profile) = DiscProfile::new(mass_kg, spin) else {
            self.clear();
            return;
        };
        self.particles.clear();
        let (inner, outer) = (profile.isco_rg, profile.outer_rg);
        // Inverse CDF of p(r) ∝ √r on [inner, outer]: r = (inner^{3/2} + u (outer^{3/2} − inner^{3/2}))^{2/3}.
        let (low, high) = (inner.powf(1.5), outer.powf(1.5));
        for _ in 0..DISC_PARTICLES {
            let r = (low + unit(&mut self.rng) * (high - low)).powf(2.0 / 3.0);
            let azimuth = std::f64::consts::TAU * unit(&mut self.rng);
            self.particles.push(DiscParticle {
                radius_rg: r.max(inner * (1.0 + 1.0e-9)),
                azimuth_rad: azimuth,
                plunge: None,
            });
        }
        self.profile = Some(profile);
    }

    /// Advance the disc by `dt_s` around a hole of `mass_kg` and `spin`, building it first if the
    /// hole is new. Clumps that cross the horizon are replaced at the outer edge.
    pub fn advance(&mut self, mass_kg: f64, spin: f64, dt_s: f64) {
        let rebuild = match &self.profile {
            Some(profile) => profile.mass_kg != mass_kg || profile.spin != spin,
            None => true,
        };
        if rebuild {
            self.build(mass_kg, spin);
        }
        if !dt_s.is_finite() || dt_s < 0.0 {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            return;
        };

        let mut lost = Vec::new();
        for (index, particle) in self.particles.iter_mut().enumerate() {
            if !particle.advance(&profile, dt_s) {
                lost.push(index);
            }
        }
        self.plunged += lost.len() as u64;
        for index in lost {
            self.particles[index] = self.fresh(&profile);
        }
    }
}

// -----------------------------------------------------------------------
// The ergosphere.
// -----------------------------------------------------------------------

/// The ergosphere's outer boundary as polylines in world coordinates about a hole centred at
/// `centre`: its equator and [`ERGOSPHERE_MERIDIANS`] meridians from pole to pole.
///
/// The boundary is `r = 1 + √(1 − a² cos²θ)` (Boyer–Lindquist), placed in Kerr–Schild Cartesian
/// coordinates: `x = (r cos φ + a sin φ) sin θ`, `y = (r sin φ − a cos φ) sin θ`, `z = r cos θ`.
/// It touches the horizon at the poles and bulges to `√(4 + a²) r_g` at the equator.
pub fn ergosphere_outline(mass_kg: f64, spin: f64, centre: Vec3) -> Vec<Vec<Vec3>> {
    let Some(hole) = KerrBlackHole::new(mass_kg, spin) else {
        return Vec::new();
    };
    let r_g = hole.gravitational_radius_m();
    let point = |theta: f64, phi: f64| -> Option<Vec3> {
        let r = ergosphere_radius(spin, theta)?;
        let (sin_t, cos_t) = theta.sin_cos();
        let (sin_p, cos_p) = phi.sin_cos();
        let ks = [
            (r * cos_p + spin * sin_p) * sin_t * r_g,
            (r * sin_p - spin * cos_p) * sin_t * r_g,
            r * cos_t * r_g,
        ];
        Some(centre + (kerr_schild_to_world(ks) / METRES_PER_UNIT).as_vec3())
    };

    let segments = ERGOSPHERE_SEGMENTS.max(4);
    let mut lines = Vec::new();
    let equator: Option<Vec<Vec3>> = (0..=segments)
        .map(|i| {
            point(
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::TAU * f64::from(i) / f64::from(segments),
            )
        })
        .collect();
    lines.extend(equator);
    for m in 0..ERGOSPHERE_MERIDIANS {
        let phi = std::f64::consts::TAU * f64::from(m) / f64::from(ERGOSPHERE_MERIDIANS.max(1));
        let meridian: Option<Vec<Vec3>> = (0..=segments / 2)
            .map(|i| {
                point(
                    std::f64::consts::PI * f64::from(i) / f64::from(segments / 2),
                    phi,
                )
            })
            .collect();
        lines.extend(meridian);
    }
    lines
}

// -----------------------------------------------------------------------
// Systems and plugin.
// -----------------------------------------------------------------------

/// Bevy system that advances the disc by this frame's simulated step, around the most massive black
/// hole of a scene with a spinning-hole gravity model. Clears it otherwise.
pub fn advance_accretion_disc_system(
    scene: Option<Res<Scene>>,
    clock: Option<Res<SimulationClock>>,
    mut disc: ResMut<AccretionDisc>,
) {
    let (Some(scene), Some(clock)) = (scene, clock) else {
        return;
    };
    match disc_source(&scene) {
        Some((mass_kg, spin, _)) => disc.advance(mass_kg, spin, clock.last_step().seconds()),
        None => {
            if !disc.particles().is_empty() {
                disc.clear();
            }
        }
    }
}

/// The hole a disc forms around: [`stream_source`]'s, but only in a scene whose black holes spin.
pub fn disc_source(scene: &Scene) -> Option<(f64, f64, Vec3)> {
    stream_source(scene).filter(|(_, spin, _)| *spin > 0.0)
}

/// Builds and advances the accretion disc around a spinning black hole.
///
/// Add it after [`ScenePlugin`](super::plugin::ScenePlugin). The disc advances headless too. It is
/// drawn by [`BlackHoleViewPlugin`](super::black_hole_view::BlackHoleViewPlugin), which ray-marches
/// the hole and reads the clumps every frame.
pub struct AccretionDiscPlugin;

impl Plugin for AccretionDiscPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AccretionDisc>();
        app.add_systems(
            Update,
            advance_accretion_disc_system.after(super::plugin::step_scene_system),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/render/accretion_disc_tests.rs"]
mod tests;
