//! Unit and property tests for the [`crate::experiments::catalog`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/experiments/catalog.rs` via `#[path]`, not as a standalone integration test.
//!
//! Two kinds of check here, and the second is the interesting one:
//!
//! - **Catalog hygiene** — every experiment is reachable by its slug, builds a valid
//!   scene and a valid spec, and frames itself. These run over `ExperimentId::ALL`, so
//!   a new experiment is covered the moment it is added rather than when someone
//!   remembers to write a test for it.
//! - **The three-body setup is what it claims** — the bodies really are on a circle,
//!   their velocities really are tangential, and the speeds really are inside the
//!   documented window. Those are the properties the experiment's whole premise rests
//!   on, and none of them are visible from the code without doing the geometry.

use super::*;
use crate::collision::contact::contact_between_props;
use crate::experiments::constants::THREE_BODY_HEIGHT_UNITS;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/experiments/proptest-regressions/catalog_tests.txt";

// -----------------------------------------------------------------------
// Catalog hygiene: properties every experiment must have.
// -----------------------------------------------------------------------

#[test]
fn every_experiment_is_reachable_by_slug_number_and_alias() {
    for id in ExperimentId::ALL {
        assert!(
            id.matches(id.slug()),
            "{:?} does not match its own slug",
            id
        );

        for alias in id.aliases() {
            assert!(
                id.matches(alias),
                "{:?} does not match its alias {alias}",
                id
            );
        }

        // Case and surrounding whitespace must not matter: a command line is typed by
        // a person.
        assert!(id.matches(&id.slug().to_ascii_uppercase()));
        assert!(id.matches(&format!("  {}  ", id.slug())));
    }
}

#[test]
fn no_two_experiments_answer_to_the_same_token() {
    // A collision here would make one of them unreachable from the command line, and
    // which one would depend on `ALL`'s order.
    for id in ExperimentId::ALL {
        for other in ExperimentId::ALL {
            if id == other {
                continue;
            }
            assert!(
                !other.matches(id.slug()),
                "{:?} also answers to {:?}'s slug",
                other,
                id
            );
            for alias in id.aliases() {
                assert!(
                    !other.matches(alias),
                    "{:?} also answers to {:?}'s alias {alias}",
                    other,
                    id
                );
            }
        }
    }
}

#[test]
fn the_slugs_are_the_numbered_ones_the_catalog_documents() {
    assert_eq!(ExperimentId::TwoBody.slug(), "experiment_1");
    assert_eq!(ExperimentId::ThreeBodyRing.slug(), "experiment_2");
    assert_eq!(ExperimentId::SolarSystem.slug(), "experiment_3");
    assert_eq!(ExperimentId::default(), ExperimentId::TwoBody);
}

#[test]
fn every_experiment_builds_a_valid_scene_and_spec() {
    for id in ExperimentId::ALL {
        let experiment = id.build(12345);

        assert_eq!(experiment.spec.id, id);
        assert!(
            experiment.spec.is_valid(),
            "{:?} produced an invalid spec: {:?}",
            id,
            experiment.spec
        );
        assert!(
            experiment.scene.bounds.is_valid(),
            "{:?} produced invalid bounds",
            id
        );
        assert!(
            experiment.scene.props.len() >= 2,
            "{:?} needs a floor and at least one body, got {}",
            id,
            experiment.scene.props.len()
        );

        for prop in &experiment.scene.props {
            assert!(
                prop.is_valid(),
                "{:?} produced an invalid prop: {prop:?}",
                id
            );
        }
    }
}

