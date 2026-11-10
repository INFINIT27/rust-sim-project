//! Unit and property tests for [`crate::physics_utils::relativity::rapidity`].
//!
//! Compiled into `src/physics_utils/relativity/rapidity.rs` via `#[path]`.
//!
//! # The claims worth the most here
//!
//! 1. **Rapidities add where velocities do not.** Checked against `compose_velocities`, which is
//!    written from the rational formula and knows nothing about `tanh`.
//! 2. **Hyperbolic motion is consistent with the rest of the sub-layer.** The traveller's clock is
//!    checked by integrating `dτ = dt/γ` numerically with `lorentz_factor`, and the distance by
//!    integrating the velocity, so neither closed form is checked against itself.
//! 3. **The Newtonian limit is `v = at` and `x = ½at²`**, to the precision `f64` has, and the
//!    distance is not zero for a slow body.

use super::*;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::physics_utils::relativity::lorentz::lorentz_factor;
use crate::physics_utils::relativity::velocity::compose_velocities;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const REGRESSION_SEEDS: &str =
    "tst/physics_utils/proptest-regressions/relativity_rapidity_tests.txt";

const C: f64 = SPEED_OF_LIGHT_M_S;
const YEAR_S: f64 = 31_557_600.0;
const LIGHT_YEAR_M: f64 = C * YEAR_S;

// -----------------------------------------------------------------------
// Rapidity.
// -----------------------------------------------------------------------

#[test]
fn rapidity_is_zero_at_rest_and_odd_in_the_velocity() {
    assert_eq!(rapidity(0.0), Some(0.0));
    let forward = rapidity(0.3 * C).expect("subluminal");
    let backward = rapidity(-0.3 * C).expect("subluminal");
    assert!(forward > 0.0 && backward < 0.0);
    // Not `assert_eq!`: the platform's `atanh` is odd only to within a rounding.
    assert!((forward + backward).abs() < 1.0e-15);
}

#[test]
fn rapidity_at_the_pythagorean_speeds_is_a_logarithm() {
    // tanh(phi) = beta means e^phi = sqrt((1+beta)/(1-beta)): 2 at beta = 3/5, 3 at beta = 4/5.
    assert!((rapidity(0.6 * C).expect("subluminal") - 2.0_f64.ln()).abs() < 1.0e-14);
    assert!((rapidity(0.8 * C).expect("subluminal") - 3.0_f64.ln()).abs() < 1.0e-14);
}

#[test]
fn gamma_and_gamma_beta_are_cosh_and_sinh() {
    for beta in [0.1, 0.5, 0.9, 0.999] {
        let v = beta * C;
        let phi = rapidity(v).expect("subluminal");
        let gamma = lorentz_factor(v).expect("subluminal");
        assert!((phi.cosh() / gamma - 1.0).abs() < 1.0e-12, "beta {beta}");
        assert!(
            (phi.sinh() / (gamma * beta) - 1.0).abs() < 1.0e-12,
            "beta {beta}"
        );
    }
}

#[test]
fn at_low_speed_rapidity_is_beta() {
    // phi = beta + beta^3/3 + ...; at Earth's orbital speed the correction is 3e-9 relative.
    let beta = 29_780.0 / C;
    let phi = rapidity(29_780.0).expect("subluminal");
    assert!((phi / beta - 1.0 - beta * beta / 3.0).abs() < 1.0e-15);
}

#[test]
fn light_speed_has_no_rapidity() {
    assert_eq!(rapidity(C), None);
    assert_eq!(rapidity(-C), None);
    assert_eq!(rapidity(1.5 * C), None);
    assert_eq!(rapidity(f64::NAN), None);
    assert_eq!(velocity_from_rapidity(f64::NAN), None);
    assert_eq!(velocity_from_rapidity(f64::INFINITY), None);
}

