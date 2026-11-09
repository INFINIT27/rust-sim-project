//! Unit tests for the [`crate::render::eclipse`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/render/eclipse.rs` via `#[path]`, not as
//! a standalone integration test.
//!
//! # What can and cannot be tested here
//!
//! The **geometry** is pure Rust and is tested exhaustively below, including against three real
//! configurations whose answers are known independently: Io's shadow on Jupiter, Jupiter's shadow on
//! Io, and Mercury transiting the Sun as seen from Earth — the last of which dims the Sun by `0.003%`,
//! a figure from the transit literature rather than from this code.
//!
//! The **shader** cannot be executed: there is no GPU in a test run, and no WGSL interpreter here. So
//! `the_shader_mirrors_the_reference_thresholds` reads `assets/shaders/eclipse.wgsl` as text and
//! checks that its four case boundaries and its two shared constants are the ones this module uses.
//! That is a guard against the failure that would actually happen — a threshold edited on one side
//! only — and it is not a guard against the shader being wrong in some way the text does not show.
//! **Verifying the pixels is a human job**, and `src/render/README.md` says where to look.

use std::f64::consts::{PI, TAU};

use bevy::math::DVec3;

use super::*;
use crate::render::constants::MAX_ECLIPSE_OCCLUDERS;

/// The shader source, read at compile time so a missing file is a build failure rather than a
/// mysteriously passing test.
const SHADER: &str = include_str!("../../assets/shaders/eclipse.wgsl");

/// A star at the origin with the Sun's radius, in world units.
const STAR_RADIUS: f32 = 0.6957;

// -----------------------------------------------------------------------
// The four cases.
// -----------------------------------------------------------------------

#[test]
fn an_unobstructed_view_is_fully_visible() {
    let star = Vec3::new(0.0, 0.0, 100.0);

    // Occluder off to one side.
    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(50.0, 50.0, 50.0), 1.0),
        1.0
    );
    // Occluder behind the viewer.
    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(0.0, 0.0, -10.0), 1.0),
        1.0
    );
    // Occluder further away than the star.
    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(0.0, 0.0, 200.0), 5.0),
        1.0
    );
}

#[test]
fn a_large_close_occluder_gives_total_eclipse() {
    // The umbra case: the occluder's disc swallows the star's.
    let star = Vec3::new(0.0, 0.0, 100.0);
    let visible = star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(0.0, 0.0, 10.0), 1.0);

    assert_eq!(visible, 0.0);
}

#[test]
fn a_small_occluder_inside_the_disc_is_annular() {
    // The case that is easy to forget: the occluder sits wholly within the star's disc, so the
    // shortfall is the ratio of areas and nothing like darkness. Omitting it would turn every transit
    // of a small body into a total eclipse.
    let star = Vec3::new(0.0, 0.0, 100.0);

    // Angular radii: star 1/100 = 0.01, occluder 0.01/10 = 0.001. Ratio 0.1, so 1% covered.
    let visible = star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(0.0, 0.0, 10.0), 0.01);

    assert!(
        (visible - 0.99).abs() < 1.0e-3,
        "expected ~0.99 visible, got {visible}"
    );
}

#[test]
fn the_penumbra_is_smooth_and_monotone() {
    // Sliding an occluder across the star must move the visible fraction monotonically from 0 to 1
    // with no jumps — that continuity *is* the soft shadow edge, and its width is the star's own
    // angular size rather than a blur anybody chose.
    let star = Vec3::new(0.0, 0.0, 100.0);
    let mut previous = 0.0_f32;
    let mut saw_partial = false;

    for step in 0..=200 {
        let offset = step as f32 * 0.002;
        let visible =
            star_visible_fraction(Vec3::ZERO, star, 1.0, Vec3::new(offset, 0.0, 10.0), 0.2);

        assert!((0.0..=1.0).contains(&visible), "out of range at {offset}");
        assert!(
            visible >= previous - 1.0e-6,
            "visibility fell from {previous} to {visible} at offset {offset}"
        );
        if visible > 0.0 && visible < 1.0 {
            saw_partial = true;
        }
        previous = visible;
    }

    assert!(saw_partial, "the sweep never passed through a penumbra");
    assert_eq!(previous, 1.0, "and it must end fully clear");
}

// -----------------------------------------------------------------------
// Three real configurations, against independently known answers.
// -----------------------------------------------------------------------

