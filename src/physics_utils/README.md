# `src/physics_utils/` — physics constants and formulas

Physical constants and closed-form formulas, in `f64`. Bevy-free like `core/`,
and the intended home for the math that will eventually be driven from
`Scene::step`, which is still a no-op seam.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer and records the `f64` rationale. |
| `physics_constants.rs` | **The layer's constants module.** `GRAVITATIONAL_CONSTANT` — `G = 6.674_30e-11 m³·kg⁻¹·s⁻²` (CODATA 2018). `STANDARD_GRAVITY_M_S2` — `g₀ = 9.806_65 m/s²` exactly, the unit a powered rocket's acceleration is stated in. Every other layer names this file `constants.rs`; this one predates [`.claude/steering/constants.md`](../../.claude/steering/constants.md) and already satisfies it, so the name stands. |
| `physics_formulas.rs` | `gravitational_constant_calculator` — Newton's law of universal gravitation (returns a force). |
| `newtons_laws.rs` | Newton's three laws: `inertial_position` (first), `net_force` and `acceleration_from_net_force` (second), `reaction_force` (third). |
| `conservation_laws.rs` | Momentum and energy: `momentum`, `total_momentum`, `kinetic_energy`, `total_kinetic_energy`, `gravitational_potential_energy`, `total_mechanical_energy`; the checks `momentum_is_conserved`, `kinetic_energy_is_conserved`, `energy_is_conserved`; and the solvers `elastic_collision_1d`, `perfectly_inelastic_collision_1d`. |
| `relativity/` | **Special relativity**, in its own directory — `c`, `β`, `γ`, time dilation, length contraction, and the velocity transformation between frames. See [`relativity/README.md`](relativity/README.md). The one sub-directory here: it has its own constant, its own domain rule (`|v| < c`), and its own error convention (`Option`, not a benign fallback). |
| `kinematics.rs` | The constant-acceleration relations — `average_velocity`, `average_acceleration`, `velocity_after`, `displacement`, `position_after`, `speed_from_displacement`, `time_to_reach_velocity` — plus `MotionState`, which carries position/velocity/acceleration together and can `advance(dt)` itself. |

## Conventions every function here follows

- **Scalar, single axis, sign carries direction.** A caller working in 3-D
  applies these componentwise and supplies the geometry itself. No vector type is
  defined or imported.
- **SI base units, named in the parameter.** `mass_kg`, `distance_m`,
  `velocity_m_s`, `elapsed_s`, `net_force_n`, `..._j`. The unit is part of the
  signature, not just the prose.
- **A body is a `(mass_kg, velocity_m_s)` tuple**, and a system is a slice of
  them. Deliberately not a struct: these functions are meant to be callable from
  a `Scene::step` that has not been designed yet. `MotionState` is the one type in
  the layer, and it deliberately does *not* own a mass — mass is a parameter to
  its `momentum`/`kinetic_energy`/`net_force` methods, because the same trajectory
  can be shared by bodies of different mass and `Scene`'s bodies own their own.
- **Free functions are the implementation; types delegate.** `MotionState`
  contains no arithmetic of its own — every method forwards to a free function, and
  a property test asserts the delegation bit-exactly. The risk of having both an
  object and a functional API is that they drift into two implementations of one
  law; this is how that is prevented.
- **Return `Option` when a question has no answer, not a benign fallback.**
  `speed_from_displacement` past a projectile's apex and `time_to_reach_velocity`
  while coasting are unreachable *states*, not invalid inputs, so they return
  `None`. Returning `0.0` would tell the caller "immediately", which is a wrong
  answer rather than a cautious one. Precondition violations in those two
  functions also yield `None`; everything else keeps the `debug_assert!`-plus-value
  contract.
- **Every release fallback is documented and chosen for its failure mode**, not
  for uniformity. Most functions return `0.0`, but `inertial_position` returns the
  initial position (returning `0.0` would teleport a body to the origin),
  `elastic_collision_1d` returns its inputs unchanged (zeroing velocities would
  destroy momentum), and the conservation checks return `false` (an unverifiable
  claim must never read as a satisfied one).

## Rules and invariants

