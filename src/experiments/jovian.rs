//! `experiment_4` — Jupiter and the four Galilean moons.
//!
//! The first experiment in the catalog whose **physics is exactly real**: real masses, real
//! orbital radii, real circular speeds, and — new here — real *contact* radii, so Jupiter is
//! physically `0.0699` units across while still being drawn large enough to see. Nothing about
//! the dynamics is scaled, softened, or capped.
//!
//! # Why this experiment needed two changes to the crate
//!
//! Moons are where the crate's two conflations of a visual quantity with a physical one both
//! became fatal at once, and each had to be separated before this scene could exist at all.
//!
//! **The drawn radius was also the contact radius.** Jupiter is drawn at `10.973` units in
//! `experiment_3` — a `157×` exaggeration, without which it is invisible from Mars's orbit.
//! Io orbits at `0.4217` units. So the *drawn* Jupiter is `26×` the radius of its innermost
//! moon's entire orbit, and all four Galilean moons would begin **inside** the planet's hull
//! and be ejected by the contact solver on the first step. The largest exaggeration that keeps
//! Jupiter inside Io's orbit is `6.03×`. There is no single number that both shows Jupiter and
//! lets it hold a moon, so
//! [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units) now carries
//! the physical size and the `Body` keeps the visual one.
//!
//! **Softening was a crate-wide length.** `GRAVITY_SOFTENING_UNITS` is `1.0` unit and Plummer
//! softening leaves `r²/(r² + ε²)` of Newton's force, so at Io's orbital radius it left
//! **`15%`** — a circular speed out by a factor of `2.6`. The moons could not have orbited
//! with the renderer switched off entirely. ε is now
//! [`Scene::softening_units`](crate::core::scene::Scene::softening_units), because ε models a
//! body's finite size and body sizes span three orders of magnitude across the catalog.
//!
//! | Separation | Force with ε = 1.0 | Force with this scene's ε = 0.001 |
//! | --- | --- | --- |
//! | Io's orbit (`0.4217` u) | `15.1%` | `99.99944%` |
//! | Europa (`0.6710` u) | `31.1%` | `99.99978%` |
//! | Callisto (`1.8827` u) | `78.0%` | `99.99997%` |
//!
//! # Nothing is exaggerated, not even the pixels
//!
//! Every body is stored, drawn, and collided at its true radius. This experiment shipped with a
//! `4×` display exaggeration for one day; amendment A16 removed it, because with the renderer
//! flooring *apparent* size there is nothing left for a scene-level factor to do — and a body with
//! one radius needs no [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units)
//! override at all, which is the simplest possible statement of "the physics is real".
//!
//! | Body | Mass | True radius | Orbit | Period |
//! | --- | --- | --- | --- | --- |
//! | Jupiter | `1.898e27 kg` | `0.0699` u | — | — |
//! | Io | `8.93e22 kg` | `0.00182` u | `0.4217` u | `1.77 d` |
//! | Europa | `4.80e22 kg` | `0.00156` u | `0.6710` u | `3.55 d` |
//! | Ganymede | `1.48e23 kg` | `0.00263` u | `1.0704` u | `7.16 d` |
//! | Callisto | `1.08e23 kg` | `0.00241` u | `1.8827` u | `16.69 d` |
//!
//! Note that a moon is enormous compared to its orbit — Io's orbit is `231` Io radii, where
//! Earth's is `23,500` Earth radii. That is why a moon system is the one thing in this crate that
//! was always showable honestly, and why Io orbits at a real `6.03` Jupiter radii here.
//!
//! # The Laplace resonance comes free
//!
//! Io, Europa, and Ganymede are locked in a `1:2:4` orbital resonance, and it is not put in by
//! hand — it falls out of the real semi-major axes. The period ratios from this table are
//! `2.0073` and `2.0147`, and `jovian_tests` checks them. Whether the simulation *maintains*
//! the resonance over many orbits is a question about the integrator, not about the initial
//! conditions, and nothing here asserts that it does.
//!
//! # There is no floor
//!
//! The shared ground plane is `5000` units to the side and the fine grid rules every `5`; this
//! whole system is `1.9` units across, so it would sit inside a single grid cell above a
//! surface with no relationship to anything in the scene. So this experiment does **not** build
//! on [`Scene::demo`] — it is the first scene in the crate with no container at all, which the
//! grid already handles (`floor_half_size` returns `None` and no grid is spawned).
//!
//! # What is still idealised
//!
//! Circular, coplanar orbits: the real Galilean moons have eccentricities up to `0.009` and
//! inclinations under `0.5°`, both small, but Io's eccentricity is forcibly maintained *by* the
//! resonance and is the reason Io is volcanic. Starting phases are arbitrary rather than an
//! ephemeris — see [`JOVIAN_MOONS`]. No tidal heating, no oblateness (Jupiter's `J₂` is what
//! actually drives the resonance's precession), no Jupiter rotation, and no other moons: the
//! remaining ~91 are all under `0.001%` of Jupiter's mass combined.

