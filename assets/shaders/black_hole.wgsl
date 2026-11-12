// The black hole, ray-marched: for each pixel, the light that reaches the camera through it is
// followed backwards along its exact Kerr path until it reaches the disc, falls into the hole, or
// comes from the sky.
//
// The Rust twin of this file is `src/render/kerr_ray.rs`, written function for function the same and
// tested against the `f64` Kerr code and Bardeen's analytic shadow. Change the twin first, let its
// tests say whether the change is right, then make the same change here.
// `black_hole_view_tests::the_shader_mirrors_its_twin` checks this file's text for the twin's
// functions and thresholds.
//
// Units: gravitational radii (r_g, M = 1), relative to the hole, Kerr–Schild axes (spin along +z).
// World axes map to Kerr–Schild ones as [z, x, y].

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
}

struct BlackHole {
    // xyz = the hole's centre in world units, w = world units per r_g.
    centre: vec4<f32>,
    // spin, horizon, trapped (prograde photon orbit), ISCO.
    hole: vec4<f32>,
    // x = the disc's outer edge, y = step fraction, z = the sphere's radius in r_g.
    disc: vec4<f32>,
    // x = most steps, y = most plane crossings, z = what to draw (0 shaded, 1 test pattern),
    // w = whether the sky has stars (1) or is left clear (0).
    limits: vec4<u32>,
    // x = exposure; y = whether the event horizon is outlined (1) or not (0); z, w = energy and
    // angular momentum of the ISCO orbit, which plunging gas keeps.
    shading: vec4<f32>,
    // x = temperature (K), y = relative flux, from the ISCO to the outer edge, uniformly in
    // sqrt(r - r_ISCO).
    profile: array<vec4<f32>, 64>,
}

@group(2) @binding(0) var<uniform> black_hole: BlackHole;
// How much of the plane the gas covers: r = coverage, over (turn round the hole, sqrt of the way
// from the horizon to the edge). Splatted from the simulation's clumps every frame.
@group(2) @binding(1) var gas_texture: texture_2d<f32>;
@group(2) @binding(2) var gas_sampler: sampler;

// Mirrors KERR_RAY_MAX_CROSSINGS.
const MAX_CROSSINGS: u32 = 3u;
// Mirrors KERR_RAY_PROFILE_SAMPLES, and the `profile` array's length.
const PROFILE_SAMPLES: u32 = 64u;

fn to_kerr_schild(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(v.z, v.x, v.y);
}

// Mirrors `radius`.
fn radius(spin: f32, x: vec3<f32>) -> f32 {
    let a2 = spin * spin;
    let b = dot(x, x) - a2;
    return sqrt(0.5 * (b + sqrt(b * b + 4.0 * a2 * x.z * x.z)));
}

// Mirrors `equatorial_coordinates`: Boyer–Lindquist r and φ of a point in the plane.
fn equatorial_coordinates(spin: f32, x: vec3<f32>) -> vec2<f32> {
    let r = radius(spin, vec3<f32>(x.x, x.y, 0.0));
    return vec2<f32>(r, atan2(x.y, x.x) + atan2(spin, r));
}

struct MetricPieces {
    f: f32,
    k: vec3<f32>,
    grad_f: vec3<f32>,
    grad_k: mat3x3<f32>,
}

