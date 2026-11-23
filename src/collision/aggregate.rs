//! Bodies made of particles: building a planet as a cluster of particles, and finding which
//! particles belong together afterwards.
//!
//! A planet built this way is held together by nothing but the gravity of its particles, and the
//! particles touch through the simulated contact of [`crate::collision::collision`]. Two such
//! planets that collide are not merged by a rule: they deform, flow into each other and throw off
//! debris because their particles push, lose energy and rearrange. That rearranging is the plastic
//! deformation of the body, even though each contact is a damped spring.
//!
//! # The packing
//!
//! The particles sit on a **face-centred cubic** lattice, the densest packing of equal spheres
//! (Kepler's conjecture, proved by Hales 2005): each touches twelve neighbours and they fill `74%`
//! of the space. Every lattice point inside the planet, less a particle radius, gets a particle;
//! the lattice spacing is chosen so the count is as close as it can be to the one asked for. All the
//! particles have the same mass, so the body has the planet's mass and a uniform density: a real
//! planet's dense core is not modelled.
//!
//! The planet's own gravity compresses the pile a little once it is released, until the contacts
//! carry the weight; the contacts' damping settles that within a few contact times.
//!
//! # Clusters
//!
//! [`contact_clusters`] groups particles that touch, directly or through others. After a collision
//! the largest cluster is the merged body and the rest is debris, which is how a run is compared
//! with the scaling law of [`crate::physics_utils::impacts`].

use std::collections::HashMap;

use crate::collision::collision::{fluid_radius_m, measure_fluid_densities};
use crate::collision::constants::{
    CLUSTER_CONTACT_TOLERANCE, COLD_ISENTROPE_STEPS, FLUID_CLUSTER_CONTACT_TOLERANCE,
    PLANET_PROFILE_BISECTIONS, PLANET_PROFILE_ISENTROPE_SAMPLES, PLANET_PROFILE_MAX_COMPRESSION,
    PLANET_PROFILE_RADIAL_STEPS,
};
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Body, Color, DVec3, Material, Pose, Prop, Quat};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::physics_utils::thermal::{
    tillotson_cold_energy, tillotson_pressure, TillotsonParameters,
};

/// A planet to build out of particles.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ParticleBody {
    /// Where its centre is, in world units.
    pub centre_units: DVec3,
    /// How it moves as a whole, in world units per second.
    pub velocity_units_s: DVec3,
    /// Its total mass, kg.
    pub mass_kg: f64,
    /// Its radius, m.
    pub radius_m: f64,
    /// The colour its particles are drawn in.
    pub color: Color,
    /// What its particles are made of. Give it a modulus
    /// ([`Material::with_modulus`]) so their contacts are simulated.
    pub material: Material,
}

/// The lattice spacing, in metres, that puts as close as the packing allows to `count` particles
/// inside a body of `radius_m`. The lattice fills in whole symmetric shells, so the count moves in
/// steps: at small counts the nearest step can be a fifth from the target. See the module docs.
///
/// Two bodies built with the same spacing have particles of the same size, which is how a scene
/// makes a small planet of fewer particles than a large one rather than of smaller particles.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{lattice_spacing_m, lattice_points};
///
/// let spacing = lattice_spacing_m(1.0e6, 200);
/// let count = lattice_points(1.0e6, spacing).len();
/// assert!((180..=220).contains(&count), "{count}");
/// ```
pub fn lattice_spacing_m(radius_m: f64, count: u32) -> f64 {
    if count <= 1 || !(radius_m.is_finite() && radius_m > 0.0) {
        return 2.0 * radius_m.max(0.0) * std::f64::consts::SQRT_2;
    }
    // Bisect on the spacing: the count falls as the spacing grows. Keep the spacing whose count is
    // nearest the target.
    let (mut small, mut large) = (radius_m * 1.0e-3, 2.0 * radius_m);
    let mut best = (u32::MAX, large);
    for _ in 0..60 {
        let middle = 0.5 * (small + large);
        let found = lattice_points(radius_m, middle).len() as u32;
        let miss = found.abs_diff(count);
        if miss < best.0 {
            best = (miss, middle);
        }
        if found > count {
            small = middle;
        } else {
            large = middle;
        }
    }
    best.1
}

