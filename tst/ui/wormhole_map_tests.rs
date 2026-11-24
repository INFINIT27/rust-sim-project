//! Tests for [`crate::ui::wormhole_map`], `#[path]`-included into it. The painting needs a window; the
//! surface, its projection, and the path on it do not.

use super::*;

fn shape() -> WormholeShape {
    WormholeShape::dneg(5.0, 2.5, 0.25).unwrap()
}

fn place(l: f64) -> WormholePlace {
    WormholePlace { l, n: DVec3::Z }
}

#[test]
fn the_surface_is_the_embedding() {
    for shape in [
        shape(),
        WormholeShape::dneg(1.0, 0.005, 0.014).unwrap(),
        WormholeShape::dneg(1.0, 5.0, 0.43).unwrap(),
        WormholeShape::Ellis { throat: 2.0 },
    ] {
        let throat = shape.throat();
        let map = WormholeMap::new(&shape, 4.0, 40);
        for l in map.rings() {
            let (r, z) = map.at(l);
            let l = f64::from(l) * throat;
            assert!((f64::from(r) - shape.radius(l) / throat).abs() < 1e-4);
            let exact = shape.embedding_height(l, 200_000) / throat;
            assert!(
                (f64::from(z) - exact).abs() < 2e-3,
                "{shape:?} {l}: {z} {exact}"
            );
        }
    }
}

#[test]
fn distances_along_the_surface_are_the_wormholes() {
    // What an embedding is for: the length of a meridian drawn is the proper distance.
    let map = WormholeMap::new(&shape(), 4.0, 400);
    let rings = map.rings();
    let drawn: f32 = rings
        .windows(2)
        .map(|pair| map.point(pair[0], 0.3).distance(map.point(pair[1], 0.3)))
        .sum();
    let proper = 2.0 * map.reach();
    assert!((drawn / proper - 1.0).abs() < 1e-3, "{drawn} {proper}");
}

#[test]
fn the_two_sides_mirror_each_other_and_the_tunnel_is_straight() {
    let map = WormholeMap::new(&shape(), 3.0, 30);
    for l in [0.2, 0.5, 0.6, 1.0, 3.0] {
        let (near, far) = (map.point(l, 1.0), map.point(-l, 1.0));
        assert!(near.distance(far * Vec3::new(1.0, -1.0, 1.0)) < 1e-6);
        assert!(near.y > 0.0);
    }
    assert_eq!(map.at(0.25), (1.0, 0.25));
    assert_eq!(map.at(-0.4), (1.0, -0.4));
    // Beyond the edge is the edge.
    assert_eq!(map.at(100.0), map.at(map.reach()));
}

#[test]
fn the_projection_is_a_view_from_above() {
    let (flat, depth) = project(Vec3::new(1.0, 2.0, 3.0), 0.0);
    assert_eq!((flat, depth), (Vec2::new(1.0, 2.0), 3.0));
    // Seen from above, what is nearer the viewer is lower on the screen.
    let (near, d_near) = project(Vec3::Z, 0.4);
    let (far, d_far) = project(Vec3::NEG_Z, 0.4);
    assert!(near.y < far.y && d_near > d_far);
}

#[test]
fn the_diagram_holds_the_camera_until_it_is_too_far_out() {
    let (least, most) = WORMHOLE_MAP_REACH_THROATS;
    assert_eq!(reach_for(&shape(), &place(0.0)), least);
    assert_eq!(reach_for(&shape(), &place(-1e9)), most);
    let l = 2.5 + 0.7 * most * 5.0;
    let map = WormholeMap::new(&shape(), reach_for(&shape(), &place(l)), 30);
    assert!(f64::from(map.reach()) > l / 5.0);
}

#[test]
fn light_straight_in_goes_down_one_meridian_to_the_far_edge() {
    let start = place(20.0);
    let points = path_points(&shape(), &start, DVec3::NEG_Z, 4.0, 2000);
    let angle = angle_of(&start) as f32;
    assert!(points.iter().all(|point| (point.y - angle).abs() < 1e-4));
    assert!(points.windows(2).all(|pair| pair[1].x < pair[0].x));
    assert!(points.last().unwrap().x <= -4.0);
}

#[test]
fn light_aimed_wide_turns_back_at_its_impact_parameter() {
    let start = place(20.0);
    let radius = shape().radius(20.0);
    // B = 1.5 throat radii.
    let sine = 7.5 / radius;
    for side in [-1.0, 1.0] {
        let direction = DVec3::new(side * sine, 0.0, -(1.0 - sine * sine).sqrt());
        let points = path_points(&shape(), &start, direction, 4.5, 2000);
        let map = WormholeMap::new(&shape(), 4.0, 400);
        let closest = points
            .iter()
            .map(|point| map.at(point.x).0)
            .fold(f32::INFINITY, f32::min);
        assert!((closest - 1.5).abs() < 0.01, "{closest}");
        assert!(points.last().unwrap().x > 4.0);
        // It goes round the way it is aimed: from +z, +x is the angle decreasing.
        let turned = points.last().unwrap().y - points[0].y;
        assert!(f64::from(turned) * side < 0.0, "{side} {turned}");
    }
}

#[test]
fn light_circling_the_throat_stops_at_its_last_point() {
    let points = path_points(&shape(), &place(0.0), DVec3::X, 4.0, 300);
    assert_eq!(points.len(), 300);
    assert!(points.iter().all(|point| point.x.abs() < 1e-3));
}

#[test]
fn the_key_shows_and_hides_it() {
    let mut app = App::new();
    app.init_resource::<WormholeMapVisibility>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, toggle_wormhole_map_system);
    let key = KeyMap::default().key(Action::ToggleWormholeMap);
    for expected in [false, true] {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        keys.press(key);
        app.update();
        let shown = app.world().resource::<WormholeMapVisibility>();
        assert_eq!(shown.visible, expected);
    }
}
