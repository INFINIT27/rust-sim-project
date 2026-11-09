// Analytic eclipse shadows.
//
// The Rust reference for this arithmetic is `star_visible_fraction` in `src/render/eclipse.rs`, and
// `eclipse_tests::the_shader_mirrors_the_reference_thresholds` checks this file's text for every
// comparison the reference makes. Two implementations of one formula is exactly the arrangement that
// drifts, so if you change one, change the other and that test's list with it.
//
// Why this exists rather than a shadow map: at Jupiter's distance from the Sun, one texel of a 1024
// cube face is 17x Jupiter's radius and 656x Io's. Resolving Io's shadow would need ~671,000 texels
// per face. The shadow has to be computed, not sampled.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}

const MAX_OCCLUDERS: u32 = 16u;

struct Eclipse {
    // xyz = the star's position, w = its radius. A w of 0 means this scene has no star.
    star: vec4<f32>,
    // x = how many `spheres` entries are live.
    count: vec4<u32>,
    // xyz = an occluder's centre, w = its radius.
    spheres: array<vec4<f32>, 16>,
}

@group(2) @binding(100) var<uniform> eclipse: Eclipse;

// Mirrors `MISS_COSINE_MARGIN`: keeps the trig-free miss test strictly inside the region the angle
// test also calls a miss, so rounding never turns a sliver of penumbra into full light.
const MISS_COSINE_MARGIN: f32 = 1.0e-6;

// The star as seen from one fragment, so it is computed once rather than once per occluder.
struct StarView {
    to_star: vec3<f32>,
    distance: f32,
    // sin, cos, and the angle itself, of the star's angular radius.
    sine: f32,
    cosine: f32,
    angle: f32,
}

// (1 - s)(1 + s) rather than 1 - s*s, which cancels to noise when s is near 1.
fn cosine_from_sine(sine: f32) -> f32 {
    return sqrt((1.0 - sine) * (1.0 + sine));
}

// What fraction of the star is visible from `point`, past one occluding sphere.
//
// Mirrors `StarView::visible_past`. The four cases are: the discs miss, the occluder swallows the
// star (umbra), the occluder sits inside the star's disc (annular), and partial overlap (penumbra).
fn star_visible_one(point: vec3<f32>, star: StarView, occluder: vec4<f32>) -> f32 {
    let occluder_radius = occluder.w;
    if occluder_radius <= 0.0 {
        return 1.0;
    }

    let to_occluder = occluder.xyz - point;
    let occluder_distance = length(to_occluder);

    // At the surface, inside it, or further off than the star: nothing to occlude. The radius test
    // is what stops a body shadowing itself.
    if occluder_distance <= occluder_radius || occluder_distance >= star.distance {
        return 1.0;
    }

    let occluder_sine = clamp(occluder_radius / occluder_distance, -1.0, 1.0);
    let occluder_cosine = cosine_from_sine(occluder_sine);
    let cosine = clamp(dot(star.to_star, to_occluder) / (star.distance * occluder_distance), -1.0, 1.0);

    // The discs miss, decided without trigonometry: separation >= star_angle + occluder_angle iff
    // cos(separation) <= cos(star_angle + occluder_angle), as both angles are at most pi/2. Nearly
    // every occluder of nearly every fragment leaves here, before the asin and acos below.
    if cosine <= star.cosine * occluder_cosine - star.sine * occluder_sine - MISS_COSINE_MARGIN {
        return 1.0;
    }

    let star_angle = star.angle;
    let occluder_angle = asin(occluder_sine);
    let separation = acos(cosine);

    if separation >= star_angle + occluder_angle {
        return 1.0;
    }
    if separation <= occluder_angle - star_angle {
        return 0.0;
    }
    if separation <= star_angle - occluder_angle {
        let ratio = occluder_angle / star_angle;
        return clamp(1.0 - ratio * ratio, 0.0, 1.0);
    }

    // Area shared by two circles of radii a and b whose centres are d apart, over the star's area.
    // This term is the penumbra, and its width is the star's own angular size.
    let a = star_angle;
    let b = occluder_angle;
    let d = separation;
    let first = acos(clamp((d * d + a * a - b * b) / (2.0 * d * a), -1.0, 1.0));
    let second = acos(clamp((d * d + b * b - a * a) / (2.0 * d * b), -1.0, 1.0));
    let triangle = sqrt(max((-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b), 0.0));

    let overlap = a * a * first + b * b * second - 0.5 * triangle;
    let covered = overlap / (3.14159265 * a * a);

    return clamp(1.0 - covered, 0.0, 1.0);
}

// The product over every live occluder. Multiplying double-counts two occluders covering the same
// part of the star's disc, which requires a syzygy and is zero error otherwise.
fn star_visible(point: vec3<f32>, normal: vec3<f32>) -> f32 {
    // No star in this scene: it is lit by a directional light and there is nothing to eclipse.
    if eclipse.star.w <= 0.0 {
        return 1.0;
    }
    let to_star = eclipse.star.xyz - point;
    // Facing away from the star: the body itself blocks it, and it is not in its own occluder list.
    // Shadowing here would only darken the ambient term, drawing the umbra on the night side too.
    if dot(normal, to_star) <= 0.0 {
        return 1.0;
    }

    let star_distance = length(to_star);
    if star_distance <= 0.0 {
        return 1.0;
    }
    let star_sine = clamp(eclipse.star.w / star_distance, -1.0, 1.0);
    let star_angle = asin(star_sine);
    if star_angle <= 0.0 {
        return 1.0;
    }
    let star = StarView(to_star, star_distance, star_sine, cosine_from_sine(star_sine), star_angle);

    var visible = 1.0;
    let count = min(eclipse.count.x, MAX_OCCLUDERS);
    for (var i = 0u; i < count; i = i + 1u) {
        visible = visible * star_visible_one(point, star, eclipse.spheres[i]);
        // Nothing left to shade.
        if visible <= 0.0 {
            return 0.0;
        }
    }
    return visible;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);

    // The shadow is applied to the lit result. A star's own surface is never in shadow: the light sits
    // at its centre, so every occluder is further from the fragment than the light is and the
    // distance test above returns full visibility.
    let visible = star_visible(in.world_position.xyz, pbr_input.world_normal);
    out.color = vec4<f32>(out.color.rgb * visible, out.color.a);

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
