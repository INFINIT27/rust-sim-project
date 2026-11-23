//! `experiment_8` — the Moon-forming impact: the Earth and a Mars-mass body, each built from
//! particles, meeting at their mutual escape speed and at `45°`, the canonical giant impact thought
//! to have made the Moon (Canup & Asphaug 2001, Nature 412:708; Canup 2004, Icarus 168:433).
//!
//! The same planets as `experiment_1` ([`super::giant_impact`]), made of the same materials, but
//! struck off-centre. That gives the pair angular momentum, `1.1` times the Earth and Moon's today,
//! and the impact has to put it somewhere: into the merged body's spin, and into whatever material
//! is thrown onto orbits that clear the surface, the disk a moon could form from. Rubble particles
//! spin too: friction at their contacts turns sliding into rotation, so that angular momentum is
//! conserved exactly, orbit and spin together. Fluid particles have no friction and push only along
//! the line between them, which conserves it without spin.
//!
//! # The numbers
//!
//! | | |
//! | --- | --- |
//! | The planets | As `experiment_1`: the Earth and a body with the mass and radius of Mars |
//! | The approach | A parabolic orbit (zero energy: the impactor would just escape), from [`MOON_FORMING_START_CONTACTS`] `= 4` contact distances apart, about their centre of mass |
//! | The impact | After about `1.7` hours, at the mutual escape speed, `9.25 km/s`, at [`MOON_FORMING_IMPACT_ANGLE_DEG`] `= 45°` |
//! | Angular momentum | `3.8e34 kg·m²/s`, `1.1` times the Earth–Moon system's |
//!
//! [`moon_forming_impact_seconds`] is the time to contact by Barker's equation for a parabolic
//! orbit, which the tests hold the run to.
//!
//! # What to expect at this resolution
//!
//! Published simulations of this impact use `10⁵` to `10⁶` particles, iron cores and a full
//! equation of state, and put one to two lunar masses into the disk. At the default detail the
//! impactor is `19` particles, each a fifth of the Moon's mass, so a disk is a handful of particles
//! at best; what the run shows reliably is the merged body spinning with the impact's angular
//! momentum. Raise Particles per planet (with the GPU) for a disk to resolve.
//!
//! # Fluid or rubble
//!
//! The planets are built as the configured planet model says (`giant_impact::add_planets`).
//! Fluid planets (the default) flow as planets do, so the Earth's tide, `78%` of the impactor's own
//! surface gravity at contact, stretches the impactor as it closes: by `30%`, measured at both
//! `2,000` and `20,000` particles a planet. Rubble planets are elastic solids and stretch by `1%`.

pub use bevy::math::{Quat, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Aabb, CollisionSettings, DVec3, Scene};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::projects::interstellar::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::projects::interstellar::constants::{
    DEFAULT_RANDOM_SEED, GIANT_IMPACT_BOUNDS_MARGIN_UNITS, GIANT_IMPACT_CAMERA_POSITION_UNITS,
    GIANT_IMPACT_FRAMED_RADIUS_UNITS, GIANT_IMPACT_SOFTENING_UNITS, GIANT_IMPACT_TIME_SCALE,
    GIANT_IMPACT_TRAIL_LENGTH_UNITS, MOON_FORMING_IMPACT_ANGLE_DEG, MOON_FORMING_START_CONTACTS,
};
use crate::projects::interstellar::giant_impact::{
    add_planets, giant_impact_contact_m, giant_impact_impactor, giant_impact_target,
};

/// `G` times the two planets' total mass, m³/s².
fn mu() -> f64 {
    GRAVITATIONAL_CONSTANT * (giant_impact_target().mass_kg + giant_impact_impactor().mass_kg)
}

/// The relative speed the planets meet at, m/s: their mutual escape speed at contact, since the
/// approach is parabolic.
pub fn moon_forming_impact_speed_m_s() -> f64 {
    (2.0 * mu() / giant_impact_contact_m()).sqrt()
}

/// The relative orbit's specific angular momentum, m²/s: the impact speed times the contact
/// distance times the sine of the impact angle.
pub fn moon_forming_specific_angular_momentum() -> f64 {
    moon_forming_impact_speed_m_s()
        * giant_impact_contact_m()
        * MOON_FORMING_IMPACT_ANGLE_DEG.to_radians().sin()
}

