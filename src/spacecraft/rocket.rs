//! A rocket that carries its propellant: stages, tanks, and the rocket equation.
//!
//! The idealised craft in [`crate::spacecraft::config`] have a fixed mass and an engine that never
//! runs out. A [`Rocket`] is the realistic one. It is a payload under a stack of [`Stage`]s, each
//! with a dry mass, a tank of propellant and an engine. Burning throws propellant out of the
//! lowest stage; when that stage's tank is empty the stage is dropped and the next one lights.
//! When the last tank is empty the engine is out and the craft glides.
//!
//! # The rocket equation
//!
//! An engine that throws propellant out at exhaust speed `v_e` changes the speed of a craft whose
//! mass falls from `m₀` to `m₁` by
//!
//! ```text
//! Δv = v_e · ln(m₀ / m₁)            (Tsiolkovsky)
//! ```
//!
//! whatever the thrust and however long the burn takes. Read the other way, a speed change `Δv`
//! leaves `m₁ = m₀ · exp(−Δv / v_e)`, so the propellant it costs depends on how heavy the craft is
//! when it burns. That is the whole model here: [`Rocket::burn`] takes a speed change, and the mass
//! follows from the equation, stage by stage.
//!
//! # Thrust, and why the acceleration rises
//!
//! A stage's engine pushes with a fixed thrust `F`, so the acceleration it gives is `F / m` and
//! grows as the tanks empty: a Saturn V leaves at `1.3 g` and its first stage ends above `4 g`.
//! The craft's acceleration limit caps it (the engine is throttled), so
//! [`Rocket::acceleration_m_s2`] is `min(limit, F / m)`, and zero once the rocket is dry.
//!
//! A burn of `Δv` therefore takes a time that depends on the mass. At full thrust the engine burns
//! `F / v_e` of propellant per second, so burning `Δm` takes `Δm · v_e / F` seconds; throttled to
//! the limit `a`, it takes `Δv / a`. [`Burn::seconds`] is the larger of the two, piece by piece.
//!
//! # What is left out
//!
//! - **Dropped stages vanish.** They are not added to the scene as bodies.
//! - **No atmosphere.** Every engine runs at its vacuum thrust and specific impulse.
//! - **No ullage, restart limits, boil-off or residuals.** A tank burns to empty, and an engine
//!   restarts as often as the flight asks.
//! - **Throttling is free.** Real engines such as the F-1 and J-2 could not throttle deeply; here
//!   the limit simply caps the acceleration.

use crate::spacecraft::config::{Engine, Stage};

/// The speed change, in m/s, of a craft whose mass falls from `initial_mass_kg` to `final_mass_kg`
/// by throwing propellant out at `exhaust_speed_m_s`: the Tsiolkovsky rocket equation,
/// `Δv = v_e · ln(m₀ / m₁)`.
///
/// Returns `0.0` for masses or an exhaust speed that are not positive and finite, or a final mass
/// above the initial one.
///
/// # Examples
///
/// ```
/// use physics_sandbox::rocket_delta_v_m_s;
///
/// // Throwing away 63.2% of the mass (a mass ratio of e) gives exactly the exhaust speed.
/// let delta_v = rocket_delta_v_m_s(4_000.0, std::f64::consts::E, 1.0);
/// assert!((delta_v - 4_000.0).abs() < 1.0e-9);
/// ```
pub fn rocket_delta_v_m_s(exhaust_speed_m_s: f64, initial_mass_kg: f64, final_mass_kg: f64) -> f64 {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    if !(positive(exhaust_speed_m_s) && positive(initial_mass_kg) && positive(final_mass_kg))
        || final_mass_kg > initial_mass_kg
    {
        return 0.0;
    }
    exhaust_speed_m_s * (initial_mass_kg / final_mass_kg).ln()
}

/// The mass, in kilograms, left after a craft of `initial_mass_kg` changes its speed by
/// `delta_v_m_s` with an exhaust speed of `exhaust_speed_m_s`: the rocket equation solved for the
/// final mass, `m₁ = m₀ · exp(−Δv / v_e)`.
///
/// Returns the initial mass unchanged for a speed change or exhaust speed that is not positive and
/// finite.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{rocket_delta_v_m_s, rocket_mass_after_kg};
///
/// // The two forms of the equation undo each other.
/// let left = rocket_mass_after_kg(100_000.0, 3_000.0, 4_128.0);
/// assert!((rocket_delta_v_m_s(4_128.0, 100_000.0, left) - 3_000.0).abs() < 1.0e-9);
/// ```
pub fn rocket_mass_after_kg(initial_mass_kg: f64, delta_v_m_s: f64, exhaust_speed_m_s: f64) -> f64 {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    if !(positive(delta_v_m_s) && positive(exhaust_speed_m_s)) {
        return initial_mass_kg;
    }
    initial_mass_kg * (-delta_v_m_s / exhaust_speed_m_s).exp()
}

