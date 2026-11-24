//! Tests for [`crate::render::wormhole_view`], `#[path]`-included into it. The drawing itself needs a
//! window; these check the uniform, the shader's text against its twin, and the mouth's own sphere.

use super::*;
use crate::performance::BlackHoleQuality;
use crate::render::constants::{
    WORMHOLE_BODY_AMBIENT, WORMHOLE_BODY_BRIGHTNESS, WORMHOLE_BODY_GLOW, WORMHOLE_NEBULA_CELLS,
    WORMHOLE_NEBULA_COLOUR, WORMHOLE_NEBULA_EDGES, WORMHOLE_NEBULA_TINT, WORMHOLE_PATTERN_FAR,
    WORMHOLE_PATTERN_LINE_RAD, WORMHOLE_PATTERN_NEAR, WORMHOLE_PATTERN_SPACING_RAD,
    WORMHOLE_RAY_REACH, WORMHOLE_RAY_TUNNEL_STEP,
};
use bevy::math::DVec3;

fn shader(name: &str) -> String {
    let path = format!("{}/assets/shaders/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path)
        .expect("the shader exists")
        .replace('\r', "")
}

fn settings() -> WormholeSettings {
    WormholeSettings {
        throat_units: 5.0,
        half_length: 0.5,
        lensing_width: 0.05,
    }
}

/// The text of `fn name(...) {...}` in `source`.
fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("\nfn {name}("))
        .unwrap_or_else(|| panic!("no fn {name}"));
    let end = source[start..].find("\n}\n").expect("it ends") + start;
    &source[start..end]
}

#[test]
fn the_shader_mirrors_its_twin() {
    let source = shader("wormhole.wgsl");
    assert!(WORMHOLE_SHADER_PATH.ends_with("wormhole.wgsl"));
    for name in [
        "radius",
        "slope",
        "rates",
        "rk4",
        "far_frame",
        "from_far_frame",
        "offset",
        "meets",
        "march_past",
        "body_light",
        "value_noise",
        "nebula",
        "near_sky",
        "far_sky",
        "pattern",
    ] {
        function(&source, name);
    }
    // Every constant, by value.
    let constant = |name: &str| -> Vec<f32> {
        let line = source
            .lines()
            .find(|line| line.starts_with(&format!("const {name}:")))
            .unwrap_or_else(|| panic!("no const {name}"));
        let value = line.split('=').nth(1).unwrap();
        value
            .replace("vec2<f32>", "")
            .replace("vec3<f32>", "")
            .split(|c: char| !(c.is_ascii_digit() || ".e-".contains(c)))
            .filter_map(|number| number.parse().ok())
            .collect()
    };
    assert_eq!(constant("REACH"), [WORMHOLE_RAY_REACH]);
    assert_eq!(constant("TUNNEL_STEP"), [WORMHOLE_RAY_TUNNEL_STEP]);
    assert_eq!(constant("MAX_BODIES"), [WORMHOLE_MAX_BODIES as f32]);
    assert_eq!(constant("BODY_GLOW"), [WORMHOLE_BODY_GLOW]);
    assert_eq!(constant("BODY_BRIGHTNESS"), [WORMHOLE_BODY_BRIGHTNESS]);
    assert_eq!(constant("BODY_AMBIENT"), [WORMHOLE_BODY_AMBIENT]);
    let array = format!("array<vec4<f32>, {WORMHOLE_MAX_BODIES}>");
    assert_eq!(source.matches(&array).count(), 3);
    assert_eq!(constant("NEBULA_CELLS"), [WORMHOLE_NEBULA_CELLS]);
    let edges = WORMHOLE_NEBULA_EDGES;
    assert_eq!(constant("NEBULA_EDGES"), [edges.0, edges.1]);
    assert_eq!(constant("NEBULA_COLOUR"), WORMHOLE_NEBULA_COLOUR.to_array());
    assert_eq!(constant("NEBULA_TINT"), WORMHOLE_NEBULA_TINT.to_array());
    assert_eq!(constant("PATTERN_LINE_RAD"), [WORMHOLE_PATTERN_LINE_RAD]);
    assert_eq!(constant("PATTERN_NEAR"), WORMHOLE_PATTERN_NEAR.to_array());
    assert_eq!(constant("PATTERN_FAR"), WORMHOLE_PATTERN_FAR.to_array());
    let spacing = constant("PATTERN_SPACING_RAD")[0];
    assert!((spacing - WORMHOLE_PATTERN_SPACING_RAD).abs() < 1e-6);
    assert!((constant("TWO_OVER_PI")[0] - std::f32::consts::FRAC_2_PI).abs() < 1e-7);
}