#[test]
fn every_experiment_frames_its_own_scene() {
    // The camera must be far enough from its target to fit `framed_radius_units` into a 45°
    // vertical field of view, so an experiment that grows its opening view without moving
    // its camera fails here rather than opening a window with bodies off screen.
    //
    // It is checked against the *declared* framed radius rather than the scene's full
    // extent, because the two genuinely differ: the solar system spans 4495 units and would
    // need a camera 10,852 units out, from where nothing in it is more than a pixel across.
    // The declaration is kept honest by the two assertions after it — the radius has to
    // contain something, and it may not exceed what the camera can actually show.
    let half_fov_tangent = (45.0_f32.to_radians() / 2.0).tan();

    for id in ExperimentId::ALL {
        let experiment = id.build(1);
        let spec = experiment.spec;

        let distance = (spec.camera_position_units - spec.camera_target_units).length();
        let needed = spec.framed_radius_units / half_fov_tangent;

        assert!(
            distance >= needed,
            "{:?}: camera is {distance} units out but needs {needed} to frame a \
             {}-unit radius",
            id,
            spec.framed_radius_units
        );

        // A framed radius that contains nothing would satisfy the check above trivially.
        //
        // Planes are excluded rather than immovable props, and the distinction matters: the floor
        // is scenery because it is a *ground plane*, not because it is fixed. `experiment_5` is a
        // static size-comparison row in which every body is immovable on purpose, and filtering by
        // mobility left it with no subjects at all.
        let nearest = experiment
            .scene
            .props
            .iter()
            .filter(|prop| !matches!(prop.body, Body::Plane { .. }))
            .map(|prop| (prop.body.center().as_vec3() - spec.camera_target_units).length())
            .fold(f32::INFINITY, f32::min);

        assert!(
            nearest <= spec.framed_radius_units,
            "{:?}: the opening view contains no body — nearest is {nearest} units out, \
             framed radius is {}",
            id,
            spec.framed_radius_units
        );
    }
}

#[test]
fn no_experiment_starts_with_bodies_already_touching() {
    // An experiment that begins in contact starts by resolving a collision, which is
    // never what the initial conditions meant to say.
    for id in ExperimentId::ALL {
        let experiment = id.build(999);
        let props = &experiment.scene.props;

        for i in 0..props.len() {
            for j in (i + 1)..props.len() {
                assert!(
                    contact_between_props(&props[i], &props[j]).is_none(),
                    "{:?}: props {i} and {j} start in contact",
                    id
                );
            }
        }
    }
}

#[test]
fn the_catalog_listing_covers_every_experiment() {
    let lines = catalog_lines();
    assert_eq!(lines.len(), ExperimentId::ALL.len());

    for (line, id) in lines.iter().zip(ExperimentId::ALL) {
        assert!(line.contains(id.slug()), "{line:?} is missing the slug");
        assert!(line.contains(id.name()), "{line:?} is missing the name");
    }
}

// -----------------------------------------------------------------------
// experiment_1: the two-body free fall.
// -----------------------------------------------------------------------

#[test]
fn the_two_body_experiment_is_the_scene_core_builds() {
    let experiment = two_body();

    assert_eq!(experiment.scene, Scene::two_body_gravity());
    assert_eq!(experiment.spec.time_scale, TWO_BODY_TIME_SCALE);
    // Deterministic: the seed cannot change the scene, whatever it is.
    assert_eq!(
        ExperimentId::TwoBody.build(1).scene,
        ExperimentId::TwoBody.build(2).scene
    );
}

// -----------------------------------------------------------------------
// experiment_2: the three-body ring. The premise, checked.
// -----------------------------------------------------------------------

/// The three bodies of a ring experiment, excluding the floor.
fn ring_bodies(seed: u64) -> Vec<Prop> {
    three_body_ring(seed)
        .scene
        .props
        .iter()
        .filter(|prop| !prop.immovable)
        .cloned()
        .collect()
}

#[test]
fn the_ring_has_three_bodies_on_a_circle() {
    let bodies = ring_bodies(7);
    assert_eq!(bodies.len(), 3);

    let centre = Vec3::new(0.0, THREE_BODY_HEIGHT_UNITS, 0.0);

    for prop in &bodies {
        let offset = prop.body.center().as_vec3() - centre;

        // On the circle: the right radius, and in the ring's horizontal plane.
        assert!(
            (offset.length() - THREE_BODY_RING_RADIUS_UNITS).abs() < 1.0e-3,
            "body is {} units from the centre, expected {}",
            offset.length(),
            THREE_BODY_RING_RADIUS_UNITS
        );
        assert!(
            offset.y.abs() < 1.0e-4,
            "body is out of the ring's plane by {}",
            offset.y
        );
    }
}

