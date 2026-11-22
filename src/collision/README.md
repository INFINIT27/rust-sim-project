# `src/collision/` — body physics: surfaces, contacts, and bounces

> **Purpose:** What happens when bodies touch: each prop's triangle hull, the overlap test,
> bounces with restitution and friction, planets merging and throwing off debris, and
> absorption by black holes.
> **Edit here to:** change hull shape or resolution, contact detection, restitution,
> substepping, when bodies merge and what debris they make, or how absorption merges props.
> **Not here:** gravity and the step loop are in [`core/`](../core/README.md), body shapes and
> sizes as data in `core/scene.rs`, and drawing in [`render/`](../render/README.md).

Where a prop *ends*. Before this layer, props were gravitational point masses
that phased through each other and a sphere's radius was decoration. This layer
gives every prop a closed triangle hull, tests those hulls for overlap, and
resolves the overlap — by a **bounce** for two solid bodies, with each body's `Material`
setting how much it rebounds and grips; by a **merge**, with debris, for two planets that
cannot escape each other; or by **absorption** when one of them is a black hole. The last
section, [Toward realistic contact](#toward-realistic-contact-the-design), is the design and
how far it has got.

Bevy-free like `core/` and `physics_utils/` — `bevy::math` only, no ECS, no
egui, no window.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | Declares the layer, the pipeline, and the import restrictions. |
| `constants.rs` | Every constant the layer reads — the whole tuning surface, summarised in the table below. |
| `trimesh.rs` | `Triangle` and `TriMesh`: the boundary representation. Winding, `is_closed`, `contains_point`, `project_onto`, `support_point`, `orient_faces_outward`. |
| `tessellate.rs` | `collision_mesh(&Body)` — the hull of each variant, in world space — and `contact_mesh(&Prop)`, which builds it at the prop's **physical** radius rather than its drawn one. |
| `contact.rs` | Narrow phase: `Contact`, `contact_between_meshes`, `contact_between_props`, `aabbs_overlap`. |
| `resolve.rs` | Bounce response: `resolve_contacts`, `substep_count`. Restitution and Coulomb friction from the pair's combined `Material`. |
| `absorb.rs` | Absorption: `absorb_contacts`. A prop touching one whose `absorbs` flag is set is merged into it — mass and momentum conserved — and **leaves the scene**. |
| `collision.rs` | Collision between bodies held together by gravity. **Simulated contact:** `apply_contact_forces` (or `push_contacts` then `ContactPass::heat`, which `Scene::step` uses), `contact_is_simulated`, `shortest_contact_seconds`, `smoothing_length_m`, and the energy ledger `particle_energy` / `ParticleEnergy`. Particles whose material has a modulus push apart through a damped spring on every substep they overlap, friction spins them, and with a thermal material what the contacts dissipate becomes heat that pushes back: how two planets built from particles deform, merge and throw material off. **Instant merge:** `merge_contacts`, `CollisionEvent`, `gravity_dominated`. Two single-sphere planets that cannot escape each other become one in a step, with debris from the Leinhardt & Stewart law above their escape speed. |
| `aggregate.rs` | Bodies made of particles: `ParticleBody`, `particle_body` (a planet as a face-centred cubic pile of equal particles), `lattice_spacing_m`, `lattice_points`, `particle_radius_m`, `contact_clusters`, which groups touching particles into bodies (through a grid, O(n)), and `impact_outcome` / `ImpactOutcome`, which sorts a collision's mass into the largest body, the disk, what falls back and what escapes. |

## The hull comes from the prop's *physical* size, not its drawn one

`contact_mesh(&Prop)` is the single seam every contact test in this layer goes through, and it
honours `Prop::contact_radius_units` when it is set. Without that, a body drawn larger than it is
collides larger than it is — which is fine until one scene holds two length scales.

Jupiter is the case that forced it (amendment A15). Drawn at `10.973` units so it is visible from
Mars's orbit, truly `0.0699`, with Io orbiting at `0.4217`: the drawn hull is `26×` its innermost
moon's entire orbit, so all four Galilean moons start inside the planet and get ejected on the
first step. The largest exaggeration that keeps Jupiter inside Io's orbit is `6.03×`, and the
planets need `157×`.

Only `Body::Sphere` honours the override — the other three variants are sized by extents, and
nothing in the crate draws a cuboid or a plane at a scale different from its physical one. A
contact radius set on one of those is ignored, which a test pins so the silence is deliberate.

The same split retires a standing defect from A14: a black hole no longer has to absorb at the
radius it is drawn.

## Four responses, because a contact can be four different things

- **A simulated contact** (`collision.rs`, `apply_contact_forces`), between particles whose
  material has a modulus: a damped spring pushing them apart on every substep they overlap. Two
  planets built of such particles deform, flow and merge by themselves.
- **An instant merge** (`collision.rs`, `merge_contacts`): two single-sphere planets that cannot
  escape each other become one in a step.
- **A bounce** (`resolve.rs`): an impulse with the pair's restitution and friction, for rigid
  bodies.
- **Absorption** (`absorb.rs`): anything that reaches a black hole.

The simulated contact pushes at the start of the substep, before gravity moves anything. Then
**absorb, merge, bounce**, in that order: a prop that reaches an event horizon is removed before
anything else sees it, and a pair of planets that merges is gone before the bounce could part them.
A simulated pair is never merged or bounced; the contact force is its whole response. The rest of
this section is about absorption.

The flag that picks absorption is `Prop::absorbs`, set only by black holes, so nothing bounces
off a black hole on its way in.

An elastic bounce off an event horizon is not an approximation of what happens, it is the
opposite of it, which is why this is a second response rather than a tuning value on the first.
The merge conserves mass exactly and momentum to floating-point tolerance, loses kinetic energy
(it is perfectly inelastic, by construction), and grows the absorber's radius linearly with its
new mass — correct, since `r_s = 2GM/c²`.

Three decisions in it worth knowing:

- **An immovable prop is never the victim.** Otherwise a black hole placed near the ground plane
  eats the floor, which is a category error rather than a physics result: an immovable prop is
  the container the simulation happens inside. Found by a test, not by inspection.
- **The absorber does not move to the barycentre**, though strictly the centre of mass should
  stay put. A hole that visibly lurched each time it ate something would read as a bug, and the
  shift is a fraction of the drawn radius. The momentum is exact.
- **Two absorbers merge into the more massive one**, ties breaking toward the lower index —
  arbitrary, deterministic, and a case the physics does not distinguish either.

Absorption is also the crate's first operation that **removes** a prop, which amends
Requirement 5.4 (see `Scene::remove` and amendment A14): a `BodyId` may stop resolving, and
once it does it never resolves again.

## The pipeline, and where it is driven from

`Scene::step(dt)` runs `substep_count` passes of *integrate then resolve*:

```text
step(dt):
  n = substep_count(props, dt)
  repeat n times:
      apply_contact_forces(props, dt/n) collision/collision.rs
      integrate_gravity(props, dt/n)     core/integrator.rs
      absorb_contacts(scene)             collision/absorb.rs
      merge_contacts(scene)              collision/collision.rs
      resolve_contacts(props)            collision/resolve.rs
```

Within one `resolve_contacts`, for each pair: bounding-sphere test, plus a
sphere-to-box test when either side is a `Cuboid` or `Plane` → tessellate both
props (at most once each per call) → AABB test → separating-axis test → elastic
impulse → positional separation. The box test is what keeps a floor wide enough
to sit under the whole solar system from tessellating every body on every substep.

## Winding is the encoding of inner versus outer

Every triangle is **counter-clockwise seen from outside**, so its normal points
out of the solid. Nothing can recover that from the vertex positions alone — a
backwards triangle is a perfectly good triangle facing the wrong way — so:

- `TriMesh::orient_faces_outward(interior)` is the single place winding is fixed,
  and every tessellator calls it. Hand-listing 80 index triples in the right
  order is not a mechanism; a signed-distance test against a known interior point
  is.
- `TriMesh::is_closed()` is the geometric statement of *this body has an inside*:
  every directed edge appears once and its reverse once. An open or inconsistently
  wound hull is a hole physics leaks through. Every tessellator is tested against
  it.
- `contains_point` is a face-plane test, valid **because** every hull here is
  convex. It is a documented precondition, not a caveat: on a concave mesh it
  returns the convex-hull answer.

## What each `Body` variant becomes

| Variant | Hull | Triangles | Note |
| --- | --- | --- | --- |
| `Sphere` | Icosphere, vertices on the sphere | 80 | Inscribed, so 6.6% small at face centres — `0.099` units on a `1.5`-unit ball |
| `Cuboid` | The box | 12 | Exact |
| `Plane` | Slab of thickness `PLANE_THICKNESS_UNITS` **below** the quad | 12 | The visible surface is the top face |
| `Line` | Square prism of side `width` along the segment | 12 | `None` when `width == 0.0` |

Two `None` cases mean *this prop has no physical border and is pass-through*: a
zero-width line (a mathematical curve has no interior) and an invalid body. That
is a physical stance, not an error path.

## Numbers worth knowing before you tune anything

All of these are declared in `constants.rs`, each with its reasoning, its readers,
and its failure modes; the table is the index, not the documentation.

| Constant | Value | Why |
| --- | --- | --- |
| `SPHERE_SUBDIVISIONS` | `1` | 80 triangles, 120 edges. Level 0 undershoots a 1.5-unit ball by 0.31 units — visibly wrong. Level 1 undershoots by 0.099. Level 2 (0.027) would push sphere-box pairs past `EDGE_AXIS_BUDGET`, costing a ball-on-floor contact its edge-edge axes; raising one means raising the other. |
| `PLANE_THICKNESS_UNITS` | `1.0` | A zero-thickness quad has no inside. Must exceed the deepest single-step penetration or a body can be found *below* the floor and pushed down. |
| `EDGE_AXIS_BUDGET` | `4096` | Edge-pair axes are quadratic: two icospheres are 14,400 axes. The cut includes every box-involving pair (where edge-edge axes are load-bearing) and excludes sphere-sphere (where 80 face normals already sample nearly every direction). |
| `NORMAL_TIE_TOLERANCE` | `0.02` | The contact normal is the average of every axis tied within this margin of the shallowest, not the shallowest alone. A single facet can be 21° off the true direction; the 24 axes tied at 2% average back to it exactly. |
| `MIN_AXIS_LENGTH` | `1e-6` | Cross products of near-parallel edges normalise into garbage directions. Short axes are skipped, never accepted as separating, so the filter can only be permissive. |
| `SURFACE_EPSILON_UNITS` | `1e-4` | Touching counts as contact. 1 part in 15,000 against a 1.5-unit sphere. |
| `MAX_SUBSTEP_DISPLACEMENT_UNITS` | `0.25` | A sixth of the shipped sphere radius, a quarter of the floor slab. |
| `MAX_SUBSTEPS` | `64` | Each substep is a full force evaluation; an unbounded count turns a hitch into a freeze. |

**The shipped scene needs no substepping, and knowing that matters.** The
two-body pair closes at about `692 m/s` relative, which at `1e9 m` per unit is
`3.5e-7` units/s each — `0.029` units per 60 Hz frame, or `0.17` at the `max_dt`
clamp. Both are inside the displacement budget, so `substep_count` returns `1`
throughout and the loop costs nothing. The machinery is insurance for faster
scenes, where the alternative is the collision quietly failing to happen.

## What is deliberately not modelled

| Missing | Consequence you will see |
| --- | --- |
| Rotation in instant contacts | The instant bounce and merge are central, so a glancing hit between single-sphere bodies spins nothing. Particles in simulated contact do spin (`Prop::spin_rad_s`, a uniform sphere's inertia), but the spin is not drawn. |
| Resting contact | One pass per substep, pairs resolved in index order, so three-way overlap is order-dependent and a stack would jitter. |
| Swept detection | Contact is tested at substep boundaries, not along the path. `substep_count` is what keeps that safe. |
| Merging in most scenes | Only `experiment_1`'s planets are built from particles. Everywhere else every body is still a single `Material::ELASTIC` sphere and bounces. Phase 5 of the design. |
| Debris from a simulated impact, exactly | With heat and the Tillotson pressure the particle model throws debris off above escape speed: `4.3%` at twice it (`20,000` particles a planet), against Leinhardt & Stewart's `2.4%` with their central constant, at the edge of their fit's range (`1.7` to `4.1%`). Published simulations also have iron cores, a density measured from the neighbours, and an equation of state for compression; here compression is the springs' and the density is fixed. |
| Melting | Hot particles keep rock's friction (`0.6`); molten rock has none. |
| A planet's interior | The particles are equal, so a planet has one density throughout (no iron core), and the springs are linear, so rock does not stiffen under pressure: a pile compresses `3–4%` under its own weight. |
| Instant-merge debris direction and spin | Debris from an instant merge leaves evenly in every direction rather than in a fan from the impact site, and an off-centre merge's angular momentum is lost, since nothing spins. |
| A heat ledger for instant contacts | The kinetic energy an instant merge or bounce loses leaves the books. Simulated contacts keep it as heat. |
| Speed-dependent restitution | `e` is one number per material. Real `e` falls as the impact speed rises; at planetary impact speeds it is effectively `0`. |

## Mass, masslessness, and immovability in a contact

The same three-way distinction `core/` keeps:

| Prop | In a contact |
| --- | --- |
| Movable, `mass_kg > 0.0` | Bounces, and is pushed out of penetration. |
| Immovable | Infinite effective mass: never moves, reflects whatever hits it. |
| Movable, massless | **Passes through** — no response at all. |

The last row is the same stance the gravitation integrator takes: `a = F/m`
cannot deflect a massless body, and a contact impulse is a force like any other.

## Rules and invariants

- **The collision laws are not reimplemented here.** `resolve.rs` calls
  `physics_utils::conservation_laws::restitution_collision_1d` and
  `coulomb_friction_speed_change` and supplies only the axis, the same way
  `core/integrator.rs` composes gravitation out of `physics_utils`. A collision
  formula written inline in this layer is a bug even if it is correct, because it
  is a second copy of a law.
- **The default material changes nothing, to the bit.** `Material::ELASTIC`
  (`e = 1`, `μ = 0`) goes through the restitution solver, which is bit-identical to
  `elastic_collision_1d` at `e = 1`, and friction is skipped at `μ = 0`. A test pins
  it (`the_default_material_bounces_exactly_as_before_materials_existed`), so a
  scene changes only when it is given materials.
- **A contact's material is the pair's, not either body's.** `Material::combined_with`
  takes the lower restitution and the geometric mean of the frictions; it is
  symmetric, so swapping `a` and `b` changes nothing.
- **Velocities stay in units/s across the solver.** The bounce is linear in
  velocity and scale-free in the length unit, and the friction cap is a ratio of
  speeds, so `METRES_PER_UNIT` does not appear in the response. Masses are
  already SI.
- **Contact normals point from `a` toward `b`**, in the argument order the
  contact was built with, oriented by the hulls' centroids. A separating axis has
  no inherent sign, and the wrong one pulls colliding bodies together.
- **`Contact` is validated at construction**, not asserted: an invalid contact is
  discarded and reported as `None`, because the response would divide by it.
- **Preconditions use `debug_assert!`, benign in release** — the crate-wide
  contract. `apply_contact` returns without touching anything rather than
  applying a bad impulse.
- **Positional separation is not momentum conserving** and adds a little energy.
  That is the standard tradeoff for projection-based de-penetration, bounded by
  the depth, and far below the integrator's own first-order error at substepped
  depths.

## Where the tests are

`tst/collision/`, one file per source file:

- `trimesh_tests.rs` — winding, `is_closed`, `contains_point` against known
  interior/exterior points, projection and support-point properties.
- `tessellate_tests.rs` — every variant's hull is valid, closed, and outward
  wound; the plane slab hangs below its quad; a zero-width line has no hull; the
  measured icosphere inradius matches the ratio the docs claim.
- `contact_tests.rs` — separated pairs report `None`, overlapping pairs report a
  normal along the offset with a plausible depth, normals are antisymmetric in
  argument order, and a sphere resting on the floor reports an upward normal.
- `collision_tests.rs` — the simulated contact pushes equally and oppositely, only on overlap,
  never on rigid bodies, and through `Scene::step` rebounds at the material's restitution (and at
  the damping cap's `2.4%` for `e = 0`); simulated pairs are neither merged nor bounced; which
  pairs merge in an instant and which are left to the bounce; below escape speed a perfect merge,
  at twice escape speed the law's debris; and, as a property, every instant merge conserves mass,
  momentum and the centre of mass, starts its debris clear, and adds no energy.
- `aggregate_tests.rs` — the packing: neighbours just touch, none overlap, every particle inside,
  the count near the target; a built body has the planet's mass, centre and motion; piles cluster.
- `resolve_tests.rs` — equal masses exchange normal velocity; momentum is
  conserved through a bounce (via `momentum_is_conserved`); kinetic energy is
  conserved (via `kinetic_energy_is_conserved`); a wall reflects; a massless prop
  passes through; separation removes the overlap; `substep_count` scales with
  speed and is clamped; the default material is bit-identical to the old elastic
  response; restitution parts a pair at `e` times its closing speed and loses
  exactly `collision_energy_loss`; friction stops or slows a slide and conserves
  momentum; a ball dropped on an `e = 0.5` floor bounces to `e²` of each previous
  height.

## Adding a file here

- `bevy::math` only. Anything needing `bevy::ecs` belongs in `render/`.
- A new `Body` variant needs a `collision_mesh` arm that returns a **closed,
  convex, outward-wound** hull, and a test asserting all three. It needs no
  change to `contact.rs` — that is the point of the representation.
- If you add a physical law, put the law in `physics_utils/` and call it from
  here.
- Document the units of every geometric quantity, and state any convexity or
  winding precondition on the function that relies on it.

## Toward realistic contact: the design

Written 2026-11-21. Until then every solid body in the crate was hard and incompressible, and
had two possible fates on contact: a perfectly elastic bounce, or instant absorption when the
other body was flagged `absorbs`. This section is the plan for replacing that with what actually
happens, and how far it has got: phases 1 and 2 are built, phase 4 in a first form, the contact
itself simulated for planets made of particles, and phase 5
for `experiment_1` and the spacecraft's landing.

### What "realistic" means here depends on size

The crate holds bodies from a `4 t` lander to a `2e30 kg` star, and contact is governed by
different physics at the two ends.

- **Small bodies (craft, boulders): material strength rules.** Two rocks bounce, lose some
  energy, and scrape. A lander touches down or crashes. The numbers that matter are the
  coefficient of restitution `e`, the coefficient of friction `μ`, and a speed above which the
  structure fails.
- **Large bodies (moons, planets, stars): gravity rules.** Self-gravity dwarfs material
  strength, so planets do not bounce off each other. Two bodies that fall together meet at
  least at their **mutual escape speed**, `v_esc = √(2G(M₁+M₂)/(R₁+R₂))` (Earth and Moon:
  `10.0 km/s`). What follows depends on the impact speed against `v_esc`, the impact angle, and
  the impact energy per unit mass against what it takes to disperse the bodies. The standard
  classification is Leinhardt & Stewart (2012, ApJ 745:79): merging, partial accretion,
  hit-and-run, erosion and disruption.

The line between the two is `collision::gravity_dominated`: `G·M/R` above
`GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG` (`1 J/kg`, a rocky body about a kilometre across).

### Phase 1, built: materials

Each `Prop` has a `Material { restitution, friction }`:

1. **Restitution.** `restitution_collision_1d` resolves the normal velocities: they part at `e`
   times the speed they closed at, momentum is conserved, and `collision_energy_loss` gives
   the `½·μ·(1 − e²)·v²` dissipated.
2. **Friction.** `coulomb_friction_speed_change` slows the sliding along the surface by at most
   `μ` times the normal speed change, and never reverses it. Against a wall the movable body
   takes all of it. Between two movable bodies it is shared by inverse mass, so momentum is
   conserved.
3. **Pairs.** A contact uses `Material::combined_with`: the lower restitution (the softer
   body takes up the energy) and the geometric mean of the frictions.
4. **Nothing moves until a scene opts in.** Every constructor sets `Material::ELASTIC`, which is
   bit-identical to the old response.

### Phase 2, built: merge when the pair cannot escape

`collision.rs` runs between absorption and the bounce. A touching, closing pair of gravity-dominated
bodies **merges** when its contact loses energy (`e < 1`) and, bounced with its material, it
would part slower than its mutual escape speed. Such a pair is bound: it would fall back and hit
again, losing more each time, until it is one body; the merge resolves that whole sequence at
once. Nothing else needed a new constant.

- **Hit-and-run falls out of it.** A grazing or fast pair parts faster than escape speed, so it
  is not merged and the bounce resolves it, with its material's losses. No separate rule.
- **A rigid body never merges.** A spacecraft touching down on the Moon is far below the
  gravity-dominated line, so it is left to the bounce (and, for now, to the voyage's touchdown).
- **An elastic pair never merges.** At `e = 1` the bounces never decay, so the pair never
  becomes one. This is what keeps every scene that has not been given materials unchanged.
- **The merged body** keeps the more massive body's id, sits at the centre of mass (unlike a
  black hole, which stays put), and takes its radius from the combined volume,
  `R = (R₁³ + R₂³)^(1/3)`, scaled by the share of mass it keeps.

Built as a separate pass rather than the single `classify` the design first proposed: the three
outcomes need different data (a hull for the bounce, spheres and masses for the merge) and the
pass order already expresses the priority.

Not built: **the energy ledger.** The kinetic energy a merge or bounce loses still leaves the
books unrecorded.

### Phase 3: rotation and resting contact

- **Angular velocity and an inertia tensor on `Prop`**, with the impulse applied at
  `Contact::point` rather than at the centre of mass. Friction then turns sliding into rolling
  and spin, and a merge carries the pair's orbital angular momentum as spin.
- **Resting contact.** One pass in index order is fine for pairs meeting one at a time. A craft
  standing on the Moon, or anything stacked, needs a few sequential-impulse iterations per
  substep, and a rest-speed threshold below which `e` is treated as `0`. Without that threshold
  an inelastic bounce under gravity makes an endless series of ever-smaller hops.
- **Speed-dependent restitution.** `e(v)` falling with impact speed, so a bounce at `1 m/s` and
  an impact at `1 km/s` can share one material.

### Simulated contact, built: planets made of particles

Added after phases 1, 2 and 4, to resolve the contact itself rather than its outcome. A planet is
built as a face-centred cubic pile of equal particles (`aggregate.rs`), held together by nothing
but their gravity. Particles whose material has a modulus touch through a force, applied on every
substep they overlap (`collision.rs`, `apply_contact_forces`): the linear spring-dashpot of the
discrete element method (Cundall & Strack 1979), in `physics_utils::soft_contact`.

- **Stiffness** `k = E*·r*`, the material's modulus combined in series and the reduced radius.
  `PLANETARY_PARTICLE_MATERIAL` uses `1e13 Pa`, about seven times the bulk modulus at the Earth's
  centre, so a pile compresses `3–4%` under its own weight rather than slumping.
- **Damping** from the restitution, by Schwager & Pöschel's (2007) exact relation for a contact
  that never pulls. That relation matters: critical damping, which the textbook says stops a pair
  dead, returns `13.5%` once the contact cannot pull. `e = 0` would need infinite damping, so it
  is capped at `ζ = 1.5` (`SOFT_CONTACT_MAX_DAMPING_RATIO`), a `7.6%` rebound. It was `3`, sized
  for one pair; in a packed pile a particle feels about four contacts' damping along any direction,
  which halves the stable limit, and particles in a settled pile flipped their velocity every
  substep for ever.
- **Friction and spin.** Friction (`0.6`, Byerlee's law, for the planets' rock) acts on the slip of
  the two surfaces at the contact point, spin included, and applies a torque to each particle.
  The arms to the contact point are the radii scaled to add up to the distance between the
  centres, so angular momentum is conserved exactly. The viscous drag that regularises Coulomb's
  law undoes at most a twelfth of a slip per step (`SOFT_CONTACT_MAX_NEIGHBOURS`): with twelve
  contacts each undoing all of it, slips flipped direction every step and a pile churned at
  `150 m/s` with no energy dissipated.
- **Heat.** With a thermal material (`Material::thermal`, basalt's Tillotson parameters for the
  planets), what the contacts take from the motion is heat (`Prop::heat_j_kg`): the normal force
  the dashpot withheld times the separating speed, and friction times the slip. It is booked in a
  second pass, after gravity, at the mean of each particle's velocity at the start and end of the
  substep (`push_contacts`, then `ContactPass::heat`). That is exactly the kinetic energy the kick
  took, whatever the damping; every simpler choice tried made or lost energy, from `15%` of a
  settling planet's binding energy (the start-of-step speed) to a percent an hour (the middle of
  the contact kick alone, which sees motion in a pile resting under its own weight).
- **Pressure.** Hot neighbours within each other's smoothing length (`1.3` particle spacings,
  `SPH_SMOOTHING_FACTOR`) push apart by their Tillotson thermal pressure, in the symmetric form of
  smoothed-particle hydrodynamics, and the work comes out of the heat. The density is each
  particle's reference density: compression is the springs' job.