/// The pair's total angular momentum about their centre of mass, kg·m²/s.
///
/// # Examples
///
/// ```
/// use physics_sandbox::moon_forming_angular_momentum;
///
/// // About 1.1 times the Earth–Moon system's 3.5e34 kg·m²/s.
/// let ratio = moon_forming_angular_momentum() / 3.5e34;
/// assert!((1.0..1.2).contains(&ratio), "{ratio}");
/// ```
pub fn moon_forming_angular_momentum() -> f64 {
    let (target, impactor) = (
        giant_impact_target().mass_kg,
        giant_impact_impactor().mass_kg,
    );
    target * impactor / (target + impactor) * moon_forming_specific_angular_momentum()
}

/// How long after the start the planets touch, s, by Barker's equation for the parabolic orbit:
///
/// ```text
/// t(r) = √(2q³/μ) · (s + s³/3),   s = √(r/q − 1),   q = h²/(2μ)
/// ```
///
/// the time from closest approach `q` out to separation `r`; the impact time is `t(start) −
/// t(contact)`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::moon_forming_impact_seconds;
///
/// let hours = moon_forming_impact_seconds() / 3600.0;
/// assert!((1.0..3.0).contains(&hours), "{hours}");
/// ```
pub fn moon_forming_impact_seconds() -> f64 {
    let h = moon_forming_specific_angular_momentum();
    let q = h * h / (2.0 * mu());
    let time = |r: f64| {
        let s = (r / q - 1.0).max(0.0).sqrt();
        (2.0 * q.powi(3) / mu()).sqrt() * (s + s.powi(3) / 3.0)
    };
    let contact = giant_impact_contact_m();
    time(MOON_FORMING_START_CONTACTS * contact) - time(contact)
}

/// Build `experiment_8` at the default detail. See [`moon_forming_impact_with`].
pub fn moon_forming_impact() -> Experiment {
    moon_forming_impact_with(&CollisionSettings::default())
}

/// Build `experiment_8`: the Earth and a Mars-mass impactor, each a pile of particles, on the
/// parabolic orbit that brings them together at `45°`. The Earth is made of
/// `settings.particles_per_body` particles, as in `experiment_1`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{moon_forming_impact_with, CollisionSettings};
///
/// let coarse = CollisionSettings { particles_per_body: 50, ..CollisionSettings::default() };
/// let scene = moon_forming_impact_with(&coarse).scene;
/// // The impactor is moving across the line between the two, not only along it.
/// let impactor = scene.props.last().unwrap();
/// assert!(impactor.velocity_units_s.y.abs() > 0.0);
/// ```
pub fn moon_forming_impact_with(settings: &CollisionSettings) -> Experiment {
    let target = giant_impact_target();
    let impactor = giant_impact_impactor();
    let total_mass = target.mass_kg + impactor.mass_kg;

    // The relative orbit at the start: the impactor `start` metres out along X from the Earth,
    // moving at the parabolic speed there, with the tangential part that carries the orbit's
    // angular momentum along Y.
    let start = MOON_FORMING_START_CONTACTS * giant_impact_contact_m();
    let speed = (2.0 * mu() / start).sqrt();
    let tangential = moon_forming_specific_angular_momentum() / start;
    let radial = -(speed * speed - tangential * tangential).max(0.0).sqrt();
    let relative_position = DVec3::new(start, 0.0, 0.0) / METRES_PER_UNIT;
    let relative_velocity = DVec3::new(radial, tangential, 0.0) / METRES_PER_UNIT;

    // About the centre of mass at the origin, at rest.
    let share_target = impactor.mass_kg / total_mass;
    let share_impactor = target.mass_kg / total_mass;
    let planets = [
        (
            &target,
            -relative_position * share_target,
            -relative_velocity * share_target,
        ),
        (
            &impactor,
            relative_position * share_impactor,
            relative_velocity * share_impactor,
        ),
    ];

    let extent = (start / METRES_PER_UNIT) as f32 + GIANT_IMPACT_BOUNDS_MARGIN_UNITS;
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-extent),
        max: Vec3::splat(extent),
    });
    scene.softening_units = GIANT_IMPACT_SOFTENING_UNITS;
    scene.collision = *settings;
    add_planets(&mut scene, settings, planets);

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::MoonForming,
            seed: DEFAULT_RANDOM_SEED,
            time_scale: GIANT_IMPACT_TIME_SCALE,
            camera_position_units: GIANT_IMPACT_CAMERA_POSITION_UNITS,
            camera_target_units: Vec3::ZERO,
            framed_radius_units: GIANT_IMPACT_FRAMED_RADIUS_UNITS,
            trail_length_units: Some(GIANT_IMPACT_TRAIL_LENGTH_UNITS),
            launches: false,
            general_lighting: true,
        },
        names: BodyNames::default(),
    }
}

#[cfg(test)]
#[path = "../../../tst/projects/interstellar/moon_forming_tests.rs"]
mod tests;
