//! Unit and property tests for the [`crate::ui::compass`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/ui/compass.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! The whole subject is one projection, so almost everything here tests
//! [`compass_axes`] — a pure function of a rotation and a radius. That is deliberate:
//! `compass_system` cannot be tested headlessly (egui needs a real windowed app, see
//! `tst/ui/README.md`), so the geometry is kept out of it and tested exhaustively instead.
//!
//! The properties worth stating plainly, because they are what "acts as a compass" means:
//!
//! - the three arms are the world axes seen from the camera, so they stay **orthonormal**
//!   under any rotation;
//! - screen `y` runs **downward**, so view-space up must come out negative;
//! - an axis pointing at the viewer and one pointing away project to the *same* arm, and
//!   are distinguished only by `toward_viewer` — which is why that field exists.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use std::f32::consts::FRAC_PI_2;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/ui/proptest-regressions/compass_tests.txt";

/// The shipped radius, so the assertions read in the units the app uses.
const RADIUS: f32 = 30.0;

#[test]
fn an_unrotated_camera_sees_x_right_y_up_and_z_toward_it() {
    // Bevy's default camera looks down world −Z, so: X to the right, Y up the screen, and
    // Z straight out of the screen at the viewer.
    let [x, y, z] = compass_axes(Quat::IDENTITY, RADIUS);

    assert_eq!([x.label, y.label, z.label], ["X", "Y", "Z"]);

    assert!(
        (x.tip_px - Vec2::new(RADIUS, 0.0)).length() < 1.0e-3,
        "{:?}",
        x.tip_px
    );
    assert!(
        (y.tip_px - Vec2::new(0.0, -RADIUS)).length() < 1.0e-3,
        "screen y grows downward, so world up must be negative: {:?}",
        y.tip_px
    );

    // Z projects to nothing on screen — it is pointing at you — and says so.
    assert!(z.tip_px.length() < 1.0e-3);
    assert!(z.toward_viewer > 0.99);
    assert!(!z.points_away());
    assert!(
        !x.points_away() && !y.points_away(),
        "flat axes are not 'away'"
    );
}

#[test]
fn turning_the_camera_turns_the_compass() {
    // Yaw the camera +90° about world Y. Working it through, because the signs are the easy
    // thing to get backwards: the camera's forward is its local −Z, and rotating `(0,0,−1)`
    // by +90° about Y gives `(−1,0,0)` — so the camera now looks down world **−X**.
    //
    // Therefore world +X is *behind* the camera, pointing back at the viewer; and the
    // camera's right (local +X, rotated) is world −Z, which puts world +Z on the **left**
    // of the screen.
    let [x, y, z] = compass_axes(Quat::from_rotation_y(FRAC_PI_2), RADIUS);

    assert!(
        x.toward_viewer > 0.99,
        "looking down −X, world +X points back at the viewer: {}",
        x.toward_viewer
    );
    assert!(!x.points_away());
    assert!(
        (z.tip_px - Vec2::new(-RADIUS, 0.0)).length() < 1.0e-3,
        "world +Z should be to the left: {:?}",
        z.tip_px
    );

    // Yaw does not touch the vertical axis.
    assert!((y.tip_px - Vec2::new(0.0, -RADIUS)).length() < 1.0e-3);
}

#[test]
fn pitching_the_camera_tilts_the_vertical_axis() {
    // Pitch −90° about X: forward `(0,0,−1)` becomes `(0,−1,0)`, so the camera looks
    // straight **down** at the floor. World +Y is then behind it — pointing back at the
    // viewer — and the camera's up (local +Y, rotated) is world −Z, which puts world +Z
    // *down* the screen.
    let [_, y, z] = compass_axes(Quat::from_rotation_x(-FRAC_PI_2), RADIUS);

    assert!(
        y.toward_viewer > 0.99,
        "looking down, world +Y points back at the viewer: {}",
        y.toward_viewer
    );
    assert!(!y.points_away());
    assert!(
        (z.tip_px - Vec2::new(0.0, RADIUS)).length() < 1.0e-3,
        "world +Z should run down the screen: {:?}",
        z.tip_px
    );
}

