//! GUI-free core: the pure-Rust world model.
//!
//! This is the only layer allowed to be renderer-agnostic, and the rule is
//! strict: modules under `core` may reference `bevy::math` (for the glam
//! `Vec3`/`Quat` re-exports) and nothing else from Bevy — no `bevy::ecs`, no
//! `bevy_egui`, no `wgpu` (Requirements 4.2 and 15.3). Everything that
//! crosses into Bevy lives in [`crate::render`], [`crate::input`], or
//! [`crate::ui`].
//!
//! Keeping the model here is what lets it be unit- and property-tested with
//! no window, no GPU, and no ECS, and what leaves [`scene::Scene::step`] free
//! to host a real physics integrator later.

//! # What is where
//!
//! - [`scene`] — the data model: [`scene::Scene`], [`scene::Prop`] (a body plus
//!   its velocity and mass), and the value types.
//! - [`integrator`] — the body of the `Scene::step` seam: mutual gravitation,
//!   composed entirely from [`crate::physics_utils`] functions.

pub mod constants;
pub mod integrator;
pub mod scene;
pub mod transfer;
