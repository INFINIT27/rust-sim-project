//! Analytic eclipse shadows: what fraction of the star a body's surface can actually see.
//!
//! When Io crosses between the Sun and Jupiter it casts a real shadow — a `3,643 km` spot on a
//! `139,822 km` planet, `2.6%` of the disc, which is the black dot in every telescope image of a
//! Galilean transit. This module is how that spot gets drawn.
//!
//! # Why not a shadow map
//!
//! Because it cannot work here, by three orders of magnitude. A Bevy point light casts shadows
//! through a cube map whose face spans `90°`, so a `1024²` map resolves `1.53e-3` radians per texel.
//! At Jupiter's `778.6`-unit distance from the Sun that texel is `1.19` world units — **`17×`
//! Jupiter's whole radius**, and `656×` Io's. At `4096²` it is still `4.3×` Jupiter's radius. To
//! resolve Io's shadow you would need about **`671,000` texels per cube face**.
//!
//! So `shadows_enabled` is off (amendment A19) and always will be: turning it on would not be a
//! compromise, it would do nothing at all.
//!
//! # What is done instead
//!
//! The shadow is *computed*, not sampled. Every body in the scene is a sphere and there is one star,
//! so for any point on any surface the question "how much of the star can this point see" has a
//! closed-form answer: the overlap area of two discs on the sky. That is exact at every scale, has no
//! resolution limit, and costs a handful of instructions per occluder.
//!
//! [`star_visible_fraction`] is that answer, in Rust, where it can be tested. The same arithmetic
//! runs per fragment in `assets/shaders/eclipse.wgsl`, and
//! `eclipse_tests::the_shader_mirrors_the_reference_thresholds` checks the shader's text for every
//! comparison made here — because two implementations of one formula is exactly the arrangement that
//! drifts.
//!
//! # The geometry
//!
//! From a point `P`, the star of radius `R` at distance `d_s` subtends an angular radius
//! `α = asin(R/d_s)`, and an occluder of radius `r` at distance `d_o` subtends `β = asin(r/d_o)`. Let
//! `θ` be the angle between the two directions. Then:
//!
//! | Condition | Meaning | Visible |
//! | --- | --- | --- |
//! | `θ ≥ α + β` | the discs miss | `1` |
//! | `θ ≤ β − α` | the occluder covers the star | `0` — umbra |
//! | `θ ≤ α − β` | the occluder sits inside the star's disc | `1 − (β/α)²` — annular |
//! | otherwise | partial overlap | `1 −` overlap area `/ πα²` |
//!
//! That last case is what gives a **penumbra**: a soft edge whose width is the star's own angular
//! size, which is the physically right answer and not a blur chosen to look nice.
//!
//! # What it costs
//!
//! This runs for every fragment against every occluder, and from almost every fragment almost every
//! occluder misses. So the miss is decided first, without trigonometry: `sin α = R/d_s` and
//! `sin β = r/d_o` are divisions, their cosines are square roots, and the discs miss iff
//! `cos θ ≤ cos α cos β − sin α sin β`. Only an occluder that survives that pays for `asin` and
//! `acos`, and the star's own terms are computed once per point rather than once per occluder.
//!
//! # What it does not model
//!
//! No refraction, so no red rim on an eclipsed moon. No limb darkening of the star, so the penumbra
//! is a shade too uniform. And an occluder is a perfect sphere, which for Jupiter — flattened by
//! `6.5%` — makes its shadow slightly the wrong shape.

use bevy::asset::Asset;
use bevy::math::{Vec3, Vec4};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

use crate::render::constants::ECLIPSE_SHADER_PATH;

/// The material every body is drawn with: Bevy's `StandardMaterial`, plus the eclipse test.
///
/// A type alias because the full name appears in every `Assets<..>` handle in the crate and in every
/// headless test that registers one.
pub type BodyMaterial = ExtendedMaterial<StandardMaterial, EclipseExtension>;

pub use uniform::EclipseUniform;

