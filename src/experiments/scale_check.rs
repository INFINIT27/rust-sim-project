//! `experiment_5` — the Sun and the eight planets in a static row, for checking the scale by eye.
//!
//! Not an experiment: a **measuring instrument**. Nothing here moves, nothing attracts anything,
//! and there is no clock worth reading. It exists because `experiment_3` draws everything at true
//! size and real distance, which is correct and makes the planets sub-pixel — so the one thing you
//! cannot do there is *check* that the sizes are right. Here they are lined up against each other
//! where the ratios are directly readable.
//!
//! # The arrangement
//!
//! Nine spheres along `+X` in orbital order, each at its true radius, **bottoms aligned** on the
//! plane `y = 0` — so a body's height above that plane *is* its diameter and the comparison is a
//! vertical one, the way every planet-size chart is drawn. Separated by
//! [`SCALE_CHECK_GAP_UNITS`], which is the whole of the layout's arbitrariness.
//!
//! From the opening camera, `2.8` units back, that reads as:
//!
//! | Body | True radius | On screen |
//! | --- | --- | --- |
//! | Sun | `0.6957` u | `1012` px |
//! | Jupiter | `0.0699` u | `102` px |
//! | Saturn | `0.0582` u | `85` px |
//! | Uranus | `0.0254` u | `37` px |
//! | Neptune | `0.0246` u | `36` px |
//! | Earth | `0.00637` u | `9.3` px |
//! | Venus | `0.00605` u | `8.8` px |
//! | Mars | `0.00339` u | `4.9` px |
//! | Mercury | `0.00244` u | `3.6` px |
//!
//! Every one of those is the true proportion — the Sun really is `109×` the Earth and `10×`
//! Jupiter — and they all fit in one view because the *row* is `1.94` units long while the solar
//! system is `4495`. Removing the distances is the whole trick. Fly right along the row and the
//! four terrestrial planets fill the screen in turn.
//!
//! # Why every body is immovable
//!
//! Because a `1.989e30 kg` Sun with Mercury parked `0.72` units away is not a scene that survives
//! one frame. [`Prop::immovable`] sets `mass_kg` to `0.0`, which makes a prop **gravitationally
//! inert** — it neither moves nor pulls — so the row is geometry rather than an initial condition.
//! That is the honest way to build a ruler: `P` would pause the clock, but a paused simulation is
//! still a simulation waiting to happen, and one stray unpause would collapse the row into the
//! Sun.
//!
//! It does mean this scene has no masses in it, so the overlay's mass column reads zero. The
//! measurements are still real — they are the same [`SOLAR_SYSTEM_PLANETS`] rows `experiment_3`
//! orbits, read for their radii instead of their dynamics.
//!
//! # What it is for, concretely
//!
//! Answering "is the scale right?" without trusting the code that produced it. The ratios you can
//! read off the screen — Sun to Jupiter, Jupiter to Earth, Earth to Mercury — are checkable against
//! any reference, which is a property no amount of unit testing provides. `scale_check_tests` pins
//! the same ratios numerically; this scene is the version you can look at.

pub use bevy::math::{Quat, Vec3};

use crate::core::scene::{Aabb, Body, Pose, Prop, Scene};
use crate::experiments::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::experiments::constants::{
    DEFAULT_RANDOM_SEED, SCALE_CHECK_CAMERA_OFFSET_UNITS, SCALE_CHECK_FRAMED_RADIUS_UNITS,
    SCALE_CHECK_GAP_UNITS, SCALE_CHECK_TIME_SCALE, SOLAR_SYSTEM_PLANETS, SOLAR_SYSTEM_SUN_COLOR,
    SOLAR_SYSTEM_SUN_RADIUS_M,
};
use crate::experiments::solar_system::radius_units;

/// One entry in the row: a name, a true radius in metres, and a colour.
///
/// Deliberately not [`Planet`](crate::experiments::solar_system::Planet): a row in this scene has
/// no orbit and no mass, and reusing a type with four unused fields would imply this experiment
/// cares about them. The Sun is a row here too, which `Planet` could not express.
///
/// [`METRES_PER_UNIT`](crate::constants::METRES_PER_UNIT) conversion happens once, in
/// [`scale_check_layout`], via [`radius_units`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ScaleRow {
    /// Display name.
    pub name: &'static str,
    /// True radius, in metres.
    pub radius_m: f64,
    /// Linear-space colour, taken from whichever table the body already lives in.
    pub color: crate::core::scene::Color,
}

/// The row, in orbital order: the Sun, then the eight planets.
///
/// Built from [`SOLAR_SYSTEM_PLANETS`] and the Sun's own constants rather than a table of its own,
/// so a radius cannot disagree with the experiment that orbits it. That is the point of a
/// measuring instrument: it has to be measuring the same thing.
pub fn scale_rows() -> Vec<ScaleRow> {
    let mut rows = Vec::with_capacity(SOLAR_SYSTEM_PLANETS.len() + 1);

    rows.push(ScaleRow {
        name: "Sun",
        radius_m: SOLAR_SYSTEM_SUN_RADIUS_M,
        color: SOLAR_SYSTEM_SUN_COLOR,
    });

    for planet in SOLAR_SYSTEM_PLANETS {
        rows.push(ScaleRow {
            name: planet.name,
            radius_m: planet.radius_m,
            color: planet.color,
        });
    }

    rows
}

