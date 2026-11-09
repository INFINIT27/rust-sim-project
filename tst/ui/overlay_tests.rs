//! Unit tests for the [`crate::ui::overlay`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/ui/overlay.rs`
//! via `#[path]`, not as a standalone integration test, so these tests keep
//! access to the module's private items and to `super::*`.

//! Unit tests for the `overlay` module.
//!
//! We deliberately do *not* spin up a Bevy `App` with `EguiPlugin`
//! here: `EguiPlugin` wires itself into the windowing and render
//! subsystems that ship with `DefaultPlugins`, and installing it
//! against `MinimalPlugins` panics at startup on systems without
//! a render backend. Instead we cover:
//!
//! - The clock readout rows (`clock_lines`, `human_duration`). The FPS reading
//!   itself moved to `ObserverClock::fps` and is tested in
//!   `tst/time/observer_tests.rs`.
//! - The plugin type itself: `DebugUiPlugin` must be
//!   zero-sized and constructible so the `add_plugins(...)` call
//!   in `main.rs` (task 10) compiles.
//!
//! The end-to-end "overlay draws without panicking" behaviour is
//! exercised by running the binary (`cargo run`), which is out of
//! scope for unit tests.

use super::*;
use crate::time::simulation::TimeStep;

/// `DebugUiPlugin` must be zero-sized and constructible so `main.rs` can add it.
#[test]
fn debug_ui_plugin_is_constructible() {
    fn takes_plugin<P: Plugin>(_p: P) {}
    takes_plugin(DebugUiPlugin);
}

// -----------------------------------------------------------------------
// The SI readout: per-prop velocity and pairwise distance.
//
// All of the formatting logic is in pure functions, so it is fully covered here
// with no egui context. What these tests are really guarding is the *unit
// conversion*: the scene works in world units (1 unit = 1e9 m) and the overlay is
// the only place besides `core::integrator` that converts to SI. A missing
// multiply would show a distance of "30" where it should read "3e10", which looks
// plausible and would be easy to miss by eye.
// -----------------------------------------------------------------------

use crate::constants::TWO_BODY_SEPARATION_UNITS;
use crate::core::scene::{Body, Color, Pose, Prop, Quat, Vec3};

/// A unit sphere at `position`, for building test props.
fn ball_at(position: Vec3) -> Body {
    Body::Sphere {
        pose: Pose {
            position,
            rotation: Quat::IDENTITY,
        },
        radius: 1.0,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    }
}

/// `units_to_metres` scales by exactly `METRES_PER_UNIT`.
#[test]
fn units_to_metres_scales_by_the_world_unit() {
    assert_eq!(units_to_metres(1.0), METRES_PER_UNIT);
    assert_eq!(units_to_metres(0.0), 0.0);
    assert_eq!(units_to_metres(30.0), 30.0 * METRES_PER_UNIT);
    assert_eq!(units_to_metres(-2.5), -2.5 * METRES_PER_UNIT);
}

/// A 30-unit separation reads as `3e10 m`, not as `30`.
///
/// The single most likely defect in this readout is a forgotten conversion, which
/// would print a plausible-looking number three orders of magnitude too small
/// squared. Pinned against the scene's own constant.
#[test]
fn a_thirty_unit_separation_reads_as_three_times_ten_to_the_ten_metres() {
    assert_eq!(units_to_metres(TWO_BODY_SEPARATION_UNITS), 3.0e10);
}

/// The shipped scene produces one row per prop, correctly labelled.
#[test]
fn prop_lines_describes_every_prop_in_si_units() {
    let lines = prop_lines(&Scene::two_body_gravity());

    assert_eq!(lines.len(), 3, "floor plus two spheres");

    assert!(lines[0].contains("#0 Plane"), "got {:?}", lines[0]);
    assert!(
        lines[0].contains("[immovable]"),
        "the floor must be tagged: {:?}",
        lines[0]
    );
    assert!(
        !lines[0].contains("|v|"),
        "an immovable prop has no velocity worth printing: {:?}",
        lines[0]
    );

    for line in &lines[1..] {
        assert!(line.contains("Sphere"), "got {:?}", line);
        assert!(
            line.contains("5.972e24 kg"),
            "mass must be in kg: {:?}",
            line
        );
        assert!(
            line.contains("|v|=0.000e0 m/s"),
            "starts at rest: {:?}",
            line
        );
        assert!(line.contains("m/s"), "velocity must be in m/s: {:?}", line);
    }
}

/// Velocity is reported in metres per second, not units per second.
///
/// A prop moving at 1 unit/s is moving at `1e9 m/s`. Getting this wrong would
/// understate every speed by a factor of a billion.
#[test]
fn velocity_is_reported_in_metres_per_second() {
    let mut scene = Scene::demo();
    scene.add(Prop::new(
        ball_at(Vec3::ZERO),
        Vec3::new(3.0, 0.0, -4.0),
        2.0,
    ));

    let lines = prop_lines(&scene);
    let sphere = &lines[1];

    // |(3, 0, -4)| = 5 units/s = 5e9 m/s.
    assert!(
        sphere.contains("|v|=5.000e9 m/s"),
        "expected 5e9 m/s, got {:?}",
        sphere
    );
    assert!(
        sphere.contains("v=(3.00e9, 0.00e0, -4.00e9)"),
        "components must be scaled and signed: {:?}",
        sphere
    );
}

