//! Unit and property tests for the [`crate::render::trail`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/render/trail.rs` via `#[path]`,
//! not as a standalone integration test.
//!
//! Almost everything here tests [`TrailPath`], which is where the two things that could plausibly
//! be wrong live: **trimming** the path to an exact length, and **fading** by distance along it.
//! `draw_trails_system` is not tested — `Gizmos` needs the render pipeline, as the egui drawers
//! need a window — which is precisely why the geometry is a pure type and the drawing system is
//! thin enough to read.
//!
//! The toggle *is* tested headlessly: it touches only a resource and the keyboard.

use super::*;
use crate::core::scene::DVec3;
use crate::experiments::catalog::two_body;
use crate::render::constants::TRAIL_LINE_WIDTH_PX;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/render/proptest-regressions/trail_tests.txt";

/// A path walked along `+X` in `step` increments, sampled at every step.
fn walked(steps: usize, step: f32, max_length: f32) -> TrailPath {
    let mut path = TrailPath::default();
    for index in 0..=steps {
        path.record(
            Vec3::new(index as f32 * step, 0.0, 0.0),
            max_length,
            step * 0.5,
            TRAIL_MAX_SAMPLES,
        );
    }
    path
}

// -----------------------------------------------------------------------
// Recording and trimming: the trail is exactly as long as it claims.
// -----------------------------------------------------------------------

#[test]
fn a_fresh_path_has_nothing_to_draw() {
    let path = TrailPath::default();

    assert!(path.is_empty());
    assert_eq!(path.head(), None);
    assert_eq!(path.length_units(), 0.0);
    assert!(
        path.segments(1.0, 0.0).is_empty(),
        "one point is not a line"
    );
}

#[test]
fn the_head_is_always_the_body_current_position() {
    // The trail must stay attached to the body, not lag by up to one sample — which is why
    // `record` slides the head rather than only pushing new points.
    let mut path = TrailPath::default();

    path.record(Vec3::ZERO, 5.0, 1.0, 64);
    assert_eq!(path.head(), Some(Vec3::ZERO));
    assert_eq!(path.len(), 1, "one position is not yet a path");

    // The first movement, however small, gives the path its anchor — a just-launched body must
    // not be trailless until it has travelled a whole sample's worth.
    path.record(Vec3::new(0.1, 0.0, 0.0), 5.0, 1.0, 64);
    assert_eq!(path.head(), Some(Vec3::new(0.1, 0.0, 0.0)));
    assert_eq!(path.len(), 2, "[live head, anchor]");

    // Further small moves slide the head and leave the anchor alone.
    path.record(Vec3::new(0.2, 0.0, 0.0), 5.0, 1.0, 64);
    assert_eq!(path.head(), Some(Vec3::new(0.2, 0.0, 0.0)));
    assert_eq!(path.len(), 2, "no sample earned yet, and none lost");

    // Once the head is a full spacing from the anchor, it commits and a new head begins.
    path.record(Vec3::new(2.0, 0.0, 0.0), 5.0, 1.0, 64);
    assert_eq!(path.head(), Some(Vec3::new(2.0, 0.0, 0.0)));
    assert_eq!(path.len(), 3);
}

// -----------------------------------------------------------------------
// Regression: the spacing test must measure from the *anchor*, not the head.
//
// The first implementation compared against `points[0]` — the head — and then overwrote it when
// the move was too small to commit, destroying the very anchor the comparison needed. A body
// moving less than `min_spacing` per frame therefore committed *nothing*, however far it
// travelled, and its trail stayed one point long forever.
//
// That is exactly the shipped case: the two-body pair moves ~0.029 units per 60 Hz frame and the
// three-body ring ~0.046, against a 0.05 spacing. The old tests missed it because every one of
// them stepped further than the spacing each time.
// -----------------------------------------------------------------------

#[test]
fn a_body_moving_less_than_the_spacing_each_frame_still_records_its_path() {
    let mut path = TrailPath::default();

    // 0.02 per frame against a 0.05 spacing: no single frame earns a sample, but 200 frames
    // travel 4 units and the trail must show them.
    for index in 0..200 {
        path.record(Vec3::new(index as f32 * 0.02, 0.0, 0.0), 10.0, 0.05, 512);
    }

    assert!(
        (path.length_units() - 199.0 * 0.02).abs() < 0.1,
        "travelled {} units but the trail is {} long",
        199.0 * 0.02,
        path.length_units()
    );
    assert!(
        path.len() > 50,
        "only {} samples for a 4-unit path",
        path.len()
    );
}

