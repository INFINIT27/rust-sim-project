//! The catalog: which experiments exist, and how each one is built.
//!
//! An [`ExperimentId`] names an experiment, [`ExperimentId::build`] turns it into an
//! [`Experiment`] — a [`Scene`] plus the [`ExperimentSpec`] that says how to run and
//! frame it — and [`ExperimentId::ALL`] is the list, which is what a `--list` flag and
//! a future GUI picker both read.
//!
//! # Adding an experiment
//!
//! Four edits, and the compiler finds three of them for you: a variant on
//! [`ExperimentId`], an entry in [`ExperimentId::ALL`], arms in `slug`/`name` and
//! `build`, and a builder function. The `match` in `build` is exhaustive, so a
//! forgotten builder is a compile error rather than a missing menu entry.
//!
//! A builder small enough to read at a glance lives here ([`two_body`],
//! [`three_body_ring`]); one with a data table of its own gets a module, as
//! [`super::solar_system`] does.
//!
//! # What a spec carries, and why the plugin reads it rather than a constant
//!
//! Time scale and camera pose are **per experiment**: a three-body ring `12` units
//! across wants a different clock and a different viewpoint than a two-body pair `30`
//! units apart. Before this layer both were constants that `ScenePlugin` and
//! `spawn_scene_entities` read directly, which made "run a different experiment" a
//! code edit in the render layer. Now the plugin reads the spec, and the spec comes
//! from the catalog.
//!
//! # Where each scene is actually built
//!
//! Asymmetric, deliberately: the two-body scene is [`Scene::two_body_gravity`] in
//! `core`, because it predates this layer, its constants are already shared with
//! `render`, and a body of tests and documentation names it. This layer *wraps* it
//! with a spec. The three-body ring is built here, because it needs randomness — and
//! `core` cannot reach [`super::random`] without a circular dependency, since this
//! layer depends on `core`.
//!
//! The rule going forward: **a new experiment's scene is built in this layer.** Only
//! the two-body one lives in `core`, and only for that history. The solar system is built
//! in [`super::solar_system`] — the same layer, its own file, because eight rows of real
//! astronomical data and the arithmetic that turns them into a scene do not belong in the
//! middle of the catalog.

pub use bevy::math::{Quat, Vec3};

use crate::constants::{TWO_BODY_HEIGHT_UNITS, TWO_BODY_TIME_SCALE};
use crate::core::scene::{Body, BodyId, Color, Pose, Prop, Scene};
use crate::experiments::constants::{
    DEFAULT_RANDOM_SEED, THREE_BODY_CAMERA_POSITION_UNITS, THREE_BODY_FRAMED_RADIUS_UNITS,
    THREE_BODY_HEIGHT_UNITS, THREE_BODY_MASS_KG, THREE_BODY_MAX_SPEED_FACTOR,
    THREE_BODY_MIN_SPEED_FACTOR, THREE_BODY_RING_RADIUS_UNITS, THREE_BODY_TIME_SCALE,
    THREE_BODY_VISUAL_RADIUS_UNITS, TWO_BODY_CAMERA_POSITION_UNITS, TWO_BODY_FRAMED_RADIUS_UNITS,
};
use crate::experiments::random::Rng;
use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

/// Which experiment to run.
///
/// The slugs are `experiment_1` and `experiment_2`, with descriptive aliases, so a
/// command line can say either what it wants or which one it wants — see
/// [`super::selection`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ExperimentId {
    /// Two Earth-mass spheres released from rest, falling together and bouncing.
    TwoBody,
    /// Three Earth-mass bodies on a circle, each with a random tangential speed.
    ThreeBodyRing,
    /// The Sun and the eight planets, on real orbits at real relative sizes.
    SolarSystem,
    /// Jupiter and the four Galilean moons — the first scene whose physics is exactly real.
    Jovian,
    /// The Sun and the eight planets in a static row, for checking the scale by eye.
    ScaleCheck,
}

impl Default for ExperimentId {
    /// [`ExperimentId::TwoBody`] — `experiment_1`, what the app has always shown.
    #[inline]
    fn default() -> Self {
        Self::TwoBody
    }
}

impl ExperimentId {
    /// Every experiment, in catalog order.
    ///
    /// The order is the order a `--list` prints and a picker would show, so it is
    /// stable and matches the slugs: `experiment_1` first.
    pub const ALL: [Self; 5] = [
        Self::TwoBody,
        Self::ThreeBodyRing,
        Self::SolarSystem,
        Self::Jovian,
        Self::ScaleCheck,
    ];

