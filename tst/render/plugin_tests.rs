//! Unit and property tests for the [`crate::render::plugin`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/render/plugin.rs`
//! via `#[path]`, not as a standalone integration test, so these tests keep
//! access to the module's private items and to `super::*`.

//! Scaffolding-level tests for task 7.1.
//!
//! These exercise the three public types in isolation so regressions
//! in later tasks (which will add real behaviour to `ScenePlugin`)
//! don't silently break the type surface that section-7 tasks
//! depend on.

use super::*;
use crate::constants::TWO_BODY_TIME_SCALE;
use crate::core::scene::{BodyId, DVec3};
use crate::experiments::catalog::{ExperimentId, ExperimentSpec};
use crate::render::eclipse::BodyMaterial;
use bevy::app::App;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// A default `SimulationClock` must cap a frame step at `0.1 s` per Requirement 13.5
/// and run at real time — the two values `SceneConfig` used to hold before amendment
/// A3 moved them onto the clock.
#[test]
fn the_default_clock_caps_at_point_one_and_runs_real_time() {
    let clock = SimulationClock::default();
    assert_eq!(clock.max_real_step_seconds(), 0.1);
    assert_eq!(
        clock.scale(),
        1.0,
        "the default must be real time even though the shipped scene overrides it"
    );
}

/// `ScenePlugin` must insert the *tuned* time scale, not the default.
///
/// The two-body scene is unwatchable at real time — the spheres take about 6.5
/// years to fall together — so the plugin deliberately overrides `time_scale`.
/// Pinned because a regression here would look like a physics bug: the pair would
/// appear frozen rather than falling.
///
/// The scale is *greater* than one, because astronomical gravitation is too slow
/// rather than too fast. An earlier close-pair configuration needed a scale far
/// below one, so the assertion checks the value rather than the direction — and
/// the check that the shipped scale is not accidentally `1.0` is what catches a
/// silently-failed edit to the constant.
#[test]
fn scene_plugin_inserts_the_tuned_time_scale() {
    let mut app = App::new();
    app.add_plugins(ScenePlugin::default());

    let clock = *app.world().resource::<SimulationClock>();
    assert_eq!(clock.scale(), TWO_BODY_TIME_SCALE);
    assert_ne!(
        clock.scale(),
        SimulationClock::default().scale(),
        "the plugin must override the real-time default"
    );

    // One real frame must advance simulated time by a meaningful fraction of the
    // free-fall time: enough to see, few enough steps to stay stable. 40 seconds of
    // watching at 60 fps is ~2400 steps.
    let frame_dt = 1.0 / 60.0;
    let steps = Scene::two_body_free_fall_seconds() / f64::from(frame_dt * clock.scale());
    assert!(
        (1000.0..10_000.0).contains(&steps),
        "the shipped scale gives {} steps over the approach; aim for a few thousand",
        steps
    );
}

/// The shipped scene is the two-body gravitation scene: floor plus two spheres.
#[test]
fn scene_plugin_inserts_the_two_body_scene() {
    let mut app = App::new();
    app.add_plugins(ScenePlugin::default());

    let scene = app.world().resource::<Scene>();
    assert_eq!(scene.props.len(), 3, "container plane plus two spheres");
    assert!(scene.props[0].immovable, "the first prop is the floor");
    assert!(
        scene.props[1..].iter().all(|p| p.responds_to_forces()),
        "both spheres must be movable and massive"
    );
}

/// `MirroredBody` is a transparent wrapper around `BodyId`; round-
/// tripping an id through the component must preserve equality so
/// `sync_scene_to_ecs_system` (task 7.5) can match entities back to
/// their `Scene` bodies by id.
#[test]
fn mirrored_body_wraps_body_id() {
    let id = BodyId(42);
    let m = MirroredBody(id);
    assert_eq!(m.0, id);

    // Copy/Clone/PartialEq round-trip for safe use in ECS queries.
    let m2 = m;
    assert_eq!(m, m2);
}

/// `ScenePlugin::build` inserts the `Scene`, a `SimulationClock`, and an
/// `ObserverClock`. Adding the plugin to a fresh `App` must therefore leave all
/// three present, with the clock capping a frame step at `0.1 s` (Requirement 13.5)
/// and a non-empty `Scene::props` (Requirements 3.1, 12.1).
#[test]
fn scene_plugin_build_inserts_resources() {
    let mut app = App::new();
    app.add_plugins(ScenePlugin::default());

    // Requirement 13.5: the simulation clock is present and caps a step at 0.1 s.
    let clock = app
        .world()
        .get_resource::<SimulationClock>()
        .expect("ScenePlugin::build must insert a SimulationClock");
    assert_eq!(clock.max_real_step_seconds(), 0.1);
    assert!(!clock.is_paused(), "the experiment must start running");

    // And the observer clock, which the camera and overlay read instead.
    app.world()
        .get_resource::<ObserverClock>()
        .expect("ScenePlugin::build must insert an ObserverClock");

    // And the experiment's spec, which the startup camera and the overlay read to know
    // *which* experiment is running (amendment A4).
    let spec = app
        .world()
        .get_resource::<ExperimentSpec>()
        .expect("ScenePlugin::build must insert an ExperimentSpec");
    assert_eq!(
        spec.id,
        ExperimentId::default(),
        "ScenePlugin::default() must run the default experiment"
    );
    assert!(spec.is_valid());
    assert_eq!(
        clock.scale(),
        spec.time_scale,
        "the clock must be built at the experiment's scale, not a constant"
    );

    // Requirement 3.1: Scene::demo present as a resource, non-empty.
    let scene = app
        .world()
        .get_resource::<Scene>()
        .expect("ScenePlugin::build must insert Scene::demo() as a resource");
    assert!(
        !scene.props.is_empty(),
        "ScenePlugin::build must insert Scene::demo() (non-empty)"
    );
}

