//! The GPU the physics runs on: finding one, and the switch that turns the GPU path off.
//!
//! The device is the layer's own, not the one Bevy renders with. That keeps the physics free of
//! the ECS and the render graph, so it runs in a headless test exactly as in the app, and the cost
//! is one more logical device on the same GPU, which drivers handle routinely. Both are made from
//! the same `wgpu` crate.
//!
//! No GPU is not an error: [`GpuDevice::request`] returns `None` and the app steps on the CPU as it
//! always has. A machine with no GPU driver at all (a CI runner, a remote session) can still have
//! one through a software Vulkan driver such as Mesa's `lavapipe`, which `wgpu` finds like any
//! other; it is slow, and it is how CI tests the GPU path.

use crate::compute::constants::GPU_PHYSICS_ENV;

/// A GPU device and its queue, ready for compute work.
pub struct GpuDevice {
    /// The device: buffers, pipelines and command encoders are made from it.
    pub device: wgpu::Device,
    /// The queue work is submitted on.
    pub queue: wgpu::Queue,
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
                required_features: wgpu::Features::empty(),
                // Everything the adapter offers: the particle buffers grow with the detail
                // setting, and the defaults cap a storage buffer at 128 MiB.
                required_limits: adapter.limits(),
            },
            None,
        ))
        .ok()?;
        // `wgpu` panics on an error nothing handles, which would take the app down with the GPU.
        // Log it instead, and remember, so the caller falls back to the CPU.
        let failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let on_error = failed.clone();
        device.on_uncaptured_error(Box::new(move |error| {
            bevy::log::error!("physics GPU error, falling back to the CPU: {error}");
            on_error.store(true, std::sync::atomic::Ordering::Relaxed);
        }));
        let on_lost = failed.clone();
        device.set_device_lost_callback(move |reason, message| {
            // Dropping the device, as the app does when it exits, also reports it lost.
            if matches!(reason, wgpu::DeviceLostReason::Dropped) {
                return;
            }
            bevy::log::error!("physics GPU lost ({reason:?}), falling back to the CPU: {message}");
            on_lost.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        Some(Self {
            device,
            queue,
            info,
            failed,
        })
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
