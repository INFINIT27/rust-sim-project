//! Unit tests for [`crate::render::light_streams`].
//!
//! Compiled into `src/render/light_streams.rs` via `#[path]`. The draw system needs a render
//! pipeline and is not tested here; everything it draws comes from [`LightStreams`] and
//! [`Photon::streak`], which are.
//!
//! The physics of each ray (the null condition, capture inside `2.6 r_s`, the weak-field bend) is
//! tested in `tst/physics_utils/relativity/gravity_tests.rs`. These tests cover the emitter (rays
//! from every direction, aimed as documented, the same every run), the stepping and removal, and the
//! drawing geometry.

use super::*;
use crate::core::scene::GravityModel;
use crate::experiments::black_hole::{black_hole, black_hole_centre_units};
use crate::experiments::constants::SGR_A_STAR_MASS_KG;
use crate::physics_utils::relativity::kerr::kerr_schild_norm;

const MASS: f64 = SGR_A_STAR_MASS_KG;

fn horizon() -> f64 {
    schwarzschild_radius(MASS).expect("valid")
}

/// Advance `streams` by `total_s` in frames of `frame_s`.
fn run(streams: &mut LightStreams, total_s: f64, frame_s: f64) {
    let mut elapsed = 0.0;
    while elapsed < total_s {
        streams.advance(MASS, 0.0, frame_s);
        elapsed += frame_s;
    }
}

/// `count` bursts emitted without moving anything: the rays at their starting points.
fn bursts(count: usize) -> Vec<Photon> {
    let mut streams = LightStreams::default();
    for _ in 0..count {
        streams.emit(MASS, 0.0);
    }
    streams.photons().to_vec()
}

/// A ray's impact parameter: its distance from the hole's centre along the line it is heading on.
fn impact_parameter(photon: &Photon) -> f64 {
    let x = photon.offset_m;
    let v = photon.velocity_m_s;
    let speed = length(v);
    let cross = [
        x[1] * v[2] - x[2] * v[1],
        x[2] * v[0] - x[0] * v[2],
        x[0] * v[1] - x[1] * v[0],
    ];
    length(cross) / speed
}

/// A lone ray along +x from 12 r_s out at impact parameter `b_rs`, run until it is captured or
/// leaves. Returns its fate and the ray as last seen.
fn fly(b_rs: f64) -> (Fate, Photon) {
    let h = horizon();
    let source = -f64::from(LIGHT_STREAM_SOURCE_DISTANCE_RS) * h;
    let hole = Hole::new(MASS, 0.0).expect("valid");
    let mut photon =
        Photon::new(MASS, 0.0, [source, 0.0, b_rs * h], [1.0, 0.0, 0.0]).expect("valid");
    for _ in 0..2_000 {
        let before = photon.clone();
        match photon.advance(&hole, 10.0) {
            Fate::Travelling => {}
            fate => return (fate, before),
        }
    }
    panic!("the ray at {b_rs} r_s neither left nor was captured");
}

// -----------------------------------------------------------------------
// The emitter.
// -----------------------------------------------------------------------

#[test]
fn each_burst_starts_on_the_source_sphere_heading_in() {
    let h = horizon();
    let source = f64::from(LIGHT_STREAM_SOURCE_DISTANCE_RS) * h;
    let max_impact = f64::from(LIGHT_STREAM_MAX_IMPACT_RS) * h;
    let photons = bursts(1);
    assert_eq!(photons.len(), LIGHT_STREAM_RAYS as usize);

    for photon in &photons {
        let r = length(photon.offset_m);
        assert!(r >= source * (1.0 - 1.0e-12));
        assert!(r <= source.hypot(max_impact) * (1.0 + 1.0e-12));

        let inward = photon.offset_m[0] * photon.velocity_m_s[0]
            + photon.offset_m[1] * photon.velocity_m_s[1]
            + photon.offset_m[2] * photon.velocity_m_s[2];
        assert!(inward < 0.0, "heading in");
        assert!(impact_parameter(photon) <= max_impact * (1.0 + 1.0e-9));
    }
}

