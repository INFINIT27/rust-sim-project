//! `experiment_3` — the Sun and the eight planets, built from real measurements.
//!
//! Masses, radii, and orbital distances are the published SI figures (see
//! [`SOLAR_SYSTEM_PLANETS`]); the starting angles are the real J2000 mean longitudes, so
//! the planets begin where they actually were on 1 January 2000 rather than in a line.
//! Every orbital speed is derived from `v = √(G·M/a)` rather than tabulated, so the scene
//! stays consistent if the Sun's mass is edited.
//!
//! # Nothing here is exaggerated
//!
//! At the crate's `1 unit = 1e9 m` ([`METRES_PER_UNIT`]) this system is mostly empty space:
//! Earth's orbit is `150` units across the radius and the Earth itself is `0.0064` units. Every
//! body is stored at exactly that — its true size — and so is every distance and every mass.
//!
//! Until amendment A16 radii were multiplied by `157`, so that the Earth came out at one unit and
//! a planet was visible from Mars's orbit. Visibility is now the renderer's job: a sphere is never
//! drawn smaller than
//! Nothing compensates for that, either — see below.
//!
//! That change removed three defects this module used to document as unavoidable:
//!
//! - **The Sun is no longer capped.** At `157×` it would have been `109.2` units — larger than
//!   Mercury's whole `57.9`-unit orbit — so it had to be drawn at `20` instead, which left Jupiter
//!   looking `5.5×` too large beside it. At its true `0.6957` units the ratio to Jupiter is the
//!   real `9.95:1` and there is nothing to cap.
//! - **Jupiter can hold moons.** Its drawn hull was `26×` the radius of Io's entire orbit, so the
//!   Galilean moons started *inside* the planet and were ejected on the first step. They are now
//!   in this scene — see the builder.
//! - **The picture no longer disagrees with the physics** about which body is inside which.
//!
//! What this costs is real and worth stating: from the opening framing, `610` units out, Jupiter
//! subtends `0.47` px and the Earth `0.04`, so every body sits on the apparent-size floor and the
//! solar system opens as nine identical dots. That is what it looks like from `4 AU`. The camera is
//! the tool for choosing scale.
//!
//! # The Sun lights the scene
//!
//! It carries a [`luminosity_solar`](crate::core::scene::Prop::luminosity_solar) of `1.0`, which
//! the render layer turns into a point light at the Sun's own position. So the lit hemisphere of a
//! planet faces the Sun — the terminator is where it belongs rather than where a fixed directional
//! light put it — and the illuminance falls off as `1/r²`, which is `120,000 lux` at Earth and
//! `133` at Neptune. The outer planets are genuinely dark, and that is not a rendering failure.
//!
//! # Why the Sun gets a velocity
//!
//! Give eight planets their orbital velocities and give the Sun none, and the system has a
//! net momentum of about `3e31 kg·m/s` — mostly Jupiter's — so the whole solar system
//! drifts sideways out of frame, about `150` units per `1e10` simulated seconds. The fix
//! is not a hack but a choice of frame: the Sun is given exactly the momentum that
//! cancels the planets', which puts the simulation in the **barycentric frame** where the
//! total momentum is zero and the centre of mass stays put. That is also the physically
//! correct thing to do, and it makes the Sun's own small wobble — the signal real
//! exoplanet detection uses — visible in its trail.
//!
//! # What is idealised
//!
//! Circular, coplanar orbits: real eccentricities run to `0.206` (Mercury) and real
//! inclinations to `7°`. Periods come out within a percent of the real ones. No asteroid belt and
//! no Pluto, and **five moons**: Earth's, and Jupiter's four Galilean ones. They are the case that
//! forced the crate to separate a body's physical size from its drawn one, and they are here because
//! it now can. The other ~285 moons are not.
//!
//! The integrator is first-order symplectic. Mercury gets about `456` steps per orbit at
//! [`SOLAR_SYSTEM_TIME_SCALE`] and Io about `900`, the latter because
//! [`substep_count`](crate::collision::resolve::substep_count) splits a frame on the tightest orbit's
//! dynamical time — one frame at this clock is `10.9%` of Io's whole orbit, so the scene takes about
//! `98` substeps and pays `3.2 ms` a frame in release for them, a fifth of a 60 Hz budget.
//!
//! If you want to *watch* the moons rather than merely have them be right, `-` divides the clock by
//! ten and `+` multiplies it: two presses of `-` puts Io's `1.77`-day orbit at `15` seconds of watching
//! instead of `1.5`, and Earth's Moon at `230`.
//!
//! # A real annular eclipse, for free
//!
//! From Earth's *centre* the Moon subtends `4.520e-3` radians and the Sun `4.650e-3` — a ratio of
//! `0.972`, so the Moon is **just too small to cover the Sun** and [`crate::render::eclipse`] draws an
//! annular eclipse rather than a total one. That is not a tuning choice; it is what the real numbers
//! give at the Moon's mean distance, and it is why real annular eclipses exist.
//!
//! The shader is computed per fragment on Earth's *surface*, which is one Earth radius — `1.7%` of the
//! Moon's orbit — nearer to the Moon, so the Moon looks `1.7%` larger there and covers `97.7%` of the
//! Sun rather than `94.5%`. That difference is not an error either: standing on the surface is the
//! whole reason a total eclipse is possible at all, and at perigee it is what tips the ratio past `1`.
//!
//! [`METRES_PER_UNIT`]: crate::constants::METRES_PER_UNIT

