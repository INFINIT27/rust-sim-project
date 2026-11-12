//! Unit tests for `black_hole_view.rs`: the uniform the shader reads, the sphere, the settings, the
//! gas texture, and the shader's text against its Rust twin, `kerr_ray.rs`.

use super::*;
use crate::performance::BlackHoleQuality;
use crate::render::constants::{
    BLACK_HOLE_EXPOSURE, BLACK_HOLE_OUTLINE_COLOUR, BLACK_HOLE_OUTLINE_WIDTH_PX,
    DISC_OUTER_RADIUS_RG, KERR_RAY_MAX_CROSSINGS, KERR_RAY_MAX_STEPS, KERR_RAY_PROFILE_SAMPLES,
    KERR_RAY_STEP_FRACTION,
};

const MASS: f64 = 4.3e6 * 1.988_47e30;

fn uniform_for(
    profile: &DiscProfile,
    view: BlackHoleView,
    settings: &PerformanceSettings,
) -> Option<BlackHoleUniform> {
    BlackHoleUniform::new(
        profile,
        Vec3::new(1.0, 2.0, 3.0),
        view,
        settings,
        120.0,
        BlackHoleOutline::default(),
    )
}

#[test]
fn the_uniform_carries_the_hole_in_gravitational_radii() {
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    let settings = PerformanceSettings::default();
    let uniform = uniform_for(&profile, BlackHoleView::Shaded, &settings).unwrap();
    assert_eq!(uniform.centre.truncate(), Vec3::new(1.0, 2.0, 3.0));
    // Sgr A*: r_g = 6.35e9 m = 6.35 world units.
    assert!((uniform.units_per_rg() - 6.35).abs() < 0.01);
    assert!((uniform.hole.x - 0.9).abs() < 1.0e-6);
    assert!((uniform.hole.y - 1.4359).abs() < 1.0e-3, "horizon");
    assert!(
        (uniform.hole.z - 1.5578).abs() < 1.0e-3,
        "prograde photon orbit"
    );
    assert!((uniform.hole.w - 2.3209).abs() < 1.0e-3, "ISCO");
    assert_eq!(uniform.disc.x, 20.0);
    assert_eq!(uniform.disc.y, KERR_RAY_STEP_FRACTION, "medium quality");
    assert_eq!(uniform.disc.z, 120.0, "the sphere's radius");
    assert_eq!(uniform.limits.x, KERR_RAY_MAX_STEPS);
    assert_eq!(uniform.limits.y, KERR_RAY_MAX_CROSSINGS as u32);
    assert_eq!(uniform.limits.z, 0, "shaded");
    assert_eq!(uniform.limits.w, 1, "stars");

    // The disc's profile, from the ISCO to the edge, the exposure, and the plunge's E and L.
    assert_eq!(uniform.profile.len(), KERR_RAY_PROFILE_SAMPLES);
    assert!(
        (uniform.profile[0].x as f64 - profile.temperature_at(profile.isco_rg)).abs()
            < 1.0e-3 * profile.peak_temperature_k()
    );
    let last = uniform.profile[KERR_RAY_PROFILE_SAMPLES - 1];
    assert!((last.y as f64 - profile.relative_flux_at(profile.outer_rg)).abs() < 1.0e-6);
    assert_eq!(uniform.shading.x, BLACK_HOLE_EXPOSURE);
    let hole = uniform.ray_hole();
    assert_eq!(uniform.shading.z, hole.isco_energy);
    assert_eq!(uniform.shading.w, hole.isco_angular_momentum);

    let pattern = uniform_for(&profile, BlackHoleView::Pattern, &settings).unwrap();
    assert_eq!(pattern.limits.z, 1);

    let mut unusable = profile.clone();
    unusable.spin = 1.5;
    assert!(uniform_for(&unusable, BlackHoleView::Shaded, &settings).is_none());
    unusable.spin = 0.9;
    unusable.mass_kg = -MASS;
    assert!(uniform_for(&unusable, BlackHoleView::Shaded, &settings).is_none());
}

