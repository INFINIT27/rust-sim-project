//! Tests for [`crate::physics_utils::relativity::wormhole`], `#[path]`-included into it.
//!
//! The geometry is held to its own definitions (its slope to its radius, its lensing width to the
//! surface it describes), and the paths to what is exact: the conserved quantity, which paths cross,
//! and the Ellis wormhole's whole bend in closed form.

use super::*;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};

const ELLIS: WormholeShape = WormholeShape::Ellis { throat: 1.0 };

fn dneg() -> WormholeShape {
    WormholeShape::dneg(1.0, 0.5, 0.05).unwrap()
}

fn shapes() -> [WormholeShape; 4] {
    [
        ELLIS,
        dneg(),
        WormholeShape::dneg(1.0, 0.005, 0.05).unwrap(),
        WormholeShape::dneg(2.0, 5.0, 0.86).unwrap(),
    ]
}

/// The complete elliptic integral of the first kind, modulus `k`, by the arithmetic–geometric mean.
fn elliptic_k(k: f64) -> f64 {
    let (mut a, mut b) = (1.0, (1.0 - k * k).sqrt());
    for _ in 0..40 {
        (a, b) = (0.5 * (a + b), (a * b).sqrt());
    }
    PI / (2.0 * a)
}

/// A path coming in from `l` with impact parameter `impact`.
fn incoming(shape: &WormholeShape, l: f64, impact: f64) -> WormholePath {
    WormholePath {
        l,
        angle: 0.0,
        p: -(1.0 - (impact / shape.radius(l)).powi(2)).sqrt(),
    }
}

#[test]
fn the_lensing_width_is_the_papers_143_flares() {
    // James et al. (2015), eq. (7): 1.42953…
    let width = lensing_width_per_flare();
    assert!((width - 1.42953).abs() < 1e-5, "{width}");
}

#[test]
fn the_lensing_width_is_the_radius_gained_as_the_surface_turns_to_45_degrees() {
    let shape = dneg();
    let l = (0..2_000_000)
        .map(|i| 0.5 + f64::from(i) * 1e-6)
        .find(|&l| shape.slope(l) >= FRAC_1_SQRT_2)
        .unwrap();
    assert!((shape.radius(l) - 1.0 - 0.05).abs() < 1e-5);
}

#[test]
fn the_radius_is_at_least_the_throat_even_and_flat_far_away() {
    for shape in shapes() {
        assert_eq!(shape.radius(0.0), shape.throat());
        for i in 1..400 {
            let l = f64::from(i) * 0.05;
            assert!(shape.radius(l) >= shape.throat());
            assert_eq!(shape.radius(l), shape.radius(-l));
            assert_eq!(shape.slope(l), -shape.slope(-l));
        }
        assert!((shape.slope(1e6) - 1.0).abs() < 1e-5);
    }
}

#[test]
fn the_slope_is_the_derivative_of_the_radius() {
    for shape in shapes() {
        for i in -300..300 {
            let l = f64::from(i) * 0.037 + 0.011;
            let h = 1e-6;
            let difference = (shape.radius(l + h) - shape.radius(l - h)) / (2.0 * h);
            assert!((shape.slope(l) - difference).abs() < 1e-5, "{shape:?} {l}");
        }
    }
}

#[test]
fn a_radius_and_its_distance_are_inverses() {
    for shape in shapes() {
        for l in [0.52, 0.6, 1.0, 5.1, 7.3, 50.0, 1e4] {
            if l > shape.half_length() {
                let back = shape.distance_at_radius(shape.radius(l));
                assert!((back - l).abs() < 1e-6 * l.max(1.0), "{shape:?} {l} {back}");
            }
        }
        assert_eq!(shape.distance_at_radius(0.0), shape.half_length());
    }
}

#[test]
fn the_ellis_embedding_is_its_known_surface() {
    // r² = ρ² + ℓ² gives dz/dℓ = ρ/r, so z = ρ asinh(ℓ/ρ): the catenoid.
    for l in [0.5, 1.0, 4.0] {
        assert!((ELLIS.embedding_height(l, 2000) - f64::asinh(l)).abs() < 1e-9);
    }
    // The cylinder is vertical: its height is its length.
    assert!((dneg().embedding_height(0.5, 100) - 0.5).abs() < 1e-12);
}