#[test]
fn the_ring_bodies_are_evenly_spaced() {
    let bodies = ring_bodies(11);
    let centre = Vec3::new(0.0, THREE_BODY_HEIGHT_UNITS, 0.0);

    // 120° apart means every pair is separated by the same chord, `R√3`.
    let expected_chord = THREE_BODY_RING_RADIUS_UNITS * 3.0_f32.sqrt();

    for i in 0..3 {
        for j in (i + 1)..3 {
            let separation = (bodies[i].body.center() - bodies[j].body.center()).length() as f32;
            assert!(
                (separation - expected_chord).abs() < 1.0e-2,
                "bodies {i} and {j} are {separation} apart, expected {expected_chord}"
            );
        }
    }

    // And they surround the centre rather than clustering: the radial unit vectors sum
    // to zero for three vectors at 120°.
    let radial_sum: Vec3 = bodies
        .iter()
        .map(|prop| (prop.body.center().as_vec3() - centre).normalize())
        .sum();
    assert!(
        radial_sum.length() < 1.0e-3,
        "not evenly spaced: {radial_sum:?}"
    );
}

#[test]
fn every_ring_velocity_is_exactly_tangential() {
    // The premise of the experiment: tangential means perpendicular to the radius and
    // in the plane of the ring, so there is angular momentum and no radial component.
    // If this drifts, the experiment silently becomes "three bodies thrown somewhere".
    let centre = Vec3::new(0.0, THREE_BODY_HEIGHT_UNITS, 0.0);

    for seed in [1_u64, 2, 3, 42, 9999] {
        for prop in ring_bodies(seed) {
            let radial = (prop.body.center().as_vec3() - centre).normalize();
            let velocity = prop.velocity_units_s.as_vec3();
            let direction = velocity.normalize();

            assert!(
                direction.dot(radial).abs() < 1.0e-3,
                "seed {seed}: velocity has a radial component ({})",
                direction.dot(radial)
            );
            assert!(
                direction.y.abs() < 1.0e-4,
                "seed {seed}: velocity leaves the ring's plane by {}",
                direction.y
            );
        }
    }
}

#[test]
fn ring_speeds_are_inside_the_documented_window() {
    let reference = ring_circular_speed_units_s(THREE_BODY_RING_RADIUS_UNITS, THREE_BODY_MASS_KG);

    for seed in [1_u64, 5, 77, 12345, u64::MAX] {
        for prop in ring_bodies(seed) {
            let factor = (prop.speed() as f32) / reference;
            assert!(
                (THREE_BODY_MIN_SPEED_FACTOR..=THREE_BODY_MAX_SPEED_FACTOR).contains(&factor),
                "seed {seed}: speed factor {factor} is outside \
                 [{THREE_BODY_MIN_SPEED_FACTOR}, {THREE_BODY_MAX_SPEED_FACTOR}]"
            );
        }
    }
}

#[test]
fn the_ring_bodies_do_not_all_get_the_same_speed() {
    // Independent draws are what break the symmetry and make this a three-body problem
    // rather than a rotating rigid triangle. One shared factor would be a much duller
    // experiment that looked identical in the code.
    let bodies = ring_bodies(4242);
    let speeds: Vec<f64> = bodies.iter().map(Prop::speed).collect();

    assert!(
        (speeds[0] - speeds[1]).abs() > 1.0e-9 || (speeds[1] - speeds[2]).abs() > 1.0e-9,
        "all three speeds are identical: {speeds:?}"
    );
}

#[test]
fn the_ring_is_reproducible_from_its_seed_and_varies_between_seeds() {
    // Both halves matter: reproducible or the experiment is an anecdote; varying or the
    // seed is decoration.
    assert_eq!(three_body_ring(2024).scene, three_body_ring(2024).scene);
    assert_ne!(three_body_ring(2024).scene, three_body_ring(2025).scene);

    // And the seed is recorded, which is what makes an interesting run repeatable.
    assert_eq!(three_body_ring(2024).spec.seed, 2024);
}

