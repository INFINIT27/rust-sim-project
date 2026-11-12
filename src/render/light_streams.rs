//! Light streams: rays of light falling in on a black hole from every direction, bending around it.
//!
//! Every [`LIGHT_STREAM_EMISSION_INTERVAL_S`] of simulated time a burst of [`LIGHT_STREAM_RAYS`]
//! rays sets off inward from points spread uniformly over a sphere
//! [`LIGHT_STREAM_SOURCE_DISTANCE_RS`] horizon radii round the hole. Each is aimed off the hole's
//! centre by a random impact parameter, spread evenly over a disc of radius
//! [`LIGHT_STREAM_MAX_IMPACT_RS`], in a random direction. The pattern comes from a fixed seed
//! ([`LIGHT_STREAM_SEED`]), so every run is the same. Each ray follows the path light really takes:
//! its velocity starts from [`kerr_schild_light_velocity`] and it is stepped with
//! [`kerr_schild_acceleration`], the same equation the integrator moves bodies with. The hole's
//! spin comes from the scene's gravity model: a spinning hole drags the light round with it, so rays
//! passing on the side moving with the spin escape closer in than rays passing against it. A
//! Schwarzschild scene is the zero-spin case.
//!
//! What to watch for:
//!
//! - **Far rays bend a little**, towards the hole.
//! - **Rays aimed inside `2.6 r_s`** (the critical impact parameter, `3√3/2 r_s`) **are captured**:
//!   they slow as they near the horizon, the distant observer's view of light that never quite
//!   arrives, and are removed when they cross the horizon (which, in the Kerr–Schild time used
//!   here, they do in a finite time).
//! - **Rays just outside it whirl round the photon sphere** before leaving, some heading back the way
//!   they came.
//!
//! Because the captured rays slow and crowd just outside the horizon, the hole shows as a dark
//! sphere wrapped in a thin bright shell of arriving light, with the whirling rays tracing the
//! photon sphere around it.
//!
//! # Drawn where they are
//!
//! Each ray is a streak about one and a half horizon radii long, at its true position in space,
//! fading towards its tail. Streaks are sampled by simulated time, so they are the same length at
//! any frame rate.
//!
//! # Scope
//!
//! Render-only: the rays are not scene props, have no mass, light no surfaces, and do not appear in
//! the overlay. They exist in a scene whose gravity model is relativistic (see [`stream_source`])
//! and that has a black hole, and they are bent by the most massive black hole only. They move in simulated
//! time, so they pause with the clock and speed up with it. [`LightStreams`] is the pure state and
//! is tested; the two systems are thin.

use std::collections::VecDeque;

use bevy::app::{App, Plugin, Update};
use bevy::color::Color as BevyColor;
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::gizmos::config::{GizmoConfigGroup, GizmoConfigStore};
use bevy::gizmos::gizmos::Gizmos;
use bevy::gizmos::AppGizmoBuilder;
use bevy::math::Vec3;
use bevy::reflect::Reflect;

use crate::constants::METRES_PER_UNIT;
use crate::core::integrator::kerr_schild_to_world;
use crate::core::scene::Scene;
use crate::experiments::random::Rng;
use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
use crate::physics_utils::relativity::gravity::schwarzschild_radius;
use crate::physics_utils::relativity::kerr::{
    horizon_radius, kerr_schild_acceleration, kerr_schild_light_velocity, kerr_schild_radius,
    KerrBlackHole,
};
use crate::render::constants::{
    LIGHT_STREAM_COLOR, LIGHT_STREAM_EMISSION_INTERVAL_S, LIGHT_STREAM_ESCAPE_RS,
    LIGHT_STREAM_LINE_WIDTH_PX, LIGHT_STREAM_MAX_AGE_S, LIGHT_STREAM_MAX_IMPACT_RS,
    LIGHT_STREAM_MAX_STEPS_PER_FRAME, LIGHT_STREAM_MAX_STEP_S, LIGHT_STREAM_RAYS,
    LIGHT_STREAM_SAMPLE_INTERVAL_S, LIGHT_STREAM_SEED, LIGHT_STREAM_SOURCE_DISTANCE_RS,
    LIGHT_STREAM_STEP_FRACTION, LIGHT_STREAM_TRAIL_SAMPLES,
};
use crate::time::simulation::SimulationClock;

/// One ray of light, relative to the hole it is bending around.
#[derive(Clone, Debug, PartialEq)]
pub struct Photon {
    /// Position relative to the hole's centre, in metres.
    pub offset_m: [f64; 3],
    /// Coordinate velocity, in metres per second.
    pub velocity_m_s: [f64; 3],
    /// Earlier positions, oldest first, one every [`LIGHT_STREAM_SAMPLE_INTERVAL_S`].
    pub trail_m: VecDeque<[f64; 3]>,
    /// Simulated seconds since the ray was emitted.
    pub age_s: f64,
    /// Simulated seconds since the last trail sample.
    since_sample_s: f64,
}