/// Wrapper module for [`EclipseUniform`], carrying one `allow` that cannot go anywhere else.
///
/// `#[derive(ShaderType)]` emits a per-field layout-assertion function that nothing calls, so the
/// fields each raise a `dead_code` warning for a `check` function this crate never wrote. The
/// attribute has to sit on the *module*, because the generated assertions are separate items and an
/// attribute on the struct does not reach them. A module is the smallest scope that does.
mod uniform {
    #![allow(dead_code)]

    use bevy::math::{UVec4, Vec4};
    use bevy::render::render_resource::ShaderType;

    use crate::render::constants::MAX_ECLIPSE_OCCLUDERS;

    /// The star and the occluders, as the shader sees them.
    ///
    /// Laid out in `vec4`s throughout, because a WGSL uniform array has a `16`-byte stride and packing
    /// three floats next to a count is the kind of thing that silently works on one driver.
    #[derive(Clone, Copy, Debug, ShaderType)]
    pub struct EclipseUniform {
        /// `xyz` — the star's position in world space. `w` — its radius, in world units.
        ///
        /// A `w` of `0.0` means **there is no star in this scene**, and the shader returns full
        /// visibility without looking at the occluders. That is the case for four of the five
        /// experiments, which are lit by a directional light instead.
        pub star: Vec4,
        /// `x` — how many entries of [`spheres`](Self::spheres) are live. The rest is padding, and it is
        /// padding rather than three more useful numbers because a `u32` beside a `vec4` array is an
        /// alignment trap.
        pub count: UVec4,
        /// `xyz` — an occluder's centre. `w` — its radius, in world units.
        ///
        /// Fixed length because a uniform cannot be dynamically sized; entries past `count.x` are
        /// ignored. See [`MAX_ECLIPSE_OCCLUDERS`] for what happens when a scene has more bodies than
        /// this.
        pub spheres: [Vec4; MAX_ECLIPSE_OCCLUDERS],
    }

    impl Default for EclipseUniform {
        /// No star and no occluders: every surface sees its light unobstructed.
        ///
        /// The right default for the four experiments with no star in them, and for the first frame of
        /// the one that has.
        fn default() -> Self {
            Self {
                star: Vec4::ZERO,
                count: UVec4::ZERO,
                spheres: [Vec4::ZERO; MAX_ECLIPSE_OCCLUDERS],
            }
        }
    }
}

/// The material extension that carries [`EclipseUniform`] to the fragment shader.
///
/// Bound at `@binding(100)`: Bevy reserves `0`–`99` of group `2` for `StandardMaterial`'s own
/// bindings, and an extension starts above them.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct EclipseExtension {
    /// The star and the occluders. Rewritten every frame by
    /// [`update_eclipse_occluders_system`](crate::render::plugin::update_eclipse_occluders_system).
    #[uniform(100)]
    pub uniform: EclipseUniform,
}

impl MaterialExtension for EclipseExtension {
    fn fragment_shader() -> ShaderRef {
        ECLIPSE_SHADER_PATH.into()
    }

    /// The same shader for the deferred path, so the material behaves identically whichever pipeline
    /// a platform picks.
    fn deferred_fragment_shader() -> ShaderRef {
        ECLIPSE_SHADER_PATH.into()
    }
}

