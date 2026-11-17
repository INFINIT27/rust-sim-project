//! `experiment_7` — the Earth and the Moon, alone, at true size and true distance.
//!
//! Two bodies and nothing else: the Earth, and the Moon orbiting it. Every size and every distance
//! is the real one, so the picture is in true proportion — the Moon is `0.273` of the Earth's
//! radius and orbits `60.3` Earth radii out. Nothing is scaled, softened to visibility, or capped.
//!
//! | Body | Mass | True radius | Distance from the barycentre |
//! | --- | --- | --- | --- |
//! | Earth | `5.9722e24 kg` | `0.006371` u | `0.00467` u |
//! | Moon | `7.342e22 kg` | `0.0017374` u | `0.37973` u |
//!
//! # Both bodies orbit, about their common centre
//!
//! The Moon is `1/81.3` of the Earth's mass, which is large for a moon: Ganymede is `1/12,800` of
//! Jupiter. So this scene does not pin the Earth and send the Moon round it. It solves the two-body
//! problem exactly: both bodies start on circles about their **barycentre**, which sits at the
//! origin, and the camera looks at that point.
//!
//! - The separation is the Moon's real semi-major axis, `384,400 km` (`0.3844` units).
//! - The barycentre is `4,668 km` from the Earth's centre, which is inside the Earth (its radius is
//!   `6,371 km`). The Earth therefore wobbles about a point under its own surface, once per month.
//! - The relative speed is `√(G·(M+m)/r)` = `1024.6 m/s`, with **both** masses in it. The Moon
//!   takes `1012.1 m/s` of that and the Earth `12.4 m/s`, in opposite directions, so the total
//!   momentum is zero and the barycentre stays where it is.
//! - The period that follows is `27.28` days, against a measured sidereal month of `27.32`. The
//!   difference is what treating an eccentric orbit as a circle costs.
//!
//! `experiment_3` also has the Moon, but there it leaves the Moon's own mass out of the speed
//! (`√(G·M/r)`), which gives `27.45` days. Here the Moon is half the experiment, so its mass is in.
//!
//! # The data has one home
//!
//! The Earth is the `"Earth"` row of [`SOLAR_SYSTEM_PLANETS`] and the Moon is the one row of
//! [`EARTH_MOONS`] — the same rows `experiment_3` reads. This module declares no mass, radius or
//! distance of its own, so the two experiments cannot disagree about either body.
//!
//! # There is no floor
//!
//! Like `experiment_4`: the whole system is `0.77` units across, and the shared ground plane is
//! `5000`. The scene is built on [`Scene::empty`].
//!
//! # What is still idealised
//!
//! - **A circular orbit.** The real one has an eccentricity of `0.0549`, so the Moon's distance
//!   swings between `363,300` and `405,500 km`.
//! - **In the scene's horizontal plane.** The real orbit is tilted `5.1°` to the ecliptic, and the
//!   Earth's axis `23.4°`; neither tilt means anything in a scene with no Sun.
//! - **No Sun.** So no phases, no eclipses, and no solar perturbation of the orbit. The scene is
//!   lit by the renderer's fixed light.
//! - **No rotation.** Neither body spins, and neither has a surface map yet: each is a plain
//!   coloured sphere.
//! - **An arbitrary starting phase.** The Moon starts at the longitude [`EARTH_MOONS`] gives it,
//!   which is not an ephemeris.

pub use bevy::math::{Quat, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Aabb, Body, DVec3, Pose, Prop, Scene};
use crate::projects::interstellar::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::projects::interstellar::constants::{
    DEFAULT_RANDOM_SEED, EARTH_MOONS, EARTH_MOON_BOUNDS_MARGIN_UNITS,
    EARTH_MOON_CAMERA_POSITION_UNITS, EARTH_MOON_FRAMED_RADIUS_UNITS, EARTH_MOON_SOFTENING_UNITS,
    EARTH_MOON_TIME_SCALE, EARTH_MOON_TRAIL_LENGTH_UNITS, SOLAR_SYSTEM_PLANETS,
};
use crate::projects::interstellar::solar_system::{
    circular_orbit_period_seconds, circular_orbit_speed_units_s, radius_units, Planet,
};

