//! Unit and property tests for the [`crate::collision::collision`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/collision/collision.rs` via `#[path]`,
//! so the private helpers are in reach.
//!
//! The pass has two jobs and each is tested in both directions: it must merge exactly the pairs
//! the module docs list (and leave alone the rest: elastic pairs, rigid bodies, receding pairs,
//! hit-and-run), and every merge must conserve mass, momentum and the centre of mass, with debris
//! that starts clear of everything and carries no more energy than the impact brought.

use super::*;
use crate::core::scene::{Aabb, CollisionSettings, Material, Vec3};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/collision/proptest-regressions/collision_tests.txt";

const EARTH_KG: f32 = 5.9722e24;
const EARTH_M: f64 = 6.371e6;
const MARS_KG: f32 = 6.4171e23;
const MARS_M: f64 = 3.3895e6;
const ROCK: Material = Material::new(0.0, 0.0);
/// Just inside contact: bodies placed exactly touching are a coin toss of `f32` rounding.
const TOUCH: f64 = 0.999;

fn grey() -> Color {
    Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    }
}

/// A sphere of `radius_m` at `x_m` along X moving at `vx_m_s`, in SI, converted to scene units.
fn body(x_m: f64, vx_m_s: f64, mass_kg: f32, radius_m: f64, material: Material) -> Prop {
    Prop::new(
        Body::Sphere {
            pose: Pose {
                position: DVec3::new(x_m / METRES_PER_UNIT, 0.0, 0.0),
                rotation: Quat::IDENTITY,
            },
            radius: (radius_m / METRES_PER_UNIT) as f32,
            color: grey(),
        },
        DVec3::new(vx_m_s / METRES_PER_UNIT, 0.0, 0.0),
        mass_kg,
    )
    .with_material(material)
}

fn scene_of(props: Vec<Prop>, fragments: u32) -> Scene {
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-1.0),
        max: Vec3::splat(1.0),
    });
    scene.collision = CollisionSettings {
        debris_fragments: fragments,
        ..CollisionSettings::default()
    };
    for prop in props {
        scene.add(prop);
    }
    scene
}

/// The Earth at rest at the origin and a Mars-mass body touching it from +X, closing at `speed`.
fn giant_impact(speed_m_s: f64, fragments: u32) -> Scene {
    scene_of(
        vec![
            body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK),
            body(
                TOUCH * (EARTH_M + MARS_M),
                -speed_m_s,
                MARS_KG,
                MARS_M,
                ROCK,
            ),
        ],
        fragments,
    )
}

/// Total momentum, kg·units/s.
fn momentum(scene: &Scene) -> DVec3 {
    scene
        .props
        .iter()
        .map(|p| p.velocity_units_s * f64::from(p.mass_kg))
        .sum()
}

/// Mass-weighted position, kg·units.
fn first_moment(scene: &Scene) -> DVec3 {
    scene
        .props
        .iter()
        .map(|p| p.body.center() * f64::from(p.mass_kg))
        .sum()
}

fn total_mass(scene: &Scene) -> f64 {
    scene.props.iter().map(|p| f64::from(p.mass_kg)).sum()
}

fn radius_m(prop: &Prop) -> f64 {
    physical_radius_m(prop).unwrap()
}

/// The speed the Earth and a Mars-mass body meet at when released from rest ten contact
/// distances apart, as `experiment_1` does.
/// Twice the mutual escape speed of the Earth and a Mars-mass body: fast enough for debris.
fn twice_escape_speed() -> f64 {
    2.0 * mutual_escape_speed(f64::from(EARTH_KG), f64::from(MARS_KG), EARTH_M, MARS_M)
}

fn from_rest_speed() -> f64 {
    mutual_escape_speed(f64::from(EARTH_KG), f64::from(MARS_KG), EARTH_M, MARS_M) * 0.9_f64.sqrt()
}

#[test]
fn planets_and_moons_are_gravity_dominated_and_a_spacecraft_is_not() {
    assert!(gravity_dominated(&body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK)));
    assert!(gravity_dominated(&body(0.0, 0.0, 7.342e22, 1.7374e6, ROCK)));
    // The Saturn V stack, generously sized.
    assert!(!gravity_dominated(&body(0.0, 0.0, 2.9e6, 50.0, ROCK)));
    // Absorbers have their own pass.
    let mut hole = body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK);
    hole.absorbs = true;
    assert!(!gravity_dominated(&hole));
}