#[test]
fn the_shipped_scenes_per_frame_motion_produces_a_visible_trail() {
    // The two figures from the bug, asserted directly rather than through a scene: whatever else
    // changes, a body creeping along at these rates must leave a trail.
    for per_frame in [0.029_f32, 0.046] {
        let mut path = TrailPath::default();

        // Two seconds at 60 fps.
        for index in 0..120 {
            path.record(
                Vec3::new(index as f32 * per_frame, 0.0, 0.0),
                TRAIL_LENGTH_UNITS,
                TRAIL_MIN_SAMPLE_SPACING_UNITS,
                TRAIL_MAX_SAMPLES,
            );
        }

        let travelled = 119.0 * per_frame;
        let expected = travelled.min(TRAIL_LENGTH_UNITS);
        assert!(
            (path.length_units() - expected).abs() < 0.2,
            "at {per_frame} units per frame the trail is {} long, expected ~{expected}",
            path.length_units()
        );
    }
}

#[test]
fn sliding_the_head_never_destroys_the_anchor() {
    // The mechanism of the bug, isolated: many sub-spacing moves in a row must leave the anchor
    // exactly where it was, because it is what the next comparison measures from.
    let mut path = TrailPath::default();
    path.record(Vec3::ZERO, 10.0, 1.0, 64);
    path.record(Vec3::new(0.01, 0.0, 0.0), 10.0, 1.0, 64);

    let anchor_before = path.segments(1.0, 0.0)[0].to;

    for index in 2..50 {
        path.record(Vec3::new(index as f32 * 0.01, 0.0, 0.0), 10.0, 1.0, 64);
    }

    assert_eq!(
        path.segments(1.0, 0.0)[0].to,
        anchor_before,
        "the anchor moved while the head was sliding"
    );
    assert!((path.length_units() - 0.49).abs() < 1.0e-3);
}

#[test]
fn a_path_is_trimmed_to_exactly_its_maximum_length() {
    // Not "within one sample of" — the tail is slid along its own segment, so the trail does not
    // pulse shorter and longer as samples arrive.
    let path = walked(20, 1.0, 5.0);

    assert!(
        (path.length_units() - 5.0).abs() < 1.0e-4,
        "expected exactly 5 units, got {}",
        path.length_units()
    );
    assert_eq!(path.head(), Some(Vec3::new(20.0, 0.0, 0.0)));
}

#[test]
fn the_length_stays_exact_as_samples_arrive() {
    // The pulsing this prevents is a per-frame artefact, so check it every step rather than once
    // at the end.
    let mut path = TrailPath::default();

    for index in 0..40 {
        path.record(Vec3::new(index as f32 * 0.7, 0.0, 0.0), 5.0, 0.3, 64);

        let length = path.length_units();
        assert!(
            length <= 5.0 + 1.0e-4,
            "step {index}: trail grew to {length}, past its 5-unit budget"
        );
        if index as f32 * 0.7 > 6.0 {
            assert!(
                (length - 5.0).abs() < 1.0e-3,
                "step {index}: trail is {length}, should be pinned at 5"
            );
        }
    }
}

#[test]
fn a_short_path_is_not_padded() {
    // A trail that has not yet grown to full length is simply shorter — it must not be stretched
    // or held at zero until it earns its full 5 units.
    let path = walked(2, 1.0, 5.0);

    assert!((path.length_units() - 2.0).abs() < 1.0e-4);
    assert_eq!(path.len(), 3);
}

#[test]
fn a_stationary_body_does_not_fill_the_buffer() {
    // Without the minimum spacing, a nearly-still body accumulates coincident points and the
    // trail's *length* stops meaning anything.
    let mut path = TrailPath::default();

    for _ in 0..1000 {
        path.record(Vec3::new(1.0, 2.0, 3.0), 5.0, 0.05, TRAIL_MAX_SAMPLES);
    }

    assert_eq!(
        path.len(),
        1,
        "a still body needs one point, not a thousand — a position identical to the head is not \
         movement, so it never earns an anchor"
    );
    assert_eq!(path.length_units(), 0.0);
    assert!(path.is_empty());
}

