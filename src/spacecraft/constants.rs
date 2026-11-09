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
