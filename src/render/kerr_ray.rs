//! The black hole's light, followed one pixel at a time: the Rust twin of the planned GPU
//! ray-marcher (phase 0 of the plan in `notes.md`).
//!
//! # Why a twin
//!
//! The lensed disc is to be drawn by a shader that follows, for every pixel, the light reaching the
//! camera through it backwards along its exact Kerr path. A shader cannot be unit-tested, so it is
//! written here first, in `f32` like the GPU, function for function as the WGSL will be, and tested
//! against the `f64` Kerr code in [`crate::physics_utils::relativity::kerr`] and against the
//! analytic shadow. The WGSL is then a transcription of this file, and this file is what it is
//! checked against.
//!
//! # Units
//!
//! Everything is in gravitational radii (`r_g = GM/c²`, so `M = 1`) and relative to the hole, in
//! Kerr–Schild axes (spin along `+z`). Numbers then stay between about `1` and `10⁴`, which `f32`
//! handles: `kerr_ray_tests` checks that the null condition and the conserved momenta hold to
//! `10⁻⁴` along rays that swing round the photon orbit.
//!
//! # Equations of motion: Hamiltonian form
//!
//! The metric is the Cartesian Kerr–Schild one, `g = η + f k k`, whose inverse is
//! `g^μν = η^μν − f k^μ k^ν` with `k^μ = (−1, k⃗)`. With the light's covariant momentum `p_μ`, the
//! Hamiltonian `H = ½ g^μν p_μ p_ν` gives
//!
//! - `dx^i/dλ = p_i − f L k_i`,
//! - `dp_i/dλ = ½ L² ∂_i f + f L (∂_i k⃗ · p⃗)`,
//!
//! with `L = k^μ p_μ = −p_t + k⃗·p⃗`. The metric does not depend on time, so `p_t` is constant: it is
//! fixed at `−1` (energy `1` far away), and only `x` and `p⃗` are stepped. No Christoffel symbols and
//! no matrix inverse are needed, only `f`, `k⃗` and their first derivatives ([`metric_pieces`]),
//! which is what makes this cheap enough for a shader. `p⃗` at the disc is also exactly what the
//! redshift needs: `P_i = p_i/(−p_t) = p_i`.
//!
//! # The march
//!
//! [`march`] starts the light at the camera, moving towards it along the pixel's direction
//! ([`start_momentum`]), and steps it backwards in time (negative `λ`) with RK4, each step
//! [`KERR_RAY_STEP_FRACTION`] of the distance from the hole. It stops when the light
//!
//! - crosses the disc between the ISCO and the outer edge: the disc is opaque ([`RayEnd::Disc`]);
//! - reaches the horizon, or is inside the innermost (prograde) photon orbit still falling in,
//!   from which it could never turn back out: the **shadow** ([`RayEnd::Shadow`]);
//! - is past everything and moving away: it came from the sky ([`RayEnd::Sky`]);
//! - runs out of steps or plane crossings ([`RayEnd::Exhausted`]).
//!
//! Every crossing of the equatorial plane on the way is recorded ([`RayCrossing`]), with its image
//! order (how many crossings came before it) and its Boyer–Lindquist `r` and `φ`.
//!
//! # Shading
//!
//! [`disc_colour`] gives a disc crossing the light the camera receives from it: the colour of a
//! blackbody ([`blackbody`]) at the redshifted temperature `g T(r)`, at an intensity `g⁴` times the
//! disc's flux there. `g⁴` is Liouville's theorem: `I_ν/ν³` is the same all along a ray, so the
//! bolometric intensity scales as `g⁴`, and the lens's magnification needs no extra factor (a
//! magnified patch of disc covers more pixels at the same intensity). The disc's temperature and
//! flux come from [`DiscShading`], [`KERR_RAY_PROFILE_SAMPLES`] samples of [`DiscProfile`] spaced as
//! it spaces them, uniformly in `√(r − r_ISCO)`.
//!
//! That intensity spans orders of magnitude, from the beamed inner edge to the outer disc, where
//! the flux falls as `r⁻³`. [`composite`] adds it up in linear light and [`tone_map`]s the total like
//! a photograph, `1 − e^{−exposure · I}` per channel, so it never clips and the faint outer disc
//! still shows.
//!
//! # The gas: plunging, and see-through
//!
//! Inside the ISCO the gas falls in, keeping the energy and angular momentum of the ISCO orbit, as
//! the simulation's clumps do; [`emitter`] gives its four-velocity from those two conserved
//! numbers in the same Kerr–Schild metric, so [`redshift`] and [`disc_colour`] work there too.
//!
//! The gas does not cover the whole plane. [`composite`] follows the light back through every
//! crossing between the horizon and the outer edge, each covered to the degree the gas texture
//! says ([`texture_coordinates`]), adding each one's colour and letting the rest through: the gaps
//! between clumps show the images behind.
//!
//! # Not yet
//!
//! The camera's pixel directions are coordinate directions; at the camera's distance (tens of
//! `r_g`) the difference from a local observer's frame is a fraction of a percent.

