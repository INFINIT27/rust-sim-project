//! Tests for [`crate::physics_utils::thermal`], `#[path]`-included into it.

use super::*;
use crate::physics_utils::physics_constants::{
    BASALT_TILLOTSON_A, BASALT_TILLOTSON_B, BASALT_TILLOTSON_E0_J_KG,
};

fn basalt(density: f64, energy: f64) -> f64 {
    tillotson_thermal_pressure(
        density,
        energy,
        BASALT_TILLOTSON_A,
        BASALT_TILLOTSON_B,
        BASALT_TILLOTSON_E0_J_KG,
    )
}

#[test]
fn cool_rock_has_the_solid_s_gruneisen_parameter_and_hot_rock_the_gas_s() {
    let gamma = |energy: f64| basalt(1000.0, energy) / (1000.0 * energy);
    assert!((gamma(1.0) - 2.0).abs() < 1e-6, "cool: {}", gamma(1.0));
    assert!((gamma(1.0e13) - 0.5).abs() < 1e-3, "hot: {}", gamma(1.0e13));
    // Monotonic in between.
    let mut previous = f64::INFINITY;
    for exponent in 4..12 {
        let g = gamma(10f64.powi(exponent));
        assert!(g <= previous);
        previous = g;
    }
}

#[test]
fn heat_never_pulls() {
    assert_eq!(basalt(2700.0, -1.0e6), 0.0);
    assert_eq!(basalt(0.0, 1.0e6), 0.0);
    assert_eq!(basalt(-1.0, 1.0e6), 0.0);
    assert!(basalt(2700.0, 1.0e6) > 0.0);
}

#[test]
fn the_kernel_integrates_to_one() {
    let h = 0.7;
    let steps = 20_000;
    let dr = 2.0 * h / steps as f64;
    let integral: f64 = (0..steps)
        .map(|i| {
            let r = (i as f64 + 0.5) * dr;
            4.0 * PI * r * r * cubic_spline_kernel(r, h) * dr
        })
        .sum();
    assert!((integral - 1.0).abs() < 1e-6, "{integral}");
}

#[test]
fn the_gradient_is_the_kernel_s_slope() {
    let h = 1.3;
    for i in 1..40 {
        let r = i as f64 * 0.065;
        let e = 1e-6;
        let slope = (cubic_spline_kernel(r + e, h) - cubic_spline_kernel(r - e, h)) / (2.0 * e);
        let gradient = cubic_spline_gradient(r, h);
        assert!(
            (slope - gradient).abs() < 1e-6,
            "r = {r}: {slope} vs {gradient}"
        );
        assert!(gradient <= 0.0);
    }
}

#[test]
fn the_kernel_is_continuous_at_its_seams() {
    let h = 1.0;
    for seam in [1.0, 2.0] {
        let below = cubic_spline_kernel(seam - 1e-9, h);
        let above = cubic_spline_kernel(seam + 1e-9, h);
        assert!((below - above).abs() < 1e-8);
    }
}

#[test]
fn a_planet_s_particles_have_its_mean_density() {
    // 1000 particles of radius r packed in a sphere of radius R have total cell volume
    // (4/3 π R³), so each particle's density is the planet's.
    let planet_radius: f64 = 6.371e6;
    let planet_mass = 5.972e24;
    let count = 1000.0;
    let mean_density = planet_mass / (4.0 / 3.0 * PI * planet_radius.powi(3));
    let cell_volume = 4.0 / 3.0 * PI * planet_radius.powi(3) / count;
    let particle_radius = (cell_volume
        * crate::physics_utils::physics_constants::FCC_PACKING_FRACTION
        / (4.0 / 3.0 * PI))
        .cbrt();
    let density = particle_density(planet_mass / count, particle_radius);
    assert!((density / mean_density - 1.0).abs() < 1e-9);
}

#[test]
fn a_sphere_s_moment_of_inertia() {
    assert_eq!(sphere_moment_of_inertia(10.0, 1.0), 4.0);
    assert_eq!(sphere_moment_of_inertia(0.0, 1.0), 0.0);
}
