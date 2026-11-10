//! Unit tests for the [`crate::render::placement`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/render/placement.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! Two halves: [`placement_position`] is pure and gets the geometry tests, and
//! [`place_object_system`] is driven through a real `App` because the thing worth testing is
//! that a keypress ends up as a prop **in the running scene** — pulling on everything else,
//! not sitting in a list somewhere.

use super::*;
use crate::core::scene::Prop;
use crate::experiments::constants::STELLAR_OBJECTS;
use bevy::app::{App, Update};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::MinimalPlugins;

/// An `App` with a camera at the origin looking down `-Z`, the placer, and a scene.
fn app_with_placement() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<PlacementState>();
    app.insert_resource(Scene::demo());
    app.add_systems(Update, place_object_system);
    app.world_mut()
        .spawn((Camera3d::default(), Transform::default()));
    app
}

fn request(app: &mut App) {
    let mut placer = app.world_mut().resource_mut::<PlacementState>();
    placer.open();
    placer.request_place();
}

// -----------------------------------------------------------------------
// Where an object goes.
// -----------------------------------------------------------------------

#[test]
fn an_object_is_placed_in_front_of_the_camera() {
    let at = placement_position(Vec3::new(10.0, 5.0, 0.0), -Vec3::Z, 1.0);

    assert_eq!(at.x, 10.0);
    assert_eq!(at.y, 5.0);
    assert!(at.z < 0.0, "in front means along the view direction");
}

#[test]
fn the_distance_is_the_floor_for_a_small_object() {
    let at = placement_position(Vec3::ZERO, -Vec3::Z, 0.30);

    assert!((at.length() - PLACEMENT_DISTANCE_UNITS).abs() < 1.0e-4);
}

#[test]
fn a_large_object_is_pushed_further_out() {
    // The catalog spans drawn radii from 0.30 to 16.8 units, so one fixed distance cannot
    // serve both: at 40 units a red dwarf would fill the view.
    let big = STELLAR_OBJECTS
        .iter()
        .map(|object| object.contact_radius_units())
        .fold(0.0_f32, f32::max);
    let at = placement_position(Vec3::ZERO, -Vec3::Z, big);

    assert!((at.length() - big * PLACEMENT_RADII_CLEARANCE).abs() < 1.0e-3);
    assert!(at.length() > PLACEMENT_DISTANCE_UNITS);
}

#[test]
fn every_object_in_the_catalog_is_placed_clear_of_the_camera() {
    // The property the clearance rule exists for: the camera must never end up inside the
    // thing it just placed. The camera does not collide, so this would not be an error — just
    // a view of the inside of a sphere with no way to tell what happened.
    for object in STELLAR_OBJECTS {
        let at = placement_position(Vec3::ZERO, -Vec3::Z, object.contact_radius_units());
        let gap = at.length() - object.contact_radius_units();

        assert!(
            gap > object.contact_radius_units(),
            "{} leaves only {gap} units of clearance",
            object.name
        );
    }
}

#[test]
fn the_direction_does_not_have_to_be_normalised() {
    let long = placement_position(Vec3::ZERO, Vec3::new(0.0, 0.0, -100.0), 1.0);
    let unit = placement_position(Vec3::ZERO, -Vec3::Z, 1.0);

    assert!((long - unit).length() < 1.0e-4);
}

#[test]
fn a_camera_looking_nowhere_places_at_its_own_position() {
    // A zero forward vector is a finite, legal pose with no direction in it — so it is not a
    // contract violation, and the object goes where the camera is rather than at a `NaN`.
    assert_eq!(
        placement_position(Vec3::new(3.0, 4.0, 5.0), Vec3::ZERO, 1.0),
        Vec3::new(3.0, 4.0, 5.0)
    );
}