/// The particle centres of a body of `radius_m` on a face-centred cubic lattice of `spacing_m`, as
/// offsets from its centre in metres. Every point lies at least a particle radius
/// ([`particle_radius_m`]) inside the surface.
///
/// A spacing too large for even one particle still gives the centre point, so every body has at
/// least one particle.
pub fn lattice_points(radius_m: f64, spacing_m: f64) -> Vec<DVec3> {
    let particle = particle_radius_m(spacing_m);
    let reach = radius_m - particle;
    if reach.is_nan() || reach <= 0.0 || spacing_m.is_nan() || spacing_m <= 0.0 {
        return vec![DVec3::ZERO];
    }
    // Lattice points `spacing·(i, j, k)/2` with `i + j + k` even: nearest neighbours `spacing/√2`
    // apart, so particles of radius `spacing/(2√2)` just touch.
    let half = 0.5 * spacing_m;
    let steps = (reach / half).ceil() as i64;
    let mut points = Vec::new();
    for i in -steps..=steps {
        for j in -steps..=steps {
            for k in -steps..=steps {
                if (i + j + k).rem_euclid(2) != 0 {
                    continue;
                }
                let point = DVec3::new(i as f64, j as f64, k as f64) * half;
                if point.length() <= reach {
                    points.push(point);
                }
            }
        }
    }
    if points.is_empty() {
        points.push(DVec3::ZERO);
    }
    points
}

/// The radius of the particles on a lattice of `spacing_m`, in metres: half the nearest-neighbour
/// distance, so neighbours just touch.
#[inline]
pub fn particle_radius_m(spacing_m: f64) -> f64 {
    spacing_m / (2.0 * std::f64::consts::SQRT_2)
}

/// Build `body` out of particles on a lattice of `spacing_m`: one prop per lattice point, all of
/// equal mass, all moving with the body. A single particle (when the spacing leaves room for no
/// more) has the body's whole radius.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{lattice_spacing_m, particle_body, Color, DVec3, Material, ParticleBody};
///
/// let body = ParticleBody {
///     centre_units: DVec3::ZERO,
///     velocity_units_s: DVec3::ZERO,
///     mass_kg: 6.0e24,
///     radius_m: 6.4e6,
///     color: Color { r: 0.2, g: 0.4, b: 0.9, a: 1.0 },
///     material: Material::new(0.0, 0.0).with_modulus(1.0e13),
/// };
/// let particles = particle_body(&body, lattice_spacing_m(body.radius_m, 100));
///
/// let mass: f64 = particles.iter().map(|p| f64::from(p.mass_kg)).sum();
/// assert!((mass / 6.0e24 - 1.0).abs() < 1e-5);
/// assert!(particles.iter().all(|p| p.is_valid()));
/// ```
pub fn particle_body(body: &ParticleBody, spacing_m: f64) -> Vec<Prop> {
    let points = lattice_points(body.radius_m, spacing_m);
    let radius_m = if points.len() == 1 {
        body.radius_m
    } else {
        particle_radius_m(spacing_m)
    };
    let mass = body.mass_kg / points.len() as f64;
    points
        .iter()
        .map(|offset| {
            Prop::new(
                Body::Sphere {
                    pose: Pose {
                        position: body.centre_units + *offset / METRES_PER_UNIT,
                        rotation: Quat::IDENTITY,
                    },
                    radius: (radius_m / METRES_PER_UNIT) as f32,
                    color: body.color,
                },
                body.velocity_units_s,
                mass as f32,
            )
            .with_material(body.material)
        })
        .collect()
}

/// A fluid planet in hydrostatic equilibrium: how its density and internal energy vary with depth,
/// from [`hydrostatic_profile`].
///
/// Sampled from the centre out, every [`PLANET_PROFILE_RADIAL_STEPS`]-th of the planet's
/// uncompressed radius, and read by mass: [`radius_enclosing`](Self::radius_enclosing) is where
/// a given share of the mass lies inside.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanetProfile {
    /// Where the pressure falls to zero, m.
    pub radius_m: f64,
    /// Where the core gives way to the mantle, m.
    pub core_radius_m: f64,
    /// The pressure at the centre, Pa.
    pub central_pressure_pa: f64,
    /// The planet's mass as the profile holds it, kg (the target to rounding).
    pub mass_kg: f64,
    radii: Vec<f64>,
    enclosed: Vec<f64>,
    density: Vec<f64>,
    energy: Vec<f64>,
}

