# `src/physics_utils/relativity/` — special relativity

> **Purpose:** Special relativity between two inertial frames: `c`, `β`, `γ`, time dilation,
> length contraction, velocity addition, momentum, force, and energy.
> **Edit here to:** add or fix a relativistic formula, or the speed of light.
> **Not here:** Newtonian formulas and `G` are in [`physics_utils/`](../README.md).

The kinematics of two frames in uniform relative motion. Everything here follows from two postulates —
the laws of physics are the same in every inertial frame, and `c` is the same in every inertial frame —
and none of it needs a force, a mass, or a `Scene`. It is geometry.

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

The last two mirror the parent layer's own split: `conservation_laws.rs` holds momentum and
`newtons_laws.rs` holds force, so `momentum.rs` and `dynamics.rs` sit in the same relationship here.

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
  every speed this crate simulates. See below.

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

`mc²` is **not** a formula for kinetic energy, and nothing here releases it. Mass is conserved throughout;
`mc²` appears because it is the natural energy unit for a body of mass `m`.

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

## What this cannot do

- **No general relativity.** No gravitational time dilation, no curvature, no perihelion precession.
  This module cannot correct Mercury's orbit and the crate's integrator is still Newtonian.
  Concretely: the published net GPS clock correction is about `+38 µs/day`, and the `−7.2 µs/day` this
  module computes is only the special-relativistic part — the GR term is larger and runs the other way.
- **One axis only.** These are the collinear formulas. Two perpendicular boosts compose into a boost
  *plus a rotation* (Thomas–Wigner), so applying these componentwise the way the rest of
  `physics_utils` is applied would be a physics error rather than a simplification. A second axis needs
  a `4×4` transformation. `collinear_composition_commutes` is pinned partly to record that the
  commutativity it asserts is exactly what fails in the general case.
- **No four-vectors, no spacetime interval, no Doppler shift.**
- **No relativistic collision solver.** The parent layer's `elastic_collision_1d` and
  `perfectly_inelastic_collision_1d` are Newtonian and have no counterpart here, so nothing yet conserves
  `Σγmv` and `Σγmc²` across a contact. `E² = (pc)² + (mc²)²` is *tested* as a cross-check between
  `energy.rs` and `momentum.rs` but is not exposed as a function.
- **No mass–energy conversion.** `rest_energy` reports `mc²`; nothing spends it. Mass is invariant and
  conserved everywhere in this crate. Length contraction here is what a frame *measures*, not what a camera would
  photograph — a fast cube looks rotated (Penrose–Terrell), not squashed, and there is no relativistic
  renderer.
- **Nothing in the simulation uses any of it.** At solar-system speeds γ−1 is `5e-9`, so `experiment_3`
  is Newtonian to eight decimal places and is right to ignore this. These functions are the vocabulary
  for whatever comes next, not a change to what runs.
