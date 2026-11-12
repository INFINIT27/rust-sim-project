# `src/experiments/` — which simulation to run

> **Purpose:** The catalog of runnable simulations (two-body, three-body ring, solar system,
> Jovian moons, size ruler, black hole), each one's clock scale and starting camera, the command-line
> parsing, and the objects the `B` key places.
> **Edit here to:** add or change an experiment, change planet or moon data, starting camera
> poses, CLI arguments or `--help`, the random seed, or the `B`-key objects.
> **Not here:** how the world steps is in [`core/`](../core/README.md), how a scene is drawn in
> [`render/`](../render/README.md), and the spacecraft in [`spacecraft/`](../spacecraft/README.md).

An **experiment** is a scene plus the settings that make it watchable: the clock scale
it wants and where the camera should start. Before this layer the app could run exactly
one — `ScenePlugin` hard-coded `Scene::two_body_gravity()` and the shipped time scale,
and the startup camera read a constant from the render layer, so running anything else
meant editing three files.

Bevy-free apart from `bevy::math`, like `core/` and `collision/`: an experiment is a
description of a world, not a renderer.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer, the catalog, and the import restrictions. |
| `constants.rs` | The layer's tunables: ring geometry, the speed window, per-experiment camera poses and framing, the solar-system scale and body tables (planets, `EARTH_MOONS`, `JOVIAN_MOONS`, `MOON_SYSTEMS`), the fallback seed. |
| `catalog.rs` | `ExperimentId`, `ExperimentSpec`, `launches` on `ExperimentSpec` (whether `R` flies a spacecraft here; which one is the fleet's, in `src/spacecraft/`), `BodyNames`, `Experiment`, the builder for each experiment, `ring_circular_speed_units_s`, `catalog_lines`. |
| `selection.rs` | `parse_args` → `Command`, `Selection`, `ParseError`, `help_text`. The command-line surface. |
| `random.rs` | `Rng`: a deterministic SplitMix64, for randomised initial conditions. |
| `solar_system.rs` | `experiment_3`: `Planet`, `MoonSystem`, `radius_units`, `circular_orbit_speed_units_s`, `circular_orbit_period_seconds`, and the builder. |
| `objects.rs` | `StellarObject` and `stellar_object`: the dwarfs, neutron star, and black holes the `B` key places into a running experiment. |
| `jovian.rs` | `experiment_4`: Jupiter and the Galilean moons. `jovian_primary`, `jovian_orbit_speed_units_s`, `jovian_orbit_period_seconds`. |
| `scale_check.rs` | `experiment_5`: a static size-comparison row. `ScaleRow`, `scale_rows`, `scale_check_layout`. A ruler, not a simulation. |
| `black_hole.rs` | `experiment_6`: Sgr A*, spinning, with an accretion disc and six real S-stars on their measured orbits. `black_hole`, `SStar` and `s_star_state` (a measured orbit to a position and velocity), `spinning_horizon_units`, `BlackHoleRadii` and `black_hole_radii` (the non-spinning radii, for comparison), `black_hole_centre_units`, `SGR_A_STAR_NAME`. |

## The six experiments

| Slug | Also | What it is |
| --- | --- | --- |
| `experiment_1` | `1`, `two-body` | Two Earth masses `30` units apart, released from rest: they fall together, bounce elastically, and oscillate. |
| `experiment_2` | `2`, `three-body`, `ring` | Three Earth masses at 120° on a `12`-unit circle, each given a **tangential** velocity of random magnitude, then released. |
| `experiment_3` | `3`, `solar-system`, `planets` | The Sun, the eight planets, Earth's Moon, and Jupiter's four moons — everything at true scale with nothing compensating for it. Deterministic. |
| `experiment_4` | `4`, `jovian`, `jupiter`, `moons` | Jupiter and the four Galilean moons, with the 1:2:4 Laplace resonance. The one system that is **visible** at true scale. |
| `experiment_5` | `5`, `scale`, `sizes`, `ruler` | The Sun and eight planets in a static row at true size, bottoms aligned. Nothing moves — it is a ruler. |
| `experiment_6` | `6`, `black-hole`, `sgr-a` | Sgr A*, spinning at `a = 0.9`, with a glowing accretion disc dragged round it. |

```bash
cargo run --release                          # experiment_1
cargo run --release -- experiment_2          # a fresh three-body ring
cargo run --release -- three-body --seed 42  # the same ring, every time
cargo run --release -- solar-system          # the solar system
cargo run --release -- moons                 # Jupiter and the Galilean moons
cargo run --release -- scale                 # the size-comparison row
cargo run --release -- black-hole            # Sgr A*, spinning, with its disc
cargo run --release -- --list                # the catalog
```

## `experiment_2`, and why tangential

Each body's velocity is perpendicular to its own radius and in the plane of the ring:
all angular momentum, no radial component. That makes the *speed* the whole experiment,
expressed as a factor of the speed that would hold a rigid ring:

- below `1.0` — gravity wins and the ring collapses inward;
- near `1.0` — it nearly holds, as a rotating equilateral triangle (the Lagrange
  solution);
- above `1.0` — the bodies swing wide and come back, usually.

Each factor is drawn **independently** from
`[THREE_BODY_MIN_SPEED_FACTOR, THREE_BODY_MAX_SPEED_FACTOR]` = `[0.4, 1.3]`, so the
symmetry is broken from the start — which is what makes this a three-body problem rather
than a rotating rigid body.

The reference speed is derived, not guessed. For three equal masses `m` at 120° on a
circle of radius `R`, any two are `R√3` apart and the net inward pull on each is
`F = 2·(G·m²/(R√3)²)·cos(30°) = G·m²/(√3·R²)`. Setting that equal to `m·v²/R`:

```text
v = √(G·m / (√3·R))     ≈ 138 m/s at R = 12 units, m = 1 Earth mass
                        ≈ 1.4e-7 units/s
```

A notional orbit therefore takes `5.4e8 s` — about 17 years — which at
`THREE_BODY_TIME_SCALE` (`2e7`) is ~27 seconds of watching.

**The bodies are solid**, so a collapse ends in an elastic bounce rather than a
singularity, and **the floor is finite**, so a body flung past its `±5000`-unit edge falls
past it rather than landing — which, at the ring's scale, means it is gone long before it
gets there.

## `experiment_3`, and exactly which numbers are real

**All of them, and nothing compensates for it.** Masses, radii, orbital distances, and starting
angles (the real J2000 mean longitudes) are published SI figures; speeds are derived from
`v = √(G·M/a)` and land within `2%` of the published mean orbital velocities. There is no size
exaggeration, no minimum apparent size, and no unit trickery. The Earth is stored *and drawn* at the
`0.006371` units it is.

It is also **Newtonian, idealised, and single-precision** — worth knowing before reading a result
off the screen:

| | |
| --- | --- |
| Orbits | Circular and coplanar — real eccentricities reach `0.206` (Mercury), inclinations `7°` |
| Relativity | None, so Mercury's perihelion precession is absent |
| Integrator | First-order symplectic Euler: momentum exact at any step, energy is not, so orbits precess |
| Precision | `f64` positions and velocities (the integrated state); shapes, rotations, masses, and everything drawn stay `f32` |
| Timestep | Frame-rate dependent; `MAX_SUBSTEPS` caps very close encounters |
| Absent | The asteroid belt, Pluto, the other ~285 moons, tides, rotation, radiation pressure |

The full statement is the milestone entry in [`CHANGELOG.md`](../../CHANGELOG.md).

**So it opens on almost nothing.** From the `610`-unit opening framing:

| Sun | Jupiter | Saturn | Earth | Mercury |
| --- | --- | --- | --- | --- |
| `4.64` px | `0.29` px | `0.15` px | `0.041` px | `0.016` px |

One small star, nine orbit trails, no planets. The trails are fixed-width gizmo lines rather than
geometry, so the *orbits* are legible from anywhere — the bodies are not, until you fly to them. The
Sun is `57` px from `50` units and `284` px from `10`.

Three alternatives were tried and each trades away the thing it was trying to show, which is why the
current answer is "the camera":

- **A uniform size exaggeration** big enough to see the Earth from that framing is `≥115×`. At the
  `157×` that "Earth = 1 unit" implies, Earth contains the Moon's orbit (`0.384` u), Jupiter contains
  Io's (`0.4217` u), and Saturn contains Mimas' (`0.186` u). The ceiling that keeps Jupiter's moons
  outside it is `6.03×` — the two demands are `19×` apart.
- **A minimum apparent size** makes every body on screen identical, *including the Sun*, which at
  `4.64` px is itself at the pixel threshold from `610` units. It destroys the one proportion a
  viewer could otherwise read.
- **Redefining the unit** changes no proportion whatsoever. Scaling every length by one factor is a
  similarity transform; the quantity that decides visibility is a ratio, and Earth's orbit is
  `23,481` Earth radii however it is measured.

Two more things to know:

- **No time scale suits the whole system.** Mercury's year is 88 days and Neptune's is 165 years.
  `SOLAR_SYSTEM_TIME_SCALE` (`1e6`) is chosen for the inner four — one Earth year every `31.6`
  seconds — which makes one Neptune orbit about 1.7 hours. `]` and `[` trade one end for the other.
- **Five moons are here — Earth's and Jupiter's four — and they cost about 98 substeps a frame.** One
  frame at this clock is `10.9%` of Io's whole orbit, so the dynamical criterion splits it ~98 ways to
  get the `900` integration steps per orbit the integrator needs. Measured, that is `3.2 ms` a frame in
  release — a fifth of a 60 Hz budget — and it is paid only by scenes with a tight orbit in them: the
  planets alone need `4`, and those come from the displacement rule. **Io sets the cost**; Earth's Moon
  asks for `6` and is free. Each moon's heliocentric velocity is **its primary's plus** its orbital
  velocity about that primary. They are two magnifications away from visible; `-` twice and a flight is
  how you get there, and `+` twice is how you come back.
- **Which moons orbit which planet is a table, not a special case.** `MOON_SYSTEMS` maps a primary's
  *name* to its moons, so Saturn's would be one row here and one table beside `JOVIAN_MOONS`, and
  reordering `SOLAR_SYSTEM_PLANETS` cannot reattach a moon to the wrong planet. Watch the timestep when
  adding one: the scene pays for its **tightest** orbit, so anything closer in than Io becomes the thing
  everything else is priced by.
- **Earth's Moon is a real annular eclipse.** It subtends `4.520e-3` radians from Earth's centre against
  the Sun's `4.650e-3` — a ratio of `0.9716`, *just* under one — so `render::eclipse` draws a ring of
  Sun rather than total darkness. `97.7%` covered from the surface, `94.5%` from the centre; both are
  consequences of the measured numbers rather than settings, and standing on the surface is why real
  total eclipses happen at all.

## Interstellar objects, all at true size

`objects.rs` is not an experiment — it is a catalog of things you can **place into** whichever
experiment is running, with `B`. Five rows, spanning twelve orders of magnitude in radius:

| Object | Mass | True radius | In units | Captures at |
| --- | --- | --- | --- | --- |
| Red dwarf | `0.122 M☉` | `1.07e8 m` | `0.107` | its surface |
| White dwarf | `1.018 M☉` | `5.85e6 m` | `0.00585` | its surface |
| Neutron star | `1.4 M☉` | `1.10e4 m` | `1.1e-5` | its surface |
| Black hole | `10 M☉` | `2.95e4 m` | `2.95e-5` | `1.499` units |
| Sgr A* | `4.3e6 M☉` | `1.27e10 m` | `12.69` | `113.1` units |

Nothing lifts the small ones to a visible minimum, so a placed neutron star subtends `0.001` px from
where it lands and is not drawn at all — the overlay is the only handle on it. That is a deliberate
outcome and a test records it as such.

**A black hole captures at its tidal disruption radius, not its horizon.** Matter is destroyed where
the hole's gravity gradient beats the infalling body's self-gravity —
`R_t ≈ R_star · (M_hole/M_star)^{1/3}`, which for a solar-type star is `50,000×` the `10 M☉` hole's
horizon. So absorption is observable *because* it is accurate.

Newtonian gravity is fine around the stellar-mass objects at showable separations (`0.004 c` at one
unit from a `10 M☉` hole) and not around Sgr A*, whose circular speed at `100` units is `0.25 c`.

## `experiment_4`, and the three limits moons exposed

The Jovian system alone: Jupiter and the four Galilean moons, real masses, real orbital radii, real
circular speeds, real radii, and softening small enough to be invisible. **Nothing in it is scaled.**

| Body | True radius | Orbit | Period | Watching at `1e4` |
| --- | --- | --- | --- | --- |
| Jupiter | `0.0699` u | — | — | — |
| Io | `0.00182` u | `0.4217` u | `1.77 d` | `15 s` |
| Europa | `0.00156` u | `0.6710` u | `3.55 d` | `31 s` |
| Ganymede | `0.00263` u | `1.0704` u | `7.16 d` | `62 s` |
| Callisto | `0.00241` u | `1.8827` u | `16.69 d` | `144 s` |

Moons are the first thing that asked the crate to hold two length scales at once, and it failed in
three independent places. Each was a single value that turned out to be per-scene or per-frame:

1. **The drawn radius was also the collision radius.** `experiment_3` drew Jupiter at `10.973`
   units — a `157×` exaggeration — and Io orbits at `0.4217`, so the drawn hull was `26×` its
   innermost moon's whole orbit and all four started inside the planet. `Prop::contact_radius_units`
   separated the two (A15), and then A16 removed the exaggeration entirely, which is better: a body
   with one radius needs no override at all.
2. **Softening was a crate-wide length.** ε = `1.0` unit leaves `r²/(r²+ε²)` of Newton's force,
   which at Io's orbit is **`15%`** — a circular speed out by `2.6×`. The moons could not have
   orbited with the renderer switched off. It is now `Scene::softening_units`, which follows the
   constant's own logic: ε models a body's finite size, and body radii span `1.5` units to
   `0.0018`.
3. **The substep count measured distance, not time.** One frame at `experiment_3`'s clock is
   **`10.9%` of Io's entire orbit**, and the displacement rule asked for 3 substeps where ~98 are
   needed — because it asks how far the fastest body moves, which is right for contact and blind to
   orbits. `substep_count` now also bounds the step to `MAX_SUBSTEP_DYNAMICAL_FRACTION` of the
   tightest `t_dyn = √(r³/GM)`. Self-tuning: a scene with no tight orbit is unaffected.

Two things fall out free. The **Laplace resonance** (`1:2:4` between Io, Europa, and Ganymede)
comes from the published semi-major axes — `2.0073` and `2.0147` — with Callisto genuinely outside
it at `2.33`. And **one clock scale suits every body**, unlike the solar system: the periods span
`9.4×` rather than `685×`.

One thing that does not work yet: the camera. `CAMERA_MOVE_SPEED_UNITS_S` is `20`, boosted to `60`,
and this whole system is `3.8` units across — so a second of boosted flight crosses it sixteen
times. Camera speed is not per-experiment.

## `experiment_5`, the instrument for the question `experiment_3` cannot answer

`experiment_3` draws everything at true size and real distance, which is correct and makes the
planets sub-pixel — so the one thing you cannot do there is *check* that the sizes are right. This is
that check: nine spheres in a row at their true radii, **bottoms aligned** on `y = 0` so a body's
height above the plane is its diameter, the way every planet-size chart is drawn.

**Removing the distances is the whole trick.** The row is `1.944` units long against a solar system
`4495` units across, so the entire `109:1` range fits one view:

| Sun | Jupiter | Saturn | Uranus | Neptune | Earth | Venus | Mars | Mercury |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1012 px | 102 px | 85 px | 37 px | 36 px | 9.3 px | 8.8 px | 4.9 px | 3.6 px |

Every one of those is the true proportion, and they are checkable against any reference — which is a
different kind of evidence from a test. `scale_check_tests` pins the same four ratios numerically
(Sun:Earth `109.2`, Sun:Jupiter `9.95`, Jupiter:Earth `10.97`, Earth:Mercury `2.611`); this is the
version a person can look at.

**Every body is immovable**, which also zeroes its mass. A real Sun beside a real Mercury `0.72`
units away does not survive one frame, and pausing the clock would leave a simulation waiting to
happen — one stray unpause and the row collapses. So this is the only scene in the catalog with no
dynamics at all, and a test runs it through the real integrator for 120 worst-case frames and
compares the whole scene for equality. A ruler must not be able to bend.

The radii come from `SOLAR_SYSTEM_PLANETS` and the Sun's own constant rather than a table of their
own: an instrument has to be measuring the same object as the experiment it checks.

## `experiment_6`, where relativity stops being a rounding error

Sgr A*, the black hole at the centre of the Milky Way, at its published mass (`4.3e6 M☉`, shared
with the `B`-key row as `SGR_A_STAR_MASS_KG`), **spinning** at `a = 0.9` (`SGR_A_STAR_SPIN`) about
the world's vertical axis, alone at the origin with no floor. Its size is not stored:
`spinning_horizon_units` computes it from the mass and spin with the relativity library's Kerr
solution (`physics_utils/relativity/kerr.rs`).

| Radius (Boyer–Lindquist) | `r_g = GM/c²` | Drawn (equatorial) | What it is |
| --- | --- | --- | --- |
| Event horizon | `1.44` | `10.8` u | Drawn, and where things are captured. `12.7` u without spin. |
| Ergosphere at the equator | `2` | `13.9` u | Inside it nothing can stand still. Not drawn. |
| Prograde photon orbit | `1.56` | | Light circling with the spin. |
| Prograde ISCO | `2.32` | `15.8` u | The disc's inner edge. `38.1` u without spin. |

A Boyer–Lindquist radius `r` in the equatorial plane is a circle of radius `√(r² + a²) r_g` in the
Cartesian (Kerr–Schild) coordinates the scene uses, which is why the drawn sizes are a little larger.

**Gravity is that of a spinning hole.** `Scene::gravity_model` is `GravityModel::Kerr { spin }`: the
hole's pull on anything placed with `B` is the exact Kerr equation of motion
(`kerr_schild_acceleration`), in Kerr–Schild coordinates and time. Matter is dragged round with the
spin, and crosses the horizon in a finite time, so the hole captures at its horizon. The scene's
integrator takes second-order (Heun) steps under the relativistic models; the first-order step it
uses elsewhere pumped energy into orbits near the hole. `black_hole_tests` checks the result: an
orbit at `1.2×` the prograde ISCO holds, where the same release round a non-spinning hole is
swallowed; an orbit just inside the ISCO plunges.

**The accretion disc.** `render/accretion_disc.rs` draws `10,000` clumps of gas in the equatorial
plane, from the ISCO out to `20 r_g` (`127` units):

- each orbits prograde at the exact Kerr rate `Ω = 1/(r^{3/2} + a)`, inner rings lapping outer ones;
- each drifts inward, sped up to three orbits per factor `e` in radius (a real disc's inflow takes
  thousands of orbits, which could not be watched); this is the one non-physical rate;
- at the ISCO it starts to fall, whirling round with the spin, along a real Kerr path, and is
  replaced at the outer edge once it crosses the horizon;
- it glows at the temperature a thin disc fed at a tenth of the Eddington rate reaches there (the
  Page–Thorne model: `2.7e5 K` at the hottest, in the ultraviolet).

**Drawn on the GPU** (`render/black_hole_view.rs`). Every pixel's light is followed backwards
along its exact Kerr path, every frame, until it reaches the gas, falls into the hole, or comes from
the sky, so the view is right from anywhere the camera goes. Nothing about the shape is drawn in;
it comes out of the light's paths:

- the **far side** of the disc, whose light goes over the top of the hole, appears **arched over
  it**: a point `6 r_g` behind the hole, which would sit `1.4 r_g` above its centre if light went
  straight, appears `6 r_g` above it;
- the far side's **underside**, whose light is bent round under the hole, appears as a **second
  image below it**, seen through the gaps in the near side;
- light that goes round once more makes a hairline third image hugging the shadow;
- the **shadow** is lopsided, flatter on the side coming towards the camera, where light orbiting
  with the spin can come closer;
- the **stars**, scattered over the whole sky, crowd towards the shadow's edge and are stretched
  into arcs round it: from `53 r_g` the hole's Einstein ring for distant stars is `16°` across.

The disc is the simulation's own: every frame its clumps are splatted into a texture the shader
reads, so the gas seen orbiting, drifting in, and plunging is the gas being simulated. Its light is
what the camera would receive: the blackbody colour at the temperature it sees after gravitational
redshift and the Doppler shift of the gas's motion, orbiting or plunging, at an intensity `g⁴`
times the disc's flux, exposed and tone-mapped like a photograph. So the side coming towards the
camera is far brighter than the side going away, and the plunging gas fades into the shadow.

`V` switches the disc to a flat test pattern (rings every `2 r_g`, spokes every `30°`; orange for
the first image, blue for the second, green for the third) that makes the bending easy to read.
**Settings > Video** has the ray-marcher's quality (Low about `160` fps at `1080p`, Medium `85`,
High `40` on the machine it was built on), the stars, and the fallback: off draws the hole as a
plain black ball.

