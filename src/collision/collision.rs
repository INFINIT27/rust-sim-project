//! Collision between bodies: what happens when two bodies held together by their own gravity meet.
//!
//! The bounce in [`crate::collision::resolve`] is right for two rocks. It is wrong for two
//! planets: a planet is held together by gravity, not by the strength of its rock, and two planets
//! that meet at a few km/s deform, flow into each other and become one. This module works that out
//! in one of two ways, depending on what the bodies are made of.
//!
//! # Simulated contact, for bodies made of particles
//!
//! A planet built from particles ([`crate::collision::aggregate`]) has particles whose material has
//! a modulus ([`Material::modulus_pa`](crate::core::scene::Material::modulus_pa)). Their contacts
//! are not resolved in an instant: [`apply_contact_forces`] pushes every overlapping pair apart with
//! a damped spring, the linear spring-dashpot of [`crate::physics_utils::soft_contact`], on every
//! substep they overlap. The stiffness comes from the modulus and the particles' size; the damping
//! from the material's restitution (by Schwager & Pöschel's relation for a contact that never
//! pulls), so `e = 0` is the heaviest damping the step allows, [`SOFT_CONTACT_MAX_DAMPING_RATIO`],
//! which returns `7.6%` of the approach speed.
//!
//! Friction at the contact acts on the slip of the two surfaces, so it spins the particles
//! ([`Prop::spin_rad_s`](crate::core::scene::Prop::spin_rad_s)) with torques that conserve angular
//! momentum exactly. For particles whose material has a thermal part, what the contacts dissipate
//! becomes heat ([`Prop::heat_j_kg`](crate::core::scene::Prop::heat_j_kg)), and hot neighbours push
//! apart by their Tillotson thermal pressure; [`particle_energy`] keeps the books, which balance to
//! the integrator's error. See [`apply_contact_forces`].
//!
//! Nothing about the merge is written down. When two such planets meet, the particles at the
//! contact are pushed into the ones behind, lose their speed to the damping, and slide past each
//! other; the two piles flatten, flow together and settle into one, held by their gravity, and the
//! particles thrown fast enough escape as debris. That **plastic** deformation, the permanent change
//! of shape, is the particles rearranging. The scaling law below becomes what such a run is checked
//! against, rather than what it is told.
//!
//! The force acts on velocities, before gravity moves anything, so each substep is a kick from
//! the contacts and gravity together and then a drift: the same symplectic Euler step the
//! integrator takes. Momentum is conserved exactly: every push on one particle is an equal and
//! opposite push on another.
//!
//! # Instant merge, for bodies that are single spheres
//!
//! Two planets that are each one rigid sphere cannot deform, so [`merge_contacts`] resolves their
//! contact in one step: they become one body, with debris from the scaling law.
//!
//! ## When a pair merges
//!
//! All of these, for a pair whose spheres overlap:
//!
//! 1. **Both are gravity-dominated**, `G·M/R` above
//!    [`GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG`] (a rocky body over about a kilometre). A rigid body,
//!    a spacecraft above all, keeps its shape and is left to the bounce.
//! 2. **They are closing.** A pair already moving apart is on its way out.
//! 3. **Their contact loses energy**: the pair's restitution is below `1`. A perfectly elastic pair
//!    bounces back to where it started forever, so it never settles into one body; that is what
//!    every prop with [`Material::ELASTIC`](crate::core::scene::Material::ELASTIC) does, and why
//!    this pass changes nothing in a scene that has not given its bodies materials.
//! 4. **Their contact is not simulated**: not both made of particles with a modulus. Those are left
//!    to [`apply_contact_forces`].
//! 5. **They would leave contact slower than their mutual escape speed.** The bounce is worked out
//!    with the pair's material, as [`crate::collision::resolve`] would apply it. If what is left of
//!    the relative speed is below
//!    [`crate::physics_utils::impacts::mutual_escape_speed`], the pair is
//!    bound: it would rise, fall back, and hit again, each time losing more, until the two are one.
//!    This pass resolves the whole sequence at once.
//!
//! A pair that fails test 5 leaves faster than escape speed: a grazing or fast **hit-and-run**, and
//! the bounce resolves it. Nothing about hit-and-run is written here; it falls out of the bounce.
//!
//! ## The merged body
//!
//! - **Mass and momentum are conserved exactly**, with the debris counted.
//! - **It sits at the pair's centre of mass**, shifted so the centre of mass of the remnant and its
//!   debris together is exactly where the pair's was. (A black hole that absorbs stays put; see
//!   [`crate::collision::absorb`]. A merged planet has no reason to.)
//! - **Its radius comes from volume**: `R = (R₁³ + R₂³)^(1/3)`, scaled by the share of mass it
//!   keeps. Its drawn radius grows in proportion, so a body drawn larger than it is stays so.
//! - **It is the more massive body**, keeping that body's id, name and colour.
//!
//! ## The debris
//!
//! How much mass the impact throws off comes from the measured scaling law of Leinhardt & Stewart
//! (2012): the specific impact energy against the catastrophic-disruption threshold, in
//! [`crate::physics_utils::impacts`]. A share under [`DEBRIS_MIN_MASS_FRACTION`] is a perfect merge.
//!
//! **Only above the mutual escape speed.** Leinhardt & Stewart's simulations put every head-on
//! impact slower than the mutual escape speed in the *perfect merging* regime; their debris law is
//! fitted to faster ones. An impact at or below escape speed is therefore a perfect merge here.
//! (Until 2026-11-21 the law was applied at any speed, and the Earth struck from rest by a Mars-mass
//! body threw off `0.55%`. The particle simulation of the same impact found no debris, which is what
//! sent us back to the paper.)
//!
//! The debris is split into [`CollisionSettings::debris_fragments`] equal bodies, the detail dial
//! the menu sets. They start on a shell just outside the merged body, spread evenly over it, at the
//! density of the bodies they came from, and leave radially at speeds spread between
//! [`DEBRIS_MIN_SPEED_FACTOR`] and [`DEBRIS_MAX_SPEED_FACTOR`] of the escape speed, so some fall back
//! (and merge again), some escape. Their kinetic energy, with the remnant's recoil, is never more
//! than the impact brought in the centre-of-mass frame; the speeds are scaled down if it would be.
//!
//! # What this leaves out
//!
//! - **Direction.** Real debris leaves in a fan from the impact site, mostly in the plane of the
//!   impact. Here it leaves evenly in every direction.
//! - **Spin.** An instant merge gives the merged body no spin, so an off-centre merge's angular
//!   momentum is lost. (Particle bodies, whose contacts are simulated, keep it.)
//! - **Heat.** The kinetic energy an instant merge does not give to the debris leaves the books, as it
//!   does in [`crate::collision::absorb`]. A ledger for it is phase 2 of the design in
//!   `src/collision/README.md`.
//!
//! [`CollisionSettings::debris_fragments`]: crate::core::scene::CollisionSettings::debris_fragments

use crate::collision::constants::{
    DEBRIS_MAX_SPEED_FACTOR, DEBRIS_MIN_MASS_FRACTION, DEBRIS_MIN_SPEED_FACTOR,
    DEBRIS_SPAWN_CLEARANCE, GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG, SOFT_CONTACT_MAX_DAMPING_RATIO,
    SOFT_CONTACT_MAX_NEIGHBOURS, SPH_COURANT, SPH_MAX_SMOOTHING_FRACTION,
    SPH_MIN_SMOOTHING_FRACTION, SPH_SMOOTHING_FACTOR, SPH_SOUND_SPEED_FLOOR_FRACTION,
    SPH_VISCOSITY_ALPHA, SPH_VISCOSITY_BETA, SPH_VISCOSITY_EPSILON,
};
use crate::constants::METRES_PER_UNIT;
use crate::core::constants::MAX_DEBRIS_FRAGMENTS;
use crate::core::scene::{Body, BodyId, Color, DVec3, Material, Pose, Prop, Quat, Scene};
use crate::physics_utils::conservation_laws::coulomb_friction_speed_change;
use crate::physics_utils::impacts::{
    catastrophic_disruption_threshold, largest_remnant_fraction, mutual_escape_speed,
    specific_impact_energy,
};
use crate::physics_utils::physics_constants::{FCC_PACKING_FRACTION, GRAVITATIONAL_CONSTANT};
use crate::physics_utils::soft_contact::{
    contact_duration, contact_stiffness, damping_ratio_for_restitution, spring_dashpot_force,
};
use crate::physics_utils::thermal::{
    cubic_spline_gradient, cubic_spline_kernel, particle_density, sphere_moment_of_inertia,
    tillotson_pressure, tillotson_sound_speed, tillotson_thermal_pressure, TillotsonParameters,
};
use crate::time::simulation::TimeStep;