// Mirrors `metric_pieces`.
fn metric_pieces(spin: f32, x: vec3<f32>) -> MetricPieces {
    let a = spin;
    let a2 = a * a;
    let r = radius(spin, x);
    let r2 = r * r;
    let rho2 = dot(x, x);

    let grad_r = (r2 * x + vec3<f32>(0.0, 0.0, a2 * x.z)) / (r * (2.0 * r2 - rho2 + a2));

    let q = r2 * r2 + a2 * x.z * x.z;
    let f = 2.0 * r2 * r / q;
    let grad_q = 4.0 * r2 * r * grad_r + vec3<f32>(0.0, 0.0, 2.0 * a2 * x.z);
    let grad_f = (6.0 * r2 * q * grad_r - 2.0 * r2 * r * grad_q) / (q * q);

    let s = r2 + a2;
    let num_x = r * x.x + a * x.y;
    let num_y = r * x.y - a * x.x;
    let k = vec3<f32>(num_x / s, num_y / s, x.z / r);
    let grad_s = 2.0 * r * grad_r;
    let grad_num_x = x.x * grad_r + vec3<f32>(r, a, 0.0);
    let grad_num_y = x.y * grad_r + vec3<f32>(-a, r, 0.0);
    let grad_kx = (s * grad_num_x - num_x * grad_s) / (s * s);
    let grad_ky = (s * grad_num_y - num_y * grad_s) / (s * s);
    let grad_kz = (r * vec3<f32>(0.0, 0.0, 1.0) - x.z * grad_r) / r2;

    return MetricPieces(f, k, grad_f, mat3x3<f32>(grad_kx, grad_ky, grad_kz));
}

struct Rates {
    dx: vec3<f32>,
    dp: vec3<f32>,
}

// Mirrors `rates`: Hamilton's equations for p_t = −1.
fn rates(spin: f32, x: vec3<f32>, p: vec3<f32>) -> Rates {
    let m = metric_pieces(spin, x);
    let l = 1.0 + dot(m.k, p);
    let dx = p - m.f * l * m.k;
    let dp = 0.5 * l * l * m.grad_f + m.f * l * (m.grad_k * p);
    return Rates(dx, dp);
}

struct RayState {
    x: vec3<f32>,
    p: vec3<f32>,
}

// Mirrors `rk4`.
fn rk4(spin: f32, x: vec3<f32>, p: vec3<f32>, h: f32) -> RayState {
    let k1 = rates(spin, x, p);
    let k2 = rates(spin, x + 0.5 * h * k1.dx, p + 0.5 * h * k1.dp);
    let k3 = rates(spin, x + 0.5 * h * k2.dx, p + 0.5 * h * k2.dp);
    let k4 = rates(spin, x + h * k3.dx, p + h * k3.dp);
    return RayState(
        x + h / 6.0 * (k1.dx + 2.0 * k2.dx + 2.0 * k3.dx + k4.dx),
        p + h / 6.0 * (k1.dp + 2.0 * k2.dp + 2.0 * k3.dp + k4.dp),
    );
}

// Mirrors `start_momentum`. w = 1 when light can arrive from `direction`, 0 when it cannot.
fn start_momentum(spin: f32, camera: vec3<f32>, direction: vec3<f32>) -> vec4<f32> {
    let n = -normalize(direction);
    let m = metric_pieces(spin, camera);
    let c = dot(m.k, n);
    let discriminant = 1.0 - m.f + m.f * c * c;
    if discriminant < 0.0 {
        return vec4<f32>(0.0);
    }
    let s = (-m.f * c + sqrt(discriminant)) / (1.0 + m.f * c * c);
    if s <= 0.0 {
        return vec4<f32>(0.0);
    }
    let k_v = 1.0 + s * c;
    let p_t = -1.0 + m.f * k_v;
    if p_t >= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>((s * n + m.f * k_v * m.k) / -p_t, 1.0);
}

// A crossing of the equatorial plane: its order, r, φ, where it is, and the light's spatial
// momentum there (p_t = -1).
struct RayHit {
    order: u32,
    radius: f32,
    azimuth: f32,
    position: vec3<f32>,
    momentum: vec3<f32>,
}

// Where, in Kerr–Schild r_g, the pixel's depth is taken from: the first gas the light passed
// through, or the hole's centre for the shadow. w = 1 once set; 0 leaves the pixel at the back.
var<private> depth_point: vec4<f32>;

// The depth written for the sky: just in front of the cleared depth (0 in Bevy's reversed Z), so
// the sky is drawn wherever nothing else is, and behind everything that is.
const SKY_DEPTH: f32 = 1.0e-12;

