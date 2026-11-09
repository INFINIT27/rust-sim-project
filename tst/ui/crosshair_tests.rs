//! Unit and property tests for the [`crate::ui::crosshair`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/ui/crosshair.rs`
//! via `#[path]`, not as a standalone integration test, so these tests keep
//! access to the module's private items — including `to_egui_color` — and to
//! `super::*`.
//!
//! All of the crosshair's real logic lives in `crosshair_segments`, which takes
//! and returns plain `f32` pairs, so it is fully covered here with no egui
//! context, no window, and no GPU. `crosshair_system` itself is the same thin
//! egui-calling shell as `debug_overlay_system` and shares its coverage gap —
//! see `tst/ui/README.md`.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
const REGRESSION_SEEDS: &str = "tst/ui/proptest-regressions/crosshair_tests.txt";

/// Index of each stroke in `crosshair_segments`' fixed return order.
const LEFT: usize = 0;
const RIGHT: usize = 1;
const UP: usize = 2;
const DOWN: usize = 3;

/// Screen coordinates, in logical pixels. Bounded to plausible window sizes with
/// room to spare.
fn valid_coordinate() -> impl Strategy<Value = f32> {
    -1.0e4..1.0e4_f32
}

/// Non-negative pixel lengths.
fn valid_length() -> impl Strategy<Value = f32> {
    0.0..1.0e3_f32
}

/// Euclidean length of a segment.
fn length_of([(start_x, start_y), (end_x, end_y)]: [(f32, f32); 2]) -> f32 {
    ((end_x - start_x).powi(2) + (end_y - start_y).powi(2)).sqrt()
}

