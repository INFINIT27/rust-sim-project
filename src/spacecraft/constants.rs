//! Every constant the spacecraft layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module. The built-in
//! spacecraft (see [`crate::spacecraft::Fleet::default`]) are assembled from the values here, so a
//! future settings menu can start from them and change any one.

use crate::core::scene::Color;
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;

// ---------------------------------------------------------------------------------------------
// What every built-in craft shares
// ---------------------------------------------------------------------------------------------

/// Height above the origin's surface a craft starts from, in metres.
///
/// **Why this value.** `200 km`, a typical low parking orbit, which is where an interplanetary
/// departure burn really starts. The craft starts at rest relative to the origin, not in orbit,
/// so the departure pays the origin's whole gravity well.
///
/// **Used by.** Every built-in craft in [`crate::spacecraft::config`].
///
/// **If you change it.** Below about `0` the craft starts inside the body. Much higher and the
/// departure leaves from somewhere visibly separate from it when zoomed in.
pub const LAUNCH_ALTITUDE_M: f64 = 200_000.0;

/// A craft's radius, in metres.
///
/// **Why this value.** About Voyager's size: its dish is `3.7 m` across. The renderer floors
/// apparent size, so a craft is drawn as a visible dot regardless.
///
/// **Used by.** Every built-in craft in [`crate::spacecraft::config`].
///
/// **If you change it.** Nothing, until it is large enough to give the craft a real collision hull.
pub const CRAFT_RADIUS_M: f64 = 2.0;

/// The colour of a craft and of its drawn path.
///
/// **Why this value.** A saturated cyan, a colour no planet uses, so the path cannot be mistaken
/// for an orbit trail.
///
/// **Used by.** Every built-in craft in [`crate::spacecraft::config`], and through them the render
/// layer's path drawing.
///
/// **If you change it.** Keep it away from the planets' colours.
pub const CRAFT_COLOR: Color = Color {
    r: 0.2,
    g: 0.95,
    b: 1.0,
    a: 1.0,
};

// ---------------------------------------------------------------------------------------------
// The crewed craft and its engine
// ---------------------------------------------------------------------------------------------

/// The thrust of the Aerojet 260 at its largest test firing, in newtons.
///
/// **Why this value.** The Aerojet 260 is the largest solid rocket motor ever fired. Its third test,
/// SL-3 on 17 June 1967, produced `5.6 million lbf` (`26.2 MN`); the first two, in 1965 and 1966,
/// peaked at `15.7 MN`. Source: <https://en.wikipedia.org/wiki/Aerojet_260>. The real motor burned
/// for `114 s`; here it can push for as long as a flight needs.
///
/// **Used by.** The built-in craft's engine in [`crate::spacecraft::config`].
///
/// **If you change it.** The crewed craft is held to [`CREW_MAX_ACCELERATION_G`] anyway, so only
/// the full-thrust craft changes: its flight times scale as `1 / √thrust`.
pub const AEROJET_260_THRUST_N: f64 = 26.2e6;

/// The Aerojet 260's exhaust speed, in m/s: its vacuum specific impulse times standard gravity.
///
/// **Why this value.** `263 s` vacuum specific impulse (same source as [`AEROJET_260_THRUST_N`]),
/// times `9.806 65 m/s²`, is `2,579 m/s`. It is what turns a burn into energy and propellant in the
/// flight report: the engine throws propellant out at this speed, so each second of thrust `F`
/// costs `F / v_e` of propellant and `F · v_e / 2` of power.
///
/// **Used by.** The built-in craft's engine in [`crate::spacecraft::config`], and through it
/// [`crate::spacecraft::FlightReport`].
///
/// **If you change it.** Energy scales with it and propellant inversely; the trajectory does not
/// change.
pub const AEROJET_260_EXHAUST_SPEED_M_S: f64 = 263.0 * STANDARD_GRAVITY_M_S2;

/// The crewed craft's mass, in kilograms.
///
/// **Why this value.** `15 t`, a typical crewed spacecraft. Fixed: the engine is idealised, so the
/// propellant it would burn is reported but never subtracted.
///
/// **Used by.** The built-in powered craft in [`crate::spacecraft::config`].
///
/// **If you change it.** At [`CREW_MAX_ACCELERATION_G`] the trajectory does not change, only the
/// energy and propellant, which scale with mass. The full-thrust craft accelerates as `1 / mass`.
pub const CREWED_CRAFT_MASS_KG: f64 = 15_000.0;