use bevy::math::{Mat3, Vec2, Vec3, Vec4};

use crate::physics_utils::relativity::kerr::{
    circular_orbit, horizon_radius, isco_radius, photon_orbit_radius,
};
use crate::render::accretion_disc::DiscProfile;
use crate::render::constants::{
    BLACK_HOLE_EXPOSURE, KERR_RAY_MAX_CROSSINGS, KERR_RAY_MAX_STEPS, KERR_RAY_PROFILE_SAMPLES,
    KERR_RAY_STEP_FRACTION,
};

/// The hole and the march's settings, as the shader's uniform will hold them. Lengths in `r_g`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHole {
    /// The spin `a/M`, `0` to `1`.
    pub spin: f32,
    /// The outer horizon `r₊`.
    pub horizon: f32,
    /// The prograde photon orbit: inside it, light falling in (backwards in time) never escapes.
    pub trapped: f32,
    /// The prograde ISCO, the disc's inner edge.
    pub isco: f32,
    /// The disc's outer edge. Below `isco` there is no disc, only the hole.
    pub outer: f32,
    /// Each step, as a fraction of the distance from the hole.
    pub step_fraction: f32,
    /// The most steps one ray takes.
    pub max_steps: u32,
    /// The most plane crossings one ray makes before it is abandoned.
    pub max_crossings: u32,
    /// Energy per unit mass of the circular orbit at the ISCO, which gas keeps as it plunges.
    pub isco_energy: f32,
    /// Angular momentum per unit mass of the circular orbit at the ISCO, which gas keeps as it
    /// plunges.
    pub isco_angular_momentum: f32,
}

impl RayHole {
    /// The hole of `spin` with a disc out to `outer_rg`, and the default march settings.
    ///
    /// # Returns
    ///
    /// `None` for a spin outside `0..=1` or a non-finite edge.
    pub fn new(spin: f64, outer_rg: f64) -> Option<RayHole> {
        if !outer_rg.is_finite() {
            return None;
        }
        let isco = isco_radius(spin, true)?;
        let orbit = circular_orbit(spin, isco * (1.0 + 1.0e-9), true)?;
        Some(RayHole {
            spin: spin as f32,
            horizon: horizon_radius(spin)? as f32,
            trapped: photon_orbit_radius(spin, true)? as f32,
            isco: isco as f32,
            outer: outer_rg as f32,
            step_fraction: KERR_RAY_STEP_FRACTION,
            max_steps: KERR_RAY_MAX_STEPS,
            max_crossings: KERR_RAY_MAX_CROSSINGS as u32,
            isco_energy: orbit.energy as f32,
            isco_angular_momentum: orbit.angular_momentum as f32,
        })
    }
}

/// `f`, `k⃗` and their spatial derivatives at one point: everything the equations of motion need.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetricPieces {
    /// `f = 2r³/(r⁴ + a²z²)`.
    pub f: f32,
    /// The spatial part of `k_μ`: `((rx + ay)/(r² + a²), (ry − ax)/(r² + a²), z/r)`.
    pub k: Vec3,
    /// `∂_i f`.
    pub grad_f: Vec3,
    /// Columns `∇k_x`, `∇k_y`, `∇k_z`, so that `grad_k * p` is the vector of `∂_i k⃗ · p⃗`.
    pub grad_k: Mat3,
}

/// The Boyer–Lindquist radius at Kerr–Schild point `x`: `f32`
/// [`kerr_schild_radius`](crate::physics_utils::relativity::kerr::kerr_schild_radius).
pub fn radius(spin: f32, x: Vec3) -> f32 {
    let a2 = spin * spin;
    let b = x.dot(x) - a2;
    (0.5 * (b + (b * b + 4.0 * a2 * x.z * x.z).sqrt())).sqrt()
}

/// The Boyer–Lindquist `r` and `φ` of a point in the equatorial plane (`z` ignored): `f32`
/// [`equatorial_coordinates`](crate::physics_utils::relativity::kerr::equatorial_coordinates).
pub fn equatorial_coordinates(spin: f32, x: Vec3) -> (f32, f32) {
    let r = radius(spin, Vec3::new(x.x, x.y, 0.0));
    (r, x.y.atan2(x.x) + spin.atan2(r))
}

