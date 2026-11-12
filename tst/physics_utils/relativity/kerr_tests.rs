//! Unit tests for [`crate::physics_utils::relativity::kerr`].
//!
//! Compiled into `src/physics_utils/relativity/kerr.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **The radii are the published ones**, and at zero spin every quantity is the Schwarzschild one
//!    from `gravity.rs`.
//! 2. **The disc's energy budget closes.** The flux, integrated over both faces and weighted by
//!    each ring's orbital energy, equals `1 − E_ISCO`: everything the gas loses on its way to the
//!    ISCO is radiated. The flux formula and the orbit formulas are separate code, so this checks
//!    both.
//! 3. **The equation of motion agrees with the orbit formulas.** A prograde circular orbit with the
//!    Bardeen–Press–Teukolsky angular velocity is exactly centripetal in Kerr–Schild coordinates, it
//!    stays circular when integrated, and orbits plunge inside the ISCO and hold outside it.
//! 4. **Conserved quantities stay conserved** along integrated paths: the energy and the angular
//!    momentum about the spin axis, for matter and light.
//! 5. **Spin drags.** A body released at rest is carried round in the direction of the spin, and
//!    light passing with the spin escapes closer in than light passing against it.

use super::*;
use crate::physics_utils::relativity::gravity::{
    innermost_stable_circular_orbit_radius, photon_sphere_radius, schwarzschild_radius,
};
use std::f64::consts::{FRAC_PI_2, PI};

const SGR_A_KG: f64 = 8.5467e36;
const C: f64 = SPEED_OF_LIGHT_M_S;

fn length(v: [f64; 3]) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}

fn add(a: [f64; 3], b: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2]]
}

/// One RK4 step of the geometric equation of motion (`r_g`, `t_g`).
fn rk4(spin: f64, x: [f64; 3], v: [f64; 3], dt: f64) -> Option<([f64; 3], [f64; 3])> {
    let acc = |x, v| geometric_acceleration(spin, x, v);
    let a1 = acc(x, v)?;
    let (x2, v2) = (add(x, v, 0.5 * dt), add(v, a1, 0.5 * dt));
    let a2 = acc(x2, v2)?;
    let (x3, v3) = (add(x, v2, 0.5 * dt), add(v, a2, 0.5 * dt));
    let a3 = acc(x3, v3)?;
    let (x4, v4) = (add(x, v3, dt), add(v, a3, dt));
    let a4 = acc(x4, v4)?;
    let mut nx = x;
    let mut nv = v;
    for i in 0..3 {
        nx[i] += dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]);
        nv[i] += dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]);
    }
    Some((nx, nv))
}

/// The circular prograde equatorial orbit at Boyer–Lindquist radius `r`, as a Kerr–Schild
/// position and velocity (geometric units), scaled by `speed_factor`.
fn circular_state(spin: f64, r: f64, speed_factor: f64) -> ([f64; 3], [f64; 3]) {
    let omega = circular_orbit(spin, r, true).unwrap().angular_velocity;
    let x = equatorial_point(spin, r, 0.0);
    // Velocity of the point (r cos φ + a sin φ, r sin φ − a cos φ) as φ advances at Ω.
    let v = [spin * omega * speed_factor, r * omega * speed_factor, 0.0];
    (x, v)
}

/// Covariant energy `−p_t` and angular momentum about z, `x p_y − y p_x`, per unit `dt`-normalised
/// four-velocity `(1, v)`. Their ratio is conserved for any geodesic; for a body, multiplying both
/// by `dt/dτ = 1/√(−norm)` conserves each.
fn constants(spin: f64, x: [f64; 3], v: [f64; 3]) -> (f64, f64, f64) {
    let g = kerr_schild_metric(spin, x);
    let full = [1.0, v[0], v[1], v[2]];
    let lower = |mu: usize| (0..4).map(|nu| g[mu][nu] * full[nu]).sum::<f64>();
    let energy = -lower(0);
    let momentum = x[0] * lower(2) - x[1] * lower(1);
    (energy, momentum, norm(&g, v))
}

