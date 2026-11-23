//! Every constant the `collision` layer reads — the tuning surface of body
//! physics.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module.
//! Nothing here is shared with another layer, and the layer deliberately reads no
//! *scale* constant at all: contact response is scale-free in the length unit, so
//! [`crate::constants::METRES_PER_UNIT`] does not appear in this layer.
//!
//! # Sections
//!
//! - **Geometry** — [`SURFACE_EPSILON_UNITS`], [`SPHERE_SUBDIVISIONS`],
//!   [`PLANE_THICKNESS_UNITS`]: what a hull *is*.
//! - **Narrow phase** — [`EDGE_AXIS_BUDGET`], [`NORMAL_TIE_TOLERANCE`],
//!   [`MIN_AXIS_LENGTH`]: how a contact is found.
//! - **Broad phase** — [`BROAD_PHASE_RELATIVE_SLACK`]: how a pair is ruled out
//!   before any hull is built.
//! - **Response** — [`MAX_SUBSTEP_DISPLACEMENT_UNITS`],
//!   [`MAX_SUBSTEP_DYNAMICAL_FRACTION`], [`MAX_SUBSTEPS`]: what happens once it is.
//! - **Merging and debris** — [`GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG`],
//!   [`DEBRIS_MIN_MASS_FRACTION`], [`DEBRIS_MIN_SPEED_FACTOR`], [`DEBRIS_MAX_SPEED_FACTOR`],
//!   [`DEBRIS_SPAWN_CLEARANCE`]: when two bodies become one, and what flies off.
//! - **Simulated contact** — [`SOFT_CONTACT_SUBSTEPS`], [`SOFT_CONTACT_MAX_DAMPING_RATIO`],
//!   [`CLUSTER_CONTACT_TOLERANCE`]: how finely
//!   a contact between particles is followed, and which particles count as one body.
//! - **Fluid particles** — [`SPH_SMOOTHING_FACTOR`], [`SPH_MIN_SMOOTHING_FRACTION`],
//!   [`SPH_VISCOSITY_ALPHA`], [`SPH_VISCOSITY_BETA`], [`SPH_VISCOSITY_EPSILON`], [`SPH_COURANT`],
//!   [`SPH_SOUND_SPEED_FLOOR_FRACTION`]: smoothed-particle hydrodynamics for fluid planets.
//!
//! # Two of these constrain each other
//!
//! [`SPHERE_SUBDIVISIONS`] sets a hull's edge count and
//! [`EDGE_AXIS_BUDGET`] caps the product of two hulls' edge counts, so **raising
//! the first silently disables edge-edge axes for pairs that currently get them**.
//! Change them together, and read both doc comments before changing either.

/// How far outside a face a point may sit and still count as touching, in world
/// units.
///
/// **Why this value.** Contact is a *closed* condition in this crate — touching
/// counts — so a point resting exactly on the surface must read as contained rather
/// than as a coin toss decided by the last bit of a dot product. `1e-4` units is
/// `1e5 m` at [`crate::constants::METRES_PER_UNIT`], which sounds enormous and is:
/// at this scene's scale every geometric tolerance is astronomical in SI. What
/// matters is the ratio to the bodies, and against a `1.5`-unit sphere this is 1
/// part in 15,000.
///
/// **Used by.** [`crate::collision::trimesh::TriMesh::contains_point`], once per
/// face.
///
/// **If you change it.** It only affects the point-in-hull query, not contact
/// detection — the separating-axis test does not consult it — so the practical
/// effect is on tests and on any caller asking "is this point inside that body".
/// Zero makes surface points ambiguous; large values inflate every hull by that
/// distance.
pub const SURFACE_EPSILON_UNITS: f32 = 1.0e-4;

/// Icosphere refinement level for a `Body::Sphere` hull.
///
/// `0` is the bare icosahedron (20 triangles, 12 vertices); each level splits every
/// triangle in four and projects the new vertices onto the sphere. Level `1` — 80
/// triangles, 42 vertices, 120 edges — is what ships.
///
/// **Why this value.** It is the cheapest level whose shape error is small enough
/// to be invisible against the bodies in play. Measured, with the hull inscribed so
/// its face centres sit inside the sphere:
///
/// | Level | Triangles | Vertices | Unique edges | Inradius / radius | Undershoot on a 1.5-unit ball |
/// | --- | --- | --- | --- | --- | --- |
/// | 0 | 20 | 12 | 30 | 0.7947 | 0.31 units — visibly wrong |
/// | 1 | 80 | 42 | 120 | 0.9342 | 0.099 units |
/// | 2 | 320 | 162 | 480 | 0.9822 | 0.027 units |
///
/// Level 0 is not usable: nearly a third of a unit of overlap before contact reads
/// as a bug. Level 2 was rejected for a specific reason rather than cost alone — at
/// 480 edges per hull it pushes **sphere-box** pairs past [`EDGE_AXIS_BUDGET`]
/// (480 × 18 = 8,640 axes), so a ball resting on the floor would lose its edge-edge
/// separating axes, the ones that matter when a curved hull meets a sharp one.
///
/// **Used by.** [`crate::collision::tessellate::collision_mesh`], for every sphere,
/// every time a hull is built — which is once per prop per contact pass.
///
/// **If you change it.** Raise [`EDGE_AXIS_BUDGET`] in the same change or you will
/// quietly downgrade the narrow phase for box-involving pairs. Cost grows fourfold
/// per level in triangles and roughly sixteenfold in the narrow phase, since the
/// axis set is quadratic in edge count. The inradius ratio above is **pinned by a
/// test** (`tessellate_tests::the_sphere_hull_undershoots_by_the_ratio_the_docs_quote`),
/// so that test must be updated with the new level's measured figure.
pub const SPHERE_SUBDIVISIONS: u32 = 1;

