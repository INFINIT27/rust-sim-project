//! The Rust twin of the wormhole's ray-marcher, `assets/shaders/wormhole.wgsl`: the `f32`
//! arithmetic the GPU does, function for function, where it can be tested.
//!
//! Change this first, let `wormhole_ray_tests` say whether the change is right against the `f64`
//! code in [`crate::physics_utils::relativity::wormhole`], then make the same change in the shader.
//! `wormhole_view_tests::the_shader_mirrors_its_twin` checks the shader's text.
//!
//! # Units and frames
//!
//! Lengths are in throat radii (`ρ = 1`). A place is `ℓ` and a **label**, the unit vector `n` that
//! says which way round the centre it is; the same labels run from one side through the throat to
//! the other. A direction at a place is given in the **label frame**: along `n` is `+ℓ`, across it
//! is round the centre.
//!
//! The two sides are drawn as two ordinary flat spaces. On the near side (`ℓ ≥ 0`) the world's axes
//! are the label frame's: a place is at `r n`. On the far side moving out is moving *down* `ℓ`, so
//! the world's axes there are the label frame's turned by [`far_frame`], and a place is at
//! `r M n` with `M` the mirror `z → −z`. The two mirrors (that one, and out ↔ in) make a rotation,
//! so nothing is seen left-right reversed, and for the opening view, along `z`, the rotation is none.

use bevy::math::Vec3;

use crate::render::constants::{
    WORMHOLE_BODY_AMBIENT, WORMHOLE_BODY_BRIGHTNESS, WORMHOLE_BODY_GLOW, WORMHOLE_NEBULA_CELLS,
    WORMHOLE_NEBULA_COLOUR, WORMHOLE_NEBULA_EDGES, WORMHOLE_NEBULA_TINT, WORMHOLE_PATTERN_FAR,
    WORMHOLE_PATTERN_LINE_RAD, WORMHOLE_PATTERN_NEAR, WORMHOLE_PATTERN_SPACING_RAD,
    WORMHOLE_RAY_MAX_STEPS, WORMHOLE_RAY_REACH, WORMHOLE_RAY_STEP_FRACTION,
    WORMHOLE_RAY_TUNNEL_STEP,
};
use crate::render::kerr_ray::{hash, star_field, unit};

/// A Dneg wormhole as the ray-marcher sees it, in throat radii, and the march's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayWormhole {
    /// Half the cylinder's length, `a/ρ`.
    pub half_length: f32,
    /// The flare, `ℳ/ρ`.
    pub flare: f32,
    /// Each step, as a fraction of the length the geometry changes over.
    pub step_fraction: f32,
    /// The most steps one ray takes.
    pub max_steps: u32,
}

impl RayWormhole {
    /// The wormhole with the shipped march settings.
    pub fn new(half_length: f32, flare: f32) -> Self {
        Self {
            half_length,
            flare,
            step_fraction: WORMHOLE_RAY_STEP_FRACTION,
            max_steps: WORMHOLE_RAY_MAX_STEPS,
        }
    }
}

/// The radius at `l`, in throat radii.
pub fn radius(w: &RayWormhole, l: f32) -> f32 {
    let outside = l.abs() - w.half_length;
    if outside <= 0.0 {
        return 1.0;
    }
    let x = std::f32::consts::FRAC_2_PI * outside / w.flare;
    1.0 + w.flare * (x * x.atan() - 0.5 * (1.0 + x * x).ln())
}

/// `dr/dℓ` at `l`.
pub fn slope(w: &RayWormhole, l: f32) -> f32 {
    let outside = l.abs() - w.half_length;
    if outside <= 0.0 {
        return 0.0;
    }
    let x = std::f32::consts::FRAC_2_PI * outside / w.flare;
    l.signum() * std::f32::consts::FRAC_2_PI * x.atan()
}

/// `(dℓ/ds, dα/ds, dp/ds)` for the ray at `(l, angle, p)` (an angle changes nothing).
pub fn rates(w: &RayWormhole, impact: f32, state: Vec3) -> Vec3 {
    let r = radius(w, state.x);
    let across = impact / (r * r);
    Vec3::new(state.z, across, impact * across * slope(w, state.x) / r)
}

/// One fourth-order Runge–Kutta step.
pub fn rk4(w: &RayWormhole, impact: f32, s: Vec3, h: f32) -> Vec3 {
    let k1 = rates(w, impact, s);
    let k2 = rates(w, impact, s + 0.5 * h * k1);
    let k3 = rates(w, impact, s + 0.5 * h * k2);
    let k4 = rates(w, impact, s + h * k3);
    s + h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4)
}

/// A label-frame direction at label `n`, in the far side's world axes: the rotation that is its
/// own description in the module docs, `M (v − 2 (v·n) n)`.
pub fn far_frame(n: Vec3, v: Vec3) -> Vec3 {
    let turned = v - 2.0 * v.dot(n) * n;
    Vec3::new(turned.x, turned.y, -turned.z)
}

