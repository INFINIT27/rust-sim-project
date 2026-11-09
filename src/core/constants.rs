//! Every constant the `core` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module;
//! values shared with another layer — the unit scale, the shipped scene's masses
//! and distances — live in [`crate::constants`] instead.
//!
//! # Sections
//!
//! - **Validation tolerance** — [`ROTATION_UNIT_TOLERANCE`], the one number in the
//!   layer's `is_valid` predicates.
//! - **Gravitation** — [`GRAVITY_SOFTENING_UNITS`], which keeps the `1/r²`
//!   singularity out of the integrator.
//! - **The starter scene** — the bounds, floor, and colours
//!   [`crate::core::scene::Scene::demo`] and
//!   [`crate::core::scene::Scene::two_body_gravity`] build from.

use crate::core::scene::{Color, Vec3};

/// How far a quaternion's length may sit from `1.0` and still count as normalized.
///
/// **Why this value.** `1e-4` is loose enough to accept any quaternion that has
/// been through a few `f32` multiplications without renormalising — the drift from
/// composing rotations is orders of magnitude below it — and tight enough to reject
/// an unnormalised quaternion built by hand, which is the mistake it exists to
/// catch. The same tolerance is used for the contact normal in
/// [`crate::collision::contact::Contact::is_valid`], deliberately, so "is this
/// vector unit length" means one thing in the crate.
///
/// **Used by.** [`crate::core::scene::Pose::is_valid`], and through it
/// `Body::is_valid`, `Prop::is_valid`, and the `debug_assert!`s in `Scene::add` and
/// `Scene::step`.
///
/// **If you change it.** Tightening it toward zero starts rejecting legitimately
/// composed rotations and fires `debug_assert!`s in debug builds for values a
/// renderer would accept. Loosening it lets a visibly skewed rotation through, and
/// nothing downstream renormalises — this layer *checks* quaternions and never
/// fixes them.
pub const ROTATION_UNIT_TOLERANCE: f32 = 1.0e-4;

/// Plummer softening length for gravitation, in world **units**.
///
/// The pairwise force is evaluated at an effective separation of `√(r² + ε²)`
/// rather than at `r`. That leaves the force essentially unchanged when `r ≫ ε` and
/// caps it at a finite maximum as `r → 0`.
///
/// **Why this value.** It is deliberately close to the shipped sphere radius
/// ([`crate::constants::TWO_BODY_VISUAL_RADIUS_UNITS`] is `1.5`), which makes it
/// principled rather than a fudge: the gravitational field *inside* a real uniform
/// sphere flattens toward zero at the centre rather than diverging, so a softened
/// core is what a body of finite size actually has. At the shipped `30`-unit
/// starting separation it weakens the pull by `ε²/r² ≈ 0.1%`, which is invisible.
///
/// **Used by.** [`crate::core::integrator::gravitational_acceleration`], on every
/// pair, every substep.
///
/// **If you change it.** Setting it to `0.0` restores the true point-mass
/// singularity: a near-miss between two props with no hulls returns an
/// astronomically large force, the velocity overflows to `inf`, and
/// `Scene::step`'s Postcondition 5.1 breaks for reasons unrelated to the caller.
/// Raising it much above a body radius measurably weakens gravity at normal
/// separations, which slows the whole scene down. Since the `collision` layer
/// arrived this matters less for *solid* props — they now bounce before they can
/// coincide — but props with no hull (a zero-width line, a massless prop) can still
/// pass through each other and this is what keeps that finite.
pub const GRAVITY_SOFTENING_UNITS: f32 = 1.0;

/// Lower corner of the starter scene's bounds, in world units.
///
/// **Why this value.** `y = -1` rather than `0` so the ground plane at the origin
/// is strictly inside the volume rather than lying on its face, and `±50` in X and
/// Z so the `30`-unit two-body separation has room to swing without leaving the
/// world. The box spans about `0.67 AU`.
///
/// **Used by.** [`crate::core::scene::Scene::demo`], as `Scene::bounds.min`.
///
/// **If you change it.** `bounds` is advisory today — nothing clamps a prop to it,
/// and `Scene::step` leaves it untouched (Postcondition 5.3) — so this is
/// documentation of intent that a future containment or culling pass would read.
/// It must keep `min < max` on every axis to satisfy `Aabb::is_valid` and
/// Requirement 12.2.
pub const DEMO_BOUNDS_MIN_UNITS: Vec3 = Vec3::new(-50.0, -1.0, -50.0);

