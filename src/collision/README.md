# `src/collision/` — body physics: surfaces, contacts, and bounces

Where a prop *ends*. Before this layer, props were gravitational point masses
that phased through each other and a sphere's radius was decoration. This layer
gives every prop a closed triangle hull, tests those hulls for overlap, and
resolves the overlap — **elastically** for two solid bodies, or by **absorption** when one of
them is a black hole.

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
| `resolve.rs` | Elastic response: `resolve_contacts`, `substep_count`. |
| `absorb.rs` | Absorption: `absorb_contacts`. A prop touching one whose `absorbs` flag is set is merged into it — mass and momentum conserved — and **leaves the scene**. |

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

## Two responses, because a contact can be two different things

`resolve.rs` bounces; `absorb.rs` swallows. The flag that picks between them is
`Prop::absorbs`, set only by black holes, and **absorption runs first** — a prop that reaches
an event horizon is removed before the elastic pass can see it, so nothing bounces off a black
hole on its way in.

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
      integrate_gravity(props, dt/n)     core/integrator.rs
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
| `RESTITUTION` | `1.0` | Perfectly elastic — the requested behaviour. |
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
| Rotation | The impulse is central, so a glancing hit does not spin anything. `Prop` has no angular velocity or inertia tensor. |
| Friction | Only the normal velocity component is touched; bodies slide frictionlessly. |
| Resting contact | One pass per substep, pairs resolved in index order, so three-way overlap is order-dependent and a stack would jitter. |
| Swept detection | Contact is tested at substep boundaries, not along the path. `substep_count` is what keeps that safe. |
| Restitution per material | One crate constant. `perfectly_inelastic_collision_1d` is already available as the other end of the range. |

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

- **The elastic law is not reimplemented here.** `resolve.rs` calls
  `physics_utils::conservation_laws::elastic_collision_1d` and supplies only the
  axis, the same way `core/integrator.rs` composes gravitation out of
  `physics_utils`. A collision formula written inline in this layer is a bug even
  if it is correct, because it is a second copy of a law.
- **Velocities stay in units/s across the solver.** The elastic solution is
  linear in velocity and scale-free in the length unit, so `METRES_PER_UNIT`
  deliberately does not appear in this layer. Masses are already SI.
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
- `resolve_tests.rs` — equal masses exchange normal velocity; momentum is
  conserved through a bounce (via `momentum_is_conserved`); kinetic energy is
  conserved (via `kinetic_energy_is_conserved`); a wall reflects; a massless prop
  passes through; separation removes the overlap; `substep_count` scales with
  speed and is clamped.

## Adding a file here

- `bevy::math` only. Anything needing `bevy::ecs` belongs in `render/`.
- A new `Body` variant needs a `collision_mesh` arm that returns a **closed,
  convex, outward-wound** hull, and a test asserting all three. It needs no
  change to `contact.rs` — that is the point of the representation.
- If you add a physical law, put the law in `physics_utils/` and call it from
  here.
- Document the units of every geometric quantity, and state any convexity or
  winding precondition on the function that relies on it.
