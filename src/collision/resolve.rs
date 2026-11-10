//! Response: what happens to two props once they are found to be touching.
//!
//! [`resolve_contacts`] is the pass [`Scene::step`](crate::core::scene::Scene::step)
//! runs after every integration substep. For each overlapping pair it does two
//! things, in this order:
//!
//! 1. **Bounce.** Swap the velocity components along the contact normal so the
//!    collision is perfectly elastic — momentum and kinetic energy both conserved.
//!    The solver is [`elastic_collision_1d`], the same one
//!    [`crate::physics_utils::conservation_laws`] already had; this module
//!    supplies the axis and adds no formula of its own, exactly as
//!    [`crate::core::integrator`] does for gravitation.
//! 2. **Separate.** Translate the pair apart along the normal by the penetration
//!    depth, split by inverse mass, so the overlap does not survive into the next
//!    step and accumulate.
//!
//! # Elastic, and only elastic
//!
//! [`RESTITUTION`] is `1.0` and there is no per-prop override yet: every contact
//! in this crate is perfectly elastic, and nothing dissipates energy. Two equal
//! spheres meeting head-on therefore exchange velocities exactly and separate as
//! fast as they arrived, which is why the shipped scene now *oscillates* instead
//! of ending — see [`Scene::two_body_gravity`](crate::core::scene::Scene::two_body_gravity).
//!
//! # What is not modelled
//!
//! Stated plainly, because each of these is a thing a viewer might expect to see
//! and will not:
//!
//! - **No rotation.** The impulse is central: it acts at the centre of mass, not
//!   at [`Contact::point`], so a glancing hit does not spin anything. `Prop` has
//!   no angular velocity or inertia tensor to spin.
//! - **No friction.** Only the normal component of the relative velocity is
//!   touched. The tangential component passes through untouched, so bodies slide
//!   frictionlessly.
//! - **No resting contact.** Each contact is resolved independently in one pass,
//!   so a body cannot be supported by two others at once without jitter; a stack
//!   needs an iterative or simultaneous solver.
//! - **No swept detection.** Contact is tested at substep boundaries, not along
//!   the path between them. That is what [`substep_count`] exists to keep safe —
//!   see its docs for the measured margin in the shipped scene.
//!
//! # Massless and immovable props
//!
//! The same three-way distinction the rest of the crate keeps (see
//! `core/README.md`):
//!
//! | Prop | In a contact |
//! | --- | --- |
//! | Movable, `mass_kg > 0.0` | Bounces and is pushed out. |
//! | **Immovable** | Infinite effective mass: never moves, and reflects whatever hits it. |
//! | **Movable, massless** | **Passes through.** No response at all. |
//!
//! The massless row is the same stance [`crate::core::integrator`] takes: `a =
//! F/m` cannot deflect a massless body, and a contact impulse is a force like any
//! other. A massless prop travels in a straight line through everything, which is
//! what the variant is reserved for.
//!
//! [`elastic_collision_1d`]: crate::physics_utils::conservation_laws::elastic_collision_1d
//! [`Contact::point`]: crate::collision::contact::Contact::point

pub use bevy::math::Vec3;

use crate::collision::constants::{
    BROAD_PHASE_RELATIVE_SLACK, MAX_SUBSTEPS, MAX_SUBSTEP_DISPLACEMENT_UNITS,
    MAX_SUBSTEP_DYNAMICAL_FRACTION, PLANE_THICKNESS_UNITS, RESTITUTION,
};
use crate::collision::contact::{contact_between_meshes, Contact};
use crate::collision::tessellate::{contact_mesh, hull_box};
use crate::collision::trimesh::TriMesh;
use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Body, DVec3, Prop};
use crate::physics_utils::conservation_laws::elastic_collision_1d;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;
use crate::time::simulation::TimeStep;