pub use bevy::math::{Quat, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::constants::DEMO_BOUNDS_MIN_UNITS;
use crate::core::scene::{Aabb, Body, Color, DVec3, Pose, Prop, Scene};
use crate::experiments::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::experiments::constants::{
    DEFAULT_RANDOM_SEED, MOON_SYSTEMS, SOLAR_SYSTEM_CAMERA_POSITION_UNITS,
    SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS, SOLAR_SYSTEM_FRAMED_RADIUS_UNITS, SOLAR_SYSTEM_PLANETS,
    SOLAR_SYSTEM_SOFTENING_UNITS, SOLAR_SYSTEM_SUN_COLOR, SOLAR_SYSTEM_SUN_MASS_KG,
    SOLAR_SYSTEM_SUN_RADIUS_M, SOLAR_SYSTEM_TIME_SCALE, SOLAR_SYSTEM_TRAIL_LENGTH_UNITS,
};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

/// One planet's real measurements: what it weighs, how big it is, how far out it orbits,
/// and where on that orbit it starts.
///
/// Everything is **SI** — kilograms, metres, degrees — deliberately: these are quoted
/// figures, and converting them to world units in the table would make them
/// unrecognisable against the source and would bake in [`METRES_PER_UNIT`]. The
/// conversion happens once, in [`solar_system`].
///
/// `f64` throughout because `semi_major_axis_m` reaches `4.5e12`, where `f32`'s spacing is
/// `500 km` — fine for a position, wrong for a number you are about to take a square root
/// of.
///
/// [`METRES_PER_UNIT`]: crate::constants::METRES_PER_UNIT
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Planet {
    /// The planet's name, for the overlay and for test failure messages.
    pub name: &'static str,
    /// Mass, in kilograms.
    pub mass_kg: f64,
    /// Volumetric mean radius, in metres — the body's **true** size, which is also what it
    /// is drawn and collided at. See [`radius_units`].
    pub radius_m: f64,
    /// Mean orbital distance from the Sun, in metres. Used as a circular orbit's radius.
    pub semi_major_axis_m: f64,
    /// J2000 mean longitude, in degrees: where on its circle the planet starts, measured
    /// prograde from the `+X` axis.
    pub mean_longitude_deg: f64,
    /// Linear-space colour, chosen to be recognisable and mutually distinguishable.
    pub color: Color,
}

impl Planet {
    /// Validation_Rules for `Planet`: finite and strictly positive mass, radius, and
    /// orbital distance; a finite starting angle; a valid colour; and a non-empty name.
    ///
    /// A zero radius would fail [`Body::is_valid`], and a zero orbital distance would make
    /// the circular-orbit speed infinite — so this is checked over the whole table by
    /// `solar_system_tests`, which is cheaper than discovering either at runtime.
    pub fn is_valid(&self) -> bool {
        let positive = |value: f64| value.is_finite() && value > 0.0;

        !self.name.is_empty()
            && positive(self.mass_kg)
            && positive(self.radius_m)
            && positive(self.semi_major_axis_m)
            && self.mean_longitude_deg.is_finite()
            && self.color.is_valid()
    }
}

const FIRST_PLANET_PROP_INDEX: usize = 2; // ALGORITHM CONSTANT: structural offset, not a tunable
                                          // — the gap between a `SOLAR_SYSTEM_PLANETS` row and
                                          // its prop index in the scene this builder produces.
                                          // Changing it does not adjust anything, it just makes
                                          // the lookup wrong, so it lives beside the loop it
                                          // indexes rather than in `constants.rs`. See the
                                          // exception in `.claude/steering/constants.md`.

/// A primary and the moons that orbit it.
///
/// Exists so the builder has no special case for Jupiter. Adding Saturn's moons is a row in
/// [`MOON_SYSTEMS`] and nothing else — which is the shape
/// the first version of this did *not* have, and the reason Earth's Moon needed a refactor rather than
/// a copy of the Jupiter block.
///
/// The primary is named rather than indexed, so a reordering of
/// [`SOLAR_SYSTEM_PLANETS`] cannot silently
/// reattach a moon to the wrong planet.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MoonSystem {
    /// The [`Planet::name`] of the body these moons orbit.
    pub primary: &'static str,
    /// The moons, described exactly as a planet is: a `Planet` on a circular orbit about its primary
    /// rather than about the Sun.
    pub moons: &'static [Planet],
}