/// Is the contact between `a` and `b` simulated as a force, rather than resolved in an instant?
///
/// True when both are movable spheres with mass whose material has a modulus: the particles of a
/// body built from particles. Such pairs are never bounced by [`crate::collision::resolve`] or
/// merged by [`merge_contacts`]; [`apply_contact_forces`] alone acts on them.
pub fn contact_is_simulated(a: &Prop, b: &Prop) -> bool {
    let soft = |prop: &Prop| {
        prop.material.is_particle()
            && prop.responds_to_forces()
            && matches!(prop.body, Body::Sphere { .. })
    };
    soft(a) && soft(b)
}

/// Push apart every overlapping pair whose contact is simulated, turn friction into spin, and turn
/// what the contacts dissipate into heat that pushes back, for one substep of `step`. Returns how
/// many pairs were in contact.
///
/// The simulated-contact half of [`Scene::step`](crate::core::scene::Scene::step), run at the start
/// of each substep, before gravity moves anything (see the module docs for why). For each pair that
/// [`contact_is_simulated`] and that overlaps by `δ`, with `n` the unit vector from `a` to `b`:
///
/// ```text
/// k  = E*·r*                    contact_stiffness: the pair's modulus and reduced radius
/// c  = 2·ζ·√(m*·k)              ζ from the pair's restitution
/// Fn = max(0, k·δ − c·v_n)      along n, never pulling
/// Ft = min(μ·Fn, c·|v_t|)       against the slip of the contact point, capped by Coulomb
/// ```
///
/// - **Spin.** The slip `v_t` is of the two surfaces at the contact point, so it includes each
///   particle's spin, and friction applies a torque `arm × Ft` to each. The arms are the two
///   radii scaled to add up to the distance between the centres, so the two torques and the two
///   forces conserve angular momentum exactly.
/// - **Heat.** What the contact takes out of the motion, `(k·δ − Fn)·v_n` for the normal part
///   (the dashpot, or the spring's energy when the contact lets go before it is spent) and
///   `Ft·|v_t|` for friction, is shared equally between the two particles as heat
///   ([`Prop::heat_j_kg`]) when their material has a [`thermal`](crate::core::scene::Material::thermal)
///   part; otherwise it leaves the simulation as before.
/// - **Pressure.** Two hot particles within each other's smoothing length push apart by their
///   Tillotson thermal pressures, in the symmetric form of smoothed-particle hydrodynamics:
///
///   ```text
///   F_a = −m_a·m_b·(P_a/ρ_a² + P_b/ρ_b²)·∇W     ∇W the kernel gradient at a, from b
///   du_a/dt = ½·m_b·(P_a/ρ_a² + P_b/ρ_b²)·(v_a − v_b)·∇W
///   ```
///
///   The work the pressure does comes out of the heat, so it creates no energy, and the force is
///   along `n`, so it creates no angular momentum. See [`crate::physics_utils::thermal`].
///
/// **Fluid particles** ([`Material::fluid`]) take none of the above. Their density is measured from
/// their neighbours first (`fluid_densities`), and two of them push on each other by their
/// Tillotson pressures and Monaghan's artificial viscosity (`fluid_pair_forces`), with each
/// particle's own pressure doing its compression work in the heat pass. A fluid and a rubble
/// particle feel only each other's gravity.
///
/// Every force is evaluated from the state at the start of the substep, so the order the pairs are
/// visited in changes nothing. [`particle_energy`] is the ledger it keeps: kinetic, spin, gravity,
/// spring and heat together stay constant to the integrator's error.
///
/// # Cost
///
/// Every pair of simulated particles is tested, `n·(n−1)/2` distance checks; the forces are worked
/// out only for the neighbours. Gravity already costs the same pairs. The GPU path
/// ([`crate::compute`]) can find the neighbours with a tree instead.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{apply_contact_forces, Body, Color, DVec3, Material, Pose, Prop, Quat, TimeStep};
///
/// let rock = Material::new(0.0, 0.0).with_modulus(1.0e9);
/// let ball = |x: f64, vx: f64| {
///     Prop::new(
///         Body::Sphere {
///             pose: Pose { position: DVec3::new(x, 0.0, 0.0), rotation: Quat::IDENTITY },
///             radius: 1.0e-9, // 1 m
///             color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
///         },
///         DVec3::new(vx, 0.0, 0.0),
///         1000.0,
///     )
///     .with_material(rock)
/// };
/// // Overlapping by 10 cm, closing.
/// let mut props = vec![ball(0.0, 1.0e-9), ball(1.9e-9, -1.0e-9)];
/// assert_eq!(apply_contact_forces(&mut props, TimeStep::from_seconds(1.0e-4).unwrap()), 1);
/// // Pushed apart, equally and oppositely.
/// assert!(props[0].velocity_units_s.x < 1.0e-9);
/// assert_eq!(props[0].velocity_units_s.x + props[1].velocity_units_s.x, 0.0);
/// ```
pub fn apply_contact_forces(props: &mut [Prop], step: TimeStep) -> usize {
    let pass = push_contacts(props, step);
    let touching = pass.touching();
    pass.heat(props, step);
    touching
}

/// The force half of [`apply_contact_forces`]: kick every simulated particle by its contacts'
/// forces and torques, and return what [`ContactPass::heat`] needs to book the heat once the rest
/// of the substep has moved them.
///
/// [`Scene::step`](crate::core::scene::Scene::step) calls this, then gravity, then
/// [`ContactPass::heat`], so the heat is worked out at the mean of each particle's velocity at the
/// start and the end of the **whole** substep. That is the velocity the forces did their work at.
/// The middle of the contact kick alone is not: in a pile resting under its own weight the contact
/// kick and gravity's cancel and nothing moves, but the contact kick's middle would show the pile
/// expanding, and its pressure would do work it never did (measured, a resting hot planet then lost
/// a percent of its binding energy an hour from its heat).
pub fn push_contacts(props: &mut [Prop], step: TimeStep) -> ContactPass {
    let dt = step.seconds();
    let mut pass = ContactPass::default();
    if dt.is_nan() || dt <= 0.0 {
        return pass;
    }

    let mut particles: Vec<Particle> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| contact_is_simulated(prop, prop))
        .filter_map(|(index, prop)| Particle::of(index, prop))
        .collect();
    if particles.len() < 2 {
        return pass;
    }
    measure_fluids(&mut particles, props);

    // Every neighbouring pair's forces and torques, from the start of the substep.
    let mut force = vec![DVec3::ZERO; particles.len()];
    let mut torque = vec![DVec3::ZERO; particles.len()];
    // Only pairs within reach can act on each other; the grid finds them in O(n), in the same
    // order the loop over every pair visited them.
    let points: Vec<DVec3> = particles.iter().map(|p| p.position).collect();
    let reach: Vec<f64> = particles
        .iter()
        .map(|p| p.fluid.map_or_else(|| p.reach(), |f| f.smoothing))
        .collect();
    {
        for (a, b) in neighbour_pairs(&points, &reach) {
            let (first, second) = (&particles[a], &particles[b]);
            let pair = match (first.fluid.is_some(), second.fluid.is_some()) {
                (true, true) => fluid_pair_forces(first, second),
                (false, false) => {
                    let materials = (&props[first.index].material, &props[second.index].material);
                    pair_forces(first, second, materials, dt)
                }
                // A fluid and a rubble particle feel only each other's gravity: every scene is
                // built of one model.
                _ => None,
            };
            let Some(pair) = pair else {
                continue;
            };
            force[a] -= pair.force_on_b;
            force[b] += pair.force_on_b;
            torque[a] += pair.torque_on_a;
            torque[b] += pair.torque_on_b;
            pass.pairs.push((a, b, pair));
        }
    }

    for (i, particle) in particles.iter().enumerate() {
        let prop = &mut props[particle.index];
        prop.velocity_units_s += force[i] * (dt / particle.mass) / METRES_PER_UNIT;
        if particle.inertia > 0.0 {
            prop.spin_rad_s += torque[i] * (dt / particle.inertia);
        }
    }
    pass.particles = particles;
    pass
}

/// What [`push_contacts`] did, kept until the substep is over so the heat can be booked: the
/// particles as they were at its start, and every pair's forces.
#[derive(Default)]
pub struct ContactPass {
    particles: Vec<Particle>,
    pairs: Vec<(usize, usize, PairForces)>,
}

impl ContactPass {
    /// How many pairs were touching.
    pub fn touching(&self) -> usize {
        self.pairs
            .iter()
            .filter(|(_, _, pair)| pair.touching)
            .count()
    }

