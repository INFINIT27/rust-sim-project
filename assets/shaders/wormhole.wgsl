// The wormhole, ray-marched: for each pixel, the light that reaches the camera through it is
// followed backwards to the sky it came from, the near side's or the far side's.
//
// The Rust twin of this file is `src/render/wormhole_ray.rs`, written function for function the same
// and tested against the `f64` paths. Change the twin first, let its tests say whether the change is
// right, then make the same change here. `wormhole_view_tests::the_shader_mirrors_its_twin` checks
// this file's text for the twin's functions and constants, and that the star field and what it calls
// are the same text as `black_hole.wgsl`'s.
//
// Units: throat radii. Directions are in the label frame (see the twin), except the skies'.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
}

struct Wormhole {
    // xyz = the camera's label (unit), w = its place l, in throat radii.
    camera: vec4<f32>,
    // x = half-length, y = flare, z = step fraction, w = exposure.
    shape: vec4<f32>,
    // x = most steps, y = what to draw (0 the skies, 1 the test pattern), z = 1 for stars, w = how many
    // bodies there are.
    limits: vec4<u32>,
    // Each body: xyz = its label, w = its place l.
    places: array<vec4<f32>, 16>,
    // xyz = the direction of its star (label frame at it, zero for none), w = its radius.
    lights: array<vec4<f32>, 16>,
    // rgb = its colour, a = 1 if it glows.
    colours: array<vec4<f32>, 16>,
}

@group(2) @binding(0) var<uniform> wormhole: Wormhole;

// Mirrors `WORMHOLE_RAY_REACH`.
const REACH: f32 = 300.0;
// Mirrors `WORMHOLE_RAY_TUNNEL_STEP`, `WORMHOLE_MAX_BODIES` (and the arrays above), and the
// `WORMHOLE_BODY_*` constants.
const TUNNEL_STEP: f32 = 0.3;
const MAX_BODIES: u32 = 16u;
const BODY_GLOW: f32 = 0.6;
const BODY_BRIGHTNESS: f32 = 0.25;
const BODY_AMBIENT: f32 = 0.06;
// Mirrors `WORMHOLE_NEBULA_CELLS`, `WORMHOLE_NEBULA_EDGES`, and the two colours.
const NEBULA_CELLS: f32 = 2.5;
const NEBULA_EDGES: vec2<f32> = vec2<f32>(0.45, 0.85);
const NEBULA_COLOUR: vec3<f32> = vec3<f32>(0.01, 0.05, 0.07);
const NEBULA_TINT: vec3<f32> = vec3<f32>(0.08, 0.015, 0.06);
// Mirrors the `WORMHOLE_PATTERN_*` constants.
const PATTERN_SPACING_RAD: f32 = 0.2617994;
const PATTERN_LINE_RAD: f32 = 0.003;
const PATTERN_NEAR: vec3<f32> = vec3<f32>(1.0, 0.45, 0.1);
const PATTERN_FAR: vec3<f32> = vec3<f32>(0.15, 0.5, 1.0);
const TWO_OVER_PI: f32 = 0.63661977;
// The very back of the view (reversed Z), so everything else in the scene is in front.
const SKY_DEPTH: f32 = 1.0e-12;

// Mirrors `STAR_LAYERS` (cells per radian, chance of a star, brightest star) and `STAR_SIZE_RAD`.
const STAR_FINE: vec3<f32> = vec3<f32>(150.0, 0.08, 0.12);
const STAR_COARSE: vec3<f32> = vec3<f32>(40.0, 0.3, 0.35);
const STAR_SIZE_RAD: f32 = 0.0012;

// Mirrors `radius`.
fn radius(l: f32) -> f32 {
    let outside = abs(l) - wormhole.shape.x;
    if outside <= 0.0 {
        return 1.0;
    }
    let x = TWO_OVER_PI * outside / wormhole.shape.y;
    return 1.0 + wormhole.shape.y * (x * atan(x) - 0.5 * log(1.0 + x * x));
}

// Mirrors `slope`.
fn slope(l: f32) -> f32 {
    let outside = abs(l) - wormhole.shape.x;
    if outside <= 0.0 {
        return 0.0;
    }
    let x = TWO_OVER_PI * outside / wormhole.shape.y;
    return sign(l) * TWO_OVER_PI * atan(x);
}