/// `ScenePlugin::build` registers the per-frame schedule so that a
/// single `app.update()` tick runs `step_scene_system` followed by
/// `sync_scene_to_ecs_system` (Requirement 2.1). This test does not
/// directly observe the ordering — Bevy's scheduler enforces it —
/// but it does confirm that one tick completes without panicking,
/// that `Scene::bodies` is unchanged across the tick (the current
/// `Scene::step` is a no-op placeholder, Requirement 5.2), and that
/// the startup system seeded one `MirroredBody` per body in
/// `Scene::demo()` (Requirement 3.5).
#[test]
fn scene_plugin_build_registers_startup_and_update_systems() {
    use bevy::MinimalPlugins;

    let mut app = App::new();
    // MinimalPlugins provides the schedules (`Startup`, `Update`)
    // and the `Time` resource `step_scene_system` needs. We also
    // need the `AssetPlugin` so that `spawn_scene_entities`'
    // `ResMut<Assets<Mesh>>` and `ResMut<Assets<BodyMaterial>>`
    // resolve — those resources don't ship with MinimalPlugins, so
    // insert them manually here rather than pulling in DefaultPlugins.
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::default());

    // Snapshot the scene so we can compare after the tick.
    let scene_before = app.world().resource::<Scene>().clone();

    // One tick: Startup runs spawn_scene_entities, then Update
    // runs step_scene_system -> sync_scene_to_ecs_system.
    app.update();

    // Requirement 3.5: one MirroredBody per body in the scene.
    let mirrored_count = app
        .world_mut()
        .query::<&MirroredBody>()
        .iter(app.world())
        .count();
    assert_eq!(
        mirrored_count,
        scene_before.props.len(),
        "expected one MirroredBody entity per body in Scene::demo()"
    );

    // Requirement 5.2: step(dt ≈ 0) is a no-op, so bodies are
    // unchanged across the tick. This also confirms
    // sync_scene_to_ecs_system did not mutate `Scene`
    // (Requirement 9.3).
    let scene_after = app.world().resource::<Scene>();
    assert_eq!(scene_after.props, scene_before.props);
    assert_eq!(scene_after.bounds, scene_before.bounds);
}

// -----------------------------------------------------------------------
// Property 9: Render bridge is pure w.r.t. Scene (task 7.8).
//
// **Validates: Requirements 9.3**
//
// `sync_scene_to_ecs_system` runs every `Update` tick alongside
// `step_scene_system`. Requirement 9.3 forbids the render bridge
// from mutating `Scene`; the type signature `Res<Scene>` already
// enforces this at compile time, but a regression test against a
// real `App` catches future refactors that might widen the
// parameter to `ResMut<Scene>` or add a sibling system that does.
// Running *several* ticks (rather than one) makes the assertion
// independent of any first-tick idiosyncrasies (e.g. Bevy's initial
// `Time::delta_seconds() == 0.0`).
// -----------------------------------------------------------------------

/// Property 9: Render bridge is pure w.r.t. Scene.
///
/// **Validates: Requirements 9.3**
///
/// Build a Bevy `App` with `MinimalPlugins` + `ScenePlugin`, run a
/// few `app.update()` ticks, and assert the `Scene` resource's
/// `bodies` and `bounds` before and after are identical. The clone
/// taken before the ticks gives us an independent snapshot so
/// `assert_eq!` can compare against the post-tick state.
#[test]
fn render_bridge_does_not_mutate_scene() {
    use bevy::MinimalPlugins;

    let mut app = App::new();
    // MinimalPlugins supplies the `Startup`/`Update` schedules and
    // the `Time` resource; `Assets<Mesh>` and
    // `Assets<BodyMaterial>` are required by
    // `spawn_scene_entities` but are not included in MinimalPlugins,
    // so insert them explicitly rather than pulling DefaultPlugins.
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::default());

    // Snapshot the full Scene state before any ticks run. `Scene`
    // derives `Clone`, so this is an independent copy we can
    // compare against after multiple ticks.
    let scene_before = app.world().resource::<Scene>().clone();

    // A handful of ticks: enough to cover more than the first
    // `dt == 0.0` frame, few enough to keep the test fast.
    for _ in 0..5 {
        app.update();
    }

    // Requirement 9.3: `Scene` is untouched by the render bridge.
    //
    // This assertion had to change when `Scene::step` stopped being a no-op. It
    // used to compare `props` before and after, which worked only because
    // *nothing* mutated the scene; now the integrator legitimately moves every
    // movable prop every tick, so equality is the wrong claim.
    //
    // The mutation-freedom of `sync_scene_to_ecs_system` is in fact enforced at
    // compile time by its `Res<Scene>` (not `ResMut`) parameter, and no runtime
    // test can add to that. What is asserted here instead is everything the
    // *structure* of the scene guarantees across a tick, which sync would break if
    // it ever gained write access: the prop count, the id set, the bounds, the
    // immovable floor, and every prop's validity.
    let scene_after = app.world().resource::<Scene>();

    assert_eq!(
        scene_after.props.len(),
        scene_before.props.len(),
        "a tick must not add or remove props"
    );
    assert_eq!(
        scene_after.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        scene_before.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        "a tick must not change the id set or its order"
    );
    assert_eq!(
        scene_after.bounds, scene_before.bounds,
        "sync_scene_to_ecs_system must not mutate Scene::bounds"
    );
    assert_eq!(
        scene_after.props[0], scene_before.props[0],
        "the immovable container plane must be untouched"
    );
    for prop in &scene_after.props {
        assert!(
            prop.is_valid(),
            "every prop must still satisfy the Validation_Rules: {:?}",
            prop
        );
    }
}

