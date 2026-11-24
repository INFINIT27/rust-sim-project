//! Pure-Rust scene model.
//!
//! This module is intentionally free of `bevy_egui` and `wgpu` imports.
//! The only Bevy crate it touches is `bevy::math`, which re-exports
//! `glam`'s `Vec3` and `Quat` — this lets the public `Scene` API stay
//! glam-compatible without declaring a direct `glam` dependency (see
//! Requirement 15.3).
//!
//! Populated incrementally by the tasks under section 2 and 3 of
//! the original spec's task list. Task 2.1 defines the data model
//! primitives (`Pose`, `Color`, `Aabb`, `BodyId`, `Body`); later tasks add
//! `is_valid` predicates, the `Scene` container, and `Scene::step`.

// Re-export `glam` types via `bevy::math` so downstream modules (and library
// consumers) pick up the exact same `Vec3`/`Quat` that Bevy uses, without
// `Scene`'s module needing a direct `glam` dependency.
pub use bevy::math::{DVec3, Quat, Vec3};

use crate::constants::{
    EARTH_MASS_KG, METRES_PER_UNIT, TWO_BODY_HEIGHT_UNITS, TWO_BODY_SEPARATION_UNITS,
    TWO_BODY_VISUAL_RADIUS_UNITS,
};
use crate::physics_utils::thermal::TillotsonParameters;
use crate::time::simulation::TimeStep;

pub use crate::core::integrator::GravityModel;

use crate::core::constants::{
    DEFAULT_DEBRIS_FRAGMENTS, DEFAULT_FRICTION, DEFAULT_PARTICLES_PER_BODY, DEFAULT_RESTITUTION,
    DEMO_BOUNDS_MAX_UNITS, DEMO_BOUNDS_MIN_UNITS, DEMO_FLOOR_COLOR, DEMO_FLOOR_HALF_SIZE_UNITS,
    GRAVITY_SOFTENING_UNITS, MAX_DEBRIS_FRAGMENTS, MAX_PARTICLES_PER_BODY, MIN_PARTICLES_PER_BODY,
    MOON_MAX_MASS_RATIO, ROTATION_UNIT_TOLERANCE, TWO_BODY_LEFT_COLOR, TWO_BODY_RIGHT_COLOR,
};

/// A rigid-body pose: a position plus a unit-quaternion rotation.
///
/// The position is `f64`. It is integrated state: every substep adds a small displacement to it, and
/// at Earth's distance from the Sun (`150` units) an `f32` position resolves only `15 km`, so the
/// rounding of each addition drifted Earth by `18,640 km` a day at a `5.6 s` substep. The rotation
/// is never integrated, so it stays `f32`.
///
/// See Requirement 6.2. Validation (finite components, unit rotation within
/// `1e-4`) lands in task 2.2 via `Pose::is_valid`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub rotation: Quat,
}

impl Pose {
    /// Validation_Rules for `Pose` (Requirement 7.5):
    /// every component of `position` is finite, and `rotation` is finite
    /// with `(|rotation| - 1.0).abs() <= 1e-4`.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.position.is_finite()
            && self.rotation.is_finite()
            && (self.rotation.length() - 1.0).abs() <= ROTATION_UNIT_TOLERANCE
    }
}

/// Linear-space RGBA color with four `f32` channels in `[0.0, 1.0]`.
///
/// See Requirement 6.4. Validation lands in task 2.2.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Validation_Rules for `Color` (Requirement 7.6):
    /// each channel is finite and lies in the closed interval `[0.0, 1.0]`.
    #[inline]
    pub fn is_valid(&self) -> bool {
        let in_unit = |c: f32| c.is_finite() && (0.0..=1.0).contains(&c);
        in_unit(self.r) && in_unit(self.g) && in_unit(self.b) && in_unit(self.a)
    }
}

/// Axis-aligned bounding box defining the logical bounds of a `Scene`.
///
/// See Requirement 6.5. Validation lands in task 2.2.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Validation_Rules for `Aabb` (Requirement 7.7):
    /// every component of `min` and `max` is finite, and
    /// `min.{x,y,z} <= max.{x,y,z}`.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.min.is_finite()
            && self.max.is_finite()
            && self.min.x <= self.max.x
            && self.min.y <= self.max.y
            && self.min.z <= self.max.z
    }
}

/// Stable identifier for a `Body` inside a `Scene`.
///
/// Monotonically increasing, handed out by `Scene::add` (task 3.2). See
/// Requirement 6.6.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct BodyId(pub u64);

/// The tagged union of drawable primitives. See Requirement 6.1.
///
/// The four variants — and only these four — are fixed by design. Each variant
/// owns everything needed to render it; validation rules per variant land in
/// task 2.2 via `Body::is_valid`.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Sphere {
        pose: Pose,
        radius: f32,
        color: Color,
    },
    Cuboid {
        pose: Pose,
        half_extents: Vec3,
        color: Color,
    },
    Line {
        from: DVec3,
        to: DVec3,
        width: f32,
        color: Color,
    },
    Plane {
        pose: Pose,
        half_size: f32,
        color: Color,
    },
}

impl Body {
    /// Validation_Rules for `Body` (Requirements 7.1–7.4):
    ///
    /// - `Sphere`: `pose.is_valid()`, `color.is_valid()`, `radius.is_finite()`,
    ///   and `radius > 0.0`.
    /// - `Cuboid`: `pose.is_valid()`, `color.is_valid()`, and every component
    ///   of `half_extents` is finite and strictly greater than `0.0`.
    /// - `Line`: `color.is_valid()`, `width.is_finite()`, `width >= 0.0`, and
    ///   every component of `from` and `to` is finite.
    /// - `Plane`: `pose.is_valid()`, `color.is_valid()`, `half_size.is_finite()`,
    ///   and `half_size > 0.0`.
    #[inline]
    pub fn is_valid(&self) -> bool {
        match self {
            Body::Sphere {
                pose,
                radius,
                color,
            } => pose.is_valid() && color.is_valid() && radius.is_finite() && *radius > 0.0,
            Body::Cuboid {
                pose,
                half_extents,
                color,
            } => {
                pose.is_valid()
                    && color.is_valid()
                    && half_extents.is_finite()
                    && half_extents.x > 0.0
                    && half_extents.y > 0.0
                    && half_extents.z > 0.0
            }
            Body::Line {
                from,
                to,
                width,
                color,
            } => {
                color.is_valid()
                    && width.is_finite()
                    && *width >= 0.0
                    && from.is_finite()
                    && to.is_finite()
            }
            Body::Plane {
                pose,
                half_size,
                color,
            } => pose.is_valid() && color.is_valid() && half_size.is_finite() && *half_size > 0.0,
        }
    }
    /// The variant's name, for display: `"Sphere"`, `"Cuboid"`, `"Line"`, or
    /// `"Plane"`.
    ///
    /// Exists so the debug overlay can label a prop without matching on `Body`
    /// itself, which would put a display concern in the UI layer that has to be
    /// updated every time a variant is added.
    #[inline]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Body::Sphere { .. } => "Sphere",
            Body::Cuboid { .. } => "Cuboid",
            Body::Line { .. } => "Line",
            Body::Plane { .. } => "Plane",
        }
    }

    /// The body's centre point, in world units.
    ///
    /// For `Sphere`, `Cuboid`, and `Plane` this is `pose.position`. `Line` has no
    /// `Pose`, so its centre is the midpoint of its two endpoints — the same point
    /// `transform_from` uses as the rendered rod's translation.
    ///
    /// Total: every variant has a centre, so there is no `Option` here.
    #[inline]
    pub fn center(&self) -> DVec3 {
        match self {
            Body::Sphere { pose, .. } | Body::Cuboid { pose, .. } | Body::Plane { pose, .. } => {
                pose.position
            }
            Body::Line { from, to, .. } => 0.5 * (*from + *to),
        }
    }

    /// Move the body by `delta` world units, preserving its shape and orientation.
    ///
    /// Translates `pose.position` for the posed variants and *both* endpoints for
    /// a `Line`, so a line keeps its length and direction. `Vec3::ZERO` is an
    /// exact no-op, which is what lets `Scene::step(0.0)` leave the scene
    /// byte-identical.
    ///
    /// Total and infallible: no precondition, no validation. A `delta` containing
    /// a non-finite component will make the body invalid, which
    /// [`Body::is_valid`] then reports — this method does not police its caller,
    /// matching the module's advisory-validation convention.
    #[inline]
    pub fn translate(&mut self, delta: DVec3) {
        match self {
            Body::Sphere { pose, .. } | Body::Cuboid { pose, .. } | Body::Plane { pose, .. } => {
                pose.position += delta;
            }
            Body::Line { from, to, .. } => {
                *from += delta;
                *to += delta;
            }
        }
    }
}

