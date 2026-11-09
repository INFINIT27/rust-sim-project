//! Time: one clock for the experiment, one for the observer.
//!
//! This layer owns every definition of duration in the crate. It exists because
//! time was previously *implicit* — a raw `f32` was clamped and scaled inside a Bevy
//! system, handed down through four physics signatures that each re-validated it,
//! and read independently by the camera and the overlay with nothing recording that
//! those two were deliberately on a different clock. There was no way to ask how
//! long the simulation had been running.
//!
//! # The two clocks
//!
//! | Type | Runs at | Read by |
//! | --- | --- | --- |
//! | [`simulation::SimulationClock`] | `scale` simulated seconds per real second (`5e6` shipped) | `Scene::step` and everything under it |
//! | [`observer::ObserverClock`] | wall time | the free-fly camera, the overlay's FPS |
//!
//! A paused experiment can still be flown around and inspected, because pausing
//! stops one clock and not the other. That is the point of the split rather than a
//! side effect of it.
//!
//! # `TimeStep` is the interface, and the type is the enforcement
//!
//! [`simulation::TimeStep`] is a validated, non-negative, finite interval of
//! simulated seconds, and it is the **only** thing that can advance physics:
//! `Scene::step`, `integrate_gravity`, and `substep_count` all take one, and it
//! cannot be built from a float except through
//! [`TimeStep::from_seconds`](simulation::TimeStep::from_seconds), which rejects
//! anything unusable. So there is no path by which an unvalidated duration reaches
//! the integrator, and no second place where a "dt" is decided — which is what
//! "central and enforced" means here. Substeps go through
//! [`TimeStep::split`](simulation::TimeStep::split), so they carry the same
//! guarantee.
//!
//! The one thing this layer does **not** claim is the closed-form relations in
//! [`crate::physics_utils`], which still take a plain `elapsed_s: f64`. They are
//! scalar mathematics, callable from a test or a spreadsheet with no clock in sight,
//! and coupling them to a simulation type would be the wrong kind of centralisation.
//! The boundary is exact: a `TimeStep` is the currency of *stepping the world*, and
//! the `f64` it yields via [`seconds`](simulation::TimeStep::seconds) is what feeds
//! the formulas inside a step.
//!
//! # Import restrictions
//!
//! **No Bevy at all** — not even `bevy::math`, since a duration is a scalar. This is
//! the only layer in the crate with no Bevy import whatsoever, which is what lets it
//! be reasoned about with no window, no ECS, and no frame loop. The `Resource` impls
//! that let Bevy hold these clocks live in [`crate::render::plugin`], the same way
//! `Scene`'s does.
//!
//! # `f64` throughout
//!
//! Durations and elapsed times are `f64`, matching [`crate::physics_utils`] rather
//! than the scene's `f32` geometry: elapsed simulated time reaches `2e8` seconds in
//! the shipped scene, and accumulating that in `f32` would start losing whole
//! seconds. Narrowing to `f32` happens at exactly one place,
//! [`TimeStep::seconds_f32`](simulation::TimeStep::seconds_f32), which is named so
//! the loss is visible at the call site.

pub mod constants;
pub mod observer;
pub mod simulation;
