//! `experiment_6` — Sagittarius A*, the black hole at the centre of the Milky Way, spinning, with
//! an accretion disc.
//!
//! The first experiment where relativity is the subject rather than a correction too small to
//! see. Everything in the solar system moves at about `1e-4 c`; here gas in the inner disc orbits at
//! more than half the speed of light, and inside the ergosphere nothing can stand still.
//!
//! # What it builds
//!
//! One body in empty space ([`Scene::empty`], no floor): Sgr A* at its real mass, `4.3e6 M☉`,
//! **spinning** at [`SGR_A_STAR_SPIN`] about the world's vertical axis. It is drawn at the
//! equatorial radius of its event horizon, [`spinning_horizon_units`]: `10.8` units, smaller than
//! the `12.7` a non-spinning hole of the same mass would have. Nothing about its size is stored; it
//! comes from the mass and spin through the relativity library
//! ([`crate::physics_utils::relativity::kerr`]).
//!
//! The scene runs under [`GravityModel::Kerr`]: anything placed with `B` falls under the exact
//! equation of motion of a spinning hole
//! ([`kerr_schild_acceleration`](crate::physics_utils::relativity::kerr::kerr_schild_acceleration)),
//! dragged round with the spin. In the Kerr–Schild time it uses, matter crosses the horizon in a
//! finite time, so the hole absorbs at its horizon.
//!
//! There is **no general light** ([`ExperimentSpec::general_lighting`] is off). The only thing
//! drawn besides the hole is **the accretion disc** ([`crate::render::accretion_disc`]): gas
//! orbiting in the equatorial plane at the exact Kerr rate, drifting inward, glowing at the
//! temperature its friction heats it to, Doppler-boosted on the side coming towards the camera, and
//! plunging once it crosses the ISCO. The hole, the disc, and a field of stars are ray-marched on the
//! GPU ([`crate::render::black_hole_view`]): every pixel's light is followed along its exact Kerr
//! path, so the far side appears arched over the hole, its underside below it, and the stars crowd
//! round the shadow. The light streams and the ergosphere outline were taken out so the disc is easy
//! to see; [`crate::render::light_streams`] is still in the library.
//!
//! # The radii of a spinning hole
//!
//! | Radius (Boyer–Lindquist) | `r_g` at `a = 0.9` | Drawn at | What happens there |
//! | --- | --- | --- | --- |
//! | Event horizon | `1.44` | `10.8` u (equatorial) | Nothing inside can get out. |
//! | Ergosphere (equator) | `2` | `13.9` u | Inside it, everything must turn with the hole. |
//! | Prograde photon orbit | `1.56` | | Light can circle with the spin here. |
//! | Prograde ISCO | `2.32` | `15.8` u | The disc's inner edge. |
//!
//! A Boyer–Lindquist radius `r` in the equatorial plane is a circle of radius `√(r² + a²)` in the
//! Cartesian coordinates the scene is drawn in, which is why the drawn sizes are a little larger
//! than `r` alone. [`black_hole_radii`] still gives the non-spinning radii for comparison.
//!
//! # What is still Newtonian
//!
//! Every pull that is not a black hole's, and the pull a placed body exerts back on the hole. The
//! integrator is first order, so a placed body's phase drifts over many orbits.
//!
//! # Capture, and why it differs from the `B` key
//!
//! The `B`-key Sgr A* ([`STELLAR_OBJECTS`](crate::experiments::constants::STELLAR_OBJECTS)) captures
//! at the **tidal disruption radius** of a Sun-like star, `113` units, because that is where a star
//! dies. This experiment captures at the **horizon**: the experiment is about the black hole itself,
//! and the horizon is the radius that belongs to the hole rather than to what falls in. The drawn
//! sphere has the horizon's equatorial radius; the horizon itself is flattened at the poles, to
//! `9.1` units, so a body arriving over a pole is captured a little before it reaches it.

