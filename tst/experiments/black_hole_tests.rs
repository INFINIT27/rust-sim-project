//! Unit tests for the [`crate::experiments::black_hole`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/experiments/black_hole.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! # What these tests are for
//!
//! 1. **The hole is Sgr A* at its true size, spinning.** The mass is the published one, the drawn
//!    radius is the spinning horizon computed by the relativity library, and the `B`-key Sgr A* is
//!    the same mass.
//! 2. **Motion in the scene is that of a spinning hole.** An orbit just outside the prograde ISCO
//!    holds, where the same release around a non-spinning hole plunges; just inside the ISCO it
//!    plunges; and a dropped rock is swallowed at the horizon without approaching `c`.
//! 3. **The scene is the hole alone,** framed for the disc the render layer draws round it
//!    (tested in `tst/render/accretion_disc_tests.rs` and `tst/render/light_streams_tests.rs`).

use super::*;
use crate::core::integrator::kerr_schild_to_world;
use crate::core::scene::Color;
use crate::experiments::catalog::ExperimentId;
use crate::experiments::constants::{BLACK_HOLE_TIME_SCALE, STELLAR_OBJECTS};
use crate::experiments::objects::stellar_object;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::kerr::{circular_orbit, equatorial_point, isco_radius};
use crate::time::constants::MAX_REAL_STEP_SECONDS;
use crate::time::simulation::SimulationClock;

/// Index of the hole in the built scene: the only body.
const HOLE_INDEX: usize = 0;

/// The solar mass the catalog quotes masses against.
const SOLAR_MASS_KG: f64 = 1.988_47e30;

fn hole(experiment: &Experiment) -> &Prop {
    &experiment.scene.props[HOLE_INDEX]
}

fn horizon_units(experiment: &Experiment) -> f32 {
    match hole(experiment).body {
        Body::Sphere { radius, .. } => radius,
        ref other => panic!("the hole should be a sphere, got {other:?}"),
    }
}

fn kerr() -> KerrBlackHole {
    KerrBlackHole::new(SGR_A_STAR_MASS_KG, SGR_A_STAR_SPIN).expect("valid")
}

// -----------------------------------------------------------------------
// The object.
// -----------------------------------------------------------------------

#[test]
fn the_hole_has_sgr_a_stars_published_mass() {
    let solar_masses = SGR_A_STAR_MASS_KG / SOLAR_MASS_KG;
    assert!(
        (solar_masses / 1.0e6 - 4.30).abs() < 0.01,
        "{solar_masses:e} solar masses"
    );

    let experiment = black_hole();
    let stored = f64::from(hole(&experiment).mass_kg);
    assert!(
        (stored / SGR_A_STAR_MASS_KG - 1.0).abs() < 1.0e-6,
        "f32 keeps it to 7 digits"
    );
}

#[test]
fn the_non_spinning_radii_are_the_published_ones() {
    // Horizon 12.7 million km, photon sphere 19.0, ISCO 38.1: in world units, millions of km.
    let radii = black_hole_radii(SGR_A_STAR_MASS_KG).expect("valid mass");
    assert!((radii.horizon_units - 12.69).abs() < 0.01);
    assert!((radii.photon_sphere_units - 19.04).abs() < 0.01);
    assert!((radii.isco_units - 38.08).abs() < 0.01);
}

#[test]
fn the_hole_spins_and_is_drawn_at_its_smaller_spinning_horizon() {
    let experiment = black_hole();
    assert_eq!(
        experiment.scene.gravity_model,
        GravityModel::Kerr {
            spin: SGR_A_STAR_SPIN
        }
    );
    const { assert!(SGR_A_STAR_SPIN > 0.0 && SGR_A_STAR_SPIN < 1.0) };

    // √(r₊² + a²) r_g, written out here: r₊ = 1 + √(1 − a²).
    let a = SGR_A_STAR_SPIN;
    let r_plus = 1.0 + (1.0 - a * a).sqrt();
    let expected = ((r_plus * r_plus + a * a).sqrt() * kerr().gravitational_radius_m()
        / crate::constants::METRES_PER_UNIT) as f32;
    assert!((horizon_units(&experiment) / expected - 1.0).abs() < 1.0e-6);

    let still = black_hole_radii(SGR_A_STAR_MASS_KG).unwrap().horizon_units;
    assert!(
        horizon_units(&experiment) < still,
        "spin shrinks the horizon"
    );
}

