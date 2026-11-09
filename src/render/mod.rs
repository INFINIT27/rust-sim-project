//! Render layer: the boundary where the pure model meets Bevy's ECS.
//!
//! - [`bridge`] holds the pure `Body -> Transform` mapping plus the per-frame
//!   system that mirrors [`crate::core::scene::Scene`] onto ECS `Transform`s.
//! - [`plugin`] mounts the model into a Bevy `App`: it adopts `Scene` as a
//!   `Resource`, seeds the world at `Startup`, and registers the per-frame
//!   `step -> sync` pair.
//!
//! Unlike [`crate::core`], modules here may import Bevy freely — that is the
//! whole point of the split.

pub mod bridge;
pub mod constants;
pub mod eclipse;
pub mod grid;
pub mod mission;
pub mod placement;
pub mod plugin;
pub mod trail;
