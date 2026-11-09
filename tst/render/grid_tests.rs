//! Unit and property tests for the [`crate::render::grid`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/render/grid.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! The geometry is a pure function, so most of this file needs no `App` at all. The two
//! tests that do stand one up check the thing a geometry test cannot: that the grid is
//! **render-only** — spawned as entities, absent from `Scene.props`, and not carrying
//! `MirroredBody`, so the Scene↔ECS mirror invariant still holds.
//!
//! Since the grid became **tiered** — axes and coarse lines over the whole floor, fine lines
//! over the core — the properties worth asserting are about how the tiers fit together: that
//! the plane is gridded to its edge, that no two lines land on the same offset, and that the
//! fine tier never overhangs the floor.

use super::*;
use crate::render::plugin::MirroredBody;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/render/proptest-regressions/grid_tests.txt";

/// The shipped layout, for the tests that care about what the app actually draws.
fn shipped_layout() -> GridLayout {
    GridLayout::for_floor(crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS, 0.0)
}

/// A layout with explicit numbers, for the tests that care about the geometry rather than the
/// shipped values.
fn layout(core: f32, fine: f32, full: f32, coarse: f32) -> GridLayout {
    GridLayout {
        core_extent_units: core,
        fine_spacing_units: fine,
        full_extent_units: full,
        coarse_spacing_units: coarse,
        surface_y: 0.0,
    }
}

/// The offset of a segment: its `z` if it runs along X, its `x` if along Z.
fn offset_of(segment: &GridSegment) -> f32 {
    if (segment.to.x - segment.from.x).abs() > 0.0 {
        segment.from.z
    } else {
        segment.from.x
    }
}

fn runs_along_x(segment: &GridSegment) -> bool {
    (segment.to.x - segment.from.x).abs() > 0.0
}

// -----------------------------------------------------------------------
// The tiers: what covers what.
// -----------------------------------------------------------------------

#[test]
fn the_axes_span_the_whole_floor_not_just_the_core() {
    // The origin has to be findable from the far end of an effectively unbounded plane, which
    // is the one thing a local fine grid cannot tell you.
    let layout = shipped_layout();
    let segments = floor_grid_segments(layout);

    let axes: Vec<&GridSegment> = segments
        .iter()
        .filter(|segment| matches!(segment.kind, GridLineKind::Axis { .. }))
        .collect();

    assert_eq!(axes.len(), 2, "one axis line per direction, no more");

    for axis in axes {
        assert!(
            (axis.length_units() - 2.0 * layout.full_extent_units).abs() < 1.0e-2,
            "an axis must span the floor: {} vs {}",
            axis.length_units(),
            2.0 * layout.full_extent_units
        );
        let midpoint = axis.midpoint();
        assert!(midpoint.x.abs() < 1.0e-6 && midpoint.z.abs() < 1.0e-6);
    }

    // One along X, one along Z, in their own colours.
    assert!(segments
        .iter()
        .any(|s| s.kind == GridLineKind::Axis { along_x: true }));
    assert!(segments
        .iter()
        .any(|s| s.kind == GridLineKind::Axis { along_x: false }));
    assert_ne!(
        GridLineKind::Axis { along_x: true }.color(),
        GridLineKind::Axis { along_x: false }.color()
    );
}

#[test]
fn coarse_lines_grid_the_plane_all_the_way_to_its_edge() {
    // The point of this change: the floor is 2000 units to the side and must not be bare.
    let layout = shipped_layout();
    let segments = floor_grid_segments(layout);

    let furthest = segments
        .iter()
        .map(|segment| offset_of(segment).abs())
        .fold(0.0_f32, f32::max);

    assert!(
        (furthest - layout.full_extent_units).abs() < 1.0e-2,
        "the outermost line is at {furthest} but the floor reaches {}",
        layout.full_extent_units
    );

    // And every coarse step in between is present, in both directions.
    let steps = (layout.full_extent_units / layout.coarse_spacing_units) as i32;
    for step in -steps..=steps {
        if step == 0 {
            continue;
        }
        let expected = step as f32 * layout.coarse_spacing_units;
        let found = segments
            .iter()
            .filter(|segment| (offset_of(segment) - expected).abs() < 1.0e-3)
            .count();
        assert_eq!(found, 2, "missing a coarse line pair at {expected}");
    }
}

