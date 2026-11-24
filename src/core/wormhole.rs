//! Bodies in a wormhole: where they are, how they coast through it, and how they meet.
//!
//! A scene with a [`WormholeSpace`] ([`Scene::wormhole`]) is stepped by [`Scene::step`] as bodies
//! coasting in curved space, instead of under gravity. The geometry and the paths are in
//! [`crate::physics_utils::relativity::wormhole`].
//!
//! # What a place is
//!
//! An ordinary position cannot say which side a body is on, nor where in the tunnel, every point of
//! which is at the same radius. So each body has a [`WormholePlace`]: `ℓ` and a **label**, the unit
//! vector that says which way round the centre, with the same labels running from one side through
//! the throat to the other. A direction at a place is given in the **label frame**: along the label
//! is `+ℓ`, across it is round the centre. A body's `position` and `velocity` follow from its place,
//! in the world axes of the side it is on ([`WormholePlace::frame`]).
//!
//! # How bodies move
//!
//! The metric has `g_tt = −1`: no gravity from the wormhole. **Nothing in the scene pulls on
//! anything** either, across the throat or on one side of it: that is the simulation's choice, for
//! now. So a body at rest stays, and a moving one follows the straightest path ([`travel`]) at a
//! constant speed; its energy and its angular momentum about the centre are kept, and it crosses if
//! its impact parameter is below the throat's radius.
//!
//! # How they meet
//!
//! Two spheres touch when the proper distance between their places is less than their radii
//! together, and bounce along the line between them, at the lower restitution of the two. The impulse
//! acts along the straightest path between the centres, so momentum passed from one to the other, and
//! angular momentum about the wormhole's centre, balance. The distance is worked out as for bodies
//! close together compared with the throat, which touching ones are unless they are its size.

use bevy::math::{DMat3, DQuat, DVec3};

use crate::core::constants::{
    WORMHOLE_MAX_SUBSTEPS, WORMHOLE_SUBSTEP_RADII, WORMHOLE_TRAVEL_STEP_FRACTION,
};
use crate::core::scene::{Body, BodyId, Prop, Scene};
use crate::physics_utils::relativity::wormhole::{WormholePath, WormholeShape};
use crate::time::simulation::TimeStep;

/// A place in the wormhole: how far along it, and which way round the centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WormholePlace {
    /// `ℓ`, in world units: positive on the near side.
    pub l: f64,
    /// The label, a unit vector.
    pub n: DVec3,
}

/// The far side's axes at label `n`: the rotation from the label frame to them, in `f64`. The same
/// as [`crate::render::wormhole_ray::far_frame`].
pub fn far_rotation(n: DVec3) -> DQuat {
    let turn = |v: DVec3| {
        let turned = v - 2.0 * v.dot(n) * n;
        DVec3::new(turned.x, turned.y, -turned.z)
    };
    DQuat::from_mat3(&DMat3::from_cols(
        turn(DVec3::X),
        turn(DVec3::Y),
        turn(DVec3::Z),
    ))
    .normalize()
}

impl WormholePlace {
    /// The rotation from the label frame here to the world axes of the side this is on.
    pub fn frame(&self) -> DQuat {
        if self.l < 0.0 {
            far_rotation(self.n)
        } else {
            DQuat::IDENTITY
        }
    }

    /// Where this is drawn, relative to the mouth's centre, in the world axes of its side: at the
    /// radius `r(ℓ)`.
    pub fn position(&self, shape: &WormholeShape) -> DVec3 {
        let mirror = if self.l < 0.0 {
            DVec3::new(self.n.x, self.n.y, -self.n.z)
        } else {
            self.n
        };
        shape.radius(self.l) * mirror
    }

    /// The place drawn at `position` on the far or the near side. Inside the throat's radius it is
    /// at the mouth, and at the centre itself, straight along `z`.
    pub fn at(shape: &WormholeShape, position: DVec3, far: bool) -> Self {
        let out = position.try_normalize().unwrap_or(DVec3::Z);
        let l = shape.distance_at_radius(position.length());
        if far {
            Self {
                // Never zero, which is the near side.
                l: -l.max(f64::MIN_POSITIVE),
                n: DVec3::new(out.x, out.y, -out.z),
            }
        } else {
            Self { l, n: out }
        }
    }

    /// The same place in a wormhole that has changed from `old` to `new`: as far outside its mouth,
    /// or as far along its tunnel, as a fraction of the tunnel's length.
    pub fn rescaled(&self, old: &WormholeShape, new: &WormholeShape) -> Self {
        let outside = self.l.abs() - old.half_length();
        let l = if outside >= 0.0 {
            new.half_length() + outside * new.throat() / old.throat()
        } else {
            self.l.abs() / old.half_length() * new.half_length()
        };
        Self {
            l: l.copysign(self.l),
            n: self.n,
        }
    }
}