/// What a prop's surface does in a contact: how much it bounces and how much it grips.
///
/// Two numbers, both dimensionless:
///
/// - [`restitution`](Self::restitution) — the coefficient of restitution `e`, in `[0, 1]`.
///   The fraction of the approach speed along the contact normal that comes back as
///   separation speed. `1` is a perfect bounce, `0` is the pair moving off together.
/// - [`friction`](Self::friction) — Coulomb's coefficient `μ`, `≥ 0`. The impulse along the
///   surface is at most `μ` times the impulse along the normal, so a glancing hit loses
///   some of its sliding speed.
///
/// # A pair, not a body
///
/// Restitution and friction are properties of *two* surfaces meeting, not of one. A
/// contact uses [`Material::combined_with`]: the **lower** restitution (the softer body
/// takes up the energy, so a steel ball on sand barely bounces) and the **geometric mean**
/// of the frictions (so a frictionless surface makes the contact frictionless).
///
/// # The default changes nothing
///
/// Every [`Prop`] constructor sets [`Material::ELASTIC`] — `e = 1`, `μ = 0` — which is the
/// behaviour contact had before materials existed. A scene opts in with
/// [`Prop::with_material`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::Material;
///
/// let rock = Material::new(0.5, 0.6);
/// let ice = Material::new(0.8, 0.05);
/// assert!(rock.is_valid() && ice.is_valid());
///
/// let contact = rock.combined_with(&ice);
/// assert_eq!(contact.restitution, 0.5);
/// assert!((contact.friction - (0.6_f32 * 0.05).sqrt()).abs() < 1e-6);
///
/// assert!(!Material::new(1.5, 0.0).is_valid(), "a bounce cannot create energy");
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Material {
    /// Coefficient of restitution, dimensionless, in `[0, 1]`.
    pub restitution: f32,
    /// Coefficient of friction, dimensionless, finite and `≥ 0`.
    pub friction: f32,
    /// The effective elastic modulus of the material, in pascals, when its contacts are
    /// **simulated** rather than resolved in one instant. `None`, the default, is a rigid body.
    ///
    /// Two props that both have a modulus touch through a force, not an impulse: a damped spring
    /// whose stiffness comes from the modulus and the particles' size, acting over every substep
    /// they overlap (see [`crate::collision::collision::apply_contact_forces`]). That is what the
    /// particles of a planet built from particles use, so that two planets meeting deform, flow and
    /// merge over the course of the contact. Must be finite and positive when `Some`.
    pub modulus_pa: Option<f32>,
    /// How the material pushes back when it is **hot**: the Tillotson parameters of its thermal
    /// pressure. `None`, the default, is a material that heat does not push.
    ///
    /// Only particles in simulated contact use it. With it, the energy their contacts dissipate
    /// becomes heat in the particles ([`Prop::heat_j_kg`]) rather than leaving the simulation, and
    /// hot neighbours push each other apart (see [`crate::collision::collision::apply_contact_forces`]).
    /// That is what lets a giant impact throw material off. Read per particle, never combined.
    pub thermal: Option<Tillotson>,
    /// Whether the material is a **fluid**: a particle of it has no modulus, no friction and no
    /// spring, and its [`thermal`](Self::thermal) equation of state is used whole, with a density
    /// measured from its neighbours, by smoothed-particle hydrodynamics (see
    /// [`crate::collision::collision::apply_contact_forces`]). `false`, the default. Set with
    /// [`Material::fluid`], which also sets the equation of state; a fluid material without one is
    /// invalid.
    ///
    /// That is how a planet behaves at its own scale: it resists compression with its pressure and
    /// barely resists a change of shape at all, so a tide stretches it.
    pub fluid: bool,
}

/// The Tillotson equation of state of a material, stored in `f32` as a [`Material`] keeps all its
/// numbers: the ten parameters of
/// [`crate::physics_utils::thermal::TillotsonParameters`], with the same
/// names and units.
///
/// - A **rubble** particle (one with a modulus) uses only `a`, `b` and `reference_energy_j_kg`,
///   for its thermal pressure at reference density
///   ([`crate::physics_utils::thermal::tillotson_thermal_pressure`]).
/// - A **fluid** particle ([`Material::fluid`]) uses all ten
///   ([`crate::physics_utils::thermal::tillotson_pressure`]).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tillotson {
    /// `ρ₀`: the density at which the cold material is under no pressure, kg/m³.
    pub reference_density_kg_m3: f32,
    /// `A`: the linear coefficient of the cold compression curve, Pa.
    pub bulk_a_pa: f32,
    /// `B`: the quadratic coefficient of the cold compression curve, Pa.
    pub bulk_b_pa: f32,
    /// The Grüneisen parameter of rock heated far past `reference_energy_j_kg`. Dimensionless.
    pub a: f32,
    /// Added to `a`, the Grüneisen parameter of cool rock. Dimensionless.
    pub b: f32,
    /// `E₀`: the heat over which the Grüneisen parameter falls from `a + b` to `a`, in joules per
    /// kilogram.
    pub reference_energy_j_kg: f32,
    /// `E_iv`: the energy at which expanded material begins to vaporise, J/kg.
    pub incipient_vaporisation_j_kg: f32,
    /// `E_cv`: the energy at which expanded material is completely vaporised, J/kg.
    pub complete_vaporisation_j_kg: f32,
    /// `α`: the decay rate of the thermal term's solid part in expanded vapour. Dimensionless.
    pub alpha: f32,
    /// `β`: the decay rate of the cold term in expanded vapour. Dimensionless.
    pub beta: f32,
}

impl Tillotson {
    /// The parameters in `f64`, as the laws in [`crate::physics_utils::thermal`] take them.
    pub fn parameters(&self) -> TillotsonParameters {
        TillotsonParameters {
            reference_density_kg_m3: f64::from(self.reference_density_kg_m3),
            bulk_a_pa: f64::from(self.bulk_a_pa),
            bulk_b_pa: f64::from(self.bulk_b_pa),
            a: f64::from(self.a),
            b: f64::from(self.b),
            reference_energy_j_kg: f64::from(self.reference_energy_j_kg),
            incipient_vaporisation_j_kg: f64::from(self.incipient_vaporisation_j_kg),
            complete_vaporisation_j_kg: f64::from(self.complete_vaporisation_j_kg),
            alpha: f64::from(self.alpha),
            beta: f64::from(self.beta),
        }
    }

    /// Valid as [`TillotsonParameters::is_valid`] says.
    pub fn is_valid(&self) -> bool {
        self.parameters().is_valid()
    }
}

impl Material {
    /// Perfectly elastic and frictionless: what every prop is until it is told otherwise.
    /// See [`DEFAULT_RESTITUTION`].
    pub const ELASTIC: Self = Self {
        restitution: DEFAULT_RESTITUTION,
        friction: DEFAULT_FRICTION,
        modulus_pa: None,
        thermal: None,
        fluid: false,
    };

    /// A rigid material with the given restitution and friction. Not validated here; see
    /// [`Material::is_valid`].
    #[inline]
    pub const fn new(restitution: f32, friction: f32) -> Self {
        Self {
            restitution,
            friction,
            modulus_pa: None,
            thermal: None,
            fluid: false,
        }
    }

    /// The same material, its contacts simulated as a force with an effective modulus of
    /// `modulus_pa` pascals. See [`modulus_pa`](Self::modulus_pa).
    #[inline]
    pub const fn with_modulus(mut self, modulus_pa: f32) -> Self {
        self.modulus_pa = Some(modulus_pa);
        self
    }

    /// The same material, pushing back when hot with the Tillotson parameters `thermal`. See
    /// [`thermal`](Self::thermal).
    #[inline]
    pub const fn with_thermal(mut self, thermal: Tillotson) -> Self {
        self.thermal = Some(thermal);
        self
    }

    /// A **fluid** with the equation of state `eos`: no bounce, no friction, no modulus. Its
    /// particles meet through their pressure and an artificial viscosity, by smoothed-particle
    /// hydrodynamics, never through a spring. See [`fluid`](Self::fluid).
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{Material, Tillotson};
    ///
    /// let basalt = Tillotson {
    ///     reference_density_kg_m3: 2700.0,
    ///     bulk_a_pa: 2.67e10,
    ///     bulk_b_pa: 2.67e10,
    ///     a: 0.5,
    ///     b: 1.5,
    ///     reference_energy_j_kg: 4.87e8,
    ///     incipient_vaporisation_j_kg: 4.72e6,
    ///     complete_vaporisation_j_kg: 1.82e7,
    ///     alpha: 5.0,
    ///     beta: 5.0,
    /// };
    /// let molten = Material::fluid(basalt);
    /// assert!(molten.is_valid() && molten.is_particle());
    /// assert_eq!(molten.modulus_pa, None);
    /// ```
    #[inline]
    pub const fn fluid(eos: Tillotson) -> Self {
        Self {
            restitution: 0.0,
            friction: 0.0,
            modulus_pa: None,
            thermal: Some(eos),
            fluid: true,
        }
    }

    /// Whether a prop of this material is a particle whose contacts are simulated as forces: it
    /// has a modulus (a rubble particle, touching through springs) or it is a
    /// [`fluid`](Self::fluid) (touching through its pressure).
    #[inline]
    pub const fn is_particle(&self) -> bool {
        self.modulus_pa.is_some() || self.fluid
    }

    /// Restitution finite and in `[0, 1]`, friction finite and non-negative, a modulus, if
    /// any, finite and positive, and a fluid with an equation of state and no modulus.
    ///
    /// Above `1` a bounce would create energy; below `0` the bodies would pass into each
    /// other. A negative friction would accelerate sliding.
    #[inline]
    pub fn is_valid(&self) -> bool {
        let modulus_ok = self
            .modulus_pa
            .is_none_or(|modulus| modulus.is_finite() && modulus > 0.0);
        let fluid_ok = !self.fluid || (self.thermal.is_some() && self.modulus_pa.is_none());
        (0.0..=1.0).contains(&self.restitution)
            && self.friction.is_finite()
            && self.friction >= 0.0
            && modulus_ok
            && fluid_ok
            && self.thermal.is_none_or(|thermal| thermal.is_valid())
    }

