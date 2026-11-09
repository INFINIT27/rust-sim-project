# `src/core/transfer/` — planning a craft's path between two bodies

Finds a coasting path for a craft from one body to another under the scene's own gravity, and keeps
it on that path once it is flying, and opens the prediction it uses to the layer above
([`src/spacecraft/`](../../spacecraft/README.md)), which decides when a craft burns and how hard. No
conics anywhere: every candidate path is flown through a
prediction of the scene made with the same integrator the live scene uses.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | The public surface: `plan_departure`, `plan_correction`, `physical_radius_m`, `TransferPlan`, `Flyby`, `NoTransfer`, `Reach`, `coasting_reach`. The horizon-doubling loop that lets direct and gravity-assisted routes compete. |
| `ephemeris.rs` | `Ephemeris`: the scene stepped forward and sampled, with distant moon systems folded into their planet. `position_rate`, the derivative of its interpolated position, which a steered craft matches rather than the stored velocities. `PlanProgress`, which a search reports to and is cancelled through. |
| `flight.rs` | One trial flight (`fly`), aimed at a `Goal`: a body's centre or a point beside it. `aim`, the Newton loop every search uses. `finish`, which turns a solved launch into a plan and records its flybys. |
| `departure.rs` | The direct search: `RadialLaunch`, the speed ladder, the direction scan, refinement. |
| `assist.rs` | The gravity-assist search: routes through up to `TRANSFER_MAX_FLYBYS` planets, fewest flybys first. Every level solves the two launch angles for a grid of aim points beside its planet, and each first planet is searched on its own thread. |
| `forecast.rs` | The public face of the prediction for the spacecraft layer: `Forecast` (where each body is, its velocity, gravity, softening), `solve_coast` and `solve_coast_with` (the launch, or the burn through a launcher, that coasts to the target around a chosen time), `Coast`. |
| `voyage.rs` | `Voyage`: a craft in the live scene: its path, its correction-burn schedule (including burns around each flyby), and touchdown. `record_path` records without the `f32` touchdown test, for a craft whose controller finds its own. |

## Rules and invariants

- **Every search checks `PlanProgress`.** The ephemeris build, every trial flight, and every
  Newton iteration. A search has no time limit, so a cancel that waited for any of them could wait
  for minutes.
- **Aims judge the along-track error too, within a margin.** The Newton step drives the miss
  projected across the approach, but a flight cut off by its window can sit on the line through
  the goal while still short of it. So convergence also needs the along-track error under
  `TRANSFER_ALONG_TRACK_FRACTION` (1%) of the pass distance. It can't be held to the metre
  tolerance, because the closest point is found on a straight segment across one integration step.
- **Assist aims solve the launch angles directly, never a nested aim.** A later flyby is thousands
  of times more sensitive to the launch than an earlier one, so moving it barely moves the earlier
  pass. Nesting one aim inside another per flyby multiplied each level's leftover error and could
  not converge at the second. Because that sensitivity changes by orders of magnitude from level
  to level, assist aims rescale their probes to what they measure (`Adapt`).
- **Only an assist-only search has a horizon limit.** When a direct path is possible the search
  doubles its horizon without limit; when only assists could work it stops at
  `TRANSFER_ASSIST_ONLY_HORIZON_S` and reports the target out of reach.
- **A flyby is unpowered.** The craft's speed relative to a planet is the same leaving as arriving,
  which is what lets the reach bound prune a route after each pass.
- **A correction must find the pass it was planned around.** An aim point is compared only in the
  plane across the planned approach, so a different encounter with the same planet, hours later
  and from another direction, can sit on it there. Corrections set `AimPoint::hold_direction`,
  which also requires the pass to approach within `TRANSFER_FLYBY_MAX_TURN_RAD` (10°) of the
  planned direction. The assist search does not, because its aim points are new passes.
- **A correction that no longer reaches the destination is not applied.** `plan_correction`
  returns `None` when its own prediction misses by more than `TRANSFER_CORRECTION_MAX_MISS_RADII`
  (one radius), and the craft carries on. A burn onto a path that misses only adds error, and
  once applied its flyby becomes what every later burn aims at.
- **A steered craft must match the target's `position_rate`, not its stored velocity.** The
  integrator updates velocity before position, so a stored velocity leads the path its positions
  trace by half a step's acceleration. Steering to match it, a landing on the Moon stalled
  `1.5 km` up, closing at the `25 m/s` it was told to on a target that receded just as fast.
  `Forecast::velocity_m_s` is the position rate.

## Known gaps

- **A route that leaves a moon from low altitude can be chaotic, and corrections cannot always
  recover it.** In `a_gravity_assisted_voyage_flown_live_passes_its_planet_and_lands`
  (Europa → Ganymede → Callisto), the live flight is `45,000 km` and `440 m/s` off the plan by
  the first correction, two days in. That drift is not step-size error: frames `16×` shorter
  barely change the substep (`9.1 s` to `8.8 s`) yet move the rocket a further `15,000 km`, and
  re-flying the plan through a slightly different ephemeris moves its Ganymede pass from
  `6,421 km` to `2,624 km`. With no burn, the drifted craft's closest pass of Ganymede in the flyby
  window is `625,000 km` out. No correction can find the planned pass from there, so with the two
  rules above every burn is refused and the craft never lands, and the test fails. Without them,
  the first burn locked onto a pass `30` hours late and missed Callisto by `942,666 km`. The route
  the planner picks depends on the platform's `sin`/`cos`, and a route recorded on another machine
  (Ganymede at `4,798 km`) did land, so whether this bites depends on the route. A fix would have to
  either keep corrections from depending on the departure arc (for example, re-plan the whole
  assisted route from the craft's state), or make the live and planned departures agree.

## Tests

`tst/core/transfer_tests.rs`. The long voyage is opt-in: `cargo test --release -- --ignored a_voyage`.
Powered flight is tested with the spacecraft layer, in `tst/spacecraft/`.
