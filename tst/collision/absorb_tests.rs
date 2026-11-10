//! Unit tests for the [`crate::collision::absorb`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/collision/absorb.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! # What is worth asserting about a merge
//!
//! Absorption is the crate's second contact response and the first operation that **removes**
//! a prop, so the tests split three ways: the conservation laws it claims to obey (mass
//! exactly, momentum to floating-point tolerance, energy not at all), the bookkeeping around
//! removal (ids never come back, `props` and `ids` stay in lockstep, the surviving props keep
//! their identity), and the cases where "who absorbs whom" is a decision rather than an
//! obvious answer.
//!
//! Deliberately not asserted: that a black hole *eventually* eats an orbiting body. That is a
//! statement about a trajectory, and this crate does not pin trajectories — a first-order
//! integrator's error is what would be under test.

use super::*;
use crate::core::scene::{Body, Color, DVec3, Pose, Prop, Quat, Vec3};

/// A unit-white sphere at `x`, with radius `radius`.
///
/// Lifted to `y = 2` so it clears the shared floor's collision slab (`y ∈ [−1, 0]`). At
/// `y = 0` a unit sphere overlaps the floor, which quietly turns every test below into a
/// three-body contact problem — and was what first showed that a black hole would otherwise
/// swallow the ground plane.
fn ball(x: f64, radius: f32) -> Body {
    Body::Sphere {
        pose: Pose {
            position: DVec3::new(x, 2.0, 0.0),
            rotation: Quat::IDENTITY,
        },
        radius,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    }
}

/// A scene with an absorber at the origin and a victim overlapping it.
///
/// Radii of `1.0` at a separation of `1.5`, so the hulls genuinely overlap — the icosphere
/// hull inscribes the sphere, so a separation just under `2.0` is not enough to guarantee
/// contact and this leaves margin.
fn touching(absorber_mass: f32, victim_mass: f32, victim_velocity: DVec3) -> Scene {
    let mut scene = Scene::demo();
    scene.add(Prop::absorbing(ball(0.0, 1.0), DVec3::ZERO, absorber_mass));
    scene.add(Prop::new(ball(1.5, 1.0), victim_velocity, victim_mass));
    scene
}

fn radius_of(scene: &Scene, index: usize) -> f32 {
    match &scene.props[index].body {
        Body::Sphere { radius, .. } => *radius,
        other => panic!("expected a sphere, got {other:?}"),
    }
}

// -----------------------------------------------------------------------
// The common case: nothing absorbs.
// -----------------------------------------------------------------------

#[test]
fn a_scene_with_no_absorber_is_untouched() {
    // The overwhelmingly common case — every experiment that ships. It must also be *cheap*:
    // the early return means no hull is ever built.
    let mut scene = Scene::two_body_gravity();
    let before = scene.clone();

    assert_eq!(absorb_contacts(&mut scene), 0);
    assert_eq!(scene, before);
}

#[test]
fn an_absorber_that_touches_nothing_absorbs_nothing() {
    let mut scene = Scene::demo();
    scene.add(Prop::absorbing(ball(0.0, 1.0), DVec3::ZERO, 10.0));
    scene.add(Prop::new(ball(50.0, 1.0), DVec3::ZERO, 5.0));

    assert_eq!(absorb_contacts(&mut scene), 0);
    assert_eq!(scene.props.len(), 3);
}

// -----------------------------------------------------------------------
// The merge, and what it conserves.
// -----------------------------------------------------------------------

#[test]
fn the_victim_leaves_and_the_absorber_takes_its_mass() {
    let mut scene = touching(10.0, 5.0, DVec3::ZERO);
    let victim_id = scene.iter().nth(2).map(|(id, _)| id).unwrap();

    assert_eq!(absorb_contacts(&mut scene), 1);

    assert_eq!(scene.props.len(), 2, "the floor and the absorber remain");
    assert!(scene.get(victim_id).is_none(), "the victim is gone");
    assert_eq!(scene.props[1].mass_kg, 15.0, "mass is conserved exactly");
}