    /// The canonical slug: `"experiment_1"` through `"experiment_5"`.
    ///
    /// Numbered rather than named because that is how they are referred to in
    /// conversation and on the command line. [`name`](Self::name) is the human label
    /// and [`aliases`](Self::aliases) covers the descriptive forms.
    #[inline]
    pub fn slug(&self) -> &'static str {
        match self {
            Self::TwoBody => "experiment_1",
            Self::ThreeBodyRing => "experiment_2",
            Self::SolarSystem => "experiment_3",
            Self::Jovian => "experiment_4",
            Self::ScaleCheck => "experiment_5",
        }
    }

    /// Short human label, for the overlay and `--list`.
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::TwoBody => "Two-body free fall",
            Self::ThreeBodyRing => "Three-body ring",
            Self::SolarSystem => "Solar system",
            Self::Jovian => "Jovian system",
            Self::ScaleCheck => "Scale check",
        }
    }

    /// One line on what the experiment does and what to watch for.
    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            Self::TwoBody => {
                "Two Earth masses 30 units apart, released from rest: they fall \
                 together, bounce elastically, and oscillate."
            }
            Self::ThreeBodyRing => {
                "Three Earth masses on a 12-unit circle, each given a random \
                 tangential speed: chaotic, and different every seed."
            }
            Self::SolarSystem => {
                "The Sun, eight planets, and five moons at true scale, with \
                 nothing compensating for it — it opens on a 4.6-pixel Sun and its trails."
            }
            Self::Jovian => {
                "Jupiter and the four Galilean moons alone, with the 1:2:4 Laplace \
                 resonance. Nothing in it is scaled."
            }
            Self::ScaleCheck => {
                "The Sun and eight planets in a static row at true size, bottoms aligned \
                 — a ruler for checking the scale by eye. Nothing moves."
            }
        }
    }

    /// Accepted alternatives to [`slug`](Self::slug) on a command line.
    ///
    /// Both a bare number and a descriptive name, because `experiment_2` is precise
    /// and `three-body` is memorable, and there is no reason to make someone choose.
    #[inline]
    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            Self::TwoBody => &["1", "two-body", "two_body"],
            Self::ThreeBodyRing => &["2", "three-body", "three_body", "ring"],
            Self::SolarSystem => &["3", "solar-system", "solar_system", "solar", "planets"],
            Self::Jovian => &["4", "jovian", "jupiter", "moons", "galilean"],
            Self::ScaleCheck => &["5", "scale", "scale-check", "sizes", "ruler"],
        }
    }

    /// Does `token` name this experiment? Case-insensitive.
    pub fn matches(&self, token: &str) -> bool {
        let token = token.trim().to_ascii_lowercase();
        token == self.slug() || self.aliases().iter().any(|alias| *alias == token)
    }

    /// Build the experiment: its scene, and the spec that says how to run it.
    ///
    /// `seed` is used only by experiments with randomised initial conditions; a
    /// deterministic one ignores it, which is why it is taken unconditionally rather
    /// than as an `Option` the caller has to reason about. The **same seed always
    /// produces the same scene**, on every platform — see [`super::random`].
    pub fn build(&self, seed: u64) -> Experiment {
        match self {
            Self::TwoBody => two_body(),
            Self::ThreeBodyRing => three_body_ring(seed),
            Self::SolarSystem => super::solar_system::solar_system(),
            Self::Jovian => super::jovian::jovian_system(),
            Self::ScaleCheck => super::scale_check::scale_check(),
        }
    }

    /// Build with [`DEFAULT_RANDOM_SEED`], for a caller that does not care.
    pub fn build_default(&self) -> Experiment {
        self.build(DEFAULT_RANDOM_SEED)
    }
}