/// [`MetricPieces`] at `x`, all analytic, as `metric_pieces` in
/// [`crate::physics_utils::relativity::kerr`]. `∂_i r` comes from differentiating
/// `r⁴ − (ρ² − a²) r² − a² z² = 0`.
pub fn metric_pieces(spin: f32, x: Vec3) -> MetricPieces {
    let a = spin;
    let a2 = a * a;
    let r = radius(spin, x);
    let r2 = r * r;
    let rho2 = x.dot(x);

    let grad_r = (r2 * x + Vec3::new(0.0, 0.0, a2 * x.z)) / (r * (2.0 * r2 - rho2 + a2));

    let q = r2 * r2 + a2 * x.z * x.z;
    let f = 2.0 * r2 * r / q;
    let grad_q = 4.0 * r2 * r * grad_r + Vec3::new(0.0, 0.0, 2.0 * a2 * x.z);
    let grad_f = (6.0 * r2 * q * grad_r - 2.0 * r2 * r * grad_q) / (q * q);

    let s = r2 + a2;
    let num_x = r * x.x + a * x.y;
    let num_y = r * x.y - a * x.x;
    let k = Vec3::new(num_x / s, num_y / s, x.z / r);
    let grad_s = 2.0 * r * grad_r;
    let grad_num_x = x.x * grad_r + Vec3::new(r, a, 0.0);
    let grad_num_y = x.y * grad_r + Vec3::new(-a, r, 0.0);
    let grad_kx = (s * grad_num_x - num_x * grad_s) / (s * s);
    let grad_ky = (s * grad_num_y - num_y * grad_s) / (s * s);
    let grad_kz = (r * Vec3::Z - x.z * grad_r) / r2;

    MetricPieces {
        f,
        k,
        grad_f,
        grad_k: Mat3::from_cols(grad_kx, grad_ky, grad_kz),
    }
}

/// `(dx/dλ, dp/dλ)` for light at `x` with spatial momentum `p` (and `p_t = −1`).
pub fn rates(spin: f32, x: Vec3, p: Vec3) -> (Vec3, Vec3) {
    let m = metric_pieces(spin, x);
    let l = 1.0 + m.k.dot(p);
    let dx = p - m.f * l * m.k;
    let dp = 0.5 * l * l * m.grad_f + m.f * l * (m.grad_k * p);
    (dx, dp)
}

/// `H = ½ g^μν p_μ p_ν` for `p_t = −1`: zero for light, so its size along a ray is the march's
/// error.
pub fn hamiltonian(spin: f32, x: Vec3, p: Vec3) -> f32 {
    let m = metric_pieces(spin, x);
    let l = 1.0 + m.k.dot(p);
    0.5 * (-1.0 + p.dot(p) - m.f * l * l)
}

/// One RK4 step of `h` in `λ` (negative to go backwards in time).
pub fn rk4(spin: f32, x: Vec3, p: Vec3, h: f32) -> (Vec3, Vec3) {
    let (dx1, dp1) = rates(spin, x, p);
    let (dx2, dp2) = rates(spin, x + 0.5 * h * dx1, p + 0.5 * h * dp1);
    let (dx3, dp3) = rates(spin, x + 0.5 * h * dx2, p + 0.5 * h * dp2);
    let (dx4, dp4) = rates(spin, x + h * dx3, p + h * dp3);
    (
        x + h / 6.0 * (dx1 + 2.0 * dx2 + 2.0 * dx3 + dx4),
        p + h / 6.0 * (dp1 + 2.0 * dp2 + 2.0 * dp3 + dp4),
    )
}

/// `√(−g_tt)` at the camera: how much a camera sitting there blueshifts what it sees, against one
/// far away. `None` inside the ergosphere, where nothing can sit still.
pub fn camera_lapse(spin: f32, camera: Vec3) -> Option<f32> {
    let f = metric_pieces(spin, camera).f;
    (f < 1.0).then(|| (1.0 - f).sqrt())
}

/// The spatial momentum, with `p_t = −1`, of light arriving at `camera` from `direction` (the way
/// the camera looks): travelling along `−direction` at the speed that makes it null there.
///
/// The null speed `s` along a unit `n` solves `(f − 1) + 2 s f c + s² (1 + f c²) = 0` with
/// `c = k⃗·n`, so `s = (−f c + √(1 − f + f c²))/(1 + f c²)`; then `p_μ = g_μν (1, s n)`, rescaled.
///
/// # Returns
///
/// `None` for a zero direction, or where light cannot move that way (inside the horizon, or against
/// the spin inside the ergosphere).
pub fn start_momentum(spin: f32, camera: Vec3, direction: Vec3) -> Option<Vec3> {
    let n = -direction.normalize_or_zero();
    if n == Vec3::ZERO {
        return None;
    }
    let m = metric_pieces(spin, camera);
    let c = m.k.dot(n);
    let discriminant = 1.0 - m.f + m.f * c * c;
    if discriminant < 0.0 {
        return None;
    }
    let s = (-m.f * c + discriminant.sqrt()) / (1.0 + m.f * c * c);
    if s <= 0.0 || !s.is_finite() {
        return None;
    }
    let k_v = 1.0 + s * c;
    let p_t = -1.0 + m.f * k_v;
    if p_t >= 0.0 {
        return None;
    }
    Some((s * n + m.f * k_v * m.k) / -p_t)
}

