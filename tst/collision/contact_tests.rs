//! Unit and property tests for the [`crate::collision::contact`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/collision/contact.rs` via `#[path]`, not as a standalone integration
//! test.
//!
//! Two things are being tested, and they are not the same thing: **whether** two
//! hulls are found to overlap, and **what** the reported normal and depth are. The
//! first has exact answers for axis-aligned boxes, so the depth assertions that
//! have to be exact are made on boxes; the sphere cases assert the direction and a
//! bracket on the depth, because an icosphere is not a sphere (see
//! `tessellate.rs`).

use super::*;
use crate::collision::tessellate::collision_mesh;
use crate::core::scene::{Body, Color, DVec3, Pose, Prop, Quat};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/collision/proptest-regressions/contact_tests.txt";

fn white() -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    }
}

fn ball(centre: Vec3, radius: f32) -> Body {
    Body::Sphere {
        pose: Pose {
            position: centre.as_dvec3(),
            rotation: Quat::IDENTITY,
        },
        radius,
        color: white(),
    }
}

fn box_at(centre: Vec3, half_extents: Vec3) -> Body {
    Body::Cuboid {
        pose: Pose {
            position: centre.as_dvec3(),
            rotation: Quat::IDENTITY,
        },
        half_extents,
        color: white(),
    }
}

fn floor_at(height: f32) -> Body {
    Body::Plane {
        pose: Pose {
            position: DVec3::new(0.0, f64::from(height), 0.0),
            rotation: Quat::IDENTITY,
        },
        half_size: 20.0,
        color: white(),
    }
}

fn hull(body: &Body) -> TriMesh {
    collision_mesh(body).expect("this body has a hull")
}

#[test]
fn boxes_that_do_not_touch_report_no_contact() {
    let left = hull(&box_at(Vec3::ZERO, Vec3::splat(1.0)));
    let right = hull(&box_at(Vec3::new(2.5, 0.0, 0.0), Vec3::splat(1.0)));

    assert_eq!(contact_between_meshes(&left, &right), None);
}

#[test]
fn overlapping_boxes_report_the_exact_overlap() {
    // Half-extents 1, centres 1.5 apart on X: the boxes overlap by 0.5, and the
    // shallowest axis is X because they are flush on Y and Z (overlap 2.0 there).
    let left = hull(&box_at(Vec3::ZERO, Vec3::splat(1.0)));
    let right = hull(&box_at(Vec3::new(1.5, 0.0, 0.0), Vec3::splat(1.0)));

    let contact = contact_between_meshes(&left, &right).expect("these overlap");

    assert!(contact.is_valid());
    assert!(
        (contact.normal - Vec3::X).length() < 1.0e-4,
        "the way out is +X, from the first hull toward the second, got {:?}",
        contact.normal
    );
    assert!(
        (contact.depth - 0.5).abs() < 1.0e-4,
        "depth should be the exact overlap 0.5, got {}",
        contact.depth
    );
}

#[test]
fn the_normal_reverses_when_the_arguments_do() {
    let left = hull(&box_at(Vec3::ZERO, Vec3::splat(1.0)));
    let right = hull(&box_at(Vec3::new(1.5, 0.0, 0.0), Vec3::splat(1.0)));

    let forward = contact_between_meshes(&left, &right).unwrap();
    let backward = contact_between_meshes(&right, &left).unwrap();

    assert!((forward.normal + backward.normal).length() < 1.0e-4);
    assert!((forward.depth - backward.depth).abs() < 1.0e-5);
}

#[test]
fn a_box_exactly_flush_with_another_is_not_a_contact() {
    // Touching with zero overlap: the AABB broad phase passes it through, and the
    // narrow phase declines it because a zero depth gives no direction to
    // separate along. `Contact::is_valid` demands a strictly positive depth.
    let left = hull(&box_at(Vec3::ZERO, Vec3::splat(1.0)));
    let right = hull(&box_at(Vec3::new(2.0, 0.0, 0.0), Vec3::splat(1.0)));

    assert_eq!(contact_between_meshes(&left, &right), None);
}

