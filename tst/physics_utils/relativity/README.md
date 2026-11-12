# `tst/physics_utils/relativity/` — tests for the relativity sub-layer

> **Purpose:** Tests for [`src/physics_utils/relativity/`](../../../src/physics_utils/relativity/README.md),
> checked against published figures (GPS, the ISS, the LHC, muons, Mercury's perihelion, light
> bending at the Sun, Pound–Rebka) and against numerical derivatives and integrals.
> **Edit here to:** add or fix a relativity test. Its proptest seeds are in
> `../proptest-regressions/`.
> **Not here:** the formulas, in `src/physics_utils/relativity/`.

Mirrors `src/physics_utils/relativity/`, one file per source file. All sixteen are `#[path]`-included
unit test modules compiled *inside* their subject. Run them with
`cargo test --release --lib physics_utils::relativity`.

| File | Subject | Shape of the tests |
| --- | --- | --- |
| `constants_tests.rs` | `constants.rs` | 5 tests: the SI-defined value, that it is exactly representable in `f64`, and two independent cross-checks (a light-year, and light's travel time across an AU). |
| `lorentz_tests.rs` | `lorentz.rs` | 5 properties, 9 unit tests: exact rational γ at the Pythagorean βs, published GPS/ISS/orbital figures, the domain including `|v| = c`, and the numerical case for `lorentz_factor_minus_one`. |
| `time_dilation_tests.rs` | `time_dilation.rs` | 4 properties, 12 unit tests: which clock reads less, GPS at `7.2 µs/day`, the ISS at `28.2`, Earth's orbit at `0.156 s/year`, and cosmic-ray muons. |
| `length_contraction_tests.rs` | `length_contraction.rs` | 4 properties, 11 unit tests: the opposite-γ-direction guard, Earth's diameter losing `6.3 cm`, and Proxima Centauri at `0.19 ly` from the ship. |
| `velocity_tests.rs` | `velocity.rs` | 6 properties, 10 unit tests: the invariance of `c`, `0.5c + 0.5c = 0.8c`, that you cannot reach `c` by adding, and that the two directions are different functions. |
| `momentum_tests.rs` | `momentum.rs` | 6 properties, 15 unit tests: the ratio to `mv` is exactly `γ`, an LHC proton's published `6.8 TeV`, that `Σmv` fails to balance under a boost where `Σγmv` holds, and the `f64` saturation boundary. |
| `dynamics_tests.rs` | `dynamics.rs` | 6 properties, 11 unit tests: `γ³` checked against a **numerical derivative** of momentum, the `γ²` gap between the two directions, and the acceleration dying off under constant force. |
| `energy_tests.rs` | `energy.rs` | 8 properties, 24 unit tests: electron and proton rest energies to six figures, `E² = (pc)² + (mc²)²` as a cross-module check, the work–energy theorem, that a transverse force does no work, and the `20%` error the naive `γ−1` produces at `10 m/s`. |
| `spacetime_tests.rs` | `spacetime.rs` | 5 properties, 18 unit tests: the interval is invariant; time dilation, length contraction and velocity addition each **derived from `transform_event`** and compared with their own files; a timelike pair keeps its order in every frame and a spacelike pair does not; muons reaching the ground; the twin paradox as two intervals. |
| `rapidity_tests.rs` | `rapidity.rs` | 5 properties, 17 unit tests: rapidities add (against `compose_velocities`), `γ = cosh φ`; a `1 g` ship after 1 and 5 years of its own time; the distance and the ship's clock checked by **numerical integration**; saturation below `c`. |
| `doppler_tests.rs` | `doppler.rs` | 5 properties, 16 unit tests: which direction reddens, exact factors at the Pythagorean βs, the factor **derived by transforming two wave crests**, factors multiplying under composition, the Ives–Stilwell average, a radar echo, and the naive redshift losing its digits. |
| `energy_momentum_tests.rs` | `energy_momentum.rs` | 4 properties, 16 unit tests: agreement with `energy.rs` and `momentum.rs`, light as `m = 0`, the `511 keV` annihilation line, D–T fusion's `17.6 MeV`, the Sun's `4.26e9 kg/s`, and an LHC proton's mass recovered from its energy and momentum. |
| `collisions_tests.rs` | `collisions.rs` | 5 properties, 16 unit tests: both solvers conserve `Σγmv` and `Σγmc²`; the result is the same in every frame while **the Newtonian solver's is not**; the Newtonian limit against the parent layer's solvers; two lumps at `±0.6c` making `2.5 kg`; colliding beams against a fixed target. |
| `velocity_3d_tests.rs` | `velocity_3d.rs` | 5 properties, 16 unit tests: agreement with `velocity.rs` on each axis, `c` invariant in every direction, composition not commuting, the Wigner angle against the perpendicular closed form, Bradley's `20.49″`, and aberration against a transformed photon. |
| `four_vector_tests.rs` | `four_vector.rs` | 4 properties, 15 unit tests: a boost preserves the Minkowski product, agrees with `transform_event` along `x`, and agrees with `transform_velocity_3d` applied to a four-momentum; two photons having mass together; the system mass against `collisions.rs`. |
| `gravity_tests.rs` | `gravity.rs` | 3 properties, 52 unit tests: light fired past the hole (null condition kept along the path, capture between `2.55` and `2.65 r_s`, a ray turned back, the weak-field bend to `1%`); Flamm's paraboloid checked by its defining property (arc length along the sheet equals the integrated proper distance, to `1e-6`), its vertical throat and slow flattening; the equation of motion integrated with **RK4** (energy and angular momentum conserved to `1e-10`, the weak-field precession to `1%`, a periapsis worked by hand, stability outside the ISCO and a plunge inside, light's circle at `c/√3`, an infall that never crosses the horizon); the photon sphere and ISCO **found by scanning the effective potentials** rather than from their formulas, Sgr A*'s three radii and its `32.6`-minute ISCO orbit, Mercury's `42.98″` per century, `1.75″` of light bending at the Sun, Pound–Rebka's `2.46e-15`, GPS at `+38.5 µs/day` (and its two halves, from `gravity.rs` and `time_dilation.rs` separately), the ISS at `−24.7`, and nothing at rest inside the horizon. |
| `kerr_tests.rs` | `kerr.rs` | 26 unit tests: `equatorial_coordinates` inverting `equatorial_point`; zero spin is Schwarzschild; the ISCO and photon orbits against Bardeen–Press–Teukolsky values; the ISCO found by scanning `L(r)`; efficiencies `5.7%`, `15.6%`, `32.1%`; the disc zero at the ISCO, Newtonian far out, and its **energy budget closing** to half a percent; a circular orbit exactly centripetal and staying circular under RK4; energy and angular momentum conserved; an orbit holding outside the ISCO and plunging inside; frame dragging from the metric matching the formula; light null everywhere and the prograde photon orbit circular; light escaping with the spin where it is captured against it; the analytic acceleration matching a numerical one. |