impl PlanetProfile {
    /// The radius inside which `fraction` of the mass lies, m, for `fraction` in `[0, 1]`.
    pub fn radius_enclosing(&self, fraction: f64) -> f64 {
        self.interpolate(
            &self.enclosed,
            &self.radii,
            fraction.clamp(0.0, 1.0) * self.mass_kg,
        )
    }

    /// The density at radius `radius_m`, kg/m³.
    pub fn density_at(&self, radius_m: f64) -> f64 {
        self.interpolate(&self.radii, &self.density, radius_m)
    }

    /// The specific internal energy at radius `radius_m`, J/kg: the cold isentrope's at the density
    /// there.
    pub fn energy_at(&self, radius_m: f64) -> f64 {
        self.interpolate(&self.radii, &self.energy, radius_m)
    }

    /// `ys` at `x` along the increasing `xs`, linearly, held at either end.
    fn interpolate(&self, xs: &[f64], ys: &[f64], x: f64) -> f64 {
        let upper = xs.partition_point(|&value| value < x);
        if upper == 0 {
            return ys[0];
        }
        if upper >= xs.len() {
            return ys[xs.len() - 1];
        }
        let (x0, x1) = (xs[upper - 1], xs[upper]);
        let t = if x1 > x0 { (x - x0) / (x1 - x0) } else { 0.0 };
        ys[upper - 1] + t * (ys[upper] - ys[upper - 1])
    }
}

/// One material's cold isentrope ([`tillotson_cold_energy`]), tabulated by density, for looking up
/// the density and energy at a pressure.
struct Isentrope {
    density: Vec<f64>,
    energy: Vec<f64>,
    pressure: Vec<f64>,
}

impl Isentrope {
    fn of(eos: &TillotsonParameters) -> Self {
        let rho0 = eos.reference_density_kg_m3;
        let samples = PLANET_PROFILE_ISENTROPE_SAMPLES.max(2);
        let mut table = Self {
            density: Vec::with_capacity(samples as usize),
            energy: Vec::with_capacity(samples as usize),
            pressure: Vec::with_capacity(samples as usize),
        };
        for k in 0..samples {
            let rho = rho0
                * (1.0
                    + (PLANET_PROFILE_MAX_COMPRESSION - 1.0) * f64::from(k)
                        / f64::from(samples - 1));
            let u = tillotson_cold_energy(rho, eos, COLD_ISENTROPE_STEPS);
            table.density.push(rho);
            table.energy.push(u);
            table.pressure.push(tillotson_pressure(rho, u, eos));
        }
        table
    }

    /// The density and energy at `pressure`, interpolated; the table's ends beyond it.
    fn at(&self, pressure: f64) -> (f64, f64) {
        let upper = self.pressure.partition_point(|&p| p < pressure);
        if upper == 0 {
            return (self.density[0], self.energy[0]);
        }
        let last = self.pressure.len() - 1;
        if upper > last {
            return (self.density[last], self.energy[last]);
        }
        let (p0, p1) = (self.pressure[upper - 1], self.pressure[upper]);
        let t = if p1 > p0 {
            (pressure - p0) / (p1 - p0)
        } else {
            0.0
        };
        let lerp = |v: &[f64]| v[upper - 1] + t * (v[upper] - v[upper - 1]);
        (lerp(&self.density), lerp(&self.energy))
    }
}