// Mirrors `rates`: the state is (l, angle, p).
fn rates(impact: f32, state: vec3<f32>) -> vec3<f32> {
    let r = radius(state.x);
    let across = impact / (r * r);
    return vec3<f32>(state.z, across, impact * across * slope(state.x) / r);
}

// Mirrors `rk4`.
fn rk4(impact: f32, s: vec3<f32>, h: f32) -> vec3<f32> {
    let k1 = rates(impact, s);
    let k2 = rates(impact, s + 0.5 * h * k1);
    let k3 = rates(impact, s + 0.5 * h * k2);
    let k4 = rates(impact, s + h * k3);
    return s + h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
}

// Mirrors `far_frame`.
fn far_frame(n: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    let turned = v - 2.0 * dot(v, n) * n;
    return vec3<f32>(turned.x, turned.y, -turned.z);
}

// Mirrors `from_far_frame`.
fn from_far_frame(n: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    let mirrored = vec3<f32>(v.x, v.y, -v.z);
    return mirrored - 2.0 * dot(mirrored, n) * n;
}

// Any unit vector across n.
fn any_across(n: vec3<f32>) -> vec3<f32> {
    let other = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.5);
    return normalize(cross(n, other));
}

// Mirrors `offset`: where the point at l and label n is from a body, in the label frame at the body.
fn offset(body: vec4<f32>, r: f32, l: f32, n: vec3<f32>) -> vec3<f32> {
    let c = dot(n, body.xyz);
    let side = select(-1.0, 1.0, body.w >= 0.0);
    let along = side * r * (c - 1.0) + (l - body.w);
    return along * body.xyz + r * (n - c * body.xyz);
}

// Mirrors `meets`.
fn meets(from_: vec3<f32>, to_: vec3<f32>, size: f32) -> f32 {
    let d = to_ - from_;
    let a = dot(d, d);
    let b = dot(from_, d);
    let c = dot(from_, from_) - size * size;
    let discriminant = b * b - a * c;
    if c < 0.0 || b >= 0.0 || discriminant < 0.0 {
        return 2.0;
    }
    let t = (-b - sqrt(discriminant)) / a;
    if t <= 1.0 {
        return t;
    }
    return 2.0;
}

struct RayEnd {
    // xyz = the direction in the sky's own axes, or the surface's outward direction at the body.
    direction: vec3<f32>,
    // 1 = the near sky, -1 = the far sky, 0 = lost, 2 = a body.
    kind: f32,
    body: u32,
}

// Mirrors `march_past`.
fn march_past(l: f32, n: vec3<f32>, direction: vec3<f32>) -> RayEnd {
    let p = dot(direction, n);
    let across = direction - p * n;
    let sine = length(across);
    var t = any_across(n);
    var impact = 0.0;
    if sine > 1.0e-6 {
        t = across / sine;
        impact = radius(l) * sine;
    }
    let half_length = wormhole.shape.x;
    let flare = wormhole.shape.y;
    let bodies = wormhole.limits.w;
    var s = vec3<f32>(l, 0.0, p);
    var before = n;
    var r_before = radius(l);
    var steps = 0u;
    while steps < wormhole.limits.x {
        let outside = abs(s.x) - half_length;
        if outside >= REACH && s.x * s.z > 0.0 {
            break;
        }
        let last = s;
        if outside < 0.0 && s.z != 0.0 {
            // Inside the cylinder nothing bends: straight to the mouth it is heading for, in pieces
            // short enough to find bodies by when there are any.
            let mouth = select(-half_length, half_length, s.z > 0.0);
            var h = (mouth - s.x) / s.z;
            s.x = mouth;
            if bodies > 0u && h > TUNNEL_STEP {
                h = TUNNEL_STEP;
                s.x = last.x + h * s.z;
            }
            s.y = s.y + impact * h;
        } else {
            let beyond = max(outside, 0.0);
            let h = wormhole.shape.z * sqrt(flare * flare + beyond * beyond);
            s = rk4(impact, s, h);
        }
        steps = steps + 1u;
        if bodies > 0u {
            let now = cos(s.y) * n + sin(s.y) * t;
            let r_now = radius(s.x);
            var nearest = 1.5;
            var hit = MAX_BODIES;
            var normal = vec3<f32>(0.0);
            for (var i = 0u; i < bodies; i = i + 1u) {
                let body = wormhole.places[i];
                let size = wormhole.lights[i].w;
                // No point is nearer a body than the difference of their l: most are out of reach.
                if min(last.x, s.x) > body.w + size || max(last.x, s.x) < body.w - size {
                    continue;
                }
                let from_ = offset(body, r_before, last.x, before);
                let to_ = offset(body, r_now, s.x, now);
                let along = meets(from_, to_, size);
                if along < nearest {
                    nearest = along;
                    hit = i;
                    normal = (from_ + along * (to_ - from_)) / size;
                }
            }
            if hit < MAX_BODIES {
                return RayEnd(normal, 2.0, hit);
            }
            before = now;
            r_before = r_now;
        }
    }
    if steps >= wormhole.limits.x {
        return RayEnd(vec3<f32>(0.0), 0.0, 0u);
    }
    let end_n = cos(s.y) * n + sin(s.y) * t;
    let end_t = cos(s.y) * t - sin(s.y) * n;
    let end = normalize(s.z * end_n + impact / radius(s.x) * end_t);
    if s.x > 0.0 {
        return RayEnd(end, 1.0, 0u);
    }
    return RayEnd(far_frame(end_n, end), -1.0, 0u);
}