#[test]
fn a_huge_rapidity_saturates_below_light_speed() {
    // tanh rounds to 1.0 above about 19. The result must still be a speed a massive body can have.
    for phi in [19.0, 50.0, 1.0e3, 1.0e300] {
        let v = velocity_from_rapidity(phi).expect("finite rapidity");
        assert!(v < C, "rapidity {phi} gave exactly c");
        assert!(lorentz_factor(v).is_some(), "and gamma must exist for it");

        let back = velocity_from_rapidity(-phi).expect("finite rapidity");
        assert_eq!(back, -v);
    }
}

// -----------------------------------------------------------------------
// Hyperbolic motion: published figures for a 1 g ship.
// -----------------------------------------------------------------------

#[test]
fn one_g_for_one_ship_year() {
    // The standard figures: 1.19 years on Earth, 0.56 light-years, 0.77c.
    let earth_time =
        coordinate_time_under_proper_acceleration(STANDARD_GRAVITY_M_S2, YEAR_S).expect("valid");
    let distance =
        distance_under_proper_acceleration(STANDARD_GRAVITY_M_S2, earth_time).expect("valid");
    let velocity =
        velocity_under_proper_acceleration(STANDARD_GRAVITY_M_S2, earth_time).expect("valid");

    assert!(
        (earth_time / YEAR_S - 1.187).abs() < 0.001,
        "{}",
        earth_time / YEAR_S
    );
    assert!(
        (distance / LIGHT_YEAR_M - 0.564).abs() < 0.001,
        "{}",
        distance / LIGHT_YEAR_M
    );
    assert!((velocity / C - 0.775).abs() < 0.001, "{}", velocity / C);
}

#[test]
fn one_g_for_five_ship_years_is_most_of_a_century() {
    let earth_time = coordinate_time_under_proper_acceleration(STANDARD_GRAVITY_M_S2, 5.0 * YEAR_S)
        .expect("valid");
    let distance =
        distance_under_proper_acceleration(STANDARD_GRAVITY_M_S2, earth_time).expect("valid");

    assert!(
        (earth_time / YEAR_S - 84.5).abs() < 0.1,
        "{}",
        earth_time / YEAR_S
    );
    assert!(
        (distance / LIGHT_YEAR_M - 83.5).abs() < 0.1,
        "{}",
        distance / LIGHT_YEAR_M
    );
}

#[test]
fn newton_would_have_reached_light_speed_and_this_has_not() {
    // At t = c/a, v = at is exactly c. The relativistic answer is c/sqrt(2).
    let time = C / STANDARD_GRAVITY_M_S2;
    assert!(
        (time / 86_400.0 - 353.8).abs() < 0.1,
        "about 354 days at 1 g"
    );

    let velocity = velocity_under_proper_acceleration(STANDARD_GRAVITY_M_S2, time).expect("valid");
    assert!((velocity / C - std::f64::consts::FRAC_1_SQRT_2).abs() < 1.0e-14);
}

// -----------------------------------------------------------------------
// Limits and domain.
// -----------------------------------------------------------------------

#[test]
fn everyday_acceleration_is_newtonian() {
    // A car: 3 m/s^2 for 10 s. k = 1e-7, so the corrections are of order 1e-14 relative.
    let velocity = velocity_under_proper_acceleration(3.0, 10.0).expect("valid");
    let distance = distance_under_proper_acceleration(3.0, 10.0).expect("valid");
    let proper = proper_time_under_proper_acceleration(3.0, 10.0).expect("valid");

    assert!((velocity / 30.0 - 1.0).abs() < 1.0e-13);
    assert!((distance / 150.0 - 1.0).abs() < 1.0e-13);
    assert!((proper / 10.0 - 1.0).abs() < 1.0e-13);
}

#[test]
fn the_distance_survives_where_the_textbook_form_does_not() {
    // (c^2/a)(sqrt(1 + k^2) - 1) with k = 1e-9: 1 + k^2 rounds to exactly 1 and the textbook form
    // returns zero. A body that accelerates at 0.3 m/s^2 for a second has moved 15 cm.
    let acceleration: f64 = 0.3;
    let time: f64 = 1.0;
    let k = acceleration * time / C;
    let textbook = C * C / acceleration * ((1.0 + k * k).sqrt() - 1.0);
    let stable = distance_under_proper_acceleration(acceleration, time).expect("valid");

    assert_eq!(textbook, 0.0, "the form this function avoids");
    assert!((stable / 0.15 - 1.0).abs() < 1.0e-14, "got {stable}");
}