/// Each unordered pair appears exactly once, with the distance in metres.
#[test]
fn distance_lines_lists_each_pair_once() {
    let lines = distance_lines(&Scene::two_body_gravity());

    assert_eq!(lines.len(), 3, "three props give three pairs");
    assert!(lines.iter().any(|l| l.starts_with("#0-#1: ")));
    assert!(lines.iter().any(|l| l.starts_with("#0-#2: ")));
    assert!(lines.iter().any(|l| l.starts_with("#1-#2: ")));

    // The two spheres start `TWO_BODY_SEPARATION_UNITS` apart.
    let sphere_pair = lines.iter().find(|l| l.starts_with("#1-#2")).unwrap();
    assert_eq!(sphere_pair, "#1-#2: 3.0000e10 m");
}

/// Distance is symmetric, so the reverse pair is never also listed.
#[test]
fn distance_lines_does_not_repeat_a_pair_in_reverse() {
    let lines = distance_lines(&Scene::two_body_gravity());

    assert!(
        !lines.iter().any(|l| l.starts_with("#1-#0")),
        "a reversed duplicate would double the list: {:?}",
        lines
    );
}

/// A single prop has no pairs at all.
#[test]
fn a_lone_prop_has_no_distance_rows() {
    assert!(distance_lines(&Scene::demo()).is_empty());
}

/// The prop list truncates loudly rather than silently.
#[test]
fn prop_lines_reports_how_many_props_it_omitted() {
    let mut scene = Scene::demo();
    for index in 0..19 {
        scene.add(Prop::at_rest(ball_at(Vec3::splat(index as f32)), 1.0));
    }
    assert_eq!(scene.props.len(), 20);

    let lines = prop_lines(&scene);

    assert_eq!(lines.len(), MAX_PROP_ROWS + 1, "rows plus one summary");
    assert_eq!(
        lines.last().unwrap(),
        &format!("... and {} more prop(s)", 20 - MAX_PROP_ROWS),
        "the count must be stated, not dropped"
    );
}

/// The distance list truncates loudly too.
///
/// Pair count grows as `n(n-1)/2`, so this limit is reached well before the prop
/// limit — six props already make 15 pairs.
#[test]
fn distance_lines_reports_how_many_pairs_it_omitted() {
    let mut scene = Scene::demo();
    for index in 0..5 {
        scene.add(Prop::at_rest(ball_at(Vec3::splat(index as f32 + 1.0)), 1.0));
    }
    assert_eq!(scene.props.len(), 6);

    let lines = distance_lines(&scene);
    let total_pairs = 6 * 5 / 2;

    assert_eq!(lines.len(), MAX_DISTANCE_ROWS + 1);
    assert_eq!(
        lines.last().unwrap(),
        &format!("... and {} more pair(s)", total_pairs - MAX_DISTANCE_ROWS)
    );
}

/// The readout tracks the simulation as it runs.
///
/// Ties the formatting to the integrator: after stepping, the spheres must be
/// closer and no longer at rest. Without this, every test above would still pass on
/// a readout that printed the starting state forever.
#[test]
fn the_readout_follows_the_simulation() {
    let mut scene = Scene::two_body_gravity();

    let before = distance_lines(&scene)
        .into_iter()
        .find(|l| l.starts_with("#1-#2"))
        .unwrap();

    for _ in 0..500 {
        scene.step(TimeStep::from_seconds(1.0e5).expect("a day and a bit is a valid step"));
    }

    let after_distance = distance_lines(&scene)
        .into_iter()
        .find(|l| l.starts_with("#1-#2"))
        .unwrap();
    let after_prop = prop_lines(&scene).remove(1);

    assert_ne!(
        before, after_distance,
        "the distance must change as they fall"
    );
    assert!(
        !after_prop.contains("|v|=0.000e0 m/s"),
        "the spheres must no longer be at rest: {:?}",
        after_prop
    );
}

// -----------------------------------------------------------------------
// The two-clock readout.
//
// `clock_lines` is what makes the simulated/observer split visible in the running
// app, so these pin the three rows and the state word rather than just the numbers.
// -----------------------------------------------------------------------

/// A running clock reports both elapsed times, the scale, and `running`.
#[test]
fn clock_lines_show_both_clocks_and_the_scale() {
    let mut simulation = SimulationClock::new(1000.0);
    let mut observer = ObserverClock::new();

    // Ten frames of 20 ms: 0.2 s real, 200 s simulated.
    for _ in 0..10 {
        observer.advance(0.02);
        simulation.advance(0.02);
    }

    let rows = clock_lines(&simulation, &observer);
    assert_eq!(rows.len(), 3);
    assert!(rows[0].starts_with("sim   2.000e2 s"), "got {:?}", rows[0]);
    assert!(
        rows[0].contains("x1.000e3"),
        "the scale belongs on the sim row: {:?}",
        rows[0]
    );
    assert!(rows[1].starts_with("real  0.20 s"), "got {:?}", rows[1]);
    assert!(rows[1].contains("50.0 fps"), "got {:?}", rows[1]);
    assert_eq!(rows[2], "clock running");
}