/// How far the sight line from `camera` along `direction` passes inside the event horizon:
/// positive if it would hit the horizon, negative if it misses, zero where it just grazes it, and
/// changing smoothly across that edge. The shader traces the outline where it is zero
/// ([`BLACK_HOLE_OUTLINE_WIDTH_PX`](crate::render::constants::BLACK_HOLE_OUTLINE_WIDTH_PX)).
///
/// In Kerr–Schild coordinates the horizon, `r = r₊`, is the oblate spheroid
/// `(x² + y²)/(r₊² + a²) + z²/r₊² = 1`. Scaled by those semi-axes it is the unit sphere, and a
/// straight line `u(t) = c + t d` comes closest to its centre where `|u|²` is least:
/// `|c × d|² / |d|²` (the same as `|c|² − (c·d)²/|d|²`, but without subtracting two large numbers, which `f32` cannot do from far off), at `t = −c·d/|d|²`. The measure is one minus that (or minus `|c|²`, the
/// camera's own, when the closest point is behind the camera).
///
/// This is the horizon where it really is, not where it is seen: light bends round the hole, so
/// every ray that ends on the horizon is seen within the shadow, which is about `2.5` times wider.
/// The outline marks the hole's true size and place inside it.
///
/// # Returns
///
/// `None` for a zero direction or a camera on or inside the horizon.
pub fn horizon_measure(spin: f32, camera: Vec3, direction: Vec3) -> Option<f32> {
    let r_plus = 1.0 + (1.0 - spin * spin).max(0.0).sqrt();
    let scale = Vec3::new(
        (r_plus * r_plus + spin * spin).sqrt(),
        (r_plus * r_plus + spin * spin).sqrt(),
        r_plus,
    );
    let c = camera / scale;
    let d = direction / scale;
    let dd = d.dot(d);
    if dd == 0.0 || c.dot(c) <= 1.0 {
        return None;
    }
    let along = c.dot(d);
    let closest = if along < 0.0 {
        c.cross(d).length_squared() / dd
    } else {
        c.dot(c)
    };
    Some(1.0 - closest)
}

/// One crossing of the equatorial plane by a backward ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayCrossing {
    /// How many crossings came before it: `0` for the first image, `1` for the second, …
    pub order: u32,
    /// Boyer–Lindquist radius, in `r_g`.
    pub radius: f32,
    /// Boyer–Lindquist azimuth, in radians.
    pub azimuth: f32,
    /// Where, in Kerr–Schild coordinates.
    pub position: Vec3,
    /// The light's spatial momentum there (`p_t = −1`): `P` in the redshift.
    pub momentum: Vec3,
    /// Whether it is on the disc, between the ISCO and the outer edge (where the ray stops).
    pub on_disc: bool,
}

/// How a backward ray ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RayEnd {
    /// It reached the disc: the last crossing.
    Disc,
    /// It fell into the hole: the pixel is in the shadow.
    Shadow,
    /// It came from the sky, from `direction` (unit, pointing away from the hole).
    Sky {
        /// Where on the sky the light came from.
        direction: Vec3,
    },
    /// It ran out of steps or crossings first.
    Exhausted,
    /// No light reaches the camera from that direction (or the camera is inside the horizon).
    Invalid,
}

/// Everything one backward ray found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayTrace {
    /// Its first [`KERR_RAY_MAX_CROSSINGS`] plane crossings, in order.
    pub crossings: [Option<RayCrossing>; KERR_RAY_MAX_CROSSINGS],
    /// How it ended.
    pub end: RayEnd,
    /// How many RK4 steps it took.
    pub steps: u32,
    /// The crossing it stopped at ([`RayEnd::Disc`]).
    pub stopped_at: Option<RayCrossing>,
}

impl RayTrace {
    /// The crossing it stopped at, if it stopped at one.
    pub fn disc(&self) -> Option<RayCrossing> {
        if self.end != RayEnd::Disc {
            return None;
        }
        self.stopped_at
    }
}

