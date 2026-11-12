//! Unit tests for [`crate::render::accretion_disc`].
//!
//! Compiled into `src/render/accretion_disc.rs` via `#[path]`. The disc is drawn by the GPU
//! ray-marcher, tested in `black_hole_view_tests` and `kerr_ray_tests`; here, the streaks it splats
//! and the redshift it is checked against.
//!
//! The disc's physics (orbits, the Page–Thorne flux, the equation of motion) is tested in
//! `tst/physics_utils/relativity/kerr_tests.rs`. These tests cover the profile built from it, the
//! clumps' motion, the plunge and replacement, the redshift and beaming, and the drawing geometry.

use super::*;
use crate::core::scene::GravityModel;
use crate::experiments::black_hole::{black_hole, black_hole_centre_units};
use crate::experiments::constants::{SGR_A_STAR_MASS_KG, SGR_A_STAR_SPIN};

const MASS: f64 = SGR_A_STAR_MASS_KG;
const SPIN: f64 = SGR_A_STAR_SPIN;

fn profile() -> DiscProfile {
    DiscProfile::new(MASS, SPIN).expect("valid hole")
}

fn length(v: [f64; 3]) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}

// -----------------------------------------------------------------------
// Blackbody colour.
// -----------------------------------------------------------------------

#[test]
fn blackbody_colours_run_from_red_through_white_to_blue() {
    let [r, g, b] = blackbody_rgb(2_000.0);
    assert!(r > g && g > b, "2,000 K is orange-red");

    let [r, g, b] = blackbody_rgb(6_500.0);
    assert!(
        r > 0.9 && g > 0.9 && b > 0.85,
        "6,500 K is nearly white: {r} {g} {b}"
    );

    let [r, _, b] = blackbody_rgb(20_000.0);
    assert!(b > r, "20,000 K is blue-white");
}

#[test]
fn blackbody_colours_are_normalised_and_clamped() {
    for t in [500.0, 1_000.0, 3_000.0, 10_000.0, 40_000.0, 1.0e7] {
        let rgb = blackbody_rgb(t);
        let brightest = rgb.iter().copied().fold(0.0_f32, f32::max);
        assert!((brightest - 1.0).abs() < 1.0e-6, "{t} K");
        assert!(rgb.iter().all(|c| (0.0..=1.0).contains(c)));
    }
    assert_eq!(blackbody_rgb(500.0), blackbody_rgb(1_000.0));
    assert_eq!(blackbody_rgb(1.0e7), blackbody_rgb(40_000.0));
}

// -----------------------------------------------------------------------
// The profile.
// -----------------------------------------------------------------------

#[test]
fn the_disc_runs_from_the_isco_to_its_outer_edge() {
    let p = profile();
    assert!((p.isco_rg - isco_radius(SPIN, true).unwrap()).abs() < 1.0e-12);
    assert!((p.outer_rg - f64::from(DISC_OUTER_RADIUS_RG)).abs() < 1.0e-12);
    assert_eq!(p.radii_rg.len(), DISC_PROFILE_SAMPLES as usize);
    assert!((p.radii_rg[0] - p.isco_rg).abs() < 1.0e-12);
    assert!((p.radii_rg[p.radii_rg.len() - 1] - p.outer_rg).abs() < 1.0e-9);
}

#[test]
fn the_profile_is_cold_at_the_isco_hot_just_outside_and_cooler_further_out() {
    let p = profile();
    assert_eq!(p.relative_flux_at(p.isco_rg), 0.0);
    assert!((p.relative_flux.iter().copied().fold(0.0, f64::max) - 1.0).abs() < 1.0e-12);

    let peak = p.peak_temperature_k();
    assert!(
        peak > 1.0e5 && peak < 1.0e6,
        "inner disc at {peak:e} K, in the ultraviolet"
    );
    assert!(p.temperature_at(p.outer_rg) < 0.4 * peak);
}

#[test]
fn the_profile_interpolates_its_own_samples_exactly() {
    let p = profile();
    for index in [3, 50, 200] {
        let r = p.radii_rg[index];
        assert!(
            (p.temperature_at(r) - p.temperature_k[index]).abs() < 1.0e-6 * p.temperature_k[index]
        );
    }
    // And clamps outside the disc.
    assert_eq!(p.temperature_at(1.0), p.temperature_at(p.isco_rg));
    assert_eq!(p.temperature_at(1.0e3), p.temperature_at(p.outer_rg));
}

// -----------------------------------------------------------------------
// The gas.
// -----------------------------------------------------------------------

