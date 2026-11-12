//! Every constant the `experiments` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module. The
//! **two-body** experiment's own numbers are not here — they are
//! [`crate::constants`]'s `TWO_BODY_*` values, because `core` builds that scene and
//! `render` frames it, so two layers already read them. Everything below either
//! belongs to the three-body ring or governs experiment selection.
//!
//! # Sections
//!
//! - **Camera framing per experiment** — where the observer starts for each, and how
//!   much of the scene that opening view is meant to contain.
//! - **The three-body ring** — geometry, mass, and the random-speed window.
//! - **The solar system** — the drawn-size scale, the body table, and the one place
//!   real proportion is deliberately broken.
//! - **Interstellar objects** — [`STELLAR_OBJECTS`], the dwarfs, neutron star, and black
//!   holes the `B` key can place into a running experiment.
//! - **Moons** — [`EARTH_MOONS`], [`JOVIAN_MOONS`], and [`MOON_SYSTEMS`], which attaches each set to
//!   its primary by name.
//! - **The Jovian system** — `experiment_4`: the Galilean moons.
//! - **The scale check** — `experiment_5`: the static size-comparison row.
//! - **The black hole** — `experiment_6`: Sgr A*, spinning, with its disc, and the S-stars round it.
//! - **Randomness** — the fallback seed.

pub use bevy::math::Vec3;

use crate::core::scene::Color;
use crate::experiments::black_hole::SStar;
use crate::experiments::objects::StellarObject;
use crate::experiments::solar_system::{MoonSystem, Planet};

/// Where the camera starts for the two-body experiment, in world units.
///
/// **Why this value.** Framing arithmetic, not taste: the spheres sit at `x = ±15`,
/// so the view needs a half-width of at least ~17 units. Bevy's default projection
/// has a 45° vertical field of view — a 22.5° half-angle — so the camera must be at
/// least `17 / tan(22.5°) ≈ 41` units away. `(0, 18, 42)` is ~44.6 units from the
/// look-at point, leaving margin for the spheres to swing past each other without
/// leaving the frame, and the `y = 18` elevation shows the floor for scale rather
/// than viewing it edge-on.
///
/// **Used by.** [`crate::experiments::catalog::two_body`], which puts it in the
/// experiment's [`ExperimentSpec`](crate::experiments::catalog::ExperimentSpec);
/// `spawn_scene_entities` then reads the spec rather than a constant, which is what
/// lets each experiment frame itself.
///
/// **If you change it.** Pinned by
/// `plugin_tests::the_startup_camera_frames_the_two_body_pair`, which recomputes the
/// minimum distance from [`crate::constants::TWO_BODY_SEPARATION_UNITS`] and the
/// field of view — so widening the scene without moving the camera fails there rather
/// than silently pushing the spheres off screen.
pub const TWO_BODY_CAMERA_POSITION_UNITS: Vec3 = Vec3::new(0.0, 18.0, 42.0);

/// Where the camera starts for the three-body ring, in world units.
///
/// **Why this value.** The ring has radius [`THREE_BODY_RING_RADIUS_UNITS`] plus a
/// body radius, so the view needs a half-width of ~13 units and therefore a distance
/// of at least `13 / tan(22.5°) ≈ 31` units. `(0, 26, 34)` is ~39 units from the ring
/// centre, and the steep elevation is deliberate: the ring lies in a **horizontal**
/// plane, so a low camera would see it edge-on as a line and the whole point of the
/// experiment — what the three orbits do to each other — would be invisible.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`].
///
/// **If you change it.** Pinned by
/// `catalog_tests::every_experiment_frames_its_own_scene`, which checks the camera is
/// far enough out for the scene it is paired with. Lower the elevation and you trade
/// the plan view for a horizon view; the bodies stay in frame either way.
pub const THREE_BODY_CAMERA_POSITION_UNITS: Vec3 = Vec3::new(0.0, 26.0, 34.0);

/// Radius about the camera target that the two-body experiment's opening view is meant
/// to contain, in world units.
///
/// **Why this value.** The spheres start at `x = ±15` with a
/// [`radius`](crate::constants::TWO_BODY_VISUAL_RADIUS_UNITS) of `1.5`, so `16.5` units
/// covers the whole scene; `17` adds a little margin. For this experiment the framed
/// radius *is* the scene's extent — every body is in the opening frame.
///
/// **Used by.** [`crate::experiments::catalog::two_body`], as the spec's
/// [`framed_radius_units`](crate::experiments::catalog::ExperimentSpec::framed_radius_units),
/// which is what `catalog_tests::every_experiment_frames_its_own_scene` checks the camera
/// distance against.
///
/// **If you change it.** Raising it without moving
/// [`TWO_BODY_CAMERA_POSITION_UNITS`] back fails that test, which is the point: it is the
/// declaration the camera distance is verified against, not a decoration.
pub const TWO_BODY_FRAMED_RADIUS_UNITS: f32 = 17.0;

/// Radius about the camera target that the three-body ring's opening view is meant to
/// contain, in world units.
///
/// **Why this value.** [`THREE_BODY_RING_RADIUS_UNITS`] (`12`) plus
/// [`THREE_BODY_VISUAL_RADIUS_UNITS`] (`1`): the circle the three bodies start on, to
/// their outer edge. As with the two-body pair, this is the whole scene — nothing starts
/// off screen.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`].
///
/// **If you change it.** See [`TWO_BODY_FRAMED_RADIUS_UNITS`]. Note the bodies *leave*
/// this radius as the experiment runs — an ejected body can go anywhere — so this
/// describes the opening frame, not a bound on the motion.
pub const THREE_BODY_FRAMED_RADIUS_UNITS: f32 = 13.0;

/// Radius of the circle the three bodies start on, in world units.
///
/// **Why this value.** `12` units is `1.2e10 m`, which with Earth masses gives a
/// notional orbital period of `5.4e8 s` (about 17 years) — long enough that the
/// interaction plays out over a watchable interval at
/// [`THREE_BODY_TIME_SCALE`], and small enough that the ring fits inside the
/// `±25`-unit floor with room to spare. It is also comfortably wider than the
/// `1`-unit bodies, so the three start clearly separated rather than already in
/// contact.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`], to place all three
/// bodies and to derive their circular-orbit speed.
///
/// **If you change it.** The notional period scales as `R^{3/2}`, so
/// [`THREE_BODY_TIME_SCALE`] wants adjusting to keep the ~27-second framing, and
/// [`THREE_BODY_CAMERA_POSITION_UNITS`] wants moving back to keep the ring in frame.
/// Past `25` units the bodies start outside the floor's footprint, which — since the
/// floor is solid and finite — means they fall past it rather than landing.
pub const THREE_BODY_RING_RADIUS_UNITS: f32 = 12.0;

/// Height of the ring's plane above the floor, in world units.
///
/// **Why this value.** High enough that a body can be pulled inward and downward for
/// a while before it reaches the floor, so the floor is a boundary rather than an
/// immediate participant. `6` units is six body radii.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`], as the `y` of every
/// body and of the camera's look-at target.
///
/// **If you change it.** Below [`THREE_BODY_VISUAL_RADIUS_UNITS`] the bodies start
/// intersecting the ground slab and are pushed out of it on the first step. Well
/// above it, the floor stops mattering at all — which is a legitimate variant, and
/// the reason this is a constant rather than a literal.
pub const THREE_BODY_HEIGHT_UNITS: f32 = 6.0;

/// Radius of each of the three bodies, in world units.
///
/// **Why this value.** Visibility, and contact: at `1` unit the bodies are large
/// enough to see from 39 units away and small enough that the `12`-unit ring reads as
/// a ring rather than as three touching spheres. Since the `collision` layer arrived
/// this is also **where they stop** — three bodies on converging orbits do collide,
/// and this sets when.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`].
///
/// **If you change it.** Contact separation is `2 ×` this. Note the interaction with
/// [`crate::core::constants::GRAVITY_SOFTENING_UNITS`] (`1.0`): a body much smaller
/// than the softening length can pass close to another and feel a *weaker* pull than
/// Newton's law gives, because softening assumes a body of finite size.
pub const THREE_BODY_VISUAL_RADIUS_UNITS: f32 = 1.0;