pub use bevy::math::{Quat, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::integrator::kerr_schild_to_world;
#[cfg(test)]
use crate::core::scene::BodyId;
use crate::core::scene::{Aabb, Body, Color, DVec3, GravityModel, Pose, Prop, Scene};
use crate::experiments::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::experiments::constants::{
    ASTRONOMICAL_UNIT_M, BLACK_HOLE_BOUNDS_EXTENT_UNITS, BLACK_HOLE_CAMERA_OFFSET_UNITS,
    BLACK_HOLE_FRAMED_RADIUS_UNITS, BLACK_HOLE_SOFTENING_UNITS, BLACK_HOLE_TIME_SCALE,
    BLACK_HOLE_TRAIL_LENGTH_UNITS, DEFAULT_RANDOM_SEED, SGR_A_STAR_COLOR, SGR_A_STAR_MASS_KG,
    SGR_A_STAR_SPIN, SOLAR_MASS_KG, SOLAR_RADIUS_M, S_STARS, S_STAR_DISTANCE_PC, S_STAR_EPOCH_YR,
};
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::gravity::{
    innermost_stable_circular_orbit_radius, photon_sphere_radius, schwarzschild_radius,
};
use crate::physics_utils::relativity::kerr::{horizon_radius, KerrBlackHole};
use crate::time::constants::SECONDS_PER_JULIAN_YEAR;

/// The name `experiment_6`'s black hole is shown under.
pub const SGR_A_STAR_NAME: &str = "Sgr A*";

/// One of the S-stars: a real star on a measured orbit round Sgr A*.
///
/// The orbit is as fitted from decades of observation (Gillessen et al. 2017, *ApJ* 837, 30,
/// Table 3): its size and shape, and its orientation against the plane of the sky. `experiment_6`
/// keeps that orientation but brings the star in close, onto a circular orbit just outside the disc
/// ([`s_star_close_state`]). The star itself (mass, radius, luminosity, colour) is mostly *not*
/// measured; see [`S_STARS`] for what is assumed for each.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SStar {
    /// Its name in the S-star catalogue: `"S2"`.
    pub name: &'static str,
    /// Semi-major axis, in arcseconds as seen from Earth.
    pub semi_major_axis_arcsec: f64,
    /// Eccentricity.
    pub eccentricity: f64,
    /// Inclination to the plane of the sky, in degrees. Below `90°` the star goes round
    /// counter-clockwise on the sky (from north through east).
    pub inclination_deg: f64,
    /// Position angle of the ascending node, in degrees from north through east.
    pub ascending_node_deg: f64,
    /// Argument of pericentre, in degrees from the ascending node along the orbit.
    pub periapsis_deg: f64,
    /// When it last passed pericentre, as a decimal year.
    pub pericentre_yr: f64,
    /// Mass, in solar masses.
    pub mass_solar: f64,
    /// Radius, in solar radii.
    pub radius_solar: f64,
    /// Luminosity, in solar luminosities.
    pub luminosity_solar: f32,
    /// Linear-space colour.
    pub color: Color,
    /// The radius of the close, circular orbit `experiment_6` puts it on, in gravitational radii:
    /// just outside its tidal radius ([`tidal_radius_m`]).
    pub close_orbit_rg: f64,
}

/// How close to a hole of `mass_kg` a star can orbit before the hole's tides tear it apart: the
/// tidal disruption radius `R★ (M_hole / M★)^{1/3}`, in metres.
///
/// Inside it the difference between the hole's pull on the star's near and far sides exceeds the
/// star's own gravity holding it together. For a `10 M☉`, `5 R☉` B dwarf round Sgr A* that is
/// `2.6e11 m` (`263` units, `41 r_g`), twice the disc's radius; for a puffed-up giant it is several
/// times more.
pub fn tidal_radius_m(star: &SStar, mass_kg: f64) -> f64 {
    let star_mass = star.mass_solar * SOLAR_MASS_KG;
    star.radius_solar * SOLAR_RADIUS_M * (mass_kg / star_mass).cbrt()
}