/// Follow the light reaching `camera` (in `r_g`, Kerr–Schild axes) from `direction` backwards
/// until it reaches the disc (opaque between the ISCO and the outer edge), falls in, comes from
/// the sky, or runs out of steps.
pub fn march(hole: &RayHole, camera: Vec3, direction: Vec3) -> RayTrace {
    march_with(hole, camera, direction, |crossing| crossing.on_disc)
}

/// [`march`], calling `stop` at every plane crossing and stopping there ([`RayEnd::Disc`]) when it
/// returns `true`: how the shader sees through gaps in the gas ([`composite`]).
pub fn march_with(
    hole: &RayHole,
    camera: Vec3,
    direction: Vec3,
    mut stop: impl FnMut(&RayCrossing) -> bool,
) -> RayTrace {
    let mut trace = RayTrace {
        crossings: [None; KERR_RAY_MAX_CROSSINGS],
        end: RayEnd::Invalid,
        steps: 0,
        stopped_at: None,
    };
    let spin = hole.spin;
    if radius(spin, camera) <= hole.horizon {
        return trace;
    }
    let Some(mut p) = start_momentum(spin, camera, direction) else {
        return trace;
    };
    let mut x = camera;
    let escape = camera.length().max(1.5 * hole.outer) * 1.05;
    let mut count = 0u32;
    trace.end = RayEnd::Exhausted;

    while trace.steps < hole.max_steps {
        let (dx, _) = rates(spin, x, p);
        let r = radius(spin, x);
        // Inside the innermost photon orbit and still falling in (backwards in time; moving
        // outward forwards): it only creeps closer to the horizon.
        if r <= hole.horizon * 1.001 || (r <= hole.trapped && x.dot(dx) > 0.0) {
            trace.end = RayEnd::Shadow;
            return trace;
        }
        // Past everything and moving away, backwards in time.
        if x.length() > escape && x.dot(dx) < 0.0 {
            trace.end = RayEnd::Sky {
                direction: -dx.normalize(),
            };
            return trace;
        }

        let h = -hole.step_fraction * x.length() / dx.length();
        let (nx, np) = rk4(spin, x, p, h);
        trace.steps += 1;

        if x.z * nx.z < 0.0 || (nx.z == 0.0 && x.z != 0.0) {
            let t = x.z / (x.z - nx.z);
            let position = x + t * (nx - x);
            let (r, phi) = equatorial_coordinates(spin, position);
            let crossing = RayCrossing {
                order: count,
                radius: r,
                azimuth: phi,
                position: Vec3::new(position.x, position.y, 0.0),
                momentum: p + t * (np - p),
                on_disc: r >= hole.isco && r <= hole.outer,
            };
            if (count as usize) < KERR_RAY_MAX_CROSSINGS {
                trace.crossings[count as usize] = Some(crossing);
            }
            if stop(&crossing) {
                trace.end = RayEnd::Disc;
                trace.stopped_at = Some(crossing);
                return trace;
            }
            count += 1;
            if count >= hole.max_crossings {
                return trace;
            }
        }
        x = nx;
        p = np;
    }
    trace
}