#[test]
fn fine_lines_cover_the_core_and_stop_there() {
    let layout = shipped_layout();
    let segments = floor_grid_segments(layout);

    let furthest_fine = segments
        .iter()
        .filter(|segment| segment.kind == GridLineKind::Minor)
        .map(|segment| offset_of(segment).abs())
        .fold(0.0_f32, f32::max);

    assert!(
        furthest_fine <= layout.core_extent_units + 1.0e-3,
        "a fine line reached {furthest_fine}, past the core extent {}",
        layout.core_extent_units
    );
    assert!(
        furthest_fine > layout.core_extent_units - layout.fine_spacing_units - 1.0e-3,
        "fine lines stop well short of the core extent: {furthest_fine}"
    );

    // Fine lines span only the core, so they do not overhang into the coarse region.
    for segment in segments.iter().filter(|s| s.kind == GridLineKind::Minor) {
        assert!(
            (segment.length_units() - 2.0 * layout.core_extent_units).abs() < 1.0e-2,
            "a fine line spans {} but the core is {} wide",
            segment.length_units(),
            2.0 * layout.core_extent_units
        );
    }
}

#[test]
fn the_fine_grid_still_covers_the_region_bodies_move_through() {
    // The tier boundary must not fall inside the world: fine lines are what motion is measured
    // against, and the bounds are where motion happens.
    let bounds_extent = crate::core::constants::DEMO_BOUNDS_MAX_UNITS.x;
    assert!(
        GRID_CORE_EXTENT_UNITS >= bounds_extent,
        "fine lines stop at {GRID_CORE_EXTENT_UNITS} but the world reaches {bounds_extent}"
    );
}

#[test]
fn no_two_lines_land_on_the_same_offset() {
    // Two coincident lines read as one thicker line, which makes the spacing look irregular —
    // the exact artefact that appears if the fine tier forgets to skip coarse offsets.
    let segments = floor_grid_segments(shipped_layout());

    for direction in [true, false] {
        let mut offsets: Vec<f32> = segments
            .iter()
            .filter(|segment| runs_along_x(segment) == direction)
            .map(offset_of)
            .collect();
        offsets.sort_by(f32::total_cmp);

        for pair in offsets.windows(2) {
            assert!(
                (pair[1] - pair[0]).abs() > 1.0e-3,
                "two lines at the same offset {}",
                pair[0]
            );
        }
    }
}

#[test]
fn the_whole_grid_stays_a_manageable_number_of_entities() {
    // The tiering exists to make covering a 5000-unit floor affordable. A single 5-unit rule
    // over the same area would be four thousand lines of grey moiré.
    let tiered = floor_grid_segments(shipped_layout()).len();
    let uniform = floor_grid_segments(layout(
        crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS,
        GRID_SPACING_UNITS,
        crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS,
        GRID_COARSE_SPACING_UNITS,
    ))
    .len();

    // The floor grew to 5000 units for the solar system, so the coarse tier grew with it:
    // 101 lines per axis at 100-unit spacing, plus the fine core. Still trivial to draw —
    // the bound is about *readability*, and the cap is what stops a future floor from
    // quietly turning the grid into a grey sheet.
    assert!(tiered < 400, "the shipped grid is {tiered} lines");
    assert!(
        tiered * 5 < uniform,
        "tiering should cut the count several-fold: {tiered} vs {uniform}"
    );
}

// -----------------------------------------------------------------------
// Per-line geometry, unchanged by the tiering.
// -----------------------------------------------------------------------