#[test]
fn momentum_is_conserved() {
    // 5 kg at 2 units/s into a stationary 10 kg gives 15 kg at 2/3 units/s. The interesting
    // property is that this is the *inelastic* answer, not the elastic one the rest of the
    // layer computes.
    let mut scene = touching(10.0, 5.0, DVec3::new(2.0, 0.0, 0.0));

    let before: DVec3 = scene
        .props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(|prop| prop.velocity_units_s * f64::from(prop.mass_kg))
        .sum();

    absorb_contacts(&mut scene);

    let after: DVec3 = scene
        .props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(|prop| prop.velocity_units_s * f64::from(prop.mass_kg))
        .sum();

    assert!(
        (after - before).length() / before.length() < 1.0e-5,
        "momentum went from {before:?} to {after:?}"
    );
    assert!((scene.props[1].velocity_units_s.x - 2.0 / 3.0).abs() < 1.0e-6);
}

#[test]
fn kinetic_energy_is_not_conserved_and_that_is_the_point() {
    // A perfectly inelastic collision loses energy, which distinguishes absorption from every
    // other contact in this crate. If this ever passed with equality, absorption would have
    // silently become an elastic bounce.
    let mut scene = touching(10.0, 5.0, DVec3::new(2.0, 0.0, 0.0));

    let energy = |scene: &Scene| -> f64 {
        scene
            .props
            .iter()
            .filter(|prop| !prop.immovable)
            .map(|prop| 0.5 * f64::from(prop.mass_kg) * prop.speed().powi(2))
            .sum()
    };

    let before = energy(&scene);
    absorb_contacts(&mut scene);
    let after = energy(&scene);

    assert!(after < before, "energy {before} -> {after}");
}

#[test]
fn a_moving_absorber_is_slowed_by_what_it_swallows() {
    // Both moving, head-on. The absorber keeps going — it is twice the mass — but slower.
    let mut scene = Scene::demo();
    scene.add(Prop::absorbing(
        ball(0.0, 1.0),
        DVec3::new(1.0, 0.0, 0.0),
        10.0,
    ));
    scene.add(Prop::new(ball(1.5, 1.0), DVec3::new(-1.0, 0.0, 0.0), 5.0));

    absorb_contacts(&mut scene);

    let velocity = scene.props[1].velocity_units_s;
    assert!(velocity.x > 0.0, "the heavier body still leads");
    assert!(velocity.x < 1.0, "but it has been slowed");
    assert!((velocity.x - (10.0 - 5.0) / 15.0).abs() < 1.0e-6);
}

#[test]
fn the_horizon_grows_with_the_mass_it_swallows() {
    // `r_s = 2GM/c²` is linear in mass, so doubling the mass doubles the radius. Applied to
    // the drawn radius, which is 27,000x life size for a stellar hole — proportionally right,
    // absolutely wrong, and documented as both.
    let mut scene = touching(10.0, 10.0, DVec3::ZERO);
    let before = radius_of(&scene, 1);

    absorb_contacts(&mut scene);

    assert_eq!(scene.props[1].mass_kg, 20.0);
    assert!(
        (radius_of(&scene, 1) / before - 2.0).abs() < 1.0e-5,
        "radius went from {before} to {}",
        radius_of(&scene, 1)
    );
}

#[test]
fn a_negligible_meal_leaves_the_horizon_effectively_unchanged() {
    // Sgr A* eating an Earth: the growth is real but 1e-12, so the drawn radius must not
    // visibly jump. This is the case that would expose a growth rule using the wrong power.
    let mut scene = touching(8.5467e36, 5.972e24, DVec3::ZERO);
    let before = radius_of(&scene, 1);

    absorb_contacts(&mut scene);

    assert!((radius_of(&scene, 1) / before - 1.0).abs() < 1.0e-6);
}

// -----------------------------------------------------------------------
// Who absorbs whom.
// -----------------------------------------------------------------------

#[test]
fn the_more_massive_absorber_wins_when_two_meet() {
    // Two black holes merging. The heavier one survives, whichever order they are in.
    for (first, second, survivor_mass) in [(10.0_f32, 30.0_f32, 40.0_f32), (30.0, 10.0, 40.0)] {
        let mut scene = Scene::demo();
        scene.add(Prop::absorbing(ball(0.0, 1.0), DVec3::ZERO, first));
        scene.add(Prop::absorbing(ball(1.5, 1.0), DVec3::ZERO, second));

        assert_eq!(absorb_contacts(&mut scene), 1);
        assert_eq!(scene.props.len(), 2);
        assert_eq!(scene.props[1].mass_kg, survivor_mass);
        assert!(
            scene.props[1].absorbs,
            "the survivor of two black holes is still a black hole"
        );
    }
}