/// Absolute tolerance for a quantity recovered by subtracting coordinates of
/// magnitude `scale`.
///
/// These are `f32` screen coordinates, and the properties below measure offsets
/// by *round-tripping* through an absolute position: computing `centre - offset`
/// and then subtracting `centre` again. `f32` has ~24 bits of mantissa, so at a
/// coordinate of 9000 px the spacing between representable values is already
/// ~1e-3 — the recovered offset can differ from the original by that much even
/// though the implementation is exactly symmetric by construction.
///
/// A fixed epsilon is therefore wrong: it is either vacuous near the origin or
/// impossible at the edge of a large window. Scaling with the coordinate keeps the
/// properties meaningful — a sign error or a swapped arm is off by twice the
/// offset, not by one ulp — while tolerating representation loss.
fn magnitude_epsilon(scale: f32) -> f32 {
    1.0e-4 * scale.abs().max(1.0)
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1: every stroke is exactly `arm_length_px` long.
    ///
    /// The core guarantee of defining the arm as a length measured from the end of
    /// the gap: the stroke size is independent of the gap, so tuning one does not
    /// silently resize the other.
    #[test]
    fn every_stroke_is_arm_length_long(
        center_x in valid_coordinate(),
        center_y in valid_coordinate(),
        arm in valid_length(),
        gap in valid_length(),
    ) {
        for segment in crosshair_segments(center_x, center_y, arm, gap) {
            let measured = length_of(segment);
            prop_assert!(
                (measured - arm).abs() <= 1.0e-3 * arm.max(1.0),
                "expected length {}, got {} for {:?}", arm, measured, segment
            );
        }
    }

    /// Property 2: the crosshair is symmetric about its centre.
    ///
    /// Left mirrors right and up mirrors down. A sign error in any one arm — the
    /// most likely typo in this function — breaks this while leaving the lengths
    /// in Property 1 intact.
    ///
    /// Compared within [`magnitude_epsilon`] rather than bit-exactly: the offsets
    /// are recovered by subtracting the centre back out, which loses up to an ulp
    /// of the coordinate. A broken arm is wrong by twice the offset, so the
    /// tolerance costs the property nothing.
    #[test]
    fn the_crosshair_is_symmetric_about_its_centre(
        center_x in valid_coordinate(),
        center_y in valid_coordinate(),
        arm in valid_length(),
        gap in valid_length(),
    ) {
        let segments = crosshair_segments(center_x, center_y, arm, gap);
        let slack_x = magnitude_epsilon(center_x.abs() + arm + gap);
        let slack_y = magnitude_epsilon(center_y.abs() + arm + gap);

        // Horizontal pair: distances from the centre match on both sides.
        let outer_left = center_x - segments[LEFT][0].0;
        let outer_right = segments[RIGHT][1].0 - center_x;
        let inner_left = center_x - segments[LEFT][1].0;
        let inner_right = segments[RIGHT][0].0 - center_x;
        prop_assert!(
            (outer_left - outer_right).abs() <= slack_x,
            "outer arms asymmetric: {} vs {}", outer_left, outer_right
        );
        prop_assert!(
            (inner_left - inner_right).abs() <= slack_x,
            "inner ends asymmetric: {} vs {}", inner_left, inner_right
        );

        // Vertical pair.
        let outer_up = center_y - segments[UP][0].1;
        let outer_down = segments[DOWN][1].1 - center_y;
        let inner_up = center_y - segments[UP][1].1;
        let inner_down = segments[DOWN][0].1 - center_y;
        prop_assert!(
            (outer_up - outer_down).abs() <= slack_y,
            "outer arms asymmetric: {} vs {}", outer_up, outer_down
        );
        prop_assert!(
            (inner_up - inner_down).abs() <= slack_y,
            "inner ends asymmetric: {} vs {}", inner_up, inner_down
        );
    }

    /// Property 3: the strokes are axis-aligned.
    ///
    /// The horizontal pair shares the centre's `y`, the vertical pair its `x`. A
    /// crosshair whose arms drifted diagonally would still satisfy the length and
    /// symmetry properties.
    #[test]
    fn the_strokes_are_axis_aligned(
        center_x in valid_coordinate(),
        center_y in valid_coordinate(),
        arm in valid_length(),
        gap in valid_length(),
    ) {
        let segments = crosshair_segments(center_x, center_y, arm, gap);

        for index in [LEFT, RIGHT] {
            prop_assert_eq!(segments[index][0].1, center_y);
            prop_assert_eq!(segments[index][1].1, center_y);
        }
        for index in [UP, DOWN] {
            prop_assert_eq!(segments[index][0].0, center_x);
            prop_assert_eq!(segments[index][1].0, center_x);
        }
    }

    /// Property 4: nothing is ever drawn inside the gap.
    ///
    /// Every endpoint is at least `gap_px` from the centre along its own axis, so
    /// the pixel the crosshair points at stays visible. This is the property that
    /// would break if the arm were defined as an outer radius and `gap_px`
    /// exceeded it.
    #[test]
    fn no_stroke_intrudes_into_the_gap(
        center_x in valid_coordinate(),
        center_y in valid_coordinate(),
        arm in valid_length(),
        gap in valid_length(),
    ) {
        let segments = crosshair_segments(center_x, center_y, arm, gap);
        let slack_x = magnitude_epsilon(center_x.abs() + arm + gap);
        let slack_y = magnitude_epsilon(center_y.abs() + arm + gap);

        for index in [LEFT, RIGHT] {
            for point in segments[index] {
                prop_assert!(
                    (point.0 - center_x).abs() >= gap - slack_x,
                    "{:?} is closer than the {} px gap", point, gap
                );
            }
        }
        for index in [UP, DOWN] {
            for point in segments[index] {
                prop_assert!(
                    (point.1 - center_y).abs() >= gap - slack_y,
                    "{:?} is closer than the {} px gap", point, gap
                );
            }
        }
    }

    /// Property 5: every coordinate is finite for valid input.
    #[test]
    fn every_coordinate_is_finite(
        center_x in valid_coordinate(),
        center_y in valid_coordinate(),
        arm in valid_length(),
        gap in valid_length(),
    ) {
        for [(start_x, start_y), (end_x, end_y)] in crosshair_segments(center_x, center_y, arm, gap) {
            prop_assert!(start_x.is_finite() && start_y.is_finite());
            prop_assert!(end_x.is_finite() && end_y.is_finite());
        }
    }

    /// Property 6: moving the centre translates the whole crosshair rigidly.
    ///
    /// The shape depends only on `arm_length_px` and `gap_px`, never on where it is
    /// drawn — so a crosshair centred in a resized window is the same crosshair.
    #[test]
    fn moving_the_centre_translates_the_shape(
        center_x in -1.0e3..1.0e3_f32,
        center_y in -1.0e3..1.0e3_f32,
        shift_x in -1.0e3..1.0e3_f32,
        shift_y in -1.0e3..1.0e3_f32,
        arm in valid_length(),
        gap in valid_length(),
    ) {
        let original = crosshair_segments(center_x, center_y, arm, gap);
        let moved = crosshair_segments(center_x + shift_x, center_y + shift_y, arm, gap);
        let slack = magnitude_epsilon(
            center_x.abs() + center_y.abs() + shift_x.abs() + shift_y.abs() + arm + gap,
        );

        for (from, to) in original.iter().zip(moved.iter()) {
            for (point, shifted) in from.iter().zip(to.iter()) {
                prop_assert!(
                    (shifted.0 - (point.0 + shift_x)).abs() <= slack,
                    "x: {} vs {}", shifted.0, point.0 + shift_x
                );
                prop_assert!(
                    (shifted.1 - (point.1 + shift_y)).abs() <= slack,
                    "y: {} vs {}", shifted.1, point.1 + shift_y
                );
            }
        }
    }
}

