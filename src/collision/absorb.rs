//! Absorption: what happens when something reaches a black hole.
//!
//! The rest of this layer resolves contact **elastically** — two hulls meet, they bounce,
//! momentum and energy are conserved. That is right for two solid bodies and wrong in the
//! strongest possible sense for an event horizon: a star that reaches one does not come
//! back. So a prop with [`absorbs`](crate::core::scene::Prop::absorbs) set gets a different
//! response, implemented here:
//! it **takes** what it touches.
//!
//! # The merge
//!
//! For an absorber `A` and a victim `V` whose hulls overlap:
//!
//! ```text
//! m'  = m_A + m_V                          mass is conserved
//! v'  = (m_A·v_A + m_V·v_V) / (m_A + m_V)   momentum is conserved
//! r'  = r_A · (m' / m_A)                    the horizon grows with mass
//! ```
//!
//! and `V` leaves the scene. Three things about that are worth stating plainly, because
//! each is a choice:
//!
//! - **Momentum is conserved; kinetic energy is not.** This is a perfectly inelastic
//!   collision, so the energy lost to the merge is real physics rather than an integration
//!   error. A black hole absorbing an infalling star radiates that energy away in the real
//!   universe; here it simply leaves the books, which is the same thing from the
//!   simulation's point of view.
//! - **The absorber does not move to the barycentre.** Strictly, the centre of mass should
//!   stay put, which for a `10 M☉` hole eating a `1 M☉` star means shifting `9%` of the way
//!   toward the star. It is left where it is, because a black hole that visibly *lurches*
//!   each time it eats something looks like a bug — and the shift is a fraction of the
//!   drawn radius. The momentum, which is what governs everything afterwards, is exact.
//! - **The horizon grows linearly with mass**, which is correct: the Schwarzschild radius
//!   is `2GM/c²`. Since A17 removed every size exaggeration the drawn radius is the real one, so
//!   this is now right in absolute terms as well as proportionally — the two sentences that used
//!   to qualify it (`27,000×` life size) were left behind by that amendment.
//! - **It does *not* grow the absorption radius.** A black hole captures at
//!   [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units) — its tidal
//!   disruption radius, three orders of magnitude outside its horizon — and `merge` leaves that
//!   field alone. So a fed black hole does **not** eat more easily, which is the opposite of what
//!   these docs claimed until it was checked. The tidal radius goes as `M^(1/3)`, so eating a Sun
//!   leaves a `10 M☉` hole's capture radius `3.2%` stale; a meal that doubled its mass would leave
//!   it `26%` stale. Recorded rather than fixed: growing it is a physics decision about what a
//!   capture radius means, not a typo.
//!
//! # Two absorbers meeting
//!
//! The more massive one wins and the other is absorbed. Ties break toward the lower index,
//! which is arbitrary but deterministic — and two black holes of exactly equal mass meeting
//! is a case the physics does not distinguish either.
//!
//! # An immovable absorber keeps its velocity at zero
//!
//! `Prop`'s invariant is that an immovable prop has exactly `Vec3::ZERO` velocity, so an
//! immovable absorber takes the mass and discards the momentum rather than breaking it. That
//! is a documented departure from conservation, and it is what "immovable" already means
//! everywhere else in the crate: the thing is nailed down, and the simulation is not going to
//! argue with it.
//!
//! # What this is not
//!
//! Not gravitational-wave emission, not an accretion disc, not tidal disruption — a star is
//! swallowed whole and instantly the moment its hull touches. Not relativity of any kind:
//! see [`crate::experiments::objects`] for where Newtonian gravity stops being defensible
//! around these objects.

use crate::collision::contact::contact_between_props;
use crate::core::scene::{Body, BodyId, Scene};

