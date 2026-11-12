//! Experiments: which simulation to run, how to run it, and how to choose.
//!
//! An **experiment** is a scene plus the settings that make it watchable — the clock
//! scale it wants and where the camera should start. Before this layer the app could
//! run exactly one: `ScenePlugin` hard-coded `Scene::two_body_gravity()` and the shipped
//! time scale, and the startup camera read a constant from the render layer. Running
//! anything else meant editing three files.
//!
//! # What is here
//!
//! | Module | Job |
//! | --- | --- |
//! | [`catalog`] | [`ExperimentId`](catalog::ExperimentId), [`ExperimentSpec`](catalog::ExperimentSpec), and the builder for each experiment. |
//! | [`selection`] | Turning a command line into a [`Selection`](selection::Selection). |
//! | [`random`] | A deterministic SplitMix64 generator, for randomised initial conditions. |
//! | [`solar_system`] | `experiment_3`: the real planetary data, and the scene built from it. |
//! | [`objects`] | The interstellar objects the `B` key can place: dwarfs, a neutron star, black holes. |
//! | [`jovian`] | `experiment_4`: Jupiter and the Galilean moons — the first exactly-real scene. |
//! | [`scale_check`] | `experiment_5`: a static size-comparison row. A ruler, not a simulation. |
//! | [`black_hole`] | `experiment_6`: Sgr A*, spinning, with an accretion disc. |
//! | [`constants`] | The layer's tunables: ring geometry, speed window, camera poses, fallback seed. |
//!
//! # The experiments
//!
//! - **`experiment_1` — two-body free fall.** Two Earth masses `30` units apart,
//!   released from rest: they fall together, bounce elastically, and oscillate. The
//!   scene the app has always shown, now one entry in a catalog.
//! - **`experiment_2` — three-body ring.** Three Earth masses at 120° on a `12`-unit
//!   circle, each given a velocity that is exactly **tangential** to the circle with a
//!   randomly drawn magnitude, then released. Chaotic by construction, and a different
//!   outcome per seed.
//! - **`experiment_3` — the solar system.** The Sun and the eight planets at their real
//!   masses, real orbital distances, and real sizes *relative to each other*, on circular
//!   coplanar orbits from their real J2000 positions. Deterministic, and the one
//!   experiment whose bodies are drawn at a scale that is knowingly exaggerated — see
//!   [`solar_system`] for exactly which numbers are real and which are not.
//! - **`experiment_4` — the Jovian system**, **`experiment_5` — the scale check**: see
//!   [`jovian`] and [`scale_check`].
//! - **`experiment_6` — the black hole.** Sgr A*, spinning, with an accretion disc dragged round it
//!   and glowing with the heat of its friction, under the exact Kerr equation of motion. See [`black_hole`].
//!
//! # Determinism, and why the seed is recorded
//!
//! Randomised initial conditions are only useful if a run can be repeated. Every
//! experiment is built from an explicit `u64` seed, the same seed always yields the same
//! scene, and the seed is stored in the [`ExperimentSpec`](catalog::ExperimentSpec) and
//! shown in the overlay. So an interesting run is always reproducible after the fact
//! with `--seed <the number you saw>`, and by default each launch picks a fresh one.
//!
//! Note the caveat, because it is a real one: identical seeds give identical *initial
//! conditions*, not identical trajectories at different time scales. A three-body system
//! amplifies the integrator's error, so the same scene run at `2e7` and at `4e7` will
//! diverge — and neither is the "true" answer at first-order accuracy.
//!
//! # Import restrictions
//!
//! **`bevy::math` only**, like [`crate::core`] and [`crate::collision`]: an experiment is
//! a description of a world, not a renderer. It may read `core` (to build a `Scene`),
//! `physics_utils` (for `G`), and `crate::constants`; it may not touch `render`, `input`,
//! or `ui`. The `Resource` impl that lets Bevy hold an `ExperimentSpec` lives in
//! [`crate::render::plugin`], the same place `Scene`'s does.
//!
//! # Selecting one
//!
//! Today: a positional argument, parsed by [`selection::parse_args`] and handed to
//! `ScenePlugin`. The plugin takes a [`Selection`](selection::Selection), so a picker in
//! the overlay would build one directly and never touch the parser — which is why the
//! parse returns data rather than doing anything.

pub mod black_hole;
pub mod catalog;
pub mod constants;
pub mod jovian;
pub mod objects;
pub mod random;
pub mod scale_check;
pub mod selection;
pub mod solar_system;