- **`f64`, not `f32`.** The render side is `f32` for glam and GPU reasons, but
  gravitation does not fit: `G ≈ 6.7e-11` against masses of `~1e30 kg` gives a
  mass product of `~1e55`, and `f32` maxes out at `~3.4e38`. Every value in this
  layer is `f64`; callers convert at the render boundary.
- **No Bevy imports.** Same restriction as `core/`, for the same reason: these
  functions must be testable with no window, no GPU, and no ECS.
- **Every function is pure and total.** No allocation, no global state, and no
  panic in a release build.
- **Preconditions use `debug_assert!`, matching `core/scene.rs`.** A debug build
  fails loudly with a message naming the function and the offending value; a
  release build returns a benign `0.0` rather than injecting `NaN`/`inf` into a
  caller's simulation state. Test both branches, split by
  `#[cfg(debug_assertions)]`, so `cargo test --release` also passes.
- **Check `is_finite()` before any ordering comparison.** `NaN` fails every
  comparison, so a sign test alone lets it through. This is the same ordering
  `clamp_dt` relies on.
- **Group float expressions for the property you need, not for readability.**
  `gravitational_constant_calculator` computes
  `G * ((m_a / r) * (m_b / r))`, not `G * m_a * m_b / (r * r)` and not
  `(G * m_a / r) * (m_b / r)`. Scaling each mass by `r` first avoids a spurious
  `1e55` intermediate, and forming the two ratios' product *before* touching `G`
  makes the result bit-exactly symmetric in the two masses — folding `G` into
  one side first disagreed by 1 ULP for lopsided mass pairs, which a property
  test caught.
- **Constants carry their unit and their source** in the doc comment, and are
  pinned bit-exactly by a test so an edit cannot be accidental.

## Naming note

`gravitational_constant_calculator` returns a **force in newtons**, not a
constant — the constant is `GRAVITATIONAL_CONSTANT`. The name is retained
because it is the requested API; `gravitational_force` would be more accurate.

## Where the tests are

- `tst/physics_utils/physics_constants_tests.rs` — 6 tests: the CODATA literal,
  finiteness and normality, order of magnitude, a dimensional check that the
  value is in SI rather than CGS units, standard gravity's defined value, and the
  `178 g` the powered rocket's docs quote.
- `tst/physics_utils/physics_formulas_tests.rs` — 5 proptest properties
  (finiteness, exact symmetry, linearity in each mass, inverse-square,
  monotonicity in separation) plus 13 unit tests including Earth–Sun attraction
  and Earth surface gravity as real-world anchors.
- `tst/physics_utils/newtons_laws_tests.rs` — 10 properties and 10 unit tests,
  including the `F = m·a` ⇄ `a = F/m` round-trip and the action–reaction
  involution.
- `tst/physics_utils/conservation_laws_tests.rs` — 13 properties and 19 unit
  tests. The collision solvers are the centre of it: the elastic one must
  conserve momentum *and* kinetic energy, the inelastic one must conserve
  momentum and *strictly reduce* energy. That contrast is what keeps the energy
  law falsifiable rather than vacuous.
- `tst/physics_utils/kinematics_tests.rs` — 16 properties and 22 unit tests. The
  seven relations are not independent, so most properties are cross-checks: each
  averaging relation inverts its forward counterpart, the timeless relation agrees
  with the time-based ones, `position_after` with `a = 0` equals
  `inertial_position` bit-exactly, and a force converted to an acceleration and
  integrated recovers `F/m`.

## Adding a file here

- Use `f64` and SI base units. Document the unit of every parameter and return
  value in the signature's doc comment.
- Put constants in `physics_constants.rs` — this layer's constants module, per
  [`.claude/steering/constants.md`](../../.claude/steering/constants.md), with the
  reason, the readers, and the impact-if-changed documented on each — and functions in
  `physics_formulas.rs`; do not inline a magic number into a formula.
- Write at least one proptest property expressing an algebraic law the formula
  must obey (symmetry, scaling, a conservation identity) and at least one unit
  test anchored to an independently published real-world value. The properties
  are what catch evaluation-order bugs; the anchors are what catch a wrong
  constant.
