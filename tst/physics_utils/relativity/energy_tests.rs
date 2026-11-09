//! Unit and property tests for [`crate::physics_utils::relativity::energy`].
//!
//! Compiled into `src/physics_utils/relativity/energy.rs` via `#[path]`.
//!
//! # The two claims worth the most here
//!
//! 1. **`KE = (γ−1)mc²` reduces to `½mv²`** at everyday speeds — and it only does so because of the
//!    stable `γ−1`. `the_naive_gamma_minus_one_gets_everyday_kinetic_energy_badly_wrong` measures the
//!    alternative: at `10 m/s` the naive form is `20%` high, and at `1 m/s` it returns exactly zero.
//!    That is the most dramatic case for `lorentz_factor_minus_one` anywhere in the sub-layer, because
//!    `c² = 8.99e16` multiplies whatever error it carries.
//! 2. **`E² = (pc)² + (mc²)²`** — the energy–momentum relation, which is not implemented anywhere and is
//!    therefore a genuine cross-check: it ties `energy.rs` to `momentum.rs` through an identity neither
//!    module was written to satisfy. If either has a stray `γ`, this fails.
//!
//! Plus the fact that separates this module from [`dynamics`](crate::physics_utils::relativity::dynamics):
//! the work–energy theorem **survives** relativity unchanged, and a transverse force does no work at all.

use super::*;
use crate::physics_utils::relativity::lorentz::lorentz_factor;
use crate::physics_utils::relativity::momentum::relativistic_momentum;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str = "tst/physics_utils/proptest-regressions/relativity_energy_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;

const ELECTRON_MASS_KG: f64 = 9.109_383_701_5e-31;
const PROTON_MASS_KG: f64 = 1.672_621_923_69e-27;
const ELECTRONVOLT_J: f64 = 1.602_176_634e-19;

// -----------------------------------------------------------------------
// Rest energy, against published particle masses.
// -----------------------------------------------------------------------

#[test]
fn the_electron_and_proton_rest_energies_match_published_values() {
    // 0.510999 MeV and 938.272 MeV. Six significant figures each, from CODATA — a check on `c`, on the
    // masses, and on the formula all at once, and one that no self-consistent implementation can fake.
    let electron_mev =
        rest_energy(ELECTRON_MASS_KG).expect("positive mass") / ELECTRONVOLT_J / 1.0e6;
    let proton_mev = rest_energy(PROTON_MASS_KG).expect("positive mass") / ELECTRONVOLT_J / 1.0e6;

    assert!(
        (electron_mev - 0.510_998_95).abs() < 1.0e-6,
        "electron rest energy came out {electron_mev} MeV"
    );
    assert!(
        (proton_mev - 938.272_088).abs() < 1.0e-3,
        "proton rest energy came out {proton_mev} MeV"
    );
}

#[test]
fn a_kilogram_is_about_twenty_one_megatons() {
    // The figure that puts `mc^2` in perspective, and a reminder of what this module does *not* do:
    // nothing here releases this energy. Mass is conserved throughout.
    let joules = rest_energy(1.0).expect("positive mass");
    let megatons = joules / 4.184e15;

    assert!(
        (megatons - 21.48).abs() < 0.01,
        "a kilogram came out {megatons} megatons"
    );
}

// -----------------------------------------------------------------------
// Kinetic energy, and the reason it needs the stable gamma-1.
// -----------------------------------------------------------------------

#[test]
fn kinetic_energy_reduces_to_half_m_v_squared_at_everyday_speeds() {
    // The correspondence principle, and the thing that would be impossible with a naive `gamma - 1.0`.
    for v in [1.0, 10.0, 100.0, 1_000.0, 30_000.0] {
        let relativistic = relativistic_kinetic_energy(1.0, v).expect("subluminal");
        let newtonian = 0.5 * 1.0 * v * v;

        assert!(
            (relativistic / newtonian - 1.0).abs() < 1.0e-7,
            "v = {v}: got {relativistic:e}, expected about {newtonian:e}"
        );
    }
}

