//! Unit tests for `kerr_ray.rs`, the `f32` twin of the GPU ray-marcher: against the `f64` Kerr code,
//! the analytic shadow, and its own conservation laws.
//!
//! The scene is `experiment_6`'s: Sgr A* at spin `0.9`, a disc out to `20 r_g`, and the opening
//! camera `10°` above the disc, `53 r_g` out.

use bevy::math::Vec3;

use super::*;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::kerr::{
    equatorial_coordinates as equatorial_coordinates_f64, isco_radius, kerr_schild_acceleration,
    kerr_schild_light_velocity, kerr_schild_metric, kerr_schild_radius, photon_orbit_radius,
    KerrBlackHole,
};
use crate::render::accretion_disc::{DiscParticle, DiscProfile};

const MASS: f64 = 4.3e6 * 1.988_47e30;
const SPIN: f64 = 0.9;
const OUTER_RG: f64 = 20.0;

fn r_g() -> f64 {
    KerrBlackHole::new(MASS, SPIN)
        .unwrap()
        .gravitational_radius_m()
}

fn hole() -> RayHole {
    RayHole::new(SPIN, OUTER_RG).unwrap()
}

/// The opening camera, `(0, 58.5, 331)` world units, in Kerr–Schild axes `[z, x, y]` and `r_g`.
fn camera() -> Vec3 {
    let r_g = r_g();
    Vec3::new((331.0e9 / r_g) as f32, 0.0, (58.5e9 / r_g) as f32)
}

/// The direction from `camera` through the point `right`, `up` (in `r_g`) from the hole's centre,
/// in the plane through the hole facing the camera.
fn aim(camera: Vec3, right: f32, up: f32) -> Vec3 {
    let forward = -camera.normalize();
    let mut side = forward.cross(Vec3::Z);
    if side.length() < 1.0e-6 {
        side = Vec3::Y;
    }
    let side = side.normalize();
    let top = side.cross(forward);
    (right * side + up * top - camera).normalize()
}

/// The same backward ray in `f64`, stepped in coordinate time with
/// [`kerr_schild_acceleration`] at a third of the twin's step: the reference. Returns each plane
/// crossing's `(r, φ, P)` up to the first on the disc, and whether it ended in the shadow.
fn reference(camera_rg: Vec3, direction: Vec3) -> (Vec<(f64, f64, [f64; 3])>, bool) {
    let r_g = r_g();
    let isco = isco_radius(SPIN, true).unwrap();
    let trapped = photon_orbit_radius(SPIN, true).unwrap();
    let horizon = crate::physics_utils::relativity::kerr::horizon_radius(SPIN).unwrap();
    let to = |v: Vec3| [v.x as f64, v.y as f64, v.z as f64];
    let add = |a: [f64; 3], b: [f64; 3], s: f64| [0, 1, 2].map(|i| a[i] + s * b[i]);
    let dot = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| a[i] * b[i]).sum::<f64>();
    let accel = |x, v| kerr_schild_acceleration(MASS, SPIN, x, v);

    let camera_m = to(camera_rg).map(|c| c * r_g);
    let mut x = camera_m;
    let mut v = kerr_schild_light_velocity(MASS, SPIN, x, to(-direction)).unwrap();
    let escape = dot(x, x).sqrt().max(1.5 * OUTER_RG * r_g) * 1.05;
    let mut crossings = Vec::new();
    for _ in 0..20_000 {
        let r = kerr_schild_radius(SPIN, x.map(|c| c / r_g));
        if r <= horizon * 1.001 || (r <= trapped && dot(x, v) > 0.0) {
            return (crossings, true);
        }
        if dot(x, x).sqrt() > escape && dot(x, v) < 0.0 {
            return (crossings, false);
        }
        let dt = -0.01 * dot(x, x).sqrt() / SPEED_OF_LIGHT_M_S;
        let a1 = accel(x, v).unwrap();
        let (x2, v2) = (add(x, v, 0.5 * dt), add(v, a1, 0.5 * dt));
        let a2 = accel(x2, v2).unwrap();
        let (x3, v3) = (add(x, v2, 0.5 * dt), add(v, a2, 0.5 * dt));
        let a3 = accel(x3, v3).unwrap();
        let (x4, v4) = (add(x, v3, dt), add(v, a3, dt));
        let a4 = accel(x4, v4).unwrap();
        let nx = [0, 1, 2].map(|i| x[i] + dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]));
        let nv = [0, 1, 2].map(|i| v[i] + dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]));
        if x[2] * nx[2] < 0.0 {
            let s = x[2] / (x[2] - nx[2]);
            let point = add(x, add(nx, x, -1.0), s);
            let velocity = add(v, add(nv, v, -1.0), s);
            let (r, phi) = equatorial_coordinates_f64(SPIN, point.map(|c| c / r_g));
            // P_i = p_i/(−p_t), lowering (1, v/c) with the metric.
            let g = kerr_schild_metric(SPIN, point.map(|c| c / r_g));
            let k = [
                1.0,
                velocity[0] / SPEED_OF_LIGHT_M_S,
                velocity[1] / SPEED_OF_LIGHT_M_S,
                velocity[2] / SPEED_OF_LIGHT_M_S,
            ];
            let lower = |mu: usize| (0..4).map(|nu| g[mu][nu] * k[nu]).sum::<f64>();
            let p_t = lower(0);
            crossings.push((r, phi, [1, 2, 3].map(|i| lower(i) / -p_t)));
            if (isco..=OUTER_RG).contains(&r) {
                return (crossings, false);
            }
        }
        x = nx;
        v = nv;
    }
    panic!("the reference ray never ended");
}

