//! Tests for [`crate::render::wormhole_camera`], `#[path]`-included into it.
//!
//! The camera must go where its light comes from, stay put when nothing moves it, never see its
//! picture jump, and fly as in flat space far from the mouth.

use super::*;
use crate::projects::interstellar::catalog::ExperimentId;
use crate::render::wormhole_ray::{far_frame, march, RaySky, RayWormhole};
use bevy::app::{App, Update};
use bevy::math::{DVec3, EulerRot};

fn settings() -> WormholeSettings {
    WormholeSettings {
        throat_units: 5.0,
        half_length: 0.5,
        lensing_width: 0.05,
    }
}

fn directions() -> Vec<DVec3> {
    (0..60)
        .map(|i| {
            let z = 1.0 - 2.0 * (f64::from(i) + 0.5) / 60.0;
            let a = 2.399_963 * f64::from(i);
            let s = (1.0 - z * z).sqrt();
            DVec3::new(s * a.cos(), s * a.sin(), z)
        })
        .collect()
}

fn ray(shape: &WormholeShape) -> RayWormhole {
    let WormholeShape::Dneg { throat, flare, .. } = *shape else {
        unreachable!()
    };
    RayWormhole::new(
        (shape.half_length() / throat) as f32,
        (flare / throat) as f32,
    )
}

/// What a camera at `place` sees along label-frame `direction`.
fn seen(shape: &WormholeShape, place: &WormholePlace, direction: DVec3) -> RaySky {
    let l = (place.l / shape.throat()) as f32;
    march(&ray(shape), l, place.n.as_vec3(), direction.as_vec3()).0
}

fn same_sky(a: RaySky, b: RaySky, tolerance: f32) -> bool {
    match (a, b) {
        (RaySky::Near(a), RaySky::Near(b)) | (RaySky::Far(a), RaySky::Far(b)) => {
            a.distance(b) < tolerance
        }
        _ => false,
    }
}

#[test]
fn the_far_rotation_is_the_ray_marchers() {
    for n in directions() {
        for v in directions().into_iter().step_by(7) {
            let a = (far_rotation(n) * v).as_vec3();
            assert!(a.distance(far_frame(n.as_vec3(), v.as_vec3())) < 1e-5);
        }
    }
}

#[test]
fn a_place_and_where_it_is_drawn_are_inverses_outside_the_mouth() {
    let shape = settings().shape();
    for n in directions() {
        for l in [2.6, 4.0, 30.0, 500.0] {
            for far in [false, true] {
                let place = WormholePlace {
                    l: if far { -l } else { l },
                    n,
                };
                let back = WormholePlace::at(&shape, place.position(&shape), far);
                assert!((back.l - place.l).abs() < 1e-6 * l, "{place:?} {back:?}");
                assert!(back.n.distance(n) < 1e-9);
            }
        }
    }
    // Inside the throat's radius there is no place: the mouth.
    let mouth = WormholePlace::at(&shape, DVec3::ZERO, false);
    assert_eq!(mouth.l, shape.half_length());
}

#[test]
fn far_away_flying_is_flat() {
    let shape = settings().shape();
    let place = WormholePlace {
        l: 1e7,
        n: DVec3::new(0.6, 0.0, 0.8),
    };
    for direction in directions() {
        let (end, carried) = travel(&shape, place, direction, 1e5);
        let flat = place.position(&shape) + direction * 1e5;
        let off = end.position(&shape).distance(flat) / 1e5;
        assert!(off < 1e-4, "{direction} {off}");
        assert!(carried.angle_between(DQuat::IDENTITY) < 1e-4);
    }
}

#[test]
fn the_camera_goes_where_its_light_comes_from() {
    // The light seen along a direction keeps arriving along the carried direction as the camera
    // flies that way: the same point of the same sky, at every place on the way.
    let shape = settings().shape();
    for start in [
        WormholePlace {
            l: 12.0,
            n: DVec3::Z,
        },
        WormholePlace {
            l: 1.0,
            n: DVec3::new(0.6, 0.64, -0.48),
        },
        WormholePlace {
            l: -7.0,
            n: DVec3::X,
        },
    ] {
        for direction in directions() {
            let first = seen(&shape, &start, direction);
            if first == RaySky::Lost {
                continue;
            }
            // Light that all but circles the throat multiplies any error.
            let across = direction - direction.dot(start.n) * start.n;
            let impact = shape.radius(start.l) / shape.throat() * across.length();
            if (impact - 1.0).abs() < 0.1 {
                continue;
            }
            let (mut place, mut heading) = (start, direction);
            for _ in 0..40 {
                let (next, carried) = travel(&shape, place, heading, 0.7);
                place = next;
                heading = (carried * heading).normalize();
                let now = seen(&shape, &place, heading);
                assert!(same_sky(first, now, 0.02), "{start:?} {direction} {now:?}");
            }
        }
    }
}

