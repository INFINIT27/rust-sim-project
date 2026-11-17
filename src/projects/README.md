# `src/projects/` — the simulations, one directory per kind

> **Purpose:** Groups the simulations by the kind of physics they explore. Each subdirectory is one
> project: its experiments, their data and constants, and its own `README.md`.
> **Edit here to:** add a new project (a new kind of simulation), or move an experiment between
> projects. To change an existing experiment, go to its project's directory.
> **Not here:** the gravity, the stepping and the units every project shares are in
> [`core/`](../core/README.md), drawing is in [`render/`](../render/README.md), and the
> spacecraft are in [`spacecraft/`](../spacecraft/README.md).

## What lives here

| Entry | Job |
| --- | --- |
| `mod.rs` | Declares each project, and the import restrictions they share. |
| [`interstellar/`](interstellar/README.md) | Gravity at astronomical scale: the two-body and three-body runs, the solar system, Jupiter's moons, the size ruler, Sgr A*, and the Earth and Moon. Also the experiment catalog and the command-line parsing that picks one. |

This directory has no constants of its own. Each project keeps its constants in its own
`constants.rs`.

## Rules and invariants

- **`bevy::math` only**, like `core/` and `collision/`. A project describes a world. It may read
  `core`, `collision`, `physics_utils`, `time` and `crate::constants`, but not `render`, `input`
  or `ui`.
- **Projects do not import each other.** What two projects need goes down into a shared layer
  (`core/`, `physics_utils/`, or `src/constants.rs`).
- **Every public item is re-exported from `src/lib.rs`**, so moving a file between projects
  breaks no caller outside the crate.

## Where the tests are

[`tst/projects/`](../../tst/projects/README.md) mirrors this directory, one subdirectory per
project. Each test file is `#[path]`-included at the end of the source file it tests.

## Adding a project

1. Create `src/projects/<name>/` with a `mod.rs`, a `constants.rs` if it has tunables, and a
   `README.md` that opens with the lookup summary
   ([`source-directory-docs.md`](../../.claude/steering/source-directory-docs.md)).
2. Declare it in this directory's `mod.rs`, and add a row to the table above.
3. Mirror it in `tst/projects/<name>/`, with its own `README.md`.
4. Add both directories to the index in the root [`README.md`](../../README.md).

Today the experiment catalog (`ExperimentId`, `ExperimentSpec`) and the command-line parsing live
in `interstellar/`, because every experiment so far is interstellar. When a second project brings
its own experiments, lift those two up into this directory, so the catalog spans every project.