// -----------------------------------------------------------------------
// Property 10: Entity-count equals body-count (task 7.9).
//
// **Validates: Requirements 3.5**
//
// After `spawn_scene_entities` has run on the `Startup` schedule,
// the number of entities carrying `MirroredBody` must equal
// `scene.props.len()`. One `app.update()` tick is sufficient
// because Bevy runs `Startup` once before the first `Update` tick.
// -----------------------------------------------------------------------

/// Property 10: Entity-count equals body-count.
///
/// **Validates: Requirements 3.5**
///
/// Build a Bevy `App` with `MinimalPlugins` + `ScenePlugin`, run
/// `app.update()` once (which triggers the `Startup` schedule's
/// `spawn_scene_entities` before the first `Update` tick), and
/// assert that the number of entities matching `With<MirroredBody>`
/// equals `scene.props.len()`.
#[test]
fn entity_count_equals_body_count() {
    use bevy::ecs::entity::Entity;
    use bevy::ecs::query::With;
    use bevy::MinimalPlugins;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::default());

    // Snapshot `bodies.len()` before the tick so the assertion's
    // expected side is pinned to the pre-`Update` scene. `Scene`'s
    // `bodies` cannot grow or shrink during a normal tick
    // (Requirements 5.2 + 9.3), so this value matches the post-tick
    // `scene.props.len()` too — but snapshotting before is what
    // the task description specifies.
    let expected = app.world().resource::<Scene>().props.len();

    // One tick: `Startup` runs `spawn_scene_entities`, then
    // `Update` runs `step_scene_system` followed by
    // `sync_scene_to_ecs_system`.
    app.update();

    // Requirement 3.5: after startup, the count of `MirroredBody`
    // entities equals `scene.props.len()`. Using a `With<MirroredBody>`
    // filter (rather than fetching the component itself) keeps the
    // query minimal — we only care about cardinality here.
    let mirrored_count = app
        .world_mut()
        .query_filtered::<Entity, With<MirroredBody>>()
        .iter(app.world())
        .count();
    assert_eq!(
        mirrored_count, expected,
        "expected {} MirroredBody entities (one per body in Scene::demo()), got {}",
        expected, mirrored_count,
    );
}

// -----------------------------------------------------------------------
// Unit spot-checks for `clamp_dt` (task 7.2).
//
// These pin the three branches of the contract to concrete values so
// a regression shows up even if the property-test strategies below
// ever drift. Each test names the clause of Requirement 13 it covers.
// -----------------------------------------------------------------------

/// Requirement 13.1: an in-range, non-negative `raw` is returned
/// unchanged.
#[test]
fn clamp_dt_passes_through_in_range() {
    assert_eq!(clamp_dt(0.0, 0.1), 0.0);
    assert_eq!(clamp_dt(0.05, 0.1), 0.05);
    assert_eq!(clamp_dt(0.1, 0.1), 0.1);
}

/// Requirement 13.2: `raw > max_dt` saturates at `max_dt`.
#[test]
fn clamp_dt_saturates_above_max() {
    assert_eq!(clamp_dt(0.5, 0.1), 0.1);
    assert_eq!(clamp_dt(1.0e6, 0.1), 0.1);
}

/// Requirement 13.3: negative or non-finite inputs collapse to `0.0`.
#[test]
fn clamp_dt_rejects_negative_and_non_finite() {
    assert_eq!(clamp_dt(-0.0001, 0.1), 0.0);
    assert_eq!(clamp_dt(-1.0e6, 0.1), 0.0);
    assert_eq!(clamp_dt(f32::NAN, 0.1), 0.0);
    assert_eq!(clamp_dt(f32::INFINITY, 0.1), 0.0);
    assert_eq!(clamp_dt(f32::NEG_INFINITY, 0.1), 0.0);
}

// -----------------------------------------------------------------------
// Property test for `clamp_dt` (task 7.3, Property 11).
// -----------------------------------------------------------------------