/// The most acceleration the crewed craft's engine is throttled to, in standard gravities.
///
/// **Why this value.** Three. A trained crew can survive about `9 g` for seconds; `3 g` is what
/// crews sit through for minutes, and the most the Space Shuttle allowed on ascent. Every burn of
/// the crewed craft, departure, corrections and landing, stays at or under it.
///
/// **Used by.** The crewed craft in [`crate::spacecraft::config`].
///
/// **If you change it.** Burns are shorter and lose less to gravity at higher values, so slightly
/// less energy is used. Below the gravity at a body's surface (`2.5 g` at Jupiter's cloud tops,
/// `28 g` at the Sun's) the craft can neither lift off from it nor land on it.
pub const CREW_MAX_ACCELERATION_G: f64 = 3.0;

// ---------------------------------------------------------------------------------------------
// The staged rocket: a Saturn V with a lunar-module descent stage
// ---------------------------------------------------------------------------------------------
//
// The figures below are the Apollo-era ones as the public summaries round them (the Saturn V and
// Apollo Lunar Module articles on Wikipedia, which follow NASA's flight manuals and press kits).
// They were entered from memory of those summaries and not checked against a source when this was
// written, and the real values vary by a percent or two from flight to flight: check them before
// relying on one. With them the rocket equation gives, stage by stage:
//
// | Stage | Lights at | Burns out at | Exhaust speed | Δv |
// | --- | --- | --- | --- | --- |
// | S-IC | `2,921.6 t` | `761.6 t` | `2,981 m/s` | `4.01 km/s` |
// | S-II | `631.6 t` | `175.5 t` | `4,129 m/s` | `5.29 km/s` |
// | S-IVB | `135.4 t` | `25.9 t` | `4,129 m/s` | `6.82 km/s` |
// | LM descent stage | `12.43 t` | `4.18 t` | `3,050 m/s` | `3.32 km/s` |
//
// `19.44 km/s` in all, pinned by `rocket_tests`. Flown from Earth to the Moon in `experiment_7`
// the trip takes `18.5 km/s`: a `15.2 km/s` departure burn, then corrections and the landing.
// It lands with about `1.4 t` of propellant left (`plan_tests`).
//
// Two things differ from Apollo, both because this craft flies straight up from rest and lands
// directly, with nothing left in lunar orbit:
//
// - The real Saturn V carried `45 t` past the third stage, the command and service modules as
//   well as the lunar module. Here the lunar module rides alone, so the S-IVB gives more.
// - The payload is the ascent stage **empty** (`2,150 kg`), not fuelled (`4,700 kg`). The flight
//   ends at the landing, so the ascent engine is never lit and its propellant would be dead
//   weight. With it aboard the stack carries `18.2 km/s`, runs dry on the way down, and hits the
//   Moon at `370 m/s`.

/// One F-1 engine's thrust in vacuum, in newtons.
///
/// **Why this value.** `7,770 kN`. The F-1 is the engine of the Saturn V's first stage, burning
/// kerosene and liquid oxygen; five of them lifted every Apollo mission. Its sea-level thrust is
/// `6,770 kN`. The vacuum figure is used because the scene has no atmosphere and the craft starts
/// `200 km` up.
///
/// **Used by.** [`crate::spacecraft::Engine::f1_cluster`].
///
/// **If you change it.** The first stage's acceleration scales with it: five give `1.35 g` on the
/// full stack. Below Earth's `0.94 g` at the launch altitude the rocket cannot lift off.
pub const F1_VACUUM_THRUST_N: f64 = 7.77e6;

/// The F-1's specific impulse in vacuum, in seconds.
///
/// **Why this value.** `304 s`, an exhaust speed of `2,981 m/s`. At sea level it is `263 s`.
///
/// **Used by.** [`crate::spacecraft::Engine::f1_cluster`].
///
/// **If you change it.** The first stage's speed change scales with it, by the rocket equation.
pub const F1_VACUUM_SPECIFIC_IMPULSE_S: f64 = 304.0;