/// Thickness of the slab that stands in for a `Body::Plane`, in world units.
///
/// A plane is rendered as an infinitely thin quad, and an infinitely thin thing has
/// no inside: a zero-thickness hull is not closed,
/// [`crate::collision::trimesh::TriMesh::contains_point`] is meaningless on it, and
/// the separating-axis test degenerates. So the floor's hull is a **slab hanging
/// below the visible surface** — the quad is its top face, and the solid extends
/// downward along the plane's local `−Y`. Downward rather than centred so the
/// surface a body lands on is exactly the surface it is drawn at.
///
/// **Why this value.** It has to exceed the deepest penetration a single step can
/// produce, or a fast body can arrive below the slab, be found on the far side, and
/// get pushed *down* out of the floor. One unit is over ten times the per-step
/// motion of anything in the shipped scene, and matching
/// [`crate::core::constants::GRAVITY_SOFTENING_UNITS`] keeps the two "how close is
/// close" constants in the crate at one scale.
///
/// **Used by.** [`crate::collision::tessellate::collision_mesh`], for every
/// `Body::Plane`.
///
/// **If you change it.** Thinner is the dangerous direction: below
/// [`MAX_SUBSTEP_DISPLACEMENT_UNITS`] a prop can cross the whole slab within one
/// substep and tunnel through the floor. Thicker is safe but the solid extends
/// further below the visible quad, so anything placed under the floor intersects it.
pub const PLANE_THICKNESS_UNITS: f32 = 1.0;

/// Largest number of edge-pair axes worth testing for one pair of hulls.
///
/// The edge-edge axis set has `|edges(a)| · |edges(b)|` members. Past this budget it
/// is skipped and the separating-axis test runs on face normals alone.
///
/// **Why this value.** It is set to include every pair involving a box-like hull (a
/// 12-triangle box has 18 unique edges, so box-box is 324 axes and sphere-box
/// 2,160) and to exclude sphere-sphere (14,400 axes — 1.2 million vertex
/// projections per pair per substep). That is the right way round: edge-edge axes
/// matter when two *sharp* features cross, the classic case being two boxes meeting
/// edge to edge where every face normal reports overlap and only the edge cross
/// product separates them. On hulls with 80 near-tangent faces the face normals
/// already sample nearly every direction.
///
/// **Used by.** [`crate::collision::contact::contact_between_meshes`], once per pair
/// that survives the AABB broad phase.
///
/// **If you change it.** Dropping axes can only make the test *more* willing to call
/// a pair overlapping and can only make the reported depth *larger*, never smaller
/// — so lowering this trades accuracy for speed with an early, slightly over-strong
/// separation as the failure mode, never a missed collision. Raising it makes
/// sphere-sphere contact exact at a cost measured in millions of dot products per
/// contact. See [`SPHERE_SUBDIVISIONS`], which this constant constrains.
pub const EDGE_AXIS_BUDGET: usize = 4096;

/// How much deeper than the shallowest axis a candidate may be and still count as
/// tied with it, as a fraction of the shallowest overlap.
///
/// Every axis within this margin of the minimum contributes to the contact normal,
/// which is their average.
///
/// **Why this value.** The single shallowest axis is a *quantised* direction: a hull
/// face is flat, so at [`SPHERE_SUBDIVISIONS`] `= 1` the facet nearest a given
/// direction can be **21° away** from it, and bouncing off that facet would send two
/// spheres meeting head-on away at 42° to the line of centres — inventing transverse
/// motion, and angular momentum, out of the tessellation. Measured on the shipped
/// hull: two level-1 icospheres meeting head-on have **24 facet axes tied within
/// 2%**, arranged symmetrically about the line of centres, and their average is that
/// line to within float precision. Widening the margin to 20% selects the same 24
/// axes, so the result is not sensitive to the figure; 2% is the smallest margin that
/// captures the tie, which keeps genuinely deeper axes out of the average.
///
/// The margin is *relative* rather than absolute because overlap depth spans orders
/// of magnitude over a bounce — deep on arrival, near zero on the way out — and a
/// fixed tolerance would mean something different at each end.
///
/// **Used by.** [`crate::collision::contact::contact_between_meshes`], when it
/// builds the normal.
///
/// **If you change it.** Zero reverts to the raw minimum-translation axis and
/// restores the 21° facet error. Large values sweep in axes pointing genuinely
/// elsewhere and drag the normal off the contact direction — on a flat contact this
/// is harmless (every tied axis is the same direction) but on a corner it is not.
pub const NORMAL_TIE_TOLERANCE: f32 = 0.02;