#[test]
fn the_settings_choose_the_step_and_the_stars() {
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    for quality in BlackHoleQuality::ALL {
        let settings = PerformanceSettings {
            black_hole_quality: quality,
            black_hole_stars: false,
            ..PerformanceSettings::default()
        };
        let uniform = uniform_for(&profile, BlackHoleView::Shaded, &settings).unwrap();
        assert_eq!(uniform.disc.y, quality.step_fraction());
        assert_eq!(uniform.limits.x, quality.max_steps());
        assert_eq!(uniform.limits.w, 0, "no stars");
    }
    // Finer steps, and the budget to go with them.
    let (low, high) = (BlackHoleQuality::Low, BlackHoleQuality::High);
    assert!(
        low.step_fraction() > KERR_RAY_STEP_FRACTION
            && high.step_fraction() < KERR_RAY_STEP_FRACTION
    );
    assert!(low.max_steps() < KERR_RAY_MAX_STEPS && high.max_steps() > KERR_RAY_MAX_STEPS);
}

#[test]
fn the_ray_marcher_can_be_switched_off_for_the_plain_body() {
    assert!(ray_marched(None), "on by default");
    assert!(ray_marched(Some(&PerformanceSettings::default())));
    let off = PerformanceSettings {
        black_hole: false,
        ..PerformanceSettings::default()
    };
    assert!(!ray_marched(Some(&off)));
}

#[test]
fn the_sphere_always_holds_the_camera() {
    const { assert!(BLACK_HOLE_VIEW_SPHERE_RG > 3.0 * DISC_OUTER_RADIUS_RG) };
    let far = 3000.0;
    // Close in, the minimum; further out, bigger than the camera's distance; never past the far
    // plane.
    assert_eq!(sphere_radius_rg(10.0, far), BLACK_HOLE_VIEW_SPHERE_RG);
    for camera in [53.0, 100.0, 400.0, 1500.0] {
        let radius = sphere_radius_rg(camera, far);
        assert!(radius > camera, "{camera} r_g out: {radius}");
    }
    assert!(sphere_radius_rg(1.0e5, far) <= 0.9 * far);
    // And the unit mesh is scaled to it.
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    let uniform = uniform_for(
        &profile,
        BlackHoleView::Shaded,
        &PerformanceSettings::default(),
    )
    .unwrap();
    let transform = sphere_transform(&uniform);
    assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 3.0));
    assert!((transform.scale.x - 120.0 * uniform.units_per_rg()).abs() < 1.0e-3);
}

#[test]
fn the_hole_body_is_the_black_hole_only_in_a_scene_with_a_disc() {
    let scene = crate::experiments::black_hole::black_hole().scene;
    let (id, prop) = scene.iter().next().unwrap();
    assert!(prop.absorbs);
    assert_eq!(hole_body(&scene), Some(id));
    assert_eq!(
        hole_body(&crate::experiments::catalog::two_body().scene),
        None
    );
}

#[test]
fn v_switches_between_the_shaded_disc_and_the_test_pattern() {
    assert_eq!(BlackHoleView::default(), BlackHoleView::Shaded);
    assert_eq!(BlackHoleView::Shaded.next(), BlackHoleView::Pattern);
    assert_eq!(BlackHoleView::Pattern.next(), BlackHoleView::Shaded);
}

