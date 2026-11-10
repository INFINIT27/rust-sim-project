# Contributing to `physics-sandbox`

This file is the entry point. The full rules are the standard operating procedures in
[`.claude/steering/`](.claude/steering/), and each directory's design and invariants are in its own
`README.md`. When this file and an SOP disagree, the SOP wins.

## Before you start

- A stable Rust toolchain, rustc 1.87 or later. The pinned dependencies are in `Cargo.lock`, which
  is committed.
- A working graphics stack for running the app. The tests need none.

## The loop

```bash
cargo run --release -- solar-system   # see the change in the real app
cargo test                            # headless, needs no window or GPU
cargo test --release                  # must pass too; see tst/README.md
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

All five must be clean before a change is finished. Some long tests are `#[ignore]`d. Run them
with `cargo test --release -- --ignored` when you touch what they cover; each one's `ignore`
reason says what that is.

A change to `assets/shaders/eclipse.wgsl` is not verified by the tests alone. See
[`assets/README.md`](assets/README.md).

## The rules, in brief

| Rule | SOP |
| --- | --- |
| Every change that alters behaviour, structure, or the documented contract gets a bullet under today's date in [`CHANGELOG.md`](CHANGELOG.md), in the same change, with the reason. | [`changelog.md`](.claude/steering/changelog.md) |
| No named constant in an implementation file. Each layer's constants live in its `constants.rs` (values shared across layers go in `src/constants.rs`), and each one documents why it has its value, who uses it, and what changes if it moves. | [`constants.md`](.claude/steering/constants.md) |
| Every directory holding code has a `README.md` covering its design, invariants, and known gaps. Its first paragraph is a fixed lookup summary (**Purpose** / **Edit here to** / **Not here**), and the root `README.md` directory map indexes them all. | [`source-directory-docs.md`](.claude/steering/source-directory-docs.md) |

And from [`src/README.md`](src/README.md) and [`tst/README.md`](tst/README.md):

- **Layer boundaries.** `time/` imports no Bevy. `core/`, `collision/`, `physics_utils/` and
  `experiments/` may use `bevy::math` and nothing else from Bevy. `ui/` is the only module that
  imports `bevy_egui`.
- **Re-exports.** Every public item is re-exported from `src/lib.rs`. Inside the crate, refer to
  items by their real path, not through the re-export.
- **Physics advances only by a `TimeStep`**, never a raw float duration.
- **Tests mirror `src/` in `tst/`.** `src/a/b.rs` is tested by `tst/a/b_tests.rs`, which is
  `#[path]`-included at the end of `b.rs`. A new integration test needs a `[[test]]` block in
  `Cargo.toml`.
- **Proptest regression seeds are committed** under `tst/**/proptest-regressions/`, and every
  `proptest!` block sets its seed file explicitly.

## Commits

One logical change per commit, with its `CHANGELOG.md` entry and any affected `README.md` updates
included. Never commit `target/`.
