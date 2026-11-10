//! Turning a [`Body`] into the closed triangle hull that bounds it.
//!
//! [`collision_mesh`] is the whole surface of this module: give it a body, get
//! back a world-space [`TriMesh`] wound outward, or `None` when the body has no
//! interior to bound. Everything else here is the per-variant construction.
//!
//! # Why derive the hull instead of storing one
//!
//! [`Body`] keeps its four variants and [`crate::core::scene::Prop`] keeps
//! its four fields; nothing in the model changed to make collision work. A hull
//! that is *derived* cannot disagree with the shape it came from, whereas a hull
//! stored beside the body drifts out of sync the first time someone edits a
//! radius. The cost is that the hull is rebuilt rather than remembered — see
//! [`super::resolve`] for how the broad phase avoids building most of them.
//!
//! # What each variant becomes
//!
//! | Variant | Hull | Triangles |
//! | --- | --- | --- |
//! | `Sphere` | Icosphere at [`SPHERE_SUBDIVISIONS`], vertices on the sphere | 80 |
//! | `Cuboid` | The box itself, two triangles per face | 12 |
//! | `Plane` | A slab of thickness [`PLANE_THICKNESS_UNITS`] hanging below the surface | 12 |
//! | `Line` | A square prism of side `width` along the segment | 12 |
//!
//! # The sphere hull is smaller than the sphere
//!
//! An icosphere with its vertices *on* the sphere is inscribed: its faces cut
//! the corner off every spherical cap, so the hull's inradius is smaller than
//! the radius it was built from. At [`SPHERE_SUBDIVISIONS`] `= 1` the face
//! centres sit at `0.934` of the radius, so two spheres touch about 6.6% later
//! than ideal spheres would — a `1.5`-unit ball's surface is up to `0.099` units
//! inside where you would draw it.
//!
//! That is the honest tradeoff of triangle modelling and it is deliberately not
//! papered over by scaling the vertices out: pushing them onto a circumscribed
//! radius trades a 6.6% undershoot at face centres for an overshoot at the 42
//! vertices, which produces contacts in empty space. Undershooting is the failure
//! mode that looks right. Raise [`SPHERE_SUBDIVISIONS`] to shrink it — the error
//! falls three- to fourfold per level, at four times the triangles.

pub use bevy::math::{Quat, Vec3};

use std::collections::HashMap;

use crate::collision::constants::{PLANE_THICKNESS_UNITS, SPHERE_SUBDIVISIONS};
use crate::collision::trimesh::TriMesh;
use crate::core::scene::{Body, Prop};

/// The closed, outward-wound, world-space triangle hull of `body`, or `None`
/// when the body bounds no volume.
///
/// # Returns
///
/// `Some(mesh)` for every valid `Sphere`, `Cuboid`, and `Plane`, and for a
/// `Line` of strictly positive width. `None` for:
///
/// - an invalid body — no hull is derivable from `NaN`, and inventing one would
///   push the problem into the narrow phase;
/// - a `Line` of zero width or zero length. `Body::is_valid` permits `width ==
///   0.0` (that asymmetry is deliberate — see `core/README.md`), and a zero-width
///   line is a mathematical curve: it has a surface of zero area and no inside,
///   so it takes part in gravity but not in contact. Same for a line whose
///   endpoints coincide.
///
/// A `None` means "this prop has no physical border", and [`super::resolve`]
/// treats such props as pass-through. That is a real physical stance, not a
/// shrug: a body with no volume has no surface to bounce off.
///
/// # Coordinates
///
/// The returned mesh is in **world units**, already posed — rotation applied,
/// then translation — so it needs no further transform before being intersected
/// with another hull.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{collision_mesh, Body, Color, DVec3, Vec3};
///
/// let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
///
/// // A zero-width line is a curve: no interior, so no hull.
/// let curve = Body::Line { from: DVec3::ZERO, to: DVec3::X, width: 0.0, color: white };
/// assert!(collision_mesh(&curve).is_none());
///
/// // Give it width and it becomes a solid rod.
/// let rod = Body::Line { from: DVec3::ZERO, to: DVec3::X, width: 0.2, color: white };
/// let mesh = collision_mesh(&rod).expect("a rod bounds a volume");
/// assert!(mesh.is_closed());
/// ```
/// The hull a **prop** physically interacts through, in world space.
///
/// [`collision_mesh`] builds a hull from a [`Body`], which is the shape as *drawn*. This
/// builds it from the shape as it physically *is*: when
/// [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units) is set, the
/// sphere's radius is replaced by it.
///
/// # Why the two can differ
///
/// Jupiter is the case that forced it. Drawn at `10.973` units so it is visible from Mars's
/// orbit, but truly `0.0699` units — while Io orbits at `0.4217`. Built from the drawn body,
/// Jupiter's hull is `26×` the radius of its innermost moon's entire orbit, so all four
/// Galilean moons start inside the planet and the contact solver ejects them on the first
/// step. No single exaggeration both shows Jupiter and lets it hold a moon, so the visual
/// radius stays on the `Body` and the physical one travels with the `Prop`.
///
/// **Every contact test in this layer goes through here**, so a prop with a contact radius set
/// is physically the size it says and visually the size it looks. Gravitation is unaffected —
/// it treats each prop as a point mass and never reads a radius at all.
///
/// # Returns
///
/// Whatever [`collision_mesh`] would return for the prop's body, except with the sphere radius
/// substituted. `None` for a prop with no physical border, or one that fails validation.
///
/// Only `Body::Sphere` honours the override: the other three variants are sized by extents
/// rather than a radius, and nothing in the crate draws a cuboid or a plane at a scale
/// different from its physical one. A contact radius set on one of those is **ignored**, which
/// `tessellate_tests` pins so the silence is deliberate rather than a surprise.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{contact_mesh, Body, Color, Pose, Prop, Quat, DVec3, Vec3};
///
/// let drawn_big = Body::Sphere {
///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
///     radius: 10.0,
///     color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
/// };
///
/// let honest = Prop::at_rest(drawn_big, 1.0).with_contact_radius(0.5);
/// let hull = contact_mesh(&honest).unwrap();
/// let bounds = hull.aabb().unwrap();
///
/// // The hull is built at 0.5 units, not 10.
/// assert!(bounds.max.x < 0.6, "the hull follows the contact radius");
/// ```
pub fn contact_mesh(prop: &Prop) -> Option<TriMesh> {
    match (prop.contact_radius_units, &prop.body) {
        (Some(radius), Body::Sphere { pose, color, .. }) => {
            // Rebuilt rather than patched in place: `Body` is the render-facing type and this
            // layer has no business mutating one. The clone is a `Pose` and a `Color`.
            let physical = Body::Sphere {
                pose: *pose,
                radius,
                color: *color,
            };
            collision_mesh(&physical)
        }
        _ => collision_mesh(&prop.body),
    }
}