/// Strategy: an arbitrary `f32`, deliberately including `NaN`,
/// `+/-inf`, subnormals, and both signs. `any::<f32>()` from
/// proptest's built-in strategies covers all of these, which is the
/// whole point of the frame-delta clamp — the raw timer can hand us
/// anything.
fn any_f32() -> impl Strategy<Value = f32> {
    any::<f32>()
}

/// Strategy: a strictly-positive, finite `max_dt`.
///
/// Lower-bounded away from zero (Requirement 13.5 forbids
/// non-positive `max_dt`) and upper-bounded well below `f32::MAX`
/// so we never stumble across values that a future refactor might
/// treat as "not really finite" on an exotic platform.
fn positive_max_dt() -> impl Strategy<Value = f32> {
    1.0e-6_f32..=1.0e6_f32
}

/// Where proptest writes the seeds of failures it has seen.
///
/// Pinned explicitly rather than left to proptest's default, which derives the
/// location from `file!()`. Because this module is pulled into its source file
/// by `#[path]`, `file!()` contains `../..` components that proptest does not
/// normalise, so the default resolves to a bogus directory inside `src/`.
///
/// The path is relative to the package root, which is Cargo's working directory
/// for tests. Commit the seeds it produces: they are the counterexamples this
/// suite has already paid for.
const REGRESSION_SEEDS: &str = "tst/render/proptest-regressions/plugin_tests.txt";

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Property 11: Frame-delta clamp.
    ///
    /// **Validates: Requirements 13.1, 13.2, 13.3, 13.4**
    ///
    /// For any `raw: f32` (including `NaN`, `+/-inf`, negatives, and
    /// huge values) and any strictly-positive finite `max_dt`, the
    /// value returned by `clamp_dt(raw, max_dt)` must:
    ///
    /// - be finite and lie in the closed interval `[0.0, max_dt]`
    ///   (Requirement 13.4),
    /// - equal `0.0` iff `raw` is not finite or `raw < 0.0`
    ///   (Requirement 13.3),
    /// - equal `max_dt` iff `raw` is finite and `raw > max_dt`
    ///   (Requirement 13.2), and
    /// - equal `raw` otherwise (Requirement 13.1).
    #[test]
    fn clamp_dt_matches_specified_branches(
        raw in any_f32(),
        max_dt in positive_max_dt(),
    ) {
        let out = clamp_dt(raw, max_dt);

        // Requirement 13.4: always finite and in [0.0, max_dt].
        prop_assert!(
            out.is_finite(),
            "clamp_dt({}, {}) = {} is not finite",
            raw,
            max_dt,
            out,
        );
        prop_assert!(
            (0.0..=max_dt).contains(&out),
            "clamp_dt({}, {}) = {} is outside [0.0, {}]",
            raw,
            max_dt,
            out,
            max_dt,
        );

        // Now pin the exact branch — one of the three cases must
        // hold, and the other two must not. We do this by computing
        // the expected value the same way the spec's prose reads and
        // asserting bit-exact equality with the actual output.
        let expected = if !raw.is_finite() || raw < 0.0 {
            0.0
        } else if raw > max_dt {
            max_dt
        } else {
            raw
        };
        prop_assert_eq!(
            out,
            expected,
            "clamp_dt({}, {}) = {} but the spec demands {}",
            raw,
            max_dt,
            out,
            expected,
        );
    }
}

// -----------------------------------------------------------------------
// Smoke test for `step_scene_system` (task 7.4).
//
// The full per-frame schedule wiring lands in task 7.7; here we only
// confirm that `step_scene_system` runs against Bevy's `Time`
// resource without panicking and leaves the `Scene` in a valid
// state. `Scene::step` is a no-op placeholder today so the observable
// postcondition is simply "bodies are still valid after the tick".
// -----------------------------------------------------------------------

/// Running `step_scene_system` once on a fresh `App` with a
/// `Scene::demo()` + default `SceneConfig` + `bevy::time::TimePlugin`
/// must leave every body in `Scene::props` valid (Requirement 5.1)
/// and preserve `bounds` (Requirement 5.3). The clock only ever hands out a valid
/// `TimeStep`, so `Scene::step`'s precondition cannot fire from this code path — the
/// type now carries what the clamp used to have to guarantee.
#[test]
fn step_scene_system_runs_without_panic() {
    use crate::core::scene::Scene;
    use bevy::time::TimePlugin;

    let mut app = App::new();
    app.add_plugins(TimePlugin);
    app.insert_resource(Scene::demo());
    app.init_resource::<SimulationClock>();
    app.init_resource::<ObserverClock>();
    app.add_systems(
        bevy::app::Update,
        (
            crate::render::plugin::advance_clocks_system,
            step_scene_system.after(crate::render::plugin::advance_clocks_system),
        ),
    );

    // One tick is enough to exercise the system with a real `Time`
    // resource. The first tick's delta is `0.0`, which is explicitly
    // required to be a no-op (Requirement 5.2), so the scene must be
    // identical to `Scene::demo()` after the call.
    let scene_before = app.world().resource::<Scene>().clone();
    app.update();
    let scene_after = app.world().resource::<Scene>();

    assert_eq!(
        scene_after.bounds, scene_before.bounds,
        "step_scene_system must not mutate Scene::bounds"
    );
    for (i, body) in scene_after.props.iter().enumerate() {
        assert!(
            body.is_valid(),
            "step_scene_system left body {} invalid: {:?}",
            i,
            body,
        );
    }
}