#[test]
fn the_b_key_sgr_a_star_has_the_same_mass() {
    let row = stellar_object(SGR_A_STAR_NAME).expect("the catalog has Sgr A*");
    assert_eq!(row.mass_kg, SGR_A_STAR_MASS_KG);
    assert!(STELLAR_OBJECTS
        .iter()
        .any(|object| object.name == SGR_A_STAR_NAME));
}

#[test]
fn bad_holes_have_no_radii() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(black_hole_radii(bad), None);
        assert_eq!(spinning_horizon_units(bad, 0.5), None);
    }
    for bad in [-0.1, 1.0, f64::NAN] {
        assert_eq!(spinning_horizon_units(SGR_A_STAR_MASS_KG, bad), None);
    }
}

// -----------------------------------------------------------------------
// The scene.
// -----------------------------------------------------------------------

#[test]
fn the_scene_is_the_hole_and_the_s_stars_with_no_floor() {
    let experiment = black_hole();
    assert_eq!(experiment.scene.props.len(), 1 + S_STARS.len());
    for (k, star) in S_STARS.iter().enumerate() {
        let id = BodyId(k as u64 + 1);
        assert_eq!(experiment.names.label(id), star.name);
        let prop = experiment.scene.get(id).unwrap();
        assert!(prop.is_valid() && prop.is_luminous() && !prop.absorbs);
    }

    let hole = hole(&experiment);
    assert!(hole.is_valid());
    assert!(hole.absorbs);
    assert!(!hole.immovable);
    assert_eq!(hole.velocity_units_s, DVec3::ZERO);
    assert_eq!(hole.luminosity_solar, 0.0, "a black hole emits nothing");
    assert_eq!(
        experiment.names.label(BodyId(HOLE_INDEX as u64)),
        SGR_A_STAR_NAME
    );
}

#[test]
fn capture_is_at_the_horizon() {
    // In Kerr–Schild time matter crosses the horizon, so nothing needs stopping short of it.
    let experiment = black_hole();
    assert_eq!(hole(&experiment).contact_radius_units, None);
}

#[test]
fn the_hole_sits_at_the_origin_inside_its_bounds() {
    let experiment = black_hole();
    assert_eq!(hole(&experiment).body.center(), DVec3::ZERO);
    assert_eq!(black_hole_centre_units(), Vec3::ZERO);

    let bounds = experiment.scene.bounds;
    assert!(bounds.is_valid());
    // The bounds take in the whole disc, 127 units out.
    assert!(bounds.min.max_element() < -127.0 && bounds.max.min_element() > 127.0);
}

#[test]
fn the_softening_is_negligible_at_the_horizon() {
    let experiment = black_hole();
    let epsilon = experiment.scene.softening_units;
    let r = horizon_units(&experiment);
    let fraction = r * r / (r * r + epsilon * epsilon);
    assert!(
        fraction > 0.9999,
        "{fraction} of Newton's pull at the horizon"
    );
}

#[test]
fn the_opening_view_frames_the_disc() {
    // The disc reaches 20 r_g in Boyer–Lindquist radius: sqrt(20^2 + a^2) r_g = 127 units.
    let experiment = black_hole();
    let r_g = black_hole_radii(SGR_A_STAR_MASS_KG).unwrap().horizon_units / 2.0;
    let disc = (400.0_f32 + 0.81).sqrt() * r_g;

    assert_eq!(experiment.spec.id, ExperimentId::BlackHole);
    assert_eq!(
        experiment.spec.camera_target_units,
        black_hole_centre_units()
    );
    assert!(experiment.spec.framed_radius_units >= disc);
    assert!(
        experiment.spec.framed_radius_units < 1.1 * disc,
        "framed tightly round the disc"
    );
    assert!(experiment.spec.is_valid());
    assert!(
        !experiment.spec.launches,
        "the fleet launches from Earth, and there is none here"
    );
    assert!(
        !experiment.spec.general_lighting,
        "lit by nothing but its disc"
    );

    // Low enough to see light bend over and under the hole, high enough to see the disc's face.
    let offset = experiment.spec.camera_position_units - experiment.spec.camera_target_units;
    let elevation = (offset.y / offset.length()).asin().to_degrees();
    assert!(
        (5.0..15.0).contains(&elevation),
        "camera {elevation} degrees above the disc"
    );
}