#[test]
fn spheres_touch_when_their_hulls_meet_and_not_before() {
    let radius = 1.0;
    let left = hull(&ball(Vec3::ZERO, radius));

    // Well clear.
    let far = hull(&ball(Vec3::new(3.0, 0.0, 0.0), radius));
    assert_eq!(contact_between_meshes(&left, &far), None);

    // Deeply overlapping: normal along +X, depth bounded by the diameter.
    let near = hull(&ball(Vec3::new(1.5, 0.0, 0.0), radius));
    let contact = contact_between_meshes(&left, &near).expect("these overlap");
    assert!(contact.is_valid());
    assert!(contact.normal.x > 0.99, "normal is {:?}", contact.normal);
    assert!(contact.depth > 0.0 && contact.depth < 2.0 * radius);

    // A graze at almost exactly the sum of the radii. The undershoot documented in
    // `tessellate.rs` is a *face-centre* figure and this is not a face-centre
    // direction: subdivision puts a vertex at exactly `+X`, so along this axis the
    // hull reaches its full radius and a 0.02-unit overlap is really there. The
    // contact is therefore reported, and its depth is that overlap — which is the
    // useful thing to pin, because it says the depth tracks real geometry rather
    // than a fudge.
    let grazing = hull(&ball(Vec3::new(1.98, 0.0, 0.0), radius));
    let graze = contact_between_meshes(&left, &grazing).expect("a vertex-on-vertex graze");
    assert!(
        graze.depth <= 0.03,
        "a graze must be shallow, got {}",
        graze.depth
    );
}

#[test]
fn a_sphere_sinking_into_the_floor_is_pushed_straight_up() {
    // Centre 0.9 above a floor at y = 0, radius 1.0: the ball's underside is 0.1
    // below the surface.
    let ball_hull = hull(&ball(Vec3::new(0.0, 0.9, 0.0), 1.0));
    let floor_hull = hull(&floor_at(0.0));

    let contact = contact_between_meshes(&ball_hull, &floor_hull).expect("these overlap");

    assert!(contact.is_valid());
    assert!(
        contact.normal.y < -0.99,
        "the normal points from the ball toward the floor, i.e. down, got {:?}",
        contact.normal
    );
    // The ball is pushed along -normal, which is up.
    assert!(contact.depth > 0.0);
    assert!(contact.depth < 1.0, "a shallow dip, got {}", contact.depth);
}

#[test]
fn a_sphere_resting_clear_of_the_floor_reports_nothing() {
    let ball_hull = hull(&ball(Vec3::new(0.0, 2.0, 0.0), 1.0));
    let floor_hull = hull(&floor_at(0.0));

    assert_eq!(contact_between_meshes(&ball_hull, &floor_hull), None);
}

#[test]
fn a_sphere_beyond_the_floors_edge_falls_past_it() {
    // The floor is a 40-unit square; this ball is outside the footprint, level
    // with the surface. A radius test against an infinite plane would report a
    // contact here; the hull does not, because the hull is the quad.
    let ball_hull = hull(&ball(Vec3::new(25.0, 0.0, 0.0), 1.0));
    let floor_hull = hull(&floor_at(0.0));

    assert_eq!(contact_between_meshes(&ball_hull, &floor_hull), None);
}

#[test]
fn the_contact_point_lies_between_the_two_hulls() {
    let left = hull(&box_at(Vec3::ZERO, Vec3::splat(1.0)));
    let right = hull(&box_at(Vec3::new(1.5, 0.0, 0.0), Vec3::splat(1.0)));

    let contact = contact_between_meshes(&left, &right).unwrap();

    // Between the two centres, and inside the overlap slab on X.
    assert!(contact.point.x > 0.0 && contact.point.x < 1.5);
}

#[test]
fn aabbs_overlap_counts_touching_as_overlapping() {
    let unit = Aabb {
        min: Vec3::ZERO,
        max: Vec3::splat(1.0),
    };
    let flush = Aabb {
        min: Vec3::new(1.0, 0.0, 0.0),
        max: Vec3::new(2.0, 1.0, 1.0),
    };
    let clear = Aabb {
        min: Vec3::new(1.001, 0.0, 0.0),
        max: Vec3::new(2.0, 1.0, 1.0),
    };

    assert!(aabbs_overlap(&unit, &unit));
    assert!(
        aabbs_overlap(&unit, &flush),
        "the broad phase must pass a flush pair to the narrow phase to rule on"
    );
    assert!(!aabbs_overlap(&unit, &clear));
    // Symmetric.
    assert!(aabbs_overlap(&flush, &unit));
}

#[test]
fn contact_between_props_agrees_with_the_mesh_form() {
    let a = Prop::at_rest(ball(Vec3::ZERO, 1.0), 1.0);
    let b = Prop::at_rest(ball(Vec3::new(1.2, 0.0, 0.0), 1.0), 1.0);

    let via_props = contact_between_props(&a, &b).expect("these overlap");
    let via_meshes = contact_between_meshes(&hull(&a.body), &hull(&b.body)).unwrap();

    assert_eq!(via_props, via_meshes);
}