/// Upper corner of the starter scene's bounds, in world units.
///
/// **Why this value.** See [`DEMO_BOUNDS_MIN_UNITS`]. `y = 50` gives headroom well
/// above the spheres' `3`-unit start so a bounce cannot leave the volume.
///
/// **Used by.** [`crate::core::scene::Scene::demo`], as `Scene::bounds.max`.
///
/// **If you change it.** Same as [`DEMO_BOUNDS_MIN_UNITS`]: advisory, but must stay
/// strictly greater than the minimum on every axis.
pub const DEMO_BOUNDS_MAX_UNITS: Vec3 = Vec3::new(50.0, 50.0, 50.0);

/// Half the side length of the starter scene's ground plane, in world units.
///
/// **Why this value.** `5000` units makes the floor **effectively unbounded**: a
/// `10,000 × 10,000`-unit square whose edge sits beyond anything an experiment puts in it.
/// The figure that sets it is the solar system: Neptune starts `4495` units out, so a floor
/// that stopped short of that would leave the two outermost planets orbiting over a void
/// with no grid under them to trace against — and the grid is the only thing that makes a
/// `4495`-unit orbit legible.
///
/// It was `25.0`, then `50.0` (exactly the bounds' XZ extent), then `2000.0`. The remaining
/// problem with matching the bounds was that "inside the world" and "over the floor" were
/// the *same* boundary, so anything that left the bounds — a body ejected from the
/// three-body ring, say — left the floor in the same instant and fell into empty space.
///
/// # What "indefinitely" does and does not mean here
///
/// The plane is **finite geometry that you cannot reach the end of**, not a mathematical
/// half-space. A true infinite plane would need the collision layer to special-case a
/// half-space test, since an unbounded surface cannot be a closed triangle hull — see
/// `src/collision/README.md`. What this value buys is the *behaviour* of an infinite floor
/// at none of that cost; what it does not buy is a guarantee, so a prop launched past
/// `5000` units would still find the edge.
///
/// **Used by.** [`crate::core::scene::Scene::demo`], as the `Body::Plane`'s
/// `half_size`; and, indirectly, by
/// [`crate::render::grid::spawn_floor_grid`], which reads the floor's own `half_size` and
/// grids **all** of it — coarse lines at
/// [`GRID_COARSE_SPACING_UNITS`](crate::render::constants::GRID_COARSE_SPACING_UNITS) to the
/// edge, fine lines only out to
/// [`GRID_CORE_EXTENT_UNITS`](crate::render::constants::GRID_CORE_EXTENT_UNITS), because a
/// 5-unit rule over this footprint would be 1,602 entities of grey moiré.
///
/// **If you change it.** Cost is not the concern it looks like: the plane's collision hull
/// is a slab spanning `y ∈ [−1, 0]`, so a prop above the floor is still rejected by the
/// broad phase on the **vertical** axis alone, however wide the footprint gets. The grid
/// does scale with it — coarse lines every
/// [`GRID_COARSE_SPACING_UNITS`](crate::render::constants::GRID_COARSE_SPACING_UNITS) means
/// `202` line entities at this size against `82` at `2000`, which is still nothing.
/// Precision is the thing to watch — hull vertices at `±5000` are exact in `f32`, but a
/// floor at `1e6` units would start losing resolution where it matters. Note the *camera*
/// must be able to see this far: [`crate::render::constants::CAMERA_FAR_UNITS`] is what
/// makes the far edge drawable at all.
pub const DEMO_FLOOR_HALF_SIZE_UNITS: f32 = 5000.0;

/// Colour of the starter scene's ground plane.
///
/// **Why this value.** A mid grey, dark enough for the white crosshair to read
/// against and light enough to separate the floor from the clear colour.
///
/// **Used by.** [`crate::core::scene::Scene::demo`].
///
/// **If you change it.** Cosmetic. Channels must stay in `[0.0, 1.0]` or
/// `Color::is_valid` fails and `Scene::add` fires a `debug_assert!`. Linear space,
/// not sRGB — the render bridge converts.
pub const DEMO_FLOOR_COLOR: Color = Color {
    r: 0.35,
    g: 0.35,
    b: 0.35,
    a: 1.0,
};

/// Colour of the sphere that starts on the `−X` side.
///
/// **Why this value.** Red against the blue of [`TWO_BODY_RIGHT_COLOR`], so the two
/// spheres stay individually identifiable through the encounter — which is the
/// moment you most want to tell them apart, and now that they bounce rather than
/// pass through, the moment their identities would otherwise be ambiguous.
///
/// **Used by.** [`crate::core::scene::Scene::two_body_gravity`].
///
/// **If you change it.** Cosmetic, with the same `[0.0, 1.0]` validity constraint
/// as [`DEMO_FLOOR_COLOR`]. Keep the two spheres visually distinct.
pub const TWO_BODY_LEFT_COLOR: Color = Color {
    r: 0.85,
    g: 0.25,
    b: 0.2,
    a: 1.0,
};

