//! Interstellar objects you can place into a running experiment: dwarfs, a neutron star,
//! and black holes.
//!
//! [`crate::experiments::constants::STELLAR_OBJECTS`] is the catalog and
//! [`StellarObject::to_prop`] turns a row into something the simulation can hold. Placing
//! one is the `B` key — see [`crate::input::placement`] for the picker and
//! [`crate::render::placement`] for the spawn.
//!
//! # These objects span twelve orders of magnitude in radius, and none of it is scaled
//!
//! | Object | Mass | True radius | In units |
//! | --- | --- | --- | --- |
//! | Red dwarf | `0.122 M☉` | `1.07e8 m` | `0.107` |
//! | White dwarf | `1.018 M☉` | `5.85e6 m` | `0.00585` |
//! | Neutron star | `1.4 M☉` | `1.10e4 m` | `1.1e-5` |
//! | Black hole | `10 M☉` | `2.95e4 m` | `2.95e-5` |
//! | Sgr A* | `4.3e6 M☉` | `1.27e10 m` | `12.69` |
//!
//! Until amendment A16 this table also carried a *drawn* radius, and needed three separate rules to
//! fill it in: the dwarfs on the planets' `157×` exaggeration, the neutron star and stellar hole
//! **floored** at `0.30` and `0.806` units because `1.1e-5` is invisible, and Sgr A* alone at true
//! scale. All three were working around one thing — the drawn radius was also the collision radius.
//!
//! Bodies are now stored *and drawn* at their true size, and nothing compensates for it (A17): a
//! sub-pixel body is not drawn at all, so a neutron star is the `1.1e-5` units it really is and you
//! have to reach it to see it. Three regimes collapsed into none, and four methods that existed only
//! to reason about the exaggeration went with them.
//!
//! # A black hole captures at its tidal radius, not its horizon
//!
//! The old drawn radius doubled as the absorption radius, which meant a `10 M☉` hole swallowed
//! things at `27,000×` its event horizon. The honest replacement is not the horizon either — matter
//! is destroyed at the **tidal disruption radius**, where the hole's gravity gradient exceeds the
//! infalling body's own self-gravity:
//!
//! ```text
//! R_t ≈ R_star · (M_hole / M_star)^{1/3}
//! ```
//!
//! For a solar-type star that is `1.5e9 m` for the `10 M☉` hole — `50,000×` its `2.95e4 m` horizon
//! — and `113` units for Sgr A*, comfortably outside its `12.7`-unit horizon, so a star is torn
//! apart well before it could cross. [`StellarObject::capture_radius_m`] carries it, and it reaches
//! the simulation through
//! [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units). So absorption
//! stayed observable *because* it became more accurate.
//!
//! # Newtonian gravity is defensible here, but only just, and not everywhere
//!
//! At the separations this crate can show — a unit is `1e9 m` — orbital speeds around a
//! stellar-mass compact object are a fraction of a percent of `c`: `0.004 c` at one unit
//! from a `10 M☉` black hole. Newton is fine there, and the interesting dynamics (a wide
//! binary, a body flung out of a planetary system) are all in that regime.
//!
//! Sgr A* is not. At `100` units its circular speed is `0.25 c`, where general relativity
//! is not a correction but the answer: no precession, no innermost stable circular orbit,
//! no radiated energy. The simulation will show a clean Newtonian ellipse and reality
//! would not. Recorded as a limitation of amendment A14 rather than fixed.

pub use bevy::math::{Quat, Vec3};

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Body, Color, DVec3, Pose, Prop};
use crate::experiments::constants::{SOLAR_SYSTEM_SUN_MASS_KG, STELLAR_OBJECTS};

/// One placeable object: what it weighs, how big it really is, how big it is drawn, and
/// whether it swallows what it touches.
///
/// SI in the table and world units in `drawn_radius_units`, deliberately: the first three
/// are quoted measurements and the fourth is a rendering decision. Keeping both means the
/// docs, the tests, and the menu can all state the size of the lie rather than only its
/// result.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StellarObject {
    /// Display name, shown in the placement picker: `"Neutron star"`.
    pub name: &'static str,
    /// Mass in kilograms. `f64` because Sgr A* is `8.5e36`, and because these are quoted
    /// figures in the units they are quoted in.
    pub mass_kg: f64,
    /// True radius in metres — the physical surface, or the **Schwarzschild radius**
    /// `2GM/c²` for a black hole, which is the only radius a black hole has.
    pub radius_m: f64,
    /// The radius at which this object **destroys** what it touches, in metres, when that is
    /// not its own surface. `None` means the surface is the boundary.
    ///
    /// Set only by the black holes, and set to the **tidal disruption radius** — the distance at
    /// which a hole's gravity gradient exceeds a star's own self-gravity and pulls it apart,
    /// `R_t ≈ R_star · (M_hole / M_star)^{1/3}` for a solar-type star. That is where matter
    /// actually dies, and it is nothing like the event horizon: `1.5e9 m` against the `10 M☉`
    /// hole's `2.95e4 m` horizon, a factor of `50,000`.
    ///
    /// So this is not a visibility fudge. It is a second real radius that a black hole genuinely
    /// has, and it is the one that governs contact — which is what
    /// [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units) exists to
    /// carry. For Sgr A* it is `113` units, comfortably outside its `12.7`-unit horizon, so a
    /// star is torn apart well before it could cross.
    pub capture_radius_m: Option<f64>,
    /// Linear-space colour.
    pub color: Color,
    /// Does this object **absorb** what it touches rather than bouncing off it?
    ///
    /// True for black holes only. An elastic bounce off an event horizon is not an
    /// approximation, it is the opposite of what happens — see
    /// [`crate::collision::absorb`].
    pub absorbs: bool,
    /// One line for the picker: what makes this object worth placing.
    pub note: &'static str,
    /// Visual luminosity in **solar luminosities**, or `0.0` for an object that emits no light.
    ///
    /// Real figures: Proxima Centauri is `0.0017 L☉`, Sirius B `0.056`. A neutron star radiates
    /// mostly in X-rays, so its *visible* output is around `1e-5` — bright per unit area and
    /// negligible in total, because it is `11 km` across. Black holes emit nothing at all: an
    /// accretion disc would, and this crate does not model one.
    ///
    /// Reaches the renderer through
    /// [`Prop::luminosity_solar`](crate::core::scene::Prop::luminosity_solar), which becomes a point
    /// light at the object's own position — so **placing a star lights the scene**, and placing a
    /// black hole does not.
    pub luminosity_solar: f32,
}