// -----------------------------------------------------------------------
// The radii and rates.
// -----------------------------------------------------------------------

#[test]
fn at_zero_spin_everything_is_schwarzschild() {
    // In r_g, r_s = 2: horizon 2, photon sphere 3, ISCO 6, ergosphere = horizon.
    let r_s = schwarzschild_radius(SGR_A_KG).unwrap();
    let r_g = r_s / 2.0;
    assert_eq!(horizon_radius(0.0), Some(2.0));
    assert!(
        (photon_orbit_radius(0.0, true).unwrap() * r_g / photon_sphere_radius(SGR_A_KG).unwrap()
            - 1.0)
            .abs()
            < 1.0e-12
    );
    assert!(
        (isco_radius(0.0, false).unwrap() * r_g
            / innermost_stable_circular_orbit_radius(SGR_A_KG).unwrap()
            - 1.0)
            .abs()
            < 1.0e-12
    );
    assert_eq!(ergosphere_radius(0.0, 0.3), Some(2.0));
    assert_eq!(frame_dragging_angular_velocity(0.0, 5.0), Some(0.0));
    assert_eq!(horizon_angular_velocity(0.0), Some(0.0));
}

#[test]
fn the_radii_match_published_values() {
    // Bardeen, Press & Teukolsky 1972; the a -> 1 limits are 1 and 9 (ISCO), 1 and 4 (photon).
    for (spin, isco_pro, isco_retro, photon_pro, photon_retro) in [
        (0.5, 4.2330, 7.5545, 2.3473, 3.5321),
        (0.9, 2.3209, 8.7173, 1.5578, 3.9103),
        (0.998, 1.2370, 8.9944, 1.0737, 3.9988),
    ] {
        assert!(
            (isco_radius(spin, true).unwrap() - isco_pro).abs() < 1.0e-3,
            "isco pro at {spin}"
        );
        assert!(
            (isco_radius(spin, false).unwrap() - isco_retro).abs() < 1.0e-3,
            "isco retro at {spin}"
        );
        assert!(
            (photon_orbit_radius(spin, true).unwrap() - photon_pro).abs() < 1.0e-3,
            "photon pro at {spin}"
        );
        assert!(
            (photon_orbit_radius(spin, false).unwrap() - photon_retro).abs() < 1.0e-3,
            "photon retro at {spin}"
        );
    }
}

#[test]
fn the_order_of_the_radii_is_horizon_photon_orbit_isco_and_the_ergosphere_bulges_at_the_equator() {
    for spin in [0.1, 0.5, 0.9, 0.99] {
        let horizon = horizon_radius(spin).unwrap();
        assert!(horizon < photon_orbit_radius(spin, true).unwrap());
        assert!(photon_orbit_radius(spin, true).unwrap() < isco_radius(spin, true).unwrap());
        assert!(isco_radius(spin, true).unwrap() < isco_radius(spin, false).unwrap());
        assert_eq!(ergosphere_radius(spin, FRAC_PI_2), Some(2.0));
        assert!((ergosphere_radius(spin, 0.0).unwrap() - horizon).abs() < 1.0e-12);
        assert!(horizon < 2.0, "a spinning hole's horizon is smaller");
    }
}

#[test]
fn space_is_dragged_fastest_at_the_horizon_and_slows_as_two_a_over_r_cubed() {
    let spin = 0.9;
    let horizon = horizon_radius(spin).unwrap();
    let at_horizon = frame_dragging_angular_velocity(spin, horizon).unwrap();
    assert!((at_horizon - horizon_angular_velocity(spin).unwrap()).abs() < 1.0e-12);
    assert!(frame_dragging_angular_velocity(spin, 3.0).unwrap() < at_horizon);
    let far = frame_dragging_angular_velocity(spin, 1.0e4).unwrap();
    assert!((far / (2.0 * spin / 1.0e12) - 1.0).abs() < 1.0e-6);
}

