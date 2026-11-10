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
//! - **Response** — [`RESTITUTION`], [`MAX_SUBSTEP_DISPLACEMENT_UNITS`],
//!   [`MAX_SUBSTEP_DYNAMICAL_FRACTION`], [`MAX_SUBSTEPS`]: what happens once it is.
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

/// Coefficient of restitution for every contact: `1.0`, perfectly elastic.
/// Dimensionless.
///
/// The fraction of the normal approach speed that comes back out as separation
/// speed. `1.0` conserves kinetic energy exactly, `0.0` is a perfectly inelastic
/// stick, and real materials sit between.
///
/// **Why this value.** Perfectly elastic contact is the behaviour body physics was
/// introduced with (amendment A2), and it is the case with a closed-form solution
/// already available in [`crate::physics_utils::conservation_laws`]. It is a crate
/// constant rather than a `Prop` field because per-material restitution is a
/// modelling decision the scene has not needed, and one honestly documented value
/// beats a field every caller sets identically.
///
/// **Used by.** [`crate::collision::resolve::resolve_contacts`], via the
/// wall-reflection path. The two-movable-props path goes through
/// [`crate::physics_utils::conservation_laws::elastic_collision_1d`], which *is* the
/// `1.0` case — so a value other than `1.0` would apply to walls only until that
/// path is generalised.
///
/// **If you change it.** Below `1.0` the crate becomes inconsistent until the
/// movable-pair path is rewritten to take restitution, since that path is elastic by
/// construction. `perfectly_inelastic_collision_1d` is already available as the
/// other end of the range. Above `1.0` a bounce creates energy and the shipped
/// scene's oscillation grows without bound.
pub const RESTITUTION: f32 = 1.0;

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