/// One J-2 engine's thrust, in newtons.
///
/// **Why this value.** `1,033 kN` in vacuum. The J-2 burns liquid hydrogen and liquid oxygen; five
/// power the Saturn V's second stage and one its third.
///
/// **Used by.** [`crate::spacecraft::Engine::j2_cluster`] and [`crate::spacecraft::Engine::j2`].
///
/// **If you change it.** The second stage lights at `0.83 g` and the third at `0.78 g`, both under
/// Earth's surface gravity, so they lose much of their burn to gravity on a straight climb. More
/// thrust loses less.
pub const J2_THRUST_N: f64 = 1.033e6;

/// The J-2's specific impulse, in seconds.
///
/// **Why this value.** `421 s` in vacuum, an exhaust speed of `4,129 m/s`: hydrogen and oxygen
/// give the fastest exhaust of any propellants flown at this scale.
///
/// **Used by.** [`crate::spacecraft::Engine::j2_cluster`] and [`crate::spacecraft::Engine::j2`].
///
/// **If you change it.** The second and third stages' speed change scales with it.
pub const J2_SPECIFIC_IMPULSE_S: f64 = 421.0;

/// The Saturn V first stage (S-IC) with its tanks empty, in kilograms.
///
/// **Why this value.** `130 t`.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** A lighter stage gives the same propellant more speed change.
pub const SATURN_S_IC_DRY_MASS_KG: f64 = 130_000.0;

/// The Saturn V first stage's propellant, in kilograms.
///
/// **Why this value.** `2,160 t` of kerosene and liquid oxygen, three quarters of the whole
/// rocket.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** More propellant gives more speed change but a heavier, slower lift-off.
pub const SATURN_S_IC_PROPELLANT_KG: f64 = 2_160_000.0;

/// The Saturn V second stage (S-II) with its tanks empty, in kilograms.
///
/// **Why this value.** `40.1 t`.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** As for [`SATURN_S_IC_DRY_MASS_KG`].
pub const SATURN_S_II_DRY_MASS_KG: f64 = 40_100.0;

/// The Saturn V second stage's propellant, in kilograms.
///
/// **Why this value.** `456.1 t` of liquid hydrogen and liquid oxygen.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** As for [`SATURN_S_IC_PROPELLANT_KG`].
pub const SATURN_S_II_PROPELLANT_KG: f64 = 456_100.0;

/// The Saturn V third stage (S-IVB) with its tanks empty, in kilograms.
///
/// **Why this value.** `13.5 t`.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** As for [`SATURN_S_IC_DRY_MASS_KG`].
pub const SATURN_S_IVB_DRY_MASS_KG: f64 = 13_500.0;

/// The Saturn V third stage's propellant, in kilograms.
///
/// **Why this value.** `109.5 t` of liquid hydrogen and liquid oxygen.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** As for [`SATURN_S_IC_PROPELLANT_KG`].
pub const SATURN_S_IVB_PROPELLANT_KG: f64 = 109_500.0;

/// The Apollo lunar module's descent stage with its tanks empty, in kilograms.
///
/// **Why this value.** `2,034 kg`.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`], as the fourth stage.
///
/// **If you change it.** As for [`SATURN_S_IC_DRY_MASS_KG`]. It is the last stage, so it is never
/// dropped: it lands with the payload.
pub const LUNAR_MODULE_DESCENT_DRY_MASS_KG: f64 = 2_034.0;

/// The Apollo lunar module's descent propellant, in kilograms.
///
/// **Why this value.** `8,248 kg` of Aerozine 50 and nitrogen tetroxide.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`].
///
/// **If you change it.** It is what the landing is flown on: `3.32 km/s` of speed change.
pub const LUNAR_MODULE_DESCENT_PROPELLANT_KG: f64 = 8_248.0;

/// The thrust of the Apollo lunar module's descent engine, in newtons.
///
/// **Why this value.** `45.04 kN` at full throttle. It gives the descent stage and its payload
/// `3.6 m/s²` with full tanks, against the Moon's `1.62 m/s²` at the surface.
///
/// **Used by.** [`crate::spacecraft::Engine::lunar_module_descent`].
///
/// **If you change it.** Under the target's surface gravity the craft cannot land softly, and the
/// planner says so.
pub const LUNAR_MODULE_DESCENT_THRUST_N: f64 = 45_040.0;