    /// The material of the **contact** between this surface and `other`.
    ///
    /// The lower restitution and the geometric mean of the frictions. Symmetric, so the
    /// result does not depend on which prop is `a`. Of two valid materials it is valid.
    ///
    /// **Why the minimum for restitution.** The energy a contact loses goes into
    /// deforming whichever body gives way, so the softer surface sets it: a rubber ball
    /// on concrete bounces like rubber, a steel ball on sand like sand. The minimum is the
    /// simplest rule with that property, and it keeps two elastic props exactly elastic.
    ///
    /// **Why the geometric mean for friction.** It goes to zero when either surface is
    /// frictionless, which the arithmetic mean does not, and it is the convention in most
    /// rigid-body engines (Box2D, Bullet), so tuned values carry over.
    ///
    /// **Why the series sum for the modulus.** Two bodies pressed together compress like two
    /// springs in series, so the contact's modulus is `E₁E₂ / (E₁ + E₂)` (the Hertz contact
    /// modulus, without Poisson's ratio). It is `Some` only when both are, because a contact is
    /// simulated as a force only between two bodies that both deform.
    #[inline]
    pub fn combined_with(&self, other: &Material) -> Material {
        Material {
            restitution: self.restitution.min(other.restitution),
            friction: (self.friction * other.friction).sqrt(),
            modulus_pa: match (self.modulus_pa, other.modulus_pa) {
                (Some(a), Some(b)) => Some(a * b / (a + b)),
                _ => None,
            },
            // Heat belongs to each particle, not to the contact: never combined.
            thermal: None,
            fluid: self.fluid && other.fluid,
        }
    }
}

impl Default for Material {
    #[inline]
    fn default() -> Self {
        Self::ELASTIC
    }
}

/// How much detail a scene simulates its collisions in.
///
/// Two dials, both counts of bodies, because every body costs gravity and contact against every
/// other, so the cost grows as the square of the count:
///
/// - [`particles_per_body`](Self::particles_per_body): how many particles a planet built from
///   particles is made of. The contact between two such planets is simulated particle by particle
///   (see [`crate::collision::collision`]), so this is the resolution of the deformation, the merge
///   and the debris. It is read when a scene is **built**, so a change applies from the next launch.
/// - [`debris_fragments`](Self::debris_fragments): for two planets that are single spheres, which
///   merge in one instant, how many bodies the debris the scaling law gives is split into. Read at
///   every impact.
///
/// Set from `config/collision.toml` and the menu (Configuration > Collisions). The scene carries
/// it, rather than the collision layer reading the configuration, because the collision layer
/// sees nothing but the scene.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{CollisionSettings, DEFAULT_DEBRIS_FRAGMENTS, MAX_DEBRIS_FRAGMENTS};
///
/// let settings = CollisionSettings::default();
/// assert_eq!(settings.debris_fragments, DEFAULT_DEBRIS_FRAGMENTS);
/// assert!(CollisionSettings { debris_fragments: 0, ..settings }.is_valid(), "a perfect merge");
/// assert!(!CollisionSettings { debris_fragments: MAX_DEBRIS_FRAGMENTS + 1, ..settings }.is_valid());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CollisionSettings {
    /// How many particles the larger planet of a particle-built scene is made of, from
    /// [`MIN_PARTICLES_PER_BODY`] to [`MAX_PARTICLES_PER_BODY`]. A smaller planet gets fewer, of
    /// the same size, in proportion to its volume.
    pub particles_per_body: u32,
    /// How many bodies the debris of one instant merge is split into. `0` keeps all the mass in the
    /// merged body, which is a perfect merge. At most [`MAX_DEBRIS_FRAGMENTS`].
    pub debris_fragments: u32,
    /// What a planet built from particles is made of: a fluid with an iron core, or rubble held
    /// by springs. Read when a scene is **built**, like
    /// [`particles_per_body`](Self::particles_per_body).
    pub planet_model: PlanetModel,
}

/// What a planet built from particles is made of. See [`CollisionSettings::planet_model`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum PlanetModel {
    /// A fluid, by smoothed-particle hydrodynamics: an iron core under a basalt mantle, in
    /// hydrostatic equilibrium, each with its whole Tillotson equation of state. It resists
    /// compression and not a change of shape, as a planet does, so a tide stretches it. The
    /// default.
    #[default]
    Fluid,
    /// Rubble: a uniform pile of basalt particles touching through damped springs, with
    /// Byerlee's friction and a thermal pressure. Stiff against tides. The model before fluid
    /// planets, kept to compare against.
    Rubble,
}

impl PlanetModel {
    /// Both models, in menu order.
    pub const ALL: [Self; 2] = [Self::Fluid, Self::Rubble];

    /// The model's name in `collision.toml`: `fluid` or `rubble`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fluid => "fluid",
            Self::Rubble => "rubble",
        }
    }

    /// The model a name in `collision.toml` stands for, or `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|model| model.name() == name)
    }

    /// How the menu shows the model.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fluid => "Fluid (SPH, iron core)",
            Self::Rubble => "Rubble (springs)",
        }
    }
}

impl CollisionSettings {
    /// [`DEFAULT_PARTICLES_PER_BODY`] particles, [`DEFAULT_DEBRIS_FRAGMENTS`] fragments, and fluid
    /// planets.
    pub const DEFAULT: Self = Self {
        particles_per_body: DEFAULT_PARTICLES_PER_BODY,
        debris_fragments: DEFAULT_DEBRIS_FRAGMENTS,
        planet_model: PlanetModel::Fluid,
    };

    /// Both counts inside their ranges.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.debris_fragments <= MAX_DEBRIS_FRAGMENTS
            && (MIN_PARTICLES_PER_BODY..=MAX_PARTICLES_PER_BODY).contains(&self.particles_per_body)
    }
}