/// How many substeps `dt` should be split into so no prop outruns
/// [`MAX_SUBSTEP_DISPLACEMENT_UNITS`].
///
/// # Parameters
///
/// - `props` — the scene's props. Only movable ones are considered: an immovable
///   prop does not travel, whatever its velocity field says.
/// - `step` — the full frame's simulated interval.
///
/// # Returns
///
/// A count in `1..=MAX_SUBSTEPS`. Never `0`, so the caller's loop always runs and
/// `dt / count` is never a division by zero.
///
/// # Why speed rather than closing speed
///
/// The bound uses each prop's own speed, not the relative speed of pairs that
/// might collide. Pairwise closing speeds would be a tighter bound and cost a
/// pass over every pair to compute — and the answer would still be a
/// conservative guess, because the props accelerate during the step. The cheap
/// bound is strictly more cautious, which is the right direction to err in.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{substep_count, Body, Color, Pose, Prop, Quat, TimeStep, DVec3, Vec3};
///
/// let ball = Body::Sphere {
///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
///     radius: 1.0,
///     color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
/// };
/// let one_second = TimeStep::from_seconds(1.0).unwrap();
///
/// // Slow enough to need no splitting.
/// let slow = [Prop::new(ball.clone(), DVec3::new(0.1, 0.0, 0.0), 1.0)];
/// assert_eq!(substep_count(&slow, one_second), 1);
///
/// // 10 units per second over a full second is 40 quarter-unit steps.
/// let fast = [Prop::new(ball, DVec3::new(10.0, 0.0, 0.0), 1.0)];
/// assert_eq!(substep_count(&fast, one_second), 40);
///
/// // A zero-length step needs no splitting either.
/// assert_eq!(substep_count(&fast, TimeStep::ZERO), 1);
/// ```
pub fn substep_count(props: &[Prop], step: TimeStep) -> u32 {
    if step.is_zero() || !step.is_valid() {
        return 1;
    }

    let dt = step.seconds_f32();

    let fastest = props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(Prop::speed)
        .filter(|speed| speed.is_finite())
        .fold(0.0_f64, f64::max);

    let travel = fastest * step.seconds();
    let cap = f64::from(MAX_SUBSTEP_DISPLACEMENT_UNITS);
    let for_contact = if travel.is_finite() && travel > cap {
        (travel / cap) as f32
    } else {
        1.0
    };

    // Whichever criterion is stricter. They answer different questions and neither subsumes
    // the other — see the doc comment.
    let needed = for_contact.max(orbital_substeps(props, dt));

    if !needed.is_finite() || needed >= MAX_SUBSTEPS as f32 {
        MAX_SUBSTEPS
    } else if needed <= 1.0 {
        1
    } else {
        // `needed > 1.0` here, so the cast cannot round down to zero.
        needed.ceil() as u32
    }
}

