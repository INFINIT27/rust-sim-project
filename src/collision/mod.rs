//! Body physics: where a prop's surface is, when two surfaces meet, and what
//! happens next.
//!
//! Before this layer existed, props were gravitational point masses that passed
//! straight through one another — a `Body::Sphere`'s radius was a rendering
//! detail with no physical meaning. This layer gives every prop a **physical
//! border**: a closed triangle mesh separating the inside of the body from the
//! outside, tested for overlap every substep and resolved elastically.
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
//! | Response | [`resolve`] | Elastic velocity swap, then separation. |
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
//! and the render side. The one physical *law* this layer applies —
//! [`elastic_collision_1d`](crate::physics_utils::conservation_laws::elastic_collision_1d)
//! — is `f64`, so [`resolve`] converts at that call and back, exactly as
//! [`crate::core::integrator`] does for gravitation.
//!
//! # Deliberate simplifications
//!
//! Elastic (except for absorption, which is perfectly *in*elastic by construction),
//! frictionless, non-rotating, one resolution pass, contact
//! tested at substep boundaries rather than swept. Each is stated with its
//! consequence in [`resolve`], and `src/collision/README.md` collects them in one
//! table.

pub mod absorb;
pub mod constants;
pub mod contact;
pub mod resolve;
pub mod tessellate;
pub mod trimesh;