/// The smallest difference between two angles.
fn angle_between(a: f64, b: f64) -> f64 {
    (a - b + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

#[test]
fn the_starting_momentum_is_null_and_sends_the_light_at_the_camera() {
    let camera = camera();
    for direction in [aim(camera, 0.0, 6.0), aim(camera, -12.0, -3.0), Vec3::X] {
        let p = start_momentum(SPIN as f32, camera, direction).unwrap();
        assert!(hamiltonian(SPIN as f32, camera, p).abs() < 1.0e-6);
        let (dx, _) = rates(SPIN as f32, camera, p);
        assert!(
            dx.normalize().dot(-direction) > 1.0 - 1.0e-6,
            "towards the camera"
        );
    }
    // Far away space is flat, and p is the unit vector the light moves along.
    let far = Vec3::new(1.0e4, 0.0, 0.0);
    let p = start_momentum(SPIN as f32, far, -Vec3::X).unwrap();
    assert!((p - Vec3::X).length() < 1.0e-3);
}

#[test]
fn f32_keeps_light_null_and_its_conserved_momenta_along_a_ray_round_the_photon_orbit() {
    // Just outside the shadow's edge on the side going away, light swings round the hole before
    // reaching the camera.
    let mut hole = RayHole::new(SPIN, 0.0).unwrap();
    hole.max_crossings = u32::MAX;
    let camera = camera();
    let a = hole.spin;
    let side = (aim(camera, 1.0, 0.0) - aim(camera, 0.0, 0.0)).normalize();
    let edge = shadow_edge(&hole, camera, side);
    let mut x = camera;
    let mut p = start_momentum(a, camera, aim(camera, 1.01 * edge, 0.0)).unwrap();
    let axial = |x: Vec3, p: Vec3| x.x * p.y - x.y * p.x;
    let l0 = axial(x, p);
    let (mut worst_h, mut worst_l, mut closest) = (0.0f32, 0.0f32, f32::MAX);
    for _ in 0..2000 {
        let (dx, _) = rates(a, x, p);
        let (nx, np) = rk4(a, x, p, -hole.step_fraction * x.length() / dx.length());
        x = nx;
        p = np;
        let r = radius(a, x);
        closest = closest.min(r);
        if x.length() > 1.05 * camera.length() || r < hole.horizon * 1.01 {
            break;
        }
        worst_h = worst_h.max(hamiltonian(a, x, p).abs());
        worst_l = worst_l.max((axial(x, p) - l0).abs() / l0.abs());
    }
    assert!(closest < 4.5, "the ray went close in: {closest}");
    assert!(worst_h < 1.0e-4, "|H| up to {worst_h}");
    assert!(worst_l < 1.0e-4, "p_φ drifted by {worst_l}");
}

#[test]
fn the_twin_finds_the_same_crossings_as_the_f64_tracer() {
    // Rays over the whole view: the near side, both sides, the far side arched over the top, and
    // the underside below the hole.
    let camera = camera();
    let hole = hole();
    let rays = [
        (0.0, -4.0),
        (0.0, -10.0),
        (-14.0, 0.0),
        (14.0, 0.0),
        (-6.0, 3.0),
        (0.0, 6.0),
        (0.0, 12.0),
        (5.0, 9.0),
        (0.0, -4.4),
        (-3.0, -4.0),
    ];
    for (right, up) in rays {
        let direction = aim(camera, right, up);
        let trace = march(&hole, camera, direction);
        let (expected, shadow) = reference(camera, direction);
        let found: Vec<RayCrossing> = trace.crossings.iter().flatten().copied().collect();
        assert_eq!(
            found.len(),
            expected.len(),
            "({right}, {up}): {found:?} vs {expected:?}"
        );
        assert_eq!(trace.end == RayEnd::Shadow, shadow, "({right}, {up})");
        for (crossing, &(r, phi, _)) in found.iter().zip(&expected) {
            let dr = (crossing.radius as f64 - r).abs();
            let dphi = angle_between(crossing.azimuth as f64, phi).abs();
            assert!(
                dr < 0.005 * r + 0.01 && dphi < 0.005,
                "({right}, {up}) order {}: r {} vs {r}, φ {} vs {phi}",
                crossing.order,
                crossing.radius,
                crossing.azimuth
            );
        }
    }
}

#[test]
fn the_far_side_is_seen_arched_over_the_hole() {
    // A point 6 r_g behind the hole, which along straight lines would sit 1.4 r_g above its centre,
    // is seen about 6 r_g above it.
    let camera = camera();
    let hit = march(&hole(), camera, aim(camera, 0.0, 6.2))
        .disc()
        .expect("reaches the disc");
    assert_eq!(hit.order, 0);
    assert!((5.0..7.0).contains(&hit.radius), "r = {}", hit.radius);
    assert!(angle_between(hit.azimuth as f64, std::f64::consts::PI).abs() < 0.1);
}

#[test]
fn the_underside_is_seen_below_the_hole() {
    // Light passing below the near side's edge, then under the hole, is bent up onto the far side's
    // underside: a second-order image behind the hole, below the shadow.
    let camera = camera();
    let found: Vec<(f32, u32, f32, f32)> = (0..60)
        .map(|k| -3.0 - 0.1 * k as f32)
        .filter_map(|up| {
            let hit = march(&hole(), camera, aim(camera, 0.0, up)).disc()?;
            Some((up, hit.order, hit.radius, hit.azimuth))
        })
        .collect();
    let underside = found
        .iter()
        .filter(|&&(_, order, _, phi)| {
            order == 1 && angle_between(phi as f64, std::f64::consts::PI).abs() < 0.3
        })
        .count();
    assert!(underside >= 3, "{found:?}");
}

/// The shadow's edge along `side` (a unit vector across the view), in `r_g` from the centre, found
/// by bisection.
fn shadow_edge(hole: &RayHole, camera: Vec3, side: Vec3) -> f32 {
    let in_shadow =
        |b: f32| march(hole, camera, (b * side - camera).normalize()).end == RayEnd::Shadow;
    let (mut inside, mut outside) = (0.0f32, 12.0f32);
    assert!(in_shadow(inside) && !in_shadow(outside));
    for _ in 0..30 {
        let mid = 0.5 * (inside + outside);
        if in_shadow(mid) {
            inside = mid;
        } else {
            outside = mid;
        }
    }
    0.5 * (inside + outside)
}

/// Bardeen's (1973) critical impact parameters for spherical photon orbits at radius `r`:
/// `ξ = L/E` and `η = Q/E²`.
fn bardeen(r: f64) -> (f64, f64) {
    let a = SPIN;
    let xi = (r * r * (3.0 - r) - a * a * (r + 1.0)) / (a * (r - 1.0));
    let eta = r.powi(3) * (4.0 * a * a - r * (r - 3.0).powi(2)) / (a * a * (r - 1.0).powi(2));
    (xi, eta)
}

#[test]
fn the_shadow_seen_edge_on_matches_bardeen() {
    // Seen from the equator the shadow's edges along it are the equatorial photon orbits,
    // `α = −ξ`: close in on the side coming towards the camera (prograde, `r = 1.56`), far out on
    // the side going away (retrograde, `r = 3.91`).
    let mut hole = RayHole::new(SPIN, 0.0).unwrap();
    hole.max_crossings = u32::MAX;
    let camera = Vec3::new(1000.0, 0.0, 0.0);
    // From +x, +y is to the right and the gas on the left (−y) comes towards the camera.
    let coming = shadow_edge(&hole, camera, -Vec3::Y) as f64;
    let going = shadow_edge(&hole, camera, Vec3::Y) as f64;
    let (xi_pro, _) = bardeen(photon_orbit_radius(SPIN, true).unwrap());
    let (xi_retro, _) = bardeen(photon_orbit_radius(SPIN, false).unwrap());
    assert!(
        (coming / xi_pro - 1.0).abs() < 0.01,
        "coming side {coming} vs {xi_pro}"
    );
    assert!(
        (going / -xi_retro - 1.0).abs() < 0.01,
        "going side {going} vs {}",
        -xi_retro
    );
}

#[test]
fn the_shadow_seen_from_the_pole_is_bardeens_circle() {
    // From the axis only photons with `ξ = 0` arrive; the shadow is a circle of radius √(η + a²).
    let mut hole = RayHole::new(SPIN, 0.0).unwrap();
    hole.max_crossings = u32::MAX;
    let camera = Vec3::new(0.0, 0.0, 1000.0);
    let (mut lo, mut hi) = (
        photon_orbit_radius(SPIN, true).unwrap(),
        photon_orbit_radius(SPIN, false).unwrap(),
    );
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if bardeen(mid).0 > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (_, eta) = bardeen(0.5 * (lo + hi));
    let expected = (eta + SPIN * SPIN).sqrt();
    for angle in [0.0f32, 1.0, 2.5, 4.0] {
        let side = Vec3::new(angle.cos(), angle.sin(), 0.0);
        let edge = shadow_edge(&hole, camera, side) as f64;
        assert!(
            (edge / expected - 1.0).abs() < 0.01,
            "at {angle} rad: {edge} vs {expected}"
        );
    }
}

#[test]
fn face_on_the_redshift_is_the_orbits_time_dilation() {
    // Light reaching the axis has no angular momentum about it, so the Doppler term vanishes and
    // `g = 1/(lapse · u^t)`, with `u^t` the circular orbit's.
    let hole = hole();
    let camera = Vec3::new(0.0, 0.0, 800.0);
    let lapse = camera_lapse(hole.spin, camera).unwrap();
    for b in [6.0f32, 10.0, 16.0] {
        let hit = march(&hole, camera, (b * Vec3::X - camera).normalize())
            .disc()
            .unwrap();
        let r = hit.radius as f64;
        let a = SPIN;
        let u_t =
            (r.powf(1.5) + a) / (r.powf(0.75) * (r.powf(1.5) - 3.0 * r.sqrt() + 2.0 * a).sqrt());
        let expected = 1.0 / (lapse as f64 * u_t);
        let g = redshift(&hole, &hit, lapse).unwrap() as f64;
        assert!(
            (g / expected - 1.0).abs() < 1.0e-3,
            "b = {b}: {g} vs {expected}"
        );
    }
}

#[test]
fn the_redshift_along_the_bent_path_matches_the_discs_own() {
    // The twin's momentum at the disc, put through its redshift, against the disc's `f64` redshift
    // with the reference tracer's emission direction.
    let camera = camera();
    let hole = hole();
    let lapse = camera_lapse(hole.spin, camera).unwrap();
    let profile = DiscProfile::new(MASS, SPIN).unwrap();
    for (right, up) in [
        (-12.0, -1.0),
        (12.0, -1.0),
        (0.0, 6.2),
        (0.0, -6.0),
        (-8.0, 2.0),
    ] {
        let direction = aim(camera, right, up);
        let hit = march(&hole, camera, direction).disc().unwrap();
        let (expected, _) = reference(camera, direction);
        let &(r, phi, emission) = expected.last().unwrap();
        let clump = DiscParticle {
            radius_rg: r,
            azimuth_rad: phi,
            plunge: None,
        };
        let truth = clump.redshift_factor_along(&profile, emission, lapse as f64);
        let g = redshift(&hole, &hit, lapse).unwrap() as f64;
        assert!(
            (g / truth - 1.0).abs() < 2.0e-3,
            "({right}, {up}): {g} vs {truth}"
        );
    }
    // And the side coming towards the camera is blueshifted, the side going away redshifted.
    let g = |right| {
        let hit = march(&hole, camera, aim(camera, right, -1.0))
            .disc()
            .unwrap();
        redshift(&hole, &hit, lapse).unwrap()
    };
    assert!(g(-12.0) > 1.0 && g(12.0) < 1.0);
}

#[test]
fn light_from_the_middle_falls_in_and_light_from_far_off_comes_from_the_sky() {
    let camera = camera();
    let hole = hole();
    assert_eq!(
        march(&hole, camera, aim(camera, 0.5, 0.0)).end,
        RayEnd::Shadow
    );

    let direction = aim(camera, 0.0, 60.0);
    let trace = march(&hole, camera, direction);
    let RayEnd::Sky { direction: sky } = trace.end else {
        panic!("{trace:?}");
    };
    // Bent by about 4/b radians at impact parameter b.
    let bend = sky.dot(direction).clamp(-1.0, 1.0).acos();
    assert!(bend < 0.1 && bend > 0.01, "bent {bend} rad");
}

#[test]
fn rays_over_the_view_take_a_few_hundred_steps() {
    // The shader's cost: every pixel over the disc's apparent size.
    let camera = camera();
    let hole = hole();
    let (mut total, mut worst, mut count) = (0u32, 0u32, 0u32);
    for i in -12..=12 {
        for j in -12..=12 {
            let trace = march(&hole, camera, aim(camera, i as f32 * 2.0, j as f32 * 2.0));
            assert_ne!(trace.end, RayEnd::Invalid);
            total += trace.steps;
            worst = worst.max(trace.steps);
            count += 1;
        }
    }
    let mean = total / count;
    assert!(mean < 250, "mean {mean} steps");
    assert!(worst < hole.max_steps, "worst {worst} steps");
}

#[test]
fn an_unusable_hole_or_a_camera_inside_the_horizon_marches_nothing() {
    assert!(RayHole::new(1.5, OUTER_RG).is_none());
    assert!(RayHole::new(SPIN, f64::NAN).is_none());
    let trace = march(&hole(), Vec3::new(0.0, 0.0, 1.0), Vec3::X);
    assert_eq!(trace.end, RayEnd::Invalid);
    assert_eq!(trace.steps, 0);
    assert!(start_momentum(SPIN as f32, camera(), Vec3::ZERO).is_none());
}

#[test]
fn the_f32_pieces_agree_with_the_f64_metric() {
    // f and k from the twin against the `f64` Kerr–Schild metric, g_tt = −1 + f and g_ti = f k_i.
    for x in [
        Vec3::new(5.0, 1.0, 2.0),
        Vec3::new(-2.0, 0.5, 0.3),
        Vec3::new(0.1, 0.2, 3.0),
        Vec3::new(40.0, -12.0, 7.0),
    ] {
        let m = metric_pieces(SPIN as f32, x);
        let g = kerr_schild_metric(SPIN, [x.x as f64, x.y as f64, x.z as f64]);
        assert!((m.f as f64 - (g[0][0] + 1.0)).abs() < 1.0e-5);
        for i in 0..3 {
            assert!((m.f as f64 * m.k[i] as f64 - g[0][i + 1]).abs() < 1.0e-5);
        }
        // The derivatives against central differences of the twin's own f and k.
        let h = 1.0e-2;
        for j in 0..3 {
            let mut step = Vec3::ZERO;
            step[j] = h;
            let (plus, minus) = (
                metric_pieces(SPIN as f32, x + step),
                metric_pieces(SPIN as f32, x - step),
            );
            let df = (plus.f - minus.f) / (2.0 * h);
            assert!(
                (df - m.grad_f[j]).abs() < 1.0e-3 * (1.0 + df.abs()),
                "∂_{j} f"
            );
            let dk = (plus.k - minus.k) / (2.0 * h);
            let analytic = Vec3::new(m.grad_k.x_axis[j], m.grad_k.y_axis[j], m.grad_k.z_axis[j]);
            assert!(
                (dk - analytic).length() < 1.0e-3,
                "∂_{j} k: {dk} vs {analytic}"
            );
        }
    }
}

#[test]
fn the_shading_table_reproduces_the_discs_profile() {
    let hole = hole();
    let profile = DiscProfile::new(MASS, SPIN).unwrap();
    let shading = DiscShading::new(&hole, &profile);
    let peak = profile.peak_temperature_k();
    for k in 0..200 {
        let r = hole.isco as f64 + (OUTER_RG - hole.isco as f64) * k as f64 / 199.0;
        let sample = profile_at(&hole, &shading, r as f32);
        let dt = (sample.x as f64 - profile.temperature_at(r)).abs() / peak;
        let df = (sample.y as f64 - profile.relative_flux_at(r)).abs();
        assert!(dt < 5.0e-3 && df < 5.0e-3, "r = {r}: {dt}, {df}");
    }
}

#[test]
fn the_f32_blackbody_matches_the_discs() {
    for t in [
        800.0, 1500.0, 3000.0, 5800.0, 6600.0, 9000.0, 20000.0, 1.0e6,
    ] {
        let twin = blackbody(t as f32);
        let reference = crate::render::accretion_disc::blackbody_rgb(t);
        for i in 0..3 {
            assert!((twin[i] - reference[i]).abs() < 1.0e-4, "{t} K channel {i}");
        }
    }
}

#[test]
fn the_disc_shines_with_g_to_the_fourth_times_its_flux_at_g_times_its_temperature() {
    // The twin's light at a crossing against the same formula built from `f64` pieces: the
    // reference tracer's redshift (`DiscParticle::redshift_factor_along`), and the disc's own
    // temperature and flux (`DiscProfile`), with the CPU blackbody.
    let camera = camera();
    let hole = hole();
    let lapse = camera_lapse(hole.spin, camera).unwrap();
    let profile = DiscProfile::new(MASS, SPIN).unwrap();
    let shading = DiscShading::new(&hole, &profile);
    for (right, up) in [
        (-12.0, -1.0),
        (12.0, -1.0),
        (0.0, 6.2),
        (-5.0, 1.5),
        (16.0, 2.0),
    ] {
        let direction = aim(camera, right, up);
        let hit = march(&hole, camera, direction).disc().unwrap();
        let light = disc_colour(&hole, &shading, &hit, lapse).unwrap();
        let (expected, _) = reference(camera, direction);
        let &(r, phi, emission) = expected.last().unwrap();
        let clump = DiscParticle {
            radius_rg: r,
            azimuth_rad: phi,
            plunge: None,
        };
        let g = clump.redshift_factor_along(&profile, emission, lapse as f64);
        let colour = crate::render::accretion_disc::blackbody_rgb(g * profile.temperature_at(r));
        let intensity = g.powi(4) * profile.relative_flux_at(r);
        for i in 0..3 {
            let truth = colour[i] as f64 * intensity;
            assert!(
                (light[i] as f64 - truth).abs() < 0.01 * intensity.max(0.01) + 0.03 * truth,
                "({right}, {up}) channel {i}: {} vs {truth}",
                light[i]
            );
        }
    }
    // At the horizon there is no gas to see.
    let mut inside = march(&hole, camera, aim(camera, -12.0, -1.0))
        .disc()
        .unwrap();
    inside.radius = hole.horizon;
    assert!(disc_colour(&hole, &shading, &inside, lapse).is_none());
}

#[test]
fn the_picture_is_tone_mapped_like_film() {
    // Faint light in proportion, bright light towards white without clipping, never negative.
    let exposure = 6.0;
    let faint = tone_map(Vec3::splat(0.001), exposure);
    assert!((faint.x - 0.006).abs() < 1.0e-4);
    let bright = tone_map(Vec3::splat(0.8), exposure);
    assert!(bright.x > 0.99 && bright.x < 1.0);
    assert_eq!(tone_map(Vec3::splat(-1.0), exposure), Vec3::ZERO);
    let a = tone_map(Vec3::splat(0.2), exposure).x;
    let b = tone_map(Vec3::splat(0.4), exposure).x;
    assert!(b > a && b < 2.0 * a, "brighter, but compressed");
}

/// A crossing of the equatorial plane at Boyer–Lindquist `r`, `phi`, with light momentum `p`.
fn crossing_at(r: f32, phi: f32, p: Vec3) -> RayCrossing {
    let a = SPIN as f32;
    let position = Vec3::new(
        r * phi.cos() + a * phi.sin(),
        r * phi.sin() - a * phi.cos(),
        0.0,
    );
    RayCrossing {
        order: 0,
        radius: r,
        azimuth: phi,
        position,
        momentum: p,
        on_disc: r >= hole().isco,
    }
}

/// `g_μν u^μ u^ν` for `u = (u_t, u)` at `x`: `−1` for a body's four-velocity.
fn four_norm(x: Vec3, u_t: f32, u: Vec3) -> f32 {
    let m = metric_pieces(SPIN as f32, x);
    let k_u = u_t + m.k.dot(u);
    -u_t * u_t + u.dot(u) + m.f * k_u * k_u
}

#[test]
fn plunging_gas_moves_on_a_real_trajectory_inside_the_isco() {
    let hole = hole();
    for r in [2.3f32, 2.1, 1.9, 1.7, 1.5] {
        let crossing = crossing_at(r, 0.7, Vec3::ZERO);
        let (u_t, u) = emitter(&hole, &crossing).unwrap();
        // A body's four-velocity is unit and timelike in the Kerr–Schild metric: this checks the
        // Boyer–Lindquist plunge and its change of coordinates together.
        let norm = four_norm(crossing.position, u_t, u);
        assert!((norm + 1.0).abs() < 2.0e-3, "r = {r}: norm {norm}");
        // Falling in, and still swept round with the spin.
        assert!(u.dot(crossing.position) < 0.0, "r = {r} falls inward");
        let x = crossing.position;
        assert!(x.x * u.y - x.y * u.x > 0.0, "r = {r} turns prograde");
    }
    // At the ISCO the plunge starts as the circular orbit.
    let just_inside = emitter(&hole, &crossing_at(hole.isco * 0.9999, 0.7, Vec3::ZERO)).unwrap();
    let on_orbit = emitter(&hole, &crossing_at(hole.isco, 0.7, Vec3::ZERO)).unwrap();
    assert!((just_inside.0 / on_orbit.0 - 1.0).abs() < 0.01);
    assert!((just_inside.1 - on_orbit.1).length() < 0.02 * on_orbit.1.length());
    // And the circular one is unit too.
    let orbit = crossing_at(8.0, 2.0, Vec3::ZERO);
    let (u_t, u) = emitter(&hole, &orbit).unwrap();
    assert!((four_norm(orbit.position, u_t, u) + 1.0).abs() < 1.0e-4);
    assert!(emitter(&hole, &crossing_at(hole.horizon, 0.0, Vec3::ZERO)).is_none());
}

#[test]
fn plunging_gas_matches_the_simulations_falling_clumps() {
    // The simulation drops a clump from the ISCO at `DISC_PLUNGE_SPEED_FACTOR` of the orbital speed
    // and follows it with the `f64` equation of motion. Done here the same way, its velocity on the
    // way in agrees with the twin's plunge.
    let hole = hole();
    let black_hole = KerrBlackHole::new(MASS, SPIN).unwrap();
    let (r_g, t_g) = (
        black_hole.gravitational_radius_m(),
        black_hole.gravitational_time_s(),
    );
    let isco = isco_radius(SPIN, true).unwrap();
    let omega = crate::physics_utils::relativity::kerr::circular_orbit(SPIN, isco, true)
        .unwrap()
        .angular_velocity
        / t_g;
    let factor = crate::render::constants::DISC_PLUNGE_SPEED_FACTOR;
    let mut x = [isco * r_g, -SPIN * r_g, 0.0];
    let mut v = [
        SPIN * r_g * omega * factor,
        isco * r_g * omega * factor,
        0.0,
    ];
    let add = |a: [f64; 3], b: [f64; 3], s: f64| [0, 1, 2].map(|i| a[i] + s * b[i]);
    let accel = |x, v| kerr_schild_acceleration(MASS, SPIN, x, v).unwrap();
    let mut checked = 0;
    for _ in 0..200_000 {
        let r = kerr_schild_radius(SPIN, x.map(|c| c / r_g)) as f32;
        if r < hole.horizon * 1.15 {
            break;
        }
        if r < hole.isco * 0.97 {
            let position = Vec3::new((x[0] / r_g) as f32, (x[1] / r_g) as f32, 0.0);
            let (_, phi) = equatorial_coordinates(SPIN as f32, position);
            let (u_t, u) = emitter(&hole, &crossing_at(r, phi, Vec3::ZERO)).unwrap();
            let analytic = u / u_t;
            let simulated = Vec3::from(v.map(|c| (c / SPEED_OF_LIGHT_M_S) as f32));
            assert!(
                (analytic - simulated).length() < 0.05 * simulated.length(),
                "r = {r}: analytic {analytic}, simulated {simulated}"
            );
            checked += 1;
        }
        let dt = 0.002 * x[0].hypot(x[1]) / SPEED_OF_LIGHT_M_S;
        let a1 = accel(x, v);
        let (x2, v2) = (add(x, v, 0.5 * dt), add(v, a1, 0.5 * dt));
        let a2 = accel(x2, v2);
        let (x3, v3) = (add(x, v2, 0.5 * dt), add(v, a2, 0.5 * dt));
        let a3 = accel(x3, v3);
        let (x4, v4) = (add(x, v3, dt), add(v, a3, dt));
        let a4 = accel(x4, v4);
        x = [0, 1, 2].map(|i| x[i] + dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]));
        v = [0, 1, 2].map(|i| v[i] + dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]));
    }
    assert!(checked > 20, "only {checked} points checked");
}

