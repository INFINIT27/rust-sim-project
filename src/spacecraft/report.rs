//! What a flight cost and how it went: the numbers the arrival pop-up shows.

use crate::spacecraft::constants::CRAFT_MAX_TOUCHDOWN_SPEED_M_S;
use crate::spacecraft::rocket::Burn;

/// A flight's running account, kept up to date substep by substep and complete on landing.
///
/// Energy and propellant follow from the engine's exhaust speed `v_e`. Thrust `F` throws out
/// propellant at `F / v_e` per second and gives that exhaust `F · v_e / 2` of kinetic energy per
/// second, whatever the craft's own speed, so a burn of acceleration `a` for `dt` on a craft of
/// mass `m` costs `m · a · dt / v_e` of propellant and `m · a · v_e · dt / 2` of energy. The mass
/// is held fixed (the engine is idealised), so the propellant is what a craft refuelled in flight
/// would have burned, not what it would have to carry: carrying it would take the rocket equation,
/// and for a crewed transfer to Mars that is millions of tonnes.
///
/// That is the idealised craft, charged by [`record_burn`](Self::record_burn). A craft that does
/// carry its propellant (a [`Rocket`](crate::spacecraft::Rocket)) is charged by
/// [`record_rocket_burn`](Self::record_rocket_burn) instead: what the rocket equation actually
/// took from its tanks. For it the report also keeps the propellant loaded and left, the stages
/// dropped, and whether it ran dry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightReport {
    /// The craft's name.
    pub craft_name: String,
    /// Simulated seconds from launch to touchdown, once landed.
    pub travel_time_s: Option<f64>,
    /// The fastest the craft went, in the scene's frame (the Sun's, in the solar system), in m/s.
    pub max_speed_m_s: f64,
    /// Total speed change from the craft's own engine (or, for a coasting craft, its booster and
    /// correction burns), in m/s.
    pub delta_v_m_s: f64,
    /// Seconds the engine worked, as time at full thrust: the speed change over the most the engine
    /// gives. Small steering trims add their share, not their duration, so hours of gentle terminal
    /// guidance count for the few seconds of thrust they amount to.
    pub burn_time_s: f64,
    /// The largest acceleration the engine gave the craft, in m/s²: what the crew felt from it.
    pub peak_acceleration_m_s2: f64,
    /// Kinetic energy given to the exhaust, in joules. `None` for a craft with no modelled engine.
    pub energy_j: Option<f64>,
    /// Propellant burned, in kilograms. `None` for a craft with no modelled engine.
    pub propellant_kg: Option<f64>,
    /// The craft's speed relative to the target at touchdown, in m/s.
    pub touchdown_speed_m_s: Option<f64>,
    /// How many correction burns the cruise needed.
    pub corrections: usize,
    /// The propellant the craft launched with, in kilograms. `None` for a craft that does not
    /// carry its propellant (see [`crate::spacecraft::rocket`]).
    pub propellant_loaded_kg: Option<f64>,
    /// The propellant still in its tanks, in kilograms. `None` as for `propellant_loaded_kg`.
    pub propellant_left_kg: Option<f64>,
    /// How many empty stages the craft dropped.
    pub stages_dropped: usize,
    /// Whether the craft emptied its last tank in flight.
    pub ran_dry: bool,
    /// For a return trip, how close the craft came to the surface of the body it swung round, in
    /// metres. `None` for a landing.
    pub pass_altitude_m: Option<f64>,
}

impl FlightReport {
    /// Did the craft's structure survive the touchdown? `None` before it touches down.
    ///
    /// `true` when it touched down at no more than
    /// [`CRAFT_MAX_TOUCHDOWN_SPEED_M_S`],
    /// what the landing gear takes. A judgement on the touchdown speed only: the craft is not
    /// simulated breaking, and is drawn on the surface either way.
    ///
    /// # Examples
    ///
    /// ```
    /// use physics_sandbox::FlightReport;
    ///
    /// let mut report = FlightReport::new("Lander", true);
    /// assert_eq!(report.structure_held(), None);
    /// report.touchdown_speed_m_s = Some(2.1);
    /// assert_eq!(report.structure_held(), Some(true));
    /// report.touchdown_speed_m_s = Some(8.8e3);
    /// assert_eq!(report.structure_held(), Some(false));
    /// ```
    pub fn structure_held(&self) -> Option<bool> {
        self.touchdown_speed_m_s
            .map(|speed| speed <= CRAFT_MAX_TOUCHDOWN_SPEED_M_S)
    }