// ---- Geometry: hand-written cases ----

/// The worked example from the doc comment, pinned exactly.
///
/// 4 px arms with a 2 px gap at `(100, 50)`: whole numbers throughout, so exact
/// equality is legitimate and any change to the layout formula shows up here
/// first.
#[test]
fn crosshair_segments_matches_the_documented_layout() {
    let [left, right, up, down] = crosshair_segments(100.0, 50.0, 4.0, 2.0);

    assert_eq!(left, [(94.0, 50.0), (98.0, 50.0)]);
    assert_eq!(right, [(102.0, 50.0), (106.0, 50.0)]);
    assert_eq!(up, [(100.0, 44.0), (100.0, 48.0)]);
    assert_eq!(down, [(100.0, 52.0), (100.0, 56.0)]);
}

/// A zero gap makes the four strokes meet, giving a solid cross.
#[test]
fn a_zero_gap_makes_the_strokes_meet_at_the_centre() {
    let segments = crosshair_segments(0.0, 0.0, 5.0, 0.0);

    assert_eq!(segments[LEFT], [(-5.0, 0.0), (0.0, 0.0)]);
    assert_eq!(segments[RIGHT], [(0.0, 0.0), (5.0, 0.0)]);
    assert_eq!(segments[UP], [(0.0, -5.0), (0.0, 0.0)]);
    assert_eq!(segments[DOWN], [(0.0, 0.0), (0.0, 5.0)]);
}

/// A zero arm length collapses every stroke to a point, drawing nothing.
///
/// The documented way to hide the crosshair without removing the plugin.
#[test]
fn a_zero_arm_length_draws_nothing() {
    let segments = crosshair_segments(10.0, 20.0, 0.0, 3.0);

    for segment in segments {
        assert_eq!(
            segment[0], segment[1],
            "a zero-length arm must be a degenerate point, got {:?}",
            segment
        );
    }
}

/// A gap far larger than the arms still produces four forward segments.
///
/// The case that would reverse or cross over if the arm were an outer radius
/// rather than a length beyond the gap.
#[test]
fn a_gap_larger_than_the_arms_is_still_well_formed() {
    let segments = crosshair_segments(0.0, 0.0, 1.0, 50.0);

    assert_eq!(segments[LEFT], [(-51.0, 0.0), (-50.0, 0.0)]);
    assert_eq!(segments[RIGHT], [(50.0, 0.0), (51.0, 0.0)]);

    for segment in segments {
        assert!(
            length_of(segment) > 0.0,
            "segment {:?} must be non-degenerate",
            segment
        );
    }
}

/// The return order is left, right, up, down.
///
/// Pinned because the constants above and any future caller depend on it.
#[test]
fn the_return_order_is_left_right_up_down() {
    let segments = crosshair_segments(0.0, 0.0, 2.0, 1.0);

    assert!(segments[LEFT][0].0 < 0.0, "index 0 must be the left stroke");
    assert!(
        segments[RIGHT][0].0 > 0.0,
        "index 1 must be the right stroke"
    );
    assert!(segments[UP][0].1 < 0.0, "index 2 must be the up stroke");
    assert!(segments[DOWN][0].1 > 0.0, "index 3 must be the down stroke");
}