/// Where each row's **centre** sits, in world units, and how long the row is.
///
/// Bottoms aligned on `y = 0`, so a body of radius `r` has its centre at `y = r` and its top at
/// `2r` — the height above the plane is the diameter, which is what makes the comparison readable
/// as a chart rather than as a scatter of spheres.
///
/// Pure, so the layout is testable without a window — the same split the grid, the compass, and the
/// trails use.
///
/// # Returns
///
/// One centre per row, in the order given, plus the total length of the row along `X`. An empty
/// input gives an empty layout and a zero length rather than a `NaN`.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{scale_check_layout, scale_rows};
///
/// let (centres, length) = scale_check_layout(&scale_rows());
///
/// // The Sun is first, and its centre is one radius in from the left end.
/// assert!((centres[0].x - 0.6957).abs() < 1.0e-3);
/// // Bottoms aligned: a body's centre height is its own radius.
/// assert!((centres[0].y - 0.6957).abs() < 1.0e-3);
///
/// assert!((length - 1.944).abs() < 0.01, "the whole row is under two units long");
/// ```
pub fn scale_check_layout(rows: &[ScaleRow]) -> (Vec<Vec3>, f32) {
    let mut centres = Vec::with_capacity(rows.len());
    let mut cursor = 0.0_f32;

    for (index, row) in rows.iter().enumerate() {
        let radius = radius_units(row.radius_m);

        cursor += radius;
        centres.push(Vec3::new(cursor, radius, 0.0));
        cursor += radius;

        if index + 1 < rows.len() {
            cursor += SCALE_CHECK_GAP_UNITS;
        }
    }

    (centres, cursor)
}

/// `experiment_5` — the size comparison.
///
/// Deterministic and static: the seed plays no part and nothing in the scene can move. Recorded as
/// [`DEFAULT_RANDOM_SEED`] so the overlay prints something honest.
///
/// # What it builds
///
/// Nine **immovable** spheres and no floor. Immovable because a real Sun beside a real Mercury at
/// `0.72` units is not survivable for one frame, and no floor because the row is `1.94` units long
/// against a `5000`-unit ground plane — see [`Scene::empty`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{scale_check, SOLAR_SYSTEM_PLANETS};
///
/// let experiment = scale_check();
///
/// // The Sun and the eight planets, and nothing else.
/// assert_eq!(experiment.scene.props.len(), SOLAR_SYSTEM_PLANETS.len() + 1);
///
/// // Nothing moves, and nothing pulls.
/// assert!(experiment.scene.props.iter().all(|prop| prop.immovable));
/// assert!(experiment.scene.props.iter().all(|prop| prop.mass_kg == 0.0));
/// ```
pub fn scale_check() -> Experiment {
    let rows = scale_rows();
    let (centres, length) = scale_check_layout(&rows);

    let tallest = rows
        .iter()
        .map(|row| radius_units(row.radius_m))
        .fold(0.0_f32, f32::max);

    let mut scene = Scene::empty(Aabb {
        min: Vec3::new(0.0, 0.0, -tallest),
        max: Vec3::new(length, 2.0 * tallest, tallest),
    });

    for (row, centre) in rows.iter().zip(&centres) {
        scene.add(Prop::immovable(Body::Sphere {
            pose: Pose {
                position: centre.as_dvec3(),
                rotation: Quat::IDENTITY,
            },
            radius: radius_units(row.radius_m),
            color: row.color,
        }));
    }

    // Framed on the row's centre, at the Sun's own centre height — so the Sun fills the left of
    // the view and the terrestrial planets sit along the bottom right, which is the arrangement
    // that makes the ratio readable rather than merely present.
    let target = Vec3::new(0.5 * length, tallest, 0.0);

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::ScaleCheck,
            seed: DEFAULT_RANDOM_SEED,
            // Nothing in this scene can move, so the clock is decorative. Real time is the least
            // surprising thing for it to say, and `is_valid` requires it to be positive.
            time_scale: SCALE_CHECK_TIME_SCALE,
            camera_position_units: target + SCALE_CHECK_CAMERA_OFFSET_UNITS,
            camera_target_units: target,
            framed_radius_units: SCALE_CHECK_FRAMED_RADIUS_UNITS,
            // Immovable props are never trailed, so this would have no effect either way.
            trail_length_units: None,
            launches: false,
        },
        names: BodyNames::default(),
    }
}

#[cfg(test)]
#[path = "../../tst/experiments/scale_check_tests.rs"]
mod tests;