#[test]
fn the_experiment_is_reachable_by_number_and_by_name() {
    for token in ["experiment_6", "6", "black-hole", "Black_Hole", "sgr-a"] {
        assert!(ExperimentId::BlackHole.matches(token), "{token}");
    }
}

// -----------------------------------------------------------------------
// Running it.
// -----------------------------------------------------------------------

/// Step `scene` for up to `frames` worst-case frames at the experiment's clock, calling `observe`
/// after each. Stops early when `observe` returns `false`. Returns the frames run.
fn run(scene: &mut Scene, frames: usize, mut observe: impl FnMut(&Scene) -> bool) -> usize {
    let mut clock = SimulationClock::new(BLACK_HOLE_TIME_SCALE);
    for frame in 0..frames {
        scene.step(clock.advance(MAX_REAL_STEP_SECONDS));
        if !observe(scene) {
            return frame + 1;
        }
    }
    frames
}

/// `experiment_6`'s scene with the S-stars taken out: the hole alone.
fn hole_alone() -> Scene {
    let mut scene = black_hole().scene;
    for k in 1..=S_STARS.len() {
        scene.remove(BodyId(k as u64));
    }
    scene
}

/// The hole alone, with a body of 1e20 kg released at world `offset_units` moving at
/// `velocity_units_s`, and that body's id.
fn hole_and_body(
    offset_units: DVec3,
    velocity_units_s: DVec3,
    model: GravityModel,
) -> (Scene, BodyId) {
    let mut scene = hole_alone();
    scene.gravity_model = model;
    let id = scene.add(Prop::new(
        Body::Sphere {
            pose: Pose {
                position: black_hole_centre_units().as_dvec3() + offset_units,
                rotation: Quat::IDENTITY,
            },
            radius: 0.1,
            color: Color {
                r: 0.8,
                g: 0.6,
                b: 0.4,
                a: 1.0,
            },
        },
        velocity_units_s,
        1.0e20,
    ));
    (scene, id)
}

/// A prograde circular equatorial orbit of the spinning hole at Boyer–Lindquist radius `r_rg`,
/// with its speed scaled by `speed_factor`, as a world offset and velocity.
fn prograde_release(r_rg: f64, speed_factor: f64) -> (DVec3, DVec3) {
    let hole = kerr();
    let (r_g, t_g) = (hole.gravitational_radius_m(), hole.gravitational_time_s());
    let a = SGR_A_STAR_SPIN;
    let omega = circular_orbit(a, r_rg, true).unwrap().angular_velocity / t_g;
    let position = equatorial_point(a, r_rg, 0.0).map(|c| c * r_g);
    let velocity = [
        a * omega * r_g * speed_factor,
        r_rg * omega * r_g * speed_factor,
        0.0,
    ];
    let units = crate::constants::METRES_PER_UNIT;
    (
        kerr_schild_to_world(position) / units,
        kerr_schild_to_world(velocity) / units,
    )
}

/// The released body's nearest approach to the hole over `frames` frames, or `None` if it was
/// swallowed.
fn nearest_approach(
    offset: DVec3,
    velocity: DVec3,
    model: GravityModel,
    frames: usize,
) -> Option<f64> {
    let (mut scene, body) = hole_and_body(offset, velocity, model);
    let mut nearest = f64::INFINITY;
    let mut swallowed = false;
    run(&mut scene, frames, |scene| {
        match (scene.get(BodyId(0)), scene.get(body)) {
            (Some(hole), Some(prop)) => {
                nearest = nearest.min((prop.body.center() - hole.body.center()).length());
                true
            }
            _ => {
                swallowed = true;
                false
            }
        }
    });
    (!swallowed).then_some(nearest)
}

#[test]
fn a_hole_with_nothing_near_it_stays_put() {
    let mut scene = hole_alone();
    let start = scene.props[HOLE_INDEX].body.center();
    run(&mut scene, 400, |_| true);

    assert_eq!(scene.props.len(), 1);
    assert_eq!(scene.props[HOLE_INDEX].body.center(), start);
    assert_eq!(scene.props[HOLE_INDEX].velocity_units_s, DVec3::ZERO);
}