#[test]
fn circular_orbits_bind_more_tightly_with_spin_and_are_keplerian_far_out() {
    for spin in [0.0, 0.5, 0.9] {
        let far = circular_orbit(spin, 1.0e6, true).unwrap();
        assert!((far.angular_velocity * 1.0e9 - 1.0).abs() < 1.0e-5);
        assert!((far.angular_momentum / 1.0e3 - 1.0).abs() < 1.0e-5);
        assert!((1.0 - far.energy) * 1.0e6 - 0.5 < 1.0e-5);
    }
    assert!(
        circular_orbit(0.9, 1.5, true).is_none(),
        "inside the prograde photon orbit"
    );
    assert!(
        circular_orbit(0.9, 3.5, false).is_none(),
        "inside the retrograde photon orbit"
    );
    let retro = circular_orbit(0.9, 10.0, false).unwrap();
    assert!(retro.angular_velocity < 0.0 && retro.angular_momentum < 0.0);
}

#[test]
fn the_isco_is_where_the_angular_momentum_of_circular_orbits_is_least() {
    // Found by scanning L(r), not from the formula.
    for spin in [0.0, 0.5, 0.9] {
        let isco = isco_radius(spin, true).unwrap();
        let steps = 100_000;
        let (low, high) = (0.8 * isco, 2.0 * isco);
        let minimum = (0..=steps)
            .map(|i| low + (high - low) * f64::from(i) / f64::from(steps))
            .filter_map(|r| circular_orbit(spin, r, true).map(|o| (r, o.angular_momentum)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .0;
        assert!(
            (minimum / isco - 1.0).abs() < 1.0e-3,
            "spin {spin}: minimum at {minimum}"
        );
    }
}

#[test]
fn the_efficiency_rises_from_six_percent_to_forty_two() {
    assert!((radiative_efficiency(0.0).unwrap() - (1.0 - (8.0_f64 / 9.0).sqrt())).abs() < 1.0e-12);
    assert!((radiative_efficiency(0.9).unwrap() - 0.1558).abs() < 1.0e-3);
    assert!((radiative_efficiency(0.998).unwrap() - 0.321).abs() < 1.0e-3);
    assert!((radiative_efficiency(0.999_999).unwrap() - (1.0 - 1.0 / 3.0_f64.sqrt())).abs() < 0.02);
}

#[test]
fn bad_spins_have_no_answers() {
    for bad in [-0.1, 1.0, 1.5, f64::NAN] {
        assert_eq!(horizon_radius(bad), None);
        assert_eq!(isco_radius(bad, true), None);
        assert_eq!(photon_orbit_radius(bad, true), None);
        assert_eq!(circular_orbit(bad, 10.0, true), None);
        assert_eq!(KerrBlackHole::new(SGR_A_KG, bad), None);
    }
    assert_eq!(KerrBlackHole::new(0.0, 0.5), None);
}

// -----------------------------------------------------------------------
// The accretion disc.
// -----------------------------------------------------------------------

#[test]
fn the_disc_radiates_nothing_at_the_isco_and_peaks_just_outside() {
    let spin = 0.9;
    let isco = isco_radius(spin, true).unwrap();
    assert_eq!(disc_flux(spin, isco), Some(0.0));
    assert_eq!(disc_flux(spin, 0.99 * isco), None);

    let (mut peak_r, mut peak) = (0.0, 0.0);
    for i in 1..2000 {
        let r = isco * (1.0 + f64::from(i) * 0.002);
        let f = disc_flux(spin, r).unwrap();
        if f > peak {
            peak = f;
            peak_r = r;
        }
    }
    assert!(
        peak_r > isco && peak_r < 2.0 * isco,
        "peak at {peak_r}, ISCO {isco}"
    );
}

#[test]
fn far_out_the_disc_is_the_newtonian_one() {
    // 3/(8π r³) (1 − C/√r): the inner edge leaves a correction falling as 1/√r, whose constant C
    // in general relativity includes logarithmic terms from the inner disc, so it is not simply
    // √r_ISCO. At r = 10⁶ the correction is a few parts in a thousand.
    for spin in [0.0, 0.9] {
        let r: f64 = 1.0e6;
        let newton = 3.0 / (8.0 * PI * r.powi(3));
        let f = disc_flux(spin, r).unwrap();
        assert!(
            f < newton && (f / newton - 1.0).abs() < 6.0e-3,
            "spin {spin}: {}",
            f / newton
        );
    }
}

#[test]
fn the_discs_energy_budget_closes() {
    // Everything gas loses spiralling from far away to the ISCO is radiated:
    // ∫ 4π r F̂ E dr from the ISCO outward = 1 − E_ISCO. Integrated in u = √(r − r_ISCO) out to
    // r = 10⁴, plus the Newtonian tail 3/(2r) beyond.
    for spin in [0.0, 0.5, 0.9] {
        let isco = isco_radius(spin, true).unwrap();
        let outer: f64 = 1.0e4;
        let steps = 4_000;
        let span = (outer - isco).sqrt();
        let h = span / f64::from(steps);
        let integrand = |u: f64| {
            if u == 0.0 {
                return 0.0;
            }
            let r = isco + u * u;
            4.0 * PI
                * r
                * disc_flux(spin, r).unwrap()
                * circular_orbit(spin, r, true).unwrap().energy
                * 2.0
                * u
        };
        let mut sum = integrand(0.0) + integrand(span);
        for i in 1..steps {
            sum += if i % 2 == 1 { 4.0 } else { 2.0 } * integrand(h * f64::from(i));
        }
        let radiated = sum * h / 3.0 + 1.5 / outer;
        let efficiency = radiative_efficiency(spin).unwrap();
        assert!(
            (radiated / efficiency - 1.0).abs() < 5.0e-3,
            "spin {spin}: {radiated} against {efficiency}"
        );
    }
}

#[test]
fn sgr_a_star_fed_at_a_tenth_of_eddington_glows_in_the_ultraviolet() {
    let hole = KerrBlackHole::new(SGR_A_KG, 0.9).unwrap();
    // L_Edd = 1.26e31 W per solar mass.
    assert!((hole.eddington_luminosity_w() / (1.2572e31 * 4.298e6) - 1.0).abs() < 2.0e-3);

    let mdot = hole.accretion_rate_kg_s(0.1).unwrap();
    let isco = isco_radius(0.9, true).unwrap();
    let peak = (1..400)
        .map(|i| isco * (1.0 + 0.01 * f64::from(i)))
        .map(|r| hole.disc_temperature_k(r, mdot).unwrap())
        .fold(0.0_f64, f64::max);
    // A few times 1e5 K: the inner disc of a supermassive hole radiates in the ultraviolet.
    assert!(peak > 1.0e5 && peak < 1.0e6, "peak temperature {peak:e} K");
    assert!(hole.disc_temperature_k(20.0, mdot).unwrap() < 0.5 * peak);
}

// -----------------------------------------------------------------------
// Kerr–Schild coordinates and the equation of motion.
// -----------------------------------------------------------------------

#[test]
fn the_kerr_schild_metric_without_spin_is_schwarzschild() {
    // g_00 = −(1 − 2/r).
    let g = kerr_schild_metric(0.0, [3.0, 4.0, 0.0]);
    assert!((g[0][0] + 1.0 - 2.0 / 5.0).abs() < 1.0e-12);
}

#[test]
fn a_prograde_circular_orbit_is_exactly_centripetal_at_keplers_rate() {
    // The orbit formula and the equation of motion are separate code. Prograde is counter-clockwise
    // about +z.
    for spin in [0.0, 0.5, 0.9] {
        for r in [isco_radius(spin, true).unwrap(), 4.0, 10.0] {
            let (x, v) = circular_state(spin, r, 1.0);
            let a = geometric_acceleration(spin, x, v).unwrap();
            let omega = circular_orbit(spin, r, true).unwrap().angular_velocity;
            for i in 0..3 {
                let centripetal = -omega * omega * x[i];
                assert!(
                    (a[i] - centripetal).abs() < 1.0e-7 * omega * omega * length(x),
                    "spin {spin} r {r} axis {i}"
                );
            }
        }
    }
}

#[test]
fn a_retrograde_orbit_needs_the_retrograde_rate() {
    let spin = 0.9;
    let r = 10.0;
    let omega = circular_orbit(spin, r, false).unwrap().angular_velocity;
    let (sin, cos) = (0.0_f64, 1.0_f64);
    let x = [r * cos + spin * sin, r * sin - spin * cos, 0.0];
    let v = [spin * omega, r * omega, 0.0];
    let a = geometric_acceleration(spin, x, v).unwrap();
    assert!((a[0] / (-omega * omega * x[0]) - 1.0).abs() < 1.0e-6);
}

#[test]
fn an_integrated_circular_orbit_stays_circular() {
    let spin = 0.9;
    let r = 4.0;
    let (mut x, mut v) = circular_state(spin, r, 1.0);
    let radius = length(x);
    let period = 2.0 * PI / circular_orbit(spin, r, true).unwrap().angular_velocity;
    let steps = 4_000;
    for _ in 0..steps {
        (x, v) = rk4(spin, x, v, period / f64::from(steps)).unwrap();
    }
    assert!((length(x) / radius - 1.0).abs() < 1.0e-6);
    assert!(
        (x[0] - equatorial_point(spin, r, 0.0)[0]).abs() < 1.0e-4 * radius,
        "back where it started"
    );
}

#[test]
fn energy_and_angular_momentum_are_conserved_along_an_eccentric_orbit() {
    let spin = 0.9;
    let (mut x, mut v) = circular_state(spin, 8.0, 0.9);
    x[2] = 1.0; // and a little out of the plane
    let (e0, l0, n0) = constants(spin, x, v);
    let (e0, l0) = (e0 / (-n0).sqrt(), l0 / (-n0).sqrt());
    for _ in 0..40_000 {
        (x, v) = rk4(spin, x, v, 0.05).unwrap();
    }
    let (e1, l1, n1) = constants(spin, x, v);
    let (e1, l1) = (e1 / (-n1).sqrt(), l1 / (-n1).sqrt());
    assert!((e1 / e0 - 1.0).abs() < 1.0e-8, "energy {e0} -> {e1}");
    assert!(
        (l1 / l0 - 1.0).abs() < 1.0e-8,
        "angular momentum {l0} -> {l1}"
    );
}

/// Integrate from `(x, v)` until captured (true) or `steps` run out (false).
fn plunges(spin: f64, mut x: [f64; 3], mut v: [f64; 3], steps: usize) -> bool {
    let horizon = horizon_radius(spin).unwrap();
    for _ in 0..steps {
        if kerr_schild_radius(spin, x) <= horizon * 1.001 {
            return true;
        }
        match rk4(spin, x, v, 0.02 * kerr_schild_radius(spin, x)) {
            Some(next) => (x, v) = next,
            None => return true,
        }
    }
    false
}

#[test]
fn a_nudged_orbit_holds_outside_the_isco_and_plunges_inside() {
    let spin = 0.9;
    let isco = isco_radius(spin, true).unwrap();
    let (x, v) = circular_state(spin, 1.15 * isco, 0.998);
    assert!(!plunges(spin, x, v, 40_000), "outside the ISCO it holds");
    let (x, v) = circular_state(spin, 0.95 * isco, 0.998);
    assert!(plunges(spin, x, v, 40_000), "inside the ISCO it plunges");
}

#[test]
fn a_body_with_no_angular_momentum_is_carried_round_at_the_frame_dragging_rate() {
    // On the equator, a body with no angular momentum about the spin axis and no radial motion is
    // carried round at ω(r). Its velocity comes from the metric (p_y = 0); ω comes from the
    // formula. At the KS point (x, 0, 0) a ring of fixed r rotating at ω moves at (0, x ω, 0).
    for spin in [0.5, 0.9] {
        for x in [2.2, 3.0, 6.0] {
            let position = [x, 0.0, 0.0];
            let g = kerr_schild_metric(spin, position);
            let v_y = -g[2][0] / g[2][2];
            let r = kerr_schild_radius(spin, position);
            let omega = frame_dragging_angular_velocity(spin, r).unwrap();
            assert!(
                (v_y / (x * omega) - 1.0).abs() < 1.0e-9,
                "spin {spin} at x = {x}"
            );
            assert!(v_y > 0.0, "dragged with the spin");
        }
    }
    let g = kerr_schild_metric(0.0, [3.0, 0.0, 0.0]);
    assert_eq!(g[2][0], 0.0, "no spin, no drag");
}

#[test]
fn light_moves_at_c_far_away_and_is_null_everywhere() {
    for spin in [0.0, 0.9] {
        for x in [[1.0e6, 0.0, 0.0], [5.0, 2.0, 1.0], [2.5, 0.0, 0.0]] {
            for d in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-0.3, 0.5, 0.8]] {
                let hole = KerrBlackHole::new(SGR_A_KG, spin).unwrap();
                let r_g = hole.gravitational_radius_m();
                let v = kerr_schild_light_velocity(SGR_A_KG, spin, x.map(|c| c * r_g), d).unwrap();
                let n = kerr_schild_norm(SGR_A_KG, spin, x.map(|c| c * r_g), v).unwrap();
                assert!(n.abs() < 1.0e-12, "not null: {n}");
            }
        }
    }
    let far =
        kerr_schild_light_velocity(SGR_A_KG, 0.9, [1.0e20, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    assert!((length(far) / C - 1.0).abs() < 1.0e-6);
}

#[test]
fn the_prograde_photon_orbit_is_a_circular_null_orbit() {
    let spin = 0.9;
    let r = photon_orbit_radius(spin, true).unwrap();
    let omega = 1.0 / (r.powf(1.5) + spin);
    let x = equatorial_point(spin, r, 0.0);
    let v = [spin * omega, r * omega, 0.0];
    assert!(
        norm(&kerr_schild_metric(spin, x), v).abs() < 1.0e-9,
        "light speed there"
    );
    let a = geometric_acceleration(spin, x, v).unwrap();
    assert!((a[0] / (-omega * omega * x[0]) - 1.0).abs() < 1.0e-6);
}

/// Fire a ray along +x from x = −40 r_g at height y = b (in the equatorial plane); true if it is
/// captured.
fn ray_captured(spin: f64, b: f64) -> bool {
    let hole = KerrBlackHole::new(SGR_A_KG, spin).unwrap();
    let r_g = hole.gravitational_radius_m();
    let mut x = [-40.0, b, 0.0];
    let mut v = kerr_schild_light_velocity(SGR_A_KG, spin, x.map(|c| c * r_g), [1.0, 0.0, 0.0])
        .unwrap()
        .map(|c| c / C);
    let horizon = horizon_radius(spin).unwrap();
    for _ in 0..200_000 {
        let r = kerr_schild_radius(spin, x);
        if r <= horizon * 1.001 {
            return true;
        }
        if r > 45.0 && x[0] * v[0] + x[1] * v[1] > 0.0 {
            return false;
        }
        match rk4(spin, x, v, 0.01 * r) {
            Some(next) => (x, v) = next,
            None => return true,
        }
    }
    panic!("the ray at b = {b} neither escaped nor was captured");
}

#[test]
fn light_passing_with_the_spin_escapes_closer_in_than_light_passing_against_it() {
    // Without spin the critical impact parameter is 3√3 = 5.196 r_g on both sides. With a = 0.9 it
    // shrinks on the side moving with the spin (about 3.5 here) and grows on the other (about 6.8).
    // A ray along +x at y = −4 passes the hole's counter-clockwise side, moving with the spin.
    assert!(ray_captured(0.0, 5.1) && !ray_captured(0.0, 5.3));
    assert!(ray_captured(0.0, -5.1) && !ray_captured(0.0, -5.3));

    assert!(!ray_captured(0.9, -4.0), "with the spin: escapes at 4 r_g");
    assert!(
        ray_captured(0.9, 4.0),
        "against the spin: captured at 4 r_g"
    );
}

#[test]
fn the_kerr_schild_radius_inverts_the_equatorial_map() {
    for spin in [0.0, 0.5, 0.9] {
        for r in [1.9, 3.0, 12.0] {
            for phi in [0.0, 1.0, 4.0] {
                let p = equatorial_point(spin, r, phi);
                assert!((kerr_schild_radius(spin, p) - r).abs() < 1.0e-12);
            }
        }
    }
}

#[test]
fn nothing_is_answered_inside_the_horizon() {
    let hole = KerrBlackHole::new(SGR_A_KG, 0.9).unwrap();
    let r_g = hole.gravitational_radius_m();
    let inside = [1.2 * r_g, 0.0, 0.0];
    assert_eq!(
        kerr_schild_acceleration(SGR_A_KG, 0.9, inside, [0.0; 3]),
        None
    );
    assert_eq!(
        kerr_schild_light_velocity(SGR_A_KG, 0.9, inside, [1.0, 0.0, 0.0]),
        None
    );
    assert_eq!(
        kerr_schild_acceleration(SGR_A_KG, 1.0, [10.0 * r_g, 0.0, 0.0], [0.0; 3]),
        None
    );
}

#[test]
fn the_analytic_acceleration_matches_the_numerical_one() {
    // The analytic Christoffel symbols against central differences of the metric and a numerically
    // inverted metric, at points on and off the plane, for matter and light speeds.
    for spin in [0.0, 0.5, 0.9] {
        for x in [
            [3.0, 1.0, 0.5],
            [-2.0, 2.5, -1.5],
            [10.0, -4.0, 6.0],
            [0.3, 0.2, 2.6],
        ] {
            for v in [[0.0, 0.0, 0.0], [0.1, -0.3, 0.2], [-0.5, 0.4, 0.1]] {
                if kerr_schild_radius(spin, x) <= horizon_radius(spin).unwrap() * 1.05 {
                    continue;
                }
                let analytic = geometric_acceleration(spin, x, v).unwrap();
                let numerical = numerical_acceleration(spin, x, v).unwrap();
                let scale = length(numerical).max(1.0e-6);
                for i in 0..3 {
                    assert!(
                        (analytic[i] - numerical[i]).abs() < 1.0e-6 * scale,
                        "spin {spin} at {x:?}, v {v:?}: {analytic:?} against {numerical:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn equatorial_coordinates_invert_equatorial_point() {
    for spin in [0.0, 0.5, 0.9, 0.998] {
        for radius in [1.5, 2.32, 6.0, 20.0] {
            for azimuth in [-3.0, -1.0, 0.0, 0.5, 2.0, 3.1] {
                let (r, phi) =
                    equatorial_coordinates(spin, equatorial_point(spin, radius, azimuth));
                let turn = (phi - azimuth).rem_euclid(std::f64::consts::TAU);
                assert!(
                    (r - radius).abs() < 1.0e-10,
                    "spin {spin}: r {r} vs {radius}"
                );
                assert!(
                    turn < 1.0e-10 || std::f64::consts::TAU - turn < 1.0e-10,
                    "spin {spin}: phi {phi} vs {azimuth}"
                );
            }
        }
    }
}