/// Merge every prop that is touching an absorber into it, and remove the absorbed.
///
/// The absorption half of [`Scene::step`](crate::core::scene::Scene::step), run **before**
/// the elastic pass so a prop swallowed by a black hole is gone before anything can bounce
/// off it.
///
/// # Parameters
///
/// - `scene` — mutated in place. Takes the whole [`Scene`] rather than `&mut [Prop]` (which
///   is what [`resolve_contacts`](crate::collision::resolve::resolve_contacts) takes)
///   because absorption *removes* props, and a slice cannot shrink.
///
/// # Returns
///
/// How many props were absorbed. `0` is the overwhelmingly common case and costs one
/// pairwise pass over the scene.
///
/// # Order, and why one pass is enough
///
/// Pairs are visited in `(i, j)` order with `i < j`, and each absorption is applied
/// immediately, so a black hole that swallows two props in one substep grows before the
/// second merge — which is the physically sensible reading of "it ate both". Removal happens
/// **after** the scan, by `BodyId`, so no index shifts underneath the loop.
///
/// A prop already absorbed in this pass is skipped rather than absorbed twice, which is what
/// makes the "two absorbers overlapping a third" case well-defined: the first absorber to
/// reach it gets it.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{absorb_contacts, Body, Color, Pose, Prop, Quat, Scene, Vec3};
///
/// let ball = |x: f32, radius: f32| Body::Sphere {
///     pose: Pose { position: Vec3::new(x, 0.0, 0.0), rotation: Quat::IDENTITY },
///     radius,
///     color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
/// };
///
/// let mut scene = Scene::demo();
/// // A black hole at the origin, and a star already touching it.
/// scene.add(Prop::absorbing(ball(0.0, 1.0), Vec3::ZERO, 10.0));
/// scene.add(Prop::new(ball(1.5, 1.0), Vec3::new(1.0, 0.0, 0.0), 5.0));
///
/// assert_eq!(absorb_contacts(&mut scene), 1);
/// assert_eq!(scene.props.len(), 2, "the floor and the black hole are left");
///
/// // Mass and momentum both conserved: 5 kg at 1 unit/s into a stationary 10 kg.
/// let hole = &scene.props[1];
/// assert_eq!(hole.mass_kg, 15.0);
/// assert!((hole.velocity_units_s.x - 1.0 / 3.0).abs() < 1.0e-6);
/// ```
pub fn absorb_contacts(scene: &mut Scene) -> usize {
    // Nothing to do in the common case, and worth the early check: this runs every substep
    // of every experiment, and only one experiment can ever have an absorber in it.
    if !scene.props.iter().any(|prop| prop.absorbs) {
        return 0;
    }

    let ids: Vec<BodyId> = scene.iter().map(|(id, _)| id).collect();
    let mut absorbed: Vec<BodyId> = Vec::new();

    for i in 0..scene.props.len() {
        for j in (i + 1)..scene.props.len() {
            // Skip anything already eaten in this pass: it is still in `props` (removal is
            // deferred so indices stay stable) but it is no longer really there.
            if absorbed.contains(&ids[i]) || absorbed.contains(&ids[j]) {
                continue;
            }

            let Some((absorber, victim)) = absorber_and_victim(scene, i, j) else {
                continue;
            };

            if contact_between_props(&scene.props[i], &scene.props[j]).is_none() {
                continue;
            }

            merge(scene, absorber, victim);
            absorbed.push(ids[victim]);
        }
    }

    for id in &absorbed {
        scene.remove(*id);
    }

    absorbed.len()
}

/// Which of the pair absorbs the other, as `(absorber, victim)` indices, or `None` when
/// nothing should be absorbed.
///
/// Neither absorbing is the common case. Both absorbing means the **more massive** one
/// wins, with ties breaking toward the lower index — arbitrary, deterministic, and a case
/// the physics does not distinguish either.
///
/// # An immovable prop is never the victim
///
/// A black hole placed just above the ground plane would otherwise **eat the floor**, which
/// is not a physics result but a category error: an immovable prop is the container the
/// simulation happens inside, not a participant in it. The same reasoning
/// [`pair_can_interact`](crate::collision::resolve) already applies to two fixed props —
/// neither can move, so there is nothing to resolve — extends here.
///
/// This was found by a test rather than by inspection: the first version deleted the ground
/// plane the moment a black hole was placed near it, and took the scene's only immovable prop
/// with it.
///
/// An immovable *absorber* is still allowed, and takes the mass while keeping its zero
/// velocity — see the module docs.
fn absorber_and_victim(scene: &Scene, i: usize, j: usize) -> Option<(usize, usize)> {
    let (first, second) = (&scene.props[i], &scene.props[j]);

    let (absorber, victim) = match (first.absorbs, second.absorbs) {
        (false, false) => return None,
        (true, false) => (i, j),
        (false, true) => (j, i),
        (true, true) => {
            if second.mass_kg > first.mass_kg {
                (j, i)
            } else {
                (i, j)
            }
        }
    };

    if scene.props[victim].immovable {
        return None;
    }

    Some((absorber, victim))
}

