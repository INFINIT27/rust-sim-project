//! Tests for [`crate::render::wormhole_ray`], the shader's `f32` twin, `#[path]`-included into it.
//!
//! Held to the `f64` paths in [`crate::physics_utils::relativity::wormhole`], which are held to what
//! is exact; and the two sides' frames to being rotations that agree at the throat.

use super::*;
use crate::physics_utils::relativity::wormhole::{WormholePath, WormholeShape};
use bevy::math::Mat3;

/// `(a, 𝒲)` in throat radii: the paper's film and figure wormholes, and the extremes of its figures.
const SHAPES: [(f64, f64); 5] = [
    (0.5, 0.05),
    (0.005, 0.05),
    (5.0, 0.05),
    (0.5, 0.014),
    (0.5, 0.43),
];

fn pair(half: f64, width: f64) -> (RayWormhole, WormholeShape) {
    let shape = WormholeShape::dneg(1.0, half, width).unwrap();
    let WormholeShape::Dneg { flare, .. } = shape else {
        unreachable!()
    };
    (RayWormhole::new(half as f32, flare as f32), shape)
}

fn directions() -> Vec<Vec3> {
    (0..200)
        .map(|i| {
            let z = 1.0 - 2.0 * (i as f32 + 0.5) / 200.0;
            let a = 2.399_963 * i as f32;
            let s = (1.0 - z * z).sqrt();
            Vec3::new(s * a.cos(), s * a.sin(), z)
        })
        .collect()
}

#[test]
fn the_radius_and_slope_are_the_f64_ones() {
    for (half, width) in SHAPES {
        let (w, shape) = pair(half, width);
        for i in -500..500 {
            let l = f64::from(i) * 0.021;
            assert!((f64::from(radius(&w, l as f32)) - shape.radius(l)).abs() < 2e-5);
            assert!((f64::from(slope(&w, l as f32)) - shape.slope(l)).abs() < 1e-4);
        }
    }
}

/// How far each ray's sky direction is from the `f64` one, and the most steps one took.
fn worst_error(w: &RayWormhole, shape: &WormholeShape) -> (f32, u32) {
    let mut worst: f32 = 0.0;
    let mut most = 0;
    for l in [0.2, 0.9, 3.0, 12.0, -0.7, -6.0] {
        for n in [Vec3::Z, Vec3::new(0.6, 0.64, -0.48)] {
            for direction in directions() {
                let p = direction.dot(n);
                let across = direction - p * n;
                let (path, impact) =
                    WormholePath::start(shape, l, f64::from(across.length().atan2(p)));
                // Rays that all but circle the throat are chaotic: any error is multiplied.
                if (impact - 1.0).abs() < 0.03 {
                    continue;
                }
                let end = path
                    .escape(shape, impact, 2000.0, 0.01, 10_000_000)
                    .unwrap();
                let t = across.normalize();
                let angle = end.angle;
                let (sin, cos) = (angle.sin() as f32, angle.cos() as f32);
                let end_n = cos * n + sin * t;
                let end_t = cos * t - sin * n;
                let v = end.p as f32 * end_n + (impact / shape.radius(end.l)) as f32 * end_t;
                let (got, steps) = march(w, l as f32, n, direction);
                most = most.max(steps);
                let error = match got {
                    RaySky::Near(d) => {
                        assert!(end.l > 0.0, "{l} {n} {direction}");
                        d.distance(v)
                    }
                    RaySky::Far(d) => {
                        assert!(end.l < 0.0, "{l} {n} {direction}");
                        d.distance(far_frame(end_n, v))
                    }
                    RaySky::Lost => panic!("lost {l} {n} {direction} {impact}"),
                };
                worst = worst.max(error);
            }
        }
    }
    (worst, most)
}

#[test]
fn every_ray_reaches_the_sky_the_f64_path_does() {
    for (half, width) in SHAPES {
        let (w, shape) = pair(half, width);
        let (error, steps) = worst_error(&w, &shape);
        println!("a {half} W {width}: worst {error:.2e} rad, most steps {steps}");
        assert!(error < 4e-3, "a {half} W {width}: {error}");
    }
}