/// What fraction of a star at `star_centre` (radius `star_radius`) is visible from `point`, given one
/// occluding sphere at `occluder_centre` with radius `occluder_radius`.
///
/// `1.0` is an unobstructed view and `0.0` is total eclipse. The **reference implementation**: the
/// shader mirrors this, and a test pins a table of values both must agree on.
///
/// # The cases, and why the third one exists
///
/// See the module docs for the table. The annular case — the occluder entirely inside the star's disc
/// — is easy to forget and is not rare: it is every transit of a small body across a large star, and
/// omitting it would make Mercury crossing the Sun produce *total* darkness rather than a `0.001%`
/// dimming.
///
/// # Returns
///
/// Always in `[0, 1]`, and always `1.0` when there is nothing to compute: a non-positive star radius
/// (meaning "no star"), a non-positive occluder radius, a non-finite input, an occluder at or beyond
/// the star's distance, or a point inside the occluder. That last one matters — a body's own surface
/// is inside its own sphere, so the caller must skip a body against itself, and this makes the
/// failure benign rather than black.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{star_visible_fraction, Vec3};
///
/// let star = Vec3::new(0.0, 0.0, 100.0);
///
/// // Nothing in the way.
/// let clear = star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(50.0, 50.0, 0.0), 1.0);
/// assert_eq!(clear, 1.0);
///
/// // A big occluder squarely in the way, close by: total eclipse.
/// let dark = star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(0.0, 0.0, 10.0), 1.0);
/// assert_eq!(dark, 0.0);
///
/// // Grazing the edge: partway between, which is the penumbra.
/// let edge = star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(0.1, 0.0, 10.0), 0.1);
/// assert!(edge > 0.0 && edge < 1.0, "got {edge}");
/// ```
pub fn star_visible_fraction(
    point: Vec3,
    star_centre: Vec3,
    star_radius: f32,
    occluder_centre: Vec3,
    occluder_radius: f32,
) -> f32 {
    StarView::new(point, star_centre, star_radius).map_or(1.0, |star| {
        star.visible_past(point, occluder_centre, occluder_radius)
    })
}

/// The visible fraction of the star at `point`, whose surface faces along `normal`, against every
/// occluder in `spheres`.
///
/// The product of [`star_visible_fraction`] over each occluder, which is what the shader computes.
/// Multiplying is an approximation — two occluders covering the *same* part of the star's disc would
/// be double-counted — and it is the right one here: two bodies overlapping on the sky as seen from a
/// third is a syzygy, and the error is zero unless one occurs.
///
/// A surface facing away from the star returns `1.0`. Its own body blocks the star there, and that
/// body is not in its own occluder list, so without this test an occluder's umbra would pass through
/// the planet and darken its ambient-lit night side. There is no direct light there to shadow.
pub fn star_visible_fraction_all(
    point: Vec3,
    normal: Vec3,
    star_centre: Vec3,
    star_radius: f32,
    spheres: &[Vec4],
) -> f32 {
    if normal.dot(star_centre - point) <= 0.0 {
        return 1.0;
    }
    let Some(star) = StarView::new(point, star_centre, star_radius) else {
        return 1.0;
    };
    spheres
        .iter()
        .map(|sphere| star.visible_past(point, sphere.truncate(), sphere.w))
        .product::<f32>()
        .clamp(0.0, 1.0)
}

/// How far below `cos(α + β)` the separation's cosine must fall before the discs are taken to miss
/// without computing any angle.
///
/// The cosine test and the angle test agree exactly in real arithmetic, but `cos(α + β)` assembled
/// from sines and square roots rounds differently from `α + β` assembled from `asin`, by a few ulps
/// of `1`. Rejecting only below this margin keeps the shortcut strictly inside the region the angle
/// test also calls a miss, so every overlapping case still reaches the original arithmetic. The cost
/// is a thin ring around each shadow that takes the slow path for nothing.
const MISS_COSINE_MARGIN: f32 = 1.0e-6;

/// The star as seen from one point: everything that does not depend on the occluder. The shader's
/// `StarView` is the same thing, built once per fragment rather than once per occluder.
struct StarView {
    to_star: Vec3,
    distance: f32,
    /// `sin α`, of the star's angular radius `α`.
    sine: f32,
    cosine: f32,
    angle: f32,
}