/// How to run and frame an experiment: everything about it that is not the scene.
///
/// Held as an ECS resource by `ScenePlugin`, so the startup camera, the overlay, and
/// anything else that needs to know *which* experiment is running can read it rather
/// than assume.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ExperimentSpec {
    /// Which experiment this is.
    pub id: ExperimentId,
    /// The seed the scene was built with. Recorded so a run can be reproduced: the
    /// overlay shows it, and `--seed <value>` replays it exactly.
    pub seed: u64,
    /// Simulated seconds per real second this experiment wants, for the
    /// [`SimulationClock`](crate::time::simulation::SimulationClock).
    ///
    /// Per experiment because the timescale that makes one watchable makes another a
    /// blur: the two-body infall takes 6.5 simulated years, the three-body ring about
    /// 17 per notional orbit.
    pub time_scale: f32,
    /// Where the camera starts, in world units.
    pub camera_position_units: Vec3,
    /// What the camera looks at, in world units.
    pub camera_target_units: Vec3,
    /// Radius about [`camera_target_units`](Self::camera_target_units) that the opening
    /// view is meant to contain, in world units.
    ///
    /// A *declaration*, not a derivation, and that is the point: for the first two
    /// experiments it is the whole scene, but the solar system opens on the inner four
    /// planets and deliberately leaves the giants off screen, because a view that
    /// contained Neptune's `4495`-unit orbit would show nothing at all. Without this
    /// field, "the camera frames its scene" could only be checked against the scene's
    /// full extent, which would either fail for the solar system or have to be dropped
    /// for every experiment.
    ///
    /// `catalog_tests::every_experiment_frames_its_own_scene` checks the camera really is
    /// far enough back for this radius, and that at least one body starts inside it.
    pub framed_radius_units: f32,
    /// How long this experiment's motion trails should be, in world units, or `None` to
    /// use the render layer's default.
    ///
    /// Per experiment because a trail length is only meaningful against the size of the
    /// path: [`TRAIL_LENGTH_UNITS`](crate::render::constants::TRAIL_LENGTH_UNITS) (`10`)
    /// is a third of the two-body scene and a hundredth of one Earth orbit. `None` — the
    /// answer for every experiment that fits the default — keeps the render layer's
    /// constant as the single source of it, rather than copying `10.0` into this layer
    /// where it would have to be kept in step.
    ///
    /// Read once at startup by
    /// [`apply_experiment_trail_length`](crate::render::trail::apply_experiment_trail_length),
    /// which also widens the sample spacing to match.
    pub trail_length_units: Option<f32>,
    /// Whether `R` launches a spacecraft in this experiment. Which spacecraft is the user's choice
    /// (the [`Fleet`](crate::spacecraft::Fleet) `G` cycles), not the experiment's; an experiment only
    /// says whether flying makes sense in it. The size-comparison ruler says no, since nothing in it
    /// is meant to move.
    pub launches: bool,
}

/// Display names for the bodies of a scene, by id. A body with no entry is shown by its id.
///
/// Kept beside the scene rather than on `Prop`, because a name is for people: physics never reads
/// it, and every prop literal in the crate would otherwise have to carry one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyNames(pub Vec<(BodyId, &'static str)>);

impl BodyNames {
    /// The body's name, or `#<id>` when it has none.
    pub fn label(&self, id: BodyId) -> String {
        self.0
            .iter()
            .find(|(named, _)| *named == id)
            .map_or_else(|| format!("#{}", id.0), |(_, name)| (*name).to_string())
    }

    /// Name a body, replacing any earlier name for it.
    pub fn insert(&mut self, id: BodyId, name: &'static str) {
        self.0.retain(|(named, _)| *named != id);
        self.0.push((id, name));
    }
}

impl ExperimentSpec {
    /// Validation_Rules for `ExperimentSpec`: a finite positive time scale, finite
    /// camera positions that are not the same point, a finite positive framed radius, and
    /// a trail length that is either absent or finite and positive.
    ///
    /// A camera at its own target has no view direction, which would make
    /// `Transform::looking_at` produce a degenerate rotation — the kind of thing that
    /// renders as a black window and takes an hour to find. A zero
    /// [`framed_radius_units`](Self::framed_radius_units) would make the framing check
    /// vacuous, and a zero trail length would leave nothing to draw.
    pub fn is_valid(&self) -> bool {
        let trail_length_ok = match self.trail_length_units {
            Some(length) => length.is_finite() && length > 0.0,
            None => true,
        };

        self.time_scale.is_finite()
            && self.time_scale > 0.0
            && self.camera_position_units.is_finite()
            && self.camera_target_units.is_finite()
            && self.camera_position_units != self.camera_target_units
            && self.framed_radius_units.is_finite()
            && self.framed_radius_units > 0.0
            && trail_length_ok
    }
}

/// A built experiment: the scene, and how to run it.
#[derive(Clone, Debug, PartialEq)]
pub struct Experiment {
    /// The scene to simulate.
    pub scene: Scene,
    /// How to run and frame it.
    pub spec: ExperimentSpec,
    /// What to call its bodies.
    pub names: BodyNames,
}