    /// Book the heat of the substep `step`: the work the dissipative and pressure forces did, at
    /// the mean of each particle's velocity and spin at the start of the substep and now.
    ///
    /// That is exactly what the kick took from the motion (a kick of `Δv` changes `½mv²` by
    /// `m·Δv·(v + Δv/2)`), so no energy is made or lost in the books, however heavy the damping.
    /// `props` must be the slice [`push_contacts`] was given, with no prop added or removed since.
    pub fn heat(self, props: &mut [Prop], step: TimeStep) {
        let dt = step.seconds();
        if self.pairs.is_empty() || dt.is_nan() || dt <= 0.0 {
            return;
        }
        let mean: Vec<(DVec3, DVec3)> = self
            .particles
            .iter()
            .map(|particle| {
                let prop = &props[particle.index];
                (
                    0.5 * (particle.velocity + prop.velocity_units_s * METRES_PER_UNIT),
                    0.5 * (particle.spin + prop.spin_rad_s),
                )
            })
            .collect();

        let mut heating = vec![0.0; self.particles.len()];
        for (a, b, pair) in &self.pairs {
            let (velocity_a, spin_a) = mean[*a];
            let (velocity_b, spin_b) = mean[*b];
            // Not clamped at zero: near the step's stability limit the start-of-step and mean speeds
            // can disagree in sign, and that step's kick then gave energy back. Clamping it booked
            // heat the step never took (measured, `6%` of a settling planet's binding energy in two
            // hours).
            let mut dissipated = pair.dashpot * (velocity_b - velocity_a).dot(pair.normal);
            if pair.friction_on_b != DVec3::ZERO {
                let surface_a = velocity_a + spin_a.cross(pair.normal * pair.arm_a);
                let surface_b = velocity_b + spin_b.cross(-pair.normal * pair.arm_b);
                dissipated -= pair.friction_on_b.dot(surface_b - surface_a);
            }
            let hot_a = self.particles[*a].thermal.is_some();
            let hot_b = self.particles[*b].thermal.is_some();
            let (share_a, share_b) = match (hot_a, hot_b) {
                (true, true) => (0.5, 0.5),
                (true, false) => (1.0, 0.0),
                (false, true) => (0.0, 1.0),
                (false, false) => (0.0, 0.0),
            };
            heating[*a] += share_a * dissipated;
            heating[*b] += share_b * dissipated;
            if let Some((coefficient, gradient_at_a)) = pair.pressure {
                // The pressure's work on the pair, `−F_a·(v_a − v_b)`, back into the heat, half each.
                let work = coefficient * (velocity_a - velocity_b).dot(gradient_at_a);
                heating[*a] += 0.5 * work;
                heating[*b] += 0.5 * work;
            }
            if let Some(fluid) = &pair.fluid {
                // The energy equation of smoothed-particle hydrodynamics (Monaghan 1992):
                // `du_a/dt = Σ m_b·(P_a/ρ_a² + Π/2)·v_ab·∇W_a`, and `b`'s own pressure for `b`.
                // Each particle's own pressure does its compression work, so cold rock squeezed
                // next to hot rock heats only by its own pressure; the two shares add up to the
                // work the pair's force did, so the books balance.
                let rate = (velocity_a - velocity_b).dot(fluid.gradient_at_a);
                heating[*a] += (fluid.own_a + 0.5 * fluid.viscous) * rate;
                heating[*b] += (fluid.own_b + 0.5 * fluid.viscous) * rate;
            }
        }
        for (particle, heat) in self.particles.iter().zip(&heating) {
            if particle.thermal.is_some() || particle.fluid.is_some() {
                let prop = &mut props[particle.index];
                prop.heat_j_kg = (prop.heat_j_kg + heat * dt / particle.mass).max(0.0);
            }
        }
    }
}

/// A simulated particle as [`apply_contact_forces`] and [`particle_energy`] see it, in SI.
#[derive(Copy, Clone, Debug)]
struct Particle {
    index: usize,
    position: DVec3,
    velocity: DVec3,
    spin: DVec3,
    radius: f64,
    mass: f64,
    inertia: f64,
    /// The thermal pressure, smoothing length and density, when the material has a thermal part
    /// and is not a fluid.
    thermal: Option<Hot>,
    /// The state of a fluid particle, when the material is one.
    fluid: Option<Fluid>,
}

/// What a fluid particle brings to smoothed-particle hydrodynamics. The density, pressure and
/// sound speed are measured by [`measure_fluids`] at the start of each substep.
#[derive(Copy, Clone, Debug)]
struct Fluid {
    eos: TillotsonParameters,
    /// The specific internal energy `u`, J/kg.
    energy: f64,
    /// The smoothing length, from the last substep's density.
    smoothing: f64,
    density: f64,
    pressure: f64,
    sound: f64,
}

/// What a particle with a thermal material brings to the pressure.
#[derive(Copy, Clone, Debug)]
struct Hot {
    pressure: f64,
    density: f64,
    smoothing: f64,
}

impl Particle {
    fn of(index: usize, prop: &Prop) -> Option<Self> {
        let radius = physical_radius_m(prop)?;
        let mass = f64::from(prop.mass_kg);
        let fluid = match (prop.material.fluid, prop.material.thermal) {
            (true, Some(eos)) => {
                let eos = eos.parameters();
                let density = if prop.density_kg_m3 > 0.0 {
                    prop.density_kg_m3
                } else {
                    eos.reference_density_kg_m3
                };
                Some(Fluid {
                    eos,
                    energy: prop.heat_j_kg,
                    smoothing: fluid_smoothing_m(mass, density, &eos),
                    density,
                    pressure: tillotson_pressure(density, prop.heat_j_kg, &eos),
                    sound: fluid_sound_speed(density, prop.heat_j_kg, &eos),
                })
            }
            _ => None,
        };
        let thermal = prop
            .material
            .thermal
            .filter(|_| fluid.is_none())
            .map(|eos| {
                let density = particle_density(mass, radius);
                Hot {
                    pressure: tillotson_thermal_pressure(
                        density,
                        prop.heat_j_kg,
                        f64::from(eos.a),
                        f64::from(eos.b),
                        f64::from(eos.reference_energy_j_kg),
                    ),
                    density,
                    smoothing: smoothing_length_m(radius),
                }
            });
        Some(Self {
            index,
            position: prop.body.center() * METRES_PER_UNIT,
            velocity: prop.velocity_units_s * METRES_PER_UNIT,
            spin: prop.spin_rad_s,
            radius,
            mass,
            inertia: sphere_moment_of_inertia(mass, radius),
            thermal,
            fluid,
        })
    }

    /// How far out this particle has neighbours: its smoothing length when it is hot-capable, else
    /// its radius.
    fn reach(&self) -> f64 {
        self.thermal
            .map_or(self.radius, |hot| hot.smoothing.max(self.radius))
    }
}

/// The smoothing length of a fluid particle of mass `mass_kg` at density `density_kg_m3`, in
/// metres: [`SPH_SMOOTHING_FACTOR`] times its spacing `(m/ρ)^(1/3)`, kept within
/// [`SPH_MIN_SMOOTHING_FRACTION`] and [`SPH_MAX_SMOOTHING_FRACTION`] of its value at the material's
/// reference density. A density that is not positive counts as the reference density.
///
/// So a particle keeps about the same number of neighbours compressed in a core as thrown into a
/// disk. The kernel reaches twice this.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{fluid_smoothing_m, TillotsonParameters, SPH_SMOOTHING_FACTOR};
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
/// let mass = 2700.0; // one cubic metre at the reference density
/// assert!((fluid_smoothing_m(mass, 2700.0, &rock) - SPH_SMOOTHING_FACTOR).abs() < 1e-12);
/// // Eight times denser: half the spacing.
/// assert!((fluid_smoothing_m(mass, 8.0 * 2700.0, &rock) - 0.5 * SPH_SMOOTHING_FACTOR).abs() < 1e-12);
/// // In near-vacuum, capped.
/// assert!(fluid_smoothing_m(mass, 1e-6, &rock) <= 3.0 * SPH_SMOOTHING_FACTOR + 1e-12);
/// ```
pub fn fluid_smoothing_m(mass_kg: f64, density_kg_m3: f64, eos: &TillotsonParameters) -> f64 {
    let reference = SPH_SMOOTHING_FACTOR * (mass_kg / eos.reference_density_kg_m3).cbrt();
    let density = if density_kg_m3.is_finite() && density_kg_m3 > 0.0 {
        density_kg_m3
    } else {
        eos.reference_density_kg_m3
    };
    let smoothing = SPH_SMOOTHING_FACTOR * (mass_kg / density).cbrt();
    smoothing.clamp(
        SPH_MIN_SMOOTHING_FRACTION * reference,
        SPH_MAX_SMOOTHING_FRACTION * reference,
    )
}