/// How many substeps the tightest **orbit** in `props` needs over a step of `dt` seconds.
///
/// # Why displacement is not enough
///
/// [`substep_count`]'s original criterion asks *how far does the fastest body move*, which is
/// the right question for contact: a prop must not cross the thing it is hitting between two
/// overlap tests. It is the wrong question for an orbit, which needs steps that are short
/// compared to the time the orbit takes — a quantity with no radius in it at all.
///
/// Jupiter's moons are where the difference became fatal. At `experiment_3`'s clock, one frame
/// advances `16,667` simulated seconds and Io's orbital period is `152,870` — so **one frame is
/// `10.9%` of Io's entire orbit**, and it takes the moon `69%` of the way across its own orbital
/// radius. The displacement rule sees `0.507` units of travel against a `0.25`-unit cap and asks
/// for **3** substeps, giving 28 integration steps per orbit; a first-order symplectic
/// integrator needs a few hundred to trace a closed ellipse, and at 28 Io spirals away. Nothing
/// about that is a contact problem, so nothing about it was visible to the old rule.
///
/// # The criterion
///
/// For each pair, the local **dynamical time** `t_dyn = √(r³ / (G·(m₁ + m₂)))` — the orbital
/// period divided by `2π`, and the natural timescale of gravitation at that separation. The
/// substep is required to be at most [`MAX_SUBSTEP_DYNAMICAL_FRACTION`] of the smallest `t_dyn`
/// in the scene, which fixes the steps per orbit rather than the distance per step.
///
/// This is what N-body codes do, and it is *self-tuning*: a scene with no tight orbit is
/// unaffected and pays one `O(n²)` pass of cheap arithmetic. The planets of `experiment_3` ask `2` of
/// it, under the `4` the displacement rule already wanted; the same scene **with the Galilean moons**
/// asks for about `98`, which measures at `3.2 ms` a frame in release.
///
/// # Returns
///
/// A count as `f32`, `1.0` or greater, for [`substep_count`] to combine with the contact
/// criterion and clamp. Pairs with no mass, a non-finite separation, or zero separation
/// contribute nothing — gravitation has nothing to resolve there either.
fn orbital_substeps(props: &[Prop], dt: f32) -> f32 {
    let mut shortest_dynamical_time = f32::INFINITY;

    for (index, subject) in props.iter().enumerate() {
        for source in &props[index + 1..] {
            // The pair has dynamics only if one of them can be moved **by the other**: one must
            // respond to forces and the other must have mass to exert them. Both halves matter, and
            // getting this wrong was a real bug.
            //
            // The shipped floor is immovable *and* massless — it can neither move nor pull — and it
            // sits `30` units below the Sun. A weaker test ("at least one can move") passed that
            // pair, and `√(r³/G·M_sun)` over `30` units is short enough that the criterion demanded
            // **6** substeps of every scene containing a Sun and a floor, for two props that do not
            // interact at all. `experiment_3` paid it for nothing until a test asserting the
            // planets-only count came out at `6` instead of `4`.
            let interacts = (subject.responds_to_forces() && source.mass_kg > 0.0)
                || (source.responds_to_forces() && subject.mass_kg > 0.0);
            if !interacts {
                continue;
            }

            let total_mass = f64::from(subject.mass_kg) + f64::from(source.mass_kg);
            if total_mass <= 0.0 {
                continue;
            }

            let separation = (source.body.center() - subject.body.center()).length();
            if !separation.is_finite() || separation <= 0.0 {
                continue;
            }

            // In SI, like every other gravitational quantity in the crate: `r³` at
            // astronomical scales overflows `f32` long before it means anything.
            let separation_m = separation * METRES_PER_UNIT;
            let dynamical_time =
                (separation_m.powi(3) / (GRAVITATIONAL_CONSTANT * total_mass)).sqrt() as f32;

            if dynamical_time.is_finite() && dynamical_time > 0.0 {
                shortest_dynamical_time = shortest_dynamical_time.min(dynamical_time);
            }
        }
    }

    if !shortest_dynamical_time.is_finite() {
        return 1.0;
    }

    let allowed = shortest_dynamical_time * MAX_SUBSTEP_DYNAMICAL_FRACTION;
    if allowed <= 0.0 || !allowed.is_finite() {
        return 1.0;
    }

    (dt / allowed).max(1.0)
}

/// Can this pair of props take part in a contact at all?
///
/// True when at least one of them can move, neither is a movable massless prop,
/// and they are not the same prop. This is the filter that implements the table
/// in the module docs, before any geometry is built.
fn pair_can_interact(a: &Prop, b: &Prop) -> bool {
    let participates = |prop: &Prop| prop.immovable || prop.responds_to_forces();
    // Two walls cannot collide: neither can move and neither can be pushed.
    let both_fixed = a.immovable && b.immovable;
    participates(a) && participates(b) && !both_fixed
}