/// The Earth, as one of the two bodies of this system.
///
/// Looked up out of [`SOLAR_SYSTEM_PLANETS`] by name rather than copied, so the Earth's mass and
/// radius have one home in the crate. Falls back to the table's third row if the name is ever
/// edited, which keeps this infallible; `earth_moon_tests` pins that the fallback is not what is
/// being used.
pub fn earth_moon_primary() -> Planet {
    SOLAR_SYSTEM_PLANETS
        .iter()
        .find(|planet| planet.name == "Earth")
        .copied()
        .unwrap_or(SOLAR_SYSTEM_PLANETS[2])
}

/// The Moon: the one row of [`EARTH_MOONS`], the same row `experiment_3` orbits round the Earth.
///
/// Its `semi_major_axis_m` is the distance between the two bodies' centres.
pub fn earth_moon_satellite() -> Planet {
    EARTH_MOONS[0]
}

/// The Moon's share of the system's mass, `m / (M + m)`: `0.01214`.
///
/// The barycentre divides the line between the two centres in this ratio, so it is this fraction
/// of the separation from the Earth's centre, and the Earth carries this fraction of the relative
/// speed.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{earth_moon_mass_fraction, earth_moon_satellite};
///
/// // The barycentre is about 4,670 km from the Earth's centre: inside the Earth.
/// let offset_km = earth_moon_mass_fraction() * earth_moon_satellite().semi_major_axis_m / 1000.0;
/// assert!((offset_km - 4_668.0).abs() < 10.0);
/// ```
pub fn earth_moon_mass_fraction() -> f64 {
    let earth = earth_moon_primary();
    let moon = earth_moon_satellite();

    moon.mass_kg / (earth.mass_kg + moon.mass_kg)
}

/// The speed of the Moon **relative to the Earth** on their circular orbit, in world units per
/// second.
///
/// `v = √(G·(M+m)/r)`, with both masses, which is the exact two-body result. It is delegated to
/// [`circular_orbit_speed_units_s`] with the total mass as the central mass, so the crate still has
/// one implementation of the formula.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{earth_moon_orbit_speed_units_s, METRES_PER_UNIT};
///
/// // 1024.6 m/s; the Moon's measured mean orbital speed is 1022.
/// let speed_m_s = earth_moon_orbit_speed_units_s() * METRES_PER_UNIT;
/// assert!((speed_m_s - 1024.6).abs() < 1.0);
/// ```
pub fn earth_moon_orbit_speed_units_s() -> f64 {
    let earth = earth_moon_primary();
    let moon = earth_moon_satellite();

    circular_orbit_speed_units_s(
        moon.semi_major_axis_m,
        (earth.mass_kg + moon.mass_kg) as f32,
    )
}

/// The period of the orbit, in seconds: `27.28` days.
///
/// Public because it is the experiment's most checkable number. The measured sidereal month is
/// `27.32` days.
///
/// # Examples
///
/// ```
/// use physics_sandbox::earth_moon_orbit_period_seconds;
///
/// let days = earth_moon_orbit_period_seconds() / 86_400.0;
/// assert!((days - 27.28).abs() < 0.02);
/// ```
pub fn earth_moon_orbit_period_seconds() -> f64 {
    let earth = earth_moon_primary();
    let moon = earth_moon_satellite();

    circular_orbit_period_seconds(
        moon.semi_major_axis_m,
        (earth.mass_kg + moon.mass_kg) as f32,
    )
}