#[test]
fn the_gas_texture_spans_the_horizon_to_the_edge_and_wraps_round() {
    let hole = hole();
    assert_eq!(texture_coordinates(&hole, hole.horizon, 0.0), Vec2::ZERO);
    assert!((texture_coordinates(&hole, hole.outer, 0.0).y - 1.0).abs() < 1.0e-6);
    let a = texture_coordinates(&hole, 9.0, 1.0);
    let b = texture_coordinates(&hole, 9.0, 1.0 + std::f32::consts::TAU);
    let c = texture_coordinates(&hole, 9.0, 1.0 - std::f32::consts::TAU);
    assert!((a - b).length() < 1.0e-5 && (a - c).length() < 1.0e-5);
    assert!((texture_radius(&hole, a.y) - 9.0).abs() < 1.0e-4);
    // Half the texture's height is inside a quarter of the span: the plunge and inner disc get the
    // most texels.
    assert!(texture_radius(&hole, 0.5) < hole.horizon + 0.26 * (hole.outer - hole.horizon));
}

#[test]
fn fully_covered_gas_composites_as_the_opaque_disc() {
    let camera = camera();
    let hole = hole();
    let lapse = camera_lapse(hole.spin, camera).unwrap();
    let profile = DiscProfile::new(MASS, SPIN).unwrap();
    let shading = DiscShading::new(&hole, &profile);
    let solid_disc = |uv: Vec2| {
        if texture_radius(&hole, uv.y) >= hole.isco {
            1.0
        } else {
            0.0
        }
    };
    for (right, up) in [(-12.0, -1.0), (0.0, 6.2), (8.0, 3.0), (0.0, -6.0)] {
        let direction = aim(camera, right, up);
        let pixel = composite(&hole, &shading, camera, direction, lapse, solid_disc, false);
        let hit = march(&hole, camera, direction).disc().unwrap();
        let expected = tone_map(
            disc_colour(&hole, &shading, &hit, lapse).unwrap(),
            shading.exposure,
        );
        assert!((pixel.w - 1.0).abs() < 1.0e-6);
        assert!(
            (pixel.truncate() - expected).length() < 1.0e-3,
            "({right}, {up}): {pixel} vs {expected}"
        );
    }
    // The shadow is black and opaque; the sky is clear.
    assert_eq!(
        composite(
            &hole,
            &shading,
            camera,
            aim(camera, 0.3, 0.0),
            lapse,
            solid_disc,
            false
        ),
        Vec4::new(0.0, 0.0, 0.0, 1.0)
    );
    assert_eq!(
        composite(
            &hole,
            &shading,
            camera,
            aim(camera, 0.0, 60.0),
            lapse,
            solid_disc,
            false
        ),
        Vec4::ZERO
    );
}