// Mirrors `body_light`.
fn body_light(colour: vec3<f32>, glows: bool, normal: vec3<f32>, star: vec3<f32>) -> vec3<f32> {
    if glows {
        return colour * BODY_GLOW;
    }
    return colour * BODY_BRIGHTNESS * (BODY_AMBIENT + max(dot(normal, star), 0.0));
}

// Mirrors `value_noise`.
fn value_noise(p: vec3<f32>) -> f32 {
    let base = floor(p);
    let f = p - base;
    let w = f * f * (3.0 - 2.0 * f);
    var total = 0.0;
    for (var k = 0u; k < 8u; k = k + 1u) {
        let corner = vec3<f32>(f32(k & 1u), f32((k >> 1u) & 1u), f32(k >> 2u));
        let cell = base + corner;
        let h = hash(
            bitcast<u32>(i32(cell.x))
                ^ hash(bitcast<u32>(i32(cell.y)) ^ hash(bitcast<u32>(i32(cell.z)))),
        );
        let blend = 1.0 - corner + (2.0 * corner - 1.0) * w;
        total = total + unit(h) * blend.x * blend.y * blend.z;
    }
    return total;
}

// Mirrors `nebula`.
fn nebula(d: vec3<f32>) -> vec3<f32> {
    let p = d * NEBULA_CELLS;
    let cloud = (value_noise(p) + 0.5 * value_noise(2.0 * p) + 0.25 * value_noise(4.0 * p)) / 1.75;
    let glow = smoothstep(NEBULA_EDGES.x, NEBULA_EDGES.y, cloud);
    let tint = value_noise(0.7 * p + 19.0);
    return glow * mix(NEBULA_COLOUR, NEBULA_TINT, tint);
}

// Mirrors `near_sky`.
fn near_sky(d: vec3<f32>) -> vec3<f32> {
    if wormhole.limits.z == 1u {
        return star_field(d, 1.0);
    }
    return vec3<f32>(0.0);
}

// Mirrors `far_sky`.
fn far_sky(d: vec3<f32>) -> vec3<f32> {
    var light = nebula(d);
    if wormhole.limits.z == 1u {
        light = light + star_field(d.yzx, 1.0);
        light = light + star_field(d.zxy, 1.0);
    }
    return light;
}

// Mirrors the `off` closure in `pattern`: how far an angle is from the nearest line.
fn off_line(angle: f32) -> f32 {
    let turns = angle / PATTERN_SPACING_RAD;
    return abs(turns - floor(turns + 0.5)) * PATTERN_SPACING_RAD;
}

