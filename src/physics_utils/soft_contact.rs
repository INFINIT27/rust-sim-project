//! Soft contact: two bodies pressed together, resolved as a force over the time they touch.
//!
//! The collision solvers in [`super::conservation_laws`] treat a contact as instantaneous: an
//! impulse that sets the outgoing velocities in one step. That is right when the contact is far
//! shorter than anything else in the scene. When the contact itself is what is being studied (two
//! planets deforming and flowing into each other over minutes), it has to be resolved as a force,
//! step by step, and these are the laws for that: the **linear spring-dashpot** of the discrete
//! element method (Cundall & Strack 1979, Géotechnique 29:47).
//!
//! ```text
//! F = max(0,  k·δ − c·v_n)        δ overlap, v_n the normal relative speed (negative closing)
//! k = E* · r*                      E* the contact modulus, r* = r₁r₂/(r₁+r₂)
//! c = 2·ζ·√(m*·k)                  m* = m₁m₂/(m₁+m₂), ζ from the restitution
//! ```
//!
//! The force never pulls: two bodies held together are held by gravity, not by the contact.
//!
//! # Why these forms
//!
//! - **The stiffness `k = E*·r*`** is the linear stand-in for Hertz's law, whose stiffness at a
//!   given overlap is `E*·√(r*·δ)`. Dimensionally it is the only stiffness a modulus and a size
//!   make, and it keeps the contact time proportional to the size, `t ∝ r·√(ρ/E)`: the time a
//!   sound wave takes to cross the body, which is what a contact physically lasts.
//! - **The damping from the restitution.** A linear spring-dashpot returns the same fraction of the
//!   approach speed whatever the speed, so `ζ` can be set from the material's restitution and one
//!   number means the same in a simulated contact as in an instant one. Because the force never
//!   pulls, the relation is Schwager & Pöschel's (2007), not the textbook one
//!   ([`restitution_for_damping_ratio`]): critical damping still returns `13.5%`, and a fully
//!   plastic contact, `e = 0`, would need infinite damping, so a simulation caps it.

/// The stiffness of a linear contact between two spheres, in N/m.
///
/// `k = E*·r*`, with `r* = r₁r₂/(r₁ + r₂)` the reduced radius. `E*` is the contact's modulus,
/// already combined from the two materials (see
/// [`Material::combined_with`](crate::core::scene::Material::combined_with)).
///
/// # Parameters
///
/// - `contact_modulus_pa` — `E*`, Pa. Finite and positive.
/// - `radius_a_m`, `radius_b_m` — radii, m. Finite and positive.
///
/// # Returns
///
/// `k` in N/m. In release, invalid inputs return `0.0`: no force.
///
/// # Examples
///
/// ```
/// use physics_sandbox::contact_stiffness;
///
/// // Two equal 1 m spheres at 1 GPa: r* = 0.5 m, so 5e8 N/m.
/// assert_eq!(contact_stiffness(1.0e9, 1.0, 1.0), 5.0e8);
/// ```
#[inline]
pub fn contact_stiffness(contact_modulus_pa: f64, radius_a_m: f64, radius_b_m: f64) -> f64 {
    let valid = contact_modulus_pa.is_finite()
        && contact_modulus_pa > 0.0
        && radius_a_m.is_finite()
        && radius_a_m > 0.0
        && radius_b_m.is_finite()
        && radius_b_m > 0.0;
    debug_assert!(
        valid,
        "contact_stiffness: need a positive modulus and radii, got E = {contact_modulus_pa}, r = ({radius_a_m}, {radius_b_m})"
    );
    if !valid {
        return 0.0;
    }
    contact_modulus_pa * radius_a_m * radius_b_m / (radius_a_m + radius_b_m)
}