## Conventions specific to this directory

- **A physics claim is checked against a published figure, never against the formula.** `γ` is one line
  of arithmetic, so a test that recomputed `1/√(1−β²)` would assert nothing. Instead: exact rational γ
  at `β = 3/5, 4/5, 5/13, 12/13, √3/2` (checkable in any textbook), and real engineering numbers — the
  GPS constellation's `−7.2 µs/day` is a correction actual hardware applies. A `γ` used upside down
  passes a self-consistency test and fails these.
- **Direction is tested separately from magnitude.** Time dilation and length contraction both
  "diminish the moving thing", so magnitudes cannot distinguish a correct implementation from an
  inverted one. `contraction_and_dilation_scale_by_gamma_in_opposite_directions` and
  `the_two_directions_are_not_the_same_function` are the tests that can.
- **A numerical-stability claim in a doc comment gets a test that would fail if it stopped being true
  — in either direction.** `the_stable_form_beats_subtracting_one_at_solar_system_speeds` asserts the
  two forms disagree by *more* than `1e-10` *and less* than `1e-6`. The lower bound is the interesting
  one: if the naive form ever becomes this accurate, `lorentz_factor_minus_one` has lost its reason to
  exist and the test says so.
- **A tolerance is measured, not guessed.** `the_invariance_is_often_bit_exact_but_not_always` sweeps
  `10,000` values of `β` and records that the invariance of `c` is bit-exact for most of them with a
  worst deviation of a couple of parts in `1e12`. It asserts the worst case is *nonzero*, so nobody
  tightens the tolerance elsewhere back to `assert_eq!` — an earlier version of these tests did, and it
  failed at `β = 0.9` by one ULP.
- **An identity the files under test do not implement makes the best cross-check.** Neither `energy.rs`
  nor `momentum.rs` contains `E² = (pc)² + (mc²)²`, so `the_energy_momentum_relation_holds_everywhere`
  is a real constraint linking the two rather than a restatement of either. A stray `γ` in one of them
  fails here and nowhere else. (`energy_momentum.rs` now exposes the relation; its own tests check it
  against those two files the other way round.)
- **A new file is checked against the older file that covers its special case**, never only against
  itself: `spacetime` against `time_dilation`, `length_contraction` and `velocity`; `velocity_3d`
  against `velocity`; `four_vector` against `spacetime` and `velocity_3d`; `collisions` against the
  parent layer's Newtonian solvers at low speed; `gravity`'s orbiting clock against `time_dilation`.
- **Property tests do not use `prop_assume!` to dodge a rounding.** `velocity_3d_tests` builds light
  rays four roundings below `c` (`LIGHT`) rather than rejecting the ones that land a rounding above
  it; the rejections exhausted proptest's budget at high case counts.
- **Tolerances were checked at 200,000 cases per property**
  (`PROPTEST_CASES=200000 cargo test --release --lib physics_utils::relativity`), and each one states
  in a comment where its size comes from.
- **A derived exponent is checked against the law it came from, not against itself.** `γ³` in
  `longitudinal_force` is a *claim* that `d(γmv)/dt = γ³ma`. `the_longitudinal_factor_is_the_derivative_of_momentum`
  differentiates the momentum numerically with a central difference and compares — so a `γ²` or `γ⁴`, or
  a dropped term in the product rule, fails. Recomputing `γ³` would have asserted the implementation
  against itself.
- **Tolerances in the Newtonian-limit properties are computed, not eyeballed.** Two of them were wrong by
  `1000×` on the first attempt: I squared `β` in my head and claimed `γ − 1 < 5.6e-11` at `1e5 m/s` when
  it is `5.6e-8`. Both now state the derivation in a comment, and the dynamics one notes that its bound is
  three times the momentum one because the longitudinal factor is `γ³`.
- **Every `proptest!` block pins `REGRESSION_SEEDS`** to
  `tst/physics_utils/proptest-regressions/relativity_<file>.txt`, prefixed because that directory is
  shared with the parent layer's test files. Left to proptest's default, `#[path]` makes `file!()`
  resolve into `src/`.