/// Shortest axis length worth trusting as a direction.
///
/// **Why this value.** Cross products of near-parallel edges come out near zero, and
/// normalising one amplifies float noise into a garbage direction that can win the
/// shallowest-overlap contest and hand back a nonsense normal. `1e-6` is well below
/// any axis a real pair of hulls produces and well above `f32` noise.
///
/// **Used by.** [`crate::collision::contact::contact_between_meshes`], as the filter
/// on candidate axes and as the guard on the averaged normal.
///
/// **If you change it.** Note what the filter does **not** do: a too-short axis is
/// skipped, never *accepted* as separating, so this can only make the test more
/// permissive and never wrongly report two overlapping hulls as disjoint. Raising it
/// discards legitimate axes; lowering it toward zero lets noise-derived directions
/// into the contest.
pub const MIN_AXIS_LENGTH: f32 = 1.0e-6;

/// Slack the box broad phase adds to a sphere's reach, as a fraction of the coordinate
/// magnitudes involved.
///
/// **Why this value.** The broad phase computes the sphere-to-box gap exactly, but the hull
/// the narrow phase intersects has its vertices rounded at the scale of the coordinates: a
/// corner at `x = 5000` carries about `5e-4` units of `f32` error. `1e-5` is about a
/// hundred ulps, comfortably above that rounding and far below any gap worth rejecting.
///
/// **Used by.** `collision::resolve::sphere_reaches_box`, once per pair involving a
/// `Cuboid` or a `Plane`.
///
/// **If you change it.** Lowering it toward zero risks rejecting a pair that grazes a large
/// box, which would be a missed collision. Raising it only sends more far-apart pairs to
/// tessellation. At `1e-5` the 5000-unit floor reaches about `0.1` units above itself.
pub const BROAD_PHASE_RELATIVE_SLACK: f32 = 1.0e-5;

/// Furthest any prop may travel within one collision substep, in world units.
///
/// **Why this value.** Contact is tested at substep boundaries, so a prop that moves
/// further than the thickness of what it is about to hit can arrive on the far side
/// and never be seen to touch it — the tunnelling failure. A quarter unit is
/// comfortably below every thickness in the shipped scene: a sixth of the spheres'
/// `1.5`-unit radius, and a quarter of [`PLANE_THICKNESS_UNITS`].
///
/// **The shipped scene never reaches it**, and that is worth knowing: the two-body
/// pair closes at about `692 m/s` relative when they meet, which at `1e9 m` per unit
/// is `3.5e-7` units per second each — `0.029` units over one 60 Hz frame, or `0.17`
/// at the `max_dt` clamp, the worst case the app allows. So
/// [`crate::collision::resolve::substep_count`] returns `1` throughout and the
/// substep loop costs nothing. The machinery is insurance for the scenes this one
/// leads to.
///
/// **Used by.** [`crate::collision::resolve::substep_count`], called once per
/// `Scene::step`.
///
/// **If you change it.** Lower splits every frame into more substeps, each a full
/// force evaluation over every pair, bounded by [`MAX_SUBSTEPS`]. Raise it above the
/// thinnest body in the scene and fast props start tunnelling through walls instead
/// of bouncing off them.
pub const MAX_SUBSTEP_DISPLACEMENT_UNITS: f32 = 0.25;

