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

// ---------------------------------------------------------------------------------------------
// The whole Tillotson equation of state.
// ---------------------------------------------------------------------------------------------

use crate::physics_utils::physics_constants::{
    BASALT_TILLOTSON_ALPHA, BASALT_TILLOTSON_BETA, BASALT_TILLOTSON_BULK_A_PA,
    BASALT_TILLOTSON_BULK_B_PA, BASALT_TILLOTSON_ECV_J_KG, BASALT_TILLOTSON_EIV_J_KG,
    BASALT_TILLOTSON_RHO0_KG_M3, IRON_TILLOTSON_A, IRON_TILLOTSON_ALPHA, IRON_TILLOTSON_B,
    IRON_TILLOTSON_BETA, IRON_TILLOTSON_BULK_A_PA, IRON_TILLOTSON_BULK_B_PA,
    IRON_TILLOTSON_E0_J_KG, IRON_TILLOTSON_ECV_J_KG, IRON_TILLOTSON_EIV_J_KG,
    IRON_TILLOTSON_RHO0_KG_M3,
};

fn basalt_eos() -> TillotsonParameters {
    TillotsonParameters {
        reference_density_kg_m3: BASALT_TILLOTSON_RHO0_KG_M3,
        bulk_a_pa: BASALT_TILLOTSON_BULK_A_PA,
        bulk_b_pa: BASALT_TILLOTSON_BULK_B_PA,
        a: BASALT_TILLOTSON_A,
        b: BASALT_TILLOTSON_B,
        reference_energy_j_kg: BASALT_TILLOTSON_E0_J_KG,
        incipient_vaporisation_j_kg: BASALT_TILLOTSON_EIV_J_KG,
        complete_vaporisation_j_kg: BASALT_TILLOTSON_ECV_J_KG,
        alpha: BASALT_TILLOTSON_ALPHA,
        beta: BASALT_TILLOTSON_BETA,
    }
}

fn iron_eos() -> TillotsonParameters {
    TillotsonParameters {
        reference_density_kg_m3: IRON_TILLOTSON_RHO0_KG_M3,
        bulk_a_pa: IRON_TILLOTSON_BULK_A_PA,
        bulk_b_pa: IRON_TILLOTSON_BULK_B_PA,
        a: IRON_TILLOTSON_A,
        b: IRON_TILLOTSON_B,
        reference_energy_j_kg: IRON_TILLOTSON_E0_J_KG,
        incipient_vaporisation_j_kg: IRON_TILLOTSON_EIV_J_KG,
        complete_vaporisation_j_kg: IRON_TILLOTSON_ECV_J_KG,
        alpha: IRON_TILLOTSON_ALPHA,
        beta: IRON_TILLOTSON_BETA,
    }
}

#[test]
fn both_materials_are_valid_and_a_bad_one_is_not() {
    assert!(basalt_eos().is_valid() && iron_eos().is_valid());
    let backwards = TillotsonParameters {
        complete_vaporisation_j_kg: 1.0e6,
        ..basalt_eos()
    };
    assert!(!backwards.is_valid(), "E_cv must be above E_iv");
}

#[test]
fn cold_uncompressed_material_is_under_no_pressure() {
    for eos in [basalt_eos(), iron_eos()] {
        assert_eq!(
            tillotson_pressure(eos.reference_density_kg_m3, 0.0, &eos),
            0.0
        );
    }
}

#[test]
fn the_cold_curve_is_a_mu_plus_b_mu_squared() {
    for eos in [basalt_eos(), iron_eos()] {
        for mu in [0.01, 0.1, 0.5, 1.0, 2.0] {
            let rho = eos.reference_density_kg_m3 * (1.0 + mu);
            let expected = eos.bulk_a_pa * mu + eos.bulk_b_pa * mu * mu;
            let p = tillotson_pressure(rho, 0.0, &eos);
            assert!(
                (p / expected - 1.0).abs() < 1e-12,
                "μ = {mu}: {p} against {expected}"
            );
        }
    }
}

