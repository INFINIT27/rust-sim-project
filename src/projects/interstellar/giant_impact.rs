//! `experiment_1` — a giant impact: the Earth and a Mars-mass body, at true size, each built from
//! particles, falling together from rest until they collide, deform and merge.
//!
//! It replaced the two-body free fall, which dropped two Earth masses drawn `235×` too large onto
//! each other and bounced them off perfectly elastically, forever. Both bodies here are their real
//! size and mass, and each is a pile of particles held together by its own gravity
//! ([`crate::collision::aggregate`]), made of [`PLANETARY_PARTICLE_MATERIAL`]. Nothing decides
//! what happens when they meet: the particles at the contact push on each other through a damped
//! spring ([`crate::collision::collision::apply_contact_forces`]), the impactor flattens and flows
//! into the Earth, the energy goes into the damping, and the two settle into one body. Particles
//! thrown fast enough escape as debris.
//!
//! # The numbers
//!
//! | | |
//! | --- | --- |
//! | The planets | The Earth (`5.97e24 kg`, `6,371 km`) and a body with the mass and radius of Mars (`6.42e23 kg`, `3,390 km`), the size usually given for Theia, the body whose impact is thought to have made the Moon |
//! | The particles | At the default detail, `201` for the Earth and `19` for the impactor, all about `1,700 km` across. The menu's Particles per planet sets the Earth's count; the impactor's follows from its volume, at the same particle size |
//! | The start | At rest, [`GIANT_IMPACT_START_CONTACTS`] `= 10` contact distances apart (`97,600 km`), about their centre of mass at the origin |
//! | The impact | Head-on after about `14` hours, at about `9 km/s`, `95%` of their mutual escape speed |
//! | The outcome | One body, holding every particle, at rest by `24` hours: `17%` of the system's binding energy goes into the contacts' damping. Below the mutual escape speed Leinhardt & Stewart (2012) find a perfect merge, and the run agrees; nothing in it is told to merge |
//!
//! # What it is not
//!
//! The Moon-forming impact itself. That was oblique (about `45°`), and the debris it threw into
//! orbit is what became the Moon. A head-on fall from rest has no angular momentum to put anything
//! in orbit. The particles are of one density, so neither planet has a dense core. See the design
//! in `src/collision/README.md`.

pub use bevy::math::{Quat, Vec3};

use crate::collision::aggregate::{lattice_spacing_m, particle_body, ParticleBody};
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Aabb, CollisionSettings, DVec3, Scene};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::projects::interstellar::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::projects::interstellar::constants::{
    DEFAULT_RANDOM_SEED, GIANT_IMPACT_BOUNDS_MARGIN_UNITS, GIANT_IMPACT_CAMERA_POSITION_UNITS,
    GIANT_IMPACT_FRAMED_RADIUS_UNITS, GIANT_IMPACT_SOFTENING_UNITS, GIANT_IMPACT_START_CONTACTS,
    GIANT_IMPACT_TIME_SCALE, GIANT_IMPACT_TRAIL_LENGTH_UNITS, PLANETARY_PARTICLE_MATERIAL,
    SOLAR_SYSTEM_PLANETS,
};
use crate::projects::interstellar::solar_system::Planet;

/// The struck planet: the Earth row of [`SOLAR_SYSTEM_PLANETS`], so every experiment agrees on it.
pub fn giant_impact_target() -> Planet {
    planet_named("Earth")
}

/// The impactor: a body with the Mars row's mass, radius and colour.
pub fn giant_impact_impactor() -> Planet {
    planet_named("Mars")
}

fn planet_named(name: &str) -> Planet {
    SOLAR_SYSTEM_PLANETS
        .iter()
        .find(|planet| planet.name == name)
        .copied()
        .unwrap_or(SOLAR_SYSTEM_PLANETS[2])
}

/// The distance at which the two planets touch, in metres: the sum of their radii.
pub fn giant_impact_contact_m() -> f64 {
    giant_impact_target().radius_m + giant_impact_impactor().radius_m
}

/// How long after release the two planets touch, in seconds.
///
/// The radial free fall of two bodies from rest at separation `d` to `r`:
///
/// ```text
/// t = √(d³ / 2GM) · ( √(x(1 − x)) + arccos √x ),    x = r/d
/// ```
///
/// which is the Kepler time along a degenerate (straight-line) ellipse.
///
/// # Examples
///
/// ```
/// use physics_sandbox::giant_impact_seconds;
///
/// // Fourteen hours.
/// assert!((giant_impact_seconds() / 3600.0 - 14.0).abs() < 0.1);
/// ```
pub fn giant_impact_seconds() -> f64 {
    let total_mass = giant_impact_target().mass_kg + giant_impact_impactor().mass_kg;
    let start = GIANT_IMPACT_START_CONTACTS * giant_impact_contact_m();
    let x = 1.0 / GIANT_IMPACT_START_CONTACTS;
    (start.powi(3) / (2.0 * GRAVITATIONAL_CONSTANT * total_mass)).sqrt()
        * ((x * (1.0 - x)).sqrt() + x.sqrt().acos())
}

/// The relative speed the two planets meet at, in m/s: energy conservation from rest.
pub fn giant_impact_speed_m_s() -> f64 {
    let total_mass = giant_impact_target().mass_kg + giant_impact_impactor().mass_kg;
    let contact = giant_impact_contact_m();
    let start = GIANT_IMPACT_START_CONTACTS * contact;
    (2.0 * GRAVITATIONAL_CONSTANT * total_mass * (1.0 / contact - 1.0 / start)).sqrt()
}