#[test]
fn a_gentle_meeting_is_a_perfect_merge_that_conserves_everything() {
    let mut scene = giant_impact(100.0, 8);
    let (mass, p, moment) = (total_mass(&scene), momentum(&scene), first_moment(&scene));

    let events = merge_contacts(&mut scene);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].debris_fraction, 0.0);
    assert!(events[0].fragments.is_empty());
    assert_eq!(scene.props.len(), 1);
    assert!((total_mass(&scene) / mass - 1.0).abs() < 1.0e-6);
    assert!((momentum(&scene) - p).length() <= 1.0e-6 * p.length());
    // Masses are `f32`, so the moment is good to about `1e-7`.
    assert!((first_moment(&scene) - moment).length() <= 1.0e-6 * moment.length().max(1.0));
    let expected_radius = (EARTH_M.powi(3) + MARS_M.powi(3)).cbrt();
    assert!((radius_m(&scene.props[0]) / expected_radius - 1.0).abs() < 1.0e-5);
}

#[test]
fn the_more_massive_body_survives_with_its_id() {
    let mut scene = giant_impact(100.0, 0);
    let earth = scene.iter().next().unwrap().0;
    let events = merge_contacts(&mut scene);
    assert_eq!(events[0].survivor, earth);
    assert!(scene.get(earth).is_some());
}

#[test]
fn an_elastic_pair_bounces_rather_than_merging() {
    let mut scene = scene_of(
        vec![
            body(0.0, 0.0, EARTH_KG, EARTH_M, Material::ELASTIC),
            body(
                TOUCH * (EARTH_M + MARS_M),
                -100.0,
                MARS_KG,
                MARS_M,
                Material::ELASTIC,
            ),
        ],
        8,
    );
    assert!(merge_contacts(&mut scene).is_empty());
    assert_eq!(scene.props.len(), 2);
}

#[test]
fn a_rigid_craft_touching_a_planet_is_left_to_the_bounce() {
    let mut scene = scene_of(
        vec![
            body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK),
            body(EARTH_M + 4.0, -2.0, 4_184.0, 5.0, ROCK),
        ],
        8,
    );
    assert!(merge_contacts(&mut scene).is_empty());
    assert_eq!(scene.props.len(), 2);
}

#[test]
fn a_pair_moving_apart_is_not_merged() {
    let mut scene = giant_impact(-100.0, 8);
    assert!(merge_contacts(&mut scene).is_empty());
}

#[test]
fn a_pair_that_is_not_touching_is_not_merged() {
    let mut scene = scene_of(
        vec![
            body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK),
            body(1.01 * (EARTH_M + MARS_M), -100.0, MARS_KG, MARS_M, ROCK),
        ],
        8,
    );
    assert!(merge_contacts(&mut scene).is_empty());
}

#[test]
fn a_fast_grazing_hit_leaves_faster_than_escape_and_is_a_hit_and_run() {
    // Touching along X, sliding past along Z at three times the escape speed: with no friction
    // the sliding survives the impact, so the pair is not bound.
    let escape = mutual_escape_speed(f64::from(EARTH_KG), f64::from(MARS_KG), EARTH_M, MARS_M);
    let mut mars = body(TOUCH * (EARTH_M + MARS_M), -100.0, MARS_KG, MARS_M, ROCK);
    mars.velocity_units_s.z = 3.0 * escape / METRES_PER_UNIT;
    let mut scene = scene_of(vec![body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK), mars], 8);
    assert!(merge_contacts(&mut scene).is_empty());
}

#[test]
fn an_impact_below_the_escape_speed_is_a_perfect_merge() {
    // The Earth struck from rest by a Mars-mass body, at 95% of their escape speed: Leinhardt &
    // Stewart's perfect-merging regime, and what the particle simulation of it finds too.
    let mut scene = giant_impact(from_rest_speed(), 8);
    let events = merge_contacts(&mut scene);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].debris_fraction, 0.0);
    assert_eq!(scene.props.len(), 1);
}