    /// A fresh report for `craft_name`, with a modelled engine (energy and propellant start at zero)
    /// or without one.
    pub fn new(craft_name: &str, has_engine: bool) -> Self {
        FlightReport {
            craft_name: craft_name.to_string(),
            energy_j: has_engine.then_some(0.0),
            propellant_kg: has_engine.then_some(0.0),
            ..Default::default()
        }
    }

    /// Account for the engine pushing a craft of `mass_kg` at `acceleration_m_s2` for `dt_s`, with
    /// exhaust leaving at `exhaust_speed_m_s`, on an engine that gives at most
    /// `max_acceleration_m_s2`.
    pub fn record_burn(
        &mut self,
        acceleration_m_s2: f64,
        dt_s: f64,
        mass_kg: f64,
        exhaust_speed_m_s: f64,
        max_acceleration_m_s2: f64,
    ) {
        if !(acceleration_m_s2 > 0.0 && dt_s > 0.0) {
            return;
        }
        let delta_v = acceleration_m_s2 * dt_s;
        self.delta_v_m_s += delta_v;
        self.burn_time_s += delta_v / max_acceleration_m_s2.max(acceleration_m_s2);
        self.peak_acceleration_m_s2 = self.peak_acceleration_m_s2.max(acceleration_m_s2);
        if let Some(energy) = &mut self.energy_j {
            *energy += 0.5 * mass_kg * delta_v * exhaust_speed_m_s;
        }
        if let Some(propellant) = &mut self.propellant_kg {
            *propellant += mass_kg * delta_v / exhaust_speed_m_s;
        }
    }

    /// Note that the craft carries its propellant, and launched with `propellant_kg` of it.
    pub fn load_propellant(&mut self, propellant_kg: f64) {
        self.propellant_loaded_kg = Some(propellant_kg);
        self.propellant_left_kg = Some(propellant_kg);
    }

    /// Account for a burn a rocket paid for out of its tanks (see
    /// [`Rocket::burn`](crate::spacecraft::Rocket::burn)): `burn` over `dt_s`, on a rocket that
    /// could give at most `max_acceleration_m_s2` then and has `propellant_left_kg` afterwards.
    /// The propellant and energy are the burn's own, from the rocket equation, not the fixed-mass
    /// estimate [`record_burn`](Self::record_burn) makes.
    pub fn record_rocket_burn(
        &mut self,
        burn: &Burn,
        dt_s: f64,
        max_acceleration_m_s2: f64,
        propellant_left_kg: f64,
    ) {
        self.stages_dropped += burn.stages_dropped;
        self.propellant_left_kg = Some(propellant_left_kg);
        self.ran_dry = propellant_left_kg <= 0.0;
        if !(burn.delta_v_m_s > 0.0 && dt_s > 0.0) {
            return;
        }
        let acceleration = burn.delta_v_m_s / dt_s;
        self.delta_v_m_s += burn.delta_v_m_s;
        self.burn_time_s += burn.delta_v_m_s / max_acceleration_m_s2.max(acceleration);
        self.peak_acceleration_m_s2 = self.peak_acceleration_m_s2.max(acceleration);
        if let Some(energy) = &mut self.energy_j {
            *energy += burn.energy_j;
        }
        if let Some(propellant) = &mut self.propellant_kg {
            *propellant += burn.propellant_kg;
        }
    }

    /// Account for a speed change with no modelled engine: a coasting craft's launch or correction.
    pub fn record_impulse(&mut self, delta_v_m_s: f64) {
        if delta_v_m_s.is_finite() && delta_v_m_s > 0.0 {
            self.delta_v_m_s += delta_v_m_s;
        }
    }

    /// Note the craft's speed in the scene's frame, keeping the fastest.
    pub fn record_speed(&mut self, speed_m_s: f64) {
        if speed_m_s.is_finite() {
            self.max_speed_m_s = self.max_speed_m_s.max(speed_m_s);
        }
    }
}

#[cfg(test)]
#[path = "../../tst/spacecraft/report_tests.rs"]
mod tests;