/// The camera must be able to *see* the experiment it is pointed at.
///
/// Bevy's default perspective projection clips at `1000` units. That was invisible while
/// every scene was tens of units across, and it silently deletes more than half of
/// `experiment_3`: Saturn starts `1433` units out, Uranus `2872`, Neptune `4495`. This
/// pins the far plane against the outermost thing the crate can put in front of it —
/// the floor's far corner, which is further than any planet.
#[test]
fn the_startup_camera_can_see_the_whole_solar_system() {
    use bevy::render::camera::Projection;
    use bevy::MinimalPlugins;

    use crate::core::constants::DEMO_FLOOR_HALF_SIZE_UNITS;
    use crate::experiments::selection::Selection;
    use crate::render::constants::CAMERA_FAR_UNITS;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::new(Selection {
        id: ExperimentId::SolarSystem,
        seed: None,
    }));
    app.update();

    let far = app
        .world_mut()
        .query::<&Projection>()
        .iter(app.world())
        .filter_map(|projection| match projection {
            Projection::Perspective(perspective) => Some(perspective.far),
            Projection::Orthographic(_) => None,
        })
        .next()
        .expect("the startup camera must have a perspective projection");

    assert_eq!(far, CAMERA_FAR_UNITS);

    // Enough to reach the far corner of the floor from the opposite corner — the longest
    // sight line the crate can produce — so nothing an experiment places can be clipped.
    let floor_diagonal = 2.0 * DEMO_FLOOR_HALF_SIZE_UNITS * 2.0_f32.sqrt();
    assert!(
        far > floor_diagonal,
        "the far plane ({far}) must clear the floor's diagonal ({floor_diagonal})"
    );
}

#[test]
fn the_camera_lock_can_bring_io_close_enough_to_fill_the_view() {
    use bevy::render::camera::Projection;
    use bevy::MinimalPlugins;

    use crate::constants::METRES_PER_UNIT;
    use crate::experiments::constants::JOVIAN_MOONS;
    use crate::experiments::selection::Selection;
    use crate::input::camera::orbit_distance_bounds;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::new(Selection {
        id: ExperimentId::SolarSystem,
        seed: None,
    }));
    app.update();

    let perspective = app
        .world_mut()
        .query::<&Projection>()
        .iter(app.world())
        .find_map(|projection| match projection {
            Projection::Perspective(perspective) => Some(perspective.clone()),
            Projection::Orthographic(_) => None,
        })
        .expect("the startup camera must have a perspective projection");

    // Io is the smallest body anyone inspects. Bodies are drawn at true size, so only getting close
    // makes it large, and the near plane sets how close the lock may get.
    let io_radius = (JOVIAN_MOONS[0].radius_m / METRES_PER_UNIT) as f32;
    let (closest, _) = orbit_distance_bounds(io_radius, perspective.near, perspective.far);
    let angular_radius = (io_radius / closest).asin();

    assert!(
        angular_radius > 0.5 * perspective.fov,
        "Io subtends {angular_radius} rad from the closest orbit, under the view's half-height of {}",
        0.5 * perspective.fov
    );
}

/// Every experiment in the catalog must be runnable through the plugin, not just
/// buildable.
///
/// The catalog tests check that a spec is valid; this checks the plugin actually accepts
/// one and spawns a body per prop. A new experiment that broke the render path — an
/// unsupported `Body` variant, say — would pass there and fail here.
#[test]
fn the_plugin_spawns_every_catalog_experiment() {
    use bevy::MinimalPlugins;

    use crate::experiments::selection::Selection;

    for id in ExperimentId::ALL {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<BodyMaterial>>();
        app.add_plugins(ScenePlugin::new(Selection { id, seed: Some(7) }));

        let expected = app.world().resource::<Scene>().props.len();
        app.update();

        let mirrored = app
            .world_mut()
            .query::<&MirroredBody>()
            .iter(app.world())
            .count();

        assert_eq!(mirrored, expected, "{id:?} did not mirror every prop");
        assert_eq!(
            app.world().resource::<ExperimentSpec>().id,
            id,
            "{id:?}: the plugin recorded a different experiment"
        );
    }
}