/// The inverse of [`far_frame`]: a far-side world direction in the label frame.
pub fn from_far_frame(n: Vec3, v: Vec3) -> Vec3 {
    let mirrored = Vec3::new(v.x, v.y, -v.z);
    mirrored - 2.0 * mirrored.dot(n) * n
}

/// Where a ray's light comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RaySky {
    /// The near side's sky, in this direction of its world axes.
    Near(Vec3),
    /// The far side's sky, in this direction of its world axes.
    Far(Vec3),
    /// It was still circling the throat when the steps ran out.
    Lost,
}

/// A spherical body as the ray-marcher sees it, in throat radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayBody {
    /// Its label.
    pub n: Vec3,
    /// Its place along the wormhole.
    pub l: f32,
    /// Its radius.
    pub radius: f32,
}

/// What a ray met first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RayEnd {
    /// A sky, or nothing.
    Sky(RaySky),
    /// A body: which, and the outward direction of its surface there, in the label frame at it.
    Body(usize, Vec3),
}

/// Where the point at `l`, of radius `r`, and label `n` is from `body`, in the label frame at the body: exact where
/// space is flat, on the side of the body, and for points near the body anywhere.
pub fn offset(body: &RayBody, r: f32, l: f32, n: Vec3) -> Vec3 {
    let c = n.dot(body.n);
    let along = body.l.signum() * r * (c - 1.0) + (l - body.l);
    along * body.n + r * (n - c * body.n)
}

/// How far along the straight piece from `from` to `to` (offsets from a body) it first meets the
/// sphere of radius `radius` about the body from outside, as a fraction; `2` if it does not.
pub fn meets(from: Vec3, to: Vec3, radius: f32) -> f32 {
    let d = to - from;
    let a = d.dot(d);
    let b = from.dot(d);
    let c = from.dot(from) - radius * radius;
    let discriminant = b * b - a * c;
    if c < 0.0 || b >= 0.0 || discriminant < 0.0 {
        return 2.0;
    }
    let t = (-b - discriminant.sqrt()) / a;
    if t <= 1.0 {
        t
    } else {
        2.0
    }
}

/// [`march_past`] with no bodies in the way.
pub fn march(w: &RayWormhole, l: f32, n: Vec3, direction: Vec3) -> (RaySky, u32) {
    match march_past(w, &[], l, n, direction) {
        (RayEnd::Sky(sky), steps) => (sky, steps),
        (RayEnd::Body(..), steps) => (RaySky::Lost, steps),
    }
}

/// Follow the light that reaches a camera at `l` and label `n` from direction `direction` (label
/// frame, unit) back to the first of `bodies` it left, or the sky it came from. Also returns the
/// steps it took.
pub fn march_past(
    w: &RayWormhole,
    bodies: &[RayBody],
    l: f32,
    n: Vec3,
    direction: Vec3,
) -> (RayEnd, u32) {
    let p = direction.dot(n);
    let across = direction - p * n;
    let sine = across.length();
    // Straight along the axis there is no plane; any will do, since it never leaves the axis.
    let t = if sine > 1.0e-6 {
        across / sine
    } else {
        n.any_orthonormal_vector()
    };
    let impact = if sine > 1.0e-6 {
        radius(w, l) * sine
    } else {
        0.0
    };
    let mut s = Vec3::new(l, 0.0, p);
    let mut before = n;
    let mut r_before = radius(w, l);
    let mut steps = 0;
    while steps < w.max_steps {
        let outside = s.x.abs() - w.half_length;
        if outside >= WORMHOLE_RAY_REACH && s.x * s.z > 0.0 {
            break;
        }
        let last = s;
        if outside < 0.0 && s.z != 0.0 {
            // Inside the cylinder nothing bends: straight to the mouth it is heading for, in pieces
            // short enough to find bodies by when there are any.
            let mouth = if s.z > 0.0 {
                w.half_length
            } else {
                -w.half_length
            };
            let mut h = (mouth - s.x) / s.z;
            s.x = mouth;
            if !bodies.is_empty() && h > WORMHOLE_RAY_TUNNEL_STEP {
                h = WORMHOLE_RAY_TUNNEL_STEP;
                s.x = last.x + h * s.z;
            }
            s.y += impact * h;
        } else {
            let h = w.step_fraction * (w.flare * w.flare + outside.max(0.0).powi(2)).sqrt();
            s = rk4(w, impact, s, h);
        }
        steps += 1;
        if !bodies.is_empty() {
            let now = s.y.cos() * n + s.y.sin() * t;
            let r_now = radius(w, s.x);
            let mut nearest = 1.5;
            let mut hit = None;
            for (index, body) in bodies.iter().enumerate() {
                // No point is nearer a body than the difference of their l: most are out of reach.
                if last.x.min(s.x) > body.l + body.radius || last.x.max(s.x) < body.l - body.radius
                {
                    continue;
                }
                let from = offset(body, r_before, last.x, before);
                let to = offset(body, r_now, s.x, now);
                let along = meets(from, to, body.radius);
                if along < nearest {
                    nearest = along;
                    hit = Some((index, (from + along * (to - from)) / body.radius));
                }
            }
            if let Some((index, normal)) = hit {
                return (RayEnd::Body(index, normal), steps);
            }
            before = now;
            r_before = r_now;
        }
    }
    if steps >= w.max_steps {
        return (RayEnd::Sky(RaySky::Lost), steps);
    }
    let end_n = s.y.cos() * n + s.y.sin() * t;
    let end_t = s.y.cos() * t - s.y.sin() * n;
    let end = (s.z * end_n + impact / radius(w, s.x) * end_t).normalize();
    if s.x > 0.0 {
        (RayEnd::Sky(RaySky::Near(end)), steps)
    } else {
        (RayEnd::Sky(RaySky::Far(far_frame(end_n, end))), steps)
    }
}