#[test]
fn io_casts_a_total_shadow_on_jupiter() {
    // The user's case, and the black dot in every telescope image of a Galilean transit. Io's disc
    // seen from Jupiter is 4.83x the Sun's, so it is a genuine umbra rather than a partial dimming.
    let sun = Vec3::ZERO;
    let jupiter = Vec3::new(778.57, 0.0, 0.0);
    let io = Vec3::new(778.57 - 0.4217, 0.0, 0.0);
    let jupiter_radius = 0.069911;
    let io_radius = 0.0018216;

    // The sub-solar point on Jupiter, directly behind Io.
    let under_io = jupiter - Vec3::new(jupiter_radius, 0.0, 0.0);
    assert_eq!(
        star_visible_fraction(under_io, sun, STAR_RADIUS, io, io_radius),
        0.0,
        "directly behind Io it must be total"
    );

    // And 0.01 units to the side — 14% of Jupiter's radius — full daylight, because the umbra is
    // only 2.6% of the planet's diameter.
    let beside_io = under_io + Vec3::new(0.0, 0.01, 0.0);
    assert_eq!(
        star_visible_fraction(beside_io, sun, STAR_RADIUS, io, io_radius),
        1.0,
        "the shadow is a small spot, not a hemisphere"
    );
}

#[test]
fn io_does_not_shadow_jupiters_night_side() {
    // The antisolar point lies on the same Sun-Io line as the sub-solar point, but Jupiter itself is
    // in the way. Jupiter is not in its own occluder list, so without a facing test the umbra came
    // straight through the planet and darkened the ambient-lit night side.
    let sun = Vec3::ZERO;
    let jupiter = Vec3::new(778.57, 0.0, 0.0);
    let io = Vec3::new(778.57 - 0.4217, 0.0, 0.0);
    let jupiter_radius = 0.069911;
    let io_radius = 0.0018216;

    let occluders = [io.extend(io_radius)];

    let day_side = jupiter - Vec3::new(jupiter_radius, 0.0, 0.0);
    assert_eq!(
        star_visible_fraction_all(day_side, -Vec3::X, sun, STAR_RADIUS, &occluders),
        0.0,
        "the real shadow on the day side must survive the facing test"
    );

    let night_side = jupiter + Vec3::new(jupiter_radius, 0.0, 0.0);
    assert_eq!(
        star_visible_fraction_all(night_side, Vec3::X, sun, STAR_RADIUS, &occluders),
        1.0,
        "Io's shadow must stop at Jupiter's day side"
    );
}

#[test]
fn jupiter_totally_eclipses_io() {
    // The reverse case, and the more dramatic one: Jupiter's disc from Io is 186x the Sun's, so the
    // whole moon goes dark.
    let sun = Vec3::ZERO;
    let jupiter = Vec3::new(778.57, 0.0, 0.0);
    let io = Vec3::new(778.57 + 0.4217, 0.0, 0.0);
    let io_radius = 0.0018216;

    let sunward = io - Vec3::new(io_radius, 0.0, 0.0);

    assert_eq!(
        star_visible_fraction(sunward, sun, STAR_RADIUS, jupiter, 0.069911),
        0.0
    );
}

#[test]
fn mercury_transiting_the_sun_dims_it_by_three_thousandths_of_a_percent() {
    // The annular case at real scale, and the number is from the transit literature, not from this
    // code: a Mercury transit dims the Sun by about 0.003%. If the annular branch were missing this
    // would read 0.0 — total darkness at noon — which is why the case exists.
    let sun = Vec3::ZERO;
    let earth = Vec3::new(149.60, 0.0, 0.0);
    let mercury = Vec3::new(57.909, 0.0, 0.0);

    let sunward = earth - Vec3::new(0.006371, 0.0, 0.0);
    let visible = star_visible_fraction(sunward, sun, STAR_RADIUS, mercury, 0.0024397);

    let dimming_percent = (1.0 - visible) * 100.0;
    assert!(
        (dimming_percent - 0.0033).abs() < 0.001,
        "Mercury should dim the Sun by ~0.003%, got {dimming_percent}%"
    );
}

// -----------------------------------------------------------------------
// Degenerate inputs, and the many-occluder product.
// -----------------------------------------------------------------------

