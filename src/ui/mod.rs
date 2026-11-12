//! UI layer: the on-screen debug overlay.
//!
//! - [`overlay`] — the `Debug` panel: FPS, body count, camera pose.
//! - [`crosshair`] — a small centre-screen crosshair marking the point the
//!   camera is looking at.
//! - [`compass`] — the three world axes, drawn top-right as an orientation gizmo.
//! - [`menu`] — the `Esc` panel: current settings and the key bindings. The only
//!   module here with an interactive widget; see its docs for why that does not
//!   break Requirement 11.4.
//!
//! This directory is deliberately the only place in the crate that imports
//! `bevy_egui`, so swapping the UI toolkit is a one-directory change. Both
//! modules are read-only with respect to simulation state, and neither may
//! consume input the camera needs (Requirement 11.4).

pub mod arrival;
pub mod compass;
pub mod constants;
pub mod crosshair;
pub mod hud;
pub mod menu;
pub mod overlay;
pub mod placement;