#[test]
fn the_light_comes_from_every_direction() {
    // 20 bursts, 960 rays: every octant of the sky gets its share (120 expected each).
    let mut octants = [0_u32; 8];
    for photon in bursts(20) {
        let x = photon.offset_m;
        let index =
            usize::from(x[0] > 0.0) | usize::from(x[1] > 0.0) << 1 | usize::from(x[2] > 0.0) << 2;
        octants[index] += 1;
    }
    for (index, count) in octants.iter().enumerate() {
        assert!(*count > 80, "octant {index} got only {count} of 960 rays");
    }
}

#[test]
fn the_impact_parameters_fill_the_disc_evenly() {
    // Spread evenly over a disc of radius 4 r_s, so the fraction aimed inside the critical
    // 2.598 r_s is (2.598/4)^2 = 42%, and the fraction inside 2 r_s is 25%.
    let photons = bursts(40);
    let h = horizon();
    let fraction = |limit_rs: f64| {
        photons
            .iter()
            .filter(|p| impact_parameter(p) < limit_rs * h)
            .count() as f64
            / photons.len() as f64
    };
    let critical = 1.5 * 3.0_f64.sqrt();

    assert!((fraction(critical) - (critical / 4.0).powi(2)).abs() < 0.04);
    assert!((fraction(2.0) - 0.25).abs() < 0.04);
}

#[test]
fn every_run_fires_the_same_pattern() {
    assert_eq!(bursts(3), bursts(3));
}

#[test]
fn the_first_step_fires_at_once_and_then_on_schedule() {
    let mut streams = LightStreams::default();
    streams.advance(MASS, 0.0, 0.0);
    assert_eq!(streams.photons().len(), LIGHT_STREAM_RAYS as usize);

    streams.advance(MASS, 0.0, LIGHT_STREAM_EMISSION_INTERVAL_S - 1.0);
    assert_eq!(streams.photons().len(), LIGHT_STREAM_RAYS as usize);
    streams.advance(MASS, 0.0, 1.0);
    assert_eq!(streams.photons().len(), 2 * LIGHT_STREAM_RAYS as usize);
}

#[test]
fn a_paused_clock_moves_nothing() {
    let mut streams = LightStreams::default();
    streams.advance(MASS, 0.0, 0.0);
    let before = streams.clone();
    for _ in 0..10 {
        streams.advance(MASS, 0.0, 0.0);
    }
    assert_eq!(streams, before);
}

// -----------------------------------------------------------------------
// The rays.
// -----------------------------------------------------------------------

#[test]
fn rays_keep_lights_speed_wherever_they_go() {
    // (1, v) stays a null vector of the metric along every path, with and without spin.
    for spin in [0.0, 0.9] {
        let mut streams = LightStreams::default();
        let mut elapsed = 0.0;
        while elapsed < 300.0 {
            streams.advance(MASS, spin, 10.0);
            elapsed += 10.0;
        }
        for photon in streams.photons() {
            let n = kerr_schild_norm(MASS, spin, photon.offset_m, photon.velocity_m_s).unwrap();
            assert!(n.abs() < 1.0e-6, "spin {spin}: not null, {n}");
        }
    }
}

#[test]
fn a_ray_aimed_inside_the_critical_parameter_is_captured() {
    let (fate, last) = fly(2.5);
    assert_eq!(fate, Fate::Captured);
    assert!(
        length(last.offset_m) < 1.05 * horizon(),
        "it crept up to the horizon"
    );
}

#[test]
fn a_ray_aimed_wide_escapes_bent_towards_the_hole() {
    // Released at z = +6 r_s heading +x; the hole at the origin pulls it towards -z.
    let (fate, last) = fly(6.0);
    assert_eq!(fate, Fate::Gone);
    assert!(last.velocity_m_s[2] < 0.0, "bent towards the hole");
    assert!(last.velocity_m_s[0] > 0.0, "but still heading on");
}

#[test]
fn a_ray_just_outside_the_critical_parameter_swings_far_round() {
    // At 2.62 r_s, just outside 2.598, the ray whirls round the photon sphere and leaves heading
    // back the way it came.
    let (fate, last) = fly(2.62);
    assert_eq!(fate, Fate::Gone);
    assert!(last.velocity_m_s[0] < 0.0, "turned by more than 90 degrees");
}