/// Colour of the sphere that starts on the `+X` side.
///
/// **Why this value.** Blue against the red of [`TWO_BODY_LEFT_COLOR`] — see there.
///
/// **Used by.** [`crate::core::scene::Scene::two_body_gravity`].
///
/// **If you change it.** Cosmetic; keep it distinct from the other sphere and from
/// [`DEMO_FLOOR_COLOR`].
pub const TWO_BODY_RIGHT_COLOR: Color = Color {
    r: 0.2,
    g: 0.5,
    b: 0.95,
    a: 1.0,
};

/// The heaviest a body can be, as a fraction of a nearby body's mass, and still have its
/// trail drawn as a **moon** of it. Dimensionless.
///
/// **Why this value.** Being inside a planet's Hill sphere is not enough to call a body its
/// moon: in the three-body ring every body sits inside its equal-mass neighbour's nominal
/// Hill sphere, and drawing those trails relative to a neighbour would wreck the orbits that
/// experiment exists to show. The Hill formula assumes the moon is much lighter than its
/// primary, and this is the threshold for that. Earth's Moon is `0.0123` of Earth and the
/// Galilean moons are under `1e-4` of Jupiter, so `0.1` keeps every real moon and drops
/// every pair of comparable bodies.
///
/// **Used by.** [`crate::core::scene::moon_primary`], and through it trails and transfer planning.
///
/// **If you change it.** Above `1.0` a heavier body could be drawn as the moon of a lighter
/// one. Below `0.0123` Earth's Moon is dropped and its trail falls back to a straight chord
/// along Earth's path.
pub const MOON_MAX_MASS_RATIO: f32 = 0.1;

// ---------------------------------------------------------------------------------------------
// Transfer planning — `crate::core::transfer`
// ---------------------------------------------------------------------------------------------

/// The craft's step in the planner, as a fraction of the time it needs to cover its distance to the
/// nearest body at its current speed.
///
/// **Why this value.** `1/50` keeps the departure hyperbola, which starts `200 km` above Earth at
/// `15.4 km/s`, resolved to a few seconds per step, and lets the cruise open out to the ephemeris
/// step. The planner's integrator is first order, like the live one, so
/// accuracy here is bought with step count.
///
/// **Used by.** [`crate::core::transfer`]'s craft propagator.
///
/// **If you change it.** Smaller is more accurate and proportionally slower near planets. Much
/// larger and the departure arc is integrated in a handful of steps, so the planned launch
/// direction disagrees with the one the live scene would fly.
pub const TRANSFER_STEP_FRACTION: f64 = 0.02;

/// The smallest step the planner's craft propagator will take, in seconds.
///
/// **Why this value.** One second, a floor that only matters in a grazing pass of a body's
/// centre, where `distance / speed` heads to zero and the step would otherwise vanish.
///
/// **Used by.** [`crate::core::transfer`]'s craft propagator.
///
/// **If you change it.** Raising it toward a minute coarsens the departure from low orbit.
pub const TRANSFER_MIN_STEP_S: f64 = 1.0;

/// How close to the target's centre a planned trajectory must pass to count as aimed, in metres.
///
/// **Why this value.** `50 km`, against Mars's `3,390 km` radius. The planner aims at the
/// *centre* rather than merely at the disc, because the live scene integrates the rocket with
/// larger steps and rounds its position to `f32` (`15 km` at Mars's distance), and every metre of
/// margin left at the planning stage is one the flight can spend.
///
/// **Used by.** [`crate::core::transfer::plan_departure`] and
/// [`crate::core::transfer::plan_correction`].
///
/// **If you change it.** Tightening it below the `f32` spacing makes Newton's method chase
/// rounding noise and fail. Loosening it toward the radius lets a plan graze the limb, which the
/// live flight can then miss.
pub const TRANSFER_AIM_TOLERANCE_M: f64 = 5.0e4;

/// How many launch azimuths [`crate::core::transfer::plan_departure`] tries before refining.
///
/// **Why this value.** `120`, one every `3°`. The closest-approach distance to Mars is smooth in
/// azimuth with one or two dips, each several degrees wide, so this sees every one.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** Each azimuth costs one full flight, so this is most of a plan's craft
/// cost. Too few and a narrow dip — the fast branch, typically — is stepped over.
pub const TRANSFER_AZIMUTH_SAMPLES: usize = 120;

/// How many of the scan's closest dips are refined into full solutions.
///
/// **Why this value.** Four. There are usually two dips (the fast and the slow way to Mars), and
/// refining a couple of spares costs little and keeps a noisy scan from hiding the answer.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** One is cheaper and will sometimes refine the slow branch only.
pub const TRANSFER_CANDIDATES: usize = 4;

