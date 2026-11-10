//! What a spacecraft is: its mass, size, engine, limits, and how it flies. Plain data, so a
//! settings menu can build or edit one without touching any flight code.
//!
//! [`Fleet`] is the roster `G` cycles through. Its [`Default`] is the built-in set, assembled from
//! [`crate::spacecraft::constants`]; anything else (a menu, a saved file) can build its own.

use crate::constants::METRES_PER_UNIT;
use crate::core::scene::{Body, Color, DVec3, Pose, Prop, Quat};
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::spacecraft::constants::{
    AEROJET_260_EXHAUST_SPEED_M_S, AEROJET_260_THRUST_N, CRAFT_COLOR, CRAFT_RADIUS_M,
    CREWED_CRAFT_MASS_KG, CREW_MAX_ACCELERATION_G, LAUNCH_ALTITUDE_M, PROBE_MASS_KG,
    PROBE_MAX_DEPARTURE_SPEED_M_S,
};

/// A rocket engine: how hard it pushes and how fast it throws its propellant out.
#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    /// A name for the status line and the arrival report.
    pub name: String,
    /// Full thrust, in newtons.
    pub thrust_n: f64,
    /// The speed its exhaust leaves at, in m/s: specific impulse times standard gravity. What turns
    /// a burn into propellant and energy in the [`FlightReport`](crate::spacecraft::FlightReport).
    pub exhaust_speed_m_s: f64,
}

/// How a craft gets from one body to another.
#[derive(Clone, Debug, PartialEq)]
pub enum Propulsion {
    /// Launched by an unmodelled booster at up to `max_departure_speed_m_s` (relative to the
    /// origin, once clear of its gravity), then coasting under gravity with small correction burns.
    /// It may use gravity assists. See [`crate::core::transfer::plan_departure`].
    Coasting { max_departure_speed_m_s: f64 },
    /// Steered under power the whole way: burn toward the target, flip, brake, land. The fastest
    /// way there, and the dearest.
    Direct {
        engine: Engine,
        /// The most the engine is throttled to, in m/s²; at most its thrust over the mass.
        max_acceleration_m_s2: f64,
    },
    /// The least-energy orbital transfer: burn onto the cheapest coasting arc to the target, coast
    /// on it with small correction burns, then brake and land. How real missions fly.
    Transfer {
        engine: Engine,
        /// The most the engine is throttled to, in m/s²; at most its thrust over the mass.
        max_acceleration_m_s2: f64,
    },
}

/// A spacecraft, complete: everything needed to plan and fly it.
#[derive(Clone, Debug, PartialEq)]
pub struct Spacecraft {
    /// A name for the status line and the arrival report.
    pub name: String,
    /// Its mass, in kilograms. Fixed: burning propellant is reported but not subtracted.
    pub mass_kg: f64,
    /// Its radius, in metres.
    pub radius_m: f64,
    /// The colour it and its path are drawn in.
    pub color: Color,
    /// How high above the origin's surface it starts, at rest relative to the origin, in metres.
    pub launch_altitude_m: f64,
    pub propulsion: Propulsion,
}

impl Spacecraft {
    /// The crewed default: `15 t` with an Aerojet 260 throttled to [`CREW_MAX_ACCELERATION_G`],
    /// flying the least-energy transfer.
    pub fn crewed_transfer() -> Self {
        Spacecraft {
            name: format!("Crewed transfer ({CREW_MAX_ACCELERATION_G} g)"),
            mass_kg: CREWED_CRAFT_MASS_KG,
            radius_m: CRAFT_RADIUS_M,
            color: CRAFT_COLOR,
            launch_altitude_m: LAUNCH_ALTITUDE_M,
            propulsion: Propulsion::Transfer {
                engine: Engine::aerojet_260(),
                max_acceleration_m_s2: CREW_MAX_ACCELERATION_G * STANDARD_GRAVITY_M_S2,
            },
        }
    }

    /// The same craft and engine with no crew limit: an Aerojet 260 at full thrust on `15 t`,
    /// about `178 g`, flying straight at the target.
    pub fn full_thrust_direct() -> Self {
        let engine = Engine::aerojet_260();
        let max_acceleration_m_s2 = engine.thrust_n / CREWED_CRAFT_MASS_KG;
        Spacecraft {
            name: "Full-thrust direct (uncrewed)".to_string(),
            mass_kg: CREWED_CRAFT_MASS_KG,
            radius_m: CRAFT_RADIUS_M,
            color: CRAFT_COLOR,
            launch_altitude_m: LAUNCH_ALTITUDE_M,
            propulsion: Propulsion::Direct {
                engine,
                max_acceleration_m_s2,
            },
        }
    }

    /// A Voyager-like probe: `722 kg`, launched at up to `15.4 km/s`, then coasting.
    pub fn coasting_probe() -> Self {
        Spacecraft {
            name: "Coasting probe (Voyager-like)".to_string(),
            mass_kg: PROBE_MASS_KG,
            radius_m: CRAFT_RADIUS_M,
            color: CRAFT_COLOR,
            launch_altitude_m: LAUNCH_ALTITUDE_M,
            propulsion: Propulsion::Coasting {
                max_departure_speed_m_s: PROBE_MAX_DEPARTURE_SPEED_M_S,
            },
        }
    }