/// Mass of each of the three bodies, in kilograms.
///
/// **Why this value.** One Earth mass, the same as the two-body experiment
/// ([`crate::constants::EARTH_MASS_KG`]), so the two experiments are comparable: the
/// only differences are the geometry and the initial velocities. Named separately
/// rather than reusing that constant because "the three-body bodies happen to be
/// Earth-massed" is a choice this experiment makes, not a fact about the other one.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`], for all three
/// bodies and for the circular-orbit speed they are randomised around.
///
/// **If you change it.** The circular-orbit speed scales as `√m` and the notional
/// period as `1/√m`, so [`THREE_BODY_TIME_SCALE`] wants adjusting with it. Unequal
/// masses would be a genuinely different experiment — and an interesting one — but
/// would need three constants rather than one.
pub const THREE_BODY_MASS_KG: f32 = 5.972e24;

/// Simulated seconds per real second for the three-body ring.
///
/// **Why this value.** The notional circular-orbit period at
/// [`THREE_BODY_RING_RADIUS_UNITS`] with [`THREE_BODY_MASS_KG`] is `5.4e8 s`, about
/// 17 years. At `2e7` that is ~27 seconds of watching per orbit: long enough to see
/// the geometry evolve, short enough to see several outcomes in a sitting. At 60 fps
/// each frame advances `3.3e5` simulated seconds, moving a body about `0.05` units —
/// well inside
/// [`crate::collision::constants::MAX_SUBSTEP_DISPLACEMENT_UNITS`], so contact is
/// resolved without substepping in the common case. At the `max_dt` clamp a frame
/// advances `2e6 s` and moves a body up to `0.36` units, which does split into two
/// substeps — so unlike the two-body scene, this experiment genuinely exercises the
/// substep loop.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`], as the spec's time
/// scale, which `ScenePlugin` puts on the [`SimulationClock`](crate::time::simulation::SimulationClock).
///
/// **If you change it.** Same tradeoff as
/// [`crate::constants::TWO_BODY_TIME_SCALE`]: faster is coarser. This scene is more
/// sensitive than the two-body one, because a three-body system is chaotic — two runs
/// at different scales will diverge from the *same* initial conditions, and neither is
/// more correct than the other at a first-order integrator's accuracy. The `[` and `]`
/// keys change it at runtime, so there is no need to edit this to look around.
pub const THREE_BODY_TIME_SCALE: f32 = 2.0e7;

/// Slowest starting speed for a three-body body, as a fraction of the circular-orbit
/// speed. Dimensionless.
///
/// **Why this value.** `0.4` is well below the speed that would hold a circular ring,
/// so a body starting here falls inward — which is half of what makes the experiment
/// worth running. Much below this and every run collapses immediately, which is one
/// outcome rather than a range of them.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`], as the lower bound
/// of the random speed window.
///
/// **If you change it.** Together with [`THREE_BODY_MAX_SPEED_FACTOR`] this sets how
/// varied the outcomes are. A window that straddles `1.0` — as this one does — gives
/// collapses, near-rings, and escapes from the same experiment; a narrow window around
/// `1.0` gives a nearly-stable rotating triangle every time.
pub const THREE_BODY_MIN_SPEED_FACTOR: f32 = 0.4;

/// Fastest starting speed for a three-body body, as a fraction of the circular-orbit
/// speed. Dimensionless.
///
/// **Why this value.** `1.3` is above the circular speed but below the escape speed
/// for this configuration, so a body starting here swings wide and comes back rather
/// than leaving for good — usually. With three bodies exchanging energy, one *can*
/// still be ejected, and that is a legitimate outcome to observe rather than a bug.
///
/// **Used by.** [`crate::experiments::catalog::three_body_ring`], as the upper bound
/// of the random speed window.
///
/// **If you change it.** See [`THREE_BODY_MIN_SPEED_FACTOR`]. Past about `1.5` most
/// runs end with everything dispersed and nothing left to watch, and the scene's
/// `±50`-unit bounds are advisory — nothing stops a body leaving the world.
pub const THREE_BODY_MAX_SPEED_FACTOR: f32 = 1.3;

/// Equatorial radius of the Earth, in metres — the **denominator of the solar system's
/// drawn-size scale**.
///
/// **Why this value.** The IUGG/IAU figure `6.371e6 m` (the volumetric mean radius).
/// Every body in `experiment_3` is drawn at `real_radius / this × `
/// `SOLAR_SYSTEM_EARTH_DRAWN_RADIUS_UNITS` (removed by A16), so this is what makes Earth exactly one
/// unit across the radius and every other body's size *relative to Earth* exactly real.
///
/// **Used by.** [`crate::experiments::solar_system::radius_units`], which every
/// body in the solar-system scene is sized through.
///
/// **If you change it.** Every drawn radius in `experiment_3` rescales together, so the
/// proportions survive and only the absolute size changes — which is
/// `SOLAR_SYSTEM_EARTH_DRAWN_RADIUS_UNITS` (removed by A16)'s job, so change that instead. This is a
/// measurement, not a tunable.
pub const EARTH_RADIUS_M: f64 = 6.371e6;

/// Mass of the Sun, in kilograms.
///
/// **Why this value.** The IAU figure `1.989e30 kg`, `333,000` Earth masses. It sets
/// every orbital speed in the experiment — `v = √(G·M/a)` — so a rounded-off value would
/// show up directly as wrong orbital periods.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`], for the central
/// body's mass and for
/// [`circular_orbit_speed_units_s`](crate::experiments::solar_system::circular_orbit_speed_units_s).
///
/// **If you change it.** Every planet's speed scales as `√M` and every period as
/// `1/√M`, so the whole experiment speeds up or slows down together and
/// [`SOLAR_SYSTEM_TIME_SCALE`] wants adjusting with it. Note the bodies stay on the same
/// *paths*: the initial positions are unchanged, only the speeds that keep them circular.
pub const SOLAR_SYSTEM_SUN_MASS_KG: f32 = 1.989e30;

/// Equatorial radius of the Sun, in metres.
///
/// **Why this value.** The IAU nominal solar radius, `6.957e8 m` — `109.2` Earth radii.
/// Recorded even though the Sun is **not** drawn at this scale, because it is what
/// `SOLAR_SYSTEM_SUN_DRAWN_RADIUS_UNITS` (removed by A16) departs from and the number that shows why the
/// departure is necessary.
///
/// **Used by.** `catalog_tests`, to pin the ratio the docs quote. Deliberately *not* used
/// to size the drawn Sun.
///
/// **If you change it.** Nothing about the running scene moves. It is a measurement kept
/// next to the compromise it explains.
pub const SOLAR_SYSTEM_SUN_RADIUS_M: f64 = 6.957e8;

/// Colour of the Sun, in linear RGBA.
///
/// **Why this value.** A bright yellow-white, the brightest thing in the scene, so the
/// centre reads as the star even though it is drawn far smaller than its real
/// proportion. The scene is lit by a *directional* light rather than by the Sun itself
/// (see `src/render/README.md`), so this colour is doing the whole job of saying "star".
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`].
///
/// **If you change it.** Cosmetic, with one caveat: [`Color`] is **linear**, not sRGB, so
/// a value copied from a colour picker will come out brighter than it looked.
pub const SOLAR_SYSTEM_SUN_COLOR: Color = Color {
    r: 1.0,
    g: 0.85,
    b: 0.35,
    a: 1.0,
};

/// Height of the ecliptic plane above the floor, in world units.
///
/// **Why this value.** The whole system is built in one horizontal plane at this height.
/// It has to clear the floor's collision slab — which spans `y ∈ [−1, 0]` — by more than
/// the largest body's radius, or the Sun would start embedded in the ground and be pushed
/// out of it. `SOLAR_SYSTEM_SUN_DRAWN_RADIUS_UNITS` (removed by A16) is `20`, so `30` leaves `10` units
/// of clearance.
///
/// It is also small compared to the orbits (`58`–`4495` units), so looking down at the
/// system from above, the grid on the floor still reads as the plane the planets are
/// moving in.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`], as the `y` of every
/// body and of the camera's look-at target.
///
/// **If you change it.** Must stay above `SOLAR_SYSTEM_SUN_DRAWN_RADIUS_UNITS` or the
/// Sun intersects the floor and the experiment starts by resolving a contact — caught by
/// `no_experiment_starts_with_bodies_already_touching`. Raising it far above the orbits
/// would tilt the grid's usefulness away, since the floor would no longer sit under the
/// motion.
pub const SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS: f32 = 30.0;