#[test]
fn the_disc_starts_full_with_the_inflows_steady_density() {
    let mut disc = AccretionDisc::default();
    disc.advance(MASS, SPIN, 0.0);
    let p = disc.profile().expect("built").clone();
    assert_eq!(disc.particles().len(), DISC_PARTICLES as usize);

    let middle = 0.5 * (p.isco_rg + p.outer_rg);
    let (inner, outer): (Vec<_>, Vec<_>) =
        disc.particles().iter().partition(|q| q.radius_rg < middle);
    for q in disc.particles() {
        assert!(q.radius_rg > p.isco_rg && q.radius_rg <= p.outer_rg);
        assert!(q.plunge.is_none());
    }
    // p(r) ∝ √r: the outer half holds more than the inner.
    assert!(outer.len() > inner.len());
}

#[test]
fn a_clump_orbits_at_the_kerr_rate_and_drifts_in() {
    let p = profile();
    let mut clump = DiscParticle {
        radius_rg: 6.0,
        azimuth_rad: 0.0,
        plunge: None,
    };
    let omega = circular_orbit(SPIN, 6.0, true).unwrap().angular_velocity / p.gravitational_time_s;
    let dt = 10.0;
    assert!(clump.advance(&p, dt));

    assert!(
        (clump.azimuth_rad - omega * dt).abs() < 1.0e-12,
        "prograde, at Ω"
    );
    let expected =
        6.0 * (-(omega * dt) / (std::f64::consts::TAU * DISC_INFLOW_ORBITS_PER_EFOLD)).exp();
    assert!((clump.radius_rg - expected).abs() < 1.0e-12);
}

#[test]
fn a_clump_reaching_the_isco_plunges_and_is_swallowed() {
    let p = profile();
    let mut clump = DiscParticle {
        radius_rg: p.isco_rg * 1.000_001,
        azimuth_rad: 1.0,
        plunge: None,
    };
    assert!(clump.advance(&p, 10.0));
    assert!(clump.plunge.is_some(), "crossed the ISCO");

    let mut alive = true;
    let mut steps = 0;
    while alive && steps < 2_000 {
        alive = clump.advance(&p, 10.0);
        steps += 1;
    }
    assert!(!alive, "never fell in");
    assert!(steps > 3, "it whirled for a while first: {steps} frames");
}

#[test]
fn swallowed_clumps_are_replaced_at_the_outer_edge() {
    let mut disc = AccretionDisc::default();
    for _ in 0..600 {
        disc.advance(MASS, SPIN, 10.0);
    }
    assert!(disc.plunged() > 0, "some gas fell in");
    assert_eq!(
        disc.particles().len(),
        DISC_PARTICLES as usize,
        "and was replaced"
    );
}

#[test]
fn a_paused_clock_moves_nothing() {
    let mut disc = AccretionDisc::default();
    disc.advance(MASS, SPIN, 0.0);
    let before = disc.clone();
    disc.advance(MASS, SPIN, 0.0);
    assert_eq!(disc, before);
}

#[test]
fn every_run_builds_the_same_disc() {
    let mut first = AccretionDisc::default();
    let mut second = AccretionDisc::default();
    first.advance(MASS, SPIN, 0.0);
    second.advance(MASS, SPIN, 0.0);
    assert_eq!(first, second);
}

// -----------------------------------------------------------------------
// Light from the gas.
// -----------------------------------------------------------------------

#[test]
fn the_side_coming_towards_the_camera_is_blueshifted_and_the_side_going_away_redshifted() {
    // Camera far out along +x (Kerr–Schild axes). A prograde clump at azimuth −90° is moving
    // towards +x, one at +90° away from it.
    let p = profile();
    let camera = [1.0e16, 0.0, 0.0];
    let approaching = DiscParticle {
        radius_rg: 4.0,
        azimuth_rad: -std::f64::consts::FRAC_PI_2,
        plunge: None,
    };
    let receding = DiscParticle {
        radius_rg: 4.0,
        azimuth_rad: std::f64::consts::FRAC_PI_2,
        plunge: None,
    };

    let g_in = approaching.redshift_factor(&p, camera);
    let g_out = receding.redshift_factor(&p, camera);
    assert!(g_in > 1.0, "approaching: g = {g_in}");
    assert!(g_out < 1.0, "receding: g = {g_out}");
}

#[test]
fn seen_face_on_the_disc_is_redshifted_by_gravity_and_time_dilation() {
    // Looking down the spin axis there is no motion along the line of sight: g = 1/u^t < 1, and
    // lower closer in.
    let p = profile();
    let camera = [0.0, 0.0, 1.0e16];
    let inner = DiscParticle {
        radius_rg: 3.0,
        azimuth_rad: 0.0,
        plunge: None,
    };
    let outer = DiscParticle {
        radius_rg: 15.0,
        azimuth_rad: 0.0,
        plunge: None,
    };
    let (g_inner, g_outer) = (
        inner.redshift_factor(&p, camera),
        outer.redshift_factor(&p, camera),
    );
    assert!(
        g_inner < g_outer && g_outer < 1.0,
        "{g_inner} < {g_outer} < 1"
    );
}