#[test]
fn flying_out_and_back_returns_to_the_start_facing_the_same_way() {
    let shape = settings().shape();
    let start = WormholePlace {
        l: 9.0,
        n: DVec3::Z,
    };
    for direction in directions() {
        let (there, carried) = travel(&shape, start, direction, 25.0);
        let (back, returned) = travel(&shape, there, -(carried * direction), 25.0);
        let across = direction - direction.dot(start.n) * start.n;
        if (shape.radius(start.l) * across.length() / shape.throat() - 1.0).abs() < 0.05 {
            continue;
        }
        assert!((back.l - start.l).abs() < 1e-4, "{direction} {back:?}");
        assert!(back.n.distance(start.n) < 1e-4);
        let whole = (returned * carried).normalize();
        assert!(whole.angle_between(DQuat::IDENTITY) < 1e-3, "{direction}");
    }
}

#[test]
fn a_carried_direction_keeps_its_angles() {
    // Parallel transport is a rotation: what was across the path stays across it.
    let shape = settings().shape();
    let start = WormholePlace {
        l: 4.0,
        n: DVec3::Z,
    };
    let direction = DVec3::new(0.5, 0.0, -0.75_f64.sqrt());
    let (end, carried) = travel(&shape, start, direction, 6.0);
    // Level flight stays level.
    assert!((carried * DVec3::Y).distance(DVec3::Y) < 1e-9);
    assert!(end.n.y.abs() < 1e-9);
}

#[test]
fn a_changed_wormhole_keeps_the_camera_at_the_same_place_in_it() {
    let old = settings().shape();
    let new = WormholeSettings {
        throat_units: 10.0,
        half_length: 1.0,
        ..settings()
    }
    .shape();
    let at = |l: f64| WormholePlace { l, n: DVec3::Z };
    let moved = at(-12.5).rescaled(&old, &new);
    // 10 units, two throat radii, outside the far mouth.
    assert!((moved.l + new.half_length() + 20.0).abs() < 1e-9);
    // Halfway along the near half of the tunnel.
    let moved = at(1.25).rescaled(&old, &new);
    assert!((moved.l - 0.5 * new.half_length()).abs() < 1e-9);
}

/// An app with the camera system, and a camera that flies straight ahead `0.2` units a frame.
fn app(yaw: f32, pitch: f32) -> App {
    let mut app = App::new();
    let mut spec = ExperimentId::Wormhole.build_default().spec;
    spec.camera_position_units = Vec3::new(0.0, 0.0, 20.0);
    app.insert_resource(spec)
        .insert_resource(settings())
        .init_resource::<WormholeTraveller>()
        .add_systems(Update, wormhole_camera_system);
    let controller = FreeFlyController {
        yaw,
        pitch,
        ..FreeFlyController::default()
    };
    app.world_mut().spawn((
        bevy::core_pipeline::core_3d::Camera3d::default(),
        Transform::from_translation(spec.camera_position_units)
            .with_rotation(bevy::math::Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)),
        controller,
        CameraLock::default(),
    ));
    app.update();
    app
}

fn fly(app: &mut App, distance: f32) {
    let mut cameras = app.world_mut().query::<&mut Transform>();
    let mut transform = cameras.single_mut(app.world_mut());
    let forward = transform.forward().as_vec3();
    transform.translation += forward * distance;
    app.update();
}

fn state(app: &mut App) -> (WormholePlace, Transform, FreeFlyController) {
    let place = app.world().resource::<WormholeTraveller>().place.unwrap();
    let mut cameras = app.world_mut().query::<(&Transform, &FreeFlyController)>();
    let (transform, controller) = cameras.single(app.world());
    (place, *transform, *controller)
}

#[test]
fn a_still_camera_stays_exactly_where_it_is() {
    let mut app = app(0.0, 0.0);
    let before = state(&mut app);
    for _ in 0..50 {
        app.update();
    }
    assert_eq!(state(&mut app), before);
}

