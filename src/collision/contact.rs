//! Narrow phase: do two hulls overlap, and if so, along which axis and by how
//! much?
//!
//! [`contact_between_meshes`] is the answer, as an `Option<Contact>`. It is the
//! only thing in the crate that decides two props are touching, and everything
//! [`super::resolve`] does follows from the [`Contact`] it returns.
//!
//! # The separating-axis theorem, over the triangles
//!
//! Two convex bodies are disjoint **exactly when** some axis exists on which
//! their projections do not overlap. For convex polyhedra that axis, if it
//! exists, is always one of a finite set:
//!
//! - a **face normal** of either hull, or
//! - a **cross product of one edge direction from each**.
//!
//! So the test is: build that axis set from the triangles, project both hulls
//! onto each axis ([`TriMesh::project_onto`]), and if every axis overlaps, the
//! shallowest one is the way out — its overlap is the penetration depth and its
//! direction the contact normal. That vector is the *minimum translation vector*:
//! the smallest move that separates the pair.
//!
//! This is why the hull is a triangle mesh rather than a radius. The axes come
//! from the geometry, so one code path handles sphere-on-sphere, box-on-floor,
//! and rod-on-box, and adding a `Body` variant needs a tessellation and no new
//! collision case.
//!
//! # Convexity is required, and every hull here has it
//!
//! SAT is only a decision procedure for **convex** bodies; for concave ones it
//! reports overlap of the convex hulls. Every mesh [`super::tessellate`] produces
//! is convex, and that is a precondition of this module rather than a caveat.
//!
//! # The shallowest axis alone is not a good normal, and why
//!
//! The minimum translation vector is the right *depth* but a poor *direction* on
//! a tessellated curve. A hull face is flat, so the axis set around a sphere's
//! surface is quantised to its facet normals: at
//! [`SPHERE_SUBDIVISIONS`](super::constants::SPHERE_SUBDIVISIONS) `= 1` the
//! facet nearest a given direction can be **21° away** from it. Bouncing off that
//! facet reflects the velocity about it, so two spheres meeting exactly head-on
//! would leave at 42° to the line of centres — inventing transverse motion, and
//! angular momentum, out of the tessellation. Momentum and energy would still
//! balance; the geometry would be a lie.
//!
//! So the normal is the **average of every near-shallowest axis**, not the single
//! shallowest one — see [`NORMAL_TIE_TOLERANCE`]. Where a smooth surface has been
//! faceted, the facets around the true contact direction are all nearly tied and
//! straddle it symmetrically, so their average recovers it. Where the contact is
//! genuinely flat — a box on the floor — every tied axis is the same direction and
//! the average changes nothing.
//!
//! # Where this is approximate, precisely
//!
//! Two places, both bounded and both deliberate:
//!
//! 1. **The hull is not the body.** An icosphere is inside the sphere it
//!    approximates (see [`super::tessellate`]), so round bodies touch marginally
//!    late. Contacts are computed exactly — for the hull.
//! 2. **The edge-axis budget.** The edge-pair axis set is quadratic in edge
//!    count, so past [`EDGE_AXIS_BUDGET`] pairs it is skipped and only face
//!    normals are tested. Dropping axes can only make the test *more* willing to
//!    call a pair overlapping and can only make the reported depth *larger*,
//!    never smaller — so the failure mode is an early, slightly over-strong
//!    separation, not a missed collision. See the constant for why that lands on
//!    sphere-sphere and why that is the right place for it.

pub use bevy::math::Vec3;

use crate::collision::constants::{EDGE_AXIS_BUDGET, MIN_AXIS_LENGTH, NORMAL_TIE_TOLERANCE};
use crate::collision::tessellate::contact_mesh;
use crate::collision::trimesh::TriMesh;
use crate::core::constants::ROTATION_UNIT_TOLERANCE;
use crate::core::scene::{Aabb, Prop};

/// An overlap between two hulls, and the way out of it.
///
/// Produced by [`contact_between_meshes`] and consumed by
/// [`super::resolve`]. Read `a` and `b` in the order they were passed to the
/// function that built it: **`normal` points from `a` toward `b`**, so moving `b`
/// along `+normal` (or `a` along `−normal`) by `depth` separates the pair.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Contact {
    /// Unit vector from `a` toward `b`: the direction along which the two are
    /// least deeply overlapped, and therefore the direction an elastic bounce
    /// acts along.
    ///
    /// Oriented by the two hulls' centroids rather than by which axis happened to
    /// win, because a separating axis has no inherent sign — `n` and `−n` separate
    /// equally well, and getting the sign wrong pulls colliding bodies *together*.
    pub normal: Vec3,
    /// Penetration depth in world units along `normal`: strictly positive, and
    /// the smallest translation that ends the overlap.
    pub depth: f32,
    /// A representative point of contact, in world units.
    ///
    /// The midpoint between `a`'s furthest vertex along `normal` and `b`'s
    /// furthest against it — i.e. between the two most deeply engaged features.
    /// It is not used by the current response, which is a central impulse with no
    /// torque, and exists so an angular response can be added without changing
    /// this signature.
    pub point: Vec3,
}