/// A body's radius in world units: `radius_m / `[`METRES_PER_UNIT`], and nothing else.
///
/// # This used to be an exaggeration, and no longer is
///
/// Until amendment A16 this function multiplied by `157` so that the Earth came out at one unit
/// and a planet was visible from Mars's orbit. That exaggeration was the crate's largest lie and
/// it cost more than it bought: it forced the Sun to be capped at `20` units (making Jupiter look
/// `5.5×` too large beside it), it made a black hole absorb at `27,000×` its event horizon, and
/// it put the drawn Jupiter at `26×` the radius of Io's entire orbit — so Jupiter could not hold
/// a moon.
///
/// Bodies are now stored, drawn, and collided at their **true** size, with nothing compensating for
/// it: a body smaller than a pixel is not drawn, and the camera is the only instrument for choosing
/// scale. See amendment A17 for why every alternative was worse.
///
/// # Returns
///
/// `0.0` for a non-finite or non-positive radius. Note that is *not* a usable body radius —
/// [`Body::is_valid`](crate::core::scene::Body::is_valid) rejects it — which is deliberate: a
/// bad measurement should fail validation loudly rather than be silently replaced by an
/// Earth-sized stand-in, now that there is no exaggeration to hide inside.
///
/// [`METRES_PER_UNIT`]: crate::constants::METRES_PER_UNIT
///
/// # Examples
///
/// ```
/// use physics_sandbox::{radius_units, EARTH_RADIUS_M};
///
/// // The Earth is 6371 km, which is 0.0064 units. It really is that small.
/// assert!((radius_units(EARTH_RADIUS_M) - 0.006371).abs() < 1.0e-9);
///
/// // And Jupiter's true 11:1 ratio to it is preserved, because nothing is scaled.
/// assert!((radius_units(6.9911e7) / radius_units(EARTH_RADIUS_M) - 10.973).abs() < 0.01);
/// ```
pub fn radius_units(radius_m: f64) -> f32 {
    debug_assert!(
        radius_m.is_finite() && radius_m > 0.0,
        "radius_units: radius must be finite and positive, got {}",
        radius_m
    );

    if !radius_m.is_finite() || radius_m <= 0.0 {
        return 0.0;
    }

    (radius_m / METRES_PER_UNIT) as f32
}