impl Default for CollisionSettings {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A thing in the scene: a shape, how it is moving, and how much it weighs.
///
/// `Body` describes only what a prop *looks like* — geometry and colour. `Prop`
/// adds what it *does*: a velocity, a mass, and the [`Material`] its surface is made of.
/// Keeping them apart means the render bridge still takes a `&Body` and knows nothing
/// about dynamics, and there is exactly one place to extend when angular velocity
/// arrives.
///
/// # The velocity vector carries both direction and speed
///
/// [`velocity_units_s`](Self::velocity_units_s) is a single vector whose **direction is
/// the heading** and whose **magnitude is the speed** in metres per second. A
/// prop at rest has `Vec3::ZERO`, which has no meaningful direction — which is
/// why [`heading`](Self::heading) returns an `Option` rather than normalising a
/// zero vector into `NaN`.
///
/// # Mass, masslessness, and immovability are three different things
///
/// These are deliberately independent, because collapsing them loses
/// information:
///
/// | `immovable` | `mass_kg` | Meaning |
/// | --- | --- | --- |
/// | `false` | `> 0.0` | An ordinary dynamic prop. Forces accelerate it. |
/// | `false` | `0.0` | **Massless but moving** — it travels at its velocity, but `F = m·a` cannot accelerate it. Reserved for photon-like props. |
/// | `true` | any | **Immovable.** Never integrated and unaffected by any force — but it still *sources* gravity in proportion to `mass_kg`, so an immovable prop with a mass is a fixed attractor. |
///
/// Note in particular that `mass_kg == 0.0` does **not** mean "static". Many
/// physics engines overload zero mass to mean infinite mass; this one does not,
/// so that genuinely massless props remain expressible. Immovability has its own
/// field, and it takes precedence for *motion*: an immovable prop is never
/// accelerated whatever its mass. Its `mass_kg` still matters as the strength of
/// the gravitational field it produces — which is exactly how you build a fixed
/// attractor. [`Prop::immovable`] sets it to `0.0`, making the prop
/// gravitationally inert; use [`Prop::immovable_with_mass`] for an attractor.
///
/// # Fields are public and validation is advisory
///
/// All four fields are `pub` so a prop can be built with a struct literal, and
/// nothing validates them on assignment. [`is_valid`](Self::is_valid) is a pure
/// predicate; the only enforcement is the `debug_assert!` in [`Scene::add`], so a
/// release build accepts an invalid prop silently (Requirement 14.4). This
/// mirrors [`Body::is_valid`].
///
/// # Examples
///
/// ```
/// use physics_sandbox::{Body, Color, Pose, Prop, Quat, DVec3, Vec3};
///
/// let ball = Body::Sphere {
///     pose: Pose { position: DVec3::new(0.0, 10.0, 0.0), rotation: Quat::IDENTITY },
///     radius: 0.5,
///     color: Color { r: 0.8, g: 0.2, b: 0.2, a: 1.0 },
/// };
///
/// // Falling straight down at 3 m/s, 2 kg.
/// let prop = Prop::new(ball, DVec3::new(0.0, -3.0, 0.0), 2.0);
///
/// assert!(prop.is_valid());
/// assert_eq!(prop.speed(), 3.0);
/// assert_eq!(prop.heading(), Some(DVec3::new(0.0, -1.0, 0.0)));
/// assert!(prop.responds_to_forces());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Prop {
    /// The shape and colour. Purely visual; carries no dynamics.
    pub body: Body,
    /// Velocity in world **units** per second — not metres per second. Direction
    /// is the heading, magnitude is the speed. `Vec3::ZERO` means at rest.
    ///
    /// Units rather than metres because `position += velocity · dt` is only
    /// dimensionally sound when the velocity uses the same length unit as the
    /// position, and positions are in units. Use [`Prop::speed_m_s`] to read the
    /// speed in SI.
    ///
    /// `f64`, like the position, because it is integrated too: a substep adds `a · dt`, and near
    /// Earth that is `0.03 m/s` on `30 km/s`, which an `f32` rounds by up to `1.8 mm/s` the same way
    /// every substep, tens of m/s a day.
    ///
    /// Must be finite. Must be exactly `DVec3::ZERO` when
    /// [`immovable`](Self::immovable) is set — an immovable prop with a velocity
    /// is a contradiction, and silently ignoring it would leave the author
    /// wondering why their wall never moved.
    pub velocity_units_s: DVec3,
    /// Mass in kilograms. Must be finite and non-negative.
    ///
    /// `0.0` means genuinely massless, **not** static — see the table on
    /// [`Prop`]. When [`immovable`](Self::immovable) is set this mass no longer
    /// accelerates the prop, but it still determines how hard the prop pulls on
    /// everything else.
    pub mass_kg: f32,
    /// When set, the prop is never moved by the simulation: no integration and no
    /// response to forces. It still exerts gravity in proportion to `mass_kg`.
    ///
    /// This is the ground plane and, later, walls and other fixed geometry. It is
    /// a field of its own rather than a magic mass value so that `mass_kg == 0.0`
    /// stays free to mean massless.
    pub immovable: bool,
    /// When set, this prop **absorbs** what it touches instead of bouncing off it: it
    /// takes the other prop's mass and momentum, and the other prop leaves the scene.
    ///
    /// Set only by black holes (see
    /// [`StellarObject`](crate::projects::interstellar::objects::StellarObject)). An elastic bounce
    /// off an event horizon is not an approximation of what happens, it is the opposite
    /// of it, which is why this is a contact *response* rather than a tuning value —
    /// [`crate::collision::absorb`] implements it and
    /// [`crate::collision::resolve`] never sees the pair.
    ///
    /// A field on `Prop` rather than a property of the object catalog because the
    /// `collision` layer has to act on it and may not read `projects`. Two absorbers
    /// in contact merge into the more massive one; see `absorb_contacts`.
    pub absorbs: bool,
    /// The radius at which this prop **physically** interacts, in world units, when that
    /// differs from the radius it is **drawn** at. `None` means the two are the same.
    ///
    /// # Why a body can need two radii
    ///
    /// A `Body::Sphere`'s radius is what the renderer draws *and* what
    /// [`collision_mesh`](crate::collision::tessellate::collision_mesh) builds a hull from,
    /// and those two jobs pull in opposite directions the moment a scene spans more than one
    /// scale. Jupiter is the case that forced this: it is drawn at `10.973` units so it can
    /// be seen from Mars's orbit, and its true radius is `0.0699` units — but Io orbits at
    /// `0.4217`, so the *drawn* Jupiter is `26×` its innermost moon's whole orbit. With one
    /// radius, the Galilean moons start inside the planet and the contact solver ejects all
    /// four on the first step. There is no exaggeration that both shows Jupiter and lets it
    /// hold a moon.
    ///
    /// So the visual radius stays on the [`Body`] and the physical one lives here. Contact
    /// and absorption use this when it is set; **gravitation never does**, because
    /// gravitation treats every prop as a point mass at its centre and has no radius in it
    /// at all.
    ///
    /// # It is the *contact* radius, not necessarily the surface
    ///
    /// For a planet or a moon it is the real surface. For a black hole it is deliberately
    /// **not** the event horizon: matter is destroyed at the tidal-disruption radius, which
    /// for a solar-type star falling into a stellar-mass hole is tens of thousands of times
    /// the horizon. See [`crate::projects::interstellar::objects`].
    ///
    /// Must be finite and strictly positive when `Some`; a non-positive value would produce
    /// a degenerate hull, so [`is_valid`](Self::is_valid) rejects it.
    pub contact_radius_units: Option<f32>,
    /// How much light this body emits, in **solar luminosities**. `0.0` — the default — means it
    /// emits none and is seen only by reflected light.
    ///
    /// A physical property of the body that only the *renderer* reads, like
    /// [`absorbs`](Self::absorbs) is one that only `collision` reads. A star is self-luminous and a
    /// planet is not, and that is a fact about the object rather than a lighting preference: the
    /// render layer turns a positive value into a point light at the body's own position, with
    /// inverse-square falloff, so the terminator on a planet is where the Sun actually puts it.
    ///
    /// In solar luminosities rather than lumens because that is how stellar output is quoted —
    /// Proxima Centauri is `0.0017 L☉`, Sirius B is `0.056` — and because the conversion to Bevy's
    /// photometric units belongs in the layer that knows about Bevy. See
    /// [`STAR_LUMENS_PER_SOLAR_LUMINOSITY`](crate::render::constants::STAR_LUMENS_PER_SOLAR_LUMINOSITY).
    ///
    /// Must be finite and non-negative.
    pub luminosity_solar: f32,
    /// What the prop's surface does in a contact: its restitution and friction. See
    /// [`Material`].
    ///
    /// [`Material::ELASTIC`] from every constructor, which is how contact behaved before this
    /// field existed; set another with [`with_material`](Self::with_material). Read only by
    /// [`crate::collision::resolve`]. Absorption ignores it, since nothing bounces off a black
    /// hole whatever it is made of.
    ///
    /// Must satisfy [`Material::is_valid`].
    pub material: Material,
    /// How fast the prop spins, as an angular velocity in radians per second (the axis is the
    /// direction, the rate the length).
    ///
    /// Zero from every constructor. Only particles in simulated contact spin: friction at a contact
    /// turns sliding into rotation, so that friction conserves angular momentum (see
    /// [`crate::collision::collision::apply_contact_forces`]). The spin is not drawn: a particle is
    /// a uniform sphere, and its pose does not turn.
    pub spin_rad_s: DVec3,
    /// The heat the prop holds, as specific internal energy above its starting state, in joules per
    /// kilogram.
    ///
    /// Zero from every constructor. Only particles in simulated contact whose material has a
    /// [`thermal`](Material::thermal) part gain heat: the energy their contacts dissipate goes here,
    /// and it is what their thermal pressure is worked out from. Must be finite and non-negative.
    ///
    /// For a particle of a [`fluid`](Material::fluid) material it is the whole specific internal
    /// energy `u` of the equation of state: the heat and the energy stored by compression, which
    /// the pressure's work moves in and out reversibly.
    pub heat_j_kg: f64,
    /// The density of a fluid particle, as smoothed-particle hydrodynamics last measured it from
    /// its neighbours, in kilograms per cubic metre; `0` until it has been measured, and for every
    /// other prop.
    ///
    /// Zero from every constructor; the planet builder sets it to the density of the planet at the
    /// particle's depth. It sets the particle's smoothing length for the next substep, and is the
    /// density the scene's ledger and the menu read.
    pub density_kg_m3: f64,
}

impl Prop {
    /// A movable prop with the given velocity and mass.
    ///
    /// Sets `immovable` to `false`. Use [`Prop::immovable`] for fixed geometry.
    #[inline]
    pub const fn new(body: Body, velocity_units_s: DVec3, mass_kg: f32) -> Self {
        Self {
            body,
            velocity_units_s,
            mass_kg,
            immovable: false,
            absorbs: false,
            contact_radius_units: None,
            luminosity_solar: 0.0,
            material: Material::ELASTIC,
            spin_rad_s: DVec3::ZERO,
            heat_j_kg: 0.0,
            density_kg_m3: 0.0,
        }
    }

    /// A movable prop that **absorbs** what it touches rather than bouncing off it.
    ///
    /// The black-hole constructor. Everything else about it is an ordinary movable prop:
    /// it is integrated, it pulls, and it can be pulled. The difference is only what
    /// happens on contact — see [`Prop::absorbs`] and [`crate::collision::absorb`].
    #[inline]
    pub const fn absorbing(body: Body, velocity_units_s: DVec3, mass_kg: f32) -> Self {
        Self {
            body,
            velocity_units_s,
            mass_kg,
            immovable: false,
            absorbs: true,
            contact_radius_units: None,
            luminosity_solar: 0.0,
            material: Material::ELASTIC,
            spin_rad_s: DVec3::ZERO,
            heat_j_kg: 0.0,
            density_kg_m3: 0.0,
        }
    }

    /// A movable prop at rest with the given mass.
    #[inline]
    pub const fn at_rest(body: Body, mass_kg: f32) -> Self {
        Self::new(body, DVec3::ZERO, mass_kg)
    }

    /// Fixed geometry: never integrated, never moved by a force.
    ///
    /// `mass_kg` is set to `0.0`, which makes the prop **gravitationally inert**:
    /// it neither moves nor pulls. That is what the ground plane wants — a floor
    /// that tugged at everything above it would be surprising. For fixed geometry
    /// that *should* attract, use [`Prop::immovable_with_mass`].
    ///
    /// The `0.0` is not a claim that the prop is massless; [`is_massless`] is
    /// false for any immovable prop, because its mass field describes a field
    /// strength rather than an inertia.
    ///
    /// [`is_massless`]: Self::is_massless
    #[inline]
    pub const fn immovable(body: Body) -> Self {
        Self {
            body,
            velocity_units_s: DVec3::ZERO,
            mass_kg: 0.0,
            immovable: true,
            absorbs: false,
            contact_radius_units: None,
            luminosity_solar: 0.0,
            material: Material::ELASTIC,
            spin_rad_s: DVec3::ZERO,
            heat_j_kg: 0.0,
            density_kg_m3: 0.0,
        }
    }

