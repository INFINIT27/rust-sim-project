//! Mutual-gravitation integrator: the body of the `Scene::step` seam.
//!
//! Every prop attracts every other prop by Newton's law of universal
//! gravitation, and each movable prop's velocity and position are advanced by
//! the resulting acceleration. This is the first module to actually *use*
//! [`crate::physics_utils`] rather than merely sit next to it — it composes
//! four functions from there and adds no formula of its own:
//!
//! | Step | Function |
//! | --- | --- |
//! | Force between a pair | [`gravitational_constant_calculator`] |
//! | Force → acceleration | [`acceleration_from_net_force`] |
//! | Acceleration → displacement | [`displacement`] |
//! | Acceleration → new velocity | [`velocity_after`] |
//!
//! # Which props move, and which pull
//!
//! Three independent questions, matching the [`Prop`] attribute table:
//!
//! - **Does it move?** Only if `!immovable`. An immovable prop is skipped
//!   entirely — no acceleration, no displacement.
//! - **Can it be accelerated?** Only if [`Prop::responds_to_forces`], i.e.
//!   movable *and* `mass_kg > 0.0`. A movable massless prop still travels at its
//!   velocity in a straight line; `a = F/m` simply cannot deflect it.
//! - **Does it pull?** Always, in proportion to its `mass_kg` — including when
//!   it is immovable. This is how a fixed attractor works: give an immovable
//!   prop a mass and it becomes a gravity well that never moves. The ground
//!   plane has `mass_kg == 0.0`, so it is gravitationally inert and the floor
//!   does not tug at anything.
//!
//! # Integration scheme
//!
//! One force evaluation per step, applied **velocity first**:
//!
//! ```text
//! v' = v + a·dt            (velocity_after — Newton's second law)
//! Δx = v'·dt               (displacement at the updated velocity)
//! ```
//!
//! Updating the velocity *before* moving is what makes this semi-implicit
//! (symplectic) Euler rather than explicit Euler, and the difference is not
//! academic. The explicit ordering — move at the old velocity, including the
//! `½·a·dt²` term — was tried first and, measured on a circular test orbit, grew
//! its radius by **5.2% per revolution**: explicit Euler injects energy every step
//! and the error is secular. Symplectic Euler's energy error is bounded and
//! oscillatory instead, and the same orbit held its radius to under 0.05%. Do not
//! "simplify" the ordering back.
//!
//! It is still only first-order accurate, so phase drifts even where shape does
//! not, and the scheme is far too coarse to trust through a close encounter at any
//! frame-sized timestep. Leapfrog or RK4 would improve both and are the obvious
//! next upgrade.
//!
//! # Softening
//!
//! Point masses have a singularity at zero separation: `F → ∞` as `r → 0`. Every
//! N-body code avoids it the same way, and so does this one — see
//! [`GRAVITY_SOFTENING_UNITS`].
//!
//! # Units
//!
//! Scene lengths are in world units and gravitation is in SI, so this module is
//! the crate's one conversion site. Separations are multiplied up by
//! [`METRES_PER_UNIT`] before the force is evaluated, and the resulting
//! acceleration is divided back down before it touches a velocity. Mass is
//! already in kilograms and time already in seconds, so neither is rescaled.

use bevy::math::Vec3;

use crate::constants::METRES_PER_UNIT;
use crate::core::constants::GRAVITY_SOFTENING_UNITS;
use crate::core::scene::Prop;
use crate::physics_utils::kinematics::{displacement, velocity_after};
use crate::physics_utils::newtons_laws::acceleration_from_net_force;
use crate::physics_utils::physics_formulas::gravitational_constant_calculator;
use crate::time::simulation::TimeStep;

/// The softening length to actually use, given what a caller asked for.
///
/// Falls back to [`GRAVITY_SOFTENING_UNITS`] for a non-finite or negative value rather than
/// propagating it: a `NaN` here would turn every acceleration in the scene into a `NaN` on the
/// next step, which is the least debuggable failure this module can produce. Zero is passed
/// through, because true point-mass gravitation is a legitimate — if singular — request.
#[inline]
fn usable_softening(softening_units: f32) -> f32 {
    if softening_units.is_finite() && softening_units >= 0.0 {
        softening_units
    } else {
        GRAVITY_SOFTENING_UNITS
    }
}