#[test]
fn the_stars_are_the_black_holes_to_the_letter() {
    let (wormhole, black_hole) = (shader("wormhole.wgsl"), shader("black_hole.wgsl"));
    for name in ["hash", "unit", "blackbody", "star_field", "tone_map"] {
        assert_eq!(
            function(&wormhole, name),
            function(&black_hole, name),
            "{name}"
        );
    }
    for name in ["STAR_FINE", "STAR_COARSE", "STAR_SIZE_RAD", "SKY_DEPTH"] {
        let line = |source: &str| {
            source
                .lines()
                .find(|line| line.starts_with(&format!("const {name}:")))
                .map(str::to_string)
        };
        assert!(line(&wormhole).is_some());
        assert_eq!(line(&wormhole), line(&black_hole), "{name}");
    }
}

#[test]
fn the_uniform_is_in_throat_radii() {
    let shape = settings().shape();
    let place = WormholePlace {
        l: -15.0,
        n: DVec3::X,
    };
    let performance = PerformanceSettings {
        black_hole_quality: BlackHoleQuality::Medium,
        black_hole_stars: true,
        ..PerformanceSettings::default()
    };
    let uniform = WormholeUniform::new(&shape, &place, BlackHoleView::Pattern, &performance);
    assert_eq!(uniform.camera, Vec4::new(1.0, 0.0, 0.0, -3.0));
    assert!((uniform.shape.x - 0.5).abs() < 1e-6);
    assert!((uniform.shape.y - 0.05 / 1.42953).abs() < 1e-5);
    assert_eq!(uniform.shape.z, WORMHOLE_RAY_STEP_FRACTION);
    assert_eq!(uniform.limits, UVec4::new(WORMHOLE_RAY_MAX_STEPS, 1, 1, 0));

    let dark = PerformanceSettings {
        black_hole_stars: false,
        ..performance
    };
    let uniform = WormholeUniform::new(&shape, &place, BlackHoleView::Shaded, &dark);
    assert_eq!((uniform.limits.y, uniform.limits.z), (0, 0));
}

#[test]
fn the_quality_scales_the_step_and_the_budget() {
    let ray = |quality| {
        ray_wormhole(
            &settings().shape(),
            &PerformanceSettings {
                black_hole_quality: quality,
                ..PerformanceSettings::default()
            },
        )
    };
    let (low, medium, high) = (
        ray(BlackHoleQuality::Low),
        ray(BlackHoleQuality::Medium),
        ray(BlackHoleQuality::High),
    );
    assert_eq!(low.step_fraction, 2.0 * medium.step_fraction);
    assert_eq!(high.step_fraction, 0.5 * medium.step_fraction);
    assert_eq!(2 * low.max_steps, medium.max_steps);
    assert_eq!(high.max_steps, 2 * medium.max_steps);
}

#[test]
fn without_a_renderer_the_plugin_only_moves_the_camera() {
    let mut app = App::new();
    app.insert_resource(peopled());
    app.add_plugins(WormholeViewPlugin);
    app.update();
    assert!(app.world().contains_resource::<WormholeSettings>());
    assert_eq!(app.world().resource::<WormholeTraveller>().place, None);
}
/// A wormhole with a star and a planet on each side.
fn peopled() -> Scene {
    use crate::core::scene::{Color, Pose, Quat};
    let mut scene = crate::projects::interstellar::wormhole::wormhole().scene;
    let shape = scene.wormhole.as_ref().unwrap().shape;
    for (far, star, position) in [
        (false, true, DVec3::new(-25.0, 11.0, -25.0)),
        (false, false, DVec3::new(11.0, -3.5, -17.5)),
        (true, true, DVec3::new(13.0, 10.0, -30.0)),
        (true, false, DVec3::new(-6.0, -2.5, -20.0)),
    ] {
        let mut prop = Prop::new(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::ZERO,
                    rotation: Quat::IDENTITY,
                },
                radius: 2.0,
                color: Color {
                    r: 0.5,
                    g: 0.6,
                    b: 0.7,
                    a: 1.0,
                },
            },
            DVec3::ZERO,
            1.0e30,
        );
        if star {
            prop = prop.with_luminosity(1.0);
        }
        scene.add_at(prop, WormholePlace::at(&shape, position, far));
    }
    scene
}