/// Speed, in world **units per second**, of a circular orbit of radius `radius_m` metres
/// about a central mass of `central_mass_kg`.
///
/// `v = √(G·M/r)`, the textbook result, evaluated in `f64` against SI and then divided by
/// [`METRES_PER_UNIT`] — the same units↔SI dance
/// [`ring_circular_speed_units_s`](crate::experiments::catalog::ring_circular_speed_units_s)
/// does, and for the same reason: `G·M_sun` is `1.33e20`, which `f32` cannot hold usefully.
///
/// The orbiting body's own mass is deliberately left out. Including it — `√(G(M+m)/r)` —
/// would give the *relative* speed of a two-body orbit, which differs from this by
/// `0.05%` even for Jupiter, and this way every planet's speed depends on the Sun alone,
/// which is what makes them independent of each other's tabulated masses.
///
/// # Returns
///
/// `0.0` for a non-positive or non-finite radius or mass, rather than a `NaN` — matching
/// the crate's benign-in-release contract. A body given zero velocity simply falls
/// straight into the Sun, which is visibly wrong rather than silently poisonous.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{circular_orbit_speed_units_s, METRES_PER_UNIT, SOLAR_SYSTEM_SUN_MASS_KG};
///
/// // Earth's orbital speed is 29.8 km/s, which is 2.98e-5 units per second.
/// let speed = circular_orbit_speed_units_s(1.4960e11, SOLAR_SYSTEM_SUN_MASS_KG);
/// let metres_per_second = f64::from(speed) * METRES_PER_UNIT;
///
/// assert!((metres_per_second - 29_785.0).abs() < 50.0, "got {metres_per_second} m/s");
/// ```
///
/// [`METRES_PER_UNIT`]: crate::constants::METRES_PER_UNIT
pub fn circular_orbit_speed_units_s(radius_m: f64, central_mass_kg: f32) -> f64 {
    debug_assert!(
        radius_m.is_finite() && radius_m > 0.0,
        "circular_orbit_speed_units_s: radius must be finite and positive, got {}",
        radius_m
    );
    debug_assert!(
        central_mass_kg.is_finite() && central_mass_kg > 0.0,
        "circular_orbit_speed_units_s: central mass must be finite and positive, got {}",
        central_mass_kg
    );

    if !radius_m.is_finite()
        || radius_m <= 0.0
        || !central_mass_kg.is_finite()
        || central_mass_kg <= 0.0
    {
        return 0.0;
    }

    let speed_m_s = (GRAVITATIONAL_CONSTANT * f64::from(central_mass_kg) / radius_m).sqrt();

    speed_m_s / METRES_PER_UNIT
}

/// Orbital period of a circular orbit of radius `radius_m` about `central_mass_kg`, in
/// **seconds**.
///
/// `T = 2πr/v` from [`circular_orbit_speed_units_s`], which is Kepler's third law written
/// the way the scene actually uses it. Public because it is how the time scale was chosen
/// and how a test can say "a year passes in 31.6 seconds of watching" without a second
/// copy of the algebra.
///
/// Returns `0.0` where the speed would be zero, for the same reason.
pub fn circular_orbit_period_seconds(radius_m: f64, central_mass_kg: f32) -> f64 {
    let speed_units_s = circular_orbit_speed_units_s(radius_m, central_mass_kg);

    if speed_units_s <= 0.0 {
        return 0.0;
    }

    std::f64::consts::TAU * (radius_m / METRES_PER_UNIT) / speed_units_s
}

/// Position of a body at `longitude_deg` on a circle of `radius_units`, in the ecliptic
/// plane.
///
/// Longitude runs **prograde**: counterclockwise about `+Y`, seen from above. That is why
/// `z` takes `−sin` rather than `+sin` — with `+Y` as north, `(cos L, 0, +sin L)` would
/// wind the planets *clockwise* and the whole system would run backwards. The
/// arrangement of the J2000 longitudes relative to each other is only meaningful with the
/// sense right.
fn ecliptic_position(longitude_deg: f64, radius_units: f64) -> DVec3 {
    let longitude = longitude_deg.to_radians();
    let radial = DVec3::new(longitude.cos(), 0.0, -longitude.sin());

    radial * radius_units + DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS)
}