/// Pausing must be *visible*, not inferred from the numbers having stopped, and a
/// queued single step must say so.
#[test]
fn clock_lines_report_the_pause_state() {
    let mut simulation = SimulationClock::new(1000.0);
    let observer = ObserverClock::new();

    simulation.set_paused(true);
    assert_eq!(clock_lines(&simulation, &observer)[2], "clock paused");

    simulation.request_single_step();
    assert_eq!(
        clock_lines(&simulation, &observer)[2],
        "clock paused (step queued)"
    );
}

/// `human_duration` picks the largest unit that leaves the value above one, which is
/// the whole reason the row exists: `4.2e7 s` says nothing, `1.3 y` says everything.
#[test]
fn human_duration_picks_a_readable_unit() {
    assert_eq!(human_duration(0.0), "0.0 s");
    assert_eq!(human_duration(45.0), "45.0 s");
    assert_eq!(human_duration(90.0), "1.5 min");
    assert_eq!(human_duration(5400.0), "1.5 h");
    assert_eq!(human_duration(129_600.0), "1.5 d");
    // The shipped scene's free-fall time, about 6.5 years.
    assert_eq!(human_duration(2.04e8), "6.5 y");
}

/// Non-finite or negative durations render as a dash rather than a wrong number.
#[test]
fn human_duration_refuses_to_invent_a_figure() {
    assert_eq!(human_duration(f64::NAN), "-");
    assert_eq!(human_duration(f64::INFINITY), "-");
    assert_eq!(human_duration(-1.0), "-");
}

// -----------------------------------------------------------------------
// The camera speed row (amendment A26).
//
// This row is not decoration. A held modifier announces itself; a mode does
// not, so the readout is the only thing that tells a user why their camera
// is 150x slower than they expect. Tested as the load-bearing part it is.
// -----------------------------------------------------------------------

#[test]
fn the_speed_row_names_the_mode_and_its_multiplier() {
    use crate::input::camera::{CameraSpeed, FreeFlyController};

    let controller = FreeFlyController::default();

    assert_eq!(
        camera_speed_line(CameraSpeed::Normal, &controller),
        "cam   normal  x1.00"
    );
    assert_eq!(
        camera_speed_line(CameraSpeed::Fast, &controller),
        "cam   fast    x3.00"
    );
    assert_eq!(
        camera_speed_line(CameraSpeed::Slow, &controller),
        "cam   slow    x0.02"
    );
}

#[test]
fn every_mode_produces_a_distinguishable_row() {
    // The point of the row is noticing at a glance that you are not in `normal`. Three rows that
    // rendered alike would defeat it, and so would a mode whose label was empty.
    use crate::input::camera::{CameraSpeed, FreeFlyController};

    let controller = FreeFlyController::default();
    let modes = [CameraSpeed::Slow, CameraSpeed::Normal, CameraSpeed::Fast];
    let rows: Vec<String> = modes
        .iter()
        .map(|mode| camera_speed_line(*mode, &controller))
        .collect();

    for (index, row) in rows.iter().enumerate() {
        assert!(!modes[index].label().is_empty());
        assert!(
            row.contains(modes[index].label()),
            "{row:?} should name its mode"
        );

        for (other, row_other) in rows.iter().enumerate() {
            if other != index {
                assert_ne!(row, row_other, "two modes rendered the same row");
            }
        }
    }
}

#[test]
fn the_multipliers_line_up_in_a_column() {
    // The names have different lengths, so the row pads them. Without that the `x` values sit at
    // three different offsets and the column stops being scannable — which is the one thing this
    // row has to be.
    use crate::input::camera::{CameraSpeed, FreeFlyController};

    let controller = FreeFlyController::default();
    let offsets: Vec<usize> = [CameraSpeed::Slow, CameraSpeed::Normal, CameraSpeed::Fast]
        .iter()
        .map(|mode| {
            camera_speed_line(*mode, &controller)
                .find('x')
                .expect("every row quotes a multiplier")
        })
        .collect();

    assert!(
        offsets.iter().all(|offset| *offset == offsets[0]),
        "the multiplier column is ragged: {offsets:?}"
    );
}

#[test]
fn the_row_honours_a_controller_with_custom_multipliers() {
    // Reads the controller rather than the constants, so a camera built with different multipliers
    // is described correctly instead of being described as the default.
    use crate::input::camera::{CameraSpeed, FreeFlyController};

    let controller = FreeFlyController {
        boost_mult: 12.5,
        precision_mult: 0.5,
        ..FreeFlyController::default()
    };

    assert!(camera_speed_line(CameraSpeed::Fast, &controller).ends_with("x12.50"));
    assert!(camera_speed_line(CameraSpeed::Slow, &controller).ends_with("x0.50"));
}