/// The radius a fluid particle of mass `mass_kg` is drawn with at density `density_kg_m3`, in
/// metres, for a material of reference density `reference_density_kg_m3`: the radius of a sphere in
/// a face-centred cubic packing of the density, the inverse of [`particle_density`], but never
/// larger than at the reference density.
///
/// So a compressed planet's particles just touch, drawn smaller in its core; but a particle that has
/// expanded, or been thrown clear of its neighbours, keeps the size it has in uncompressed rock. Its
/// measured density there is not a size: alone in space a particle measures only itself, `1/186` of
/// rock's density at the largest smoothing length, and drawn at that density it would be `5.7` times
/// too large, which after an impact puffed every hot and flung particle up and, through
/// [`crate::collision::aggregate::impact_outcome`], made the merged body read `16,700 km` across.
/// Only drawing and the clustering of bodies ([`crate::collision::aggregate`]) read this radius;
/// the forces read the smoothing length.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{fluid_radius_m, particle_density};
///
/// // Compressed: the packing radius of that density.
/// let radius = fluid_radius_m(1.0e20, 3000.0, 2700.0);
/// assert!((particle_density(1.0e20, radius) / 3000.0 - 1.0).abs() < 1e-12);
/// // Expanded: no larger than in uncompressed rock.
/// assert_eq!(fluid_radius_m(1.0e20, 10.0, 2700.0), fluid_radius_m(1.0e20, 2700.0, 2700.0));
/// ```
pub fn fluid_radius_m(mass_kg: f64, density_kg_m3: f64, reference_density_kg_m3: f64) -> f64 {
    let density = density_kg_m3.max(reference_density_kg_m3);
    (3.0 * mass_kg * FCC_PACKING_FRACTION / (4.0 * std::f64::consts::PI * density)).cbrt()
}

/// The sound speed of a fluid particle: [`tillotson_sound_speed`], floored at
/// [`SPH_SOUND_SPEED_FLOOR_FRACTION`] of the material's bulk sound speed `√(A/ρ₀)` so that thin
/// vapour still carries a signal.
fn fluid_sound_speed(density: f64, energy: f64, eos: &TillotsonParameters) -> f64 {
    let floor =
        SPH_SOUND_SPEED_FLOOR_FRACTION * (eos.bulk_a_pa / eos.reference_density_kg_m3).sqrt();
    tillotson_sound_speed(density, energy, eos).max(floor)
}

/// Every pair of `points` closer than the sum of their `reach`es, each pair once as `(a, b)` with
/// `a < b`, found through a grid of cells as wide as the largest summed reach: O(n) for a body of
/// particles, where testing every pair would be O(n²).
pub(crate) fn neighbour_pairs(points: &[DVec3], reach: &[f64]) -> Vec<(usize, usize)> {
    let largest = reach.iter().copied().fold(0.0_f64, f64::max);
    let cell = 2.0 * largest;
    let mut pairs = Vec::new();
    if !(cell.is_finite() && cell > 0.0) {
        return pairs;
    }
    let key = |p: DVec3| {
        let c = (p / cell).floor();
        (c.x as i64, c.y as i64, c.z as i64)
    };
    let mut grid: std::collections::HashMap<(i64, i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, point) in points.iter().enumerate() {
        grid.entry(key(*point)).or_default().push(i);
    }
    for (a, point) in points.iter().enumerate() {
        let (x, y, z) = key(*point);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(members) = grid.get(&(x + dx, y + dy, z + dz)) else {
                        continue;
                    };
                    for &b in members {
                        if b <= a {
                            continue;
                        }
                        let limit = reach[a] + reach[b];
                        if (points[b] - *point).length_squared() < limit * limit {
                            pairs.push((a, b));
                        }
                    }
                }
            }
        }
    }
    pairs.sort_unstable();
    pairs
}

/// What [`fluid_densities`] needs of a fluid particle, in SI.
#[derive(Copy, Clone, Debug)]
struct FluidSample {
    position: DVec3,
    mass: f64,
    smoothing: f64,
    /// The material's reference density, for Woolfson's factor.
    reference_density: f64,
}

/// The SPH density of each fluid particle in `particles`, in kg/m³:
///
/// ```text
/// ρ_a = m_a·W(0, h_a) + Σ_b f_ab·m_b·W(r_ab, h̄),    f_ab = ρ₀_a / ρ₀_b
/// ```
///
/// The factor `f_ab` is the density correction of Woolfson, M. M. (2007), The development of
/// structure in planetary interiors with smoothed particle hydrodynamics, *Monthly Notices of the
/// Royal Astronomical Society* 376(3), 1173–1178, doi:10.1111/j.1365-2966.2007.11498.x, in its
/// simplest form: Woolfson's factor is the ratio of the two materials' densities at the same pressure
/// and temperature, and here it is the ratio at zero pressure, exact at the surface and treating the
/// two as equally compressed below it. Between two particles of one material it is `1`, the
/// standard summation. Without it, rock next to iron counts the iron's mass as its own and reads
/// far too dense: at 200 particles a planet's mantle at the core read 40% over its true density and
/// blew the planet apart. The more accurate treatment of Ruiz-Bonilla, S. et al. (2022), Dealing
/// with density discontinuities in planetary SPH simulations, *MNRAS*, doi:10.1093/mnras/stac857,
/// is not built.
fn fluid_densities(particles: &[FluidSample]) -> Vec<f64> {
    let points: Vec<DVec3> = particles.iter().map(|p| p.position).collect();
    let reach: Vec<f64> = particles.iter().map(|p| p.smoothing).collect();
    let mut density: Vec<f64> = particles
        .iter()
        .map(|p| p.mass * cubic_spline_kernel(0.0, p.smoothing))
        .collect();
    for (a, b) in neighbour_pairs(&points, &reach) {
        let (first, second) = (particles[a], particles[b]);
        let distance = (second.position - first.position).length();
        let weight = cubic_spline_kernel(distance, 0.5 * (first.smoothing + second.smoothing));
        density[a] += second.mass * weight * (first.reference_density / second.reference_density);
        density[b] += first.mass * weight * (second.reference_density / first.reference_density);
    }
    density
}

/// Measure the density of every fluid prop in `props` from its neighbours (as
/// [`apply_contact_forces`] does at the start of each substep), write it to
/// [`Prop::density_kg_m3`], and redraw each at the radius it gives. O(n) through a grid.
///
/// For a planet builder: a fluid particle's internal energy has to agree with the density the
/// simulation will measure for it, not only with the density it was meant to have, or its first
/// steps do work it never had the energy for (see [`crate::collision::aggregate`]).
pub fn measure_fluid_densities(props: &mut [Prop]) {
    let fluid: Vec<(usize, FluidSample)> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| prop.material.fluid)
        .filter_map(|(index, prop)| {
            let eos = prop.material.thermal?.parameters();
            let mass = f64::from(prop.mass_kg);
            let density = if prop.density_kg_m3 > 0.0 {
                prop.density_kg_m3
            } else {
                eos.reference_density_kg_m3
            };
            Some((
                index,
                FluidSample {
                    position: prop.body.center() * METRES_PER_UNIT,
                    mass,
                    smoothing: fluid_smoothing_m(mass, density, &eos),
                    reference_density: eos.reference_density_kg_m3,
                },
            ))
        })
        .collect();
    let described: Vec<FluidSample> = fluid.iter().map(|f| f.1).collect();
    for ((index, sample), density) in fluid.iter().zip(fluid_densities(&described)) {
        let prop = &mut props[*index];
        prop.density_kg_m3 = density;
        if let Body::Sphere { radius, .. } = &mut prop.body {
            *radius = (fluid_radius_m(sample.mass, density, sample.reference_density)
                / METRES_PER_UNIT) as f32;
        }
    }
}