/// The structure of a fluid planet of mass `mass_kg` with `core_fraction` of it in a core of
/// `core` under a mantle of `mantle`, in hydrostatic equilibrium at zero temperature: or `None` for
/// a mass or fraction out of range, or a material that is not valid.
///
/// ```text
/// dm/dr = 4π·r²·ρ,    dP/dr = −G·m·ρ / r²,    ρ, u on the cold isentrope at P
/// ```
///
/// integrated outward by fourth-order Runge–Kutta, in the core's material while the enclosed mass
/// is under `core_fraction·M` and the mantle's after, until the pressure falls to zero: that is the
/// surface. The central pressure is bisected until the mass inside the surface is `M`. Each
/// material's density and energy at a pressure come from its cold isentrope
/// ([`tillotson_cold_energy`]), so the planet starts in equilibrium with the energy compression put
/// into it, as WoMa builds planets (`woma/eos/tillotson.py`, <https://github.com/srbonilla/WoMa>, read
/// 2026-11-22). The radius is an outcome, not an input.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{hydrostatic_profile, TillotsonParameters};
///
/// let rock = TillotsonParameters {
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
/// // A small rock body is barely compressed: its radius is that of uncompressed rock.
/// let mass = 1.0e20;
/// let profile = hydrostatic_profile(mass, 0.0, &rock, &rock).unwrap();
/// let uncompressed = (3.0 * mass / (4.0 * std::f64::consts::PI * 2700.0)).cbrt();
/// assert!((profile.radius_m / uncompressed - 1.0).abs() < 0.01);
/// ```
pub fn hydrostatic_profile(
    mass_kg: f64,
    core_fraction: f64,
    core: &TillotsonParameters,
    mantle: &TillotsonParameters,
) -> Option<PlanetProfile> {
    let valid = mass_kg.is_finite()
        && mass_kg > 0.0
        && (0.0..1.0).contains(&core_fraction)
        && core.is_valid()
        && mantle.is_valid();
    if !valid {
        return None;
    }
    let (core_table, mantle_table) = (Isentrope::of(core), Isentrope::of(mantle));
    let core_mass = core_fraction * mass_kg;
    let state = |pressure: f64, enclosed: f64| {
        if enclosed < core_mass {
            core_table.at(pressure)
        } else {
            mantle_table.at(pressure)
        }
    };
    let uncompressed =
        (3.0 * mass_kg / (4.0 * std::f64::consts::PI * mantle.reference_density_kg_m3)).cbrt();
    let dr = uncompressed / f64::from(PLANET_PROFILE_RADIAL_STEPS);
    // Far more steps than any planet needs; a profile that has not reached zero pressure by then
    // (a central pressure the table cannot hold up) counts as holding all the mass, so the
    // bisection lowers the pressure.
    let step_limit = 100 * PLANET_PROFILE_RADIAL_STEPS;

    // Integrate from central pressure `central`, returning the samples and the surface's (r, m).
    let integrate = |central: f64, record: bool| {
        let mut samples = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let derivative = |r: f64, m: f64, p: f64| {
            let (rho, _) = state(p, m);
            let dm = 4.0 * std::f64::consts::PI * r * r * rho;
            let dp = if r > 0.0 {
                -GRAVITATIONAL_CONSTANT * m * rho / (r * r)
            } else {
                0.0
            };
            (dm, dp)
        };
        let (mut r, mut m, mut p) = (0.0_f64, 0.0_f64, central);
        for _ in 0..step_limit {
            if record {
                let (rho, u) = state(p, m);
                samples.0.push(r);
                samples.1.push(m);
                samples.2.push(rho);
                samples.3.push(u);
            }
            let k1 = derivative(r, m, p);
            let k2 = derivative(r + 0.5 * dr, m + 0.5 * dr * k1.0, p + 0.5 * dr * k1.1);
            let k3 = derivative(r + 0.5 * dr, m + 0.5 * dr * k2.0, p + 0.5 * dr * k2.1);
            let k4 = derivative(r + dr, m + dr * k3.0, p + dr * k3.1);
            let next_m = m + dr / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0);
            let next_p = p + dr / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1);
            if next_p <= 0.0 {
                // The surface lies inside this step, where the pressure crosses zero.
                let t = p / (p - next_p);
                let surface = (r + t * dr, m + t * (next_m - m));
                if record {
                    let (rho, u) = state(0.0, surface.1);
                    samples.0.push(surface.0);
                    samples.1.push(surface.1);
                    samples.2.push(rho);
                    samples.3.push(u);
                }
                return (samples, surface);
            }
            (r, m, p) = (r + dr, next_m, next_p);
        }
        (samples, (r, f64::INFINITY))
    };

    // The surface mass grows with the central pressure: bisect its logarithm.
    let (mut low, mut high) = (6.0_f64, 15.0_f64);
    for _ in 0..PLANET_PROFILE_BISECTIONS {
        let middle = 0.5 * (low + high);
        let (_, (_, surface_mass)) = integrate(10f64.powf(middle), false);
        if surface_mass < mass_kg {
            low = middle;
        } else {
            high = middle;
        }
    }
    let central = 10f64.powf(0.5 * (low + high));
    let ((radii, enclosed, density, energy), (radius, mass)) = integrate(central, true);
    if !(radius.is_finite() && mass.is_finite()) || radii.len() < 2 {
        return None;
    }
    let mut profile = PlanetProfile {
        radius_m: radius,
        core_radius_m: 0.0,
        central_pressure_pa: central,
        mass_kg: mass,
        radii,
        enclosed,
        density,
        energy,
    };
    profile.core_radius_m = if core_mass > 0.0 {
        profile.radius_enclosing(core_fraction)
    } else {
        0.0
    };
    Some(profile)
}