/// `experiment_1` — the two-body free fall.
///
/// Wraps [`Scene::two_body_gravity`] with its spec. The scene itself is built in
/// `core`; see the module docs for why that one is the exception.
pub fn two_body() -> Experiment {
    Experiment {
        scene: Scene::two_body_gravity(),
        spec: ExperimentSpec {
            id: ExperimentId::TwoBody,
            // Deterministic: the pair is released from rest, so there is nothing to
            // randomise. The seed is recorded as the default so the overlay has
            // something honest to print rather than a number that did nothing.
            seed: DEFAULT_RANDOM_SEED,
            time_scale: TWO_BODY_TIME_SCALE,
            camera_position_units: TWO_BODY_CAMERA_POSITION_UNITS,
            camera_target_units: Vec3::new(0.0, TWO_BODY_HEIGHT_UNITS, 0.0),
            framed_radius_units: TWO_BODY_FRAMED_RADIUS_UNITS,
            // The default `10`-unit trail is a third of this scene's width — already the
            // right length, so this experiment has nothing to say about it.
            trail_length_units: None,
            launches: true,
        },
        names: BodyNames::default(),
    }
}

/// `experiment_2` — three equal masses on a circle, with random tangential speeds.
///
/// # The setup
///
/// Three bodies at 120° intervals on a circle of radius
/// [`THREE_BODY_RING_RADIUS_UNITS`], in a **horizontal** plane
/// [`THREE_BODY_HEIGHT_UNITS`] above the floor. Each is given a velocity that is
/// **exactly tangential** to the circle — perpendicular to its own radius, in the
/// plane of the ring — with a magnitude drawn uniformly from
/// `[THREE_BODY_MIN_SPEED_FACTOR, THREE_BODY_MAX_SPEED_FACTOR]` times the speed that
/// would hold a *rigid* ring in place. Then they are released.
///
/// # Why tangential, and why that reference speed
///
/// A tangential velocity is the one that carries angular momentum and no radial
/// component, so the initial condition is "orbiting, at some speed" rather than
/// "thrown somewhere". That makes the speed factor the whole experiment: below `1.0`
/// gravity wins and the ring collapses inward; near `1.0` it nearly holds as a
/// rotating triangle — the Lagrange equilateral solution; above `1.0` the bodies swing
/// wide. Because each body's factor is drawn *independently*, the symmetry is broken
/// from the start, which is what makes the result a three-body problem rather than a
/// rotating rigid body.
///
/// The reference speed is derived, not guessed. For three equal masses `m` at 120° on
/// a circle of radius `R`, the separation between any two is `R√3`, and the net inward
/// pull on each is
///
/// ```text
/// F = 2 · (G·m²/(R√3)²) · cos(30°) = G·m²/(√3·R²)
/// ```
///
/// Setting that equal to `m·v²/R` gives `v = √(G·m/(√3·R))` — about `138 m/s` at the
/// shipped radius and mass, or `1.4e-7` units per second.
///
/// # What to expect
///
/// Chaos, and a different outcome per seed: collapse to a close encounter, a lopsided
/// orbit that survives several turns, or one body flung out while the other two pair
/// up. The bodies are **solid** — they bounce rather than pass through — so a collapse
/// ends in contact rather than in a singularity. Two runs with the same seed are
/// identical; two runs with the same seed at different time scales are **not**, because
/// a chaotic system amplifies the integrator's error.
pub fn three_body_ring(seed: u64) -> Experiment {
    // Start from the shared container so the floor and bounds match every other
    // experiment.
    let mut scene = Scene::demo();
    let mut rng = Rng::new(seed);

    let radius = THREE_BODY_RING_RADIUS_UNITS;
    let reference_speed = ring_circular_speed_units_s(radius, THREE_BODY_MASS_KG);

    for index in 0..3u32 {
        // 120° apart, measured in the horizontal XZ plane so the ring lies flat and
        // the camera above it sees a circle rather than a line.
        let angle = index as f32 * std::f32::consts::TAU / 3.0;
        let radial = Vec3::new(angle.cos(), 0.0, angle.sin());
        let position = radial * radius + Vec3::new(0.0, THREE_BODY_HEIGHT_UNITS, 0.0);

        // Tangential direction: perpendicular to the radius *and* to the ring's
        // normal, which is what `cross` gives. All three take the same sense of
        // rotation — mixing senses would be a different experiment (a head-on
        // three-way), and a far shorter one.
        let tangent = Vec3::Y.cross(radial).normalize();

        let factor = rng.next_f32_in(THREE_BODY_MIN_SPEED_FACTOR, THREE_BODY_MAX_SPEED_FACTOR);

        scene.add(Prop::new(
            Body::Sphere {
                pose: Pose {
                    position: position.as_dvec3(),
                    rotation: Quat::IDENTITY,
                },
                radius: THREE_BODY_VISUAL_RADIUS_UNITS,
                color: ring_body_color(index),
            },
            (tangent * (reference_speed * factor)).as_dvec3(),
            THREE_BODY_MASS_KG,
        ));
    }

    Experiment {
        scene,
        spec: ExperimentSpec {
            id: ExperimentId::ThreeBodyRing,
            seed,
            time_scale: THREE_BODY_TIME_SCALE,
            camera_position_units: THREE_BODY_CAMERA_POSITION_UNITS,
            camera_target_units: Vec3::new(0.0, THREE_BODY_HEIGHT_UNITS, 0.0),
            framed_radius_units: THREE_BODY_FRAMED_RADIUS_UNITS,
            trail_length_units: None,
            launches: true,
        },
        names: BodyNames::default(),
    }
}