impl StellarObject {
    /// Validation_Rules for `StellarObject`: positive finite mass, true radius, and drawn
    /// radius; a valid colour; a non-empty name and note.
    ///
    /// A zero drawn radius would fail [`Body::is_valid`] and take the scene with it, which
    /// is why it is checked over the whole table rather than at the point of placement.
    pub fn is_valid(&self) -> bool {
        let positive = |value: f64| value.is_finite() && value > 0.0;

        !self.name.is_empty()
            && !self.note.is_empty()
            && positive(self.mass_kg)
            && positive(self.radius_m)
            && self
                .capture_radius_m
                .is_none_or(|radius| radius.is_finite() && radius > 0.0)
            && self.luminosity_solar.is_finite()
            && self.luminosity_solar >= 0.0
            && self.color.is_valid()
    }

    /// The object's mass in solar masses — how these figures are usually quoted.
    #[inline]
    pub fn solar_masses(&self) -> f64 {
        self.mass_kg / f64::from(SOLAR_SYSTEM_SUN_MASS_KG)
    }

    /// The object's radius in world units — its true size, which is also what it is drawn at.
    #[inline]
    pub fn radius_units(&self) -> f32 {
        (self.radius_m / METRES_PER_UNIT) as f32
    }

    /// The radius at which this object destroys what it touches, in world units.
    ///
    /// [`capture_radius_m`](Self::capture_radius_m) when set, otherwise the object's own
    /// surface.
    #[inline]
    pub fn contact_radius_units(&self) -> f32 {
        match self.capture_radius_m {
            Some(radius) => (radius / METRES_PER_UNIT) as f32,
            None => self.radius_units(),
        }
    }

    /// Build the prop for this object at `position`, at rest.
    ///
    /// # At rest, and why
    ///
    /// A placed object gets no velocity: it is dropped into whatever gravitational field is
    /// already there and falls from rest, which is the one initial condition that needs no
    /// further choices from the person placing it. Anything else — matching the camera's
    /// motion, or a circular orbit about the nearest mass — is a guess about intent, and
    /// both are things the object can be *given* later by a richer placement UI.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::{stellar_object, DVec3, Vec3};
    ///
    /// let hole = stellar_object("Black hole").unwrap();
    /// let prop = hole.to_prop(DVec3::new(0.0, 30.0, 0.0));
    ///
    /// assert!(prop.is_valid());
    /// assert!(prop.absorbs, "a black hole swallows rather than bounces");
    /// assert_eq!(prop.velocity_units_s, DVec3::ZERO);
    /// ```
    pub fn to_prop(&self, position: DVec3) -> Prop {
        debug_assert!(
            self.is_valid(),
            "StellarObject::to_prop: object fails Validation_Rules: {:?}",
            self
        );

        let body = Body::Sphere {
            pose: Pose {
                position,
                rotation: Quat::IDENTITY,
            },
            // The object's **true** radius, and nothing else. A neutron star is stored at its
            // real `1.1e-5` units rather than the `0.30`-unit stand-in it used to need — a
            // stand-in that also had to serve as its collision radius, which is what made the
            // whole arrangement untenable.
            radius: self.radius_units(),
            color: self.color,
        };

        let mass_kg = self.mass_kg as f32;

        let prop = if self.absorbs {
            Prop::absorbing(body, DVec3::ZERO, mass_kg)
        } else {
            Prop::at_rest(body, mass_kg)
        };

        // A black hole's contact radius is its tidal disruption radius, not its surface — see
        // `capture_radius_m`. Everything else touches at its surface, so the override is only
        // set where the two genuinely differ.
        let prop = if self.capture_radius_m.is_some() {
            prop.with_contact_radius(self.contact_radius_units())
        } else {
            prop
        };

        prop.with_luminosity(self.luminosity_solar)
    }
}

/// The object with this name, or `None`.
///
/// Case-insensitive and trimmed, matching [`ExperimentId::matches`](crate::ExperimentId).
/// Exists so a test — or a future command-line flag — can name an object without indexing
/// into the table and depending on its order.
///
/// # Examples
///
/// ```
/// use physics_sandbox::stellar_object;
///
/// assert!(stellar_object("neutron star").is_some());
/// assert!(stellar_object("  Sgr A*  ").is_some());
/// assert!(stellar_object("Death Star").is_none());
/// ```
pub fn stellar_object(name: &str) -> Option<&'static StellarObject> {
    let wanted = name.trim().to_ascii_lowercase();

    STELLAR_OBJECTS
        .iter()
        .find(|object| object.name.to_ascii_lowercase() == wanted)
}

#[cfg(test)]
#[path = "../../tst/experiments/objects_tests.rs"]
mod tests;
