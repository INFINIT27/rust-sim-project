# Changelog

Day-by-day record of changes to `physics-sandbox`. Newest day first. One `##`
heading per calendar day (ISO `YYYY-MM-DD`), grouped under
`Added` / `Changed` / `Removed` / `Fixed` / `Docs`. Every entry says what
changed and why, and links the spec clause or amendment that governs it where
one exists. The convention is specified in
[`.claude/steering/changelog.md`](.claude/steering/changelog.md).

## 2026-11-12

### Fixed

- **Correction burns no longer forget the moon a flight left from.** `plan_transfer_correction`
  and `plan_correction` (and so `plan_correction_for`, which now takes the flight's `origin`)
  built their prediction with no origin, so `fold_distant_moons` folded the origin's moon system
  away. A crewed flight from the Moon to Earth was corrected against a scene with the Moon merged
  into Earth at their barycentre, about `4,700 km` off Earth's centre: the corrections burned
  `436`, then `818 m/s` against a pull that was not there, and the flight landed `4.9%` late.
  That failed `the_crewed_transfer_lands_back_on_earth_from_the_moon` on Linux CI, whose maths
  library pushed it past the `5%` limit. Now the corrections keep the same bodies as the plan: one
  `23 m/s` trim, and the flight lands within `0.1%` of the plan. A first fix that widened the limit
  to `10%` and blamed the planner is undone. The planner was right all along: flown with no
  corrections, the flight lands within `0.1%` of the plan. The earlier "finer frames" measurements
  never changed the live step, which sets its own substeps. The test (now `…_on_time`) holds this
  flight to `1%`.
- **CI's lint step failed on Rust 1.100.** Its clippy flags `needless_range_loop` on
  `boosting_into_the_rest_frame_leaves_only_the_rest_energy`, which indexed `space[axis]` in a
  range loop; it now iterates the components. CI installs the latest stable, so a lint that
  passes on an older local toolchain can fail there: check with `cargo +stable` after
  `rustup update` before pushing.

## 2026-11-11

### Added

- **Continuous integration** (`.github/workflows/rust.yml`): on every push to `main` and every
  pull request into it, GitHub checks formatting, runs clippy with warnings as errors, builds, and
  runs every test, on Ubuntu with Bevy's system libraries installed and dependencies cached
  between runs. It replaces the starter `rust.yaml` at the root, which GitHub never ran because
  workflows must live in `.github/workflows/`.
- **Six real S-stars orbit Sgr A* just outside its disc** in `experiment_6`: S2, S12, S13, S14, S38,
  and S55, from Gillessen et al. (2017, *ApJ* 837, 30, Table 3). Each keeps the plane of its measured
  orbit (with the plane of the sky taken as the hole's equatorial plane, spin towards Earth) and its
  direction from the hole on 11 November 2026, but goes round a circle just outside the disc, at
  `1.1`–`1.3` times its tidal radius (`tidal_radius_m`): `36`–`58 r_g`, and `160` for the giant S38.
  So they lie north and south of the equator, move at `13`–`17%` of light, orbit every `8`–`16`
  simulated hours, and precess round the spin under the scene's Kerr gravity (`S_STARS`,
  `s_star_state`, `s_star_close_state`).
- **`N` turns the stars round the black hole off and on** (`toggle_stars`), flipping the same setting
  as the menu's.
- **`O` outlines the black hole's event horizon** (`toggle_black_hole_outline`, `BlackHoleOutline`),
  off by default: an amber line (`BLACK_HOLE_OUTLINE_COLOUR`) `2` pixels wide
  (`BLACK_HOLE_OUTLINE_WIDTH_PX`) round the horizon itself, an oblate spheroid `√(r₊² + a²)` across
  the equator and `r₊` to the poles, where it really is (`kerr_ray::horizon_measure`, the closest
  approach of each straight sight line to it). It sits well inside the shadow, which is how the
  bent light makes the horizon look; the outline shows the hole's true size within it. The measure
  changes smoothly across the edge, so the shader draws the line an even width, and costs nothing
  measurable.
- **`H` hides the HUD** (`toggle_hud`, `ui/hud.rs`): the debug panel and mission readout, the
  crosshair, the compass, and the `B` picker, for a clean view; again shows it. The menu and the
  arrival pop-up stay.
- **A lensed star field round the black hole** (phase 5 of `notes.md`). Stars are scattered evenly
  over the whole sky, in every direction (`kerr_ray::star_field`: hashed cells in two layers, each
  star kept only in its own cell), and every ray that leaves the scene ends on them, so the hole
  lenses the whole sky: stars crowd towards the shadow's edge and are stretched into arcs round it
  within its `16°` Einstein ring. The ray-marched sphere now grows with the camera's distance so it
  always holds the camera (`sphere_radius_rg`, `BLACK_HOLE_VIEW_CAMERA_MARGIN`), and every pixel is
  marched. The direction a ray leaves in at the march's escape radius is already its direction on
  the sky to a few `10⁻⁴` rad, because in ingoing Kerr–Schild coordinates the rest of the bending is
  in the coordinates (`kerr_ray_tests`).
- **Black hole settings** (phase 7). **Settings > Video**, `config/performance.toml`, and new
  environment variables: the ray-marcher on or off (`RGA_BLACK_HOLE`; off draws the hole as its
  plain black ball, the fallback for a GPU that cannot run it), its quality (`RGA_BLACK_HOLE_QUALITY`:
  `low`, `medium`, `high`, steps of `6%`, `3%`, `1.5%` of the distance from the hole; about `160`,
  `85`, and `40` fps at `1080p` from the opening view), and the stars (`RGA_BLACK_HOLE_STARS`). The
  gas splat is spread over up to eight threads, about `2 ms` a frame for `10,000` clumps.

- **The ray-marched disc is the simulated disc** (phase 4 of `notes.md`). Every frame the
  simulation's clumps are splatted into a gas texture (`splat_gas`, `512 × 128`, over the turn round
  the hole and the radius from the horizon to the edge), each as a strip `DISC_CLUMP_WIDTH_RG` wide
  along its streak. The shader reads how much of the plane the gas covers wherever the light
  crosses it and sees through the gaps to the images behind (`kerr_ray::composite`), so the disc
  orbits, drifts in, and has the CPU disc's streaky texture. Gas inside the ISCO is drawn too: its
  plunge four-velocity comes from the ISCO orbit's conserved energy and angular momentum, solved in
  the Kerr–Schild metric itself (`kerr_ray::emitter`), and matches a clump dropped as the simulation
  drops them to within 5%. About 84 fps at 1080p. New constants: `DISC_GAS_TEXTURE_WIDTH`,
  `DISC_GAS_TEXTURE_HEIGHT`, `DISC_CLUMP_WIDTH_RG`.
- **The ray-marched disc is shaded** (phase 3 of `notes.md`): each pixel's colour is a blackbody at
  the temperature the camera sees, `g T(r)`, with `g` from the light's own momentum where it left
  the disc, brightened by `g⁴` on the side coming towards the camera, exactly as the CPU-drawn disc
  shades its clumps (`kerr_ray::disc_colour`, matched against `DiscParticle::appearance_for` in
  `kerr_ray_tests`). The disc's temperature and flux ride in the shader's uniform
  (`KERR_RAY_PROFILE_SAMPLES = 64`). `V` now cycles three views: shaded, the flat test pattern, and
  the CPU-drawn disc.
- **`experiment_6`'s black hole is ray-marched on the GPU** (`render/black_hole_view.rs`,
  `assets/shaders/black_hole.wgsl`), phases 1–2 of the plan in `notes.md`. A sphere round the hole
  runs a fragment shader that follows every pixel's light backwards along its exact Kerr path, a
  transcription of `kerr_ray.rs`. It draws the shadow and, for now, the disc as a flat test pattern
  tinted by image order, from any camera position every frame (about 95 fps at 1080p), which fixes
  the CPU lens map's lag and warping when the camera moves. `V` (`toggle_black_hole_view`, a new
  rebindable action) switches back to the CPU-drawn disc; while the GPU view is on, the CPU disc is
  neither drawn nor traced, and the hole's own opaque ball is hidden. New constants:
  `BLACK_HOLE_SHADER_PATH`, `BLACK_HOLE_VIEW_SPHERE_RG`.
- **The `f32` twin of the GPU black-hole ray-marcher** (`render/kerr_ray.rs`), phase 0 of the plan
  in `notes.md` to draw `experiment_6`'s disc by following every pixel's light along its exact Kerr
  path on the GPU. A shader cannot be unit-tested, so it is written first in Rust, in `f32`,
  function for function as the WGSL will be: Hamiltonian equations of motion in Kerr–Schild
  coordinates and `r_g` (no Christoffel symbols, no matrix inverse), RK4 stepped backwards from the
  camera, plane crossings with image orders, shadow and sky endings, and the redshift from the
  light's momentum at the disc. `kerr_ray_tests` checks it against the `f64` Kerr code, Bardeen's
  analytic shadow (to `1%`, edge-on and from the pole), and its own conservation laws, which `f32`
  keeps to `10⁻⁴`. Nothing is drawn with it yet. New constants: `KERR_RAY_STEP_FRACTION`,
  `KERR_RAY_MAX_STEPS`, `KERR_RAY_MAX_CROSSINGS`.

### Changed

- **The ray-marched black hole writes its own depth**: the first gas the light crossed, the hole's
  centre for the shadow, and the very back for the sky. Bodies now show against the stars instead of
  behind them, and while the stars are on the material is opaque, so trails passing behind the disc
  are hidden by it.
### Removed

- **The CPU-drawn disc and its lens map** (phase 6): `render/disc_lensing.rs` (`LensMap`,
  `DiscLensing`), the gizmo drawing in `accretion_disc.rs` (`draw_accretion_disc_system`,
  `lensed_streaks`, `unlensed_streaks`, `DiscStreak`, `AccretionDiscGizmos`,
  `update_disc_lensing_system`, `DiscParticle::streak`, `appearance`, `appearance_for`), their tests,
  and the constants only they used (`DISC_LENS_*`, `DISC_BRIGHTNESS_EXPONENT`,
  `DISC_BEAMING_EXPONENT`, `DISC_LINE_WIDTH_PX`). The simulation stays, and the GPU ray-marcher
  draws it. The ray-marcher's twin is tested against its own `f64` tracer, so the lens map was not
  needed as an oracle.

## 2026-11-10

### Added

- **A menu with pages, and every setting editable from it.** `Esc` always opens the first page:
  **Settings**, **Configuration**, **Exit** (quits the app), and a summary of the run.
  - **Settings > Controls** lists every binding. Click one and press a key to rebind it; `Esc`
    cancels, and a key already in use swaps with it. `Esc` and the mouse cannot be rebound, so the
    menu stays reachable. **Settings > Video**: anti-aliasing, vsync, and the floor's eclipse
    shadows, applied while running (`apply_performance_settings_system`).
  - **Configuration** has a tab per file: camera (applied to the flying camera at once,
    `apply_camera_settings_system`), interface (crosshair, compass, arrival pop-up), trails, and
    the spacecraft fleet (each craft's mass, size, colour, engine, limit, and which flies next).
  - **Every editing page has Reset and Save.** Reset puts the page back to the built-in values.
    Save writes the page's file in `config/` (`ConfigPlugin`, `save_requested_system`), so the
    change survives a restart; a saved file is generated, so hand comments in it are not kept.
    Edits are written back only when they change something, so an open page does not make the
    camera and video re-apply their settings every frame.
- **Keys are rebindable everywhere, through one `KeyMap`.** Every input system asks the `KeyMap`
  (`keymap.just_pressed(&keys, Action::Pause)`) instead of naming a key, and the defaults are the
  new `ACTIONS` table in `src/constants.rs`, so `--help`, the menu, and what the keys do can no
  longer disagree. The bindings load from, and save to, the new `config/controls.toml`; a file
  that puts two actions on one key is an error naming both.
- **The rest of relativity, in `src/physics_utils/relativity/`.** Eight new files finish what that
  directory's README listed as missing. Nothing outside the directory calls them and they are not
  re-exported from the crate root; they are reached by path
  (`physics_utils::relativity::doppler::redshift`).
  - `spacetime.rs`: the Lorentz transformation of an event, the interval and its three kinds, proper
    time and distance between events, and the relativity of simultaneity.
  - `rapidity.rs`: rapidity, and motion under constant proper acceleration (the `1 g` ship).
  - `doppler.rs`: longitudinal, transverse and any-angle Doppler shift, and redshift.
  - `energy_momentum.rs`: `E² = (pc)² + (mc²)²`, invariant mass, light as `m = 0`, and mass–energy
    conversion.
  - `collisions.rs`: the centre-of-momentum frame and elastic and perfectly inelastic collisions on
    one axis. The inelastic solver returns the merged mass, which is more than the sum.
  - `velocity_3d.rs` and `four_vector.rs`: velocities and boosts in any direction, the Wigner
    rotation, aberration, and four-momentum.
  - `gravity.rs`: general relativity's closed forms around one mass (Schwarzschild radius, clock
    rates, redshift, perihelion precession, light bending, the GPS clock offset). It is formulas
    only; the integrator is still Newtonian.
  - 177 tests, in `tst/physics_utils/relativity/`, check them against measurements (Mercury's
    `42.98″` per century, `1.75″` at the Sun, GPS `+38.5 µs/day`) and against the older files that
    cover each one's special case.

- **`experiment_6`: Sgr A*, the Milky Way's central black hole** (`black-hole`, `6`). The ground
  plane and one body: the hole at its published `4.3e6 M☉`, drawn at its true `12.7`-unit horizon
  and `40` units up so the floor's grid gives it a scale. The radius is computed from the mass with
  the relativity library's `schwarzschild_radius`, not stored. Whatever reaches the horizon is
  absorbed. The clock runs at `100×` (an ISCO orbit, `32.6` minutes, takes `20` seconds).
- **Relativistic motion near black holes.** `schwarzschild_acceleration` in
  `physics_utils/relativity/gravity.rs` is the exact Schwarzschild equation of motion for a test
  body, timed by a distant observer's clock. `Scene::gravity_model` (`GravityModel::Newtonian` by
  default, so every other experiment is unchanged) selects it: under `Schwarzschild` a black hole's
  pull on anything that is not a black hole uses it, through the new `integrate_gravity_with`. Its
  tests integrate it with RK4 and check conserved energy and angular momentum, weak-field
  precession, ISCO stability, and an infall that never crosses the horizon.
- **`experiment_6` runs relativistically.** Anything placed near the hole with `B` falls under the
  Schwarzschild equation. Capture moved from the horizon to `1.0101 r_s`, where a falling body's
  light is redshifted tenfold, since nothing reaches the horizon in the distant observer's time.
- **The gravity well: the black hole's equatorial plane drawn as curved space**
  (`render/gravity_well.rs`, `GravityWellPlugin`). Around every black hole in a relativistic scene,
  a grid of rings and radial spokes on Flamm's paraboloid, `z = 2√(r_s(r − r_s))`
  (`flamm_paraboloid_height` in `physics_utils/relativity/gravity.rs`), the shape a sheet needs so
  that distances measured along it are the true radial distances. It meets the horizon at the
  hole's equator and rises to `6 r_s` at the `10 r_s` rim, so the hole sits at the bottom of a
  funnel. The photon sphere (amber) and ISCO (green) are marked on it at their heights. A test
  checks the defining property by integrating the proper distance and the arc length along the
  sheet. The opening view pulls back and looks down into it.
- **Light streams bending around the black hole** (`render/light_streams.rs`, `LightStreamsPlugin`).
  Every `60` simulated seconds a row of `25` parallel rays crosses the hole's equatorial plane from
  `11 r_s` out, spread over `±6 r_s`. Each starts at light's speed in the hole's coordinates
  (`light_velocity`, new in `physics_utils/relativity/gravity.rs`) and is stepped with the
  Schwarzschild equation: wide rays bend, rays inside `2.6 r_s` slow and are captured, rays just
  outside whirl round the photon sphere. They are drawn as fading streaks lifted onto the curved
  sheet, so one circling the photon sphere runs along its ring. Render-only: not scene props, no
  mass, and they light nothing. New tests fire rays and check capture switching between `2.55` and
  `2.65 r_s`, a ray at `2.65 r_s` turned back, and the weak-field bend to `1%`.
- **`ExperimentSpec::general_lighting`.** Whether the renderer adds its fixed directional light and
  ambient fill. True for every experiment but the black hole, which is lit by nothing but its light
  streams.
- **`photon_sphere_radius` and `innermost_stable_circular_orbit_radius`** in
  `physics_utils/relativity/gravity.rs`, and `black_hole_radii` in `experiments/black_hole.rs`, which
  gives the horizon, photon sphere and ISCO in world units. The tests find both radii by scanning
  the effective potentials rather than trusting the `1.5` and `3`.

- **Spinning black holes: `physics_utils/relativity/kerr.rs`.** The Kerr solution: horizon,
  ergosphere, prograde and retrograde ISCO and photon orbits, frame dragging, circular orbits, the
  radiative efficiency of a disc, the Page–Thorne thin-disc flux, and the Eddington limit and disc
  temperature for a real mass. And the equation of motion for matter and light in Kerr–Schild
  coordinates, with analytic Christoffel symbols. Its tests check published radii, the disc's energy
  budget closing, orbits holding and plunging either side of the ISCO, conserved quantities, frame
  dragging against the metric, and light escaping with the spin where it is captured against it.
- **`GravityModel::Kerr { spin }`**, with the spin along world `+Y`. Under it (and under
  `Schwarzschild`) the integrator takes second-order Heun steps: the first-order step pumped energy
  into orbits near a hole, so a body nudged inside the ISCO drifted out instead of plunging.
- **`experiment_6`'s Sgr A* spins (`a = 0.9`, `SGR_A_STAR_SPIN`) and has an accretion disc**
  (`render/accretion_disc.rs`, `AccretionDiscPlugin`): `3,000` clumps of gas orbiting at the Kerr
  rate, drifting inward (sped up), plunging along real paths inside the ISCO, and glowing at the
  Page–Thorne temperature for a tenth of the Eddington rate, as the camera sees it: redshifted by
  gravity, Doppler-shifted and `g⁴`-beamed by their motion. The ergosphere is drawn as an outline.
  The hole is drawn at its spinning horizon (`10.8` units, down from `12.7`) and captures there, and
  the camera is placed to see it (see Changed).

- **The accretion disc is seen through the hole's gravitational lens** (`render/disc_lensing.rs`).
  Rays traced backwards from the camera along exact Kerr light paths show where each part of the
  disc appears: the far side arched over the hole, its underside as a second image below it, a
  hairline third image at the shadow's edge, and the near side barely moved. Each image's redshift
  uses the direction its light really left in. Traced in about three seconds on a background
  thread, and again only when the camera moves in, out, up, or down. `equatorial_coordinates` in
  `kerr.rs` inverts `equatorial_point`.

### Removed

- **`experiment_6` shows only its accretion disc.** The light streams (`LightStreamsPlugin`) are no
  longer added to the app and the ergosphere outline is no longer drawn, so the disc is easy to see.
  Both stay in the library (`render/light_streams.rs`, `ergosphere_outline`) with their tests. The
  camera moves in to frame the disc (`135` units, from `160`).
- **The redshift-ten capture radius** (`capture_radius_units`, `BLACK_HOLE_CAPTURE_REDSHIFT_FACTOR`).
  Under the Kerr–Schild time `experiment_6` now uses, matter crosses the horizon in a finite time, so
  there is nothing to stop short of.
- **`experiment_6`'s floor and its curved equatorial sheet** (`render/gravity_well.rs`,
  `GravityWellPlugin`, the photon-sphere and ISCO rings on it, and the `GRAVITY_WELL_*` and
  `BLACK_HOLE_RING_*` constants), so the scene is the hole and its light alone. The hole moved to
  the origin, and `BLACK_HOLE_HEIGHT_UNITS` went with the floor. `flamm_paraboloid_height` stays in
  the relativity library with its tests.

- **`experiment_6`'s three orbiting neutron stars and the flat photon-sphere and ISCO rings**, both
  added earlier the same day. The stars made way for the gravity well; the rings moved onto it.
  The check that an orbit inside the ISCO plunges, where its Newtonian twin does not, now places
  its own body in the test.

### Changed

- **The disc is denser** (`10,000` clumps, from `3,000`), and `experiment_6`'s camera starts `10°`
  above the disc (`(0, 58.5, 331)`), low enough to see its underside below the hole.
