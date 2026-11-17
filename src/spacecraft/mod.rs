//! Spacecraft: what a craft is, how it plans a flight, how it flies it, and what the flight cost.
//!
//! The layer above `core` that decides *how* a craft gets from one body to another. `core`
//! answers the orbital mechanics (where will each body be, what coasts from here to there);
//! this layer chooses when to burn, how hard, and toward what. Like `core`, it imports nothing
//! from Bevy but `bevy::math`.
//!
//! - [`config`]: [`Spacecraft`], [`Propulsion`], [`Engine`], [`Stage`], and the [`Fleet`] `G`
//!   cycles through. Plain data, so a settings menu can edit it.
//! - [`rocket`]: [`Rocket`], a craft that carries its propellant. The rocket equation, tanks that
//!   drain as it burns, stages dropped when empty, and gliding once dry.
//! - [`plan`]: [`plan_flight`], by propulsion. The default, crewed craft flies the least-energy
//!   orbital transfer, found with [`lambert`] and solved in full by `core`.
//! - [`guidance`]: the steering laws a powered craft flies by.
//! - [`flight`]: [`Flight`], a craft in the live scene.
//! - [`report`]: [`FlightReport`], what the arrival pop-up shows.
//!
//! The constants every built-in craft is assembled from are in [`constants`].

pub mod config;
pub mod constants;
pub mod flight;
pub mod guidance;
pub mod lambert;
pub mod plan;
pub mod report;
pub mod rocket;

pub use config::{Engine, Fleet, Propulsion, Spacecraft, Stage};
pub use flight::{Flight, Phase};
pub use plan::{
    central_body, departure_burn, departure_burn_with, plan_correction_for, plan_flight,
    plan_transfer_correction, surface_gravity_m_s2, FlightPlan, NoFlight, Push,
};
pub use report::FlightReport;
pub use rocket::{rocket_delta_v_m_s, rocket_mass_after_kg, Burn, Rocket};