/// Follow the straightest path from `place` along `direction` (label frame, unit) for `distance`.
///
/// # Returns
///
/// The place it ends at, and the rotation, in the label frame's axes, that carries a direction at
/// the start to the same direction, not turned, at the end.
pub fn travel(
    shape: &WormholeShape,
    place: WormholePlace,
    direction: DVec3,
    distance: f64,
) -> (WormholePlace, DQuat) {
    let p = direction.dot(place.n);
    let across = direction - p * place.n;
    let sine = across.length();
    let (t, heading) = if sine > 1.0e-12 {
        (across / sine, sine.atan2(p))
    } else {
        (
            place.n.any_orthonormal_vector(),
            if p < 0.0 { std::f64::consts::PI } else { 0.0 },
        )
    };
    let (mut start, impact) = WormholePath::start(shape, place.l, heading);
    let impact = if sine > 1.0e-12 { impact } else { 0.0 };
    start.p = p;
    let end = start.advance(shape, impact, distance, WORMHOLE_TRAVEL_STEP_FRACTION);
    let n = (end.angle.cos() * place.n + end.angle.sin() * t).normalize();
    // The path's own direction is carried along by it, and everything else keeps its angle to it
    // within the plane: so the carry is the turn of that direction, about the plane's normal.
    let turn = end.direction(shape, impact) - heading;
    let carried = DQuat::from_axis_angle(place.n.cross(t).normalize(), turn);
    (WormholePlace { l: end.l, n }, carried)
}

/// The wormhole a scene is in: its shape, and where each body is.
#[derive(Clone, Debug, PartialEq)]
pub struct WormholeSpace {
    /// Its shape, in world units.
    pub shape: WormholeShape,
    /// Where each body is, for those that have been given a place; in the scene's order.
    places: Vec<(BodyId, WormholePlace)>,
}

impl WormholeSpace {
    /// An empty wormhole of `shape`.
    pub fn new(shape: WormholeShape) -> Self {
        Self {
            shape,
            places: Vec::new(),
        }
    }

    /// Where body `id` is, or `None` when it has not been given a place.
    pub fn place(&self, id: BodyId) -> Option<WormholePlace> {
        let found = self.places.iter().find(|(placed, _)| *placed == id);
        found.map(|(_, place)| *place)
    }

    /// Give every body a place and forget those that are gone: a body added with [`Scene::add`] is
    /// put where its position says, on the near side.
    fn settle(&mut self, scene: &Scene) {
        let up_to_date = self.places.len() == scene.props.len()
            && self
                .places
                .iter()
                .zip(scene.iter())
                .all(|(a, b)| a.0 == b.0);
        if up_to_date {
            return;
        }
        self.places = scene
            .iter()
            .map(|(id, prop)| {
                let place = self
                    .place(id)
                    .unwrap_or_else(|| WormholePlace::at(&self.shape, prop.body.center(), false));
                (id, place)
            })
            .collect();
    }

    /// How many substeps a step of `seconds` needs.
    pub(crate) fn substeps(&self, props: &[Prop], seconds: f64) -> u32 {
        let fastest = props.iter().map(Prop::speed).fold(0.0, f64::max);
        let smallest = props.iter().map(radius).fold(f64::INFINITY, f64::min);
        let wanted = fastest * seconds / (WORMHOLE_SUBSTEP_RADII * smallest);
        if wanted.is_finite() {
            (wanted.ceil() as u32).clamp(1, WORMHOLE_MAX_SUBSTEPS)
        } else {
            1
        }
    }

    /// One substep of `scene`: every body coasts, then those that touch bounce.
    pub(crate) fn coast(&mut self, scene: &mut Scene, substep: TimeStep) {
        self.settle(scene);
        let shape = self.shape;
        for (prop, (_, place)) in scene.props.iter_mut().zip(&mut self.places) {
            let speed = prop.velocity_units_s.length();
            if prop.immovable || speed == 0.0 {
                continue;
            }
            let frame = place.frame();
            let direction = frame.inverse() * (prop.velocity_units_s / speed);
            let (after, carried) = travel(&shape, *place, direction, speed * substep.seconds());
            // The speed is kept exactly; only the direction turns.
            let heading = after.frame() * (carried * direction);
            prop.velocity_units_s = heading.normalize() * speed;
            *place = after;
            put(prop, place, &shape);
        }
        for i in 0..scene.props.len() {
            for j in i + 1..scene.props.len() {
                let (head, tail) = scene.props.split_at_mut(j);
                let (a, b) = self.places.split_at_mut(j);
                bounce(
                    &shape,
                    (&mut head[i], &mut a[i].1),
                    (&mut tail[0], &mut b[0].1),
                );
            }
        }
    }
}

