//! Unit and property tests for the [`crate::collision::tessellate`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/collision/tessellate.rs` via `#[path]`, not as a standalone integration
//! test.
//!
//! Every hull this module builds is a precondition of the narrow phase, so the
//! tests assert those preconditions directly for every `Body` variant: **valid,
//! closed, convex-consistent, and wound outward**. The rest pin the numbers the
//! docs quote — the icosphere's inradius ratio, the slab's placement under its
//! quad — so a doc claim cannot rot into a lie.

use super::*;
use crate::collision::trimesh::TriMesh;
use crate::core::scene::{Color, DVec3, Pose};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/collision/proptest-regressions/tessellate_tests.txt";

fn white() -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    }
}

fn pose_at(position: Vec3) -> Pose {
    Pose {
        position: position.as_dvec3(),
        rotation: Quat::IDENTITY,
    }
}

fn sphere(centre: Vec3, radius: f32) -> Body {
    Body::Sphere {
        pose: pose_at(centre),
        radius,
        color: white(),
    }
}

fn cuboid(centre: Vec3, half_extents: Vec3) -> Body {
    Body::Cuboid {
        pose: pose_at(centre),
        half_extents,
        color: white(),
    }
}

fn plane(centre: Vec3, half_size: f32) -> Body {
    Body::Plane {
        pose: pose_at(centre),
        half_size,
        color: white(),
    }
}

/// Every check the narrow phase relies on, in one place: valid, closed, and every
/// face wound so that `interior` is on its inner side.
fn assert_is_a_usable_hull(mesh: &TriMesh, interior: Vec3) {
    assert!(mesh.is_valid(), "hull fails Validation_Rules");
    assert!(mesh.is_closed(), "hull is not a closed surface");
    assert!(
        mesh.contains_point(interior),
        "hull does not contain the interior point {interior:?}"
    );

    for face in mesh.faces() {
        let distance = face
            .signed_distance(interior)
            .expect("no face of a hull is degenerate");
        assert!(
            distance < 0.0,
            "a face is wound inward: interior point is {distance} outside it"
        );
    }
}

#[test]
fn a_sphere_hull_is_a_usable_closed_icosphere() {
    let centre = Vec3::new(3.0, -2.0, 1.0);
    let mesh = collision_mesh(&sphere(centre, 2.0)).expect("a sphere always has a hull");

    assert_is_a_usable_hull(&mesh, centre);
    // Level 1: 20 · 4 triangles, 42 shared vertices, 120 unique edges.
    assert_eq!(mesh.triangles.len(), 80);
    assert_eq!(mesh.vertices.len(), 42);
    assert_eq!(mesh.edges().len(), 120);
}

#[test]
fn sphere_hull_vertices_sit_exactly_on_the_sphere() {
    let centre = Vec3::new(-4.0, 5.0, 0.5);
    let radius = 1.5;
    let mesh = collision_mesh(&sphere(centre, radius)).unwrap();

    for &vertex in &mesh.vertices {
        let distance = (vertex - centre).length();
        assert!(
            (distance - radius).abs() < 1.0e-4,
            "vertex at {distance} from the centre, expected {radius}"
        );
    }
}

#[test]
fn the_sphere_hull_undershoots_by_the_ratio_the_docs_quote() {
    // The claim in `SPHERE_SUBDIVISIONS`: at level 1 the face centres sit at
    // 0.9342 of the radius, which is 0.099 units of undershoot on a 1.5-unit ball.
    // Measured as the smallest distance from the centre to any face plane.
    let radius = 1.5;
    let mesh = collision_mesh(&sphere(Vec3::ZERO, radius)).unwrap();

    let inradius = mesh
        .faces()
        .filter_map(|face| face.signed_distance(Vec3::ZERO))
        .map(f32::abs)
        .fold(f32::INFINITY, f32::min);

    let ratio = inradius / radius;
    assert!(
        (ratio - 0.9342).abs() < 1.0e-3,
        "inradius ratio is {ratio}, docs claim 0.9342"
    );
    assert!(
        (radius - inradius - 0.099).abs() < 2.0e-3,
        "undershoot is {}, docs claim 0.099 units",
        radius - inradius
    );
}