/// Hard ceiling on substeps per `Scene::step` call.
///
/// **Why this value.** Each substep is a full force evaluation over every pair, so
/// an unbounded count turns one slow frame into a frozen window — and because the
/// frame delta feeds the next `dt`, a hitch could feed itself.
///
/// It was `64`, which was ample while the only criterion was contact displacement.
/// [`MAX_SUBSTEP_DYNAMICAL_FRACTION`] changed that: the solar system **with Jupiter's moons in
/// it** needs about `98` substeps to resolve Io's `1.77`-day orbit against a `16,667`-second
/// frame, and a ceiling of `64` would have capped it silently at 28 steps per orbit — accuracy
/// quietly stopping short of what was asked for, which is the worst way for a limit to be hit.
/// `256` leaves headroom above that case while still bounding the worst frame.
///
/// **Used by.** [`crate::collision::resolve::substep_count`], as the clamp on its
/// result.
///
/// **Where it actually bites, measured.** A `10 M☉` black hole that has eaten the Sun
/// (`2.19e31 kg`) approaching a `1.4 M☉` neutron star saturates this at a separation of
/// **`5.22` units** and is `6.5×` under-resolved by the time it reaches its `1.499`-unit capture
/// radius. So the *shape* of a close encounter between two placed objects is wrong, even though
/// nothing about it looks wrong: see below.
///
/// **Saturation is invisible to the conservation laws.** Symplectic Euler updates a pair by forces
/// that are equal and opposite by construction, so **momentum is conserved exactly at any step
/// size** — `momentum_is_conserved_even_when_the_step_is_far_too_large` pins that with a step a
/// thousand times too long. A capped substep count therefore produces a wrong trajectory that
/// passes every conservation check, which is why this constant is the thing to suspect when an
/// encounter looks strange and momentum looks fine.
///
/// **If you change it.** Raising it lets a fast scene stay accurate at the cost of
/// frame time; lowering it makes the step give up on resolving contact accurately
/// sooner, which for a viewer is the better failure than a freeze. A scene that
/// routinely saturates this is asking for a smaller
/// [`crate::constants::TWO_BODY_TIME_SCALE`], not a larger ceiling — or, for placed objects
/// specifically, for `-` to slow the clock before the encounter rather than a bigger ceiling
/// to absorb it.
pub const MAX_SUBSTEPS: u32 = 256;

/// Largest substep the integrator may take, as a fraction of the tightest **dynamical time**
/// in the scene. Dimensionless.
///
/// **Why this value.** [`MAX_SUBSTEP_DISPLACEMENT_UNITS`] asks *how far does the fastest body
/// move*, which is the right question for contact and the wrong one for an orbit. An orbit needs
/// steps short compared to the *time* it takes, and the natural such time is
/// `t_dyn = √(r³/(G·M))` — the orbital period over `2π`.
///
/// `0.007` puts about `2π / 0.007 ≈ 900` integration steps in one orbit, which is what the
/// first-order symplectic integrator needs to trace a closed ellipse rather than a spiral: at
/// `28` steps per orbit — what the displacement rule alone gave Io inside `experiment_3` — the
/// moon leaves. Around `100` steps the orbit closes but visibly precesses; `900` is comfortable.
///
/// **Used by.** `orbital_substeps`, called from
/// [`substep_count`](crate::collision::resolve::substep_count), which takes whichever of the two
/// criteria is stricter.
///
/// **If you change it.** It is **self-tuning and free when nothing is tightly bound**: the planets of
/// `experiment_3` ask nothing of it — Mercury's `t_dyn` is `1.2e6` s against a `16,667`-second frame,
/// so `2` substeps, under the `4` the *displacement* rule already wanted. It bites only when a scene
/// contains an orbit far tighter than its clock, which is exactly the case it exists for: the solar
/// system *with* the Galilean moons asks for about `98`, measured at `3.2 ms` a frame in release.
/// Larger values are cheaper and coarser; smaller ones hit [`MAX_SUBSTEPS`] sooner, at which point
/// accuracy silently stops improving.
pub const MAX_SUBSTEP_DYNAMICAL_FRACTION: f32 = 0.007;

/// The specific gravitational binding, `G·M/R`, above which a body counts as **held together by
/// its gravity** rather than by the strength of its material, in J/kg.
///
/// Only two such bodies can merge (see [`crate::collision::collision`]). A rigid body keeps its shape
/// and bounces, or rests, whatever it lands on: a spacecraft touching down on the Moon is not
/// absorbed into it.
///
/// **Why this value.** `1 J/kg` is a rocky body about `1.1 km` in radius at `3000 kg/m³`, which is
/// where collision studies put the change from strength to gravity (Benz & Asphaug 1999, Icarus
/// 142:5: the disruption threshold is lowest near `100 m` to `1 km`, and gravity controls it above).
/// Every moon, planet and star in the crate is many orders of magnitude above it (the Moon is
/// `2.8e6 J/kg`), and every spacecraft many orders below (the `2,900 t` Saturn V stack on a
/// `50 m` radius is `4e-6`).
///
/// **Used by.** `collision::collision::gravity_dominated`.
///
/// **If you change it.** Nothing in the shipped scenes is anywhere near it, so moderate changes do
/// nothing. Raised past a planet's `G·M/R`, planets stop merging and bounce.
pub const GRAVITY_DOMINATED_SPECIFIC_ENERGY_J_KG: f64 = 1.0;