#[test]
fn the_circular_speed_matches_the_documented_derivation() {
    // v = sqrt(G·m / (√3·R)), computed here from first principles in `f64` rather than
    // by copying the implementation, and checked against the figure the docs quote:
    // ~138 m/s at the shipped radius and mass.
    use crate::constants::METRES_PER_UNIT;
    use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

    let radius_m = f64::from(THREE_BODY_RING_RADIUS_UNITS) * METRES_PER_UNIT;
    let expected_m_s = (GRAVITATIONAL_CONSTANT * f64::from(THREE_BODY_MASS_KG)
        / (3.0_f64.sqrt() * radius_m))
        .sqrt();

    let actual_units_s =
        ring_circular_speed_units_s(THREE_BODY_RING_RADIUS_UNITS, THREE_BODY_MASS_KG);
    let actual_m_s = f64::from(actual_units_s) * METRES_PER_UNIT;

    assert!(
        (actual_m_s - expected_m_s).abs() < 1.0e-3 * expected_m_s,
        "got {actual_m_s} m/s, expected {expected_m_s}"
    );
    assert!(
        (135.0..142.0).contains(&actual_m_s),
        "the docs quote ~138 m/s, got {actual_m_s}"
    );
}

#[test]
fn a_nonsense_ring_radius_or_mass_yields_no_speed_rather_than_a_nan() {
    #[cfg(not(debug_assertions))]
    {
        assert_eq!(ring_circular_speed_units_s(0.0, THREE_BODY_MASS_KG), 0.0);
        assert_eq!(
            ring_circular_speed_units_s(f32::NAN, THREE_BODY_MASS_KG),
            0.0
        );
        assert_eq!(
            ring_circular_speed_units_s(THREE_BODY_RING_RADIUS_UNITS, -1.0),
            0.0
        );
    }

    // In debug the same inputs fire a `debug_assert!`; that contract is covered by the
    // panic tests below, so this body is release-only rather than duplicating them.
    #[cfg(debug_assertions)]
    {
        assert!(
            ring_circular_speed_units_s(THREE_BODY_RING_RADIUS_UNITS, THREE_BODY_MASS_KG) > 0.0
        );
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "radius must be finite and positive")]
fn a_zero_ring_radius_panics_in_debug() {
    let _ = ring_circular_speed_units_s(0.0, THREE_BODY_MASS_KG);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "mass must be finite and positive")]
fn a_negative_ring_mass_panics_in_debug() {
    let _ = ring_circular_speed_units_s(THREE_BODY_RING_RADIUS_UNITS, -1.0);
}

#[test]
fn an_invalid_spec_is_rejected() {
    let mut spec = two_body().spec;
    assert!(spec.is_valid());

    // A camera sitting on its own target has no view direction, which renders as a
    // black window and takes an hour to diagnose.
    spec.camera_target_units = spec.camera_position_units;
    assert!(!spec.is_valid());

    let mut spec = two_body().spec;
    spec.time_scale = 0.0;
    assert!(!spec.is_valid());

    let mut spec = two_body().spec;
    spec.time_scale = f32::NAN;
    assert!(!spec.is_valid());

    // A zero framed radius would make the framing check vacuous — it would be satisfied by
    // any camera anywhere.
    let mut spec = two_body().spec;
    spec.framed_radius_units = 0.0;
    assert!(!spec.is_valid());

    // `None` is the normal answer for a trail length; a present-but-nonsense one is not.
    let mut spec = two_body().spec;
    spec.trail_length_units = None;
    assert!(spec.is_valid());

    spec.trail_length_units = Some(-1.0);
    assert!(!spec.is_valid());

    spec.trail_length_units = Some(f32::NAN);
    assert!(!spec.is_valid());
}