/// A fluid planet to build out of particles: an iron core under a rock mantle. See
/// [`layered_particle_body`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LayeredBody {
    /// Where its centre is, in world units.
    pub centre_units: DVec3,
    /// How it moves as a whole, in world units per second.
    pub velocity_units_s: DVec3,
    /// Its total mass, kg.
    pub mass_kg: f64,
    /// The share of the mass in the core, in `[0, 1)`; the innermost particles by mass are the core.
    pub core_fraction: f64,
    /// What the core's particles are made of: a [`Material::fluid`].
    pub core: Material,
    /// What the mantle's particles are made of: a [`Material::fluid`].
    pub mantle: Material,
    /// The colour the core's particles are drawn in.
    pub core_color: Color,
    /// The colour the mantle's particles are drawn in.
    pub mantle_color: Color,
}

/// Build `body` out of `count` fluid particles of equal mass (as near as the lattice allows),
/// following `profile` ([`hydrostatic_profile`], for the same mass, core and materials).
///
/// A face-centred cubic lattice is laid inside a unit sphere ([`lattice_points`]) and each point is
/// moved along its own direction to the radius that encloses its share of the mass: the `k`-th
/// point out of `n`, by distance from the centre, goes to where `(k + ½)/n` of the mass lies inside.
/// So every particle stands for an equal mass and the lattice takes the profile's density, dense in
/// the core and lighter outwards. The innermost `core_fraction` of them are the core. Each starts
/// at rest relative to the body, with the profile's density
/// ([`Prop::density_kg_m3`]) and internal energy ([`Prop::heat_j_kg`]) at its radius, drawn at the
/// radius that density gives ([`crate::collision::collision::fluid_radius_m`]).
///
/// Empty when `count` is zero.
pub fn layered_particle_body(body: &LayeredBody, profile: &PlanetProfile, count: u32) -> Vec<Prop> {
    if count == 0 {
        return Vec::new();
    }
    let spacing = lattice_spacing_m(1.0, count);
    let mut points = lattice_points(1.0, spacing);
    if points.is_empty() {
        points.push(DVec3::ZERO);
    }
    points.sort_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
    let n = points.len();
    let mass = body.mass_kg / n as f64;
    // The share of the mass inside each point: the middle of its shell's ranks, so that every point
    // of a shell of the lattice (points at one distance from the centre) goes to one radius and the
    // planet keeps the lattice's symmetry (its centre of mass at its centre).
    let mut shares = vec![0.0; n];
    let mut first = 0;
    while first < n {
        let length = points[first].length_squared();
        let mut last = first + 1;
        while last < n && points[last].length_squared() - length <= 1.0e-9 * length.max(1.0e-12) {
            last += 1;
        }
        let share = 0.5 * (first + last) as f64 / n as f64;
        shares[first..last].fill(share);
        first = last;
    }
    let mut props = points
        .iter()
        .zip(&shares)
        .enumerate()
        .map(|(rank, (point, &share))| {
            let radius = profile.radius_enclosing(share);
            let direction = point.normalize_or_zero();
            // The material by the particle's own rank, so the core holds its share of the mass to one
            // particle even where a shell straddles its edge; the masses are equal, so this moves
            // nothing.
            let in_core = (rank as f64 + 0.5) / (n as f64) < body.core_fraction;
            let density = profile.density_at(radius);
            let mut prop = Prop::new(
                Body::Sphere {
                    pose: Pose {
                        position: body.centre_units + direction * (radius / METRES_PER_UNIT),
                        rotation: Quat::IDENTITY,
                    },
                    // Redrawn at the measured density below; the profile never falls under ρ₀.
                    radius: (fluid_radius_m(mass, density, density) / METRES_PER_UNIT) as f32,
                    color: if in_core {
                        body.core_color
                    } else {
                        body.mantle_color
                    },
                },
                body.velocity_units_s,
                mass as f32,
            )
            .with_material(if in_core { body.core } else { body.mantle });
            prop.density_kg_m3 = density;
            prop
        })
        .collect::<Vec<Prop>>();
    // The simulation will measure each particle's density from its neighbours, which differs from
    // the profile's where the kernel reaches past the surface or across the core's edge. Measure it
    // as the simulation will (twice, so the smoothing lengths follow the measured densities), and put
    // each particle on its material's cold isentrope at that density: its pressure and its energy
    // then agree from the first step, and no particle can do more work expanding than compression
    // stored in it.
    measure_fluid_densities(&mut props);
    measure_fluid_densities(&mut props);
    for prop in &mut props {
        if let Some(eos) = prop.material.thermal {
            prop.heat_j_kg =
                tillotson_cold_energy(prop.density_kg_m3, &eos.parameters(), COLD_ISENTROPE_STEPS);
        }
    }
    props
}

