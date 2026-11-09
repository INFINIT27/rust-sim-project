//! Spacecraft: what a craft is, how it plans a flight, how it flies it, and what the flight cost.
//!
//! The layer above `core` that decides *how* a craft gets from one body to another. `core`
//! answers the orbital mechanics (where will each body be, what coasts from here to there);
//! this layer chooses when to burn, how hard, and toward what. Like `core`, it imports nothing
//! from Bevy but `bevy::math`.
//!
//! - [`config`]: [`Spacecraft`], [`Propulsion`], [`Engine`], and the [`Fleet`] `G` cycles through.
//!   Plain data, so a settings menu can edit it.
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

pub use config::{Engine, Fleet, Propulsion, Spacecraft};
pub use flight::{Flight, Phase};
pub use plan::{
    central_body, departure_burn, plan_correction_for, plan_flight, plan_transfer_correction,
    surface_gravity_m_s2, FlightPlan, NoFlight,
};
pub use report::FlightReport;