pub fn collision_mesh(body: &Body) -> Option<TriMesh> {
    if !body.is_valid() {
        return None;
    }

    match body {
        Body::Sphere { pose, radius, .. } => Some(sphere_hull(
            pose.position.as_vec3(),
            pose.rotation,
            *radius,
            SPHERE_SUBDIVISIONS,
        )),
        Body::Cuboid { .. } | Body::Plane { .. } => {
            let frame = hull_box(body)?;
            Some(box_hull(frame.centre, frame.rotation, frame.half_extents))
        }
        Body::Line {
            from, to, width, ..
        } => line_hull(from.as_vec3(), to.as_vec3(), *width),
    }
}

/// The box a `Cuboid` or `Plane` hull is built from, exactly as [`collision_mesh`] builds it.
///
/// `rotation` is the pose's quaternion as stored, not renormalised, because that is what
/// [`box_hull`] applies.
pub(crate) struct HullBox {
    pub centre: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
}

/// The [`HullBox`] behind `body`'s hull, or `None` for a variant whose hull is not a posed box.
///
/// Shared with the broad phase in [`super::resolve`], so the box it tests against cannot drift
/// from the box the narrow phase intersects.
pub(crate) fn hull_box(body: &Body) -> Option<HullBox> {
    match body {
        Body::Cuboid {
            pose, half_extents, ..
        } => Some(HullBox {
            centre: pose.position.as_vec3(),
            rotation: pose.rotation,
            half_extents: *half_extents,
        }),
        Body::Plane {
            pose, half_size, ..
        } => {
            // The quad is the slab's top face, so the solid's centre sits half a
            // thickness below it — along the plane's own local `-Y`, so a rotated
            // floor thickens into itself rather than into the world's down.
            let half_thickness = 0.5 * PLANE_THICKNESS_UNITS;
            Some(HullBox {
                centre: pose.position.as_vec3()
                    + pose.rotation * Vec3::new(0.0, -half_thickness, 0.0),
                rotation: pose.rotation,
                half_extents: Vec3::new(*half_size, half_thickness, *half_size),
            })
        }
        Body::Sphere { .. } | Body::Line { .. } => None,
    }
}

/// An icosphere hull of `radius` centred at `centre`.
///
/// `rotation` is applied even though a sphere is rotationally symmetric: the
/// *hull* is not — it is a 42-vertex polyhedron — so honouring the pose keeps the
/// hull attached to the body's frame and keeps the function honest about what it
/// is approximating.
fn sphere_hull(centre: Vec3, rotation: Quat, radius: f32, subdivisions: u32) -> TriMesh {
    let mut mesh = unit_icosphere(subdivisions);
    for vertex in &mut mesh.vertices {
        *vertex = centre + rotation * (*vertex * radius);
    }
    mesh.orient_faces_outward(centre);
    mesh
}