#[test]
fn thin_gas_lets_the_images_behind_show_through() {
    // Half-covered gas everywhere: a ray through the near disc passes on to the underside image
    // behind it, and a ray past the edge sees nothing.
    let camera = camera();
    let hole = hole();
    let lapse = camera_lapse(hole.spin, camera).unwrap();
    let profile = DiscProfile::new(MASS, SPIN).unwrap();
    let shading = DiscShading::new(&hole, &profile);
    let half = |_: Vec2| 0.5;
    let direction = aim(camera, 0.0, -4.4);
    let pixel = composite(&hole, &shading, camera, direction, lapse, half, false);
    let crossings = march_with(&hole, camera, direction, |_| false)
        .crossings
        .iter()
        .flatten()
        .filter(|c| c.radius <= hole.outer && c.radius > hole.horizon * 1.01)
        .count();
    assert!(crossings >= 2, "{crossings} crossings");
    // Each crossing lets half through, unless the ray then falls in (opaque).
    assert!(pixel.w >= 0.75 - 1.0e-6, "alpha {}", pixel.w);
    let none = composite(&hole, &shading, camera, direction, lapse, |_| 0.0, false);
    assert!(
        none.w == 0.0 || none.w == 1.0,
        "only the shadow can be opaque without gas"
    );
}