/// Resolve every contact among `props`: bounce elastically, then push apart.
///
/// The body of the collision half of [`Scene::step`](crate::core::scene::Scene::step).
/// Builds each prop's hull once, tests every pair, and applies a response to each
/// overlapping pair it finds.
///
/// # Parameters
///
/// - `props` — the scene's props, mutated in place. Immovable props are never
///   moved and never have their velocity touched, so their `Vec3::ZERO` invariant
///   survives.
///
/// # Returns
///
/// How many contacts were resolved. `0` means nothing was touching — the common
/// case, and cheap to reach thanks to the broad phase in [`may_touch`].
///
/// # One pass, in index order
///
/// Pairs are visited in `(i, j)` order with `i < j` and each is resolved
/// immediately, so a later pair sees the corrections made by an earlier one. The
/// two hulls involved are rebuilt after each response for exactly that reason —
/// a stale hull would place a body where it no longer is.
///
/// This makes the result order-dependent when three or more props overlap at
/// once, which is the "no resting contact" limitation in the module docs. For the
/// scenes here — pairs meeting one at a time — a single pass is exact.
///
/// # Cost
///
/// `n·(n−1)/2` broad-phase tests, each a few dozen flops, then a tessellation per prop and
/// a separating-axis test per pair that survives. Tessellation dominates whenever a pair
/// survives, so the broad phase has to be tight against large shapes: a bounding sphere
/// around a 5000-unit floor contains the whole solar system, which is why box-shaped props
/// are tested as boxes.
pub fn resolve_contacts(props: &mut [Prop]) -> usize {
    // Hulls are built **lazily**, one per prop at most, and only for props that survive the
    // broad phase below. Tessellating an icosphere is by far the most expensive thing
    // this function can do, and in a scene where nothing is touching it is entirely wasted.
    //
    // That waste used to be invisible because the shipped scenes ran at one substep. It stopped
    // being invisible when `substep_count` gained a dynamical-time criterion: the solar system
    // with Jupiter's moons in it takes about 98 substeps per frame, which under the old
    // build-everything-up-front approach meant 1,274 icospheres per frame for a scene whose
    // closest pair is 0.42 units apart with a combined radius of 0.07.
    let mut hulls: Vec<Option<TriMesh>> = vec![None; props.len()];
    let mut built = vec![false; props.len()];

    let mut resolved = 0;

    for i in 0..props.len() {
        for j in (i + 1)..props.len() {
            if !pair_can_interact(&props[i], &props[j]) {
                continue;
            }

            if !may_touch(&props[i], &props[j]) {
                continue;
            }

            ensure_hull(&mut hulls, &mut built, props, i);
            ensure_hull(&mut hulls, &mut built, props, j);

            // Scoped so the hull borrows end before the response mutates `props`.
            let contact = match (hulls[i].as_ref(), hulls[j].as_ref()) {
                (Some(hull_a), Some(hull_b)) => contact_between_meshes(hull_a, hull_b),
                _ => None,
            };

            let Some(contact) = contact else {
                continue;
            };

            apply_contact(props, i, j, &contact);
            resolved += 1;

            // Both props may have moved, so their hulls are stale. Rebuilding is
            // cheaper than reasoning about which later pairs could be affected.
            hulls[i] = contact_mesh(&props[i]);
            hulls[j] = contact_mesh(&props[j]);
        }
    }

    resolved
}

/// Build the hull for `props[index]` if it has not been built already.
///
/// The laziness is the point: a prop in a scene where nothing touches is never tessellated at
/// all. `built` is separate from `hulls[index].is_some()` because `None` is a legitimate result —
/// a zero-width line has no hull — and re-attempting it every pair would defeat the caching.
fn ensure_hull(hulls: &mut [Option<TriMesh>], built: &mut [bool], props: &[Prop], index: usize) {
    if !built[index] {
        hulls[index] = contact_mesh(&props[index]);
        built[index] = true;
    }
}

/// Could these two props possibly be touching? The broad phase, run **before** any tessellation.
///
/// Conservative: it never rejects a pair whose hulls the narrow phase would find overlapping.
/// Each test is a necessary condition for overlap and only rejects when it can bound both
/// shapes, so their conjunction is one too.
fn may_touch(a: &Prop, b: &Prop) -> bool {
    bounding_spheres_overlap(a, b) && sphere_reaches_box(a, b) && sphere_reaches_box(b, a)
}

/// Does `other`'s bounding sphere reach `boxed`'s hull, when that hull is a box?
///
/// Exact for a sphere against a `Cuboid` or a `Plane` slab: it measures from the sphere's centre
/// to the closest point of the box. `true` when `boxed` is not box-shaped or `other` has no
/// bounding radius, which leaves the decision to the other tests.
fn sphere_reaches_box(boxed: &Prop, other: &Prop) -> bool {
    let (Some(frame), Some(radius)) = (hull_box(&boxed.body), bounding_radius(other)) else {
        return true;
    };

    // glam rotates by `q·v·q*`, which for a quaternion off unit length (validation allows
    // `ROTATION_UNIT_TOLERANCE`) is a rotation by `q/|q|` scaled by `|q|²`. The hull is built
    // from the raw quaternion, so this is the box it actually is.
    let scale = frame.rotation.length_squared();
    let extent = frame.half_extents * scale;
    let point = other.body.center().as_vec3();
    let local = frame.rotation.normalize().conjugate() * (point - frame.centre);
    let gap = (local - local.clamp(-extent, extent)).length();

    // The hull's vertices and the narrow phase's projections are rounded at the magnitude of
    // the coordinates, not of the gap, so a sphere grazing a 5000-unit floor needs slack
    // proportional to 5000.
    let magnitude = frame.centre.abs().max_element() + point.abs().max_element() + extent.length();
    let slack = BROAD_PHASE_RELATIVE_SLACK * (magnitude + radius);

    !gap.is_finite() || gap <= radius + slack
}