#[test]
fn every_line_stays_inside_the_floor() {
    let layout = shipped_layout();
    for segment in floor_grid_segments(layout) {
        for point in [segment.from, segment.to] {
            assert!(
                point.x.abs() <= layout.full_extent_units + 1.0e-3,
                "{point:?}"
            );
            assert!(
                point.z.abs() <= layout.full_extent_units + 1.0e-3,
                "{point:?}"
            );
        }
    }
}

#[test]
fn every_line_is_axis_aligned() {
    // Both families run along an axis, which is what lets `spawn_floor_grid` scale a cuboid
    // instead of rotating one — a rotation would need a well-defined `looking_at`.
    for segment in floor_grid_segments(shipped_layout()) {
        let along = segment.to - segment.from;
        assert!(
            along.x.abs() < 1.0e-6 || along.z.abs() < 1.0e-6,
            "diagonal grid line: {along:?}"
        );
        assert!(
            along.y.abs() < 1.0e-6,
            "a grid line must stay in the floor's plane"
        );
    }
}

#[test]
fn the_grid_is_lifted_off_the_surface_to_avoid_z_fighting() {
    // Not cosmetic: coplanar surfaces flicker in the depth buffer as the camera moves.
    let surface_y = 3.0;
    let mut layout = shipped_layout();
    layout.surface_y = surface_y;

    for segment in floor_grid_segments(layout) {
        for point in [segment.from, segment.to] {
            assert!(
                (point.y - (surface_y + GRID_LIFT_UNITS)).abs() < 1.0e-6,
                "line at y={} but the surface is at {surface_y}",
                point.y
            );
        }
    }

    const {
        // A compile-time check: a zero lift reintroduces z-fighting, and no runtime is needed
        // to notice that.
        assert!(GRID_LIFT_UNITS > 0.0);
    }
}

#[test]
fn major_lines_are_the_coarse_reference_at_both_scales() {
    let layout = shipped_layout();
    let major_within_core = layout.fine_spacing_units * GRID_MAJOR_EVERY as f32;

    for segment in floor_grid_segments(layout) {
        if segment.kind != GridLineKind::Major {
            continue;
        }
        let offset = offset_of(&segment).abs();

        let on_core_major = (offset % major_within_core).abs() < 1.0e-3;
        let on_coarse = (offset % layout.coarse_spacing_units).abs() < 1.0e-3;

        assert!(
            on_core_major || on_coarse,
            "{offset} is neither a core major nor a coarse offset"
        );
    }
}

#[test]
fn the_three_kinds_are_visually_distinguishable() {
    assert!(
        GridLineKind::Minor.thickness_units() < GridLineKind::Major.thickness_units(),
        "a major line must be thicker than a minor one"
    );
    assert!(
        GridLineKind::Major.thickness_units()
            < GridLineKind::Axis { along_x: true }.thickness_units(),
        "an axis must be thicker than a major line"
    );
    assert_ne!(GridLineKind::Minor.color(), GridLineKind::Major.color());
}

// -----------------------------------------------------------------------
// The core-extent cap, and degenerate layouts.
// -----------------------------------------------------------------------

#[test]
fn a_small_floor_gets_fine_lines_that_match_it_exactly() {
    // Below the cap the fine tier still follows the floor, so the grid never overhangs the
    // surface it is painted on.
    assert_eq!(grid_core_extent_units(20.0), 20.0);
    assert_eq!(
        grid_core_extent_units(GRID_CORE_EXTENT_UNITS),
        GRID_CORE_EXTENT_UNITS
    );

    let small = GridLayout::for_floor(20.0, 0.0);
    assert_eq!(small.core_extent_units, 20.0);
    assert_eq!(small.full_extent_units, 20.0);
}

#[test]
fn an_effectively_unbounded_floor_gets_a_capped_fine_tier() {
    let floor = crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS;
    assert!(
        floor > GRID_CORE_EXTENT_UNITS,
        "the shipped floor should exceed the cap"
    );

    let layout = GridLayout::for_floor(floor, 0.0);
    assert_eq!(layout.core_extent_units, GRID_CORE_EXTENT_UNITS);
    assert_eq!(
        layout.full_extent_units, floor,
        "but coarse lines reach the edge"
    );
    assert!(layout.is_valid());
}