/// Fold the prop at `victim` into the one at `absorber`: mass, momentum, and horizon.
///
/// Leaves the victim in place — [`absorb_contacts`] removes it after the scan, so indices
/// stay stable for the rest of the pass.
///
/// # Preconditions and release behaviour
///
/// `absorber != victim`, both in bounds, and `scene.props[absorber].absorbs`. Violations
/// fire a `debug_assert!`; in release the function returns without touching anything,
/// because a bad merge silently corrupts a simulation in a way that is far harder to notice
/// than a missed absorption.
fn merge(scene: &mut Scene, absorber: usize, victim: usize) {
    debug_assert!(
        absorber != victim && absorber < scene.props.len() && victim < scene.props.len(),
        "merge: need distinct in-bounds indices, got {} and {} of {}",
        absorber,
        victim,
        scene.props.len()
    );
    debug_assert!(
        scene.props[absorber].absorbs,
        "merge: props[{}] does not absorb",
        absorber
    );

    if absorber == victim
        || absorber >= scene.props.len()
        || victim >= scene.props.len()
        || !scene.props[absorber].absorbs
    {
        return;
    }

    let (victim_mass, victim_velocity) = {
        let prop = &scene.props[victim];
        (prop.mass_kg, prop.velocity_units_s)
    };

    let prop = &mut scene.props[absorber];
    let combined_mass = prop.mass_kg + victim_mass;

    // Momentum in `f64`: the masses here span from a `2e30` neutron star to an `8.5e36`
    // supermassive black hole, and the sum of two products at that magnitude loses digits
    // in `f32` exactly where the ratio between them matters.
    if combined_mass > 0.0 && combined_mass.is_finite() {
        if prop.immovable {
            // An immovable prop has exactly `Vec3::ZERO` velocity, by `Prop`'s own
            // Validation_Rules. It takes the mass and discards the momentum rather than
            // breaking that invariant — see the module docs.
        } else {
            let total = f64::from(prop.mass_kg) + f64::from(victim_mass);
            let blend = |a: f32, b: f32| {
                ((f64::from(prop.mass_kg) * f64::from(a) + f64::from(victim_mass) * f64::from(b))
                    / total) as f32
            };

            prop.velocity_units_s = crate::core::scene::Vec3::new(
                blend(prop.velocity_units_s.x, victim_velocity.x),
                blend(prop.velocity_units_s.y, victim_velocity.y),
                blend(prop.velocity_units_s.z, victim_velocity.z),
            );
        }

        // The horizon grows with mass: `r_s = 2GM/c²` is linear in `M`. Applied to the drawn
        // radius, which since A17 is the real one — so this is right absolutely, not merely
        // proportionally. It does **not** touch `contact_radius_units`, so a black hole's capture
        // radius does not grow with what it eats; see the module docs for why that is recorded as
        // a limitation rather than patched here.
        let growth = combined_mass / prop.mass_kg;
        if let Body::Sphere { radius, .. } = &mut prop.body {
            if growth.is_finite() && growth >= 1.0 {
                *radius *= growth;
            }
        }

        prop.mass_kg = combined_mass;
    }

    debug_assert!(
        scene.props[absorber].is_valid(),
        "merge: absorber left its Validation_Rules: {:?}",
        scene.props[absorber]
    );
}

#[cfg(test)]
#[path = "../../tst/collision/absorb_tests.rs"]
mod tests;