/// The four-velocity `(u^t, u⃗)` of the gas at `crossing`, in Kerr–Schild coordinates.
///
/// - **On the disc** (`r ≥ r_ISCO`): a circular orbit at `Ω = 1/(r^{3/2} + a)`, so
///   `u⃗ = u^t Ω (−y, x, 0)` and `u^t = 1/√(1 − v² − f (1 + k⃗·v)²)`.
/// - **Inside the ISCO**: falling in with the ISCO orbit's energy `E` and angular momentum `L`, as
///   the simulation's clumps do. Both are conserved along the fall, because `∂_t` and
///   `x∂_y − y∂_x` are symmetries of the metric, so the four-velocity is fixed by
///   `u_t = −E`, `x u_y − y u_x = L`, and `u·u = −1`, all in the same Kerr–Schild metric
///   ([`metric_pieces`]) everything else uses, with no change of coordinates. Writing the in-plane
///   velocity as `u⃗ = A r̂ + B φ̂`, the first two give `u^t` and `B` linear in `A`, and the third a
///   quadratic in `A`, whose more negative root is the gas falling in.
///
/// # Returns
///
/// `None` at or inside `1.01` horizon radii, or where the numbers break down.
pub fn emitter(hole: &RayHole, crossing: &RayCrossing) -> Option<(f32, Vec3)> {
    let spin = hole.spin;
    let r = crossing.radius;
    let x = crossing.position;
    let m = metric_pieces(spin, x);
    if r >= hole.isco {
        let omega = 1.0 / (r * r.sqrt() + spin);
        let v = omega * Vec3::new(-x.y, x.x, 0.0);
        let k_v = 1.0 + m.k.dot(v);
        let norm = -1.0 + v.dot(v) + m.f * k_v * k_v;
        if norm >= 0.0 {
            return None;
        }
        let u_t = 1.0 / (-norm).sqrt();
        return Some((u_t, u_t * v));
    }
    if r <= hole.horizon * 1.01 {
        return None;
    }
    let (e, l, f) = (hole.isco_energy, hole.isco_angular_momentum, m.f);
    let rho = (x.x * x.x + x.y * x.y).sqrt();
    let out = Vec3::new(x.x, x.y, 0.0) / rho;
    let round = Vec3::new(-x.y, x.x, 0.0) / rho;
    let (k_out, k_round) = (m.k.dot(out), m.k.dot(round));
    // K = k_μ u^μ = u^t + k⃗·u⃗; from u_t = −u^t + f K = −E and the definition of K:
    let c = 1.0 - f + f * k_round * k_round;
    // For a radial speed `along`: K, u^t, B, and how far u·u is from −1.
    let parts = |along: f32| {
        let k_u = (e + l * k_round / rho + along * k_out) / c;
        let u_t = e + f * k_u;
        let across = l / rho - f * k_u * k_round;
        let miss = -e * u_t
            + along * along
            + across * across
            + f * k_u * (along * k_out + across * k_round)
            + 1.0;
        (u_t, across, miss)
    };
    let (n0, n1, n2) = (parts(0.0).2, parts(1.0).2, parts(-1.0).2);
    let (qa, qb, qc) = (0.5 * (n1 + n2) - n0, 0.5 * (n1 - n2), n0);
    let along = (-qb - (qb * qb - 4.0 * qa * qc).max(0.0).sqrt()) / (2.0 * qa);
    let (u_t, across, _) = parts(along);
    let u = along * out + across * round;
    (u_t.is_finite() && u.is_finite() && u_t > 0.0).then_some((u_t, u))
}

/// The redshift factor `g = ν_seen/ν_emitted` of the light at `crossing`, for a camera with
/// [`camera_lapse`] `lapse`: `g = 1/(lapse (u^t − P·u⃗))` with the gas's four-velocity from
/// [`emitter`]. On the disc that is `1/(lapse · u^t (1 − P·v))`, the formula in
/// [`crate::render::accretion_disc::DiscParticle::redshift_factor_along`].
///
/// # Returns
///
/// `None` where [`emitter`] has no answer (at the horizon) or the numbers break down.
pub fn redshift(hole: &RayHole, crossing: &RayCrossing, lapse: f32) -> Option<f32> {
    let (u_t, u) = emitter(hole, crossing)?;
    let g = 1.0 / (lapse * (u_t - crossing.momentum.dot(u)));
    (g.is_finite() && g > 0.0).then_some(g)
}

/// Where the gas texture is read for the point at Boyer–Lindquist `r`, `φ`: `x` the turn round the
/// hole (`φ/2π`, wrapping), `y` the radius from the horizon to the outer edge, uniformly in
/// `√(r − r₊)` so the plunge and the inner disc get the most texels.
pub fn texture_coordinates(hole: &RayHole, r: f32, phi: f32) -> Vec2 {
    let turn = phi / std::f32::consts::TAU;
    let u = ((r - hole.horizon) / (hole.outer - hole.horizon)).clamp(0.0, 1.0);
    Vec2::new(turn - turn.floor(), u.sqrt())
}

/// The inverse of [`texture_coordinates`]'s `y`: the radius at texture height `y`.
pub fn texture_radius(hole: &RayHole, y: f32) -> f32 {
    hole.horizon + (hole.outer - hole.horizon) * y * y
}

/// What the disc looks like, as the shader's uniform will hold it: its temperature and flux against
/// radius, and the exposure the picture is taken at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiscShading {
    /// `(temperature in kelvin, flux over the peak flux)` at radii from the ISCO to the outer edge,
    /// uniformly spaced in `√(r − r_ISCO)`.
    pub profile: [Vec2; KERR_RAY_PROFILE_SAMPLES],
    /// [`BLACK_HOLE_EXPOSURE`]: what [`tone_map`] multiplies the light by.
    pub exposure: f32,
}

