//! Unit and property tests for the [`crate::core::scene`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/core/scene.rs`
//! via `#[path]`, not as a standalone integration test, so these tests keep
//! access to the module's private items and to `super::*`.

use super::*;
use crate::time::simulation::TimeStep;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Strategy: a finite `f32` in a safe closed range.
///
/// The range is bounded well below `f32::MAX` so downstream arithmetic
/// (e.g. `2.0 * half_extents` in the render bridge) cannot overflow to
/// infinity, while still being wide enough to cover realistic scene
/// coordinates.
/// A `TimeStep` from a plain number of seconds, for tests that are about durations
/// rather than about validation. Panics on an invalid duration, which is what a test
/// wants: an unusable literal here is a mistake in the test, not a case to handle.
fn seconds(value: f64) -> TimeStep {
    TimeStep::from_seconds(value).expect("test durations must be valid")
}

fn finite_f32() -> impl Strategy<Value = f32> {
    -1.0e6_f32..=1.0e6_f32
}

/// Strategy: a strictly positive, finite `f32`.
///
/// Lower-bounded away from zero to avoid subnormals; upper-bounded to
/// keep downstream arithmetic finite.
fn positive_finite_f32() -> impl Strategy<Value = f32> {
    1.0e-3_f32..1.0e6_f32
}

/// Strategy: a non-negative, finite `f32` (for `Body::Line.width`).
fn non_negative_finite_f32() -> impl Strategy<Value = f32> {
    0.0_f32..1.0e6_f32
}