#[test]
fn an_axis_coming_toward_you_and_one_going_away_differ_only_in_depth() {
    // The reason `toward_viewer` exists. Both project to a zero-length arm, so a compass
    // without the depth cue would be ambiguous exactly when looking along an axis.
    let toward = compass_axes(Quat::IDENTITY, RADIUS)[2];
    let away = compass_axes(Quat::from_rotation_y(std::f32::consts::PI), RADIUS)[2];

    assert!(
        (toward.tip_px - away.tip_px).length() < 1.0e-3,
        "identical on screen"
    );
    assert!(toward.toward_viewer > 0.99 && away.toward_viewer < -0.99);
    assert!(!toward.points_away() && away.points_away());
}

#[test]
fn translation_is_ignored() {
    // A compass shows heading, not position: it must read the same from anywhere in the
    // scene. `compass_axes` takes only a rotation, which is how that is guaranteed — this
    // test pins the *signature* decision as much as the behaviour.
    let rotation = Quat::from_euler(bevy::math::EulerRot::YXZ, 0.7, -0.3, 0.0);
    assert_eq!(
        compass_axes(rotation, RADIUS),
        compass_axes(rotation, RADIUS)
    );
}

#[test]
fn every_axis_has_its_own_colour_and_they_match_the_grid_convention() {
    let [x, y, z] = compass_axes(Quat::IDENTITY, RADIUS);

    assert_eq!(x.color_srgba, COMPASS_X_COLOR_SRGBA);
    assert_eq!(y.color_srgba, COMPASS_Y_COLOR_SRGBA);
    assert_eq!(z.color_srgba, COMPASS_Z_COLOR_SRGBA);
    assert_ne!(x.color_srgba, y.color_srgba);
    assert_ne!(y.color_srgba, z.color_srgba);
    assert_ne!(x.color_srgba, z.color_srgba);

    // Red for X, blue for Z, matching the floor grid's axis lines: the compass and the
    // lines on the floor must agree about which axis is which.
    assert!(x.color_srgba[0] > x.color_srgba[2], "X should read as red");
    assert!(z.color_srgba[2] > z.color_srgba[0], "Z should read as blue");
    assert!(
        y.color_srgba[1] > y.color_srgba[0],
        "Y should read as green"
    );
}

#[test]
fn a_degenerate_radius_collapses_the_compass_instead_of_producing_nans() {
    #[cfg(not(debug_assertions))]
    for radius in [0.0_f32, -10.0, f32::NAN, f32::INFINITY] {
        for axis in compass_axes(Quat::IDENTITY, radius) {
            assert!(
                axis.tip_px.is_finite(),
                "radius {radius} produced {:?}",
                axis.tip_px
            );
            assert_eq!(axis.tip_px, Vec2::ZERO);
        }
    }

    // The depth cue survives a collapsed radius, since it does not scale with it.
    #[cfg(debug_assertions)]
    assert!(compass_axes(Quat::IDENTITY, RADIUS)[2].toward_viewer > 0.99);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "radius_px must be finite and positive")]
fn a_zero_radius_panics_in_debug() {
    let _ = compass_axes(Quat::IDENTITY, 0.0);
}

#[test]
fn a_non_finite_rotation_falls_back_to_identity_rather_than_nan() {
    // A `NaN` quaternion cannot be inverted usefully. Drawing a wrong-but-finite compass
    // beats filling the painter with `NaN` positions, which egui would not enjoy.
    let broken = Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0);

    for axis in compass_axes(broken, RADIUS) {
        assert!(axis.tip_px.is_finite());
        assert!(axis.toward_viewer.is_finite());
    }
    assert_eq!(
        compass_axes(broken, RADIUS),
        compass_axes(Quat::IDENTITY, RADIUS)
    );
}

#[test]
fn the_config_default_is_the_layers_constants() {
    let config = CompassConfig::default();

    assert_eq!(config.radius_px, COMPASS_RADIUS_PX);
    assert_eq!(config.margin_px, COMPASS_MARGIN_PX);
    assert_eq!(config.thickness_px, COMPASS_THICKNESS_PX);
    assert_eq!(config.label_size_px, COMPASS_LABEL_SIZE_PX);
}

#[test]
fn the_compass_plugin_is_constructible() {
    // `compass_system` needs a real egui context, so the plugin cannot be ticked headlessly
    // — see the module docs. This pins the type surface `main.rs` depends on.
    fn takes_plugin<P: bevy::app::Plugin>(_p: P) {}
    takes_plugin(CompassPlugin);
}