- **The books balance.** `particle_energy` adds kinetic, spin, gravitational (the potential of the
  integrator's softened force), spring and heat energy. A pile settling for two hours keeps it to
  print precision; the whole of `experiment_1`, `30` hours through the impact, to `0.04%` of the
  binding energy, at the moment of contact.
- **Resolution.** In a scene made only of particles the orbital criterion is O(n): the tightest
  pair is two particles touching, so each particle's dynamical time with a copy of itself at twice
  its radius bounds every pair, where the general criterion looks at every pair.
  `substep_count` gains a third criterion, at least `20` substeps per contact
  (`SOFT_CONTACT_SUBSTEPS`). Integrated through `Scene::step`, a contact at `e = 0.5` rebounds at
  `0.5` to within three percent. The thermal pressure needs no criterion of its own: its sound
  speed, a few km/s, is far below the springs' `45 km/s`.
- **On the GPU.** A scene made only of particles is stepped on the GPU when there is one
  ([`compute/`](../compute/README.md)), with these same laws in `f32`; the CPU path here is the
  reference it is tested against.
- **The merge is not written anywhere.** In `experiment_1` the two piles meet at the time the
  free-fall law gives (`14.0 h`), the impactor flattens into the Earth, a tenth of the binding
  energy becomes heat, and the merged body comes to rest, puffed up by its heat.

**Checked against Leinhardt & Stewart.** `impact_outcome` sorts the mass into the largest body,
what is bound and falling back, what orbits clear of the surface (the disk), and what escapes.

- **At the escape speed** (`experiment_1`) their simulations find a perfect merge, and so does
  this: nothing escapes, and the `4%` the heat lofts is bound and falling back.
- **At twice the escape speed** their law loses `2.4%` of the mass with the central constant
  `c* = 5`, and `1.7` to `4.1%` across their fit's `c* = 5 ± 2`. This model loses `3.6%` at the
  default detail and `3.9`, `4.9` and `4.3%` at `2,000`, `5,000` and `20,000` particles per
  planet: at the top of their range. Without heat it lost nothing at any detail; the pressure of
  shocked rock is what throws debris off, as the published work says.
- **The Moon-forming impact** (`experiment_8`, `45°` at escape speed) puts `1.6` to `2.0%` of the
  mass into the disk from `2,000` to `20,000` particles per planet, `1.5` lunar masses: inside
  the one to two Canup (2004) finds.

Going back to the paper also showed the instant merge was applying the debris law below escape
speed, where the paper has none; it now merges perfectly there.

### Phase 4, first form built: debris

For the instant merge of single-sphere planets. An impact at or below the mutual escape speed is a
perfect merge, as Leinhardt & Stewart find. Above it, a merge throws off the share of mass their
law gives: the specific impact energy
`Q_R = ½·μ·v² / (M₁ + M₂)` against the catastrophic-disruption threshold `Q*_RD`, with the
planet-sized constants of Stewart & Leinhardt (2012). The largest remnant is
`(M₁ + M₂)·(1 − Q_R / (2·Q*_RD))`, with a power law past `1.8·Q*_RD`. The laws are in
`physics_utils::impacts`. A share under `DEBRIS_MIN_MASS_FRACTION` is a perfect merge, which is
what stops debris falling back from making more debris.

The debris is split into **`Scene::collision.debris_fragments` equal bodies**, the detail dial:
`8` by default, up to `512`, set in `config/collision.toml` or under `Esc` > Configuration >
Collisions, and copied into the scene every frame so a change applies to the next impact. The
debris mass does not depend on it. Fragments start on a shell just clear of the merged body and
of each other, leave radially at `0.7` to `1.3` times the escape speed, and never carry more
kinetic energy than the impact brought. Mass, momentum and the centre of mass are all exactly
conserved.

Still to do: a power-law size distribution instead of equal fragments, debris thrown in a fan
from the impact site instead of evenly in every direction, and the impact angle in `Q_R`.

### Phase 5: apply it to every scenario

| Scenario | Status |
| --- | --- |
| `experiment_1` | **Done.** The giant impact: the Earth and a Mars-mass body at true size, each a pile of particles of `PLANETARY_PARTICLE_MATERIAL` (`200` for the Earth by default, set under Configuration > Collisions), fall together from rest, meet at about `9 km/s`, and merge through the simulated contact. The old two-body scene survives as `Scene::two_body_gravity`, a fixture the `core` tests are written against. |
| The spacecraft | **Landing target done.** Powered craft now aim to touch down at `2 m/s`, under the `3 m/s` the structure takes (the Apollo LM gear's design limit). Every landing in the tests, all 90 pairs included, comes in under it, and a faster touchdown is reported as a crash. Simulating the structure itself, and the craft resting on the surface by contact rather than by the voyage's bookkeeping, is phase 3. |
| `experiment_3` and `experiment_4` (Jovian) | To do: give planets and moons `PLANETARY_MATERIAL` so they merge on contact. Nothing touches in the shipped scenes, so this changes only what happens when the user throws something. |
| Objects placed with `B` (`objects.rs`), `experiment_6` | To do: stars and neutron stars merge. The black hole keeps absorbing. |
| `experiment_2`, the demo floor | To do: rock materials. |

Material values belong in the catalog rows (`projects/interstellar/constants.rs`), each with
a source, as the masses and radii already are.