/// Simulated seconds per real second for the solar system.
///
/// **Why this value.** There is no single right answer here, and that is worth stating
/// plainly: **Mercury's year is 88 days and Neptune's is 165 years**, a spread of `685×`.
/// No clock scale makes both watchable.
///
/// `1e6` is chosen for the **inner** system, which is what the opening camera frames: one
/// Earth year every `31.6` seconds, one Mercury year every `7.6`. At 60 fps that is
/// `1.67e4` simulated seconds per frame, so Mercury — the fastest body, at `47.9 km/s` —
/// moves `0.80` units per frame and gets about `456` integration steps per orbit, which
/// is enough for the symplectic integrator to trace a closed ellipse rather than a
/// spiral. Jupiter takes 12 minutes of watching per orbit and Neptune about 1.7 hours;
/// `]` doubles the scale when you want the outer system, at the cost of the inner one's
/// accuracy.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`], as the spec's time
/// scale.
///
/// **If you change it.** Accuracy degrades with the *step*, and the inner planets pay
/// first: at `1e7` Mercury gets only `46` steps per orbit and visibly precesses. Note
/// `0.80` units per frame is already over
/// [`crate::collision::constants::MAX_SUBSTEP_DISPLACEMENT_UNITS`] (`0.25`), so every
/// frame is split into four substeps — four full force evaluations over nine bodies,
/// which is the experiment's per-frame cost and scales linearly with this value until
/// [`crate::collision::constants::MAX_SUBSTEPS`] (`64`) caps it.
pub const SOLAR_SYSTEM_TIME_SCALE: f32 = 1.0e6;

/// Where the camera starts for the solar system, in world units.
///
/// **Why this value.** Framed on the **inner** system, not the whole thing: to fit
/// Neptune's `4495`-unit orbit in a 45° vertical field of view the camera would have to
/// be `10,850` units out, from where the Earth's `1`-unit body spans a seventh of a pixel
/// and there is nothing to see. `(0, 380, 500)` is `610` units from the look-at point,
/// enough for [`SOLAR_SYSTEM_FRAMED_RADIUS_UNITS`] (`240` — out to Mars), and the steep
/// elevation shows the ecliptic as a plane of circles rather than edge-on as a line.
///
/// The outer planets are reached by flying: `W` with `Left Shift` covers `60` units a
/// second, so Neptune is about a minute out.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`].
///
/// **If you change it.** Pinned against `SOLAR_SYSTEM_FRAMED_RADIUS_UNITS` by
/// `catalog_tests::every_experiment_frames_its_own_scene`. Note the camera's far clip
/// plane is [`crate::render::constants::CAMERA_FAR_UNITS`], not Bevy's default `1000` —
/// without that change everything past Jupiter is clipped away whatever this value is.
pub const SOLAR_SYSTEM_CAMERA_POSITION_UNITS: Vec3 = Vec3::new(0.0, 380.0, 500.0);

/// Radius about the camera target that the solar system's opening view contains, in
/// world units.
///
/// **Why this value.** Mars's orbit is `227.9` units, so `240` frames the four terrestrial
/// planets with margin. Unlike the other two experiments this is deliberately **less than
/// the scene's extent**: the four giants start outside the opening frame, because a view
/// that contained them would show nothing useful about any of the nine bodies.
///
/// **Used by.** [`crate::experiments::catalog::ExperimentSpec::framed_radius_units`] for
/// this experiment, and through it `every_experiment_frames_its_own_scene`.
///
/// **If you change it.** It must stay achievable from
/// [`SOLAR_SYSTEM_CAMERA_POSITION_UNITS`] — `610 × tan(22.5°) = 253` units is the most
/// that camera can frame — and it must still contain at least one planet, both of which
/// that test checks. Widen it to `790` (with a camera `1900` units out) to open on
/// Jupiter's orbit instead.
pub const SOLAR_SYSTEM_FRAMED_RADIUS_UNITS: f32 = 240.0;

/// Trail length for the solar system, in world units.
///
/// **Why this value.** The crate-wide default
/// ([`crate::render::constants::TRAIL_LENGTH_UNITS`], `10`) is two grid squares — right
/// for a scene `30` units across, and an invisible stub on an orbit `940` units around.
/// `400` units is `43%` of Earth's orbital circumference and slightly more than Mercury's
/// whole orbit, so the trail reads as the *path* it is there to show. On Jupiter it is
/// `8%` of the way round, which is still a clear arc.
///
/// **Used by.** [`crate::experiments::catalog::ExperimentSpec::trail_length_units`], read
/// once at startup by
/// [`crate::render::trail::apply_experiment_trail_length`], which also widens the sample
/// spacing to match so the sample budget is not exhausted.
///
/// **If you change it.** Sample spacing is derived from it, so the cost per body stays
/// flat — see [`crate::render::constants::TRAIL_TARGET_SEGMENTS`]. Long enough to close
/// the loop (`940` for Earth) and the trail meets its own tail, which looks like a solid
/// ring and hides which way the body is going.
pub const SOLAR_SYSTEM_TRAIL_LENGTH_UNITS: f32 = 400.0;

/// The eight planets, in order out from the Sun: mass, radius, orbital radius, starting
/// angle, and colour.
///
/// **Why these values.** They are **measurements**, not choices — NASA/JPL planetary fact
/// sheet figures, in SI, so the experiment can be checked against published physics
/// rather than against itself. Four numbers per planet, each doing a distinct job:
///
/// | Field | What it sets | Source |
/// | --- | --- | --- |
/// | `mass_kg` | how hard it pulls on the others | planetary fact sheet |
/// | `radius_m` | drawn size, via [`EARTH_RADIUS_M`] | volumetric mean radius |
/// | `semi_major_axis_m` | how far out it starts | mean orbital distance |
/// | `mean_longitude_deg` | where on its circle it starts | J2000 mean longitude |
///
/// The **orbits are treated as circular and coplanar**, which is where the real solar
/// system is idealised: true eccentricities run from `0.007` (Venus) to `0.206`
/// (Mercury), and inclinations to the ecliptic reach `7°`. Keeping one plane and one
/// radius each is what makes the initial condition statable in four numbers, and the
/// resulting periods are within a percent of the real ones. The J2000 longitudes are
/// real, though, so the planets start in their actual relative positions rather than
/// lined up — a lineup would give the system a spurious net momentum in one direction and
/// would look like an alignment that does not happen.
///
/// Colours are recognisable rather than photometric: Mars rust, Uranus pale cyan, Neptune
/// deep blue. Eight bodies on eight circles are only traceable if they can be told apart.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`], which builds one prop
/// per row.
///
/// **If you change it.** Adding a row is enough to add a body — the builder is a loop
/// over this table, and `catalog_tests` checks the whole table rather than named planets.
/// A body added *inside* Mercury's orbit needs checking against
/// `SOLAR_SYSTEM_SUN_DRAWN_RADIUS_UNITS` (removed by A16) (`20` units), which is where the drawn Sun
/// stops.
pub const SOLAR_SYSTEM_PLANETS: [Planet; 8] = [
    Planet {
        name: "Mercury",
        mass_kg: 3.3011e23,
        radius_m: 2.4397e6,
        semi_major_axis_m: 5.7909e10,
        mean_longitude_deg: 252.25,
        color: Color {
            r: 0.55,
            g: 0.53,
            b: 0.50,
            a: 1.0,
        },
    },
    Planet {
        name: "Venus",
        mass_kg: 4.8675e24,
        radius_m: 6.0518e6,
        semi_major_axis_m: 1.0821e11,
        mean_longitude_deg: 181.98,
        color: Color {
            r: 0.90,
            g: 0.80,
            b: 0.55,
            a: 1.0,
        },
    },
    Planet {
        name: "Earth",
        mass_kg: 5.9722e24,
        radius_m: 6.371e6,
        semi_major_axis_m: 1.4960e11,
        mean_longitude_deg: 100.46,
        color: Color {
            r: 0.20,
            g: 0.45,
            b: 0.85,
            a: 1.0,
        },
    },
    Planet {
        name: "Mars",
        mass_kg: 6.4171e23,
        radius_m: 3.3895e6,
        semi_major_axis_m: 2.2792e11,
        mean_longitude_deg: 355.45,
        color: Color {
            r: 0.75,
            g: 0.30,
            b: 0.15,
            a: 1.0,
        },
    },
    Planet {
        name: "Jupiter",
        mass_kg: 1.8982e27,
        radius_m: 6.9911e7,
        semi_major_axis_m: 7.7857e11,
        mean_longitude_deg: 34.40,
        color: Color {
            r: 0.80,
            g: 0.65,
            b: 0.45,
            a: 1.0,
        },
    },
    Planet {
        name: "Saturn",
        mass_kg: 5.6834e26,
        radius_m: 5.8232e7,
        semi_major_axis_m: 1.4335e12,
        mean_longitude_deg: 49.94,
        color: Color {
            r: 0.85,
            g: 0.75,
            b: 0.50,
            a: 1.0,
        },
    },
    Planet {
        name: "Uranus",
        mass_kg: 8.6810e25,
        radius_m: 2.5362e7,
        semi_major_axis_m: 2.8725e12,
        mean_longitude_deg: 313.23,
        color: Color {
            r: 0.55,
            g: 0.80,
            b: 0.85,
            a: 1.0,
        },
    },
    Planet {
        name: "Neptune",
        mass_kg: 1.02413e26,
        radius_m: 2.4622e7,
        semi_major_axis_m: 4.4951e12,
        mean_longitude_deg: 304.88,
        color: Color {
            r: 0.25,
            g: 0.40,
            b: 0.85,
            a: 1.0,
        },
    },
];

