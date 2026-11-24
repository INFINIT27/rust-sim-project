//! The GPU the physics runs on: finding one, and the switch that turns the GPU path off.
//!
//! In the app the device is the one Bevy renders with ([`GpuDevice::from_render`]), so the particles
//! the physics steps are the buffer the renderer draws, with nothing copied back to the CPU to be
//! drawn. In a headless test there is no renderer, and [`GpuDevice::request`] makes a device of its
//! own, wrapped in the same types.
//!
//! Particle simulations need a GPU (see `src/compute/README.md`): without one [`GpuDevice::request`]
//! returns `None` and the app falls back to the frozen CPU path. A machine with no GPU driver at all (a CI runner, a remote session) can still have
//! one through a software Vulkan driver such as Mesa's `lavapipe`, which `wgpu` finds like any
//! other; it is slow, and CI runs the GPU tests on it in a job that is not required.

use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::compute::constants::GPU_PHYSICS_ENV;

/// A GPU device and its queue, ready for compute work: Bevy's, in the app.
pub struct GpuDevice {
    /// The device: buffers, pipelines and command encoders are made from it. Bevy's handle type, so
    /// a buffer made from it can be handed to the renderer; [`GpuDevice::wgpu`] is the raw device.
    pub device: RenderDevice,
    /// The queue work is submitted on.
    pub queue: RenderQueue,
    /// What the device is, for the menu and the logs: its name and backend.
    pub info: wgpu::AdapterInfo,
    /// Set when the device has been lost or has reported an error: the GPU was reset (Windows
    /// resets one that runs a single piece of work for more than about two seconds), or a
    /// command was invalid. Once set the device is not used again and the physics steps on the CPU.
    pub failed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl GpuDevice {
    /// The best GPU available, or `None` when there is none (or none that can run compute).
    ///
    /// Prefers a high-performance adapter, which on a laptop with two GPUs is the discrete one.
    /// Blocks while the driver answers, which takes milliseconds at startup and never happens
    /// again.
    ///
    /// Not compiled into unit tests without the `gpu-tests` feature, so a GPU test that is not marked
    /// as one fails to compile there (see `tst/README.md`).
    #[cfg(any(not(test), feature = "gpu-tests"))]
    pub fn request() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter =
            bevy::tasks::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            }))?;
        let info = adapter.get_info();
        let (device, queue) = bevy::tasks::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("physics"),
                // Timestamps when the adapter has them, for the GPU pass timer
                // (`compute::profiler`). Bevy's own device asks for every feature the adapter has.
                required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
                // Everything the adapter offers: the particle buffers grow with the detail
                // setting, and the defaults cap a storage buffer at 128 MiB.
                required_limits: adapter.limits(),
            },
            None,
        ))
        .ok()?;
        Some(Self::watched(
            RenderDevice::from(device),
            RenderQueue(std::sync::Arc::new(
                bevy::render::renderer::WgpuWrapper::new(queue),
            )),
            info,
        ))
    }

    /// The device Bevy renders with, so the physics' buffers are the renderer's too. `info` is
    /// Bevy's `RenderAdapterInfo`.
    pub fn from_render(device: RenderDevice, queue: RenderQueue, info: wgpu::AdapterInfo) -> Self {
        Self::watched(device, queue, info)
    }

    /// `device` and `queue`, with an error and a lost device logged and remembered rather than
    /// panicking.
    fn watched(device: RenderDevice, queue: RenderQueue, info: wgpu::AdapterInfo) -> Self {
        // `wgpu` panics on an error nothing handles, which would take the app down with the GPU.
        // Log it instead, and remember, so the caller falls back to the CPU.
        let failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let device_handle = device.wgpu_device();
        let on_error = failed.clone();
        device_handle.on_uncaptured_error(Box::new(move |error| {
            bevy::log::error!("GPU error, physics falling back to the CPU: {error}");
            on_error.store(true, std::sync::atomic::Ordering::Relaxed);
        }));
        let on_lost = failed.clone();
        device_handle.set_device_lost_callback(move |reason, message| {
            // Dropping the device, as the app does when it exits, also reports it lost.
            if matches!(reason, wgpu::DeviceLostReason::Dropped) {
                return;
            }
            bevy::log::error!("physics GPU lost ({reason:?}), falling back to the CPU: {message}");
            on_lost.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        Self {
            device,
            queue,
            info,
            failed,
        }
    }

    /// The raw `wgpu` device.
    pub fn wgpu(&self) -> &wgpu::Device {
        self.device.wgpu_device()
    }

    /// The device's name and backend, for display: `"NVIDIA GeForce RTX 4070 (Vulkan)"`.
    pub fn describe(&self) -> String {
        format!("{} ({:?})", self.info.name, self.info.backend)
    }

    /// Has the device been lost, or reported an error? See [`failed`](Self::failed).
    pub fn has_failed(&self) -> bool {
        self.failed.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Whether the GPU physics may be used, by the [`GPU_PHYSICS_ENV`] variable: anything but `off`,
/// `0`, `false` or `no` (in any case) leaves it on, and so does the variable being unset.
///
/// # Examples
///
/// ```
/// use physics_sandbox::gpu_physics_enabled_by;
///
/// assert!(gpu_physics_enabled_by(None));
/// assert!(gpu_physics_enabled_by(Some("on")));
/// assert!(!gpu_physics_enabled_by(Some("OFF")));
/// ```
pub fn gpu_physics_enabled_by(value: Option<&str>) -> bool {
    !matches!(
        value
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Some("off" | "0" | "false" | "no")
    )
}

/// [`gpu_physics_enabled_by`] for this process's environment.
pub fn gpu_physics_enabled() -> bool {
    gpu_physics_enabled_by(std::env::var(GPU_PHYSICS_ENV).ok().as_deref())
}

#[cfg(test)]
#[path = "../../tst/compute/device_tests.rs"]
mod tests;

/// The tests that open a GPU: see the `gpu-tests` feature in `Cargo.toml`.
#[cfg(all(test, feature = "gpu-tests"))]
#[path = "../../tst/compute/device_gpu_tests.rs"]
mod gpu_tests;