// -----------------------------------------------------------------------
// Drawing.
// -----------------------------------------------------------------------

#[test]
fn an_orbiting_clump_is_an_arc_trailing_it_along_its_orbit() {
    let p = profile();
    let clump = DiscParticle {
        radius_rg: 7.0,
        azimuth_rad: 2.0,
        plunge: None,
    };
    let streak = clump.streak_coordinates(&p);
    assert_eq!(streak.len(), DISC_STREAK_POINTS as usize);
    for &(r, _) in &streak {
        assert_eq!(r, 7.0, "on its orbit");
    }
    let (first, last) = (streak[0].1, streak[streak.len() - 1].1);
    assert!((last - 2.0).abs() < 1.0e-12, "ending where it is");
    assert!(
        (last - first - DISC_STREAK_ARC_RAD).abs() < 1.0e-12,
        "trailing it"
    );

    // And where it is, in Kerr–Schild coordinates, is the point at that radius and azimuth.
    let (position, _) = clump.state(&p);
    let point = equatorial_point(SPIN, 7.0, 2.0).map(|c| c * p.gravitational_radius_m);
    assert!(length([0, 1, 2].map(|k| position[k] - point[k])) < 1.0e-6 * length(point));
}

#[test]
fn the_ergosphere_bulges_to_root_four_plus_a_squared_and_meets_the_horizon_at_the_poles() {
    let centre = Vec3::ZERO;
    let lines = ergosphere_outline(MASS, SPIN, centre);
    assert_eq!(lines.len(), 1 + ERGOSPHERE_MERIDIANS as usize);

    let r_g = KerrBlackHole::new(MASS, SPIN)
        .unwrap()
        .gravitational_radius_m()
        / METRES_PER_UNIT;
    let equator = (4.0 + SPIN * SPIN).sqrt() * r_g;
    for point in &lines[0] {
        assert!((f64::from(point.length()) - equator).abs() < 1.0e-3 * equator);
        assert!(point.y.abs() < 1.0e-3);
    }

    let pole = horizon_radius(SPIN).unwrap() * r_g;
    let meridian = &lines[1];
    let top = meridian[0];
    assert!(
        (f64::from(top.y) - pole).abs() < 1.0e-3 * pole,
        "touches the horizon over the pole"
    );
    assert!(f64::from(top.x.hypot(top.z)) < 1.0e-6 * pole);
}

// -----------------------------------------------------------------------
// Where, and the plugin.
// -----------------------------------------------------------------------

#[test]
fn a_disc_forms_only_round_a_spinning_hole() {
    let mut scene = black_hole().scene;
    let (mass, spin, centre) = disc_source(&scene).expect("the experiment's hole spins");
    assert!((mass / MASS - 1.0).abs() < 1.0e-6);
    assert_eq!(spin, SPIN);
    assert_eq!(centre, black_hole_centre_units());

    scene.gravity_model = GravityModel::Schwarzschild;
    assert_eq!(disc_source(&scene), None);
    scene.gravity_model = GravityModel::Newtonian;
    assert_eq!(disc_source(&scene), None);
}

#[test]
fn the_plugin_builds_the_disc_headless() {
    use bevy::app::App;
    use bevy::MinimalPlugins;

    let mut clock = SimulationClock::new(100.0);
    clock.advance(0.05);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(black_hole().scene);
    app.insert_resource(clock);
    app.add_plugins(AccretionDiscPlugin);
    app.update();

    let disc = app.world().resource::<AccretionDisc>();
    assert_eq!(disc.particles().len(), DISC_PARTICLES as usize);
    assert!(disc.profile().is_some());
    let _ = length([0.0; 3]);
}

#[test]
fn the_redshift_along_a_straight_path_is_the_straight_line_redshift() {
    let profile = profile();
    let clump = DiscParticle {
        radius_rg: 7.0,
        azimuth_rad: 0.4,
        plunge: None,
    };
    let camera = [2.0e11, -1.0e11, 0.5e11];
    let (position, _) = clump.state(&profile);
    let to_camera = [0, 1, 2].map(|k| camera[k] - position[k]);
    let n = to_camera.map(|c| c / length(to_camera));
    let along = clump.redshift_factor_along(&profile, n, 1.0);
    assert!((along - clump.redshift_factor(&profile, camera)).abs() < 1.0e-12);
    // A camera deeper in the well sees everything bluer.
    assert!(clump.redshift_factor_along(&profile, n, 0.9) > along);
}