#[test]
fn one_absorber_can_take_two_props_in_a_single_pass() {
    // Both sides at once. The absorber grows between the two merges, which is the sensible
    // reading of "it ate both", and the deferred removal is what keeps the indices valid
    // while that happens.
    let mut scene = Scene::demo();
    scene.add(Prop::absorbing(ball(0.0, 1.0), DVec3::ZERO, 10.0));
    scene.add(Prop::new(ball(1.5, 1.0), DVec3::ZERO, 5.0));
    scene.add(Prop::new(ball(-1.5, 1.0), DVec3::ZERO, 5.0));

    assert_eq!(absorb_contacts(&mut scene), 2);
    assert_eq!(scene.props.len(), 2);
    assert_eq!(scene.props[1].mass_kg, 20.0);
}

#[test]
fn a_prop_between_two_absorbers_is_taken_once() {
    // The case that makes "skip anything already eaten" necessary: without it the victim
    // would be merged into both holes and its mass would be created twice.
    let mut scene = Scene::demo();
    scene.add(Prop::absorbing(ball(-1.5, 1.0), DVec3::ZERO, 10.0));
    scene.add(Prop::new(ball(0.0, 1.0), DVec3::ZERO, 5.0));
    scene.add(Prop::absorbing(ball(1.5, 1.0), DVec3::ZERO, 10.0));

    let absorbed = absorb_contacts(&mut scene);
    let total: f32 = scene
        .props
        .iter()
        .filter(|prop| !prop.immovable)
        .map(|prop| prop.mass_kg)
        .sum();

    assert!(absorbed >= 1);
    assert_eq!(total, 25.0, "no mass may be created or lost");
}

#[test]
fn an_immovable_absorber_takes_the_mass_and_keeps_its_invariant() {
    // `Prop` requires an immovable prop to have exactly zero velocity. The mass is taken, the
    // momentum is discarded, and the invariant survives — which is what "immovable" already
    // means everywhere else in the crate.
    let mut scene = Scene::demo();
    scene.props[0].absorbs = true; // the floor, as a wall that eats
    scene.add(Prop::new(
        Body::Sphere {
            pose: Pose {
                position: DVec3::new(0.0, 0.5, 0.0),
                rotation: Quat::IDENTITY,
            },
            radius: 1.0,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        },
        DVec3::new(0.0, -3.0, 0.0),
        7.0,
    ));

    assert_eq!(absorb_contacts(&mut scene), 1);
    assert_eq!(scene.props[0].mass_kg, 7.0);
    assert_eq!(scene.props[0].velocity_units_s, DVec3::ZERO);
    assert!(scene.props[0].is_valid());
}

// -----------------------------------------------------------------------
// Bookkeeping around removal.
// -----------------------------------------------------------------------

#[test]
fn the_surviving_props_keep_their_ids() {
    // The mirror in the render layer matches entities to props by `BodyId`, so a removal that
    // shifted anyone else's id would silently redraw the wrong body.
    let mut scene = Scene::demo();
    let hole = scene.add(Prop::absorbing(ball(0.0, 1.0), DVec3::ZERO, 10.0));
    let victim = scene.add(Prop::new(ball(1.5, 1.0), DVec3::ZERO, 5.0));
    let bystander = scene.add(Prop::new(ball(80.0, 1.0), DVec3::ZERO, 1.0));

    absorb_contacts(&mut scene);

    assert!(scene.get(hole).is_some());
    assert!(scene.get(victim).is_none());
    assert!(scene.get(bystander).is_some());
    assert_eq!(scene.get(bystander).unwrap().mass_kg, 1.0);
}