#[test]
fn the_naive_gamma_minus_one_gets_everyday_kinetic_energy_badly_wrong() {
    // The measurement behind the module's insistence on `lorentz_factor_minus_one`. This is not a
    // question of a few digits: `c^2` is 8.99e16, so it multiplies whatever error `gamma - 1.0` carries
    // into something visible.
    let c_squared = C * C;

    // At 1 m/s the naive form returns *exactly zero* — a body with 0.5 J of kinetic energy reported as
    // having none.
    let naive_at_one = (lorentz_factor(1.0).unwrap() - 1.0) * 1.0 * c_squared;
    assert_eq!(naive_at_one, 0.0, "the naive form must underflow here");
    let stable_at_one = relativistic_kinetic_energy(1.0, 1.0).expect("subluminal");
    assert!(
        (stable_at_one - 0.5).abs() < 1.0e-9,
        "the stable form should give 1/2 m v^2 = 0.5 J, got {stable_at_one}"
    );

    // At 10 m/s the naive form is 59.87 J against a true 50 J — nearly 20% high.
    let naive_at_ten = (lorentz_factor(10.0).unwrap() - 1.0) * 1.0 * c_squared;
    let stable_at_ten = relativistic_kinetic_energy(1.0, 10.0).expect("subluminal");

    assert!(
        (stable_at_ten - 50.0).abs() < 1.0e-9,
        "the stable form should give 50 J, got {stable_at_ten}"
    );
    assert!(
        naive_at_ten / stable_at_ten > 1.1,
        "the naive form should be badly high here; got {naive_at_ten} against {stable_at_ten}"
    );
}

#[test]
fn at_gamma_two_the_kinetic_energy_equals_the_rest_energy() {
    // beta = sqrt(3)/2 gives gamma = 2, so KE = (2-1)mc^2 = mc^2 exactly. Readable without a reference.
    let v = 3.0_f64.sqrt() / 2.0 * C;
    let ke = relativistic_kinetic_energy(1.0, v).expect("subluminal");
    let rest = rest_energy(1.0).expect("positive mass");

    assert!(
        (ke / rest - 1.0).abs() < 1.0e-12,
        "KE/mc^2 came out {}",
        ke / rest
    );
}

#[test]
fn the_three_energies_are_one_equation() {
    // E = E0 + KE, at every speed. Cheap, and it stops `total_energy` from drifting into an independent
    // claim about `gamma`.
    for beta in [0.0, 0.1, 0.5, 0.6, 0.9, 0.99] {
        let v = beta * C;
        let total = total_energy(2.0, v).expect("subluminal");
        let rest = rest_energy(2.0).expect("positive mass");
        let kinetic = relativistic_kinetic_energy(2.0, v).expect("subluminal");

        assert!(
            (total - (rest + kinetic)).abs() / total < 1.0e-12,
            "beta {beta}: {total:e} vs {:e}",
            rest + kinetic
        );
        assert!(total >= rest, "total energy is never below rest energy");
    }
}

#[test]
fn an_lhc_proton_carries_about_six_point_eight_tev_of_kinetic_energy() {
    // Total 6.8 TeV, of which the rest energy is only 0.938 GeV — so the kinetic energy is 6.799 TeV
    // and the proton is 7247 times more energetic than a stationary one.
    let gamma = 7247.4_f64;
    let beta = (1.0 - 1.0 / (gamma * gamma)).sqrt();

    let ke_tev = relativistic_kinetic_energy(PROTON_MASS_KG, beta * C).expect("subluminal")
        / ELECTRONVOLT_J
        / 1.0e12;

    assert!(
        (ke_tev - 6.799).abs() < 0.005,
        "kinetic energy came out {ke_tev} TeV"
    );
}

#[test]
fn earths_orbital_kinetic_energy_matches_the_newtonian_figure() {
    // The figure that says why `experiment_3` needs none of this: Earth's orbital kinetic energy is
    // 2.65e33 J, and the relativistic correction is 7.4e-9 of it.
    let earth_mass = 5.972e24;
    let v = 29_780.0;

    let relativistic = relativistic_kinetic_energy(earth_mass, v).expect("subluminal");
    let newtonian = 0.5 * earth_mass * v * v;

    assert!(
        (relativistic / 2.647e33 - 1.0).abs() < 0.01,
        "came out {relativistic:e} J"
    );
    assert!(
        (relativistic / newtonian - 1.0) < 1.0e-8,
        "the correction should be under a part in 1e8, got {:e}",
        relativistic / newtonian - 1.0
    );
}

// -----------------------------------------------------------------------
// The energy-momentum relation: a genuine cross-check.
// -----------------------------------------------------------------------