#[test]
fn the_sky_direction_at_the_escape_radius_is_already_the_final_one() {
    // Backwards in time the light heads out, but forwards it is falling in, and in ingoing
    // Kerr–Schild coordinates ingoing light runs almost straight far from the hole: the bending the
    // straight-line estimate `(2/b)(1 − cos α)` would still expect after the escape radius is
    // already in the coordinates. So the march's direction at escape (55 r_g here) is the direction
    // on the sky, to a few 10⁻⁴ rad, checked against the same rays marched on to 20,000 r_g.
    let camera = camera();
    let mut hole = RayHole::new(SPIN, 0.0).unwrap();
    hole.max_crossings = u32::MAX;
    let a = hole.spin;
    for (right, up) in [(9.0, 0.0), (0.0, 12.0), (-7.0, -5.0), (20.0, 15.0)] {
        let direction = aim(camera, right, up);
        let RayEnd::Sky { direction: sky } = march(&hole, camera, direction).end else {
            panic!("({right}, {up}) reaches the sky");
        };
        let mut x = camera;
        let mut p = start_momentum(a, camera, direction).unwrap();
        loop {
            let (dx, _) = rates(a, x, p);
            if x.length() > 20_000.0 && x.dot(dx) < 0.0 {
                let error = sky.dot(-dx.normalize()).clamp(-1.0, 1.0).acos();
                assert!(error < 1.0e-3, "({right}, {up}): off by {error} rad");
                break;
            }
            (x, p) = rk4(a, x, p, -hole.step_fraction * x.length() / dx.length());
        }
    }
}

