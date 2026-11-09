---
inclusion: always
---

# SOP: constants live in constants files, and every one explains itself

## Rule

**No named constant is declared in an implementation file.** Every constant lives
in a constants module, and every constant carries the three things a reader needs
in order to touch it: *why this value*, *who uses it*, and *what breaks if it
changes*.

A tuning value buried three hundred lines into the file that happens to use it is
invisible: nobody can answer "what knobs does this project have?" without reading
every file, and nobody can safely change one without reading every caller. The two
halves of this rule fix the two halves of that problem — the file layout makes the
knobs findable, and the required documentation makes them changeable.

## Where a constant goes

One decision, in this order:

1. **Used by more than one layer, or owned by `main.rs`?** →
   [`src/constants.rs`](../../src/constants.rs). This is the crate's shared
   vocabulary: the unit scale, the shipped scene's configuration, the window. It
   is deliberately small, and a value that only one layer reads does not belong in
   it.
2. **Otherwise** → `src/<layer>/constants.rs`, next to the code it governs.

That is the whole taxonomy. Exactly one constants module per directory, named
`constants.rs` — with one grandfathered exception,
`src/physics_utils/physics_constants.rs`, which predates this SOP and already
satisfies it. Do not add a second constants file to a directory; if a layer's
constants file is getting long, that is a signal the layer has too many knobs, not
that it needs two lists.

Constants modules are `pub` and re-exported at the crate root like everything else
(see `src/README.md`).

### The constants file imports, and is imported by, everything in its layer

A constants module may reference its layer's own types (a `Color`, a `Vec3`), but
it must not do any work: no functions, no `impl` blocks, no computation beyond a
`const` expression that is clearer than the literal it replaces. It is a list of
values with their reasons. Anything else in it will end up needing tests, and then
it is not a constants file any more.

## What must move, and what must not

Move anything a person might reasonably want to **tune, and could set differently
without the code becoming wrong**:

- physical and scene parameters — masses, separations, radii, time scales;
- tolerances, epsilons, and margins;
- budgets, caps, limits, and row counts;
- default field values used by a `Default` impl;
- dimensions and pixel sizes;
- subdivision counts, iteration caps, and other quality-versus-cost dials.

**Leave alone** the numbers that are part of an expression's meaning, where a
different value would simply be a different (wrong) formula:

- `0.5` in `½·m·v²`, `2.0` in a diameter, `3.0` when averaging three vertices;
- `1.0` and `0.0` used as identities, and unit-forming values like
  `Vec3::splat(1.0)` for a unit mesh;
- array indices, bit shifts, and loop bounds derived from a structure's shape.

The test is not "is it a number" but **"could this be different?"** `0.5` in a
kinetic-energy formula could not. `0.25` as a substep displacement budget could,
and someone will want to.

Naming a formula's `0.5` as `HALF` is not compliance with this SOP; it is
obfuscation, and it is explicitly not wanted.

## Associated constants are part of a type, not configuration

`TimeStep::ZERO` stays on `TimeStep`, in `simulation.rs`, and so would any
`Type::IDENTITY` or `Type::MAX`. An associated constant that is a **distinguished
value of the type** is API — it is how `Vec3::ZERO` works, and moving it to a
constants file would separate it from the type whose invariant it satisfies while
making it impossible to write `TimeStep::ZERO`.

The distinction is not "is it inside an `impl`" but **"is it a value someone might
configure?"** A restitution coefficient could be tuned and belongs in a constants
file; the zero-length step could not — it is what zero *means* for that type. Such
constants still need their doc comment, but "why this value" is usually one line.

The audit at the bottom of this file excludes them by matching `: Self =` (an identity
like `TimeStep::ZERO`) and `: [Self;` (a catalog like `ExperimentId::ALL`).

## Algorithm and format constants

A published algorithm's magic numbers and a format's own limits are **not knobs**, and
they belong next to the code that implements them:

```rust
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15; // ALGORITHM CONSTANT: SplitMix64
```

Changing SplitMix64's increment does not *configure* the generator, it makes it a
different and unstudied one — the same category as the `0.5` in `½·m·v²`. Likewise
`2^-24` for `f32`'s mantissa width: that is a property of the format, not a decision.

Four requirements, so the exception cannot be used to smuggle a tunable out of a
constants file:

1. **Private** (`const`, never `pub const`) — a knob other layers can read is a knob.
2. **Adjacent to its only use**, ideally function-local.
3. **A doc comment saying it is not tunable**, and what a different value would mean.
4. **The trailing marker** `// ALGORITHM CONSTANT: <what algorithm>` — which is what
   makes the audit mechanical rather than a matter of reviewer judgement.