/// Could these two props possibly be touching, judged by bounding spheres alone?
///
/// A conservative pre-filter for [`resolve_contacts`], run **before** any tessellation. Exact for
/// two spheres, which is every body in every experiment that has a tight orbit; conservative for
/// the other variants, which only means the narrow phase is consulted slightly more often than it
/// strictly must be.
///
/// Returns `true` when either prop has no finite bounding radius, so an unusual body is passed
/// through to the narrow phase rather than silently excluded from collision.
fn bounding_spheres_overlap(a: &Prop, b: &Prop) -> bool {
    let (Some(radius_a), Some(radius_b)) = (bounding_radius(a), bounding_radius(b)) else {
        return true;
    };

    let separation = (b.body.center() - a.body.center()).length();
    if !separation.is_finite() {
        return true;
    }

    separation <= f64::from(radius_a + radius_b)
}

/// A radius that certainly contains this prop's physical extent, in world units.
///
/// Uses [`Prop::contact_radius_units`] when set, so a body drawn larger than it is — Jupiter, at
/// `157×` life size before amendment A16 — is filtered on its *physical* size, exactly as the
/// hull would be. `None` for a body whose extent cannot be bounded cheaply, which sends the pair
/// to the narrow phase.
fn bounding_radius(prop: &Prop) -> Option<f32> {
    // `|q|²` is how much a not-quite-unit pose rotation stretches the hull; see
    // `sphere_reaches_box`.
    let radius = match &prop.body {
        Body::Sphere { pose, radius, .. } => {
            prop.contact_radius_units.unwrap_or(*radius) * pose.rotation.length_squared()
        }
        // Half the diagonal of the box, which contains it under any rotation.
        Body::Cuboid {
            pose, half_extents, ..
        } => half_extents.length() * pose.rotation.length_squared(),
        // A plane is a slab: its footprint plus the thickness the hull gives it.
        Body::Plane {
            pose, half_size, ..
        } => {
            (*half_size * *half_size * 2.0 + PLANE_THICKNESS_UNITS * PLANE_THICKNESS_UNITS).sqrt()
                * pose.rotation.length_squared()
        }
        Body::Line {
            from, to, width, ..
        } => 0.5 * (*to - *from).length() as f32 + *width,
    };

    if radius.is_finite() && radius >= 0.0 {
        Some(radius)
    } else {
        None
    }
}

/// Apply one contact to the pair at `(i, j)`: elastic bounce, then separation.
///
/// `contact.normal` points from `props[i]` toward `props[j]`, matching what
/// [`contact_between_meshes`] returned.
///
/// # Preconditions and release behaviour
///
/// `i != j`, both in bounds, `contact.is_valid()`, and the pair passes
/// [`pair_can_interact`]. Violations fire a `debug_assert!`; in release the
/// function returns without touching anything, because a bad impulse corrupts a
/// simulation in a way that is far harder to notice than a missed bounce.
fn apply_contact(props: &mut [Prop], i: usize, j: usize, contact: &Contact) {
    debug_assert!(
        i < j && j < props.len(),
        "apply_contact: need i < j < props.len(), got i={}, j={}, len={}",
        i,
        j,
        props.len()
    );
    debug_assert!(
        contact.is_valid(),
        "apply_contact: contact fails Validation_Rules: {:?}",
        contact
    );

    if i >= j || j >= props.len() || !contact.is_valid() {
        return;
    }

    // `split_at_mut` is how two `&mut` into one slice are obtained safely. `i < j`
    // is what makes the split point correct, which is why it is a precondition
    // rather than something to sort out here.
    let (head, tail) = props.split_at_mut(j);
    let a = &mut head[i];
    let b = &mut tail[0];

    let normal = contact.normal.as_dvec3();

    // Negative means the pair is closing along the normal. A pair that is already
    // separating gets no impulse — it is on its way out, and reversing it again
    // would trap it in a contact it can never leave.
    let approach = (b.velocity_units_s - a.velocity_units_s).dot(normal);

    if approach < 0.0 {
        match (a.immovable, b.immovable) {
            // A movable prop against a wall: the infinite-mass limit of the
            // elastic solution, where the wall's outgoing velocity is unchanged
            // and the movable prop's normal component is negated.
            (false, true) => reflect_off_wall(a, normal),
            (true, false) => reflect_off_wall(b, normal),
            // Two movable props with positive mass — guaranteed by
            // `pair_can_interact`. Both velocities are resolved onto the contact
            // normal, handed to the 1-D elastic solver, and the *change* is put
            // back along the normal. Only the normal component is touched, so the
            // tangential motion survives untouched (there is no friction).
            (false, false) => {
                let normal_a = a.velocity_units_s.dot(normal);
                let normal_b = b.velocity_units_s.dot(normal);

                // Velocities are in units/s rather than m/s. The elastic solution
                // is linear in velocity and scale-free in the length unit, so
                // feeding it units/s yields units/s — no conversion needed, and
                // `METRES_PER_UNIT` deliberately does not appear here. Masses are
                // already SI.
                let (outgoing_a, outgoing_b) =
                    elastic_collision_1d(a.mass_kg as f64, normal_a, b.mass_kg as f64, normal_b);

                a.velocity_units_s += (outgoing_a - normal_a) * normal;
                b.velocity_units_s += (outgoing_b - normal_b) * normal;
            }
            // Two walls. Filtered out before we get here.
            (true, true) => {}
        }
    }

    separate(a, b, normal, contact.depth);
}