#[test]
fn energy_and_momentum_satisfy_the_invariant_relation() {
    // `E^2 = (pc)^2 + (mc^2)^2`. Nothing in the crate implements this — it is an identity that
    // `energy.rs` and `momentum.rs` happen to satisfy because both are right. A stray `gamma` in either
    // fails here and nowhere else.
    for beta in [0.1, 0.3, 0.5, 0.7, 0.9, 0.99, 0.999] {
        let v = beta * C;
        let mass = 3.0;

        let e = total_energy(mass, v).expect("subluminal");
        let p = relativistic_momentum(mass, v).expect("subluminal");
        let rest = rest_energy(mass).expect("positive mass");

        let left = e * e;
        let right = (p * C) * (p * C) + rest * rest;

        assert!(
            (left / right - 1.0).abs() < 1.0e-12,
            "beta {beta}: E^2 = {left:e} but (pc)^2 + (mc^2)^2 = {right:e}"
        );
    }
}

// -----------------------------------------------------------------------
// Work: the theorem that survives.
// -----------------------------------------------------------------------

#[test]
fn work_is_the_change_in_kinetic_energy() {
    // The work-energy theorem, unchanged in form from Newtonian mechanics. Worth pinning precisely
    // because the neighbouring module is about a Newtonian law that does *not* survive.
    for (from, to) in [(0.0, 0.5), (0.2, 0.9), (0.9, 0.99), (0.99, 0.5)] {
        let v1 = from * C;
        let v2 = to * C;

        let w = work_done(1.0, v1, v2).expect("subluminal");
        let delta = relativistic_kinetic_energy(1.0, v2).expect("subluminal")
            - relativistic_kinetic_energy(1.0, v1).expect("subluminal");

        assert!(
            (w - delta).abs() / delta.abs().max(1.0) < 1.0e-12,
            "beta {from} -> {to}: work {w:e} vs delta KE {delta:e}"
        );
    }
}

#[test]
fn accelerating_from_rest_to_gamma_two_costs_exactly_the_rest_energy() {
    let w = work_done(1.0, 0.0, 3.0_f64.sqrt() / 2.0 * C).expect("subluminal");
    let rest = rest_energy(1.0).expect("positive mass");

    assert!(
        (w / rest - 1.0).abs() < 1.0e-12,
        "W/mc^2 came out {}",
        w / rest
    );
}

#[test]
fn slowing_a_body_returns_exactly_what_speeding_it_up_cost() {
    let v = 0.9 * C;
    let out = work_done(1.0, 0.0, v).expect("subluminal");
    let back = work_done(1.0, v, 0.0).expect("subluminal");

    assert!(out > 0.0 && back < 0.0, "the signs must be opposite");
    assert!(
        (out + back).abs() / out < 1.0e-12,
        "{out:e} and {back:e} should cancel"
    );
}

#[test]
fn turning_a_body_around_takes_no_net_work() {
    // Energy depends on speed, not direction — so this is exactly zero, not merely small. It is also the
    // reason a transverse force does no work: see the test below.
    for beta in [0.1, 0.5, 0.9] {
        let v = beta * C;
        assert_eq!(
            work_done(1.0, v, -v),
            Some(0.0),
            "beta {beta}: reversing direction must be free"
        );
    }
}

#[test]
fn a_transverse_force_does_no_work_and_a_longitudinal_one_does() {
    // Ties this module to `dynamics`. A transverse force changes direction without changing speed, so
    // gamma is unchanged and no energy is transferred — which is why a synchrotron's bending magnets
    // cost nothing in beam energy while its cavities cost everything.
    let v = 0.9 * C;

    // The transverse case, as the energy module sees it: same speed before and after.
    assert_eq!(work_done(1.0, v, v), Some(0.0));

    // The longitudinal case: any change in speed costs energy.
    let w = work_done(1.0, v, 0.95 * C).expect("subluminal");
    assert!(w > 0.0, "speeding up must cost energy, got {w:e}");
}