    /// The engine, for a powered craft.
    pub fn engine(&self) -> Option<&Engine> {
        match &self.propulsion {
            Propulsion::Coasting { .. } => None,
            Propulsion::Direct { engine, .. } | Propulsion::Transfer { engine, .. } => Some(engine),
        }
    }

    /// The most acceleration the craft's engine gives it, in m/s²: its throttle limit, or its
    /// thrust over its mass if that is less. `None` for a coasting craft.
    pub fn acceleration_m_s2(&self) -> Option<f64> {
        match &self.propulsion {
            Propulsion::Coasting { .. } => None,
            Propulsion::Direct {
                engine,
                max_acceleration_m_s2,
            }
            | Propulsion::Transfer {
                engine,
                max_acceleration_m_s2,
            } => Some(max_acceleration_m_s2.min(engine.thrust_n / self.mass_kg)),
        }
    }

    /// Whether every number is one a flight can use: positive, finite masses, sizes, thrusts, and
    /// limits, and a non-negative launch altitude.
    pub fn is_valid(&self) -> bool {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        let propulsion = match &self.propulsion {
            Propulsion::Coasting {
                max_departure_speed_m_s,
            } => positive(*max_departure_speed_m_s),
            Propulsion::Direct {
                engine,
                max_acceleration_m_s2,
            }
            | Propulsion::Transfer {
                engine,
                max_acceleration_m_s2,
            } => {
                positive(engine.thrust_n)
                    && positive(engine.exhaust_speed_m_s)
                    && positive(*max_acceleration_m_s2)
            }
        };
        positive(self.mass_kg)
            && positive(self.radius_m)
            && self.launch_altitude_m.is_finite()
            && self.launch_altitude_m >= 0.0
            && propulsion
    }

    /// The craft as a prop, at rest at the scene's origin; launching places and aims it.
    pub fn prop(&self) -> Prop {
        Prop::new(
            Body::Sphere {
                pose: Pose {
                    position: DVec3::ZERO,
                    rotation: Quat::IDENTITY,
                },
                radius: (self.radius_m / METRES_PER_UNIT) as f32,
                color: self.color,
            },
            DVec3::ZERO,
            self.mass_kg as f32,
        )
    }
}

impl Engine {
    /// The Aerojet 260 at its SL-3 firing: `26.2 MN`, `263 s` vacuum specific impulse.
    pub fn aerojet_260() -> Self {
        Engine {
            name: "Aerojet 260".to_string(),
            thrust_n: AEROJET_260_THRUST_N,
            exhaust_speed_m_s: AEROJET_260_EXHAUST_SPEED_M_S,
        }
    }
}

/// The spacecraft `R` can launch, and which one it launches next.
#[derive(Clone, Debug, PartialEq)]
pub struct Fleet {
    craft: Vec<Spacecraft>,
    selected: usize,
}

impl Fleet {
    /// A fleet of `craft`, the first selected. `None` for an empty list or an invalid craft, since
    /// `R` must always have something it can fly.
    pub fn new(craft: Vec<Spacecraft>) -> Option<Self> {
        (!craft.is_empty() && craft.iter().all(Spacecraft::is_valid))
            .then_some(Fleet { craft, selected: 0 })
    }

    /// Every craft, in the order `G` cycles through them.
    pub fn craft(&self) -> &[Spacecraft] {
        &self.craft
    }

    /// Every craft, to edit in place, as the menu's spacecraft page does. The count cannot change
    /// through this, so the selection stays in range; an edit that leaves a craft invalid is the
    /// editor's to prevent (see [`Spacecraft::is_valid`]).
    pub fn craft_mut(&mut self) -> &mut [Spacecraft] {
        &mut self.craft
    }

    /// The craft `R` launches next.
    pub fn selected(&self) -> &Spacecraft {
        &self.craft[self.selected]
    }

    /// The index of the selected craft in [`Fleet::craft`].
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Select the next craft, wrapping round. What `G` does.
    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % self.craft.len();
    }

    /// Select craft `index`, if there is one. Returns whether it changed the selection.
    pub fn select(&mut self, index: usize) -> bool {
        let valid = index < self.craft.len();
        if valid {
            self.selected = index;
        }
        valid
    }
}

impl Default for Fleet {
    /// The built-in set: the crewed least-energy transfer (selected), the full-thrust direct
    /// flight, and the coasting probe.
    fn default() -> Self {
        Fleet {
            craft: vec![
                Spacecraft::crewed_transfer(),
                Spacecraft::full_thrust_direct(),
                Spacecraft::coasting_probe(),
            ],
            selected: 0,
        }
    }
}

#[cfg(test)]
#[path = "../../tst/spacecraft/config_tests.rs"]
mod tests;
