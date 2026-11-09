//! Integration test: headless app tick.
//!
//! Task 11.1 from the original spec's task list:
//!
//! > Build a `bevy::App` with `MinimalPlugins` + `ScenePlugin`, run
//! > `app.update()` a few ticks, and assert:
//! >
//! > - The number of entities matching `With<MirroredBody>` equals
//! >   `scene.props.len()` (Property 10 — validates Requirement 3.5).
//! > - For every entity with `(&MirroredBody(id), &Transform)`,
//! >   `transform.translation` equals
//! >   `transform_from(scene.get(id).unwrap()).translation` within
//! >   `1e-5` tolerance (validates Requirement 9.4).
//! > - `scene.props.len()` is unchanged across the ticks (validates
//! >   Requirements 5.4 and 9.3).
//!
//! This exercises the full section-7 wiring (startup spawn + per-frame
//! `step → sync`) end-to-end without touching Bevy's render pipeline,
//! so it runs on CI without a GPU. `MinimalPlugins` provides the
//! `Startup`/`Update` schedules and the `Time` resource; the asset
//! collections needed by `spawn_scene_entities` are inserted manually
//! rather than pulling in `DefaultPlugins` (which would require a
//! windowing stack and a GPU adapter).

use bevy::app::App;
use bevy::asset::Assets;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::math::Vec3;
use bevy::render::mesh::Mesh;
use bevy::transform::components::Transform;
use bevy::MinimalPlugins;

use physics_sandbox::{transform_from, BodyId, BodyMaterial, MirroredBody, Scene, ScenePlugin};

/// Build a headless `App` configured the way task 11.1 specifies:
/// `MinimalPlugins` + `ScenePlugin`, plus the two asset resources
/// [`spawn_scene_entities`](physics_sandbox::spawn_scene_entities)
/// consumes. `MinimalPlugins` doesn't ship `Assets<Mesh>` /
/// `Assets<BodyMaterial>`, so we `init_resource` them directly
/// rather than pulling in `DefaultPlugins` — the point of the headless
/// test is to avoid the render pipeline entirely.
fn build_headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::default());
    app
}

