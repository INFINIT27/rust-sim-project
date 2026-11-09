//! Conservation of momentum and energy, and the collision solvers that follow
//! from them.
//!
//! Scalars along a single caller-chosen axis, in SI units, with sign carrying
//! direction — the convention [`super::newtons_laws`] uses. A body is a
//! `(mass_kg, velocity_m_s)` pair, and a system is a slice of them.
//!
//! # Structure
//!
//! - **Quantities** — [`momentum`], [`total_momentum`], [`kinetic_energy`],
//!   [`total_kinetic_energy`], [`gravitational_potential_energy`],
//!   [`total_mechanical_energy`].
//! - **Conservation checks** — [`momentum_is_conserved`],
//!   [`kinetic_energy_is_conserved`], [`energy_is_conserved`]. These exist so
//!   the float-tolerance question is answered once here rather than re-invented
//!   at every call site, and so a future `Scene::step` can assert its own
//!   postconditions.
//! - **Collision solvers** — [`elastic_collision_1d`],
//!   [`perfectly_inelastic_collision_1d`]. These are the conservation laws made
//!   executable: the elastic solver is the simultaneous solution of the momentum
//!   and kinetic-energy equations, and the inelastic one solves momentum alone.
//!
//! # Why the totals use compensated summation
//!
//! A naive `iter().sum()` accumulates rounding error proportional to the number
//! of terms and to the spread of their magnitudes, which is exactly the
//! situation a conservation check faces: many bodies, wildly different masses,
//! and a total that may be near zero through cancellation. Both totals therefore
//! use Neumaier compensated summation, which tracks the lost low-order bits in a
//! second accumulator. This does not make the sum invariant under reordering —
//! nothing cheap does — but it keeps the drift small enough that a tolerance of
//! `1e-12` is meaningful rather than aspirational.

use super::physics_constants::GRAVITATIONAL_CONSTANT;

/// Neumaier (improved Kahan) compensated summation.
///
/// Returns `sum + compensation`, where `compensation` accumulates the low-order
/// bits each addition discards. Handles the case the original Kahan algorithm
/// gets wrong — when the incoming term is larger in magnitude than the running
/// sum — which matters here because a body list is not sorted by mass.
fn compensated_sum(terms: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;

    for term in terms {
        let tentative = sum + term;
        if sum.abs() >= term.abs() {
            compensation += (sum - tentative) + term;
        } else {
            compensation += (term - tentative) + sum;
        }
        sum = tentative;
    }

    sum + compensation
}

/// True when every `(mass, velocity)` pair in `bodies` satisfies the shared
/// precondition: finite non-negative mass, finite velocity.
fn all_bodies_valid(bodies: &[(f64, f64)]) -> bool {
    bodies.iter().all(|&(mass_kg, velocity_m_s)| {
        mass_kg.is_finite() && mass_kg >= 0.0 && velocity_m_s.is_finite()
    })
}

/// Relative-difference test against a caller-supplied scale.
///
/// Returns `true` when `a` and `b` agree to within `relative_tolerance` of
/// `scale`. When `scale` is zero there is no magnitude to be relative to, so it
/// demands exact equality — in practice both callers only reach that branch when
/// `a` and `b` are already both zero, so it is a guard against a meaningless
/// comparison rather than a discriminating test.
fn agrees_within(a: f64, b: f64, scale: f64, relative_tolerance: f64) -> bool {
    if scale == 0.0 {
        return a == b;
    }
    (a - b).abs() <= relative_tolerance * scale
}

/// Linear momentum of a single body: `p = m·v`.
///
/// # Parameters
///
/// - `mass_kg` — mass in kilograms. Must be finite and non-negative.
/// - `velocity_m_s` — velocity in metres per second. Any finite value; the sign
///   is the direction along the caller's axis.
///
/// # Returns
///
/// Momentum in kilogram-metres per second, with the same sign as
/// `velocity_m_s`.
///
/// # Preconditions and release behaviour
///
/// Both finite, `mass_kg >= 0.0`. Violations fire a `debug_assert!`; in release
/// the function returns `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::momentum;
///
/// assert_eq!(momentum(2.0, 3.0), 6.0);
/// assert_eq!(momentum(2.0, -3.0), -6.0);
/// ```
#[inline]
pub fn momentum(mass_kg: f64, velocity_m_s: f64) -> f64 {
    debug_assert!(
        mass_kg.is_finite() && mass_kg >= 0.0,
        "momentum: mass_kg must be finite and non-negative, got {}",
        mass_kg
    );
    debug_assert!(
        velocity_m_s.is_finite(),
        "momentum: velocity_m_s must be finite, got {}",
        velocity_m_s
    );

    if !mass_kg.is_finite() || mass_kg < 0.0 || !velocity_m_s.is_finite() {
        return 0.0;
    }

    mass_kg * velocity_m_s
}

