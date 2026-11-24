//! Tests for [`crate::core::wormhole`], `#[path]`-included into it: bodies coasting through a
//! wormhole keep what they should keep, cross when they should, and bounce. The frames and the paths
//! themselves are also tested from the camera's side, in `tst/render/wormhole_camera_tests.rs`.

use super::*;
use crate::core::scene::{Aabb, Color, Material, Pose, Quat, Vec3};

fn shape() -> WormholeShape {
    WormholeShape::dneg(5.0, 2.5, 0.25).unwrap()
}

fn scene() -> Scene {
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-50.0),
        max: Vec3::splat(50.0),
    });
    scene.wormhole = Some(WormholeSpace::new(shape()));
    scene
}

fn ball(velocity: DVec3, mass: f32, radius: f32) -> Prop {
    let mut prop = Prop::new(
        Body::Sphere {
            pose: Pose {
                position: DVec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            radius,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        },
        velocity,
        mass,
    );
    prop.material = Material::new(1.0, 0.0);
    prop
}

fn near(position: DVec3) -> WormholePlace {
    WormholePlace::at(&shape(), position, false)
}

fn seconds(value: f64) -> TimeStep {
    TimeStep::from_seconds(value).unwrap()
}

/// A body's velocity in the label frame, and where it is.
fn state(scene: &Scene, id: BodyId) -> (WormholePlace, DVec3) {
    let place = scene.wormhole.as_ref().unwrap().place(id).unwrap();
    let velocity = scene.get(id).unwrap().velocity_units_s;
    (place, place.frame().inverse() * velocity)
}

/// The angular momentum about the wormhole's centre, per mass: `r n × v`, kept by the symmetry.
fn spin(scene: &Scene, id: BodyId) -> DVec3 {
    let (place, velocity) = state(scene, id);
    shape().radius(place.l) * place.n.cross(velocity)
}

#[test]
fn nothing_pulls_on_anything() {
    let mut scene = scene();
    let ids = [
        scene.add_at(
            ball(DVec3::ZERO, 1e30, 1.0),
            near(DVec3::new(0.0, 0.0, 12.0)),
        ),
        scene.add_at(
            ball(DVec3::ZERO, 1e30, 1.0),
            near(DVec3::new(4.0, 0.0, 12.0)),
        ),
    ];
    let before = scene.clone();
    scene.step(seconds(1e6));
    for id in ids {
        assert_eq!(scene.get(id), before.get(id));
    }
}

#[test]
fn a_body_that_crosses_keeps_its_energy_and_its_angular_momentum() {
    let mut scene = scene();
    let id = scene.add_at(
        ball(DVec3::new(0.3, -0.2, -2.0), 1e24, 0.5),
        near(DVec3::new(1.0, -0.5, 14.0)),
    );
    let (speed, start) = (scene.props[0].speed(), spin(&scene, id));
    let mut crossed = false;
    for _ in 0..600 {
        scene.step(seconds(1.0 / 30.0));
        assert!((scene.props[0].speed() - speed).abs() < 1e-12 * speed);
        assert!((spin(&scene, id) - start).length() < 1e-5 * start.length());
        crossed |= state(&scene, id).0.l < 0.0;
        assert!(scene.props[0].is_valid());
    }
    assert!(crossed);
    let (place, velocity) = state(&scene, id);
    // Well out on the far side, and still going.
    assert!(place.l < -15.0 && velocity.dot(place.n) < 0.0);
    // Drawn at its radius, in the far side's own axes.
    let drawn = scene.props[0].body.center();
    assert!((drawn.length() - shape().radius(place.l)).abs() < 1e-9);
}

#[test]
fn a_body_crosses_exactly_when_its_impact_parameter_is_below_the_throat() {
    for (fraction, crosses) in [(0.9, true), (1.1, false)] {
        let mut scene = scene();
        let start = near(DVec3::new(0.0, 0.0, 20.0));
        let sine = fraction * 5.0 / 20.0;
        let cosine = f64::sqrt(1.0 - sine * sine);
        let id = scene.add_at(ball(DVec3::new(sine, 0.0, -cosine), 1.0, 0.2), start);
        for _ in 0..400 {
            scene.step(seconds(0.1));
        }
        assert_eq!(state(&scene, id).0.l < 0.0, crosses, "{fraction}");
    }
}

#[test]
fn far_from_the_mouth_a_body_goes_straight() {
    let mut scene = scene();
    let velocity = DVec3::new(1.0, 2.0, -0.5);
    let start = DVec3::new(3e6, 1e6, -2e6);
    scene.add_at(ball(velocity, 1.0, 1.0), near(start));
    scene.step(seconds(1000.0));
    let moved = scene.props[0].body.center() - start;
    assert!(moved.distance(velocity * 1000.0) < 1.0, "{moved}");
}

#[test]
fn bodies_on_different_sides_at_the_same_drawn_position_do_not_meet() {
    let mut scene = scene();
    let at = DVec3::new(0.0, 0.0, 10.0);
    let a = scene.add_at(ball(DVec3::ZERO, 1.0, 1.0), near(at));
    let b = scene.add_at(
        ball(DVec3::new(0.0, 0.0, 1e-3), 1.0, 1.0),
        WormholePlace::at(&shape(), at, true),
    );
    let drawn = scene.get(a).unwrap().body.center();
    assert!(drawn.distance(scene.get(b).unwrap().body.center()) < 1e-6);
    scene.step(seconds(0.1));
    assert_eq!(scene.get(a).unwrap().velocity_units_s, DVec3::ZERO);
}

#[test]
fn two_bodies_meeting_in_the_tunnel_bounce_and_keep_what_they_should() {
    // Head-on along the tunnel, unequal masses, perfectly elastic.
    let mut scene = scene();
    let n = DVec3::new(0.0, 0.6, 0.8);
    let other = DVec3::new(0.02, 0.6, 0.8).normalize();
    let (m1, m2) = (2.0, 3.0);
    let a = scene.add_at(ball(0.5 * n, m1 as f32, 0.3), WormholePlace { l: 0.1, n });
    let b = scene.add_at(
        ball(-0.25 * other, m2 as f32, 0.3),
        WormholePlace { l: 1.5, n: other },
    );
    let totals = |scene: &Scene| {
        let (va, vb) = (
            scene.props[0].velocity_units_s,
            scene.props[1].velocity_units_s,
        );
        (
            m1 * va.length_squared() + m2 * vb.length_squared(),
            m1 * spin(scene, a) + m2 * spin(scene, b),
        )
    };
    let (energy, momentum) = totals(&scene);
    for _ in 0..200 {
        scene.step(seconds(0.02));
    }
    let (after_energy, after_momentum) = totals(&scene);
    assert!((after_energy / energy - 1.0).abs() < 1e-9);
    assert!(
        (after_momentum - momentum).length() < 2e-3,
        "{after_momentum} {momentum}"
    );
    // They did meet: each has turned back.
    assert!(state(&scene, a).1.dot(n) < 0.0);
    assert!(state(&scene, b).1.dot(other) > 0.0);
    let apart = separation(&shape(), &state(&scene, a).0, &state(&scene, b).0).unwrap();
    assert!(apart.1 > 0.6 - 1e-9);
}

#[test]
fn a_fast_body_does_not_pass_through_a_still_one() {
    let mut scene = scene();
    let still = scene.add_at(
        ball(DVec3::ZERO, 1.0, 0.2),
        near(DVec3::new(0.0, 0.0, 20.0)),
    );
    scene.add_at(
        ball(DVec3::new(0.0, 0.0, -10.0), 1.0, 0.2),
        near(DVec3::new(0.0, 0.0, 24.0)),
    );
    scene.step(seconds(1.0));
    assert!(scene.get(still).unwrap().speed() > 9.0);
}

#[test]
fn a_body_added_the_ordinary_way_is_on_the_near_side_where_it_was_put() {
    let mut scene = scene();
    let mut prop = ball(DVec3::ZERO, 1.0, 1.0);
    prop.body.translate(DVec3::new(0.0, 3.0, 14.0));
    let id = scene.add(prop);
    let gone = scene.add_at(ball(DVec3::ZERO, 1.0, 1.0), near(DVec3::X * 30.0));
    scene.remove(gone);
    scene.step(seconds(1.0));
    let space = scene.wormhole.as_ref().unwrap();
    assert!(space.place(id).unwrap().l > 0.0);
    assert_eq!(space.place(gone), None);
    let drawn = scene.get(id).unwrap().body.center();
    assert!(drawn.distance(DVec3::new(0.0, 3.0, 14.0)) < 1e-9);
}

#[test]
fn a_reshaped_wormhole_keeps_the_scene_in_throat_radii() {
    let mut scene = scene();
    let id = scene.add_at(
        ball(DVec3::new(0.0, 0.0, -0.1), 1.0, 0.5),
        near(DVec3::new(0.0, 0.0, 20.0)),
    );
    let outside = state(&scene, id).0.l - 2.5;
    let bigger = WormholeShape::dneg(15.0, 7.5, 0.75).unwrap();
    scene.reshape_wormhole(bigger);
    let (place, _) = state(&scene, id);
    assert!((place.l - 7.5 - 3.0 * outside).abs() < 1e-9);
    assert!((scene.props[0].speed() - 0.1).abs() < 1e-12);
    assert!(matches!(scene.props[0].body, Body::Sphere { radius, .. } if radius == 1.5));
    assert!((scene.props[0].body.center().z - 60.0).abs() < 1e-5);
    let unchanged = scene.clone();
    scene.reshape_wormhole(bigger);
    assert_eq!(scene, unchanged);
}

#[test]
fn a_scene_without_a_wormhole_is_stepped_as_ever() {
    let mut flat = scene();
    flat.wormhole = None;
    flat.reshape_wormhole(shape());
    assert_eq!(flat.wormhole, None);
    let place = near(DVec3::Z * 20.0);
    let mut prop = ball(DVec3::X, 1.0, 1.0);
    prop.body.translate(DVec3::Z * 9.0);
    let id = flat.add_at(prop, place);
    flat.step(seconds(2.0));
    let moved = flat.get(id).unwrap().body.center();
    assert!(moved.distance(DVec3::new(2.0, 0.0, 9.0)) < 1e-9);
}
