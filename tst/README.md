# `tst/` — all test code

> **Purpose:** All test code. It mirrors `src/` directory for directory, and this file covers
> how Cargo finds it: `#[path]` stubs, `[[test]]` targets, and proptest seed files.
> **Edit here to:** change how tests are wired in, the headless integration test
> (`headless_app.rs`), or the key-binding tests. A layer's tests are in `tst/<layer>/`.
> **Not here:** the code under test, in [`src/`](../src/README.md).

Mirrors `src/` directory for directory. It is named `tst/`, not Cargo's
conventional `tests/`, so Cargo is pointed at it explicitly in `Cargo.toml`.

## What lives here

| Path | Job |
| --- | --- |
| `bindings_tests.rs` | Unit tests for `src/bindings.rs` and the `KEY_BINDINGS` table: no duplicated key, no empty row, no empty group, and that `--help` and the menu draw the same rows. The one root-level `*_tests.rs`, mirroring the one root-level module. |
| `headless_app.rs` | The only true integration test: builds `MinimalPlugins` + `ScenePlugin`, ticks 5 frames, asserts the Scene↔ECS mirror invariants. Declared as a `[[test]]` target in `Cargo.toml`. |
| `core/` | Unit + property tests for `src/core/`. |
| `collision/` | Unit + property tests for `src/collision/`, including the end-to-end "the spheres bounce instead of phasing through" check. |
| `render/` | Unit + property tests for `src/render/`, including the floor grid's geometry, the trails' length and fade, and both features' render-only-ness. |
| `input/` | Unit + property tests for `src/input/`. |
| `ui/` | Unit tests for `src/ui/`. |
| `physics_utils/` | Unit + property tests for `src/physics_utils/`. |
| `time/` | Unit + property tests for `src/time/` — the `TimeStep` guarantee and both clocks. |
| `performance/` | Unit tests for `src/performance/` — switch parsing, the benchmark summary, and the ignored floor-eclipse measurement. |
| `config/` | Unit tests for `src/config/`: the shipped files equal the defaults, optional files and keys, and errors naming the file and key. |
| `experiments/` | Unit + property tests for `src/experiments/` — catalog hygiene, the three-body premise, argument parsing, the generator. |
| `spacecraft/` | Unit tests for `src/spacecraft/`: the built-in craft and fleet, the Lambert solver, the steering laws, the flight report, and whole flights flown live, from the crewed transfer to the Moon to full-thrust Mercury to Jupiter. Two opt-in sweeps. |

## The two kinds of test file here, and why it matters

**Unit tests** (`<layer>/<module>_tests.rs`) are *not* integration tests. Each is
pulled into its module by a stub at the end of the corresponding source file:

```rust
#[cfg(test)]
#[path = "../../tst/<layer>/<module>_tests.rs"]
mod tests;
```

They therefore compile *inside* the module and keep access to its private items
and to `super::*`, while the source file stays focused on implementation. They
use `crate::…` paths, not `physics_sandbox::…`.

**Integration tests** (`headless_app.rs`) are separate binaries that see only the
crate's public API and use `physics_sandbox::…` paths. Cargo does not auto-discover
tests outside `tests/`, so each one needs its own `[[test]]` block in
`Cargo.toml`. Adding a file here will *not* run it unless you either add that
block or wire it in via a `#[path]` stub.

## Rules and invariants

- **Mirror the `src/` path exactly.** A test file's location is derived from its
  subject's location, so `src/a/b.rs` ⇒ `tst/a/b_tests.rs`. No exceptions, so
  that "where are this file's tests?" never needs a search.
- **All tests run headlessly** — no window, no GPU. Bevy tests use
  `MinimalPlugins` and `init_resource` the asset collections by hand, because
  `MinimalPlugins` omits `AssetPlugin`.
- **`cargo test --release` must pass too.** Tests asserting `debug_assert!`
  panics are gated `#[cfg(debug_assertions)]`, with the release contract asserted
  under `#[cfg(not(debug_assertions))]`.
- **proptest regression seeds are pinned explicitly.** Because `file!()` contains
  `../..` for `#[path]`-included modules, proptest's default seed location
  resolves to a bogus directory under `src/`. Every `proptest!` block in this tree
  therefore sets `failure_persistence` to
  `tst/<layer>/proptest-regressions/<module>_tests.txt` via a local
  `REGRESSION_SEEDS` const. New blocks must do the same, and the seed files must
  be committed — they are counterexamples the suite has already paid for.

## Adding a test file here

1. Put it at the mirrored path and end its subject with a `#[path]` stub.
2. Give the new directory a `README.md`
   (see `.claude/steering/source-directory-docs.md`).
3. Start the file with a `//!` header naming its subject module and stating that
   it is a `#[path]`-included unit-test module, not an integration test.
4. If it is a genuine integration test instead, add a `[[test]]` block to
   `Cargo.toml` and keep it at the `tst/` root — it has no `src/` counterpart to
   mirror.