#[test]
fn the_coarse_spacing_is_a_whole_multiple_of_the_fine_one() {
    // Otherwise the tiers interleave into a beat pattern instead of lining up, and the
    // same-offset skip in `floor_grid_segments` stops landing exactly.
    let ratio = GRID_COARSE_SPACING_UNITS / GRID_SPACING_UNITS;
    assert!((ratio - ratio.round()).abs() < 1.0e-6, "ratio is {ratio}");
}

#[test]
fn a_floor_smaller_than_its_spacings_gets_no_grid() {
    // A single line through the middle of a tiny floor looks like a bug, so the answer is no
    // grid — but the axes still mark the origin.
    let tiny = floor_grid_segments(layout(2.0, 5.0, 2.0, 100.0));
    assert_eq!(tiny.len(), 2, "only the two axes");
    assert!(tiny
        .iter()
        .all(|s| matches!(s.kind, GridLineKind::Axis { .. })));
}

#[test]
fn an_invalid_layout_is_rejected() {
    let mut bad = shipped_layout();
    bad.core_extent_units = bad.full_extent_units * 2.0;
    assert!(
        !bad.is_valid(),
        "fine lines outside the floor is not a layout"
    );

    #[cfg(not(debug_assertions))]
    {
        assert!(floor_grid_segments(bad).is_empty());

        for broken in [f32::NAN, 0.0, -10.0] {
            let mut bad = shipped_layout();
            bad.full_extent_units = broken;
            assert!(!bad.is_valid());
            assert!(floor_grid_segments(bad).is_empty());
        }
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "layout fails Validation_Rules")]
fn a_nonsense_layout_panics_in_debug() {
    let mut bad = shipped_layout();
    bad.full_extent_units = 0.0;
    let _ = floor_grid_segments(bad);
}

#[test]
fn the_floor_is_found_by_measuring_the_scene_not_by_assuming() {
    // This is what keeps the grid and the floor in agreement when the floor changes size.
    let scene = Scene::demo();
    let (half_size, surface_y) =
        floor_half_size(&scene).expect("the starter scene has a ground plane");

    assert_eq!(
        half_size,
        crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS
    );
    assert_eq!(surface_y, 0.0);

    // And a scene with no plane has no grid to draw.
    let mut planeless = Scene::demo();
    planeless.props.clear();
    assert_eq!(floor_half_size(&planeless), None);
}

// -----------------------------------------------------------------------
// The grid is render-only. These are the tests a geometry check cannot make.
// -----------------------------------------------------------------------

#[test]
fn spawning_the_grid_adds_entities_without_touching_the_scene() {
    use bevy::app::App;
    use bevy::MinimalPlugins;

    // `MinimalPlugins` omits `AssetPlugin`, so the two asset collections `spawn_floor_grid`
    // writes into are inserted by hand — the same approach `plugin_tests.rs` takes for
    // `spawn_scene_entities`.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.init_resource::<Assets<crate::render::eclipse::BodyMaterial>>();
    app.insert_resource(Scene::demo());
    app.add_systems(Startup, spawn_floor_grid);

    let props_before = app.world().resource::<Scene>().props.len();
    app.update();

    let grid_lines = app
        .world_mut()
        .query::<&FloorGridLine>()
        .iter(app.world())
        .count();

    // Through `GridLayout::for_floor`, not a hand-computed count: a test that recomputed the
    // layout itself could pass while disagreeing with the system.
    assert_eq!(
        grid_lines,
        floor_grid_segments(shipped_layout()).len(),
        "one entity per segment"
    );
    assert!(
        grid_lines < 400,
        "the grid must stay a readable size, got {grid_lines} lines"
    );

    assert_eq!(
        app.world().resource::<Scene>().props.len(),
        props_before,
        "the grid must not add props to the simulated scene"
    );
}

