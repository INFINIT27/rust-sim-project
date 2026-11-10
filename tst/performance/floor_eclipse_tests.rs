//! The measurement behind [`crate::performance::constants::DEFAULT_FLOOR_ECLIPSE`]: how dark can an
//! eclipse shadow on experiment_3's floor get?
//!
//! `#[ignore]` because it simulates two years and evaluates millions of floor points; run it with
//! `cargo test --release -- --ignored darkest_floor_eclipse --nocapture`.

use crate::core::scene::Body;
use crate::experiments::catalog::ExperimentId;
use crate::render::eclipse::star_visible_fraction;
use crate::time::simulation::TimeStep;
use bevy::math::Vec3;

const DAYS: usize = 730;
const DISTANCE_SAMPLES: usize = 2000;

#[test]
#[ignore = "two simulated years; run on demand to re-derive the floor-eclipse figure"]
fn darkest_floor_eclipse() {
    let mut scene = ExperimentId::SolarSystem.build_default().scene;
    let day = TimeStep::from_seconds(86_400.0).unwrap();
    let mut darkest = 1.0_f32;

    for _ in 0..DAYS {
        let (floor_centre, half_size) = scene
            .props
            .iter()
            .find_map(|prop| match &prop.body {
                Body::Plane {
                    pose, half_size, ..
                } => Some((pose.position.as_vec3(), *half_size)),
                _ => None,
            })
            .expect("experiment_3 has a floor");
        let (star_centre, star_radius) = scene
            .props
            .iter()
            .find(|prop| prop.is_luminous())
            .and_then(|prop| match &prop.body {
                Body::Sphere { pose, radius, .. } => Some((pose.position.as_vec3(), *radius)),
                _ => None,
            })
            .expect("experiment_3 has a star");

        for prop in scene.props.iter().filter(|prop| !prop.is_luminous()) {
            let Body::Sphere { pose, radius, .. } = &prop.body else {
                continue;
            };
            // The darkest floor point lies under the shadow's axis, so the search runs along it at
            // log-spaced distances, with a few lateral offsets for the penumbra's width.
            let position = pose.position.as_vec3();
            let axis = (position - star_centre).normalize();
            let across = axis.cross(Vec3::Y).normalize();
            for step in 0..DISTANCE_SAMPLES {
                let distance = 1e-3_f32 * 1e7_f32.powf(step as f32 / (DISTANCE_SAMPLES - 1) as f32);
                for offset in -4..=4 {
                    let mut point =
                        position + axis * distance + across * (offset as f32 * radius * 0.5);
                    point.y = floor_centre.y;
                    let local = point - floor_centre;
                    if local.x.abs() > half_size || local.z.abs() > half_size {
                        continue;
                    }
                    darkest = darkest.min(star_visible_fraction(
                        point,
                        star_centre,
                        star_radius,
                        position,
                        *radius,
                    ));
                }
            }
        }
        scene.step(day);
    }

    println!("darkest floor visibility over {DAYS} days: {darkest:.6}");
    // Recorded in `src/performance/README.md`: `1.3%` dimming. A large move means the scene changed
    // and the README's tradeoff needs re-deriving.
    assert!(
        (0.98..1.0).contains(&darkest),
        "darkest floor visibility {darkest} has moved from ~0.987"
    );
}