    /// Fixed geometry that still attracts: a gravity well that never moves.
    ///
    /// Never integrated and never accelerated, but sources gravity in proportion
    /// to `mass_kg`. This is how you pin a planet in place without it drifting
    /// under the pull of the things orbiting it.
    #[inline]
    pub const fn immovable_with_mass(body: Body, mass_kg: f32) -> Self {
        Self {
            body,
            velocity_units_s: DVec3::ZERO,
            mass_kg,
            immovable: true,
            absorbs: false,
            contact_radius_units: None,
            luminosity_solar: 0.0,
            material: Material::ELASTIC,
            spin_rad_s: DVec3::ZERO,
            heat_j_kg: 0.0,
            density_kg_m3: 0.0,
        }
    }

    /// The same prop, but interacting physically at `radius_units` rather than at the
    /// radius it is drawn with.
    ///
    /// The builder behind every body whose visual size is an exaggeration — see
    /// [`contact_radius_units`](Self::contact_radius_units) for why that is unavoidable
    /// once one scene holds both a planet and its moons.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{Body, Color, Pose, Prop, Quat, DVec3, Vec3};
    ///
    /// let jupiter = Body::Sphere {
    ///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
    ///     radius: 10.973,                       // drawn 157x life size, to be visible
    ///     color: Color { r: 0.8, g: 0.65, b: 0.45, a: 1.0 },
    /// };
    ///
    /// // ...but it collides at its real radius, which is what lets Io orbit at 0.42 units.
    /// let prop = Prop::at_rest(jupiter, 1.8982e27).with_contact_radius(0.069911);
    ///
    /// assert!(prop.is_valid());
    /// assert_eq!(prop.contact_radius_units, Some(0.069911));
    /// ```
    #[inline]
    pub fn with_contact_radius(mut self, radius_units: f32) -> Self {
        self.contact_radius_units = Some(radius_units);
        self
    }

    /// The same prop, emitting `luminosity_solar` solar luminosities of light.
    ///
    /// The star constructor. See [`luminosity_solar`](Self::luminosity_solar) for why a body's own
    /// output is model state rather than a render setting.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{Body, Color, Pose, Prop, Quat, DVec3, Vec3};
    ///
    /// let sun = Body::Sphere {
    ///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
    ///     radius: 0.6957,
    ///     color: Color { r: 1.0, g: 0.85, b: 0.35, a: 1.0 },
    /// };
    ///
    /// let prop = Prop::at_rest(sun, 1.989e30).with_luminosity(1.0);
    ///
    /// assert!(prop.is_valid());
    /// assert_eq!(prop.luminosity_solar, 1.0);
    /// ```
    #[inline]
    pub fn with_luminosity(mut self, luminosity_solar: f32) -> Self {
        self.luminosity_solar = luminosity_solar;
        self
    }

    /// The same prop, with its surface made of `material`.
    ///
    /// How a scene opts out of perfectly elastic, frictionless contact. See [`Material`].
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{Body, Color, Material, Pose, Prop, Quat, DVec3};
    ///
    /// let ball = Body::Sphere {
    ///     pose: Pose { position: DVec3::ZERO, rotation: Quat::IDENTITY },
    ///     radius: 1.0,
    ///     color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
    /// };
    ///
    /// let prop = Prop::at_rest(ball, 1.0).with_material(Material::new(0.5, 0.3));
    /// assert!(prop.is_valid());
    /// assert_eq!(prop.material.restitution, 0.5);
    /// ```
    #[inline]
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// Does this body emit light of its own?
    #[inline]
    pub fn is_luminous(&self) -> bool {
        self.luminosity_solar.is_finite() && self.luminosity_solar > 0.0
    }

    /// Does this prop satisfy the Validation_Rules?
    ///
    /// True when the [`body`](Self::body) is valid, `velocity_units_s` is finite,
    /// `mass_kg` is finite and non-negative, the [`material`](Self::material) is valid, the spin is
    /// finite and the heat finite and non-negative, and
    /// — if [`immovable`](Self::immovable) is set — the velocity is exactly `Vec3::ZERO`.
    #[inline]
    pub fn is_valid(&self) -> bool {
        let contact_radius_ok = match self.contact_radius_units {
            Some(radius) => radius.is_finite() && radius > 0.0,
            None => true,
        };

        self.body.is_valid()
            && self.velocity_units_s.is_finite()
            && self.mass_kg.is_finite()
            && self.mass_kg >= 0.0
            && (!self.immovable || self.velocity_units_s == DVec3::ZERO)
            && contact_radius_ok
            && self.luminosity_solar.is_finite()
            && self.luminosity_solar >= 0.0
            && self.material.is_valid()
            && self.spin_rad_s.is_finite()
            && self.heat_j_kg.is_finite()
            && self.heat_j_kg >= 0.0
            && self.density_kg_m3.is_finite()
            && self.density_kg_m3 >= 0.0
    }

    /// Speed in world units per second: the magnitude of the velocity vector.
    #[inline]
    pub fn speed(&self) -> f64 {
        self.velocity_units_s.length()
    }

    /// Speed in metres per second — the SI reading of [`Prop::speed`].
    ///
    /// Returned as `f64` because the scaled figure is large: one unit per second
    /// is `1e9 m/s`, itself over three times the speed of light, so this is the
    /// number to check when you want to know whether a simulated speed is
    /// physically sensible.
    #[inline]
    pub fn speed_m_s(&self) -> f64 {
        self.speed() * METRES_PER_UNIT
    }

    /// Unit vector in the direction of travel, or `None` when at rest.
    ///
    /// `None` rather than a zero vector or a `NaN`: a stationary prop has no
    /// heading, and normalising `DVec3::ZERO` is undefined.
    #[inline]
    pub fn heading(&self) -> Option<DVec3> {
        let speed = self.speed();
        if speed > 0.0 && speed.is_finite() {
            Some(self.velocity_units_s / speed)
        } else {
            None
        }
    }

    /// Can a force accelerate this prop?
    ///
    /// True only when the prop is movable **and** has positive mass, because
    /// `a = F/m` is undefined at zero mass. A massless movable prop still travels
    /// at its velocity — it just cannot be pushed.
    #[inline]
    pub fn responds_to_forces(&self) -> bool {
        !self.immovable && self.mass_kg > 0.0
    }

    /// Is this prop movable but massless?
    ///
    /// False for immovable props: their `mass_kg` carries no meaning, so calling
    /// them massless would be a claim the field does not support.
    #[inline]
    pub fn is_massless(&self) -> bool {
        !self.immovable && self.mass_kg == 0.0
    }
}

/// Pure-Rust container for the simulated world.
///
/// `Scene` owns every piece of scene state — the list of props and the logical
/// bounds — and exposes a small, GUI-free API. See Requirement 4.1 and
/// Requirement 4.2 (`Scene` references no `bevy::ecs`, `bevy_egui`, or `wgpu`
/// types). The only Bevy crate this type touches is `bevy::math` for the
/// `Vec3`/`Quat` re-exports, which themselves re-export `glam`, per Requirement
/// 15.3.
///
/// # Storage layout
///
/// Props and their `BodyId`s are kept in parallel vectors (`props` and `ids`) so
/// iteration order equals insertion order. `next_id` is the
/// monotonically-increasing id allocator used by `Scene::add`.
///
/// Fields:
///
/// - `props` (public): the props currently in the scene — shape, velocity, mass.
/// - `bounds` (public): the `Aabb` defining the logical bounds of the scene.
/// - `ids` (private): the `BodyId` assigned to each entry in `props`, at the
///   matching index. `ids.len() == props.len()` is an invariant maintained by
///   every method that mutates the scene.
/// - `next_id` (private): the next `u64` the id allocator will hand out.
///   Strictly greater than every id currently stored in `ids` or previously
///   returned by a successful `add` on this `Scene`.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The props currently in the scene, in insertion order.
    pub props: Vec<Prop>,
    /// The logical bounds of the scene.
    pub bounds: Aabb,
    /// Plummer softening length for gravitation in **this** scene, in world units.
    ///
    /// The pairwise force is evaluated at `√(r² + ε²)` rather than at `r`, which caps it at
    /// a finite maximum as `r → 0` instead of diverging.
    ///
    /// # Why it is per scene rather than a constant
    ///
    /// Because ε is a **length**, and — per
    /// [`crate::core::constants::GRAVITY_SOFTENING_UNITS`]'s own
    /// reasoning — it is meant to be about the size of a body: the field inside a real
    /// uniform sphere flattens toward zero at the centre rather than diverging, so a softened
    /// core is what a body of finite size actually has. That makes it correct only relative
    /// to the bodies in front of it, and body sizes now span three orders of magnitude:
    ///
    /// | Scene | Smallest body | ε = 1.0 is | Force at the closest real separation |
    /// | --- | --- | --- | --- |
    /// | `experiment_1` | `1.5` units | ⅔ of a body | `90%` of Newton at contact — intended |
    /// | `experiment_3` | `0.0064` units (Earth) | `157` Earths | `99.996%` at Earth's orbit |
    /// | the Jovian system | `0.0018` units (Io) | `14×` Jupiter | **`15%` at Io's orbit** |
    ///
    /// At Io's orbital radius of `0.4217` units the shipped `ε = 1.0` leaves only `15%` of
    /// Newton's force, which puts the circular speed out by a factor of `2.6` — so a moon
    /// system is impossible with a crate-wide value, whatever the renderer does. Every
    /// existing scene keeps the old constant and is bit-for-bit unchanged; the Jovian system
    /// sets its own.
    ///
    /// Must be finite and non-negative. `0.0` is legal and means true point-mass gravitation,
    /// singularity included — see the constant's docs for what that costs.
    pub softening_units: f32,
    /// Which law of gravity this scene's black holes obey. [`GravityModel::Newtonian`] everywhere
    /// except the black-hole experiment, which sets [`GravityModel::Schwarzschild`]. See
    /// [`crate::core::integrator::GravityModel`].
    pub gravity_model: GravityModel,
    /// How much detail collisions are simulated in: how many debris bodies an impact breaks off.
    /// See [`CollisionSettings`]. The app copies it from `config/collision.toml` (and the menu's
    /// Configuration > Collisions page) every frame, so a change applies to the next impact.
    pub collision: CollisionSettings,
    /// The wormhole this scene is in, if it is in one: then its bodies coast through curved space and
    /// nothing pulls on anything. See [`crate::core::wormhole`].
    pub wormhole: Option<crate::core::wormhole::WormholeSpace>,
    /// `BodyId` for each entry in `props`, kept in lockstep (same length, same
    /// order). Private so callers cannot desynchronize the two vectors.
    ids: Vec<BodyId>,
    /// Monotonic id allocator. Each successful `add` returns `BodyId(next_id)`
    /// and then increments `next_id`. Private so the allocator cannot be
    /// reset out from under live `BodyId`s.
    next_id: u64,
    /// Which set of props this is: a fresh value from [`next_revision`] whenever the scene is built
    /// or a prop is added or removed. The GPU stepper keeps a scene's particles between frames and
    /// uploads them again only when this changes. Private so it cannot be set back.
    revision: u64,
}