/// Arbitrary rotations, built from Euler angles so every strategy value is a valid unit
/// quaternion rather than a random four-tuple that would need normalising.
fn rotation() -> impl Strategy<Value = Quat> {
    use std::f32::consts::PI;

    // Full yaw and roll, and pitch over its usable range — the same interval
    // `clamp_pitch` enforces on the real camera, so the strategy explores orientations the
    // app can actually be in.
    (-PI..PI, -FRAC_PI_2..FRAC_PI_2, -PI..PI).prop_map(|(yaw, pitch, roll)| {
        Quat::from_euler(bevy::math::EulerRot::YXZ, yaw, pitch, roll)
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// However the camera is oriented, each arm is a projected **unit** vector: the screen
    /// length and the depth together make up the radius. This is the invariant that makes
    /// the compass a rotation of a fixed shape rather than three independent lines.
    #[test]
    fn each_arm_is_a_projected_unit_vector(rotation in rotation(), radius in 1.0_f32..200.0) {
        for axis in compass_axes(rotation, radius) {
            let on_screen = axis.tip_px.length() / radius;
            let magnitude = (on_screen * on_screen + axis.toward_viewer * axis.toward_viewer).sqrt();

            prop_assert!(
                (magnitude - 1.0).abs() < 1.0e-3,
                "{} has magnitude {magnitude}",
                axis.label
            );
            prop_assert!(axis.tip_px.length() <= radius + 1.0e-3);
            // Tolerated rather than exact: `q⁻¹ · d` for a normalised `f32` quaternion can
            // land a component a few ULPs outside ±1, and proptest eventually finds one. The
            // depth cue only reads the *sign*, so a magnitude of `1.0000001` costs nothing —
            // the assertion is here to catch a component that has stopped being a direction
            // cosine at all.
            prop_assert!(
                (-1.0 - 1.0e-3..=1.0 + 1.0e-3).contains(&axis.toward_viewer),
                "{} has toward_viewer {}",
                axis.label,
                axis.toward_viewer
            );
        }
    }

    /// The three arms stay mutually perpendicular in 3D under any rotation — a rotation
    /// cannot shear an orthonormal basis, and if this ever fails the projection has stopped
    /// being a rotation.
    #[test]
    fn the_three_axes_stay_orthogonal(rotation in rotation()) {
        let axes = compass_axes(rotation, 1.0);

        // Reassemble each arm's full 3D view-space vector: screen x, negated screen y, depth.
        let view = |axis: &CompassAxis| {
            bevy::math::Vec3::new(axis.tip_px.x, -axis.tip_px.y, axis.toward_viewer)
        };

        for (left, right) in [(0, 1), (1, 2), (0, 2)] {
            let dot = view(&axes[left]).dot(view(&axes[right]));
            prop_assert!(dot.abs() < 1.0e-3, "axes {left} and {right} are not perpendicular: {dot}");
        }
    }

    /// Arms scale linearly with the radius: doubling the compass doubles every arm and
    /// leaves every depth alone. That is what lets `CompassConfig::radius_px` be a size
    /// knob rather than a shape one.
    #[test]
    fn the_radius_scales_the_arms_and_not_the_depths(rotation in rotation(), radius in 1.0_f32..90.0) {
        let small = compass_axes(rotation, radius);
        let large = compass_axes(rotation, radius * 2.0);

        for (small, large) in small.iter().zip(large.iter()) {
            prop_assert!((large.tip_px - small.tip_px * 2.0).length() < 1.0e-3);
            prop_assert!((large.toward_viewer - small.toward_viewer).abs() < 1.0e-6);
        }
    }

    /// Labels and colours are fixed per axis, whatever the view — the compass may spin, but
    /// X is always X.
    #[test]
    fn labels_and_colours_never_change(rotation in rotation()) {
        let axes = compass_axes(rotation, RADIUS);

        prop_assert_eq!([axes[0].label, axes[1].label, axes[2].label], ["X", "Y", "Z"]);
        prop_assert_eq!(axes[0].color_srgba, COMPASS_X_COLOR_SRGBA);
        prop_assert_eq!(axes[1].color_srgba, COMPASS_Y_COLOR_SRGBA);
        prop_assert_eq!(axes[2].color_srgba, COMPASS_Z_COLOR_SRGBA);
    }
}