/// The restitution of a linear spring-dashpot contact that never pulls, for damping ratio `ζ`.
/// Dimensionless, in `(0, 1]`.
///
/// The textbook `e = exp(−ζπ/√(1 − ζ²))` assumes the contact lasts until the overlap returns to
/// zero, which means the dashpot pulls the bodies back together as they part. A contact that only
/// pushes ([`spring_dashpot_force`]) ends earlier, when the force falls to zero, and returns more.
/// Schwager & Pöschel (2007, Phys. Rev. E 75:051304) solved it exactly; in units of the natural
/// frequency, with the contact ending at `t` where `k·x + c·ẋ = 0`:
///
/// ```text
/// ζ < 1/√2:   ω_d·t = π − atan(2ζ√(1−ζ²) / (1−2ζ²))       e = exp(−ζt)·(ζ/√(1−ζ²)·sin ω_d t − cos ω_d t)
/// 1/√2 ≤ ζ < 1: ω_d·t = atan(2ζ√(1−ζ²) / (2ζ²−1))          (the same e)
/// ζ = 1:      t = 2                                       e = exp(−2) = 0.135
/// ζ > 1:      overdamped, the same condition on the two decaying exponentials
/// ```
///
/// So critical damping does **not** stop a pair dead: it returns `13.5%`. Restitutions below that
/// take an overdamped contact, and `e = 0` would take infinite damping.
///
/// # Examples
///
/// ```
/// use physics_sandbox::restitution_for_damping_ratio;
///
/// assert_eq!(restitution_for_damping_ratio(0.0), 1.0);
/// assert!((restitution_for_damping_ratio(1.0) - (-2.0_f64).exp()).abs() < 1e-9);
/// assert!(restitution_for_damping_ratio(3.0) < 0.03, "overdamped: nearly plastic");
/// ```
pub fn restitution_for_damping_ratio(damping_ratio: f64) -> f64 {
    let zeta = damping_ratio;
    if zeta.is_nan() || zeta <= 0.0 {
        return 1.0;
    }
    if !zeta.is_finite() {
        return 0.0;
    }
    if (zeta - 1.0).abs() < 1.0e-9 {
        return (-2.0_f64).exp();
    }
    if zeta < 1.0 {
        let root = (1.0 - zeta * zeta).sqrt();
        let y = 2.0 * zeta * root;
        let x = 1.0 - 2.0 * zeta * zeta;
        // The first zero of the force after the deepest compression: in (π/2, π) for light
        // damping, (0, π/2) past ζ = 1/√2.
        let phase = if x > 0.0 {
            std::f64::consts::PI - (y / x).atan()
        } else if x < 0.0 {
            (y / -x).atan()
        } else {
            std::f64::consts::FRAC_PI_2
        };
        let time = phase / root;
        return (-zeta * time).exp() * (zeta / root * phase.sin() - phase.cos());
    }
    // Overdamped: x ∝ e^{r₁t} − e^{r₂t}, and the force k·x + c·ẋ vanishes where
    // e^{(r₁−r₂)t} = (1 + 2ζr₂)/(1 + 2ζr₁).
    let s = (zeta * zeta - 1.0).sqrt();
    let (r1, r2) = (-zeta + s, -zeta - s);
    let ratio = (1.0 + 2.0 * zeta * r2) / (1.0 + 2.0 * zeta * r1);
    if ratio.is_nan() || ratio <= 0.0 {
        return 0.0;
    }
    let time = ratio.ln() / (r1 - r2);
    let speed = (r1 * (r1 * time).exp() - r2 * (r2 * time).exp()) / (r1 - r2);
    (-speed).clamp(0.0, 1.0)
}

