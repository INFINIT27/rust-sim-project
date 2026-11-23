//! GPU physics: stepping a scene made of particles on the GPU.
//!
//! The rest of the crate's physics runs on the CPU, in `f64`, one pair at a time. That is right
//! for a solar system of a dozen bodies and stops scaling for a planet built from thousands of
//! particles, where gravity and contact are both a sum over every pair. This layer runs exactly
//! that sum on the GPU, thousands of pairs at once, for scenes made only of particles:
//!
//! | Module | What it holds |
//! | --- | --- |
//! | [`device`] | [`device::GpuDevice`]: finding a GPU (or not), and the `RGA_GPU_PHYSICS` switch. |
//! | [`particles`] | [`particles::GpuParticles`]: the stepper, and `particles.wgsl`, its kernel. |
//! | [`profiler`] | [`profiler::GpuProfile`]: the GPU pass timer, from timestamp queries (`RGA_GPU_PROFILE`). |
//! | [`constants`] | The workgroup size and the switch's name. |
//!
//! The app uses it when it can and falls back to the CPU when it cannot: no GPU, the switch off, a
//! scene with anything but particles, or a read-back that failed. Both paths step the same
//! simulation; see [`particles`] for how far they agree and why not to the last bit.
//!
//! # Import restrictions
//!
//! `wgpu`, `bevy::tasks` (to block on the device's answers), and the crate's own `core`,
//! `collision`, `physics_utils` and `time`. Nothing from `bevy::ecs`, the renderer, or the layers
//! above: the Bevy resource and plugin that put this in the app live in `render`, so this layer is
//! testable with no window and no ECS, like the physics it mirrors.

pub mod constants;
pub mod device;
pub mod particles;
pub mod profiler;