#[test]
fn the_shader_mirrors_its_twin() {
    // WGSL cannot be compiled here, so its text is checked for the twin's functions, in order, and
    // for every threshold the twin uses.
    let shader = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/",
        "shaders/black_hole.wgsl"
    ))
    .expect("the shader exists");
    assert!(BLACK_HOLE_SHADER_PATH.ends_with("black_hole.wgsl"));
    let mut from = 0;
    for function in [
        "fn radius(",
        "fn equatorial_coordinates(",
        "fn metric_pieces(",
        "fn rates(",
        "fn rk4(",
        "fn start_momentum(",
        "fn texture_coordinates(",
        "fn gas_coverage(",
        "fn tone_map(",
        "fn finish(",
        "fn march(",
        "fn hash(",
        "fn unit(",
        "fn star_field(",
        "fn camera_lapse(",
        "fn emitter(",
        "fn plunge_parts(",
        "fn redshift(",
        "fn profile_at(",
        "fn blackbody(",
        "fn disc_colour(",
    ] {
        let at = shader[from..]
            .find(function)
            .unwrap_or_else(|| panic!("{function} missing or out of order"));
        from += at;
    }
    for line in [
        "let l = 1.0 + dot(m.k, p);",
        "let dx = p - m.f * l * m.k;",
        "let dp = 0.5 * l * l * m.grad_f + m.f * l * (m.grad_k * p);",
        "let discriminant = 1.0 - m.f + m.f * c * c;",
        "let escape = max(length(camera), 1.5 * outer) * 1.05;",
        "if r <= horizon * 1.001 || (r <= trapped && dot(x, dx) > 0.0) {",
        "if length(x) > escape && dot(x, dx) < 0.0 {",
        "let h = -step_fraction * length(x) / length(dx);",
        "let on_disc = polar.x >= isco && polar.x <= outer;",
        "let k_u = (e + l * k_round / rho + along * k_out) / c;",
        "let along = (-qb - sqrt(max(qb * qb - 4.0 * qa * qc, 0.0))) / (2.0 * qa);",
        "let g = 1.0 / (lapse * (u.w - dot(hit.momentum, u.xyz)));",
        "return vec3<f32>(1.0) - exp(-exposure * max(radiance, vec3<f32>(0.0)));",
        "return blackbody(g * sample.x) * max(sample.y, 0.0) * g2 * g2;",
        "x = x * 0x7feb352du;",
        "x = x * 0x846ca68bu;",
        "return f32(h >> 8u) / 16777216.0;",
        "let star = normalize((cell + 0.5 + 0.8 * jitter) / cells);",
        "let brightness = settings.z * (0.05 + 0.95 * pow(unit(h), 6.0));",
        "let temperature = 2500.0 + 9500.0 * unit(h);",
        "return finish(colour + transmittance * star_field(-normalize(dx), lapse), 0.0);",
        "sample = vec2<f32>(edge.x, max(edge.y, 0.05) * fade);",
        "colour = colour + transmittance * covered * disc_colour(hit, lapse);",
        "transmittance = transmittance * (1.0 - covered);",
        "const OPAQUE_TRANSMITTANCE: f32 = 0.02;",
        "return vec2<f32>(turn - floor(turn), sqrt(u));",
        "@group(2) @binding(1) var gas_texture: texture_2d<f32>;",
        "@group(2) @binding(2) var gas_sampler: sampler;",
        "@group(2) @binding(0) var<uniform> black_hole: BlackHole;",
        "let omega = 1.0 / (r * sqrt(r) + spin);",
        "let norm = -1.0 + dot(v, v) + m.f * k_v * k_v;",
        "let u_t = 1.0 / sqrt(-norm);",
        "let s = sqrt(u) * last;",
        "profile: array<vec4<f32>, 64>,",
    ] {
        assert!(shader.contains(line), "the shader lacks `{line}`");
    }
    assert!(shader.contains(&format!(
        "const MAX_CROSSINGS: u32 = {}u;",
        KERR_RAY_MAX_CROSSINGS
    )));
    assert!(shader.contains(&format!(
        "const PROFILE_SAMPLES: u32 = {}u;",
        KERR_RAY_PROFILE_SAMPLES
    )));
    assert!(shader.contains(&format!(
        "profile: array<vec4<f32>, {}>,",
        KERR_RAY_PROFILE_SAMPLES
    )));
    // And the twin really says the same.
    let twin = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/render/kerr_ray.rs"
    ))
    .unwrap();
    for line in [
        "let l = 1.0 + m.k.dot(p);",
        "let dx = p - m.f * l * m.k;",
        "let dp = 0.5 * l * l * m.grad_f + m.f * l * (m.grad_k * p);",
        "let escape = camera.length().max(1.5 * hole.outer) * 1.05;",
        "if r <= hole.horizon * 1.001 || (r <= hole.trapped && x.dot(dx) > 0.0) {",
        "let h = -hole.step_fraction * x.length() / dx.length();",
        "let omega = 1.0 / (r * r.sqrt() + spin);",
        "let norm = -1.0 + v.dot(v) + m.f * k_v * k_v;",
        "let k_u = (e + l * k_round / rho + along * k_out) / c;",
        "let along = (-qb - (qb * qb - 4.0 * qa * qc).max(0.0).sqrt()) / (2.0 * qa);",
        "let g = 1.0 / (lapse * (u_t - crossing.momentum.dot(u)));",
        "Vec3::ONE - (-exposure * radiance.max(Vec3::ZERO)).exp()",
        "Some(blackbody(g * sample.x) * sample.y.max(0.0) * g.powi(4))",
        "x = x.wrapping_mul(0x7feb_352d);",
        "x = x.wrapping_mul(0x846c_a68b);",
        "let star = ((cell + 0.5 + 0.8 * jitter) / cells).normalize();",
        "let brightness = scale * (0.05 + 0.95 * next().powi(6));",
        "let temperature = 2500.0 + 9500.0 * next();",
        "sample = Vec2::new(edge.x, edge.y.max(0.05) * fade);",
        "colour += transmittance * covered * seen;",
        "transmittance *= 1.0 - covered;",
        "pub const OPAQUE_TRANSMITTANCE: f32 = 0.02;",
        "Vec2::new(turn - turn.floor(), u.sqrt())",
        "let s = u.sqrt() * last;",
    ] {
        assert!(twin.contains(line), "the twin lacks `{line}`");
    }
}