/// Seed used when a run does not ask for a specific one.
///
/// **Why this value.** Arbitrary, and that is the point: it is a *fallback*, not a
/// default outcome. The binary normally derives a seed from the wall clock so each run
/// is a fresh experiment; this value is what a caller gets when it builds an
/// experiment with no seed and no entropy — a library user, or a test that wants a
/// known scene without inventing a number.
///
/// **Used by.** [`crate::experiments::selection::Selection::resolve_seed`] as the
/// fallback when the system clock is unavailable, and by any caller of
/// `ExperimentId::build` that does not choose a seed.
///
/// **If you change it.** Every run that relies on the fallback produces a different
/// scene. Tests that need a fixed scene pass their own seed rather than relying on
/// this, precisely so that changing it cannot break them.
pub const DEFAULT_RANDOM_SEED: u64 = 0x5EED_1234_ABCD_0001;

/// The placeable interstellar objects: mass, true radius, capture radius, and whether the
/// object swallows what it touches.
///
/// **Why these values.** Measurements. The masses and radii are published figures — Proxima
/// Centauri for the red dwarf, Sirius B for the white dwarf, a canonical `1.4 M☉` neutron star, a
/// `10 M☉` stellar black hole, and Sgr A*. A black hole's `radius_m` is its **Schwarzschild
/// radius** `2GM/c²`, which is the only radius it has.
///
/// | Object | Mass | True radius | In units | Capture radius | Luminosity |
/// | --- | --- | --- | --- | --- | --- |
/// | Red dwarf | `0.122 M☉` | `1.07e8 m` | `0.107` | its surface | `0.0017 L☉` |
/// | White dwarf | `1.018 M☉` | `5.85e6 m` | `0.00585` | its surface | `0.056 L☉` |
/// | Neutron star | `1.4 M☉` | `1.10e4 m` | `1.1e-5` | its surface | `1e-5 L☉` (visible) |
/// | Black hole | `10 M☉` | `2.95e4 m` | `2.95e-5` | `1.499` units | none |
/// | Sgr A* | `4.3e6 M☉` | `1.27e10 m` | `12.69` | `113.1` units | none |
///
/// **The three stars light the scene when placed**, at their real relative output — a red dwarf is
/// `600×` dimmer than the Sun rather than merely redder. The two black holes emit nothing, which is
/// correct: an accretion disc would, and this crate does not model one.
///
/// **This table used to carry a drawn radius as well, and no longer does.** Until amendment A16
/// there were three size regimes in it — the dwarfs on the planets' `157×` exaggeration, the
/// neutron star and stellar hole *floored* at `0.30` and `0.806` units because `1.7e-3` is
/// invisible, and Sgr A* alone at true scale. All of it was working around one problem: the drawn
/// radius was also the collision radius. Bodies are now stored *and drawn* at their true size with
/// nothing compensating for it (A17), so a neutron star is the `1.1e-5` units it really is and you
/// have to reach it to see it. Three regimes collapsed into none.
///
/// **The capture radius is physics, not visibility.** A black hole does not destroy matter at its
/// event horizon — it destroys it at the **tidal disruption radius**, where the gravity gradient
/// exceeds the infalling body's self-gravity: `R_t ≈ R_star · (M_hole/M_star)^{1/3}`, which for a
/// solar-type star is `1.5e9 m` against the `10 M☉` hole's `2.95e4 m` horizon — a factor of
/// `50,000`. So absorption stays observable *because* it became more accurate, not in spite of it.
///
/// **Used by.** [`crate::experiments::objects::stellar_object`] and the `B`-key placement
/// picker: [`crate::input::placement`] selects a row, [`crate::render::placement`] turns it into a
/// prop.
///
/// **If you change it.** A mass much larger than Sgr A*'s will not fit in the `f32`
/// [`Prop::mass_kg`](crate::core::scene::Prop::mass_kg): M87* is `1.3e40 kg`, beyond `f32`'s
/// `3.4e38` ceiling, which is why the catalog stops here.
pub const STELLAR_OBJECTS: [StellarObject; 5] = [
    StellarObject {
        name: "Red dwarf",
        mass_kg: 2.4286e29,
        radius_m: 1.070e8,
        capture_radius_m: None,
        color: Color {
            r: 0.90,
            g: 0.38,
            b: 0.18,
            a: 1.0,
        },
        absorbs: false,
        note: "0.12 Msun, Proxima Centauri — the commonest kind of star",
        luminosity_solar: 0.0017,
    },
    StellarObject {
        name: "White dwarf",
        mass_kg: 2.0248e30,
        radius_m: 5.850e6,
        capture_radius_m: None,
        color: Color {
            r: 0.90,
            g: 0.93,
            b: 1.00,
            a: 1.0,
        },
        absorbs: false,
        note: "1.02 Msun, Sirius B — a Sun's mass in an Earth's volume",
        luminosity_solar: 0.056,
    },
    StellarObject {
        name: "Neutron star",
        mass_kg: 2.7846e30,
        radius_m: 1.100e4,
        capture_radius_m: None,
        color: Color {
            r: 0.72,
            g: 0.84,
            b: 1.00,
            a: 1.0,
        },
        absorbs: false,
        note: "1.4 Msun in a 22 km ball — denser than an atomic nucleus",
        luminosity_solar: 1.0e-5,
    },
    StellarObject {
        name: "Black hole",
        mass_kg: 1.9890e31,
        radius_m: 2.9540e4,
        capture_radius_m: Some(1.4988e9),
        color: Color {
            r: 0.02,
            g: 0.02,
            b: 0.03,
            a: 1.0,
        },
        absorbs: true,
        note: "10 Msun, horizon 30 km — absorbs what it touches",
        luminosity_solar: 0.0,
    },
    StellarObject {
        name: "Sgr A*",
        mass_kg: SGR_A_STAR_MASS_KG,
        radius_m: 1.26933e10,
        capture_radius_m: Some(1.13104e11),
        color: SGR_A_STAR_COLOR,
        absorbs: true,
        note: "4.3 million Msun, Sgr A* — horizon 12.7 million km",
        luminosity_solar: 0.0,
    },
];

