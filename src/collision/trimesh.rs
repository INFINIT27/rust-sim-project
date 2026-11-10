//! Triangles and triangle meshes: the geometry that says where a body ends.
//!
//! A [`Triangle`] is three points with an orientation, and a [`TriMesh`] is an
//! indexed set of them. Together they are how this crate answers the question
//! [`Body`](crate::core::scene::Body) alone cannot: **which points are inside a
//! prop and which are outside**. A `Body::Sphere` says "radius 1.5"; a `TriMesh`
//! says "here is the surface, and here is which side of it is solid".
//!
//! # The winding rule is the whole contract
//!
//! Every triangle here is **counter-clockwise when seen from outside the body**,
//! so its normal `(b − a) × (c − a)` points *out* of the solid. Nothing in this
//! module can recover the inside from the geometry alone — a triangle with
//! reversed winding is a perfectly good triangle whose normal points the wrong
//! way — so the winding *is* the encoding of inner versus outer, and every
//! function that talks about "inside", "outside", or "penetration" depends on
//! it. [`TriMesh::orient_faces_outward`] is the one place a mesh's winding gets
//! fixed, and every constructor in [`super::tessellate`] ends with a call to it.
//!
//! # Convexity is assumed where it is documented
//!
//! [`TriMesh::contains_point`] and the separating-axis test in
//! [`super::contact`] are only correct for **convex** meshes. Every mesh
//! [`super::tessellate`] produces is convex, which is what makes the cheap
//! face-plane test valid; a concave mesh needs a ray-crossing or winding-number
//! test instead. The precondition is stated on each affected function rather
//! than assumed silently.
//!
//! # Units and precision
//!
//! World units and `f32`, matching [`crate::core::scene`] — this is scene
//! geometry, not SI physics, so it stays on the `f32` side of the boundary that
//! [`crate::physics_utils`] sits on the other side of.
//!
//! # Allocation
//!
//! Unlike `physics_utils`, this module allocates: a mesh owns two `Vec`s, and
//! [`TriMesh::edges`] and [`TriMesh::is_closed`] build temporaries. That is the
//! price of a boundary representation, and it is why meshes are built once per
//! collision pass rather than per pair (see [`super::resolve`]).

use std::collections::HashMap;

pub use bevy::math::Vec3;

use crate::collision::constants::SURFACE_EPSILON_UNITS;
use crate::core::scene::Aabb;

/// A triangle with an orientation: counter-clockwise seen from **outside** the
/// body it bounds.
///
/// The vertex order carries the orientation, so `Triangle { a, b, c }` and
/// `Triangle { a, c, b }` are different triangles with opposite normals even
/// though they cover the same points. See the module docs — this is the encoding
/// of inner versus outer.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{Triangle, Vec3};
///
/// // Counter-clockwise in the XZ plane seen from +Y, so the normal is +Y.
/// let t = Triangle {
///     a: Vec3::new(0.0, 0.0, 0.0),
///     b: Vec3::new(0.0, 0.0, 1.0),
///     c: Vec3::new(1.0, 0.0, 0.0),
/// };
///
/// assert_eq!(t.normal(), Some(Vec3::Y));
///
/// // Reversing two vertices flips the outside.
/// let flipped = Triangle { a: t.a, b: t.c, c: t.b };
/// assert_eq!(flipped.normal(), Some(-Vec3::Y));
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