fn sgr_a() -> (RayHole, DiscProfile) {
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    let uniform = uniform_for(
        &profile,
        BlackHoleView::Shaded,
        &PerformanceSettings::default(),
    )
    .unwrap();
    (uniform.ray_hole(), profile)
}

fn orbiting(radius_rg: f64, azimuth_rad: f64) -> DiscParticle {
    DiscParticle {
        radius_rg,
        azimuth_rad,
        plunge: None,
    }
}

#[test]
fn the_uniform_rebuilds_the_twins_hole() {
    let (hole, _) = sgr_a();
    assert_eq!(hole, RayHole::new(0.9, hole.outer as f64).unwrap());
}

#[test]
fn an_empty_disc_splats_nothing() {
    let (hole, profile) = sgr_a();
    let gas = splat_gas(&hole, &profile, &[]);
    assert_eq!(gas.len(), DISC_GAS_TEXTURE_WIDTH * DISC_GAS_TEXTURE_HEIGHT);
    assert!(gas.iter().all(|&texel| texel == 0));
}

#[test]
fn a_clump_is_splatted_along_its_streak_where_the_shader_reads_it() {
    let (hole, profile) = sgr_a();
    let clump = orbiting(10.0, 2.0);
    let gas = splat_gas(&hole, &profile, std::slice::from_ref(&clump));
    let texel = |r: f32, phi: f32| {
        let uv = texture_coordinates(&hole, r, phi);
        let i = ((uv.x * DISC_GAS_TEXTURE_WIDTH as f32) as usize) % DISC_GAS_TEXTURE_WIDTH;
        let j = ((uv.y * DISC_GAS_TEXTURE_HEIGHT as f32) as usize).min(DISC_GAS_TEXTURE_HEIGHT - 1);
        gas[j * DISC_GAS_TEXTURE_WIDTH + i]
    };
    // Along its streak, which trails it by `DISC_STREAK_ARC_RAD`.
    let arc = crate::render::constants::DISC_STREAK_ARC_RAD as f32;
    for back in [0.02, 0.5 * arc, arc - 0.02] {
        assert!(texel(10.0, 2.0 - back) > 0, "{back} rad behind it");
    }
    // Not ahead of it, not across the disc, not far in or out.
    assert_eq!(texel(10.0, 2.3), 0);
    assert_eq!(texel(10.0, 2.0 + std::f32::consts::PI), 0);
    assert_eq!(texel(5.0, 1.9), 0);
    assert_eq!(texel(15.0, 1.9), 0);
}