/// The most Newton iterations an aim may take before it is abandoned.
///
/// **Why this value.** Newton's method on the miss distance converges in three to six iterations
/// from a scan dip; twenty leaves room for a damped start without spinning on a hopeless one.
///
/// **Used by.** [`crate::core::transfer`]'s shared aiming loop.
///
/// **If you change it.** Lower and a slow-converging candidate is dropped; higher only costs time
/// on candidates that were never going to converge.
pub const TRANSFER_MAX_ITERATIONS: usize = 20;

/// How long after launch a voyage makes its first correction burn, in seconds.
///
/// **Why this value.** Three days, by which a craft leaving at `15.4 km/s` is `4 million km` out —
/// well clear of the Moon and of the departure arc. The live scene integrates that arc at the
/// substep cap, much more coarsely than the planner does, so the craft leaves Earth slightly off
/// the plan. The error is a velocity error and stops growing once the arc is behind it, so this
/// is the earliest point at which correcting it is cheap and final. Real missions put their first
/// trajectory-correction manoeuvre a few days after launch for a similar reason.
///
/// **Used by.** [`crate::core::transfer::Voyage::next_correction_at`].
///
/// **If you change it.** Earlier and the burn re-plans from inside the departure arc and repeats
/// the error it is meant to fix. Much later and the same velocity error has become a position error
/// that costs more to remove.
pub const TRANSFER_FIRST_CORRECTION_S: f64 = 3.0 * 86_400.0;

/// The rest of a voyage's correction burns, as fractions of the way from launch to arrival.
///
/// **Why these values.** Halfway, then progressively closer to Mars. Each burn removes the drift
/// since the last one; the live scene's `f32` positions round to `15 km` near Mars's orbit, so the
/// drift grows with distance flown and the last burn has to be close enough that what remains is
/// well inside Mars's `3,390 km` radius. Measured: the final predicted miss is under `10 km` and the
/// live flight passes `835 km` from the centre.
///
/// **Used by.** [`crate::core::transfer::Voyage::next_correction_at`].
///
/// **If you change it.** Dropping the late entries lets the live flight drift by more than Mars's
/// radius. Every entry costs one plan, during which the clock is paused.
pub const TRANSFER_CORRECTION_FRACTIONS: [f64; 4] = [0.5, 0.85, 0.95, 0.98];

/// The furthest a correction burn's own prediction may pass from the destination's centre and
/// still be applied, in destination radii.
///
/// **Why this value.** One radius: the corrected path must still be predicted to hit the body,
/// since landing is the goal. A correction aimed at the destination converges to within metres of
/// its centre, and one aimed at a planned flyby rejoins the planned route, which was itself aimed
/// at the centre (`22.6 km` from Callisto's in the Europa → Ganymede → Callisto plan). A burn that
/// misses by more has steered onto a different path. One such burn sent the craft `942,666 km`
/// wide, and every later burn then aimed at its wrong flyby.
///
/// **Used by.** [`crate::core::transfer::plan_correction`].
///
/// **If you change it.** Larger lets a burn that misses the destination through, leaving the craft
/// further from any path that lands. Below one, a correction that would land near the limb is
/// skipped and its slot wasted.
pub const TRANSFER_CORRECTION_MAX_MISS_RADII: f64 = 1.0;

/// How many samples a predicted ephemeris holds, whatever its horizon.
///
/// **Why this value.** `8,000`. The step is the horizon over this, so a six-hour hop to the Moon is
/// sampled every minute and a decade to Neptune every eleven hours. The samples hold velocities as
/// well as positions and are joined by cubic Hermite interpolation, whose error falls as the fourth
/// power of the step: Earth over an eleven-hour step is off by metres.
///
/// **Used by.** [`crate::core::transfer`]'s ephemeris. Its step is also the craft's longest step.
///
/// **If you change it.** Memory is this times the body count times 48 bytes, about `6 MB` for the
/// solar system. Fewer samples make a long cruise coarser in the planner and leave more for the
/// correction burns to fix.
pub const TRANSFER_EPHEMERIS_SAMPLES: usize = 8_000;

/// The shortest ephemeris step, in seconds.
///
/// **Why this value.** One minute, so a very short hop does not spend its samples on
/// sub-second intervals that `integrate_gravity` would only substep again.
///
/// **Used by.** [`crate::core::transfer`]'s ephemeris.
///
/// **If you change it.** Smaller only matters for flights of under two hours.
pub const TRANSFER_MIN_EPHEMERIS_STEP_S: f64 = 60.0;