#[test]
fn a_cuboid_hull_is_the_box_itself() {
    let centre = Vec3::new(1.0, 2.0, 3.0);
    let half_extents = Vec3::new(1.0, 2.0, 0.5);
    let mesh = collision_mesh(&cuboid(centre, half_extents)).expect("a cuboid always has a hull");

    assert_is_a_usable_hull(&mesh, centre);
    assert_eq!(mesh.triangles.len(), 12, "two triangles per face");
    assert_eq!(mesh.vertices.len(), 8);

    let aabb = mesh.aabb().unwrap();
    assert!((aabb.min - (centre - half_extents)).length() < 1.0e-5);
    assert!((aabb.max - (centre + half_extents)).length() < 1.0e-5);
}

#[test]
fn a_rotated_cuboid_hull_follows_its_pose() {
    // A quarter turn about Y swaps the X and Z extents.
    let body = Body::Cuboid {
        pose: Pose {
            position: DVec3::ZERO,
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        },
        half_extents: Vec3::new(2.0, 1.0, 0.5),
        color: white(),
    };
    let mesh = collision_mesh(&body).unwrap();
    let aabb = mesh.aabb().unwrap();

    assert!(
        (aabb.max.x - 0.5).abs() < 1.0e-4,
        "x half-extent is now 0.5"
    );
    assert!(
        (aabb.max.z - 2.0).abs() < 1.0e-4,
        "z half-extent is now 2.0"
    );
    assert!(
        (aabb.max.y - 1.0).abs() < 1.0e-4,
        "y is untouched by a Y rotation"
    );
    assert_is_a_usable_hull(&mesh, Vec3::ZERO);
}

#[test]
fn the_plane_slab_hangs_below_its_visible_quad() {
    let surface_height = 0.0;
    let mesh = collision_mesh(&plane(Vec3::new(0.0, surface_height, 0.0), 25.0))
        .expect("a plane always has a hull");
    let aabb = mesh.aabb().unwrap();

    assert!(
        (aabb.max.y - surface_height).abs() < 1.0e-5,
        "the top of the slab is the surface bodies land on, got {}",
        aabb.max.y
    );
    assert!(
        (aabb.min.y - (surface_height - PLANE_THICKNESS_UNITS)).abs() < 1.0e-5,
        "the solid extends one thickness downward, got {}",
        aabb.min.y
    );
    // Footprint is the quad's, unchanged by the thickening.
    assert!((aabb.max.x - 25.0).abs() < 1.0e-4);
    assert!((aabb.min.z + 25.0).abs() < 1.0e-4);

    assert_is_a_usable_hull(&mesh, Vec3::new(0.0, -0.5 * PLANE_THICKNESS_UNITS, 0.0));
}

#[test]
fn a_point_just_above_the_floor_is_outside_it_and_just_below_is_inside() {
    let mesh = collision_mesh(&plane(Vec3::ZERO, 10.0)).unwrap();

    assert!(!mesh.contains_point(Vec3::new(0.0, 0.01, 0.0)));
    assert!(mesh.contains_point(Vec3::new(0.0, -0.01, 0.0)));
    // Outside the footprint, at any height.
    assert!(!mesh.contains_point(Vec3::new(11.0, -0.5, 0.0)));
}

#[test]
fn a_line_with_width_is_a_solid_rod_along_the_segment() {
    let from = Vec3::new(-2.0, 1.0, 0.0);
    let to = Vec3::new(2.0, 1.0, 0.0);
    let body = Body::Line {
        from: from.as_dvec3(),
        to: to.as_dvec3(),
        width: 0.5,
        color: white(),
    };

    let mesh = collision_mesh(&body).expect("a rod bounds a volume");
    assert_is_a_usable_hull(&mesh, 0.5 * (from + to));
    assert_eq!(mesh.triangles.len(), 12);

    let aabb = mesh.aabb().unwrap();
    assert!(
        (aabb.max.x - 2.0).abs() < 1.0e-4,
        "the rod spans the segment"
    );
    assert!(
        (aabb.max.y - 1.25).abs() < 1.0e-4,
        "half a width above the axis"
    );
    // The endpoints are on the rod's end caps, so they count as contained.
    assert!(mesh.contains_point(from));
    assert!(mesh.contains_point(to));
}