/// Strategy: a `Vec3` with finite components.
fn finite_vec3() -> impl Strategy<Value = Vec3> {
    (finite_f32(), finite_f32(), finite_f32()).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

/// Strategy: a finite `DVec3`, for the positions and velocities the simulation integrates.
fn finite_dvec3() -> impl Strategy<Value = DVec3> {
    finite_vec3().prop_map(|v| v.as_dvec3())
}

/// Strategy: a `Vec3` whose components are all strictly positive and finite,
/// suitable for `Body::Cuboid.half_extents`.
fn positive_vec3() -> impl Strategy<Value = Vec3> {
    (
        positive_finite_f32(),
        positive_finite_f32(),
        positive_finite_f32(),
    )
        .prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

/// Strategy: a unit quaternion (within `1e-4` of length `1.0`).
///
/// Generates raw xyzw components in `[-1, 1]`, filters out near-zero
/// vectors (which would NaN under `normalize`), then normalizes.
fn unit_quat() -> impl Strategy<Value = Quat> {
    (
        -1.0_f32..=1.0,
        -1.0_f32..=1.0,
        -1.0_f32..=1.0,
        -1.0_f32..=1.0,
    )
        .prop_filter(
            "quaternion must be non-degenerate before normalization",
            |(x, y, z, w)| {
                let sq = x * x + y * y + z * z + w * w;
                sq > 1.0e-6
            },
        )
        .prop_map(|(x, y, z, w)| Quat::from_xyzw(x, y, z, w).normalize())
}

/// Strategy: a `Pose` satisfying `Pose::is_valid`.
fn valid_pose() -> impl Strategy<Value = Pose> {
    (finite_dvec3(), unit_quat()).prop_map(|(position, rotation)| Pose { position, rotation })
}

/// Strategy: a `Color` satisfying `Color::is_valid`
/// (each channel finite and in `[0.0, 1.0]`).
fn valid_color() -> impl Strategy<Value = Color> {
    (0.0_f32..=1.0, 0.0_f32..=1.0, 0.0_f32..=1.0, 0.0_f32..=1.0).prop_map(|(r, g, b, a)| Color {
        r,
        g,
        b,
        a,
    })
}

/// Strategy: a `Body::Sphere` with valid fields.
fn valid_sphere() -> impl Strategy<Value = Body> {
    (valid_pose(), positive_finite_f32(), valid_color()).prop_map(|(pose, radius, color)| {
        Body::Sphere {
            pose,
            radius,
            color,
        }
    })
}

/// Strategy: a `Body::Cuboid` with valid fields.
fn valid_cuboid() -> impl Strategy<Value = Body> {
    (valid_pose(), positive_vec3(), valid_color()).prop_map(|(pose, half_extents, color)| {
        Body::Cuboid {
            pose,
            half_extents,
            color,
        }
    })
}

/// Strategy: a `Body::Line` with valid fields.
fn valid_line() -> impl Strategy<Value = Body> {
    (
        finite_dvec3(),
        finite_dvec3(),
        non_negative_finite_f32(),
        valid_color(),
    )
        .prop_map(|(from, to, width, color)| Body::Line {
            from,
            to,
            width,
            color,
        })
}

/// Strategy: a `Body::Plane` with valid fields.
fn valid_plane() -> impl Strategy<Value = Body> {
    (valid_pose(), positive_finite_f32(), valid_color()).prop_map(|(pose, half_size, color)| {
        Body::Plane {
            pose,
            half_size,
            color,
        }
    })
}

/// Strategy: any `Body` variant with fields satisfying the per-variant
/// Validation_Rules from Requirement 7.
fn valid_body() -> impl Strategy<Value = Body> {
    prop_oneof![valid_sphere(), valid_cuboid(), valid_line(), valid_plane()]
}

/// Strategy: a mass in kilograms satisfying `Prop`'s rules — finite and
/// non-negative. Includes `0.0`, which means genuinely massless (not static).
fn valid_mass() -> impl Strategy<Value = f32> {
    0.0_f32..1.0e6_f32
}

/// Strategy: a movable `Prop` with a finite velocity and a valid mass.
fn movable_prop() -> impl Strategy<Value = Prop> {
    (valid_body(), finite_dvec3(), valid_mass())
        .prop_map(|(body, velocity_units_s, mass_kg)| Prop::new(body, velocity_units_s, mass_kg))
}

/// Strategy: an immovable `Prop`. Velocity is forced to zero, which is what
/// `Prop::is_valid` requires of immovable props.
fn immovable_prop() -> impl Strategy<Value = Prop> {
    valid_body().prop_map(Prop::immovable)
}

/// Strategy: any valid `Prop`, movable or immovable.
fn valid_prop() -> impl Strategy<Value = Prop> {
    prop_oneof![movable_prop(), immovable_prop()]
}

/// Strategy: an arbitrary starting `Scene`, built by making a sequence of
/// between `0` and `9` valid `Scene::add` calls on a freshly-constructed
/// empty `Scene` with fixed valid bounds.
///
/// The bounds mirror the ones used by `Scene::demo` so they satisfy
/// `Aabb::is_valid` with strictly positive extent on every axis; the
/// property under test is about `add`, not about `bounds`, so fixing them
/// keeps the shrinker focused on the body-shaped inputs.
fn arbitrary_scene() -> impl Strategy<Value = Scene> {
    prop::collection::vec(valid_prop(), 0..10).prop_map(|props| {
        let mut scene = Scene {
            props: Vec::new(),
            bounds: Aabb {
                min: Vec3::new(-50.0, -1.0, -50.0),
                max: Vec3::new(50.0, 50.0, 50.0),
            },
            softening_units: crate::core::constants::GRAVITY_SOFTENING_UNITS,
            ids: Vec::new(),
            next_id: 0,
        };
        for prop in props {
            scene.add(prop);
        }
        scene
    })
}

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
const REGRESSION_SEEDS: &str = "tst/core/proptest-regressions/scene_tests.txt";

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 1: Body construction preserves validation.
    ///
    /// **Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5, 7.6**
    ///
    /// Every `Body` built from fields that already satisfy the per-variant
    /// Validation_Rules (positive finite radii/extents/half_sizes, finite
    /// positions, normalized quaternions, RGBA in `[0, 1]`, non-negative
    /// finite line widths) must report `is_valid() == true`.
    #[test]
    fn body_construction_preserves_validation(body in valid_body()) {
        prop_assert!(body.is_valid(), "expected valid body, got {:?}", body);
    }

    /// Property 1b: every prop the strategies build is valid.
    ///
    /// Guards the strategies themselves as much as `Prop::is_valid` — an
    /// `immovable_prop()` that forgot to zero the velocity would fail here rather
    /// than silently weakening every property that consumes it.
    #[test]
    fn prop_construction_preserves_validation(prop in valid_prop()) {
        prop_assert!(prop.is_valid(), "expected valid prop, got {:?}", prop);
    }

    /// Property 1c: `speed` is the velocity's magnitude and `heading` is a unit
    /// vector whenever it is `Some`.
    ///
    /// The heading's unit length is the real content: it is the guarantee a caller
    /// needs before multiplying it by a scalar distance, and it is exactly what
    /// normalising a near-zero vector would break.
    #[test]
    fn speed_and_heading_agree_with_the_velocity_vector(prop in movable_prop()) {
        prop_assert_eq!(prop.speed(), prop.velocity_units_s.length());

        match prop.heading() {
            Some(heading) => {
                prop_assert!(
                    (heading.length() - 1.0).abs() <= 1.0e-4,
                    "heading {:?} is not unit length", heading
                );
                prop_assert!(prop.speed() > 0.0, "a prop with a heading must be moving");
            }
            None => prop_assert_eq!(
                prop.speed(), 0.0,
                "only a stationary prop may lack a heading"
            ),
        }
    }

    /// Property 1d: the three attributes stay independent.
    ///
    /// `responds_to_forces` and `is_massless` must each be exactly their stated
    /// conjunction — never a shortcut that conflates zero mass with immovability,
    /// which is the confusion this data model exists to avoid.
    #[test]
    fn mass_and_immovability_stay_independent(prop in valid_prop()) {
        prop_assert_eq!(
            prop.responds_to_forces(),
            !prop.immovable && prop.mass_kg > 0.0
        );
        prop_assert_eq!(prop.is_massless(), !prop.immovable && prop.mass_kg == 0.0);

        // The two are mutually exclusive: a prop cannot both be pushable and be
        // massless.
        prop_assert!(!(prop.responds_to_forces() && prop.is_massless()));
    }

    /// Property 2: `Scene::add` is additive and id-stable.
    ///
    /// **Validates: Requirements 4.3, 4.4, 4.5**
    ///
    /// For an arbitrary starting `Scene` (built by a sequence of valid
    /// `add` calls on a fresh empty scene) and an arbitrary valid `Body`,
    /// calling `scene.add(b)` must:
    ///
    /// - grow `props.len()` by exactly 1 (Requirement 4.3),
    /// - leave the prefix of `props` up to the old length unchanged
    ///   (Requirement 4.3),
    /// - result in `props.last() == Some(&p)` (Requirement 4.3),
    /// - return a `BodyId` not present in the prior id set
    ///   (Requirement 4.3),
    /// - make `scene.get(returned_id) == Some(&b)` (Requirements 4.4, 4.5).
    #[test]
    fn add_is_additive_and_id_stable(
        initial_scene in arbitrary_scene(),
        new_prop in valid_prop(),
    ) {
        let mut scene = initial_scene;

        // Snapshot pre-state so we can compare after the call.
        let old_len = scene.props.len();
        let old_prefix: Vec<Prop> = scene.props.clone();
        let old_ids: std::collections::HashSet<BodyId> =
            scene.iter().map(|(id, _)| id).collect();

        let returned_id = scene.add(new_prop.clone());

        // Requirement 4.3: length grew by exactly 1.
        prop_assert_eq!(
            scene.props.len(),
            old_len + 1,
            "Scene::add must grow props.len() by exactly 1"
        );

        // Requirement 4.3: the prefix up to the old length is unchanged.
        prop_assert_eq!(
            &scene.props[..old_len],
            old_prefix.as_slice(),
            "Scene::add must not mutate previously-present props"
        );

        // Requirement 4.3: the newly-appended body is `b`.
        prop_assert_eq!(
            scene.props.last(),
            Some(&new_prop),
            "Scene::add must append the provided prop at the end"
        );

        // Requirement 4.3: the returned id is fresh.
        prop_assert!(
            !old_ids.contains(&returned_id),
            "Scene::add returned a BodyId {:?} that was already present \
             in the prior id set {:?}",
            returned_id,
            old_ids
        );

        // Requirements 4.4 and 4.5: the returned id resolves to `p`.
        prop_assert_eq!(
            scene.get(returned_id),
            Some(&new_prop),
            "scene.get(returned_id) must yield the just-added prop"
        );
    }

    /// Property 3: `Scene::step` preserves invariants.
    ///
    /// **Validates: Requirements 5.1, 5.3**
    ///
    /// For an arbitrary valid `Scene` (built by a sequence of valid `add`
    /// calls on a fresh empty scene with fixed valid bounds) and any `dt`
    /// in the clamped range `[0.0, 0.1]` used by `step_scene_system`,
    /// calling `scene.step(seconds(f64::from(dt)))` must:
    ///
    /// - leave every `Body` in `scene.props` satisfying the
    ///   Validation_Rules (Requirement 5.1), and
    /// - leave `scene.bounds` unchanged (Requirement 5.3).
    #[test]
    fn step_preserves_invariants(
        initial_scene in arbitrary_scene(),
        dt in 0.0_f32..=0.1_f32,
    ) {
        let mut scene = initial_scene;

        // Snapshot bounds so we can assert they are unchanged after the
        // call (Requirement 5.3).
        let bounds_before = scene.bounds;

        scene.step(seconds(f64::from(dt)));

        // Requirement 5.3: `bounds` is unchanged.
        prop_assert_eq!(
            scene.bounds,
            bounds_before,
            "Scene::step must not modify scene.bounds"
        );

        // Requirement 5.1: every body still satisfies the Validation_Rules.
        for (i, body) in scene.props.iter().enumerate() {
            prop_assert!(
                body.is_valid(),
                "Scene::step left body at index {} invalid: {:?}",
                i,
                body
            );
        }
    }

    /// Property 4: `step(0.0)` is a no-op.
    ///
    /// **Validates: Requirements 5.2, 5.3**
    ///
    /// For an arbitrary valid `Scene`, snapshotting `bodies` and `bounds`
    /// before calling `scene.step(seconds(0.0))` and comparing them afterward must
    /// yield equality on both:
    ///
    /// - `scene.props` is byte-identical (`==`) to its pre-call value
    ///   (Requirement 5.2), and
    /// - `scene.bounds` is unchanged (Requirement 5.3).
    #[test]
    fn step_zero_is_noop(initial_scene in arbitrary_scene()) {
        let mut scene = initial_scene;

        // Snapshot `bodies` and `bounds` before the call so we can assert
        // exact equality afterward.
        let bodies_before = scene.props.clone();
        let bounds_before = scene.bounds;

        scene.step(seconds(0.0));

        // Requirement 5.2: `bodies` is unchanged by `step(0.0)`.
        prop_assert_eq!(
            &scene.props,
            &bodies_before,
            "Scene::step(0.0) must not modify scene.props"
        );

        // Requirement 5.3: `bounds` is unchanged by `step(0.0)`.
        prop_assert_eq!(
            scene.bounds,
            bounds_before,
            "Scene::step(0.0) must not modify scene.bounds"
        );
    }

    /// Property 5: `Scene::step` preserves `BodyId` resolvability.
    ///
    /// **Validates: Requirements 5.4**
    ///
    /// For an arbitrary valid `Scene` and any `dt` in the clamped range
    /// `[0.0, 0.1]` used by `step_scene_system`, the set of `BodyId`s
    /// resolvable by `scene.get(_)` must be invariant across the call:
    ///
    /// - every `id` yielded by `scene.iter()` before the call still
    ///   resolves via `scene.get(id).is_some()` after the call, and
    /// - the id set after the call is exactly equal to the id set before
    ///   the call — no fresh ids, no lost ids.
    #[test]
    fn step_preserves_id_resolvability(
        initial_scene in arbitrary_scene(),
        dt in 0.0_f32..=0.1_f32,
    ) {
        let mut scene = initial_scene;

        // Snapshot the set of resolvable ids before the call.
        let ids_before: std::collections::HashSet<BodyId> =
            scene.iter().map(|(id, _)| id).collect();

        scene.step(seconds(f64::from(dt)));

        // Every id resolvable before the call is still resolvable after.
        for id in &ids_before {
            prop_assert!(
                scene.get(*id).is_some(),
                "Scene::step dropped BodyId {:?} that was resolvable before the call",
                id
            );
        }

        // The id set after the call is exactly the id set before — no
        // new ids introduced, no existing ids removed.
        let ids_after: std::collections::HashSet<BodyId> =
            scene.iter().map(|(id, _)| id).collect();
        prop_assert_eq!(
            ids_after,
            ids_before,
            "Scene::step must preserve the exact set of BodyIds"
        );
    }
}

// -----------------------------------------------------------------------
// Unit tests for validation edge cases (task 2.4).
//
// Each test targets one boundary rejection from the Validation_Rules in
// Requirement 7. Positive sanity-check tests are included alongside each
// variant to contrast with the rejection cases and guard against an
// `is_valid` that trivially always returns `false`.
// -----------------------------------------------------------------------

/// A canonical valid `Pose` used as a stable baseline by the edge-case
/// tests below so they only vary the field under test.
fn baseline_pose() -> Pose {
    Pose {
        position: DVec3::ZERO,
        rotation: Quat::IDENTITY,
    }
}

/// A canonical valid `Color` used as a stable baseline by the edge-case
/// tests below so they only vary the field under test.
fn baseline_color() -> Color {
    Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    }
}