#[test]
fn a_non_finite_pose_yields_a_finite_position_rather_than_a_nan() {
    // Release behaviour: a `NaN` position would poison the scene the instant it was added,
    // and every later step with it. A wrongly-placed object is visible; a `NaN` is not. In
    // debug the same inputs fire a `debug_assert!`, covered below.
    #[cfg(not(debug_assertions))]
    for (position, forward) in [
        (Vec3::ZERO, Vec3::new(f32::NAN, 0.0, 0.0)),
        (Vec3::splat(f32::NAN), -Vec3::Z),
        (Vec3::splat(f32::INFINITY), -Vec3::Z),
    ] {
        assert!(placement_position(position, forward, 1.0).is_finite());
    }

    assert!(placement_position(Vec3::ZERO, -Vec3::Z, 1.0).is_finite());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "camera pose must be finite")]
fn a_non_finite_pose_panics_in_debug() {
    let _ = placement_position(Vec3::ZERO, Vec3::new(f32::NAN, 0.0, 0.0), 1.0);
}

#[test]
fn a_nonsense_radius_is_treated_as_zero_rather_than_poisoning_the_distance() {
    for radius in [0.0, -1.0, f32::NAN] {
        let at = placement_position(Vec3::ZERO, -Vec3::Z, radius);

        assert!(at.is_finite());
        assert!((at.length() - PLACEMENT_DISTANCE_UNITS).abs() < 1.0e-4);
    }
}

// -----------------------------------------------------------------------
// Placing, through the real system.
// -----------------------------------------------------------------------

#[test]
fn a_request_adds_exactly_one_prop_and_is_consumed() {
    let mut app = app_with_placement();
    let before = app.world().resource::<Scene>().props.len();

    request(&mut app);
    app.update();

    assert_eq!(app.world().resource::<Scene>().props.len(), before + 1);
    assert!(!app.world().resource::<PlacementState>().place_requested());

    // And it does not keep placing on subsequent frames, which is what "consumed" has to mean.
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Scene>().props.len(), before + 1);
}

#[test]
fn no_request_places_nothing() {
    let mut app = app_with_placement();
    let before = app.world().resource::<Scene>().props.len();

    for _ in 0..5 {
        app.update();
    }

    assert_eq!(app.world().resource::<Scene>().props.len(), before);
}

#[test]
fn the_placed_prop_is_the_selected_object() {
    let mut app = app_with_placement();

    {
        let mut placer = app.world_mut().resource_mut::<PlacementState>();
        placer.open();
        placer.select_next();
        placer.select_next();
        placer.request_place();
    }
    let expected = *app.world().resource::<PlacementState>().selected();
    app.update();

    let placed: &Prop = app.world().resource::<Scene>().props.last().unwrap();
    assert_eq!(placed.mass_kg, expected.mass_kg as f32);
    assert_eq!(placed.absorbs, expected.absorbs);
}

#[test]
fn the_placed_prop_joins_the_simulation_properly() {
    // Not decoration: it is valid, movable, massive, and at rest, so gravity takes over on
    // the next step.
    let mut app = app_with_placement();
    request(&mut app);
    app.update();

    let placed = app
        .world()
        .resource::<Scene>()
        .props
        .last()
        .unwrap()
        .clone();

    assert!(placed.is_valid());
    assert!(placed.responds_to_forces());
    assert_eq!(placed.velocity_units_s.as_vec3(), Vec3::ZERO);
    assert!(placed.body.center().is_finite());
}

#[test]
fn a_request_with_no_camera_is_consumed_rather_than_left_armed() {
    // Otherwise the keypress would fire later, when the view has moved somewhere else
    // entirely — the kind of bug that looks like the app placing objects at random.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<PlacementState>();
    app.insert_resource(Scene::demo());
    app.add_systems(Update, place_object_system);

    let before = app.world().resource::<Scene>().props.len();
    request(&mut app);
    app.update();

    assert!(!app.world().resource::<PlacementState>().place_requested());
    assert_eq!(app.world().resource::<Scene>().props.len(), before);
}

#[test]
fn placing_several_objects_works() {
    // Each `Enter` is one object, and the scene keeps them all — the placer is not a
    // single-shot.
    let mut app = app_with_placement();
    let before = app.world().resource::<Scene>().props.len();

    for _ in 0..3 {
        request(&mut app);
        app.update();
    }

    assert_eq!(app.world().resource::<Scene>().props.len(), before + 3);
}