impl StarView {
    /// `None` when nothing can shade `point`: a non-finite input, no star, or a point at the star's
    /// centre.
    fn new(point: Vec3, star_centre: Vec3, star_radius: f32) -> Option<Self> {
        if !point.is_finite()
            || !star_centre.is_finite()
            || !star_radius.is_finite()
            || star_radius <= 0.0
        {
            return None;
        }

        let to_star = star_centre - point;
        let distance = to_star.length();
        if distance <= 0.0 {
            return None;
        }

        let sine = (star_radius / distance).clamp(-1.0, 1.0);
        let angle = sine.asin();
        if angle <= 0.0 {
            return None;
        }

        Some(Self {
            to_star,
            distance,
            sine,
            cosine: cosine_from_sine(sine),
            angle,
        })
    }

    /// [`star_visible_fraction`] for one occluder, from the point this view was built at.
    fn visible_past(&self, point: Vec3, occluder_centre: Vec3, occluder_radius: f32) -> f32 {
        if !occluder_centre.is_finite() || !occluder_radius.is_finite() || occluder_radius <= 0.0 {
            return 1.0;
        }

        let to_occluder = occluder_centre - point;
        let occluder_distance = to_occluder.length();

        // At the surface, inside it, or further away than the star itself: nothing to occlude. The
        // `<=` on the radius is what makes a body's own sphere harmless rather than opaque.
        if occluder_distance <= occluder_radius || occluder_distance >= self.distance {
            return 1.0;
        }

        let occluder_sine = (occluder_radius / occluder_distance).clamp(-1.0, 1.0);
        let occluder_cosine = cosine_from_sine(occluder_sine);
        let cosine =
            (self.to_star.dot(to_occluder) / (self.distance * occluder_distance)).clamp(-1.0, 1.0);

        // The discs miss, decided without trigonometry: `θ ≥ α + β` iff `cos θ ≤ cos(α + β)`, which
        // holds because `α` and `β` are each at most `π/2`, so their sum is in `[0, π]` where cosine
        // is monotone. From nearly every point, nearly every occluder leaves here.
        if cosine <= self.cosine * occluder_cosine - self.sine * occluder_sine - MISS_COSINE_MARGIN
        {
            return 1.0;
        }

        let star_angle = self.angle;
        let occluder_angle = occluder_sine.asin();
        let separation = cosine.acos();

        // The discs miss each other entirely.
        if separation >= star_angle + occluder_angle {
            return 1.0;
        }
        // The occluder's disc swallows the star's: umbra.
        if separation <= occluder_angle - star_angle {
            return 0.0;
        }
        // The occluder's disc sits wholly inside the star's: annular, and the shortfall is the ratio
        // of areas.
        if separation <= star_angle - occluder_angle {
            let ratio = occluder_angle / star_angle;
            return (1.0 - ratio * ratio).clamp(0.0, 1.0);
        }

        // Partial overlap: the area shared by two circles of radii `a` and `b` whose centres are `d`
        // apart, over the star's own area. This is the term that produces a penumbra whose width is
        // the star's angular size — the physically correct softness rather than a chosen blur.
        let (a, b, d) = (star_angle, occluder_angle, separation);
        let first = ((d * d + a * a - b * b) / (2.0 * d * a))
            .clamp(-1.0, 1.0)
            .acos();
        let second = ((d * d + b * b - a * a) / (2.0 * d * b))
            .clamp(-1.0, 1.0)
            .acos();
        let triangle = ((-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b))
            .max(0.0)
            .sqrt();

        let overlap = a * a * first + b * b * second - 0.5 * triangle;
        let covered = overlap / (std::f32::consts::PI * a * a);

        (1.0 - covered).clamp(0.0, 1.0)
    }
}

/// `cos x` from `sin x`, for `x` in `[0, π/2]`.
///
/// `(1 - s)(1 + s)` rather than `1 - s²`: the subtraction is exact when `s` is near `1`, where
/// `1 - s²` cancels to noise and the cosine would carry none of its digits.
fn cosine_from_sine(sine: f32) -> f32 {
    ((1.0 - sine) * (1.0 + sine)).sqrt()
}

#[cfg(test)]
#[path = "../../tst/render/eclipse_tests.rs"]
mod tests;