/// Measure the density of every fluid particle from its neighbours ([`fluid_densities`]), and from
/// it its pressure and sound speed; write the density back to the prop (for the next substep's
/// smoothing length) and redraw the prop at the radius it gives.
fn measure_fluids(particles: &mut [Particle], props: &mut [Prop]) {
    let (fluid, described): (Vec<usize>, Vec<FluidSample>) = particles
        .iter()
        .enumerate()
        .filter_map(|(i, particle)| {
            let state = particle.fluid?;
            Some((
                i,
                FluidSample {
                    position: particle.position,
                    mass: particle.mass,
                    smoothing: state.smoothing,
                    reference_density: state.eos.reference_density_kg_m3,
                },
            ))
        })
        .unzip();
    if fluid.is_empty() {
        return;
    }
    let measured = fluid_densities(&described);
    let mut density = vec![0.0; particles.len()];
    for (&i, value) in fluid.iter().zip(measured) {
        density[i] = value;
    }
    for &i in &fluid {
        let particle = &mut particles[i];
        let Some(state) = particle.fluid.as_mut() else {
            continue;
        };
        state.density = density[i];
        state.pressure = tillotson_pressure(state.density, state.energy, &state.eos);
        state.sound = fluid_sound_speed(state.density, state.energy, &state.eos);
        particle.radius = fluid_radius_m(
            particle.mass,
            state.density,
            state.eos.reference_density_kg_m3,
        );
        let prop = &mut props[particle.index];
        prop.density_kg_m3 = state.density;
        if let Body::Sphere { radius, .. } = &mut prop.body {
            *radius = (particle.radius / METRES_PER_UNIT) as f32;
        }
    }
}

/// The pressure and artificial-viscosity forces between two fluid neighbours, or `None` when they
/// are not neighbours. With `h̄` the mean smoothing length and `∇W_a` the kernel's gradient at `a`:
///
/// ```text
/// F_a = −m_a·m_b·(P_a/ρ_a² + P_b/ρ_b² + Π_ab)·∇W_a,     F_b = −F_a
/// Π_ab = (−α·c̄·μ + β·μ²)/ρ̄,  μ = h̄·(v_ab·r_ab)/(r² + ε·h̄²)    when v_ab·r_ab < 0, else 0
/// ```
///
/// Monaghan, J. J. (1992), Smoothed particle hydrodynamics, *Annual Review of Astronomy and
/// Astrophysics* 30, 543–574, doi:10.1146/annurev.aa.30.090192.002551: the symmetric pressure force
/// (equal and opposite, along the line between the two, so momentum and angular momentum are
/// conserved exactly) and the viscosity, with [`SPH_VISCOSITY_ALPHA`], [`SPH_VISCOSITY_BETA`] and
/// [`SPH_VISCOSITY_EPSILON`].
fn fluid_pair_forces(a: &Particle, b: &Particle) -> Option<PairForces> {
    let (fluid_a, fluid_b) = (a.fluid?, b.fluid?);
    let offset = b.position - a.position;
    let reach = fluid_a.smoothing + fluid_b.smoothing;
    let distance_squared = offset.length_squared();
    if distance_squared >= reach * reach {
        return None;
    }
    let distance = distance_squared.sqrt();
    if distance.is_nan() || distance <= 0.0 {
        return None;
    }
    let smoothing = 0.5 * reach;
    let slope = cubic_spline_gradient(distance, smoothing);
    if slope >= 0.0 {
        return None;
    }
    let normal = offset / distance;
    // ∇W at a, from b: the slope along the unit vector from b to a.
    let gradient_at_a = -normal * slope;
    let masses = a.mass * b.mass;
    let own_a = masses * fluid_a.pressure / (fluid_a.density * fluid_a.density);
    let own_b = masses * fluid_b.pressure / (fluid_b.density * fluid_b.density);
    // `v_ab·r_ab`, negative when they approach.
    let approach = (a.velocity - b.velocity).dot(-offset);
    let viscous = if approach < 0.0 {
        let mu = smoothing * approach
            / (distance_squared + SPH_VISCOSITY_EPSILON * smoothing * smoothing);
        let sound = 0.5 * (fluid_a.sound + fluid_b.sound);
        let density = 0.5 * (fluid_a.density + fluid_b.density);
        masses * (-SPH_VISCOSITY_ALPHA * sound * mu + SPH_VISCOSITY_BETA * mu * mu) / density
    } else {
        0.0
    };
    let arm_a = distance * a.radius / (a.radius + b.radius);
    Some(PairForces {
        touching: distance < a.radius + b.radius,
        normal,
        arm_a,
        arm_b: distance - arm_a,
        // The force on a is −(…)·∇W_a, away from b when the bracket is positive; b takes the reverse.
        force_on_b: gradient_at_a * (own_a + own_b + viscous),
        torque_on_a: DVec3::ZERO,
        torque_on_b: DVec3::ZERO,
        dashpot: 0.0,
        friction_on_b: DVec3::ZERO,
        pressure: None,
        fluid: Some(FluidPair {
            own_a,
            own_b,
            viscous,
            gradient_at_a,
        }),
    })
}

/// The time step the fluid particles of `props` allow, in seconds, or `None` when there are none:
/// the Courant condition of Monaghan (1992),
///
/// ```text
/// dt = C · min_a h_a / (c_a·(1 + 1.2·α) + 1.2·β·max_b |μ_ab|)
/// ```
///
/// with [`SPH_COURANT`] for `C`, each particle's smoothing length and sound speed from its last
/// measured density, and `μ_ab` the artificial viscosity's `μ` (see `fluid_pair_forces`) for each
/// approaching neighbour `b`. `μ` peaks at about five times the approach speed for two particles a
/// tenth of a smoothing length apart, so it, not the approach speed, sets the step in a shock.
/// O(n), through the neighbour grid.
///
/// What [`crate::collision::resolve::substep_count`] divides each step by, beside its other
/// criteria.
pub fn fluid_courant_seconds(props: &[Prop]) -> Option<f64> {
    struct State {
        position: DVec3,
        velocity: DVec3,
        smoothing: f64,
        sound: f64,
    }
    let fluids: Vec<State> = props
        .iter()
        .filter(|prop| prop.material.fluid && prop.responds_to_forces())
        .filter_map(|prop| {
            let eos = prop.material.thermal?.parameters();
            let density = if prop.density_kg_m3 > 0.0 {
                prop.density_kg_m3
            } else {
                eos.reference_density_kg_m3
            };
            Some(State {
                position: prop.body.center() * METRES_PER_UNIT,
                velocity: prop.velocity_units_s * METRES_PER_UNIT,
                smoothing: fluid_smoothing_m(f64::from(prop.mass_kg), density, &eos),
                sound: fluid_sound_speed(density, prop.heat_j_kg, &eos),
            })
        })
        .collect();
    if fluids.is_empty() {
        return None;
    }
    let points: Vec<DVec3> = fluids.iter().map(|f| f.position).collect();
    let reach: Vec<f64> = fluids.iter().map(|f| f.smoothing).collect();
    let mut largest_mu = vec![0.0_f64; fluids.len()];
    for (a, b) in neighbour_pairs(&points, &reach) {
        let (first, second) = (&fluids[a], &fluids[b]);
        let offset = first.position - second.position;
        let approach = (first.velocity - second.velocity).dot(offset);
        if approach >= 0.0 {
            continue;
        }
        let smoothing = 0.5 * (first.smoothing + second.smoothing);
        let mu = (smoothing * approach
            / (offset.length_squared() + SPH_VISCOSITY_EPSILON * smoothing * smoothing))
            .abs();
        largest_mu[a] = largest_mu[a].max(mu);
        largest_mu[b] = largest_mu[b].max(mu);
    }
    fluids
        .iter()
        .zip(&largest_mu)
        .filter_map(|(fluid, mu)| {
            let signal =
                fluid.sound * (1.0 + 1.2 * SPH_VISCOSITY_ALPHA) + 1.2 * SPH_VISCOSITY_BETA * mu;
            let dt = SPH_COURANT * fluid.smoothing / signal;
            (dt.is_finite() && dt > 0.0).then_some(dt)
        })
        .reduce(f64::min)
}

/// What a pair of neighbours does to each other over a substep, and what the heat pass needs.
struct PairForces {
    touching: bool,
    /// The unit vector from `a` to `b`.
    normal: DVec3,
    /// The arms from each centre to the contact point; they add up to the distance.
    arm_a: f64,
    arm_b: f64,
    force_on_b: DVec3,
    torque_on_a: DVec3,
    torque_on_b: DVec3,
    /// `k·δ − Fn`: the part of the spring's force the contact did not deliver, which is what it
    /// dissipates per unit of separating speed.
    dashpot: f64,
    /// The friction force on `b`, zero when there is none.
    friction_on_b: DVec3,
    /// `m_a·m_b·(P_a/ρ_a² + P_b/ρ_b²)` and the kernel gradient at `a`, for two hot neighbours.
    pressure: Option<(f64, DVec3)>,
    /// The terms of two fluid neighbours' forces, for their energy equation.
    fluid: Option<FluidPair>,
}

/// What [`ContactPass::heat`] needs of two fluid neighbours: their forces' three terms and the
/// kernel gradient at `a`.
struct FluidPair {
    /// `m_a·m_b·P_a/ρ_a²`.
    own_a: f64,
    /// `m_a·m_b·P_b/ρ_b²`.
    own_b: f64,
    /// `m_a·m_b·Π_ab`, zero unless they approach.
    viscous: f64,
    gradient_at_a: DVec3,
}