#[test]
fn the_sample_cap_bounds_the_buffer_even_with_zero_spacing() {
    // The cap exists for exactly this: a mis-set spacing must not grow memory without bound.
    let mut path = TrailPath::default();

    for index in 0..2000 {
        path.record(Vec3::new(index as f32 * 1.0e-4, 0.0, 0.0), 5.0, 0.0, 32);
    }

    assert!(path.len() <= 32, "stored {} samples, cap is 32", path.len());
}

#[test]
fn a_non_finite_position_is_refused_rather_than_stored() {
    // A `NaN` point would poison every length and alpha downstream; dropping the sample merely
    // shortens the trail.
    let mut path = walked(6, 1.0, 5.0);
    let before = path.clone();

    path.record(Vec3::new(f32::NAN, 0.0, 0.0), 5.0, 0.5, 64);
    path.record(Vec3::new(0.0, f32::INFINITY, 0.0), 5.0, 0.5, 64);

    assert_eq!(path, before);
    assert!(path.is_valid());
}

#[test]
fn a_nonsense_maximum_length_clears_the_path_rather_than_keeping_a_stale_one() {
    let mut path = walked(10, 1.0, 5.0);
    assert!(!path.is_empty());

    path.record(Vec3::new(11.0, 0.0, 0.0), 0.0, 0.5, 64);
    assert!(path.is_empty(), "a zero-length trail keeps nothing");
}

