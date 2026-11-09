# `tst/spacecraft/` — tests for `src/spacecraft/`

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
| `report_tests.rs` | `report.rs` | Energy and propellant from a burn, engine time for gentle trims, and a craft with no engine. |

## Conventions specific to this directory

- **Whole flights are flown at the app's frame size**, about 1,200 frames per flight, so the tests
  see what the app does.
- **Every live flight asserts a soft landing and the craft's acceleration limit**, not just arrival.
- **The long flights are `#[ignore]`d** with a reason naming the command that runs them; run them in
  release.