/// Where `experiment_6` puts an S-star, and how fast it moves: Kerr–Schild axes, metres and metres
/// per second.
///
/// In the plane of its measured orbit, in the direction from the hole its measured orbit puts it on
/// `epoch_yr` ([`s_star_state`]), but on a **circle** of [`SStar::close_orbit_rg`] gravitational
/// radii, just outside the disc and just outside the star's tidal radius, going round the same
/// way. The speed is the circular one, `√(GM/r)`: the coordinate speed of a circular orbit round a
/// non-spinning hole in these coordinates, whose time differs from the Schwarzschild time by a
/// function of `r` alone. The spin changes it by `a/r^{3/2}`, under `0.3%` this far out, and twists
/// the tilted orbits slowly round the spin axis instead (Lense–Thirring precession), which the
/// scene's Kerr gravity then shows.
///
/// Its real orbit is far larger (S2's never comes within `120` AU, `18,000` units); this keeps each
/// star's orientation, so they lie north and south of the equator as the real ones do, at the
/// distance asked for.
pub fn s_star_close_state(star: &SStar, mass_kg: f64, epoch_yr: f64) -> (DVec3, DVec3) {
    let (position, velocity) = s_star_state(star, mass_kg, epoch_yr);
    let out = position.normalize();
    let normal = position.cross(velocity).normalize();
    let mu = GRAVITATIONAL_CONSTANT * mass_kg;
    let r_g = mu / (SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S);
    let radius = star.close_orbit_rg * r_g;
    (out * radius, normal.cross(out) * (mu / radius).sqrt())
}

/// Where an S-star is at `epoch_yr`, and how fast it moves, round a hole of `mass_kg`: Kerr–Schild
/// axes (the hole's spin along `+z`), metres and metres per second.
///
/// Its orbit is a Kepler ellipse of semi-major axis `a″ · R0` (an arcsecond at
/// [`S_STAR_DISTANCE_PC`] parsecs is that many AU), timed by Kepler's third law for *this* hole's
/// mass, so the star starts on the orbit the scene's gravity will keep it on. The position comes
/// from the mean anomaly since [`SStar::pericentre_yr`] by Newton's method on Kepler's equation,
/// turned onto the sky by the Thiele–Innes constants.
///
/// # How the sky maps onto the hole
///
/// The hole's spin is not measured well enough to place it against the sky. Here the plane of the
/// sky is taken as the hole's equatorial plane, with the spin pointing towards Earth (the Event
/// Horizon Telescope's 2022 images of Sgr A* favour a spin axis close to the line of sight). Then
/// Kerr–Schild `x` is north, `y` east, and `z` towards Earth, a right-handed set: an orbit with
/// `i < 90°` goes round with the spin, one with `i > 90°` against it, and each star is north of the
/// equator (towards Earth) on part of its orbit and south of it on the rest.
pub fn s_star_state(star: &SStar, mass_kg: f64, epoch_yr: f64) -> (DVec3, DVec3) {
    let a = star.semi_major_axis_arcsec * S_STAR_DISTANCE_PC * ASTRONOMICAL_UNIT_M;
    let e = star.eccentricity;
    let mu = GRAVITATIONAL_CONSTANT * mass_kg;
    let n = (mu / (a * a * a)).sqrt();
    let mean = (n * (epoch_yr - star.pericentre_yr) * SECONDS_PER_JULIAN_YEAR)
        .rem_euclid(std::f64::consts::TAU);

    // Kepler's equation, M = E − e sin E.
    let mut anomaly = if e < 0.8 { mean } else { std::f64::consts::PI };
    for _ in 0..50 {
        let step = (anomaly - e * anomaly.sin() - mean) / (1.0 - e * anomaly.cos());
        anomaly -= step;
        if step.abs() < 1.0e-14 {
            break;
        }
    }
    let (sin_e, cos_e) = anomaly.sin_cos();
    let root = (1.0 - e * e).sqrt();
    let rate = n / (1.0 - e * cos_e);
    // In the orbit's own plane: x towards pericentre, y ninety degrees on along the motion.
    let (x, y) = (a * (cos_e - e), a * root * sin_e);
    let (vx, vy) = (-a * rate * sin_e, a * rate * root * cos_e);

    // Thiele–Innes: onto north, east, and away from Earth.
    let (i, node, peri) = (
        star.inclination_deg.to_radians(),
        star.ascending_node_deg.to_radians(),
        star.periapsis_deg.to_radians(),
    );
    let (si, ci) = i.sin_cos();
    let (sn, cn) = node.sin_cos();
    let (sp, cp) = peri.sin_cos();
    let north = DVec3::new(cp * cn - sp * sn * ci, -sp * cn - cp * sn * ci, 0.0);
    let east = DVec3::new(cp * sn + sp * cn * ci, -sp * sn + cp * cn * ci, 0.0);
    let away = DVec3::new(sp * si, cp * si, 0.0);
    let onto = |u: f64, w: f64| {
        DVec3::new(
            north.x * u + north.y * w,
            east.x * u + east.y * w,
            // Kerr–Schild z is towards Earth.
            -(away.x * u + away.y * w),
        )
    };
    (onto(x, y), onto(vx, vy))
}