#[test]
fn a_prop_without_a_hull_never_reports_a_contact() {
    // A zero-width line has no physical border, so it passes through everything.
    let curve = Prop::at_rest(
        Body::Line {
            from: DVec3::new(-1.0, 0.0, 0.0),
            to: DVec3::new(1.0, 0.0, 0.0),
            width: 0.0,
            color: white(),
        },
        1.0,
    );
    let solid = Prop::at_rest(ball(Vec3::ZERO, 1.0), 1.0);

    assert_eq!(contact_between_props(&curve, &solid), None);
    assert_eq!(contact_between_props(&solid, &curve), None);
}

#[test]
fn an_empty_mesh_reports_no_contact() {
    let empty = TriMesh::new(Vec::new(), Vec::new());
    let solid = hull(&ball(Vec3::ZERO, 1.0));

    assert_eq!(contact_between_meshes(&empty, &solid), None);
    assert_eq!(contact_between_meshes(&solid, &empty), None);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Two spheres whose centres are further apart than the sum of their radii
    /// cannot touch: the hulls are inscribed, so they are strictly inside the
    /// spheres they approximate. No separation distance, however small, may
    /// produce a contact.
    #[test]
    fn separated_spheres_never_report_a_contact(
        separation in 2.01_f32..20.0,
        radius in 0.2_f32..1.0,
    ) {
        let left = hull(&ball(Vec3::ZERO, radius));
        let right = hull(&ball(Vec3::new(separation * radius, 0.0, 0.0), radius));

        prop_assert_eq!(contact_between_meshes(&left, &right), None);
    }

    /// Concentric-ish spheres always report a contact, always with a valid
    /// normal, and always with a depth no larger than the diameter — the range a
    /// minimum translation vector can occupy.
    #[test]
    fn deeply_overlapping_spheres_report_a_bounded_depth(
        offset in 0.05_f32..1.5,
        radius in 0.5_f32..3.0,
    ) {
        let left = hull(&ball(Vec3::ZERO, radius));
        let right = hull(&ball(Vec3::new(offset * radius, 0.0, 0.0), radius));

        let contact = contact_between_meshes(&left, &right).expect("overlapping");
        prop_assert!(contact.is_valid());
        prop_assert!(contact.depth <= 2.0 * radius + 1.0e-3);
        // The normal points from the first hull toward the second. Its *direction*
        // is only pinned to `+X` when the pair is well separated along X: for
        // nearly-concentric hulls the shallowest axis can be almost any face
        // normal, and asserting `x > 0.9` there would be asserting an accident.
        prop_assert!(contact.normal.dot(Vec3::X) > 0.0);
    }

    /// Axis-aligned boxes overlapping on one axis only report that axis and that
    /// exact depth. Boxes are where the depth can be asserted exactly, so this is
    /// the property that pins the separating-axis arithmetic itself.
    #[test]
    fn box_overlap_depth_is_exact(overlap in 0.01_f32..0.9) {
        let half = 1.0_f32;
        let left = hull(&box_at(Vec3::ZERO, Vec3::splat(half)));
        let right = hull(&box_at(Vec3::new(2.0 * half - overlap, 0.0, 0.0), Vec3::splat(half)));

        let contact = contact_between_meshes(&left, &right).expect("overlapping");
        prop_assert!((contact.depth - overlap).abs() < 1.0e-4);
        prop_assert!((contact.normal - Vec3::X).length() < 1.0e-4);
    }

    /// A contact is unchanged by translating both hulls together: the geometry is
    /// relative, so a pair 40 units from the origin behaves like a pair at it.
    #[test]
    fn contact_is_invariant_under_a_shared_translation(
        shift_x in -30.0_f32..30.0,
        shift_y in -30.0_f32..30.0,
    ) {
        let shift = Vec3::new(shift_x, shift_y, 0.0);

        let at_origin = contact_between_meshes(
            &hull(&box_at(Vec3::ZERO, Vec3::splat(1.0))),
            &hull(&box_at(Vec3::new(1.5, 0.0, 0.0), Vec3::splat(1.0))),
        ).unwrap();

        let shifted = contact_between_meshes(
            &hull(&box_at(shift, Vec3::splat(1.0))),
            &hull(&box_at(shift + Vec3::new(1.5, 0.0, 0.0), Vec3::splat(1.0))),
        ).unwrap();

        prop_assert!((shifted.normal - at_origin.normal).length() < 1.0e-3);
        prop_assert!((shifted.depth - at_origin.depth).abs() < 1.0e-3);
    }
}