impl Triangle {
    /// A triangle from its three vertices, in outward-facing winding order.
    #[inline]
    pub const fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { a, b, c }
    }

    /// Validation_Rules for `Triangle`: every vertex finite, and the triangle
    /// non-degenerate — its two edge vectors are not parallel, so it spans an
    /// actual plane.
    ///
    /// Degeneracy is tested through the cross product rather than by comparing
    /// vertices, because three *distinct* collinear points are degenerate too and
    /// a vertex comparison would miss them.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.a.is_finite()
            && self.b.is_finite()
            && self.c.is_finite()
            && self.raw_normal().length_squared() > 0.0
    }

    /// The unnormalized outward normal `(b − a) × (c − a)`.
    ///
    /// Its magnitude is twice the triangle's area, which is why
    /// [`area`](Self::area) and [`normal`](Self::normal) both start here.
    #[inline]
    pub fn raw_normal(&self) -> Vec3 {
        (self.b - self.a).cross(self.c - self.a)
    }

    /// The unit outward normal, or `None` for a degenerate triangle.
    ///
    /// `None` rather than a zero vector: a triangle with no area does not face
    /// any direction, and normalising a zero cross product yields `NaN`. Every
    /// caller here has to decide what to do about a degenerate face, so the type
    /// makes them.
    #[inline]
    pub fn normal(&self) -> Option<Vec3> {
        let raw = self.raw_normal();
        let length = raw.length();
        if length > 0.0 && length.is_finite() {
            Some(raw / length)
        } else {
            None
        }
    }

    /// Area in square world units. `0.0` for a degenerate triangle.
    #[inline]
    pub fn area(&self) -> f32 {
        0.5 * self.raw_normal().length()
    }

    /// The centroid: the average of the three vertices.
    #[inline]
    pub fn centroid(&self) -> Vec3 {
        (self.a + self.b + self.c) / 3.0
    }

    /// Signed distance from `point` to this triangle's **plane**, positive on the
    /// outward side. `None` when the triangle is degenerate.
    ///
    /// This is distance to the infinite plane, not to the triangle: a point far
    /// off to the side of a small triangle can still return a small distance.
    /// Only the convex face-plane tests use this, and for a convex mesh the
    /// planes are exactly what bound the solid.
    #[inline]
    pub fn signed_distance(&self, point: Vec3) -> Option<f32> {
        self.normal().map(|normal| (point - self.a).dot(normal))
    }
}

/// An indexed triangle mesh: the boundary between a body's inside and its
/// outside.
///
/// Vertices are in **world units**, in whatever frame the caller built them in —
/// [`super::tessellate`] produces world-space meshes, already posed. Each entry
/// of `triangles` is three indices into `vertices` in outward-facing
/// counter-clockwise order.
///
/// # Fields are public and validation is advisory
///
/// Matching [`crate::core::scene`]: both fields are `pub`, nothing validates on
/// assignment, and [`is_valid`](Self::is_valid) is a pure predicate. Functions
/// that need a well-formed mesh state the precondition and `debug_assert!` it.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{collision_mesh, Body, Color, Pose, Quat, DVec3, Vec3};
///
/// let cube = Body::Cuboid {
///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
///     half_extents: Vec3::splat(1.0),
///     color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
/// };
///
/// let mesh = collision_mesh(&cube).expect("a cuboid always has a hull");
///
/// assert!(mesh.is_valid());
/// assert!(mesh.is_closed());
/// assert_eq!(mesh.triangles.len(), 12); // two per face
///
/// // The centre is inside; a point well outside is not.
/// assert!(mesh.contains_point(Vec3::ZERO));
/// assert!(!mesh.contains_point(Vec3::new(2.0, 0.0, 0.0)));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct TriMesh {
    /// The mesh's vertices, in world units.
    pub vertices: Vec<Vec3>,
    /// Triangles as `[i, j, k]` indices into `vertices`, wound
    /// counter-clockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

impl TriMesh {
    /// A mesh from its vertices and index triples. No validation — see
    /// [`is_valid`](Self::is_valid).
    #[inline]
    pub fn new(vertices: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Self {
        Self {
            vertices,
            triangles,
        }
    }

    /// Validation_Rules for `TriMesh`:
    ///
    /// - at least four triangles — fewer cannot enclose a volume, so an inner
    ///   side would not exist;
    /// - every vertex finite;
    /// - every index in bounds;
    /// - every triangle non-degenerate ([`Triangle::is_valid`]).
    ///
    /// Note this does **not** check closedness or convexity: those are
    /// [`is_closed`](Self::is_closed) and a documented precondition
    /// respectively, because they are expensive and undecidable-cheaply in turn.
    pub fn is_valid(&self) -> bool {
        if self.triangles.len() < 4 {
            return false;
        }
        if !self.vertices.iter().all(|vertex| vertex.is_finite()) {
            return false;
        }
        let vertex_count = self.vertices.len() as u32;
        self.triangles.iter().all(|indices| {
            indices.iter().all(|&index| index < vertex_count)
                && self
                    .triangle(indices)
                    .is_some_and(|triangle| triangle.is_valid())
        })
    }

    /// The triangle for one index triple, or `None` if any index is out of
    /// bounds.
    #[inline]
    pub fn triangle(&self, indices: &[u32; 3]) -> Option<Triangle> {
        let [i, j, k] = *indices;
        Some(Triangle::new(
            *self.vertices.get(i as usize)?,
            *self.vertices.get(j as usize)?,
            *self.vertices.get(k as usize)?,
        ))
    }

    /// Every triangle in the mesh, in index order. Out-of-bounds triples are
    /// skipped rather than panicking, matching the crate's benign-in-release
    /// contract.
    pub fn faces(&self) -> impl Iterator<Item = Triangle> + '_ {
        self.triangles
            .iter()
            .filter_map(|indices| self.triangle(indices))
    }