/// Build `experiment_1` at the default detail. See [`giant_impact_with`].
pub fn giant_impact() -> Experiment {
    giant_impact_with(&CollisionSettings::default())
}

/// Build `experiment_1`: the Earth and a Mars-mass impactor, each a pile of particles, at rest and
/// about to fall together. The Earth is made of `settings.particles_per_body` particles (as near as
/// the packing allows), and the impactor of as many of the same size as fill its volume.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{giant_impact_with, CollisionSettings};
///
/// let coarse = CollisionSettings { particles_per_body: 50, ..CollisionSettings::default() };
/// let experiment = giant_impact_with(&coarse);
/// let count = experiment.scene.props.len();
/// assert!((50..70).contains(&count), "about 50 for the Earth and a few for the impactor: {count}");
/// assert!(experiment.scene.props.iter().all(|p| p.material.modulus_pa.is_some()));
/// ```
pub fn giant_impact_with(settings: &CollisionSettings) -> Experiment {
    let target = giant_impact_target();
    let impactor = giant_impact_impactor();
    let total_mass = target.mass_kg + impactor.mass_kg;

    // About their centre of mass at the origin, along X.
    let separation_units = GIANT_IMPACT_START_CONTACTS * giant_impact_contact_m() / METRES_PER_UNIT;
    let target_centre = DVec3::new(-separation_units * impactor.mass_kg / total_mass, 0.0, 0.0);
    let impactor_centre = DVec3::new(separation_units * target.mass_kg / total_mass, 0.0, 0.0);

    let extent = separation_units as f32 + GIANT_IMPACT_BOUNDS_MARGIN_UNITS;
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-extent),
        max: Vec3::splat(extent),
    });
    scene.softening_units = GIANT_IMPACT_SOFTENING_UNITS;
    scene.collision = *settings;

    // One particle size for both planets, set by the Earth's count.
    let spacing = lattice_spacing_m(target.radius_m, settings.particles_per_body);
    for (planet, centre) in [(&target, target_centre), (&impactor, impactor_centre)] {
        let body = ParticleBody {
            centre_units: centre,
            velocity_units_s: DVec3::ZERO,
            mass_kg: planet.mass_kg,
            radius_m: planet.radius_m,
            color: planet.color,
            material: PLANETARY_PARTICLE_MATERIAL,
        };
        for particle in particle_body(&body, spacing) {
            scene.add(particle);
        }
    }

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::GiantImpact,
            // Deterministic: released from rest, nothing to randomise.
            seed: DEFAULT_RANDOM_SEED,
            time_scale: GIANT_IMPACT_TIME_SCALE,
            camera_position_units: GIANT_IMPACT_CAMERA_POSITION_UNITS,
            camera_target_units: Vec3::ZERO,
            framed_radius_units: GIANT_IMPACT_FRAMED_RADIUS_UNITS,
            trail_length_units: Some(GIANT_IMPACT_TRAIL_LENGTH_UNITS),
            // Hundreds of particles, none of them a place to fly a spacecraft to.
            launches: false,
            general_lighting: true,
        },
        // Hundreds of particles, none named.
        names: BodyNames::default(),
    }
}

/// `experiment_1`'s planets fired at each other head-on, to meet at `impact_speed_m_s` rather than
/// fall together from rest: the same start, given the inward speeds that energy conservation says
/// bring them together at that speed. For checking the particle model against the debris
/// Leinhardt & Stewart's law gives above the mutual escape speed. Below the speed the fall from rest
/// already reaches ([`giant_impact_speed_m_s`]), they start at rest as in `experiment_1`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{giant_impact_at_speed, giant_impact_speed_m_s, CollisionSettings};
///
/// let coarse = CollisionSettings { particles_per_body: 50, ..CollisionSettings::default() };
/// let fast = giant_impact_at_speed(&coarse, 2.0 * giant_impact_speed_m_s()).scene;
/// // The Earth (at negative X) moves toward the impactor, and the impactor toward it.
/// assert!(fast.props[0].velocity_units_s.x > 0.0);
/// assert!(fast.props.last().unwrap().velocity_units_s.x < 0.0);
/// ```
pub fn giant_impact_at_speed(settings: &CollisionSettings, impact_speed_m_s: f64) -> Experiment {
    let mut experiment = giant_impact_with(settings);
    let (target, impactor) = (giant_impact_target(), giant_impact_impactor());
    let total = target.mass_kg + impactor.mass_kg;
    let mu = GRAVITATIONAL_CONSTANT * total;
    let contact = giant_impact_contact_m();
    let start = GIANT_IMPACT_START_CONTACTS * contact;
    let at_start = (impact_speed_m_s.powi(2) - 2.0 * mu * (1.0 / contact - 1.0 / start))
        .max(0.0)
        .sqrt();
    for prop in &mut experiment.scene.props {
        let earth = prop.body.center().x < 0.0;
        let (share, sign) = if earth {
            (impactor.mass_kg / total, 1.0)
        } else {
            (target.mass_kg / total, -1.0)
        };
        prop.velocity_units_s.x = sign * at_start * share / METRES_PER_UNIT;
    }
    experiment
}

#[cfg(test)]
#[path = "../../../tst/projects/interstellar/giant_impact_tests.rs"]
mod tests;