#[test]
fn an_absorbed_id_never_comes_back() {
    // `Scene::remove` documents this and A14 amends Requirement 5.4 for it: an id, once gone,
    // is gone. Anything holding one — a trail, an entity — can therefore treat `None` as
    // final rather than as "not yet".
    let mut scene = touching(10.0, 5.0, DVec3::ZERO);
    let victim = scene.iter().nth(2).map(|(id, _)| id).unwrap();

    absorb_contacts(&mut scene);
    assert!(scene.get(victim).is_none());

    scene.add(Prop::new(ball(40.0, 1.0), DVec3::ZERO, 1.0));
    assert!(
        scene.get(victim).is_none(),
        "a new prop must not reuse the id"
    );
}

#[test]
fn every_surviving_prop_is_still_valid() {
    let mut scene = touching(10.0, 5.0, DVec3::new(3.0, -2.0, 1.0));

    absorb_contacts(&mut scene);

    for (index, prop) in scene.props.iter().enumerate() {
        assert!(prop.is_valid(), "prop {index} went invalid: {prop:?}");
    }
}

// -----------------------------------------------------------------------
// Through the real step, which is how it will actually run.
// -----------------------------------------------------------------------

#[test]
fn a_black_hole_absorbs_through_scene_step_rather_than_bouncing() {
    // The end-to-end statement, and the reason absorption runs *before* the elastic pass: a
    // body that reaches an event horizon must not come back. Run through `Scene::step`, so
    // the ordering inside it is what is under test.
    use crate::time::simulation::TimeStep;

    let mut scene = Scene::demo();
    scene.add(Prop::absorbing(ball(0.0, 1.0), DVec3::ZERO, 1.0e30));
    scene.add(Prop::new(ball(1.5, 1.0), DVec3::ZERO, 1.0e24));

    scene.step(TimeStep::from_seconds(1.0).unwrap());

    assert_eq!(scene.props.len(), 2, "the star was swallowed, not bounced");
    assert!(scene.props[1].mass_kg > 1.0e30);
}

#[test]
fn absorption_leaves_a_scene_that_keeps_stepping() {
    // The failure this guards is a merge that leaves a `NaN` mass or velocity: the scene would
    // stay the right *shape* and every subsequent step would be nonsense.
    use crate::time::simulation::TimeStep;

    let mut scene = touching(2.0e30, 1.0e24, DVec3::new(0.01, 0.0, 0.0));
    let step = TimeStep::from_seconds(1.0).unwrap();

    for frame in 0..20 {
        scene.step(step);

        for (index, prop) in scene.props.iter().enumerate() {
            assert!(
                prop.is_valid() && prop.body.center().is_finite(),
                "prop {index} went bad on frame {frame}: {prop:?}"
            );
        }
    }
}

#[test]
fn an_immovable_prop_is_never_swallowed() {
    // A black hole hovering over the ground plane must not delete it. The floor is the
    // container the simulation happens inside, not a participant — the same reasoning that
    // makes two fixed props skip the elastic pass. Found by a test, not by inspection.
    let mut scene = Scene::demo();
    let floor = scene.iter().next().map(|(id, _)| id).unwrap();

    // Radius 1 at y = 0.5, so the sphere overlaps the floor slab at y ∈ [−1, 0].
    scene.add(Prop::absorbing(
        Body::Sphere {
            pose: Pose {
                position: DVec3::new(0.0, 0.5, 0.0),
                rotation: Quat::IDENTITY,
            },
            radius: 1.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        },
        DVec3::ZERO,
        1.0e31,
    ));

    assert_eq!(absorb_contacts(&mut scene), 0);
    assert!(scene.get(floor).is_some(), "the floor must survive");
    assert_eq!(scene.props.len(), 2);
}

// -----------------------------------------------------------------------
// The scenario a user reported: a black hole eats the Sun, then a neutron
// star is placed and pulls on it.
//
// The question was whether momentum is conserved for an *absorber* the way
// it is for a planet. These measure it end to end at the real masses,
// through the real integrator, rather than reasoning about the code.
// -----------------------------------------------------------------------