// Mirrors `OPAQUE_TRANSMITTANCE`: below this much light let through, a ray stops.
const OPAQUE_TRANSMITTANCE: f32 = 0.02;

// Mirrors `texture_coordinates`: x = the turn round the hole, y = sqrt of the way from the
// horizon to the edge.
fn texture_coordinates(r: f32, phi: f32) -> vec2<f32> {
    let horizon = black_hole.hole.y;
    let outer = black_hole.disc.x;
    let turn = phi / 6.2831853;
    let u = clamp((r - horizon) / (outer - horizon), 0.0, 1.0);
    return vec2<f32>(turn - floor(turn), sqrt(u));
}

// How much of the plane the gas covers at r, φ: the gas texture, splatted from the simulation's
// clumps each frame.
fn gas_coverage(r: f32, phi: f32) -> f32 {
    return textureSampleLevel(gas_texture, gas_sampler, texture_coordinates(r, phi), 0.0).r;
}

// Mirrors `tone_map`: 1 - e^(-exposure I) per channel.
fn tone_map(radiance: vec3<f32>, exposure: f32) -> vec3<f32> {
    return vec3<f32>(1.0) - exp(-exposure * max(radiance, vec3<f32>(0.0)));
}

// rgb (tone-mapped, not premultiplied) and alpha from what a ray gathered.
fn finish(colour: vec3<f32>, transmittance: f32) -> vec4<f32> {
    let alpha = 1.0 - transmittance;
    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(tone_map(colour / alpha, black_hole.shading.x), alpha);
}

// Mirrors `march_with` and `composite`: the light followed back through every crossing of the gas,
// each adding its colour times its coverage times the light still let through. The shadow is black
// and opaque; the sky lets through what is left. In the test pattern the disc is opaque instead.
fn march(camera: vec3<f32>, direction: vec3<f32>, lapse: f32) -> vec4<f32> {
    let spin = black_hole.hole.x;
    let horizon = black_hole.hole.y;
    let trapped = black_hole.hole.z;
    let isco = black_hole.hole.w;
    let outer = black_hole.disc.x;
    let step_fraction = black_hole.disc.y;
    let pattern = black_hole.limits.z == 1u;
    if radius(spin, camera) <= horizon {
        return vec4<f32>(0.0);
    }
    let start = start_momentum(spin, camera, direction);
    if start.w == 0.0 {
        return vec4<f32>(0.0);
    }
    var p = start.xyz;
    var x = camera;
    let escape = max(length(camera), 1.5 * outer) * 1.05;
    var count = 0u;
    var colour = vec3<f32>(0.0);
    var transmittance = 1.0;

    for (var step = 0u; step < black_hole.limits.x; step = step + 1u) {
        let dx = rates(spin, x, p).dx;
        let r = radius(spin, x);
        if r <= horizon * 1.001 || (r <= trapped && dot(x, dx) > 0.0) {
            if depth_point.w == 0.0 {
                depth_point = vec4<f32>(0.0, 0.0, 0.0, 1.0);
            }
            return finish(colour, 0.0);
        }
        if length(x) > escape && dot(x, dx) < 0.0 {
            if black_hole.limits.w == 1u {
                return finish(colour + transmittance * star_field(-normalize(dx), lapse), 0.0);
            }
            return finish(colour, transmittance);
        }

        let h = -step_fraction * length(x) / length(dx);
        let next = rk4(spin, x, p, h);

        if x.z * next.x.z < 0.0 || (next.x.z == 0.0 && x.z != 0.0) {
            let t = x.z / (x.z - next.x.z);
            let position = x + t * (next.x - x);
            let polar = equatorial_coordinates(spin, position);
            let hit = RayHit(
                count,
                polar.x,
                polar.y,
                vec3<f32>(position.x, position.y, 0.0),
                p + t * (next.p - p),
            );
            let on_disc = polar.x >= isco && polar.x <= outer;
            if pattern {
                if on_disc {
                    depth_point = vec4<f32>(hit.position, 1.0);
                    return vec4<f32>(disc_pattern(hit), 1.0);
                }
            } else if polar.x <= outer {
                let covered = clamp(gas_coverage(polar.x, polar.y), 0.0, 1.0);
                if covered > 0.0 {
                    if depth_point.w == 0.0 {
                        depth_point = vec4<f32>(hit.position, 1.0);
                    }
                    colour = colour + transmittance * covered * disc_colour(hit, lapse);
                    transmittance = transmittance * (1.0 - covered);
                }
                if transmittance < OPAQUE_TRANSMITTANCE {
                    return finish(colour, 0.0);
                }
            }
            count = count + 1u;
            if count >= black_hole.limits.y {
                return finish(colour, transmittance);
            }
        }
        x = next.x;
        p = next.p;
    }
    return finish(colour, transmittance);
}

