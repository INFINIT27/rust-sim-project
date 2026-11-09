//! Unit and property tests for the [`crate::collision::trimesh`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/collision/trimesh.rs` via `#[path]`, not as a standalone integration
//! test, so these tests keep access to the module's private items and to
//! `super::*`.
//!
//! The subject here is the *representation*, so the tests are about the one thing
//! the representation is for: whether the mesh knows its inside from its outside.
//! Winding, closedness, and `contains_point` carry most of the file. Meshes are
//! built by hand rather than through `tessellate`, so a tessellation bug cannot
//! make these pass.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. Pinned explicitly
/// because `file!()` contains `../..` for a `#[path]`-included module — see
/// `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/collision/proptest-regressions/trimesh_tests.txt";

/// A counter-clockwise triangle in the XZ plane whose outward normal is `+Y`.
fn upward_triangle() -> Triangle {
    Triangle::new(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 0.0),
    )
}

/// A unit tetrahedron with its four vertices at alternating cube corners,
/// centred on the origin, wound outward.
///
/// Built by hand — four triangles is the smallest mesh that encloses a volume,
/// which makes it the smallest thing that can have an inside to test.
fn tetrahedron() -> TriMesh {
    let vertices = vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
    ];
    let triangles = vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]];
    let mut mesh = TriMesh::new(vertices, triangles);
    mesh.orient_faces_outward(Vec3::ZERO);
    mesh
}

#[test]
fn a_counter_clockwise_triangle_faces_the_way_you_wound_it() {
    assert_eq!(upward_triangle().normal(), Some(Vec3::Y));
}

#[test]
fn swapping_two_vertices_flips_the_outside() {
    let triangle = upward_triangle();
    let flipped = Triangle::new(triangle.a, triangle.c, triangle.b);

    assert_eq!(flipped.normal(), Some(-Vec3::Y));
    // Same points, same area — only the orientation differs.
    assert_eq!(flipped.area(), triangle.area());
}

#[test]
fn a_collinear_triangle_is_degenerate_even_with_three_distinct_vertices() {
    let collinear = Triangle::new(Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 0.0));

    assert!(!collinear.is_valid());
    assert_eq!(collinear.normal(), None);
    assert_eq!(collinear.area(), 0.0);
    assert_eq!(collinear.signed_distance(Vec3::Y), None);
}

#[test]
fn a_triangle_with_a_non_finite_vertex_is_invalid() {
    let bad = Triangle::new(Vec3::ZERO, Vec3::new(f32::NAN, 0.0, 0.0), Vec3::Z);
    assert!(!bad.is_valid());
}

#[test]
fn area_is_half_the_cross_product() {
    // Legs of 1 and 1: area 0.5.
    assert_eq!(upward_triangle().area(), 0.5);
}

#[test]
fn signed_distance_is_positive_on_the_outward_side() {
    let triangle = upward_triangle();

    assert_eq!(
        triangle.signed_distance(Vec3::new(0.0, 2.0, 0.0)),
        Some(2.0)
    );
    assert_eq!(
        triangle.signed_distance(Vec3::new(0.0, -2.0, 0.0)),
        Some(-2.0)
    );
    // A point in the plane is at zero distance, whatever its lateral offset.
    assert_eq!(
        triangle.signed_distance(Vec3::new(9.0, 0.0, 9.0)),
        Some(0.0)
    );
}

#[test]
fn the_hand_built_tetrahedron_is_a_valid_closed_mesh() {
    let mesh = tetrahedron();

    assert!(mesh.is_valid());
    assert!(mesh.is_closed());
    assert_eq!(mesh.triangles.len(), 4);
    assert_eq!(mesh.vertices.len(), 4);
    // Four vertices, six edges — a complete graph on four nodes.
    assert_eq!(mesh.edges().len(), 6);
}

#[test]
fn every_face_of_an_oriented_mesh_points_away_from_the_interior() {
    let mesh = tetrahedron();

    for face in mesh.faces() {
        let distance = face
            .signed_distance(Vec3::ZERO)
            .expect("no face of the tetrahedron is degenerate");
        assert!(
            distance < 0.0,
            "the centre must be on the inner side of every face, got {distance}"
        );
    }
}

#[test]
fn orient_faces_outward_fixes_a_deliberately_reversed_mesh() {
    let good = tetrahedron();

    // Reverse every triangle: a mesh that is still closed and still valid, but
    // inside out. Nothing about the vertex positions says so.
    let mut inside_out = TriMesh::new(
        good.vertices.clone(),
        good.triangles.iter().map(|&[i, j, k]| [i, k, j]).collect(),
    );
    assert!(inside_out.is_valid());
    assert!(inside_out.is_closed());
    assert!(
        !inside_out.contains_point(Vec3::ZERO),
        "an inside-out hull reports its own centre as outside — which is exactly \
         why winding cannot be left to careful index typing"
    );

    inside_out.orient_faces_outward(Vec3::ZERO);

    assert!(inside_out.contains_point(Vec3::ZERO));
    assert_eq!(inside_out.triangles, good.triangles);
}