/// The ECS mirror must follow a `Scene` that gains and loses props.
///
/// `spawn_scene_entities` runs once, on `Startup`, which was enough for exactly as long as
/// the scene was fixed at build time. Two things broke that in opposite directions: the `B`
/// key **adds** a prop, and a black hole **removes** one. Without
/// `sync_mirror_entities_system` the first would be simulated but drawn nowhere, and the
/// second would leave an entity frozen at a dead prop's last position — which
/// `sync_scene_to_ecs_system` deliberately leaves untouched (Requirement 9.2).
///
/// So Requirement 3.5's mirror invariant is now maintained every frame rather than
/// established once, and this is the test that says so.
#[test]
fn the_mirror_follows_props_appearing_and_disappearing() {
    use bevy::MinimalPlugins;

    use crate::core::scene::{Body, Color, Pose, Prop, Quat};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::default());
    app.update();

    let mirrored = |app: &mut App| -> usize {
        app.world_mut()
            .query::<&MirroredBody>()
            .iter(app.world())
            .count()
    };

    let started_with = app.world().resource::<Scene>().props.len();
    assert_eq!(mirrored(&mut app), started_with);

    // A prop appears, the way the `B` key makes one appear.
    let added = app.world_mut().resource_mut::<Scene>().add(Prop::at_rest(
        Body::Sphere {
            pose: Pose {
                position: DVec3::new(0.0, 40.0, 0.0),
                rotation: Quat::IDENTITY,
            },
            radius: 1.0,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        },
        1.0e24,
    ));
    app.update();

    assert_eq!(
        mirrored(&mut app),
        started_with + 1,
        "a prop added at runtime must be drawn"
    );

    // And exactly one entity carries its id — not two, which is what a sync that spawned
    // every frame would produce.
    let matching = app
        .world_mut()
        .query::<&MirroredBody>()
        .iter(app.world())
        .filter(|body| body.0 == added)
        .count();
    assert_eq!(matching, 1);

    // Several more frames must not spawn duplicates.
    app.update();
    app.update();
    assert_eq!(mirrored(&mut app), started_with + 1);

    // A prop leaves, the way absorption makes one leave.
    app.world_mut().resource_mut::<Scene>().remove(added);
    app.update();

    assert_eq!(
        mirrored(&mut app),
        started_with,
        "a prop removed at runtime must stop being drawn"
    );
    assert_eq!(
        app.world_mut()
            .query::<&MirroredBody>()
            .iter(app.world())
            .filter(|body| body.0 == added)
            .count(),
        0,
        "no entity may outlive its prop"
    );
}

/// A prop placed this frame is drawn in the right place on its first frame.
///
/// The ordering `sync_mirror_entities_system` -> `sync_scene_to_ecs_system` is what buys
/// this. Registered the other way round, a new entity would spend one frame at the origin
/// before the transform sync caught up — a visible flash at the world's centre, for an object
/// the user placed forty units in front of themselves.
#[test]
fn a_prop_added_at_runtime_is_drawn_where_it_actually_is() {
    use bevy::transform::components::Transform;
    use bevy::MinimalPlugins;

    use crate::core::scene::{Body, Color, Pose, Prop, Quat, Vec3};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::default());
    app.update();

    let at = Vec3::new(12.0, 34.0, -56.0);
    let added = app.world_mut().resource_mut::<Scene>().add(Prop::at_rest(
        Body::Sphere {
            pose: Pose {
                position: at.as_dvec3(),
                rotation: Quat::IDENTITY,
            },
            radius: 2.0,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        },
        1.0,
    ));
    app.update();

    let drawn = app
        .world_mut()
        .query::<(&MirroredBody, &Transform)>()
        .iter(app.world())
        .find(|(body, _)| body.0 == added)
        .map(|(_, transform)| transform.translation)
        .expect("the new prop must have an entity");

    assert!(
        (drawn - at).length() < 1.0e-4,
        "drawn at {drawn:?}, but the prop is at {at:?}"
    );
}

/// The scene must clear to the colour of empty space, not Bevy's mid grey.
///
/// Since amendment A17 `experiment_3` opens with a `4.6`-px Sun and nothing else, and a `4.6`-px Sun
/// against `0.25` grey has almost no contrast. This is the one render setting the whole visual result
/// of that experiment rests on.
#[test]
fn the_background_is_the_colour_of_empty_space() {
    use bevy::render::camera::ClearColor;

    use crate::render::constants::BACKGROUND_COLOR;

    let mut app = App::new();
    app.add_plugins(ScenePlugin::default());

    let clear = app
        .world()
        .get_resource::<ClearColor>()
        .expect("ScenePlugin must set the clear colour");

    let linear = clear.0.to_linear();
    assert!((linear.red - BACKGROUND_COLOR.r).abs() < 1.0e-6);
    assert!((linear.green - BACKGROUND_COLOR.g).abs() < 1.0e-6);
    assert!((linear.blue - BACKGROUND_COLOR.b).abs() < 1.0e-6);

    // Dark enough to read as black, and not *exactly* black — a frame that has failed to render at
    // all should still be distinguishable from one that has.
    let brightest = linear.red.max(linear.green).max(linear.blue);
    assert!(
        brightest < 0.02,
        "the background is too bright: {brightest}"
    );
    assert!(brightest > 0.0, "and pure black loses a free diagnostic");
}