/// Directions spread evenly over the sky: a Fibonacci sphere.
fn sky_directions(count: usize) -> impl Iterator<Item = Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
    (0..count).map(move |i| {
        let z = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let ring = (1.0 - z * z).sqrt();
        let angle = golden * i as f32;
        Vec3::new(ring * angle.cos(), ring * angle.sin(), z)
    })
}

#[test]
fn stars_are_scattered_over_the_whole_sky() {
    // Count the directions that land on a star in each of the eight octants and in the polar caps
    // and the equatorial band: the same share everywhere, not only round the disc's plane.
    let mut octants = [0u32; 8];
    let mut samples = [0u32; 8];
    let (mut caps, mut band, mut cap_samples, mut band_samples) = (0u32, 0u32, 0u32, 0u32);
    for d in sky_directions(400_000) {
        let octant =
            usize::from(d.x > 0.0) | usize::from(d.y > 0.0) << 1 | usize::from(d.z > 0.0) << 2;
        let lit = star_field(d, 1.0).max_element() > 0.005;
        samples[octant] += 1;
        octants[octant] += u32::from(lit);
        if d.z.abs() > 0.7 {
            cap_samples += 1;
            caps += u32::from(lit);
        } else if d.z.abs() < 0.3 {
            band_samples += 1;
            band += u32::from(lit);
        }
    }
    let share = |hits: u32, of: u32| hits as f32 / of as f32;
    let shares: Vec<f32> = (0..8).map(|i| share(octants[i], samples[i])).collect();
    let mean = shares.iter().sum::<f32>() / 8.0;
    assert!(mean > 0.005, "some of the sky is stars: {mean}");
    for s in &shares {
        assert!((s / mean - 1.0).abs() < 0.2, "octant shares {shares:?}");
    }
    let (caps, band) = (share(caps, cap_samples), share(band, band_samples));
    assert!((caps / band - 1.0).abs() < 0.2, "caps {caps}, band {band}");
}