/// Two scenes are equal when their props, bounds, settings and ids are: the revision says which build
/// a scene came from, not what is in it, so two scenes built alike compare equal.
impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.props == other.props
            && self.bounds == other.bounds
            && self.softening_units == other.softening_units
            && self.gravity_model == other.gravity_model
            && self.collision == other.collision
            && self.wormhole == other.wormhole
            && self.ids == other.ids
            && self.next_id == other.next_id
    }
}

/// A value no scene has had before: what [`Scene::revision`] is set to.
fn next_revision() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Scene {
    /// A scene with **no props at all** and the given bounds.
    ///
    /// [`Scene::demo`] is the shared *container* — bounds plus a ground plane — and almost
    /// every scene wants it. This is for the ones that do not.
    ///
    /// # When a floor is the wrong thing
    ///
    /// The shipped floor is `5000` units to the side and the render layer rules a grid over it
    /// every `5`. That is a sensible measuring aid for a scene tens of units across and
    /// meaningless for one that is smaller than a single grid cell: the Jovian system is `1.9`
    /// units from Jupiter to Callisto's orbit, so a ground plane would be a surface with no
    /// relationship to anything in the scene, sitting under all of it. Deep space has no floor,
    /// and a moon system is deep space.
    ///
    /// The render layer already copes: `floor_half_size` finds no `Body::Plane`, returns `None`,
    /// and no grid is spawned.
    ///
    /// # Contract
    ///
    /// Identical to [`Scene::demo`] except that `props` is empty: `bounds` is whatever the
    /// caller passed, [`softening_units`](Self::softening_units) is
    /// [`GRAVITY_SOFTENING_UNITS`] until the caller changes it, and the id allocator starts at
    /// zero, so the first [`add`](Self::add) returns `BodyId(0)`.
    ///
    /// A caller passing invalid bounds gets them back unchanged — this constructor validates
    /// nothing, exactly as `demo` validates nothing about the constants it reads. `is_valid` on
    /// the bounds is the caller's to check, and `catalog_tests` checks it for every experiment.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{Aabb, Scene, Vec3};
    ///
    /// let scene = Scene::empty(Aabb { min: Vec3::splat(-2.0), max: Vec3::splat(2.0) });
    ///
    /// assert!(scene.props.is_empty(), "no floor, no anything");
    /// assert!(scene.bounds.is_valid());
    /// ```
    pub fn empty(bounds: Aabb) -> Self {
        Scene {
            props: Vec::new(),
            bounds,
            softening_units: GRAVITY_SOFTENING_UNITS,
            gravity_model: GravityModel::Newtonian,
            collision: CollisionSettings::DEFAULT,
            wormhole: None,
            ids: Vec::new(),
            next_id: 0,
            revision: next_revision(),
        }
    }

    /// Return the starter `Scene`: a single ground plane, and nothing else.
    ///
    /// This is the **simulation container** — the surface that props added by
    /// a simulation will rest on, fall toward, and be positioned relative to.
    /// It is deliberately empty of props so that everything visible in a
    /// running app is something the simulation put there.
    ///
    /// Contract:
    ///
    /// - `bounds.is_valid()` holds and every axis has strictly positive extent
    ///   (`max.{x,y,z} > min.{x,y,z}`) — Requirements 12.2, 3.3.
    /// - `props` contains exactly one prop: an **immovable** `Body::Plane` at
    ///   the origin with identity rotation, satisfying the Validation_Rules —
    ///   Requirement 12.3. The floor is immovable rather than a zero-mass body,
    ///   so a future integrator skips it without needing a special case.
    ///
    /// # Deliberate deviation from the original spec
    ///
    /// Requirements 3.2 and 12.1 require the startup scene to contain at least
    /// one of each of the four `Body` variants — a blue cube at `(-2, 1, 0)`, a
    /// red sphere at `(2, 1, 0)`, and a yellow line along +X, per `design.md`
    /// under "Building a demo scene". Those three props were **removed on
    /// purpose** when the project turned from a rendering demo into a physics
    /// simulation: fixed decoration is indistinguishable from simulated output
    /// once props start moving, which makes it useless as a visual check and
    /// actively misleading. The amendment is recorded in
    /// the original requirements spec.
    ///
    /// `Body` still has all four variants and `transform_from` still renders
    /// all four (Requirement 7.1 is untouched) — only the *startup content*
    /// changed. A simulation is free to add any variant.
    pub fn demo() -> Self {
        // Scene bounds: a 100 × 51 × 100 box centred roughly on the ground
        // plane. Every axis has strictly positive extent (Requirement 12.2).
        let bounds = Aabb {
            min: DEMO_BOUNDS_MIN_UNITS,
            max: DEMO_BOUNDS_MAX_UNITS,
        };

        // Start from an empty scene and use `add` so ids are allocated
        // through the normal path. `add` fires `debug_assert!` on any body
        // that fails validation, giving us a guardrail if the constants
        // below ever drift (Requirement 12.3).
        let mut scene = Scene {
            props: Vec::new(),
            bounds,
            softening_units: GRAVITY_SOFTENING_UNITS,
            gravity_model: GravityModel::Newtonian,
            collision: CollisionSettings::DEFAULT,
            wormhole: None,
            ids: Vec::new(),
            next_id: 0,
            revision: next_revision(),
        };

        // The ground plane, and the only body in the starter scene. At y = 0
        // with identity rotation, so the floor is the XZ plane through the
        // origin and +Y is up — the convention `transform_from` assumes when it
        // maps a `Plane`'s `half_size` onto `(2·h, 1, 2·h)`.
        //
        // half_size 25 → a 50×50 quad. Note this is half the footprint of
        // `bounds` (±50 in X and Z), so a body can be inside the world and
        // still be off the edge of the floor. Left as-is for now; widen this to
        // 50.0 if the floor should span the whole simulation volume.
        scene.add(Prop::immovable(Body::Plane {
            pose: Pose {
                position: DVec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            half_size: DEMO_FLOOR_HALF_SIZE_UNITS,
            color: DEMO_FLOOR_COLOR,
        }));

        scene
    }

    /// The two-body gravitation scene: the container plane plus two equal-mass
    /// spheres **released from rest**, falling toward each other under their
    /// mutual pull.
    ///
    /// This is what `ScenePlugin` inserts, and what a running app shows. It builds
    /// on [`Scene::demo`], so the ground plane and bounds are identical; the two
    /// spheres are simulation subjects, not decoration, which is why they do not
    /// contradict amendment A1 (see [`Scene::demo`]).
    ///
    /// # Configuration
    ///
    /// | Quantity | Value |
    /// | --- | --- |
    /// | Mass of each sphere | [`EARTH_MASS_KG`] = `5.972e24 kg` |
    /// | Starting separation | [`TWO_BODY_SEPARATION_UNITS`] = `30` units = `3e10 m` |
    /// | Starting velocity | `Vec3::ZERO` — at rest |
    /// | Centre | `(0, 3, 0)` |
    /// | Visual radius | [`TWO_BODY_VISUAL_RADIUS_UNITS`] = `1.5` units each |
    ///
    /// The spheres start at `(∓15, 3, 0)` and accelerate along the X axis toward
    /// each other. Nothing is given any tangential velocity, so the motion is
    /// purely radial: there is no orbit and no angular momentum.
    ///
    /// # What you see, and how long it takes
    ///
    /// Radial free fall from rest at separation `d` for two masses `m` takes
    ///
    /// ```text
    /// t = π · √(d³ / (8·G·(m₁ + m₂)))
    /// ```
    ///
    /// which for these values is about `2.0e8 s` — roughly **6.5 years** of
    /// simulated time. `SceneConfig::time_scale` stretches that across about 40
    /// seconds of watching.
    ///
    /// The motion is deliberately unhurried at first and then dramatic: at the
    /// starting separation each sphere accelerates at only `4.4e-7 m/s²`, and most
    /// of the closing happens in the final third. That acceleration curve *is* the
    /// interesting part, and a wider starting separation buys more of it — free-fall
    /// time scales as `d^{3/2}`, so this separation gives far longer to watch than
    /// a close pair would.
    ///
    /// # The spheres now bounce, so the scene never ends
    ///
    /// The two spheres are solid: their triangle hulls meet when their centres are
    /// `2 · TWO_BODY_VISUAL_RADIUS_UNITS` apart and
    /// [`crate::collision::resolve::resolve_contacts`] resolves the contact
    /// **perfectly elastically**. Equal masses meeting head-on exchange velocities,
    /// so they leave the collision as fast as they arrived, climb back out against
    /// their mutual pull, stall, and fall again — an oscillation that repeats for as
    /// long as the window is open. The visual radius is no longer decoration; it is
    /// where the bodies stop.
    ///
    /// Two things to keep in mind about the numbers:
    ///
    /// - **The bounce is central and frictionless**, and nothing spins: `Prop` has
    ///   no angular velocity. See `src/collision/README.md` for the full list of
    ///   what the response does not model.
    /// - **Successive bounces will not be identical.** The integrator is
    ///   first-order and the positional separation that removes penetration is not
    ///   momentum conserving, so a little energy leaks in or out at each encounter
    ///   and the oscillation's amplitude drifts. The infall is still the part worth
    ///   watching; the bounce is what stops it being a lie.
    ///
    /// Gravitational softening (see
    /// [`crate::core::constants::GRAVITY_SOFTENING_UNITS`]) still matters, because
    /// props with no hull — a zero-width line, a massless prop — can still coincide
    /// with something and must not receive an infinite force.
    pub fn two_body_gravity() -> Self {
        let mut scene = Self::demo();

        let half_separation = 0.5 * TWO_BODY_SEPARATION_UNITS;
        let height = TWO_BODY_HEIGHT_UNITS;

        // Both at rest: no tangential component, so no angular momentum and no
        // orbit. Everything that follows is produced by gravitation alone.
        scene.add(Prop::at_rest(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::new(-f64::from(half_separation), f64::from(height), 0.0),
                    rotation: Quat::IDENTITY,
                },
                radius: TWO_BODY_VISUAL_RADIUS_UNITS,
                color: TWO_BODY_LEFT_COLOR,
            },
            EARTH_MASS_KG,
        ));

        scene.add(Prop::at_rest(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::new(f64::from(half_separation), f64::from(height), 0.0),
                    rotation: Quat::IDENTITY,
                },
                radius: TWO_BODY_VISUAL_RADIUS_UNITS,
                color: TWO_BODY_RIGHT_COLOR,
            },
            EARTH_MASS_KG,
        ));

        scene
    }

    /// Simulated time for the two spheres of [`Scene::two_body_gravity`] to meet,
    /// in seconds.
    ///
    /// The radial free-fall time `t = π·√(d³ / (8·G·(m₁+m₂)))`, computed from
    /// [`GRAVITATIONAL_CONSTANT`] and the scene's own constants so it stays correct
    /// if either is edited. Used to derive `SceneConfig::time_scale` and to size
    /// the integrator's tests.
    ///
    /// This ignores softening, which slightly *shortens* the real answer by
    /// weakening the pull at close range; the difference is under a percent at
    /// these separations.
    ///
    /// [`GRAVITATIONAL_CONSTANT`]: crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT
    pub fn two_body_free_fall_seconds() -> f64 {
        use crate::physics_utils::physics_constants::GRAVITATIONAL_CONSTANT;

        let separation_m = TWO_BODY_SEPARATION_UNITS as f64 * METRES_PER_UNIT;
        let total_mass = 2.0 * EARTH_MASS_KG as f64;

        std::f64::consts::PI
            * (separation_m.powi(3) / (8.0 * GRAVITATIONAL_CONSTANT * total_mass)).sqrt()
    }

    /// Append `prop` to the scene and return its freshly-allocated `BodyId`.
    ///
    /// Contract (Requirements 4.3, 4.4, 4.5, 4.6, 7.8, 14.3, 14.4):
    ///
    /// - In debug builds, fires `debug_assert!` with a descriptive message if
    ///   `prop` does not satisfy the Validation_Rules. In release builds the
    ///   prop is appended unconditionally (no panic — see Requirement 14.4).
    /// - Appends `prop` so `self.props.len()` grows by exactly 1, the prefix
    ///   of `self.props` up to the old length is unchanged, and
    ///   `self.props.last() == Some(&prop)`.
    /// - Returns a fresh `BodyId` (monotonically increasing via `next_id`)
    ///   that has never been returned by a prior successful `add` on this
    ///   `Scene`. `self.get(returned_id) == Some(&prop)` immediately after.
    #[inline]
    pub fn add(&mut self, prop: Prop) -> BodyId {
        debug_assert!(
            prop.is_valid(),
            "Scene::add: prop fails Validation_Rules: {:?}",
            prop
        );
        let id = BodyId(self.next_id);
        // Pre-increment so a future `add` always hands out a strictly-greater
        // id, even if a caller ignores the one we just returned. `u64` is
        // large enough that overflow is not a practical concern.
        self.next_id += 1;
        self.props.push(prop);
        // `get` binary-searches `ids`, which this keeps sorted.
        debug_assert!(self.ids.last().is_none_or(|last| *last < id));
        self.ids.push(id);
        self.revision = next_revision();
        id
    }

    /// Look up the prop associated with `id`.
    ///
    /// Returns `Some(&prop)` when `id` was returned by a prior successful
    /// `add` on this `Scene` and the prop has not been removed; otherwise
    /// `None`. Never mutates `self` (Requirements 4.5, 4.6).
    ///
    /// A binary search over `ids`, O(log n). `ids` is always sorted: [`add`](Self::add) hands out
    /// ids in increasing order and appends them, and [`remove`](Self::remove) keeps the order of
    /// what is left. It was a linear search, which was cheaper for a handful of props and became
    /// the frame's cost once a planet was built from thousands of particles: the render bridge
    /// looks every body up once a frame, which made the frame O(n²).
    #[inline]
    /// Which set of props this is; see the field's docs. Changes whenever a prop is added or removed.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn get(&self, id: BodyId) -> Option<&Prop> {
        self.index_of(id).map(|index| &self.props[index])
    }

    /// Where `id` sits in `props`, by binary search over the sorted `ids`.
    #[inline]
    fn index_of(&self, id: BodyId) -> Option<usize> {
        self.ids.binary_search(&id).ok()
    }

    /// Yield every `(BodyId, &Prop)` pair currently in the scene.
    ///
    /// Iteration order matches insertion order (Requirement 4.7). Never
    /// mutates `self`. `ids` and `props` are maintained in lockstep by
    /// `add`, so zipping them pairs each id with its prop.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (BodyId, &Prop)> + '_ {
        self.ids.iter().copied().zip(self.props.iter())
    }

    /// Remove the prop with this `BodyId`, returning it if it was there.
    ///
    /// # The one operation that breaks a postcondition on purpose
    ///
    /// Requirement 5.4 said every `BodyId` resolvable before a step is still resolvable
    /// after it. That was true while nothing could leave the scene. Absorption — a black
    /// hole swallowing a star — is exactly a prop leaving, so **5.4 is amended by A14**:
    /// an id may stop resolving, and once it does it never resolves again, because
    /// `next_id` only ever increases and is never reused.
    ///
    /// Everything downstream was written for this. The render mirror despawns entities
    /// whose id has gone (`sync_mirror_entities_system`), and `Trails::record_from` already
    /// dropped trails for departed ids before anything could depart — that code says as
    /// much in its own docs.
    ///
    /// # Contract
    ///
    /// - `props` and `ids` stay in lockstep, which is this type's central invariant.
    /// - Relative order of the surviving props is preserved, so index-based reasoning
    ///   inside one `step` stays valid as long as it re-reads after a removal.
    /// - Every *other* id still resolves to the same prop it did before.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::Scene;
    ///
    /// let mut scene = Scene::two_body_gravity();
    /// let (id, _) = scene.iter().nth(1).unwrap();
    ///
    /// assert!(scene.remove(id).is_some());
    /// assert!(scene.get(id).is_none(), "an id never comes back");
    /// assert!(scene.remove(id).is_none(), "removing twice is not an error");
    /// assert_eq!(scene.props.len(), 2);
    /// ```
    pub fn remove(&mut self, id: BodyId) -> Option<Prop> {
        let index = self.index_of(id)?;

        // `remove` rather than `swap_remove`: order is part of what callers see, both in
        // the overlay's listing and in the `(i, j)` pair order the contact pass walks.
        self.ids.remove(index);
        let prop = self.props.remove(index);
        self.revision = next_revision();

        debug_assert_eq!(
            self.ids.len(),
            self.props.len(),
            "Scene::remove: ids and props must stay in lockstep"
        );

        Some(prop)
    }

    /// Advance the scene by `dt` seconds. This is the reserved physics
    /// integration seam; today it is a no-op placeholder.
    ///
    /// Contract (Requirements 5.1, 5.2, 5.3, 5.4, 5.5):
    ///
    /// - **Precondition (5.5)**: in a debug build, fires `debug_assert!`
    ///   with a descriptive message when `dt < 0.0` or `!dt.is_finite()`.
    ///   Release builds do not panic.
    /// - **Postcondition (5.1)**: every `Prop` in `self.props` still
    ///   satisfies the Validation_Rules after the call.
    /// - **Postcondition (5.2)**: when `dt == 0.0`, `self.props` is
    ///   byte-identical (`==`) to its pre-call value.
    /// - **Postcondition (5.3)**: `self.bounds` is unchanged.
    /// - **Postcondition (5.4)**, *as amended by A14*: every `BodyId` previously
    ///   resolvable by `self.get(id)` still resolves to `Some` after the call **unless it
    ///   was absorbed**, and an id that has been absorbed never resolves again. Before
    ///   absorption existed this held unconditionally; see [`Scene::remove`].
    ///
    /// # What it does now
    ///
    /// Two things per step, in this order, repeated over
    /// [`substep_count`](crate::collision::resolve::substep_count) substeps:
    ///
    /// 1. **Mutual gravitation**, delegated to
    ///    [`crate::core::integrator::integrate_gravity`]: every prop attracts
    ///    every other in proportion to their masses, and each movable prop's
    ///    velocity and position are advanced by the result. Immovable props are
    ///    skipped, so the ground plane stays put and keeps its zero-velocity
    ///    invariant.
    /// 2. **Absorption**, delegated to
    ///    [`crate::collision::absorb::absorb_contacts`]: a prop touching one whose
    ///    [`absorbs`](Prop::absorbs) flag is set is merged into it — mass and momentum
    ///    conserved — and **leaves the scene**. Runs before the elastic pass so nothing
    ///    bounces off a black hole on its way in.
    /// 3. **Contact**, delegated to
    ///    [`crate::collision::resolve::resolve_contacts`]: any two props whose
    ///    triangle hulls overlap bounce elastically and are pushed apart. Props
    ///    are solid — they no longer pass through each other.
    ///
    /// # Why substeps
    ///
    /// Contact is tested at substep boundaries, so a prop that crosses more than
    /// the thickness of what it is hitting within one step could arrive on the far
    /// side untouched. `substep_count` splits `dt` until nothing travels further
    /// than
    /// [`MAX_SUBSTEP_DISPLACEMENT_UNITS`](crate::collision::constants::MAX_SUBSTEP_DISPLACEMENT_UNITS)
    /// per substep, capped at
    /// [`MAX_SUBSTEPS`](crate::collision::constants::MAX_SUBSTEPS).
    ///
    /// For the shipped scene the answer is always `1`: the two-body pair closes at
    /// only `3.5e-7` units per second each, or `0.029` units per 60 Hz frame, so
    /// the loop runs once and costs nothing. It earns its place in the scenes this
    /// one leads to, where the alternative is contact quietly failing to happen.
    ///
    /// # `dt == 0.0` returns immediately
    ///
    /// Not just as an optimisation — it is what Postcondition 5.2 requires. A zero
    /// step must leave `props` byte-identical, and the contact pass is a function
    /// of *position*, not of `dt`: given an already-overlapping pair it would
    /// separate them however small the timestep. Integration is genuinely a no-op
    /// at `dt == 0.0`; resolution is not, so the guard is on the whole step.
    ///
    /// The seam is no longer a placeholder, but it is still a seam: the schedule
    /// around it (`clamp_dt` → `step` → `sync_scene_to_ecs`) is unchanged, so
    /// swapping in a symplectic integrator or a third-party engine touches only
    /// `core/integrator.rs` and `collision/`.
    #[inline]
    pub fn step(&mut self, step: TimeStep) {
        self.step_observed(step, |_| {});
    }

    /// [`Scene::step`], calling `after_substep` with the scene after every substep.
    ///
    /// For observers that need the path between frames rather than only where a frame ends:
    /// one frame of the solar system is `11%` of Io's orbit, so a trail sampled once per frame
    /// cannot draw Io's path. The scene stays free of any render type, since the observer is
    /// a closure and nothing here knows what it records.
    pub fn step_observed(&mut self, step: TimeStep, mut after_substep: impl FnMut(&Scene)) {
        debug_assert!(
            step.is_valid(),
            "Scene::step: TimeStep must be finite and non-negative, got {:?}",
            step
        );

        if step.is_zero() || !step.is_valid() {
            return;
        }

        let substeps = self.substeps(step);
        let substep = step.split(substeps);

        for _ in 0..substeps {
            self.substep(substep);
            after_substep(self);
        }
    }

    /// [`Scene::step_observed`], with a controller that acts on the scene before each substep.
    ///
    /// For a craft whose state the scene cannot carry by itself. A powered craft is tracked in
    /// `f64` by its voyage, because near a landing it moves centimetres per substep, which `f32`
    /// positions a hundred units from the origin round away entirely; `before_substep` writes that
    /// state into the scene, and fires its engine, before gravity moves anything.
    ///
    /// The substeps are exactly the ones [`Scene::step_observed`] takes, so the rest of the scene
    /// moves as it would with no controller. Both callbacks are handed `state`, so a controller
    /// that both steers and records (a voyage does) can be borrowed by each in turn instead of
    /// captured mutably by two closures at once.
    pub fn step_steered<S>(
        &mut self,
        step: TimeStep,
        state: &mut S,
        mut before_substep: impl FnMut(&mut S, &mut Scene, TimeStep),
        mut after_substep: impl FnMut(&mut S, &Scene),
    ) {
        debug_assert!(
            step.is_valid(),
            "Scene::step_steered: TimeStep must be finite and non-negative, got {:?}",
            step
        );

        if step.is_zero() || !step.is_valid() {
            return;
        }

        let substeps = self.substeps(step);
        let substep = step.split(substeps);

        for _ in 0..substeps {
            before_substep(state, self, substep);
            self.substep(substep);
            after_substep(state, self);
        }
    }

    /// How many substeps `step` is split into.
    fn substeps(&self, step: TimeStep) -> u32 {
        match &self.wormhole {
            Some(space) => space.substeps(&self.props, step.seconds()),
            None => crate::collision::resolve::substep_count(&self.props, step),
        }
    }

    /// One substep: simulated contact, gravity, then absorption, then merging, then the bounce.
    fn substep(&mut self, substep: TimeStep) {
        if let Some(mut space) = self.wormhole.take() {
            space.coast(self, substep);
            self.wormhole = Some(space);
            return;
        }
        // Read once: `integrate_gravity` takes `&mut self.props`, so the field cannot be
        // borrowed from `self` inside the call.
        let softening = self.softening_units;
        let model = self.gravity_model;
        // The simulated contacts push first, from the positions the substep starts at, and then
        // gravity kicks and drifts: together one symplectic Euler step. Only particles of bodies
        // built from particles are touched; in any other scene this is one pass that finds none.
        let contacts = crate::collision::collision::push_contacts(&mut self.props, substep);
        crate::core::integrator::integrate_gravity_with(&mut self.props, substep, softening, model);
        // The heat the contacts made, now that the substep's velocities are final: see
        // `push_contacts` for why it waits for gravity.
        contacts.heat(&mut self.props, substep);
        // Absorption first, and that ordering is load-bearing: a prop swallowed by a
        // black hole is gone before the elastic pass can see it, so nothing bounces off
        // an event horizon on its way in. Both passes use the same overlap test, so
        // there is no window in which one runs and the other does not.
        crate::collision::absorb::absorb_contacts(self);
        // Then merging, for the same reason: two planets that cannot escape each other become
        // one body (and debris) before the bounce could send them apart. It acts only on pairs
        // whose contact loses energy, so a scene whose props all keep the default elastic
        // material is untouched by it.
        crate::collision::collision::merge_contacts(self);
        crate::collision::resolve::resolve_contacts(&mut self.props);
    }
}