#[test]
fn a_curved_path_keeps_its_shape() {
    // Trimming must remove length from the tail, not resample or straighten the path.
    let mut path = TrailPath::default();
    for index in 0..40 {
        let angle = index as f32 * 0.2;
        path.record(
            Vec3::new(angle.cos() * 3.0, 0.0, angle.sin() * 3.0),
            5.0,
            0.05,
            TRAIL_MAX_SAMPLES,
        );
    }

    // Every stored point is still on the circle it was recorded on — with one documented
    // exception: the **trimmed tail** is interpolated along the chord between two samples, so it
    // sits inside the arc by the chord's sag. At 0.2 rad between samples on a radius-3 circle
    // that is `3·(1 − cos 0.1) ≈ 0.015`, which is what the tolerance below allows. Trimming to an
    // exact length and following a curve exactly are not simultaneously possible with straight
    // segments; the length is the property worth being exact about.
    for segment in path.segments(1.0, 0.0) {
        for point in [segment.from, segment.to] {
            let radius = Vec3::new(point.x, 0.0, point.z).length();
            assert!(
                (radius - 3.0).abs() < 2.0e-2,
                "a point moved off the path: radius {radius}"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Fading: brightest at the body, gone at the far end.
// -----------------------------------------------------------------------

#[test]
fn alpha_runs_from_full_at_the_body_to_zero_at_the_far_end() {
    let path = walked(20, 0.5, 5.0);
    let segments = path.segments(TRAIL_HEAD_ALPHA, TRAIL_TAIL_ALPHA);

    assert!(!segments.is_empty());
    assert!(
        (segments[0].from_alpha - TRAIL_HEAD_ALPHA).abs() < 1.0e-4,
        "the segment at the body must be fully bright, got {}",
        segments[0].from_alpha
    );
    assert!(
        (segments.last().unwrap().to_alpha - TRAIL_TAIL_ALPHA).abs() < 1.0e-4,
        "the far end must fade out, got {}",
        segments.last().unwrap().to_alpha
    );
}

#[test]
fn alpha_decreases_monotonically_away_from_the_body() {
    // The gradient is what carries the direction: a non-monotone fade would make some middle
    // point read as the newest.
    let path = walked(30, 0.4, 5.0);
    let segments = path.segments(1.0, 0.0);

    let mut previous = f32::INFINITY;
    for segment in &segments {
        assert!(
            segment.from_alpha <= previous + 1.0e-6,
            "alpha went back up"
        );
        assert!(
            segment.to_alpha <= segment.from_alpha + 1.0e-6,
            "a segment brightens away from the body"
        );
        previous = segment.to_alpha;
    }
}

#[test]
fn alpha_is_interpolated_by_distance_not_by_sample_index() {
    // Samples are not evenly spaced — a body accelerating leaves longer gaps — so an
    // index-based fade would make the fast part of a path fade faster than the slow part.
    let mut path = TrailPath::default();
    // Uneven steps: 0, 0.2, 1.2, 1.4, 3.4 ... along X.
    for point in [0.0_f32, 0.2, 1.2, 1.4, 3.4] {
        path.record(Vec3::new(point, 0.0, 0.0), 100.0, 0.1, 64);
    }

    let segments = path.segments(1.0, 0.0);
    let total = path.length_units();

    // The head is the *last* recorded point, so distance from head runs backwards along X.
    for segment in segments {
        let distance_from_head = 3.4 - segment.from.x;
        let expected = 1.0 - distance_from_head / total;
        assert!(
            (segment.from_alpha - expected).abs() < 1.0e-3,
            "alpha {} at distance {distance_from_head}, expected {expected}",
            segment.from_alpha
        );
    }
}

#[test]
fn a_partial_trail_fades_over_the_length_it_has() {
    // Otherwise a trail that has not yet reached 5 units would be uniformly bright, and the
    // fade would appear only once it filled — which reads as a rendering glitch.
    let path = walked(4, 0.5, 5.0);
    assert!(path.length_units() < 5.0);

    let segments = path.segments(1.0, 0.0);
    assert!((segments.last().unwrap().to_alpha).abs() < 1.0e-4);
}

// -----------------------------------------------------------------------
// Recording from a scene, and the toggle.
// -----------------------------------------------------------------------

#[test]
fn recording_skips_immovable_props() {
    // The floor does not move; a trail on it would be one point drawn forever.
    let mut trails = Trails::default();
    let config = TrailConfig::default();
    let scene = Scene::two_body_gravity();

    trails.record_from(&scene, &config);

    assert_eq!(trails.len(), 2, "the two spheres, not the floor");
    for (id, _) in trails.iter() {
        let prop = scene.get(id).expect("a trailed id must be in the scene");
        assert!(!prop.immovable);
    }
}

#[test]
fn recording_follows_a_body_as_the_scene_steps() {
    use crate::time::simulation::TimeStep;

    let mut trails = Trails::default();
    let config = TrailConfig::default();
    let mut scene = Scene::two_body_gravity();

    for _ in 0..200 {
        scene.step(TimeStep::from_seconds(2.0e5).unwrap());
        trails.record_from(&scene, &config);
    }

    for (id, path) in trails.iter() {
        let centre = scene.get(id).unwrap().body.center().as_vec3();
        assert_eq!(
            path.head(),
            Some(centre),
            "a trail's head must be where its body is"
        );
        assert!(path.length_units() <= config.length_units + 1.0e-3);
        assert!(path.is_valid());
    }
}

#[test]
fn a_prop_that_leaves_the_scene_leaves_no_trail_behind() {
    // Nothing removes props today; this is what keeps the invariant true when something does.
    let mut trails = Trails::default();
    let config = TrailConfig::default();
    let mut scene = Scene::two_body_gravity();

    trails.record_from(&scene, &config);
    assert_eq!(trails.len(), 2);

    scene.props.truncate(2); // drop the second sphere
    trails.record_from(&scene, &config);

    assert_eq!(trails.len(), 1, "the departed body's trail must be dropped");
}

// -----------------------------------------------------------------------
// Moons: trailed relative to their planet, so the trail shows the orbit.
// -----------------------------------------------------------------------

fn prop_id_with_mass(scene: &Scene, mass_kg: f64) -> BodyId {
    scene
        .iter()
        .find(|(_, prop)| prop.mass_kg == mass_kg as f32)
        .map(|(id, _)| id)
        .expect("a prop with that mass")
}

#[test]
fn the_solar_systems_moons_are_matched_to_their_planets_and_nothing_else_is() {
    use crate::experiments::constants::{EARTH_MOONS, JOVIAN_MOONS, SOLAR_SYSTEM_PLANETS};

    let scene = crate::experiments::solar_system::solar_system().scene;
    let earth = prop_id_with_mass(&scene, SOLAR_SYSTEM_PLANETS[2].mass_kg);
    let jupiter = prop_id_with_mass(&scene, SOLAR_SYSTEM_PLANETS[4].mass_kg);

    let mut expected: HashMap<BodyId, BodyId> = HashMap::new();
    expected.insert(prop_id_with_mass(&scene, EARTH_MOONS[0].mass_kg), earth);
    for moon in JOVIAN_MOONS {
        expected.insert(prop_id_with_mass(&scene, moon.mass_kg), jupiter);
    }

    for (id, _) in scene.iter() {
        let primary = moon_primary(&scene, id).map(|(primary, _)| primary);
        assert_eq!(primary, expected.get(&id).copied(), "primary of {id:?}");
    }
}

#[test]
fn comparable_bodies_are_never_treated_as_moons() {
    for scene in [
        two_body().scene,
        crate::experiments::catalog::three_body_ring(7).scene,
    ] {
        for (id, _) in scene.iter() {
            assert_eq!(moon_primary(&scene, id), None, "{id:?} must not be a moon");
        }
    }
}

fn distance_to_polyline(point: Vec3, path: &TrailPath) -> f32 {
    path.segments(1.0, 0.0)
        .iter()
        .map(|segment| {
            let along = segment.to - segment.from;
            let t = if along.length_squared() > 0.0 {
                ((point - segment.from).dot(along) / along.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            point.distance(segment.from + along * t)
        })
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn a_moons_trail_follows_the_absolute_path_the_integrator_computed() {
    use crate::experiments::constants::{EARTH_MOONS, JOVIAN_MOONS, SOLAR_SYSTEM_PLANETS};
    use crate::time::simulation::SimulationClock;

    let crate::experiments::catalog::Experiment {
        mut scene, spec, ..
    } = crate::experiments::solar_system::solar_system();
    let mut config = TrailConfig::default();
    config.length_units = spec.trail_length_units.unwrap();
    config.min_spacing_units = config.length_units / TRAIL_TARGET_SEGMENTS as f32;
    let mut clock = SimulationClock::new(spec.time_scale);
    let mut trails = Trails::default();

    let moons: Vec<BodyId> = EARTH_MOONS
        .iter()
        .chain(JOVIAN_MOONS.iter())
        .map(|moon| prop_id_with_mass(&scene, moon.mass_kg))
        .collect();
    let io = prop_id_with_mass(&scene, JOVIAN_MOONS[0].mass_kg);
    let jupiter = prop_id_with_mass(&scene, SOLAR_SYSTEM_PLANETS[4].mass_kg);
    let mut truth: HashMap<BodyId, Vec<Vec3>> = HashMap::new();

    // One second of the app at 60 fps: six Io orbits at the shipped clock.
    for _ in 0..60 {
        scene.step_observed(clock.advance(1.0 / 60.0), |scene| {
            trails.record_from(scene, &config);
            for &moon in &moons {
                truth
                    .entry(moon)
                    .or_default()
                    .push(scene.get(moon).unwrap().body.center().as_vec3());
            }
        });
    }

    for &moon in &moons {
        let path = trails.path(moon).unwrap();
        assert_eq!(
            path.head(),
            Some(scene.get(moon).unwrap().body.center().as_vec3())
        );
        assert!(
            path.len() < TRAIL_MAX_SAMPLES,
            "{moon:?}'s trail hit the sample cap, so it is shorter than it says"
        );

        // Every substep position the trail still covers must lie on the drawn line.
        let mut travelled = 0.0;
        for pair in truth[&moon].windows(2).rev() {
            travelled += pair[0].distance(pair[1]);
            if travelled > 0.95 * path.length_units() {
                break;
            }
            let off_by = distance_to_polyline(pair[0], path);
            assert!(
                off_by < 0.002,
                "{moon:?}'s trail strays {off_by} units from where it actually was"
            );
        }
    }

    // Io outruns Jupiter, so its real path loops backwards. The trail must show that.
    let heading = scene.get(jupiter).unwrap().velocity_units_s.as_vec3();
    let retrograde = trails
        .path(io)
        .unwrap()
        .segments(1.0, 0.0)
        .iter()
        .any(|segment| (segment.from - segment.to).dot(heading) < 0.0);
    assert!(retrograde, "Io's trail must include its backward loops");
}

#[test]
fn trails_are_on_by_default() {
    let config = TrailConfig::default();

    assert!(config.visible, "trails ship visible");
    assert_eq!(config.length_units, TRAIL_LENGTH_UNITS);
    assert_eq!(config.head_alpha, TRAIL_HEAD_ALPHA);
    assert_eq!(config.tail_alpha, TRAIL_TAIL_ALPHA);
}

/// An `App` with the toggle system and the trail resources.
fn app_with_toggle() -> bevy::app::App {
    use bevy::app::App;
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TrailConfig>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_systems(Update, toggle_trails_system);
    app
}

/// Press `key` for exactly one frame, the way Bevy's input plugin would.
///
/// The `release` matters as much as the `clear`: `press` only registers a *new* `just_pressed`
/// for a key that is not already held, so without releasing it the second simulated press does
/// nothing and the test silently checks one toggle twice.
fn press(app: &mut bevy::app::App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
}

#[test]
fn t_toggles_visibility() {
    let mut app = app_with_toggle();
    assert!(app.world().resource::<TrailConfig>().visible);

    press(&mut app, KeyCode::KeyT);
    assert!(!app.world().resource::<TrailConfig>().visible);

    press(&mut app, KeyCode::KeyT);
    assert!(app.world().resource::<TrailConfig>().visible);
}

#[test]
fn holding_t_does_not_flicker_the_setting() {
    // Edge-triggered: a held key must toggle once, not sixty times a second, which would leave
    // the state a coin toss decided by the frame count.
    let mut app = app_with_toggle();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyT);

    for _ in 0..9 {
        app.update();
    }

    assert!(
        !app.world().resource::<TrailConfig>().visible,
        "one press, one toggle"
    );
}

#[test]
fn hiding_trails_does_not_forget_them() {
    // Recording continues while hidden, so `T` reads as a view setting rather than something
    // destructive: press it twice and the trail is still there.
    let mut trails = Trails::default();
    let config = TrailConfig {
        visible: false,
        ..Default::default()
    };

    let mut path_lengths = Vec::new();
    let mut scene = Scene::two_body_gravity();

    for _ in 0..50 {
        scene.props[1].body.translate(DVec3::new(0.2, 0.0, 0.0));
        trails.record_from(&scene, &config);
    }

    for (_, path) in trails.iter() {
        path_lengths.push(path.length_units());
    }

    assert!(
        path_lengths.iter().any(|length| *length > 1.0),
        "trails must keep recording while hidden: {path_lengths:?}"
    );
}

#[test]
fn the_toggle_key_does_not_collide_with_the_other_bindings() {
    // `T` lives in the render layer rather than `input/controls.rs` — see the module docs — so
    // this is the one place the whole keyboard is checked in one list.
    let camera_keys = [
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::KeyQ,
        KeyCode::KeyE,
        KeyCode::Space,
        KeyCode::ControlLeft,
        KeyCode::ShiftLeft,
        KeyCode::Escape,
    ];
    let clock_keys = [
        KeyCode::KeyP,
        KeyCode::Period,
        KeyCode::BracketRight,
        KeyCode::BracketLeft,
        KeyCode::Backslash,
    ];

    assert!(!camera_keys.contains(&KeyCode::KeyT));
    assert!(!clock_keys.contains(&KeyCode::KeyT));
}

// -----------------------------------------------------------------------
// Thickness, and the plugin's behaviour without a render pipeline.
// -----------------------------------------------------------------------

#[test]
fn the_trail_line_width_is_three_times_the_gizmo_default() {
    // Bevy's default gizmo line width is 2 px, at which a trail fading to transparent against a
    // lit 3D scene is technically present and practically invisible. Asserting the *ratio* rather
    // than the value, so the intent survives a change to either number.
    let bevy_default = 2.0_f32;
    assert!(
        (TRAIL_LINE_WIDTH_PX / bevy_default - 3.0).abs() < 1.0e-6,
        "trail width is {TRAIL_LINE_WIDTH_PX} px, expected 3× the {bevy_default} px default"
    );
}

#[test]
fn the_trail_length_is_two_fine_grid_cells() {
    // The trail is read against the floor grid, so its length being a whole number of cells is
    // what lets you judge it at a glance rather than estimate it.
    let cells = TRAIL_LENGTH_UNITS / crate::render::constants::GRID_SPACING_UNITS;
    assert!(
        (cells - cells.round()).abs() < 1.0e-6,
        "a {TRAIL_LENGTH_UNITS}-unit trail is {cells} grid cells"
    );
    assert!(
        cells >= 2.0,
        "a trail shorter than two cells is hard to follow"
    );
}

#[test]
fn the_sample_cap_is_above_what_the_shipped_trail_needs() {
    // A cap below `length / spacing` silently shortens every trail, which looks like a bug in the
    // length rather than in the cap. Doubling the length doubled this requirement.
    let needed = (TRAIL_LENGTH_UNITS / TRAIL_MIN_SAMPLE_SPACING_UNITS).ceil() as usize;
    assert!(
        TRAIL_MAX_SAMPLES > needed,
        "cap is {TRAIL_MAX_SAMPLES} but a full trail needs {needed} samples"
    );
}

#[test]
fn the_plugin_records_trails_without_a_render_pipeline() {
    // `MinimalPlugins` has no `GizmoPlugin`, so there is no gizmo config store and no way to
    // draw. The plugin must still install its resources and keep recording rather than panicking
    // — which is what lets a headless app hold trails without pretending it can paint them.
    use bevy::app::App;
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(Scene::two_body_gravity());
    app.add_plugins(TrailPlugin);

    // Nudge a body so there is something to record.
    for _ in 0..5 {
        app.world_mut().resource_mut::<Scene>().props[1]
            .body
            .translate(DVec3::new(0.5, 0.0, 0.0));
        app.update();
    }

    let trails = app.world().resource::<Trails>();
    assert_eq!(trails.len(), 2, "both spheres should be trailed");
    assert!(
        trails.iter().any(|(_, path)| path.length_units() > 0.0),
        "recording must run even with no way to draw"
    );
    assert!(app.world().resource::<TrailConfig>().visible);
}

// -----------------------------------------------------------------------
// Per-experiment trail length.
// -----------------------------------------------------------------------

/// A test `App` with the trail plugin and, optionally, an experiment's spec.
fn app_with_trails(spec: Option<ExperimentSpec>) -> bevy::app::App {
    use bevy::app::App;
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(Scene::two_body_gravity());
    if let Some(spec) = spec {
        app.insert_resource(spec);
    }
    app.add_plugins(TrailPlugin);
    app.update();
    app
}

#[test]
fn an_experiment_that_asks_for_nothing_keeps_the_shipped_trail() {
    // The two- and three-body experiments both say `None`, so this is the common path and
    // the one that must not move: the default is the render layer's constant, full stop.
    let app = app_with_trails(Some(two_body().spec));
    let config = *app.world().resource::<TrailConfig>();

    assert_eq!(config.length_units, TRAIL_LENGTH_UNITS);
    assert_eq!(config.min_spacing_units, TRAIL_MIN_SAMPLE_SPACING_UNITS);
}

#[test]
fn no_experiment_at_all_also_keeps_the_shipped_trail() {
    // Most render tests are an app with no experiment in it. The startup system must treat
    // that as "nothing to say" rather than as an error.
    let app = app_with_trails(None);

    assert_eq!(
        app.world().resource::<TrailConfig>().length_units,
        TRAIL_LENGTH_UNITS
    );
}

#[test]
fn an_experiment_can_ask_for_a_longer_trail() {
    // The solar system does: 10 units is a hundredth of one Earth orbit.
    let spec = crate::experiments::solar_system::solar_system().spec;
    let requested = spec
        .trail_length_units
        .expect("the solar system asks for its own trail length");

    let app = app_with_trails(Some(spec));
    let config = *app.world().resource::<TrailConfig>();

    assert_eq!(config.length_units, requested);
    assert!(
        config.length_units > TRAIL_LENGTH_UNITS,
        "the point of the field is a *longer* trail"
    );
}

#[test]
fn a_longer_trail_does_not_cost_more_samples() {
    // The failure this prevents: keeping the 0.05-unit spacing for a 400-unit trail would
    // ask for 8000 samples, hit the 512 cap, and draw a trail a twentieth of its stated
    // length — which looks like the length not working rather than the spacing.
    let spec = crate::experiments::solar_system::solar_system().spec;
    let app = app_with_trails(Some(spec));
    let config = *app.world().resource::<TrailConfig>();

    let samples = (config.length_units / config.min_spacing_units).ceil() as usize;

    assert_eq!(samples, TRAIL_TARGET_SEGMENTS);
    assert!(
        samples < config.max_samples,
        "{samples} samples must stay under the {} cap",
        config.max_samples
    );
}

#[test]
fn a_short_requested_trail_cannot_sample_finer_than_the_default() {
    // The derived spacing is floored, so an experiment asking for a *shorter* trail than the
    // default cannot quietly multiply the per-frame cost.
    let mut spec = two_body().spec;
    spec.trail_length_units = Some(1.0);

    let app = app_with_trails(Some(spec));
    let config = *app.world().resource::<TrailConfig>();

    assert_eq!(config.length_units, 1.0);
    assert_eq!(config.min_spacing_units, TRAIL_MIN_SAMPLE_SPACING_UNITS);
}

#[test]
fn a_nonsense_requested_length_is_ignored() {
    // Release behaviour: the config is left at its default rather than made non-finite,
    // which would take every trail with it. In debug this fires a `debug_assert!`.
    #[cfg(not(debug_assertions))]
    for bad in [0.0, -5.0, f32::NAN] {
        let mut spec = two_body().spec;
        spec.trail_length_units = Some(bad);

        let app = app_with_trails(Some(spec));
        assert_eq!(
            app.world().resource::<TrailConfig>().length_units,
            TRAIL_LENGTH_UNITS,
            "{bad} should have been ignored"
        );
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "trail length must be finite and positive")]
fn a_nonsense_requested_length_panics_in_debug() {
    let mut spec = two_body().spec;
    spec.trail_length_units = Some(-1.0);
    let _ = app_with_trails(Some(spec));
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// However far a body travels and however finely it is sampled, the trail never exceeds its
    /// budget and every stored point stays finite. This is the invariant the drawing system
    /// relies on instead of checking.
    #[test]
    fn a_trail_never_outgrows_its_budget(
        steps in 1_usize..200,
        step in 0.01_f32..2.0,
        max_length in 0.5_f32..20.0,
    ) {
        let mut path = TrailPath::default();

        for index in 0..steps {
            path.record(
                Vec3::new(index as f32 * step, 0.0, 0.0),
                max_length,
                0.05,
                TRAIL_MAX_SAMPLES,
            );

            prop_assert!(path.is_valid());
            prop_assert!(path.length_units() <= max_length + 1.0e-3);
            prop_assert!(path.len() <= TRAIL_MAX_SAMPLES);
        }
    }

    /// A trail's length is the distance travelled, capped at its budget — **whatever the
    /// per-frame step size**, including steps far below the sample spacing. This is the property
    /// the anchor bug violated: the old implementation recorded nothing at all below the spacing,
    /// and no test in the file explored that region.
    #[test]
    fn a_trail_measures_the_distance_travelled(
        per_frame in 0.001_f32..0.5,
        frames in 20_usize..400,
    ) {
        let mut path = TrailPath::default();

        for index in 0..frames {
            path.record(
                Vec3::new(index as f32 * per_frame, 0.0, 0.0),
                TRAIL_LENGTH_UNITS,
                TRAIL_MIN_SAMPLE_SPACING_UNITS,
                TRAIL_MAX_SAMPLES,
            );
        }

        let travelled = (frames - 1) as f32 * per_frame;
        let expected = travelled.min(TRAIL_LENGTH_UNITS);

        // The shortfall is bounded by one sample spacing: the head slides between commits, so the
        // stored path can trail the true one by up to that much.
        prop_assert!(path.length_units() <= expected + 1.0e-3);
        prop_assert!(
            path.length_units() >= expected - TRAIL_MIN_SAMPLE_SPACING_UNITS - 1.0e-3,
            "travelled {} but recorded {}",
            expected,
            path.length_units()
        );
    }

    /// Alpha is always within the two endpoints and never increases away from the body, whatever
    /// the path's shape.
    #[test]
    fn the_fade_is_always_monotone(
        points in prop::collection::vec(
            (-20.0_f32..20.0, -20.0_f32..20.0, -20.0_f32..20.0),
            2..40,
        ),
    ) {
        let mut path = TrailPath::default();
        for (x, y, z) in points {
            path.record(Vec3::new(x, y, z), 5.0, 0.05, TRAIL_MAX_SAMPLES);
        }

        let mut previous = f32::INFINITY;
        for segment in path.segments(1.0, 0.0) {
            prop_assert!((0.0..=1.0).contains(&segment.from_alpha));
            prop_assert!((0.0..=1.0).contains(&segment.to_alpha));
            prop_assert!(segment.from_alpha <= previous + 1.0e-6);
            prop_assert!(segment.to_alpha <= segment.from_alpha + 1.0e-6);
            previous = segment.to_alpha;
        }
    }

    /// The segments always join up into one continuous polyline starting at the body: a gap
    /// would draw as a dashed trail, and a wrong order would draw it inside out.
    #[test]
    fn the_segments_form_a_continuous_path_from_the_body(steps in 3_usize..80) {
        let mut path = TrailPath::default();
        for index in 0..steps {
            let angle = index as f32 * 0.3;
            path.record(
                Vec3::new(angle.cos() * 4.0, angle * 0.1, angle.sin() * 4.0),
                5.0,
                0.05,
                TRAIL_MAX_SAMPLES,
            );
        }

        let segments = path.segments(1.0, 0.0);
        prop_assume!(!segments.is_empty());

        prop_assert_eq!(segments[0].from, path.head().unwrap());
        for pair in segments.windows(2) {
            prop_assert_eq!(pair[0].to, pair[1].from);
        }
    }
}