/// The radius of a body, `0` for one that is not a sphere.
fn radius(prop: &Prop) -> f64 {
    match prop.body {
        Body::Sphere { radius, .. } => f64::from(radius),
        _ => 0.0,
    }
}

/// Put `prop` where `place` is drawn.
fn put(prop: &mut Prop, place: &WormholePlace, shape: &WormholeShape) {
    let moved = place.position(shape) - prop.body.center();
    prop.body.translate(moved);
}

/// The direction at `a` (label frame, unit) to `b` and the proper distance between them, for places
/// close together compared with the throat. `None` when they are the same place.
pub fn separation(
    shape: &WormholeShape,
    a: &WormholePlace,
    b: &WormholePlace,
) -> Option<(DVec3, f64)> {
    let across = b.n - b.n.dot(a.n) * a.n;
    let angle = across.length().atan2(b.n.dot(a.n));
    let round = shape.radius(0.5 * (a.l + b.l)) * angle;
    let along = b.l - a.l;
    let distance = along.hypot(round);
    let t = across.try_normalize().unwrap_or(DVec3::ZERO);
    (distance > 0.0).then(|| ((along * a.n + round * t) / distance, distance))
}

/// Bounce two bodies off one another if they touch and are closing, and part them.
fn bounce(
    shape: &WormholeShape,
    (a, at): (&mut Prop, &mut WormholePlace),
    (b, bt): (&mut Prop, &mut WormholePlace),
) {
    let reach = radius(a) + radius(b);
    // No closer than the difference of their ℓ, so this is a cheap miss.
    if (at.l - bt.l).abs() > reach {
        return;
    }
    let Some((direction, distance)) = separation(shape, at, bt) else {
        return;
    };
    if distance >= reach {
        return;
    }
    let inverse = |prop: &Prop| {
        if prop.immovable || prop.mass_kg <= 0.0 {
            0.0
        } else {
            1.0 / f64::from(prop.mass_kg)
        }
    };
    let (wa, wb) = (inverse(a), inverse(b));
    if wa + wb == 0.0 {
        return;
    }
    // The line between the centres, at each end.
    let (_, carried) = travel(shape, *at, direction, distance);
    let line_a = at.frame() * direction;
    let line_b = bt.frame() * (carried * direction);
    let closing = a.velocity_units_s.dot(line_a) - b.velocity_units_s.dot(line_b);
    if closing > 0.0 {
        let restitution = f64::from(a.material.combined_with(&b.material).restitution);
        let impulse = (1.0 + restitution) * closing / (wa + wb);
        a.velocity_units_s -= impulse * wa * line_a;
        b.velocity_units_s += impulse * wb * line_b;
    }
    let overlap = reach - distance;
    for (prop, place, way, share) in [
        (a, at, -direction, wa / (wa + wb)),
        (b, bt, carried * direction, wb / (wa + wb)),
    ] {
        if share == 0.0 {
            continue;
        }
        let before = *place;
        let (after, turned) = travel(shape, before, way, overlap * share);
        let turn = after.frame() * turned * before.frame().inverse();
        prop.velocity_units_s = turn * prop.velocity_units_s;
        *place = after;
        put(prop, place, shape);
    }
}

impl Scene {
    /// Add `prop` at `place` in the wormhole of this scene, its position following: the velocity
    /// given is in the world axes of the side `place` is on. Without a wormhole, the same as
    /// [`add`](Self::add).
    pub fn add_at(&mut self, mut prop: Prop, place: WormholePlace) -> BodyId {
        if let Some(space) = &self.wormhole {
            put(&mut prop, &place, &space.shape);
        }
        let id = self.add(prop);
        if let Some(space) = &mut self.wormhole {
            space.places.push((id, place));
        }
        id
    }

    /// Change the shape of the wormhole of this scene, keeping every body where it is in it and as
    /// large, in throat radii. Speeds stay as they are, so none passes that of light. Does nothing
    /// without a wormhole, or if the shape is the same.
    pub fn reshape_wormhole(&mut self, shape: WormholeShape) {
        let Some(mut space) = self.wormhole.take() else {
            return;
        };
        if space.shape != shape {
            space.settle(self);
            let old = space.shape;
            let ratio = shape.throat() / old.throat();
            for (prop, (_, place)) in self.props.iter_mut().zip(&mut space.places) {
                *place = place.rescaled(&old, &shape);
                put(prop, place, &shape);
                if let Body::Sphere { radius, .. } = &mut prop.body {
                    *radius = (f64::from(*radius) * ratio) as f32;
                }
            }
            space.shape = shape;
        }
        self.wormhole = Some(space);
    }
}

#[cfg(test)]
#[path = "../../tst/core/wormhole_tests.rs"]
mod tests;