/// What one call to [`Rocket::burn`] did.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Burn {
    /// The speed change the rocket gave, in m/s. Less than was asked for if it ran dry.
    pub delta_v_m_s: f64,
    /// The propellant thrown out, in kilograms.
    pub propellant_kg: f64,
    /// The kinetic energy given to that propellant, in joules: `½ · Δm · v_e²`.
    pub energy_j: f64,
    /// The shortest time the burn can take, in seconds: at full thrust, or at the acceleration
    /// limit where that is lower.
    pub seconds: f64,
    /// How many empty stages were dropped during the burn.
    pub stages_dropped: usize,
}

/// A rocket in flight: its payload, its stages, and how much propellant is left. See the module
/// docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Rocket {
    payload_kg: f64,
    stages: Vec<Stage>,
    /// The acceleration limit, in m/s².
    max_acceleration_m_s2: f64,
    /// The index of the stage burning now. Every stage before it has been dropped.
    current: usize,
    /// Propellant left in the current stage, in kilograms.
    propellant_kg: f64,
}

impl Rocket {
    /// A rocket with full tanks: `payload_kg` on top of `stages`, which burn in order from the
    /// first, held to `max_acceleration_m_s2`.
    pub fn new(payload_kg: f64, stages: Vec<Stage>, max_acceleration_m_s2: f64) -> Self {
        let propellant_kg = stages.first().map_or(0.0, |stage| stage.propellant_kg);
        Rocket {
            payload_kg,
            stages,
            max_acceleration_m_s2,
            current: 0,
            propellant_kg,
        }
    }

    /// The stage burning now, or the last one once the rocket is dry. `None` for a rocket with no
    /// stages.
    pub fn stage(&self) -> Option<&Stage> {
        self.stages.get(self.current).or(self.stages.last())
    }

    /// The engine of the stage burning now.
    pub fn engine(&self) -> Option<&Engine> {
        self.stage().map(|stage| &stage.engine)
    }

    /// The number of the stage burning now, counting from one, and how many there are.
    pub fn stage_number(&self) -> (usize, usize) {
        ((self.current + 1).min(self.stages.len()), self.stages.len())
    }

    /// How many stages have been dropped.
    pub fn stages_dropped(&self) -> usize {
        self.current.min(self.stages.len().saturating_sub(1))
    }

    /// The rocket's mass now, in kilograms: the payload, every stage not yet dropped, and the
    /// propellant left in them.
    pub fn mass_kg(&self) -> f64 {
        let above: f64 = self
            .stages
            .iter()
            .skip(self.current + 1)
            .map(|stage| stage.dry_mass_kg + stage.propellant_kg)
            .sum();
        let burning = self
            .stages
            .get(self.current)
            .map_or(0.0, |stage| stage.dry_mass_kg + self.propellant_kg);
        self.payload_kg + above + burning
    }

    /// The propellant left in every stage, in kilograms.
    pub fn propellant_left_kg(&self) -> f64 {
        let above: f64 = self
            .stages
            .iter()
            .skip(self.current + 1)
            .map(|stage| stage.propellant_kg)
            .sum();
        self.propellant_kg + above
    }

    /// Whether every tank is empty.
    pub fn is_dry(&self) -> bool {
        self.propellant_left_kg() <= 0.0
    }

    /// The most acceleration the rocket can give itself now, in m/s²: the current stage's thrust
    /// over the current mass, capped at the limit. Zero when dry.
    pub fn acceleration_m_s2(&self) -> f64 {
        if self.is_dry() {
            return 0.0;
        }
        self.full_thrust_acceleration_m_s2()
            .min(self.max_acceleration_m_s2)
    }

    /// The current stage's thrust over the current mass, in m/s², whether or not any propellant is
    /// left.
    fn full_thrust_acceleration_m_s2(&self) -> f64 {
        self.stage()
            .map_or(0.0, |stage| stage.engine.thrust_n / self.mass_kg())
    }

    /// The acceleration the last stage gives when it lights with full tanks, capped at the limit,
    /// in m/s²: the weakest the rocket can be when it lands on that stage.
    pub fn last_stage_acceleration_m_s2(&self) -> f64 {
        self.stages.last().map_or(0.0, |stage| {
            let mass = self.payload_kg + stage.dry_mass_kg + stage.propellant_kg;
            (stage.engine.thrust_n / mass).min(self.max_acceleration_m_s2)
        })
    }