#[test]
fn nonsense_inputs_are_fully_visible_rather_than_black() {
    // Benign in the direction that fails *open*. A shadow bug that darkens is far harder to notice
    // than one that does not, and a `NaN` multiplied into a colour propagates.
    let star = Vec3::new(0.0, 0.0, 100.0);
    let occluder = Vec3::new(0.0, 0.0, 10.0);

    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, 0.0, occluder, 1.0),
        1.0
    );
    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, -1.0, occluder, 1.0),
        1.0
    );
    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, 1.0, occluder, 0.0),
        1.0
    );
    assert_eq!(
        star_visible_fraction(Vec3::splat(f32::NAN), star, 1.0, occluder, 1.0),
        1.0
    );
    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, f32::NAN, occluder, 1.0),
        1.0
    );
}

#[test]
fn a_point_inside_an_occluder_is_not_shadowed_by_it() {
    // A body's surface sits exactly at its own radius, so this is the case that stops a sphere
    // shadowing itself when the caller forgets to exclude it. The update system *does* exclude it —
    // this is the second line of defence, and it fails open.
    let star = Vec3::new(0.0, 0.0, 100.0);
    let occluder = Vec3::new(0.0, 0.0, 1.0);

    assert_eq!(
        star_visible_fraction(Vec3::ZERO, star, 1.0, occluder, 2.0),
        1.0
    );
}

#[test]
fn many_occluders_multiply() {
    // Two occluders on opposite sides, neither covering the star: still fully visible.
    let star = Vec3::new(0.0, 0.0, 100.0);
    let clear = [
        Vec3::new(50.0, 0.0, 10.0).extend(0.1),
        Vec3::new(-50.0, 0.0, 10.0).extend(0.1),
    ];
    assert_eq!(
        star_visible_fraction_all(Vec3::ZERO, Vec3::Z, star, 1.0, &clear),
        1.0
    );

    // One of them squarely in the way: total, regardless of the other.
    let blocked = [
        Vec3::new(50.0, 0.0, 10.0).extend(0.1),
        Vec3::new(0.0, 0.0, 10.0).extend(1.0),
    ];
    assert_eq!(
        star_visible_fraction_all(Vec3::ZERO, Vec3::Z, star, 1.0, &blocked),
        0.0
    );

    // An empty list is an unobstructed sky.
    assert_eq!(
        star_visible_fraction_all(Vec3::ZERO, Vec3::Z, star, 1.0, &[]),
        1.0
    );
}

// -----------------------------------------------------------------------
// The Rust and the shader must not drift apart.
// -----------------------------------------------------------------------

#[test]
fn the_shader_mirrors_the_reference_thresholds() {
    // The shader cannot be executed here, so this checks the text for the things that would actually
    // diverge: the four case boundaries and the two shared constants. It is a real guard against a
    // threshold edited on one side only, and it is *not* a guard against the shader being wrong in
    // some way the source text does not reveal. Verifying pixels is a human job.
    let required = [
        // The four cases, in the same order and with the same comparisons.
        "separation >= star_angle + occluder_angle",
        "separation <= occluder_angle - star_angle",
        "separation <= star_angle - occluder_angle",
        // The trig-free miss test that runs before them, with the same margin and the same
        // cancellation-free cosine.
        "cosine <= star.cosine * occluder_cosine - star.sine * occluder_sine - MISS_COSINE_MARGIN",
        "sqrt((1.0 - sine) * (1.0 + sine))",
        // The distance guards that stop self-shadowing and back-projection.
        "occluder_distance <= occluder_radius",
        "occluder_distance >= star.distance",
        // The facing test that stops an umbra passing through the receiver to its night side.
        "let to_star = eclipse.star.xyz - point;",
        "dot(normal, to_star) <= 0.0",
        // The annular term and the overlap-area term.
        "1.0 - ratio * ratio",
        "a * a * first + b * b * second - 0.5 * triangle",
        // "no star in this scene" is signalled by a zero radius, on both sides.
        "eclipse.star.w <= 0.0",
    ];

    for fragment in required {
        assert!(
            SHADER.contains(fragment),
            "the shader no longer contains `{fragment}` — has the Rust reference moved without it?"
        );
    }

    assert_eq!(MISS_COSINE_MARGIN, 1.0e-6);
    assert!(
        SHADER.contains("const MISS_COSINE_MARGIN: f32 = 1.0e-6;"),
        "the shader's miss margin must match MISS_COSINE_MARGIN"
    );
}