#[test]
fn the_work_to_reach_light_speed_grows_without_bound() {
    // The speed limit as economics. The published multiples of mc^2 needed to reach each beta.
    let rest = rest_energy(1.0).expect("positive mass");
    let cases = [
        (0.1, 0.005_038),
        (0.5, 0.154_701),
        (0.9, 1.294_157),
        (0.99, 6.088_802),
        (0.999, 21.366_275),
    ];

    let mut previous = 0.0;

    for (beta, expected_multiple) in cases {
        let w = work_done(1.0, 0.0, beta * C).expect("subluminal") / rest;

        assert!(
            (w / expected_multiple - 1.0).abs() < 1.0e-3,
            "beta {beta}: expected {expected_multiple} mc^2, got {w}"
        );
        assert!(w > previous, "the cost must climb monotonically");
        previous = w;
    }
}

// -----------------------------------------------------------------------
// The inverse.
// -----------------------------------------------------------------------

#[test]
fn one_rest_energy_of_work_takes_a_body_from_rest_to_gamma_two() {
    let v = velocity_after_work(1.0, 0.0, rest_energy(1.0).unwrap()).expect("valid");

    assert!(
        (v / C - 3.0_f64.sqrt() / 2.0).abs() < 1.0e-12,
        "reached beta = {}",
        v / C
    );
}

#[test]
fn the_inverse_uses_a_form_that_survives_tiny_energies() {
    // `beta = sqrt(g(g+2))/(1+g)` rather than the textbook `sqrt(1 - 1/gamma^2)`, which cancels
    // catastrophically for small `g`. At KE = 1e-17 mc^2 the textbook form returns exactly zero.
    let rest = rest_energy(1.0).expect("positive mass");
    let tiny_work = 1.0e-17 * rest;

    let v = velocity_after_work(1.0, 0.0, tiny_work).expect("valid");
    assert!(
        v > 0.0,
        "a tiny but nonzero work must give a tiny but nonzero speed"
    );
    assert!(
        (v / C / 4.472_136e-9 - 1.0).abs() < 0.01,
        "expected beta about 4.47e-9, got {:e}",
        v / C
    );

    // What the textbook form would have produced.
    let gamma = 1.0_f64 + 1.0e-17;
    let textbook = (1.0 - 1.0 / (gamma * gamma)).sqrt();
    assert_eq!(
        textbook, 0.0,
        "the textbook form must underflow, or this test is moot"
    );
}

#[test]
fn no_finite_work_reaches_light_speed() {
    let rest = rest_energy(1.0).expect("positive mass");

    // Up to 1e200 rest-energies. The large end is not decoration: `g(g+2)` overflows above
    // `g = 1.3e154`, and the first version of `velocity_after_work` returned `None` there instead of a
    // speed. That is what the two-branch form in the implementation is for.
    for exponent in [0, 6, 20, 100, 200] {
        let work = 10.0_f64.powi(exponent) * rest;
        let v = velocity_after_work(1.0, 0.0, work)
            .unwrap_or_else(|| panic!("W = 1e{exponent} mc^2 must have an answer"));

        assert!(v < C, "W = 1e{exponent} mc^2 gave {}c", v / C);
        assert!(
            lorentz_factor(v).is_some(),
            "and the result must stay usable by the rest of the sub-layer"
        );
    }
}

#[test]
fn the_two_branches_of_the_inverse_agree_where_they_meet() {
    // `velocity_after_work` switches formula at `g = 1`, i.e. one rest energy of work. A discontinuity
    // there would be invisible to every other test — both sides are individually plausible — so this
    // walks across the boundary and asserts the result stays smooth and monotonic.
    let rest = rest_energy(1.0).expect("positive mass");
    let mut previous = 0.0;

    for step in 0..=40 {
        let g = 0.8 + 0.01 * f64::from(step);
        let v = velocity_after_work(1.0, 0.0, g * rest).expect("valid");

        assert!(
            v > previous,
            "g = {g}: speed must increase monotonically, got {v}"
        );
        assert!(v < C, "g = {g}: got {}c", v / C);
        previous = v;
    }

    // And exactly at the boundary the answer is the closed-form beta = sqrt(3)/2.
    let at_one = velocity_after_work(1.0, 0.0, rest).expect("valid");
    assert!(
        (at_one / C - 3.0_f64.sqrt() / 2.0).abs() < 1.0e-12,
        "at g = 1 exactly, got beta = {}",
        at_one / C
    );
}