#[test]
fn the_uniform_holds_every_body_where_it_is_lit_from_its_own_side() {
    let scene = peopled();
    let space = scene.wormhole.as_ref().unwrap();
    let camera = WormholePlace {
        l: 30.0,
        n: DVec3::Z,
    };
    let performance = PerformanceSettings::default();
    let uniform = WormholeUniform::new(&space.shape, &camera, BlackHoleView::Shaded, &performance)
        .with_bodies(&scene, &camera);
    assert_eq!(uniform.limits.w as usize, scene.props.len());
    let throat = space.shape.throat() as f32;
    for (id, prop) in scene.iter() {
        let place = space.place(id).unwrap();
        let wanted = place
            .n
            .as_vec3()
            .extend((place.l / space.shape.throat()) as f32);
        let i = uniform.places.iter().position(|p| *p == wanted).unwrap();
        let Body::Sphere { radius, .. } = prop.body else {
            panic!("a sphere")
        };
        assert!((uniform.lights[i].w - radius / throat).abs() < 1e-6);
        assert_eq!(uniform.colours[i].w == 1.0, prop.is_luminous());
        if !prop.is_luminous() {
            // Each side has its star, and the light comes from it: back in that side's own axes, the
            // direction is the one to the star's drawn position.
            let star = scene
                .iter()
                .find(|(other, lamp)| {
                    lamp.is_luminous() && is_far(&space.place(*other).unwrap()) == is_far(&place)
                })
                .unwrap()
                .1;
            let way = (star.body.center() - prop.body.center()).normalize();
            let lit = place.frame() * uniform.lights[i].truncate().as_dvec3();
            assert!(lit.distance(way) < 1e-5);
        }
    }
}

#[test]
fn too_many_bodies_keeps_the_nearest_along_the_wormhole() {
    let mut scene = peopled();
    let prop = scene.props[1].clone();
    for i in 0..40 {
        let place = WormholePlace {
            l: 100.0 + f64::from(i),
            n: DVec3::X,
        };
        scene.add_at(prop.clone(), place);
    }
    let camera = WormholePlace {
        l: 139.0,
        n: DVec3::Z,
    };
    let shape = scene.wormhole.as_ref().unwrap().shape;
    let performance = PerformanceSettings::default();
    let uniform = WormholeUniform::new(&shape, &camera, BlackHoleView::Shaded, &performance)
        .with_bodies(&scene, &camera);
    assert_eq!(uniform.limits.w as usize, WORMHOLE_MAX_BODIES);
    let nearest = uniform
        .places
        .iter()
        .map(|p| p.w)
        .fold(f32::INFINITY, f32::min);
    assert!(nearest * shape.throat() as f32 > 123.0);
}

#[test]
fn only_bodies_on_the_cameras_side_are_meshes_or_targets() {
    for (marched, camera_far) in [(true, false), (false, false), (false, true)] {
        let scene = peopled();
        let place = WormholePlace {
            l: if camera_far { -30.0 } else { 30.0 },
            n: DVec3::Z,
        };
        let mut traveller = WormholeTraveller::default();
        traveller.place = Some(place);
        let mut app = App::new();
        app.insert_resource(scene.clone())
            .insert_resource(traveller)
            .insert_resource(PerformanceSettings {
                black_hole: marched,
                ..PerformanceSettings::default()
            })
            .add_systems(Update, sync_wormhole_bodies_system);
        let ids: Vec<_> = scene.iter().map(|(id, _)| id).collect();
        let entities: Vec<_> = ids
            .iter()
            .map(|&id| {
                app.world_mut()
                    .spawn((MirroredBody(id), Visibility::Inherited, CameraTarget))
                    .id()
            })
            .collect();
        app.update();
        let space = scene.wormhole.as_ref().unwrap();
        for (id, entity) in ids.into_iter().zip(entities) {
            let same = is_far(&space.place(id).unwrap()) == camera_far;
            let shown = *app.world().get::<Visibility>(entity).unwrap() != Visibility::Hidden;
            assert_eq!(shown, same && !marched);
            assert_eq!(app.world().get::<CameraTarget>(entity).is_some(), same);
        }
    }
}

#[test]
fn the_scene_follows_the_settings() {
    let mut app = App::new();
    app.insert_resource(peopled())
        .insert_resource(WormholeSettings {
            throat_units: 8.0,
            ..settings()
        })
        .add_systems(Update, reshape_wormhole_system);
    app.update();
    let scene = app.world().resource::<Scene>();
    assert_eq!(scene.wormhole.as_ref().unwrap().shape.throat(), 8.0);
}