/// The specific impulse of the Apollo lunar module's descent engine, in seconds.
///
/// **Why this value.** `311 s`, an exhaust speed of `3,050 m/s`.
///
/// **Used by.** [`crate::spacecraft::Engine::lunar_module_descent`].
///
/// **If you change it.** The landing stage's speed change scales with it.
pub const LUNAR_MODULE_DESCENT_SPECIFIC_IMPULSE_S: f64 = 311.0;

/// The Apollo lunar module's ascent stage with its tanks empty, in kilograms: the staged rocket's
/// payload.
///
/// **Why this value.** `2,150 kg`, the crew cabin. Fuelled it weighs `4,700 kg`, but the flight
/// ends at the landing, so its engine is never lit and its propellant is left behind. See the
/// section's opening comment for what carrying it would cost.
///
/// **Used by.** [`crate::spacecraft::Spacecraft::saturn_v_lander`], as the craft's `mass_kg`.
///
/// **If you change it.** Every stage carries it, so every stage's speed change falls as it grows,
/// the last stage's most.
pub const LUNAR_MODULE_ASCENT_STAGE_DRY_MASS_KG: f64 = 2_150.0;

// ---------------------------------------------------------------------------------------------
// The coasting probe
// ---------------------------------------------------------------------------------------------

/// The fastest the coasting probe may leave a body at, relative to it once clear of its gravity,
/// in m/s.
///
/// **Why this value.** `15.4 km/s`, Voyager 2's speed. The coasting planner picks any departure
/// speed up to it, whichever arrives first. Gravity still speeds the probe up on the way into a
/// well, and its speed relative to the Sun includes the origin's own orbital speed. Leaving from
/// `200 km` above Earth at the cap takes `18.9 km/s` there.
///
/// **Used by.** The coasting probe in [`crate::spacecraft::config`].
///
/// **If you change it.** Lower and the planner needs longer horizons, and below Earth's
/// `12.3 km/s` solar-escape excess the outer planets take decades. Higher and every flight can be
/// shorter.
pub const PROBE_MAX_DEPARTURE_SPEED_M_S: f64 = 15_400.0;

/// The coasting probe's mass, in kilograms.
///
/// **Why this value.** Voyager 2's, `722 kg`. Only its order of magnitude matters: anything this
/// light is a test particle whose pull on the planets is far below `f32` resolution. It must be
/// non-zero, because a massless prop is never accelerated (see `Prop`).
///
/// **Used by.** The coasting probe in [`crate::spacecraft::config`].
///
/// **If you change it.** Nothing visible until it approaches a moon's mass.
pub const PROBE_MASS_KG: f64 = 722.0;

// ---------------------------------------------------------------------------------------------
// The least-energy transfer
// ---------------------------------------------------------------------------------------------

/// The shortest time of flight the least-energy search tries, as a fraction of the Hohmann time
/// between the two orbits.
///
/// **Why this value.** A tenth. Faster arcs cost far more energy than a Hohmann-like one, so the
/// optimum is never down here, but launching *now* (not at the best launch window) can put the
/// target where only a fast or a long arc reaches it cheaply, and the search must see both.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Higher can miss a cheap fast arc; lower only spends samples on dear ones.
pub const ORBIT_SEARCH_MIN_FRACTION: f64 = 0.1;

/// The longest time of flight the least-energy search tries, as a fraction of the Hohmann time.
///
/// **Why this value.** Two. A single-revolution transfer arc longer than twice the Hohmann time
/// goes most of the way round the central body and is never the cheapest; longer flights also
/// need longer predictions, which cost time to build.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Larger finds long, slow arcs when the target is badly placed, at the cost
/// of planning time. Smaller can miss them.
pub const ORBIT_SEARCH_MAX_FRACTION: f64 = 2.0;

/// How many times of flight the least-energy search samples between its bounds.
///
/// **Why this value.** `240`, log-spaced, so neighbouring samples differ by about `1.3%`, finer than
/// the dip of any minimum, which a golden-section search then refines. Each sample is a two-body
/// Lambert solve, microseconds each.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Fewer can step over a narrow minimum. More costs nothing that matters.
pub const ORBIT_SEARCH_SAMPLES: usize = 240;

