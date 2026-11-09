//! Unit tests for the [`crate::experiments::scale_check`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/experiments/scale_check.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! # What these tests are for
//!
//! `experiment_5` is a measuring instrument, so the tests check the two things an instrument has to
//! get right: that it is **measuring the real quantity** (the radii are the same figures
//! `experiment_3` orbits, and the ratios match published astronomy), and that it is **readable**
//! (every body fits the opening frame and is more than a pixel across, since nothing floors apparent
//! size any more).
//!
//! There is a third: that it cannot move. A static scene that quietly starts collapsing is worse
//! than no scene, because you would be reading a ruler that had bent.

use super::*;
use crate::core::scene::Prop;
use crate::experiments::constants::{
    SCALE_CHECK_CAMERA_OFFSET_UNITS, SCALE_CHECK_FRAMED_RADIUS_UNITS, SCALE_CHECK_GAP_UNITS,
    SOLAR_SYSTEM_PLANETS, SOLAR_SYSTEM_SUN_RADIUS_M,
};

fn radius_of(prop: &Prop) -> f32 {
    match &prop.body {
        Body::Sphere { radius, .. } => *radius,
        other => panic!("expected a sphere, got {other:?}"),
    }
}

// -----------------------------------------------------------------------
// It measures the real thing.
// -----------------------------------------------------------------------

#[test]
fn the_row_is_the_sun_and_the_eight_planets_in_order() {
    let rows = scale_rows();

    assert_eq!(rows.len(), SOLAR_SYSTEM_PLANETS.len() + 1);
    assert_eq!(rows[0].name, "Sun");

    for (row, planet) in rows[1..].iter().zip(SOLAR_SYSTEM_PLANETS) {
        assert_eq!(row.name, planet.name);
    }
}

#[test]
fn every_radius_is_the_one_experiment_3_orbits() {
    // The instrument has to be measuring the same object as the experiment, or it proves nothing.
    // Radii come from `SOLAR_SYSTEM_PLANETS` and the Sun's own constant rather than a second table.
    let rows = scale_rows();

    assert_eq!(rows[0].radius_m, SOLAR_SYSTEM_SUN_RADIUS_M);
    for (row, planet) in rows[1..].iter().zip(SOLAR_SYSTEM_PLANETS) {
        assert_eq!(row.radius_m, planet.radius_m, "{} disagrees", row.name);
    }
}

#[test]
fn the_ratios_on_screen_are_the_published_ones() {
    // What a viewer actually reads off the row, checked against figures anyone can look up. These
    // are the numbers the experiment exists to let you verify by eye.
    let experiment = scale_check();
    let props = &experiment.scene.props;

    let sun = radius_of(&props[0]);
    let earth = radius_of(&props[3]);
    let jupiter = radius_of(&props[5]);
    let mercury = radius_of(&props[1]);

    assert!(
        (sun / earth - 109.2).abs() < 0.2,
        "Sun:Earth = {}",
        sun / earth
    );
    assert!(
        (sun / jupiter - 9.95).abs() < 0.05,
        "Sun:Jupiter = {}",
        sun / jupiter
    );
    assert!(
        (jupiter / earth - 10.97).abs() < 0.05,
        "Jupiter:Earth = {}",
        jupiter / earth
    );
    assert!(
        (earth / mercury - 2.611).abs() < 0.01,
        "Earth:Mercury = {}",
        earth / mercury
    );
}

// -----------------------------------------------------------------------
// The layout.
// -----------------------------------------------------------------------

#[test]
fn bottoms_are_aligned_on_the_plane() {
    // The comparison is vertical — a body's height above `y = 0` is its diameter — which is what
    // makes the row readable as a chart rather than as a scatter of spheres.
    let rows = scale_rows();
    let (centres, _) = scale_check_layout(&rows);

    for (row, centre) in rows.iter().zip(&centres) {
        let radius = radius_units(row.radius_m);
        assert!(
            (centre.y - radius).abs() < 1.0e-9,
            "{} sits at y = {}, expected its radius {radius}",
            row.name,
            centre.y
        );
        assert_eq!(centre.z, 0.0, "{} is off the row's axis", row.name);
    }
}

#[test]
fn the_row_runs_left_to_right_with_the_documented_gap() {
    let rows = scale_rows();
    let (centres, length) = scale_check_layout(&rows);

    for index in 1..rows.len() {
        let left = radius_units(rows[index - 1].radius_m);
        let right = radius_units(rows[index].radius_m);
        let separation = centres[index].x - centres[index - 1].x;

        assert!(
            (separation - (left + right + SCALE_CHECK_GAP_UNITS)).abs() < 1.0e-6,
            "{} to {} is {separation} units apart",
            rows[index - 1].name,
            rows[index].name
        );
    }

    // The whole row is under two units long, which is the only reason nine bodies spanning 109:1 in
    // radius can share one view at true scale. The solar system is 4495 units across.
    assert!(
        (length - 1.944).abs() < 0.01,
        "the row is {length} units long"
    );
}