#[test]
fn an_impact_at_twice_the_escape_speed_throws_off_what_the_law_says() {
    let mut scene = giant_impact(twice_escape_speed(), 8);
    let events = merge_contacts(&mut scene);
    assert_eq!(events.len(), 1);
    let debris = events[0].debris_fraction;
    assert!((0.020..0.030).contains(&debris), "debris {debris}");
    assert_eq!(events[0].fragments.len(), 8);
    assert_eq!(scene.props.len(), 9);
}

#[test]
fn zero_fragments_makes_every_merge_perfect() {
    let mut scene = giant_impact(twice_escape_speed(), 0);
    let events = merge_contacts(&mut scene);
    assert_eq!(events[0].debris_fraction, 0.0);
    assert_eq!(scene.props.len(), 1);
}

#[test]
fn a_fragment_falling_back_merges_without_making_more_debris() {
    let mut scene = giant_impact(twice_escape_speed(), 8);
    merge_contacts(&mut scene);
    let remnant = scene.props[0].clone();
    let fragment = scene.props[1].clone();

    // Put the fragment back on the remnant's surface, falling in at the escape speed.
    let r = radius_m(&remnant) + radius_m(&fragment);
    let escape = mutual_escape_speed(
        f64::from(remnant.mass_kg),
        f64::from(fragment.mass_kg),
        radius_m(&remnant),
        radius_m(&fragment),
    );
    let mut falling = body(
        TOUCH * r,
        -0.99 * escape,
        fragment.mass_kg,
        radius_m(&fragment),
        ROCK,
    );
    falling.body.translate(remnant.body.center());
    falling.velocity_units_s += remnant.velocity_units_s;
    let mut again = scene_of(vec![remnant, falling], 8);

    let events = merge_contacts(&mut again);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].debris_fraction, 0.0);
    assert_eq!(again.props.len(), 1);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Whatever the detail and however hard the impact (above the escape speed, where there is
    /// debris; the pair is still bound, as `e = 0` parts it at rest), a merge conserves mass,
    /// momentum and the centre of mass; the debris
    /// starts clear of the remnant and of itself; and the kinetic energy left in the
    /// centre-of-mass frame is no more than the impact brought.
    #[test]
    fn a_merge_with_debris_conserves_and_starts_clear(
        fragments in 1_u32..48,
        speed_fraction in 1.01_f64..3.0,
        mass_ratio in 0.05_f32..1.0,
    ) {
        let small_kg = EARTH_KG * mass_ratio;
        let small_m = EARTH_M * f64::from(mass_ratio).cbrt();
        let escape = mutual_escape_speed(f64::from(EARTH_KG), f64::from(small_kg), EARTH_M, small_m);
        let speed = speed_fraction * escape;
        let mut scene = scene_of(
            vec![
                body(0.0, 0.0, EARTH_KG, EARTH_M, ROCK),
                body(TOUCH * (EARTH_M + small_m), -speed, small_kg, small_m, ROCK),
            ],
            fragments,
        );
        let (mass, p, moment) = (total_mass(&scene), momentum(&scene), first_moment(&scene));
        let reduced = f64::from(EARTH_KG) * f64::from(small_kg) / mass;
        let available = 0.5 * reduced * speed * speed;
        let centre_velocity = p / mass;

        let events = merge_contacts(&mut scene);
        prop_assert_eq!(events.len(), 1);

        prop_assert!((total_mass(&scene) / mass - 1.0).abs() < 1.0e-5);
        prop_assert!((momentum(&scene) - p).length() <= 1.0e-5 * p.length());
        prop_assert!((first_moment(&scene) - moment).length() <= 1.0e-6 * moment.length().max(1.0)
            + 1.0e-6 * mass * EARTH_M / METRES_PER_UNIT);

        let after: f64 = scene.props.iter().map(|prop| {
            let v = (prop.velocity_units_s - centre_velocity) * METRES_PER_UNIT;
            0.5 * f64::from(prop.mass_kg) * v.length_squared()
        }).sum();
        prop_assert!(after <= available * (1.0 + 1.0e-6), "{after:e} > {available:e}");

        for (index, a) in scene.props.iter().enumerate() {
            prop_assert!(a.is_valid());
            for b in &scene.props[index + 1..] {
                let gap = (a.body.center() - b.body.center()).length() * METRES_PER_UNIT;
                prop_assert!(gap > radius_m(a) + radius_m(b), "bodies start overlapping");
            }
        }
    }
}