impl DiscShading {
    /// `profile` sampled at the radii [`profile_at`] reads, from `hole`'s ISCO to its outer edge.
    pub fn new(hole: &RayHole, profile: &DiscProfile) -> DiscShading {
        let span = (hole.outer - hole.isco).max(0.0) as f64;
        let mut samples = [Vec2::ZERO; KERR_RAY_PROFILE_SAMPLES];
        for (i, sample) in samples.iter_mut().enumerate() {
            let s = i as f64 / (KERR_RAY_PROFILE_SAMPLES - 1) as f64;
            let r = hole.isco as f64 + span * s * s;
            *sample = Vec2::new(
                profile.temperature_at(r) as f32,
                profile.relative_flux_at(r) as f32,
            );
        }
        DiscShading {
            profile: samples,
            exposure: BLACK_HOLE_EXPOSURE,
        }
    }
}

/// `(temperature, relative flux)` at radius `r`, interpolated from `shading`'s samples.
pub fn profile_at(hole: &RayHole, shading: &DiscShading, r: f32) -> Vec2 {
    let last = (KERR_RAY_PROFILE_SAMPLES - 1) as f32;
    let u = ((r - hole.isco) / (hole.outer - hole.isco)).clamp(0.0, 1.0);
    let s = u.sqrt() * last;
    let i = s.floor().min(last - 1.0);
    let t = s - i;
    let i = i as usize;
    shading.profile[i].lerp(shading.profile[i + 1], t)
}

/// The colour of a blackbody at `temperature` kelvin, linear RGB scaled so its brightest channel
/// is `1`: `f32`
/// [`blackbody_rgb`](crate::render::accretion_disc::blackbody_rgb).
pub fn blackbody(temperature: f32) -> Vec3 {
    let t = temperature.clamp(1000.0, 40000.0) / 100.0;
    let red = if t <= 66.0 {
        255.0
    } else {
        329.698_73 * (t - 60.0).powf(-0.133_204_76)
    };
    let green = if t <= 66.0 {
        99.470_8 * t.ln() - 161.119_57
    } else {
        288.122_17 * (t - 60.0).powf(-0.075_514_85)
    };
    let blue = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_73 * (t - 10.0).ln() - 305.044_8
    };
    let srgb = (Vec3::new(red, green, blue) / 255.0).clamp(Vec3::ZERO, Vec3::ONE);
    let low = srgb / 12.92;
    let high = ((srgb + 0.055) / 1.055).powf(2.4);
    let linear = Vec3::select(srgb.cmple(Vec3::splat(0.040_45)), low, high);
    linear / linear.max_element().max(1.0e-12)
}

/// The light the camera receives from a `crossing` through the gas, for a camera with
/// [`camera_lapse`] `lapse`, in linear RGB before exposure: [`blackbody`] at `g T`, times `g⁴`
/// times the flux (`1` at the disc's peak).
///
/// On the disc `T` and the flux are the profile's at `r`. Inside the ISCO the Page–Thorne disc
/// radiates nothing, but the plunging gas is drawn anyway so the plunge can be seen: it keeps the
/// temperature of the disc's inner edge (the profile's second sample) and its flux, at least
/// `0.05`, fades to nothing at the horizon.
///
/// # Returns
///
/// `None` where [`redshift`] has no answer (at the horizon).
pub fn disc_colour(
    hole: &RayHole,
    shading: &DiscShading,
    crossing: &RayCrossing,
    lapse: f32,
) -> Option<Vec3> {
    let g = redshift(hole, crossing, lapse)?;
    let mut sample = profile_at(hole, shading, crossing.radius);
    if crossing.radius < hole.isco {
        let fade = ((crossing.radius - hole.horizon) / (hole.isco - hole.horizon)).clamp(0.0, 1.0);
        let edge = shading.profile[1];
        sample = Vec2::new(edge.x, edge.y.max(0.05) * fade);
    }
    Some(blackbody(g * sample.x) * sample.y.max(0.0) * g.powi(4))
}

/// Light `radiance` (linear RGB, unbounded) as the picture shows it at `exposure`:
/// `1 − e^{−exposure · radiance}` per channel. Faint light is shown in proportion, bright light
/// approaches white without clipping, like film.
pub fn tone_map(radiance: Vec3, exposure: f32) -> Vec3 {
    Vec3::ONE - (-exposure * radiance.max(Vec3::ZERO)).exp()
}

/// Below this much light still let through, a ray stops: what is behind can no longer show.
pub const OPAQUE_TRANSMITTANCE: f32 = 0.02;