/// The first horizon a departure is searched over, as a multiple of the straight-line time at the
/// departure speed.
///
/// **Why this value.** `2`. A real path curves and the target moves, so the straight-line time is a
/// lower bound. Twice it holds the Moon (`6.9 h` straight, about `7 h` flown) and Earth–Mars at
/// close approach. When it holds nothing the horizon doubles, so a low value costs one extra
/// attempt rather than a missed path.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** Much larger makes short hops pay for an ephemeris far longer than the
/// flight, and every attempt's cost is linear in its horizon.
pub const TRANSFER_HORIZON_FACTOR: f64 = 2.0;

/// The shortest horizon any plan is searched over, in seconds.
///
/// **Why this value.** One hour: two moons almost touching would otherwise ask for an ephemeris of a
/// few minutes, too short for the correction schedule to fit inside.
///
/// **Used by.** [`crate::core::transfer::plan_departure`] and
/// [`crate::core::transfer::plan_correction`].
///
/// **If you change it.** Only very short hops notice.
pub const TRANSFER_MIN_HORIZON_S: f64 = 3_600.0;

/// How many points a drawn path is made of, measured against the distance between start and target.
///
/// **Why this value.** `500`: a point every five-hundredth of the starting separation. Earth–Mars
/// gets a point every `0.6 Gm`, the Moon every `770 km`, so every path is smooth at the zoom that
/// shows it.
///
/// **Used by.** [`crate::core::transfer`], for `TransferPlan::path_units` and
/// `Voyage::flown_path_units`.
///
/// **If you change it.** More costs gizmo lines; fewer turns the departure arc into a polygon.
pub const TRANSFER_PATH_POINTS: f64 = 500.0;

/// The first correction burn of a short flight, as a fraction of the flight.
///
/// **Why this value.** A tenth. Three days is right for Mars, but a hop to the Moon takes hours, so
/// the first burn comes at whichever is earlier, three days or a tenth of the flight. At a tenth the
/// craft is several origin radii out and past most of the departure arc.
///
/// **Used by.** [`crate::core::transfer::Voyage::next_correction_at`].
///
/// **If you change it.** Much smaller and the burn is planned from inside the departure arc.
pub const TRANSFER_FIRST_CORRECTION_FRACTION: f64 = 0.1;

/// How many times an aiming step may be halved before the aim is abandoned.
///
/// **Why this value.** Five, down to `1/32` of the Newton step. Near a strong gravity well the miss
/// is far from linear in the launch angle, and an undamped step can bounce between two points
/// forever, which is exactly what a Jupiter launch did before this existed.
///
/// **Used by.** [`crate::core::transfer`]'s shared aiming loop.
///
/// **If you change it.** More halvings rescue more marginal candidates, at one flight each.
pub const TRANSFER_LINE_SEARCH_HALVINGS: usize = 5;

/// The longest an ephemeris step may be, as a fraction of the shortest local dynamical time
/// (`√(r³/GM)`, a period over `2π`) of any body left in the prediction.
///
/// **Why this value.** `0.1`, about `63` samples per orbit. Cubic Hermite interpolation over a
/// tenth of a radian of orbit is off by `(0.1)⁴ / 384` of the radius, which for Mercury is `15 km`
/// and for the Moon about `250 m` from Earth. Short horizons never reach this cap, because their
/// step is already the horizon over [`TRANSFER_EPHEMERIS_SAMPLES`]. Long ones do, and from then
/// on the sample count grows with the horizon: about `50 MB` a century when the Moon is kept,
/// a fifth of that when it is folded.
///
/// **Used by.** [`crate::core::transfer`]'s ephemeris.
///
/// **If you change it.** Larger lets a long search run in less memory and puts the fastest orbits
/// where they are not. Past about `0.5` the Moon is visibly in the wrong place during departure.
pub const TRANSFER_EPHEMERIS_ORBIT_FRACTION: f64 = 0.1;

/// The departure speeds a search scans, as fractions of the craft's cap.
///
/// **Why these values.** The fastest arrival is usually at the cap, but launch is *now*, whatever
/// the geometry, and from a bad one a slower departure can drop onto an orbit that meets the target
/// sooner than any fast one does. Five rungs from the cap down to `15%` cover a Hohmann transfer to
/// Mars (`2.9 km/s`, `19%` of `15.4`) through to the cap.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** Each rung is a full direction scan, so this multiplies the cost of every
/// plan. With only `1.0` the planner is back to leaving at the cap every time.
pub const TRANSFER_SPEED_FRACTIONS: [f64; 5] = [1.0, 0.75, 0.5, 0.3, 0.15];