/// The primary that `id` orbits as a moon, and the moon's distance from it in world units, or
/// `None` if `id` is not a moon.
///
/// Lives in `core` because it reads nothing but the scene: trails use it to sample moons more
/// finely, and transfer planning uses it to tell which moon systems a flight passes through.
///
/// A body is a moon of `P` when it lies inside `P`'s Hill sphere, measured against the
/// scene's most massive movable body, and weighs at most [`MOON_MAX_MASS_RATIO`] of
/// `P`. The Hill sphere is the region where `P`'s gravity holds a satellite against the
/// dominant body's pull, and plain nearest-strongest-pull is wrong here: the Sun pulls
/// Earth's Moon twice as hard as Earth does. The most massive body is never a moon and never
/// a primary, and neither is an immovable prop. When a body sits inside several Hill spheres,
/// the nearest primary wins.
pub fn moon_primary(scene: &Scene, id: BodyId) -> Option<(BodyId, f64)> {
    let moon = scene.get(id).filter(|prop| !prop.immovable)?;
    let movable = || scene.iter().filter(|(_, prop)| !prop.immovable);
    let (dominant_id, dominant) =
        movable().max_by(|(_, a), (_, b)| a.mass_kg.total_cmp(&b.mass_kg))?;
    if dominant_id == id {
        return None;
    }
    let dominant_at = dominant.body.center();
    let moon_at = moon.body.center();

    movable()
        .filter(|(primary_id, primary)| {
            *primary_id != id
                && *primary_id != dominant_id
                && moon.mass_kg <= MOON_MAX_MASS_RATIO * primary.mass_kg
        })
        .filter_map(|(primary_id, primary)| {
            let primary_at = primary.body.center();
            let hill_radius = primary_at.distance(dominant_at)
                * (f64::from(primary.mass_kg) / (3.0 * f64::from(dominant.mass_kg))).cbrt();
            let distance = moon_at.distance(primary_at);
            (distance < hill_radius).then_some((primary_id, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
#[path = "../../tst/core/scene_tests.rs"]
mod tests;