#[test]
fn grid_lines_are_not_mirrored_bodies() {
    use bevy::app::App;
    use bevy::MinimalPlugins;

    // The mirror invariant is `count_entities_with::<MirroredBody>() == scene.props.len()`.
    // A grid line carrying that marker would break the headless integration test and, more
    // importantly, would make the grid look like part of the model.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.init_resource::<Assets<crate::render::eclipse::BodyMaterial>>();
    app.insert_resource(Scene::demo());
    app.add_systems(Startup, spawn_floor_grid);
    app.update();

    let mirrored = app
        .world_mut()
        .query::<&MirroredBody>()
        .iter(app.world())
        .count();
    assert_eq!(mirrored, 0, "the grid must not mirror anything");
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Whatever the floor, the grid reaches its edge and never past it — the property the
    /// whole tiering exists to deliver.
    #[test]
    fn any_floor_is_gridded_to_its_edge(half in 5.0_f32..5000.0, surface_y in -10.0_f32..10.0) {
        let layout = GridLayout::for_floor(half, surface_y);
        let segments = floor_grid_segments(layout);

        prop_assert!(!segments.is_empty());

        let furthest = segments
            .iter()
            .map(|segment| offset_of(segment).abs())
            .fold(0.0_f32, f32::max);

        // The outermost line is within one coarse spacing of the edge, and never beyond it.
        prop_assert!(furthest <= half + 1.0e-2);
        prop_assert!(furthest >= half - layout.coarse_spacing_units - 1.0e-2);

        for segment in segments {
            prop_assert!(segment.from.x.abs() <= half + 1.0e-2);
            prop_assert!(segment.from.z.abs() <= half + 1.0e-2);
            prop_assert!((segment.midpoint().y - (surface_y + GRID_LIFT_UNITS)).abs() < 1.0e-3);
        }
    }

    /// No two lines share an offset in the same direction, at any floor size — the artefact
    /// that appears if the fine tier stops skipping coarse offsets.
    #[test]
    fn no_offset_is_ever_drawn_twice(half in 5.0_f32..3000.0) {
        let segments = floor_grid_segments(GridLayout::for_floor(half, 0.0));

        for direction in [true, false] {
            let mut offsets: Vec<f32> = segments
                .iter()
                .filter(|segment| runs_along_x(segment) == direction)
                .map(offset_of)
                .collect();
            offsets.sort_by(f32::total_cmp);

            for pair in offsets.windows(2) {
                prop_assert!((pair[1] - pair[0]).abs() > 1.0e-3);
            }
        }
    }

    /// A grid is always symmetric about the origin: for every line at `+offset` there is one
    /// at `−offset`. That is what makes the axes read as the centre.
    #[test]
    fn a_grid_is_symmetric_about_the_origin(half in 10.0_f32..1000.0) {
        let segments = floor_grid_segments(GridLayout::for_floor(half, 0.0));

        let offsets: Vec<f32> = segments
            .iter()
            .filter(|segment| runs_along_x(segment))
            .map(offset_of)
            .collect();

        for offset in &offsets {
            prop_assert!(
                offsets.iter().any(|other| (other + offset).abs() < 1.0e-3),
                "no mirror for offset {}",
                offset
            );
        }
    }

    /// Every line is one of the three kinds, and a fine line never reaches past the core.
    #[test]
    fn the_tiers_keep_to_their_extents(half in 5.0_f32..2000.0) {
        let layout = GridLayout::for_floor(half, 0.0);

        for segment in floor_grid_segments(layout) {
            match segment.kind {
                GridLineKind::Minor => {
                    prop_assert!(offset_of(&segment).abs() <= layout.core_extent_units + 1.0e-3);
                }
                GridLineKind::Major | GridLineKind::Axis { .. } => {
                    prop_assert!(offset_of(&segment).abs() <= layout.full_extent_units + 1.0e-3);
                }
            }
        }
    }
}