/// A scene with a star is lit **from the star**, and one without keeps the directional light.
///
/// The two cannot both be present: a fixed directional light on top of a point light at the Sun
/// would wash out the terminator that lighting exists to produce. Which one appears is decided by
/// whether any prop is luminous, so a scene gets the right answer without declaring anything.
#[test]
fn a_star_lights_its_own_scene_and_a_starless_one_falls_back() {
    use bevy::pbr::{DirectionalLight, PointLight};
    use bevy::MinimalPlugins;

    use crate::experiments::selection::Selection;
    use crate::render::constants::STAR_LUMENS_PER_SOLAR_LUMINOSITY;

    let lights = |id: ExperimentId| -> (usize, usize, f32) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<BodyMaterial>>();
        app.add_plugins(ScenePlugin::new(Selection { id, seed: Some(1) }));
        app.update();

        let points: Vec<f32> = app
            .world_mut()
            .query::<&PointLight>()
            .iter(app.world())
            .map(|light| light.intensity)
            .collect();
        let directional = app
            .world_mut()
            .query::<&DirectionalLight>()
            .iter(app.world())
            .count();

        (
            points.len(),
            directional,
            points.first().copied().unwrap_or(0.0),
        )
    };

    // The solar system has a Sun, so it is lit by it and by nothing else.
    let (points, directional, intensity) = lights(ExperimentId::SolarSystem);
    assert_eq!(points, 1, "exactly one star lights the solar system");
    assert_eq!(directional, 0, "and no directional light competes with it");
    assert!(
        (intensity - STAR_LUMENS_PER_SOLAR_LUMINOSITY).abs() < 1.0,
        "one solar luminosity, by definition: got {intensity}"
    );

    // These have no star, so they keep the fixed light or they would be silhouettes.
    for id in [
        ExperimentId::TwoBody,
        ExperimentId::ThreeBodyRing,
        ExperimentId::Jovian,
        ExperimentId::ScaleCheck,
    ] {
        let (points, directional, _) = lights(id);
        assert_eq!(points, 0, "{id:?} has no star");
        assert_eq!(directional, 1, "{id:?} must still be lit somehow");
    }

    // The black hole has no general light: only its S-stars shine, each a light of its own.
    let (points, directional, _) = lights(ExperimentId::BlackHole);
    assert_eq!(
        (points, directional),
        (crate::experiments::constants::S_STARS.len(), 0),
        "the black hole is lit by nothing but its S-stars"
    );
}

/// The black hole turns off the ambient fill too, which every other experiment keeps.
#[test]
fn the_black_hole_has_no_ambient_light() {
    use bevy::pbr::AmbientLight;
    use bevy::MinimalPlugins;

    use crate::experiments::selection::Selection;

    let ambient = |id: ExperimentId| -> Option<f32> {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<BodyMaterial>>();
        app.add_plugins(ScenePlugin::new(Selection { id, seed: Some(1) }));
        app.update();
        app.world()
            .get_resource::<AmbientLight>()
            .map(|light| light.brightness)
    };

    assert_eq!(ambient(ExperimentId::BlackHole), Some(0.0));
    assert_ne!(
        ambient(ExperimentId::TwoBody),
        Some(0.0),
        "the others keep Bevy's default fill (or none at all headless)"
    );
}

/// A star's light must be a **child** of the star, not a component on it.
///
/// This is a regression test for a real bug: the light was originally put on the mesh entity, because
/// `PointLightBundle` adds only three components beyond `PbrBundle` and transform propagation then
/// came for free. It worked, and the scene went dark the moment the camera looked away from the Sun.
///
/// `extract_lights` skips any light whose entity has `ViewVisibility == false`, and
/// `check_visibility` frustum-culls exactly those entities that **have an `Aabb`** — which a mesh
/// entity gets from `calculate_bounds` and a light-only entity never does. So a light sharing an
/// entity with a mesh inherits the mesh's culling. A child has no bounds of its own and is therefore
/// never culled.
///
/// The assertion is structural rather than visual, because frustum culling cannot be exercised
/// without a render app: the light must be on an entity that has **no mesh**, and it must be
/// parented to the star.
#[test]
fn a_stars_light_is_a_child_entity_with_no_mesh_of_its_own() {
    use bevy::asset::Handle;
    use bevy::ecs::entity::Entity;
    use bevy::hierarchy::Parent;
    use bevy::pbr::PointLight;
    use bevy::render::mesh::Mesh;
    use bevy::MinimalPlugins;

    use crate::experiments::selection::Selection;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::new(Selection {
        id: ExperimentId::SolarSystem,
        seed: None,
    }));
    app.update();

    // Exactly one light, and it is *not* on a mesh entity — which is the bug this pins.
    let on_a_mesh = app
        .world_mut()
        .query::<(&PointLight, &Handle<Mesh>)>()
        .iter(app.world())
        .count();
    assert_eq!(
        on_a_mesh, 0,
        "a light on a mesh entity is frustum-culled with the mesh and goes dark off screen"
    );

    // It is a child of the star, so the parent's transform carries it.
    let parented: Vec<Entity> = app
        .world_mut()
        .query::<(&PointLight, &Parent)>()
        .iter(app.world())
        .map(|(_, parent)| parent.get())
        .collect();
    assert_eq!(
        parented.len(),
        1,
        "exactly one star lights the solar system"
    );

    let star = parented[0];
    let star_id = app
        .world()
        .get::<MirroredBody>(star)
        .expect("the light's parent must be a mirrored body")
        .0;

    // And that parent is the Sun.
    let sun_id = app
        .world()
        .resource::<Scene>()
        .iter()
        .find(|(_, prop)| prop.is_luminous())
        .map(|(id, _)| id)
        .expect("the solar system has a luminous body");
    assert_eq!(star_id, sun_id);
}