/// The first speed step of the hill climb after the ladder, as a fraction of the cap.
///
/// **Why this value.** Half the gap between the ladder's top rungs, so the climb starts inside the
/// interval the best rung stands for.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** Larger jumps past nearby better speeds; smaller needs more rounds.
pub const TRANSFER_SPEED_REFINE_FRACTION: f64 = 0.125;

/// How many times the speed hill climb halves its step.
///
/// **Why this value.** Three rounds narrow the best speed to `1.5%` of the cap, `0.2 km/s`, below
/// which the arrival time barely moves.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** Each round is two seeded aims.
pub const TRANSFER_SPEED_REFINE_ROUNDS: usize = 3;

/// How far beyond the two-body coasting reach a target may lie and still be searched for, as a
/// factor on that reach.
///
/// **Why this value.** `1.05`. The reach bound is exact for the craft and the primary alone. The
/// other planets nudge the craft's energy by a few percent on an ordinary pass. Only a close flyby,
/// a gravity assist, changes it by more, and the search does not aim for those. Without this check,
/// Mercury to Jupiter searched past a thousand years for a path that cannot exist.
///
/// **Used by.** [`crate::core::transfer::plan_departure`].
///
/// **If you change it.** Larger lets the search try targets it can only reach by luck, which can
/// mean searching forever. `1.0` rejects targets right at the edge that perturbations make reachable.
pub const TRANSFER_REACH_MARGIN: f64 = 1.05;

/// How far past a flyby body a flight aimed at it keeps going, as a multiple of its closest pass.
///
/// **Why this value.** `20`. A flight aimed at a point beside a planet only needs to reach the
/// closest point and a little beyond it to know it got there, and stopping then saves flying the
/// rest of a decades-long horizon on every Newton step. Twenty closest-pass distances is well
/// past the pass for any periapsis, and short against any interplanetary leg.
///
/// **Used by.** [`crate::core::transfer`]'s trial flights.
///
/// **If you change it.** Near `1` a flight can stop before its true closest point, which makes
/// the aim noisy. Larger is only slower.
pub const TRANSFER_FLYBY_PASSED_FACTOR: f64 = 20.0;

/// The most planets a route may pass for a gravity assist on its way to the destination.
///
/// **Why this value.** Two, enough for Mercury→Earth→Jupiter or Earth→Venus→Earth→Jupiter, which
/// are the shapes real missions fly. Each extra flyby nests one more level of aiming, which
/// multiplies every trial by the grid of aim points below it, so two-flyby routes are only searched
/// when no one-flyby route arrives.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** `0` turns gravity assists off. Above `2` is not supported: the search
/// only builds routes of one or two planets.
pub const TRANSFER_MAX_FLYBYS: usize = 2;

/// How many launch directions toward the first flyby planet each route search starts from.
///
/// **Why this value.** Six. Each seed is a different encounter with the planet, and whether a flyby
/// sends the craft to the target depends on where the target is when it gets there: a timing match
/// that one encounter in several makes. Two seeds found no Mercury→Earth→Jupiter route at any
/// horizon.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Each seed reruns every route that starts with that planet.
pub const TRANSFER_FLYBY_SEEDS: usize = 6;

/// How many directions around a flyby planet the aim-point grid tries.
///
/// **Why this value.** Six, one every `60°`. The side of the planet the craft passes on sets which
/// way it is turned, and six sides cover every turn to within a grid step the refinement closes.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** The grid is this times [`TRANSFER_FLYBY_RADII`] solves per flyby.
pub const TRANSFER_FLYBY_ANGLES: usize = 6;

/// The distances from a flyby planet's centre the aim-point grid tries, in planet radii.
///
/// **Why these values.** From just above the surface, where the turn is greatest, out to sixteen
/// radii, doubling each ring, where it is small, spaced about evenly in logarithm because the turn falls off roughly as
/// one over the distance.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Below `1.0` the craft hits the planet. Fewer rings miss narrow routes.
pub const TRANSFER_FLYBY_RADII: [f64; 5] = [1.2, 2.0, 4.0, 8.0, 16.0];

/// How many of the closest aim points of each flyby grid are refined.
///
/// **Why this value.** Two. More rarely find a faster route and cost a full refinement each.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Each extra candidate refines, and at a first flyby recurses into, one more.
pub const TRANSFER_FLYBY_CANDIDATES: usize = 2;

/// How long before each flyby a gravity-assisted voyage makes a correction burn, in seconds.
///
/// **Why these values.** Five days and one day. A flyby multiplies an error in where the craft
/// passes the planet by the ratio of the next leg to the pass distance, often thousands of times,
/// so the craft must arrive at the pass on its aim point. The first burn removes the cruise's drift,
/// the second the residue of the first, close enough that little can grow after it.
///
/// **Used by.** [`crate::core::transfer::Voyage::next_correction_at`].
///
/// **If you change it.** Without them the burn after the flyby has to undo a miss magnified by it.
pub const TRANSFER_FLYBY_CORRECTION_LEADS_S: [f64; 2] = [5.0 * 86_400.0, 86_400.0];