/// The four Galilean moons, in order out from Jupiter.
///
/// **Why these values.** Measurements — the NASA/JPL satellite fact sheet, in SI. Reusing
/// [`Planet`] rather than declaring a `Moon` type is deliberate: a body on a circular orbit
/// about a primary is described by exactly the same five numbers whatever it orbits, and one
/// type means [`crate::experiments::solar_system::circular_orbit_speed_units_s`] and its period
/// counterpart serve both experiments without a second copy of `√(GM/r)`.
///
/// | Moon | Mass | Radius | Orbit | Period | Orbit in its own radii |
/// | --- | --- | --- | --- | --- | --- |
/// | Io | `8.932e22 kg` | `1822 km` | `421,700 km` | `1.769 d` | `231` |
/// | Europa | `4.800e22 kg` | `1561 km` | `671,034 km` | `3.552 d` | `430` |
/// | Ganymede | `1.482e23 kg` | `2634 km` | `1,070,412 km` | `7.155 d` | `406` |
/// | Callisto | `1.076e23 kg` | `2410 km` | `1,882,700 km` | `16.69 d` | `781` |
///
/// That last column is the reason a moon system is showable at nearly true scale where a
/// planetary one is not: Earth's orbit is `23,500` Earth radii, so a planet is `100×` smaller
/// relative to its orbit than a moon is.
///
/// `mean_longitude_deg` here is **not an ephemeris** — unlike
/// [`SOLAR_SYSTEM_PLANETS`], where the J2000 mean longitudes are the real ones. These are
/// arbitrary starting phases — spread but deliberately **not** evenly, because four moons at
/// `90°` intervals put two of them diametrically opposite and read as a contrived alignment
/// rather than a moment in time. Stating that plainly matters: the `1:2:4` Laplace resonance between Io, Europa, and Ganymede emerges from
/// the *period ratios*, which are real, and not from the phases, which are not. Starting at a
/// different epoch is a legitimate initial condition; claiming a false one is not.
///
/// **Used by.** [`crate::experiments::jovian::jovian_system`], which builds one prop per row.
///
/// **If you change it.** Adding a row is enough to add a moon — the builder is a loop, and the
/// scene's bounds are derived from the outermost orbit. A moon *inside* Io's orbit needs
/// checking against Jupiter's drawn radius (`0.2796` units at
/// [`JOVIAN_DISPLAY_EXAGGERATION`]): closer than that and it would visually vanish into the
/// planet, though it would still orbit correctly, because contact and gravitation use the true
/// radius. Amalthea, at `181,400 km` = `0.1814` units, is exactly that case.
pub const JOVIAN_MOONS: [Planet; 4] = [
    Planet {
        name: "Io",
        mass_kg: 8.932e22,
        radius_m: 1.8216e6,
        semi_major_axis_m: 4.217e8,
        mean_longitude_deg: 0.0,
        color: Color {
            r: 0.85,
            g: 0.75,
            b: 0.35,
            a: 1.0,
        },
    },
    Planet {
        name: "Europa",
        mass_kg: 4.800e22,
        radius_m: 1.5608e6,
        semi_major_axis_m: 6.71034e8,
        mean_longitude_deg: 75.0,
        color: Color {
            r: 0.88,
            g: 0.87,
            b: 0.80,
            a: 1.0,
        },
    },
    Planet {
        name: "Ganymede",
        mass_kg: 1.4819e23,
        radius_m: 2.6341e6,
        semi_major_axis_m: 1.070412e9,
        mean_longitude_deg: 160.0,
        color: Color {
            r: 0.60,
            g: 0.57,
            b: 0.50,
            a: 1.0,
        },
    },
    Planet {
        name: "Callisto",
        mass_kg: 1.0759e23,
        radius_m: 2.4103e6,
        semi_major_axis_m: 1.8827e9,
        mean_longitude_deg: 290.0,
        color: Color {
            r: 0.42,
            g: 0.40,
            b: 0.38,
            a: 1.0,
        },
    },
];

/// How many times life size the Jovian system's bodies are **drawn**. Dimensionless.
///
/// **Why this value.** It is the only exaggeration in `experiment_4`, it is applied uniformly —
/// so every size ratio in the picture is real — and it is bounded on both sides by things worth
/// stating:
///
/// - **Below about `2×`** the moons fall under `4` px at the opening framing and stop being
///   objects you can see; at true scale (`1×`) they are `1.7` px.
/// - **Above `6.03×`** Jupiter's drawn surface would swallow Io's orbit, since Io orbits at
///   `6.03` Jupiter radii. At `4×` the drawn Jupiter is `0.2796` units and Io's orbit is
///   `0.4217`, leaving `0.135` units of clear space — so **nothing visually contains anything**,
///   which is the property that makes the scene readable.
///
/// `4×` sits in the middle of that window with the moons at `6`–`9` px. Note this is a *purely*
/// visual number: since [`Prop::contact_radius_units`](crate::core::scene::Prop::contact_radius_units)
/// carries the true radius, changing it cannot alter the physics by so much as a rounding error.
///
/// **Used by.** [`crate::experiments::jovian::jovian_drawn_radius_units`].
///
/// **If you change it.** Nothing physical moves. Past `6.03` the picture becomes a lie about
/// which bodies are inside which, and `jovian_tests::nothing_visually_contains_anything` fails —
/// which is exactly the guard that makes this safe to tune.
pub const JOVIAN_DISPLAY_EXAGGERATION: f32 = 4.0;

/// Plummer softening length for the Jovian system, in world units.
///
/// **Why this value.** `0.001` units is `1000 km` — well under Io's `1822 km` radius, and
/// `1/422` of its orbital radius, so it leaves `99.99944%` of Newton's force at Io's orbit
/// against the crate default's **`15%`**. That default (`1.0` unit) is `14×` Jupiter's entire
/// radius: it models a body's finite size, and at these separations there is no body remotely
/// that large.
///
/// It is very nearly vestigial here, and deliberately not zero. With true contact radii every
/// body in this scene collides long before it can coincide, so the singularity ε exists to cap
/// is already unreachable — but a scene is edited by people, and a `0.0` would make the first
/// hull-less prop anyone adds able to produce an infinite force.
///
/// **Used by.** [`crate::experiments::jovian::jovian_system`], as the scene's
/// [`softening_units`](crate::core::scene::Scene::softening_units).
///
/// **If you change it.** The force error at Io's orbit is `(ε/r)²`, so `0.01` costs `0.06%` and
/// `0.1` costs `5.3%` — the last of which would visibly open the moons' orbits out. Larger than
/// a body's radius and the softening has stopped modelling a body at all.
pub const JOVIAN_SOFTENING_UNITS: f32 = 0.001;

/// Simulated seconds per real second for the Jovian system.
///
/// **Why this value.** `1e4` puts Io's `1.769`-day orbit at `15.3` seconds of watching, Europa
/// at `30.7`, Ganymede at `61.8`, and Callisto at `144.2` — so the whole `1:2:4` resonance
/// plays out inside a couple of minutes and the beat between Io and Ganymede is visible
/// directly. Unlike the solar system, this experiment's periods span only `9.4×`, so **one
/// clock scale genuinely suits every body in it**.
///
/// At 60 fps a frame advances `167` simulated seconds and Io — the fastest body, at `17.3 km/s`
/// — moves `0.0029` units, about `1/145` of its orbital radius. That is roughly `900`
/// integration steps per orbit, comfortably enough for the first-order symplectic integrator to
/// trace a closed circle, and far under
/// [`crate::collision::constants::MAX_SUBSTEP_DISPLACEMENT_UNITS`], so no frame is ever split.
///
/// **Used by.** [`crate::experiments::jovian::jovian_system`], as the spec's time scale.
///
/// **If you change it.** Accuracy degrades with the step and Io pays first: at `1e5` it gets
/// `90` steps per orbit and visibly precesses. The `[` and `]` keys change it at runtime, so
/// there is no need to edit this to look around.
pub const JOVIAN_TIME_SCALE: f32 = 1.0e4;

/// Where the camera starts for the Jovian system, in world units.
///
/// **Why this value.** Callisto orbits at `1.8827` units, so the opening view has to hold a
/// radius of about `2`, needing a distance of `2 / tan(22.5°) ≈ 4.83`. `(0, 3.2, 4.0)` is
/// `5.12` units from the origin — Jupiter's own position — and frames `2.12` units. The
/// elevation is `39°` above the orbital plane: steep enough that four coplanar orbits read as
/// circles rather than as a line, shallow enough that the moons are not all foreshortened into
/// the same place.
///
/// **Used by.** [`crate::experiments::jovian::jovian_system`].
///
/// **If you change it.** Pinned against [`JOVIAN_FRAMED_RADIUS_UNITS`] by
/// `catalog_tests::every_experiment_frames_its_own_scene`. Note the whole system is `3.8` units
/// across, so `Left Shift` boosted flight (`60` units/s) crosses it in a sixteenth of a second —
/// the camera speed is tuned for a planetary system and is wildly too fast here.
pub const JOVIAN_CAMERA_POSITION_UNITS: Vec3 = Vec3::new(0.0, 3.2, 4.0);

/// Radius about the origin that the Jovian system's opening view contains, in world units.
///
/// **Why this value.** Callisto's orbit is `1.8827` units and `2.0` frames it with margin.
/// Unlike `experiment_3`, this is the **whole scene** — every body is in the opening frame,
/// which is how it should be for a system small enough to hold in one view.
///
/// **Used by.** [`crate::experiments::jovian::jovian_system`], as the spec's
/// [`framed_radius_units`](crate::experiments::catalog::ExperimentSpec::framed_radius_units).
///
/// **If you change it.** It must stay achievable from [`JOVIAN_CAMERA_POSITION_UNITS`] —
/// `5.12 × tan(22.5°) = 2.12` units is the most that camera can frame — and it must contain at
/// least one body. Both are checked.
pub const JOVIAN_FRAMED_RADIUS_UNITS: f32 = 2.0;

