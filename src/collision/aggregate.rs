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

use crate::collision::constants::CLUSTER_CONTACT_TOLERANCE;
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Body, Color, DVec3, Material, Pose, Prop, Quat};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

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

/// Group the particles of `props` that touch, directly or through a chain of others, into clusters,
/// largest (by mass) first. Each cluster is a list of indices into `props`.
///
/// Only props whose contacts are simulated (both have a [`Material::modulus_pa`]) and that are
/// spheres are grouped; every other prop is left out. Two count as touching when their centres are
/// within their summed radii plus [`CLUSTER_CONTACT_TOLERANCE`] of it, so a pile resting under its
/// own weight, whose particles touch with almost no overlap, is one cluster.
pub fn contact_clusters(props: &[Prop]) -> Vec<Vec<usize>> {
    let members: Vec<(usize, DVec3, f64)> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| prop.material.modulus_pa.is_some())
        .filter_map(|(index, prop)| match &prop.body {
            Body::Sphere { pose, radius, .. } => Some((index, pose.position, f64::from(*radius))),
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
    let cell = 2.0 * largest * (1.0 + CLUSTER_CONTACT_TOLERANCE);
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
                            let reach =
                                (entry.2 + members[b].2) * (1.0 + CLUSTER_CONTACT_TOLERANCE);
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
