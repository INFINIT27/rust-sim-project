//! Render bridge: pure functions that map `Scene` bodies onto Bevy
//! `Transform`s, plus (later) the per-frame `sync_scene_to_ecs_system`
//! that writes those transforms onto ECS entities.
//!
//! Unlike the `scene` module — which is kept free of every Bevy crate
//! other than `bevy::math` (Requirement 15.3) — this module is the
//! designated boundary where the pure `Scene` model crosses into Bevy's
//! render world, so it is allowed to import freely from `bevy`.
//!
//! Task 6.1 lands the `transform_from` function here. Subsequent tasks
//! (7.5) add `sync_scene_to_ecs_system` alongside it.

use bevy::core_pipeline::core_3d::Camera3d;
use bevy::ecs::query::Without;
use bevy::prelude::{Quat, Query, Res, Transform, Vec3};

use crate::core::scene::{Body, Scene};
use crate::render::plugin::MirroredBody;

/// Map a `Body` to the `bevy::prelude::Transform` used to render it.
///
/// Contract (Requirements 8.1–8.6, 14.2):
///
/// - For any `body` satisfying the Validation_Rules, every component of the
///   returned `translation`, `rotation`, and `scale` is finite
///   (Requirement 8.1) and `rotation` has magnitude within `1e-4` of `1.0`
///   (Requirement 8.2).
/// - `Body::Sphere { pose, radius, .. }` →
///   `Transform { translation: pose.position, rotation: pose.rotation,
///   scale: Vec3::splat(radius) }` (Requirement 8.3).
/// - `Body::Cuboid { pose, half_extents, .. }` →
///   `Transform { translation: pose.position, rotation: pose.rotation,
///   scale: 2.0 * half_extents }` (Requirement 8.4).
/// - `Body::Plane { pose, half_size, .. }` →
///   `Transform { translation: pose.position, rotation: pose.rotation,
///   scale: Vec3::new(2.0 * half_size, 1.0, 2.0 * half_size) }`
///   (Requirement 8.5).
/// - `Body::Line { from, to, .. }` → a `Transform` whose `translation` is
///   `0.5 * (from + to)`, whose `scale.x` is `(to - from).length()`, and
///   whose `rotation` maps `Vec3::X` onto `(to - from).normalize_or_zero()`
///   (Requirement 8.6).
///
/// # Degenerate `Line` (`from == to`)
///
/// Requirement 14.2 requires this function to be total — no panics — even
/// for a degenerate `Line` where `from == to`. In that case:
///
/// - `dir = to - from == Vec3::ZERO`, so `dir.length() == 0.0` and the
///   returned `scale.x == 0.0` (the line renders as empty rather than
///   crashing the renderer).
/// - `dir.normalize_or_zero() == Vec3::ZERO` (per `glam`'s documented
///   contract: zero-length vectors normalize to `Vec3::ZERO` rather than
///   producing NaN).
/// - `Quat::from_rotation_arc(Vec3::X, Vec3::ZERO)` does not panic under
///   Bevy's default feature set (glam's `glam-assert` is off by default,
///   so the non-normalized-input assertion is compiled out) and evaluates
///   to `Quat::IDENTITY`: with `dot == 0.0` neither singularity branch is
///   taken, `from.cross(to) == Vec3::ZERO`, and the final
///   `Quat::from_xyzw(0, 0, 0, 1.0).normalize()` is the identity, a
///   unit quaternion. The returned `rotation` therefore still satisfies
///   Requirement 8.2 (`|rotation| ≈ 1`) even on degenerate lines.
#[inline]
pub fn transform_from(body: &Body) -> Transform {
    match body {
        Body::Sphere { pose, radius, .. } => Transform {
            translation: pose.position.as_vec3(),
            rotation: pose.rotation,
            // The body's **true** radius, with nothing added. Amendment A17 removed the
            // apparent-size floor that used to sit here: a body at real scale is sub-pixel from
            // any camera that can also see its orbit, and lifting it to a visible minimum made
            // every body on screen the same size — including the Sun. Drawing the truth means
            // distant planets are not drawn at all, and the orbits are read from their trails.
            scale: Vec3::splat(*radius),
        },
        Body::Cuboid {
            pose, half_extents, ..
        } => Transform {
            translation: pose.position.as_vec3(),
            rotation: pose.rotation,
            scale: 2.0 * *half_extents,
        },
        Body::Plane {
            pose, half_size, ..
        } => Transform {
            translation: pose.position.as_vec3(),
            rotation: pose.rotation,
            scale: Vec3::new(2.0 * *half_size, 1.0, 2.0 * *half_size),
        },
        Body::Line { from, to, .. } => {
            // Lines are rendered from a unit-length mesh aligned with +X:
            // translate to the midpoint, rotate to aim at `to`, and scale
            // the X axis by `|to - from|`. `normalize_or_zero` keeps this
            // total on degenerate inputs (`from == to`) — see the
            // degenerate-line note in the doc comment above.
            let dir = (*to - *from).as_vec3();
            let len = dir.length();
            let rotation = Quat::from_rotation_arc(Vec3::X, dir.normalize_or_zero());
            Transform {
                translation: (0.5 * (*from + *to)).as_vec3(),
                rotation,
                scale: Vec3::new(len, 1.0, 1.0),
            }
        }
    }
}