// Mirrors `BLACK_HOLE_OUTLINE_COLOUR`.
const OUTLINE_COLOUR: vec3<f32> = vec3<f32>(1.0, 0.45, 0.1);
// Mirrors `BLACK_HOLE_OUTLINE_WIDTH_PX`.
const OUTLINE_WIDTH_PX: f32 = 2.0;
// Mirrors `horizon_measure`: positive where the sight line would hit the horizon (the spheroid
// (x² + y²)/(r₊² + a²) + z²/r₊² = 1), negative where it misses, zero where it grazes it.
// x = the measure, y = 1 if there is one.
fn horizon_measure(spin: f32, camera: vec3<f32>, direction: vec3<f32>) -> vec2<f32> {
    let r_plus = 1.0 + sqrt(max(1.0 - spin * spin, 0.0));
    let equator = sqrt(r_plus * r_plus + spin * spin);
    let scale = vec3<f32>(equator, equator, r_plus);
    let c = camera / scale;
    let d = direction / scale;
    let dd = dot(d, d);
    if dd == 0.0 || dot(c, c) <= 1.0 {
        return vec2<f32>(0.0);
    }
    let along = dot(c, d);
    var closest = dot(c, c);
    if along < 0.0 {
        let across = cross(c, d);
        closest = dot(across, across) / dd;
    }
    return vec2<f32>(1.0 - closest, 1.0);
}