/// Net gravitational acceleration on `props[target]`, in world **units per second
/// squared**.
///
/// `softening_units` is the scene's Plummer softening length — see
/// [`Scene::softening_units`](crate::core::scene::Scene::softening_units) for why it is a
/// per-scene quantity rather than a constant, and [`GRAVITY_SOFTENING_UNITS`] for what the
/// number means. Pass that constant to get the crate's historical behaviour.
///
/// The force is evaluated in SI internally — separations scaled up by
/// [`METRES_PER_UNIT`], masses already in kilograms — and the resulting
/// acceleration is scaled back down before it is returned, so the value composes
/// directly with a velocity in units per second.
///
/// Sums the pull of every *other* prop. Returns `Vec3::ZERO` when the target
/// cannot be accelerated — immovable, or massless — and skips any source that is
/// exactly coincident with the target, since the direction of the pull is
/// undefined there.
///
/// # Parameters
///
/// - `props` — the whole scene's props; every entry other than `target` is
///   treated as a gravitational source.
/// - `target` — index into `props` of the prop being accelerated. Must be in
///   bounds.
///
/// # Preconditions and release behaviour
///
/// `target < props.len()`, and every prop valid. Violations fire a
/// `debug_assert!`; in release the function returns `Vec3::ZERO`, so a corrupt
/// scene coasts rather than exploding.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{
///     gravitational_acceleration, Body, Color, Pose, Prop, Quat, Vec3, GRAVITY_SOFTENING_UNITS,
/// };
///
/// fn ball(x: f32) -> Body {
///     Body::Sphere {
///         pose: Pose { position: Vec3::new(x, 0.0, 0.0), rotation: Quat::IDENTITY },
///         radius: 1.0,
///         color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
///     }
/// }
///
/// // Two 1e24 kg props 10 units apart on the X axis.
/// let props = [
///     Prop::at_rest(ball(0.0), 1.0e24),
///     Prop::at_rest(ball(10.0), 1.0e24),
/// ];
///
/// // The first is pulled toward the second, i.e. along +X.
/// let a = gravitational_acceleration(&props, 0, GRAVITY_SOFTENING_UNITS);
/// assert!(a.x > 0.0 && a.y == 0.0 && a.z == 0.0);
///
/// // And the second is pulled back, equally hard (Newton's third law).
/// let b = gravitational_acceleration(&props, 1, GRAVITY_SOFTENING_UNITS);
/// assert!((a.x + b.x).abs() < 1.0e-9 * a.x.abs().max(1.0));
/// ```
pub fn gravitational_acceleration(props: &[Prop], target: usize, softening_units: f32) -> Vec3 {
    debug_assert!(
        target < props.len(),
        "gravitational_acceleration: target {} out of bounds for {} props",
        target,
        props.len()
    );

    if target >= props.len() {
        return Vec3::ZERO;
    }

    let subject = &props[target];

    debug_assert!(
        subject.is_valid(),
        "gravitational_acceleration: target prop fails Validation_Rules: {:?}",
        subject
    );

    // An immovable or massless prop cannot be accelerated. Bail before dividing
    // by its mass — `acceleration_from_net_force` rejects a zero mass, and it is
    // right to.
    if !subject.responds_to_forces() || !subject.is_valid() {
        return Vec3::ZERO;
    }

    let subject_center = subject.body.center();
    let subject_mass = subject.mass_kg as f64;
    let softening_m = usable_softening(softening_units) as f64 * METRES_PER_UNIT;

    let mut acceleration = Vec3::ZERO;

    for (index, source) in props.iter().enumerate() {
        if index == target || !source.is_valid() {
            continue;
        }

        let offset = source.body.center() - subject_center;
        let separation = offset.length();

        // Exactly coincident: the pull has a magnitude but no direction, so
        // there is nothing meaningful to add. Softening handles *near*-coincident
        // props; only the exact case has to be skipped. The `is_finite` test comes
        // first so a `NaN` separation lands here rather than slipping through the
        // comparison, the same ordering `clamp_dt` relies on.
        if !separation.is_finite() || separation <= 0.0 {
            continue;
        }

        // Into SI for the gravitation, and Plummer softening on the way: evaluate
        // the force at an effective separation of sqrt(r² + ε²). Passing that to
        // the unmodified force formula is exactly equivalent to dividing by
        // (r² + ε²), so no new formula is needed.
        let separation_m = separation as f64 * METRES_PER_UNIT;
        let effective_separation_m =
            (separation_m * separation_m + softening_m * softening_m).sqrt();

        let force_n = gravitational_constant_calculator(
            subject_mass,
            source.mass_kg as f64,
            effective_separation_m,
        );

        // Back out of SI: `acceleration_from_net_force` yields m/s², and velocities
        // are in units/s, so divide by the scale before the value ever meets a
        // stored quantity. Done in f64 so the intermediate cannot underflow.
        let magnitude_units = acceleration_from_net_force(force_n, subject_mass) / METRES_PER_UNIT;

        // Direction from the *true* offset, magnitude from the softened
        // separation — which is what makes this Plummer softening rather than a
        // distance clamp.
        acceleration += (offset / separation) * (magnitude_units as f32);
    }

    acceleration
}

