//! Tests for [`crate::performance`]'s systems, `#[path]`-included into `src/performance/mod.rs`:
//! a change of the video settings (as the menu makes) reaching the renderer's resources.

use super::*;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(PerformanceSettings::default());
    app.insert_resource(Msaa::Sample4);
    app.add_systems(Update, apply_performance_settings_system);
    app.world_mut().spawn(Camera3d::default());
    app.update();
    app
}

#[test]
fn a_new_anti_aliasing_mode_takes_effect_while_running() {
    let mut app = app();

    app.world_mut()
        .resource_mut::<PerformanceSettings>()
        .anti_aliasing = AntiAliasing::Smaa;
    app.update();
    assert_eq!(
        *app.world().resource::<Msaa>(),
        Msaa::Off,
        "SMAA runs without MSAA under it"
    );
    let mut cameras = app
        .world_mut()
        .query_filtered::<Entity, With<SmaaSettings>>();
    assert_eq!(cameras.iter(app.world()).count(), 1);

    app.world_mut()
        .resource_mut::<PerformanceSettings>()
        .anti_aliasing = AntiAliasing::Msaa2;
    app.update();
    assert_eq!(*app.world().resource::<Msaa>(), Msaa::Sample2);
    let mut cameras = app
        .world_mut()
        .query_filtered::<Entity, With<SmaaSettings>>();
    assert_eq!(
        cameras.iter(app.world()).count(),
        0,
        "the post-process pass is removed"
    );
}