/// The smallest share of the colliding mass worth breaking off as debris. Dimensionless.
///
/// **Why this value.** `1e-4`. A merge whose debris would be a smaller share than this is made a
/// perfect merge instead. Without a floor every fragment that falls back onto the body it came from
/// would itself make debris (a few parts in ten million of the mass, for a `1/1000`-mass fragment
/// landing at escape speed), and the cloud would never settle. The Earth struck by a Mars-mass body
/// throws off `0.55%`, fifty times this.
///
/// **Used by.** `collision::collision`.
///
/// **If you change it.** Lower lets small impacts spray debris too, and lets debris re-make debris.
/// Higher turns gentle giant impacts into perfect merges.
pub const DEBRIS_MIN_MASS_FRACTION: f64 = 1.0e-4;

/// The slowest a debris fragment leaves the merged body, as a fraction of the escape speed from
/// where it starts. Dimensionless.
///
/// **Why this value.** In simulations of giant impacts the debris leaves at speeds spread around
/// the escape speed: some falls back, some goes into orbit, some escapes (Canup 2004, Icarus
/// 168:433). `0.7` to [`DEBRIS_MAX_SPEED_FACTOR`]'s `1.3` gives that spread: the slowest
/// fragments rise to about a body radius and fall back, the fastest leave for good.
///
/// **Used by.** `collision::collision`.
///
/// **If you change it.** Lower makes more of the debris fall straight back. At or above `1`
/// nothing falls back.
pub const DEBRIS_MIN_SPEED_FACTOR: f64 = 0.7;

/// The fastest a debris fragment leaves the merged body, as a fraction of the escape speed from
/// where it starts. Dimensionless.
///
/// **Why this value.** See [`DEBRIS_MIN_SPEED_FACTOR`].
///
/// **Used by.** `collision::collision`.
///
/// **If you change it.** Higher sends more debris away for good, and is capped in any case by the
/// energy the impact brought: the fragments' speeds are scaled down if their kinetic energy would
/// exceed it.
pub const DEBRIS_MAX_SPEED_FACTOR: f64 = 1.3;

/// The gap left between debris fragments, and between them and the merged body, when they are
/// created, as a fraction of the distances involved. Dimensionless.
///
/// **Why this value.** `0.05`. Fragments are placed on a shell round the merged body, and must not
/// start touching it or each other: a touching pair would be resolved on the very next substep,
/// which is a collision that never happened. Five percent is clear of the hulls' facets and of the
/// small shift the merged body makes to keep the centre of mass where it was.
///
/// **Used by.** `collision::collision`.
///
/// **If you change it.** At zero, fragments start touching. Larger values start the debris further
/// out, which is only less faithful to where it came from.
pub const DEBRIS_SPAWN_CLEARANCE: f64 = 0.05;

/// How many substeps a simulated contact is followed over, at the least. Dimensionless.
///
/// A contact between two particles is a damped spring, integrated step by step, and lasts
/// `π·√(m*/k)` (see [`crate::physics_utils::soft_contact::contact_duration`]). The substep is kept
/// to at most this fraction of the shortest such time in the scene.
///
/// **Why this value.** `20`. An explicit spring is stable only for a step under `2/ω` of its
/// natural frequency `ω`, which is `0.64` of the contact time; twenty steps per contact is a factor
/// of thirteen inside that, so the damping and the rebound come out right to within a percent
/// (`soft_contact_tests` measures the restitution against it at `2,000` steps, and the collision
/// tests the pile at this value). Discrete-element codes use `20` to `50`.
///
/// **Used by.** [`crate::collision::resolve::substep_count`], for scenes with particle bodies. It
/// costs nothing where nothing has a modulus.
///
/// **If you change it.** Fewer than about `5` and contacts gain energy step by step until the
/// particles fly apart. More only costs time. When [`MAX_SUBSTEPS`] caps the count first (the clock
/// sped up far past the experiment's own) contacts are under-resolved, so slow the clock with `-`.
pub const SOFT_CONTACT_SUBSTEPS: f64 = 20.0;

/// How far apart two particles may be and still count as one body, as a fraction of their summed
/// radii. Dimensionless.
///
/// **Why this value.** `0.05`. Particles resting under their own weight touch with a slight overlap,
/// but in a body that is flowing or settling some neighbours are momentarily a hair apart; five
/// percent keeps those in their body while a fragment that has left at any speed is out within
/// seconds.
///
/// **Used by.** [`crate::collision::aggregate::contact_clusters`].
///
/// **If you change it.** Zero splits flowing bodies into many clusters. Large values count debris
/// that is still near the body as part of it.
pub const CLUSTER_CONTACT_TOLERANCE: f64 = 0.05;