/// What became of a ray in one step.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Fate {
    Travelling,
    Captured,
    Gone,
}

fn length(v: [f64; 3]) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}

fn add(a: [f64; 3], b: [f64; 3], scale: f64) -> [f64; 3] {
    [
        a[0] + scale * b[0],
        a[1] + scale * b[1],
        a[2] + scale * b[2],
    ]
}

/// One classical Runge–Kutta step of the Schwarzschild equation of motion, or `None` if the ray
/// reached the horizon within it.
fn rk4(hole: &Hole, x: [f64; 3], v: [f64; 3], dt: f64) -> Option<([f64; 3], [f64; 3])> {
    let accel = |x, v| kerr_schild_acceleration(hole.mass_kg, hole.spin, x, v);
    let a1 = accel(x, v)?;
    let (x2, v2) = (add(x, v, 0.5 * dt), add(v, a1, 0.5 * dt));
    let a2 = accel(x2, v2)?;
    let (x3, v3) = (add(x, v2, 0.5 * dt), add(v, a2, 0.5 * dt));
    let a3 = accel(x3, v3)?;
    let (x4, v4) = (add(x, v3, dt), add(v, a3, dt));
    let a4 = accel(x4, v4)?;

    let mut nx = x;
    let mut nv = v;
    for i in 0..3 {
        nx[i] += dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]);
        nv[i] += dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]);
    }
    Some((nx, nv))
}

/// The hole the rays bend round, with the lengths they are measured against.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Hole {
    mass_kg: f64,
    spin: f64,
    /// `r_g = GM/c²`, in metres.
    gravitational_radius_m: f64,
    /// `2GM/c²`, in metres: the length the stream constants are quoted in.
    schwarzschild_radius_m: f64,
    /// The horizon's Boyer–Lindquist radius, in `r_g`.
    horizon_rg: f64,
}

impl Hole {
    fn new(mass_kg: f64, spin: f64) -> Option<Self> {
        let kerr = KerrBlackHole::new(mass_kg, spin)?;
        Some(Hole {
            mass_kg,
            spin,
            gravitational_radius_m: kerr.gravitational_radius_m(),
            schwarzschild_radius_m: schwarzschild_radius(mass_kg)?,
            horizon_rg: horizon_radius(spin)?,
        })
    }

    /// Whether a point (Kerr–Schild axes, metres) is at or inside the horizon, with a hair of
    /// margin so the step that would land on it counts.
    fn swallows(&self, offset_m: [f64; 3]) -> bool {
        let r = kerr_schild_radius(self.spin, offset_m.map(|c| c / self.gravitational_radius_m));
        r <= self.horizon_rg * 1.001
    }
}

impl Photon {
    /// A ray at `offset_m` (Kerr–Schild axes: spin along `+z`) from a hole of `mass_kg` and `spin`,
    /// heading along `direction`, or `None` at or inside the horizon.
    pub fn new(mass_kg: f64, spin: f64, offset_m: [f64; 3], direction: [f64; 3]) -> Option<Self> {
        Some(Photon {
            offset_m,
            velocity_m_s: kerr_schild_light_velocity(mass_kg, spin, offset_m, direction)?,
            trail_m: VecDeque::with_capacity(LIGHT_STREAM_TRAIL_SAMPLES as usize),
            age_s: 0.0,
            since_sample_s: 0.0,
        })
    }

    /// Advance the ray by `dt_s` simulated seconds about `hole`.
    fn advance(&mut self, hole: &Hole, dt_s: f64) -> Fate {
        let mut remaining = dt_s;
        // However large the frame, at most this many steps; past it the steps grow instead.
        let floor_step = dt_s / f64::from(LIGHT_STREAM_MAX_STEPS_PER_FRAME);

        while remaining > 0.0 {
            let r = length(self.offset_m);
            if hole.swallows(self.offset_m) {
                return Fate::Captured;
            }

            // Steps scale with the ray's light-travel time to the hole's centre, r/c: short near
            // the photon orbits, where paths bend hardest, and long far out.
            let natural = LIGHT_STREAM_STEP_FRACTION * r / SPEED_OF_LIGHT_M_S;
            let h = natural
                .min(LIGHT_STREAM_MAX_STEP_S)
                .max(floor_step)
                .min(remaining);

            match rk4(hole, self.offset_m, self.velocity_m_s, h) {
                Some((x, v)) => {
                    self.offset_m = x;
                    self.velocity_m_s = v;
                }
                None => return Fate::Captured,
            }

            remaining -= h;
            self.age_s += h;
            self.since_sample_s += h;

            if self.since_sample_s >= LIGHT_STREAM_SAMPLE_INTERVAL_S {
                self.since_sample_s -= LIGHT_STREAM_SAMPLE_INTERVAL_S;
                self.trail_m.push_back(self.offset_m);
                if self.trail_m.len() > LIGHT_STREAM_TRAIL_SAMPLES as usize {
                    self.trail_m.pop_front();
                }
            }
        }

        let r = length(self.offset_m);
        let outward = self.offset_m[0] * self.velocity_m_s[0]
            + self.offset_m[1] * self.velocity_m_s[1]
            + self.offset_m[2] * self.velocity_m_s[2]
            > 0.0;

        if hole.swallows(self.offset_m) {
            Fate::Captured
        } else if (outward && r > LIGHT_STREAM_ESCAPE_RS * hole.schwarzschild_radius_m)
            || self.age_s > LIGHT_STREAM_MAX_AGE_S
        {
            Fate::Gone
        } else {
            Fate::Travelling
        }
    }