/// Negate `prop`'s velocity component along `normal`, scaled by
/// [`RESTITUTION`].
///
/// The infinite-mass limit of [`elastic_collision_1d`]: as one mass grows without
/// bound the heavy body is undeflected and the light one's approach speed is
/// returned in full. Written directly rather than by passing a huge mass to the
/// solver, because "a very large number" is not the same claim as "immovable" and
/// the limit is exact.
fn reflect_off_wall(prop: &mut Prop, normal: DVec3) {
    let along_normal = prop.velocity_units_s.dot(normal);
    prop.velocity_units_s -= (1.0 + f64::from(RESTITUTION)) * along_normal * normal;
}

/// Push the pair apart along `normal` by `depth`, split by inverse mass.
///
/// Runs whether or not an impulse was applied: the overlap is a fact about the
/// current positions, and leaving it in place lets penetration accumulate across
/// steps until a body sinks through. The heavier prop moves less, in the ratio
/// `1/m` — the same weighting the impulse uses, so a light prop bouncing off a
/// heavy one is pushed out of it rather than the other way about. An immovable
/// prop does not move at all and the movable one takes the whole correction.
///
/// This is a position change with no velocity change, so it is not momentum
/// conserving and it does add a little energy. That is the standard tradeoff for
/// projection-based separation, and it is bounded by the depth: at the depths a
/// substepped scene produces the addition is far below the integrator's own
/// first-order error.
fn separate(a: &mut Prop, b: &mut Prop, normal: DVec3, depth: f32) {
    if !depth.is_finite() || depth <= 0.0 {
        return;
    }

    // Zero inverse mass is the infinite-mass limit: an immovable prop, or one
    // whose mass cannot be inverted, absorbs none of the correction. The
    // finiteness test comes first so a `NaN` mass lands here rather than slipping
    // through the comparison.
    let inverse_mass = |prop: &Prop| {
        if prop.immovable || !prop.mass_kg.is_finite() || prop.mass_kg <= 0.0 {
            0.0
        } else {
            1.0 / prop.mass_kg
        }
    };

    let weight_a = inverse_mass(a);
    let weight_b = inverse_mass(b);
    let total = weight_a + weight_b;

    // Both effectively immobile: nothing to share the correction between, and any
    // move would be arbitrary.
    if !total.is_finite() || total <= 0.0 {
        return;
    }

    let share_a = depth * (weight_a / total);
    let share_b = depth * (weight_b / total);

    if share_a > 0.0 {
        a.body.translate(-normal * f64::from(share_a));
    }
    if share_b > 0.0 {
        b.body.translate(normal * f64::from(share_b));
    }
}

#[cfg(test)]
#[path = "../../tst/collision/resolve_tests.rs"]
mod tests;