/// Group the particles of `props` that touch, directly or through a chain of others, into clusters,
/// largest (by mass) first. Each cluster is a list of indices into `props`.
///
/// Only particles whose contacts are simulated (rubble with a [`Material::modulus_pa`], or a
/// [`fluid`](Material::fluid)) and that are spheres are grouped; every other prop is left out. Two
/// count as touching when their centres are within their summed radii plus
/// [`CLUSTER_CONTACT_TOLERANCE`] of it, so a pile resting under its own weight, whose particles
/// touch with almost no overlap, is one cluster. A fluid particle's radius follows its measured
/// density ([`crate::collision::collision::fluid_radius_m`]), so the same rule holds for it.
pub fn contact_clusters(props: &[Prop]) -> Vec<Vec<usize>> {
    let members: Vec<(usize, DVec3, f64)> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| prop.material.is_particle())
        .filter_map(|(index, prop)| match &prop.body {
            // Each member's radius with its share of the tolerance, so two members touch when
            // their centres are within the sum.
            Body::Sphere { pose, radius, .. } => {
                let tolerance = if prop.material.fluid {
                    FLUID_CLUSTER_CONTACT_TOLERANCE
                } else {
                    CLUSTER_CONTACT_TOLERANCE
                };
                Some((index, pose.position, f64::from(*radius) * (1.0 + tolerance)))
            }
            _ => None,
        })
        .collect();

    // Union-find over the members, finding touching pairs through a grid of cells as wide as the
    // largest reach, so each member is compared only with those in its own and the 26 neighbouring
    // cells: O(n) for a body of particles, where every pair would be O(n²) (`2e8` checks at
    // `20,000` particles).
    let mut parent: Vec<usize> = (0..members.len()).collect();
    fn root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    let largest = members.iter().map(|m| m.2).fold(0.0_f64, f64::max);
    let cell = 2.0 * largest;
    if cell.is_finite() && cell > 0.0 {
        let key = |p: DVec3| {
            let c = (p / cell).floor();
            (c.x as i64, c.y as i64, c.z as i64)
        };
        let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (member, entry) in members.iter().enumerate() {
            grid.entry(key(entry.1)).or_default().push(member);
        }
        for (a, entry) in members.iter().enumerate() {
            let (x, y, z) = key(entry.1);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(cell_members) = grid.get(&(x + dx, y + dy, z + dz)) else {
                            continue;
                        };
                        for &b in cell_members {
                            if b <= a {
                                continue;
                            }
                            let reach = entry.2 + members[b].2;
                            if (entry.1 - members[b].1).length_squared() <= reach * reach {
                                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                                if ra != rb {
                                    parent[ra] = rb;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot_of_root: Vec<Option<usize>> = vec![None; members.len()];
    for (member, entry) in members.iter().enumerate() {
        let r = root(&mut parent, member);
        let slot = *slot_of_root[r].get_or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[slot].push(entry.0);
    }
    let mass =
        |group: &Vec<usize>| -> f64 { group.iter().map(|&i| f64::from(props[i].mass_kg)).sum() };
    groups.sort_by(|a, b| mass(b).total_cmp(&mass(a)));
    groups
}

/// Where the mass of a collision of particle bodies has gone, as fractions of the total that add up
/// to `1`. See [`impact_outcome`].
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ImpactOutcome {
    /// The largest body: the most massive cluster of touching particles.
    pub largest: f64,
    /// Bound to the largest body on an orbit that clears its surface: the disk a moon forms from.
    pub disk: f64,
    /// Bound to the largest body on an orbit that comes back down to it.
    pub falling_back: f64,
    /// Faster than the largest body's escape speed where it is.
    pub escaping: f64,
    /// The largest body's radius, from its centre of mass to its farthest particle's far side, in
    /// metres: the surface the disk's orbits are measured against.
    pub radius_m: f64,
}

impl ImpactOutcome {
    /// The mass that ends up in the largest body once what falls back has landed: Leinhardt &
    /// Stewart's (2012) largest remnant, which is what
    /// [`crate::physics_utils::impacts::largest_remnant_fraction`] predicts.
    pub fn largest_remnant(&self) -> f64 {
        self.largest + self.falling_back
    }
}

/// Sort the particles of a collision by where their mass ends up: in the largest body, in orbit
/// around it, on their way back to it, or escaping.
///
/// The largest body is the most massive of the [`contact_clusters`]. Every other particle is judged
/// on its two-body orbit about that body's centre of mass, treating the body as a point mass: bound
/// when its energy `½·v² − G·M/r` relative to the body is negative, and then in the **disk** when
/// the orbit's closest approach, `h² / (G·M·(1 + e))`, clears the body's radius (the criterion of
/// Canup & Asphaug 2001, Nature 412:708, for the protolunar disk). Particles that are not simulated
/// particles are left out.
pub fn impact_outcome(props: &[Prop]) -> ImpactOutcome {
    let clusters = contact_clusters(props);
    let mut outcome = ImpactOutcome::default();
    let Some(body) = clusters.first() else {
        return outcome;
    };
    let mass_of = |i: usize| f64::from(props[i].mass_kg);
    let position = |i: usize| props[i].body.center() * METRES_PER_UNIT;
    let velocity = |i: usize| props[i].velocity_units_s * METRES_PER_UNIT;
    let radius = |i: usize| match &props[i].body {
        Body::Sphere { radius, .. } => {
            f64::from(props[i].contact_radius_units.unwrap_or(*radius)) * METRES_PER_UNIT
        }
        _ => 0.0,
    };

    let body_mass: f64 = body.iter().map(|&i| mass_of(i)).sum();
    let centre = body
        .iter()
        .map(|&i| position(i) * mass_of(i))
        .sum::<DVec3>()
        / body_mass;
    let drift = body
        .iter()
        .map(|&i| velocity(i) * mass_of(i))
        .sum::<DVec3>()
        / body_mass;
    outcome.radius_m = body
        .iter()
        .map(|&i| (position(i) - centre).length() + radius(i))
        .fold(0.0, f64::max);
    let mu = GRAVITATIONAL_CONSTANT * body_mass;

    let mut total = body_mass;
    outcome.largest = body_mass;
    for &i in clusters[1..].iter().flatten() {
        let m = mass_of(i);
        total += m;
        let r = position(i) - centre;
        let v = velocity(i) - drift;
        let energy = 0.5 * v.length_squared() - mu / r.length();
        if energy.is_nan() || energy >= 0.0 {
            outcome.escaping += m;
            continue;
        }
        let h = r.cross(v).length();
        let eccentricity = (1.0 + 2.0 * energy * h * h / (mu * mu)).max(0.0).sqrt();
        let periapsis = h * h / (mu * (1.0 + eccentricity));
        if periapsis > outcome.radius_m {
            outcome.disk += m;
        } else {
            outcome.falling_back += m;
        }
    }
    outcome.largest /= total;
    outcome.disk /= total;
    outcome.falling_back /= total;
    outcome.escaping /= total;
    outcome
}

#[cfg(test)]
#[path = "../../tst/collision/aggregate_tests.rs"]
mod tests;
