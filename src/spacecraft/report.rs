//! What a flight cost and how it went: the numbers the arrival pop-up shows.

/// A flight's running account, kept up to date substep by substep and complete on landing.
///
/// Energy and propellant follow from the engine's exhaust speed `v_e`. Thrust `F` throws out
/// propellant at `F / v_e` per second and gives that exhaust `F · v_e / 2` of kinetic energy per
/// second, whatever the craft's own speed, so a burn of acceleration `a` for `dt` on a craft of
/// mass `m` costs `m · a · dt / v_e` of propellant and `m · a · v_e · dt / 2` of energy. The mass
/// is held fixed (the engine is idealised), so the propellant is what a craft refuelled in flight
/// would have burned, not what it would have to carry: carrying it would take the rocket equation,
/// and for a crewed transfer to Mars that is millions of tonnes.
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
}

impl FlightReport {
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