#[test]
fn spin_lets_an_orbit_hold_where_a_still_hole_would_swallow_it() {
    // At 1.2 times the prograde ISCO (2.8 r_g, 17.8 units out) a spinning hole has stable circular
    // orbits. A non-spinning hole has none inside 6 r_g, and nothing at all inside its 3 r_g photon
    // sphere: the same release falls in.
    let isco = isco_radius(SGR_A_STAR_SPIN, true).unwrap();
    let (offset, velocity) = prograde_release(1.2 * isco, 1.0);
    let start = offset.length();

    let nearest = nearest_approach(
        offset,
        velocity,
        GravityModel::Kerr {
            spin: SGR_A_STAR_SPIN,
        },
        300,
    )
    .expect("held its orbit round the spinning hole");
    assert!(nearest > 0.9 * start, "dipped to {nearest} from {start}");

    assert_eq!(
        nearest_approach(offset, velocity, GravityModel::Schwarzschild, 300),
        None,
        "the still hole swallows it"
    );
}

#[test]
fn inside_the_prograde_isco_an_orbit_plunges() {
    let isco = isco_radius(SGR_A_STAR_SPIN, true).unwrap();
    let (offset, velocity) = prograde_release(0.9 * isco, 0.998);
    assert_eq!(
        nearest_approach(
            offset,
            velocity,
            GravityModel::Kerr {
                spin: SGR_A_STAR_SPIN
            },
            1_000
        ),
        None
    );
}

/// The fastest a rock dropped from rest 30 units out goes before it is swallowed, as a fraction of
/// `c`, and the frames taken.
fn fall(model: GravityModel) -> (f64, usize) {
    let (mut scene, rock) = hole_and_body(DVec3::new(30.0, 0.0, 0.0), DVec3::ZERO, model);
    let mut fastest = 0.0_f64;
    let frames = run(&mut scene, 1_000, |scene| match scene.get(rock) {
        Some(prop) => {
            fastest = fastest.max(prop.speed_m_s() / SPEED_OF_LIGHT_M_S);
            true
        }
        None => false,
    });
    (fastest, frames)
}

#[test]
fn a_dropped_rock_is_swallowed_and_never_reaches_light_speed() {
    let (relativistic, frames) = fall(GravityModel::Kerr {
        spin: SGR_A_STAR_SPIN,
    });
    assert!(frames < 1_000, "never swallowed");
    assert!(relativistic < 1.0, "peaked at {relativistic}c");

    let (newtonian, _) = fall(GravityModel::Newtonian);
    assert!(
        newtonian > relativistic,
        "Newton {newtonian}c against {relativistic}c"
    );
}

#[test]
fn newtonian_gravity_reaches_light_speed_at_the_horizon() {
    // Why the relativistic equation is needed: the Newtonian escape speed sqrt(2GM/r) is exactly
    // c at r = 2GM/c^2, so a Newtonian body falling from far away arrives at the horizon at c.
    let horizon_m = f64::from(black_hole_radii(SGR_A_STAR_MASS_KG).unwrap().horizon_units)
        * crate::constants::METRES_PER_UNIT;
    let escape = (2.0 * GRAVITATIONAL_CONSTANT * SGR_A_STAR_MASS_KG / horizon_m).sqrt();
    assert!((escape / SPEED_OF_LIGHT_M_S - 1.0).abs() < 1.0e-6);
}

// -----------------------------------------------------------------------
// The S-stars.
// -----------------------------------------------------------------------

fn star(name: &str) -> SStar {
    *S_STARS.iter().find(|s| s.name == name).unwrap()
}

fn au(metres: f64) -> f64 {
    metres / ASTRONOMICAL_UNIT_M
}

#[test]
fn s2_comes_within_120_au_at_its_pericentre_and_goes_out_to_1970() {
    // a = 0.1255" at 8,323 pc is 1,045 AU; e = 0.884 puts pericentre at 121 AU (GRAVITY measured
    // 120 AU at the 2018 passage) and apocentre at 1,968 AU.
    let s2 = star("S2");
    let (at_pericentre, speed) = {
        let (r, v) = s_star_state(&s2, SGR_A_STAR_MASS_KG, s2.pericentre_yr);
        (au(r.length()), v.length())
    };
    assert!((at_pericentre - 121.2).abs() < 0.5, "{at_pericentre} AU");
    // About 7,700 km/s there, 2.6% of light.
    assert!((speed / 7.65e6 - 1.0).abs() < 0.02, "{speed} m/s");
    let half_period = 8.0;
    let (r, _) = s_star_state(&s2, SGR_A_STAR_MASS_KG, s2.pericentre_yr + half_period);
    assert!(
        (au(r.length()) - 1968.0).abs() < 15.0,
        "{} AU at apocentre",
        au(r.length())
    );
}

