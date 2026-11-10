//! Placing an interstellar object into the running scene.
//!
//! The third of three modules behind the `B` key: [`crate::input::placement`] holds the
//! selection and sets a flag, [`crate::ui::placement`] draws the picker, and this one adds
//! the prop. It is here rather than in `input` for the layer rule that made this crate's
//! shape: **`input` never reads or writes the [`Scene`]**, and placing an object is nothing
//! but writing to it.
//!
//! # Where the object goes
//!
//! In front of the camera, at rest — see [`placement_position`] for the distance and why it
//! depends on what is being placed. At rest because it is the one initial condition that
//! needs no further intent from the person placing it: whatever gravity is already there
//! takes over immediately.
//!
//! # It really is placed into the running experiment
//!
//! Not a separate list, not a decoration: the prop joins `Scene::props` and from that instant
//! it pulls on every other prop, is pulled by them, collides, and — if it is a black hole —
//! absorbs. `sync_mirror_entities_system` gives it an entity on the same frame, and
//! `Trails::record_from` starts trailing it because it is movable.
//!
//! The consequence worth knowing before you press `Enter`: **a black hole placed in a
//! planetary experiment ends that experiment.** A `10 M☉` hole is 3.3 million Earth masses;
//! dropped `40` units from the inner solar system it does not perturb the orbits, it removes
//! them. Sgr A* is `4.3 million` solar masses and will fling everything out of the world
//! bounds within a frame or two. Both are the correct Newtonian answer to an absurd question,
//! and both are worth seeing once.

use bevy::app::{App, Plugin, Update};
use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Query, ResMut};
use bevy::math::Vec3;
use bevy::transform::components::Transform;

use crate::core::scene::Scene;
use crate::experiments::catalog::BodyNames;
use crate::input::placement::PlacementState;
use crate::render::constants::{PLACEMENT_DISTANCE_UNITS, PLACEMENT_RADII_CLEARANCE};

/// Where an object of drawn radius `radius_units` should be placed, given a camera at
/// `camera_position` looking along `camera_forward`.
///
/// # The distance is not a constant, and it cannot be
///
/// [`PLACEMENT_DISTANCE_UNITS`] is a floor, not the answer: the objects in the catalog span
/// drawn radii from `0.30` units to `16.8`, so a fixed distance that comfortably frames a
/// neutron star puts the camera **inside** a red dwarf. The distance is therefore the larger
/// of the floor and [`PLACEMENT_RADII_CLEARANCE`] times the object's own radius, which keeps
/// every object fully in front of the viewer whatever it is.
///
/// # Returns
///
/// A finite position, always. A non-finite camera pose or a degenerate forward vector falls
/// back to placing the object at the camera's own position rather than producing a `NaN`
/// that would poison the scene the moment it was added — the object appears in the wrong
/// place, which is visible, instead of silently breaking every subsequent step.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{placement_position, Vec3};
///
/// // Looking down -Z from the origin, placing something small: straight ahead.
/// let at = placement_position(Vec3::ZERO, -Vec3::Z, 0.3);
/// assert!(at.z < 0.0 && at.x == 0.0);
///
/// // Something large is placed further out, so it is not around your ears.
/// let small = placement_position(Vec3::ZERO, -Vec3::Z, 0.3).length();
/// let large = placement_position(Vec3::ZERO, -Vec3::Z, 16.8).length();
/// assert!(large > small);
/// ```
pub fn placement_position(camera_position: Vec3, camera_forward: Vec3, radius_units: f32) -> Vec3 {
    debug_assert!(
        camera_position.is_finite() && camera_forward.is_finite(),
        "placement_position: camera pose must be finite, got {:?} and {:?}",
        camera_position,
        camera_forward
    );

    if !camera_position.is_finite() {
        return Vec3::ZERO;
    }

    let direction = camera_forward.normalize_or_zero();
    if direction == Vec3::ZERO {
        return camera_position;
    }

    let radius = if radius_units.is_finite() && radius_units > 0.0 {
        radius_units
    } else {
        0.0
    };

    let distance = PLACEMENT_DISTANCE_UNITS.max(radius * PLACEMENT_RADII_CLEARANCE);

    camera_position + direction * distance
}

/// Bevy system that carries out a placement request.
///
/// Consumes [`PlacementState::place_requested`], clears it, and adds the selected object's
/// prop to the [`Scene`]. Does nothing on a frame with no request, which is almost all of
/// them.
///
/// # Why the flag is cleared before the work
///
/// The request is cleared as soon as it is seen, so a frame in which the camera is missing —
/// possible on the first tick under some plugin orderings — consumes the keypress rather than
/// leaving it armed to fire later, when the view has moved somewhere else entirely.
pub fn place_object_system(
    mut placer: ResMut<PlacementState>,
    mut scene: ResMut<Scene>,
    mut names: Option<ResMut<BodyNames>>,
    camera: Query<&Transform, With<Camera3d>>,
) {
    if !placer.place_requested() {
        return;
    }
    placer.clear_request();

    let Ok(transform) = camera.get_single() else {
        return;
    };

    let object = placer.selected();
    // The **contact** radius, not the drawn one: since amendment A16 a body is stored at its true
    // size, and what matters for clearance is the radius at which it can destroy something. Sgr
    // A*'s tidal disruption radius is `113` units against a `12.7`-unit horizon, so placing it by
    // its drawn size would put the camera — and anything near it — inside the region it consumes.
    let position = placement_position(
        transform.translation,
        transform.forward().as_vec3(),
        object.contact_radius_units(),
    );

    let id = scene.add(object.to_prop(position.as_dvec3()));
    if let Some(names) = names.as_mut() {
        names.insert(id, object.name);
    }
}

/// Plugin that places objects the picker has asked for.
///
/// Add it after [`ScenePlugin`](crate::render::plugin::ScenePlugin), which inserts the
/// [`Scene`] this writes to, and alongside
/// [`PlacementInputPlugin`](crate::input::placement::PlacementInputPlugin), which owns the
/// keys. Registered to run **before** `step_scene_system` so an object placed this frame is
/// integrated on this frame rather than sitting still for one tick.
pub struct PlacementPlugin;

impl Plugin for PlacementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlacementState>();
        app.add_systems(
            Update,
            place_object_system.before(crate::render::plugin::step_scene_system),
        );
    }
}

#[cfg(test)]
#[path = "../../tst/render/placement_tests.rs"]
mod tests;