- **The light streams use the Kerr equation**, with the spin from the scene: a spinning hole drags
  them, so its shadow is lopsided, and captured rays fall through the horizon rather than piling up
  at it.
- **The light falls in on the black hole from every direction, far more densely.** Instead of one
  row of `25` parallel rays across the plane every `60 s`, `48` rays every `10 s` set off inward from
  points spread evenly over a sphere `12 r_s` round the hole (fixed seed, `LIGHT_STREAM_SEED`), each
  aimed off centre by an impact parameter spread over a `4 r_s` disc: about three to six thousand
  rays in flight. They are drawn at their true positions in 3-D. A ray's step now scales with
  `r/c` rather than with its distance to the horizon, which cut the cost fourfold (rays creeping
  towards the horizon were taking hundredth-of-a-second steps) without changing any outcome.
- **Sgr A*'s mass and colour are shared constants** (`SGR_A_STAR_MASS_KG`, `SGR_A_STAR_COLOR`), so
  the `B`-key hole and `experiment_6` are the same object.
- **`KEY_BINDINGS` is gone**, replaced by `ACTIONS` (rebindable), `FIXED_BINDINGS` (mouse, `Esc`,
  quit keys), and `BINDABLE_KEYS`. Paired rows like `W / S` are now one row per key. The numpad's
  `+` is no longer a second key for "time scale x10"; bind it in the menu if you want it.
- **`MenuState` has pages** (`MenuPage`, `SettingsTab`, `ConfigTab`) and a rebind in progress.

## 2026-11-09

### Added

