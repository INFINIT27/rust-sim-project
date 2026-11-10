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
| `config.rs` | `Spacecraft`, `Propulsion` (`Coasting`, `Direct`, `Transfer`), `Engine`, and `Fleet`, the roster `G` cycles. Plain data, so a settings menu can build or edit it. `Fleet::default` is the built-in set: crewed transfer (selected), full-thrust direct, coasting probe. |
| `plan.rs` | `plan_flight`, by propulsion: the coasting planner, the direct flight flown through the forecast, or the least-energy transfer. `departure_burn`, `plan_transfer_correction`, `plan_correction_for`, `central_body`, `NoFlight`. |
| `lambert.rs` | `lambert`: the two-body arc between two points in a given time, universal variables, solved by bisection. The transfer search's cost model. |
| `guidance.rs` | The steering laws: `steer` (land on the braking curve; a direct flight also burns toward it and flies around bodies), `match_velocity` (correction burns), `braking_m_s2`, `blocking`, `segment_entry_m`. |
| `flight.rs` | `Flight`: a craft in the live scene, its `Phase`, its `f64` state for a powered craft, its correction schedule, and its report. |
| `report.rs` | `FlightReport`: time, peak speed, Δv, engine time, peak g, energy, propellant, touchdown. |

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

## Rules and invariants

- **Bevy-free but for `bevy::math`**, like `core`. It may import `core`, `physics_utils`, and
  `time`; nothing in `core` may import it.
- **A spacecraft is data.** No flight code reads a constant directly for a craft's numbers; it reads
  the `Spacecraft`. A new kind of craft is a new `Spacecraft` (or a new `Propulsion` variant), not a
  new code path in the mission.
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