    /// The ray's streak, oldest point first and its current position last, in world coordinates
    /// (`+Y` up, the spin axis) about a hole centred at `centre`.
    pub fn streak(&self, centre: Vec3) -> Vec<Vec3> {
        self.trail_m
            .iter()
            .chain(std::iter::once(&self.offset_m))
            .map(|offset| centre + (kerr_schild_to_world(*offset) / METRES_PER_UNIT).as_vec3())
            .collect()
    }
}

/// Every ray in flight, and the emitter's timing and random state.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LightStreams {
    photons: Vec<Photon>,
    since_emission_s: f64,
    started: bool,
    captured: u64,
    rng: Rng,
}

impl Default for LightStreams {
    /// No rays yet, and the emitter seeded with [`LIGHT_STREAM_SEED`].
    fn default() -> Self {
        LightStreams {
            photons: Vec::new(),
            since_emission_s: 0.0,
            started: false,
            captured: 0,
            rng: Rng::new(LIGHT_STREAM_SEED),
        }
    }
}

/// A uniform random number in `[0, 1)` with full `f64` resolution.
fn unit(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Two unit vectors perpendicular to the unit vector `d` and to each other.
fn perpendicular_basis(d: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    // Cross with whichever axis is least aligned with `d`, so the result is never near zero.
    let helper = if d[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let first = cross(d, helper);
    let length = length(first);
    let first = first.map(|c| c / length);
    (first, cross(d, first))
}

impl LightStreams {
    /// The rays in flight.
    pub fn photons(&self) -> &[Photon] {
        &self.photons
    }

    /// How many rays have been captured since the streams started.
    pub fn captured(&self) -> u64 {
        self.captured
    }

    /// Remove every ray and restart the emitter.
    pub fn clear(&mut self) {
        *self = LightStreams::default();
    }

    /// Fire one burst of [`LIGHT_STREAM_RAYS`] rays in on a hole of `mass_kg` and `spin`.
    ///
    /// Each ray starts from a point drawn uniformly on the sphere of radius
    /// [`LIGHT_STREAM_SOURCE_DISTANCE_RS`], shifted sideways by its impact parameter, and heads
    /// straight in. The impact parameter is spread evenly over a disc of radius
    /// [`LIGHT_STREAM_MAX_IMPACT_RS`] (radius `b_max·√u`), in a uniformly random direction.
    pub fn emit(&mut self, mass_kg: f64, spin: f64) {
        let Some(hole) = Hole::new(mass_kg, spin) else {
            return;
        };

        let source = f64::from(LIGHT_STREAM_SOURCE_DISTANCE_RS) * hole.schwarzschild_radius_m;
        let max_impact = f64::from(LIGHT_STREAM_MAX_IMPACT_RS) * hole.schwarzschild_radius_m;

        for _ in 0..LIGHT_STREAM_RAYS {
            // A uniform direction on the sphere: z uniform in [-1, 1], azimuth uniform.
            let z = 2.0 * unit(&mut self.rng) - 1.0;
            let azimuth = std::f64::consts::TAU * unit(&mut self.rng);
            let ring = (1.0 - z * z).max(0.0).sqrt();
            let outward = [ring * azimuth.cos(), ring * azimuth.sin(), z];

            let impact = max_impact * unit(&mut self.rng).sqrt();
            let turn = std::f64::consts::TAU * unit(&mut self.rng);
            let (first, second) = perpendicular_basis(outward);

            let mut start = [0.0; 3];
            for axis in 0..3 {
                start[axis] = source * outward[axis]
                    + impact * (turn.cos() * first[axis] + turn.sin() * second[axis]);
            }
            let inward = outward.map(|c| -c);

            if let Some(photon) = Photon::new(mass_kg, spin, start, inward) {
                self.photons.push(photon);
            }
        }
    }

    /// Advance every ray by `dt_s` simulated seconds about a hole of `mass_kg` and `spin`, emitting new bursts
    /// as they fall due and removing rays that have been captured or have left.
    ///
    /// The first call emits a burst at once, so the streams start with the experiment.
    pub fn advance(&mut self, mass_kg: f64, spin: f64, dt_s: f64) {
        let Some(hole) = Hole::new(mass_kg, spin) else {
            return;
        };
        if !dt_s.is_finite() || dt_s < 0.0 {
            return;
        }

        if !self.started {
            self.started = true;
            self.emit(mass_kg, spin);
        }

        let mut captured = 0;
        self.photons
            .retain_mut(|photon| match photon.advance(&hole, dt_s) {
                Fate::Travelling => true,
                Fate::Captured => {
                    captured += 1;
                    false
                }
                Fate::Gone => false,
            });
        self.captured += captured;

        self.since_emission_s += dt_s;
        while self.since_emission_s >= LIGHT_STREAM_EMISSION_INTERVAL_S {
            self.since_emission_s -= LIGHT_STREAM_EMISSION_INTERVAL_S;
            self.emit(mass_kg, spin);
        }
    }
}

/// The black hole the streams bend around: the most massive prop that absorbs, in a scene whose
/// gravity is relativistic (`GravityModel::Schwarzschild`, as spin zero, or `GravityModel::Kerr`).
/// Its mass in kilograms, its spin, and its centre.
pub fn stream_source(scene: &Scene) -> Option<(f64, f64, Vec3)> {
    let spin = scene.gravity_model.black_hole_spin()?;

    scene
        .props
        .iter()
        .filter(|prop| prop.absorbs && prop.mass_kg > 0.0)
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
        .map(|prop| (f64::from(prop.mass_kg), spin, prop.body.center().as_vec3()))
}

/// Bevy system that advances the streams by this frame's simulated step.
///
/// Runs after the scene is stepped. Clears the streams in a scene with nothing to bend them.
pub fn advance_light_streams_system(
    scene: Option<Res<Scene>>,
    clock: Option<Res<SimulationClock>>,
    mut streams: ResMut<LightStreams>,
) {
    let (Some(scene), Some(clock)) = (scene, clock) else {
        return;
    };

    match stream_source(&scene) {
        Some((mass_kg, spin, _)) => streams.advance(mass_kg, spin, clock.last_step().seconds()),
        None => {
            if !streams.photons().is_empty() {
                streams.clear();
            }
        }
    }
}

/// The gizmo group the streams are drawn in, so their width is their own setting.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct LightStreamGizmos;

/// Bevy system that draws each ray as a streak fading towards its tail.
///
/// Not headless-testable: `Gizmos` needs the render pipeline's config store. The geometry is
/// [`Photon::streak`], which is.
pub fn draw_light_streams_system(
    scene: Res<Scene>,
    streams: Res<LightStreams>,
    mut gizmos: Gizmos<LightStreamGizmos>,
) {
    let Some((_, _, centre)) = stream_source(&scene) else {
        return;
    };
    let colour = LIGHT_STREAM_COLOR;

    for photon in streams.photons() {
        let points = photon.streak(centre);
        if points.len() < 2 {
            continue;
        }

        let last = (points.len() - 1) as f32;
        gizmos.linestrip_gradient(points.into_iter().enumerate().map(|(index, point)| {
            let alpha = colour.a * index as f32 / last;
            (
                point,
                BevyColor::linear_rgba(colour.r, colour.g, colour.b, alpha),
            )
        }));
    }
}

/// Emits and draws the light streams around the black hole of a relativistic scene.
///
/// Add it after [`ScenePlugin`](super::plugin::ScenePlugin). The streams advance headless too; only
/// drawing needs a render pipeline.
pub struct LightStreamsPlugin;

impl Plugin for LightStreamsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightStreams>();
        app.add_systems(
            Update,
            advance_light_streams_system.after(super::plugin::step_scene_system),
        );

        if app.world().get_resource::<GizmoConfigStore>().is_some() {
            app.init_gizmo_group::<LightStreamGizmos>();
            {
                let mut store = app.world_mut().resource_mut::<GizmoConfigStore>();
                store.config_mut::<LightStreamGizmos>().0.line_width = LIGHT_STREAM_LINE_WIDTH_PX;
            }
            app.add_systems(
                Update,
                draw_light_streams_system.after(advance_light_streams_system),
            );
        }
    }
}

#[cfg(test)]
#[path = "../../tst/render/light_streams_tests.rs"]
mod tests;