// --- Body::Sphere ------------------------------------------------------

/// Sanity: a positive-radius sphere with a valid pose and color validates.
/// Requirement 7.1.
#[test]
fn sphere_with_positive_radius_is_valid() {
    let body = Body::Sphere {
        pose: baseline_pose(),
        radius: 1.0,
        color: baseline_color(),
    };
    assert!(body.is_valid());
}

/// Requirement 7.1: `radius == 0.0` is rejected (strict `> 0.0`).
#[test]
fn sphere_with_zero_radius_is_invalid() {
    let body = Body::Sphere {
        pose: baseline_pose(),
        radius: 0.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.1: negative radius is rejected.
#[test]
fn sphere_with_negative_radius_is_invalid() {
    let body = Body::Sphere {
        pose: baseline_pose(),
        radius: -1.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.1: `NaN` radius is rejected (must be finite).
#[test]
fn sphere_with_nan_radius_is_invalid() {
    let body = Body::Sphere {
        pose: baseline_pose(),
        radius: f32::NAN,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.1: infinite radius is rejected (must be finite).
#[test]
fn sphere_with_infinite_radius_is_invalid() {
    let body = Body::Sphere {
        pose: baseline_pose(),
        radius: f32::INFINITY,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

// --- Body::Cuboid ------------------------------------------------------

/// Sanity: a cuboid with strictly positive half-extents validates.
/// Requirement 7.2.
#[test]
fn cuboid_with_positive_half_extents_is_valid() {
    let body = Body::Cuboid {
        pose: baseline_pose(),
        half_extents: Vec3::splat(0.5),
        color: baseline_color(),
    };
    assert!(body.is_valid());
}

/// Requirement 7.2: `half_extents.x == 0.0` is rejected (strict `> 0.0`
/// component-wise).
#[test]
fn cuboid_with_zero_half_extents_x_is_invalid() {
    let body = Body::Cuboid {
        pose: baseline_pose(),
        half_extents: Vec3::new(0.0, 0.5, 0.5),
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.2: a non-finite component in `half_extents` is rejected.
#[test]
fn cuboid_with_non_finite_half_extents_is_invalid() {
    let body = Body::Cuboid {
        pose: baseline_pose(),
        half_extents: Vec3::new(0.5, f32::NAN, 0.5),
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

// --- Body::Line --------------------------------------------------------

/// Sanity: a line with finite endpoints and non-negative width validates.
/// Requirement 7.3.
#[test]
fn line_with_finite_endpoints_is_valid() {
    let body = Body::Line {
        from: DVec3::ZERO,
        to: DVec3::new(1.0, 0.0, 0.0),
        width: 0.0,
        color: baseline_color(),
    };
    assert!(body.is_valid());
}

/// Requirement 7.3: a non-finite component in `from` is rejected.
#[test]
fn line_with_non_finite_from_is_invalid() {
    let body = Body::Line {
        from: DVec3::new(f64::INFINITY, 0.0, 0.0),
        to: DVec3::ZERO,
        width: 0.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.3: a non-finite component in `to` is rejected.
#[test]
fn line_with_non_finite_to_is_invalid() {
    let body = Body::Line {
        from: DVec3::ZERO,
        to: DVec3::new(0.0, f64::NAN, 0.0),
        width: 0.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.3: negative width is rejected (`width >= 0.0`).
#[test]
fn line_with_negative_width_is_invalid() {
    let body = Body::Line {
        from: DVec3::ZERO,
        to: DVec3::X,
        width: -0.1,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

// --- Body::Plane -------------------------------------------------------

/// Sanity: a plane with positive half-size validates. Requirement 7.4.
#[test]
fn plane_with_positive_half_size_is_valid() {
    let body = Body::Plane {
        pose: baseline_pose(),
        half_size: 10.0,
        color: baseline_color(),
    };
    assert!(body.is_valid());
}

/// Requirement 7.4: `half_size == 0.0` is rejected (strict `> 0.0`).
#[test]
fn plane_with_zero_half_size_is_invalid() {
    let body = Body::Plane {
        pose: baseline_pose(),
        half_size: 0.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.4: negative half-size is rejected.
#[test]
fn plane_with_negative_half_size_is_invalid() {
    let body = Body::Plane {
        pose: baseline_pose(),
        half_size: -1.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.4: non-finite half-size is rejected.
#[test]
fn plane_with_non_finite_half_size_is_invalid() {
    let body = Body::Plane {
        pose: baseline_pose(),
        half_size: f32::INFINITY,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

// --- Color -------------------------------------------------------------

/// Sanity: a color with all channels in `[0, 1]` validates.
/// Requirement 7.6.
#[test]
fn color_with_in_range_channels_is_valid() {
    let color = Color {
        r: 0.0,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    };
    assert!(color.is_valid());
}

/// Requirement 7.6: a channel greater than `1.0` is rejected.
#[test]
fn color_with_channel_above_one_is_invalid() {
    let color = Color {
        r: 1.1,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    assert!(!color.is_valid());
}

/// Requirement 7.6: a channel less than `0.0` is rejected.
#[test]
fn color_with_negative_channel_is_invalid() {
    let color = Color {
        r: 0.5,
        g: -0.01,
        b: 0.5,
        a: 1.0,
    };
    assert!(!color.is_valid());
}

/// Requirement 7.6: a non-finite channel is rejected.
#[test]
fn color_with_non_finite_channel_is_invalid() {
    let color = Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: f32::NAN,
    };
    assert!(!color.is_valid());
}

/// Requirement 7.6 (carried through 7.1–7.4): a `Body` whose otherwise
/// valid fields carry an out-of-range color is itself invalid.
#[test]
fn sphere_with_out_of_range_color_is_invalid() {
    let body = Body::Sphere {
        pose: baseline_pose(),
        radius: 1.0,
        color: Color {
            r: 2.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    };
    assert!(!body.is_valid());
}

// --- Pose --------------------------------------------------------------

/// Sanity: a pose with finite position and the identity rotation validates.
/// Requirement 7.5.
#[test]
fn pose_with_identity_rotation_is_valid() {
    let pose = Pose {
        position: DVec3::new(1.0, 2.0, 3.0),
        rotation: Quat::IDENTITY,
    };
    assert!(pose.is_valid());
}

/// Requirement 7.5: a non-unit quaternion is rejected.
///
/// `Quat::from_xyzw(2.0, 0.0, 0.0, 0.0)` has length `2.0`, which is well
/// outside the `1e-4` tolerance of unit length.
#[test]
fn pose_with_non_unit_rotation_is_invalid() {
    let pose = Pose {
        position: DVec3::ZERO,
        rotation: Quat::from_xyzw(2.0, 0.0, 0.0, 0.0),
    };
    assert!(!pose.is_valid());
}

/// Requirement 7.5: a zero quaternion (length `0`) is rejected.
#[test]
fn pose_with_zero_rotation_is_invalid() {
    let pose = Pose {
        position: DVec3::ZERO,
        rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
    };
    assert!(!pose.is_valid());
}

/// Requirement 7.5: a non-finite component in `position` is rejected.
#[test]
fn pose_with_non_finite_position_is_invalid() {
    let pose = Pose {
        position: DVec3::new(f64::INFINITY, 0.0, 0.0),
        rotation: Quat::IDENTITY,
    };
    assert!(!pose.is_valid());
}

/// Requirement 7.5: a `NaN` component in `position` is rejected.
#[test]
fn pose_with_nan_position_is_invalid() {
    let pose = Pose {
        position: DVec3::new(0.0, f64::NAN, 0.0),
        rotation: Quat::IDENTITY,
    };
    assert!(!pose.is_valid());
}

/// Requirement 7.5 (carried through 7.1): a sphere with a non-finite pose
/// position is itself invalid.
#[test]
fn sphere_with_non_finite_pose_position_is_invalid() {
    let body = Body::Sphere {
        pose: Pose {
            position: DVec3::new(f64::NAN, 0.0, 0.0),
            rotation: Quat::IDENTITY,
        },
        radius: 1.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

/// Requirement 7.5 (carried through 7.1): a sphere with a non-unit
/// rotation is itself invalid.
#[test]
fn sphere_with_non_unit_rotation_is_invalid() {
    let body = Body::Sphere {
        pose: Pose {
            position: DVec3::ZERO,
            // Length 2.0, far outside the 1e-4 tolerance.
            rotation: Quat::from_xyzw(2.0, 0.0, 0.0, 0.0),
        },
        radius: 1.0,
        color: baseline_color(),
    };
    assert!(!body.is_valid());
}

// --- Aabb --------------------------------------------------------------

/// Sanity: a well-ordered, finite `Aabb` validates. Requirement 7.7.
#[test]
fn aabb_with_min_less_than_max_is_valid() {
    let aabb = Aabb {
        min: Vec3::new(-1.0, -2.0, -3.0),
        max: Vec3::new(1.0, 2.0, 3.0),
    };
    assert!(aabb.is_valid());
}

/// Requirement 7.7: `min == max` is allowed (inclusive `<=`), a zero-volume
/// box is still valid.
#[test]
fn aabb_with_min_equal_to_max_is_valid() {
    let aabb = Aabb {
        min: Vec3::new(1.0, 2.0, 3.0),
        max: Vec3::new(1.0, 2.0, 3.0),
    };
    assert!(aabb.is_valid());
}

/// Requirement 7.7: `min.x > max.x` is rejected.
#[test]
fn aabb_with_min_x_greater_than_max_x_is_invalid() {
    let aabb = Aabb {
        min: Vec3::new(1.0, 0.0, 0.0),
        max: Vec3::new(-1.0, 1.0, 1.0),
    };
    assert!(!aabb.is_valid());
}

/// Requirement 7.7: `min.y > max.y` is rejected (exercising a second axis).
#[test]
fn aabb_with_min_y_greater_than_max_y_is_invalid() {
    let aabb = Aabb {
        min: Vec3::new(0.0, 5.0, 0.0),
        max: Vec3::new(1.0, 1.0, 1.0),
    };
    assert!(!aabb.is_valid());
}

/// Requirement 7.7: a non-finite component in `min` is rejected.
#[test]
fn aabb_with_non_finite_min_is_invalid() {
    let aabb = Aabb {
        min: Vec3::new(f32::NEG_INFINITY, 0.0, 0.0),
        max: Vec3::new(1.0, 1.0, 1.0),
    };
    assert!(!aabb.is_valid());
}

/// Requirement 7.7: a non-finite component in `max` is rejected.
#[test]
fn aabb_with_non_finite_max_is_invalid() {
    let aabb = Aabb {
        min: Vec3::new(0.0, 0.0, 0.0),
        max: Vec3::new(1.0, f32::NAN, 1.0),
    };
    assert!(!aabb.is_valid());
}

// -----------------------------------------------------------------------
// -----------------------------------------------------------------------
// Unit tests for `Prop`: the velocity vector, the mass, and immovability.
//
// The three attributes are deliberately independent, and the tests below are
// mostly about pinning that independence — in particular that `mass_kg == 0.0`
// means *massless*, never *static*, so a future photon-like prop stays
// expressible. See the table on `Prop` in `src/core/scene.rs`.
// -----------------------------------------------------------------------

/// A valid unit sphere, for building props without repeating the geometry.
fn baseline_body() -> Body {
    Body::Sphere {
        pose: baseline_pose(),
        radius: 1.0,
        color: baseline_color(),
    }
}

/// `Prop::new` is movable, keeps its velocity and mass, and validates.
#[test]
fn prop_new_is_movable_and_keeps_its_attributes() {
    let velocity = DVec3::new(1.0, -2.0, 3.0);
    let prop = Prop::new(baseline_body(), velocity, 2.5);

    assert_eq!(prop.velocity_units_s, velocity);
    assert_eq!(prop.mass_kg, 2.5);
    assert!(!prop.immovable);
    assert!(prop.is_valid());
}

/// `Prop::at_rest` is movable with a zero velocity.
#[test]
fn prop_at_rest_has_zero_velocity() {
    let prop = Prop::at_rest(baseline_body(), 4.0);

    assert_eq!(prop.velocity_units_s, DVec3::ZERO);
    assert_eq!(prop.mass_kg, 4.0);
    assert!(!prop.immovable);
    assert_eq!(prop.speed(), 0.0);
    assert!(prop.is_valid());
}

/// `Prop::immovable` sets the flag and forces the velocity to zero.
#[test]
fn prop_immovable_is_flagged_and_stationary() {
    let prop = Prop::immovable(baseline_body());

    assert!(prop.immovable);
    assert_eq!(prop.velocity_units_s, DVec3::ZERO);
    assert!(prop.is_valid());
}

/// The velocity vector's magnitude is the speed, and its direction the heading.
///
/// This is the whole point of storing movement as one vector, so it is pinned
/// against a 3-4-5-style triple whose length is exact in binary floating point.
#[test]
fn the_velocity_vector_carries_both_speed_and_heading() {
    // (3, 4, 0) has length exactly 5.
    let prop = Prop::new(baseline_body(), DVec3::new(3.0, 4.0, 0.0), 1.0);

    assert_eq!(prop.speed(), 5.0);
    assert_eq!(prop.heading(), Some(DVec3::new(0.6, 0.8, 0.0)));
}

/// A prop at rest has no heading.
///
/// `None` rather than a zero vector: normalising `Vec3::ZERO` is undefined, and
/// returning `Vec3::ZERO` would read as a legitimate direction.
#[test]
fn a_prop_at_rest_has_no_heading() {
    assert_eq!(Prop::at_rest(baseline_body(), 1.0).heading(), None);
    assert_eq!(Prop::immovable(baseline_body()).heading(), None);
}

/// Mass zero means massless, not static.
///
/// The distinction the data model exists to preserve: a massless movable prop
/// still travels at its velocity, it simply cannot be accelerated by a force.
#[test]
fn zero_mass_means_massless_not_static() {
    let photon_like = Prop::new(baseline_body(), DVec3::new(0.0, 0.0, -1.0), 0.0);

    assert!(photon_like.is_valid(), "a massless prop is valid");
    assert!(photon_like.is_massless());
    assert!(!photon_like.immovable, "zero mass must not imply immovable");
    assert!(
        !photon_like.responds_to_forces(),
        "F = m*a cannot accelerate a massless prop"
    );
    assert_eq!(photon_like.speed(), 1.0, "but it still moves");
}

/// Only a movable prop with positive mass responds to forces.
#[test]
fn responds_to_forces_requires_movable_and_massive() {
    assert!(Prop::at_rest(baseline_body(), 1.0).responds_to_forces());
    assert!(!Prop::at_rest(baseline_body(), 0.0).responds_to_forces());
    assert!(!Prop::immovable(baseline_body()).responds_to_forces());
}

/// An immovable prop is never reported as massless.
///
/// Its `mass_kg` carries no simulation meaning, so calling it massless would be a
/// claim the field does not support — even though `Prop::immovable` happens to
/// store `0.0` there.
#[test]
fn an_immovable_prop_is_not_massless() {
    let wall = Prop::immovable(baseline_body());

    assert_eq!(wall.mass_kg, 0.0);
    assert!(!wall.is_massless());
}

/// `is_valid` rejects a non-finite velocity component.
#[test]
fn a_non_finite_velocity_is_invalid() {
    let mut prop = Prop::at_rest(baseline_body(), 1.0);

    prop.velocity_units_s = DVec3::new(f64::NAN, 0.0, 0.0);
    assert!(!prop.is_valid());

    prop.velocity_units_s = DVec3::new(0.0, f64::INFINITY, 0.0);
    assert!(!prop.is_valid());
}

/// `is_valid` rejects a negative or non-finite mass.
#[test]
fn a_negative_or_non_finite_mass_is_invalid() {
    let mut prop = Prop::at_rest(baseline_body(), 1.0);

    prop.mass_kg = -1.0;
    assert!(!prop.is_valid(), "negative mass is not physical");

    prop.mass_kg = f32::NAN;
    assert!(!prop.is_valid());

    prop.mass_kg = f32::INFINITY;
    assert!(
        !prop.is_valid(),
        "infinite mass must be spelled `immovable`, not stored as a mass"
    );
}

/// An immovable prop with a velocity is a contradiction, and invalid.
///
/// Deliberately rejected rather than silently ignored: the author of
/// `immovable: true, velocity_units_s: (0, 1, 0)` meant something, and leaving them
/// to wonder why their wall never moved is worse than a loud failure.
#[test]
fn an_immovable_prop_with_a_velocity_is_invalid() {
    let mut wall = Prop::immovable(baseline_body());
    wall.velocity_units_s = DVec3::new(0.0, 1.0, 0.0);

    assert!(!wall.is_valid());
}

/// An invalid body makes the whole prop invalid.
///
/// `Prop::is_valid` composes `Body::is_valid` rather than restating it.
#[test]
fn an_invalid_body_makes_the_prop_invalid() {
    let bad = Prop::at_rest(
        Body::Sphere {
            pose: baseline_pose(),
            radius: 0.0,
            color: baseline_color(),
        },
        1.0,
    );

    assert!(!bad.is_valid());
}

// Unit tests for `Scene::demo` content.
//
// `Scene::demo` is the simulation container: one ground plane and nothing
// else. These tests pin:
//   - the container shape: exactly one body, and it is a ground `Plane`.
//   - 12.2: `bounds` is valid and has strictly positive extent on every
//     axis.
//   - 12.3: every body in `bodies` satisfies the Body Validation_Rules.
//
// Requirements 3.2 and 12.1 originally demanded one of each of the four
// variants at startup. Those props were deliberately removed when the
// project turned into a physics simulation — see the amendment in
// the original requirements spec and the note on `Scene::demo`.
// The test below asserts the *opposite* of the original 12.1 on purpose, so
// that re-adding decoration is a visible, deliberate act rather than a
// silent drift back.
// -----------------------------------------------------------------------

/// `Scene::demo` is exactly one ground plane — the simulation container.
///
/// Asserted as an exact body count rather than "contains a plane", because the
/// guarantee being made is *emptiness*: anything else visible in a running app
/// must have been put there by a simulation. A prop added back here would fail
/// this test, which is the intent.
#[test]
fn demo_is_exactly_one_ground_plane() {
    let scene = Scene::demo();

    assert_eq!(
        scene.props.len(),
        1,
        "Scene::demo must hold only the container plane, got {:?}",
        scene.props
    );

    match &scene.props[0].body {
        Body::Plane {
            pose,
            half_size,
            color,
        } => {
            assert_eq!(
                pose.position,
                DVec3::ZERO,
                "the container plane must sit at the origin"
            );
            assert_eq!(
                pose.rotation,
                Quat::IDENTITY,
                "the container plane must be unrotated, so the floor is the XZ plane and +Y is up"
            );
            assert!(
                *half_size > 0.0,
                "the container plane must have a positive half_size, got {}",
                half_size
            );
            assert!(
                color.is_valid(),
                "the container plane's colour must be valid"
            );
        }
        other => panic!(
            "Scene::demo's only body must be a Body::Plane, got {:?}",
            other
        ),
    }
}

/// The container plane's surface is inside the bounds vertically, and its footprint
/// **covers** them horizontally.
///
/// This assertion was inverted by amendment A5's successor: the floor used to be required to
/// fit *within* the bounds (`half_size` 25 against `±50`), which meant "inside the world" and
/// "over the floor" were the same boundary — so a body ejected from the world left the floor
/// in the same instant and fell into empty space. The floor is now `2000` units to the side,
/// deliberately far outside the bounds, so the meaningful invariant is the other way round:
/// **anywhere a prop can legally be, there is floor beneath it.**
///
/// The vertical check is unchanged and still matters: a floor outside the world on `y` would
/// put every simulated body below it.
#[test]
fn the_container_plane_covers_the_scene_bounds() {
    let scene = Scene::demo();

    let Body::Plane {
        pose, half_size, ..
    } = &scene.props[0].body
    else {
        panic!("Scene::demo's only body must be a Body::Plane");
    };

    assert!(
        pose.position.y >= f64::from(scene.bounds.min.y)
            && pose.position.y <= f64::from(scene.bounds.max.y),
        "the floor at y = {} must be inside bounds y ∈ [{}, {}]",
        pose.position.y,
        scene.bounds.min.y,
        scene.bounds.max.y
    );
    assert!(
        pose.position.x - f64::from(*half_size) <= f64::from(scene.bounds.min.x)
            && pose.position.x + f64::from(*half_size) >= f64::from(scene.bounds.max.x)
            && pose.position.z - f64::from(*half_size) <= f64::from(scene.bounds.min.z)
            && pose.position.z + f64::from(*half_size) >= f64::from(scene.bounds.max.z),
        "the floor's {}×{} footprint must cover bounds {:?}..{:?}",
        2.0 * half_size,
        2.0 * half_size,
        scene.bounds.min,
        scene.bounds.max
    );
}

/// Requirement 12.2: `Scene::demo().bounds` is valid and has strictly
/// positive extent on every axis (`max > min` component-wise, not merely
/// `>=`, which `Aabb::is_valid` alone would permit).
#[test]
fn demo_bounds_is_valid_with_strictly_positive_extent() {
    let scene = Scene::demo();
    let bounds = scene.bounds;

    assert!(bounds.is_valid(), "Scene::demo().bounds must be valid");
    assert!(
        bounds.max.x > bounds.min.x,
        "expected positive x-extent, got min.x={} max.x={}",
        bounds.min.x,
        bounds.max.x
    );
    assert!(
        bounds.max.y > bounds.min.y,
        "expected positive y-extent, got min.y={} max.y={}",
        bounds.min.y,
        bounds.max.y
    );
    assert!(
        bounds.max.z > bounds.min.z,
        "expected positive z-extent, got min.z={} max.z={}",
        bounds.min.z,
        bounds.max.z
    );
}

/// Requirement 12.3: every body in `Scene::demo().props` satisfies the
/// Body Validation_Rules.
#[test]
fn demo_every_body_is_valid() {
    let scene = Scene::demo();
    assert!(
        !scene.props.is_empty(),
        "Scene::demo must contain at least one body"
    );
    for (i, body) in scene.props.iter().enumerate() {
        assert!(
            body.is_valid(),
            "Scene::demo body at index {} is invalid: {:?}",
            i,
            body
        );
    }
}

// -----------------------------------------------------------------------
// Unit tests for `Scene::add` debug-vs-release behavior (task 3.6).
//
// Requirements 4.6, 7.8, and 14.3 together state that:
//   - In debug builds, `Scene::add` must fire `debug_assert!` on a body
//     that fails the Body Validation_Rules (7.8 / 14.3).
//   - `Scene::get(id)` returns `None` for any `id` not previously handed
//     out by a successful `Scene::add` on this `Scene` (4.6).
//
// The `should_panic` test is gated behind `cfg(debug_assertions)` so that
// `cargo test --release` still succeeds: Requirement 14.4 permits release
// builds to append the invalid body silently, and a `should_panic` test
// under release would report a false failure.
// -----------------------------------------------------------------------

/// Requirements 7.8 and 14.3: in a debug build, `Scene::add` fires
/// `debug_assert!` when handed a body that fails the Validation_Rules.
/// A `Body::Sphere` with `radius == 0.0` violates Requirement 7.1
/// (`radius > 0.0`), so it must trigger the assert.
///
/// `#[should_panic(expected = "Scene::add")]` matches the prefix of the
/// descriptive message from the `debug_assert!` in `Scene::add`.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "Scene::add")]
fn add_with_invalid_body_panics_in_debug() {
    let mut scene = Scene::demo();
    let invalid = Prop::at_rest(
        Body::Sphere {
            pose: baseline_pose(),
            radius: 0.0,
            color: baseline_color(),
        },
        1.0,
    );
    let _ = scene.add(invalid);
}

/// Requirement 4.6: `Scene::get` returns `None` for any `BodyId` that
/// was not returned by a prior successful `Scene::add` on this `Scene`.
///
/// Exercised with `BodyId(u64::MAX)` on a freshly-constructed empty
/// scene: the id allocator starts at `0` and only grows by one per
/// successful `add`, so `u64::MAX` cannot have been handed out.
#[test]
fn get_with_unknown_body_id_returns_none() {
    let scene = Scene {
        props: Vec::new(),
        bounds: Aabb {
            min: Vec3::new(-1.0, -1.0, -1.0),
            max: Vec3::new(1.0, 1.0, 1.0),
        },
        softening_units: crate::core::constants::GRAVITY_SOFTENING_UNITS,
        ids: Vec::new(),
        next_id: 0,
    };
    assert!(
        scene.get(BodyId(u64::MAX)).is_none(),
        "Scene::get(BodyId(u64::MAX)) on an empty scene must return None"
    );
}

// -----------------------------------------------------------------------
// Requirement 5.5 is now enforced by the *type*, not by an assertion.
//
// `Scene::step` used to take an `f32` and `debug_assert!` that it was finite and
// non-negative — a check that fired at the bottom of the call stack, in a debug
// build, after the bad value had already been passed around. It now takes a
// `TimeStep`, which cannot hold an invalid duration at all, so these tests assert
// that the *construction* fails instead. That is strictly stronger: it holds in
// release too, and it is a compile-time guarantee that no other physics entry point
// can be reached with an unvalidated duration (amendment A3).
// -----------------------------------------------------------------------

/// Requirement 5.5, as a type: a negative duration is not a `TimeStep`, so
/// `Scene::step` cannot be called with one.
#[test]
fn a_negative_duration_cannot_become_a_step() {
    assert_eq!(TimeStep::from_seconds(-1.0), None);
}

/// Requirement 5.5, as a type: nor can a non-finite one.
#[test]
fn a_non_finite_duration_cannot_become_a_step() {
    assert_eq!(TimeStep::from_seconds(f64::NAN), None);
    assert_eq!(TimeStep::from_seconds(f64::INFINITY), None);
    assert_eq!(TimeStep::from_seconds(f64::NEG_INFINITY), None);
}

/// And the one duration that *is* legal but must do nothing: a zero step leaves the
/// scene byte-identical (Postcondition 5.2), in debug and release alike.
#[test]
fn a_zero_step_is_a_no_op() {
    let mut scene = Scene::two_body_gravity();
    let before = scene.clone();

    scene.step(TimeStep::ZERO);

    assert_eq!(scene, before);
}

// -----------------------------------------------------------------------
// `Scene::remove` — the one operation that breaks a stated postcondition.
// -----------------------------------------------------------------------

#[test]
fn remove_takes_the_named_prop_and_leaves_the_rest_alone() {
    let mut scene = Scene::two_body_gravity();
    let ids: Vec<_> = scene.iter().map(|(id, _)| id).collect();

    let taken = scene.remove(ids[1]).expect("the prop was there");

    assert_eq!(scene.props.len(), 2);
    assert!(scene.get(ids[1]).is_none());
    assert!(scene.get(ids[0]).is_some(), "the floor is untouched");
    assert!(scene.get(ids[2]).is_some(), "and so is the other sphere");
    assert!(taken.is_valid(), "the prop is handed back intact");
}

#[test]
fn removing_an_unknown_id_is_not_an_error() {
    // Absorption defers its removals to the end of a pass, so the same id can be offered
    // twice in pathological cases. `None` rather than a panic is what makes that safe.
    let mut scene = Scene::two_body_gravity();
    let id = scene.iter().nth(1).map(|(id, _)| id).unwrap();

    assert!(scene.remove(id).is_some());
    assert!(scene.remove(id).is_none());
    assert!(scene.remove(BodyId(9_999)).is_none());
}

#[test]
fn a_removed_id_is_never_handed_out_again() {
    // Amendment A14 lets an id stop resolving; it must never start resolving to something
    // *else*, or every holder of one — a trail, a mirror entity — would silently rebind to a
    // different body.
    let mut scene = Scene::two_body_gravity();
    let removed = scene.iter().nth(1).map(|(id, _)| id).unwrap();
    scene.remove(removed);

    for _ in 0..5 {
        let fresh = scene.add(Prop::at_rest(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::new(0.0, 50.0, 0.0),
                    rotation: Quat::IDENTITY,
                },
                radius: 1.0,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            },
            1.0,
        ));
        assert_ne!(fresh, removed, "the allocator must not reuse an id");
    }

    assert!(scene.get(removed).is_none());
}

#[test]
fn remove_preserves_the_order_of_what_is_left() {
    // Order is part of what callers see — the overlay lists props in it, and the contact pass
    // walks pairs in it — so `remove` is a shift rather than a swap.
    let mut scene = Scene::demo();
    let mut added = Vec::new();
    for index in 0..4 {
        added.push(scene.add(Prop::at_rest(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::new(f64::from(index) * 10.0, 20.0, 0.0),
                    rotation: Quat::IDENTITY,
                },
                radius: 1.0,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            },
            1.0,
        )));
    }

    scene.remove(added[1]);

    let surviving: Vec<_> = scene.iter().map(|(id, _)| id).collect();
    assert_eq!(surviving, vec![surviving[0], added[0], added[2], added[3]]);
}

#[test]
fn remove_keeps_the_scene_steppable() {
    use crate::time::simulation::TimeStep;

    let mut scene = Scene::two_body_gravity();
    let id = scene.iter().nth(1).map(|(id, _)| id).unwrap();
    scene.remove(id);

    scene.step(TimeStep::from_seconds(1.0).unwrap());

    for prop in &scene.props {
        assert!(prop.is_valid());
    }
}

#[test]
fn luminosity_defaults_to_dark_and_is_validated() {
    // A body emits no light unless it is told to, so `with_luminosity` is the only way in and every
    // existing prop is unaffected.
    let ball = Body::Sphere {
        pose: Pose {
            position: DVec3::ZERO,
            rotation: Quat::IDENTITY,
        },
        radius: 1.0,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    };

    let dark = Prop::at_rest(ball.clone(), 1.0);
    assert_eq!(dark.luminosity_solar, 0.0);
    assert!(!dark.is_luminous());
    assert!(dark.is_valid());

    let star = Prop::at_rest(ball.clone(), 1.0).with_luminosity(1.0);
    assert!(star.is_luminous());
    assert!(star.is_valid());

    // Zero is legal and means dark; negative and non-finite are not.
    assert!(Prop::at_rest(ball.clone(), 1.0)
        .with_luminosity(0.0)
        .is_valid());
    assert!(!Prop::at_rest(ball.clone(), 1.0)
        .with_luminosity(-1.0)
        .is_valid());
    assert!(!Prop::at_rest(ball, 1.0)
        .with_luminosity(f32::NAN)
        .is_valid());
}

// ---------------------------------------------------------------------------
// `Scene::step_steered`: a controller that acts on the scene before each substep.
// ---------------------------------------------------------------------------

/// The solar system, where gravity actually moves things within a step.
fn solar_system_scene() -> Scene {
    crate::experiments::solar_system::solar_system().scene
}

#[test]
fn step_steered_with_a_passive_controller_matches_step_observed() {
    let step = seconds(3_600.0);
    let mut observed = solar_system_scene();
    let mut steered = observed.clone();
    let mut observed_substeps = 0;
    let mut steered_substeps = 0;
    observed.step_observed(step, |_| observed_substeps += 1);
    steered.step_steered(
        step,
        &mut steered_substeps,
        |_, _, _| {},
        |count, _| *count += 1,
    );
    assert_eq!(observed_substeps, steered_substeps);
    assert_eq!(observed, steered);
}

#[test]
fn step_steered_calls_the_controller_before_every_substep_with_its_length() {
    let step = seconds(3_600.0);
    let mut scene = solar_system_scene();
    let expected = crate::collision::resolve::substep_count(&scene.props, step);
    let mut lengths: Vec<f64> = Vec::new();
    scene.step_steered(
        step,
        &mut lengths,
        |lengths, _, substep| lengths.push(substep.seconds()),
        |_, _| {},
    );
    assert_eq!(lengths.len(), expected as usize);
    let total: f64 = lengths.iter().sum();
    assert!((total - 3_600.0).abs() < 1.0e-9, "integrated {total} s");
}

#[test]
fn step_steered_applies_the_controllers_kick_before_gravity() {
    // A velocity set by the controller moves the prop within the same substep.
    let mut scene = solar_system_scene();
    let (id, _) = scene.iter().find(|(_, p)| !p.immovable).unwrap();
    let start = scene.get(id).unwrap().body.center();
    let kick = DVec3::new(1.0e-3, 0.0, 0.0);
    let mut first = true;
    scene.step_steered(
        seconds(1.0),
        &mut first,
        |first, scene, _| {
            if std::mem::take(first) {
                let index = scene.iter().position(|(other, _)| other == id).unwrap();
                scene.props[index].velocity_units_s += kick;
            }
        },
        |_, _| {},
    );
    let moved = scene.get(id).unwrap().body.center() - start;
    assert!(moved.x > 0.5e-3, "moved {moved:?}");
}

#[test]
fn step_steered_does_nothing_for_a_zero_step() {
    let mut scene = solar_system_scene();
    let before = scene.clone();
    let mut calls = 0;
    scene.step_steered(
        TimeStep::ZERO,
        &mut calls,
        |calls, _, _| *calls += 1,
        |_, _| {},
    );
    assert_eq!(calls, 0);
    assert_eq!(scene, before);
}
