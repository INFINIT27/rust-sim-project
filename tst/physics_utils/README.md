# `tst/physics_utils/` — tests for `src/physics_utils/`

Unit and property tests for the physics constants and formulas. See
`src/physics_utils/README.md` for the `f64` rationale and the float-grouping
rule these tests enforce.

## What lives here

| File | Job |
| --- | --- |
| `physics_constants_tests.rs` | 4 tests pinning `GRAVITATIONAL_CONSTANT`. |
| `physics_formulas_tests.rs` | 5 proptest properties + 13 unit tests for `gravitational_constant_calculator`. |
| `newtons_laws_tests.rs` | 10 proptest properties + 10 unit tests for Newton's three laws. |
| `conservation_laws_tests.rs` | 13 proptest properties + 19 unit tests for momentum, energy, and the collision solvers. |
| `kinematics_tests.rs` | 16 proptest properties + 22 unit tests for the constant-acceleration relations and `MotionState`. |
| `proptest-regressions/` | Committed proptest failure seeds. Tool-owned; no `README.md` required. |

## What is covered

**Constants.** The CODATA 2018 literal bit-exactly; finiteness, positivity, and
normality; the order of magnitude independently of the exact literal; and a
dimensional check that feeds `(1 kg, 1 kg, 1 m)` through the formula and expects
`G` back — which distinguishes SI from CGS units, a `1e-3` difference that every
other assertion here would accept.

**Formulas.** Properties: the result is always finite and non-negative; it is
bit-exactly symmetric in the two masses; it is linear in each mass; it obeys the
inverse-square law; and it never increases with separation. Unit tests:
`f(1,1,1) == G` exactly, a hand-computed `12.5·G`, Earth–Sun attraction
(`≈3.54e22 N`), Earth surface gravity (`≈9.82 m/s²`, an independent check on
`G`'s exponent), zero mass from either argument, extreme in-range magnitudes
staying finite, a shrinking separation with no singularity, and all six
precondition violations in both build profiles.

**Newton's laws.** Properties: inertial motion is additive over time (so step
size cannot matter for a force-free body); zero time and zero velocity are both
no-ops; `F = m·a` and `a = F/m` round-trip exactly; force is linear in mass;
heavier bodies accelerate no more under the same force; zero net force gives zero
acceleration (the second law implying the first); action and reaction sum to zero
bit-exactly; reaction is an involution; magnitudes match and signs oppose. Unit
tests anchor the weight of a 70 kg mass under standard gravity at 686.5 N.

**Conservation laws.** Properties: the elastic solver conserves momentum, conserves
kinetic energy, and reverses relative velocity exactly; the inelastic solver
conserves momentum, never gains energy, and produces a velocity between the two
inputs; both totals are additive over the system; an unchanged system conserves
everything at zero tolerance; injected momentum is detected; gravitational
potential energy is bit-exactly symmetric, non-positive, and rises toward zero
with separation; and the private `compensated_sum` agrees with a naive sum. Unit
tests pin equal-mass velocity exchange, the heavy-body wall limit, a strict
energy loss in an inelastic merge, Earth–Sun potential energy at `−5.30e33 J`, and
the sign of total mechanical energy as the bound/unbound criterion.

**Kinematics.** The seven relations are interdependent, so the properties are
mostly cross-checks rather than restatements of each formula: `average_velocity`
and `average_acceleration` invert their forward counterparts and agree with the
analytic mean `(v₀ + v₁)/2`; advancing in 2 steps and in 100 steps both agree with
one large step (the exactness claim for constant acceleration); `position_after`
with `a = 0` equals `inertial_position` **bit-exactly**, which is the seam between
this module and Newton's first law; the timeless relation agrees with the
time-based ones for non-reversing motion; `time_to_reach_velocity` round-trips
through `velocity_after` and never returns a negative duration; `MotionState`
methods match the free functions they delegate to bit-exactly; and a force
converted to an acceleration, integrated, then differentiated back recovers `F/m`.
Unit tests anchor free fall, a body thrown up returning with reversed velocity,
impact speed after a known drop, the apex as the boundary of the reachable set, and
every unreachable case returning `None`.

## Two things to preserve

**The symmetry property is bit-exact on purpose.** It originally failed at 1 ULP
(`mass_a = 1e-9`, `mass_b = 8.77e29`, `distance = 1e-3`) because the
implementation folded `G` into one mass before applying the other. Do not weaken
it to a tolerance — the exact form is the guarantee, and the seed for that case
is committed in `proptest-regressions/`.

**Precondition tests are split by build profile.** Debug asserts the panic
messages via `#[should_panic]`; release asserts the `0.0` fallback under
`#[cfg(not(debug_assertions))]`. Run `cargo test` *and* `cargo test --release` —
neither profile exercises the other's half of the contract.

## proptest seed location

`file!()` contains `../..` for `#[path]`-included modules, so proptest's default
seed location resolves under `src/` instead of next to the test. The `proptest!`
block here therefore sets `failure_persistence` explicitly to
`tst/physics_utils/proptest-regressions/physics_formulas_tests.txt` via the local
`REGRESSION_SEEDS` const. Every other `proptest!` block in the crate does the
same for its own layer; copy the pattern into any new one.

## Adding a test here

Every new formula needs at least one property expressing an algebraic law it must
obey and at least one unit test anchored to an independently published value. The
properties catch evaluation-order and exponent bugs; the anchors catch a wrong
constant. Use relative tolerances — values here span 60+ orders of magnitude, so
an absolute epsilon is meaningless.
