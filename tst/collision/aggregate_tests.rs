//! Tests for [`crate::collision::aggregate`], `#[path]`-included into it.
//!
//! The packing is checked as geometry (count near the target, every particle inside the body,
//! neighbours exactly touching and none overlapping), the built body as physics (its mass and
//! centre of mass), and the clustering on piles built by hand.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/collision/proptest-regressions/aggregate_tests.txt";

fn rock() -> Material {
    Material::new(0.0, 0.0).with_modulus(1.0e13)
}

fn body(centre_m: DVec3, radius_m: f64, mass_kg: f64) -> ParticleBody {
    ParticleBody {
        centre_units: centre_m / METRES_PER_UNIT,
        velocity_units_s: DVec3::new(1.0e-9, 0.0, 0.0),
        mass_kg,
        radius_m,
        color: Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        },
        material: rock(),
    }
}

#[test]
fn neighbours_just_touch_and_none_overlap() {
    let spacing = lattice_spacing_m(1.0e6, 300);
    let points = lattice_points(1.0e6, spacing);
    let diameter = 2.0 * particle_radius_m(spacing);
    let mut nearest = f64::INFINITY;
    for (i, a) in points.iter().enumerate() {
        for b in &points[i + 1..] {
            nearest = nearest.min((*a - *b).length());
        }
    }
    assert!(
        (nearest / diameter - 1.0).abs() < 1.0e-9,
        "nearest {nearest}, diameter {diameter}"
    );
}

#[test]
fn every_particle_lies_inside_the_body() {
    let spacing = lattice_spacing_m(1.0e6, 300);
    let radius = particle_radius_m(spacing);
    for point in lattice_points(1.0e6, spacing) {
        assert!(point.length() + radius <= 1.0e6 * (1.0 + 1.0e-12));
    }
}

#[test]
fn a_body_of_one_particle_is_one_sphere_its_own_size() {
    let particles = particle_body(
        &body(DVec3::ZERO, 2.0e6, 1.0e22),
        lattice_spacing_m(2.0e6, 1),
    );
    assert_eq!(particles.len(), 1);
    let Body::Sphere { radius, .. } = particles[0].body else {
        panic!("a sphere")
    };
    assert!((f64::from(radius) * METRES_PER_UNIT / 2.0e6 - 1.0).abs() < 1.0e-6);
}

#[test]
fn the_built_body_has_the_planet_s_mass_centre_and_motion() {
    let centre = DVec3::new(3.0e7, -1.0e7, 2.0e6);
    let particles = particle_body(&body(centre, 6.4e6, 6.0e24), lattice_spacing_m(6.4e6, 150));
    let mass: f64 = particles.iter().map(|p| f64::from(p.mass_kg)).sum();
    assert!((mass / 6.0e24 - 1.0).abs() < 1.0e-5);
    let moment: DVec3 = particles
        .iter()
        .map(|p| p.body.center() * f64::from(p.mass_kg))
        .sum();
    let found = moment / mass * METRES_PER_UNIT;
    assert!((found - centre).length() < 1.0e-3 * 6.4e6, "{found:?}");
    assert!(particles
        .iter()
        .all(|p| p.velocity_units_s == DVec3::new(1.0e-9, 0.0, 0.0) && p.material == rock()));
}

#[test]
fn a_pile_is_one_cluster_and_two_apart_are_two_largest_first() {
    let spacing = lattice_spacing_m(1.0e6, 60);
    let mut props = particle_body(&body(DVec3::ZERO, 1.0e6, 1.0e22), spacing);
    assert_eq!(contact_clusters(&props).len(), 1);

    let small = particle_body(&body(DVec3::new(5.0e6, 0.0, 0.0), 5.0e5, 1.0e21), spacing);
    let first_small = props.len();
    props.extend(small);
    let clusters = contact_clusters(&props);
    assert_eq!(clusters.len(), 2);
    assert!(
        clusters[0].iter().all(|&i| i < first_small),
        "the bigger pile first"
    );
    assert!(clusters[1].iter().all(|&i| i >= first_small));
}