#[test]
fn the_splat_covers_as_much_of_the_plane_as_the_streaks_do() {
    // Thin gas, so τ is small and coverage ≈ τ: summed over the texture, each texel's coverage times
    // its area is the clumps' total area, width × streak length.
    let (hole, profile) = sgr_a();
    let clumps: Vec<DiscParticle> = (0..40)
        .map(|k| orbiting(6.0 + 0.3 * k as f64, 0.15 * k as f64))
        .collect();
    let gas = splat_gas(&hole, &profile, &clumps);
    let (w, h) = (DISC_GAS_TEXTURE_WIDTH, DISC_GAS_TEXTURE_HEIGHT);
    let span = hole.outer - hole.horizon;
    let mut covered = 0.0f64;
    for j in 0..h {
        let y = (j as f32 + 0.5) / h as f32;
        let r = texture_radius(&hole, y);
        let area = r * 2.0 * span * y / h as f32 * std::f32::consts::TAU / w as f32;
        for i in 0..w {
            let c = gas[j * w + i] as f32 / 255.0;
            covered += (-(1.0 - c).max(1.0e-6).ln() * area) as f64;
        }
    }
    let arc = crate::render::constants::DISC_STREAK_ARC_RAD;
    let expected: f64 = clumps
        .iter()
        .map(|c| c.radius_rg * arc * DISC_CLUMP_WIDTH_RG as f64)
        .sum();
    assert!(
        (covered / expected - 1.0).abs() < 0.1,
        "covered {covered} r_g², streaks {expected}"
    );
}

#[test]
fn the_simulated_disc_covers_most_of_the_plane_with_gaps() {
    // The full disc: well covered on average, but not a solid sheet, so the images behind show.
    let (hole, profile) = sgr_a();
    let mut disc = AccretionDisc::default();
    disc.advance(MASS, 0.9, 0.0);
    let gas = splat_gas(&hole, &profile, disc.particles());
    let (w, h) = (DISC_GAS_TEXTURE_WIDTH, DISC_GAS_TEXTURE_HEIGHT);
    let row = |r: f32| {
        let j = (texture_coordinates(&hole, r, 0.0).y * h as f32) as usize;
        gas[j * w..(j + 1) * w]
            .iter()
            .map(|&t| t as f32 / 255.0)
            .sum::<f32>()
            / w as f32
    };
    for r in [5.0, 10.0, 15.0] {
        let mean = row(r);
        assert!((0.4..0.95).contains(&mean), "r = {r}: mean coverage {mean}");
    }
    assert!(gas.iter().any(|&t| t < 64), "there are gaps");
}

#[test]
fn the_gas_image_wraps_round_the_hole_and_filters() {
    let image = gas_image(vec![7; DISC_GAS_TEXTURE_WIDTH * DISC_GAS_TEXTURE_HEIGHT]);
    assert_eq!(
        image.texture_descriptor.size.width as usize,
        DISC_GAS_TEXTURE_WIDTH
    );
    assert_eq!(
        image.texture_descriptor.size.height as usize,
        DISC_GAS_TEXTURE_HEIGHT
    );
    let ImageSampler::Descriptor(sampler) = &image.sampler else {
        panic!("a sampler of its own");
    };
    assert!(matches!(sampler.address_mode_u, ImageAddressMode::Repeat));
    assert!(matches!(
        sampler.address_mode_v,
        ImageAddressMode::ClampToEdge
    ));
    assert!(matches!(sampler.mag_filter, ImageFilterMode::Linear));
}

#[test]
fn the_shaders_stars_are_the_twins() {
    // The star layers and size are written in both; WGSL cannot import Rust's.
    use crate::render::kerr_ray::{STAR_LAYERS, STAR_SIZE_RAD};
    let shader = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/shaders/black_hole.wgsl"
    ))
    .unwrap();
    let layer = |(cells, chance, scale): (f32, f32, f32)| {
        format!("vec3<f32>({cells:?}, {chance:?}, {scale:?})")
    };
    assert!(shader.contains(&format!(
        "const STAR_FINE: vec3<f32> = {};",
        layer(STAR_LAYERS[0])
    )));
    assert!(shader.contains(&format!(
        "const STAR_COARSE: vec3<f32> = {};",
        layer(STAR_LAYERS[1])
    )));
    assert!(shader.contains(&format!("const STAR_SIZE_RAD: f32 = {STAR_SIZE_RAD:?};")));
}

#[test]
#[ignore = "timing, not correctness: cargo test --release -- --ignored splat_timing"]
fn splat_timing() {
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    let hole = uniform_for(
        &profile,
        BlackHoleView::Shaded,
        &PerformanceSettings::default(),
    )
    .unwrap()
    .ray_hole();
    let mut disc = AccretionDisc::default();
    disc.advance(MASS, 0.9, 0.0);
    let start = std::time::Instant::now();
    let runs = 50;
    for _ in 0..runs {
        std::hint::black_box(splat_gas(&hole, &profile, disc.particles()));
    }
    println!(
        "splat_gas: {:.2} ms per frame for {} clumps",
        start.elapsed().as_secs_f64() * 1000.0 / runs as f64,
        disc.particles().len()
    );
}