impl Contact {
    /// Validation_Rules for `Contact`: `normal` is finite and unit length within
    /// `1e-4`, `depth` is finite and strictly positive, and `point` is finite.
    ///
    /// The same tolerance [`Pose::is_valid`](crate::core::scene::Pose::is_valid)
    /// uses for a quaternion, for the same reason: a normalised `f32` vector is
    /// not exactly unit length and demanding that it be is a bug generator.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.normal.is_finite()
            && (self.normal.length() - 1.0).abs() <= ROTATION_UNIT_TOLERANCE
            && self.depth.is_finite()
            && self.depth > 0.0
            && self.point.is_finite()
    }
}

/// Do two axis-aligned boxes overlap on every axis?
///
/// The broad phase. Touching counts as overlapping (`<=`), so a pair that is
/// exactly flush is passed to the narrow phase to rule on rather than rejected
/// here.
#[inline]
pub fn aabbs_overlap(a: &Aabb, b: &Aabb) -> bool {
    a.min.x <= b.max.x
        && b.min.x <= a.max.x
        && a.min.y <= b.max.y
        && b.min.y <= a.max.y
        && a.min.z <= b.max.z
        && b.min.z <= a.max.z
}

/// The contact between two hulls, or `None` when they are disjoint.
///
/// **Precondition: both meshes are convex, closed, and wound outward** — as every
/// mesh from [`super::tessellate`] is. Violations do not panic; the answer is
/// just the convex-hull answer.
///
/// # Parameters
///
/// - `a`, `b` — world-space hulls. The returned [`Contact::normal`] points from
///   `a` toward `b`.
///
/// # Returns
///
/// `Some(contact)` when the hulls overlap or exactly touch, with a strictly
/// positive depth; `None` when a separating axis exists, when either mesh is
/// empty or invalid, or when the pair overlaps only by an amount too small to
/// give a trustworthy direction.
///
/// # Cost
///
/// One AABB test, then `|faces(a)| + |faces(b)|` axes plus up to
/// [`EDGE_AXIS_BUDGET`] more, each costing one dot product per vertex of both
/// hulls, plus one `Vec` holding the overlapping axes so the tied ones can be
/// averaged. The AABB test is what keeps all of that off the bill for pairs that
/// are nowhere near each other, which in a gravitational infall is almost every
/// pair on almost every step.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{collision_mesh, contact_between_meshes, Body, Color, DVec3, Pose, Quat};
///
/// fn ball_at(x: f64) -> Body {
///     Body::Sphere {
///         pose: Pose { position: DVec3::new(x, 0.0, 0.0), rotation: Quat::IDENTITY },
///         radius: 1.0,
///         color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
///     }
/// }
///
/// // Centres 3 units apart, radius 1 each: no contact.
/// let left = collision_mesh(&ball_at(0.0)).unwrap();
/// let right = collision_mesh(&ball_at(3.0)).unwrap();
/// assert!(contact_between_meshes(&left, &right).is_none());
///
/// // Centres 1.5 units apart: overlapping, and the normal points +X, from the
/// // first hull toward the second.
/// let near = collision_mesh(&ball_at(1.5)).unwrap();
/// let contact = contact_between_meshes(&left, &near).expect("these overlap");
/// assert!(contact.is_valid());
/// assert!(contact.normal.x > 0.99);
/// assert!(contact.depth > 0.0);
/// ```
pub fn contact_between_meshes(a: &TriMesh, b: &TriMesh) -> Option<Contact> {
    // Broad phase. Cheap, and it rejects nearly everything.
    if !aabbs_overlap(&a.aabb()?, &b.aabb()?) {
        return None;
    }

    let centre_a = a.centroid()?;
    let centre_b = b.centroid()?;

    // Every candidate axis that overlapped, with its overlap, so the near-tied
    // ones can be averaged into the normal afterwards. The shallowest alone is a
    // quantised direction — see the module docs.
    let mut overlaps: Vec<(Vec3, f32)> = Vec::new();
    let mut best_depth = f32::INFINITY;

    // A closure rather than a loop body so the face axes and the edge axes share
    // one definition of "test this axis and record its overlap". Returns `false`
    // the moment an axis separates the pair, which is the whole point of SAT: one
    // such axis proves disjointness and ends the search.
    let consider = |axis: Vec3, overlaps: &mut Vec<(Vec3, f32)>, best: &mut f32| -> bool {
        let length = axis.length();
        if !length.is_finite() || length < MIN_AXIS_LENGTH {
            // Not a usable direction. Skipping it can only make the test more
            // permissive, never wrongly separating.
            return true;
        }
        let unit = axis / length;

        let Some((min_a, max_a)) = a.project_onto(unit) else {
            return true;
        };
        let Some((min_b, max_b)) = b.project_onto(unit) else {
            return true;
        };

        // Overlap of the two intervals. Non-positive means this axis separates
        // them and there is no contact.
        let overlap = max_a.min(max_b) - min_a.max(min_b);
        if !overlap.is_finite() || overlap <= 0.0 {
            return false;
        }

        if overlap < *best {
            *best = overlap;
        }
        overlaps.push((unit, overlap));
        true
    };

    // Face normals of both hulls. For any pair of convex bodies whose contact is
    // face-on — a sphere on the floor, a box on the floor — the answer is already
    // in this set.
    for face in a.faces().chain(b.faces()) {
        if let Some(normal) = face.normal() {
            if !consider(normal, &mut overlaps, &mut best_depth) {
                return None;
            }
        }
    }

    // Edge-pair axes: the ones that catch two sharp features crossing. Budgeted,
    // because this set is quadratic — see `EDGE_AXIS_BUDGET`.
    let edges_a = a.edges();
    let edges_b = b.edges();

    if edges_a.len().saturating_mul(edges_b.len()) <= EDGE_AXIS_BUDGET {
        let direction = |mesh: &TriMesh, (from, to): (u32, u32)| -> Option<Vec3> {
            Some(*mesh.vertices.get(to as usize)? - *mesh.vertices.get(from as usize)?)
        };

        for &edge_a in &edges_a {
            let Some(along_a) = direction(a, edge_a) else {
                continue;
            };
            for &edge_b in &edges_b {
                let Some(along_b) = direction(b, edge_b) else {
                    continue;
                };
                if !consider(along_a.cross(along_b), &mut overlaps, &mut best_depth) {
                    return None;
                }
            }
        }
    }

    if !best_depth.is_finite() || best_depth <= 0.0 || overlaps.is_empty() {
        return None;
    }

    // A separating axis carries no sign, so every axis is oriented from `a` toward
    // `b` before it is used or averaged. Centroid to centroid is the reliable way
    // round for convex hulls; if the centroids coincide there is no meaningful
    // direction and the pair is left alone rather than pushed somewhere arbitrary.
    let centre_offset = centre_b - centre_a;
    let oriented = |axis: Vec3| -> Option<Vec3> {
        match centre_offset.dot(axis) {
            towards if towards > 0.0 => Some(axis),
            towards if towards < 0.0 => Some(-axis),
            // Perpendicular to the line of centres: it has no side to be on, so it
            // cannot contribute a direction.
            _ => None,
        }
    };

    // Average every axis tied with the shallowest. On a faceted curve these
    // straddle the true contact direction and their average recovers it; on a flat
    // contact they are all the same direction and the average is that direction.
    let tie_threshold = best_depth * (1.0 + NORMAL_TIE_TOLERANCE);
    let summed = overlaps
        .iter()
        .filter(|&&(_, overlap)| overlap <= tie_threshold)
        .filter_map(|&(axis, _)| oriented(axis))
        .fold(Vec3::ZERO, |sum, axis| sum + axis);

    // The sum degenerates only if the tied axes cancel — possible in principle for
    // a symmetric deep overlap — in which case fall back to the shallowest single
    // axis rather than inventing a direction.
    let normal = if summed.length() >= MIN_AXIS_LENGTH {
        summed.normalize()
    } else {
        let (shallowest, _) = overlaps
            .iter()
            .copied()
            .min_by(|(_, left), (_, right)| left.total_cmp(right))?;
        oriented(shallowest)?
    };

    // Depth is measured along the normal that was actually chosen, not along the
    // axis that won the contest — they are the same for a flat contact and differ
    // slightly for an averaged one, and separating by the wrong figure would leave
    // residual overlap or overshoot.
    let (min_a, max_a) = a.project_onto(normal)?;
    let (min_b, max_b) = b.project_onto(normal)?;
    let depth_along_normal = max_a.min(max_b) - min_a.max(min_b);

    let best_depth = if depth_along_normal.is_finite() && depth_along_normal > 0.0 {
        depth_along_normal
    } else {
        best_depth
    };

    // The two most deeply engaged features: `a`'s furthest vertex along the
    // normal and `b`'s furthest against it.
    let deepest_a = a.support_point(normal)?;
    let deepest_b = b.support_point(-normal)?;

    let contact = Contact {
        normal,
        depth: best_depth,
        point: 0.5 * (deepest_a + deepest_b),
    };

    // A contact that fails its own rules is worse than no contact: the response
    // would divide by it. This is the one place the invariant is enforced rather
    // than asserted, because the inputs are floats from an unbounded scene.
    if contact.is_valid() {
        Some(contact)
    } else {
        None
    }
}

/// The contact between two props, tessellating both hulls on the spot.
///
/// The convenience form of [`contact_between_meshes`] for callers holding props
/// rather than meshes — tests, and any one-off query. **Not** what
/// [`super::resolve`] uses: it builds each hull once per pass and reuses it
/// across pairs, which this cannot do.
///
/// Returns `None` when either prop has no hull (see [`contact_mesh`]) as well
/// as when the two are disjoint. The normal points from `a` toward `b`.
pub fn contact_between_props(a: &Prop, b: &Prop) -> Option<Contact> {
    let mesh_a = contact_mesh(a)?;
    let mesh_b = contact_mesh(b)?;
    contact_between_meshes(&mesh_a, &mesh_b)
}

#[cfg(test)]
#[path = "../../tst/collision/contact_tests.rs"]
mod tests;
