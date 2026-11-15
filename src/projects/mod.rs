//! Projects: the simulations, grouped by the kind of physics they explore.
//!
//! Each submodule is one **project**, a bucket of related experiments with its own data, constants,
//! and README. The layers below (`core`, `collision`, `physics_utils`, `time`) are shared by every
//! project. A project builds a world out of them, and `render`, `input` and `ui` show it.
//!
//! | Project | What it holds |
//! | --- | --- |
//! | [`interstellar`] | Gravity at astronomical scale: two- and three-body runs, the solar system, Jupiter's moons, the size ruler, and Sgr A*. It also holds the experiment catalog and the command-line parsing that picks one. |
//!
//! # Import restrictions
//!
//! The same for every project: **`bevy::math` only**. A project describes a world. It may read
//! `core`, `collision`, `physics_utils`, `time`, and `crate::constants`, but not `render`, `input`,
//! or `ui`. Projects do not import each other.

pub mod interstellar;
