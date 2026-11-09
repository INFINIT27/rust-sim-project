//! Tests for [`crate::spacecraft::lambert`], `#[path]`-included as a unit-test module of it, not
//! an integration test.

use super::*;

const EARTH_MU: f64 = 3.986_004_418e14;

#[test]
fn it_reproduces_curtis_example_5_2() {
    // Curtis, Orbital Mechanics for Engineering Students, Example 5.2: from r1 to r2 about Earth in
    // one hour, prograde.
    let r1 = DVec3::new(5_000.0e3, 10_000.0e3, 2_100.0e3);
    let r2 = DVec3::new(-14_600.0e3, 2_500.0e3, 7_000.0e3);
    let normal = r1.cross(r2);
    let (v1, v2) = lambert(r1, r2, 3_600.0, EARTH_MU, normal).expect("a solution");
    let expected_v1 = DVec3::new(-5_992.5, 1_925.4, 3_245.6);
    let expected_v2 = DVec3::new(-3_312.5, -4_196.6, -385.29);
    assert!((v1 - expected_v1).length() < 5.0, "{v1:?}");
    assert!((v2 - expected_v2).length() < 5.0, "{v2:?}");
}

/// Propagate a two-body orbit with a fine fixed step, to check an arc ends where it should.
fn propagate(r: DVec3, v: DVec3, mu: f64, t: f64) -> DVec3 {
    let steps = 200_000;
    let dt = t / steps as f64;
    let (mut r, mut v) = (r, v);
    // Velocity Verlet: second order, so a fine step converges.
    let accel = |r: DVec3| -r * (mu / r.length().powi(3));
    let mut a = accel(r);
    for _ in 0..steps {
        v += a * (0.5 * dt);
        r += v * dt;
        a = accel(r);
        v += a * (0.5 * dt);
    }
    r
}

#[test]
fn the_arc_it_finds_actually_arrives() {
    // Earth to Mars distances about the Sun, a quarter-turn apart, in 200 days.
    let sun_mu = 1.327_124_4e20;
    let r1 = DVec3::new(1.496e11, 0.0, 0.0);
    let r2 = DVec3::new(0.0, 0.0, -2.279e11);
    let normal = DVec3::Y;
    let tof = 200.0 * 86_400.0;
    let (v1, _) = lambert(r1, r2, tof, sun_mu, normal).unwrap();
    let arrived = propagate(r1, v1, sun_mu, tof);
    assert!(
        (arrived - r2).length() < 1.0e-5 * r2.length(),
        "missed by {:.0} km",
        (arrived - r2).length() / 1e3
    );
}

#[test]
fn the_normal_chooses_the_way_round() {
    let r1 = DVec3::new(1.0e7, 0.0, 0.0);
    let r2 = DVec3::new(0.0, 1.0e7, 0.0);
    let (prograde, _) = lambert(r1, r2, 3.0e4, EARTH_MU, DVec3::Z).unwrap();
    let (retrograde, _) = lambert(r1, r2, 3.0e4, EARTH_MU, -DVec3::Z).unwrap();
    assert!(
        r1.cross(prograde).z > 0.0,
        "the short way, anticlockwise about +z"
    );
    assert!(
        r1.cross(retrograde).z < 0.0,
        "the long way round, clockwise"
    );
}

#[test]
fn degenerate_and_impossible_arcs_have_no_answer() {
    let r1 = DVec3::new(1.0e7, 0.0, 0.0);
    // In line with the body: the plane is undefined.
    assert!(lambert(r1, r1 * 2.0, 3.0e4, EARTH_MU, DVec3::Z).is_none());
    // A time of flight no arc can make, and nonsense inputs.
    assert!(lambert(r1, DVec3::new(0.0, 1.0e7, 0.0), 1.0e-6, EARTH_MU, DVec3::Z).is_none());
    assert!(lambert(r1, DVec3::new(0.0, 1.0e7, 0.0), -1.0, EARTH_MU, DVec3::Z).is_none());
    assert!(lambert(DVec3::ZERO, r1, 3.0e4, EARTH_MU, DVec3::Z).is_none());
}