/// Total momentum of a system of bodies: `Σ mᵢ·vᵢ`.
///
/// This is the quantity **conservation of momentum** says is constant for a
/// closed system, and the reason it is constant is Newton's third law: internal
/// forces come in equal and opposite pairs that cancel in the total.
///
/// # Parameters
///
/// - `bodies` — `(mass_kg, velocity_m_s)` pairs. An empty slice is valid and
///   totals `0.0`. Every mass must be finite and non-negative and every velocity
///   finite.
///
/// # Returns
///
/// The total momentum in kilogram-metres per second. May legitimately be `0.0`
/// for a system whose bodies cancel.
///
/// # Preconditions and release behaviour
///
/// Every entry valid as above. A violation fires a `debug_assert!` naming the
/// offending index; in release the whole total is `0.0`, because a partial sum
/// over a corrupt list is worse than an obviously wrong one.
///
/// # Examples
///
/// ```
/// use physics_sandbox::total_momentum;
///
/// // Equal and opposite: the system is going nowhere.
/// assert_eq!(total_momentum(&[(2.0, 5.0), (5.0, -2.0)]), 0.0);
///
/// assert_eq!(total_momentum(&[]), 0.0);
/// ```
pub fn total_momentum(bodies: &[(f64, f64)]) -> f64 {
    debug_assert!(
        all_bodies_valid(bodies),
        "total_momentum: every body must have a finite non-negative mass and a finite velocity, got {:?}",
        bodies
    );

    if !all_bodies_valid(bodies) {
        return 0.0;
    }

    compensated_sum(
        bodies
            .iter()
            .map(|&(mass_kg, velocity_m_s)| mass_kg * velocity_m_s),
    )
}

/// Kinetic energy of a single body: `T = ½·m·v²`.
///
/// The velocity is squared, so only its magnitude matters and a signed velocity
/// is accepted as-is — callers working along an axis need not take an absolute
/// value first.
///
/// # Parameters
///
/// - `mass_kg` — mass in kilograms. Must be finite and non-negative.
/// - `velocity_m_s` — velocity in metres per second. Any finite value, either
///   sign.
///
/// # Returns
///
/// Kinetic energy in joules: always non-negative.
///
/// # Preconditions and release behaviour
///
/// Both finite, `mass_kg >= 0.0`. Violations fire a `debug_assert!`; in release
/// the function returns `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::kinetic_energy;
///
/// assert_eq!(kinetic_energy(2.0, 3.0), 9.0);
///
/// // Direction does not matter: energy is a scalar.
/// assert_eq!(kinetic_energy(2.0, -3.0), kinetic_energy(2.0, 3.0));
/// ```
#[inline]
pub fn kinetic_energy(mass_kg: f64, velocity_m_s: f64) -> f64 {
    debug_assert!(
        mass_kg.is_finite() && mass_kg >= 0.0,
        "kinetic_energy: mass_kg must be finite and non-negative, got {}",
        mass_kg
    );
    debug_assert!(
        velocity_m_s.is_finite(),
        "kinetic_energy: velocity_m_s must be finite, got {}",
        velocity_m_s
    );

    if !mass_kg.is_finite() || mass_kg < 0.0 || !velocity_m_s.is_finite() {
        return 0.0;
    }

    0.5 * mass_kg * (velocity_m_s * velocity_m_s)
}

