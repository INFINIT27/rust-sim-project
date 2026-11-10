//! Performance: the switches that trade image quality or fidelity for frame time, and the
//! benchmark that measures them.
//!
//! Everything is chosen by environment variable when the app starts, so a configuration can be
//! compared against another without a rebuild:
//!
//! | Variable | Values | Default | Effect |
//! | --- | --- | --- | --- |
//! | `RGA_AA` | `msaa4`, `msaa2`, `smaa`, `fxaa`, `off` | `msaa4` | [`settings::AntiAliasing`] |
//! | `RGA_FLOOR_ECLIPSE` | `on`, `off` | `on` | eclipse shadows on the floor slab |
//! | `RGA_VSYNC` | `on`, `off` | `on` | present mode; forced off while benchmarking |
//! | `RGA_BENCH` | `on`, `off` | `off` | run [`bench`], print one `BENCH` line, exit |
//! | `RGA_BENCH_WARMUP`, `RGA_BENCH_SECONDS` | seconds | `3`, `10` | benchmark timing |
//!
//! The defaults are the shipped configuration and live in [`constants`], each with the
//! measurement behind it. `src/performance/README.md` records the full set of measurements.
//!
//! [`PerformancePlugin`] applies the settings; `main.rs` builds them with
//! [`PerformanceSettings::from_env`] before the `App` exists, because vsync is fixed when the window
//! is created.

pub mod bench;
pub mod constants;
pub mod settings;

use bevy::core_pipeline::fxaa::Fxaa;
use bevy::core_pipeline::smaa::{SmaaPreset, SmaaSettings};
use bevy::prelude::*;

#[cfg(test)]
#[path = "../../tst/performance/floor_eclipse_tests.rs"]
mod floor_eclipse_tests;

#[cfg(test)]
#[path = "../../tst/performance/plugin_tests.rs"]
mod plugin_tests;

pub use settings::{AntiAliasing, BenchConfig, PerformanceSettings, SettingsError};

/// Applies a [`PerformanceSettings`]: inserts it as a resource, sets MSAA, puts the post-process
/// anti-aliasing on every 3D camera, and runs the benchmark when it is on.
///
/// Systems that read the settings take `Option<Res<PerformanceSettings>>` and use
/// [`PerformanceSettings::default`] when it is absent, so a headless test app without this plugin
/// behaves like the shipped one.
pub struct PerformancePlugin {
    settings: PerformanceSettings,
}

impl PerformancePlugin {
    pub fn new(settings: PerformanceSettings) -> Self {
        Self { settings }
    }
}

impl Plugin for PerformancePlugin {
    fn build(&self, app: &mut App) {
        let settings = self.settings;
        app.insert_resource(settings)
            .insert_resource(settings.anti_aliasing.msaa())
            .add_systems(
                Update,
                (
                    post_process_anti_aliasing_system,
                    apply_performance_settings_system,
                ),
            );

        if let Some(config) = settings.bench {
            app.insert_resource(bench::BenchRun::new(config))
                .add_systems(Last, bench::record_frame_system);
        }
    }
}

/// Add SMAA or FXAA to each 3D camera when it appears. A query on `Added` rather than a startup
/// system, so a camera spawned later is covered too.
pub fn post_process_anti_aliasing_system(
    mut commands: Commands,
    settings: Res<PerformanceSettings>,
    cameras: Query<Entity, Added<Camera3d>>,
) {
    for camera in &cameras {
        match settings.anti_aliasing {
            AntiAliasing::Smaa => {
                commands.entity(camera).insert(SmaaSettings {
                    preset: SmaaPreset::High,
                });
            }
            AntiAliasing::Fxaa => {
                commands.entity(camera).insert(Fxaa::default());
            }
            AntiAliasing::Msaa4 | AntiAliasing::Msaa2 | AntiAliasing::Off => {}
        }
    }
}

/// Re-apply the video settings whenever they change, as the menu's video page changes them:
/// the MSAA sample count, the post-process anti-aliasing on every camera, and the window's vsync.
/// [`post_process_anti_aliasing_system`] covers a camera spawned later.
pub fn apply_performance_settings_system(
    mut commands: Commands,
    settings: Res<PerformanceSettings>,
    msaa: Option<ResMut<Msaa>>,
    cameras: Query<Entity, With<Camera3d>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if !settings.is_changed() {
        return;
    }
    if let Some(mut msaa) = msaa {
        let wanted = settings.anti_aliasing.msaa();
        if *msaa != wanted {
            *msaa = wanted;
        }
    }
    for camera in &cameras {
        let mut camera = commands.entity(camera);
        camera.remove::<SmaaSettings>().remove::<Fxaa>();
        match settings.anti_aliasing {
            AntiAliasing::Smaa => {
                camera.insert(SmaaSettings {
                    preset: SmaaPreset::High,
                });
            }
            AntiAliasing::Fxaa => {
                camera.insert(Fxaa::default());
            }
            AntiAliasing::Msaa4 | AntiAliasing::Msaa2 | AntiAliasing::Off => {}
        }
    }
    let present_mode = settings.present_mode();
    for mut window in &mut windows {
        if window.present_mode != present_mode {
            window.present_mode = present_mode;
        }
    }
}