/// Trail length for the Jovian system, in world units.
///
/// **Why this value.** `0.5` units is `19%` of Io's orbital circumference (`2.65` units) and
/// `4%` of Callisto's (`11.8`) — an arc long enough to read as a curve on the innermost orbit
/// without closing into a solid ring, which is what a trail as long as the orbit itself would
/// look like. The crate default of `10` units is four times the whole system.
///
/// **Used by.** [`crate::experiments::catalog::ExperimentSpec::trail_length_units`] for this
/// experiment.
///
/// **If you change it.** Sample spacing is derived from the length, so cost per body stays flat
/// — see [`crate::render::constants::TRAIL_TARGET_SEGMENTS`]. Past `2.65` units Io's trail meets
/// its own tail and stops showing which way it is going.
pub const JOVIAN_TRAIL_LENGTH_UNITS: f32 = 0.5;

/// How far beyond the outermost orbit the Jovian system's bounds reach, in world units.
///
/// **Why this value.** `0.5` units past Callisto's `1.8827` gives bounds of `±2.38`, so the
/// scene's stated volume contains every body with room for the small excursions a first-order
/// integrator introduces, without claiming a volume the experiment does not use.
///
/// **Used by.** [`crate::experiments::jovian::jovian_system`], which derives
/// [`Scene::bounds`](crate::core::scene::Scene::bounds) from the outermost orbit plus this —
/// derived rather than declared, so adding a moon widens the bounds automatically.
///
/// **If you change it.** Bounds are advisory: nothing stops a prop leaving them. It is a
/// statement about what the scene is *for*, and `jovian_tests` checks every body starts inside.
pub const JOVIAN_BOUNDS_MARGIN_UNITS: f32 = 0.5;

/// Plummer softening length for the solar system, in world units.
///
/// **Why this value.** `0.001` units is `1000 km` — smaller than every body in the scene except
/// the moons, and `1/422` of Io's orbital radius, so it leaves `99.99944%` of Newton's force
/// there. The crate default of `1.0` unit leaves **`15%`**, which is why `experiment_3` could not
/// hold Jupiter's moons until softening became a per-scene quantity: ε models a body's *finite
/// size*, and this scene contains bodies from the Sun's `0.696` units down to Europa's `0.00156`.
///
/// At planetary separations it makes no difference whatsoever — at Mercury's `57.9`-unit orbit the
/// error is `3e-10` — so the planets' trajectories are the same as they were under the old
/// default to far beyond `f32`'s resolution.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`], as the scene's
/// [`softening_units`](crate::core::scene::Scene::softening_units).
///
/// **If you change it.** The force error at a separation `r` is `(ε/r)²`, so this is only ever
/// about the *closest* pair in the scene, which is Io and Jupiter. Anything above about `0.04`
/// units starts visibly opening Io's orbit out. `0.0` is legal and means true point-mass
/// gravitation, singularity included.
pub const SOLAR_SYSTEM_SOFTENING_UNITS: f32 = 0.001;

/// Gap between adjacent bodies in the scale-comparison row, in world units.
///
/// **Why this value.** The one arbitrary number in `experiment_5`, and it is deliberately tiny:
/// `0.02` units is `3%` of the Sun's radius and `3×` Earth's, which is enough to read the bodies as
/// separate objects without letting the gaps dominate a row whose whole point is the *sizes*. Larger
/// and the four terrestrial planets drift apart into empty space; zero and the hulls of adjacent
/// bodies would touch, which
/// `catalog_tests::no_experiment_starts_with_bodies_already_touching` forbids.
///
/// **Used by.** [`crate::experiments::scale_check::scale_check_layout`], between every pair.
///
/// **If you change it.** It shifts every centre after the first and lengthens the row, so
/// [`SCALE_CHECK_FRAMED_RADIUS_UNITS`] and the camera want checking with it — a test recomputes the
/// framing from the layout, so it fails rather than silently pushing Neptune off screen.
pub const SCALE_CHECK_GAP_UNITS: f32 = 0.02;

/// Where the scale-check camera sits relative to the row's centre, in world units.
///
/// **Why this value.** Straight back along `+Z` by `2.8` units, with no elevation: the row is a
/// chart and a chart is read face-on, so any tilt would foreshorten the very heights being compared.
/// `2.8` units frames a half-height of `1.16`, against the Sun's `1.39`-unit diameter — so the Sun
/// fills most of the frame vertically and Earth is still `9.3` px, which is the widest ratio a single
/// view can carry.
///
/// **Used by.** [`crate::experiments::scale_check::scale_check`], added to the row's centre.
///
/// **If you change it.** Closer and the Sun overflows the frame; further and Mercury (`3.6` px at
/// `2.8`) drops under a pixel and vanishes, since nothing floors apparent size any more (amendment
/// A17). Flying right along the row is the intended way to inspect the small end.
pub const SCALE_CHECK_CAMERA_OFFSET_UNITS: Vec3 = Vec3::new(0.0, 0.0, 2.8);

/// Radius about the row's centre that the scale check's opening view contains, in world units.
///
/// **Why this value.** The row is `1.944` units long, so half of it is `0.972`; `1.05` frames that
/// with margin. As with `experiment_1` and `experiment_2` — and unlike `experiment_3` — this is the
/// whole scene, which is the point of a chart.
///
/// **Used by.** `scale_check`, as the spec's
/// [`framed_radius_units`](crate::experiments::catalog::ExperimentSpec::framed_radius_units).
///
/// **If you change it.** Pinned against [`SCALE_CHECK_CAMERA_OFFSET_UNITS`] by
/// `catalog_tests::every_experiment_frames_its_own_scene`, and against the row's actual length by
/// `scale_check_tests`.
pub const SCALE_CHECK_FRAMED_RADIUS_UNITS: f32 = 1.05;

/// Simulated seconds per real second for the scale check.
///
/// **Why this value.** `1.0` — real time — because **nothing in this scene can move**: every body is
/// [`immovable`](crate::core::scene::Prop::immovable), so the clock has nothing to advance and any
/// value would look identical. Real time is the least surprising thing for the overlay to report,
/// and [`ExperimentSpec::is_valid`](crate::experiments::catalog::ExperimentSpec::is_valid) requires
/// it to be finite and positive.
///
/// **Used by.** [`crate::experiments::scale_check::scale_check`].
///
/// **If you change it.** Nothing happens. That is worth stating rather than leaving a reader to
/// wonder what a scale is doing on a static scene: the field is not optional, so it has to say
/// something, and this is the most honest thing it can say.
pub const SCALE_CHECK_TIME_SCALE: f32 = 1.0;

/// Earth's Moon.
///
/// **Why these values.** Measurements: `7.342e22 kg`, a `1737.4 km` mean radius, and a `384,400 km`
/// semi-major axis — `60.3` Earth radii, which is the figure worth remembering because it is what makes
/// the Earth–Moon system drawable at true scale. The derived period comes out at `27.45` days against a
/// real `27.32`, and the orbital speed at `1018 m/s` against `1022`; both are within the error of
/// treating an eccentric orbit as circular.
///
/// `mean_longitude_deg` is an **arbitrary phase**, like the Galilean moons' and unlike the planets'
/// real J2000 longitudes. `210°` puts the Moon clear of Earth's own starting direction from the Sun so
/// the two are visibly separate bodies at the opening framing rather than one dot.
///
/// **A real annular eclipse falls out of this.** From Earth's centre the Moon subtends `4.520e-3`
/// radians and the Sun `4.650e-3` — a ratio of `0.972`, so the Moon is *just* too small to cover the
/// Sun and [`crate::render::eclipse`] draws an annular eclipse rather than a total one. On the surface,
/// one Earth radius nearer, it covers `97.7%` instead of `94.5%`. Both figures are what the real
/// numbers give; neither was chosen.
///
/// **Used by.** [`MOON_SYSTEMS`], and through it
/// [`crate::experiments::solar_system::solar_system`].
///
/// **If you change it.** Its dynamical time is `3.75e5 s`, which asks `6` substeps of the shipped
/// clock — free beside Io's `98`. Bringing it much closer than `10` Earth radii would make it, not Io,
/// the body that sets the scene's timestep.
pub const EARTH_MOONS: [Planet; 1] = [Planet {
    name: "Moon",
    mass_kg: 7.342e22,
    radius_m: 1.7374e6,
    semi_major_axis_m: 3.844e8,
    mean_longitude_deg: 210.0,
    color: Color {
        r: 0.55,
        g: 0.54,
        b: 0.52,
        a: 1.0,
    },
}];