/// Bevy system that mirrors the current `Scene` state onto every ECS
/// entity tagged with [`MirroredBody`].
///
/// Contract (Requirements 2.3, 9.1, 9.2, 9.3, 9.4):
///
/// - For every entity matching `(&MirroredBody(id), &mut Transform)`:
///   - if `scene.get(id) == Some(body)`, overwrite `*transform` with
///     `transform_from(body)` (Requirements 9.1, 9.4);
///   - if `scene.get(id) == None`, leave the entity's `Transform`
///     unchanged (Requirement 9.2). This path is unreachable in the
///     current implementation — nothing removes bodies from `Scene`
///     between the startup spawn and the sync — but handling it
///     explicitly is what makes the system total if a future cleanup
///     system ever lands.
/// - Never mutates `Scene`. The `Res<Scene>` parameter type enforces
///   this at compile time (Requirement 9.3); a `ResMut<Scene>` here
///   would silently allow the body of `scene.step` to sneak into the
///   render-bridge phase and break the `step → sync` ordering of
///   Requirement 2.1.
///
/// Complexity: `O(n)` in the number of mirrored entities, with one
/// `Scene::get` lookup per entity. `Scene::get` itself is `O(n)` in
/// the number of bodies for now (linear scan), so the whole system is
/// `O(n·m)`. At the scale of the demo scene — a handful of bodies —
/// that is immaterial, and it keeps `Scene` free of a hash map for
/// tasks where the body count is tiny. A future refactor can swap the
/// `Scene::get` implementation without touching this system.
pub fn sync_scene_to_ecs_system(
    scene: Res<Scene>,
    mut query: Query<(&MirroredBody, &mut Transform), Without<Camera3d>>,
) {
    for (MirroredBody(id), mut transform) in &mut query {
        // `Option::map` would be tidier but would force a borrow of
        // `transform` through the closure; the explicit `if let` keeps
        // the mutable borrow scoped to the assignment. `scene.get`
        // returns `Some` in the steady state (task 7.6 spawns a
        // `MirroredBody` for every prop present at startup), so the
        // `None` branch below is the Requirement-9.2 fallback.
        //
        // Only the prop's `body` crosses into the render world: velocity and
        // mass are simulation state with no visual representation, so this
        // layer stays unaware of them.
        if let Some(prop) = scene.get(*id) {
            *transform = transform_from(&prop.body);
        }
        // Requirement 9.2: when `scene.get(id) == None`, leave the
        // entity's `Transform` unchanged. No code needed — we simply
        // fall through.
    }
}

#[cfg(test)]
#[path = "../../tst/render/bridge_tests.rs"]
mod tests;
