# `tst/projects/` — tests for the projects

> **Purpose:** Tests for [`src/projects/`](../../src/projects/README.md), one subdirectory per
> project.
> **Edit here to:** add a project's test directory. To change a test, go to its project's
> subdirectory.
> **Not here:** the simulations themselves, in [`src/projects/`](../../src/projects/README.md).

## What lives here

| Entry | Tests |
| --- | --- |
| [`interstellar/`](interstellar/README.md) | `src/projects/interstellar/`: catalog hygiene, scenes checked against published astronomy, argument parsing, and the random generator. |

`src/projects/mod.rs` declares modules only, so it has no test file.

## Adding a project's tests

Mirror the source path: `src/projects/<name>/<file>.rs` is tested by
`tst/projects/<name>/<file>_tests.rs`, `#[path]`-included at the end of the source file as
`#[path = "../../../tst/projects/<name>/<file>_tests.rs"]`. Give the new directory its own
`README.md`, and follow [`tst/README.md`](../README.md) for everything else.