/// Total kinetic energy of a system: `Σ ½·mᵢ·vᵢ²`.
///
/// Unlike [`total_momentum`], this cannot cancel — every term is non-negative —
/// so a zero total means every body is at rest.
///
/// # Parameters
///
/// - `bodies` — `(mass_kg, velocity_m_s)` pairs, same contract as
///   [`total_momentum`]. An empty slice totals `0.0`.
///
/// # Returns
///
/// Total kinetic energy in joules: always non-negative.
///
/// # Preconditions and release behaviour
///
/// Every entry valid. A violation fires a `debug_assert!`; in release the total
/// is `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::total_kinetic_energy;
///
/// assert_eq!(total_kinetic_energy(&[(2.0, 3.0), (4.0, -1.0)]), 11.0);
/// assert_eq!(total_kinetic_energy(&[]), 0.0);
/// ```
pub fn total_kinetic_energy(bodies: &[(f64, f64)]) -> f64 {
    debug_assert!(
        all_bodies_valid(bodies),
        "total_kinetic_energy: every body must have a finite non-negative mass and a finite velocity, got {:?}",
        bodies
    );

    if !all_bodies_valid(bodies) {
        return 0.0;
    }

    compensated_sum(
        bodies
            .iter()
            .map(|&(mass_kg, velocity_m_s)| 0.5 * mass_kg * (velocity_m_s * velocity_m_s)),
    )
}

/// Gravitational potential energy of a two-body pair: `U = −G·m_a·m_b / r`.
///
/// Negative by convention, approaching zero as the separation grows: the zero of
/// potential is infinite separation, so a bound pair has negative energy and
/// `|U|` is the work needed to separate them completely. This is the potential
/// partner to [`super::physics_formulas::gravitational_constant_calculator`],
/// which gives the force — the force is the negative gradient of this.
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — the two masses in kilograms. Finite and
///   non-negative.
/// - `distance_m` — separation of the centres of mass, in metres. Finite and
///   strictly positive.
///
/// # Returns
///
/// Potential energy in joules: always `<= 0.0`, and exactly `-0.0` when either
/// mass is zero.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `0.0`.
///
/// # Accuracy
///
/// Evaluated as `-(G · (m_a · m_b) / r)`. Forming the mass product first makes
/// the result bit-exactly symmetric in the two masses, since IEEE-754
/// multiplication is commutative — the same reasoning as
/// `gravitational_constant_calculator`, but with the opposite grouping, because
/// here there is a single `r` to divide by rather than two. The mass product
/// overflows only above `~1e154 kg`, far outside any physical input.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{gravitational_potential_energy, GRAVITATIONAL_CONSTANT};
///
/// // Two 1 kg masses 1 m apart: U = -G.
/// assert_eq!(gravitational_potential_energy(1.0, 1.0, 1.0), -GRAVITATIONAL_CONSTANT);
///
/// // Potential rises toward zero with separation.
/// assert!(gravitational_potential_energy(1.0, 1.0, 2.0)
///     > gravitational_potential_energy(1.0, 1.0, 1.0));
/// ```
#[inline]
pub fn gravitational_potential_energy(mass_a_kg: f64, mass_b_kg: f64, distance_m: f64) -> f64 {
    debug_assert!(
        mass_a_kg.is_finite() && mass_a_kg >= 0.0,
        "gravitational_potential_energy: mass_a_kg must be finite and non-negative, got {}",
        mass_a_kg
    );
    debug_assert!(
        mass_b_kg.is_finite() && mass_b_kg >= 0.0,
        "gravitational_potential_energy: mass_b_kg must be finite and non-negative, got {}",
        mass_b_kg
    );
    debug_assert!(
        distance_m.is_finite() && distance_m > 0.0,
        "gravitational_potential_energy: distance_m must be finite and strictly positive, got {}",
        distance_m
    );

    if !mass_a_kg.is_finite()
        || !mass_b_kg.is_finite()
        || !distance_m.is_finite()
        || mass_a_kg < 0.0
        || mass_b_kg < 0.0
        || distance_m <= 0.0
    {
        return 0.0;
    }

    -(GRAVITATIONAL_CONSTANT * (mass_a_kg * mass_b_kg) / distance_m)
}