#[test]
fn removing_more_energy_than_a_body_has_is_not_a_question_with_an_answer() {
    // Not clamped to zero: a body at rest cannot have work taken out of it, and returning `0.0` would
    // report a successful impossible operation.
    assert_eq!(velocity_after_work(1.0, 0.0, -1.0), None);

    // But removing exactly all of it is fine, and lands at rest.
    let ke = relativistic_kinetic_energy(1.0, 0.5 * C).expect("subluminal");
    assert_eq!(velocity_after_work(1.0, 0.5 * C, -ke), Some(0.0));

    // And removing slightly more is not.
    assert_eq!(velocity_after_work(1.0, 0.5 * C, -ke * 1.000_001), None);
}

#[test]
fn the_inverse_returns_a_speed_not_a_velocity() {
    // Energy is a scalar and cannot say which way the body ended up going, so the result is always
    // non-negative and the caller keeps track of direction. Documented, and pinned so it is not mistaken
    // for a sign bug.
    let w = work_done(1.0, -0.9 * C, -0.5 * C).expect("subluminal");
    assert!(w < 0.0, "slowing down, so the work is negative");

    let v = velocity_after_work(1.0, -0.9 * C, w).expect("valid");
    assert!(
        v > 0.0,
        "the answer is a speed, so it comes back positive: {v}"
    );
    assert!(
        (v / (0.5 * C) - 1.0).abs() < 1.0e-9,
        "and it is the right magnitude"
    );
}

// -----------------------------------------------------------------------
// Domain.
// -----------------------------------------------------------------------

#[test]
fn a_massless_body_is_rejected_by_every_function() {
    for mass in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(rest_energy(mass), None, "mass {mass}");
        assert_eq!(
            relativistic_kinetic_energy(mass, 0.5 * C),
            None,
            "mass {mass}"
        );
        assert_eq!(total_energy(mass, 0.5 * C), None, "mass {mass}");
        assert_eq!(work_done(mass, 0.0, 0.5 * C), None, "mass {mass}");
        assert_eq!(velocity_after_work(mass, 0.0, 1.0), None, "mass {mass}");
    }
}

#[test]
fn superluminal_and_non_finite_inputs_have_no_answer() {
    for bad in [C, 2.0 * C, f64::NAN, f64::INFINITY] {
        assert_eq!(
            relativistic_kinetic_energy(1.0, bad),
            None,
            "velocity {bad}"
        );
        assert_eq!(total_energy(1.0, bad), None, "velocity {bad}");
        assert_eq!(work_done(1.0, 0.0, bad), None, "final velocity {bad}");
        assert_eq!(work_done(1.0, bad, 0.0), None, "initial velocity {bad}");
        assert_eq!(
            velocity_after_work(1.0, bad, 1.0),
            None,
            "initial velocity {bad}"
        );
    }

    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(velocity_after_work(1.0, 0.0, bad), None, "work {bad}");
    }
}