At these temperatures every part of the disc is blue-white: that is the true colour of a blackbody
above about `20,000 K`. The brightness, not the hue, is what varies.

This is Sgr A* as it would be if it were fed like a quasar. The real Sgr A* takes in about a
billionth of its Eddington rate, and its gas is a hot, faint, puffed-up flow rather than a thin
disc.

**The S-stars.** Six real stars orbit the hole just outside its disc: S2, S12, S13, S14, S38, and
S55, the S-stars of Gillessen et al. (2017) with orbits under `0.3″`. Their real orbits are far
larger (S2 never comes within `120` AU, `18,000` units), so each is brought in close
(`s_star_close_state`): it keeps the **plane** of its measured orbit and its direction from the hole
on 11 November 2026, but goes round a **circle** just outside the disc. How close is set by tides: a
star nearer than its tidal radius `R★ (M/m)^{1/3}` (`tidal_radius_m`) is torn apart, `41`–`44 r_g`
for the B dwarfs, `31` for the small S55, and `141` for the giant S38. Each orbits at `1.1`–`1.3`
times its own, `36`–`58 r_g` (`230`–`370` units, against the disc's `127`), and S38 at `160 r_g`.

The spin's direction on the sky is not measured well, so the plane of the sky is taken as the
hole's equatorial plane, with the spin towards Earth: each star's tilt to the equator is then its
real orbit's inclination, and every star is north of the equator on half its circle and south on
the other. S13 and S12 go round with the spin, tilted `25°` and `34°`; S14 nearly over the poles;
S2, S55, and S38 against it. They move at `13`–`17%` of the speed of light, an orbit every `8`–`16`
simulated hours (`5`–`10` minutes at the `100×` clock), under the hole's Kerr gravity, which twists
the tilted orbits slowly round the spin axis (Lense–Thirring precession, about `2°` an orbit). The
masses, sizes, and luminosities are typical of each star's type, not measured (see `S_STARS`).
They are not lensed: like anything else in the scene they are drawn where they are, and hidden
behind the shadow or the disc.

**Keys.** `V` switches the disc to the test pattern, `N` turns the stars off and on, `O` outlines
the event horizon itself in amber (off at the start), showing the hole's true size well inside
the shadow, and `H` hides the
HUD for a clean view.

**Only the disc, the stars, and the S-stars.** `general_lighting` is off, and nothing else is drawn
round the hole: the light streams and the ergosphere outline were taken out so the disc is easy to see. `render/light_streams.rs`
(light bent and dragged by the spin) is still in the library, just not added to the app. The camera
is framed on the disc, `135` units round the hole, and `10°` above its plane. That is low enough for
the second image: light passing under the hole crosses the plane in front of it, and from higher up
it crosses on the disc, which blocks it. From `15°` the far side's underside is hidden.

The clock runs at `100×`: an orbit at the ISCO takes about ten minutes, six seconds to watch; the
disc's outer edge takes two minutes to watch.

**Known limits.** Only black holes pull relativistically; every other pull, and a placed body's pull
on the hole, is Newtonian. The drawn sphere has the horizon's equatorial radius, but the horizon is
flattened at the poles (`9.1` units), so a body arriving over a pole is captured a little early. The
disc is drawn by the renderer: it has no mass, collides with nothing, and lights no
surfaces.

## Determinism, and why the seed is on screen

Randomised initial conditions are only useful if a run can be repeated. Every experiment
is built from an explicit `u64` seed, the same seed always yields the same scene, and the
seed is stored in the spec and shown in the overlay's `exp` row. So an interesting run is
reproducible after the fact with `--seed <the number you saw>`; by default each launch
picks a fresh seed from the wall clock.

One caveat, and it is a real one: the same seed gives identical *initial conditions*, not
identical trajectories at different time scales. A three-body system amplifies the
integrator's error, so the same scene run at `2e7` and `4e7` diverges — and at
first-order accuracy neither is the true answer.

## Where each scene is built, and why it is asymmetric

The two-body scene is `Scene::two_body_gravity()` in `core/`, because it predates this
layer, its constants are already shared with `render`, and a body of tests and docs names
it. This layer wraps it with a spec. The three-body ring is built **here**, because it
needs randomness, and `core` cannot reach `random.rs` without a circular dependency.

**The rule going forward: a new experiment's scene is built in this layer.** Only the
two-body one lives in `core`, and only for that history.

## Adding an experiment

Four edits, three of which the compiler will demand:

1. a variant on `ExperimentId`;
2. an entry in `ExperimentId::ALL`;
3. arms in `slug`, `name`, `description`, `aliases`, and `build` (the `match` is
   exhaustive, so a forgotten builder is a compile error, not a missing menu entry);
4. a builder function returning `Experiment` — in `catalog.rs` if it reads at a glance,
   in its own module (as `solar_system.rs` is) if it needs a data table of its own.

Then its constants go in `constants.rs`, and `catalog_tests.rs` covers it automatically —
several tests iterate `ExperimentId::ALL` and assert validity, framing, no
already-touching bodies, and that it survives 400 frames of being run.

## Rules and invariants

- **`bevy::math` only.** May read `core`, `physics_utils`, and `crate::constants`; may
  not touch `render`, `input`, or `ui`. The `Resource` impl for `ExperimentSpec` lives in
  `render/plugin.rs`, as `Scene`'s does.
- **A spec is validated, not assumed.** `ExperimentSpec::is_valid` requires a finite
  positive time scale, a positive framed radius, a trail length that is absent or positive,
  and a camera that is not sitting on its own target — the last renders as a black window
  and takes an hour to diagnose.
- **An experiment declares what its opening view frames.** `framed_radius_units` is a
  declaration rather than a derivation, because for the solar system the scene's full
  extent is not a view worth opening on. The declaration is kept honest by tests on both
  sides: the camera must be far enough back for it, and it must contain at least one body.
- **An experiment may ask for its own trail length, and usually should not.**
  `trail_length_units: None` means the render layer's default is right. Only a scene whose
  paths are a different order of magnitude overrides it.
- **The parse returns data.** `parse_args` produces a `Selection`; it does not mutate
  anything or read the environment. That is why a GUI picker will be able to build a
  `Selection` directly and never touch `selection.rs`.
- **One experiment per run.** A second positional argument is an error rather than a
  silent choice between them.
- **The catalog is the single source of the menu.** `--list` and `--help` both call
  `catalog_lines()`, so a new experiment appears in both without anyone remembering.

## Where the tests are

- `tst/experiments/black_hole_tests.rs` — 16 unit tests: the published mass, the spinning horizon
  written out and compared, the hole alone at the origin capturing at its horizon, the view framing
  the disc from above, and through the real integrator: the hole staying put, an orbit at
  `1.2×` the prograde ISCO holding where a non-spinning hole swallows it, an orbit inside the ISCO
  plunging, and a dropped rock never reaching `c`.
- `tst/experiments/scale_check_tests.rs` — 13 unit tests in three groups, matching what an
  instrument has to get right: that it measures the real quantity, that it is readable, and that it
  cannot move.
- `tst/experiments/jovian_tests.rs` — 27 unit tests. The published masses, radii, orbits, and
  periods; the Laplace resonance derived rather than asserted; every prop colliding at its true
  radius with a uniformly `4×` drawn one; **and two tests that pin why the crate had to change** —
  that the old softening leaves under `20%` of Newton at Io's orbit, and that the old drawn radius
  would swallow every moon. Plus Io through the real integrator for one full period, and all four
  holding their circles for ten.
- `tst/experiments/solar_system_tests.rs` — 29 unit tests, most of them checking the scene
  against **published astronomy** rather than against its own constants: orbital speeds in
  km/s, periods in days, size ratios to Earth. Plus the two exaggerations pinned as
  exaggerations, the barycentric momentum cancelling to zero, and
  `earth_completes_one_orbit_in_one_year_and_comes_back` — the real integrator, at the
  shipped clock scale, for one simulated year.
- `tst/experiments/catalog_tests.rs` — 2 properties, 19 unit tests. Catalog hygiene over
  `ALL` (reachability, no token collisions, validity, framing, nothing pre-touching), the
  three-body premise checked geometrically (on the circle, evenly spaced, exactly
  tangential, speeds in window, seeds reproducible and varying), the circular-speed
  derivation recomputed independently, and `every_experiment_runs_without_going_invalid`
  — 400 frames at the app's worst-case step, per experiment.
- `tst/experiments/selection_tests.rs` — 15 unit tests over every accepted form and every
  error variant.
- `tst/experiments/random_tests.rs` — 2 properties, 9 unit tests: determinism, range, a
  coarse spread, and the zero seed.

## Adding a file here

- `bevy::math` only, and no `Resource` impls.
- Constants go in `constants.rs` with the four documented parts, per
  `.claude/steering/constants.md`.
- If a new experiment's premise is a geometric claim — "on a circle", "tangential",
  "co-planar" — assert it in `catalog_tests.rs`. Those properties are invisible in the
  code without redoing the geometry, which is exactly why they rot.