#[test]
fn at_the_reference_density_it_is_the_thermal_pressure() {
    let eos = basalt_eos();
    for u in [1.0e3, 1.0e5, 3.0e6, 1.0e7, 5.0e7, 1.0e9] {
        let whole = tillotson_pressure(eos.reference_density_kg_m3, u, &eos);
        let thermal = basalt(eos.reference_density_kg_m3, u);
        assert!(
            (whole / thermal - 1.0).abs() < 1e-12,
            "u = {u}: {whole} against {thermal}"
        );
    }
}

#[test]
fn the_branches_meet_without_a_jump() {
    // Expanded material at the two vaporisation energies: the blend between the branches starts at
    // the condensed one and ends at the expanded one, so the pressure is continuous in `u`.
    for eos in [basalt_eos(), iron_eos()] {
        let rho = 0.8 * eos.reference_density_kg_m3;
        for edge in [
            eos.incipient_vaporisation_j_kg,
            eos.complete_vaporisation_j_kg,
        ] {
            let below = tillotson_pressure(rho, edge * (1.0 - 1e-9), &eos);
            let above = tillotson_pressure(rho, edge * (1.0 + 1e-9), &eos);
            let scale = below.abs().max(above.abs()).max(1.0);
            assert!(
                (below - above).abs() < 1e-6 * scale,
                "at {edge}: {below} and {above}"
            );
        }
        // And in density across `ρ₀`, hot enough to sit between the energies.
        let u = 0.5 * (eos.incipient_vaporisation_j_kg + eos.complete_vaporisation_j_kg);
        let rho0 = eos.reference_density_kg_m3;
        let below = tillotson_pressure(rho0 * (1.0 - 1e-9), u, &eos);
        let above = tillotson_pressure(rho0 * (1.0 + 1e-9), u, &eos);
        assert!(
            (below - above).abs() < 1e-5 * above,
            "across ρ₀: {below} and {above}"
        );
    }
}

#[test]
fn the_pressure_is_never_negative() {
    for eos in [basalt_eos(), iron_eos()] {
        for eta in [0.1, 0.3, 0.6, 0.9, 0.99, 1.0, 1.5] {
            for u in [0.0, 1.0e5, 3.0e6, 1.0e7, 1.0e8] {
                let p = tillotson_pressure(eta * eos.reference_density_kg_m3, u, &eos);
                assert!(p >= 0.0 && p.is_finite(), "η {eta}, u {u}: {p}");
            }
        }
    }
}

#[test]
fn the_sound_speed_of_cold_rock_is_the_bulk_wave_speed_and_rises_with_compression() {
    for eos in [basalt_eos(), iron_eos()] {
        let rho0 = eos.reference_density_kg_m3;
        let c0 = tillotson_sound_speed(rho0, 0.0, &eos);
        let bulk = (eos.bulk_a_pa / rho0).sqrt();
        assert!((c0 / bulk - 1.0).abs() < 1e-3, "{c0} against {bulk}");
        let squeezed = tillotson_sound_speed(1.5 * rho0, 0.0, &eos);
        assert!(squeezed > c0, "{squeezed} against {c0}");
        assert!(tillotson_sound_speed(rho0, 1.0e6, &eos) > 0.0);
    }
}

#[test]
fn the_cold_isentrope_stores_the_work_of_compression() {
    // Along the isentrope du/dρ = P/ρ²: check the slope at a few densities by a finite difference
    // of the integrated energy.
    let eos = basalt_eos();
    let rho0 = eos.reference_density_kg_m3;
    for eta in [1.2, 2.0, 3.0] {
        let rho = eta * rho0;
        let d = 1.0e-4 * rho;
        let slope = (tillotson_cold_energy(rho + d, &eos, 400)
            - tillotson_cold_energy(rho - d, &eos, 400))
            / (2.0 * d);
        let u = tillotson_cold_energy(rho, &eos, 400);
        let expected = tillotson_pressure(rho, u, &eos) / (rho * rho);
        assert!(
            (slope / expected - 1.0).abs() < 1e-4,
            "η {eta}: {slope} against {expected}"
        );
    }
    assert_eq!(tillotson_cold_energy(0.5 * rho0, &eos, 64), 0.0);
}