// Mirrors `hash`: Chris Wellons' lowbias32.
fn hash(value: u32) -> u32 {
    var x = value;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

// Mirrors `unit`: a number in [0, 1) from a hash.
fn unit(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0;
}

// Mirrors `STAR_LAYERS` (cells per radian, chance of a star, brightest star) and `STAR_SIZE_RAD`.
const STAR_FINE: vec3<f32> = vec3<f32>(150.0, 0.08, 0.12);
const STAR_COARSE: vec3<f32> = vec3<f32>(40.0, 0.3, 0.35);
const STAR_SIZE_RAD: f32 = 0.0012;

// Mirrors `star_field`: the stars seen in direction d, scattered evenly over the whole sky.
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

// Mirrors `camera_lapse`: sqrt(-g_tt) at the camera, or 0 inside the ergosphere.
fn camera_lapse(spin: f32, camera: vec3<f32>) -> f32 {
    let f = metric_pieces(spin, camera).f;
    if f >= 1.0 {
        return 0.0;
    }
    return sqrt(1.0 - f);
}

// Mirrors `emitter`: the gas's four-velocity (xyz = u, w = u^t); w = 0 where there is none.
// On the disc a circular orbit; inside the ISCO the plunge with the ISCO orbit's E and L, from
// u_t = -E, x u_y - y u_x = L, and u.u = -1 in the Kerr–Schild metric.
fn emitter(hit: RayHit) -> vec4<f32> {
    let spin = black_hole.hole.x;
    let r = hit.radius;
    let x = hit.position;
    let m = metric_pieces(spin, x);
    if r >= black_hole.hole.w {
        let omega = 1.0 / (r * sqrt(r) + spin);
        let v = omega * vec3<f32>(-x.y, x.x, 0.0);
        let k_v = 1.0 + dot(m.k, v);
        let norm = -1.0 + dot(v, v) + m.f * k_v * k_v;
        if norm >= 0.0 {
            return vec4<f32>(0.0);
        }
        let u_t = 1.0 / sqrt(-norm);
        return vec4<f32>(u_t * v, u_t);
    }
    if r <= black_hole.hole.y * 1.01 {
        return vec4<f32>(0.0);
    }
    let e = black_hole.shading.z;
    let l = black_hole.shading.w;
    let f = m.f;
    let rho = sqrt(x.x * x.x + x.y * x.y);
    let out = vec3<f32>(x.x, x.y, 0.0) / rho;
    let round = vec3<f32>(-x.y, x.x, 0.0) / rho;
    let k_out = dot(m.k, out);
    let k_round = dot(m.k, round);
    let c = 1.0 - f + f * k_round * k_round;
    let n0 = plunge_parts(0.0, e, l, f, rho, k_out, k_round, c).z;
    let n1 = plunge_parts(1.0, e, l, f, rho, k_out, k_round, c).z;
    let n2 = plunge_parts(-1.0, e, l, f, rho, k_out, k_round, c).z;
    let qa = 0.5 * (n1 + n2) - n0;
    let qb = 0.5 * (n1 - n2);
    let qc = n0;
    let along = (-qb - sqrt(max(qb * qb - 4.0 * qa * qc, 0.0))) / (2.0 * qa);
    let parts = plunge_parts(along, e, l, f, rho, k_out, k_round, c);
    if parts.x <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(along * out + parts.y * round, parts.x);
}

// Mirrors the `parts` closure in `emitter`: for a radial speed `along`, (u^t, B, u.u + 1).
fn plunge_parts(
    along: f32,
    e: f32,
    l: f32,
    f: f32,
    rho: f32,
    k_out: f32,
    k_round: f32,
    c: f32,
) -> vec3<f32> {
    let k_u = (e + l * k_round / rho + along * k_out) / c;
    let u_t = e + f * k_u;
    let across = l / rho - f * k_u * k_round;
    let miss = -e * u_t + along * along + across * across
        + f * k_u * (along * k_out + across * k_round) + 1.0;
    return vec3<f32>(u_t, across, miss);
}

// Mirrors `redshift`: g = 1/(lapse (u^t - P.u)). 0 where there is no answer.
fn redshift(hit: RayHit, lapse: f32) -> f32 {
    let u = emitter(hit);
    if u.w <= 0.0 {
        return 0.0;
    }
    let g = 1.0 / (lapse * (u.w - dot(hit.momentum, u.xyz)));
    if g > 0.0 {
        return g;
    }
    return 0.0;
}

// Mirrors `profile_at`: (temperature, relative flux) at r.
fn profile_at(r: f32) -> vec2<f32> {
    let isco = black_hole.hole.w;
    let outer = black_hole.disc.x;
    let last = f32(PROFILE_SAMPLES - 1u);
    let u = clamp((r - isco) / (outer - isco), 0.0, 1.0);
    let s = sqrt(u) * last;
    let i = min(floor(s), last - 1.0);
    let t = s - i;
    let index = u32(i);
    return mix(black_hole.profile[index].xy, black_hole.profile[index + 1u].xy, t);
}

// Mirrors `blackbody`: linear RGB of a blackbody, brightest channel 1.
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

// Mirrors `disc_colour`: the light received, a blackbody at g T times g^4 times the flux. Inside
// the ISCO the gas keeps the inner edge's temperature, and its flux (at least 0.05) fades to the
// horizon.
fn disc_colour(hit: RayHit, lapse: f32) -> vec3<f32> {
    let g = redshift(hit, lapse);
    if g <= 0.0 {
        return vec3<f32>(0.0);
    }
    let horizon = black_hole.hole.y;
    let isco = black_hole.hole.w;
    var sample = profile_at(hit.radius);
    if hit.radius < isco {
        let fade = clamp((hit.radius - horizon) / (isco - horizon), 0.0, 1.0);
        let edge = black_hole.profile[1].xy;
        sample = vec2<f32>(edge.x, max(edge.y, 0.05) * fade);
    }
    let g2 = g * g;
    return blackbody(g * sample.x) * max(sample.y, 0.0) * g2 * g2;
}

// The test pattern on the disc: rings every 2 r_g and spokes every 30°, tinted by image order, so
// the lens's distortion is easy to read.
fn disc_pattern(hit: RayHit) -> vec3<f32> {
    let ring = fract(hit.radius / 2.0) < 0.5;
    let spoke = fract(hit.azimuth / 0.5235988) < 0.5;
    let shade = select(0.55, 1.0, ring != spoke);
    var tint = vec3<f32>(1.0, 0.62, 0.25);
    if hit.order == 1u {
        tint = vec3<f32>(0.3, 0.75, 1.0);
    } else if hit.order >= 2u {
        tint = vec3<f32>(0.6, 1.0, 0.4);
    }
    return tint * shade;
}

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

// Bevy's reversed-Z depth of a point in Kerr–Schild r_g, through the view's projection.
fn depth_of(point: vec3<f32>) -> f32 {
    let units = black_hole.centre.w;
    let world = black_hole.centre.xyz + units * vec3<f32>(point.y, point.z, point.x);
    let clip = view.clip_from_world * vec4<f32>(world, 1.0);
    return clamp(clip.z / clip.w, SKY_DEPTH, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let units = black_hole.centre.w;
    let camera_world = view.world_position;
    let direction = to_kerr_schild(normalize(in.world_position.xyz - camera_world));
    var camera = to_kerr_schild((camera_world - black_hole.centre.xyz) / units);
    let lapse = camera_lapse(black_hole.hole.x, camera);
    let camera_start = camera;

    // From outside the sphere the light is followed from where it enters it; bending outside it is
    // negligible.
    let sphere = black_hole.disc.z;
    let b = dot(camera, direction);
    let c = dot(camera, camera) - sphere * sphere;
    var inside = true;
    if c > 0.0 {
        let discriminant = b * b - c;
        if discriminant <= 0.0 || b > 0.0 {
            inside = false;
        } else {
            camera = camera + (-b - sqrt(discriminant)) * direction;
        }
    }

    depth_point = vec4<f32>(0.0);
    var pixel = vec4<f32>(0.0);
    if inside {
        pixel = march(camera, direction, lapse);
    }
    var depth = SKY_DEPTH;
    if depth_point.w == 1.0 {
        depth = depth_of(depth_point.xyz);
    }

    // The outline: where the horizon measure is zero, drawn an even width by how fast it changes
    // from pixel to pixel. Nothing is thrown away before `fwidth`, which compares this pixel with
    // its neighbours, so all of them must reach it; the switch is uniform, so they all do.
    if black_hole.shading.y == 1.0 {
        let measure = horizon_measure(black_hole.hole.x, camera_start, direction);
        let change = fwidth(measure.x);
        let across = abs(measure.x) / max(change, 1.0e-20);
        let cover = measure.y * (1.0 - smoothstep(0.5 * OUTLINE_WIDTH_PX - 0.5, 0.5 * OUTLINE_WIDTH_PX + 0.5, across));
        if cover > 0.0 {
            pixel = vec4<f32>(mix(pixel.rgb, OUTLINE_COLOUR, cover), max(pixel.a, cover));
            depth = depth_of(vec3<f32>(0.0));
        }
    }
    if pixel.a <= 0.0 {
        discard;
    }
    var out: FragmentOutput;
    out.colour = pixel;
    out.depth = depth;
    return out;
}