// --- Simulated contact ----------------------------------------------------------------------

/// A particle of `radius_m` at `x_m`, moving at `vx_m_s`, made of rock with a modulus of
/// `modulus_pa` and the given restitution.
fn particle(x_m: f64, vx_m_s: f64, mass_kg: f32, radius_m: f64, restitution: f32) -> Prop {
    body(
        x_m,
        vx_m_s,
        mass_kg,
        radius_m,
        Material::new(restitution, 0.0).with_modulus(1.0e9),
    )
}

/// Collide two 1 m, 1 t particles head-on at 2 m/s relative through `Scene::step`, and return
/// the speed they part at. Gravity between them is negligible, so this is the contact alone.
fn simulated_rebound(restitution: f32) -> f64 {
    let mut scene = scene_of(
        vec![
            particle(0.0, 1.0, 1000.0, 1.0, restitution),
            particle(2.0, -1.0, 1000.0, 1.0, restitution),
        ],
        8,
    );
    let step = TimeStep::from_seconds(1.0e-3).unwrap();
    for _ in 0..200 {
        scene.step(step);
    }
    (scene.props[1].velocity_units_s.x - scene.props[0].velocity_units_s.x) * METRES_PER_UNIT
}

#[test]
fn the_contact_force_pushes_apart_equally_and_oppositely() {
    let mut props = vec![
        particle(0.0, 1.0, 1000.0, 1.0, 0.0),
        particle(1.9, -1.0, 3000.0, 1.0, 0.0),
    ];
    let before: DVec3 = props
        .iter()
        .map(|p| p.velocity_units_s * f64::from(p.mass_kg))
        .sum();
    assert_eq!(
        apply_contact_forces(&mut props, TimeStep::from_seconds(1.0e-4).unwrap()),
        1
    );
    let after: DVec3 = props
        .iter()
        .map(|p| p.velocity_units_s * f64::from(p.mass_kg))
        .sum();
    assert!((after - before).length() <= 1.0e-12 * before.length().max(1.0e-9));
    assert!(
        props[0].velocity_units_s.x < 1.0 / METRES_PER_UNIT,
        "pushed back"
    );
    assert!(
        props[1].velocity_units_s.x > -1.0 / METRES_PER_UNIT,
        "pushed back"
    );
}

#[test]
fn no_overlap_no_force_and_rigid_bodies_are_left_alone() {
    let mut apart = vec![
        particle(0.0, 1.0, 1000.0, 1.0, 0.0),
        particle(2.1, -1.0, 1000.0, 1.0, 0.0),
    ];
    let untouched = apart.clone();
    assert_eq!(
        apply_contact_forces(&mut apart, TimeStep::from_seconds(1.0).unwrap()),
        0
    );
    assert_eq!(apart, untouched);

    let mut rigid = vec![
        body(0.0, 1.0, 1000.0, 1.0, ROCK),
        body(1.9, -1.0, 1000.0, 1.0, ROCK),
    ];
    let untouched = rigid.clone();
    assert_eq!(
        apply_contact_forces(&mut rigid, TimeStep::from_seconds(1.0).unwrap()),
        0
    );
    assert_eq!(rigid, untouched);
}

#[test]
fn a_simulated_pair_is_neither_merged_in_an_instant_nor_bounced() {
    // Two planets of soft rock, touching and bound: the contact force is theirs alone.
    let soft = ROCK.with_modulus(1.0e13);
    let mut scene = scene_of(
        vec![
            body(0.0, 0.0, EARTH_KG, EARTH_M, soft),
            body(TOUCH * (EARTH_M + MARS_M), -100.0, MARS_KG, MARS_M, soft),
        ],
        8,
    );
    assert!(merge_contacts(&mut scene).is_empty());
    assert_eq!(
        crate::collision::resolve::resolve_contacts(&mut scene.props),
        0
    );
}