    /// The least acceleration the rocket will be able to give at any moment of its next
    /// `delta_v_m_s` of burning, in m/s²: now, or as a later stage lights within that burn. Within
    /// a stage the acceleration only rises, so the weakest moments are the ignitions.
    ///
    /// What a landing must be planned on. A rocket that starts braking on a strong stage and
    /// finishes on a weak one has to leave room for the weak one from the start. A burn the tanks
    /// cannot cover is judged on the stages there are; zero only when the rocket is already dry.
    pub fn weakest_acceleration_m_s2(&self, delta_v_m_s: f64) -> f64 {
        let mut probe = self.clone();
        let mut weakest = probe.acceleration_m_s2();
        let mut left = delta_v_m_s;
        while left > 0.0 && !probe.is_dry() {
            // To the end of the current stage, then one step into the next to light it.
            let mass = probe.mass_kg();
            let in_stage = probe.engine().map_or(0.0, |engine| {
                rocket_delta_v_m_s(engine.exhaust_speed_m_s, mass, mass - probe.propellant_kg)
            });
            if in_stage >= left {
                break;
            }
            probe.burn(in_stage);
            left -= in_stage;
            if probe.burn(f64::MIN_POSITIVE).stages_dropped == 0 && probe.propellant_kg <= 0.0 {
                break;
            }
            weakest = weakest.min(probe.acceleration_m_s2());
        }
        weakest
    }

    /// The speed change the propellant left can give, in m/s: the rocket equation over each stage
    /// in turn, each one carrying everything above it.
    pub fn delta_v_left_m_s(&self) -> f64 {
        let mut probe = self.clone();
        probe.burn(f64::INFINITY).delta_v_m_s
    }

    /// Burn for a speed change of `delta_v_m_s`, draining the tanks by the rocket equation and
    /// dropping each stage as it empties. Returns what was done; the speed change is short of what
    /// was asked for if the rocket ran dry. See the module docs.
    pub fn burn(&mut self, delta_v_m_s: f64) -> Burn {
        let mut done = Burn::default();
        if delta_v_m_s.is_nan() || delta_v_m_s <= 0.0 {
            return done;
        }
        let mut wanted = delta_v_m_s;
        while wanted > 0.0 && self.current < self.stages.len() {
            if self.propellant_kg <= 0.0 {
                // An empty stage is dropped only when there is another to light.
                if self.current + 1 >= self.stages.len() {
                    break;
                }
                self.current += 1;
                self.propellant_kg = self.stages[self.current].propellant_kg;
                done.stages_dropped += 1;
                continue;
            }
            let (exhaust, thrust) = {
                let engine = &self.stages[self.current].engine;
                (engine.exhaust_speed_m_s, engine.thrust_n)
            };
            let mass = self.mass_kg();
            let available = rocket_delta_v_m_s(exhaust, mass, mass - self.propellant_kg);
            let (delta_v, burned) = if wanted >= available {
                (available, self.propellant_kg)
            } else {
                (wanted, mass - rocket_mass_after_kg(mass, wanted, exhaust))
            };
            self.propellant_kg = (self.propellant_kg - burned).max(0.0);
            wanted -= delta_v;
            done.delta_v_m_s += delta_v;
            done.propellant_kg += burned;
            done.energy_j += 0.5 * burned * exhaust * exhaust;
            // At full thrust the engine burns `F / v_e` a second; the limit may hold it slower.
            done.seconds += (burned * exhaust / thrust).max(delta_v / self.max_acceleration_m_s2);
        }
        done
    }

    /// The time, in seconds, a planner should allow for a burn of `delta_v_m_s`, draining the
    /// tanks as [`burn`](Self::burn) does.
    ///
    /// Unlike [`burn`](Self::burn) it never stops short: past the last drop of propellant it
    /// carries on at the acceleration the rocket had as it ran dry. A plan made this way is one
    /// the rocket may not be able to finish, and the live flight is what runs dry. That is
    /// deliberate: a flight is always planned, and the propellant decides how far it gets.
    pub fn plan_burn(&mut self, delta_v_m_s: f64) -> f64 {
        if delta_v_m_s.is_nan() || delta_v_m_s <= 0.0 {
            return 0.0;
        }
        let done = self.burn(delta_v_m_s);
        let short = delta_v_m_s - done.delta_v_m_s;
        if short <= 0.0 {
            return done.seconds;
        }
        let acceleration = self
            .full_thrust_acceleration_m_s2()
            .min(self.max_acceleration_m_s2);
        done.seconds + short / acceleration.max(f64::MIN_POSITIVE)
    }
}

#[cfg(test)]
#[path = "../../tst/spacecraft/rocket_tests.rs"]
mod tests;