/// `experiment_3` — the Sun and the eight planets on real orbits.
///
/// Deterministic: the seed plays no part, since every initial condition is a measurement.
/// It is recorded in the spec as [`DEFAULT_RANDOM_SEED`] so the overlay has something
/// honest to print, the same way [`two_body`](crate::experiments::catalog::two_body) does.
///
/// # What it builds
///
/// Ten props: the floor, the Sun at the centre of the ecliptic plane, and one sphere per
/// row of [`SOLAR_SYSTEM_PLANETS`], each placed at its real distance and J2000 longitude
/// and given the exactly tangential velocity that makes its orbit circular. The Sun then
/// receives the velocity that zeroes the system's total momentum — see the module docs.
///
/// The scene's [`bounds`](Scene::bounds) are widened from the shared `±50` to enclose
/// Neptune's orbit, because a solar system inside a `50`-unit box is a claim no part of
/// the code should be making.
///
/// # What to expect
///
/// The four terrestrial planets are in the opening frame and visibly moving: Mercury
/// completes an orbit every `7.6` seconds of watching, Earth every `31.6`. The four giants
/// are out beyond the frame — Jupiter `778` units out, Neptune `4495` — and need either
/// flying to (`W`, with `Left Shift` to boost) or a faster clock (`]`). Nothing collides:
/// the orbits are separated by hundreds of units and stay that way, so this is the first
/// experiment in the catalog whose point is that the contact solver never fires.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{solar_system, EARTH_MOONS, JOVIAN_MOONS, SOLAR_SYSTEM_PLANETS};
///
/// // Floor + Sun + eight planets + Earth's Moon + Jupiter's four Galilean moons.
/// let experiment = solar_system();
///
/// assert_eq!(
///     experiment.scene.props.len(),
///     SOLAR_SYSTEM_PLANETS.len() + EARTH_MOONS.len() + JOVIAN_MOONS.len() + 2
/// );
///
/// // Earth is stored at its true 0.0064 units. Nothing here is exaggerated; the renderer
/// // floors *apparent* size instead, so a body this small is still a visible dot.
/// let earth = &experiment.scene.props[4];
/// match &earth.body {
///     physics_sandbox::Body::Sphere { radius, .. } => {
///         assert!((radius - 0.006371).abs() < 1.0e-6)
///     }
///     other => panic!("expected a sphere, got {other:?}"),
/// }
/// ```
pub fn solar_system() -> Experiment {
    let mut scene = Scene::demo();

    // Small enough to be invisible at every separation in the scene, which the moons require:
    // the crate default of `1.0` unit leaves only `15%` of Newton's force at Io's `0.4217`-unit
    // orbit. See `SOLAR_SYSTEM_SOFTENING_UNITS`.
    scene.softening_units = SOLAR_SYSTEM_SOFTENING_UNITS;

    // The Sun goes in first, so its index is known and stable: prop 1, after the floor.
    // Its velocity is filled in at the end, once the planets' momentum is known.
    let mut names = BodyNames::default();
    let sun = scene.add(
        Prop::at_rest(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS),
                    rotation: Quat::IDENTITY,
                },
                radius: radius_units(SOLAR_SYSTEM_SUN_RADIUS_M),
                color: SOLAR_SYSTEM_SUN_COLOR,
            },
            SOLAR_SYSTEM_SUN_MASS_KG,
        )
        // One solar luminosity, by definition — this *is* the Sun. The render layer turns it into a
        // point light at the Sun's own position with inverse-square falloff, so a planet's
        // terminator is where the Sun actually puts it, and Neptune receives the `1/900` of Earth's
        // light that it really does.
        .with_luminosity(1.0),
    );
    names.insert(sun, "Sun");

    // Momentum of the planets, accumulated in `f64` and in kg·units/s. Only X and Z are
    // needed: every orbit is in the ecliptic plane, so the Y component is exactly zero by
    // construction and carrying it would only invite floating-point dust.
    let mut momentum_x = 0.0_f64;
    let mut momentum_z = 0.0_f64;

    let mut outermost_units = radius_units(SOLAR_SYSTEM_SUN_RADIUS_M);

    for planet in SOLAR_SYSTEM_PLANETS {
        let orbit_units = planet.semi_major_axis_m / METRES_PER_UNIT;
        let body_radius = radius_units(planet.radius_m);
        let position = ecliptic_position(planet.mean_longitude_deg, orbit_units);

        // Exactly tangential: perpendicular to the radius and to the ecliptic normal,
        // which is what `cross` gives. Taken in the sense that makes longitude increase
        // — see `ecliptic_position`.
        let radial =
            (position - DVec3::Y * f64::from(SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS)) / orbit_units;
        let tangent = DVec3::Y.cross(radial).normalize();
        let speed =
            circular_orbit_speed_units_s(planet.semi_major_axis_m, SOLAR_SYSTEM_SUN_MASS_KG);
        let velocity = tangent * speed;

        momentum_x += planet.mass_kg * velocity.x;
        momentum_z += planet.mass_kg * velocity.z;

        outermost_units = outermost_units.max(orbit_units as f32 + body_radius);

        let id = scene.add(Prop::new(
            Body::Sphere {
                pose: Pose {
                    position,
                    rotation: Quat::IDENTITY,
                },
                radius: body_radius,
                color: planet.color,
            },
            velocity,
            planet.mass_kg as f32,
        ));
        names.insert(id, planet.name);
    }

    // The moons, one system at a time. Added after the planets so each primary's own state is known:
    // a moon's heliocentric velocity is **its primary's plus** its orbital velocity about that
    // primary, and omitting the first term would fling it out of the solar system rather than leaving
    // it in orbit.
    //
    // They are here at true scale with no compromise, which took several amendments to make possible.
    // Earth is `0.006371` units and its Moon orbits at `0.3844` — `60.3` Earth radii, the real figure —
    // and Jupiter is `0.0699` with Io at `0.4217`. Under the old `157×` drawn radius Earth's hull was
    // `2.6×` its Moon's whole orbit and Jupiter's was `26×` Io's, so every moon started inside its
    // planet.
    //
    // Cost: `substep_count` splits this frame about `98` ways, set by Io — the tightest orbit in the
    // scene. Earth's Moon asks for `6`, so it is free. That is `3.2 ms` a frame in release, a fifth of
    // a 60 Hz budget, for `900` integration steps per Io orbit.
    for system in MOON_SYSTEMS {
        let Some(primary_row) = SOLAR_SYSTEM_PLANETS
            .iter()
            .position(|planet| planet.name == system.primary)
        else {
            continue;
        };

        let primary = SOLAR_SYSTEM_PLANETS[primary_row];
        let primary_index = primary_row + FIRST_PLANET_PROP_INDEX;
        let primary_position = scene.props[primary_index].body.center();
        let primary_velocity = scene.props[primary_index].velocity_units_s;

        for moon in system.moons {
            let orbit_units = moon.semi_major_axis_m / METRES_PER_UNIT;
            let longitude = moon.mean_longitude_deg.to_radians();

            // Prograde about +Y, the same convention the planets use.
            let radial = DVec3::new(longitude.cos(), 0.0, -longitude.sin());
            let tangent = DVec3::Y.cross(radial).normalize();
            let orbital_speed =
                circular_orbit_speed_units_s(moon.semi_major_axis_m, primary.mass_kg as f32);

            let position = primary_position + radial * orbit_units;
            let velocity = primary_velocity + tangent * orbital_speed;

            momentum_x += moon.mass_kg * velocity.x;
            momentum_z += moon.mass_kg * velocity.z;

            let id = scene.add(Prop::new(
                Body::Sphere {
                    pose: Pose {
                        position,
                        rotation: Quat::IDENTITY,
                    },
                    radius: radius_units(moon.radius_m),
                    color: moon.color,
                },
                velocity,
                moon.mass_kg as f32,
            ));
            names.insert(id, moon.name);
        }
    }

    // The barycentric frame: give the Sun exactly the momentum that cancels the planets',
    // so the total is zero and the system does not drift out of frame. Index 1 is the Sun,
    // added above.
    let sun_mass = f64::from(SOLAR_SYSTEM_SUN_MASS_KG);
    scene.props[1].velocity_units_s =
        DVec3::new(-momentum_x / sun_mass, 0.0, -momentum_z / sun_mass);

    // Bounds that actually contain the system. Derived rather than declared, so adding a
    // body to the table widens them automatically. The vertical extent keeps the shared
    // floor slab below and clears the top of the Sun.
    scene.bounds = Aabb {
        min: Vec3::new(-outermost_units, DEMO_BOUNDS_MIN_UNITS.y, -outermost_units),
        max: Vec3::new(
            outermost_units,
            SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS + radius_units(SOLAR_SYSTEM_SUN_RADIUS_M),
            outermost_units,
        ),
    };

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::SolarSystem,
            seed: DEFAULT_RANDOM_SEED,
            time_scale: SOLAR_SYSTEM_TIME_SCALE,
            camera_position_units: SOLAR_SYSTEM_CAMERA_POSITION_UNITS,
            camera_target_units: Vec3::Y * SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS,
            framed_radius_units: SOLAR_SYSTEM_FRAMED_RADIUS_UNITS,
            trail_length_units: Some(SOLAR_SYSTEM_TRAIL_LENGTH_UNITS),
            launches: true,
        },
        names,
    }
}

#[cfg(test)]
#[path = "../../tst/experiments/solar_system_tests.rs"]
mod tests;
