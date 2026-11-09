//! Unit and property tests for the [`crate::render::bridge`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/render/bridge.rs`
//! via `#[path]`, not as a standalone integration test, so these tests keep
//! access to the module's private items and to `super::*`.

//! Tests for `transform_from`.
//!
//! Strategies are defined locally (rather than re-used from the `scene`
//! module's private `tests` module) to keep coupling low and the
//! strategies visible alongside the properties that consume them.

use super::*;
use crate::core::scene::{Body, Color, Pose};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

// -----------------------------------------------------------------------
// Local proptest strategies for valid `Body` fields.
//
// Numeric ranges are bounded well below `f32::MAX` so downstream
// arithmetic in `transform_from` (e.g. `2.0 * half_extents`,
// `(to - from).length()`) stays finite, while still being wide enough
// to exercise realistic scene coordinates.
// -----------------------------------------------------------------------

/// A finite `f32` in a safe closed range.
fn finite_f32() -> impl Strategy<Value = f32> {
    -1.0e6_f32..=1.0e6_f32
}

/// A strictly positive, finite `f32`.
///
/// Lower-bounded away from zero to avoid subnormals; upper-bounded to
/// keep `2.0 * x` finite.
fn positive_finite_f32() -> impl Strategy<Value = f32> {
    1.0e-3_f32..1.0e6_f32
}