/// Total mechanical energy: `E = T + U`.
///
/// The quantity **conservation of energy** says is constant for a system acted
/// on only by conservative forces. Adding the two terms is trivial; naming the
/// sum is not, because it is the thing a conservation check must be applied to.
/// Feeding [`energy_is_conserved`] kinetic energy alone is correct only for a
/// system with no potential — a free-flying collision, not an orbit.
///
/// # Parameters
///
/// - `kinetic_energy_j` — total kinetic energy in joules. Finite; normally
///   non-negative, but not required to be, so a caller may pass a difference.
/// - `potential_energy_j` — total potential energy in joules. Finite. Typically
///   negative for gravity (see [`gravitational_potential_energy`]).
///
/// # Returns
///
/// Total mechanical energy in joules. A bound system has `E < 0`; an unbound one
/// has `E >= 0` — which is exactly the escape-velocity condition.
///
/// # Preconditions and release behaviour
///
/// Both finite. Violations fire a `debug_assert!`; in release the function
/// returns `0.0`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::total_mechanical_energy;
///
/// // Bound: kinetic energy does not cover the potential well.
/// assert!(total_mechanical_energy(3.0, -5.0) < 0.0);
///
/// // Unbound: it does.
/// assert!(total_mechanical_energy(7.0, -5.0) > 0.0);
/// ```
#[inline]
pub fn total_mechanical_energy(kinetic_energy_j: f64, potential_energy_j: f64) -> f64 {
    debug_assert!(
        kinetic_energy_j.is_finite(),
        "total_mechanical_energy: kinetic_energy_j must be finite, got {}",
        kinetic_energy_j
    );
    debug_assert!(
        potential_energy_j.is_finite(),
        "total_mechanical_energy: potential_energy_j must be finite, got {}",
        potential_energy_j
    );

    if !kinetic_energy_j.is_finite() || !potential_energy_j.is_finite() {
        return 0.0;
    }

    kinetic_energy_j + potential_energy_j
}

/// Did a transition conserve the system's total momentum?
///
/// **Conservation of momentum.** For a closed system — no external forces — the
/// total momentum before a transition equals the total after, however violent
/// the interaction.
///
/// # The scale problem, and how this resolves it
///
/// Total momentum can legitimately be zero through cancellation, so a relative
/// comparison against the total itself is degenerate: two bodies at `+1000` and
/// `-1000` have a total of `0`, and no relative tolerance on `0` is meaningful.
/// The comparison is therefore scaled by the larger total **absolute** momentum
/// `Σ|mᵢ·vᵢ|` of the two states, which is the physically meaningful magnitude of
/// what is moving.
///
/// Note what that implies, because it is easy to misread: the scale is zero only
/// when *both* states are entirely at rest, and then both totals are `0.0` and
/// conservation holds trivially. So the exact-equality branch never rejects
/// anything — detection of momentum appearing from rest comes from the ordinary
/// relative comparison against the `after` state's own scale, and therefore
/// depends on `relative_tolerance` being small. A tolerance of `1.0` accepts any
/// transition by construction.
///
/// # Parameters
///
/// - `before`, `after` — the system's `(mass_kg, velocity_m_s)` pairs on each
///   side of the transition. They need not be the same length: a collision that
///   merges two bodies into one is still momentum-conserving.
/// - `relative_tolerance` — permitted drift as a fraction of the absolute
///   momentum scale. Must be finite and non-negative. `1e-12` is a reasonable
///   default for `f64` arithmetic over a modest number of bodies.
///
/// # Returns
///
/// `true` when the totals agree within tolerance.
///
/// # Preconditions and release behaviour
///
/// Every body valid, `relative_tolerance` finite and `>= 0.0`. Violations fire a
/// `debug_assert!`; in release the function returns `false`, because "I could not
/// verify this" must never be reported as "conserved".
///
/// # Examples
///
/// ```
/// use physics_sandbox::momentum_is_conserved;
///
/// // A perfectly inelastic merge: 2 kg at 3 m/s hits 1 kg at rest.
/// let before = [(2.0, 3.0), (1.0, 0.0)];
/// let after = [(3.0, 2.0)];
/// assert!(momentum_is_conserved(&before, &after, 1.0e-12));
///
/// // Inventing momentum from nothing is caught.
/// assert!(!momentum_is_conserved(&[(1.0, 0.0)], &[(1.0, 5.0)], 1.0e-12));
/// ```
pub fn momentum_is_conserved(
    before: &[(f64, f64)],
    after: &[(f64, f64)],
    relative_tolerance: f64,
) -> bool {
    debug_assert!(
        all_bodies_valid(before),
        "momentum_is_conserved: every `before` body must have a finite non-negative mass and a finite velocity, got {:?}",
        before
    );
    debug_assert!(
        all_bodies_valid(after),
        "momentum_is_conserved: every `after` body must have a finite non-negative mass and a finite velocity, got {:?}",
        after
    );
    debug_assert!(
        relative_tolerance.is_finite() && relative_tolerance >= 0.0,
        "momentum_is_conserved: relative_tolerance must be finite and non-negative, got {}",
        relative_tolerance
    );

    if !all_bodies_valid(before)
        || !all_bodies_valid(after)
        || !relative_tolerance.is_finite()
        || relative_tolerance < 0.0
    {
        return false;
    }

    let absolute_momentum = |bodies: &[(f64, f64)]| {
        compensated_sum(
            bodies
                .iter()
                .map(|&(mass_kg, velocity_m_s)| (mass_kg * velocity_m_s).abs()),
        )
    };
    let scale = absolute_momentum(before).max(absolute_momentum(after));

    agrees_within(
        total_momentum(before),
        total_momentum(after),
        scale,
        relative_tolerance,
    )
}