#[test]
fn an_open_mesh_is_not_closed() {
    // The tetrahedron with one face removed: three triangles, and a hole.
    let full = tetrahedron();
    let open = TriMesh::new(
        full.vertices.clone(),
        full.triangles.iter().copied().take(3).collect(),
    );

    assert!(!open.is_closed());
    // ... and `is_valid` does *not* catch it: four triangles is its floor, and
    // this has three, so use a four-triangle open mesh to make the point.
    let doubled = TriMesh::new(
        full.vertices.clone(),
        vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [0, 1, 2]],
    );
    assert!(doubled.is_valid(), "validity is not closedness");
    assert!(!doubled.is_closed());
}

#[test]
fn a_mesh_with_too_few_triangles_or_a_bad_index_is_invalid() {
    let mesh = tetrahedron();

    let three_faces = TriMesh::new(
        mesh.vertices.clone(),
        mesh.triangles.iter().copied().take(3).collect(),
    );
    assert!(
        !three_faces.is_valid(),
        "fewer than four triangles cannot enclose a volume"
    );

    let bad_index = TriMesh::new(
        mesh.vertices.clone(),
        vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [0, 1, 9]],
    );
    assert!(!bad_index.is_valid());
    assert_eq!(bad_index.triangle(&[0, 1, 9]), None);
}

#[test]
fn contains_point_separates_inside_from_outside() {
    let mesh = tetrahedron();

    assert!(mesh.contains_point(Vec3::ZERO));
    assert!(!mesh.contains_point(Vec3::new(5.0, 0.0, 0.0)));
    // Every vertex is on the surface, and touching counts as contained.
    for &vertex in &mesh.vertices {
        assert!(
            mesh.contains_point(vertex),
            "vertex {vertex:?} reads as outside"
        );
    }
    // A non-finite point is never inside anything.
    assert!(!mesh.contains_point(Vec3::new(f32::NAN, 0.0, 0.0)));
}

#[test]
fn the_bounding_box_covers_every_vertex() {
    let mesh = tetrahedron();
    let aabb = mesh.aabb().expect("the tetrahedron has vertices");

    assert!(aabb.is_valid());
    assert_eq!(aabb.min, Vec3::splat(-1.0));
    assert_eq!(aabb.max, Vec3::splat(1.0));
}

#[test]
fn an_empty_mesh_has_no_box_no_centroid_and_no_projection() {
    let empty = TriMesh::new(Vec::new(), Vec::new());

    assert_eq!(empty.aabb(), None);
    assert_eq!(empty.centroid(), None);
    assert_eq!(empty.project_onto(Vec3::X), None);
    assert_eq!(empty.support_point(Vec3::X), None);
    assert!(!empty.is_closed());
    assert!(!empty.is_valid());
}

#[test]
fn translation_moves_the_hull_without_reshaping_it() {
    let mesh = tetrahedron();
    let mut moved = mesh.clone();
    moved.translate(Vec3::new(10.0, 0.0, 0.0));

    assert_eq!(moved.triangles, mesh.triangles, "topology is unchanged");
    assert!(moved.contains_point(Vec3::new(10.0, 0.0, 0.0)));
    assert!(!moved.contains_point(Vec3::ZERO));

    // Winding survives, which is what makes translation safe to apply to a hull
    // after a positional correction instead of re-tessellating.
    for face in moved.faces() {
        assert!(face
            .signed_distance(Vec3::new(10.0, 0.0, 0.0))
            .is_some_and(|distance| distance < 0.0));
    }
}

/// A `Vec3` strategy over a modest, exactly-representable range.
fn vector() -> impl Strategy<Value = Vec3> {
    (-20.0_f32..20.0, -20.0_f32..20.0, -20.0_f32..20.0).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// The projection interval is exactly the range of the vertex projections —
    /// the property the separating-axis test rests on.
    #[test]
    fn projection_brackets_every_vertex(axis in vector()) {
        let mesh = tetrahedron();
        prop_assume!(axis.length() > 1.0e-3);

        let (min, max) = mesh.project_onto(axis).expect("non-empty mesh");
        prop_assert!(min <= max);

        for &vertex in &mesh.vertices {
            let value = vertex.dot(axis);
            prop_assert!(value >= min - 1.0e-4 && value <= max + 1.0e-4);
        }
    }

    /// The support point along an axis attains the projection's maximum. This is
    /// what lets a contact point be placed on the surface rather than guessed.
    #[test]
    fn the_support_point_attains_the_maximum(axis in vector()) {
        let mesh = tetrahedron();
        prop_assume!(axis.length() > 1.0e-3);

        let (_, max) = mesh.project_onto(axis).expect("non-empty mesh");
        let support = mesh.support_point(axis).expect("non-empty mesh");

        prop_assert!((support.dot(axis) - max).abs() <= 1.0e-4);
    }

    /// Orienting is idempotent: a mesh already wound outward is left exactly as
    /// it was, so running the fix twice cannot flip a face back.
    #[test]
    fn orienting_an_oriented_mesh_changes_nothing(offset in vector()) {
        let mut mesh = tetrahedron();
        mesh.translate(offset);
        let before = mesh.clone();

        mesh.orient_faces_outward(offset);

        prop_assert_eq!(mesh.triangles, before.triangles);
    }
}