#[test]
fn rays_are_captured_and_removed_and_the_count_stays_bounded() {
    let mut streams = LightStreams::default();
    run(&mut streams, 3_000.0, 10.0);

    // 301 bursts of 48 have been fired. About 42% of each is aimed to fall in, and a ray takes
    // around 700 s to arrive and be let go, so the first 230 bursts' captures are in: 4,600.
    let fired = 301 * LIGHT_STREAM_RAYS as u64;
    assert!(
        streams.captured() > 3_000,
        "captured {}",
        streams.captured()
    );
    assert!(
        (streams.photons().len() as u64) < fired / 2,
        "{} in flight",
        streams.photons().len()
    );
}

// -----------------------------------------------------------------------
// Drawing.
// -----------------------------------------------------------------------

#[test]
fn a_streak_is_the_rays_recent_path_ending_where_it_is() {
    let mut streams = LightStreams::default();
    run(&mut streams, 200.0, 10.0);
    let centre = Vec3::new(5.0, -3.0, 2.0);
    let photon = &streams.photons()[0];
    let streak = photon.streak(centre);

    assert_eq!(streak.len(), photon.trail_m.len() + 1);
    assert!(streak.len() <= LIGHT_STREAM_TRAIL_SAMPLES as usize + 1);

    // Offsets are held in Kerr–Schild axes (spin along +z) and drawn in world axes (+Y up).
    for (point, offset) in streak.iter().zip(
        photon
            .trail_m
            .iter()
            .chain(std::iter::once(&photon.offset_m)),
    ) {
        let expected = centre + (kerr_schild_to_world(*offset) / METRES_PER_UNIT).as_vec3();
        assert!(point.distance(expected) < 1.0e-3);
    }
}

#[test]
fn a_streak_is_a_fixed_span_of_simulated_time_at_any_frame_rate() {
    let mut coarse = LightStreams::default();
    let mut fine = LightStreams::default();
    run(&mut coarse, 200.0, 10.0);
    run(&mut fine, 200.0, 1.0);
    assert_eq!(
        coarse.photons()[0].trail_m.len(),
        fine.photons()[0].trail_m.len()
    );
}

// -----------------------------------------------------------------------
// Which hole, and the plugin.
// -----------------------------------------------------------------------

#[test]
fn the_streams_bend_round_the_most_massive_black_hole_of_a_relativistic_scene() {
    let mut scene = black_hole().scene;
    let (mass, spin, centre) = stream_source(&scene).expect("a black hole");
    assert!((mass / MASS - 1.0).abs() < 1.0e-6);
    assert_eq!(Some(spin), scene.gravity_model.black_hole_spin());
    assert_eq!(centre, black_hole_centre_units());

    scene.gravity_model = GravityModel::Newtonian;
    assert_eq!(stream_source(&scene), None, "no streams under Newton");
}

#[test]
fn a_massless_hole_fires_nothing() {
    let mut streams = LightStreams::default();
    streams.advance(0.0, 0.0, 10.0);
    assert!(streams.photons().is_empty());
}

#[test]
fn the_plugin_runs_the_streams_headless() {
    use bevy::app::App;
    use bevy::MinimalPlugins;

    let mut clock = SimulationClock::new(100.0);
    clock.advance(0.05);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(black_hole().scene);
    app.insert_resource(clock);
    app.add_plugins(LightStreamsPlugin);
    app.update();

    let streams = app.world().resource::<LightStreams>();
    assert_eq!(streams.photons().len(), LIGHT_STREAM_RAYS as usize);
}

#[test]
fn a_spinning_hole_captures_and_releases_light_too() {
    // At spin 0.9 the streams still run, capture some rays, and let the rest go.
    let mut streams = LightStreams::default();
    let mut elapsed = 0.0;
    while elapsed < 2_000.0 {
        streams.advance(MASS, 0.9, 10.0);
        elapsed += 10.0;
    }
    assert!(streams.captured() > 500, "captured {}", streams.captured());
    assert!(!streams.photons().is_empty());
}