/// The smoothing length of a particle of radius `radius_m` packed in a face-centred cubic lattice:
/// [`SPH_SMOOTHING_FACTOR`] times its spacing `(m/ρ)^(1/3)`, which for such a particle is
/// `r·(4π / (3φ))^(1/3)`, `1.78` radii.
pub fn smoothing_length_m(radius_m: f64) -> f64 {
    let spacing = radius_m * (4.0 * std::f64::consts::PI / (3.0 * FCC_PACKING_FRACTION)).cbrt();
    SPH_SMOOTHING_FACTOR * spacing
}

/// The contact, friction and pressure forces between two neighbouring particles, or `None` when
/// they are not neighbours.
fn pair_forces(
    a: &Particle,
    b: &Particle,
    (material_a, material_b): (&Material, &Material),
    dt: f64,
) -> Option<PairForces> {
    let offset = b.position - a.position;
    let distance_squared = offset.length_squared();
    let reach = a.reach() + b.reach();
    if distance_squared >= reach * reach {
        return None;
    }
    let distance = distance_squared.sqrt();
    if distance.is_nan() || distance <= 0.0 {
        // Two centres on top of each other have no direction to push along.
        return None;
    }
    let normal = offset / distance;
    let arm_a = distance * a.radius / (a.radius + b.radius);
    let mut pair = PairForces {
        touching: false,
        normal,
        arm_a,
        arm_b: distance - arm_a,
        force_on_b: DVec3::ZERO,
        torque_on_a: DVec3::ZERO,
        torque_on_b: DVec3::ZERO,
        dashpot: 0.0,
        friction_on_b: DVec3::ZERO,
        pressure: None,
        fluid: None,
    };

    let overlap = a.radius + b.radius - distance;
    let material = material_a.combined_with(material_b);
    if let (true, Some(modulus)) = (overlap > 0.0, material.modulus_pa) {
        pair.touching = true;
        let stiffness = contact_stiffness(f64::from(modulus), a.radius, b.radius);
        let reduced_mass = a.mass * b.mass / (a.mass + b.mass);
        // Capped: a restitution of `0` would ask for infinite damping.
        let zeta = damping_ratio_for_restitution(f64::from(material.restitution))
            .min(SOFT_CONTACT_MAX_DAMPING_RATIO);
        let damping = 2.0 * zeta * (reduced_mass * stiffness).sqrt();

        let normal_speed = (b.velocity - a.velocity).dot(normal);
        let push = spring_dashpot_force(stiffness, damping, overlap, normal_speed);
        pair.force_on_b = normal * push;
        pair.dashpot = stiffness * overlap - push;

        // Friction against the slip of the two surfaces where they touch.
        let surface_a = a.velocity + a.spin.cross(normal * pair.arm_a);
        let surface_b = b.velocity + b.spin.cross(-normal * pair.arm_b);
        let slip = surface_b - surface_a;
        let sliding = slip - normal * slip.dot(normal);
        let sliding_speed = sliding.length();
        if material.friction > 0.0 && sliding_speed > 0.0 {
            // The slip's inertia: both particles' masses and, through the arms, their moments of
            // inertia. The viscous drag that regularises Coulomb's law near zero slip undoes at most
            // a twelfth of the slip per step, so all of a particle's contacts together cannot
            // overshoot (see `SOFT_CONTACT_MAX_NEIGHBOURS`).
            let slip_mass = 1.0
                / (1.0 / a.mass
                    + 1.0 / b.mass
                    + pair.arm_a * pair.arm_a / a.inertia
                    + pair.arm_b * pair.arm_b / b.inertia);
            let drag = (f64::from(material.friction) * push)
                .min(slip_mass * sliding_speed / (dt * SOFT_CONTACT_MAX_NEIGHBOURS));
            // `sliding` is how `b`'s surface slides past `a`'s; friction on `b` is against it.
            let friction_on_b = -sliding / sliding_speed * drag;
            pair.friction_on_b = friction_on_b;
            pair.force_on_b += friction_on_b;
            pair.torque_on_a = (normal * pair.arm_a).cross(-friction_on_b);
            pair.torque_on_b = (-normal * pair.arm_b).cross(friction_on_b);
        }
    }

    if let (Some(hot_a), Some(hot_b)) = (a.thermal, b.thermal) {
        let smoothing = 0.5 * (hot_a.smoothing + hot_b.smoothing);
        let slope = cubic_spline_gradient(distance, smoothing);
        let both = hot_a.pressure / (hot_a.density * hot_a.density)
            + hot_b.pressure / (hot_b.density * hot_b.density);
        if slope < 0.0 && both > 0.0 {
            // ∇W at a, from b: the slope along the unit vector from b to a.
            let gradient_at_a = -normal * slope;
            let coefficient = a.mass * b.mass * both;
            // The force on a is −m_a·m_b·(…)·∇W, away from b; b takes the reverse.
            pair.force_on_b += gradient_at_a * coefficient;
            pair.pressure = Some((coefficient, gradient_at_a));
        }
    }

    (pair.touching || pair.pressure.is_some()).then_some(pair)
}

/// The energy of the simulated particles in `props`, in joules, by kind. See
/// [`apply_contact_forces`]: their sum is what the contacts conserve.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ParticleEnergy {
    /// `½·m·v²`, summed.
    pub kinetic: f64,
    /// `½·I·ω²`, summed.
    pub spin: f64,
    /// The gravitational potential of every pair, for the integrator's softened force.
    pub gravity: f64,
    /// `½·k·δ²` of every overlapping pair: what the contact springs hold.
    pub spring: f64,
    /// `m·u`, summed: the heat.
    pub heat: f64,
}

impl ParticleEnergy {
    /// Everything together.
    pub fn total(&self) -> f64 {
        self.kinetic + self.spin + self.gravity + self.spring + self.heat
    }
}

/// The energy ledger of the simulated particles in `props`, with the scene's softening length
/// `softening_units`. O(n²).
///
/// The gravitational term is the potential of the integrator's softened force, whose magnitude is
/// `G·m₁·m₂ / (r² + ε²)`: `U = −(G·m₁·m₂/ε)·(π/2 − atan(r/ε))`, which is `−G·m₁·m₂/r` for `ε = 0`.
pub fn particle_energy(props: &[Prop], softening_units: f32) -> ParticleEnergy {
    let particles: Vec<Particle> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| contact_is_simulated(prop, prop))
        .filter_map(|(index, prop)| Particle::of(index, prop))
        .collect();
    let softening = f64::from(softening_units.max(0.0)) * METRES_PER_UNIT;
    let mut energy = ParticleEnergy::default();
    for (a, first) in particles.iter().enumerate() {
        energy.kinetic += 0.5 * first.mass * first.velocity.length_squared();
        energy.spin += 0.5 * first.inertia * first.spin.length_squared();
        energy.heat += first.mass * props[first.index].heat_j_kg;
        for second in &particles[a + 1..] {
            let distance = (second.position - first.position).length();
            let product = GRAVITATIONAL_CONSTANT * first.mass * second.mass;
            energy.gravity -= if softening > 0.0 {
                product / softening * (std::f64::consts::FRAC_PI_2 - (distance / softening).atan())
            } else {
                product / distance
            };
            let overlap = first.radius + second.radius - distance;
            let material = props[first.index]
                .material
                .combined_with(&props[second.index].material);
            if let (true, Some(modulus)) = (overlap > 0.0, material.modulus_pa) {
                let stiffness = contact_stiffness(f64::from(modulus), first.radius, second.radius);
                energy.spring += 0.5 * stiffness * overlap * overlap;
            }
        }
    }
    energy
}

/// The shortest simulated contact among `props`, in seconds: the time a contact between two
/// copies of the stiffest, lightest particle lasts. `None` when nothing's contact is simulated.
///
/// What [`crate::collision::resolve::substep_count`] divides by
/// [`SOFT_CONTACT_SUBSTEPS`](crate::collision::constants::SOFT_CONTACT_SUBSTEPS) to keep every
/// contact resolved. A pair of different particles lasts at least as long as the shorter of their
/// two self-pairs, so the minimum over self-pairs bounds every pair from below.
pub fn shortest_contact_seconds(props: &[Prop]) -> Option<f64> {
    props
        .iter()
        .filter(|prop| contact_is_simulated(prop, prop))
        .filter_map(|prop| {
            let radius = physical_radius_m(prop)?;
            let modulus = prop.material.combined_with(&prop.material).modulus_pa?;
            let stiffness = contact_stiffness(f64::from(modulus), radius, radius);
            let duration = contact_duration(0.5 * f64::from(prop.mass_kg), stiffness);
            duration.is_finite().then_some(duration)
        })
        .reduce(f64::min)
}