#[test]
fn no_acceleration_is_no_motion() {
    assert_eq!(velocity_under_proper_acceleration(0.0, 1.0e9), Some(0.0));
    assert_eq!(distance_under_proper_acceleration(0.0, 1.0e9), Some(0.0));
    assert_eq!(
        proper_time_under_proper_acceleration(0.0, 1.0e9),
        Some(1.0e9)
    );
    assert_eq!(
        coordinate_time_under_proper_acceleration(0.0, 1.0e9),
        Some(1.0e9)
    );
    assert_eq!(velocity_under_proper_acceleration(9.8, 0.0), Some(0.0));
}

#[test]
fn deceleration_mirrors_acceleration() {
    let time = 0.7 * YEAR_S;
    assert_eq!(
        velocity_under_proper_acceleration(-9.8, time),
        velocity_under_proper_acceleration(9.8, time).map(|v| -v)
    );
    assert_eq!(
        distance_under_proper_acceleration(-9.8, time),
        distance_under_proper_acceleration(9.8, time).map(|x| -x)
    );
    assert_eq!(
        proper_time_under_proper_acceleration(-9.8, time),
        proper_time_under_proper_acceleration(9.8, time)
    );
}

#[test]
fn negative_time_and_non_finite_inputs_have_no_answer() {
    assert_eq!(velocity_under_proper_acceleration(9.8, -1.0), None);
    assert_eq!(distance_under_proper_acceleration(9.8, -1.0), None);
    assert_eq!(proper_time_under_proper_acceleration(9.8, -1.0), None);
    assert_eq!(coordinate_time_under_proper_acceleration(9.8, -1.0), None);

    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(velocity_under_proper_acceleration(bad, 1.0), None);
        assert_eq!(velocity_under_proper_acceleration(1.0, bad), None);
        assert_eq!(distance_under_proper_acceleration(bad, 1.0), None);
        assert_eq!(proper_time_under_proper_acceleration(1.0, bad), None);
        assert_eq!(coordinate_time_under_proper_acceleration(bad, 1.0), None);
    }
}

#[test]
fn an_enormous_acceleration_saturates_instead_of_reaching_light_speed() {
    let velocity = velocity_under_proper_acceleration(1.0e30, 1.0e30).expect("finite inputs");
    assert!(velocity < C);
    assert!(lorentz_factor(velocity).is_some());

    // The ship's clock overflowing sinh is reported, not returned as infinity.
    assert_eq!(coordinate_time_under_proper_acceleration(9.8, 1.0e12), None);
}

// -----------------------------------------------------------------------
// The closed forms against numerical integration.
// -----------------------------------------------------------------------

/// Simpson's rule on `[0, end]` with an even number of steps.
fn integrate(end: f64, steps: usize, f: impl Fn(f64) -> f64) -> f64 {
    let h = end / steps as f64;
    let mut sum = f(0.0) + f(end);
    for i in 1..steps {
        let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
        sum += weight * f(i as f64 * h);
    }
    sum * h / 3.0
}

#[test]
fn the_distance_is_the_integral_of_the_velocity() {
    let end = 3.0 * YEAR_S;
    let integral = integrate(end, 20_000, |t| {
        velocity_under_proper_acceleration(STANDARD_GRAVITY_M_S2, t).expect("valid")
    });
    let closed = distance_under_proper_acceleration(STANDARD_GRAVITY_M_S2, end).expect("valid");

    assert!(
        (integral / closed - 1.0).abs() < 1.0e-10,
        "integral {integral:e} against closed form {closed:e}"
    );
}