#[test]
fn stars_are_small_coloured_points_that_stay_put() {
    // A star's light falls off within a few of its widths, the same direction always gives the same
    // light, and the stars come in colours.
    let mut brightest = (0.0f32, Vec3::Z);
    for d in sky_directions(200_000) {
        let light = star_field(d, 1.0).max_element();
        if light > brightest.0 {
            brightest = (light, d);
        }
    }
    let (peak, d) = brightest;
    assert_eq!(star_field(d, 1.0), star_field(d, 1.0));
    let side = d.cross(Vec3::Z).normalize();
    let away = (d + 5.0 * STAR_SIZE_RAD * side).normalize();
    assert!(star_field(away, 1.0).max_element() < 0.05 * peak);
    let (mut red, mut blue) = (0, 0);
    for d in sky_directions(200_000) {
        let light = star_field(d, 1.0);
        if light.max_element() > 0.05 {
            if light.x > light.z {
                red += 1;
            } else {
                blue += 1;
            }
        }
    }
    assert!(red > 10 && blue > 10, "{red} red, {blue} blue");
}

#[test]
fn the_sky_behind_the_hole_is_lensed_into_the_picture() {
    // With stars on, a ray that misses the disc and the hole ends on the sky: opaque, and its
    // colour is the stars in its lensed direction.
    let camera = camera();
    let hole = hole();
    let lapse = camera_lapse(hole.spin, camera).unwrap();
    let profile = DiscProfile::new(MASS, SPIN).unwrap();
    let shading = DiscShading::new(&hole, &profile);
    let direction = aim(camera, 0.0, 25.0);
    let RayEnd::Sky { direction: sky } = march(&hole, camera, direction).end else {
        panic!("reaches the sky");
    };
    let pixel = composite(&hole, &shading, camera, direction, lapse, |_| 0.0, true);
    assert_eq!(pixel.w, 1.0);
    assert!(
        (pixel.truncate() - tone_map(star_field(sky, lapse), shading.exposure)).length() < 1.0e-6
    );
    // Light passing above the hole is bent towards it: the star seen there really lies lower, nearer
    // the line straight behind the hole.
    assert!(sky.z < direction.z - 0.05, "{sky} vs {direction}");
}