    /// The mesh's axis-aligned bounding box, or `None` when it has no vertices.
    ///
    /// This is the broad phase: comparing two of these is a handful of
    /// comparisons, and it rejects the overwhelming majority of pairs before the
    /// separating-axis test in [`super::contact`] ever runs.
    pub fn aabb(&self) -> Option<Aabb> {
        let mut iter = self.vertices.iter().copied();
        let first = iter.next()?;
        let (min, max) = iter.fold((first, first), |(min, max), vertex| {
            (min.min(vertex), max.max(vertex))
        });
        Some(Aabb { min, max })
    }

    /// The average of the mesh's vertices, or `None` when it has none.
    ///
    /// The *vertex* centroid, not the volume centroid: it is used only to point
    /// a separating axis from one body toward the other, where any interior
    /// reference point does the job. For the tessellations here — a uniform
    /// icosphere, a box — the two coincide anyway.
    pub fn centroid(&self) -> Option<Vec3> {
        if self.vertices.is_empty() {
            return None;
        }
        let sum: Vec3 = self.vertices.iter().copied().sum();
        Some(sum / self.vertices.len() as f32)
    }

    /// Every unique undirected edge, as an index pair with the lower index
    /// first.
    ///
    /// Wanted by the separating-axis test, which needs each edge *direction*
    /// once: a closed mesh has each edge in two triangles, and running the
    /// edge-pair cross products twice per edge would quadruple the work for
    /// nothing.
    pub fn edges(&self) -> Vec<(u32, u32)> {
        let mut seen = HashMap::new();
        for &[i, j, k] in &self.triangles {
            for (from, to) in [(i, j), (j, k), (k, i)] {
                let key = if from <= to { (from, to) } else { (to, from) };
                seen.insert(key, ());
            }
        }
        let mut edges: Vec<(u32, u32)> = seen.into_keys().collect();
        // Sorted so the axis order — and therefore which of several equally
        // shallow separating axes wins a tie — does not depend on hash order.
        edges.sort_unstable();
        edges
    }

    /// Is this mesh a closed surface — does every edge have exactly one triangle
    /// on each side, wound in opposite directions?
    ///
    /// This is the geometric statement of "the body has an inside". An open mesh
    /// has a hole to leak through, and a mesh whose faces disagree about their
    /// winding has no consistent outside at all, so both make
    /// [`contains_point`](Self::contains_point) meaningless.
    ///
    /// Checked by counting *directed* edges: in a closed, consistently wound
    /// mesh every directed edge `(i, j)` appears exactly once and its reverse
    /// `(j, i)` exactly once.
    ///
    /// Not called on the hot path — it is a property the tessellators are tested
    /// against, not a per-frame check.
    pub fn is_closed(&self) -> bool {
        if self.triangles.is_empty() {
            return false;
        }

        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for &[i, j, k] in &self.triangles {
            for edge in [(i, j), (j, k), (k, i)] {
                *directed.entry(edge).or_insert(0) += 1;
            }
        }

        directed.iter().all(|(&(from, to), &count)| {
            count == 1 && directed.get(&(to, from)).copied() == Some(1)
        })
    }

