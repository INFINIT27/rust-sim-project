//! Body physics: where a prop's surface is, when two surfaces meet, and what
//! happens next.
//!
//! Before this layer existed, props were gravitational point masses that passed
//! straight through one another — a `Body::Sphere`'s radius was a rendering
//! detail with no physical meaning. This layer gives every prop a **physical
//! border**: a closed triangle mesh separating the inside of the body from the
//! outside, tested for overlap every substep and resolved with each prop's
//! [`Material`](crate::core::scene::Material): restitution and friction, perfectly elastic and
//! frictionless by default.
//!
//! # Why triangles
//!
//! A radius answers "how big is this sphere"; it does not answer "is this point
//! inside that body", and it stops answering anything at all as soon as the body
//! is a box, a rod, or a floor. A triangle mesh answers the general question with
//! one representation:
//!
//! - **Inside versus outside is encoded in the winding.** Every triangle is
//!   counter-clockwise seen from outside, so its normal points out of the solid.
//!   A closed, consistently wound mesh therefore *is* the boundary — see
//!   [`trimesh`].
//! - **The contact test comes from the geometry, not from the variant.** The
//!   separating axes are the mesh's own face normals and edge directions, so one
//!   code path handles every pair of shapes and a new `Body` variant needs a
//!   tessellation rather than a new collision case — see [`contact`].
//!
//! # The pipeline
//!
//! | Stage | Module | What it produces |
//! | --- | --- | --- |
//! | Broad phase | [`resolve`] | Bounding sphere, and sphere-to-box for `Cuboid` and `Plane`, before any hull exists. |
//! | Tessellate | [`tessellate`] | A world-space closed hull per `Body`, only for pairs that survived. |
//! | Broad phase | [`contact`] | AABB overlap of the hulls. |
//! | Narrow phase | [`contact`] | A [`contact::Contact`]: normal, depth, point. |
//! | Response | [`absorb`] | A prop that touches an absorber is merged into it and leaves. |
//! | Response | [`collision`] | Two planets that cannot escape each other become one body, and the debris the impact throws off. |
//! | Response | [`resolve`] | Bounce with the pair's restitution, Coulomb friction, then separation. |
//!
//! There are **two** responses because there are two physically different things a
//! contact can be. Two solid bodies bounce ([`resolve`]); anything that reaches a black
//! hole does not come back ([`absorb`]). An elastic bounce off an event horizon is not an
//! approximation of what happens, it is the opposite of it — so the absorber's flag lives
//! on [`Prop`](crate::core::scene::Prop) and absorption runs **first**, which is what stops
//! a star bouncing off a hole on its way in.
//!
//! [`Scene::step`](crate::core::scene::Scene::step) drives it: integrate a
//! substep, absorb, resolve contacts, repeat. [`resolve::substep_count`] decides how
//! finely a frame has to be sliced for the overlap test to catch what is moving.
//!
//! # Import restrictions
//!
//! **`bevy::math` only** — the same rule as [`crate::core`], for the same reason:
//! contact geometry must be testable with no window, no GPU, and no ECS. It may
//! use `crate::core` (for `Body`, `Prop`, and `Aabb`) and `crate::physics_utils`
//! (for the elastic solver), and nothing from `render`, `input`, or `ui`.
//!
//! # `f32` here, `f64` in `physics_utils`
//!
//! Geometry is `f32` because it is scene data and shares its numbers with glam
//! and the render side. The physical *laws* this layer applies —
//! [`restitution_collision_1d`](crate::physics_utils::conservation_laws::restitution_collision_1d)
//! and
//! [`coulomb_friction_speed_change`](crate::physics_utils::conservation_laws::coulomb_friction_speed_change)
//! — are `f64`, so [`resolve`] converts at those calls and back, exactly as
//! [`crate::core::integrator`] does for gravitation.
//!
//! # Deliberate simplifications
//!
//! Non-rotating (so friction removes sliding rather than turning it into rolling), one
//! resolution pass with no resting-contact solver, debris of equal fragments thrown evenly in
//! every direction ([`collision`]), and contact tested at substep boundaries rather than swept.
//! Each is stated with its
//! consequence in [`resolve`], and `src/collision/README.md` collects them in one
//! table, followed by the design for lifting them, one phase at a time.

pub mod absorb;
pub mod aggregate;
// Named after the layer on purpose: it is where the behaviour between two bodies is worked out,
// which is what the layer is for. Clippy flags a module named like its parent; here that is the
// name asked for.
#[allow(clippy::module_inception)]
pub mod collision;
pub mod constants;
pub mod contact;
pub mod resolve;
pub mod tessellate;
pub mod trimesh;