/// The light a body sends towards the camera from where its surface faces `normal`: its own, if it
/// glows, or that of the star in direction `star` (both label frame at the body), or, with no star on
/// its side (`star` zero), a little.
pub fn body_light(colour: Vec3, glows: bool, normal: Vec3, star: Vec3) -> Vec3 {
    if glows {
        return colour * WORMHOLE_BODY_GLOW;
    }
    colour * WORMHOLE_BODY_BRIGHTNESS * (WORMHOLE_BODY_AMBIENT + normal.dot(star).max(0.0))
}

/// Smooth noise in `[0, 1]`: the hash at the corners of the cell `p` is in, blended.
pub fn value_noise(p: Vec3) -> f32 {
    let base = p.floor();
    let f = p - base;
    let w = f * f * (3.0 - 2.0 * f);
    let mut total = 0.0;
    for k in 0..8u32 {
        let corner = Vec3::new((k & 1) as f32, ((k >> 1) & 1) as f32, (k >> 2) as f32);
        let cell = base + corner;
        let h = hash(
            (cell.x as i32 as u32) ^ hash((cell.y as i32 as u32) ^ hash(cell.z as i32 as u32)),
        );
        let blend = Vec3::ONE - corner + (2.0 * corner - Vec3::ONE) * w;
        total += unit(h) * blend.x * blend.y * blend.z;
    }
    total
}

/// The glow of the far side's sky in direction `d`: a nebula, so the two skies cannot be taken for
/// one another.
pub fn nebula(d: Vec3) -> Vec3 {
    let p = d * WORMHOLE_NEBULA_CELLS;
    let cloud = (value_noise(p) + 0.5 * value_noise(2.0 * p) + 0.25 * value_noise(4.0 * p)) / 1.75;
    let edges = WORMHOLE_NEBULA_EDGES;
    let x = ((cloud - edges.0) / (edges.1 - edges.0)).clamp(0.0, 1.0);
    let glow = x * x * (3.0 - 2.0 * x);
    let tint = value_noise(0.7 * p + Vec3::splat(19.0));
    glow * WORMHOLE_NEBULA_COLOUR.lerp(WORMHOLE_NEBULA_TINT, tint)
}

/// The near side's sky: the same stars as the black hole's.
pub fn near_sky(d: Vec3, stars: bool) -> Vec3 {
    if stars {
        star_field(d, 1.0)
    } else {
        Vec3::ZERO
    }
}

/// The far side's sky: other stars, twice as many, and the nebula.
pub fn far_sky(d: Vec3, stars: bool) -> Vec3 {
    let mut light = nebula(d);
    if stars {
        light += star_field(Vec3::new(d.y, d.z, d.x), 1.0);
        light += star_field(Vec3::new(d.z, d.x, d.y), 1.0);
    }
    light
}

/// The test pattern: lines of latitude and longitude, one colour a side.
pub fn pattern(d: Vec3, far: bool) -> Vec3 {
    let spacing = WORMHOLE_PATTERN_SPACING_RAD;
    let off = |angle: f32| {
        let turns = angle / spacing;
        (turns - (turns + 0.5).floor()).abs() * spacing
    };
    let latitude = d.y.clamp(-1.0, 1.0).asin();
    let longitude = d.x.atan2(-d.z);
    let near_line = off(latitude) < WORMHOLE_PATTERN_LINE_RAD
        || off(longitude) * latitude.cos() < WORMHOLE_PATTERN_LINE_RAD;
    let colour = if far {
        WORMHOLE_PATTERN_FAR
    } else {
        WORMHOLE_PATTERN_NEAR
    };
    colour * if near_line { 1.0 } else { 0.08 }
}

#[cfg(test)]
#[path = "../../tst/render/wormhole_ray_tests.rs"]
mod tests;