// ---- Config and colour ----

/// The default appearance is pinned field by field.
///
/// A tuning change should be a deliberate edit that updates this test, not a
/// silent drift in how the app looks.
#[test]
fn crosshair_config_default_matches_spec() {
    let config = CrosshairConfig::default();

    assert_eq!(config.arm_length_px, 6.0);
    assert_eq!(config.gap_px, 3.0);
    assert_eq!(config.thickness_px, 1.0);
    assert_eq!(config.color, [255, 255, 255, 200]);
}

/// The default config produces a visible, well-formed crosshair.
///
/// Ties the two halves together: the defaults are not merely the documented
/// numbers, they actually draw something.
#[test]
fn the_default_config_produces_a_visible_crosshair() {
    let config = CrosshairConfig::default();
    let segments = crosshair_segments(400.0, 300.0, config.arm_length_px, config.gap_px);

    for segment in segments {
        assert!(
            (length_of(segment) - config.arm_length_px).abs() < 1.0e-4,
            "default arm should be {} px, got {:?}",
            config.arm_length_px,
            segment
        );
    }
}

/// Colour bytes reach egui unchanged, with straight alpha.
///
/// `from_rgba_unmultiplied` is the load-bearing choice: the premultiplied
/// constructor would darken the stroke as its alpha fell, so a translucent white
/// crosshair would render grey.
#[test]
fn to_egui_color_preserves_srgb_bytes_with_straight_alpha() {
    let color = to_egui_color([255, 255, 255, 200]);

    assert_eq!(
        color,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)
    );
    assert_eq!(color.a(), 200);

    // Fully opaque colours round-trip exactly, in both constructors.
    assert_eq!(
        to_egui_color([12, 34, 56, 255]),
        egui::Color32::from_rgb(12, 34, 56)
    );
}

/// `CrosshairPlugin` satisfies `impl Plugin`.
///
/// A compile-level check, which is all `main.rs`'s `add_plugins(CrosshairPlugin)`
/// needs. The system itself cannot be run here: `EguiPlugin::build` requires an
/// `Assets<Shader>` resource that `MinimalPlugins` does not provide — see
/// `tst/ui/README.md` for the full headless recipe.
#[test]
fn crosshair_plugin_is_constructible() {
    fn takes_plugin<P: Plugin>(_plugin: P) {}

    takes_plugin(CrosshairPlugin);
}

// ---- Precondition violations ----
//
// Split by build profile, mirroring `tst/core/scene_tests.rs`: descriptive
// `debug_assert!` panics in debug, a documented benign value in release. Both
// `cargo test` and `cargo test --release` must pass.

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "crosshair_segments: centre must be finite")]
fn a_non_finite_centre_panics_in_debug() {
    crosshair_segments(f32::NAN, 0.0, 1.0, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "crosshair_segments: arm_length_px")]
fn a_negative_arm_length_panics_in_debug() {
    crosshair_segments(0.0, 0.0, -1.0, 1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "crosshair_segments: gap_px")]
fn a_non_finite_gap_panics_in_debug() {
    crosshair_segments(0.0, 0.0, 1.0, f32::INFINITY);
}

/// In release, invalid input collapses every stroke to the origin.
///
/// Drawing nothing is the right failure mode here: a `NaN` coordinate would smear
/// an undefined stroke across the frame, and egui would happily try.
#[test]
#[cfg(not(debug_assertions))]
fn precondition_violations_draw_nothing_in_release() {
    for segments in [
        crosshair_segments(f32::NAN, 0.0, 1.0, 1.0),
        crosshair_segments(0.0, f32::INFINITY, 1.0, 1.0),
        crosshair_segments(0.0, 0.0, -1.0, 1.0),
        crosshair_segments(0.0, 0.0, f32::NAN, 1.0),
        crosshair_segments(0.0, 0.0, 1.0, -1.0),
        crosshair_segments(0.0, 0.0, 1.0, f32::NAN),
    ] {
        for segment in segments {
            assert_eq!(
                segment,
                [(0.0, 0.0), (0.0, 0.0)],
                "invalid input must collapse to a degenerate point at the origin"
            );
        }
    }
}