/// An absorbed star must take its light with it.
///
/// The light is a child entity, so a plain `despawn` would leave it orphaned and still lighting the
/// scene from wherever the star was when it was swallowed — a light with no body, which is the exact
/// failure `despawn_recursive` avoids.
#[test]
fn removing_a_star_removes_its_light() {
    use bevy::pbr::PointLight;
    use bevy::MinimalPlugins;

    use crate::experiments::selection::Selection;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BodyMaterial>>();
    app.add_plugins(ScenePlugin::new(Selection {
        id: ExperimentId::SolarSystem,
        seed: None,
    }));
    app.update();

    let lights = |app: &mut App| -> usize {
        app.world_mut()
            .query::<&PointLight>()
            .iter(app.world())
            .count()
    };
    assert_eq!(lights(&mut app), 1);

    let sun_id = app
        .world()
        .resource::<Scene>()
        .iter()
        .find(|(_, prop)| prop.is_luminous())
        .map(|(id, _)| id)
        .unwrap();

    app.world_mut().resource_mut::<Scene>().remove(sun_id);
    app.update();

    assert_eq!(
        lights(&mut app),
        0,
        "the light must not outlive the body it belongs to"
    );
}

/// The startup camera must actually frame the pair it is placed for.
///
/// Pins the arithmetic in [`CAMERA_START_POSITION_UNITS`]'s docs against the scene
/// constants it was derived from, so widening
/// [`crate::constants::TWO_BODY_SEPARATION_UNITS`] — or moving the camera in — fails
/// here rather than silently pushing the spheres off screen, which no headless test
/// would otherwise notice.
#[test]
fn the_startup_camera_frames_the_two_body_pair() {
    use crate::constants::{
        TWO_BODY_HEIGHT_UNITS, TWO_BODY_SEPARATION_UNITS, TWO_BODY_VISUAL_RADIUS_UNITS,
    };
    use crate::experiments::catalog::two_body;

    // Bevy's default perspective projection is a 45-degree *vertical* field of view.
    let half_fov = 45.0_f32.to_radians() / 2.0;

    // The widest thing that has to fit: half the separation plus one sphere.
    let half_extent = 0.5 * TWO_BODY_SEPARATION_UNITS + TWO_BODY_VISUAL_RADIUS_UNITS;
    let minimum_distance = half_extent / half_fov.tan();

    let target = Vec3::new(0.0, TWO_BODY_HEIGHT_UNITS, 0.0);
    let actual_distance = (two_body().spec.camera_position_units - target).length();

    assert!(
        actual_distance >= minimum_distance,
        "camera is {actual_distance} units from the scene but needs at least \
         {minimum_distance} to frame a {TWO_BODY_SEPARATION_UNITS}-unit separation"
    );
}

/// `RGA_FLOOR_ECLIPSE=off` must silence the floor and only the floor: the floor's uniform says
/// "no star", which the shader reads as fully lit without running the occluder loop, while every
/// sphere keeps its occluders. With no `PerformanceSettings` at all the floor computes shadows, as
/// shipped.
#[test]
fn the_floor_eclipse_switch_silences_only_the_floor() {
    use bevy::MinimalPlugins;

    use crate::experiments::selection::Selection;
    use crate::performance::PerformanceSettings;

    let star_radius_by_kind = |settings: Option<PerformanceSettings>| {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<BodyMaterial>>();
        app.add_plugins(ScenePlugin::new(Selection {
            id: ExperimentId::SolarSystem,
            seed: None,
        }));
        if let Some(settings) = settings {
            app.insert_resource(settings);
        }
        app.update();

        let world = app.world_mut();
        let handles: Vec<(BodyId, Handle<BodyMaterial>)> = world
            .query::<(&MirroredBody, &Handle<BodyMaterial>)>()
            .iter(world)
            .map(|(mirrored, handle)| (mirrored.0, handle.clone()))
            .collect();
        let scene = world.resource::<Scene>();
        let materials = world.resource::<Assets<BodyMaterial>>();
        let (mut floor, mut spheres) = (Vec::new(), Vec::new());
        for (id, handle) in handles {
            let star_w = materials.get(&handle).unwrap().extension.uniform.star.w;
            match scene.get(id).map(|prop| &prop.body) {
                Some(Body::Plane { .. }) => floor.push(star_w),
                Some(Body::Sphere { .. }) => spheres.push(star_w),
                _ => {}
            }
        }
        (floor, spheres)
    };

    let (floor, spheres) = star_radius_by_kind(None);
    assert!(
        !floor.is_empty() && floor.iter().all(|w| *w > 0.0),
        "shipped: the floor is shadowed"
    );
    assert!(spheres.iter().all(|w| *w > 0.0));

    let off = PerformanceSettings {
        floor_eclipse: false,
        ..PerformanceSettings::default()
    };
    let (floor, spheres) = star_radius_by_kind(Some(off));
    assert!(
        floor.iter().all(|w| *w == 0.0),
        "off: the floor skips the loop"
    );
    assert!(
        !spheres.is_empty() && spheres.iter().all(|w| *w > 0.0),
        "off: spheres are still shadowed"
    );
}