/// Did a transition conserve total energy?
///
/// **Conservation of energy**, in its general scalar form: the caller does the
/// energy bookkeeping — kinetic only for a free collision,
/// [`total_mechanical_energy`] for anything with a potential — and this decides
/// whether the two figures agree.
///
/// Scaled by `max(|before|, |after|)`, and demanding exact equality when both
/// are zero. Unlike momentum, energy totals do not cancel, so the scale is the
/// magnitude itself.
///
/// # Parameters
///
/// - `total_before_j`, `total_after_j` — total energy in joules on each side of
///   the transition. Both finite.
/// - `relative_tolerance` — permitted drift as a fraction of the larger
///   magnitude. Finite and non-negative.
///
/// # Returns
///
/// `true` when the two agree within tolerance.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `false` — an unverifiable claim is not a satisfied one.
///
/// # Examples
///
/// ```
/// use physics_sandbox::energy_is_conserved;
///
/// assert!(energy_is_conserved(11.0, 11.0, 1.0e-12));
/// assert!(energy_is_conserved(11.0, 11.0 + 1.0e-13, 1.0e-12));
///
/// // An inelastic collision does *not* conserve kinetic energy.
/// assert!(!energy_is_conserved(11.0, 6.0, 1.0e-12));
/// ```
#[inline]
pub fn energy_is_conserved(
    total_before_j: f64,
    total_after_j: f64,
    relative_tolerance: f64,
) -> bool {
    debug_assert!(
        total_before_j.is_finite(),
        "energy_is_conserved: total_before_j must be finite, got {}",
        total_before_j
    );
    debug_assert!(
        total_after_j.is_finite(),
        "energy_is_conserved: total_after_j must be finite, got {}",
        total_after_j
    );
    debug_assert!(
        relative_tolerance.is_finite() && relative_tolerance >= 0.0,
        "energy_is_conserved: relative_tolerance must be finite and non-negative, got {}",
        relative_tolerance
    );

    if !total_before_j.is_finite()
        || !total_after_j.is_finite()
        || !relative_tolerance.is_finite()
        || relative_tolerance < 0.0
    {
        return false;
    }

    let scale = total_before_j.abs().max(total_after_j.abs());

    agrees_within(total_before_j, total_after_j, scale, relative_tolerance)
}

/// Did a transition conserve the system's total kinetic energy?
///
/// The slice-shaped convenience over [`energy_is_conserved`], mirroring
/// [`momentum_is_conserved`], for systems with no potential energy — a free
/// collision, not an orbit. An elastic collision satisfies this; a perfectly
/// inelastic one does not, and that contrast is the whole distinction between
/// [`elastic_collision_1d`] and [`perfectly_inelastic_collision_1d`].
///
/// # Parameters
///
/// - `before`, `after` — the system's `(mass_kg, velocity_m_s)` pairs. Need not
///   be the same length.
/// - `relative_tolerance` — finite and non-negative.
///
/// # Returns
///
/// `true` when the totals agree within tolerance. `false` on any precondition
/// violation.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{elastic_collision_1d, kinetic_energy_is_conserved};
///
/// let before = [(2.0, 4.0), (3.0, -1.0)];
/// let (a, b) = elastic_collision_1d(2.0, 4.0, 3.0, -1.0);
/// assert!(kinetic_energy_is_conserved(&before, &[(2.0, a), (3.0, b)], 1.0e-12));
/// ```
pub fn kinetic_energy_is_conserved(
    before: &[(f64, f64)],
    after: &[(f64, f64)],
    relative_tolerance: f64,
) -> bool {
    debug_assert!(
        all_bodies_valid(before),
        "kinetic_energy_is_conserved: every `before` body must have a finite non-negative mass and a finite velocity, got {:?}",
        before
    );
    debug_assert!(
        all_bodies_valid(after),
        "kinetic_energy_is_conserved: every `after` body must have a finite non-negative mass and a finite velocity, got {:?}",
        after
    );

    if !all_bodies_valid(before) || !all_bodies_valid(after) {
        return false;
    }

    energy_is_conserved(
        total_kinetic_energy(before),
        total_kinetic_energy(after),
        relative_tolerance,
    )
}