/// [`CLUSTER_CONTACT_TOLERANCE`] for fluid particles: how far apart two may be and still count as
/// one body, as a fraction of their summed radii. Dimensionless.
///
/// **Why this value.** `0.5`. A fluid's particles do not stay on a lattice: they settle into a
/// disordered arrangement in which a neighbour is often a fifth or more past touching, though the
/// two are firmly in one body. Measured on a fluid Earth of `2,000` particles settling for four
/// hours: at `0.05` it read as `572` bodies. Half again the touching distance is still well inside
/// the kernel's reach (`2.6` spacings), and a fragment thrown off at any speed leaves it in seconds.
///
/// **Used by.** [`crate::collision::aggregate::contact_clusters`], for pairs of fluid particles.
///
/// **If you change it.** Smaller breaks a settled fluid planet into many bodies; much larger counts
/// the inner edge of a disk as part of the planet.
pub const FLUID_CLUSTER_CONTACT_TOLERANCE: f64 = 0.5;

/// The most damping a simulated contact is given, as a damping ratio `ζ`. Dimensionless.
///
/// A contact that only pushes returns `13.5%` of the approach speed even when critically damped
/// (`ζ = 1`), and needs infinite damping to return none (see
/// [`crate::physics_utils::soft_contact::restitution_for_damping_ratio`]). A material asking for
/// restitution `0` is given this much instead.
///
/// **Why this value.** `1.5`, which returns `7.6%` of the approach speed: still all but plastic
/// (`99.4%` of the approach energy dissipated). It is half as heavy as the step allows a particle
/// packed among neighbours. An explicit dashpot is stable while the damping a particle feels along
/// one direction, times `dt` over its mass, stays under `2`. One pair alone allows `2·ζ·ω·dt < 2`,
/// but in a packed lattice a particle feels about four contacts' worth along any direction (the
/// `cos²` weights of its twelve neighbours sum to `4`), which halves that to `2·ζ·ω·dt < 1`. At
/// [`SOFT_CONTACT_SUBSTEPS`] steps per contact `ω·dt = π/20`, so `ζ = 1.5` uses `0.47` of the `1`.
///
/// It was `3`, sized for one pair: `0.94` of the lattice's limit. Measured, particles in a settled
/// pile then flipped their velocity every substep, `±160 m/s` for ever, with the contact's refusal
/// to pull keeping the flip-flop from either growing or dying.
///
/// **Used by.** [`crate::collision::collision::apply_contact_forces`] and its GPU twin.
///
/// **If you change it.** Above about `3` the flip-flop comes back in packed piles; raise
/// [`SOFT_CONTACT_SUBSTEPS`] with it. Lower and `e = 0` rebounds more: `ζ = 1` returns `13.5%`.
pub const SOFT_CONTACT_MAX_DAMPING_RATIO: f64 = 1.5;

/// The smoothing length of a hot particle's pressure, as a multiple of the particle spacing
/// `(m/ρ)^(1/3)`. Dimensionless.
///
/// **Why this value.** `1.3`, the usual choice in smoothed-particle hydrodynamics (Monaghan 2005,
/// Rep. Prog. Phys. 68:1703, recommends `1.2` to `1.5`): the kernel then reaches `2.6` spacings,
/// about `55` neighbours in a face-centred cubic lattice, enough for a smooth pressure gradient.
/// For the lattice's particles that is a smoothing length of `2.32` radii.
///
/// **Used by.** [`crate::collision::collision::smoothing_length_m`], and so by the thermal pressure
/// in [`crate::collision::collision::apply_contact_forces`] and its GPU twin.
///
/// **If you change it.** Smaller and the pressure is felt by fewer neighbours and turns noisy; below
/// `0.9` hot particles feel only those they touch. Larger costs more pairs per particle.
pub const SPH_SMOOTHING_FACTOR: f64 = 1.3;

/// The fewest and most a fluid particle's smoothing length may be, as multiples of its value at
/// the material's reference density, `SPH_SMOOTHING_FACTOR·(m/ρ₀)^(1/3)`. Dimensionless.
///
/// A fluid particle's smoothing length follows its density, `SPH_SMOOTHING_FACTOR·(m/ρ)^(1/3)`, so
/// it keeps about the same number of neighbours whether compressed in a core or thrown into the
/// disk. These bounds keep a particle alone in space from reaching across the whole scene, and a
/// strongly compressed one from losing its neighbours.
///
/// **Why these values.** `0.5` and `3`. The most compressed material in a planet, iron at the
/// Earth's centre, is under twice its reference density, which needs `0.79` of the reference
/// length; `0.5` is never reached by gravity. At `3` a particle feels neighbours out to `7.8` of its
/// own spacings: a disk `27` times thinner than rock is still smoothed, and anything thinner is
/// vapour or debris whose pressure is nothing anyway. No published value; chosen here.
///
/// **Used by.** [`crate::collision::collision::apply_contact_forces`] and its GPU twin.
///
/// **If you change it.** A larger maximum makes isolated particles reach further, which costs
/// neighbour pairs (and, on the GPU tree, larger boxes to open).
pub const SPH_MIN_SMOOTHING_FRACTION: f64 = 0.5;