/// How long after each flyby a gravity-assisted voyage makes a correction burn, in seconds.
///
/// **Why this value.** One day, once the craft is clear of the planet, to take out what the pass
/// added while the error is still small.
///
/// **Used by.** [`crate::core::transfer::Voyage::next_correction_at`].
///
/// **If you change it.** Later and the post-flyby error has longer to grow.
pub const TRANSFER_FLYBY_CORRECTION_AFTER_S: f64 = 86_400.0;

/// The most of a leg a flyby correction's lead or lag may take, as a fraction of the time from
/// launch to that flyby.
///
/// **Why this value.** A tenth, so the burns around a flyby hours after launch, as between Jupiter's
/// moons, still come after launch and close to the pass.
///
/// **Used by.** [`crate::core::transfer::Voyage::next_correction_at`].
///
/// **If you change it.** Near `1` the lead can fall before launch and the burn is skipped.
pub const TRANSFER_FLYBY_LEAD_FRACTION: f64 = 0.1;

/// How far along its track a trial flight's closest point may sit from the goal, as a fraction of
/// its distance from the body, and still count as having reached it.
///
/// **Why this value.** One percent. The closest point is found on a straight segment across one
/// integration step, which near a planet is off along the curving track by kilometres, so the
/// along-track part cannot meet a metre's tolerance. A flight cut off by its window before reaching
/// the body is off along the track by about its whole distance, so one percent still rejects it.
///
/// **Used by.** [`crate::core::transfer`]'s aiming loop.
///
/// **If you change it.** Near `1` the window-cut flights it exists to reject pass as hits. Near `0`
/// aims at points beside a planet stall short of their tolerance.
pub const TRANSFER_ALONG_TRACK_FRACTION: f64 = 0.01;

/// How close to an aim point beside a planet a launch must put the craft, as a fraction of the
/// planet's radius.
///
/// **Why this value.** A thousandth: `6 km` at Earth, against aim points from `1.2` radii out. The
/// next level starts from these angles and aims afresh, so this only has to put the grid point where
/// it says it is.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Tighter costs Newton steps per grid point for no better route.
pub const TRANSFER_FLYBY_AIM_FRACTION: f64 = 1.0e-3;

/// How far a correction's pass may turn from the planned approach direction and still count as the
/// planned flyby, in radians.
///
/// **Why this value.** Ten degrees. A correction removes drift of a few hundred m/s at most, which
/// turns an approach of about `10 km/s` by `1–3°`, so the planned pass stays well inside. Another
/// encounter with the same planet comes from wherever the planet has moved to by then: in the
/// Europa → Ganymede → Callisto test, the pass a correction found `30` hours late approached `59°`
/// off, sat on the aim point within the plane it is compared in, and sent the craft `942,666 km`
/// wide of Callisto.
///
/// **Used by.** [`crate::core::transfer`]'s flyby corrections.
///
/// **If you change it.** Wider lets a different encounter pass for the planned one; narrower
/// rejects the planned pass once a correction has bent its approach.
pub const TRANSFER_FLYBY_MAX_TURN_RAD: f64 = 10.0 * std::f64::consts::PI / 180.0;

/// The first Newton probe of an assist aim, in radians of launch angle, before it has measured how
/// sensitive the path is.
///
/// **Why this value.** `1e-9`. Behind one flyby the path moves about `1e15 m` per radian, so this
/// moves it a megametre, which every aim can measure. From then on the probe adapts.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Much larger and the first derivative behind a flyby is not linear.
pub const TRANSFER_FLYBY_FIRST_PROBE_RAD: f64 = 1.0e-9;

/// The first step limit of an assist aim, in radians of launch angle, before it adapts.
///
/// **Why this value.** `1e-6`, about what moves the first pass by a tenth of a planet's radius.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Larger and the first step can jump to a different route.
pub const TRANSFER_FLYBY_FIRST_STEP_RAD: f64 = 1.0e-6;

/// What an adaptive aim probes for, as a fraction of its length scale.
///
/// **Why this value.** A thousandth: a change large against `f64` rounding of the path and small
/// against the scale over which it stops being linear.
///
/// **Used by.** [`crate::core::transfer`]'s aiming loop, with `Adapt`.
///
/// **If you change it.** Within a decade either way is fine.
pub const TRANSFER_ADAPTIVE_PROBE_FRACTION: f64 = 1.0e-3;