/// How many launch or landing directions the search tries when the origin or target is the body
/// the transfer is computed about (Earth, for Earth to the Moon).
///
/// **Why this value.** `24`, every `15°` around the transfer plane. Launching from a planet to its
/// own moon, where on the planet the craft starts decides the arc, so it is searched too.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Fewer can miss the cheap side; more only costs planning time.
pub const ORBIT_SEARCH_DIRECTIONS: usize = 24;

/// How many golden-section steps refine the best sampled time of flight.
///
/// **Why this value.** `40` narrows the bracket by `0.618⁴⁰`, about `4e-9`, far below where the
/// energy stops changing.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Fewer leaves a coarser optimum; more costs nothing that matters.
pub const ORBIT_SEARCH_REFINE_STEPS: usize = 40;

/// How many of the cheapest two-body arcs the planner tries to fly in full before giving up.
///
/// **Why this value.** Five. The two-body estimate leaves out every other planet and the origin
/// bending the departure, so its best arc occasionally does not converge in the full solve; the
/// next-cheapest usually does.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Fewer gives up sooner on awkward routes; more takes longer to fail.
pub const ORBIT_SEARCH_CANDIDATES: usize = 5;

/// How far either side of the estimated arrival time the full solve looks for the arrival, as a
/// fraction of the time of flight.
///
/// **Why this value.** A quarter: wide enough for what the two-body estimate leaves out, narrow
/// enough to exclude the next pass of the target a different arc would make.
///
/// **Used by.** [`crate::spacecraft::plan`]'s transfer search.
///
/// **If you change it.** Narrower can miss the arrival; wider can capture a different, dearer one.
pub const ORBIT_SOLVE_WINDOW_FRACTION: f64 = 0.25;

/// The velocity error at which a velocity-matching burn counts as done, in m/s.
///
/// **Why this value.** Ten centimetres per second: below what the next correction burn would
/// measure, and above the error a substep at full thrust can leave.
///
/// **Used by.** [`crate::spacecraft::flight`]'s departure and correction burns.
///
/// **If you change it.** Larger leaves more for the corrections. Smaller lets a burn chase rounding.
pub const BURN_DONE_M_S: f64 = 0.1;

/// How many braking distances out a transfer's cruise ends and its landing begins.
///
/// **Why this value.** One and a half. The landing law starts braking only once the craft is on
/// its braking curve, so starting it early costs nothing, and the margin lets it straighten the
/// approach before braking hard.
///
/// **Used by.** [`crate::spacecraft::flight`].
///
/// **If you change it.** Below `1` the craft arrives above its braking curve and lands hard.
/// Larger only starts steering earlier.
pub const LANDING_START_BRAKING_DISTANCES: f64 = 1.5;

/// A coasting transfer's step, as a fraction of the shortest dynamical time `√(d³/μ)` from the craft
/// to any body.
///
/// **Why this value.** A hundredth. The dynamical time is how long gravity takes to bend a path
/// appreciably, so a hundredth of it follows the bend closely: about a minute just after leaving a
/// planet, days in deep space, where the scene's own substep is the limit instead. A fixed one-minute
/// step made a 17-year cruise to Neptune nine million steps.
///
/// **Used by.** [`crate::spacecraft::flight`].
///
/// **If you change it.** Larger coarsens the cruise, which the corrections then pay for. Smaller
/// only costs time.
pub const CRUISE_STEP_FRACTION: f64 = 0.01;

// ---------------------------------------------------------------------------------------------
// Steering: shared by the landing of every powered craft, and the whole of a direct flight
// ---------------------------------------------------------------------------------------------

/// The share of a powered craft's spare acceleration its landing plans to brake with: of what its
/// engine gives beyond the target's surface gravity (see [`crate::spacecraft::guidance::braking_m_s2`]).
///
/// **Why this value.** Four fifths. Holding against the surface gravity takes part of the engine
/// just to stand still, so only the rest can brake: at `3 g` that is `19.6 m/s²` over Earth and
/// `4.6 m/s²` over Jupiter. Planning with all of it would leave nothing for what the curve does not
/// see: gravity growing as the craft descends, steering, and the target moving. With the fifth
/// kept back every landing in the sweeps touches down at its design speed.
///
/// **Used by.** [`crate::spacecraft::guidance`], and the direct flight's horizon.
///
/// **If you change it.** Near `1` the craft falls behind its braking curve and lands hard, worst at
/// heavy bodies. Lower and every landing starts braking earlier and takes longer.
pub const POWERED_BRAKE_FRACTION: f64 = 0.8;