/// Advance every movable prop by `dt` seconds under mutual gravitation.
///
/// The body of [`crate::core::scene::Scene::step`]. Two passes:
///
/// 1. Compute every prop's acceleration from the *current* positions. This has
///    to complete before anything moves, or props updated early would pull props
///    updated late from their new positions — which is neither the explicit nor
///    the implicit scheme, just a bug that depends on iteration order.
/// 2. Apply displacement and velocity to each movable prop.
///
/// # Parameters
///
/// - `props` — the scene's props, mutated in place. Immovable props are left
///   untouched, so their `Vec3::ZERO` velocity invariant survives.
/// - `step` — the interval to advance by, as a validated [`TimeStep`]. Its type
///   carries the "finite and non-negative" guarantee that used to be an assertion
///   here, and a zero-length step leaves every prop byte-identical, which is what
///   `Scene::step`'s Postcondition 5.2 requires.
///
/// # Preconditions and release behaviour
///
/// A `TimeStep` cannot be invalid unless it was built by an `impl` that broke the
/// type's invariant, so the `debug_assert!` here guards the type rather than the
/// caller; in release nothing moves.
///
/// # Allocation
///
/// Allocates one `Vec<Vec3>` per call to hold the first pass's accelerations.
/// Fine at the current scale; an integrator running thousands of props would
/// want a scratch buffer carried across frames.
pub fn integrate_gravity(props: &mut [Prop], step: TimeStep, softening_units: f32) {
    debug_assert!(
        step.is_valid(),
        "integrate_gravity: TimeStep must be finite and non-negative, got {:?}",
        step
    );

    if !step.is_valid() {
        return;
    }

    // The one narrowing: `physics_utils` works in `f64`, and this is where a step
    // becomes the `elapsed_s` those relations take.
    let dt = step.seconds();

    // Pass 1: accelerations from the current configuration, before anything
    // moves.
    let accelerations: Vec<Vec3> = (0..props.len())
        .map(|index| gravitational_acceleration(props, index, softening_units))
        .collect();

    // Pass 2: advance each movable prop.
    for (prop, acceleration) in props.iter_mut().zip(accelerations) {
        if prop.immovable {
            continue;
        }

        let velocity = prop.velocity_units_s;

        // Componentwise use of the shared kinematic relations, in the order that
        // makes the step symplectic: accelerate, then move at the *new* velocity.
        //
        // A massless movable prop has zero acceleration here, so both lines
        // reduce to uniform motion and it coasts in a straight line — correct,
        // not an oversight.
        let new_velocity = Vec3::new(
            velocity_after(velocity.x as f64, acceleration.x as f64, dt) as f32,
            velocity_after(velocity.y as f64, acceleration.y as f64, dt) as f32,
            velocity_after(velocity.z as f64, acceleration.z as f64, dt) as f32,
        );

        // Acceleration is passed as `0.0` deliberately: it has already been
        // folded into `new_velocity`, so what remains is uniform motion over the
        // step. Passing it again here would double-count it and give explicit
        // Euler's energy drift back.
        let delta = Vec3::new(
            displacement(new_velocity.x as f64, 0.0, dt) as f32,
            displacement(new_velocity.y as f64, 0.0, dt) as f32,
            displacement(new_velocity.z as f64, 0.0, dt) as f32,
        );

        prop.body.translate(delta);
        prop.velocity_units_s = new_velocity;
    }
}

#[cfg(test)]
#[path = "../../tst/core/integrator_tests.rs"]
mod tests;
