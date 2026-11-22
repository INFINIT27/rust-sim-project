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
