//! Input layer: turning keyboard, mouse, and window state into camera motion.
//!
//! [`camera`] holds the free-fly controller. It is separated from
//! [`crate::render`] because it drives the camera entity's `Transform`
//! directly from input and never touches [`crate::core::scene::Scene`].
//!
//! [`controls`] binds the simulation clock's keys, and [`menu`] holds the
//! **input mode**: while the menu is open, only `Esc` is bound, and the
//! camera, the clock keys, and the trail toggle all stand down. That state
//! lives here rather than in [`crate::ui`] because three systems in two
//! other layers read it to decide whether to run, and none of them should
//! have to depend on the UI toolkit to ask.

pub mod camera;
pub mod constants;
pub mod controls;
pub mod menu;
pub mod mission;
pub mod placement;