/// A `Vec3` with finite components.
fn finite_vec3() -> impl Strategy<Value = Vec3> {
    (finite_f32(), finite_f32(), finite_f32()).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

/// A `Vec3` whose components are all strictly positive and finite,
/// suitable for `Body::Cuboid.half_extents`.
fn positive_vec3() -> impl Strategy<Value = Vec3> {
    (
        positive_finite_f32(),
        positive_finite_f32(),
        positive_finite_f32(),
    )
        .prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

/// A unit quaternion (within `1e-4` of length `1.0`).
///
/// Generates raw xyzw components in `[-1, 1]`, filters out vectors too
/// close to zero (which would NaN under `normalize`), then normalizes.
fn unit_quat() -> impl Strategy<Value = Quat> {
    (
        -1.0_f32..=1.0,
        -1.0_f32..=1.0,
        -1.0_f32..=1.0,
        -1.0_f32..=1.0,
    )
        .prop_filter(
            "quaternion must be non-degenerate before normalization",
            |(x, y, z, w)| x * x + y * y + z * z + w * w > 1.0e-6,
        )
        .prop_map(|(x, y, z, w)| Quat::from_xyzw(x, y, z, w).normalize())
}

/// A `Pose` satisfying `Pose::is_valid`.
fn valid_pose() -> impl Strategy<Value = Pose> {
    (finite_vec3(), unit_quat()).prop_map(|(position, rotation)| Pose { position, rotation })
}

/// A `Color` satisfying `Color::is_valid`.
fn valid_color() -> impl Strategy<Value = Color> {
    (0.0_f32..=1.0, 0.0_f32..=1.0, 0.0_f32..=1.0, 0.0_f32..=1.0).prop_map(|(r, g, b, a)| Color {
        r,
        g,
        b,
        a,
    })
}

/// A `Body::Sphere` with valid fields.
fn valid_sphere() -> impl Strategy<Value = Body> {
    (valid_pose(), positive_finite_f32(), valid_color()).prop_map(|(pose, radius, color)| {
        Body::Sphere {
            pose,
            radius,
            color,
        }
    })
}

/// A `Body::Cuboid` with valid fields.
fn valid_cuboid() -> impl Strategy<Value = Body> {
    (valid_pose(), positive_vec3(), valid_color()).prop_map(|(pose, half_extents, color)| {
        Body::Cuboid {
            pose,
            half_extents,
            color,
        }
    })
}

/// A `Body::Line` with valid fields.
///
/// `width` generated as non-negative finite so `Body::is_valid` holds.
fn valid_line() -> impl Strategy<Value = Body> {
    (
        finite_vec3(),
        finite_vec3(),
        0.0_f32..1.0e6_f32,
        valid_color(),
    )
        .prop_map(|(from, to, width, color)| Body::Line {
            from,
            to,
            width,
            color,
        })
}

/// A `Body::Plane` with valid fields.
fn valid_plane() -> impl Strategy<Value = Body> {
    (valid_pose(), positive_finite_f32(), valid_color()).prop_map(|(pose, half_size, color)| {
        Body::Plane {
            pose,
            half_size,
            color,
        }
    })
}

/// Any `Body` variant with fields satisfying the per-variant
/// Validation_Rules.
fn valid_body() -> impl Strategy<Value = Body> {
    prop_oneof![valid_sphere(), valid_cuboid(), valid_line(), valid_plane()]
}

/// An opaque black `Color`, matching the `BLACK` placeholder used by
/// the Line-midpoint property described in the spec.
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
///
/// The path is relative to the package root, which is Cargo's working directory
/// for tests. Commit the seeds it produces: they are the counterexamples this
/// suite has already paid for.
const REGRESSION_SEEDS: &str = "tst/render/proptest-regressions/bridge_tests.txt";

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 6: `transform_from` is finite and valid.
    ///
    /// **Validates: Requirements 8.1, 8.2**
    ///
    /// For every `Body` whose fields satisfy the per-variant
    /// Validation_Rules, `transform_from(&body)` must produce:
    ///
    /// - `translation` with every component finite (Requirement 8.1),
    /// - `scale` with every component finite (Requirement 8.1), and
    /// - `rotation` with magnitude within `1e-4` of `1.0` (Requirement 8.2).
    #[test]
    fn transform_from_is_finite_and_unit_rotation(body in valid_body()) {
        let t = transform_from(&body);

        prop_assert!(
            t.translation.is_finite(),
            "translation has a non-finite component: {:?} for body {:?}",
            t.translation,
            body,
        );
        prop_assert!(
            t.scale.is_finite(),
            "scale has a non-finite component: {:?} for body {:?}",
            t.scale,
            body,
        );
        let rot_len = t.rotation.length();
        prop_assert!(
            (rot_len - 1.0).abs() <= 1.0e-4,
            "rotation magnitude {} is not within 1e-4 of 1.0 for body {:?}",
            rot_len,
            body,
        );
    }

    /// Property 7 (Sphere case): `transform_from` respects pose on spheres.
    ///
    /// **Validates: Requirements 8.3**
    ///
    /// For a valid `Body::Sphere { pose, radius, .. }`,
    /// `transform_from` must return a `Transform` whose `translation`
    /// equals `pose.position`, whose `rotation` equals `pose.rotation`,
    /// and whose `scale` equals `Vec3::splat(radius)` within `1e-5`
    /// absolute tolerance. The pose is passed through untouched in the
    /// implementation, so strict `==` is expected for `translation`
    /// and `rotation`.
    #[test]
    fn transform_from_respects_pose_on_sphere(body in valid_sphere()) {
        let t = transform_from(&body);
        let (pose, radius) = match &body {
            Body::Sphere { pose, radius, .. } => (*pose, *radius),
            _ => unreachable!("valid_sphere must produce Body::Sphere"),
        };

        // Pose is passed through bit-exact by `transform_from`.
        prop_assert_eq!(t.translation, pose.position);
        prop_assert_eq!(t.rotation, pose.rotation);
        // Sphere scale formula (Requirement 8.3).
        prop_assert!(
            t.scale.abs_diff_eq(Vec3::splat(radius), 1.0e-5),
            "scale {:?} deviates from Vec3::splat(radius = {}) beyond 1e-5",
            t.scale,
            radius,
        );
    }

    /// Property 7 (Cuboid case): `transform_from` respects pose on cuboids.
    ///
    /// **Validates: Requirements 8.4**
    ///
    /// For a valid `Body::Cuboid { pose, half_extents, .. }`,
    /// `transform_from` must return a `Transform` whose `translation`
    /// equals `pose.position`, whose `rotation` equals `pose.rotation`,
    /// and whose `scale` equals `2.0 * half_extents` within `1e-5`
    /// absolute tolerance.
    #[test]
    fn transform_from_respects_pose_on_cuboid(body in valid_cuboid()) {
        let t = transform_from(&body);
        let (pose, half_extents) = match &body {
            Body::Cuboid {
                pose, half_extents, ..
            } => (*pose, *half_extents),
            _ => unreachable!("valid_cuboid must produce Body::Cuboid"),
        };

        prop_assert_eq!(t.translation, pose.position);
        prop_assert_eq!(t.rotation, pose.rotation);
        // Cuboid scale formula (Requirement 8.4).
        prop_assert!(
            t.scale.abs_diff_eq(2.0 * half_extents, 1.0e-5),
            "scale {:?} deviates from 2.0 * half_extents {:?} beyond 1e-5",
            t.scale,
            half_extents,
        );
    }

    /// Property 7 (Plane case): `transform_from` respects pose on planes.
    ///
    /// **Validates: Requirements 8.5**
    ///
    /// For a valid `Body::Plane { pose, half_size, .. }`,
    /// `transform_from` must return a `Transform` whose `translation`
    /// equals `pose.position`, whose `rotation` equals `pose.rotation`,
    /// and whose `scale` equals
    /// `Vec3::new(2.0 * half_size, 1.0, 2.0 * half_size)` within `1e-5`
    /// absolute tolerance.
    #[test]
    fn transform_from_respects_pose_on_plane(body in valid_plane()) {
        let t = transform_from(&body);
        let (pose, half_size) = match &body {
            Body::Plane {
                pose, half_size, ..
            } => (*pose, *half_size),
            _ => unreachable!("valid_plane must produce Body::Plane"),
        };

        prop_assert_eq!(t.translation, pose.position);
        prop_assert_eq!(t.rotation, pose.rotation);
        // Plane scale formula (Requirement 8.5).
        let expected = Vec3::new(2.0 * half_size, 1.0, 2.0 * half_size);
        prop_assert!(
            t.scale.abs_diff_eq(expected, 1.0e-5),
            "scale {:?} deviates from {:?} beyond 1e-5",
            t.scale,
            expected,
        );
    }

    /// Property 8: `transform_from` midpoints lines.
    ///
    /// **Validates: Requirements 8.6**
    ///
    /// For any finite `from` and `to`, `transform_from` on a
    /// `Body::Line` must produce a `Transform` whose `translation`
    /// equals `0.5 * (from + to)` within `1e-5` absolute tolerance and
    /// whose `scale.x` equals `(to - from).length()` within `1e-5`.
    #[test]
    fn transform_from_line_midpoint_and_length(
        from in finite_vec3(),
        to in finite_vec3(),
    ) {
        let body = Body::Line {
            from,
            to,
            width: 0.0,
            color: BLACK,
        };
        let t = transform_from(&body);

        let expected_midpoint = 0.5 * (from + to);
        prop_assert!(
            t.translation.abs_diff_eq(expected_midpoint, 1.0e-5),
            "line translation {:?} deviates from midpoint {:?} beyond 1e-5 \
             (from = {:?}, to = {:?})",
            t.translation,
            expected_midpoint,
            from,
            to,
        );

        let expected_length = (to - from).length();
        prop_assert!(
            (t.scale.x - expected_length).abs() <= 1.0e-5,
            "line scale.x {} deviates from (to - from).length() {} beyond 1e-5 \
             (from = {:?}, to = {:?})",
            t.scale.x,
            expected_length,
            from,
            to,
        );
    }
}

// -----------------------------------------------------------------------
// Unit test: degenerate `Line` (from == to).
//
// Requirement 14.2: `transform_from` must be total on a `Body::Line`
// whose endpoints coincide. `scale.x` collapses to `0.0`, `translation`
// stays finite, and `rotation` is still a unit quaternion via
// `normalize_or_zero` + `Quat::from_rotation_arc`'s identity fallback.
// -----------------------------------------------------------------------

/// A degenerate `Line` whose endpoints coincide must produce a total
/// `Transform`: zero `scale.x`, finite `translation`, and a unit-length
/// `rotation`. Covers Requirement 14.2.
#[test]
fn transform_from_degenerate_line_is_total() {
    let p = Vec3::new(1.0, 2.0, -3.0);
    let body = Body::Line {
        from: p,
        to: p,
        width: 0.0,
        color: BLACK,
    };

    let t = transform_from(&body);

    // `(to - from).length() == 0.0` → `scale.x == 0.0`.
    assert_eq!(
        t.scale.x, 0.0,
        "degenerate line must collapse scale.x to exactly 0.0, got {}",
        t.scale.x,
    );
    // Translation is `0.5 * (p + p) == p`, which is finite.
    assert!(
        t.translation.is_finite(),
        "degenerate line translation must be finite, got {:?}",
        t.translation,
    );
    // `normalize_or_zero` + `from_rotation_arc(X, ZERO)` yields a unit
    // quaternion (Quat::IDENTITY in practice) — magnitude within 1e-4.
    let rot_len = t.rotation.length();
    assert!(
        (rot_len - 1.0).abs() <= 1.0e-4,
        "degenerate line rotation magnitude must be within 1e-4 of 1.0, got {}",
        rot_len,
    );
}

// -----------------------------------------------------------------------
// Unit tests for `sync_scene_to_ecs_system` (task 7.5).
//
// The per-frame render-bridge system is exercised headlessly by
// building a `bevy::App` with `MinimalPlugins`, spawning a `Scene`
// resource and a handful of entities tagged with `MirroredBody`,
// running a single `Update` tick, and asserting:
//
//   - entities whose `MirroredBody(id)` resolves via `scene.get(id)`
//     have had their `Transform` overwritten with `transform_from`
//     (Requirements 9.1, 9.4);
//   - entities whose `MirroredBody(id)` does not resolve are left
//     alone (Requirement 9.2);
//   - the `Scene` resource is not mutated (Requirement 9.3 — also
//     enforced at compile time by `Res<Scene>`).
//
// We deliberately do *not* go through `ScenePlugin` here (that
// wiring is task 7.7); the point is to test the system in isolation.
// -----------------------------------------------------------------------

use crate::core::scene::{BodyId, Prop, Scene};
use crate::render::plugin::MirroredBody;
use bevy::app::{App, Update};
use bevy::ecs::system::Query;
use bevy::MinimalPlugins;

/// Build a scene and add a freshly-constructed `Body::Sphere` at a
/// known pose, returning the scene, the id assigned by `Scene::add`,
/// and a clone of the body itself so tests can compute the expected
/// `Transform` without re-deriving it from `Scene::get`.
///
/// `Scene`'s private fields prevent constructing an empty `Scene`
/// from outside the module, so we start from `Scene::demo()` and
/// simply append our test body; `Scene::get` on the returned id
/// still resolves to the body we added.
fn demo_scene_with_sphere(position: Vec3, radius: f32) -> (Scene, BodyId, Body) {
    let body = Body::Sphere {
        pose: Pose {
            position,
            rotation: Quat::IDENTITY,
        },
        radius,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    };
    let mut scene = Scene::demo();
    // A movable 1 kg prop at rest: the sync system reads only `prop.body`, so the
    // dynamics are irrelevant here beyond having to be valid.
    let id = scene.add(Prop::at_rest(body.clone(), 1.0));
    (scene, id, body)
}

/// `sync_scene_to_ecs_system` must overwrite `Transform` with
/// `transform_from(body)` for every entity whose `MirroredBody(id)`
/// resolves in `Scene`. Covers Requirements 9.1 and 9.4.
#[test]
fn sync_overwrites_transform_for_resolvable_ids() {
    // Build a Scene::demo() with an extra Sphere body at (3, 4, 5).
    let expected_translation = Vec3::new(3.0, 4.0, 5.0);
    let (scene, id, body) = demo_scene_with_sphere(expected_translation, 2.0);
    let expected_transform = transform_from(&body);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(scene);
    app.add_systems(Update, sync_scene_to_ecs_system);

    // Spawn one ECS entity tagged with MirroredBody(id), starting from
    // a deliberately-wrong Transform so we can see the overwrite.
    let entity = app
        .world_mut()
        .spawn((MirroredBody(id), Transform::from_xyz(-99.0, -99.0, -99.0)))
        .id();

    app.update();

    let actual = *app
        .world()
        .entity(entity)
        .get::<Transform>()
        .expect("entity must still carry a Transform");
    assert!(
        actual
            .translation
            .abs_diff_eq(expected_transform.translation, 1.0e-5),
        "translation {:?} != {:?}",
        actual.translation,
        expected_transform.translation,
    );
    assert!(
        actual.scale.abs_diff_eq(expected_transform.scale, 1.0e-5),
        "scale {:?} != {:?}",
        actual.scale,
        expected_transform.scale,
    );
}

/// `sync_scene_to_ecs_system` must leave `Transform` unchanged for
/// entities whose `MirroredBody(id)` does not resolve in `Scene`.
/// Covers Requirement 9.2.
#[test]
fn sync_leaves_transform_untouched_for_unresolvable_ids() {
    // Any Scene works; `BodyId(u64::MAX)` is guaranteed never to
    // have been handed out by `Scene::add` on a fresh `Scene::demo()`
    // (which only allocates a handful of small ids), so the sync
    // system must take the "None" branch for it.
    let scene = Scene::demo();
    let bogus_id = BodyId(u64::MAX);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(scene);
    app.add_systems(Update, sync_scene_to_ecs_system);

    let initial_transform = Transform::from_xyz(7.0, 8.0, 9.0);
    let entity = app
        .world_mut()
        .spawn((MirroredBody(bogus_id), initial_transform))
        .id();

    app.update();

    let actual = *app
        .world()
        .entity(entity)
        .get::<Transform>()
        .expect("entity must still carry a Transform");
    assert_eq!(
        actual.translation, initial_transform.translation,
        "Requirement 9.2: Transform must be unchanged when Scene::get returns None"
    );
    assert_eq!(actual.rotation, initial_transform.rotation);
    assert_eq!(actual.scale, initial_transform.scale);
}

/// `sync_scene_to_ecs_system` must not mutate the `Scene` resource.
/// Covers Requirement 9.3. (The compile-time `Res<Scene>` parameter
/// already enforces this — this test guards against a future
/// refactor that widens the parameter to `ResMut<Scene>`.)
#[test]
fn sync_does_not_mutate_scene() {
    let (scene, id, _body) = demo_scene_with_sphere(Vec3::ZERO, 1.0);
    let scene_before = scene.clone();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(scene);
    app.add_systems(Update, sync_scene_to_ecs_system);
    app.world_mut()
        .spawn((MirroredBody(id), Transform::default()));

    app.update();

    let scene_after = app.world().resource::<Scene>();
    assert_eq!(
        scene_after.props, scene_before.props,
        "Scene::bodies must be untouched by sync_scene_to_ecs_system"
    );
    assert_eq!(
        scene_after.bounds, scene_before.bounds,
        "Scene::bounds must be untouched by sync_scene_to_ecs_system"
    );
}

// Silence unused-import warnings when earlier tests shift; `Query`
// is part of the public signature surface we exercise here and is
// imported for readability even though the test systems don't take
// it directly.
#[allow(dead_code)]
fn _sync_signature_stub(_scene: Res<Scene>, _query: Query<(&MirroredBody, &mut Transform)>) {}