#[test]
fn an_empty_row_yields_an_empty_layout() {
    let (centres, length) = scale_check_layout(&[]);

    assert!(centres.is_empty());
    assert_eq!(length, 0.0);
}

// -----------------------------------------------------------------------
// It is readable.
// -----------------------------------------------------------------------

#[test]
fn every_body_fits_the_opening_frame() {
    // Recomputed from the layout rather than compared to a stored figure, so lengthening the row —
    // by widening the gap, or adding a body — fails here rather than pushing Neptune off screen.
    let (_, length) = scale_check_layout(&scale_rows());
    let half = 0.5 * length;

    assert!(
        SCALE_CHECK_FRAMED_RADIUS_UNITS >= half,
        "the framed radius {SCALE_CHECK_FRAMED_RADIUS_UNITS} must cover half the row's {length}"
    );

    let distance = SCALE_CHECK_CAMERA_OFFSET_UNITS.length();
    let reach = distance * (45.0_f32.to_radians() / 2.0).tan();
    assert!(
        reach >= SCALE_CHECK_FRAMED_RADIUS_UNITS,
        "a camera {distance} units back frames {reach}, short of the declared \
         {SCALE_CHECK_FRAMED_RADIUS_UNITS}"
    );
}

#[test]
fn every_body_is_more_than_a_pixel_across() {
    // The property that makes this experiment worth having: nothing floors apparent size any more
    // (amendment A17), so a body is readable here only because removing the *distances* brought the
    // whole 109:1 range inside one frame. Mercury is the tight case at 3.6 px.
    let experiment = scale_check();
    let pixels_per_radian = 1600.0 / (45.0_f32).to_radians();
    let distance = SCALE_CHECK_CAMERA_OFFSET_UNITS.length();

    for prop in &experiment.scene.props {
        let pixels = 2.0 * radius_of(prop) / distance * pixels_per_radian;

        assert!(
            pixels > 3.0,
            "a body spans only {pixels:.2} px, which is not readable"
        );
    }

    // And the Sun still fits the frame vertically.
    let sun_pixels = 2.0 * radius_of(&experiment.scene.props[0]) / distance * pixels_per_radian;
    assert!(
        (900.0..1600.0).contains(&sun_pixels),
        "the Sun spans {sun_pixels:.0} px; it should fill most of the view without overflowing"
    );
}

// -----------------------------------------------------------------------
// It cannot move.
// -----------------------------------------------------------------------

#[test]
fn nothing_can_move_and_nothing_pulls() {
    // A real Sun beside a real Mercury 0.72 units away would not survive one frame. `immovable`
    // also zeroes the mass, so the row is geometry rather than an initial condition — a ruler that
    // cannot bend.
    let experiment = scale_check();

    for prop in &experiment.scene.props {
        assert!(prop.immovable, "every body in a ruler must be fixed");
        assert_eq!(prop.mass_kg, 0.0, "and gravitationally inert");
        assert_eq!(prop.velocity_units_s, Vec3::ZERO);
        assert!(prop.is_valid());
    }
}

#[test]
fn stepping_the_scene_changes_nothing_at_all() {
    // The strongest statement of "static": run it through the real integrator and the contact
    // solver at the app's worst-case frame and compare the whole scene for equality.
    use crate::time::constants::MAX_REAL_STEP_SECONDS;
    use crate::time::simulation::SimulationClock;

    let Experiment {
        mut scene, spec, ..
    } = scale_check();
    let before = scene.clone();
    let mut clock = SimulationClock::new(spec.time_scale);

    for _ in 0..120 {
        scene.step(clock.advance(MAX_REAL_STEP_SECONDS));
    }

    assert_eq!(scene, before, "a ruler must not move");
}

#[test]
fn there_is_no_floor() {
    // The row is 1.94 units long against a 5000-unit ground plane, which would put the whole
    // instrument inside a single grid cell.
    let experiment = scale_check();

    assert!(!experiment
        .scene
        .props
        .iter()
        .any(|prop| matches!(prop.body, Body::Plane { .. })));
    assert!(crate::render::grid::floor_half_size(&experiment.scene).is_none());
}

#[test]
fn the_spec_is_valid_and_static() {
    let spec = scale_check().spec;

    assert_eq!(spec.id, ExperimentId::ScaleCheck);
    assert!(spec.is_valid());
    assert_eq!(
        spec.trail_length_units, None,
        "immovable props are never trailed"
    );
}

#[test]
fn the_bounds_enclose_the_whole_row() {
    let experiment = scale_check();
    let bounds = experiment.scene.bounds;

    assert!(bounds.is_valid());
    for prop in &experiment.scene.props {
        let centre = prop.body.center();
        assert!(
            centre.x >= bounds.min.x
                && centre.x <= bounds.max.x
                && centre.y >= bounds.min.y
                && centre.y <= bounds.max.y,
            "{centre:?} is outside {bounds:?}"
        );
    }
}