#[test]
fn a_simulated_collision_rebounds_at_the_material_s_restitution() {
    // Through `Scene::step`, so the substep count has to resolve the contact for this to hold.
    let half = simulated_rebound(0.5);
    assert!(
        (half / 2.0 - 0.5).abs() < 0.03,
        "parted at {:.3} of 2 m/s",
        half / 2.0
    );
    // Restitution 0 asks for infinite damping; the cap leaves a 7.6% rebound.
    let plastic = simulated_rebound(0.0);
    let capped = crate::physics_utils::soft_contact::restitution_for_damping_ratio(
        SOFT_CONTACT_MAX_DAMPING_RATIO,
    );
    assert!(
        (plastic / 2.0 - capped).abs() < 0.01,
        "parted at {:.4} of 2 m/s, the cap allows {capped:.4}",
        plastic / 2.0
    );
}

#[test]
fn the_shortest_contact_is_found_and_nothing_rigid_counts() {
    let props = vec![
        particle(0.0, 0.0, 1000.0, 1.0, 0.0),
        body(10.0, 0.0, 1000.0, 1.0, ROCK),
    ];
    // One soft particle: a self-pair of 1 m, 1 t at 1 GPa. Two springs in series halve the
    // modulus, so k = 5e8 Pa · 0.5 m = 2.5e8 N/m; m* = 500 kg; the contact lasts π·√2 ms.
    let shortest = shortest_contact_seconds(&props).unwrap();
    let expected = std::f64::consts::PI * std::f64::consts::SQRT_2 * 1.0e-3;
    assert!((shortest / expected - 1.0).abs() < 1.0e-6, "{shortest}");
    assert_eq!(shortest_contact_seconds(&props[1..]), None);
}

#[test]
fn simulated_friction_slows_a_slide_and_never_speeds_it_up() {
    // Overlapping along X, sliding past each other along Y: friction must bring the sliding
    // speeds together, never drive them apart (it once did, with the sign the wrong way round).
    let grippy = Material::new(0.0, 0.8).with_modulus(1.0e9);
    let mut a = body(0.0, 0.0, 1000.0, 1.0, grippy);
    let mut b = body(1.9, 0.0, 1000.0, 1.0, grippy);
    a.velocity_units_s.y = 1.0 / METRES_PER_UNIT;
    b.velocity_units_s.y = -1.0 / METRES_PER_UNIT;
    let mut props = vec![a, b];
    let sliding = |props: &[Prop]| {
        (props[1].velocity_units_s.y - props[0].velocity_units_s.y) * METRES_PER_UNIT
    };
    let before = sliding(&props);
    apply_contact_forces(&mut props, TimeStep::from_seconds(1.0e-4).unwrap());
    let after = sliding(&props);
    assert!(after.abs() < before.abs(), "slide {before} → {after}");
    assert!(after * before >= 0.0, "and never reversed");
}

/// Hot rock: basalt's Tillotson parameters.
fn basalt() -> crate::core::scene::Tillotson {
    crate::core::scene::Tillotson {
        reference_density_kg_m3: 2700.0,
        bulk_a_pa: 2.67e10,
        bulk_b_pa: 2.67e10,
        a: 0.5,
        b: 1.5,
        reference_energy_j_kg: 4.87e8,
        incipient_vaporisation_j_kg: 4.72e6,
        complete_vaporisation_j_kg: 1.82e7,
        alpha: 5.0,
        beta: 5.0,
    }
}

/// Total angular momentum about the origin, orbital and spin, in SI.
fn angular_momentum(props: &[Prop]) -> DVec3 {
    props
        .iter()
        .map(|p| {
            let r = p.body.center() * METRES_PER_UNIT;
            let v = p.velocity_units_s * METRES_PER_UNIT;
            let radius = physical_radius_m(p).unwrap();
            let inertia = sphere_moment_of_inertia(f64::from(p.mass_kg), radius);
            r.cross(v) * f64::from(p.mass_kg) + p.spin_rad_s * inertia
        })
        .sum()
}

fn steps(scene: &mut Scene, seconds: f64, count: usize) {
    for _ in 0..count {
        scene.step(TimeStep::from_seconds(seconds).unwrap());
    }
}