/// The three radii of a **non-rotating** black hole, in world units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BlackHoleRadii {
    /// The event horizon, `r_s = 2GM/c²`.
    pub horizon_units: f32,
    /// The photon sphere, `1.5·r_s`.
    pub photon_sphere_units: f32,
    /// The innermost stable circular orbit, `3·r_s`.
    pub isco_units: f32,
}

/// The horizon, photon sphere, and ISCO of a non-rotating black hole of `mass_kg`, in world units.
///
/// Computed with the relativity library and converted at [`METRES_PER_UNIT`]. For a spinning hole
/// see [`spinning_horizon_units`] and [`crate::physics_utils::relativity::kerr`].
///
/// # Returns
///
/// `None` if the mass is not positive and finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{black_hole_radii, SGR_A_STAR_MASS_KG};
///
/// let radii = black_hole_radii(SGR_A_STAR_MASS_KG).unwrap();
///
/// assert!((radii.horizon_units - 12.69).abs() < 0.01);
/// assert!((radii.photon_sphere_units - 19.04).abs() < 0.01);
/// assert!((radii.isco_units - 38.08).abs() < 0.01);
/// ```
pub fn black_hole_radii(mass_kg: f64) -> Option<BlackHoleRadii> {
    let units = |metres: f64| (metres / METRES_PER_UNIT) as f32;

    Some(BlackHoleRadii {
        horizon_units: units(schwarzschild_radius(mass_kg)?),
        photon_sphere_units: units(photon_sphere_radius(mass_kg)?),
        isco_units: units(innermost_stable_circular_orbit_radius(mass_kg)?),
    })
}

/// The equatorial radius of a spinning black hole's horizon as drawn, in world units:
/// `√(r₊² + a²) r_g`, the Cartesian size of the circle where the horizon meets the equatorial plane.
///
/// # Returns
///
/// `None` for a mass that is not positive and finite or a spin outside `[0, 1)`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{black_hole_radii, spinning_horizon_units, SGR_A_STAR_MASS_KG};
///
/// // Without spin it is the Schwarzschild radius.
/// let still = spinning_horizon_units(SGR_A_STAR_MASS_KG, 0.0).unwrap();
/// assert!((still / black_hole_radii(SGR_A_STAR_MASS_KG).unwrap().horizon_units - 1.0).abs() < 1.0e-6);
///
/// // Spinning at 0.9 it is smaller: 10.8 units against 12.7.
/// let spinning = spinning_horizon_units(SGR_A_STAR_MASS_KG, 0.9).unwrap();
/// assert!((spinning - 10.76).abs() < 0.01);
/// ```
pub fn spinning_horizon_units(mass_kg: f64, spin: f64) -> Option<f32> {
    let hole = KerrBlackHole::new(mass_kg, spin)?;
    let r_plus = horizon_radius(spin)?;
    let radius_rg = (r_plus * r_plus + spin * spin).sqrt();
    Some((radius_rg * hole.gravitational_radius_m() / METRES_PER_UNIT) as f32)
}

