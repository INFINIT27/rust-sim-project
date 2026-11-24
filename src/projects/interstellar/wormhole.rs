//! `experiment_9` — a traversable wormhole between two skies.
//!
//! The geometry and light's paths are in [`crate::physics_utils::relativity::wormhole`]; the drawing
//! and the flight through it are in [`crate::render::wormhole_view`] and
//! [`crate::render::wormhole_camera`]. Here are its settings and the scene, which is empty but for
//! what the `B` key places. There is no gravity: see [`crate::core::wormhole`].

use crate::core::scene::{Aabb, Scene, Vec3};
use crate::core::wormhole::WormholeSpace;
use crate::physics_utils::relativity::wormhole::WormholeShape;
use crate::projects::interstellar::catalog::{BodyNames, Experiment, ExperimentId, ExperimentSpec};
use crate::projects::interstellar::constants::{
    DEFAULT_RANDOM_SEED, DEFAULT_WORMHOLE_HALF_LENGTH, DEFAULT_WORMHOLE_LENSING_WIDTH,
    DEFAULT_WORMHOLE_THROAT_UNITS, WORMHOLE_CAMERA_DISTANCE_THROATS, WORMHOLE_FRAMED_THROATS,
    WORMHOLE_HALF_LENGTH_RANGE, WORMHOLE_LENSING_WIDTH_RANGE, WORMHOLE_THROAT_RANGE_UNITS,
    WORMHOLE_TIME_SCALE,
};

/// The wormhole's shape, as set in `config/wormhole.toml` and the menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WormholeSettings {
    /// The throat's radius `ρ`, in world units.
    pub throat_units: f32,
    /// Half the tunnel's length, `a/ρ`.
    pub half_length: f32,
    /// The lensing width, `𝒲/ρ`.
    pub lensing_width: f32,
}

impl Default for WormholeSettings {
    fn default() -> Self {
        Self {
            throat_units: DEFAULT_WORMHOLE_THROAT_UNITS,
            half_length: DEFAULT_WORMHOLE_HALF_LENGTH,
            lensing_width: DEFAULT_WORMHOLE_LENSING_WIDTH,
        }
    }
}

impl WormholeSettings {
    /// Each value inside its range: [`WORMHOLE_THROAT_RANGE_UNITS`], [`WORMHOLE_HALF_LENGTH_RANGE`]
    /// and [`WORMHOLE_LENSING_WIDTH_RANGE`].
    pub fn is_valid(&self) -> bool {
        let within = |x: f32, (low, high): (f32, f32)| x >= low && x <= high;
        within(self.throat_units, WORMHOLE_THROAT_RANGE_UNITS)
            && within(self.half_length, WORMHOLE_HALF_LENGTH_RANGE)
            && within(self.lensing_width, WORMHOLE_LENSING_WIDTH_RANGE)
    }

    /// The shape these describe, in world units. Invalid settings give the default's shape.
    pub fn shape(&self) -> WormholeShape {
        let settings = if self.is_valid() {
            *self
        } else {
            Self::default()
        };
        let throat = f64::from(settings.throat_units);
        let half_length = f64::from(settings.half_length) * throat;
        let flare = f64::from(settings.lensing_width) * throat
            / crate::physics_utils::relativity::wormhole::lensing_width_per_flare();
        WormholeShape::Dneg {
            throat,
            half_length,
            flare,
        }
    }
}

/// Whether `spec` is the wormhole experiment.
pub fn is_wormhole(spec: &ExperimentSpec) -> bool {
    spec.id == ExperimentId::Wormhole
}

/// `experiment_9`: the mouth of a wormhole, seen from the near side, level with it and along `z`.
///
/// Nothing is in it but what the `B` key places. Nothing pulls on anything (see
/// [`crate::core::wormhole`]).
pub fn wormhole() -> Experiment {
    let throat = DEFAULT_WORMHOLE_THROAT_UNITS;
    let mut scene = Scene::empty(Aabb {
        min: Vec3::splat(-WORMHOLE_FRAMED_THROATS * throat),
        max: Vec3::splat(WORMHOLE_FRAMED_THROATS * throat),
    });
    scene.wormhole = Some(WormholeSpace::new(WormholeSettings::default().shape()));
    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::Wormhole,
            seed: DEFAULT_RANDOM_SEED,
            time_scale: WORMHOLE_TIME_SCALE,
            camera_position_units: Vec3::Z * WORMHOLE_CAMERA_DISTANCE_THROATS * throat,
            camera_target_units: Vec3::ZERO,
            framed_radius_units: WORMHOLE_FRAMED_THROATS * throat,
            trail_length_units: None,
            // A flight is planned under gravity, and there is none here.
            launches: false,
            general_lighting: true,
        },
        names: BodyNames::default(),
    }
}

#[cfg(test)]
#[path = "../../../tst/projects/interstellar/wormhole_tests.rs"]
mod tests;