/// Where `horizon_measure` changes sign along `side`, in `r_g` from the centre in the plane through
/// the hole facing `camera`, by bisection.
fn horizon_edge(spin: f32, camera: Vec3, side: Vec3) -> f32 {
    let inside =
        |b: f32| horizon_measure(spin, camera, (b * side - camera).normalize()).unwrap() > 0.0;
    let (mut lo, mut hi) = (0.0f32, 4.0f32);
    assert!(inside(lo) && !inside(hi));
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        if inside(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn without_spin_the_horizon_outline_is_a_sphere_of_two_gravitational_radii() {
    // The sight line from `D` out to `b` across the hole passes `D b / √(D² + b²)` from its centre:
    // `2` when `b = 2D / √(D² − 4)`.
    let camera = Vec3::new(30.0, 0.0, 0.0);
    let expected = 2.0 * 30.0 / (30.0f32 * 30.0 - 4.0).sqrt();
    for side in [Vec3::Y, Vec3::Z, Vec3::new(0.0, 0.6, 0.8)] {
        let edge = horizon_edge(0.0, camera, side);
        assert!(
            (edge / expected - 1.0).abs() < 1.0e-4,
            "{edge} vs {expected}"
        );
    }
}

#[test]
fn the_spinning_horizon_outline_is_squashed_at_the_poles() {
    // From far off along the equator: `√(r₊² + a²) = √(2 r₊)` across the equator, `r₊` along the
    // axis.
    let spin = SPIN as f32;
    let r_plus = 1.0 + (1.0 - spin * spin).sqrt();
    let camera = Vec3::new(1.0e4, 0.0, 0.0);
    let across = horizon_edge(spin, camera, Vec3::Y);
    let along = horizon_edge(spin, camera, Vec3::Z);
    assert!(
        (across / (2.0 * r_plus).sqrt() - 1.0).abs() < 1.0e-3,
        "{across}"
    );
    assert!((along / r_plus - 1.0).abs() < 1.0e-3, "{along}");
    // From over the pole it is a circle of the equatorial radius.
    let over = Vec3::new(0.0, 0.0, 1.0e4);
    for side in [Vec3::X, Vec3::Y, Vec3::new(0.6, 0.8, 0.0)] {
        let edge = horizon_edge(spin, over, side);
        assert!(
            (edge / (2.0 * r_plus).sqrt() - 1.0).abs() < 1.0e-3,
            "{edge}"
        );
    }
}

#[test]
fn the_horizon_outline_lies_well_inside_the_shadow() {
    // Light bends: the horizon is seen filling the shadow, which is far wider than the horizon is.
    let mut hole = RayHole::new(SPIN, 0.0).unwrap();
    hole.max_crossings = u32::MAX;
    let camera = camera();
    let out = camera.normalize();
    let right = Vec3::Z.cross(out).normalize();
    let up = out.cross(right);
    for side in [right, up, -right, -up] {
        let horizon = horizon_edge(SPIN as f32, camera, side);
        let shadow = shadow_edge(&hole, camera, side);
        assert!(
            shadow > 1.5 * horizon,
            "shadow {shadow} vs horizon {horizon}"
        );
    }
}

#[test]
fn the_horizon_measure_changes_evenly_across_the_edge() {
    // Evenly spaced pixels across the edge: no jumps, so the outline is drawn an even width.
    let spin = SPIN as f32;
    let camera = camera();
    let edge = horizon_edge(spin, camera, Vec3::Z);
    let values: Vec<f32> = (-10..=10)
        .map(|i| {
            let b = edge + 0.005 * i as f32;
            horizon_measure(spin, camera, (b * Vec3::Z - camera).normalize()).unwrap()
        })
        .collect();
    let steps: Vec<f32> = values.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        steps.iter().all(|&d| d < 0.0),
        "falls steadily outwards: {values:?}"
    );
    let (least, most) = steps.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &d| {
        (lo.min(d.abs()), hi.max(d.abs()))
    });
    assert!(most < 1.5 * least, "evenly: {steps:?}");
}

#[test]
fn looking_away_from_the_hole_never_meets_its_horizon() {
    let spin = SPIN as f32;
    let camera = camera();
    assert!(horizon_measure(spin, camera, camera.normalize()).unwrap() < 0.0);
    assert!(horizon_measure(spin, camera, -camera.normalize()).unwrap() > 0.0);
    assert_eq!(
        horizon_measure(spin, Vec3::new(1.0, 0.0, 0.0), Vec3::X),
        None
    );
    assert_eq!(horizon_measure(spin, camera, Vec3::ZERO), None);
}