- **Settings files, read at startup: `config/*.toml`, loaded by the new `src/config/` layer.**
  Settings that were compile-time defaults can now be changed by editing a file and relaunching,
  with no rebuild. See [`config/README.md`](config/README.md) and
  [`src/config/README.md`](src/config/README.md).
  - **Five files.** `spacecraft.toml` (the fleet `G` cycles through, with engines given by thrust
    and specific impulse, and limits in g), `camera.toml`, `ui.toml` (crosshair, compass, arrival
    pop-up), `trails.toml`, and `performance.toml`.
  - **Optional, and strict.** Every file and every key is optional, and the shipped files hold
    exactly the built-in values; a test keeps them in step with the code. A misspelt key, a wrong
    type, or an out-of-range value stops the app with status 2, naming the file and key, the same
    contract as a bad argument or `RGA_*` variable.
  - **The environment still wins.** `RGA_AA`, `RGA_FLOOR_ECLIPSE` and `RGA_VSYNC` override
    `performance.toml` for one run (`PerformanceSettings::from_env_over`). An experiment that sets
    its own trail length still overrides `trails.toml`. `RGA_CONFIG` points the app at another
    directory.
  - **New resources** so these settings can be inserted: `CameraSettings` (the tuning a new
    camera's `FreeFlyController` starts with) and `ArrivalPopupConfig`. Also new:
    `Mission::with_fleet`, and the public `AntiAliasing::parse` and `AntiAliasing::name`.
  - **New dependencies:** `serde` and `toml`. Only `src/config/` uses them, and the runtime types
    carry no `serde` derives, so the file format is that layer's choice.

- **`src/spacecraft/`, a new layer for everything about a craft.** `core` keeps the orbital
  mechanics (where each body will be, what coasts from here to there); the new layer decides when a
  craft burns, how hard, and toward what, and counts what it cost. Bevy-free but for `bevy::math`,
  like `core`, and above it. See [`src/spacecraft/README.md`](src/spacecraft/README.md).
  - **A spacecraft is data.** `Spacecraft` (mass, size, colour, launch altitude), `Propulsion`
    (`Coasting`, `Direct`, `Transfer`), and `Engine` (thrust and exhaust speed), so a settings menu
    can build or edit one with no flight code involved. `Fleet` is the roster.
  - **Three built-in craft**, in `Fleet::default`: a crewed `15 t` ship with an Aerojet 260 throttled
    to `3 g` flying the least-energy transfer (the default), the same ship at full thrust (`178 g`,
    uncrewed) flying straight at the target, and the Voyager-like coasting probe.
- **The crewed least-energy transfer, the new default.** A human can survive about `9 g` for
  seconds; `3 g` is what crews sit through for minutes, so every burn stays at or under it. The
  flight goes the way real missions do:
  - **Plan.** For every time of flight from a tenth to twice the Hohmann time, the two-body arc
    (Lambert's problem, new `spacecraft::lambert`) about the body the trip is computed about: the
    planet for a trip to its own moon, the Sun otherwise. Its cost is the speed change to leave the
    origin's surface from rest plus the speed change to land; for a fixed exhaust speed and mass a
    burn's energy is proportional to it. Every local minimum is refined, and the cheapest are
    solved in full, every body pulling, until one converges. A trip from a planet to its own moon
    climbs straight up instead, aimed at where the moon will be: a sideways `3 g` burn from rest
    falls back before it builds orbital speed. Planning takes about `0.1 s` (`2 s` to Neptune).
  - **Depart.** A `3 g` burn in a fixed direction, solved for together with the coast, so the
    craft is on its arc after a real, finite burn, not an instant one.
  - **Cruise.** Unpowered. Correction burns come at `1%` of the flight and then each time half of
    what is left has passed, until six hours remain, each planned with the same full solve.
  - **Land.** The landing law starts by twenty radii out at the latest, steering straight at the
    centre (terminal guidance) and braking on its curve, never speeding the craft up.
  - **Measured, flown live:**

    | Route | Time | Δv | Touchdown | Peak |
    | --- | --- | --- | --- | --- |
    | Earth → Moon | 4.6 d | 15.7 km/s | 10.2 m/s | 3.00 g |
    | Moon → Earth | 5.0 d | 17.6 km/s | 10.1 m/s | 3.00 g |
    | Earth → Venus | 292 d | 38.2 km/s | 10.2 m/s | 3.00 g |
    | Earth → Mars | 517 d | 34.7 km/s | 10.1 m/s | 3.00 g |
    | Mars → Earth | 516 d | 24.6 km/s | 10.2 m/s | 3.00 g |
    | Mercury → Jupiter | 590 d | 181.8 km/s | 10.3 m/s | 3.00 g |
    | Earth → Neptune | 17.0 y | 67.8 km/s | 10.2 m/s | 3.00 g |

    The launch is when `R` is pressed, not at the best launch window, so a badly placed target
    costs more. Landing on a heavy body is dear at `3 g`: braking against Jupiter's `2.5 g` leaves
    `0.47 g` to brake with, which is why Mercury → Jupiter takes `182 km/s` against an estimate of
    `83 km/s` that leaves out such gravity losses.
- **The arrival pop-up** (`ui::arrival`): when a craft lands, a window with the craft, the route,
  the time of travel, the maximum speed and its fraction of light, the speed change, the engine time
  at full thrust, the peak acceleration in g, the energy used (with its TNT equivalent), the
  propellant burned, the touchdown speed, and the correction burns. It is not interactive, since a
  click in the window captures the cursor for mouse-look; the next `R` closes it.
- **`FlightReport`**: the running account behind the pop-up. Energy and propellant follow from the
  engine's exhaust speed (`263 s` specific impulse for the Aerojet 260): thrust `F` burns `F / v_e`
  of propellant a second and gives its exhaust `F · v_e / 2` of power.
- **`core::transfer::Forecast`, `solve_coast`, `solve_coast_with`, `Coast`, and a public
  `physical_radius_m`**: the prediction and the coasting solver, opened read-only to the layer
  above. `solve_coast_with` takes a launcher, so a finite burn can be solved for, not just a launch
  velocity. `Voyage::record_path` records a flight's path without the `f32` touchdown test.
- **`STANDARD_GRAVITY_M_S2`** in `physics_utils`, the defined `9.806 65 m/s²` a g is counted in.

### Changed

- **`G` selects the next spacecraft in the fleet** instead of switching an engine on or off. The
  fleet lives on the `Mission` resource, so whatever edits it changes the next launch.
- **The full-thrust direct flight moved from `core::transfer` to `spacecraft`.** `plan_powered`,
  `Drive`, `Voyage::launch_powered`, and `Voyage::steer` are gone; `spacecraft::plan_flight` and
  `spacecraft::Flight` fly every kind of craft. The direct flight behaves as before.
- **The landing law is `√(2 · a_brake · s + v_touch²)`.** It brakes at a constant rate all the way
  to `v_touch`, where the old `min(√(2 · a_brake · s), s / τ) + v_touch` asked for twice the braking
  budget where its two parts met. That was invisible at `178 g` but landed a `3 g` craft at
  `23 m/s`. `a_brake` is now four fifths of what the engine gives *beyond* the target's surface
  gravity, and the smallest step shrinks with `v_touch / a_brake`, so the full-thrust craft still
  lands at `10 m/s`. `POWERED_FINAL_APPROACH_S` is gone.
- **`ExperimentSpec::launches` replaces `ExperimentSpec::transfer: Option<TransferCraft>`.** Which
  craft flies is the fleet's choice, not the experiment's; an experiment only says whether flying
  makes sense in it. `TransferCraft` is gone.
- **The craft constants moved from `experiments` to `spacecraft::constants`**, renamed so the root
  re-exports cannot collide: `TRANSFER_MAX_DEPARTURE_SPEED_M_S` is `PROBE_MAX_DEPARTURE_SPEED_M_S`,
  `TRANSFER_CRAFT_MASS_KG` is `PROBE_MASS_KG`, `TRANSFER_LAUNCH_ALTITUDE_M` is `LAUNCH_ALTITUDE_M`,
  `TRANSFER_CRAFT_RADIUS_M` and `TRANSFER_CRAFT_COLOR` are `CRAFT_RADIUS_M` and `CRAFT_COLOR`, and
  `POWERED_CRAFT_MASS_KG` is `CREWED_CRAFT_MASS_KG`. The steering's `POWERED_*` constants moved
  with it.
- **`Scene::step_steered` takes exactly `step_observed`'s substeps**, with a controller before each
  and one shared state for both callbacks. Shortening substeps, as it allowed before, froze every
  planet too: their per-substep motion fell below what `f32` positions resolve.
- **`NoTransfer::Blocked` and `NoTransfer::NoArrival` moved to `spacecraft::NoFlight`**, with
  `TooMuchGravity` (a `3 g` craft can neither lift off from nor land on the Sun, at `28 g`) and
  `NoArc`.
- **Positions and velocities are `f64`.** `Pose::position`, `Body::Line`'s endpoints,
  `Prop::velocity_units_s`, `TransferPlan`'s start, and `gravitational_acceleration`'s result are
  `DVec3`, and `integrate_gravity` steps in `f64`. They are the integrated state, and `f32` lost
  what each substep added. At Earth's distance an `f32` position resolves `15 km`, and a `5.6 s`
  substep (forced on the whole scene by a craft near Earth) moves Earth `168 km`, rounded the same
  way every time. Measured against `f64` over one simulated day, Earth drifted `18,640 km` and
  the Earth–Moon vector `70,739 km`. Velocities lost up to `1.8 mm/s` a substep the same way.
  Shapes, rotations, masses, collision hulls, drawn paths, and everything handed to Bevy stay
  `f32` and are narrowed at the boundary. `DVec3` is re-exported at the crate root. See
  [`src/core/README.md`](src/core/README.md).
- **`fly` integrates with kick-drift-kick leapfrog**, second order, instead of symplectic Euler,
  at the same cost of one force evaluation a step. Every planner search goes through it, so
  every plan shifts slightly.

### Fixed

- **`a_gravity_assisted_voyage_flown_live_passes_its_planet_and_lands` passes.** The route was
  not chaotic, as recorded on 2026-11-08: it did not exist. It passes Jupiter at `2.1` radii and
  `40 km/s`, where first-order steps of `0.02` rad gave `fly` a `1%` velocity error, `520 m/s` a
  day later. Integrated accurately, the planned launch passes Ganymede `500,000 km` out and misses
  Callisto by `54,683 km`, and a `30 m` change of start moves that by at most `23 km`. The
  leapfrog `fly` fixed it. See "Resolved" in
  [`src/core/transfer/README.md`](src/core/transfer/README.md).
- **The crewed Earth→Moon trip flown through the app lands.** It used to climb to
  `87,000 km`, fall back, and reach the Moon only because an unplanned close pass of Earth threw
  it there. The live scene's `f32` drift near Earth was costing it about `300 m/s`.
  `a_rocket_flown_through_the_app_lands_where_it_was_aimed` passed before only because of that
  pass, and failed once the leapfrog `fly` moved it. With `f64` state it passes.
- **A transfer plan's start was rounded to `f32`**, `5 km` off the planned launch altitude at
  Earth's distance. On the straight climb to the Moon, `5 km` moves the top of the climb by about
  `40,000 km`. Fixed by the `f64` `TransferPlan`.
- **The arrival pop-up can be dismissed, and frees the cursor.** It used to appear under the
  crosshair with the cursor still captured, and stayed until the next `R`. Now it opens a new
  `PopupState` (`src/input/popup.rs`). While that is open, the camera releases the cursor and
  stands down as it does for the `Esc` menu. The pop-up's title-bar ✕, or `Esc`, closes it for
  that landing; `Esc` closes a pop-up before it opens the menu. It runs after the camera system,
  so the click on ✕ does not capture the cursor. The crosshair now paints on egui's background
  layer, under every window, so the pop-up and the menu cover it rather than being crossed by it.
  `F`, `C` and scroll-zoom stand down behind it too, not only capture and movement.
- **A crewed flight could crash the app near its arrival.** The last correction comes due up to
  `6 h` before arrival, and at a fast clock one frame is longer than that, so a frame could step
  past the arrival with the correction still due. It was then planned with a negative time to
  arrival, which built a prediction of one sample and overflowed on the first lookup. Now
  `Ephemeris::build` refuses a horizon that is not positive, and an overdue correction is skipped
  (`Flight::skip_correction`, `Voyage::skip_correction`) rather than planned.
- **`P` during a flight plan or a correction let the planets move.** The plan relied on pausing
  the clock, and `P` cleared that pause, so the craft launched from where its origin had been, and
  a powered craft drifted unsteered during a correction. The mission now holds the clock
  (`SimulationClock::hold`), which `P` cannot release; a pause pressed meanwhile takes effect
  afterwards. The overlay and menu show the clock as held.
- **A failed prediction was reported as a cancel.** `plan_flight` now reports `NoFlight::NoForecast`
  ("could not predict where the bodies will be") unless the search really was cancelled.

### Changed (from the code review)

- **Trial flights are about twice as fast.** `fly` interpolated every body's position four or five
  times per step; it now does so once (`Ephemeris::positions_into`). Same results; the planner
  tests went from `37 s` to `16.6 s`.
- **The live gravity step checks each prop's validity once per substep**, not once per pair.
- **One copy of `segment_entry`, `to_metres` and `to_units`**, in `core::transfer`. The spacecraft
  layer's copies, including the public `segment_entry_m`, are gone.
- **Comments brought up to date** with `f64` positions, and the `G` and `Esc` bindings now say the
  fleet is configurable and that `Esc` closes a pop-up.

### Docs

- **The `README.md` files are now an index.** Every directory `README.md` opens with a fixed
  lookup summary (**Purpose** / **Edit here to** / **Not here**) right under its heading, so
  finding where a change goes means reading one paragraph per candidate directory instead of
  the whole file. The **Not here** line names the wrong guess a reader is most likely to make
  and points to the right directory. The rule, its audit, and a one-command dump of the whole
  index are in
  [`.claude/steering/source-directory-docs.md`](.claude/steering/source-directory-docs.md).
- **The root `README.md` directory map lists every directory.** It was missing
  `src/core/transfer/`, `src/physics_utils/relativity/`, `src/performance/`, and `assets/`.
  Its column now says what to go to each directory to change, not what the directory covers.
- **Fixed two stale facts.** `src/README.md` said the crate has eight layers; it has ten.
  `src/core/README.md` listed `transfer.rs`, which is now the `transfer/` directory.

## 2026-11-08

### Added

- **Powered rockets, the new default.** A coasting rocket is limited to its launch speed
  (`15.4 km/s`) and whatever the planets lend it, which is why it could not reach Jupiter from
  Mercury. A powered rocket has an engine that pushes the whole way: a `15 t` ship with the thrust
  of an Aerojet 260 at its largest test firing (SL-3, 17 June 1967, `26.2 MN`), which is
  `1,747 m/s²`, or `178 g`. The engine is idealised: unlimited propellant and constant mass.
  - **Steering.** `core/transfer/powered.rs`. The craft wants to close at
    `min(√(2 · a_brake · s), s / τ) + 10 m/s`, where `s` is the distance to the target's surface
    and `a_brake` is `80%` of the thrust. Far away that means full thrust toward the target, then a
    flip and braking along the curve, and close in a short, smooth approach (`τ = 1 s`). The law is
    recomputed before every substep, so gravity and drift are removed as they arise and no
    correction burns are needed. A body on the straight line, within two radii, is flown around.
  - **Planning.** `plan_powered` flies the same law through the ephemeris for the path, the
    arrival time, `TransferPlan::peak_speed_m_s`, and anything in the way (`NoTransfer::Blocked`,
    `NoTransfer::NoArrival`). It takes milliseconds.
  - **Live flight.** `Voyage::launch_powered` and `Voyage::steer`. The craft's true state is kept in
    `f64`, relative to its target, because near a landing it moves centimetres per substep and
    the scene's `f32` positions change in steps of `15 km` at Earth's distance, so every move
    would round away. `step_scene_system` flies it through the new `Scene::step_steered`.
  - **Measured.** Every pair of the Sun, the eight planets, and the Moon (90 flights) lands live:
    at `10.0–10.35 m/s` on a planet or moon and up to `12.9 m/s` on the Sun, and within `1.1%` of
    the planned time. Earth → Moon takes `0.5 h`, Earth → Mars `8.0 h` at up to `7.2%` of light
    speed, Mercury → Jupiter `13.0 h` at `11.9%`, and Earth → Neptune `30.5 h` at `28.3%`. With
    Mars moved behind the Sun, the rocket goes around it at `2.8` solar radii and lands.
  - **Newtonian, deliberately.** The rest of the simulation is Newtonian, and so is this. At
    `28%` of light speed that is off by about `4%`, so the status line shows the peak speed as a
    fraction of `c`.
- **`G` switches the rocket between powered and coasting** for the next launch. The coasting
  rocket, its gravity assists, and its correction burns are unchanged. The status line shows
  which one is armed once a departure body is picked.
- **`Scene::step_steered`**: `step_observed` with a controller called before each substep, taking
  exactly the same substeps, so a controller cannot change how the rest of the scene moves. Both
  callbacks share one `state`, so a voyage can both steer and record.
- **`Ephemeris::position_rate`**: the derivative of the ephemeris's interpolated position. A
  powered craft matches this instead of the stored velocities, which lead the positions' path by
  half a step's acceleration, because the integrator updates velocity first. Matching the stored
  velocity, a landing on the Moon stalled `1.5 km` up, closing at `25 m/s` on a target receding
  just as fast.
- **Constants.** `AEROJET_260_THRUST_N` and `POWERED_CRAFT_MASS_KG` (experiments),
  `STANDARD_GRAVITY_M_S2` (physics_utils), and the steering's `POWERED_*` tuning (core), each
  with its reasoning and measurements.
- **Tests.** The steering law, the detour, the `f64` touchdown test, `position_rate`, invalid and
  cancelled plans, live flights to the Moon, to Mars, Mercury → Jupiter, and around the Sun, the
  `G` key, the coasting rocket still cruising, and Mercury → Jupiter through the running app. The
  90-pair sweep is opt-in (`cargo test --release -- --ignored every_pair`, about `8 s`).

- **`Cargo.toml`**, which was missing from the tree, so nothing could build. It was rebuilt from
  what the code and docs require:
  - `bevy = "0.14"`: the code uses 0.14's bundle API. It resolves to `0.14.2`.
  - `bevy_egui = "0.28"`: the line that targets Bevy 0.14.
  - `proptest = "1"` as a dev dependency.
  - `autotests = false`, plus a `[[test]]` block for `tst/headless_app.rs`, because tests live in
    `tst/` and are `#[path]`-included (see `tst/README.md`).
  - The `[profile.dev]` recorded on 2026-11-07: dependencies at `opt-level = 3`, the crate at `1`.

  With it, `cargo clippy --all-targets -- -D warnings` is clean, and the release binary runs the
  solar system on Windows/Vulkan.
- **`Cargo.lock`**, committed because this is an application: the lockfile pins the Bevy build that
  tests and measurements were taken with.
- **`.gitignore`**, covering `target/` and the default `bench.tsv` log.
- **`CONTRIBUTING.md`**, which `README.md` already linked to. It is a short entry point: the
  commands a change must pass, a summary of the steering SOPs, and the layer and test rules from
  `src/README.md` and `tst/README.md`.

### Changed

- **The minimum Rust version is 1.87, not 1.85.** `core/transfer/flight.rs` calls
  `is_multiple_of`, which was stabilized in 1.87, so the documented 1.85 could never build the crate.
  Clippy's `incompatible_msrv` caught it once `rust-version` was declared. The README is updated.
- **The project builds with plain Cargo only.** `bench.sh` now runs `target/release/physics-sandbox`
  by default. Its old default was a build directory that a Cargo build never writes to, so the
  script found no binary. References to the old build system are removed from the docs, comments
  and earlier changelog entries.
- **The project is renamed from `rust-gui-app` to `physics-sandbox`.** The old name said nothing
  about what the program does, and "sandbox" leaves room for physics beyond gravity as features are
  added. The rename covers:
  - the package and binary (`target/release/physics-sandbox`)
  - the crate path (`physics_sandbox::…` in the integration test)
  - `WINDOW_TITLE` and the `--help` banner
  - the two egui layer IDs (`physics_sandbox_compass`, `physics_sandbox_crosshair`)
  - every doc that names the project

  Two things keep the old name on purpose:
  - entries from earlier days, which are history
  - the `RGA_*` environment variables, so existing scripts still work
- **The old performance results are removed.** `src/performance/RESULTS.md` and
  `measurements.tsv` described one particular machine, so they are deleted, along with the
  README's summary table and the machine named in the constants' docs. The switches, the benchmark,
  and the scripts are unchanged. New measurements start fresh, and `bench.sh` writes them to
  `bench.tsv`.
- **Links to the steering SOPs point at `.claude/steering/`, where the SOPs now live.**
  References to the original spec, which is not in the tree, now name it in words ("the original
  requirements spec") instead of linking a missing file. The changelog SOP no longer asks for
  amendments to be recorded in that spec.

### Fixed

- **A flyby correction could lock onto the wrong encounter with the planet.** It aims for the
  planned point beside the planet, but compares the pass with that point only in the plane across
  the planned approach. In the Europa → Ganymede → Callisto voyage, the first correction found a
  pass of Ganymede `30` hours late, approaching `59°` off and `12,491 km` out against a planned
  `6,421 km`. In that plane it sat within `51 km` of the aim point, so it counted as a hit, and the
  burn sent the craft `942,666 km` wide of Callisto. A correction now also requires the pass to
  approach within the new `TRANSFER_FLYBY_MAX_TURN_RAD` (10°) of the planned direction
  (`AimPoint::hold_direction`). The assist search is unchanged, so every plan stays the same.
  Pinned by four tests built from the measured passes.
- **A correction burn that no longer reached the destination was applied anyway.**
  `Voyage::apply_correction` took any plan it was given, and replaced the route's flyby with the
  plan's, so every later burn aimed at the wrong pass and the error could never be undone.
  `plan_correction` now returns `None` when its own prediction misses the destination by more
  than the new `TRANSFER_CORRECTION_MAX_MISS_RADII` (one radius), and the craft carries on
  unburned.
- **Still failing: `a_gravity_assisted_voyage_flown_live_passes_its_planet_and_lands`.** The two
  fixes above stop corrections from making the voyage worse, but they cannot make it land. The
  live flight is `45,000 km` and `440 m/s` off the plan by the first correction, because leaving
  Europa from low altitude makes this route chaotic, not because of step size. From there no burn
  finds the planned pass, so every one is now refused and the craft does not land. Recorded under
  "Known gaps" in `src/core/transfer/README.md`, with what a fix would take.

## 2026-11-07

### Added

- **Gravity-assist routes.** The planner can now route a rocket past up to two planets on the way,
  so their gravity flings it toward the next body. The rocket never burns during a pass: an
  unpowered flyby keeps its speed relative to the planet and only turns it, and because the planet
  is moving, the turn changes the rocket's speed about the Sun. The search's only lever is where
  beside each planet the rocket passes.
  - **Search.** `core/transfer/assist.rs` scans launch directions for first passes of each
    candidate planet, solves the launch for a grid of aim points beside it, flies each one on, and
    takes the closest onward. The last leg is aimed at the destination's centre.
  - **Competition.** Direct and assisted routes compete, and the earliest arrival wins.
  - **Escalation.** Routes with one flyby are searched first; two flybys only when no one-flyby
    route arrives.
  - **Validity.** An assisted result counts only if it still passes every planet of its route,
    since the final aim can otherwise slide onto a direct path.
  - **Measured.** Europa → Ganymede → Callisto, planned with the assist search alone, is flown frame
    by frame through the live integrator. It passes Ganymede at `4,808 km` against a planned
    `4,798 km` and lands. The direct route is faster for that pair, so in the app the direct one
    wins.
- **`Flyby`** on `TransferPlan::flybys` and `Voyage::flybys`: every planet a path passes inside the
  Hill sphere of, whether aimed for or met by chance. It records when, how close, the speed gained
  about the Sun, and the pass geometry. The status window shows the whole route ("Europa →
  Ganymede → Callisto").
- **`TRANSFER_ASSIST_ONLY_HORIZON_S`** (12 years). When no direct path exists, the assist search
  stops there and reports the target out of reach. A direct search needs no limit, because the
  energy bound rules out the targets it could chase forever, but nothing bounds an assisted one.
  Mercury→Jupiter, out of direct reach, now searches every one- and two-flyby route out to 12 years
  in about `150 s` in release, finds none, and says so. With the launch fixed to the moment `R` is
  pressed, Earth and Jupiter have to line up as well, and from J2000 they don't within that time.
  One Earth flyby could otherwise lift the orbit to about `6.3 AU`.
- **Correction burns around each flyby**: five days and one day before it, and one day after. While
  a flyby is ahead, a correction aims for its planned point beside the planet, not the destination.
  Aiming straight at the destination from before a flyby finds the cheapest burn that hits it, and
  in the Ganymede test that burn skipped the flyby and cancelled the assist.

- **`PlanProgress`**, shared by a departure search and whoever waits for it. It shows the horizon
  the search has reached, and `cancel` stops it. The "Rocket" window shows "searching flights of
  up to 88.0 y (R cancels)", and `R` during the search cancels it, restarts the clock, and launches
  nothing.

- **`src/performance/`: frame-time switches and a benchmark.** `RGA_AA` (`msaa4`, `msaa2`,
  `smaa`, `fxaa`, `off`), `RGA_FLOOR_ECLIPSE`, and `RGA_VSYNC` are read once at launch into
  `PerformanceSettings` and applied by `PerformancePlugin`. A misspelt value exits with status 2.
  `RGA_BENCH=on` records frame times with vsync off, prints one `BENCH` line, and exits. The
  defaults are the shipped look. FXAA with no floor eclipse roughly halves experiment_3's frame
  time, at the cost of a `1.3%` penumbra.
- **`[profile.dev]`.** Dependencies build at `opt-level = 3`, the crate at `1`. A dev build of
  experiment_3 went from `14–41 ms` per frame to `4.39 ms`, the same as release, for a one-time
  dependency rebuild.

### Changed

- **`core/transfer.rs` became the directory `core/transfer/`**, because the assist search took it past
  the size where one file stays readable. The split is `mod.rs` (public API), `ephemeris.rs`,
  `flight.rs`, `departure.rs`, `assist.rs` and `voyage.rs`, with no behaviour change, checked by
  the unchanged tests.
- **Trial flights aim at a `Goal`**, a body's centre or a point beside it, so flights can go toward
  any body. Aims judge along-track error within `TRANSFER_ALONG_TRACK_FRACTION` (1%) of the pass
  distance. Assist aims rescale their Newton probes to the sensitivity they measure, because
  behind a flyby the path moves a million times further per radian of launch angle.
- **The departure search has no time limit.** `TRANSFER_MAX_HORIZON_S` (30 years) is gone. The
  search doubles its horizon until a path turns up, and the first horizon that holds any path holds
  the fastest one, since every faster path would fit in it too. To keep long horizons accurate, the
  ephemeris step is capped at `TRANSFER_EPHEMERIS_ORBIT_FRACTION` (`0.1`) of the shortest
  dynamical time of any body in the prediction. Past that point a long search costs memory instead
  of accuracy: about `50 MB` a century when the Moon is kept.
- **The rocket's departure speed is chosen by the planner, up to a cap.** `TransferCraft::
  departure_speed_m_s` became `max_departure_speed_m_s`, and `TRANSFER_DEPARTURE_SPEED_M_S` became
  `TRANSFER_MAX_DEPARTURE_SPEED_M_S`, still `15.4 km/s` (Voyager 2). Each search scans
  `TRANSFER_SPEED_FRACTIONS` (`1.0, 0.75, 0.5, 0.3, 0.15` of the cap), keeps the earliest arrival,
  then hill-climbs on speed. The chosen speed is `TransferPlan::departure_speed_m_s`, and the status
  window shows it. The cap is on what the engine gives the rocket relative to the body it leaves.
  Gravity still speeds it up on the way into a well. Measured from J2000 in release: the inner
  pairs still arrive soonest at the cap (Earth→Mars `489 d`), but Earth→Jupiter arrives in
  `3,060 d` leaving at `11.55 km/s`, `8%` faster than the cap's `8.95 y`, and Earth→Saturn in
  `17.6 y` at `10.59 km/s`, which the capped search never found.
- **Flights stop at the earliest arrival found so far.** Once a speed has produced a path, later
  speeds fly only up to that time, because a path that arrives later cannot win. This took
  Earth→Jupiter from `423 s` to `26 s`. Earth→Saturn takes `147 s`, inner pairs `0.1–2.7 s`.

- **The eclipse shader rejects non-overlapping occluders without trigonometry.** The star's
  distance and angle are computed once per fragment instead of once per occluder, and a cosine-space
  test with a `1e-6` margin settles a miss before any `asin` or `acos`. The Rust reference is
  restructured the same way and is bit-identical to the original over `2.2M` configurations.
  Experiment_3's frame time went from `4.82` to `4.41 ms`.
- **The collision broad phase tests boxes as boxes.** A `Plane` or `Cuboid` is now checked by the
  distance from the other prop's bounding sphere to its closest point, instead of by a bounding
  sphere. Experiment_3's floor sphere enclosed the whole solar system, so every body's icosphere was
  rebuilt every substep, about `1,270` times a frame and `1.6 ms` of CPU. A property test checks the
  broad phase never rejects a pair the narrow phase finds touching.

### Fixed

- **An unreachable target was searched for forever.** Mercury→Jupiter ran its search past a
  thousand years. At the `15.4 km/s` cap a rocket leaving Mercury has at most
  `47.9 + 15.4 = 63.3 km/s` about the Sun. That energy bounds its orbit at `−μ/E = 3.05 AU` from
  the Sun whatever the direction, and Jupiter never comes inside `5.2 AU`. No direct path exists at
  any horizon. `plan_departure` now checks this bound (`coasting_reach`) first, whenever origin and
  target orbit the same primary, and returns `Err(NoTransfer::OutOfReach)` with the numbers. The
  check returns at once, and the status window says it needs about `17.5 km/s` or a gravity assist.
  `TRANSFER_REACH_MARGIN` (`1.05`) allows for ordinary planetary perturbations. Only the outward
  bound is checked. `plan_departure` returns `Result<TransferPlan, NoTransfer>` instead of
  `Option`, so the mission can tell a cancel from an impossible route.
- **Cancelling a long search could take minutes.** Only the ephemeris build checked for a cancel.
  A cancel landing during the trial flights or Newton iterations of an `88`-year horizon was
  measured waiting `142 s`, with the clock paused the whole time. Every flight now checks every
  `1,024` steps and every Newton iteration checks too: `4 ms`.

- **A launched rocket left from where its origin had been a frame earlier and fell into an orbit of
  its own.** `mission_system` had no ordering against `advance_clocks_system`. When the clock
  computed the frame's step first, pausing it for the plan did not cancel that step, so the scene
  moved one frame past the snapshot the plan was made from. At the solar system's clock that frame
  is `16,667 s`, and Earth covers `484,000 km` in it, so the rocket started that far from Earth with
  Earth's old velocity. Correction burns were one frame stale the same way. The mission systems now
  run after `step_scene_system`, so every plan is made from, and applied to, the state the next frame
  steps from. `mission_tests` now asserts the rocket starts `200 km` above Earth and flies Earth→Moon
  through the real plugin to touchdown. Both tests fail with the old ordering.

## 2026-11-06

### Added

- **Rocket transfers between any two bodies, launched with `R`.** `R` with the crosshair on a body
  picks the origin, the same pick `F` uses to lock on. `R` on a second body picks the destination
  and launches. `R` on the origin again cancels. Any pair works, in every experiment but the
  scale ruler, including objects placed with `B`. The rocket is an ordinary `722 kg` prop that
  `Scene::step` moves under the same gravity as everything else, so nothing is on rails. It leaves
  `200 km` above the origin at a hyperbolic excess speed of `15.4 km/s` (Voyager 2's speed), which
  works out to `18.9 km/s` from Earth. Measured from the J2000 start in release: Earth→Moon `6.7 h`,
  Io→Europa `9 h`, Earth→Venus `99 d`, Mars→Earth `157 d`, Earth→Mars `489 d` (Mars starts on the
  far side of the Sun). Each of those is flown frame by frame through the live integrator in a test
  and lands. Earth→Jupiter finds a `10.6`-year path because Jupiter starts `66°` behind Earth.
  Earth→Neptune finds none within `30` years at this speed, and the status window says so.
- **`core/transfer.rs`**: `plan_departure`, `plan_correction`, `TransferPlan`, `Voyage`. A plan
  steps a clone of the scene forward with the live integrator and substep rule to get an ephemeris.
  It flies the rocket through that ephemeris as a test particle, scans `120` launch directions in
  the plane of the two bodies' relative motion, and refines the closest dips with damped Newton
  steps and a halving line search. It keeps the path that arrives first. The horizon starts at twice
  the straight-line time and doubles until a path turns up, so a six-hour hop never pays for a
  decade's ephemeris. Contacts are left out of the prediction, because hull tessellation was `93%`
  of its cost and nothing in a transfer scene touches.
- **The ephemeris is `8,000` samples whatever its length, joined by cubic Hermite interpolation**
  over each sample's position and velocity. The error falls as the fourth power of the step, which
  is what lets a decade be sampled every eleven hours.
- **Moon systems the flight does not visit are folded into their planet** in the prediction, with
  mass and momentum conserved. Io's `1.77`-day orbit forces a substep every `170 s`, and keeping it
  for Earth→Jupiter cost `190 s` of planning. With the fold it takes `1.8 s`, and Earth→Mars dropped
  from `3.1 s` to `0.8 s`. The origin's own moons and a moon target's siblings are kept. The live
  scene keeps everything.
- **Correction burns** at three days or a tenth of the flight, whichever is earlier, then at
  `50 / 85 / 95 / 98%` of it. The live scene steps the rocket more coarsely than the planner near
  the origin and rounds its position to `f32`, so it drifts by about the size of the target. Burn
  totals in the tests: Io→Europa `41 m/s`, Mars→Earth `38`, Earth→Venus `95`, Earth→Mars `307`,
  Earth→Moon `1,126`. The Moon is the outlier because its slowed clock steps the rocket a few
  hundred km at a time, against `f32`'s `15 km` resolution at `1 AU`.
- **Short flights slow the clock** so they last at least `MISSION_MIN_WATCH_S` (`20 s`) to watch,
  and the old scale comes back on arrival. At `1e6` the Moon hop is under two frames, which is too
  short to see and too coarse to land.
- **`TransferCraft`** on `ExperimentSpec` (`transfer`), the craft and how it leaves, as
  `TRANSFER_CRAFT`. **`BodyNames`** on `Experiment`, inserted as a resource. The solar and Jovian
  systems name their bodies, and placing an object with `B` adds its name, so the status reads
  "Earth → Moon".
- **`render/mission.rs`** (`Mission`, `MissionPhase`, `pick_route_system`, `mission_system`,
  `draw_mission_system`, `MissionPlugin`) and **`input/mission.rs`** (`LaunchRequest`,
  `LaunchInputPlugin`). Plans run on `AsyncComputeTaskPool` with the clock paused, so the snapshot
  a plan was made from is still the present when it lands, and the camera stays live. A ring marks
  the picked origin. The flown path is drawn solid and the remaining plan faint. On touchdown the
  rocket leaves the scene and a ring marks the spot. A "Rocket" overlay window shows progress.
- **The `R` row in `KEY_BINDINGS`**, so `--help` and the `Esc` menu list it.

- **`Scene::step_observed`**, which steps like `Scene::step` and calls a closure with the scene
  after every substep. `step` now calls it with an empty closure, so behaviour is unchanged. Trails
  needed it because one frame of the solar system is `11%` of Io's orbit, so positions read once per
  frame cannot draw Io's path. The scene still knows nothing about rendering, because the observer is
  a closure.
- **`moon_primary`** in `render/trail.rs`, which finds the planet a body orbits as a moon. A body is
  a moon of `P` when it lies inside `P`'s Hill sphere, measured against the scene's most massive
  body, and weighs at most `TRAIL_MOON_MAX_MASS_RATIO` (`0.1`) of `P`. The mass test keeps the
  equal-mass bodies of the three-body ring from counting as moons of each other. The scene has no
  parent links, so the planet is derived each frame.
- New tuning constants: `TRAIL_MOON_SAMPLES_PER_ORBIT` (`64`), `TRAIL_MAX_TURN_RADIANS` (`10°`),
  `TRAIL_MIN_SPACING_FRACTION` (`1/16`), and `TRAIL_MOON_MAX_MASS_RATIO` (`0.1`).

### Changed

- **`moon_primary` moved from `render/trail.rs` to `core/scene.rs`**, and
  `TRAIL_MOON_MAX_MASS_RATIO` became `core::constants::MOON_MAX_MASS_RATIO`. It reads only the
  scene, and transfer planning needs it to decide which moon systems to fold. Trails call it from
  its new home and behave the same.
- **`step_scene_system` always steps through `step_observed`** and shows each substep to the
  `Mission` as well as to `Trails`, so touchdown is caught at the substep the rocket enters Mars.
  One substep there moves it about `3,400 km`, against a `6,780 km` disc.

- **`step_scene_system` records trails after every physics substep** when `Trails` and
  `TrailConfig` exist, about `98` samples per frame in the solar system instead of `1`. This costs
  `0.27 ms` a frame in release (`4.03` to `4.30 ms`). `record_trails_system` still runs once per
  frame, to cover a paused clock.
- **A trail's sample spacing is measured along the path, not in a straight line**, and a sample is
  also taken once the path has turned `10°`, provided it has travelled at least `1/16` of a spacing.
  The straight-line test skipped any turn that went out and back within one spacing.
- **Moons are sampled `64` times per orbit** instead of at the scene's spacing. A moon's trail is
  `256` of those samples long, about four orbits (`10.6` units for Io), capped at the scene's trail
  length.

### Fixed

- **Moon trails in the solar system draw the path the moons actually took.** Before, they were a
  straight chord along the planet's path. The solar system's `1.56`-unit sample spacing was about
  twice the width of Io's whole orbit, and recording once a frame gave only about `9` points per Io
  orbit. Every moon's trail now stays within `0.0011` units of the integrator's substep positions,
  down from `0.04`, and Io's backward loops show. Pinned by
  `a_moons_trail_follows_the_absolute_path_the_integrator_computed`. Earth's Moon still looks almost
  straight, which is correct: its real path wobbles only `0.38` units off Earth's.

## 2026-11-04

### Added

- **Camera lock on `F`.** Locks onto the sphere or cuboid nearest the crosshair, within `15°`
  (`LOCK_PICK_MAX_ANGLE_RAD`), and turns to face it. A second `F` releases it. While locked, the
  camera follows the body as it moves, and mouse-look (after a left click to capture) orbits it at a
  fixed distance. The orbit reuses the controller's yaw and pitch, so releasing the lock continues
  free flight from the same view. WASD and the other flight keys do nothing while locked.
- **Scroll to zoom while locked.** Each line multiplies the orbit distance by
  `LOCK_ZOOM_STEP_PER_LINE` (`0.85`), and trackpad pixel deltas count `LOCK_ZOOM_PIXELS_PER_LINE`
  (`50`) to a line. The closest the camera can get is `1.5` drawn radii plus the near plane, so a
  body is never clipped or entered. The farthest is half the far plane.
- **The lock releases itself when the target is despawned** (absorbed), leaving the camera where it
  is.
- `CameraTarget`, a marker the render layer puts on spheres and cuboids, so `input/` picks a target
  without reading `Scene`. `CameraLock(Option<Orbit>)` on the camera, `toggle_camera_lock_system`,
  and `orbit_camera_system`, which runs in `PostUpdate` so the camera follows this frame's position
  rather than last frame's.
- A `lock` row in the debug overlay (`camera_lock_line`) showing `off` or the orbit distance in
  metres, because the lock silently disables the flight keys.
- `F` and `Scroll` rows in `KEY_BINDINGS`, and `KeyF` in `no_binding_collides_with_the_camera`.

### Changed

- **The camera's near plane is `1e-4` units (100 km), down from Bevy's default `0.1` (100,000 km).**
  New constant `CAMERA_NEAR_UNITS`. With bodies at true size, the camera lock's closest orbit is
  `1.5` radii plus the near plane, so the default kept the camera `0.103` units from Io, where Io is
  about `1°` in radius and hard to see. Now the closest orbit is `0.0028` units and Io fills the view
  at about `40°`. Bevy 0.14's infinite reversed-Z projection stores `near / distance` in a 32-bit
  float, so the change costs no measurable depth precision. Pinned by
  `the_camera_lock_can_bring_io_close_enough_to_fill_the_view`, which fails at `0.1`.
- **Sizes stay true; nothing is exaggerated.** Drawing bodies `4×` larger was tried and reverted.
  Scaling distances by the same factor would only change the unit of length, so it can't make a moon
  more visible. Visibility comes from getting close, which the near plane above now allows.

### Docs

- The root `README.md` key table lists `F` (camera lock) and scroll (zoom while locked).

## 2026-10-30

### Fixed

- **Eclipse shadows no longer show through to a body's night side.** A body is left out of its own
  occluder list, so nothing stopped an umbra at the receiver: Io's shadow on Jupiter also appeared
  on the far hemisphere, where it darkened the ambient light. `star_visible_fraction_all` gained a
  `normal` parameter, and it and the shader's `star_visible` return full visibility for a surface
  facing away from the star. Pinned by `io_does_not_shadow_jupiters_night_side` and a new fragment
  in `the_shader_mirrors_the_reference_thresholds`.

### Docs

- **Root `README.md` cut from 463 lines to setup only**, leading with the solar system
  (`cargo run --release -- solar-system`) since that is the experiment worth opening first. The
  per-file layout tree became a directory table linking each directory's `README.md`, where the
  detail already lived and where it is updated alongside the code.
- Moved what only the root had into the directory that owns it: the full key map into
  `src/input/README.md`, the solar system's idealisations table into `src/experiments/README.md`,
  and the full-screen window and how to quit it into `src/README.md`, whose `main.rs` row still
  said 800×600.
- `source-directory-docs.md` now says the root holds a directory map rather than the whole-tree
  layout, matching the new root.

## 2026-09-15 (last)

### Added

- **`relativity/energy.rs`** — `rest_energy` (`mc²`), `relativistic_kinetic_energy` (`(γ−1)mc²`),
  `total_energy` (`γmc²`), `work_done` (`ΔKE`), and `velocity_after_work`. Amendment A29.
- **8 properties and 24 unit tests** (`32` new, `136` across the sub-layer), against published values: the electron's `0.510999 MeV` and the
  proton's `938.272 MeV` rest energies to six significant figures, an LHC proton's `6.799 TeV` of kinetic
  energy, and `1 kg` as `21.48` megatons.

### The three things worth knowing

- **The work–energy theorem survives relativity unchanged.** A28 was about a Newtonian law breaking —
  `F = ma` splitting into `γ³ma` and `γma`. This is the opposite: `W = ΔKE` holds in exactly its Newtonian
  form, and only the expression for `KE` differs. The contrast between the two modules is the most useful
  thing to take from them together.
- **A transverse force does no work**, which ties `energy` back to `dynamics`: it changes direction, not
  speed, so `γ` is unchanged. Only the longitudinal component of a force ever changes a body's energy —
  why a synchrotron's bending magnets cost nothing in beam energy while its cavities cost everything.
- **`E² = (pc)² + (mc²)²` is tested but deliberately not implemented.** It is an identity `energy.rs` and
  `momentum.rs` satisfy because both are right, so as a test it is a real cross-module constraint that
  fails on a stray `γ` in either. As a function it would have been a tautology.

### Fixed

- **`velocity_after_work` returned `None` for very large work.** `g(g+2)` overflows for `g > 1.3e154`, so
  `1e200` rest-energies of work produced `inf` and then no answer, instead of "essentially `c`". Now
  evaluated in two branches split at `g = 1`, dividing through by `g` above it. This is the same overflow
  shape as A28.7's `velocity_from_momentum` fix and was found the same way — by a test at absurd
  magnitude. `the_two_branches_of_the_inverse_agree_where_they_meet` walks the boundary, since a
  discontinuity there would be invisible to every other test.

### Notes

- **`(γ−1)mc²` is where the stable `γ−1` matters most in the whole sub-layer.** `c² = 8.99e16` multiplies
  whatever error it carries, so the naive `γ − 1.0` is wrong by *tens of percent*, not a few digits: for
  `1 kg` at `10 m/s` it gives `59.87 J` against a true `50 J`, and at `1 m/s` exactly `0 J` against
  `0.5 J`.
- **`velocity_after_work` uses `β = √(g(g+2))/(1+g)`, not the textbook `√(1 − 1/γ²)`** — algebraically the
  same, numerically not. The textbook form returns exactly zero at `KE = 1e-17 mc²` where the right answer
  is `β = 4.47e-9`.
- **Named `relativistic_kinetic_energy`** so it does not shadow the parent layer's Newtonian
  `kinetic_energy` at the crate root. A caller has to say which one they mean.

## 2026-09-15 (later still)

### Added

- **`relativity/momentum.rs`** — `relativistic_momentum` (`p = γmv`), `velocity_from_momentum`, and
  `momentum_excess`. Amendment A28.
- **`relativity/dynamics.rs`** — `longitudinal_force` (`γ³ma`), `transverse_force` (`γma`), and their
  inverses `longitudinal_acceleration` and `transverse_acceleration`. The two modules mirror the parent
  layer's own split, where `conservation_laws` holds momentum and `newtons_laws` holds force.
- **12 properties and 26 unit tests** (`38` new, `104` across the sub-layer), checked against
  sources: an LHC proton's published `6.8 TeV` and `0.938 GeV` rest energy, `γ³ = 3.8e11` at that energy,
  and the `γ²` gap between the two force directions.

### The three things worth knowing

- **`F = ma` fails by changing shape, not by a factor.** `F = dp/dt` survives; `dp/dt = ma` does not.
  Along the motion the factor is `γ³`, across it `γ` — so **the same force produces two different
  accelerations**, differing by exactly `γ²` (`5.26×` at `β = 0.9`), and force and acceleration are
  therefore **not parallel**. There is deliberately no `relativistic_mass` function: those two factors
  were once called longitudinal and transverse mass, and the existence of *two* is the proof the idea
  does not work.
- **`p = mv` was replaced because it is not conserved, not because it is inaccurate.** A collision that
  conserves `Σmv` in one frame does not in another, which contradicts the first postulate.
  `newtonian_momentum_fails_to_balance_under_a_boost_where_relativistic_momentum_holds` demonstrates the
  failure rather than asserting it.
- **The `γ³` is checked against a numerical derivative of momentum**, not against `γ³`. Recomputing the
  exponent in the test would have asserted the implementation against itself; a central difference of
  `relativistic_momentum` catches a `γ²`, a `γ⁴`, or a dropped product-rule term.

### Fixed

- **`velocity_from_momentum` returned exactly `c` for large momenta.** In exact arithmetic the inverse of
  `p = γmv` is always subluminal for any finite `p`; in `f64` it is not — above `γβ ≈ 9.5e7` the term
  `1 + 1/r²` rounds to `1.0` and the formula yielded exactly `c`. That is a superluminal answer for a
  massive body, and worse, `lorentz_factor` rejects `c`, so a perfectly reasonable momentum would poison
  the next call. Now clamped to the largest representable speed below `c`, with the cost documented: past
  that boundary the velocity carries no information about the momentum's magnitude. Found by my own
  property test at `p = 1e12·mc`.
- **Two Newtonian-limit test tolerances were wrong by `1000×`.** I claimed `γ − 1 < 5.6e-11` at
  `1e5 m/s`; it is `5.6e-8` — I squared `β` in my head and lost three orders of magnitude. Both tests now
  state the derivation in a comment, and the dynamics one records that its bound is three times the
  momentum one because the longitudinal factor is `γ³`.

## 2026-09-15 (later)

### Added

- **`physics_utils/relativity/` — special relativity.** Amendment A27. The first sub-directory in
  `physics_utils`, and the crate's first departure from Newtonian mechanics. Five modules:
  `constants` (`c`), `lorentz` (`β`, `γ`), `time_dilation`, `length_contraction`, and `velocity`.
  Additive — nothing in the simulation calls it and no existing behaviour changes.
- **`lorentz_factor`, `lorentz_factor_minus_one`, `beta_from_velocity`, `is_subluminal`.**
- **`dilated_time`, `proper_time`, `clock_lag_seconds`** — time dilation both ways, plus the drift.
- **`contracted_length`, `proper_length`, `length_contraction_deficit`** — length contraction both ways.
- **`transform_velocity`, `compose_velocities`** — the collinear Lorentz velocity transformation, in
  both directions. `compose_velocities` delegates to `transform_velocity` with the frame reversed rather
  than repeating the algebra, so the two cannot disagree about a sign.
- **`SPEED_OF_LIGHT_M_S`**, written out as `299_792_458` because it is *exact* by definition of the
  metre — unlike `G`, which is the least precisely known fundamental constant. That difference is why
  the tolerances here are float-epsilon rather than five significant figures.
- **19 properties and 47 unit tests** (`66` in total), checked against published figures rather than against the
  formulas: exact rational `γ` at the Pythagorean `β`s (`3/5 → 5/4`, `√3/2 → 2`), the GPS
  constellation's `−7.2 µs/day`, the ISS at `28.2`, Earth's orbit at `0.156 s/year`, cosmic-ray muons
  living `29×` longer, Earth's diameter contracting by `6.3 cm`, and `0.5c + 0.5c = 0.8c`.

### Notes on three decisions worth finding again

- **`Option`, breaking this layer's convention.** Every other `physics_utils` function uses
  `debug_assert!` plus a benign value; `|v| ≥ c` returns `None` instead. A benign fallback is *actively
  wrong* here — `1.0` for `γ` says "no relativistic effect", the most misleading answer available, and
  `INFINITY` turns into `NaN` one multiplication later. See A27.2.
- **Three separate "difference" functions**, for a numerical reason rather than API symmetry. `γ − 1` is
  `4.9e-9` at Earth's orbital speed and `2.2e-17` at a walking pace, so subtracting `1.0` from `γ` loses
  half the significant digits at the former and returns **exactly zero** at the latter. The stable form
  multiplies through by the conjugate. Both halves of that claim are pinned by tests, including a lower
  bound on the disagreement — if the naive form ever becomes this accurate, the stable one has lost its
  reason to exist and the test says so.
- **The invariance of `c` is not bit-exact in `f64`.** It is exact in real arithmetic, but `c − v` and
  `1 − u·v/c²` round independently. Measured over `10⁵` values of `β`: bit-identical for about `65%`,
  worst relative deviation `1.9e-12` (~`8,600` ULP) as `β → 1`. My first draft asserted `assert_eq!` and
  failed at `β = 0.9` by one ULP; the tests now use a `1e-11` tolerance and one of them pins the worst
  case as *nonzero* so the claim cannot be strengthened back.

### Docs

- **`SPEED_OF_LIGHT_M_S`'s docs and a test now state the unit explicitly**, after the value was queried
  as possibly being km/s. It is m/s: `299_792_458 m/s` is `2.998e8 m/s`, the same speed as the commonly
  quoted `3e5 km/s` — the decimal point moves, the unit does not. Recorded with three cross-checks
  (light reaches the Moon in `1.28 s`, Earth in `8.32 min`, and laps the equator `7.5` times a second)
  because a unit error here would be off by exactly `1000×` while every derived figure still looked
  plausible.

## Milestone — a solar system under Newtonian mechanics (2026-09-15)

A marker, not a change: no code moved for this entry. `experiment_3` is a solar system integrated from
Newton's law of gravitation, built entirely from published measurements, drawn at true scale.

### What is actually real

- **Newtonian N-body gravitation**, every pair, no central-force approximation and no fixed orbits.
  Nothing is on rails: each body's path is the accumulated result of the forces from all the others,
  so the planets perturb each other and the moons are held by their primaries rather than scripted.
- **Real measurements throughout** — masses, radii, and semi-major axes for the Sun, all eight planets,
  Earth's Moon, and the four Galilean moons, plus the planets' real J2000 mean longitudes. Nothing was
  tuned to make the scene work.
- **True scale, with nothing compensating for it.** No size exaggeration and no apparent-size floor
  anywhere (A16/A17). The Sun is `109×` Earth and `10×` Jupiter because those are the numbers, and the
  planets are sub-pixel from any viewpoint that can see their orbits because at real distances they are.
- **A barycentric frame**, so the Sun carries exactly the momentum that cancels the planets' and the
  system does not drift.
- **Adaptive substepping** on two criteria — displacement and dynamical time — so Io's `1.77`-day orbit
  gets `~900` integration steps while the outer planets are not paid for.
- **Real light and real shadows.** A point light at the Sun with inverse-square falloff (A19), and
  per-fragment analytic sphere shadows (A22) — which produce a `97.7%` annular eclipse on Earth,
  because the Moon's angular radius is `0.9716` of the Sun's and so is genuinely too small to cover it.

### Verified against sources, not against itself

`769` lib tests, `2` integration, `77` doctests; `731` in release. The ones that matter for this claim
compare the scene to **published astronomy** rather than to the constants that built it, so a mistyped
exponent fails even though the scene it produces is self-consistent:

- every planet's orbital speed against published `km/s` and period against published day counts;
- Earth completes one orbit in one simulated year and returns, through the real integrator;
- Io at the real `6.03` Jupiter radii, the Moon at the real `60.3` Earth radii;
- the Galilean `1:2:4` Laplace resonance derived from the distances;
- a black hole's radius recomputed as `2GM/c²` and its capture radius as the tidal disruption radius;
- `experiment_5`, a static true-scale row, so the size ratios can be checked by eye against any
  reference rather than trusted.

### What it is not — the honest half

This is Newtonian, idealised, and single-precision. None of the following is a defect to be filed; they
are the boundaries of the claim:

- **Orbits are circular and coplanar.** Real eccentricities run to `0.206` (Mercury) and inclinations to
  `7°`. Periods still come out within a percent, but Mercury is the body this flatters most.
- **No relativity**, so Mercury's perihelion precession is absent — the historically famous thing
  Newtonian mechanics gets wrong about this exact system.
- **First-order symplectic Euler.** Momentum is exact at any step size; energy is not, so orbits precess
  slowly. **No test pins a trajectory**, deliberately — that would be pinning the integrator's error.
- **`f32` positions.** At Jupiter's distance the position quantum is `61 km`, about `3%` of Io's radius.
- **`MAX_SUBSTEPS` caps close encounters.** Measured: a fed black hole approaching a neutron star
  saturates the cap at `5.22` units and is `6.5×` under-resolved at contact. Momentum conservation
  cannot detect this, which is why it is written down here.
- **Physics is frame-rate dependent** — there is no `FixedUpdate`, so the simulation advances per
  rendered frame.
- **Absent entirely:** the asteroid belt, Pluto, the other ~285 moons, tides, rotation, radiation
  pressure, and any non-gravitational force. A black hole's capture radius does not grow as it eats.

## 2026-09-15

### Changed

- **Camera speed is now a mode cycled by `C`, not two held modifiers.** Amendment A26. `C` steps
  `normal (1×) → fast (3×) → slow (0.02×) → normal`; `Left Shift` is unbound. Holding a key was worst
  at exactly the flights it existed for — crossing `4495` units to Neptune, or creeping up on a
  `0.0064`-unit Earth are the two longest flights in the crate, and a modifier optimised for a
  momentary correction was being used for a sustained one.
- **The two speeds no longer stack.** A20 had `Shift` and `C` multiply, so holding both gave `0.06×` —
  a speed nobody asked for and nobody could name. Three speeds you can say out loud beats nine you
  cannot, and the precedence question goes away with the stacking.
- **`Left Shift` removed from `no_binding_collides_with_the_camera`'s camera-key list**, so it is
  genuinely free for a future binding rather than reserved by a stale test.
- **`KEY_BINDINGS` is 20 rows, down from 21** — one row for the cycle where there were two for the
  modifiers.

### Added

- **A `cam` row in the debug overlay** (`camera_speed_line`) naming the current mode and its
  multiplier. This is the part of A26 that makes the rest of it usable rather than merely fewer
  keystrokes: a held key announces itself, a mode does not, so without this row a user who tapped `C`
  once too often has a camera `150×` slower than they expect and nothing on screen to say why. Four
  tests treat it as load-bearing — that the three rows are mutually distinguishable, that the
  multiplier column is not ragged, and that it reads the controller rather than the constants.
- **`cycle_camera_speed_system`, deliberately separate from `free_fly_camera_system`.** The cycle is
  edge-triggered and movement is level-triggered, and one system that is both is the shape that
  produces "the mode advanced twice" bugs. `holding_c_does_not_race_through_the_modes` holds `C` for
  sixty frames and asserts exactly one advance — including simulating Bevy's per-frame clearing of
  the just-pressed set, which `MinimalPlugins` does not do and without which the test would have
  passed for the wrong reason.
- **Eight camera tests for the modes**, replacing the five that tested the holds:
  `the_camera_starts_at_normal_speed` (a fresh run flies as it always did),
  `c_cycles_normal_then_fast_then_slow_and_wraps`, `each_mode_applies_its_own_multiplier`,
  `every_movement_key_obeys_the_mode`, `slow_is_much_further_from_normal_than_fast_is`, and
  `the_old_hold_modifiers_do_nothing` — the last named so the removal of deep muscle memory is on the
  record rather than a silent no-op.

## 2026-09-14 (last, later)

### Added

- **Five tests measuring momentum conservation for an absorber**, prompted by a user asking whether
  a black hole conserves momentum the way a planet does after it swallows the Sun. It does, and the
  tests say so at the real masses rather than at the `10 kg`/`5 kg` of the existing doctest:
  `a_black_hole_eating_the_sun_conserves_momentum_at_real_masses`,
  `mass_is_conserved_exactly_by_a_merge_at_real_masses`,
  `gravitation_conserves_momentum_with_an_absorber_in_the_scene`, and
  `the_absorbs_flag_changes_no_force` — the last being the cleanest statement of the answer: two
  scenes differing only in `absorbs` integrate to bit-identical states, because absorption is a
  contact response and the force loop does not know it exists.
- **`momentum_is_conserved_even_when_the_step_is_far_too_large`**, which is the one worth reading.
  Symplectic Euler updates a pair by forces equal and opposite *by construction*, so the momentum
  error is exactly zero at any step size — a step a thousand times too long conserves momentum as
  perfectly as a correct one. Momentum conservation therefore cannot detect a wrong trajectory here,
  which is why checking it would not have found anything either way.
- **`a_placed_black_hole_and_neutron_star_saturate_the_substep_cap_before_contact`** — the real
  limitation the question surfaced. Measured: a `10 M☉` black hole that has eaten the Sun
  (`2.19e31 kg`) closing on a `1.4 M☉` neutron star saturates `MAX_SUBSTEPS` (`256`) at a separation
  of `5.22` units and is `6.5×` under-resolved by its `1.499`-unit capture radius. So the *shape* of
  a close encounter between two placed objects is wrong while every conservation check passes.
  Recorded in `MAX_SUBSTEPS`' docs, including the pointer to `-` as the way to resolve an encounter
  you actually want to watch.

### Docs

- **Three claims left stale by A17's removal of size exaggeration, now corrected.** All were found
  while answering the momentum question, none by a test:
  - `absorb.rs` said the horizon growth was applied to a radius `27,000×` life size and so was
    "proportionally right and absolutely wrong". Radii have been real since A17, so it is simply
    right.
  - `absorb.rs` also claimed the merge "grows the absorption radius, so a fed black hole eats more
    easily". **It does not.** A black hole captures at `contact_radius_units` — its tidal disruption
    radius — and `merge` never touches that field, so the documented behaviour did not happen. The
    tidal radius goes as `M^(1/3)`, so eating a Sun leaves a `10 M☉` hole's capture radius `3.2%`
    stale and a mass-doubling meal would leave it `26%` stale. Recorded as a limitation rather than
    patched: whether a capture radius should grow is a physics decision, not a typo.
  - Two user-visible picker notes were wrong. The neutron star's said "drawn 27,000x life size" and
    Sgr A*'s said it was "the only object here drawn true to scale" — every object has been true to
    scale since A17.

## 2026-09-14 (last)

### Added

- **Earth's Moon in `experiment_3`** — `EARTH_MOONS` in `src/experiments/constants.rs`, with the
  measured `7.342e22 kg`, `1737.4 km` radius, and `384,400 km` semi-major axis. Amendment A24. It costs
  nothing: its dynamical time asks `6` substeps of the shipped clock against Io's `98`, so Io still sets
  the frame price. It fits at true scale with `60×` clearance — the case that "Earth = 1 unit" would have
  made impossible, since at `157×` Earth's drawn hull was `2.6×` its Moon's whole orbit.
- **A real annular eclipse, as a consequence rather than a setting.** The Moon subtends `4.520e-3`
  radians from Earth's centre against the Sun's `4.650e-3` — a ratio of `0.9716`, *just* under one — so
  A22's shader draws a bright ring rather than total darkness. `94.5%` of the Sun covered from Earth's
  centre, `97.7%` from the surface one Earth radius nearer, which is the same closeness that makes real
  total eclipses possible. `the_moon_is_just_too_small_to_cover_the_sun` checks the ratio against the
  published figure and then puts the configuration through `star_visible_fraction` itself, because "the
  geometry is right" and "the renderer draws it that way" are two claims.
- **`MOON_SYSTEMS`, and `MoonSystem` beside `Planet`** — moons attached to primaries by *name*, so the
  builder has no special case for any one planet, reordering `SOLAR_SYSTEM_PLANETS` cannot reattach a
  moon to the wrong planet, and Saturn's moons would be one row plus one table. Amendment A24.3.
- **`+` multiplies the clock scale by ten**, the inverse of `-`, bound to `KeyCode::Equal` and
  `KeyCode::NumpadAdd`. Amendment A23. Bevy's `KeyCode` is a physical position, so there is no
  `KeyCode::Plus` — `Equal` is the key a user pressing `+` presses, and it fires with or without `Shift`
  because `=` and `+` are one key.
- **Nine tests for the two** — five in `controls_tests.rs` including `plus_and_minus_are_exact_inverses`
  (bit-exact, since the two share one constant) and the `MAX_TIME_SCALE` clamp, and two in
  `solar_system_tests.rs` for the Moon's published figures and the eclipse geometry.

### Changed

- **The precision movement modifier is `C`, not `Cmd`/`Alt`.** Amendment A25. `Cmd` was the wrong key
  on the platform this runs on: `Cmd`-`W` is macOS's close-window shortcut and `Cmd`-`W` here meant
  *precision plus forward*, the most natural pairing the modifier has — so the binding could shut the
  application mid-flight. A20 already knew that and added `Alt` as a safer alias rather than fixing it.
  `C` is beside `WASD`, unowned by any platform, and one `KeyCode` where the old scheme needed four
  (`SuperLeft`, `SuperRight`, `AltLeft`, `AltRight`). A20's recorded limitation about `Cmd` being
  intercepted by macOS is deleted rather than carried forward.
- **The old modifiers are removed rather than kept as aliases**, and
  `the_old_modifier_keys_no_longer_slow_the_camera` pins that — a stale `Cmd` binding on macOS is a
  live hazard, not a harmless leftover, so it gets a test that fails if it returns.
- **A20 clause 2 reversed.** It claimed there would deliberately be no `×10` key because `\` was the
  way back from `-`. That was wrong: `\` resets to the *experiment's* scale, discarding whatever pace
  the user had chosen, so `-` was a one-way door out of any scale but the default. Coming back needs an
  inverse, not a reset. Struck through in the spec with the reasoning, and corrected in
  `TIME_SCALE_DECADE_FACTOR`'s docs, which asserted the same thing.
- **The moon tests now loop over `MOON_SYSTEMS`** — `jupiters_moons_orbit_jupiter_and_not_the_sun`
  became `every_moon_orbits_its_primary_and_not_the_sun`, and both it and
  `nothing_overlaps_anything_at_true_scale` hold Earth's Moon to identical assertions rather than to a
  copy. A new system is covered the moment it is added, the convention the catalog hygiene tests already
  use.
- **`- ` and `+` are one bindings row**, `+ / -`, following `] / [`. The count stays at 21, and the row
  is inside the `Simulation` group — a binding outside its own group splits it in the `Esc` menu while
  leaving the flat help text unchanged, which `bindings_tests` catches and which was already got wrong
  once with `-`.

### Fixed

- **The eclipse shader was not found when the binary was run directly.** Bevy resolves its asset root
  relative to the *executable*, so `./target/release/rust-gui-app` looked in `target/release/assets/`
  and logged `Path not found` for `shaders/eclipse.wgsl`. `cargo run` sets the working directory to the
  crate root, so this failed only outside the way everyone had been launching it — and it failed
  *quietly* in the window, because a material whose fragment shader never loads still draws. The
  symptom was "the shadows stopped working" with the reason in a log line nobody was reading.
  `point_bevy_at_the_assets_directory` in `main.rs` now sets `BEVY_ASSET_ROOT` from
  `CARGO_MANIFEST_DIR` before `App::new`, but only when that directory really has an `assets/` in it
  and only when the caller has not set the variable themselves — an explicit choice outranks a
  compile-time guess, and a binary copied away from its source tree falls back to Bevy's own
  behaviour rather than to a path that no longer exists. Found by running the release binary as a
  binary instead of through `cargo run`, which is now worth doing after any asset change.
- **`moons_of` computes its slice offset from `MOON_SYSTEMS`** rather than reading to the end of
  `props`. The Jupiter-only helper was unbounded, so adding Earth's Moon silently put a fifth prop in
  the slice the Jovian assertions were iterating — the same class of failure that
  `the_planets_do_not_all_start_in_a_line` hit when the moons first went in.

## 2026-09-14 (later)

Eclipse shadows: Io's black dot crossing Jupiter, computed rather than sampled.

### Added

- **Analytic eclipse shadows** (`render/eclipse.rs` and `assets/shaders/eclipse.wgsl`), so a body casts
  a real shadow on another — the small black spot when Io transits Jupiter, and total darkness when Io
  passes behind it. Recorded as amendment A22 in
  the original requirements spec.
  - **Shadow maps cannot do this, and not by a little.** A Bevy point light casts through a cube map
    whose face spans `90°`, so a `1024²` map resolves `1.53e-3` rad per texel — at Jupiter's distance
    from the Sun that is `1.19` world units, **`17×` Jupiter's whole radius** and `656×` Io's. At
    `4096²` it is still `4.3×` Jupiter's radius. Resolving Io's shadow would need about **`671,000`
    texels per cube face**. So A19's `shadows_enabled: false` stands, now justified rather than merely
    explained.
  - Instead the shadow is **computed**: every body is a sphere and there is one star, so "how much of
    the star can this point see" is the overlap area of two discs on the sky. Exact at every scale, no
    resolution limit, a few instructions per occluder.
  - **Four cases**, and the third is the one that is easy to forget: the discs miss, the occluder
    swallows the star (umbra), the occluder sits *inside* the star's disc (annular), or they partly
    overlap — the last being a **penumbra whose softness is the star's own angular size** rather than a
    blur anyone chose. Without the annular case, Mercury crossing the Sun would produce total darkness
    instead of the real `0.003%` dimming.
  - Every body is drawn with `BodyMaterial` = `ExtendedMaterial<StandardMaterial, EclipseExtension>`,
    so PBR is untouched and the shadow multiplies the lit result. Each body gets **its own** uniform so
    it can be excluded from its own occluder list — a sphere's surface is exactly at its own radius, and
    self-occlusion would make the terminator turn on floating-point noise.
  - A scene with no star sets `star.w = 0`, which the shader reads as "nothing to eclipse" for the cost
    of one comparison. That is four of the five experiments.
- `assets/` — the crate's first non-Rust source directory, with a README explaining why the arithmetic
  exists twice and what the rule is for changing it.

### Tests

- 14 tests on the geometry, three of them against answers known independently of this code: Io's shadow
  on Jupiter is **total** (its disc is `4.83×` the Sun's from there) and only `2.6%` of the disc wide,
  Jupiter's shadow on Io is total (`186×`), and **Mercury transiting the Sun dims it by `0.003%`** —
  the figure from the transit literature.
- `the_penumbra_is_smooth_and_monotone` sweeps an occluder across the star in 200 steps and asserts the
  visible fraction never falls and never leaves `[0, 1]`. That continuity *is* the soft edge.
- `nonsense_inputs_are_fully_visible_rather_than_black` — every degenerate input fails **open**. A
  shadow bug that darkens is far harder to notice than one that does not, and a `NaN` multiplied into a
  colour propagates.
- `the_shader_mirrors_the_reference_thresholds` reads the WGSL as text and pins its four case
  boundaries and its two shared constants. It catches a threshold edited on one side only, which is the
  divergence that would actually happen — and it cannot catch more than that.

### What is and is not verified

There is no GPU in a test run, so the shader's **behaviour** is unverifiable here. What was collected:
the app runs on Metal for `22` seconds with an **empty log** for `experiment_3`, and for the three
starless experiments, which proves the shader compiled and the bind group matched. It proves nothing
about what it drew. **Fly to Jupiter and watch a moon cross it** — that is the remaining verification,
and `assets/README.md` says so.

### Known limitations

Multiple occluders multiply, which double-counts two bodies covering the same part of the star's disc —
that needs a syzygy. Only the first luminous prop casts shadows, so a scene with two stars shadows from
one. No refraction (no red rim on an eclipsed moon), no limb darkening (the penumbra is a shade too
uniform), and occluders are perfect spheres, so Jupiter's `6.5%` flattening makes its shadow slightly
the wrong shape.

## 2026-09-14

The moons are back in the solar system, at true scale, unchanged.

### Added

- **Jupiter's four Galilean moons return to `experiment_3`**, at true scale with no compromise to the
  sizes. Recorded as amendment A21 in the original requirements spec,
  reversing A17's clause 7.
  - A17 removed them for **cost, not correctness**, and recorded the cost as a worry rather than a
    measurement. Measured: **`3.2 ms` a frame in release — a fifth of a 60 Hz budget** — for `900`
    integration steps per Io orbit. The scaling needed no change at all: Jupiter is `0.0699` units and
    Io orbits at `0.4217`, so nothing overlaps and nothing is exaggerated.
  - Each moon's heliocentric velocity is **Jupiter's plus** its orbital velocity about Jupiter;
    omitting the first term would leave them near rest relative to the Sun and they would fall in.
  - Two things that did not exist when they were removed help now: `-` divides the clock by a decade,
    so two presses puts Io's `1.77`-day orbit at `15` seconds of watching rather than `1.5`; and the
    substep cost is paid only by scenes that contain a tight orbit.

### Fixed

- **A defect in the dynamical substep criterion, found by re-adding the moons.** A test asserting the
  planets-only substep count came out at `6` where the arithmetic says `4`. `orbital_substeps` skipped
  a pair only when **neither** prop could move — which passes for the Sun and the ground plane, and
  the floor sits `30` units below a `1.989e30 kg` Sun, where `√(r³/G·M)` is short enough to demand
  `6` substeps. **Two props that cannot interact were setting the timestep for the whole scene**, and
  every scene with a Sun and a floor had been paying for it since A16.
  - The condition is now that one prop must respond to forces **and the other must have mass to exert
    them**. Both halves matter: the floor is immovable *and* massless, so it fails both directions.
  - Also corrected: A16's claim that "the solar system alone asks for `1` substep" was wrong twice
    over — `4` from the displacement rule, which predates all of this, and `6` with the bug. The
    constant's docs and `orbital_substeps`' docs both said it.

### Tests

- `jupiters_moons_orbit_jupiter_and_not_the_sun` checks the velocity composition directly: relative to
  Jupiter the motion is tangential at the circular speed *about Jupiter*, and the heliocentric speed is
  still dominated by Jupiter's own.
- `nothing_overlaps_anything_at_true_scale` asserts Io orbits at the real `6.03` Jupiter radii — the
  figure that was `26×` too small under the old drawn radius.
- `the_moons_force_the_scene_to_substep_and_it_is_affordable` pins `98` substeps with the moons and
  `4` without, and that the ceiling is not silently capping. It is the test that found the bug above.

### Known limitations

The debug suite is back to about `130s` against `8s` in release — a `19%` runtime cost is a `16×` test
cost, because the long-running tests step this scene thousands of times. The moons remain two
magnifications away from visible (Io's orbit is `0.42` units at Jupiter's `778`-unit distance), which
is what `-` and a flight are for. And `f32` is the limit being approached: at Jupiter's distance the
position quantum is `61 km`, `3%` of Io's radius.

## 2026-09-13 (later)

Two controls, both consequences of drawing bodies at true size.

### Added

- **`-` divides the clock scale by 10.** `[` and `]` halve and double, which is right for finding a
  pace and nine presses too slow for changing *regime* — the scales here differ by decades (`1e4`
  Jovian, `1e6` solar, `2e7` ring), so a decade key matches how they actually differ. Clamps at
  `MIN_TIME_SCALE` like every other speed control. Recorded as amendment A20 in
  the original requirements spec.
  - No `×10` counterpart, deliberately: `\` resets to the experiment's own scale, which is the way
    back, and every extra binding is one more key to hit by accident.
- **Holding `Cmd` (or `Alt`) moves the camera at `0.02×` speed**, for approaching a body.
  Multiplicative with `Left Shift`'s boost, so holding both is `0.06×` — the simplest rule, and the
  only one with no invisible precedence order.
  - **The asymmetry with boost is the point.** Boost (`3×`) exists to cross a `4495`-unit solar
    system; precision (`0.02×`) exists to approach a body `0.0064` units across. At `0.4` units/s the
    camera crosses the Earth in `16 ms` and takes `3.5` seconds to pass the Sun.
  - **`Alt` is bound as an alias for `Cmd`** because on macOS holding `Cmd` and pressing `W` —
    forward — is the system's close-window shortcut. A movement modifier that can shut the app is not
    one to discover mid-flight, so the safe key is there without removing the one that was asked for.

### Tests

- Four tests on the decade key: the division, the floor clamp, that it composes with `[` and `]`
  rather than being a mode, and that it never advances time.
- Six on the modifier: that all four of `SuperLeft`/`SuperRight`/`AltLeft`/`AltRight` work and work
  identically, that it slows every movement key, that it combines multiplicatively with boost, that a
  bare modifier is not a direction, and that the slow end is more than `10×` more aggressive than the
  fast end — which is the property that keeps it useful.
- `bindings_tests::the_bindings_the_help_text_promises_are_the_ones_the_menu_draws` caught the first
  attempt: the new `-` row was inserted outside its own `BindingGroup`, which splits the group in the
  `Esc` menu while leaving the flat help text unchanged. Exactly the divergence that test exists for.

### Known limitations

`Cmd` may still be intercepted by macOS — prefer `Alt` there. `0.02×` is *still* not slow enough to
inspect a planet at true scale (the Earth is `0.0064` units and the camera covers `0.4` a second); a
per-experiment camera speed is the real fix. And no modifier scales look sensitivity, only
translation, so aiming at a body you are crawling toward is as coarse as it was.

## 2026-09-13

Space is dark, and the Sun lights the solar system.

### Changed

- **The camera clears to near-black** (`BACKGROUND_COLOR`, `(0.004, 0.005, 0.012)` linear) instead of
  Bevy's default mid grey. Recorded as amendment A19 in
  the original requirements spec. The grey was fine for a lit demo scene
  and actively wrong for space: since A17, `experiment_3` opens with a `4.6`-px Sun and nothing else,
  and a `4.6`-px Sun against `0.25` grey has almost no contrast. Not *pure* black, so a window that
  failed to render is still distinguishable from one that rendered empty space.
- **A star lights its own scene, from its own position.** The fixed directional light is now spawned
  only when no prop in the scene is luminous — both at once would wash out the terminator that
  lighting exists to produce. `experiment_3` is lit by its Sun; the two-body pair, the three-body
  ring, the Jovian system, and the scale-check row have no star and keep the old light. Nothing
  declares which: it follows from the scene.
  - **The intensity is derived, not chosen.** The Sun delivers `120,000 lux` at Earth's `149.6`-unit
    orbit and Bevy's illuminance is `intensity / (4π d²)`, so
    `STAR_LUMENS_PER_SOLAR_LUMINOSITY` is `3.375e10` lumens. The falloff is therefore right at every
    orbit — Mercury clips to white and Neptune receives `1/900` of Earth's light, because it does.
  - **The light is a child entity of the star**, which gets the star's position through transform
    propagation. `sync_mirror_entities_system` now despawns recursively, so an absorbed star takes its
    light with it rather than leaving an orphan lighting the scene from where it died.
  - **A star's material is emissive**, at `4×` its colour: its light source is inside it, so with
    reflected light alone its surface would render black — the one thing a star must not be.

### Added

- **`Prop::luminosity_solar`** and `Prop::with_luminosity`: a body's own light output, in solar
  luminosities. A physical property that only the renderer reads, exactly as `Prop::absorbs` is one
  only `collision` reads — a star is self-luminous and a planet is not, and that is a fact about the
  object rather than a lighting preference.
- **Real luminosities for the three placeable stars** — Proxima `0.0017 L☉`, Sirius B `0.056`, a
  neutron star `1e-5` in visible light — so placing a red dwarf lights the scene at `1/600` of the
  Sun's output rather than merely looking redder. Both black holes emit nothing, which is correct: an
  accretion disc would, and this crate does not model one.

### Tests

- `the_background_is_the_colour_of_empty_space` pins the clear colour and both of its bounds: dark
  enough to read as black, and not exactly black.
- `a_star_lights_its_own_scene_and_a_starless_one_falls_back` checks all five experiments — one point
  light and no directional light for the solar system, the reverse for the other four — and that the
  intensity is exactly one solar luminosity.
- `the_light_travels_with_the_star_it_belongs_to` asserts the mesh, the `BodyId`, and the
  `PointLight` are all on **one** entity, and that it sits where the Sun does.

### Fixed

- **The scene went dark whenever the camera looked away from the Sun.** The `PointLight` was on the
  Sun's *mesh* entity — `PointLightBundle` adds only three components beyond `PbrBundle`, so it seemed
  free — and `extract_lights` skips any light whose entity has `ViewVisibility == false`.
  `check_visibility` frustum-culls exactly those entities that **have an `Aabb`**, which a mesh entity
  gets from `calculate_bounds` and a light-only entity never does. So the light inherited the mesh's
  culling and the Sun leaving the frame took the sunlight with it. The light is now a **child**
  entity: no bounds, never culled, and `InheritedVisibility` is unaffected by frustum culling, so it
  stays lit while the parent mesh is off screen.
  - Pinned two ways: `a_stars_light_is_a_child_entity_with_no_mesh_of_its_own` asserts that no
    `PointLight` shares an entity with a `Handle<Mesh>`, and `removing_a_star_removes_its_light`
    covers the orphaned-light failure the recursive despawn prevents.

### Known limitations

No shadows — a cube shadow map spanning the `20,000`-unit light range would have a texel wider than a
planet — so nothing eclipses anything. The exposure is fixed, so Mercury clips and Neptune is nearly
black; both are true. And a star's *emissive* brightness is one constant for all of them, so a red
dwarf's surface looks as bright as the Sun's while the light it casts is `600×` weaker: Bevy models
those separately and only the second is physics.

## 2026-09-12 (and finally)

A ruler, so the scale can be checked by eye rather than by arithmetic.

### Added

- **`experiment_5` — the scale check** (`experiments/scale_check.rs`): the Sun and the eight planets
  in a static row at true size, bottoms aligned, for verifying the proportions directly.
  `cargo run --release -- scale`. Recorded as amendment A18 in
  the original requirements spec.
  - It exists because of what A17 cost: `experiment_3` draws everything at true size and real
    distance, so the planets are sub-pixel — and therefore the one thing you cannot do there is
    *check* that the sizes are right. Three amendments in a row traded honesty against legibility and
    each was decided from computed pixel figures rather than from looking at anything.
  - **Removing the distances is the whole trick.** The row is `1.944` units long against a solar
    system `4495` units across, so the entire `109:1` range fits one view: from the opening camera
    the Sun is `1012` px, Jupiter `102`, Earth `9.3`, and Mercury `3.6` — every one a true
    proportion.
  - **Every body is immovable**, which also zeroes its mass and makes it gravitationally inert. A
    `1.989e30 kg` Sun beside a real Mercury `0.72` units away does not survive one frame, and pausing
    the clock would leave a simulation waiting to happen. A ruler must not be able to bend, so this is
    the only scene in the catalog with no dynamics at all — `stepping_the_scene_changes_nothing_at_all`
    runs it through the real integrator for 120 worst-case frames and compares the whole scene for
    equality.
  - Radii come from `SOLAR_SYSTEM_PLANETS` and the Sun's own constant rather than a second table: an
    instrument has to be measuring the same object as the experiment it checks.

### Fixed

- `catalog_tests::every_experiment_frames_its_own_scene` filtered out **immovable** props to skip the
  floor, so a scene that is entirely immovable had no subjects and its "the view contains a body"
  assertion read `inf`. It now excludes `Body::Plane`, which is the rule it always meant — the floor
  is scenery because it is a ground plane, not because it is fixed.

### Tests

- `tst/experiments/scale_check_tests.rs` — 13 tests in three groups, matching what an instrument has
  to get right: that it measures the real quantity (radii identical to `experiment_3`'s, and the
  Sun:Earth `109.2`, Sun:Jupiter `9.95`, Jupiter:Earth `10.97`, Earth:Mercury `2.611` ratios against
  published figures), that it is readable (every body over 3 px, the Sun under the frame height, the
  framing recomputed from the layout rather than stored), and that it cannot move.

## 2026-09-12 (later still)

The apparent-size floor is gone. A body is drawn at its true radius and nothing compensates for it.

### Removed

- **The apparent-size floor** (`MIN_APPARENT_RADIUS_FRACTION`, `apparent_radius_floor`, and
  `transform_from`'s `min_radius_units` parameter). Recorded as amendment A17 in
  the original requirements spec.
  - It was added hours earlier to make true-scale bodies visible, and it made them **all the same
    size** — including the Sun, which at true scale subtends `4.64` px from the opening framing and
    so sat at the `4.89`-px floor along with Mercury's `0.016` px. A floor that renders the Sun
    indistinguishable from Mercury has destroyed the one proportion a viewer could read while
    claiming to be an aid. Drawing nothing is more informative than drawing nine equal dots.
  - What replaces it is the camera. `experiment_3` now opens as one `4.6`-px Sun and nine orbit
    trails — the trails are fixed-width gizmo lines, so the *orbits* stay legible even where the
    bodies are not — and each planet becomes an object at the right size when you fly to it. The Sun
    is `57` px from `50` units out and `284` px from `10`.
- **Jupiter's moons in `experiment_3`.** Not because they failed — they worked, and they are the
  reason the crate now separates physical size from drawn size — but because a moon system inside a
  planetary one needs a step `100×` finer than the planets do. One frame at
  `SOLAR_SYSTEM_TIME_SCALE` is `10.9%` of Io's whole orbit, so the scene took about `98` substeps to
  carry four bodies that are `0.041` px across from where the camera starts. `experiment_4` is the
  same physics without the mismatch.
  - The debug test suite went from `131s` back to `53s` as a direct result.

### Note on why one unit cannot fix this

Recorded because it is the obvious next idea and it does not work. Redefining the unit — "1 unit =
Mercury's radius, and scale sizes *and* distances against it" — changes no proportion at all: scaling
every length by one factor is a similarity transform, and Mercury's radius over the Sun's is `0.00351`
before and after. The quantity that governs visibility is a **ratio**, and a ratio is unit-free —
Earth's orbit is `23,481` Earth radii however it is measured. The only factor that would make Earth
visible from the opening framing is `≥115×` applied to *sizes but not distances*, which is the
exaggeration A16 removed: at `157×`, Earth is drawn `1.0` unit and contains the Moon's orbit
(`0.384`), Jupiter contains Io's, and Saturn contains Mimas'.

### Changed

- `SOLAR_SYSTEM_SOFTENING_UNITS` keeps its `0.001` value, and its justification is now the simpler
  one: softening models a body's finite size, and the largest body in the scene is the Sun at `0.696`
  units — so the crate-wide default of `1.0` is larger than the Sun itself.

### Tests

- The tests that guarded the floor now assert its absence and say why:
  `the_compact_objects_are_genuinely_invisible_at_range_and_that_is_the_truth` records that a placed
  neutron star subtends `0.001` px and is simply not drawn, and
  `the_scene_softens_far_below_the_size_of_its_smallest_body` compares ε against the Sun's true
  radius rather than against Io's orbit.
- `there_are_no_moons` replaces the moon-orbit test, with the reason in the body — a test that says
  what was removed and why is worth more than a silent deletion.

## 2026-09-12 (later)

Jupiter's moons in the solar system — which meant giving up the size exaggeration, and that
removed four compromises at once.

### Changed

- **Every body is now stored, drawn, and collided at its true size.** The `157×` exaggeration is
  gone, and visibility moved to the renderer: a sphere is never drawn smaller than
  `MIN_APPARENT_RADIUS_FRACTION` (`0.0012`) of its distance from the camera — a constant *angular*
  size, so distant bodies read as points and each grows into its real proportions as you approach.
  Recorded as amendment A16 in the original requirements spec.
  - The exaggeration existed for exactly one thing — apparent size — and silently governed four
    others. Removing it fixed all four: **the Sun** is no longer capped at `20` units, so its ratio
    to Jupiter is the true `9.95:1` rather than `1.8:1`; **a black hole** no longer absorbs at
    `27,000×` its event horizon; **`STELLAR_OBJECTS`** collapsed from three drawn-size regimes to
    none, so a neutron star can be the `1.1e-5` units it really is; and **Jupiter can hold a moon**.
  - The floor cannot affect the simulation by a rounding error. Contact reads
    `Prop::contact_radius_units` or the body's radius; gravitation reads no radius at all. The drawn
    size is the only non-physical quantity left in the crate, it is computed per frame in the render
    layer, and it is stored nowhere.
- **`substep_count` now considers the tightest orbit's dynamical time**, not only the fastest
  body's displacement, taking whichever criterion is stricter
  (`MAX_SUBSTEP_DYNAMICAL_FRACTION = 0.007`, about 900 steps per orbit). The displacement rule asks
  *how far does the fastest body move* — right for contact, blind to orbits. At `experiment_3`'s
  clock **one frame is 10.9% of Io's entire orbit** and the old rule asked for 3 substeps where 98
  are needed. `MAX_SUBSTEPS` rose `64` → `256` so that ceiling is not reached silently.
  - This also improved `experiment_1`: its close encounter now gets **12 substeps** where it had 1,
    because `t_dyn` falls from `1.8e8` s at the `30`-unit start to `5.8e6` s at contact. The final
    approach had been the most coarsely integrated part of the experiment and nothing could see it.
- **A black hole's contact radius is its tidal disruption radius**,
  `R_t ≈ R_star · (M_hole/M_star)^{1/3}` — `1.5e9 m` for the `10 M☉` hole against a `2.95e4 m`
  horizon. Absorption stayed observable *because* it became more accurate, which is the happiest
  kind of fix.

### Added

- **Jupiter's four Galilean moons in `experiment_3`.** Each carries a heliocentric velocity of
  Jupiter's **plus** its orbital velocity about Jupiter; omitting the first term would leave them at
  rest relative to the Sun and they would fall straight in. The scene sets
  `SOLAR_SYSTEM_SOFTENING_UNITS` (`0.001`), since the crate default leaves `15%` of Newton's force
  at Io's `0.4217`-unit orbit.
- `apparent_radius_floor`, and `transform_from` gains a `min_radius_units` parameter. Applied to
  spheres only — the other variants are extended geometry whose apparent size was never a problem.
- `resolve_contacts` builds hulls **lazily**, behind a bounding-sphere pre-filter. At 98 substeps
  the old approach meant 1,274 icospheres per frame for a scene whose closest pair is 0.42 units
  apart with a combined radius of 0.07.

### Removed

- `SOLAR_SYSTEM_EARTH_DRAWN_RADIUS_UNITS`, `SOLAR_SYSTEM_SUN_DRAWN_RADIUS_UNITS`,
  `JOVIAN_DISPLAY_EXAGGERATION`, `StellarObject::drawn_radius_units` and the four methods that
  existed to reason about it (`radius_at_planet_scale_units`, `is_drawn_to_scale`,
  `is_drawn_true_size`, `exaggeration`). Deleting these was the point of the change.

### Tests

- The tests that used to *guard* the two exaggerations now assert the opposite, and each names the
  defect it replaced — `the_sun_is_now_correctly_proportioned_against_jupiter`,
  `every_body_is_stored_at_its_true_size`, `a_black_hole_captures_at_its_tidal_radius_not_its_horizon`.
  "There is no longer a lie here" is only meaningful if the lie is on the record.
- `jupiters_moons_orbit_jupiter_and_not_the_sun` checks the velocity composition directly: relative
  to Jupiter the motion is tangential at the circular speed *about Jupiter*, and the heliocentric
  speed is still dominated by Jupiter's own.
- **The two-body encounter test found a real behaviour change and got stronger for it.** With 12
  substeps the approach-bounce-recede sequence now fits inside one frame, so a once-per-frame
  distance sample missed the contact entirely. It now detects the bounce by the pair **separating** —
  gravitation alone cannot reverse a pair released from rest — which is frame-rate independent and a
  stronger claim than "they were once within 3 units".

### Note

The debug test suite is `9×` slower (`14s` → `129s`): the solar system takes ~98 substeps per frame
in the long-running tests. Release is `8.8s`, which is what the app runs on, so this is a
test-ergonomics cost rather than a runtime one.

## 2026-09-12

Moons around Jupiter — and the two places the crate had to stop conflating a visual quantity with
a physical one before they were possible.

### Added

- **`experiment_4` — Jupiter and the four Galilean moons** (`experiments/jovian.rs`). The only
  scene in the crate whose **physics is exactly real**: real masses (Jupiter from the same
  `SOLAR_SYSTEM_PLANETS` row `experiment_3` orbits around the Sun, so its mass has one home), real
  orbital radii, real circular speeds derived from `√(GM/r)`, real *contact* radii, and softening
  small enough to be invisible. Recorded as amendment A15 in
  the original requirements spec. `cargo run --release -- moons`.
  - **The only scaled quantity is the pixels.** Every body is drawn at `4×` its true radius,
    applied uniformly, so every size ratio in the picture is real. That factor is bounded on both
    sides and the bounds are the interesting part: under `2×` the moons fall below `4` px, and over
    `6.03×` Jupiter's drawn surface swallows Io's orbit. At `4×` there is `0.135` units of clear
    space, so **nothing visually contains anything** — which a test enforces.
  - **The Laplace resonance is not put in by hand.** Io : Europa : Ganymede come out `2.0073` and
    `2.0147` from the published semi-major axes, and Callisto is genuinely outside it at `2.33`.
  - **One clock scale suits every body**, unlike the solar system: the periods span `9.4×` rather
    than `685×`. Io comes round every `15` seconds of watching, Callisto every `144`.
  - Barycentric frame, as `experiment_3` uses for the Sun. Jupiter's recoil is `2.5e-9` units/s —
    tiny, free, and correct.
- **`Prop::contact_radius_units`** — the radius a prop **physically** interacts at, when that
  differs from the radius it is drawn with, plus `collision::tessellate::contact_mesh(&Prop)` as
  the single seam every contact test now builds its hull through. Contact and absorption read it;
  **gravitation never does**, because gravitation treats every prop as a point mass and has no
  radius in it at all.
- **`Scene::softening_units`** — Plummer softening is now a property of the scene.
  `integrate_gravity` and `gravitational_acceleration` take it as a parameter, and
  `GRAVITY_SOFTENING_UNITS` remains the default every scene starts from.
- **`Scene::empty(bounds)`** — a scene with no props at all. `experiment_4` uses it: the shared
  floor is `5000` units across with a grid ruled every `5`, and the whole Jovian system is `1.9`
  units, so a ground plane would be a surface with no relationship to anything in the scene. The
  render layer already coped — `floor_half_size` returns `None` and no grid is spawned.

### Why those two changes were forced

Worth recording as a finding rather than only as a diff, because both were invisible until a scene
needed two length scales at once:

- **The drawn radius was also the collision radius.** `experiment_3` draws Jupiter at `10.973`
  units — a `157×` exaggeration, without which it is invisible from Mars's orbit. Io orbits at
  `0.4217`. The drawn Jupiter is therefore **`26×` the radius of its innermost moon's entire
  orbit**, so all four Galilean moons begin inside the planet's hull and the contact solver ejects
  them on the first step. The largest exaggeration that keeps Jupiter inside Io's orbit is `6.03×`.
  No single number both shows Jupiter and lets it hold a moon.
- **Softening was a crate-wide length.** `GRAVITY_SOFTENING_UNITS` is `1.0` unit and Plummer
  softening leaves `r²/(r² + ε²)` of Newton's force, so at Io's orbital radius it left **`15%`** —
  a circular speed out by a factor of `2.6`. The moons could not have orbited with the renderer
  switched off entirely. Nor could ε simply be lowered globally: at `experiment_1`'s contact
  separation of `3` units it is doing `10%` of the work. The constant's own docs say it models a
  body's *finite size*, so it was always a per-scene quantity — body radii now span `1.5` units to
  `0.0018`.

### Changed

- Amendment A15 narrows A2's contact model and A11's uniform-exaggeration clause. **No existing
  experiment changes**: all three set no contact radius and keep the default softening, which two
  tests assert. The render layer needed **no change at all**, which is the clearest evidence the
  split was cut in the right place.
- `ExperimentId::ALL` grows to four, so `--list`, `--help`, and every catalog hygiene test pick up
  `experiment_4` without a second edit.

### Tests

- `tst/experiments/jovian_tests.rs` — 27 tests. Published masses, radii, orbits, and periods; the
  resonance derived from the distances rather than asserted; Ganymede confirmed larger than
  Mercury; every prop colliding at its true radius under a uniformly `4×` drawn one.
- **Two tests pin the limits rather than the fix**: that the crate-wide softening leaves under
  `20%` of Newton's force at Io's orbit, and that `experiment_3`'s drawn Jupiter is more than `25×`
  Io's orbital radius. Neither tests the Jovian system — they test the *reasons*
  `Scene::softening_units` and `Prop::contact_radius_units` exist, so a future simplification that
  removes either fails with the argument attached.
- Two real integrations: Io through one full orbital period and back to within `10°` of its
  starting longitude on the same circle, and all four moons holding their radii to `5%` over ten Io
  periods.
- `scene_tests` gains five tests for `Scene::remove`'s neighbour `Scene::empty` and the new
  `Prop` validation.

### Docs

- Amendment **A15**, with the limitations recorded: circular coplanar orbits (so a circular Io
  cannot show the tidal heating its real eccentricity causes), arbitrary starting phases rather
  than an ephemeris, no Jupiter oblateness — so the simulation holds the resonance for the wrong
  reason, because it started that way — and the camera still tuned for a planetary system, at `20`
  units/s across a `3.8`-unit scene.
- `src/experiments/README.md`, `src/core/README.md`, and `src/collision/README.md` each record the
  split from their own side; the root `README.md` states what is real in `experiment_4` and what is
  not.

## 2026-09-08

Interstellar objects you can place into a running experiment, and the second contact response
they needed.

### Added

- **A catalog of five interstellar objects** (`experiments/objects.rs`, `STELLAR_OBJECTS`): a
  red dwarf (Proxima Centauri), a white dwarf (Sirius B), a `1.4 M☉` neutron star, a `10 M☉`
  black hole, and Sgr A*. Masses and radii are published figures, and a black hole's radius is
  its Schwarzschild radius `2GM/c²` — recomputed from the mass in a test rather than trusted.
  Recorded as amendment A14 in the original requirements spec.
  - **Drawn size follows three rules, because one is impossible.** The dwarfs use the planets'
    `157×` exaggeration unchanged — a white dwarf is Earth-sized, so it lands at `0.918` units
    for free, which is why it is in the catalog. The neutron star and the stellar black hole are
    `0.0017` and `0.0046` units at that scale — three orders of magnitude below a pixel — so
    they are **floored** at `0.30` and `0.806`, chosen to keep their true `2.685:1` ratio to
    each other. Sgr A* is drawn at its **true size**, `12.693` units: a supermassive event
    horizon is the one astronomical object this crate can show honestly.
  - Each regime is pinned *as* a regime by `objects_tests`, because the real risk with a table
    like this is that a floor chosen deliberately comes to look like a scale everyone assumes
    is real.
- **`B` places an object into the running scene**: the picker opens bottom-centre, `Up`/`Down`
  choose, `Enter` places it in front of the camera at rest. Three layers for one feature —
  `input/placement.rs` holds the selection, `ui/placement.rs` draws the list,
  `render/placement.rs` adds the prop — because **only `render` may write the `Scene`**, which
  is the rule that gives this crate its shape.
  - **Not modal**, unlike the `Esc` menu: the object goes where you are pointing, so flying and
    looking keep working. None of its four keys collide with the camera or the clock, and a test
    says so. Opening the menu *closes* the picker, so dismissing the menu cannot reveal a
    forgotten picker with `Enter` armed.
  - The placement distance is `max(40 units, 3 × the object's drawn radius)` — a floor plus a
    clearance rule, because the catalog spans radii from `0.30` to `16.8` units and one fixed
    distance would put the camera inside a red dwarf.
- **Absorption: a second contact response** (`collision/absorb.rs`). A prop touching one whose
  `absorbs` flag is set is merged into it and **leaves the scene**. An elastic bounce off an
  event horizon is not an approximation of what happens, it is the opposite of it — so this is
  a second response rather than a tuning value on the first, and it runs **before** the elastic
  pass so nothing bounces off a black hole on its way in.
  - Mass conserved exactly, momentum conserved, kinetic energy **not** — perfectly inelastic by
    construction, and a test asserts the loss rather than tolerating it. The absorber's radius
    grows linearly with its new mass, which is correct for `r_s = 2GM/c²`.
  - **An immovable prop is never absorbed.** The first version deleted the ground plane the
    moment a black hole was placed near it — a category error rather than a physics result,
    since an immovable prop is the container the simulation happens inside. Found by a test,
    not by inspection.
- **`Scene::remove`**, the first operation that takes a prop out of the world, and
  **`sync_mirror_entities_system`**, which reconciles the ECS mirror every frame instead of
  once at startup. Placement adds props and absorption removes them; without per-frame
  reconciliation a placed object would be simulated and drawn nowhere, and an absorbed one
  would leave an entity frozen at a dead prop's last position.

### Changed

- **Requirement 5.4 is narrowed by A14**: a `BodyId` may stop resolving if it was absorbed, and
  once it does it never resolves again. Everything downstream was already written for this —
  `Trails::record_from` dropped trails for departed ids before anything could depart, and said
  so in its own docs.
- **Requirement 3.5's mirror invariant is now *maintained* rather than established.** The count
  of `MirroredBody` entities matches `scene.props.len()` every frame, not just after `Startup`.
- `Prop` gains an `absorbs` field and a `Prop::absorbing` constructor. A field on `Prop` rather
  than a property of the object catalog, because the `collision` layer has to act on it and may
  not read `experiments`.
- `KEY_BINDINGS` gains a `Place an object` group (`B`, `Up`/`Down`, `Enter`), so both `--help`
  and the `Esc` menu list the new keys without a second edit.

### Fixed

- A **pre-existing flaky property test**, `compass_tests::each_arm_is_a_projected_unit_vector`,
  which asserted `toward_viewer ∈ [-1, 1]` exactly. `q⁻¹ · d` for a normalised `f32` quaternion
  can land a component a few ULPs outside `±1`, and proptest finally found one. The depth cue
  only reads the *sign*, so the bound is now tolerant — the assertion is there to catch a
  component that has stopped being a direction cosine, not to police the last bit.

### Docs

- Amendment **A14**, with the limitations recorded plainly: the drawn radius is also the contact
  radius (so a black hole absorbs at `27,000×` its true horizon), Newtonian gravity stops being
  defensible around Sgr A* (`0.25 c` at `100` units), placing a supermassive black hole ends the
  experiment immediately, and the picker cannot give an object a velocity.
- `src/experiments/README.md` gains the three-regime size table; `src/collision/README.md`
  explains why there are two responses and which runs first; `src/render/README.md` records why
  the mirror is now reconciled every frame and why the system order matters.

## 2026-09-07

An `Esc` menu, and with it a single input mode.

### Added

- **An in-window menu, opened with `Esc`** (`ui/menu.rs` for the panel, `input/menu.rs` for
  the state and the key). Two read-only sections: the **current settings** — experiment,
  seed, time scale, paused or running, elapsed simulated time, trail length, unit scale —
  and the **key bindings**. Recorded as amendment A13 in
  the original requirements spec.
  - **Why now:** thirteen bindings and no way to see them without quitting and running
    `--help`, in an app that is full-screen (A12) and so has no title bar to hang anything
    off. `Esc` was already doing a job — releasing the cursor — that is exactly what should
    happen when a menu opens, so the two fit together rather than competing.
  - **The state is an input mode, and it lives in `input/`.** `MenuState` is read by the
    camera, the clock keys, and the trail toggle to decide whether to run at all. Holding it
    in `ui/` would mean three systems in two layers importing the UI toolkit to ask a
    question about input, inverting the dependency direction that already exists.
  - **One rule while the menu is open: only `Esc` is bound.** Movement, mouse-look, capture,
    `P`, `.`, `]`, `[`, `\`, and `T` are all inert. One rule rather than a per-key
    judgement — a menu in which `W` still flies is a menu you cannot read, and "which keys
    should still work" is a set nobody can remember.
  - Two things the gate deliberately still does: **releases the cursor** (in the camera
    system, which already owns that field, so it keeps exactly one writer) and **drains
    `MouseMotion`** every frame — buffered deltas would otherwise all land on the frame after
    the menu closed and snap the view. Both have tests.
  - **The menu does not pause the simulation.** `P` does that; a menu that also paused would
    silently change the run you opened it to inspect.
  - Every settings row is read from the resource that owns it — the `ExperimentSpec`, the
    `SimulationClock`, the `TrailConfig` — so the panel cannot disagree with the app about
    what the app is doing.
- **`src/bindings.rs` and `KEY_BINDINGS`** — the key bindings as data, at the crate root
  beside `constants.rs`. `--help` (in `experiments`) and the menu (in `ui`) both need the
  list and neither layer may import the other, so a table owned by either would have been
  duplicated in the other — and a duplicated key list drifts the first time a binding
  changes, in the one place a user looks to find out what the bindings *are*.
  `bindings_tests::the_bindings_the_help_text_promises_are_the_ones_the_menu_draws` is the
  test that would have caught exactly that.

### Changed

- **`Esc` no longer releases the cursor as its own branch** in
  `free_fly_camera_system`; it opens the menu, and the camera system releases the cursor
  *because* the menu is open. Same effect for the user, one writer of the window's cursor
  state, and Requirement 10.8 amended rather than quietly contradicted.
- **`--help` prints the bindings from `KEY_BINDINGS`** instead of a hand-written block, so it
  now shows them grouped (Camera / Simulation / View / Menu / Window) and lists `Left click`,
  `Mouse`, and the quit keys it had been silent about.
- The three gated systems take **`Option<Res<MenuState>>`** rather than `Res<MenuState>`:
  absent state means no menu, which means closed. Half the tests in this crate build the
  smallest app that exercises one system, and a hard `Res` turns "I added one system" into a
  first-frame panic.

### Docs

- Amendment **A13**, including what is deliberately not done: the menu is read-only, the
  panel's layout is only verifiable by running it, the binding table is documentation and can
  lie, and there is no scrim behind the panel.
- `src/input/README.md` gains a section on why the menu's state is an input mode and what the
  gate must still do; `src/ui/README.md` records that the menu is the one module here allowed
  an interactive widget, and why that does not breach Requirement 11.4.
- `tst/input/README.md` notes that the gate's tests need an `ObserverClock` that has already
  ticked — without it the "camera holds still" assertion passes for the wrong reason.

## 2026-09-06

`experiment_3`: the solar system. One new experiment, and the three changes it forced on
the rest of the app.

### Added

- **`experiment_3` — the Sun and the eight planets** (`experiments/solar_system.rs`), built
  from published measurements: masses, mean radii, and orbital distances in SI, starting
  positions from the real **J2000 mean longitudes**. Orbital speeds are *derived* from
  `v = √(G·M/a)` rather than tabulated, and land within `2%` of the published mean orbital
  velocities. Recorded as amendment A11 in
  the original requirements spec. Run it with
  `cargo run --release -- solar-system`.
  - **Distances are not scaled.** Mercury starts at `57.9` units and Neptune at `4495.1`,
    exactly `a / METRES_PER_UNIT` — so the gravity the integrator computes matches the
    distances on screen. That is what a distance-compressed diagram gives up, and it is why
    this is a simulation rather than an illustration.
  - **Drawn radii are exaggerated by one uniform factor of `~157`**, fixed by drawing the
    Earth at `1.0` unit — deliberately the same size as a body in `experiment_2`, against the
    same `5`-unit grid. At true scale the Earth is `0.0064` units, a four-hundredth of one
    grid square and far below a pixel from any camera that also shows an orbit; a
    strictly-true-scale rendering is an empty screen. Uniform means every ratio *between*
    planets stays exactly real — Jupiter is `10.973` units to Earth's `1.000`.
  - **The Sun is the one exception, and it is forced rather than chosen.** On the planets'
    scale it would be `109.2` units, larger than Mercury's whole orbit; since props became
    solid, it would begin with the inner four *inside* it and eject them on the first step. It
    is drawn at `20` units — still `1.8×` Jupiter, and `26` units clear of Mercury's
    perihelion. The existing "nothing starts touching" catalog test is what bounds that
    constant.
  - **The Sun is given the momentum that cancels the planets'** — the barycentric frame.
    Without it the net momentum is about `3e31 kg·m/s`, mostly Jupiter's, and the whole system
    drifts out of frame at `150` units per `1e10` simulated seconds. It also makes the Sun's
    own wobble, the signal real exoplanet searches measure, visible in its trail.
  - The scene **widens its own `Scene::bounds`** from the shared `±50`, which would be a false
    statement about a system `4495` units across.
  - Deterministic: every initial condition is a measurement, so the seed cannot change it.
- **`ExperimentSpec::framed_radius_units`** — a *declaration* of what the opening view
  contains, because for the solar system that is deliberately less than the scene's extent:
  framing Neptune would put the camera `10,850` units out, from where nothing is a pixel
  across. `240` units, out to Mars. Kept honest from both sides —
  `every_experiment_frames_its_own_scene` now checks the camera is far enough back for the
  declared radius *and* that the radius contains at least one body. Before this the framing
  test could only compare against the full extent, which for this experiment is not a view
  worth opening on.
- **`ExperimentSpec::trail_length_units`** and `apply_experiment_trail_length` — an experiment
  may choose its own trail length; `None` means the render layer's default is right, which is
  the answer for both existing experiments, so neither changed. `experiment_3` asks for `400`
  units, since `10` is a hundredth of one Earth orbit. **Sample spacing is derived from the
  length** (`TRAIL_TARGET_SEGMENTS`, `256`), so cost per body stays flat: keeping the
  `0.05`-unit default for a `400`-unit trail would want `8000` samples, hit the `512` cap, and
  silently draw a trail a twentieth of its stated length.
- **`CAMERA_FAR_UNITS` (`20,000`)** on the scene camera's projection. Bevy's default clips at
  `1000` units, which was invisible while every scene was `30` units across and deletes
  Saturn, Uranus, Neptune, and most of the floor from this one. Reversed-Z depth means the
  precision cost of a distant far plane is essentially nothing.

### Changed

- **The window opens full-screen** (`WINDOW_MODE` = `WindowMode::BorderlessFullscreen`),
  superseding Requirement 1.1's 800×600 window; recorded as amendment A12. This is an app you
  look *through* — free-fly camera, centre crosshair, cursor capture — and the pixel budget is
  what limits what can be seen: at 800×600 a `1`-unit planet on a `150`-unit orbit is two or
  three pixels across, which `experiment_3` made binding rather than merely unfortunate.
  **Borderless** rather than exclusive keeps the display's resolution and refresh rate, so there
  is no mode switch on launch and no other window rearranged on exit. `WINDOW_WIDTH_PX` ×
  `WINDOW_HEIGHT_PX` is retained as the windowed fallback and as the 4:3 aspect the
  camera-framing constants were derived against — every framing in the crate comes from the
  *vertical* field of view, so a wider display adds horizontal margin rather than cropping.
  The cost is recorded in A12: no title bar means no close button, so quitting is `Cmd`-`Q`,
  `Alt`-`F4`, or `Ctrl`-`C`, and both `--help` and the README now say so.
- **`DEMO_FLOOR_HALF_SIZE_UNITS`: `2000.0` → `5000.0`**, so all eight orbits have grid under
  them — Uranus at `2872` and Neptune at `4495` were over the floor's edge. The grid measures
  the floor rather than assuming it, so it followed with no second edit: `202` coarse line
  entities against `82`. Amends A8's extent; the two grid entity-count ceilings moved from
  `200` to `400` to match, and they are still bounds on *readability* rather than on cost.

### Docs

- `src/experiments/README.md` gains a section stating exactly which numbers in `experiment_3`
  are real and which are not, and why the Sun has to be the exception; the root `README.md`
  and `--help` say the same in shorter form, including the two things a first-time viewer has
  to be told — the opening view is the inner four, and no clock scale suits both Mercury and
  Neptune.
- `src/render/README.md` records the per-experiment trail length and the far plane;
  `src/core/README.md` and the grid section were updated for the larger floor.
- Amendment **A11** in the original requirements spec, including
  the limitations recorded deliberately: circular coplanar orbits, no single workable clock
  scale, the Sun not lighting the scene, no moons, and the giants starting outside the frame.

### Tests

- `tst/experiments/solar_system_tests.rs` — 29 tests. Most check the scene against **published
  astronomy** rather than against its own constants: orbital speeds in `km/s`, periods in days,
  size ratios to Earth. A test that only compared the scene to the constants that built it
  would pass with the mass of the Sun mistyped.
- Both exaggerations are pinned *as* exaggerations, with the reason in the test body, so
  neither can drift into looking accidental — and `the_sun_is_the_only_body_off_the_planet_scale`
  fails if the Sun ever grows enough to reach Mercury.
- `earth_completes_one_orbit_in_one_year_and_comes_back` runs the real integrator at the
  shipped clock scale for one simulated year and checks Earth returns to within `10°` of its
  starting longitude at the same orbital radius — the end-to-end statement that the
  gravitation, the unit scale, and the integrator agree.
- `plugin_tests` gains the far-plane check against the floor's diagonal and
  `the_plugin_spawns_every_catalog_experiment`; `trail_tests` gains six tests over the
  per-experiment length, including that a longer trail costs no more samples.

## 2026-09-05

The changelog starts here, so this entry does double duty. The **`collision/`
layer** and the changes that go with it are today's actual work. Everything after
it is a **baseline**: the state of the tree as the changelog found it, recorded in
one entry because it predates the changelog and is not separable into days from
the git history (it is all one uncommitted body of work on top of `8dc188e`).

### Added

- **Fading motion trails** (`render/trail.rs`): the last `5` units of each moving body's path,
  brightest at the body and invisible at the far end, in the body's own colour — so one orbit can
  be followed out of three in `experiment_2`. `T` toggles them; on by default. Recorded as
  amendment A10 in the original requirements spec.
  - **Length is distance, not time.** Under a variable timestep a fixed sample count would give a
    trail whose length varied with the frame rate and the clock scale. The tail is trimmed to
    *exactly* 5 units by sliding the last point along its own segment, so the trail does not pulse
    by a sample's worth as points arrive — a per-frame artefact, so a test checks it every step
    rather than once at the end.
  - **The fade is interpolated by distance along the path**, not by sample index: samples are
    unevenly spaced, since an accelerating body leaves longer gaps, and an index-based fade would
    make the fast part of a path fade faster than the slow part.
  - **Drawn with immediate-mode gizmos**, not entities — the geometry differs every frame, and an
    entity-per-segment scheme would spawn and despawn hundreds per second. The cost is that
    `draw_trails_system` cannot be tested headlessly, which is why the length and fade live in
    the pure `TrailPath`: 3 properties and 22 unit tests, against a drawing function thin enough
    to read.
  - Trails are **outside the model**, like the grid and the compass: `Trails` is a render-layer
    resource derived from prop positions, never state on `Prop`. Immovable props are skipped, and
    a trail whose body leaves the scene is dropped.
  - Recording continues while hidden, so `T` reads as a view setting rather than something
    destructive. The binding lives in the render layer because it controls a render setting —
    putting it in `input/controls.rs` would mean `input` depending on `render`, inverting the
    direction that already exists.
- **An orientation compass in the top-right corner** (`ui/compass.rs`): the three world axes
  as they look from where you are standing, labelled and colour-matched to the floor grid
  (red X, blue Z, green Y — the axis with no line on the floor). Rotates with the camera
  every frame. Recorded as amendment A7 in
  the original requirements spec.
  - **The depth cue is load-bearing, not decoration.** An axis pointing at your eye and one
    pointing away project to the *same* zero-length arm, so `CompassAxis::toward_viewer`
    carries the sign and the drawing fades a receding arm to `COMPASS_DIM_ALPHA` and draws
    back-to-front. Without it the compass would be ambiguous exactly when you are looking
    along an axis — the moment you most need it.
  - `compass_axes(rotation, radius)` is a **pure projection**, so the geometry is tested
    without a window: `compass_system` needs a real egui context and cannot be ticked
    headlessly, which is why the maths is kept out of it. It takes only a *rotation* — a
    compass shows heading, not position, and a test pins that signature decision.
  - 4 properties and 11 unit tests, including that the three arms stay orthonormal under any
    rotation (a projection that stopped being a rotation would fail there), that the radius
    scales arms without touching depths, and that a non-finite quaternion falls back to
    identity rather than filling the painter with `NaN`.
- **`Q` and `E` move the camera along world Z**, joining `Space`/`Left Ctrl` on world Y as
  the second family of world-axis bindings — they ignore where the camera is looking, which
  is the point: the camera's *local* Z is already `W`/`S`, and a world-axis key whose
  direction depended on the view would be useless for moving along a known axis of the
  scene. `E` is `+Z`, which in Bevy's right-handed Y-up frame is *toward* the default view,
  and is the same Z the grid's blue axis line marks. Recorded as amendment A6 in
  the original requirements spec.
- **Tests for what every movement key does.** The camera's translation had no coverage at
  all — only a smoke test that one tick does not panic — so a key wired to the wrong axis
  would have been obvious in the window and invisible in the suite. Six new tests: each
  key's axis, that `Q`/`E` ignore a rotated view, that opposite keys cancel, that a
  three-key diagonal is normalised rather than faster, and that `Shift` boosts the new keys.
  They also pin something previously untested: the observer clock's motion clamp, since a
  one-second frame moves the camera by `speed × 0.1`, not `speed × 1.0`.
- **A tracing grid on the floor** (`render/grid.rs`). A body moving against a featureless
  plane cannot be traced: you cannot see whether an orbit is drifting, whether a bounce
  returned to where it started, or how far apart two bodies are — which is most of what the
  three-body experiment is for. Minor lines every `5` units, major every `25`, and the X and
  Z axes in their own colours. Recorded as amendment A5 in
  the original requirements spec.
  - **Render-only.** Marked `FloorGridLine`, *not* `MirroredBody`, and absent from
    `Scene.props`: a visual aid must not show up in the overlay's prop readout, must not
    enter the `O(n²)` gravitation and contact loops, and must not perturb the mirror
    invariant the headless test asserts. Two tests exist purely to pin that.
  - **It measures the floor rather than assuming it.** `spawn_floor_grid` reads the plane
    prop's own `half_size` out of the `Scene`, so a floor of any size gets a matching grid
    with no second edit — a grid that disagreed with the floor would be worse than none.
  - `GRID_LIFT_UNITS` (`0.02`) is required, not decorative: lines drawn *at* the surface
    are coplanar with it in the depth buffer and flicker as the camera moves.
- **`experiments/` — a catalog of simulations, chosen at launch.** The app could
  simulate exactly one thing, and its identity was spread across three layers: the scene
  in `core`, the clock scale in `render`'s plugin, the camera pose in `render`'s
  constants. Adding a second meant editing all three and losing the first. Recorded as
  amendment A4 in the original requirements spec.
  - `ExperimentId` / `ExperimentSpec` / `Experiment` — an experiment is a `Scene` plus
    the settings that make it watchable (time scale, camera position and target, seed).
    `ScenePlugin` now takes a `Selection` and inserts the spec as a resource;
    `spawn_scene_entities` frames the camera from it rather than from a constant, and the
    `SimulationClock` is built at *that experiment's* scale.
  - **`experiment_1`** — the existing two-body free fall, now one entry in a catalog.
    `ScenePlugin::default()` runs it, so the old behaviour is the default.
  - **`experiment_2` — three-body ring.** Three Earth masses at 120° on a `12`-unit
    circle in a horizontal plane, each given a velocity **exactly tangential** to the
    circle with a magnitude drawn from `[0.4, 1.3]` times the rigid-ring circular speed
    `√(G·m/(√3·R))` — derived, not guessed, and ≈`138 m/s` at the shipped radius and
    mass. The factors are drawn **independently per body**, which is what breaks the
    symmetry and makes it a three-body problem rather than a rotating rigid triangle.
    Outcomes range from collapse into contact to one body being ejected.
  - `Rng` — a hand-rolled deterministic SplitMix64, so the same seed always produces the
    same scene. Ten lines with a cited algorithm rather than a new dependency; the
    reasoning, and when to switch to `rand` instead, is at the type.
  - Command line: `[<experiment>] [--seed <u64>] [--list] [--help]`, with slugs, numbers,
    and descriptive aliases all accepted. The parse is a pure function returning data, so
    a future in-window picker builds a `Selection` directly and needs no change to it.
    Bad arguments print the error *and* the usage, and exit 2.
- **`time/` — one clock for the experiment, one for the observer, and `TimeStep` as
  the only way to advance physics.** Time was the one quantity with no owner: a raw
  `f32` frame delta was clamped and scaled inside a Bevy system, then passed through
  four signatures that each re-validated it; the camera and the overlay independently
  read the real clock with nothing recording that they were deliberately unscaled; and
  nothing tracked elapsed simulated time, so the documented claim that the shipped
  spheres meet after `2.0e8 s` was untestable. Recorded as amendment A3 in
  the original requirements spec.
  - `TimeStep` — a validated, finite, non-negative interval of simulated seconds.
    `Scene::step`, `integrate_gravity`, and `substep_count` now take one instead of an
    `f32`, and the only constructor from a float returns `Option`. **The type is the
    enforcement**: there is no path by which an unvalidated duration reaches the
    integrator, and Requirement 5.5's `dt >= 0` went from a `debug_assert!` at the
    bottom of the call stack to a property that also holds in release. Substeps come
    from `TimeStep::split`, so they carry the same guarantee.
  - `SimulationClock` — owns the scale, the pause state, the frame-step ceiling, and
    the **total elapsed simulated time**, which the crate could not previously report.
    The end-to-end collision test now asserts the pair meets within the free-fall time
    the scene's own constants predict, which was not checkable before.
  - `ObserverClock` — wall time, read by the camera and the FPS row. The camera was
    already unscaled; this makes it a type-level fact instead of an undocumented one,
    and it is what lets a paused experiment be flown around.
  - The layer imports **no Bevy at all**, not even `bevy::math` — a duration is a
    scalar. The `Resource` impls live in `render/plugin.rs`, as `Scene`'s does.
- **Clock controls**: `P` pause/resume, `.` single-step while paused, `]` / `[` double
  and halve the scale, `\` reset it. In `input/controls.rs`, edge-triggered so a held
  key cannot run the simulation away, and none of the bindings collide with the
  camera's — `P` rather than the conventional `Space`, which is already "move up".
- `SECONDS_PER_MINUTE` / `_HOUR` / `_DAY` / `_JULIAN_YEAR` in `time/constants.rs` —
  the duration unit definitions the overlay's `human_duration` reads. They started as
  function-local `const`s, which the constants audit correctly flagged: definitional or
  not, a named constant belongs in a constants module. The Julian year (365.25 d) is the
  IAU convention the scene's "6.5 years" figure is quoted in, so the overlay and the
  docs cannot drift apart.
- Overlay rows for both clocks (`clock_lines`, `human_duration`): simulated elapsed
  time in SI and in a human unit, the scale, real elapsed time, the frame rate, and
  the pause state. Their ratio is the scale *actually* being achieved, which is not
  the configured one whenever frames are being clamped.
- **A constants module per layer, and a crate-level one for shared values.**
  Eighteen declared constants were scattered across the implementation files that
  happened to use them (`physics_utils` was already compliant), and fifteen more
  tunable values were inline literals with no name at all — camera defaults, the crosshair's appearance, the `SceneConfig`
  defaults, the window size, the starter scene's bounds and colours, the pitch
  clamp's margin, the quaternion-validity tolerance. Nobody could answer "what knobs
  does this project have?" without reading every file. Now:
  - `src/constants.rs` holds what **more than one layer** reads — `METRES_PER_UNIT`,
    the shipped scene's `TWO_BODY_*` configuration and `EARTH_MASS_KG` — plus the
    window values `main.rs` owns, since the shell has no layer directory.
  - `src/<layer>/constants.rs` holds the rest, one module per layer.
    `physics_utils/physics_constants.rs` already was one and keeps its name.
  - Every constant now documents four things: what it is with its unit, **why this
    value** (with the measurement where one exists), **who reads it**, and **what
    changes if you move it**. That last part is the one that was missing everywhere
    and is the reason the layout change was worth making.
- `ROTATION_UNIT_TOLERANCE` — the `1e-4` quaternion-validity tolerance, previously
  an inline literal in `Pose::is_valid` and *separately* an inline literal in
  `Contact::is_valid`. Both now read the one constant, so "is this vector unit
  length" means one thing in the crate.
- **`collision/` — body physics: props are solid.** Every prop now carries a
  closed, outward-wound triangle hull derived from its `Body`, and
  `Scene::step` resolves hull overlap as a perfectly elastic collision. Before
  this, props were gravitational point masses that passed through each other and
  a sphere's radius had no physical meaning — which made the shipped scene's
  most-watched moment the one moment it was not modelling. Recorded as amendment
  A2 in the original requirements spec.
  - `collision/trimesh.rs` — `Triangle` and `TriMesh`. Inside versus outside is
    encoded in the face winding, so `orient_faces_outward` is the single place
    winding is fixed and `is_closed` is the geometric statement of "this body has
    an inside". Hand-listing 80 index triples correctly is not a mechanism.
  - `collision/tessellate.rs` — `collision_mesh(&Body)`: an 80-face icosphere for
    a sphere, 12 triangles for a cuboid, a `1.0`-unit slab hanging *below* the
    floor quad (a zero-thickness plane has no inside; hanging it below means bodies
    rest exactly where the floor is drawn), and a square prism for a rod. `None`
    for a zero-width line, because a curve has no interior to collide with.
  - `collision/contact.rs` — separating-axis narrow phase over the meshes' own
    face normals and edge directions, behind an AABB broad phase. The axes come
    from the geometry, so one code path covers every pair of shapes and a new
    `Body` variant needs a tessellation rather than a new collision case.
  - `collision/resolve.rs` — elastic response via the existing
    `elastic_collision_1d`, then inverse-mass-weighted separation. The layer adds
    no physical formula of its own, matching how `core/integrator.rs` composes
    gravitation out of `physics_utils`.
- `NORMAL_TIE_TOLERANCE` (2%) — the contact normal is the average of every
  separating axis tied with the shallowest, not the shallowest alone. Measured
  during implementation: a single facet normal can be **21° off** the true contact
  direction on an 80-face hull, so two spheres meeting head-on would have left at
  42° to the line of centres, inventing transverse motion and angular momentum out
  of the tessellation. Averaging the 24 axes that tie within 2% recovers the line
  of centres to float precision, and changes nothing on a genuinely flat contact
  where every tied axis is the same direction.
- Substepping in `Scene::step` — `substep_count` splits `dt` so nothing travels
  further than `0.25` units per substep (capped at 64), because contact is tested
  at substep boundaries rather than swept. **The shipped scene does not need it**:
  the pair closes at `0.029` units per 60 Hz frame (`0.17` at the `max_dt` clamp),
  so the count is always `1` and the loop is free. It is insurance for faster
  scenes, where the alternative is contact quietly failing to happen.
- `core/` — GUI-free scene layer. `Scene` owns `bodies` and `bounds`; `Prop` is
  a body plus a velocity vector and a mass; `Scene::step(dt)` is the physics
  seam. `bevy::math` only, no ECS/egui/wgpu.
- `core/integrator.rs` — mutual Newtonian gravitation as the body of the `step`
  seam. Every prop attracts every other; the only place world units are
  converted to SI (`METRES_PER_UNIT`).
- `render/` — model → Bevy ECS boundary: `transform_from` and
  `sync_scene_to_ecs_system`, plus `ScenePlugin` (resources, startup spawn,
  step/sync ordering).
- `input/camera.rs` — `FreeFlyCameraPlugin`: WASD, `Space`/`Left Ctrl`,
  `Left Shift` boost, mouse-look with click-to-capture and `Esc` to release.
- `ui/` — `DebugUiPlugin` (FPS, body count, camera pose) and `CrosshairPlugin`.
  The only layer that imports egui.
- `physics_utils/` — Bevy-free physics math in `f64`: gravitational constant
  (CODATA 2018), universal gravitation, Newton's three laws, momentum/energy
  conservation with collision solvers, and constant-acceleration kinematics.
- Frame-delta clamping in `step_scene_system` — deltas are clamped to
  `SceneConfig.max_dt` (default `0.1 s`) and non-finite or negative deltas
  become `0.0`, so a pause cannot make the integrator explode (Requirement 13).
- `SceneConfig::time_scale` (shipped at `5e6`) — turns the ~6.5-year real fall
  time of the two-Earth-mass pair into ~41 seconds of wall clock. Masses,
  forces, and formulas are unscaled; only simulated time per frame changes.
- Headless test suite under `tst/`, mirroring `src/` one file per source file,
  plus `tst/headless_app.rs` as the single integration test target. No window
  or GPU required.

### Changed

- **Trails are twice as long and three times thicker**: `TRAIL_LENGTH_UNITS` `5.0` → `10.0` (two
  fine grid cells, so the length can be read off the floor) and a new `TRAIL_LINE_WIDTH_PX` of
  `6.0` — three times Bevy's 2-px gizmo default, at which a trail fading to transparent against a
  lit 3D scene was technically present and practically invisible. `TRAIL_MAX_SAMPLES` `256` → `512`
  to stay above the doubled requirement.
  - The width lives on a **dedicated `TrailGizmos` config group** rather than Bevy's default one,
    so it is a trail setting instead of a global for every gizmo the app might later draw.
  - `TrailPlugin` now registers the draw system only when a `GizmoConfigStore` exists, so a
    headless app records trails without pretending it can paint them — covered by a test.
- **The ground plane is now effectively unbounded**: `DEMO_FLOOR_HALF_SIZE_UNITS` `50.0` →
  `2000.0`, a 4000-unit square, `40×` the scene's bounds. Matching the bounds (below) left
  *the same boundary* ending both the world and the floor, so a body ejected from the
  three-body ring left the floor in the same instant and fell into empty space. The edge now
  lies far beyond Bevy's 1000-unit far plane, so it is never rendered and cannot be reached in
  practice. It is **finite geometry you cannot get to the end of**, not a half-space — a true
  infinite plane would need the collision layer to special-case a half-space test, since an
  unbounded surface cannot be a closed convex hull. Recorded as amendment A8.
  - Cost is not the concern the size suggests: the floor's hull is a slab spanning
    `y ∈ [−1, 0]`, so a prop above it is still rejected by the broad phase on the **vertical**
    axis alone, however wide the footprint gets.
  - `tst/core/scene_tests.rs` had its invariant **inverted**: the floor used to be required to
    fit *within* the bounds, and now must **cover** them.
- **The tracing grid now covers the whole floor, at two spacings** (amendment A9). The
  previous step capped it at ±60, which left the newly-unbounded plane reading as
  unfinished — a gridded island in a bare expanse, with nothing to judge motion against
  outside it. Covering it at a single 5-unit spacing was the obvious fix and the wrong one:
  1,602 entities whose distant half is finer than the pixels available to draw it.
  - Three tiers: **axes** spanning the whole plane (the origin has to be findable from the far
    end), **coarse** lines every 100 units to the edge, and **fine** lines every 5 out to
    `GRID_CORE_EXTENT_UNITS` (60) where the bodies are. ~130 entities, with the resolution
    where it can be read.
  - `GridLayout` replaces `floor_grid_segments`' positional arguments — two extents and two
    spacings are easy to transpose — and `GridLayout::for_floor` derives them from a floor.
  - Two subtle-on-screen invariants are now tested: the coarse spacing is a whole multiple of
    the fine one, so the tiers line up rather than interleaving into a beat pattern; and **no
    offset is ever drawn twice**, since two coincident lines read as one thicker line and make
    the spacing look irregular.
  - A floor *smaller* than the core extent still gets fine lines that match it exactly, since a
    grid overhanging its own surface looks broken.
- **`Q` and `E` moved from world Z to world Y**: `E` up, `Q` down, as **aliases** for
  `Space`/`Left Ctrl` rather than a third axis, because muscle memory differs by tool. Both
  sum into one `delta` before normalisation, so holding `E` and `Space` is one unit of rise
  and not two — a test pins that, since applying them separately is the classic aliasing bug.
  **Consequence: no key moves along world Z any more**, which
  `no_key_moves_along_world_z_any_more` records as a decision rather than a regression; the
  compass is now the only thing showing where Z has gone.
- **The ground plane spans the world**: `DEMO_FLOOR_HALF_SIZE_UNITS` `25.0` → `50.0`, then
  `2000.0` as above; at this step it became exactly the XZ half-extent of `Scene::bounds`. This closes a gap that had been recorded in
  `core/README.md` since before the floor was solid — a prop could be inside the bounds and
  off the edge of the floor, which since the `collision` layer means falling straight past
  it rather than landing.
- **The camera moves four times faster**: `CAMERA_MOVE_SPEED_UNITS_S` `5.0` → `20.0`, which
  crosses the 100-unit world in five seconds instead of twenty. Task 8.1's `5.0` was chosen
  against a scene half this wide. A `3×`-boosted 60 Hz frame still moves less than the
  smallest body's diameter, so the camera cannot skip past what you are flying toward —
  both bounds are now asserted rather than assumed.
- `tst/input/camera_tests.rs` stopped pinning the controller defaults as literals and pins
  the *constants* instead: two places to edit for one decision, and the test was the one
  that would be forgotten.
- `ScenePlugin` is no longer a unit struct: it carries the `Selection` to run.
  `ScenePlugin::default()` is the old behaviour, and every existing call site became
  that.
- The startup camera pose left `render/constants.rs` for `experiments/constants.rs`,
  because framing is per experiment — a `12`-unit ring wants a different viewpoint than a
  `30`-unit pair — and it reaches `spawn_scene_entities` through the `ExperimentSpec`
  resource.
- The overlay gained an `exp` row naming the running experiment and its seed. Without the
  seed on screen an interesting three-body run would be unrepeatable — you would have
  watched something you could never show anyone.
- `SceneConfig` is **retired**: `max_dt` became `SimulationClock::max_real_step_seconds()`
  and `time_scale` became `scale()`. Requirement 13.5's `0.1 s` default is unchanged and
  now lives in `time/constants.rs`; `clamp_dt` remains as Requirement 13's named helper
  and delegates to `time::clamp_real_step`, so there is one implementation with two
  names rather than two implementations.
- `step_scene_system` no longer computes a timestep — a new `advance_clocks_system`
  runs first in the `Update` chain, feeds both clocks from the one Bevy timer read in
  the crate, and the stepping system reads `clock.last_step()`.
- The camera reads `ObserverClock::motion_step_seconds()` rather than
  `Time::delta_seconds()`, so a stalled frame no longer teleports the viewer: the raw
  delta is kept for FPS and the capped one for motion.
- `ui/overlay.rs`'s private `fps_from_dt` is gone; `ObserverClock::fps` replaced it,
  and its three tests moved to `tst/time/observer_tests.rs`.
- The tests that asserted `Scene::step` and `integrate_gravity` panic on a negative or
  `NaN` `dt` now assert that `TimeStep::from_seconds` **rejects** those values instead.
  That is strictly stronger: it holds in release too, where a `debug_assert!` does not.
- Layer `README.md` file tables list `constants.rs`, and `collision/README.md`'s
  tuning table gained the two rows it was missing (`NORMAL_TIE_TOLERANCE`,
  `MIN_AXIS_LENGTH`) — the SOP requires the table to match the code.
- `Scene::step(dt)` now runs *integrate gravitation → resolve contacts* per
  substep instead of gravitation alone, and returns immediately at `dt == 0.0`.
  The early return is required rather than an optimisation: contact resolution is
  a function of position, so it would separate an already-overlapping pair even at
  a zero-length step and break Postcondition 5.2 (a zero step leaves `props`
  byte-identical).
- The shipped two-body scene no longer ends with the spheres swinging through each
  other — they bounce and **oscillate indefinitely**, since equal masses meeting
  head-on exchange velocities and nothing dissipates energy. Successive bounces
  are not identical: the integrator is first-order and the positional separation
  that removes penetration is not momentum conserving, so the amplitude drifts.
- A `Body`'s radius, half-extents, and half-size are now physical rather than
  visual: they are where the body stops.
- The running scene is now a physics simulation rather than a rendering demo:
  two Earth-mass spheres 30 world units (`3e10 m`, ~0.2 AU) apart, released at
  rest, falling radially under mutual gravity above a ground plane.
- Scene lengths are world units at `1 unit = 1e9 m` rather than metres —
  storing metres directly puts `f32` spacing at ~2 km by `3e10 m`.

### Removed

- The startup cube, sphere, and line props. The startup scene is exactly one
  `Body::Plane` at the origin, so everything visible is something simulation
  code put there — see amendment A1 in
  the original requirements spec. `Body` still has
  all four variants and `transform_from` still renders all four.

### Fixed

- **Trails recorded almost nothing.** The sample-spacing test measured the distance from the
  path's *head* — and then overwrote that head whenever the move was too small to commit a
  sample, destroying the very anchor the comparison needed. A body moving less than
  `TRAIL_MIN_SAMPLE_SPACING_UNITS` (`0.05`) per frame therefore committed **no samples at all**,
  however far it travelled, and its trail stayed one point long. Both shipped scenes sit in
  exactly that range — the two-body pair moves ~`0.029` units per 60 Hz frame and the three-body
  ring ~`0.046` — so trails appeared only sporadically, when frame times happened to spike.
  - The head (`points[0]`, always exactly where the body is) and the newest committed sample
    (`points[1]`) are now distinct, and the spacing is measured against the latter.
  - The old tests all stepped *further* than the spacing each time, which is why none of them
    explored the region the bug lived in. Three tests and a property now do, including one that
    pins the two shipped per-frame rates directly.
- A **pre-existing flaky property test**, `total_momentum_is_the_sum_of_body_momenta`,
  which the suite failed on during this change (in release, where proptest happened to draw
  the case). It compared the *compensated* `total_momentum` against a naive `iter().sum()`
  reference at a fixed `1e-12` relative tolerance — so when the terms nearly cancel the
  reference is the inaccurate side, and the test was asserting that the accurate sum
  matches the inferior one. The counterexample was four momenta of order `1e11` summing to
  `1.7e7`: a four-order cancellation, `1.8e-12` relative error. The tolerance is now scaled
  by the sum's condition number `Σ|terms| / |Σ terms|`, which is exactly the factor by
  which cancellation amplifies the reference's rounding error. The seed proptest found is
  committed with the other regressions.

### Docs

- Amendment A10 in the original requirements spec — the trail contract, the
  distance-not-time rule, and its limitations: no headless test of the drawing, the trimmed tail
  sits on a chord rather than a curve, and a body that is *moved* rather than flown (de-penetration
  after a contact) draws a straight jump, because the trail records positions and not the physics
  between them.
- Amendment A9 in the original requirements spec — the three tiers and what each
  is for, why a uniform fine grid was rejected with the entity count, and the limitations: the
  tier boundary is visible as a change of density, the grid still does not follow the camera,
  and "major" now names the coarse reference at two different scales.
- Amendment A8 in the original requirements spec — the unbounded floor and the
  `Q`/`E` rebinding, what each supersedes in A5 and A6, why "unbounded" is a size rather than
  a half-space, and the recorded consequence that world Z has no binding left.
- Amendment A7 in the original requirements spec — the compass contract, why it
  is a second egui drawer rather than a change to Requirement 11's single window, and its
  limitations: no arrowheads (a few pixels at a 28-px radius would muddy the tip), no
  negative-axis stubs, and no headless test of the drawing itself.
- Amendment A5 in the original requirements spec — the floor's new extent, the
  grid, the rule that render-only entities may exist alongside mirrored ones, and the
  grid's limitations: spawned once at startup (a floor that changed mid-run would leave a
  stale grid), no hide toggle, and the lines are cuboids so their thickness shows at a very
  oblique angle.
- Amendment A4 in the original requirements spec — the catalog contract, what
  it leaves untouched (Requirements 3.1/3.5, `Scene::demo`'s container, A3's clocks), and
  its limitations: the experiment is fixed at launch (in-window switching would need the
  scene, the clock, and the mirrored entities rebuilt mid-run), `experiment_2` is
  reproducible across runs at the same time scale but **not** across scales, and the
  generator is not a statistical instrument.
- `src/experiments/README.md` and `tst/experiments/README.md`, including the ring-speed
  derivation and the rule that a new experiment's scene is built in the experiments layer
  — only the two-body one lives in `core`, and only for history.
- Amendment A3 in the original requirements spec — the `TimeStep`
  contract, the two clocks, the retirement of `SceneConfig`, the new key bindings, and
  what is explicitly **deferred**: reverse time. A signed step needs a reversible
  integrator (semi-implicit Euler is not its own inverse), a contact policy for
  reverse (positional de-penetration is a projection and cannot be inverted), and
  amendments to 5.5 and 13.3 — so a signed type without those would look like it
  worked and silently diverge. The three prerequisites are documented at the type.
- `src/time/README.md` and `tst/time/README.md`, including why test durations are
  binary-exact fractions (`1.0 / 64.0`, not `0.01`) wherever an assertion is exact.
- `.claude/steering/constants.md` gained an **algorithm and format constants** section:
  SplitMix64's increment and `f32`'s 24-bit mantissa are not knobs, so they stay next to
  the code that implements them — but only if private, adjacent, documented as not
  tunable, and carrying an `// ALGORITHM CONSTANT:` marker, which is what keeps the audit
  mechanical instead of a matter of reviewer judgement. The audit also now exempts
  `: [Self;` catalogs like `ExperimentId::ALL`.
- `.claude/steering/constants.md` gained an **associated constants** section: a
  distinguished value of a type (`TimeStep::ZERO`, mirroring `Vec3::ZERO`) is API and
  stays on the type, because the test is "could someone configure this?" rather than
  "is it inside an `impl`". The audit grep excludes them by matching `: Self =`.
- New SOP `.claude/steering/constants.md` — where a constant goes (two-step rule),
  what must move versus what is part of a formula's meaning (`0.5` in `½·m·v²`
  stays; naming it `HALF` is explicitly not wanted), the unit-in-the-name
  convention, the four required documentation parts, when a value must be pinned by
  a test, and a one-line grep audit.
- `.claude/steering/source-directory-docs.md` gained a sixth required README section
  ("what its constants are"), a checklist item for `constants.rs`, and a
  related-SOPs cross-link.
- Amendment A2 in the original requirements spec — the solidity and
  elastic-contact contract, what it leaves untouched (`Body`'s four variants,
  `Prop`'s four fields, every Requirement 5 postcondition), and the limitations it
  ships with: no rotation, no friction, no resting contact, no swept detection.
- `src/collision/README.md` and `tst/collision/README.md`, plus the tuning-constant
  table with the reason for each value and the measured cost of changing it.
- Root `README.md`: scene description, unit convention and why the clock is
  scaled, controls, test layout, and the full project tree.
- A `README.md` in every source directory under `src/` and `tst/`, per
  [`.claude/steering/source-directory-docs.md`](.claude/steering/source-directory-docs.md).