#[test]
fn an_s_star_comes_back_after_its_period() {
    // Kepler's third law for the scene's hole gives each star the period it was observed to have.
    for (name, observed) in [("S2", 16.00), ("S13", 49.00), ("S38", 19.2), ("S55", 12.80)] {
        let s = star(name);
        let a = s.semi_major_axis_arcsec * S_STAR_DISTANCE_PC * ASTRONOMICAL_UNIT_M;
        let mu =
            crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT * SGR_A_STAR_MASS_KG;
        let period = std::f64::consts::TAU * (a * a * a / mu).sqrt()
            / crate::time::constants::SECONDS_PER_JULIAN_YEAR;
        assert!(
            (period / observed - 1.0).abs() < 0.02,
            "{name}: {period} yr vs {observed}"
        );
        let (start, _) = s_star_state(&s, SGR_A_STAR_MASS_KG, 2020.0);
        let (again, _) = s_star_state(&s, SGR_A_STAR_MASS_KG, 2020.0 + period);
        assert!((start - again).length() < 1.0e-6 * start.length(), "{name}");
    }
}

#[test]
fn each_orbit_is_tilted_to_the_equator_by_its_inclination() {
    // The sky is the hole's equatorial plane, so an orbit's angular momentum makes its inclination
    // with the spin axis: below 90° it goes round with the spin, above it against.
    for s in &S_STARS {
        let (r, v) = s_star_state(s, SGR_A_STAR_MASS_KG, S_STAR_EPOCH_YR);
        let tilt = r
            .cross(v)
            .normalize()
            .z
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        assert!(
            (tilt - s.inclination_deg).abs() < 1.0e-6,
            "{}: {tilt}°",
            s.name
        );
    }
}
#[test]
fn on_its_real_orbit_s14_dives_closest_and_fastest() {
    let s14 = star("S14");
    let (r, v) = s_star_state(&s14, SGR_A_STAR_MASS_KG, s14.pericentre_yr);
    assert!((au(r.length()) - 57.0).abs() < 1.5, "{} AU", au(r.length()));
    let fraction = v.length() / SPEED_OF_LIGHT_M_S;
    assert!((0.035..0.042).contains(&fraction), "{fraction} c");
}

/// `r_g` of Sgr A*, in metres.
fn r_g_m() -> f64 {
    kerr().gravitational_radius_m()
}

#[test]
fn a_b_dwarf_is_torn_apart_inside_about_41_gravitational_radii() {
    // R★ (M/m)^(1/3): 5 R☉ × (4.3e6 / 10)^(1/3) = 2.6e11 m.
    let b_dwarf = star("S13");
    let tidal = tidal_radius_m(&b_dwarf, SGR_A_STAR_MASS_KG);
    assert!((tidal / 2.63e11 - 1.0).abs() < 0.01, "{tidal} m");
    assert!((tidal / r_g_m() - 41.4).abs() < 0.3);
    // A giant, ten times the size for a fifth of the mass, three times further.
    let giant = tidal_radius_m(&star("S38"), SGR_A_STAR_MASS_KG);
    assert!(giant > 3.0 * tidal);
}

#[test]
fn each_star_orbits_just_outside_the_disc_and_its_tidal_radius() {
    let disc_edge = f64::from(crate::render::constants::DISC_OUTER_RADIUS_RG);
    let mut radii = Vec::new();
    for s in &S_STARS {
        let tidal_rg = tidal_radius_m(s, SGR_A_STAR_MASS_KG) / r_g_m();
        assert!(s.close_orbit_rg > disc_edge, "{} inside the disc", s.name);
        assert!(
            (1.1..1.35).contains(&(s.close_orbit_rg / tidal_rg)),
            "{}: {} r_g against a tidal radius of {tidal_rg}",
            s.name,
            s.close_orbit_rg
        );
        radii.push(s.close_orbit_rg);
    }
    radii.sort_by(f64::total_cmp);
    assert!(
        radii.windows(2).all(|pair| pair[1] - pair[0] >= 3.0),
        "no two share a radius: {radii:?}"
    );
    // All but the giant within three disc radii.
    assert!(radii[radii.len() - 2] < 3.0 * disc_edge);
}