/// The furthest an adaptive aim steps, as a fraction of its length scale.
///
/// **Why this value.** One: a step may move the path by up to the current miss, which is what a
/// full Newton step does, and never by more. The scale is floored at the planet's radius, so close
/// to an aim point a step cannot jump the craft through the planet. The line search halves any step
/// that does not help.
///
/// **Used by.** [`crate::core::transfer`]'s aiming loop, with `Adapt`.
///
/// **If you change it.** Larger lets a bad linearisation throw the aim away; much smaller slows
/// convergence from a far seed, which at `0.5` of a planet radius took more iterations than an aim
/// is allowed.
pub const TRANSFER_ADAPTIVE_STEP_FRACTION: f64 = 1.0;

/// How far outside the band between the origin's and target's orbits a planet may orbit and still
/// be tried as a flyby, as a factor on that band's edges.
///
/// **Why this value.** `1.3`. A planet a little beyond the target can still bend the craft back onto
/// it, but one far outside the band, such as Saturn on the way from Mercury to Jupiter, only helps by
/// overshooting and returning. That is never fastest, and it cost seconds per route.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Larger tries more planets for routes that do not win; `1.0` drops planets
/// just past the target that sometimes do.
pub const TRANSFER_FLYBY_BAND_MARGIN: f64 = 1.3;

/// How near a planet, in its Hill radii, a scanned launch must pass to seed a flyby there.
///
/// **Why this value.** Twenty: `30 million km` at Earth. The scan steps the launch direction by `3°`,
/// which moves an encounter an AU away by millions of kilometres, more than Earth's whole
/// `1.5 million km` Hill sphere. Keeping only passes inside the Hill sphere found one or two
/// encounters per planet; at twenty radii every near miss is kept, and Newton's method pulls it in.
///
/// **Used by.** [`crate::core::transfer`]'s assist search.
///
/// **If you change it.** Larger seeds from misses too far off for Newton to pull in; smaller misses
/// encounters.
pub const TRANSFER_FLYBY_SCAN_HILL_RADII: f64 = 20.0;

/// How far either side of a planned flyby's time a pass of that planet still counts as that flyby,
/// as a fraction of the time to it.
///
/// **Why this value.** A fifth. A correction must find the same pass it was planned around after the
/// drift it removes, which shifts the pass by hours or days. The planet's next encounter must fall
/// outside the window: with half-to-one-and-a-half the time, a correction aimed at Ganymede's pass
/// on day 43 captured its next one on day 56 and lost the route.
///
/// **Used by.** [`crate::core::transfer`]'s assist search and flyby corrections.
///
/// **If you change it.** Wider risks the next encounter; narrower risks losing the planned pass to
/// drift.
pub const TRANSFER_FLYBY_WINDOW_FRACTION: f64 = 0.2;

/// The longest horizon searched for a gravity-assisted route when no direct path exists, in seconds.
///
/// **Why this value.** Twelve years, one orbit of Jupiter: past that the same planetary alignments
/// come round again, so a longer search mostly repeats encounters already tried. A direct search has
/// no limit because `coasting_reach` rules out the targets it could chase forever; nothing bounds an
/// assisted one, and Mercury to Jupiter searched for minutes without finding a route.
///
/// **Used by.** [`crate::core::transfer::plan_departure`], which then reports the target out of
/// reach.
///
/// **If you change it.** Each doubling of the horizon roughly doubles the search time.
pub const TRANSFER_ASSIST_ONLY_HORIZON_S: f64 = 12.0 * 365.25 * 86_400.0;

/// The first Newton probe of [`crate::core::transfer::solve_coast`], in m/s of launch velocity.
///
/// **Why this value.** One centimetre per second, the same probe a correction burn uses: small
/// enough that the path responds linearly, large enough that the response is far above the
/// integration's rounding.
///
/// **Used by.** [`crate::core::transfer::solve_coast`].
///
/// **If you change it.** Much larger and the derivative is no longer linear on a long arc. Much
/// smaller and it drowns in rounding.
pub const COAST_SOLVE_PROBE_M_S: f64 = 0.01;

/// The largest single Newton step of [`crate::core::transfer::solve_coast`], in m/s per component.
///
/// **Why this value.** One kilometre per second. Its guess is a two-body (Lambert) solution, which
/// leaves out the origin bending the departure and the other planets, and is off by up to a few
/// hundred m/s; a step this size reaches across that in one or two iterations, while a bad
/// linearisation cannot throw the solve onto a different arc.
///
/// **Used by.** [`crate::core::transfer::solve_coast`].
///
/// **If you change it.** Smaller needs more iterations than it is allowed from a Lambert guess.
/// Larger lets one bad step leave the guess's basin.
pub const COAST_SOLVE_MAX_STEP_M_S: f64 = 1_000.0;