/// Which moons orbit which planet.
///
/// **Why this exists.** So the solar-system builder has no special case for any one planet. The first
/// version hard-coded Jupiter, and adding Earth's Moon would have meant a second copy of the same
/// twenty lines — this table is what turned that into a row.
///
/// The primary is named rather than indexed, so reordering [`SOLAR_SYSTEM_PLANETS`] cannot silently
/// reattach a moon to the wrong planet. A row naming a planet that is not in the table is skipped
/// rather than panicking, which is what makes a partial edit survivable.
///
/// **Used by.** [`crate::experiments::solar_system::solar_system`], one loop over the whole table.
///
/// **If you change it.** Adding a row is enough to add a moon system — Saturn's would be one line here
/// and one table beside [`JOVIAN_MOONS`]. Watch the timestep: the scene's substep count is set by its
/// **tightest** orbit, so a moon closer in than Io (`0.4217` units at `2.43e4 s` of dynamical time)
/// makes itself the thing everything else pays for. Amalthea, at `0.1814` units, would roughly triple
/// the cost.
pub const MOON_SYSTEMS: [MoonSystem; 2] = [
    MoonSystem {
        primary: "Earth",
        moons: &EARTH_MOONS,
    },
    MoonSystem {
        primary: "Jupiter",
        moons: &JOVIAN_MOONS,
    },
];

/// The mass of Sagittarius A*, the black hole at the centre of the Milky Way, in kilograms.
///
/// **Why this value.** `4.3e6 M☉`, from the orbits of the stars that circle it (the GRAVITY and
/// Keck results, `4.15`–`4.3` million solar masses). Its horizon radius follows from it:
/// `2GM/c² = 1.27e10 m`, `12.7` world units.
///
/// **Used by.** The `"Sgr A*"` row of [`STELLAR_OBJECTS`] (the `B`-key placement) and
/// [`crate::experiments::black_hole::black_hole`] (`experiment_6`), so the two holes are the same
/// object.
///
/// **If you change it.** Both change together. Keep it under `f32::MAX` (`3.4e38`), the ceiling of
/// [`Prop::mass_kg`](crate::core::scene::Prop::mass_kg). `black_hole_tests` checks the table's
/// stored horizon radius against the one computed from this mass.
pub const SGR_A_STAR_MASS_KG: f64 = 8.5467e36;

/// The colour Sgr A* is drawn in: almost black, with a trace of blue.
///
/// **Why this value.** A black hole emits nothing and reflects nothing, so it is drawn as dark as a
/// lit material can be while still showing its outline against a dark sky. It is not pure black so
/// that the directional light still gives its edge a hint of shape.
///
/// **Used by.** The `"Sgr A*"` row of [`STELLAR_OBJECTS`] and `experiment_6`.
///
/// **If you change it.** Visual only.
pub const SGR_A_STAR_COLOR: Color = Color {
    r: 0.01,
    g: 0.01,
    b: 0.02,
    a: 1.0,
};

/// Where the camera starts for `experiment_6`, relative to the black hole's centre, in world units.
///
/// **Why this value.** The opening view frames [`BLACK_HOLE_FRAMED_RADIUS_UNITS`]. Through a 45°
/// field of view that needs the camera at least `135 / tan(22.5°) = 326` units away;
/// `(0, 58.5, 331)` is `336` units out and `10°` above the hole's equator: close enough that the disc
/// fills the view, and low enough to see light bending both ways round the hole. The far side of
/// the disc appears arched over the hole, and its underside, seen by light bent under the hole,
/// appears below it. That second image only shows from near the disc's plane: light passing under
/// the hole crosses the plane in front of it, and from higher up it crosses on the disc, which
/// blocks it. From `15°` the underside is hidden; from `30°` the second image shows only `1.5%` of
/// the disc.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`].
///
/// **If you change it.** `catalog_tests::every_experiment_frames_its_own_scene` checks it is still
/// far enough back.
pub const BLACK_HOLE_CAMERA_OFFSET_UNITS: Vec3 = Vec3::new(0.0, 58.5, 331.0);

/// Radius about the black hole that `experiment_6`'s opening view contains, in world units.
///
/// **Why this value.** The accretion disc reaches `20 r_g` (`render::constants::DISC_OUTER_RADIUS_RG`;
/// this layer may not read it, so the two are kept in step by hand), which is `127` units in the
/// scene's coordinates. `135` frames all of it with a little margin.
///
/// **Used by.** `black_hole`, as the spec's
/// [`framed_radius_units`](crate::experiments::catalog::ExperimentSpec::framed_radius_units).
///
/// **If you change it.** Pinned against [`BLACK_HOLE_CAMERA_OFFSET_UNITS`] by `catalog_tests`.
pub const BLACK_HOLE_FRAMED_RADIUS_UNITS: f32 = 135.0;

/// Simulated seconds per real second for `experiment_6`.
///
/// **Why this value.** The natural clock near Sgr A* is its light-crossing time,
/// `r_s/c = 42 s`, and an orbit at the ISCO takes `32.6` minutes. At `100×` that orbit takes `20`
/// seconds to watch, and an object dropped from `100` units falls in within about a minute.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`].
///
/// **If you change it.** Faster than about `1e4` and a fall from rest near the horizon happens
/// within a frame or two.
pub const BLACK_HOLE_TIME_SCALE: f32 = 100.0;

/// How long motion trails are in `experiment_6`, in world units.
///
/// **Why this value.** Nothing moves here until something is placed with `B`, and a body placed
/// near the hole falls or orbits on paths a few hundred units long. `200` shows most of a close
/// orbit; the render layer's default of `10` would be a short dash.
///
/// **Used by.** `black_hole`, as the spec's
/// [`trail_length_units`](crate::experiments::catalog::ExperimentSpec::trail_length_units).
///
/// **If you change it.** Visual only.
pub const BLACK_HOLE_TRAIL_LENGTH_UNITS: f32 = 200.0;

/// Gravitational softening for `experiment_6`, in world units.
///
/// **Why this value.** Softening stands for a body's finite size and weakens gravity inside about
/// that distance. The crate-wide default of `1.0` unit would leave only `99.4%` of the true pull at
/// Sgr A*'s horizon. At `0.1` it is `99.994%` there, and nothing gets closer than the horizon
/// without being swallowed.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`], as the scene's
/// [`softening_units`](crate::core::scene::Scene::softening_units).
///
/// **If you change it.** Keep it well below the horizon radius.
pub const BLACK_HOLE_SOFTENING_UNITS: f32 = 0.1;

/// How far the scene's bounds extend from the black hole's centre, in world units.
///
/// **Why this value.** Bounds are descriptive (nothing is removed for leaving them), and `150`
/// takes in the whole accretion disc (`127` units).
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`].
///
/// **If you change it.** Nothing in the simulation reads it; `catalog_tests` requires valid bounds.
pub const BLACK_HOLE_BOUNDS_EXTENT_UNITS: f32 = 150.0;

/// The dimensionless spin `a/M` of `experiment_6`'s Sgr A*, about the world's vertical axis.
///
/// **Why this value.** Spin is the hardest property of a black hole to measure, and Sgr A*'s is not
/// pinned down. Event Horizon Telescope imaging and flare timing favour a high spin, with estimates
/// commonly between about `0.5` and `0.94`; `0.9` sits in that range and is large enough that every
/// effect of spin is plain: the horizon shrinks to `1.44 r_g`, the ISCO to `2.32 r_g`, and a disc's
/// efficiency rises from `5.7%` to `15.6%`.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`], as the scene's
/// `GravityModel::Kerr` spin, and so by the disc the render layer draws round it.
///
/// **If you change it.** Must be in `[0, 1)`. `0` is a Schwarzschild hole (and the disc, which
/// needs spin, disappears).
pub const SGR_A_STAR_SPIN: f64 = 0.9;

/// One astronomical unit, in metres (IAU 2012, exact).
///
/// **Why this value.** It is the definition.
///
/// **Used by.** [`crate::experiments::black_hole::s_star_state`]: an arcsecond at a distance of `d`
/// parsecs is `d` AU, which is how the S-stars' orbits are quoted.
///
/// **If you change it.** Don't.
pub const ASTRONOMICAL_UNIT_M: f64 = 1.495_978_707e11;