If you find yourself wanting to *change* one of these to get different behaviour, it was
never an algorithm constant and it belongs in `constants.rs`.

## Naming

- `SCREAMING_SNAKE_CASE`, and **the unit is in the name**: `_UNITS`, `_SECONDS`,
  `_KG`, `_PX`, `_RAD`, `_UNITS_S`. A bare `MAX_SPEED` is not acceptable when
  `MAX_SPEED_UNITS_S` is available; the crate has two length scales and a scaled
  clock, and unit confusion is the defect class this naming exists to prevent.
- Dimensionless values say so in the doc comment (`RESTITUTION`,
  `NORMAL_TIE_TOLERANCE` — a ratio and a fraction).
- Prefix by subject where a layer has groups of related values
  (`CROSSHAIR_*`, `TWO_BODY_*`, `DEMO_*`), so the file reads as sections.

## Required documentation

Every constant needs a doc comment with all four parts. Terse is fine; missing is
not.

```rust
/// Furthest any prop may travel within one collision substep, in world units.
///
/// **Why this value.** A sixth of the shipped spheres' `1.5`-unit radius and a
/// quarter of the floor slab's thickness, so nothing can cross a body it should
/// have hit. Measured: the shipped pair moves `0.029` units per 60 Hz frame, so
/// the budget is never reached and `substep_count` returns `1`.
///
/// **Used by.** [`substep_count`], which is called once per `Scene::step`.
///
/// **If you change it.** Lower splits every frame into more substeps — each one a
/// full force evaluation over every pair — and is capped by [`MAX_SUBSTEPS`].
/// Raise it above the thinnest body in the scene and fast props start tunnelling
/// through walls instead of bouncing.
pub const MAX_SUBSTEP_DISPLACEMENT_UNITS: f32 = 0.25;
```

1. **What it is, with its unit**, in one line.
2. **Why this value** — the reasoning, and *the measurement* where one exists.
   "Chosen empirically" is not a reason; the number that came out of the
   experiment is. Cite the source for a published constant (CODATA, IAU).
3. **Used by** — the functions or types that read it, as intra-doc links. This is
   what makes the blast radius of a change visible from the definition.
4. **If you change it** — the direction of each tradeoff, and the failure mode at
   the extremes. A reader deciding whether they may touch the value should not
   have to derive this.

Where two constants constrain each other, say so in both
(`SPHERE_SUBDIVISIONS` and `EDGE_AXIS_BUDGET` are the worked example: raising one
requires raising the other).

## Pin a constant with a test when a claim depends on it

If prose anywhere states a consequence of a value — a ratio, a duration, a
measured error — a test must fail when the value drifts away from it.
`physics_constants_tests.rs` pins `GRAVITATIONAL_CONSTANT` bit-exactly against
CODATA; `tessellate_tests.rs` pins the icosphere inradius ratio the
`SPHERE_SUBDIVISIONS` docs quote. A documented number with no test is a comment
that will eventually be wrong.

## Checklist before finishing a change

- [ ] No free `const` in any implementation file — the audit below returns nothing.
      The two exemptions are associated constants that are part of a type's API
      (`TimeStep::ZERO`, `ExperimentId::ALL`) and marked algorithm or format constants,
      which must be private, adjacent, documented as not tunable, and carry the
      `// ALGORITHM CONSTANT:` marker.
- [ ] Each new constant is in the right module by the two-step rule above.
- [ ] Each has all four documentation parts, including **Used by** and
      **If you change it**.
- [ ] The unit is in the name.
- [ ] Any consequence stated in prose is pinned by a test.
- [ ] The layer's `README.md` file table lists `constants.rs`, and its
      tuning-constant table (where it has one) matches the code.
- [ ] `lib.rs` re-exports the new constant.
- [ ] `CHANGELOG.md` records it, per
      [`changelog.md`](changelog.md) — a new knob is a change to the project's
      surface even when no behaviour moved.

## Audit

Every constant declaration should be inside a constants module:

```bash
grep -rn "const " src --include="*.rs" \
  | grep -v "const fn" \
  | grep -v "/constants.rs:" \
  | grep -v "/physics_constants.rs:" \
  | grep -v ": Self =" \
  | grep -v ": \[Self;" \
  | grep -v "ALGORITHM CONSTANT"
```

The last three exclusions are the two documented exceptions: associated constants that
are part of a type's API (`TimeStep::ZERO`, `ExperimentId::ALL`) and marked algorithm or
format constants. Everything else printed is a constant that has escaped.

Empty output means compliant. Any line printed is a constant that has escaped into
an implementation file.