// Mirrors `pattern`.
fn pattern(d: vec3<f32>, far: bool) -> vec3<f32> {
    let latitude = asin(clamp(d.y, -1.0, 1.0));
    let longitude = atan2(d.x, -d.z);
    let on_line = off_line(latitude) < PATTERN_LINE_RAD
        || off_line(longitude) * cos(latitude) < PATTERN_LINE_RAD;
    return select(PATTERN_NEAR, PATTERN_FAR, far) * select(0.08, 1.0, on_line);
}

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let n = wormhole.camera.xyz;
    let l = wormhole.camera.w;
    var direction = normalize(in.world_position.xyz - view.world_position);
    if l < 0.0 {
        direction = from_far_frame(n, direction);
    }
    let seen = march_past(l, n, direction);
    var light = vec3<f32>(0.0);
    if seen.kind == 2.0 {
        let colour = wormhole.colours[seen.body];
        let star = wormhole.lights[seen.body].xyz;
        light = body_light(colour.rgb, colour.a == 1.0, seen.direction, star);
        light = tone_map(light, wormhole.shape.w);
    } else if wormhole.limits.y == 1u {
        if seen.kind != 0.0 {
            light = pattern(seen.direction, seen.kind < 0.0);
        }
    } else {
        if seen.kind > 0.0 {
            light = near_sky(seen.direction);
        } else if seen.kind < 0.0 {
            light = far_sky(seen.direction);
        }
        light = tone_map(light, wormhole.shape.w);
    }
    var out: FragmentOutput;
    out.colour = vec4<f32>(light, 1.0);
    out.depth = SKY_DEPTH;
    return out;
}

// The same text as `black_hole.wgsl` from here on (checked by a test).

fn hash(value: u32) -> u32 {
    var x = value;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

fn unit(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0;
}

fn blackbody(temperature: f32) -> vec3<f32> {
    let t = clamp(temperature, 1000.0, 40000.0) / 100.0;
    var red = 255.0;
    var green = 0.0;
    var blue = 255.0;
    if t <= 66.0 {
        green = 99.4708 * log(t) - 161.11957;
    } else {
        red = 329.69873 * pow(t - 60.0, -0.13320476);
        green = 288.12217 * pow(t - 60.0, -0.07551485);
    }
    if t < 66.0 {
        if t <= 19.0 {
            blue = 0.0;
        } else {
            blue = 138.51773 * log(t - 10.0) - 305.0448;
        }
    }
    let srgb = clamp(vec3<f32>(red, green, blue) / 255.0, vec3<f32>(0.0), vec3<f32>(1.0));
    let low = srgb / 12.92;
    let high = pow((srgb + 0.055) / 1.055, vec3<f32>(2.4));
    let linear = select(high, low, srgb <= vec3<f32>(0.04045));
    return linear / max(max(max(linear.x, linear.y), linear.z), 1.0e-12);
}

fn star_field(d: vec3<f32>, lapse: f32) -> vec3<f32> {
    var total = vec3<f32>(0.0);
    var blueshift = 1.0;
    if lapse > 0.0 {
        blueshift = 1.0 / lapse;
    }
    for (var layer = 0u; layer < 2u; layer = layer + 1u) {
        let settings = select(STAR_COARSE, STAR_FINE, layer == 0u);
        let cells = settings.x;
        let base = floor(d * cells - 0.5);
        for (var k = 0u; k < 8u; k = k + 1u) {
            let cell = base + vec3<f32>(f32(k & 1u), f32((k >> 1u) & 1u), f32(k >> 2u));
            var h = hash(
                bitcast<u32>(i32(cell.x))
                    ^ hash(bitcast<u32>(i32(cell.y)) ^ hash(bitcast<u32>(i32(cell.z)) ^ (layer + 1u))),
            );
            if unit(h) >= settings.y {
                continue;
            }
            h = hash(h);
            let jx = unit(h);
            h = hash(h);
            let jy = unit(h);
            h = hash(h);
            let jz = unit(h);
            let jitter = vec3<f32>(jx, jy, jz) - 0.5;
            let star = normalize((cell + 0.5 + 0.8 * jitter) / cells);
            if any(floor(star * cells) != cell) {
                continue;
            }
            h = hash(h);
            let brightness = settings.z * (0.05 + 0.95 * pow(unit(h), 6.0));
            h = hash(h);
            let temperature = 2500.0 + 9500.0 * unit(h);
            let offset = d - star;
            let spot = exp(-dot(offset, offset) / (STAR_SIZE_RAD * STAR_SIZE_RAD));
            total = total + blackbody(temperature * blueshift) * brightness * spot;
        }
    }
    return total;
}

fn tone_map(radiance: vec3<f32>, exposure: f32) -> vec3<f32> {
    return vec3<f32>(1.0) - exp(-exposure * max(radiance, vec3<f32>(0.0)));
}