#[test]
fn the_conserved_quantity_is_conserved() {
    for shape in shapes() {
        for heading in [PI, 3.1, 2.8, 2.0, FRAC_PI_2, 0.3] {
            let (path, impact) = WormholePath::start(&shape, 6.0, heading);
            let end = path.escape(&shape, impact, 100.0, 0.05, 100_000).unwrap();
            let error = end.error(&shape, impact);
            assert!(error < 1e-5, "{shape:?} {heading} {error}");
        }
    }
}

#[test]
fn a_path_crosses_exactly_when_its_impact_parameter_is_below_the_throat() {
    for shape in shapes() {
        for fraction in [0.0, 0.5, 0.9, 0.99, 0.999, 1.001, 1.01, 1.5, 4.0] {
            let impact = fraction * shape.throat();
            let end = incoming(&shape, 20.0, impact)
                .escape(&shape, impact, 40.0, 0.02, 1_000_000)
                .unwrap();
            assert_eq!(end.l < 0.0, fraction < 1.0, "{shape:?} {fraction}");
        }
    }
}

#[test]
fn a_turning_path_turns_where_the_radius_is_its_impact_parameter() {
    for shape in shapes() {
        let impact = 1.7 * shape.throat();
        let mut path = incoming(&shape, 20.0, impact);
        let mut closest = f64::INFINITY;
        while path.l < 21.0 {
            path = path.step(&shape, impact, 2e-4);
            closest = closest.min(shape.radius(path.l));
        }
        assert!((closest - impact).abs() < 1e-5, "{shape:?} {closest}");
    }
}

#[test]
fn the_ellis_bend_matches_its_closed_form() {
    // With r = ρ/sin u the swept angle is a complete elliptic integral:
    //   B > ρ (turns back): 2 K(ρ/B).      B < ρ (crosses): 2 (B/ρ) K(B/ρ).
    // Followed from and to a far |ℓ|, so what is left beyond, 2 asin(B/r), is added.
    let far = 1e5;
    for impact in [0.1, 0.5, 0.9, 0.99, 1.01, 1.2, 2.0, 10.0] {
        let end = incoming(&ELLIS, far, impact)
            .escape(&ELLIS, impact, far, 0.02, 1_000_000)
            .unwrap();
        let swept = end.angle + (impact / ELLIS.radius(far)).asin();
        let swept = swept + (impact / ELLIS.radius(end.l)).asin();
        let exact = if impact > 1.0 {
            2.0 * elliptic_k(1.0 / impact)
        } else {
            2.0 * impact * elliptic_k(impact)
        };
        assert!((swept - exact).abs() < 1e-5, "{impact}: {swept} {exact}");
    }
}

#[test]
fn far_from_the_mouth_the_paths_are_straight() {
    for shape in shapes() {
        let impact = 1000.0 * shape.throat();
        let start = incoming(&shape, 1e6, impact);
        let end = start.escape(&shape, impact, 1e6, 0.02, 100_000).unwrap();
        let turn = end.direction(&shape, impact) - start.direction(&shape, impact);
        assert!(turn.abs() < 3e-3, "{shape:?} {turn}");
    }
}

#[test]
fn paths_run_backwards_to_where_they_came_from() {
    let shape = dneg();
    for impact in [0.3, 0.8, 1.4] {
        let start = incoming(&shape, 3.0, impact);
        let there = start.advance(&shape, impact, 9.0, 0.02);
        let turned = WormholePath {
            p: -there.p,
            ..there
        };
        let back = turned.advance(&shape, -impact, 9.0, 0.02);
        assert!((back.l - start.l).abs() < 1e-5, "{impact}");
        assert!((back.angle - start.angle).abs() < 1e-5, "{impact}");
        assert!((back.p + start.p).abs() < 1e-5, "{impact}");
    }
}

#[test]
fn unusable_shapes_are_refused() {
    for (throat, half, width) in [
        (0.0, 1.0, 1.0),
        (-1.0, 1.0, 1.0),
        (1.0, -0.1, 1.0),
        (1.0, 1.0, 0.0),
        (f64::NAN, 1.0, 1.0),
        (1.0, f64::INFINITY, 1.0),
    ] {
        assert_eq!(WormholeShape::dneg(throat, half, width), None);
    }
}