#[test]
fn a_body_at_rest_has_no_kinetic_energy_and_needs_no_work_to_stay_there() {
    assert_eq!(relativistic_kinetic_energy(5.0, 0.0), Some(0.0));
    assert_eq!(work_done(5.0, 0.0, 0.0), Some(0.0));
    assert_eq!(velocity_after_work(5.0, 0.0, 0.0), Some(0.0));
    assert_eq!(total_energy(5.0, 0.0), rest_energy(5.0));
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// `E² = (pc)² + (mc²)²` at every mass and speed.
    ///
    /// The property form of the cross-check above. This identity is implemented nowhere, so it is a real
    /// constraint linking `energy.rs` and `momentum.rs` rather than a restatement of either.
    #[test]
    fn the_energy_momentum_relation_holds_everywhere(
        mass in 1.0e-6_f64..1.0e6,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let e = total_energy(mass, v).expect("subluminal");
        let p = relativistic_momentum(mass, v).expect("subluminal");
        let rest = rest_energy(mass).expect("positive mass");

        let left = e * e;
        let right = (p * C) * (p * C) + rest * rest;

        prop_assert!(
            (left / right - 1.0).abs() < 1.0e-9,
            "E^2 {:e} vs (pc)^2 + (mc^2)^2 {:e}", left, right
        );
    }

    /// Work and the inverse round-trip.
    #[test]
    fn work_and_velocity_round_trip(
        mass in 1.0e-3_f64..1.0e3,
        from in 0.0_f64..0.99,
        to in 0.0_f64..0.99,
    ) {
        prop_assume!((from - to).abs() > 1.0e-4);

        let v1 = from * C;
        let v2 = to * C;

        let w = work_done(mass, v1, v2).expect("subluminal");
        let reached = velocity_after_work(mass, v1, w).expect("valid");

        prop_assert!(
            (reached - v2).abs() / C < 1.0e-9,
            "{}c -> W {:e} -> {}c, expected {}c", from, w, reached / C, to
        );
    }

    /// Kinetic energy is non-negative, zero only at rest, and monotonic in speed.
    #[test]
    fn kinetic_energy_climbs_with_speed(a in 0.0_f64..0.99, b in 0.0_f64..0.99) {
        prop_assume!((a - b).abs() > 1.0e-6);

        let (slower, faster) = if a < b { (a, b) } else { (b, a) };
        let low = relativistic_kinetic_energy(1.0, slower * C).expect("subluminal");
        let high = relativistic_kinetic_energy(1.0, faster * C).expect("subluminal");

        prop_assert!(low >= 0.0, "negative kinetic energy {low}");
        prop_assert!(high > low, "beta {faster} gave {high:e}, beta {slower} gave {low:e}");
    }

    /// Kinetic energy always exceeds the Newtonian `½mv²`, never falls short.
    #[test]
    fn kinetic_energy_is_never_below_the_newtonian_value(
        mass in 1.0e-3_f64..1.0e3,
        beta in -0.999_f64..0.999,
    ) {
        let v = beta * C;
        let relativistic = relativistic_kinetic_energy(mass, v).expect("subluminal");
        let newtonian = 0.5 * mass * v * v;

        prop_assert!(
            relativistic >= newtonian * (1.0 - 1.0e-12),
            "{:e} is below {:e}", relativistic, newtonian
        );
    }

    /// Energy is even in velocity: direction never matters.
    #[test]
    fn energy_depends_only_on_speed(mass in 1.0e-3_f64..1.0e3, beta in 0.0_f64..0.999) {
        let v = beta * C;

        prop_assert_eq!(
            relativistic_kinetic_energy(mass, v),
            relativistic_kinetic_energy(mass, -v)
        );
        prop_assert_eq!(total_energy(mass, v), total_energy(mass, -v));
        prop_assert_eq!(work_done(mass, v, -v), Some(0.0));
    }

    /// Every energy is linear in mass.
    #[test]
    fn energies_are_linear_in_mass(mass in 1.0e-3_f64..1.0e3, beta in -0.99_f64..0.99) {
        let v = beta * C;

        let single = relativistic_kinetic_energy(mass, v).expect("subluminal");
        let double = relativistic_kinetic_energy(2.0 * mass, v).expect("subluminal");
        prop_assume!(single > 0.0);

        prop_assert!((double / (2.0 * single) - 1.0).abs() < 1.0e-12);
        prop_assert_eq!(
            rest_energy(2.0 * mass).unwrap(),
            2.0 * rest_energy(mass).unwrap()
        );
    }

    /// At everyday speeds kinetic energy is `½mv²` to a part in `1e7`.
    ///
    /// The bound is `γ − 1` at the top of the range: at `1e5 m/s` that is `5.6e-8`. Stated rather than
    /// eyeballed, after two tolerances in this sub-layer were first written `1000×` too tight.
    #[test]
    fn slow_kinetic_energy_is_newtonian(mass in 1.0e-3_f64..1.0e3, v in -1.0e5_f64..1.0e5) {
        prop_assume!(v.abs() > 1.0);

        let relativistic = relativistic_kinetic_energy(mass, v).expect("subluminal");
        let newtonian = 0.5 * mass * v * v;

        prop_assert!(
            (relativistic - newtonian).abs() / newtonian < 1.0e-7,
            "{:e} vs {:e}", relativistic, newtonian
        );
    }

    /// The inverse never produces a superluminal speed, for any finite work.
    #[test]
    fn the_inverse_is_always_subluminal(
        mass in 1.0e-6_f64..1.0e6,
        exponent in -20.0_f64..40.0,
    ) {
        let work = 10.0_f64.powf(exponent);
        let v = velocity_after_work(mass, 0.0, work).expect("non-negative work");

        prop_assert!(v < C, "got {}c", v / C);
        prop_assert!(v >= 0.0, "and a speed is never negative, got {v}");
        prop_assert!(lorentz_factor(v).is_some(), "the sub-layer rejects {v}");
    }
}