#[test]
fn low_quality_still_reaches_the_right_sky() {
    for (half, width) in SHAPES {
        let (mut w, shape) = pair(half, width);
        w.step_fraction *= 2.0;
        w.max_steps /= 2;
        let (error, steps) = worst_error(&w, &shape);
        println!("a {half} W {width}: worst {error:.2e} rad, most steps {steps}");
        assert!(error < 2e-2, "a {half} W {width}: {error}");
    }
}

#[test]
fn the_far_frame_is_a_rotation_and_none_along_z() {
    for n in directions() {
        let m = Mat3::from_cols(
            far_frame(n, Vec3::X),
            far_frame(n, Vec3::Y),
            far_frame(n, Vec3::Z),
        );
        assert!((m.determinant() - 1.0).abs() < 1e-5);
        assert!((m * m.transpose()).abs_diff_eq(Mat3::IDENTITY, 1e-5));
        // Going down ℓ is going away from the far mouth: out along the far side's own radius.
        let mirror = Vec3::new(n.x, n.y, -n.z);
        assert!(far_frame(n, -n).distance(mirror) < 1e-5);
        for v in directions().into_iter().step_by(17) {
            assert!(from_far_frame(n, far_frame(n, v)).distance(v) < 1e-5);
        }
    }
    assert_eq!(
        far_frame(Vec3::Z, Vec3::new(1.0, 2.0, 3.0)),
        Vec3::new(1.0, 2.0, 3.0)
    );
}

#[test]
fn a_level_crossing_keeps_the_horizon_level() {
    // For labels in the horizontal plane, up stays up: crossing never rolls the view.
    for i in 0..24 {
        let a = i as f32 * 0.26;
        let n = Vec3::new(a.cos(), 0.0, a.sin());
        assert!(far_frame(n, Vec3::Y).distance(Vec3::Y) < 1e-6);
    }
}

#[test]
fn looking_straight_in_sees_straight_out_of_the_other_side() {
    let (w, _) = pair(0.5, 0.05);
    for n in directions() {
        let (sky, _) = march(&w, 5.0, n, -n);
        let RaySky::Far(d) = sky else {
            panic!("{sky:?}")
        };
        assert!(d.distance(far_frame(n, -n)) < 1e-4);
        let (sky, _) = march(&w, 5.0, n, n);
        let RaySky::Near(d) = sky else {
            panic!("{sky:?}")
        };
        assert!(d.distance(n) < 1e-5);
    }
}

#[test]
fn the_two_sides_are_alike() {
    // Swapping the sides is a symmetry: a camera at −ℓ looking along v sees on the near sky what
    // the one at ℓ looking along the same v, mirrored along its radius, sees on the far one.
    let (w, _) = pair(0.5, 0.05);
    let n = Vec3::Z;
    for v in directions() {
        let mirrored = v - 2.0 * v.dot(n) * n;
        let (a, _) = march(&w, 4.0, n, v);
        let (b, _) = march(&w, -4.0, n, mirrored);
        match (a, b) {
            (RaySky::Near(_), RaySky::Far(_)) | (RaySky::Far(_), RaySky::Near(_)) => {}
            (RaySky::Lost, RaySky::Lost) => {}
            other => panic!("{v}: {other:?}"),
        }
    }
}

#[test]
fn the_mouth_from_outside_is_as_wide_as_its_impact_parameter() {
    // From radius r, rays within asin(ρ/r) of straight in cross; the rest do not.
    let (w, shape) = pair(0.5, 0.05);
    let l = 10.0_f64;
    let edge = (1.0 / shape.radius(l)).asin() as f32;
    for (angle, crosses) in [(0.97 * edge, true), (1.03 * edge, false)] {
        let direction = Vec3::new(angle.sin(), 0.0, -angle.cos());
        let (sky, _) = march(&w, l as f32, Vec3::Z, direction);
        assert_eq!(matches!(sky, RaySky::Far(_)), crosses, "{angle} {sky:?}");
    }
}