#[test]
fn a_zero_width_line_has_no_hull_because_a_curve_has_no_inside() {
    let curve = Body::Line {
        from: DVec3::ZERO,
        to: DVec3::X,
        width: 0.0,
        color: white(),
    };

    assert!(
        curve.is_valid(),
        "zero width is a valid Body — see core/README.md"
    );
    assert!(
        collision_mesh(&curve).is_none(),
        "but it bounds no volume, so it has no physical border"
    );
}

#[test]
fn a_degenerate_line_has_no_hull() {
    let point = Body::Line {
        from: DVec3::ZERO,
        to: DVec3::ZERO,
        width: 0.5,
        color: white(),
    };

    assert!(collision_mesh(&point).is_none());
}

#[test]
fn an_invalid_body_has_no_hull() {
    let nan_sphere = sphere(Vec3::new(f32::NAN, 0.0, 0.0), 1.0);
    assert!(!nan_sphere.is_valid());
    assert!(collision_mesh(&nan_sphere).is_none());

    let zero_radius = sphere(Vec3::ZERO, 0.0);
    assert!(!zero_radius.is_valid());
    assert!(collision_mesh(&zero_radius).is_none());
}

/// Positions well inside the scene's bounds.
fn position() -> impl Strategy<Value = Vec3> {
    (-30.0_f32..30.0, -30.0_f32..30.0, -30.0_f32..30.0).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Whatever a sphere's position and radius, its hull is usable: closed, valid,
    /// and wound outward. This is the property the whole narrow phase assumes.
    #[test]
    fn every_sphere_hull_is_usable(centre in position(), radius in 0.01_f32..20.0) {
        let mesh = collision_mesh(&sphere(centre, radius)).expect("valid sphere");

        prop_assert!(mesh.is_valid());
        prop_assert!(mesh.is_closed());
        prop_assert!(mesh.contains_point(centre));
    }

    /// The same for cuboids, over every combination of half-extents.
    #[test]
    fn every_cuboid_hull_is_usable(
        centre in position(),
        x in 0.01_f32..10.0,
        y in 0.01_f32..10.0,
        z in 0.01_f32..10.0,
    ) {
        let mesh = collision_mesh(&cuboid(centre, Vec3::new(x, y, z))).expect("valid cuboid");

        prop_assert!(mesh.is_valid());
        prop_assert!(mesh.is_closed());
        prop_assert!(mesh.contains_point(centre));
    }

    /// A rod in any direction is a usable hull, and it contains its own axis.
    #[test]
    fn every_rod_hull_contains_its_axis(from in position(), to in position(), width in 0.01_f32..2.0) {
        prop_assume!((to - from).length() > 0.1);

        let body = Body::Line { from: from.as_dvec3(), to: to.as_dvec3(), width, color: white() };
        let mesh = collision_mesh(&body).expect("a positive-width segment bounds a volume");

        prop_assert!(mesh.is_valid());
        prop_assert!(mesh.is_closed());
        prop_assert!(mesh.contains_point(0.5 * (from + to)));
        prop_assert!(mesh.contains_point(from + 0.25 * (to - from)));
    }

    /// A hull is inscribed, never circumscribed: no vertex is further from the
    /// centre than the radius. This is the direction of the approximation the
    /// module docs commit to — undershoot, never overshoot, because a contact in
    /// empty space is the worse failure.
    #[test]
    fn a_sphere_hull_never_reaches_past_its_radius(radius in 0.1_f32..10.0) {
        let mesh = collision_mesh(&sphere(Vec3::ZERO, radius)).unwrap();

        for &vertex in &mesh.vertices {
            prop_assert!(vertex.length() <= radius * (1.0 + 1.0e-5));
        }
    }
}