// -----------------------------------------------------------------------
// The trig-free miss test changes no answer.
// -----------------------------------------------------------------------

/// `star_visible_fraction` as it was before the cosine-space miss test and the hoisted star terms:
/// every occluder pays for `asin` and `acos`. The optimised path must reproduce it exactly.
fn star_visible_fraction_oracle(
    point: Vec3,
    star_centre: Vec3,
    star_radius: f32,
    occluder_centre: Vec3,
    occluder_radius: f32,
) -> f32 {
    if !point.is_finite()
        || !star_centre.is_finite()
        || !occluder_centre.is_finite()
        || !star_radius.is_finite()
        || !occluder_radius.is_finite()
        || star_radius <= 0.0
        || occluder_radius <= 0.0
    {
        return 1.0;
    }

    let to_star = star_centre - point;
    let to_occluder = occluder_centre - point;
    let star_distance = to_star.length();
    let occluder_distance = to_occluder.length();

    if star_distance <= 0.0
        || occluder_distance <= occluder_radius
        || occluder_distance >= star_distance
    {
        return 1.0;
    }

    let star_angle = (star_radius / star_distance).clamp(-1.0, 1.0).asin();
    let occluder_angle = (occluder_radius / occluder_distance)
        .clamp(-1.0, 1.0)
        .asin();

    if star_angle <= 0.0 {
        return 1.0;
    }

    let cosine = (to_star.dot(to_occluder) / (star_distance * occluder_distance)).clamp(-1.0, 1.0);
    let separation = cosine.acos();

    if separation >= star_angle + occluder_angle {
        return 1.0;
    }
    if separation <= occluder_angle - star_angle {
        return 0.0;
    }
    if separation <= star_angle - occluder_angle {
        let ratio = occluder_angle / star_angle;
        return (1.0 - ratio * ratio).clamp(0.0, 1.0);
    }

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

/// SplitMix64, so the sample is the same on every run and every machine.
struct Seeded(u64);

impl Seeded {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    /// Log-uniform in `[low, high)`, so tiny and large angles are sampled alike.
    fn log_range(&mut self, low: f64, high: f64) -> f64 {
        (low.ln() + (high.ln() - low.ln()) * self.unit()).exp()
    }

    fn direction(&mut self) -> DVec3 {
        let z = self.range(-1.0, 1.0);
        let phi = self.range(0.0, TAU);
        let r = (1.0 - z * z).sqrt();
        DVec3::new(r * phi.cos(), r * phi.sin(), z)
    }
}

/// A point and the star it sees, built in `f64` so that each occluder's true separation `θ` from the
/// star can be placed on purpose.
struct Sky {
    point: DVec3,
    towards_star: DVec3,
    star_distance: f64,
    star_angle: f64,
}

impl Sky {
    fn sample(rng: &mut Seeded) -> Self {
        Self {
            // Up to the solar system's scale, so the subtractions round as they do in experiment_3.
            point: rng.direction() * rng.range(0.0, 1000.0),
            towards_star: rng.direction(),
            star_distance: rng.log_range(1.0e-2, 1.0e3),
            star_angle: rng.log_range(1.0e-5, 1.4),
        }
    }

    fn star(&self) -> Vec3 {
        (self.point + self.towards_star * self.star_distance).as_vec3()
    }

    fn star_radius(&self) -> f32 {
        (self.star_distance * self.star_angle.sin()) as f32
    }

    /// An occluder nearer than the star whose separation from it is anywhere, or just either side
    /// of a case boundary. Half land on the miss boundary `θ = α + β`, the only one the shortcut
    /// moves.
    fn occluder(&self, rng: &mut Seeded) -> Vec4 {
        let distance = self.star_distance * rng.log_range(1.0e-4, 1.0);
        let angle = rng.log_range(1.0e-5, 1.4);

        let boundary = match rng.next_u64() % 4 {
            0 => None,
            1 => Some((angle - self.star_angle).abs()),
            _ => Some(angle + self.star_angle),
        };
        let separation = match boundary {
            None => rng.range(0.0, PI),
            Some(edge) => {
                let sign = if rng.next_u64().is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                (edge * (1.0 + sign * rng.log_range(1.0e-8, 1.0e-1))).clamp(0.0, PI)
            }
        };

        let across = self.towards_star.any_orthonormal_vector();
        let spin = rng.range(0.0, TAU);
        let across = across * spin.cos() + self.towards_star.cross(across) * spin.sin();
        let towards = self.towards_star * separation.cos() + across * separation.sin();

        (self.point + towards * distance)
            .as_vec3()
            .extend((distance * angle.sin()) as f32)
    }
}

#[test]
fn the_trig_free_miss_test_reproduces_the_original_formula_exactly() {
    // Every result must be bit-identical to the original, not merely close: the shortcut may only
    // skip work, never change an answer.
    const SAMPLES: usize = 2_000_000;
    let mut rng = Seeded(0x0EC1_1F5E);
    let (mut clear, mut dark, mut partial, mut barely_shaded) = (0, 0, 0, 0);

    for _ in 0..SAMPLES {
        let sky = Sky::sample(&mut rng);
        let occluder = sky.occluder(&mut rng);
        let (point, star, star_radius) = (sky.point.as_vec3(), sky.star(), sky.star_radius());

        let expected =
            star_visible_fraction_oracle(point, star, star_radius, occluder.truncate(), occluder.w);
        let actual =
            star_visible_fraction(point, star, star_radius, occluder.truncate(), occluder.w);

        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "point {point:?}, star {star:?} r {star_radius}, occluder {occluder:?}: got {actual}, \
             the original gives {expected}",
        );

        if expected == 1.0 {
            clear += 1;
        } else if expected == 0.0 {
            dark += 1;
        } else if expected > 0.999_99 {
            barely_shaded += 1;
        } else {
            partial += 1;
        }
    }

    // The sample must reach every outcome, including the faintest edge of the penumbra, which is
    // where a shortcut that rejected too eagerly would first show.
    for (name, count) in [
        ("clear", clear),
        ("dark", dark),
        ("partial", partial),
        ("barely shaded", barely_shaded),
    ] {
        assert!(count > SAMPLES / 1000, "only {count} {name} samples");
    }
}