/// Task 11.1: after a few `Update` ticks, the mirror invariants that
/// connect the pure-Rust `Scene` to the ECS mirror must all hold:
///
/// - **Property 10 / Requirement 3.5**: one `MirroredBody` entity per
///   body in `Scene::bodies`.
/// - **Requirement 9.4**: every mirrored entity's
///   `Transform.translation` equals
///   `transform_from(scene.get(id).unwrap()).translation` within
///   `1e-5` absolute tolerance.
/// - **Requirements 5.4 + 9.3**: `scene.props.len()` is invariant
///   across the ticks (nothing in the per-frame pipeline removes or
///   adds bodies today).
///
/// Five ticks is enough to cover more than the first `dt == 0.0`
/// frame while keeping the test fast.
#[test]
fn headless_app_tick_preserves_mirror_invariants() {
    let mut app = build_headless_app();

    // Snapshot `bodies.len()` before ticking: Requirements 5.4 and 9.3
    // together require this count to be invariant across the ticks, so
    // we pin it now and compare after. `Scene::demo()` populates the
    // resource at `ScenePlugin::build` time (before the first tick), so
    // this read is well-defined.
    let body_count_before = app.world().resource::<Scene>().props.len();

    // Run a few `Update` ticks. The `Startup` schedule fires once on
    // the first `app.update()` (spawning one `MirroredBody` per body),
    // and every tick thereafter runs `step_scene_system` followed by
    // `sync_scene_to_ecs_system` exactly once, per Requirement 2.1.
    for _ in 0..5 {
        app.update();
    }

    // --------------------------------------------------------------
    // Requirement 3.5 (Property 10): one MirroredBody per body.
    //
    // Use a `With<MirroredBody>` filter on `Entity` rather than
    // fetching the component itself — we only care about cardinality
    // at this step.
    // --------------------------------------------------------------
    let body_count_after = app.world().resource::<Scene>().props.len();
    let mirrored_count = app
        .world_mut()
        .query_filtered::<Entity, With<MirroredBody>>()
        .iter(app.world())
        .count();
    assert_eq!(
        mirrored_count, body_count_after,
        "Requirement 3.5: expected one MirroredBody entity per body \
         (scene.props.len() = {}, MirroredBody entities = {})",
        body_count_after, mirrored_count,
    );

    // --------------------------------------------------------------
    // Requirements 5.4 + 9.3: body count invariant across the ticks.
    //
    // `Scene::step` is a no-op today (Requirement 5.2) and
    // `sync_scene_to_ecs_system` takes `Res<Scene>` (Requirement 9.3),
    // so neither system can grow or shrink `Scene::bodies`. This
    // assertion guards against a future refactor that breaks either.
    // --------------------------------------------------------------
    assert_eq!(
        body_count_after, body_count_before,
        "Requirements 5.4 + 9.3: scene.props.len() must be invariant across ticks \
         (before: {}, after: {})",
        body_count_before, body_count_after,
    );

    // --------------------------------------------------------------
    // Requirement 9.4: every mirrored entity's transform.translation
    // matches transform_from(scene.get(id).unwrap()).translation within
    // 1e-5.
    //
    // We collect `(id, translation)` pairs into an owned `Vec` first
    // so the ECS query's borrow of `app.world()` releases before we
    // re-borrow `app.world()` to resolve each id against `Scene`.
    // Without that split, the second borrow would conflict with the
    // first and fail to compile.
    // --------------------------------------------------------------
    let pairs: Vec<(BodyId, Vec3)> = app
        .world_mut()
        .query::<(&MirroredBody, &Transform)>()
        .iter(app.world())
        .map(|(MirroredBody(id), t)| (*id, t.translation))
        .collect();

    let scene = app.world().resource::<Scene>();
    for (id, entity_translation) in pairs {
        let prop = scene.get(id).unwrap_or_else(|| {
            panic!(
                "MirroredBody({:?}) did not resolve in Scene — the sync contract \
                 assumes every mirrored entity has a live prop (Requirement 9.1 / 9.4)",
                id,
            )
        });
        let expected = transform_from(&prop.body).translation;
        assert!(
            entity_translation.abs_diff_eq(expected, 1.0e-5),
            "Requirement 9.4: translation {:?} deviates from \
             transform_from({:?}).translation {:?} beyond 1e-5 for id {:?}",
            entity_translation,
            prop.body,
            expected,
            id,
        );
    }
}

/// The app must launch **full-screen**, and the windowed fallback must stay usable.
///
/// `main.rs` is the only reader of these, and it cannot be tested through an `App` without a
/// windowing stack and a GPU — so this pins the two facts that matter about the window
/// configuration where a headless run can still see them.
///
/// The mode is borderless rather than exclusive full-screen on purpose: it keeps the
/// display's current resolution and refresh rate, so there is no mode switch on launch. See
/// `WINDOW_MODE` and amendment A12.
#[test]
fn the_window_starts_full_screen_with_a_usable_windowed_fallback() {
    use bevy::window::WindowMode;
    use physics_sandbox::{WINDOW_HEIGHT_PX, WINDOW_MODE, WINDOW_TITLE, WINDOW_WIDTH_PX};

    assert_eq!(
        WINDOW_MODE,
        WindowMode::BorderlessFullscreen,
        "the app is meant to open full-screen without switching display modes"
    );

    // Not dead weight: this is the size the window takes the moment `WINDOW_MODE` goes back
    // to `Windowed`, and the aspect ratio every camera-framing constant was derived against.
    // Only the ratio is asserted — clippy rejects a comparison of two constants to zero as an
    // assertion with a constant value, and it is right that it says nothing.
    assert!(
        (WINDOW_WIDTH_PX / WINDOW_HEIGHT_PX - 4.0 / 3.0).abs() < 1.0e-6,
        "the framing constants assume 4:3 when windowed"
    );

    assert!(
        !WINDOW_TITLE.is_empty(),
        "a window list needs something to show"
    );
}