/// Where `experiment_6` puts the black hole's centre, in world units: the origin. With no floor
/// there is nothing to be above.
#[inline]
pub fn black_hole_centre_units() -> Vec3 {
    Vec3::ZERO
}

/// `experiment_6` — Sgr A*, spinning, alone in empty space.
///
/// Deterministic: the seed plays no part. Recorded as [`DEFAULT_RANDOM_SEED`] so the overlay prints
/// something honest.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{black_hole, GravityModel, SGR_A_STAR_SPIN};
///
/// let experiment = black_hole();
///
/// // The hole and the six S-stars: the disc is drawn by the render layer.
/// assert_eq!(experiment.scene.props.len(), 7);
///
/// let hole = &experiment.scene.props[0];
/// assert!(hole.absorbs, "whatever falls in is gone");
/// assert!(!hole.immovable, "it has mass, and it can be pulled");
/// assert_eq!(experiment.scene.gravity_model, GravityModel::Kerr { spin: SGR_A_STAR_SPIN });
/// ```
pub fn black_hole() -> Experiment {
    let horizon = spinning_horizon_units(SGR_A_STAR_MASS_KG, SGR_A_STAR_SPIN)
        .expect("SGR_A_STAR_MASS_KG and SGR_A_STAR_SPIN are a valid hole");
    let centre = black_hole_centre_units();

    let extent = BLACK_HOLE_BOUNDS_EXTENT_UNITS;
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-extent),
        max: Vec3::splat(extent),
    });
    scene.softening_units = BLACK_HOLE_SOFTENING_UNITS;
    scene.gravity_model = GravityModel::Kerr {
        spin: SGR_A_STAR_SPIN,
    };

    // At rest. Drawn, and capturing, at the horizon's equatorial radius.
    let hole_id = scene.add(Prop::absorbing(
        Body::Sphere {
            pose: Pose {
                position: centre.as_dvec3(),
                rotation: Quat::IDENTITY,
            },
            radius: horizon,
            color: SGR_A_STAR_COLOR,
        },
        DVec3::ZERO,
        SGR_A_STAR_MASS_KG as f32,
    ));

    let mut names = BodyNames::default();
    names.insert(hole_id, SGR_A_STAR_NAME);

    // The S-stars, brought in close: their measured orbits' planes, on circles just outside the
    // disc, moving under the hole's Kerr gravity from here on.
    for star in &S_STARS {
        let (position, velocity) = s_star_close_state(star, SGR_A_STAR_MASS_KG, S_STAR_EPOCH_YR);
        let to_world = |v: DVec3| kerr_schild_to_world(v.to_array()) / METRES_PER_UNIT;
        let id = scene.add(
            Prop::new(
                Body::Sphere {
                    pose: Pose {
                        position: centre.as_dvec3() + to_world(position),
                        rotation: Quat::IDENTITY,
                    },
                    radius: (star.radius_solar * SOLAR_RADIUS_M / METRES_PER_UNIT) as f32,
                    color: star.color,
                },
                to_world(velocity),
                (star.mass_solar * SOLAR_MASS_KG) as f32,
            )
            .with_luminosity(star.luminosity_solar),
        );
        names.insert(id, star.name);
    }

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::BlackHole,
            seed: DEFAULT_RANDOM_SEED,
            time_scale: BLACK_HOLE_TIME_SCALE,
            camera_position_units: centre + BLACK_HOLE_CAMERA_OFFSET_UNITS,
            camera_target_units: centre,
            framed_radius_units: BLACK_HOLE_FRAMED_RADIUS_UNITS,
            trail_length_units: Some(BLACK_HOLE_TRAIL_LENGTH_UNITS),
            // The fleet launches from Earth, and there is no Earth here.
            launches: false,
            // Lit by nothing but its own disc.
            general_lighting: false,
        },
        names,
    }
}

#[cfg(test)]
#[path = "../../tst/experiments/black_hole_tests.rs"]
mod tests;