/// A box hull: `half_extents` about `centre`, rotated by `rotation`.
///
/// Twelve triangles, two per face. Shared by `Cuboid` (directly), `Plane` (as a
/// thin slab) and `Line` (as a prism along the segment), which is why the three
/// agree exactly on how a box is built.
fn box_hull(centre: Vec3, rotation: Quat, half_extents: Vec3) -> TriMesh {
    // The eight corners, in the order given by reading the low three bits of the
    // index as (x, y, z) sign bits. The triples below depend on this order.
    let mut vertices = Vec::with_capacity(8);
    for index in 0..8u32 {
        let sign = |bit: u32| if index & (1 << bit) == 0 { -1.0 } else { 1.0 };
        let local = Vec3::new(
            sign(0) * half_extents.x,
            sign(1) * half_extents.y,
            sign(2) * half_extents.z,
        );
        vertices.push(centre + rotation * local);
    }

    // Two triangles per face. Winding is not hand-verified here — every quad is
    // listed in a consistent local order and `orient_faces_outward` fixes the
    // ones that came out inward, which is a mechanism rather than a hope.
    let quads = [
        [0, 1, 3, 2], // -Z
        [4, 5, 7, 6], // +Z
        [0, 1, 5, 4], // -Y
        [2, 3, 7, 6], // +Y
        [0, 2, 6, 4], // -X
        [1, 3, 7, 5], // +X
    ];
    let mut triangles = Vec::with_capacity(12);
    for [a, b, c, d] in quads {
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }

    let mut mesh = TriMesh::new(vertices, triangles);
    mesh.orient_faces_outward(centre);
    mesh
}

/// A square prism of side `width` running from `from` to `to`, or `None` when
/// that describes no volume.
///
/// The rendered `Line` is a rod, and this is that rod as a solid: a box whose
/// local X axis is rotated onto the segment, `width` on a side. `None` when
/// `width <= 0.0` or the endpoints coincide — see [`collision_mesh`] for why
/// that is a physical answer rather than an error.
fn line_hull(from: Vec3, to: Vec3, width: f32) -> Option<TriMesh> {
    let along = to - from;
    let length = along.length();

    if !length.is_finite() || length <= 0.0 || !width.is_finite() || width <= 0.0 {
        return None;
    }

    // `from_rotation_arc` is undefined for antiparallel inputs in some glam
    // versions and this one handles it, but the direction here comes from a
    // finite non-zero segment so the only special case is already excluded.
    let rotation = Quat::from_rotation_arc(Vec3::X, along / length);
    let half_width = 0.5 * width;

    Some(box_hull(
        0.5 * (from + to),
        rotation,
        Vec3::new(0.5 * length, half_width, half_width),
    ))
}

/// A unit-radius icosphere: an icosahedron subdivided `subdivisions` times, with
/// every vertex projected back onto the unit sphere.
///
/// Starts from the golden-ratio icosahedron — 12 vertices, 20 faces — because it
/// is the most uniform triangulation of a sphere available and its faces are all
/// congruent, so the shape error is the same in every direction. A UV sphere
/// would give the same triangle count a far worse worst case, bunching vertices
/// at the poles.
///
/// Midpoints are shared between neighbouring triangles via a lookup on the
/// vertex-index pair, so the result stays a closed manifold — split them and the
/// mesh becomes 20·4ⁿ disconnected triangles with cracks between them, which
/// `is_closed` would reject and physics would leak through.
fn unit_icosphere(subdivisions: u32) -> TriMesh {
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;

    let mut vertices: Vec<Vec3> = vec![
        Vec3::new(-1.0, phi, 0.0),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(-1.0, -phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(0.0, -1.0, phi),
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, -1.0, -phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(phi, 0.0, -1.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(-phi, 0.0, -1.0),
        Vec3::new(-phi, 0.0, 1.0),
    ]
    .into_iter()
    .map(|vertex| vertex.normalize())
    .collect();

    let mut triangles: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    for _ in 0..subdivisions {
        // Cache of edge midpoints, keyed by the ordered index pair, so the two
        // triangles sharing an edge get the *same* new vertex.
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut refined = Vec::with_capacity(triangles.len() * 4);

        for [i, j, k] in triangles {
            let mut midpoint = |a: u32, b: u32, vertices: &mut Vec<Vec3>| -> u32 {
                let key = if a <= b { (a, b) } else { (b, a) };
                if let Some(&existing) = midpoints.get(&key) {
                    return existing;
                }
                let position = ((vertices[a as usize] + vertices[b as usize]) * 0.5).normalize();
                let index = vertices.len() as u32;
                vertices.push(position);
                midpoints.insert(key, index);
                index
            };

            let ij = midpoint(i, j, &mut vertices);
            let jk = midpoint(j, k, &mut vertices);
            let ki = midpoint(k, i, &mut vertices);

            // Each child keeps its parent's orientation, so a consistently wound
            // input stays consistently wound.
            refined.push([i, ij, ki]);
            refined.push([ij, j, jk]);
            refined.push([ki, jk, k]);
            refined.push([ij, jk, ki]);
        }

        triangles = refined;
    }

    let mut mesh = TriMesh::new(vertices, triangles);
    mesh.orient_faces_outward(Vec3::ZERO);
    mesh
}

#[cfg(test)]
#[path = "../../tst/collision/tessellate_tests.rs"]
mod tests;