#[test]
fn there_is_an_einstein_ring() {
    // Some direction outside the mouth's image sees the point straight behind the mouth.
    let (w, _) = pair(0.5, 0.05);
    let behind = -Vec3::Z;
    let closest = (0..4000)
        .filter_map(|i| {
            let angle = 0.105 + i as f32 * 5e-5;
            let direction = Vec3::new(angle.sin(), 0.0, -angle.cos());
            match march(&w, 10.0, Vec3::Z, direction).0 {
                RaySky::Near(d) => Some((d.distance(behind), angle)),
                _ => None,
            }
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    assert!(closest.0 < 5e-3, "{closest:?}");
}

#[test]
fn the_noise_is_smooth_and_the_nebula_is_partial() {
    let mut bright = 0;
    for d in directions() {
        let p = 5.0 * d;
        let v = value_noise(p);
        assert!((0.0..=1.0).contains(&v));
        assert!((value_noise(p + Vec3::splat(1e-4)) - v).abs() < 2e-3);
        let glow = nebula(d);
        assert!(glow.min_element() >= 0.0);
        bright += usize::from(glow.max_element() > 0.0);
    }
    assert!(bright > 20 && bright < 200, "{bright} of 200");
}

#[test]
fn the_two_skies_differ() {
    let differ = directions()
        .into_iter()
        .filter(|&d| near_sky(d, true) != far_sky(d, true))
        .count();
    assert!(differ > 50, "{differ}");
    assert_ne!(pattern(Vec3::Z, false), pattern(Vec3::Z, true));
}

/// The `f64` place and label-frame direction a distance `far` along a ray from `l`, label `n`.
fn along(
    shape: &WormholeShape,
    l: f64,
    n: Vec3,
    direction: Vec3,
    far: f64,
) -> (crate::core::wormhole::WormholePlace, bevy::math::DVec3) {
    let place = crate::core::wormhole::WormholePlace { l, n: n.as_dvec3() };
    let direction = direction.as_dvec3().normalize();
    let (end, carried) = crate::core::wormhole::travel(shape, place, direction, far);
    (end, carried * direction)
}

#[test]
fn a_ray_meets_a_body_on_its_path_and_misses_one_beside_it() {
    for (half, width) in SHAPES {
        let (w, shape) = pair(half, width);
        for l in [6.0, 0.1, -3.0] {
            for direction in directions().into_iter().step_by(3) {
                let n = Vec3::new(0.6, 0.64, -0.48);
                let across = direction - direction.dot(n) * n;
                let impact = shape.radius(l) * f64::from(across.length());
                if (impact - 1.0).abs() < 0.15 {
                    continue;
                }
                for far in [0.8, 3.0, 9.0, 20.0] {
                    let (at, _) = along(&shape, l, n, direction, far);
                    let body = |l: f64, n: bevy::math::DVec3| RayBody {
                        n: n.as_vec3(),
                        l: l as f32,
                        radius: 0.1,
                    };
                    let (end, _) = march_past(&w, &[body(at.l, at.n)], l as f32, n, direction);
                    assert!(
                        matches!(end, RayEnd::Body(0, normal) if (normal.length() - 1.0).abs() < 1e-2),
                        "a {half} W {width}: {l} {direction} {far} {at:?}: {end:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_body_far_from_the_mouth_is_seen_as_wide_as_it_is() {
    // Looking away from the mouth, space is flat: a sphere covers asin(R/D).
    let (w, shape) = pair(0.5, 0.05);
    let n = Vec3::Z;
    let camera = 50.0;
    let (at, _) = along(&shape, camera, n, Vec3::new(0.8, 0.0, 0.6), 10.0);
    let body = RayBody {
        n: at.n.as_vec3(),
        l: at.l as f32,
        radius: 1.0,
    };
    let edge = (1.0_f32 / 10.0).asin();
    for (angle, seen) in [(0.0, true), (0.97 * edge, true), (1.03 * edge, false)] {
        let (sin, cos) = angle.sin_cos();
        let direction = cos * Vec3::new(0.8, 0.0, 0.6) + sin * Vec3::Y;
        let (end, _) = march_past(&w, &[body], camera as f32, n, direction);
        assert_eq!(matches!(end, RayEnd::Body(..)), seen, "{angle} {end:?}");
    }
}

#[test]
fn a_body_straight_behind_the_mouth_is_seen_as_a_ring() {
    // Not through the mouth, but bent round it in every direction alike.
    let (w, shape) = pair(0.5, 0.05);
    let l = shape.distance_at_radius(8.0) as f32;
    let body = RayBody {
        n: Vec3::NEG_Z,
        l,
        radius: 0.5,
    };
    let seen = |angle: f32, turn: f32| {
        let (sin, cos) = angle.sin_cos();
        let direction = Vec3::new(sin * turn.cos(), sin * turn.sin(), -cos);
        matches!(
            march_past(&w, &[body], 10.0, Vec3::Z, direction).0,
            RayEnd::Body(..)
        )
    };
    assert!(!seen(0.0, 0.0));
    let mouth = 1.0 / radius(&w, 10.0);
    let angles: Vec<f32> = (0..400)
        .map(|i| mouth + i as f32 * 5e-4)
        .filter(|&angle| seen(angle, 0.0))
        .collect();
    assert!(!angles.is_empty());
    let middle = angles[angles.len() / 2];
    for i in 0..16 {
        assert!(seen(middle, i as f32 * 0.4));
    }
}

#[test]
fn a_body_on_the_far_side_is_seen_only_through_the_mouth() {
    let (w, _) = pair(0.5, 0.05);
    let body = RayBody {
        n: Vec3::Z,
        l: -6.0,
        radius: 1.0,
    };
    // Straight in, it is there; where it would be drawn in the near side's space, it is not.
    let (end, _) = march_past(&w, &[body], 10.0, Vec3::Z, Vec3::NEG_Z);
    assert!(matches!(end, RayEnd::Body(0, _)), "{end:?}");
    let (end, _) = march_past(&w, &[body], 4.0, Vec3::Z, Vec3::Z);
    assert!(matches!(end, RayEnd::Sky(RaySky::Near(_))), "{end:?}");
    let mouth = (1.0 / radius(&w, 10.0)).asin();
    for i in 0..300 {
        let angle = 1.02 * mouth + i as f32 * 0.01;
        let direction = Vec3::new(angle.sin(), 0.0, -angle.cos());
        let (end, _) = march_past(&w, &[body], 10.0, Vec3::Z, direction);
        assert!(!matches!(end, RayEnd::Body(..)), "{angle}");
    }
}

#[test]
fn the_nearest_of_two_bodies_is_the_one_seen() {
    let (w, _) = pair(0.5, 0.05);
    let body = |l: f32| RayBody {
        n: Vec3::Z,
        l,
        radius: 0.4,
    };
    for (l, first) in [([4.0, 6.0], 1), ([6.0, 4.0], 0), ([0.0, -4.0], 0)] {
        let bodies = l.map(body);
        let (end, _) = march_past(&w, &bodies, 10.0, Vec3::Z, Vec3::NEG_Z);
        let RayEnd::Body(index, normal) = end else {
            panic!("{end:?}")
        };
        assert_eq!(index, first);
        // Its face towards the camera.
        assert!(normal.distance(Vec3::Z) < 1e-3, "{normal}");
    }
}

#[test]
fn a_body_is_lit_by_its_star_and_a_star_by_itself() {
    let c = Vec3::new(0.5, 0.4, 0.3);
    let facing = body_light(c, false, Vec3::X, Vec3::X);
    let away = body_light(c, false, Vec3::NEG_X, Vec3::X);
    let alone = body_light(c, false, Vec3::X, Vec3::ZERO);
    assert!(facing.x > 5.0 * away.x && away.x > 0.0);
    assert_eq!(alone, away);
    assert_eq!(
        body_light(c, true, Vec3::X, Vec3::ZERO),
        c * WORMHOLE_BODY_GLOW
    );
}