#[test]
fn many_occluders_reproduce_the_original_product_exactly() {
    const SAMPLES: usize = 200_000;
    let mut rng = Seeded(0x50_1A25);

    for _ in 0..SAMPLES {
        let sky = Sky::sample(&mut rng);
        let spheres: Vec<Vec4> = (0..MAX_ECLIPSE_OCCLUDERS)
            .map(|_| sky.occluder(&mut rng))
            .collect();
        let normal = rng.direction().as_vec3();
        let (point, star, star_radius) = (sky.point.as_vec3(), sky.star(), sky.star_radius());

        let expected = if normal.dot(star - point) <= 0.0 {
            1.0
        } else {
            spheres
                .iter()
                .map(|s| star_visible_fraction_oracle(point, star, star_radius, s.truncate(), s.w))
                .product::<f32>()
                .clamp(0.0, 1.0)
        };
        let actual = star_visible_fraction_all(point, normal, star, star_radius, &spheres);

        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "occluders {spheres:?}"
        );
    }
}

#[test]
fn the_shader_and_the_constant_agree_on_the_occluder_limit() {
    // WGSL cannot import a Rust constant, so the array length is written twice. A mismatch would
    // either truncate silently or overflow the uniform.
    assert_eq!(MAX_ECLIPSE_OCCLUDERS, 16);
    assert!(
        SHADER.contains("const MAX_OCCLUDERS: u32 = 16u;"),
        "the shader's occluder limit must match MAX_ECLIPSE_OCCLUDERS"
    );
    assert!(
        SHADER.contains("array<vec4<f32>, 16>"),
        "and so must the array it indexes"
    );
}

#[test]
fn the_shader_binds_above_the_standard_material() {
    // Bevy reserves bindings 0-99 of group 2 for `StandardMaterial`; an extension starts at 100. A
    // collision here is a runtime pipeline error with a message that does not mention this file.
    assert!(SHADER.contains("@group(2) @binding(100)"));
}

#[test]
fn the_uniform_is_defaulted_to_no_star() {
    // The first frame draws before `update_eclipse_occluders_system` has run, and a scene with no star
    // never gets an update at all. Both must look like plain PBR rather than like darkness.
    let uniform = EclipseUniform::default();

    assert_eq!(uniform.star.w, 0.0, "a zero radius means: no star");
    assert_eq!(uniform.count.x, 0);
    assert_eq!(uniform.spheres.len(), MAX_ECLIPSE_OCCLUDERS);
}
