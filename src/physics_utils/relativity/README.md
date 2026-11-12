# `src/physics_utils/relativity/` — relativity

> **Purpose:** The formulas of relativity. Special relativity in full: `c`, `β`, `γ`, time dilation,
> length contraction, the Lorentz transformation and the spacetime interval, velocity addition on
> one axis and in 3-D, rapidity, Doppler shift, momentum, force, energy, `E² = (pc)² + (mc²)²`,
> collisions, and four-vectors. Plus the closed-form results of general relativity around one mass
> (clock rates, redshift, Mercury's precession, light bending).
> **Edit here to:** add or fix a relativistic formula, or the speed of light.
> **Not here:** Newtonian formulas and `G` are in [`physics_utils/`](../README.md). The black-hole
> experiment that uses `gravity.rs` is [`experiments/black_hole.rs`](../../experiments/README.md).

Everything in special relativity follows from two postulates: the laws of physics are the same in
every inertial frame, and `c` is the same in every inertial frame. The files here build up from
those in order: `β` and `γ`, then what they do to time, length and velocity, then to momentum,
force and energy, then the transformation of events that all of those are special cases of.
`gravity.rs` is the one file about general relativity.

This is the first sub-directory in `physics_utils/`, and it earns one because it is the only topic with
its own constant (`c`), its own domain rule (`|v| < c`), and its own error convention.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the sub-layer, states the sign convention once, and records what is deliberately absent. |
| `constants.rs` | `SPEED_OF_LIGHT_M_S` — `299_792_458 m/s`, **exact** by definition of the metre. |
| `lorentz.rs` | `beta_from_velocity`, `lorentz_factor` (γ), `lorentz_factor_minus_one`, `is_subluminal`. Everything else is built from these. |
| `time_dilation.rs` | `dilated_time` (τ→t), `proper_time` (t→τ), `clock_lag_seconds`. |
| `length_contraction.rs` | `contracted_length` (L₀→L), `proper_length` (L→L₀), `length_contraction_deficit`. |
| `velocity.rs` | `transform_velocity` (remove a frame's motion), `compose_velocities` (add it). |
| `momentum.rs` | `relativistic_momentum` (`p = γmv`), `velocity_from_momentum`, `momentum_excess`. |
| `dynamics.rs` | `longitudinal_force` / `transverse_force` and their inverses `longitudinal_acceleration` / `transverse_acceleration`. |
| `energy.rs` | `rest_energy`, `relativistic_kinetic_energy`, `total_energy`, `work_done`, `velocity_after_work`. |
| `spacetime.rs` | Events and the interval: `transform_event` and `inverse_transform_event` (the Lorentz transformation), `interval_squared_m2`, `Separation` and `classify_separation` (timelike, lightlike, spacelike), `proper_time_between`, `proper_distance_between`, `simultaneity_offset_s`. |
| `rapidity.rs` | `rapidity` (`atanh β`, the speed that adds) and `velocity_from_rapidity`. Motion under constant proper acceleration: `velocity_under_proper_acceleration`, `distance_under_proper_acceleration`, `proper_time_under_proper_acceleration`, `coordinate_time_under_proper_acceleration`. |
| `doppler.rs` | `doppler_factor`, `observed_frequency`, `redshift`, `velocity_from_redshift` along the line of sight; `transverse_doppler_factor`; `doppler_factor_at_angle`. |
| `energy_momentum.rs` | `E² = (pc)² + (mc²)²` with no velocity: `energy_from_momentum`, `momentum_from_energy`, `invariant_mass`, `velocity_from_energy_and_momentum`, `photon_momentum`. Mass and energy: `mass_equivalent`, `energy_released`. The one file that accepts a mass of zero (light). |
| `collisions.rs` | `total_relativistic_momentum`, `total_relativistic_energy`, `centre_of_momentum_velocity`, `system_invariant_mass`, and the solvers `elastic_collision_1d` and `perfectly_inelastic_collision_1d` (which returns the merged **mass** as well as the velocity). |
| `velocity_3d.rs` | Any direction: `transform_velocity_3d`, `compose_velocities_3d`, `speed`, `wigner_rotation_angle`, `aberration_angle`. |
| `four_vector.rs` | `FourVector` with `minkowski_product`, `invariant`, `plus`, and `boost` (the `4×4` Lorentz transformation in any direction); `event_four_vector`, `four_momentum`. |
| `gravity.rs` | **General relativity**, closed forms only: `schwarzschild_radius`, `photon_sphere_radius`, `innermost_stable_circular_orbit_radius`, `schwarzschild_acceleration` (the exact equation of motion near a black hole), `flamm_paraboloid_height` (the shape of the equatorial plane as curved space), `light_velocity` (light's velocity near the hole, for a given direction), `gravitational_time_dilation_factor`, `gravitational_rate_deficit`, `gravitational_redshift`, `perihelion_precession_per_orbit`, `light_deflection_angle`, `orbiting_clock_rate_offset`. |
| `kerr.rs` | **Spinning black holes** (the Kerr solution), in gravitational radii `r_g = GM/c²`: `horizon_radius`, `ergosphere_radius`, `isco_radius` and `photon_orbit_radius` (prograde and retrograde), `frame_dragging_angular_velocity`, `horizon_angular_velocity`, `circular_orbit` (`Ω`, `E`, `L`), `radiative_efficiency`, `disc_flux` (Page–Thorne thin disc). `KerrBlackHole` for SI: Eddington luminosity, accretion rate, disc flux and temperature. Motion in Kerr–Schild coordinates: `kerr_schild_acceleration` (matter and light), `kerr_schild_light_velocity`, `kerr_schild_norm`, `kerr_schild_metric`, `kerr_schild_radius`, `equatorial_point` and its inverse `equatorial_coordinates`. |

`momentum.rs`, `dynamics.rs` and `collisions.rs` mirror the parent layer's own split:
`conservation_laws.rs` holds momentum and the collision solvers and `newtons_laws.rs` holds force.

### How the files depend on each other

```text
constants ─ lorentz ─┬─ time_dilation, length_contraction, velocity
                     ├─ momentum ─ dynamics ─ energy
                     ├─ spacetime        (the transformation the three above are cases of)
                     ├─ rapidity, doppler
                     ├─ energy_momentum  (no γ at all: only c)
                     ├─ collisions       (uses velocity)
                     └─ velocity_3d, four_vector
constants + G ─ gravity, kerr           (nothing else here uses G)
```

The only constant is still `SPEED_OF_LIGHT_M_S`. `gravity.rs` takes `G` from the parent layer's
`physics_constants.rs`.

## Conventions specific to this directory

- **`S` is our frame; `S'` moves at `frame_velocity_m_s` along `+x` as measured in `S`.** Stated once
  in `mod.rs` and never re-derived. Arguments are named after the convention rather than after the
  letters, because `u`, `u'` and `v` are precisely the three symbols that get swapped by accident.
- **`Option`, not a benign fallback.** The rest of `physics_utils` uses `debug_assert!` plus a
  documented benign value; this sub-layer returns `None` for `|v| ≥ c`. That is not a style
  disagreement — it is the same reasoning as `speed_from_displacement` past a projectile's apex: a
  question with no answer. And the candidate fallbacks are all worse than nothing. `1.0` for γ claims
  there is *no relativistic effect*, the most misleading answer available, and `INFINITY` turns into
  `NaN` one multiplication later, at which point the caller has lost the ability to tell a bad input
  from a bad computation.
- **`|v| = c` exactly is rejected, not just `> c`.** A massive object never reaches `c`, and light has
  no rest frame, so there is no interval for γ to scale.
- **A *frame* must be strictly subluminal; an *object* may sit exactly at `±c`.** This asymmetry in
  `transform_velocity` is physics, not convenience — and it is what makes the invariance of `c`
  expressible as a test rather than an assertion.
- **Every "difference" quantity has its own function.** `clock_lag_seconds` and
  `length_contraction_deficit` exist because subtracting two nearly-equal numbers is catastrophic at
  every speed this crate simulates. See below. `redshift`, `gravitational_rate_deficit`,
  `gravitational_redshift`, `interval_squared_m2` and `distance_under_proper_acceleration` follow the
  same rule, and each has a test showing the naive form failing.
- **A result that should be below `c` is below `c`.** Where `f64` would round an inverse to exactly
  `c` (`velocity_from_rapidity`, `velocity_from_redshift`, `velocity_under_proper_acceleration`, like
  `velocity_from_momentum` before them), the result is clamped to the largest speed below `c`, so the
  next call does not reject it.
- **Mass is positive, except in `energy_momentum.rs`.** `γmv` is the wrong formula for light, so the
  velocity-based files reject `m = 0`. `E² = (pc)² + (mc²)²` is the right formula for light, so that
  file accepts it.
- **Doppler takes a recession velocity: positive means moving apart**, which lowers the frequency and
  gives a positive redshift.
- **A 3-D velocity is a `[f64; 3]`.** No vector type is imported, as in the parent layer.
  `FourVector` is the one struct, and its time component is pre-scaled by `c` so all four components
  share a unit.
- **`gravity.rs` says which of its formulas are exact and which are first order** in `r_s/r`. Radii
  there are distances from the centre of the mass.
- **The newer files are not re-exported from the crate root.** Call them by path:
  `physics_sandbox::physics_utils::relativity::doppler::redshift`. The original eight files are
  flattened in `lib.rs`; the rest were added without touching anything outside this directory.

## Why there are three functions where two would do

`lorentz_factor_minus_one`, `clock_lag_seconds`, and `length_contraction_deficit` are all the same
argument: **every relativistic correction is proportional to γ−1, and γ−1 is tiny here.**

| Motion | `v` | `γ − 1` |
| --- | --- | --- |
| A fast walk | `2 m/s` | `2.2e-17` |
| An airliner | `250 m/s` | `3.5e-13` |
| A GPS satellite | `3,874 m/s` | `8.3e-11` |
| The ISS | `7,660 m/s` | `3.3e-10` |
| Earth around the Sun | `29,780 m/s` | `4.9e-9` |
| Mercury around the Sun | `47,360 m/s` | `1.2e-8` |

Computing `lorentz_factor(v) - 1.0` at Earth's orbital speed throws away **half the significant
digits**, and at `1 m/s` it returns **exactly zero** — an answer that reads as "relativity does not
apply" rather than "this is below `f64`'s resolution". The stable form multiplies through by the
conjugate, `γ−1 = β²/(s(1+s))` with `s = √(1−β²)`, and never subtracts nearly-equal quantities.
`the_naive_form_underflows_where_the_stable_one_does_not` pins both halves of that claim.

## The one test that matters most

`contraction_and_dilation_scale_by_gamma_in_opposite_directions`. Time dilation *multiplies* by γ and
length contraction *divides* by it, but both are described as "the moving thing is diminished" — so
every test about magnitudes passes even if the two are wired to the same helper. That test asserts the
*directions*: a unit interval grows, a unit rod shrinks, and their product is exactly one.

## `F = ma` does not survive, and that is the interesting part

Newton's second law survives as `F = dp/dt`. What does not survive is `dp/dt = ma`, because `p = γmv`
and `γ` depends on `v`. Differentiating gives **two different answers depending on direction**:

| `β` | `F∥/(ma)` = `γ³` | `F⊥/(ma)` = `γ` | ratio `γ²` |
| --- | --- | --- | --- |
| `0` | `1.000` | `1.000` | `1.00` |
| `0.5` | `1.540` | `1.155` | `1.33` |
| `0.9` | `12.07` | `2.294` | `5.26` |
| `0.99` | `356.2` | `7.089` | `50.3` |

So **force and acceleration are not parallel**: push a fast body at `45°` to its motion and it
accelerates in some other direction. This is why there is no `relativistic_mass` function here — the two
factors were once called "longitudinal mass" and "transverse mass", and the existence of *two* of them is
the proof that the idea does not work. No single number multiplying `a` reproduces `dp/dt`.

`the_longitudinal_factor_is_the_derivative_of_momentum` checks the `γ³` against a **numerical
derivative** of `relativistic_momentum` rather than against the formula, so the exponent is verified to
be `dp/dt` and not merely restated.

The `γ⁻³` in `longitudinal_acceleration` is also where the speed limit lives: under constant force the
acceleration dies as `(1 − β²)^{3/2}`, so nothing forbids `v = c` — it just takes forever. An LHC proton
at `6.8 TeV` has `γ³ = 3.8e11`, which is why a collider's last few TeV cost so much more than its first.

## Energy is the law that *survives*

`dynamics.rs` is a story about a Newtonian law breaking. `energy.rs` is the opposite: **`W = ΔKE` holds
unchanged.** Only the expression for `KE` differs.

```text
E  = γmc²            total
E₀ = mc²             rest — present even at rest, which is new
KE = E − E₀ = (γ−1)mc²
W  = ΔKE             unchanged in form
```

Two consequences worth knowing. **A transverse force does no work** — it changes direction, not speed, so
`γ` is unchanged; only the longitudinal component of a force ever changes a body's energy, which is why a
synchrotron's bending magnets cost nothing in beam energy while its cavities cost everything. And
**reaching `c` costs infinite energy**: `KE/mc²` is `0.155` at `β = 0.5`, `1.00` at `β = 0.866`, `21.4` at
`β = 0.999`, with no upper bound. That is the third time the speed limit appears in this sub-layer, after
`γ⁻³` in `dynamics` and the `√(1+r²)` in `momentum`.

`mc²` is **not** a formula for kinetic energy, and nothing in `energy.rs` releases it: a single body's
mass does not change. What happens when the mass of a *system* changes (a reaction, or two bodies
sticking together) is in `energy_momentum.rs` (`energy_released`) and `collisions.rs`.

### `(γ−1)mc²` is where the stable `γ−1` matters most

`c² = 8.99e16` multiplies whatever error `γ−1` carries, so the naive form is not merely imprecise — it is
wrong by tens of percent. For a `1 kg` body, against the `½mv²` it must reduce to:

| `v` | `½mv²` | stable | naive `γ − 1.0` |
| --- | --- | --- | --- |
| `1 m/s` | `0.5 J` | `0.5 J` | **`0 J`** |
| `10 m/s` | `50 J` | `50 J` | **`59.87 J`** — `20%` high |
| `1 km/s` | `5.0e5 J` | `5.0e5 J` | `5.0001e5 J` |

## One place where `f64` runs out before the physics does

`velocity_from_momentum` inverts `p = γmv`, and in exact arithmetic the result is always below `c` for
any finite momentum. In `f64` it is not: above `γβ ≈ 9.5e7` the term `1 + 1/r²` rounds to exactly `1.0`
and the bare formula returns **exactly `c`** — a superluminal answer for a massive body, and one that
`lorentz_factor` then rejects, so a reasonable momentum would poison the next call.

The result is therefore clamped to the largest representable speed below `c`. The promise is kept, but be
clear what it costs: past that boundary the returned velocity carries no information about how large the
momentum was. If you need to distinguish ultrarelativistic momenta, stay in `r = p/(mc)` and do not
round-trip through velocity.

## The transformation everything else is a case of

`spacetime.rs` has the Lorentz transformation of an event, `t' = γ(t − vx/c²)`, `x' = γ(x − vt)`.
Time dilation is that with `Δx = 0`, length contraction is it with `Δt = 0`, and velocity addition
is `x'/t'`. The tests derive each of the three from `transform_event` and compare with the file that
implements it separately.

What the transformation leaves alone is the interval `s² = (cΔt)² − Δx²`, and its sign sorts pairs
of events:

| `s²` | Kind | Every frame agrees on |
| --- | --- | --- |
| `> 0` | timelike | which came first; one can cause the other |
| `= 0` | lightlike | that light just connects them |
| `< 0` | spacelike | nothing about their order; neither can cause the other |

`simultaneity_offset_s` is the effect with no Newtonian counterpart: two flashes a light-second apart
and simultaneous on the platform are `0.75 s` apart on a train passing at `0.6c`.

## Collisions make mass

`collisions.rs` conserves `Σγmv` and `Σγmc²`. Total energy is conserved in **every** collision, so a
perfectly inelastic one does not lose energy: two `1 kg` lumps meeting at `±0.6c` leave one `2.5 kg`
lump. Both solvers work in the centre-of-momentum frame, where an elastic collision reverses each
velocity and an inelastic one leaves the merged body at rest.

The test that justifies the file is `the_newtonian_solver_gives_a_different_answer_in_a_different_frame`:
the parent layer's solver disagrees with itself across frames by more than `0.01c`, and this one does
not.

## Two boosts are a boost and a rotation

On one axis velocities compose in either order. In 3-D, `u ⊕ v` and `v ⊕ u` have the same speed and
**different directions**. The angle between them is the Wigner rotation (`wigner_rotation_angle`):
`12.7°` for two perpendicular boosts at `0.6c`, `4.9e-9 rad` at Earth's orbital speed. This is why the
one-axis functions must not be applied per component, and why `FourVector::boost` exists.

## General relativity around one mass

`gravity.rs` holds what the Schwarzschild solution gives in closed form, and one equation of motion.
Each closed form is tested against a measurement:

| Function | Check |
| --- | --- |
| `perihelion_precession_per_orbit` | Mercury: `42.98″` per century |
| `light_deflection_angle` | Starlight at the Sun's edge: `1.75″`, twice the Newtonian value |
| `gravitational_redshift` | Pound–Rebka: `2.46e-15` over `22.5 m`; the Sun's surface: `2.12e-6` |
| `orbiting_clock_rate_offset` | GPS: `+38.5 µs/day`; the ISS: `−24.7 µs/day` |

The GPS figure is where the two theories meet. Height makes the satellite clock fast by
`45.7 µs/day`; speed makes it slow by the `7.2 µs/day` that `time_dilation.rs` computes. A test adds
the two from separate code and compares with the combined formula.

### Spinning holes: `kerr.rs`

The Kerr solution is the exact spacetime of a spinning, uncharged black hole. Spin shrinks the
horizon (`2 r_g` to `1.44 r_g` at `a = 0.9`), lets matter orbiting with the spin circle far closer
in (the prograde ISCO falls from `6 r_g` to `2.32 r_g`), and so raises the fraction of rest energy an
accretion disc radiates from `5.7%` to `15.6%` (`42%` at the limit). Inside the ergosphere
(`2 r_g` at the equator) space is dragged round faster than anything can move against it.

`disc_flux` is the Page–Thorne flux of a thin disc, from conservation of energy and angular momentum
alone: zero at the ISCO, peaking just outside, `r⁻³` far out. Its test that matters most is the
energy budget: the flux over both faces, weighted by each ring's orbital energy, adds up to
`1 − E_ISCO`, to half a percent.

`kerr_schild_acceleration` is the geodesic equation in Kerr–Schild coordinates, with the Christoffel
symbols computed analytically from the metric's `η + f k k` form (a slower numerical version in the
tests agrees with it). Its tests: a prograde circular orbit with the Bardeen–Press–Teukolsky rate is
exactly centripetal and stays circular when integrated; energy and angular momentum about the spin
axis are conserved; an orbit holds just outside the ISCO and plunges just inside; a body with no
angular momentum co-rotates at the frame-dragging rate; and light passing with the spin escapes at
`4 r_g` where light passing against it is captured.

### The shape of space: Flamm's paraboloid

`flamm_paraboloid_height` is `z = 2√(r_s(r − r_s))`. In a black hole's equatorial plane the distance
measured outward between two circles exceeds the difference of their radii, by `1/√(1 − r_s/r)`, so
the plane cannot be drawn flat. Lifting each circle to height `z(r)` makes distance along the sheet
equal the true distance; the test checks exactly that, by integrating both. The sheet meets the
horizon vertically and never flattens. Nothing draws it at the moment; `experiment_6` drew it as a
grid for a while, and the function and its tests stay as part of the library.

### The equation of motion

`schwarzschild_acceleration` is the exact geodesic equation for a small body near a non-rotating
black hole, in the hole's Schwarzschild coordinates with the distant observer's clock as time. It is
the one function here the simulation steps with: `experiment_6` runs under
`GravityModel::Schwarzschild`, and the integrator calls it for every black hole's pull. Its tests
integrate it with RK4 so the formula, not the integrator, is what they measure:

- the geodesic's energy and angular momentum, which come from the metric rather than the equation,
  stay constant to `1e-10`;
- a weak-field orbit precesses by `perihelion_precession_per_orbit` to `1%`;
- an orbit released at `9 r_s` reaches the `4.5 r_s` periapsis worked by hand;
- nudged circular orbits survive outside the ISCO and plunge inside it;
- light circles at `1.5 r_s` at `c/√3`;
- a body falling from far away peaks at `2c/(3√3) = 0.385c` and never crosses the horizon.

The same equation moves light. `light_velocity` gives a ray its starting velocity: in these
coordinates light near the hole moves slower than `c`, `A·c` radially and `√A·c` across, and the
returned velocity satisfies the null condition exactly. The light tests fire rays and check that
they stay null, that capture switches between impact parameters `2.55` and `2.65 r_s` (the critical
value is `3√3/2 = 2.598`), that a ray at `2.65 r_s` comes back the way it came, and that a distant
ray bends by `light_deflection_angle` to `1%`.

## Tests

[`tst/physics_utils/relativity/`](../../../tst/physics_utils/relativity/README.md), one file per
source file, each `#[path]`-included into its subject. Proptest seeds go to
`tst/physics_utils/proptest-regressions/relativity_<file>_tests.txt`.

## Adding a file here

- Start from `SPEED_OF_LIGHT_M_S` and `lorentz.rs`; do not recompute `γ`.
- Return `Option`, with `None` for `|v| ≥ c`, a non-finite input, or a result that overflowed.
- Name the unit in each parameter (`_m_s`, `_kg`, `_j`, `_s`, `_m`, `_rad`).
- If the result is a small difference of two large numbers, give it a stable form and a test that
  shows the naive form failing.
- Add `tst/physics_utils/relativity/<file>_tests.rs` with a pinned `REGRESSION_SEEDS`, a row in the
  table above, a row in that directory's README, and a line in `mod.rs`.
- A new constant goes in `constants.rs` with its why / used-by / if-changed docs.

## What this cannot do

- **General relativity around one mass, spinning or not, for test bodies only.** The equations of
  motion treat the orbiting body as massless and the hole as fixed in its own frame; two black holes
  meeting, charged masses, gravitational waves, and cosmology are all absent. The accretion disc is
  the thin-disc model only: no thick or hot flows, no magnetic fields, no radiation transfer. Only black holes pull relativistically, and only in a scene that asks for it, so
  Mercury's orbit in `experiment_3` does not precess; `perihelion_precession_per_orbit` only says by
  how much it should.
- **Three of the gravity formulas are first order** in `r_s/r` (precession, light bending, the
  orbiting clock). They are right to parts per million in the solar system and wrong near a black
  hole.
- **Collisions are on one axis, between two bodies.** There is no 2-D or 3-D scattering, no decay
  into several bodies, and no collision involving light (Compton scattering). `FourVector` can
  express all of those; nothing solves them.
- **Boosts do not carry orientation.** `FourVector::boost` is a pure boost and
  `wigner_rotation_angle` reports the angle only. There is no rotation matrix, no composition of
  boosts into a single Lorentz transformation, and no Thomas precession rate for an orbit.
- **Doppler is for flat spacetime.** `velocity_from_redshift` does not apply to cosmological
  redshifts, and there is no combined gravitational-plus-motion shift.
- **No electromagnetism.** Fields do not transform here.
- **No relativistic renderer.** Length contraction here is what a frame *measures*, not what a camera
  would photograph. A fast cube looks rotated (Penrose–Terrell), not squashed.
- **Only `experiment_6` uses it.** At solar-system speeds γ−1 is `5e-9`, so `experiment_3` is
  Newtonian to eight decimal places and is right to ignore this. `experiment_6` is a spinning Sgr A*:
  it is sized, steps anything placed near it, draws its accretion disc, and bends light round it, all
  with `kerr.rs`.