/// What one merge did: which bodies were involved and how much debris it made.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionEvent {
    /// The body that survives, holding the merged mass.
    pub survivor: BodyId,
    /// The body that was merged into it and has left the scene.
    pub merged: BodyId,
    /// The relative speed they met at, m/s.
    pub impact_speed_m_s: f64,
    /// The share of the pair's mass thrown off as debris, `0` for a perfect merge.
    pub debris_fraction: f64,
    /// The debris bodies created, in order.
    pub fragments: Vec<BodyId>,
}

/// Merge every touching pair of gravity-dominated bodies that cannot escape each other, and spray
/// the debris their impact throws off. Returns what happened, one event per merge.
///
/// The merging half of [`Scene::step`](crate::core::scene::Scene::step), run after absorption and
/// before the bounce, so a pair that merges is never bounced. Takes the whole scene because it
/// removes and adds bodies.
///
/// # Order
///
/// Pairs are visited in `(i, j)` order and each merge is applied at once, so a body that merges
/// twice in one substep grows before the second. A body already merged away in this pass is
/// skipped. Removal and the new debris are applied after the scan, so indices stay stable, and the
/// debris cannot merge in the substep it is made.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{merge_contacts, Aabb, Body, Color, DVec3, Material, Pose, Prop, Quat, Scene, Vec3};
///
/// // Two Moon-sized bodies just touching, closing at 100 m/s, made of rock that does not bounce.
/// let moon = |x: f64, vx: f64| {
///     Prop::new(
///         Body::Sphere {
///             pose: Pose { position: DVec3::new(x, 0.0, 0.0), rotation: Quat::IDENTITY },
///             radius: 0.0017,
///             color: Color { r: 0.6, g: 0.6, b: 0.6, a: 1.0 },
///         },
///         DVec3::new(vx, 0.0, 0.0),
///         7.3e22,
///     )
///     .with_material(Material::new(0.0, 0.0))
/// };
/// let mut scene = Scene::empty(Aabb { min: Vec3::splat(-1.0), max: Vec3::splat(1.0) });
/// scene.add(moon(-0.0017, 5.0e-8));
/// scene.add(moon(0.0017, -5.0e-8));
///
/// let events = merge_contacts(&mut scene);
/// assert_eq!(events.len(), 1);
/// assert_eq!(scene.props.len(), 1, "one body, and too gentle a meeting for debris");
/// assert_eq!(scene.props[0].mass_kg, 1.46e23);
/// assert_eq!(scene.props[0].velocity_units_s, DVec3::ZERO);
/// ```
pub fn merge_contacts(scene: &mut Scene) -> Vec<CollisionEvent> {
    let candidates: Vec<usize> = (0..scene.props.len())
        .filter(|&index| gravity_dominated(&scene.props[index]))
        .collect();
    if candidates.len() < 2 {
        return Vec::new();
    }

    let ids: Vec<BodyId> = scene.iter().map(|(id, _)| id).collect();
    let fragments_per_impact = scene.collision.debris_fragments.min(MAX_DEBRIS_FRAGMENTS);
    let mut merged_away: Vec<BodyId> = Vec::new();
    let mut new_debris: Vec<(usize, Vec<Prop>)> = Vec::new();
    let mut events = Vec::new();

    for (position, &i) in candidates.iter().enumerate() {
        for &j in &candidates[position + 1..] {
            if merged_away.contains(&ids[i]) || merged_away.contains(&ids[j]) {
                continue;
            }
            let Some(impact) = bound_impact(&scene.props[i], &scene.props[j]) else {
                continue;
            };

            // The more massive body survives; ties go to the lower index.
            let (survivor, victim) = if scene.props[j].mass_kg > scene.props[i].mass_kg {
                (j, i)
            } else {
                (i, j)
            };

            let outcome = merge_pair(scene, survivor, victim, impact, fragments_per_impact);
            merged_away.push(ids[victim]);
            events.push(CollisionEvent {
                survivor: ids[survivor],
                merged: ids[victim],
                impact_speed_m_s: impact.speed_m_s,
                debris_fraction: outcome.debris_fraction,
                fragments: Vec::new(),
            });
            new_debris.push((events.len() - 1, outcome.fragments));
        }
    }

    for id in &merged_away {
        scene.remove(*id);
    }
    for (event, fragments) in new_debris {
        for fragment in fragments {
            let id = scene.add(fragment);
            events[event].fragments.push(id);
        }
    }

    events
}

/// Is this body held together by its own gravity, and so able to merge?
///
/// A movable sphere with mass, not an absorber (absorption is its own pass), whose `G·M/R` at its
/// physical radius is at least [`GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG`].
pub fn gravity_dominated(prop: &Prop) -> bool {
    if !prop.responds_to_forces() || prop.absorbs {
        return false;
    }
    let Some(radius_m) = physical_radius_m(prop) else {
        return false;
    };
    GRAVITATIONAL_CONSTANT * f64::from(prop.mass_kg) / radius_m
        >= GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG
}

/// The radius a sphere collides at, in metres: its contact radius when it has one, otherwise its
/// drawn radius. `None` for a body that is not a sphere or has no positive radius.
pub(crate) fn physical_radius_m(prop: &Prop) -> Option<f64> {
    let Body::Sphere { radius, .. } = &prop.body else {
        return None;
    };
    let radius_m = f64::from(prop.contact_radius_units.unwrap_or(*radius)) * METRES_PER_UNIT;
    (radius_m.is_finite() && radius_m > 0.0).then_some(radius_m)
}

/// The facts of an impact that ends in a merge.
#[derive(Copy, Clone, Debug)]
struct Impact {
    /// The relative speed at contact, m/s.
    speed_m_s: f64,
    /// Both physical radii, m, in argument order.
    radii_m: (f64, f64),
    /// The pair's mutual escape speed, m/s.
    escape_m_s: f64,
}

/// The impact between `a` and `b`, if they touch, are closing, and cannot escape each other after
/// it. See the module docs for the four tests.
fn bound_impact(a: &Prop, b: &Prop) -> Option<Impact> {
    // A simulated contact is left to the contact force, which merges the pair by itself.
    if contact_is_simulated(a, b) {
        return None;
    }
    let (radius_a, radius_b) = (physical_radius_m(a)?, physical_radius_m(b)?);

    let offset = (b.body.center() - a.body.center()) * METRES_PER_UNIT;
    let distance = offset.length();
    if !(distance.is_finite() && distance > 0.0) || distance > radius_a + radius_b {
        return None;
    }
    let normal = offset / distance;

    let relative = (b.velocity_units_s - a.velocity_units_s) * METRES_PER_UNIT;
    let approach = relative.dot(normal);
    if approach.is_nan() || approach >= 0.0 {
        return None;
    }

    let material = a.material.combined_with(&b.material);
    let restitution = f64::from(material.restitution);
    if restitution >= 1.0 {
        return None;
    }

    // The bounce the resolver would give: the normal speed returned at `e`, the sliding slowed by
    // friction. What is left is the speed the pair would part at.
    let sliding = (relative - approach * normal).length();
    let slowed = coulomb_friction_speed_change(
        sliding,
        (1.0 + restitution) * approach.abs(),
        f64::from(material.friction),
    );
    let parting_speed = (restitution * approach).hypot(sliding - slowed);

    let escape = mutual_escape_speed(
        f64::from(a.mass_kg),
        f64::from(b.mass_kg),
        radius_a,
        radius_b,
    );
    (parting_speed < escape).then_some(Impact {
        speed_m_s: relative.length(),
        radii_m: (radius_a, radius_b),
        escape_m_s: escape,
    })
}

/// What [`merge_pair`] produced.
struct MergeOutcome {
    debris_fraction: f64,
    fragments: Vec<Prop>,
}

