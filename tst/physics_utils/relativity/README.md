# `tst/physics_utils/relativity/` — tests for the relativity sub-layer

Mirrors `src/physics_utils/relativity/`, one file per source file. All five are `#[path]`-included unit
test modules compiled *inside* their subject.

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
- **An identity nothing implements makes the best cross-check.** `E² = (pc)² + (mc²)²` is not a function
  anywhere in the crate, so `the_energy_momentum_relation_holds_everywhere` is a real constraint linking
  `energy.rs` to `momentum.rs` rather than a restatement of either. A stray `γ` in one of them fails here
  and nowhere else.
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