/// Total momentum of every prop, in kg·units/s, summed in `f64`.
///
/// `f64` because the whole point is to detect a small asymmetry against a large total: at
/// `2e31 kg` and `3e-5 units/s` the components are `~6e26`, and an `f32` sum would quantise at
/// `~4e19` — coarser than the errors worth finding.
fn total_momentum_units(scene: &Scene) -> [f64; 3] {
    let mut total = [0.0_f64; 3];

    for prop in &scene.props {
        // Immovable props are excluded deliberately: they are the container, not participants.
        // One with mass would be an external force and would break conservation legitimately —
        // `an_immovable_absorber_is_an_external_force` below pins that.
        if prop.immovable {
            continue;
        }

        let mass = f64::from(prop.mass_kg);
        total[0] += mass * prop.velocity_units_s.x;
        total[1] += mass * prop.velocity_units_s.y;
        total[2] += mass * prop.velocity_units_s.z;
    }

    total
}

/// The reported scene: a `10 M☉` black hole, the Sun, and a neutron star, all movable and free.
///
/// No floor — an immovable prop is not part of a momentum budget, and leaving it out makes the
/// assertion about the physics rather than about the bookkeeping.
fn hole_sun_and_neutron_star() -> Scene {
    use crate::experiments::constants::{SOLAR_SYSTEM_SUN_MASS_KG, STELLAR_OBJECTS};

    let mut scene = Scene::empty(crate::core::scene::Aabb {
        min: Vec3::splat(-500.0),
        max: Vec3::splat(500.0),
    });
    scene.softening_units = crate::experiments::constants::SOLAR_SYSTEM_SOFTENING_UNITS;

    let hole = STELLAR_OBJECTS
        .iter()
        .find(|object| object.absorbs && object.mass_kg < 1.0e35)
        .expect("a stellar-mass black hole");
    let neutron = STELLAR_OBJECTS
        .iter()
        .find(|object| object.name.contains("Neutron"))
        .expect("a neutron star");

    // The black hole falling in, the Sun drifting across it, the neutron star at rest — the
    // three states the reported run actually had.
    let mut hole_prop = hole.to_prop(DVec3::new(-3.0, 0.0, 0.0));
    hole_prop.velocity_units_s = DVec3::new(2.0e-5, 0.0, 0.0);
    scene.add(hole_prop);

    scene.add(Prop::new(
        Body::Sphere {
            pose: Pose {
                position: DVec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            radius: crate::experiments::solar_system::radius_units(
                crate::experiments::constants::SOLAR_SYSTEM_SUN_RADIUS_M,
            ),
            color: crate::experiments::constants::SOLAR_SYSTEM_SUN_COLOR,
        },
        DVec3::new(0.0, 0.0, 1.0e-5),
        SOLAR_SYSTEM_SUN_MASS_KG,
    ));

    scene.add(neutron.to_prop(DVec3::new(0.0, 0.0, 40.0)));

    scene
}

#[test]
fn a_black_hole_eating_the_sun_conserves_momentum_at_real_masses() {
    // The doctest already checks this at 10 kg and 5 kg. The reported case is 1.989e30 against
    // 1.989e31 — a 10:1 ratio at a magnitude where an f32 momentum sum would lose the smaller
    // term's low digits entirely. `merge` blends in f64 for exactly this reason, so this test is
    // what makes that choice load-bearing rather than decorative.
    let mut scene = hole_sun_and_neutron_star();
    let before = total_momentum_units(&scene);

    // Put the Sun in contact with the hole's capture radius.
    scene.props[0].body.translate(DVec3::new(3.0, 0.0, 0.0));

    assert_eq!(
        absorb_contacts(&mut scene),
        1,
        "the hole should eat the Sun"
    );

    let after = total_momentum_units(&scene);

    for axis in 0..3 {
        let scale = before[axis].abs().max(1.0);
        assert!(
            (after[axis] - before[axis]).abs() / scale < 1.0e-6,
            "axis {axis}: momentum went from {} to {}",
            before[axis],
            after[axis]
        );
    }
}

#[test]
fn mass_is_conserved_exactly_by_a_merge_at_real_masses() {
    // Exactly, not approximately: the sum of two f32 masses is one f32 addition, and a merge
    // that lost mass would change every subsequent force in the scene.
    let mut scene = hole_sun_and_neutron_star();
    let before: f32 = scene.props.iter().map(|prop| prop.mass_kg).sum();

    scene.props[0].body.translate(DVec3::new(3.0, 0.0, 0.0));
    assert_eq!(absorb_contacts(&mut scene), 1);

    let after: f32 = scene.props.iter().map(|prop| prop.mass_kg).sum();
    assert_eq!(before, after, "a merge must not create or destroy mass");
}

#[test]
fn gravitation_conserves_momentum_with_an_absorber_in_the_scene() {
    // The user's actual question: once the hole is a 2e31 kg absorber and a neutron star starts
    // pulling on it, is the *force* symmetric? It has to be — the pair term is
    // `G·m₁·m₂/r_eff²` with one shared softened separation, so Newton's third law holds by
    // construction and `absorbs` never enters the force loop at all. This measures that rather
    // than asserting it: 400 substeps of the real integrator across a 10,000:1 mass ratio.
    let mut scene = hole_sun_and_neutron_star();
    let before = total_momentum_units(&scene);

    let step = crate::time::simulation::TimeStep::from_seconds(10.0).expect("a valid step");
    for _ in 0..400 {
        crate::core::integrator::integrate_gravity(&mut scene.props, step, scene.softening_units);
    }

    let after = total_momentum_units(&scene);

    for axis in 0..3 {
        let scale = before[axis].abs().max(1.0);
        assert!(
            (after[axis] - before[axis]).abs() / scale < 1.0e-4,
            "axis {axis}: momentum drifted from {} to {} over 400 steps",
            before[axis],
            after[axis]
        );
    }
}

#[test]
fn the_absorbs_flag_changes_no_force() {
    // The cleanest statement of the answer: absorption is a *contact response*, and gravitation
    // does not know it exists. Two identical scenes differing only in `absorbs` must integrate
    // to bit-identical states, so no black hole can pull or be pulled differently from a planet
    // of the same mass.
    let mut absorbing = hole_sun_and_neutron_star();
    let mut inert = hole_sun_and_neutron_star();

    for prop in &mut inert.props {
        prop.absorbs = false;
    }

    let step = crate::time::simulation::TimeStep::from_seconds(10.0).expect("a valid step");
    for _ in 0..200 {
        crate::core::integrator::integrate_gravity(
            &mut absorbing.props,
            step,
            absorbing.softening_units,
        );
        crate::core::integrator::integrate_gravity(&mut inert.props, step, inert.softening_units);
    }

    for (index, (a, b)) in absorbing.props.iter().zip(&inert.props).enumerate() {
        assert_eq!(
            a.body.center(),
            b.body.center(),
            "prop {index} moved differently with `absorbs` set"
        );
        assert_eq!(
            a.velocity_units_s, b.velocity_units_s,
            "prop {index} accelerated differently with `absorbs` set"
        );
    }
}

#[test]
fn momentum_is_conserved_even_when_the_step_is_far_too_large() {
    // Why "is momentum conserved?" cannot answer "is this trajectory right?".
    //
    // Symplectic Euler updates every velocity by a pairwise force that is equal and opposite by
    // construction, so the momentum error is **exactly zero at any step size** — the two updates
    // are `+m₂a·dt` and `-m₁a·dt` with the same `a·dt`. A step a thousand times too long conserves
    // momentum just as perfectly as a correct one while putting the body somewhere else entirely.
    //
    // So this test passes, and that is the point: it is the same assertion as
    // `gravitation_conserves_momentum_with_an_absorber_in_the_scene` with a deliberately absurd
    // step, and it still holds. Momentum conservation is not a measure of accuracy here, and
    // anyone reaching for it to diagnose a strange-looking trajectory should read this first.
    // `MAX_SUBSTEPS`' own docs record where the real limit is.
    let mut scene = hole_sun_and_neutron_star();
    let before = total_momentum_units(&scene);

    // 16,667 s in one go — the whole solar-system frame, unsplit, against a pair that asks for
    // over a thousand substeps once it is close.
    let absurd = crate::time::simulation::TimeStep::from_seconds(16_667.0).expect("a valid step");
    for _ in 0..50 {
        crate::core::integrator::integrate_gravity(&mut scene.props, absurd, scene.softening_units);
    }

    for axis in 0..3 {
        let scale = before[axis].abs().max(1.0);
        let after = total_momentum_units(&scene);
        assert!(
            (after[axis] - before[axis]).abs() / scale < 1.0e-4,
            "axis {axis}: even a hopeless step must conserve momentum, got {} from {}",
            after[axis],
            before[axis]
        );
    }
}