/// Fold `victim` into `survivor`, and return the debris to add once the scan is over. The victim
/// is left in place for [`merge_contacts`] to remove.
fn merge_pair(
    scene: &mut Scene,
    survivor: usize,
    victim: usize,
    impact: Impact,
    fragments_per_impact: u32,
) -> MergeOutcome {
    let (a, b) = (&scene.props[survivor], &scene.props[victim]);
    let (mass_a, mass_b) = (f64::from(a.mass_kg), f64::from(b.mass_kg));
    let total_mass = mass_a + mass_b;
    let (radius_a_m, radius_b_m) = if survivor < victim {
        impact.radii_m
    } else {
        (impact.radii_m.1, impact.radii_m.0)
    };

    // Centre of mass and its velocity, in scene units: what the merge must conserve.
    let barycentre = (a.body.center() * mass_a + b.body.center() * mass_b) / total_mass;
    let centre_velocity = (a.velocity_units_s * mass_a + b.velocity_units_s * mass_b) / total_mass;

    // How much survives. Below the mutual escape speed Leinhardt & Stewart (2012) find a perfect
    // merge: the scaling law below is for impacts above it, and applying it lower throws off
    // debris the simulations do not.
    let impact_energy = specific_impact_energy(mass_a, mass_b, impact.speed_m_s);
    let threshold = catastrophic_disruption_threshold(mass_a, mass_b);
    let mut remnant_fraction = largest_remnant_fraction(impact_energy, threshold);
    if impact.speed_m_s <= impact.escape_m_s
        || 1.0 - remnant_fraction < DEBRIS_MIN_MASS_FRACTION
        || fragments_per_impact == 0
    {
        remnant_fraction = 1.0;
    }

    let combined_volume_radius_m = (radius_a_m.powi(3) + radius_b_m.powi(3)).cbrt();
    let remnant_mass = total_mass * remnant_fraction;
    let remnant_radius_m = combined_volume_radius_m * remnant_fraction.cbrt();

    let colour = blend(body_colour(&a.body), body_colour(&b.body));
    let material = a.material;

    let (fragments, remnant_offset_units, remnant_velocity_units_s) = if remnant_fraction < 1.0 {
        debris(DebrisSpec {
            count: fragments_per_impact,
            debris_mass_kg: total_mass - remnant_mass,
            remnant_mass_kg: remnant_mass,
            remnant_radius_m,
            density_kg_m3: total_mass
                / (4.0 / 3.0 * std::f64::consts::PI * combined_volume_radius_m.powi(3)),
            available_energy_j: 0.5 * (mass_a * mass_b / total_mass) * impact.speed_m_s.powi(2),
            barycentre_units: barycentre,
            centre_velocity_units_s: centre_velocity,
            colour,
            material,
        })
    } else {
        (Vec::new(), DVec3::ZERO, centre_velocity)
    };

    let prop = &mut scene.props[survivor];
    let old_radius_m = radius_a_m;
    let growth = (remnant_radius_m / old_radius_m) as f32;
    prop.mass_kg = remnant_mass as f32;
    prop.velocity_units_s = remnant_velocity_units_s;
    let target = barycentre + remnant_offset_units;
    prop.body.translate(target - prop.body.center());
    if let Body::Sphere { radius, .. } = &mut prop.body {
        *radius *= growth;
    }
    if let Some(contact) = prop.contact_radius_units.as_mut() {
        *contact = (remnant_radius_m / METRES_PER_UNIT) as f32;
    }

    debug_assert!(
        prop.is_valid(),
        "merge_pair: the merged body left its Validation_Rules: {prop:?}"
    );

    MergeOutcome {
        debris_fraction: 1.0 - remnant_fraction,
        fragments,
    }
}

/// Everything [`debris`] needs.
struct DebrisSpec {
    count: u32,
    debris_mass_kg: f64,
    remnant_mass_kg: f64,
    remnant_radius_m: f64,
    density_kg_m3: f64,
    /// The impact's kinetic energy in the centre-of-mass frame, J: the most the debris and the
    /// remnant's recoil may carry.
    available_energy_j: f64,
    barycentre_units: DVec3,
    centre_velocity_units_s: DVec3,
    colour: Color,
    material: crate::core::scene::Material,
}

/// The debris fragments, the offset of the remnant from the barycentre, and the remnant's
/// velocity, chosen so that mass, momentum and the centre of mass are all exactly conserved.
fn debris(spec: DebrisSpec) -> (Vec<Prop>, DVec3, DVec3) {
    let count = spec.count.max(1) as usize;
    let fragment_mass = spec.debris_mass_kg / count as f64;
    let fragment_radius_m =
        (3.0 * fragment_mass / (4.0 * std::f64::consts::PI * spec.density_kg_m3)).cbrt();

    let directions: Vec<DVec3> = (0..count).map(|k| fibonacci_direction(k, count)).collect();
    let direction_sum: DVec3 = directions.iter().copied().sum();

    // The remnant moves opposite the debris' own centre of mass so the total stays put; its offset
    // is `w·|Σ dir|` of the shell radius. The shell is made big enough that the fragments clear the
    // remnant after that shift, and clear each other: neighbours on an even shell of `n` points are
    // about `R·√(4π/n)` apart.
    let weight = fragment_mass / spec.remnant_mass_kg;
    let shift = (weight * direction_sum.length()).min(0.75);
    let clear = 1.0 + DEBRIS_SPAWN_CLEARANCE;
    let around_remnant = (spec.remnant_radius_m + fragment_radius_m) * clear / (1.0 - shift);
    let between_fragments =
        2.0 * fragment_radius_m * clear / (4.0 * std::f64::consts::PI / count as f64).sqrt();
    let shell_radius_m = around_remnant.max(between_fragments);

    // Speeds spread between the two factors of the escape speed from the shell. The golden-ratio
    // sequence spreads them evenly without tying a speed to a direction.
    let escape_m_s = (2.0 * GRAVITATIONAL_CONSTANT * spec.remnant_mass_kg / shell_radius_m).sqrt();
    let golden = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut speeds_m_s: Vec<f64> = (0..count)
        .map(|k| {
            let spread = (k as f64 * golden).fract();
            escape_m_s
                * (DEBRIS_MIN_SPEED_FACTOR
                    + (DEBRIS_MAX_SPEED_FACTOR - DEBRIS_MIN_SPEED_FACTOR) * spread)
        })
        .collect();

    // Momentum in the centre-of-mass frame is zero, so the remnant recoils against the debris.
    // Cap the kinetic energy of both at what the impact brought, scaling every speed alike.
    let recoil = |speeds: &[f64]| -> DVec3 {
        -directions
            .iter()
            .zip(speeds)
            .map(|(direction, speed)| *direction * (fragment_mass * speed))
            .sum::<DVec3>()
            / spec.remnant_mass_kg
    };
    let energy = |speeds: &[f64]| -> f64 {
        0.5 * fragment_mass * speeds.iter().map(|s| s * s).sum::<f64>()
            + 0.5 * spec.remnant_mass_kg * recoil(speeds).length_squared()
    };
    let carried = energy(&speeds_m_s);
    if carried > spec.available_energy_j && carried > 0.0 {
        let scale = (spec.available_energy_j / carried).sqrt();
        for speed in &mut speeds_m_s {
            *speed *= scale;
        }
    }
    let remnant_recoil_units_s = recoil(&speeds_m_s) / METRES_PER_UNIT;

    let shell_units = shell_radius_m / METRES_PER_UNIT;
    let remnant_offset_units = -direction_sum * (weight * shell_units);
    let fragment_radius_units = (fragment_radius_m / METRES_PER_UNIT) as f32;

    let fragments = directions
        .iter()
        .zip(&speeds_m_s)
        .map(|(direction, speed)| {
            Prop::new(
                Body::Sphere {
                    pose: Pose {
                        position: spec.barycentre_units + *direction * shell_units,
                        rotation: Quat::IDENTITY,
                    },
                    radius: fragment_radius_units,
                    color: spec.colour,
                },
                spec.centre_velocity_units_s + *direction * (speed / METRES_PER_UNIT),
                fragment_mass as f32,
            )
            .with_material(spec.material)
        })
        .collect();

    (
        fragments,
        remnant_offset_units,
        spec.centre_velocity_units_s + remnant_recoil_units_s,
    )
}

/// The `k`-th of `n` directions spread evenly over the sphere: the Fibonacci lattice. Unit length.
fn fibonacci_direction(k: usize, n: usize) -> DVec3 {
    let golden_angle = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let y = 1.0 - 2.0 * (k as f64 + 0.5) / n as f64;
    let ring = (1.0 - y * y).max(0.0).sqrt();
    let angle = golden_angle * k as f64;
    DVec3::new(ring * angle.cos(), y, ring * angle.sin())
}

/// The colour a body is drawn in.
fn body_colour(body: &Body) -> Color {
    match body {
        Body::Sphere { color, .. }
        | Body::Cuboid { color, .. }
        | Body::Plane { color, .. }
        | Body::Line { color, .. } => *color,
    }
}

/// The average of two colours: debris is a mix of both bodies.
fn blend(a: Color, b: Color) -> Color {
    Color {
        r: 0.5 * (a.r + b.r),
        g: 0.5 * (a.g + b.g),
        b: 0.5 * (a.b + b.b),
        a: 0.5 * (a.a + b.a),
    }
}

#[cfg(test)]
#[path = "../../tst/collision/collision_tests.rs"]
mod tests;