#[test]
fn each_star_keeps_its_real_orbit_s_plane_and_place_on_a_circle() {
    let mu = GRAVITATIONAL_CONSTANT * SGR_A_STAR_MASS_KG;
    for s in &S_STARS {
        let (real_r, real_v) = s_star_state(s, SGR_A_STAR_MASS_KG, S_STAR_EPOCH_YR);
        let (r, v) = s_star_close_state(s, SGR_A_STAR_MASS_KG, S_STAR_EPOCH_YR);
        // On its circle, at the circular speed, square to the radius.
        assert!((r.length() / (s.close_orbit_rg * r_g_m()) - 1.0).abs() < 1.0e-12);
        assert!((v.length() / (mu / r.length()).sqrt() - 1.0).abs() < 1.0e-12);
        assert!(r.dot(v).abs() < 1.0e-9 * r.length() * v.length());
        // In the same direction from the hole, going round the same way in the same plane.
        assert!(
            r.normalize().dot(real_r.normalize()) > 1.0 - 1.0e-12,
            "{}",
            s.name
        );
        let (h, real_h) = (r.cross(v).normalize(), real_r.cross(real_v).normalize());
        assert!(h.dot(real_h) > 1.0 - 1.0e-12, "{}", s.name);
    }
}

#[test]
fn the_s_stars_are_north_and_south_of_the_equator() {
    // The hole's spin is along world +y, so its equatorial plane is world y = 0.
    let experiment = black_hole();
    let centre = black_hole_centre_units().as_dvec3();
    let heights: Vec<f64> = (1..=S_STARS.len())
        .map(|k| {
            experiment
                .scene
                .get(BodyId(k as u64))
                .unwrap()
                .body
                .center()
                .y
                - centre.y
        })
        .collect();
    assert!(heights.iter().any(|&h| h > 20.0), "north: {heights:?}");
    assert!(heights.iter().any(|&h| h < -20.0), "south: {heights:?}");
}

#[test]
fn the_s_stars_start_on_their_close_orbits_in_the_scene() {
    // Placed at world position and velocity, the scene's star is where `s_star_close_state` says,
    // in Kerr–Schild axes swapped to the world's, well within the camera's reach.
    let experiment = black_hole();
    let centre = black_hole_centre_units().as_dvec3();
    let units = crate::constants::METRES_PER_UNIT;
    for (k, s) in S_STARS.iter().enumerate() {
        let prop = experiment.scene.get(BodyId(k as u64 + 1)).unwrap();
        let (r, v) = s_star_close_state(s, SGR_A_STAR_MASS_KG, S_STAR_EPOCH_YR);
        let expected = kerr_schild_to_world(r.to_array()) / units;
        assert!(((prop.body.center() - centre) - expected).length() < 1.0e-6 * expected.length());
        let expected_v = kerr_schild_to_world(v.to_array()) / units;
        assert!((prop.velocity_units_s - expected_v).length() < 1.0e-9 * expected_v.length());
        assert!(
            expected.length() < 0.1 * f64::from(crate::render::constants::CAMERA_FAR_UNITS),
            "{} at {} units",
            s.name,
            expected.length()
        );
    }
}

#[test]
fn the_s_stars_hold_their_close_orbits_under_the_scene_s_gravity() {
    // A tenth of S55's eight-hour orbit at the experiment's clock: every star still there, and each
    // still at its radius to within a percent (the hole's Kerr gravity twists the tilted orbits
    // round the spin slowly; it does not pull them in or fling them out).
    let mut experiment = black_hole();
    let centre = black_hole_centre_units().as_dvec3();
    let units = crate::constants::METRES_PER_UNIT;
    let mut clock = SimulationClock::new(BLACK_HOLE_TIME_SCALE);
    let mut elapsed = 0.0;
    while elapsed < 2900.0 {
        let step = clock.advance(MAX_REAL_STEP_SECONDS);
        elapsed += step.seconds();
        experiment.scene.step(step);
    }
    assert_eq!(experiment.scene.props.len(), 1 + S_STARS.len());
    for (k, s) in S_STARS.iter().enumerate() {
        let prop = experiment.scene.get(BodyId(k as u64 + 1)).unwrap();
        let radius_m = (prop.body.center() - centre).length() * units;
        let ratio = radius_m / (s.close_orbit_rg * r_g_m());
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "{}: radius {ratio} of its circle",
            s.name
        );
    }
}
