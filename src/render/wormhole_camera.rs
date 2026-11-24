//! The camera's way through the wormhole: where it is, and how flying moves it.
//!
//! An ordinary position cannot say where a camera in the tunnel is, since every point of the tunnel
//! is at the same radius. So in the wormhole experiment the camera's place is kept here
//! ([`WormholeTraveller`]), as `ℓ` and a label (see [`crate::render::wormhole_ray`] for the frames),
//! and the camera's `Transform` follows from it.
//!
//! # How it moves
//!
//! The free-fly camera adds a displacement to its `Transform`, as in flat space. Each frame
//! [`wormhole_camera_system`] takes that displacement as a direction and a distance and follows the
//! straightest path through the curved space that far ([`travel`]). That path is also light's, so
//! the camera goes where its crosshair points, including through the throat and out of the other
//! side. Its orientation is carried along without turning (parallel transport), so the view never
//! jumps.
//!
//! # The two sides' axes
//!
//! Each side is an ordinary flat space with its own fixed axes, and the camera's `Transform` is in
//! those of the side it is on. They change over at the middle of the tunnel, where the camera's
//! world rotation is turned by exactly as much as the axes are: the picture does not change, but the
//! compass does. Where the crossing is level this is a turn about the vertical. A slanted crossing
//! leaves the vertical itself tilted; [`FreeFlyController::frame`] holds what "up" and "level" have
//! become, so flying stays consistent with what is seen.
//!
//! Round a closed loop through curved space a carried direction comes back turned. That is the
//! curvature, not a fault, and only happens near the mouth.

use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, ResMut, Resource};
use bevy::math::{DQuat, Vec3};
use bevy::transform::components::Transform;

pub use crate::core::wormhole::{far_rotation, travel, WormholePlace};
use crate::input::camera::{CameraLock, FreeFlyController};
use crate::physics_utils::relativity::wormhole::WormholeShape;
use crate::projects::interstellar::catalog::ExperimentSpec;
use crate::projects::interstellar::wormhole::{is_wormhole, WormholeSettings};

/// Where the camera is in the wormhole experiment. Absent or unused in every other.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WormholeTraveller {
    /// The camera's place, once the experiment has started.
    pub place: Option<WormholePlace>,
    /// The `Transform` translation last written, to tell what moved it since.
    written: Vec3,
    /// The settings the place was worked out for.
    settings: Option<WormholeSettings>,
}

/// Bevy system that moves the camera through the wormhole, in its experiment only.
///
/// Runs in `PostUpdate` after everything else that moves the camera, so the one displacement it
/// sees is the whole frame's. While the camera is locked on a body it orbits it on the side it is
/// on, kept outside the mouth; unlocked, it flies ([`travel`]). A change of settings keeps it as far
/// from the mouth, in throat radii.
pub fn wormhole_camera_system(
    spec: Option<Res<ExperimentSpec>>,
    settings: Res<WormholeSettings>,
    mut traveller: ResMut<WormholeTraveller>,
    mut camera: Query<
        (&mut Transform, &mut FreeFlyController, Option<&CameraLock>),
        With<Camera3d>,
    >,
) {
    if !spec.is_some_and(|spec| is_wormhole(&spec)) {
        return;
    }
    let Ok((mut transform, mut controller, lock)) = camera.get_single_mut() else {
        return;
    };
    let shape = settings.shape();
    let moved = transform.translation;
    let (before, resized) = match (traveller.place, traveller.settings) {
        (Some(place), Some(old)) if old != *settings => {
            (place.rescaled(&old.shape(), &shape), true)
        }
        (Some(place), _) => (place, false),
        // The opening view is the experiment's, on the near side, in throat radii: the experiment is
        // laid out for the shipped throat.
        (None, _) => {
            let shipped = WormholeSettings::default().shape();
            let place = WormholePlace::at(&shipped, moved.as_dvec3(), false);
            (place.rescaled(&shipped, &shape), false)
        }
    };

    let mut after = before;
    if lock.is_some_and(CameraLock::is_locked) {
        after = WormholePlace::at(&shape, moved.as_dvec3(), before.l < 0.0);
    } else if traveller.place.is_some() {
        let displacement = (moved - traveller.written).as_dvec3();
        let distance = displacement.length();
        if distance > 0.0 {
            let frame = before.frame();
            let direction = frame.inverse() * (displacement / distance);
            let (place, carried) = travel(&shape, before, direction, distance);
            after = place;
            // From the old side's world axes to the new one's.
            let turn = (after.frame() * carried * frame.inverse()).normalize();
            if turn.angle_between(DQuat::IDENTITY) > 0.0 {
                let turned = (turn * controller.frame.as_dquat()).normalize().as_quat();
                let rotation = (turn * transform.rotation.as_dquat()).normalize().as_quat();
                controller.frame = turned;
                transform.rotation = rotation;
            }
        }
    }

    let position = after.position(&shape).as_vec3();
    if resized || after != before || traveller.place.is_none() {
        transform.translation = position;
    }
    traveller.written = transform.translation;
    traveller.place = Some(after);
    traveller.settings = Some(*settings);
}

/// What the debug overlay says of the camera's place: the side, `ℓ`, and the radius, in throat
/// radii.
pub fn place_line(place: &WormholePlace, shape: &WormholeShape) -> String {
    let throat = shape.throat();
    let side = if place.l.abs() < shape.half_length() {
        "in the tunnel"
    } else if place.l < 0.0 {
        "far side"
    } else {
        "near side"
    };
    format!(
        "wormhole: {side}, l = {:.3} rho, r = {:.3} rho",
        place.l / throat,
        shape.radius(place.l) / throat
    )
}

#[cfg(test)]
#[path = "../../tst/render/wormhole_camera_tests.rs"]
mod tests;