/// The speed a powered craft closes on the surface at touchdown, in m/s.
///
/// **Why this value.** `10 m/s`, a hard but survivable landing. It must be above zero: the braking
/// curve alone reaches zero speed exactly at the surface, and a craft steered by it would approach
/// ever more slowly without touching. Measured: the crewed transfer lands at `10.1` to `10.3 m/s` on
/// every route from the Moon to Neptune, and the full-thrust craft at or under `13.5 m/s` on all 90
/// pairs of the Sun, the planets, and the Moon, the Sun's `274 m/s²` pull adding the most.
///
/// **Used by.** [`crate::spacecraft::guidance`].
///
/// **If you change it.** At zero the craft never lands. Higher only lands harder.
pub const POWERED_TOUCHDOWN_SPEED_M_S: f64 = 10.0;

/// A powered craft's substep, as a fraction of the time it would take to reach the surface at its
/// present closing speed.
///
/// **Why this value.** A fiftieth, so the steering is re-aimed at least fifty times on any approach
/// at any scale: from thousands of km/s far out to metres per second at the surface. Because the
/// approach shrinks geometrically, that costs a few hundred substeps per landing, not millions.
///
/// **Used by.** [`crate::spacecraft::guidance`].
///
/// **If you change it.** Larger overshoots the braking curve between substeps and lands harder.
/// Smaller only costs time.
pub const POWERED_SUBSTEP_FRACTION: f64 = 0.02;

/// The shortest substep a powered craft asks for, in seconds.
///
/// **Why this value.** `10 ms`, at which a craft at its touchdown speed moves `10 cm`, well below
/// anything the scene resolves. It only bounds the substep count right at touchdown.
///
/// **Used by.** [`crate::spacecraft::guidance`].
///
/// **If you change it.** Shorter costs substeps for nothing; much longer makes the last metres
/// coarse.
pub const POWERED_MIN_SUBSTEP_S: f64 = 0.01;

/// The longest substep a powered craft asks for, in seconds.
///
/// **Why this value.** One minute: a `1,747 m/s²` engine changes the craft's speed by about
/// `105 km/s` in it, under `1%` of the tens of thousands of km/s a cruise reaches, so the burn and
/// the flip stay smooth.
///
/// **Used by.** [`crate::spacecraft::guidance`].
///
/// **If you change it.** Longer coarsens the burn and the flip. Shorter only costs substeps
/// during the cruise.
pub const POWERED_MAX_SUBSTEP_S: f64 = 60.0;

/// How close, in radii of a body, a powered craft's straight line to its target may pass before
/// the craft steers around that body.
///
/// **Why this value.** Two radii, so the steering turns away while the body is still a body's
/// width off the line.
///
/// **Used by.** [`crate::spacecraft::guidance`].
///
/// **If you change it.** Toward `1` the craft only turns once a graze is already under way.
/// Larger detours around bodies the line would have cleared.
pub const POWERED_CLEARANCE_RADII: f64 = 2.0;

/// How far from a body's centre, in its radii, a powered craft aims while steering around it.
///
/// **Why this value.** Three radii, outside [`POWERED_CLEARANCE_RADII`], so that once the craft is
/// heading past the body, its line to the target has cleared it and the detour ends.
///
/// **Used by.** [`crate::spacecraft::guidance`].
///
/// **If you change it.** At or inside the clearance the detour may never end. Much larger
/// lengthens every detour.
pub const POWERED_AIM_CLEARANCE_RADII: f64 = 3.0;

/// How many times a straight burn-and-brake's duration a powered plan flies before giving up.
///
/// **Why this value.** Three: room for a detour around a body and for the target moving on, while
/// a flight that circles without landing still ends.
///
/// **Used by.** [`crate::spacecraft::plan`]'s direct flight.
///
/// **If you change it.** Tighter reports long detours as not arriving. Looser only takes longer
/// to give up.
pub const POWERED_HORIZON_FACTOR: f64 = 3.0;

/// How many substeps a powered plan flies between checks for a cancel.
///
/// **Why this value.** `256`, a few milliseconds of planning, so a cancel is felt at once.
///
/// **Used by.** [`crate::spacecraft::plan`]'s direct flight.
///
/// **If you change it.** Larger delays a cancel; smaller costs nothing measurable.
pub const POWERED_CANCEL_CHECK_STEPS: u64 = 256;