    /// Is `point` inside the solid this mesh bounds?
    ///
    /// **Precondition: the mesh is convex, closed, and wound outward.** Every
    /// mesh from [`super::tessellate`] is. For a convex body, "inside" is
    /// exactly "on the inner side of every face plane", which is what this
    /// checks — one signed distance per face, allowing
    /// [`SURFACE_EPSILON_UNITS`] of slack so a point on the surface counts as
    /// inside.
    ///
    /// A concave mesh will report false positives (every point in the convex
    /// hull passes), which is why the precondition is a precondition and not a
    /// caveat. Degenerate faces are skipped: they bound nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{collision_mesh, Body, Color, Pose, Quat, DVec3, Vec3};
    ///
    /// let ball = Body::Sphere {
    ///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
    ///     radius: 2.0,
    ///     color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
    /// };
    /// let mesh = collision_mesh(&ball).unwrap();
    ///
    /// assert!(mesh.contains_point(Vec3::ZERO));
    /// assert!(!mesh.contains_point(Vec3::new(0.0, 3.0, 0.0)));
    /// ```
    pub fn contains_point(&self, point: Vec3) -> bool {
        if !point.is_finite() {
            return false;
        }
        self.faces().all(|face| {
            face.signed_distance(point)
                .is_none_or(|distance| distance <= SURFACE_EPSILON_UNITS)
        })
    }

    /// The mesh's extent along `axis`, as `(min, max)` of the vertex
    /// projections. `None` when the mesh has no vertices.
    ///
    /// The primitive the separating-axis test is built out of: two convex bodies
    /// are disjoint exactly when some axis's intervals do not overlap.
    /// `axis` need not be normalized — overlap is a sign test — but the
    /// *depth* the caller reads off the interval is only in world units if it
    /// is, so [`super::contact`] normalizes.
    pub fn project_onto(&self, axis: Vec3) -> Option<(f32, f32)> {
        let mut iter = self.vertices.iter().map(|vertex| vertex.dot(axis));
        let first = iter.next()?;
        Some(iter.fold((first, first), |(min, max), value| {
            (min.min(value), max.max(value))
        }))
    }

    /// The vertex furthest along `axis` — the support point of the convex hull.
    ///
    /// Used to place a contact point on the surface rather than at a centroid.
    /// Ties are broken by index order, so the result is deterministic.
    pub fn support_point(&self, axis: Vec3) -> Option<Vec3> {
        self.vertices
            .iter()
            .copied()
            .fold(None, |best: Option<(f32, Vec3)>, vertex| {
                let value = vertex.dot(axis);
                match best {
                    Some((best_value, _)) if best_value >= value => best,
                    _ => Some((value, vertex)),
                }
            })
            .map(|(_, vertex)| vertex)
    }

    /// Move every vertex by `delta`, preserving shape and winding.
    ///
    /// Lets [`super::resolve`] follow a positional correction without
    /// re-tessellating: translation cannot change a mesh's topology, its face
    /// normals, or its convexity.
    pub fn translate(&mut self, delta: Vec3) {
        for vertex in &mut self.vertices {
            *vertex += delta;
        }
    }

    /// Flip any face that is wound inward, taking `interior` as a point known to
    /// be inside the solid.
    ///
    /// **Precondition: the mesh is star-shaped about `interior`** — every face is
    /// fully visible from it, which is true of any convex mesh and of every mesh
    /// [`super::tessellate`] builds. Under that assumption a face is outward
    /// exactly when its normal points away from `interior`, so the fix is a
    /// signed-distance test and a vertex swap.
    ///
    /// This exists so the tessellators do not have to get 80 hand-written index
    /// triples' winding right by inspection. Winding is the encoding of inner
    /// versus outer (see the module docs), a silently reversed face is a hole
    /// physics leaks through, and "list the indices carefully" is not a
    /// mechanism. Degenerate faces are left alone — there is nothing to orient.
    pub fn orient_faces_outward(&mut self, interior: Vec3) {
        for indices in &mut self.triangles {
            let [i, j, k] = *indices;
            let Some(triangle) = ({
                let vertex = |index: u32| self.vertices.get(index as usize).copied();
                match (vertex(i), vertex(j), vertex(k)) {
                    (Some(a), Some(b), Some(c)) => Some(Triangle::new(a, b, c)),
                    _ => None,
                }
            }) else {
                continue;
            };

            // Negative means `interior` is on the outward side of this face,
            // i.e. the face is wound backwards. Exactly zero means the interior
            // point lies in the face plane, which contradicts the precondition;
            // leave it be rather than guess.
            if triangle
                .signed_distance(interior)
                .is_some_and(|distance| distance > 0.0)
            {
                *indices = [i, k, j];
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tst/collision/trimesh_tests.rs"]
mod tests;