/// What a pixel shows, as the shader composites it: `(rgb, alpha)`, with `rgb` tone-mapped
/// ([`tone_map`] at `shading`'s exposure) and not premultiplied.
///
/// The light is followed back through every crossing of the gas, from the horizon to the outer
/// edge. `coverage` says how much of the plane the gas covers there (`0` to `1`, from the gas
/// texture at [`texture_coordinates`]); each crossing adds its [`disc_colour`] times its coverage
/// times the light still let through, and lets through `1 − coverage` of the rest. Gaps between
/// clumps so show the images behind. A ray that falls in is black and opaque. One from the sky
/// ends, with `stars`, on the [`star_field`] in the direction it came from, and is opaque too;
/// without, it keeps whatever it has not been covered by.
pub fn composite(
    hole: &RayHole,
    shading: &DiscShading,
    camera: Vec3,
    direction: Vec3,
    lapse: f32,
    coverage: impl Fn(Vec2) -> f32,
    stars: bool,
) -> Vec4 {
    let mut colour = Vec3::ZERO;
    let mut transmittance = 1.0f32;
    let trace = march_with(hole, camera, direction, |crossing| {
        if crossing.radius > hole.outer {
            return false;
        }
        let covered =
            coverage(texture_coordinates(hole, crossing.radius, crossing.azimuth)).clamp(0.0, 1.0);
        if covered > 0.0 {
            if let Some(seen) = disc_colour(hole, shading, crossing, lapse) {
                colour += transmittance * covered * seen;
            }
            transmittance *= 1.0 - covered;
        }
        transmittance < OPAQUE_TRANSMITTANCE
    });
    match trace.end {
        RayEnd::Shadow | RayEnd::Disc => transmittance = 0.0,
        RayEnd::Sky { direction } if stars => {
            colour += transmittance * star_field(direction, lapse);
            transmittance = 0.0;
        }
        _ => {}
    }
    let alpha = 1.0 - transmittance;
    if alpha <= 0.0 {
        return Vec4::ZERO;
    }
    tone_map(colour / alpha, shading.exposure).extend(alpha)
}

/// A 32-bit integer hash (Chris Wellons' `lowbias32`), the same in WGSL.
pub fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// A uniform number in `[0, 1)` from a hash.
pub fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// The star field's layers: cells per radian, the chance a cell holds a star, and how bright its
/// brightest stars are. A fine layer of many faint stars and a coarse one of fewer bright ones.
pub const STAR_LAYERS: [(f32, f32, f32); 2] = [(150.0, 0.08, 0.12), (40.0, 0.3, 0.35)];

/// A star's angular radius (Gaussian `σ`), in radians: about one pixel at the camera's field of view.
pub const STAR_SIZE_RAD: f32 = 0.0012;

/// The light of the stars seen in direction `d` (unit, Kerr–Schild axes) by a camera with
/// [`camera_lapse`] `lapse`: linear RGB.
///
/// The sky is cut into cubes `1/N` across (for each of [`STAR_LAYERS`]); each cube that the hash
/// gives a star holds one, at a random point in it projected onto the sky, kept only if it lands in
/// its own cube. So the stars are scattered evenly over the whole sky, in every direction, and
/// every star belongs to exactly one cube. A direction looks at the `2 × 2 × 2` cubes nearest it, so
/// no star is cut off at a cube's edge. Each star is a Gaussian [`STAR_SIZE_RAD`] across, a
/// blackbody between `2,500` and `12,000 K`, blueshifted by `1/lapse` (light falling down to the
/// camera), with most stars faint and a few bright.
pub fn star_field(d: Vec3, lapse: f32) -> Vec3 {
    let mut total = Vec3::ZERO;
    let blueshift = if lapse > 0.0 { 1.0 / lapse } else { 1.0 };
    for (layer, &(cells, chance, scale)) in STAR_LAYERS.iter().enumerate() {
        let base = (d * cells - 0.5).floor();
        for k in 0..8u32 {
            let cell = base + Vec3::new((k & 1) as f32, ((k >> 1) & 1) as f32, (k >> 2) as f32);
            let mut h = hash(
                (cell.x as i32 as u32)
                    ^ hash(
                        (cell.y as i32 as u32) ^ hash((cell.z as i32 as u32) ^ (layer as u32 + 1)),
                    ),
            );
            if unit(h) >= chance {
                continue;
            }
            let mut next = || {
                h = hash(h);
                unit(h)
            };
            let jitter = Vec3::new(next(), next(), next()) - 0.5;
            let star = ((cell + 0.5 + 0.8 * jitter) / cells).normalize();
            if (star * cells).floor() != cell {
                continue;
            }
            let brightness = scale * (0.05 + 0.95 * next().powi(6));
            let temperature = 2500.0 + 9500.0 * next();
            let offset = d - star;
            let spot = (-offset.dot(offset) / (STAR_SIZE_RAD * STAR_SIZE_RAD)).exp();
            total += blackbody(temperature * blueshift) * brightness * spot;
        }
    }
    total
}

#[cfg(test)]
#[path = "../../tst/render/kerr_ray_tests.rs"]
mod tests;