/// Resolve a one-dimensional perfectly elastic collision.
///
/// The unique outcome that conserves **both** total momentum and total kinetic
/// energy. Solving those two equations simultaneously gives
///
/// ```text
/// v_a' = v_a − 2·m_b/(m_a + m_b) · (v_a − v_b)
/// v_b' = v_b − 2·m_a/(m_a + m_b) · (v_b − v_a)
/// ```
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — the two masses in kilograms. Finite and
///   **strictly positive**: a massless body has no elastic response, and the
///   solution divides by the total mass.
/// - `velocity_a_m_s`, `velocity_b_m_s` — incoming velocities in metres per
///   second, signed along the caller's axis. Any finite values, including ones
///   that describe bodies moving apart — the caller decides whether a collision
///   is actually occurring; this function only resolves it.
///
/// # Returns
///
/// The outgoing velocities `(v_a', v_b')` in metres per second.
///
/// # Notable cases
///
/// - **Equal masses** exchange velocities exactly: `(v_a, v_b) → (v_b, v_a)`.
/// - **A very heavy body** is barely deflected while the light one reverses
///   relative to it, approaching a perfect bounce off a wall as the ratio
///   diverges.
/// - **Equal velocities** leave both unchanged: there is no relative motion to
///   redistribute.
///
/// # Preconditions and release behaviour
///
/// Velocities finite, masses finite and strictly positive. Violations fire a
/// `debug_assert!`; in release the function returns the **inputs unchanged**.
/// That is deliberately not the `0.0` other functions here fall back to: zeroing
/// the velocities would destroy momentum and corrupt a running simulation,
/// whereas declining to resolve the collision conserves everything trivially.
///
/// # Accuracy
///
/// Written in relative-velocity form rather than the textbook
/// `((m_a − m_b)·v_a + 2·m_b·v_b)/(m_a + m_b)`. The two are algebraically
/// identical, but `m_a − m_b` loses precision catastrophically for nearly equal
/// masses, which is the most common case in a simulation of similar bodies.
///
/// # Examples
///
/// ```
/// use physics_sandbox::elastic_collision_1d;
///
/// // Equal masses exchange velocities exactly.
/// assert_eq!(elastic_collision_1d(1.0, 5.0, 1.0, -2.0), (-2.0, 5.0));
///
/// // A moving body striking an equal one at rest stops dead.
/// assert_eq!(elastic_collision_1d(2.0, 3.0, 2.0, 0.0), (0.0, 3.0));
/// ```
#[inline]
pub fn elastic_collision_1d(
    mass_a_kg: f64,
    velocity_a_m_s: f64,
    mass_b_kg: f64,
    velocity_b_m_s: f64,
) -> (f64, f64) {
    debug_assert!(
        mass_a_kg.is_finite() && mass_a_kg > 0.0,
        "elastic_collision_1d: mass_a_kg must be finite and strictly positive, got {}",
        mass_a_kg
    );
    debug_assert!(
        mass_b_kg.is_finite() && mass_b_kg > 0.0,
        "elastic_collision_1d: mass_b_kg must be finite and strictly positive, got {}",
        mass_b_kg
    );
    debug_assert!(
        velocity_a_m_s.is_finite(),
        "elastic_collision_1d: velocity_a_m_s must be finite, got {}",
        velocity_a_m_s
    );
    debug_assert!(
        velocity_b_m_s.is_finite(),
        "elastic_collision_1d: velocity_b_m_s must be finite, got {}",
        velocity_b_m_s
    );

    let total_mass = mass_a_kg + mass_b_kg;

    if !mass_a_kg.is_finite()
        || !mass_b_kg.is_finite()
        || mass_a_kg <= 0.0
        || mass_b_kg <= 0.0
        || !velocity_a_m_s.is_finite()
        || !velocity_b_m_s.is_finite()
        || !total_mass.is_finite()
        || total_mass <= 0.0
    {
        return (velocity_a_m_s, velocity_b_m_s);
    }

    let approach_speed = velocity_a_m_s - velocity_b_m_s;

    (
        velocity_a_m_s - 2.0 * (mass_b_kg / total_mass) * approach_speed,
        velocity_b_m_s + 2.0 * (mass_a_kg / total_mass) * approach_speed,
    )
}