/// The Sun's mass, in kilograms (IAU 2015 nominal, `GM☉/G`).
///
/// **Why this value.** The S-stars' masses are quoted in solar masses.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`], for the S-stars.
///
/// **If you change it.** Every S-star's mass moves with it; their orbits do not, since the hole is
/// `300,000` times heavier than any of them.
pub const SOLAR_MASS_KG: f64 = 1.988_47e30;

/// The Sun's radius, in metres (IAU 2015 nominal).
///
/// **Why this value.** The S-stars' radii are quoted in solar radii.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`], for the S-stars.
///
/// **If you change it.** Only their drawn size changes.
pub const SOLAR_RADIUS_M: f64 = 6.957e8;

/// The distance to the Galactic Centre the S-stars' orbits were fitted with, in parsecs.
///
/// **Why this value.** Gillessen et al. (2017) fit it together with the orbits and the hole's mass:
/// `R0 = 8.32 kpc` (`8.323 ± 0.070` statistical). The orbits' angular sizes are only consistent with
/// their periods at that distance and their mass of `4.28e6 M☉`, close to this scene's `4.3e6`.
///
/// **Used by.** [`crate::experiments::black_hole::s_star_state`]: an arcsecond is `8,323` AU.
///
/// **If you change it.** The orbits scale with it, and their periods (from Kepler's third law for
/// the scene's mass) as its `1.5` power; they then no longer match the observed periods.
pub const S_STAR_DISTANCE_PC: f64 = 8323.0;

/// The date the S-stars start at, as a decimal year: 11 November 2026.
///
/// **Why this value.** The day `experiment_6` gained them; each opens where its measured orbit puts
/// it then. S2, for one, is half a year past apocentre (2026.3) and heading back in towards its
/// 2034 pericentre.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`].
///
/// **If you change it.** Every star starts elsewhere on its orbit.
pub const S_STAR_EPOCH_YR: f64 = 2026.86;

/// The colour of the young, hot (B-type) S-stars, linear RGB: blue-white, as a blackbody at about
/// `20,000 K`.
///
/// **Why this value.** All but one of these are early-type: B dwarfs of `10`–`15 M☉`.
///
/// **Used by.** [`S_STARS`].
///
/// **If you change it.** Visual only.
pub const S_STAR_EARLY_TYPE_COLOR: Color = Color {
    r: 0.62,
    g: 0.72,
    b: 1.0,
    a: 1.0,
};

/// The colour of a late-type (cool giant) S-star, linear RGB: orange, as a blackbody at about
/// `4,000 K`.
///
/// **Why this value.** S38 is classed late-type in Gillessen et al. (2017).
///
/// **Used by.** [`S_STARS`].
///
/// **If you change it.** Visual only.
pub const S_STAR_LATE_TYPE_COLOR: Color = Color {
    r: 1.0,
    g: 0.55,
    b: 0.22,
    a: 1.0,
};

/// The S-stars `experiment_6` puts round Sgr A*: the six of Gillessen et al. (2017, *ApJ* 837, 30,
/// Table 3) with orbits under `0.3″` (`2,500` AU).
///
/// **Why these values.** The orbits (`a`, `e`, `i`, `Ω`, `ω`, `t_P`) are the paper's, as printed.
/// Between them they lie at every angle to the hole's equatorial plane: S13 and S12 `25`–`34°` from
/// it (going round with the spin), S14 nearly at right angles to it, S2 and S55 tilted back past
/// `90°` (going round against the spin), and S38 almost flat but retrograde, so each spends part of
/// its orbit north of the equator and part south. On their real orbits S14 comes closest, `57` AU
/// (`8,500` units) at pericentre.
///
/// `experiment_6` brings them in close ([`crate::experiments::black_hole::s_star_close_state`]):
/// each keeps its orbit's plane but goes round a circle of `close_orbit_rg` gravitational radii,
/// just outside the disc (`20 r_g`) and just outside its own tidal radius
/// ([`crate::experiments::black_hole::tidal_radius_m`]), the closest a real star can orbit without
/// being torn apart: `1.1`–`1.3` times it, staggered so no two share a radius. The B dwarfs' tidal
/// radii are `41`–`44 r_g`, S55's `31`, and the giant S38's `141`, which is why it sits much
/// further out. At `36`–`58 r_g` they move at `13`–`17%` of the speed of light, an orbit every
/// `8`–`16` simulated hours (`5`–`10` minutes at the experiment's clock); S38 takes three days.
///
/// The stars themselves are mostly **not** measured, and these numbers are typical, not fitted:
/// - **S2**: a B0–B2.5 dwarf; its mass is estimated at about `14 M☉` (Habibi et al. 2017), and a
///   radius of `6 R☉` and luminosity of `2×10⁴ L☉` go with that.
/// - **S12, S13, S14**: early-type (B) stars, given a typical B dwarf's `10 M☉`, `5 R☉`, `10⁴ L☉`.
/// - **S38**: late-type, so a cool giant: `2 M☉`, `10 R☉`, `50 L☉`.
/// - **S55**: spectral type unknown (the paper says so) and faint (`K = 17.5`), so a smaller B dwarf:
///   `5 M☉`, `3 R☉`, `10³ L☉`.
///
/// None of that changes the orbits: each star is `300,000` or more times lighter than the hole.
///
/// **Used by.** [`crate::experiments::black_hole::black_hole`].
///
/// **If you change it.** Adding a star from the same table is one row. Stars with larger orbits
/// (S1, S8, S29 reach `5,000`–`10,000` AU) pass beyond the camera's far plane.
pub const S_STARS: [SStar; 6] = [
    SStar {
        name: "S2",
        semi_major_axis_arcsec: 0.1255,
        eccentricity: 0.8839,
        inclination_deg: 134.18,
        ascending_node_deg: 226.94,
        periapsis_deg: 65.51,
        pericentre_yr: 2002.33,
        mass_solar: 14.0,
        radius_solar: 6.0,
        luminosity_solar: 2.0e4,
        color: S_STAR_EARLY_TYPE_COLOR,
        close_orbit_rg: 58.0,
    },
    SStar {
        name: "S12",
        semi_major_axis_arcsec: 0.2987,
        eccentricity: 0.8883,
        inclination_deg: 33.56,
        ascending_node_deg: 230.1,
        periapsis_deg: 317.9,
        pericentre_yr: 1995.59,
        mass_solar: 10.0,
        radius_solar: 5.0,
        luminosity_solar: 1.0e4,
        color: S_STAR_EARLY_TYPE_COLOR,
        close_orbit_rg: 46.0,
    },
    SStar {
        name: "S13",
        semi_major_axis_arcsec: 0.2641,
        eccentricity: 0.4250,
        inclination_deg: 24.70,
        ascending_node_deg: 74.5,
        periapsis_deg: 245.2,
        pericentre_yr: 2004.86,
        mass_solar: 10.0,
        radius_solar: 5.0,
        luminosity_solar: 1.0e4,
        color: S_STAR_EARLY_TYPE_COLOR,
        close_orbit_rg: 50.0,
    },
    SStar {
        name: "S14",
        semi_major_axis_arcsec: 0.2863,
        eccentricity: 0.9761,
        inclination_deg: 100.59,
        ascending_node_deg: 226.38,
        periapsis_deg: 334.59,
        pericentre_yr: 2000.12,
        mass_solar: 10.0,
        radius_solar: 5.0,
        luminosity_solar: 1.0e4,
        color: S_STAR_EARLY_TYPE_COLOR,
        close_orbit_rg: 54.0,
    },
    SStar {
        name: "S38",
        semi_major_axis_arcsec: 0.1416,
        eccentricity: 0.8201,
        inclination_deg: 171.1,
        ascending_node_deg: 101.06,
        periapsis_deg: 17.99,
        pericentre_yr: 2003.19,
        mass_solar: 2.0,
        radius_solar: 10.0,
        luminosity_solar: 50.0,
        color: S_STAR_LATE_TYPE_COLOR,
        close_orbit_rg: 160.0,
    },
    SStar {
        name: "S55",
        semi_major_axis_arcsec: 0.1078,
        eccentricity: 0.7209,
        inclination_deg: 150.1,
        ascending_node_deg: 325.5,
        periapsis_deg: 331.5,
        pericentre_yr: 2009.34,
        mass_solar: 5.0,
        radius_solar: 3.0,
        luminosity_solar: 1.0e3,
        color: S_STAR_EARLY_TYPE_COLOR,
        close_orbit_rg: 36.0,
    },
];
