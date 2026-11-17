# `tst/spacecraft/` — tests for `src/spacecraft/`

> **Purpose:** Tests for [`src/spacecraft/`](../../src/spacecraft/README.md): the built-in
> craft, Lambert, the steering and landing laws, the flight report, and whole flights flown
> live. Two opt-in sweeps.
> **Edit here to:** add or fix a test of a craft, a planner, or a guidance law.
> **Not here:** the flight code, in `src/spacecraft/`.

Unit tests, one file per source file, each `#[path]`-included at the end of its subject, so they
compile inside the module and see its private items through `use super::*`.

## What lives here

| File | Subject | What it pins |
| --- | --- | --- |
| `config_tests.rs` | `config.rs` | The built-in craft (3 g crewed, 178 g full thrust, coasting probe), their limits and validity, and the fleet's default and cycling. |
| `plan_tests.rs` | `plan.rs` | Routing (`central_body`), refusals (same body, invalid craft, the Sun's `28 g`, cancel), the departure burn's smoothness, and whole flights flown live: the crewed transfer to the Moon and back, full-thrust flights to the Moon, Mercury to Jupiter, and around the Sun. Two opt-in sweeps. |
| `lambert_tests.rs` | `lambert.rs` | Curtis Example 5.2, an arc that propagates to its end point, the direction of travel, and degenerate inputs. |
| `guidance_tests.rs` | `guidance.rs` | The braking budget, the landing law (burn, brake, coast on the curve, touch down at `10 m/s`, never speed up when landing), step sizes, the velocity-matching burn against Jupiter's gravity, detours, and the touchdown test. |
| `flight_tests.rs` | `flight.rs` | Each propulsion's first phase, the departure burn ending in a cruise, the correction schedule, and a correction becoming a burn. |
| `report_tests.rs` | `report.rs` | Energy and propellant from a burn, engine time for gentle trims, a craft with no engine, and a rocket's burn charged what its tanks lost. |
| `rocket_tests.rs` | `rocket.rs` | The rocket equation in both directions; a tank draining by it; the acceleration rising as the mass falls and capped at the limit; burn times at full thrust and throttled; running dry; staging (a dropped stage, a burn across one, staging beating one lump); the look-ahead to the weakest stage; and the Saturn V lander's mass and its `4.01`, `5.29`, `6.82` and `3.32 km/s` stages. |

The staged rocket also has tests in the other files: its craft data in `config_tests.rs`; its
departure burn, tanks, staging and running dry in `flight_tests.rs`; and in `plan_tests.rs` its
departure burn against the constant one, the whole flight from Earth to a soft landing on the Moon
in `experiment_7`'s scene, the same flight with too heavy a payload running dry, and the two
refusals (a first stage too weak to lift off, a last stage too weak to land).

## Conventions specific to this directory

- **Whole flights are flown at the app's frame size**, about 1,200 frames per flight, so the tests
  see what the app does.
- **Every live flight asserts a soft landing and the craft's acceleration limit**, not just arrival.
- **The long flights are `#[ignore]`d** with a reason naming the command that runs them; run them in
  release.