#[test]
fn the_material_is_opaque_with_stars_and_blended_without() {
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    let with = PerformanceSettings::default();
    let without = PerformanceSettings {
        black_hole_stars: false,
        ..PerformanceSettings::default()
    };
    let material = |settings: &PerformanceSettings| BlackHoleMaterial {
        uniform: uniform_for(&profile, BlackHoleView::Shaded, settings).unwrap(),
        gas: Handle::default(),
    };
    assert!(matches!(material(&with).alpha_mode(), AlphaMode::Opaque));
    assert!(matches!(material(&without).alpha_mode(), AlphaMode::Blend));
}

#[test]
fn the_shader_writes_its_own_depth() {
    let shader = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/shaders/black_hole.wgsl"
    ))
    .unwrap();
    for line in [
        "@builtin(frag_depth) depth: f32,",
        "let clip = view.clip_from_world * vec4<f32>(world, 1.0);",
        "const SKY_DEPTH: f32 = 1.0e-12;",
        "depth = depth_of(depth_point.xyz);",
        "out.depth = depth;",
    ] {
        assert!(shader.contains(line), "the shader lacks `{line}`");
    }
}

#[test]
fn the_outline_starts_off_and_reaches_the_shader_as_a_flag() {
    assert!(!BlackHoleOutline::default().0);
    let profile = DiscProfile::new(MASS, 0.9).unwrap();
    let settings = PerformanceSettings::default();
    let with = |outline| {
        BlackHoleUniform::new(
            &profile,
            Vec3::ZERO,
            BlackHoleView::Shaded,
            &settings,
            120.0,
            BlackHoleOutline(outline),
        )
        .unwrap()
    };
    assert_eq!(with(false).shading.y, 0.0);
    assert_eq!(with(true).shading.y, 1.0);
    // Nothing else changes.
    let (off, on) = (with(false), with(true));
    assert_eq!(off.shading.x, on.shading.x);
    assert_eq!((off.shading.z, off.shading.w), (on.shading.z, on.shading.w));
    assert_eq!(off.limits, on.limits);
}

#[test]
fn o_toggles_the_outline_except_while_the_menu_is_open() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = bevy::app::App::new();
    app.init_resource::<BlackHoleOutline>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyO);
    app.world_mut().run_system_once(toggle_outline_system);
    assert!(app.world().resource::<BlackHoleOutline>().0, "on");
    app.world_mut().run_system_once(toggle_outline_system);
    assert!(!app.world().resource::<BlackHoleOutline>().0, "off again");

    let mut menu = MenuState::default();
    menu.toggle();
    app.insert_resource(menu);
    app.world_mut().run_system_once(toggle_outline_system);
    assert!(
        !app.world().resource::<BlackHoleOutline>().0,
        "not while the menu is open"
    );
}

#[test]
fn the_shader_outlines_the_horizon_in_the_outline_colour() {
    let shader = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/shaders/black_hole.wgsl"
    ))
    .unwrap();
    let [r, g, b] = BLACK_HOLE_OUTLINE_COLOUR;
    let colour = format!("const OUTLINE_COLOUR: vec3<f32> = vec3<f32>({r:?}, {g:?}, {b:?});");
    let width = format!("const OUTLINE_WIDTH_PX: f32 = {BLACK_HOLE_OUTLINE_WIDTH_PX:?};");
    for line in [
        colour.as_str(),
        "if black_hole.shading.y == 1.0 {",
        "let change = fwidth(measure.x);",
        "let scale = vec3<f32>(equator, equator, r_plus);",
        "closest = dot(across, across) / dd;",
        "return vec2<f32>(1.0 - closest, 1.0);",
        "let measure = horizon_measure(black_hole.hole.x, camera_start, direction);",
        width.as_str(),
    ] {
        assert!(shader.contains(line), "the shader lacks `{line}`");
    }
    // The derivative needs every pixel of its quad: no discard before it.
    let fragment = &shader[shader.find("fn fragment(").unwrap()..];
    let derivative = fragment.find("fwidth(").unwrap();
    assert!(!fragment[..derivative].contains("discard"));
}