/// See [`SPH_MIN_SMOOTHING_FRACTION`].
pub const SPH_MAX_SMOOTHING_FRACTION: f64 = 3.0;

/// The linear coefficient `α` of the artificial viscosity of fluid particles. Dimensionless.
///
/// Smoothed-particle hydrodynamics resolves no shock on its own: particles would pass through each
/// other where a shock should be. The artificial viscosity of Monaghan, J. J. (1992), Smoothed
/// particle hydrodynamics, *Annual Review of Astronomy and Astrophysics* 30, 543–574,
/// doi:10.1146/annurev.aa.30.090192.002551, adds a pressure `Π` between approaching neighbours,
///
/// ```text
/// Π = (−α·c̄·μ + β·μ²) / ρ̄,    μ = h̄·(v_ab·r_ab) / (r² + ε·h̄²)    (v_ab·r_ab < 0, else Π = 0)
/// ```
///
/// and turns the kinetic energy it takes into heat, which is how a shock heats rock.
///
/// **Why this value.** `1`, with [`SPH_VISCOSITY_BETA`] `= 2`: the values Monaghan (1992) gives for
/// shocks, and the usual choice in impact simulations.
///
/// **Used by.** [`crate::collision::collision::apply_contact_forces`] and its GPU twin, and the
/// time step ([`crate::collision::collision::fluid_courant_seconds`]).
///
/// **If you change it.** Larger smears shocks and heats more in shear (a viscosity has no way to
/// tell shear from compression without a Balsara switch, not built); smaller lets particles
/// interpenetrate in a strong shock.
pub const SPH_VISCOSITY_ALPHA: f64 = 1.0;

/// The quadratic coefficient `β` of the artificial viscosity. Dimensionless.
///
/// **Why this value.** `2`, Monaghan (1992); see [`SPH_VISCOSITY_ALPHA`]. The quadratic term stops
/// particles passing through each other at high Mach numbers, which a giant impact at `9 km/s`
/// into rock whose sound speed is `3 km/s` is.
///
/// **Used by.** As [`SPH_VISCOSITY_ALPHA`].
///
/// **If you change it.** As [`SPH_VISCOSITY_ALPHA`].
pub const SPH_VISCOSITY_BETA: f64 = 2.0;

/// The softening `ε` of the artificial viscosity's `μ`, as a fraction of the smoothing length
/// squared. Dimensionless.
///
/// **Why this value.** `0.01`, Monaghan (1992; see [`SPH_VISCOSITY_ALPHA`]): it keeps `μ` finite for
/// two particles on top of each other and is otherwise negligible, at `1%` of `h²`.
///
/// **Used by.** As [`SPH_VISCOSITY_ALPHA`].
///
/// **If you change it.** Larger weakens the viscosity between close neighbours.
pub const SPH_VISCOSITY_EPSILON: f64 = 0.01;

/// The Courant factor of the fluid time step. Dimensionless.
///
/// A fluid particle's substep must be shorter than the time a signal takes to cross its smoothing
/// length,
///
/// ```text
/// dt ≤ C · h / (c·(1 + 1.2·α) + 1.2·β·v)
/// ```
///
/// with `c` its sound speed, `α` and `β` the viscosity's coefficients, and `v` the fastest approach
/// of any two particles. Monaghan (1992; see [`SPH_VISCOSITY_ALPHA`]) gives this form with the
/// largest `|μ|` among the neighbours for `v`; here `v` is bounded by twice the fastest particle's
/// speed in the centre-of-mass frame, which needs no pair loop.
///
/// **Why this value.** `0.3`, Monaghan (1992).
///
/// **Used by.** [`crate::collision::collision::fluid_courant_seconds`], through
/// [`crate::collision::resolve::substep_count`].
///
/// **If you change it.** Above about `0.5` the explicit step goes unstable in a shock; below `0.3`
/// costs substeps for no gain.
pub const SPH_COURANT: f64 = 0.3;

/// The least sound speed a fluid particle is given, as a fraction of its material's bulk sound
/// speed `√(A/ρ₀)`. Dimensionless.
///
/// The Tillotson sound speed falls towards zero in thin, cool vapour, where the pressure has nearly
/// vanished. The artificial viscosity and the time step need a signal speed even there: without
/// one, two vapour particles approaching each other would feel no viscosity at all.
///
/// **Why this value.** `0.25`, about `0.8 km/s` for basalt and `1 km/s` for iron: below the
/// speed of any impact here, so it never sets the time step of a real impact, and above zero. No
/// published value; chosen here.
///
/// **Used by.** [`crate::collision::collision::apply_contact_forces`] and its GPU twin, and
/// [`crate::collision::collision::fluid_courant_seconds`].
///
/// **If you change it.** Larger shortens the step and strengthens the viscosity in vapour.
pub const SPH_SOUND_SPEED_FLOOR_FRACTION: f64 = 0.25;