#[test]
fn the_ship_clock_is_the_integral_of_one_over_gamma() {
    // d(tau) = dt / gamma, with gamma from the module that knows nothing about asinh.
    let end = 3.0 * YEAR_S;
    let integral = integrate(end, 20_000, |t| {
        let v = velocity_under_proper_acceleration(STANDARD_GRAVITY_M_S2, t).expect("valid");
        1.0 / lorentz_factor(v).expect("subluminal")
    });
    let closed = proper_time_under_proper_acceleration(STANDARD_GRAVITY_M_S2, end).expect("valid");

    assert!(
        (integral / closed - 1.0).abs() < 1.0e-10,
        "integral {integral:e} against closed form {closed:e}"
    );
}

// -----------------------------------------------------------------------
// Properties.
// -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// **Rapidities add.** `velocity_from_rapidity(φ₁ + φ₂)` is the relativistic sum of the two
    /// velocities. Compared in rapidity, where a relative tolerance means the same thing at every
    /// speed.
    #[test]
    fn rapidities_add(
        first in -0.99_f64..0.99,
        second in -0.99_f64..0.99,
    ) {
        let sum = rapidity(first * C).expect("subluminal") + rapidity(second * C).expect("subluminal");
        let composed = compose_velocities(first * C, second * C).expect("valid");
        let composed_rapidity = rapidity(composed).expect("subluminal");

        prop_assert!(
            (composed_rapidity - sum).abs() < 1.0e-11,
            "{} + {} gave rapidity {} against {}", first, second, composed_rapidity, sum
        );
    }

    /// Velocity to rapidity and back.
    #[test]
    fn rapidity_round_trips(beta in -0.999_f64..0.999) {
        let phi = rapidity(beta * C).expect("subluminal");
        let back = velocity_from_rapidity(phi).expect("finite");
        // d(beta)/d(phi) = 1 - beta^2 <= 1, so an error in phi is not amplified.
        prop_assert!((back / C - beta).abs() < 1.0e-14);
    }

    /// The velocity never reaches `c`, whatever the acceleration and however long it lasts.
    #[test]
    fn the_velocity_stays_below_light_speed(
        acceleration in -1.0e6_f64..1.0e6,
        time in 0.0_f64..1.0e12,
    ) {
        let velocity = velocity_under_proper_acceleration(acceleration, time).expect("valid");
        prop_assert!(velocity.abs() < C);

        // And the distance stays inside the light cone.
        let distance = distance_under_proper_acceleration(acceleration, time).expect("valid");
        prop_assert!(distance.abs() <= C * time);
    }

    /// The traveller's clock never reads more than the stay-at-home's, and the two conversions
    /// between them are inverses.
    #[test]
    fn the_two_clocks_convert_both_ways(
        acceleration in 0.01_f64..100.0,
        years in 0.0_f64..5.0,
    ) {
        let time = years * YEAR_S;
        let proper = proper_time_under_proper_acceleration(acceleration, time).expect("valid");
        prop_assert!(proper <= time);

        let back = coordinate_time_under_proper_acceleration(acceleration, proper).expect("valid");
        // sinh amplifies an error in tau by cosh(phi)*phi/sinh(phi) ~ phi, at most ~50 here.
        prop_assert!((back - time).abs() <= 1.0e-12 * time, "{} came back as {}", time, back);
    }

    /// The rapidity of a body under constant proper acceleration is `a·τ/c`: linear in its own
    /// clock. This ties the two halves of the module together.
    #[test]
    fn rapidity_grows_linearly_with_the_ship_clock(
        acceleration in 0.1_f64..100.0,
        years in 0.01_f64..3.0,
    ) {
        let time = years * YEAR_S;
        let velocity = velocity_under_proper_acceleration(acceleration, time).expect("valid");
        let proper = proper_time_under_proper_acceleration(acceleration, time).expect("valid");

        let phi = rapidity(velocity).expect("subluminal");
        let expected = acceleration * proper / C;
        // atanh amplifies the velocity's rounding by gamma^2, up to ~1e4 at the top of this range.
        prop_assert!(
            (phi - expected).abs() <= 1.0e-9 * expected.max(1.0),
            "phi {} against a*tau/c {}", phi, expected
        );
    }
}