#[test]
fn rigid_props_are_left_out_of_clusters() {
    let spacing = lattice_spacing_m(1.0e6, 20);
    let mut props = particle_body(&body(DVec3::ZERO, 1.0e6, 1.0e22), spacing);
    for prop in &mut props {
        prop.material = Material::ELASTIC;
    }
    assert!(contact_clusters(&props).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Fluid planets: the hydrostatic profile and the layered build.
// ---------------------------------------------------------------------------------------------

use crate::core::scene::Tillotson;
use crate::physics_utils::physics_constants::{
    BASALT_TILLOTSON_A, BASALT_TILLOTSON_ALPHA, BASALT_TILLOTSON_B, BASALT_TILLOTSON_BETA,
    BASALT_TILLOTSON_BULK_A_PA, BASALT_TILLOTSON_BULK_B_PA, BASALT_TILLOTSON_E0_J_KG,
    BASALT_TILLOTSON_ECV_J_KG, BASALT_TILLOTSON_EIV_J_KG, BASALT_TILLOTSON_RHO0_KG_M3,
    IRON_TILLOTSON_A, IRON_TILLOTSON_ALPHA, IRON_TILLOTSON_B, IRON_TILLOTSON_BETA,
    IRON_TILLOTSON_BULK_A_PA, IRON_TILLOTSON_BULK_B_PA, IRON_TILLOTSON_E0_J_KG,
    IRON_TILLOTSON_ECV_J_KG, IRON_TILLOTSON_EIV_J_KG, IRON_TILLOTSON_RHO0_KG_M3,
};

/// The Earth's mass, kg.
const EARTH_KG: f64 = 5.972e24;

fn basalt() -> Tillotson {
    Tillotson {
        reference_density_kg_m3: BASALT_TILLOTSON_RHO0_KG_M3 as f32,
        bulk_a_pa: BASALT_TILLOTSON_BULK_A_PA as f32,
        bulk_b_pa: BASALT_TILLOTSON_BULK_B_PA as f32,
        a: BASALT_TILLOTSON_A as f32,
        b: BASALT_TILLOTSON_B as f32,
        reference_energy_j_kg: BASALT_TILLOTSON_E0_J_KG as f32,
        incipient_vaporisation_j_kg: BASALT_TILLOTSON_EIV_J_KG as f32,
        complete_vaporisation_j_kg: BASALT_TILLOTSON_ECV_J_KG as f32,
        alpha: BASALT_TILLOTSON_ALPHA as f32,
        beta: BASALT_TILLOTSON_BETA as f32,
    }
}

fn iron() -> Tillotson {
    Tillotson {
        reference_density_kg_m3: IRON_TILLOTSON_RHO0_KG_M3 as f32,
        bulk_a_pa: IRON_TILLOTSON_BULK_A_PA as f32,
        bulk_b_pa: IRON_TILLOTSON_BULK_B_PA as f32,
        a: IRON_TILLOTSON_A as f32,
        b: IRON_TILLOTSON_B as f32,
        reference_energy_j_kg: IRON_TILLOTSON_E0_J_KG as f32,
        incipient_vaporisation_j_kg: IRON_TILLOTSON_EIV_J_KG as f32,
        complete_vaporisation_j_kg: IRON_TILLOTSON_ECV_J_KG as f32,
        alpha: IRON_TILLOTSON_ALPHA as f32,
        beta: IRON_TILLOTSON_BETA as f32,
    }
}

fn earth_profile() -> PlanetProfile {
    hydrostatic_profile(EARTH_KG, 0.3, &iron().parameters(), &basalt().parameters()).unwrap()
}

#[test]
fn the_earth_s_profile_holds_its_mass_and_reports_its_size() {
    let profile = earth_profile();
    println!(
        "Earth: radius {:.0} km, core {:.0} km, centre {:.3e} Pa, centre density {:.0}, surface density {:.0}",
        profile.radius_m / 1e3,
        profile.core_radius_m / 1e3,
        profile.central_pressure_pa,
        profile.density_at(0.0),
        profile.density_at(profile.radius_m),
    );
    assert!(
        (profile.mass_kg / EARTH_KG - 1.0).abs() < 1e-9,
        "{}",
        profile.mass_kg
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        cases: 32,
        ..ProptestConfig::default()
    })]

    /// The packing finds a count near the one asked for, at any size.
    #[test]
    fn the_count_is_near_the_target(target in 20_u32..600, radius in 1.0e5_f64..1.0e7) {
        let found = lattice_points(radius, lattice_spacing_m(radius, target)).len() as f64;
        let wanted = f64::from(target);
        // The lattice fills in whole symmetric shells, so the count moves in steps: at small counts
        // the nearest step can be a fifth away (110 asked, 87 found).
        prop_assert!((found - wanted).abs() <= 0.25 * wanted + 6.0, "{target} → {found}");
    }
}