#[test]
fn flying_straight_in_comes_out_of_the_far_side_still_going_straight_out() {
    let mut app = app(0.0, 0.0);
    let mut last = state(&mut app).0.l;
    for _ in 0..300 {
        fly(&mut app, 0.2);
        let l = state(&mut app).0.l;
        assert!(last - l > 0.0 && last - l < 0.21, "{last} to {l}");
        last = l;
    }
    let (place, transform, controller) = state(&mut app);
    assert!(place.l < -35.0, "{place:?}");
    // Along z the two sides' axes agree: out of the far mouth along −z, not turned.
    assert!(transform.translation.z < -30.0 && transform.translation.x.abs() < 1e-3);
    assert!(controller.frame.angle_between(bevy::math::Quat::IDENTITY) < 1e-3);
}

#[test]
fn the_picture_never_jumps_on_the_way_through() {
    // Slanted, so that everything turns: after each small move every fixed direction of the camera
    // sees nearly what it saw before.
    for (yaw, pitch) in [(0.1, 0.0), (0.1, 0.15), (-0.05, -0.12)] {
        let mut app = app(yaw, pitch);
        let shape = settings().shape();
        let looks = [
            Vec3::NEG_Z,
            Vec3::new(0.3, 0.1, -0.95).normalize(),
            Vec3::new(-0.2, 0.4, -0.9).normalize(),
            Vec3::Z,
        ];
        let mut crossed = false;
        let picture = |place: &WormholePlace, transform: &Transform, look: Vec3| {
            let world = (transform.rotation * look).as_dvec3();
            seen(&shape, place, place.frame().inverse() * world)
        };
        for _ in 0..2500 {
            let (a, ta, _) = state(&mut app);
            fly(&mut app, 0.02);
            let (b, tb, controller) = state(&mut app);
            crossed |= a.l >= 0.0 && b.l < 0.0;
            // The controller's own idea of the rotation is the one it has.
            let own =
                controller.frame * bevy::math::Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
            assert!(own.angle_between(tb.rotation) < 2e-2);
            // Straight ahead is what the path keeps exactly.
            let (before, after) = (picture(&a, &ta, Vec3::NEG_Z), picture(&b, &tb, Vec3::NEG_Z));
            assert!(same_sky(before, after, 0.05), "{a:?}: {before:?} {after:?}");
            // At the change of axes nothing at all may jump.
            if a.l >= 0.0 && b.l < 0.0 {
                for look in looks {
                    let (before, after) = (picture(&a, &ta, look), picture(&b, &tb, look));
                    assert!(same_sky(before, after, 0.2), "{look}: {before:?} {after:?}");
                }
            }
        }
        assert!(crossed, "{yaw} {pitch}");
    }
}

#[test]
fn a_locked_camera_keeps_to_its_side_and_outside_the_mouth() {
    let mut app = app(0.0, 0.0);
    for _ in 0..200 {
        fly(&mut app, 0.2);
    }
    assert!(state(&mut app).0.l < 0.0);
    let target = app.world_mut().spawn_empty().id();
    let mut cameras = app.world_mut().query::<(&mut CameraLock, &mut Transform)>();
    let (mut lock, mut transform) = cameras.single_mut(app.world_mut());
    lock.0 = Some(crate::input::camera::Orbit {
        target,
        distance: 9.0,
    });
    transform.translation = Vec3::new(9.0, 0.0, 0.0);
    app.update();
    let (place, transform, _) = state(&mut app);
    assert!(place.l < 0.0);
    assert!(transform.translation.distance(Vec3::new(9.0, 0.0, 0.0)) < 1e-4);
    // Put inside the throat's radius, it is drawn at the mouth.
    let mut cameras = app.world_mut().query::<&mut Transform>();
    cameras.single_mut(app.world_mut()).translation = Vec3::new(1.0, 0.0, 0.0);
    app.update();
    assert!((state(&mut app).1.translation.length() - 5.0).abs() < 1e-3);
}

#[test]
fn other_experiments_are_left_alone() {
    let mut app = app(0.0, 0.0);
    app.insert_resource(ExperimentId::SolarSystem.build_default().spec);
    let before = state(&mut app).0;
    fly(&mut app, 100.0);
    let (place, transform, _) = state(&mut app);
    assert_eq!(place, before);
    assert!((transform.translation.z + 80.0).abs() < 1e-3);
}

#[test]
fn the_place_is_described_by_its_side() {
    let shape = settings().shape();
    let at = |l: f64| place_line(&WormholePlace { l, n: DVec3::Z }, &shape);
    assert!(at(10.0).contains("near side, l = 2.000"));
    assert!(at(0.0).contains("in the tunnel, l = 0.000 rho, r = 1.000"));
    assert!(at(-10.0).contains("far side, l = -2.000"));
}