pub use bevy::math::{Quat, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Aabb, Body, DVec3, Pose, Prop, Scene};
use crate::experiments::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::experiments::constants::{
    DEFAULT_RANDOM_SEED, JOVIAN_BOUNDS_MARGIN_UNITS, JOVIAN_CAMERA_POSITION_UNITS,
    JOVIAN_DISPLAY_EXAGGERATION, JOVIAN_FRAMED_RADIUS_UNITS, JOVIAN_MOONS, JOVIAN_SOFTENING_UNITS,
    JOVIAN_TIME_SCALE, JOVIAN_TRAIL_LENGTH_UNITS, SOLAR_SYSTEM_PLANETS,
};
use crate::experiments::solar_system::{circular_orbit_period_seconds, Planet};

/// Jupiter, as the primary of this system.
///
/// Looked up out of [`SOLAR_SYSTEM_PLANETS`] by name rather than copied, so Jupiter's mass and
/// radius have exactly one home in the crate: `experiment_3` orbits this body around the Sun
/// and `experiment_4` orbits moons around it, and the two can never disagree about how heavy
/// it is.
///
/// Falls back to the table's fifth row if the name is ever edited, which keeps this
/// infallible; `jovian_tests` pins that the fallback is not what is being used.
pub fn jovian_primary() -> Planet {
    SOLAR_SYSTEM_PLANETS
        .iter()
        .find(|planet| planet.name == "Jupiter")
        .copied()
        .unwrap_or(SOLAR_SYSTEM_PLANETS[4])
}

/// The radius a body of true radius `radius_m` is **drawn** at in this experiment, in units.
///
/// `radius_m / METRES_PER_UNIT × `[`JOVIAN_DISPLAY_EXAGGERATION`] — uniform, so every ratio in
/// the picture is real. This is the only scaled quantity in the experiment; contact and
/// gravitation both use the true radius.
///
/// # Examples
///
/// ```
/// use physics_sandbox::jovian_drawn_radius_units;
///
/// // Jupiter's true radius is 6.9911e7 m, so at 4x it is drawn at 0.2796 units.
/// assert!((jovian_drawn_radius_units(6.9911e7) - 0.2796).abs() < 1.0e-4);
///
/// // And the ratio to Io survives, because the exaggeration is uniform.
/// let ratio = jovian_drawn_radius_units(6.9911e7) / jovian_drawn_radius_units(1.8216e6);
/// assert!((ratio - 6.9911e7 / 1.8216e6).abs() < 1.0e-3);
/// ```
pub fn jovian_drawn_radius_units(radius_m: f64) -> f32 {
    debug_assert!(
        radius_m.is_finite() && radius_m > 0.0,
        "jovian_drawn_radius_units: radius must be finite and positive, got {}",
        radius_m
    );

    if !radius_m.is_finite() || radius_m <= 0.0 {
        return JOVIAN_DISPLAY_EXAGGERATION;
    }

    (radius_m / METRES_PER_UNIT) as f32 * JOVIAN_DISPLAY_EXAGGERATION
}

/// The true radius of a body, in world units — what it physically collides at.
#[inline]
fn true_radius_units(radius_m: f64) -> f32 {
    (radius_m / METRES_PER_UNIT) as f32
}

/// Circular orbital speed about Jupiter at `radius_m`, in world units per second.
///
/// `v = √(G·M_J/r)`, delegated to
/// [`circular_orbit_speed_units_s`](crate::experiments::solar_system::circular_orbit_speed_units_s)
/// with Jupiter as the central mass — the same function `experiment_3` uses for the planets, so
/// there is one implementation of the formula in the crate rather than two.
///
/// The moon's own mass is left out, as it is for the planets: Ganymede is `7.8e-5` of Jupiter,
/// so including it would change the speed by `0.004%`.
pub fn jovian_orbit_speed_units_s(radius_m: f64) -> f64 {
    crate::experiments::solar_system::circular_orbit_speed_units_s(
        radius_m,
        jovian_primary().mass_kg as f32,
    )
}

/// Orbital period about Jupiter at `radius_m`, in seconds.
///
/// Public because the period ratios are the experiment's most checkable claim — `1.77`,
/// `3.55`, `7.16`, and `16.69` days, and the `1:2:4` resonance between the first three.
pub fn jovian_orbit_period_seconds(radius_m: f64) -> f64 {
    circular_orbit_period_seconds(radius_m, jovian_primary().mass_kg as f32)
}