#[test]
fn only_the_experiments_that_need_one_ask_for_a_trail_length() {
    // `None` means "the render layer's default is right", which is the answer for any scene
    // of the size the default was chosen for. An experiment overriding it should be doing so
    // because its paths are a different order of magnitude — as the solar system's are.
    assert_eq!(two_body().spec.trail_length_units, None);
    assert_eq!(three_body_ring(1).spec.trail_length_units, None);

    let solar = ExperimentId::SolarSystem.build_default().spec;
    let length = solar
        .trail_length_units
        .expect("the solar system's orbits are far larger than the default trail");
    assert!(length > crate::render::constants::TRAIL_LENGTH_UNITS);
}

/// Every experiment survives being *run*, not just built.
///
/// Drives each one through its own clock at its own scale for a few hundred frames —
/// the three-body ring included, where three chaotic orbits, a solid floor, and elastic
/// contact all interact. Asserts only what must hold for any experiment: every prop
/// stays valid, positions and velocities stay finite, and the clock keeps its
/// invariants. It deliberately does **not** assert a trajectory: a chaotic system has no
/// stable outcome to pin, and a test that demanded one would be pinning the integrator's
/// error rather than the physics.
///
/// This is the end-to-end guard for the layer. If a new experiment produces a scene
/// that explodes, tunnels, or NaNs within a few seconds of simulated watching, it fails
/// here rather than in the window.
#[test]
fn every_experiment_runs_without_going_invalid() {
    use crate::time::constants::MAX_REAL_STEP_SECONDS;
    use crate::time::simulation::SimulationClock;

    for id in ExperimentId::ALL {
        let Experiment {
            mut scene, spec, ..
        } = id.build(20_260_905);
        let mut clock = SimulationClock::new(spec.time_scale);

        // The app's worst-case frame, which is also the coarsest step the integrator
        // ever sees — the hardest case for both accuracy and contact detection.
        for frame in 0..400 {
            scene.step(clock.advance(MAX_REAL_STEP_SECONDS));

            for (index, prop) in scene.props.iter().enumerate() {
                assert!(
                    prop.is_valid(),
                    "{:?}: prop {index} went invalid on frame {frame}: {prop:?}",
                    id
                );
                assert!(
                    prop.body.center().is_finite() && prop.velocity_units_s.is_finite(),
                    "{:?}: prop {index} went non-finite on frame {frame}",
                    id
                );
            }
        }

        assert!(clock.is_valid());
        assert!(
            clock.elapsed_seconds() > 0.0,
            "{:?} did not advance at all",
            id
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Whatever the seed, a ring is a *valid* ring: three bodies, on the circle, with
    /// tangential velocities in the documented speed window. This is the property the
    /// experiment's premise rests on, over the whole seed space rather than a handful
    /// of hand-picked values.
    #[test]
    fn any_seed_produces_a_valid_tangential_ring(seed in any::<u64>()) {
        let centre = Vec3::new(0.0, THREE_BODY_HEIGHT_UNITS, 0.0);
        let reference =
            ring_circular_speed_units_s(THREE_BODY_RING_RADIUS_UNITS, THREE_BODY_MASS_KG);
        let bodies = ring_bodies(seed);

        prop_assert_eq!(bodies.len(), 3);

        for prop in bodies {
            prop_assert!(prop.is_valid());

            let offset = prop.body.center().as_vec3() - centre;
            prop_assert!((offset.length() - THREE_BODY_RING_RADIUS_UNITS).abs() < 1.0e-3);

            let direction = prop.velocity_units_s.as_vec3().normalize();
            prop_assert!(direction.dot(offset.normalize()).abs() < 1.0e-3);

            let factor = (prop.speed() as f32) / reference;
            prop_assert!(factor >= THREE_BODY_MIN_SPEED_FACTOR - 1.0e-4);
            prop_assert!(factor <= THREE_BODY_MAX_SPEED_FACTOR + 1.0e-4);
        }
    }

    /// Every experiment, built from any seed, produces a scene the simulation can
    /// legally step: valid props, valid spec, nothing in contact.
    #[test]
    fn any_seed_produces_a_runnable_experiment(seed in any::<u64>()) {
        for id in ExperimentId::ALL {
            let experiment = id.build(seed);

            prop_assert!(experiment.spec.is_valid());
            for prop in &experiment.scene.props {
                prop_assert!(prop.is_valid());
            }
        }
    }
}