/// How many radial steps a fluid planet's hydrostatic profile is integrated in, across the radius
/// the planet would have uncompressed. Dimensionless.
///
/// **Why this value.** `4000`: about `1.7 km` a step for the Earth, a thousand times finer than the
/// finest particle spacing, so the profile's error is far below what the particles can resolve.
/// Fourth-order Runge–Kutta at that step is exact to print precision; the jump in density at the
/// core's edge is resolved to one step.
///
/// **Used by.** [`crate::collision::aggregate::hydrostatic_profile`], once per planet built.
///
/// **If you change it.** Fewer smears the core's edge over more depth. Each profile costs this many
/// steps times [`PLANET_PROFILE_BISECTIONS`].
pub const PLANET_PROFILE_RADIAL_STEPS: u32 = 4000;

/// How many times a fluid planet's central pressure is bisected to make its profile hold the
/// planet's mass. Dimensionless.
///
/// **Why this value.** `80`, on the logarithm of the central pressure between `1e6` and `1e15 Pa`:
/// after `80` halvings the pressure is known to `1e-23` of that range's span, far past `f64`'s
/// precision, so the last bisections change nothing and the mass is matched to rounding.
///
/// **Used by.** [`crate::collision::aggregate::hydrostatic_profile`].
///
/// **If you change it.** Below about `50` the planet's mass is matched less exactly, which the
/// builder then corrects by giving each particle its share of the true mass anyway.
pub const PLANET_PROFILE_BISECTIONS: u32 = 80;

/// How many densities each material's cold isentrope is tabulated at, from `ρ₀` to
/// [`PLANET_PROFILE_MAX_COMPRESSION`] times it, for the hydrostatic profile to look up the density
/// and energy at a pressure. Dimensionless.
///
/// **Why this value.** `2000`: a step of `0.25%` of `ρ₀`, over which the isentrope is so nearly
/// straight that linear interpolation errs by under `1e-5` in density.
///
/// **Used by.** [`crate::collision::aggregate::hydrostatic_profile`].
///
/// **If you change it.** Fewer coarsens the interpolation; more costs time at each build only.
pub const PLANET_PROFILE_ISENTROPE_SAMPLES: u32 = 2000;

/// The densest a fluid planet's material is tabulated to, as a multiple of its reference density.
/// Dimensionless.
///
/// **Why this value.** `6`. The most compressed material in a rocky planet the size of the Earth
/// is under three times its reference density with these equations of state, so `6` leaves the
/// table room; a pressure past its end is given the table's densest entry.
///
/// **Used by.** [`crate::collision::aggregate::hydrostatic_profile`].
///
/// **If you change it.** Below what the planet's centre needs and the centre is built too light;
/// above costs only table entries.
pub const PLANET_PROFILE_MAX_COMPRESSION: f64 = 6.0;

/// How many Runge–Kutta steps [`crate::physics_utils::thermal::tillotson_cold_energy`] takes to
/// integrate the cold isentrope up to one density. Dimensionless.
///
/// **Why this value.** `64`: the integrand is smooth, and fourth-order Runge–Kutta in `64` steps over
/// even a fivefold compression is exact to about `1e-9`.
///
/// **Used by.** [`crate::collision::aggregate::hydrostatic_profile`], for each tabulated density.
///
/// **If you change it.** Fewer only loses digits nobody reads.
pub const COLD_ISENTROPE_STEPS: u32 = 64;

/// The most neighbours a simulated particle touches at once: the coordination number of a
/// face-centred cubic packing. Dimensionless.
///
/// Friction is modelled as a viscous drag capped by its Coulomb limit,
/// and a drag with no spring behind it is stable in an explicit step only while it undoes less than
/// all of a slip per step. A particle feels every one of its contacts' drags at once, so each
/// contact may undo at most this fraction of a slip per step. With the normal damping's strength
/// instead, twelve contacts overshot together, the slip flipped direction every substep, and a
/// settled pile churned at `150 m/s` for ever with no energy dissipated, because the flip-flop's
/// mean is zero.
///
/// **Why this value.** `12`, exact for equal spheres (Kepler's packing): no contact arrangement of
/// equal spheres has more.
///
/// **Used by.** [`crate::collision::collision::apply_contact_forces`] and its GPU twin.
///
/// **If you change it.** Lower and the flip-flop comes back for particles with that many contacts.
/// Higher only weakens the drag below its Coulomb limit for slow slips.
pub const SOFT_CONTACT_MAX_NEIGHBOURS: f64 = 12.0;