#[test]
fn friction_spins_a_glancing_contact_and_conserves_angular_momentum() {
    // Two 1 m, 1 t particles passing with their centres 1.2 m apart across the line of motion.
    let grippy = Material::new(0.5, 0.8).with_modulus(1.0e9);
    let mut a = body(0.0, 1.0, 1000.0, 1.0, grippy);
    let mut b = body(3.0, -1.0, 1000.0, 1.0, grippy);
    a.body
        .translate(DVec3::new(0.0, -0.6 / METRES_PER_UNIT, 0.0));
    b.body
        .translate(DVec3::new(0.0, 0.6 / METRES_PER_UNIT, 0.0));
    let mut scene = scene_of(vec![a, b], 0);
    scene.softening_units = 0.0;
    let before = angular_momentum(&scene.props);
    steps(&mut scene, 1.0e-3, 3000);
    let after = angular_momentum(&scene.props);
    assert!(
        scene.props[0].spin_rad_s.length() > 0.01,
        "friction spun it"
    );
    assert!(
        (after - before).length() < 1.0e-9 * before.length(),
        "{before} → {after}"
    );
}

#[test]
fn a_hot_contact_keeps_its_energy_as_heat() {
    // The same particles meeting head-on at 2 m/s, half-elastic, made of hot-capable rock: what
    // the contact takes from the motion is in the heat, so the total stays put, to the step's error.
    // That error is first order, as the integrator is: measured, `1.5%` of the approach energy at
    // `44` steps per contact and a tenth of that at `440`.
    let rock = Material::new(0.5, 0.3)
        .with_modulus(1.0e9)
        .with_thermal(basalt());
    let error = |step: f64| {
        let mut scene = scene_of(
            vec![
                body(0.0, 1.0, 1000.0, 1.0, rock),
                body(2.5, -1.0, 1000.0, 1.0, rock),
            ],
            0,
        );
        scene.softening_units = 0.0;
        let before = particle_energy(&scene.props, 0.0);
        steps(&mut scene, step, (2.0 / step) as usize);
        let after = particle_energy(&scene.props, 0.0);
        assert!(
            after.heat > 0.5 * before.kinetic,
            "most of it is heat now: {after:?}"
        );
        (after.total() - before.total()).abs() / before.kinetic
    };
    let coarse = error(1.0e-4);
    let fine = error(1.0e-5);
    assert!(coarse < 0.02, "{coarse}");
    assert!(fine < 0.2 * coarse, "first order: {coarse} then {fine}");
}

#[test]
fn hot_neighbours_push_apart_and_cool_doing_it() {
    // Two hot particles at rest, a hair apart (not touching), within each other's smoothing
    // length: the pressure pushes them apart, and the work comes out of the heat.
    let rock = Material::new(0.5, 0.0)
        .with_modulus(1.0e9)
        .with_thermal(basalt());
    let mut a = body(0.0, 0.0, 1000.0, 1.0, rock);
    let mut b = body(2.2, 0.0, 1000.0, 1.0, rock);
    a.heat_j_kg = 1.0e3;
    b.heat_j_kg = 1.0e3;
    let mut scene = scene_of(vec![a, b], 0);
    scene.softening_units = 0.0;
    let before = particle_energy(&scene.props, 0.0);
    steps(&mut scene, 1.0e-3, 2000);
    let after = particle_energy(&scene.props, 0.0);
    let parting =
        (scene.props[1].velocity_units_s.x - scene.props[0].velocity_units_s.x) * METRES_PER_UNIT;
    assert!(parting > 0.0, "pushed apart");
    assert!(after.heat < before.heat, "and cooler");
    assert!(
        (after.total() - before.total()).abs() < 1.0e-3 * before.heat,
        "{before:?} → {after:?}"
    );
    // Equal and opposite.
    let momentum: f64 = scene.props.iter().map(|p| p.velocity_units_s.x).sum();
    assert!(momentum.abs() < 1.0e-15);
}

#[test]
fn cold_rock_that_is_not_hot_capable_gains_no_heat() {
    let rock = Material::new(0.5, 0.3).with_modulus(1.0e9);
    let mut scene = scene_of(
        vec![
            body(0.0, 1.0, 1000.0, 1.0, rock),
            body(2.5, -1.0, 1000.0, 1.0, rock),
        ],
        0,
    );
    steps(&mut scene, 1.0e-4, 20_000);
    assert!(scene.props.iter().all(|p| p.heat_j_kg == 0.0));
}