/// `experiment_7` — the Earth and the Moon on their real orbit, at true size.
///
/// Deterministic: every initial condition is a measurement, so the seed plays no part and is
/// recorded as [`DEFAULT_RANDOM_SEED`] so the overlay has something honest to print.
///
/// # What it builds
///
/// Two props and **no floor**: the Earth first, then the Moon, on opposite sides of the origin,
/// which is their barycentre. Each has the tangential velocity of its own circle about that point,
/// so the total momentum is zero. Each is stored, drawn and collided at its true radius, and the
/// scene sets its own [`softening_units`](crate::core::scene::Scene::softening_units).
///
/// # What to expect
///
/// The Moon comes round every `23.6` seconds of watching at [`EARTH_MOON_TIME_SCALE`]. From the
/// opening camera the Earth is a small disc and the Moon a smaller one, a long way out: that gap is
/// the real one. The Earth's own motion is a circle smaller than the Earth, so it shows only as a
/// slight wobble when you fly close.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{earth_moon_system, EARTH_RADIUS_M, METRES_PER_UNIT};
///
/// let experiment = earth_moon_system();
///
/// // The Earth and the Moon, and no floor.
/// assert_eq!(experiment.scene.props.len(), 2);
/// assert!(experiment.scene.props.iter().all(|prop| !prop.immovable));
///
/// // Their centres are 384,400 km apart.
/// let earth = experiment.scene.props[0].body.center();
/// let moon = experiment.scene.props[1].body.center();
/// assert!(((moon - earth).length() * METRES_PER_UNIT / 3.844e8 - 1.0).abs() < 1.0e-6);
///
/// // That is 60.3 Earth radii.
/// assert!(((moon - earth).length() * METRES_PER_UNIT / EARTH_RADIUS_M - 60.3).abs() < 0.1);
/// ```
pub fn earth_moon_system() -> Experiment {
    let earth = earth_moon_primary();
    let moon = earth_moon_satellite();

    let separation_units = moon.semi_major_axis_m / METRES_PER_UNIT;
    let moon_fraction = earth_moon_mass_fraction();
    let earth_fraction = 1.0 - moon_fraction;

    // Prograde about +Y, the convention `experiment_3` and `experiment_4` use: `z` takes `−sin` so
    // that increasing longitude winds counterclockwise seen from above.
    let longitude = moon.mean_longitude_deg.to_radians();
    let radial = DVec3::new(longitude.cos(), 0.0, -longitude.sin());
    let tangent = DVec3::Y.cross(radial).normalize();
    let relative_velocity = tangent * earth_moon_orbit_speed_units_s();

    // Each body sits on the far side of the barycentre from the other, at a distance in inverse
    // proportion to its mass, and takes the same share of the relative velocity.
    let moon_position = radial * (separation_units * earth_fraction);
    let earth_position = -radial * (separation_units * moon_fraction);
    let moon_velocity = relative_velocity * earth_fraction;
    let earth_velocity = -relative_velocity * moon_fraction;

    // `Scene::empty`, not `Scene::demo()`: the system is 0.77 units across and the shared floor is
    // 5000. See the module docs.
    let extent = (separation_units * earth_fraction) as f32 + EARTH_MOON_BOUNDS_MARGIN_UNITS;
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-extent),
        max: Vec3::splat(extent),
    });

    scene.softening_units = EARTH_MOON_SOFTENING_UNITS;

    let mut names = BodyNames::default();

    let earth_id = scene.add(Prop::new(
        Body::Sphere {
            pose: Pose {
                position: earth_position,
                rotation: Quat::IDENTITY,
            },
            radius: radius_units(earth.radius_m),
            color: earth.color,
        },
        earth_velocity,
        earth.mass_kg as f32,
    ));
    names.insert(earth_id, earth.name);

    let moon_id = scene.add(Prop::new(
        Body::Sphere {
            pose: Pose {
                position: moon_position,
                rotation: Quat::IDENTITY,
            },
            radius: radius_units(moon.radius_m),
            color: moon.color,
        },
        moon_velocity,
        moon.mass_kg as f32,
    ));
    names.insert(moon_id, moon.name);

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::EarthMoon,
            seed: DEFAULT_RANDOM_SEED,
            time_scale: EARTH_MOON_TIME_SCALE,
            camera_position_units: EARTH_MOON_CAMERA_POSITION_UNITS,
            // The barycentre, which is the one point in the scene that does not move.
            camera_target_units: Vec3::ZERO,
            framed_radius_units: EARTH_MOON_FRAMED_RADIUS_UNITS,
            trail_length_units: Some(EARTH_MOON_TRAIL_LENGTH_UNITS),
            launches: true,
            general_lighting: true,
        },
        names,
    }
}

#[cfg(test)]
#[path = "../../../tst/projects/interstellar/earth_moon_tests.rs"]
mod tests;
