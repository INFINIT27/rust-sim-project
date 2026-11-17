# `src/spacecraft/` — what a craft is, and how it flies

> **Purpose:** What a craft is and how it flies: the fleet `G` cycles, flight planning by
> propulsion (the least-energy Lambert transfer by default), steering and landing, correction
> burns, and the flight report.
> **Edit here to:** add or change a craft or engine, the crew g-limit, burns, the guidance or
> landing law, or what the report counts.
> **Not here:** body prediction and the coasting-path search are in
> [`core/transfer/`](../core/transfer/README.md). Picking the route and drawing the flight are
> in `render/mission.rs`. The arrival pop-up is `ui/arrival.rs`.

Everything about a spacecraft, as opposed to the orbital mechanics around it. `core` answers
where each body will be and what coasts from here to there; this layer decides when to burn, how
hard, and toward what, and counts what the flight cost.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer and re-exports its surface. |
| `constants.rs` | Every constant the layer reads: the built-in craft (Aerojet 260 thrust and exhaust speed, masses, the `3 g` crew limit, the probe's launch speed), the transfer search (`ORBIT_*`), burns and corrections, and the steering law (`POWERED_*`). |
| `config.rs` | `Spacecraft`, `Propulsion` (`Coasting`, `Direct`, `Transfer`, `Staged`), `Engine`, `Stage`, and `Fleet`, the roster `G` cycles. Plain data, so a settings menu can build or edit it. `Fleet::default` is the built-in set: crewed transfer (selected), full-thrust direct, coasting probe, staged Saturn V lander. |
| `rocket.rs` | `Rocket`: a craft that carries its propellant. `rocket_delta_v_m_s` and `rocket_mass_after_kg` (the rocket equation, both ways), `Rocket::burn` (drain the tanks for a speed change, drop empty stages), `acceleration_m_s2` (thrust over present mass, capped), `weakest_acceleration_m_s2` (what a landing is planned on), `plan_burn`, `Burn`. |
| `plan.rs` | `plan_flight`, by propulsion: the coasting planner, the direct flight flown through the forecast, or the least-energy transfer. `departure_burn`, `plan_transfer_correction`, `plan_correction_for`, `central_body`, `NoFlight`. |
| `lambert.rs` | `lambert`: the two-body arc between two points in a given time, universal variables, solved by bisection. The transfer search's cost model. |
| `guidance.rs` | The steering laws: `steer` (land on the braking curve; a direct flight also burns toward it and flies around bodies), `match_velocity` (correction burns), `braking_m_s2`, `blocking`. Touchdown uses `core::transfer::segment_entry`. |
| `flight.rs` | `Flight`: a craft in the live scene, its `Phase` (`Dry` once a rocket's tanks are empty), its `f64` state for a powered craft, its `Rocket` if it carries propellant, its correction schedule, and its report. |
| `report.rs` | `FlightReport`: time, peak speed, Δv, engine time, peak g, energy, propellant, touchdown; for a rocket also the propellant loaded and left, stages dropped, and whether it ran dry. |

## The rocket that carries its propellant

Three of the built-in craft are idealised: a fixed mass, and an engine that pushes for as long as
the flight asks. Their report says how much propellant that would have burned, but they never
carry it. A `Propulsion::Staged` craft does. It is a payload (`Spacecraft::mass_kg`) under a list
of `Stage`s, each a dry mass, a propellant load and an engine, burned in order from the first.

- **Mass follows the rocket equation.** A speed change `Δv` at exhaust speed `v_e` leaves
  `m₁ = m₀ · exp(−Δv / v_e)`, which is Tsiolkovsky's `Δv = v_e · ln(m₀ / m₁)` read backwards.
  `Rocket::burn` takes the speed change a step asked for and drains the tank by exactly that, so
  the same burn costs less propellant the lighter the rocket already is.
- **Acceleration is thrust over the present mass**, capped at the craft's limit. It rises as the
  tanks empty. The Saturn stack lifts off at `1.36 g`, reaches the `3 g` crew limit before its
  first stage is out, and its second stage lights at `0.83 g`.
- **An empty stage is dropped**, taking its dry mass with it, and the next one lights. The last
  stage is kept: it is the craft.
- **An empty rocket glides.** `Rocket::burn` gives only the speed change the tanks hold. Once the
  last one is empty the flight is `Phase::Dry`: no engine, no corrections, moved by gravity alone
  until it hits something. Nothing stops a launch the propellant cannot cover; the planner plans
  the trip as if it could (`Rocket::plan_burn`), and the rocket runs dry where it runs dry.
- **It flies the same route as the crewed transfer**: the least-energy arc, a departure burn onto
  it, a cruise with corrections, a braking burn and a landing. Two things had to change for a
  varying acceleration:
  - *The departure burn* is cut into `DEPARTURE_BURN_STEPS` equal shares of speed change, not
    equal times, each taking as long as the rocket needs at its mass then
    (`departure_burn_with`). The planner and the live flight both fly it that way.
  - *The landing* is planned on the weakest acceleration the rocket will have before it is down
    (`Rocket::weakest_acceleration_m_s2`, over `LANDING_LOOKAHEAD_FACTOR` times its impact
    speed). Planned on the present acceleration, a rocket that began braking on the end of a
    strong stage ran that stage out part-way down and hit the Moon at `2.2 km/s` with its lander
    stage almost full.
- **The built-in one is `Spacecraft::saturn_v_lander`**: the Saturn V's three stages (`5 × F-1`,
  `5 × J-2`, `J-2`), a lunar-module descent stage, and the empty ascent stage as payload.
  `2,922 t` at launch and `19.44 km/s` in all. From Earth to the Moon in `experiment_7` it spends
  `18.5 km/s` and lands at `10 m/s` with about `1.4 t` of propellant left. The stage figures and
  their caveats are in `constants.rs`.

What the model leaves out: dropped stages vanish instead of becoming bodies; there is no
atmosphere, so every engine runs at its vacuum figures; throttling to the limit is free, which the
real F-1 and J-2 could not do; and a tank burns to its last drop with no reserve.

A real Saturn V could not fly this profile with Apollo's payload, and the reason is the route, not
the rocket. The craft here starts at rest `200 km` up and climbs straight out, so its departure
costs `15.2 km/s` against the `12.9 km/s` the same climb costs at a constant `3 g`: the upper
stages light under `1 g` and lose the difference to gravity. Apollo went into orbit first, where
gravity losses are small. Launching from orbit is the natural next step.

## Constants

All in `constants.rs`, each with its reasoning. The ones that shape a flight most:

| Constant | Value | Why |
| --- | --- | --- |
| `CREW_MAX_ACCELERATION_G` | `3` | What crews sit through for minutes; every crewed burn stays at or under it. |
| `AEROJET_260_THRUST_N` | `26.2 MN` | The SL-3 firing, 1967: the largest solid motor ever fired. |
| `AEROJET_260_EXHAUST_SPEED_M_S` | `2,579 m/s` | Its `263 s` vacuum specific impulse; turns Δv into energy and propellant. |
| `POWERED_BRAKE_FRACTION` | `0.8` | Brake with four fifths of what the engine gives beyond the target's surface gravity. |
| `CORRECTION_FIRST_FRACTION`, `CORRECTION_MIN_REMAINING_S` | `1%`, `6 h` | Correct soon after departure, then each time half the time left has passed. |
| `LANDING_START_RADII` | `20` | The landing law starts by twenty radii out at the latest, as terminal guidance. |
| `DEPARTURE_BURN_STEPS` | `2,000` | A fixed step count keeps the burn smooth in what is solved for. |
| `LANDING_LOOKAHEAD_FACTOR` | `2` | A rocket plans its landing on the weakest stage it will light within twice its impact speed of burning. |
| `F1_*`, `J2_*`, `SATURN_*`, `LUNAR_MODULE_*` | — | The staged rocket's engines and stage masses: Apollo-era figures, with the Δv each stage gives. |

## Rules and invariants

- **Bevy-free but for `bevy::math`**, like `core`. It may import `core`, `physics_utils`, and
  `time`; nothing in `core` may import it.
- **A spacecraft is data.** No flight code reads a constant directly for a craft's numbers; it reads
  the `Spacecraft`. A new kind of craft is a new `Spacecraft` (or a new `Propulsion` variant), not a
  new code path in the mission.
- **A rocket's mass is its `Rocket`'s.** Flight code never subtracts propellant itself: it asks
  the `Rocket` to burn a speed change and uses the speed change it got back. The scene's copy of
  the craft is given the rocket's mass each substep, for the overlay.
- **The idealised craft are untouched by the rocket.** A craft with no `Rocket` takes the same
  code path, with the same numbers, as before the rocket existed.
- **The default craft never exceeds its acceleration limit.** Every burn, departure, correction and
  landing, is capped at `Spacecraft::acceleration_m_s2`; `FlightReport::peak_acceleration_m_s2`
  records what was used, and the live tests assert it.
- **A powered craft lives in `f64`, relative to its target**, flown by `Flight::steer` before every
  scene substep. Near a landing it moves centimetres per substep, below what the scene's `f32`
  positions resolve (`15 km` at Earth, `60 km` at Jupiter). The scene's copy is written so the
  scene's step ends where the craft is; it is for drawing.
- **The departure burn is open-loop and flown in `DEPARTURE_BURN_STEPS` equal steps**, identically
  by the planner and the live flight. A burn that stops on a threshold ends a whole step early or
  late as Newton probes it, and over a 17-year coast that jitter defeated the solve.
- **Energy is counted as Δv.** For a fixed exhaust speed and mass a burn's energy is proportional to
  its speed change, so the least-energy transfer is the least-Δv one. The search's cost leaves out
  gravity losses during the finite burns, so the live Δv is higher, most at heavy bodies: Mercury to
  Jupiter is estimated at `83 km/s` and flies at `182 km/s`, because braking at `3 g` against
  Jupiter's `2.5 g` leaves `0.47 g` to brake with.
- **Newtonian, like everything else.** The full-thrust craft reaches `28%` of light speed; the
  report and status line show the fraction.

## Where the tests are

`tst/spacecraft/`, one `*_tests.rs` per file, each `#[path]`-included at the end of its subject.
Two long sweeps are opt-in: `cargo test --release -- --ignored crewed_transfers` (the crewed
transfer to Mars, Venus, Jupiter and Neptune, about three minutes) and `--ignored every_pair` (all 90
full-thrust pairs, about eight seconds).

## Adding a file here

Keep it Bevy-free, put its tunable values in `constants.rs` with their reasoning, re-export its
public items from `mod.rs` and `src/lib.rs`, end it with a `#[path]` stub to
`tst/spacecraft/<file>_tests.rs`, and list it in the table above.