/// Resolve a one-dimensional perfectly inelastic collision.
///
/// The two bodies coalesce and move off together, so momentum alone determines
/// the outcome:
///
/// ```text
/// v' = (m_a·v_a + m_b·v_b) / (m_a + m_b)
/// ```
///
/// This is the maximally dissipative case: **momentum is conserved, kinetic
/// energy is not.** The energy lost — to deformation, heat, and sound — is the
/// kinetic energy of the relative motion, and it is zero only when the bodies
/// were already moving at the same velocity. Pairing this with
/// [`elastic_collision_1d`] is what makes conservation of energy testable in
/// both directions: one outcome must preserve it, the other must strictly
/// reduce it.
///
/// # Parameters
///
/// - `mass_a_kg`, `mass_b_kg` — masses in kilograms. Finite and strictly
///   positive.
/// - `velocity_a_m_s`, `velocity_b_m_s` — incoming velocities in metres per
///   second, signed. Finite.
///
/// # Returns
///
/// The shared outgoing velocity in metres per second: the centre-of-mass
/// velocity of the pair, which always lies between the two inputs.
///
/// # Preconditions and release behaviour
///
/// As above. Violations fire a `debug_assert!`; in release the function returns
/// `0.0`. Unlike [`elastic_collision_1d`] there is no "leave the inputs alone"
/// fallback available, because the result is a single merged velocity rather
/// than a pair.
///
/// # Examples
///
/// ```
/// use physics_sandbox::perfectly_inelastic_collision_1d;
///
/// // 1 kg at 8 m/s catches 3 kg at rest: 8 kg·m/s over 4 kg.
/// assert_eq!(perfectly_inelastic_collision_1d(1.0, 8.0, 3.0, 0.0), 2.0);
///
/// // A head-on pair with equal and opposite momentum stops dead.
/// assert_eq!(perfectly_inelastic_collision_1d(1.0, 3.0, 3.0, -1.0), 0.0);
/// ```
#[inline]
pub fn perfectly_inelastic_collision_1d(
    mass_a_kg: f64,
    velocity_a_m_s: f64,
    mass_b_kg: f64,
    velocity_b_m_s: f64,
) -> f64 {
    debug_assert!(
        mass_a_kg.is_finite() && mass_a_kg > 0.0,
        "perfectly_inelastic_collision_1d: mass_a_kg must be finite and strictly positive, got {}",
        mass_a_kg
    );
    debug_assert!(
        mass_b_kg.is_finite() && mass_b_kg > 0.0,
        "perfectly_inelastic_collision_1d: mass_b_kg must be finite and strictly positive, got {}",
        mass_b_kg
    );
    debug_assert!(
        velocity_a_m_s.is_finite(),
        "perfectly_inelastic_collision_1d: velocity_a_m_s must be finite, got {}",
        velocity_a_m_s
    );
    debug_assert!(
        velocity_b_m_s.is_finite(),
        "perfectly_inelastic_collision_1d: velocity_b_m_s must be finite, got {}",
        velocity_b_m_s
    );

    let total_mass = mass_a_kg + mass_b_kg;

    if !mass_a_kg.is_finite()
        || !mass_b_kg.is_finite()
        || mass_a_kg <= 0.0
        || mass_b_kg <= 0.0
        || !velocity_a_m_s.is_finite()
        || !velocity_b_m_s.is_finite()
        || !total_mass.is_finite()
        || total_mass <= 0.0
    {
        return 0.0;
    }

    // Grouped as a mass-weighted average of the velocities rather than
    // `(m_a·v_a + m_b·v_b) / M`, so each weight is formed at a magnitude near 1
    // and the result cannot overflow for masses whose product would.
    (mass_a_kg / total_mass) * velocity_a_m_s + (mass_b_kg / total_mass) * velocity_b_m_s
}

#[cfg(test)]
#[path = "../../tst/physics_utils/conservation_laws_tests.rs"]
mod tests;