/// Speed, in world **units per second**, that would hold three equal masses in a rigid
/// ring of radius `radius_units`.
///
/// `v = √(G·m / (√3·R))` — the derivation is in [`three_body_ring`]'s docs. Public
/// because it is the reference the random factors are relative to, so a test asserting
/// "this body started at 0.9 of circular" needs the same number the builder used
/// rather than its own copy of the algebra.
///
/// Evaluated in `f64` against SI, then divided by
/// [`METRES_PER_UNIT`](crate::constants::METRES_PER_UNIT) — the same units↔SI dance
/// [`crate::core::integrator`] does, and for the same reason: `G·m` is `4e14`, which
/// does not fit comfortably in `f32`.
///
/// Returns `0.0` for a non-positive or non-finite radius or mass rather than a `NaN`,
/// matching the crate's benign-in-release contract.
pub fn ring_circular_speed_units_s(radius_units: f32, mass_kg: f32) -> f32 {
    debug_assert!(
        radius_units.is_finite() && radius_units > 0.0,
        "ring_circular_speed_units_s: radius must be finite and positive, got {}",
        radius_units
    );
    debug_assert!(
        mass_kg.is_finite() && mass_kg > 0.0,
        "ring_circular_speed_units_s: mass must be finite and positive, got {}",
        mass_kg
    );

    if !radius_units.is_finite() || radius_units <= 0.0 || !mass_kg.is_finite() || mass_kg <= 0.0 {
        return 0.0;
    }

    let radius_m = f64::from(radius_units) * crate::constants::METRES_PER_UNIT;
    let speed_m_s =
        (GRAVITATIONAL_CONSTANT * f64::from(mass_kg) / (3.0_f64.sqrt() * radius_m)).sqrt();

    (speed_m_s / crate::constants::METRES_PER_UNIT) as f32
}

/// Colour of the ring body at `index`, cycling through three distinguishable hues.
///
/// Three bodies that look alike make a chaotic trajectory impossible to follow — which
/// is the one thing this experiment is for. Red, green, and blue, in linear space to
/// match [`Color`]'s contract.
fn ring_body_color(index: u32) -> Color {
    match index % 3 {
        0 => Color {
            r: 0.90,
            g: 0.30,
            b: 0.25,
            a: 1.0,
        },
        1 => Color {
            r: 0.30,
            g: 0.85,
            b: 0.35,
            a: 1.0,
        },
        _ => Color {
            r: 0.30,
            g: 0.45,
            b: 0.95,
            a: 1.0,
        },
    }
}

/// Human-readable catalog listing, one line per experiment.
///
/// What `--list` prints. Built here rather than in `main.rs` so the catalog is the
/// single source of the menu, and so it is testable without running the binary.
pub fn catalog_lines() -> Vec<String> {
    ExperimentId::ALL
        .iter()
        .map(|id| format!("{:<14} {} — {}", id.slug(), id.name(), id.description()))
        .collect()
}

#[cfg(test)]
#[path = "../../tst/experiments/catalog_tests.rs"]
mod tests;