/// How many equal steps a departure burn is flown in, by the planner and the live flight alike.
///
/// **Why this value.** `2,000`. The burn is open-loop and flown in a fixed number of steps so that
/// where it ends changes smoothly with the burn being solved for, which the planner's Newton solve
/// needs: a burn that stopped on a threshold ended a whole step earlier or later as the solve
/// probed it, and over a 17-year coast to Neptune that jitter alone defeated the solve. A burn of
/// a few hundred seconds then takes steps of a fraction of a second, far finer than the origin's
/// gravity changes over.
///
/// **Used by.** [`crate::spacecraft::plan::departure_burn`] and [`crate::spacecraft::flight`].
///
/// **If you change it.** Far fewer and the burn's own integration error grows; more costs planning
/// time in every Newton iteration.
pub const DEPARTURE_BURN_STEPS: usize = 2_000;

/// When a transfer's first correction burn comes, as a fraction of the planned flight.
///
/// **Why this value.** One percent: soon after the departure burn, when its drift is still small
/// to remove, and long enough after it (`1.1 h` to the Moon, `64 d` to Neptune) for that drift to
/// be measurable.
///
/// **Used by.** [`crate::spacecraft::Flight::next_correction_at`].
///
/// **If you change it.** Later and the departure's drift costs more to remove. Earlier only spends
/// a correction on drift too small to matter.
pub const CORRECTION_FIRST_FRACTION: f64 = 0.01;

/// How much of a transfer's flight must be left for another correction burn to be planned, in
/// seconds.
///
/// **Why this value.** Six hours. After the first, each correction comes halfway through what is
/// left, so the gap to the target halves each time: about fifteen corrections to Neptune, four or
/// five to the Moon. Closer in than six hours the landing law, which starts steering within a
/// braking distance and a half, takes over.
///
/// **Used by.** [`crate::spacecraft::Flight::next_correction_at`].
///
/// **If you change it.** Larger leaves more drift for the landing to absorb; on a 17-year flight
/// to Neptune the last of the old fixed corrections came 131 days out and the craft missed by
/// `443,800 km`. Smaller adds corrections that only measure noise.
pub const CORRECTION_MIN_REMAINING_S: f64 = 6.0 * 3_600.0;

/// How many of the target's radii out a transfer's landing begins at the latest, whatever its
/// braking distance.
///
/// **Why this value.** Twenty. The landing law steers the craft's velocity straight at the target's
/// centre, so starting it this far out makes it terminal guidance as well: it removes the drift the
/// last correction left before the target's gravity bends it into a miss. On a 17-year flight to
/// Neptune, corrections alone left the craft swinging past at about `98,000 km` (four radii); from
/// twenty radii the redirection costs a fraction of a km/s.
///
/// **Used by.** [`crate::spacecraft::flight`].
///
/// **If you change it.** Smaller lets an off-target arrival slip past. Larger starts steering where
/// the corrections still do the job more cheaply.
pub const LANDING_START_RADII: f64 = 20.0;

/// How much speed change a rocket's landing looks ahead over, as a multiple of the speed it would
/// hit the target's surface at unbraked.
///
/// **Why this value.** Two. A rocket's landing is planned on the weakest acceleration it will have
/// while it brakes (see [`crate::spacecraft::Rocket::weakest_acceleration_m_s2`]), which means
/// knowing which stages it will light before it is down. Braking takes the impact speed plus what
/// gravity adds during the burn; at the lunar module's `3 m/s²` against the Moon's `1.62 m/s²`
/// that is somewhat under twice the impact speed. Without the look-ahead, a rocket that began
/// braking on the last of its third stage planned for `3 g`, emptied that stage part-way down, and
/// hit the Moon at `2.2 km/s` with most of its lander's propellant unburned.
///
/// **Used by.** [`crate::spacecraft::flight`], for a staged rocket only.
///
/// **If you change it.** Lower risks planning on a stage that empties before touchdown. Higher
/// plans on a weaker stage than the landing will reach, so it brakes earlier and more gently than
/// it needs to and loses more to gravity.
pub const LANDING_LOOKAHEAD_FACTOR: f64 = 2.0;