/// `experiment_4` — Jupiter with Io, Europa, Ganymede, and Callisto.
///
/// Deterministic: every initial condition is a measurement, so the seed plays no part and is
/// recorded as [`DEFAULT_RANDOM_SEED`] so the overlay has something honest to print.
///
/// # What it builds
///
/// Five props and **no floor**: Jupiter at the origin, and one moon per row of
/// [`JOVIAN_MOONS`] at its real orbital radius with the exactly tangential velocity that makes
/// the orbit circular. Every prop carries a
/// [`contact_radius_units`](crate::core::scene::Prop::contact_radius_units) equal to its
/// **true** radius while its `Body` is drawn at [`JOVIAN_DISPLAY_EXAGGERATION`], and the scene
/// sets its own [`softening_units`](crate::core::scene::Scene::softening_units) — the two
/// things that make a moon system possible at all.
///
/// Jupiter is given the velocity that cancels the moons' momentum, putting the simulation in
/// the barycentric frame, exactly as `experiment_3` does for the Sun. The recoil is tiny
/// (`2.5e-9` units/s) but it is free and it is correct.
///
/// # What to expect
///
/// Io comes round every `15` seconds of watching at [`JOVIAN_TIME_SCALE`], Europa every `30`,
/// Ganymede every `61`, Callisto every `144` — the `1:2:4` resonance visible as Io and Europa
/// lining up with Ganymede on a repeating beat. The moons are small: `6`–`9` px at the opening
/// framing, and they resolve as you fly in. Nothing collides, and with real contact radii
/// nothing can: Jupiter's surface is `0.0699` units and Io's orbit is `0.4217`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{jovian_system, JOVIAN_MOONS};
///
/// let experiment = jovian_system();
///
/// // Jupiter plus four moons, and no floor.
/// assert_eq!(experiment.scene.props.len(), JOVIAN_MOONS.len() + 1);
/// assert!(experiment.scene.props.iter().all(|prop| !prop.immovable));
///
/// // Physics is real, and since amendment A16 that needs no override at all: a body's stored
/// // radius *is* its true radius, so there is no drawn size to correct for.
/// assert!(experiment.scene.props.iter().all(|prop| prop.contact_radius_units.is_none()));
/// ```
pub fn jovian_system() -> Experiment {
    let jupiter = jovian_primary();

    // `Scene::empty`, not `Scene::demo()`: this system is 1.9 units across and the shared
    // floor is 5000, with a grid ruled every 5 — the whole experiment would sit inside one grid
    // cell above a surface unrelated to anything in it. See the module docs.
    //
    // Bounds are replaced once the outermost orbit is known; this is a placeholder that is
    // valid in case an early return is ever added above.
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-JOVIAN_BOUNDS_MARGIN_UNITS),
        max: Vec3::splat(JOVIAN_BOUNDS_MARGIN_UNITS),
    });

    scene.softening_units = JOVIAN_SOFTENING_UNITS;

    let mut names = BodyNames::default();
    let jupiter_id = scene.add(Prop::at_rest(
        Body::Sphere {
            pose: Pose {
                position: DVec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            radius: true_radius_units(jupiter.radius_m),
            color: jupiter.color,
        },
        jupiter.mass_kg as f32,
    ));

    // Momentum of the moons, in kg·units/s. Only X and Z: every orbit is in one plane, so the
    // Y component is exactly zero by construction.
    let mut momentum_x = 0.0_f64;
    let mut momentum_z = 0.0_f64;
    let mut outermost_units = 0.0_f32;

    for moon in JOVIAN_MOONS {
        let orbit_units = moon.semi_major_axis_m / METRES_PER_UNIT;
        let longitude = moon.mean_longitude_deg.to_radians();

        // Prograde about +Y, the same convention `experiment_3` uses: `z` takes `−sin` so that
        // increasing longitude winds counterclockwise seen from above.
        let radial = DVec3::new(longitude.cos(), 0.0, -longitude.sin());
        let position = radial * orbit_units;
        let tangent = DVec3::Y.cross(radial).normalize();
        let velocity = tangent * jovian_orbit_speed_units_s(moon.semi_major_axis_m);

        momentum_x += moon.mass_kg * velocity.x;
        momentum_z += moon.mass_kg * velocity.z;
        outermost_units = outermost_units.max(orbit_units as f32);

        let id = scene.add(Prop::new(
            Body::Sphere {
                pose: Pose {
                    position,
                    rotation: Quat::IDENTITY,
                },
                radius: true_radius_units(moon.radius_m),
                color: moon.color,
            },
            velocity,
            moon.mass_kg as f32,
        ));
        names.insert(id, moon.name);
    }

    names.insert(jupiter_id, jupiter.name);

    // The barycentric frame: Jupiter takes the momentum that cancels the moons'.
    let jupiter_mass = jupiter.mass_kg;
    scene.props[0].velocity_units_s =
        DVec3::new(-momentum_x / jupiter_mass, 0.0, -momentum_z / jupiter_mass);

    let extent = outermost_units + JOVIAN_BOUNDS_MARGIN_UNITS;
    scene.bounds = Aabb {
        min: Vec3::splat(-extent),
        max: Vec3::splat(extent),
    };

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::Jovian,
            seed: DEFAULT_RANDOM_SEED,
            time_scale: JOVIAN_TIME_SCALE,
            camera_position_units: JOVIAN_CAMERA_POSITION_UNITS,
            camera_target_units: Vec3::ZERO,
            framed_radius_units: JOVIAN_FRAMED_RADIUS_UNITS,
            trail_length_units: Some(JOVIAN_TRAIL_LENGTH_UNITS),
            launches: true,
            general_lighting: true,
        },
        names,
    }
}

#[cfg(test)]
#[path = "../../tst/experiments/jovian_tests.rs"]
mod tests;