/// The damping ratio `ζ` of a linear spring-dashpot contact that never pulls and returns
/// `restitution` of the approach speed. Dimensionless, `≥ 0`.
///
/// The inverse of [`restitution_for_damping_ratio`], found by bisection (the restitution falls
/// steadily as the damping grows). `e = 1` is no damping; `e = exp(−2) ≈ 0.135` is critical
/// damping; below it the contact is overdamped, and `e = 0` returns `f64::INFINITY`, which a caller
/// integrating in finite steps has to cap.
///
/// # Returns
///
/// `ζ`. In release, a restitution outside `[0, 1]` returns `f64::INFINITY`, the plastic end.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{damping_ratio_for_restitution, restitution_for_damping_ratio};
///
/// assert_eq!(damping_ratio_for_restitution(1.0), 0.0);
/// assert_eq!(damping_ratio_for_restitution(0.0), f64::INFINITY);
/// let zeta = damping_ratio_for_restitution(0.5);
/// assert!((restitution_for_damping_ratio(zeta) - 0.5).abs() < 1e-9);
/// ```
pub fn damping_ratio_for_restitution(restitution: f64) -> f64 {
    debug_assert!(
        (0.0..=1.0).contains(&restitution),
        "damping_ratio_for_restitution: restitution must be in [0, 1], got {restitution}"
    );
    if !(0.0..=1.0).contains(&restitution) || restitution == 0.0 {
        return f64::INFINITY;
    }
    if restitution == 1.0 {
        return 0.0;
    }
    let (mut light, mut heavy) = (0.0_f64, 1.0_f64);
    while restitution_for_damping_ratio(heavy) > restitution {
        heavy *= 2.0;
        if heavy > 1.0e9 {
            return f64::INFINITY;
        }
    }
    for _ in 0..200 {
        let middle = 0.5 * (light + heavy);
        if restitution_for_damping_ratio(middle) > restitution {
            light = middle;
        } else {
            heavy = middle;
        }
        if heavy - light <= 1.0e-12 * heavy {
            break;
        }
    }
    0.5 * (light + heavy)
}

/// How long a linear spring contact of stiffness `k` between bodies of reduced mass `m*` lasts with
/// no damping, in seconds: half a period, `π·√(m*/k)`.
///
/// The time scale a simulation must resolve to follow the contact: a step a small fraction of
/// this. Damping only lengthens a real contact, so this is the shortest it can be.
///
/// # Returns
///
/// The time, s. In release, invalid inputs return `f64::INFINITY`: nothing to resolve.
///
/// # Examples
///
/// ```
/// use physics_sandbox::contact_duration;
///
/// // 1 kg on 1 N/m: π seconds.
/// assert_eq!(contact_duration(1.0, 1.0), std::f64::consts::PI);
/// ```
#[inline]
pub fn contact_duration(reduced_mass_kg: f64, stiffness_n_m: f64) -> f64 {
    let valid = reduced_mass_kg.is_finite()
        && reduced_mass_kg > 0.0
        && stiffness_n_m.is_finite()
        && stiffness_n_m > 0.0;
    debug_assert!(
        valid,
        "contact_duration: need a positive mass and stiffness, got m = {reduced_mass_kg}, k = {stiffness_n_m}"
    );
    if !valid {
        return f64::INFINITY;
    }
    std::f64::consts::PI * (reduced_mass_kg / stiffness_n_m).sqrt()
}

/// The normal force of a spring-dashpot contact, in newtons, pushing the bodies apart.
///
/// `F = max(0, k·δ − c·v_n)`. `v_n` is the relative speed along the normal from the first body to
/// the second, negative while they close, so closing adds to the spring and separating takes from
/// it. Clamped at zero: a contact pushes and never pulls, which is what lets a critically damped
/// pair come apart at rest instead of sticking.
///
/// # Parameters
///
/// - `stiffness_n_m` — `k`, from [`contact_stiffness`]. `≥ 0`.
/// - `damping_n_s_m` — `c = 2·ζ·√(m*·k)`. `≥ 0`.
/// - `overlap_m` — `δ`, how far the surfaces interpenetrate. `≥ 0`.
/// - `normal_speed_m_s` — `v_n`.
///
/// # Returns
///
/// The force, N, `≥ 0`. Non-finite inputs return `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::spring_dashpot_force;
///
/// // Closing at 2 m/s adds to the spring: 10·0.1 + 3·2 = 7 N.
/// assert_eq!(spring_dashpot_force(10.0, 3.0, 0.1, -2.0), 7.0);
/// // Separating fast enough, the dashpot would pull; the contact lets go instead.
/// assert_eq!(spring_dashpot_force(10.0, 3.0, 0.1, 2.0), 0.0);
/// ```
#[inline]
pub fn spring_dashpot_force(
    stiffness_n_m: f64,
    damping_n_s_m: f64,
    overlap_m: f64,
    normal_speed_m_s: f64,
) -> f64 {
    let force = stiffness_n_m * overlap_m - damping_n_s_m * normal_speed_m_s;
    if force.is_finite() && force > 0.0 {
        force
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "../../tst/physics_utils/soft_contact_tests.rs"]
mod tests;
