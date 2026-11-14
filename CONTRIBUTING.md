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

All five must be clean before a change is finished. Before pushing, also run
[the CI checks](#before-every-push-the-ci-checks) below. Some long tests are `#[ignore]`d. Run them
with `cargo test --release -- --ignored` when you touch what they cover; each one's `ignore`
reason says what that is.

A change to `assets/shaders/eclipse.wgsl` is not verified by the tests alone. See
[`assets/README.md`](assets/README.md).

## Before every push: the CI checks

GitHub runs these checks on every push to `main` and every pull request into it
([`.github/workflows/rust.yml`](.github/workflows/rust.yml)). Run the same commands locally, in
this order, and push only if all five pass:

```bash
cargo fmt --check                                    # formatting (fix with `cargo fmt`)
cargo clippy --all-targets --locked -- -D warnings   # lints: any warning fails
cargo build --locked                                 # the build
cargo test --locked --all-targets                    # unit and integration tests
cargo test --locked --doc -- --test-threads=4        # doc tests
```

CI runs the doc tests inside a single `cargo test`. Here they run on their own with four threads,
because compiling all of them at once fails spuriously on Windows. `--locked` fails if
`Cargo.lock` would change, as it does in CI.

In Windows PowerShell 5.1, which has no `&&`, the same as one line that stops at the first failure:

```powershell
cargo fmt --check; if ($?) { cargo clippy --all-targets --locked -- -D warnings }; if ($?) { cargo build --locked }; if ($?) { cargo test --locked --all-targets }; if ($?) { cargo test --locked --doc -- --test-threads=4 }
```

CI and your machine both use the Rust version pinned in [`rust-toolchain.toml`](rust-toolchain.toml),
which `cargo` installs on first use, so a lint that fails on GitHub also fails locally.
`rustc --version` shows which version is in use. Bump the pin on purpose, in a change of its own.
What these commands cannot catch is a difference between platforms (the maths library's last bits
differ between Windows and Linux). For that, CI on the pull request is the final check before
merging.

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

### From a change to a merged pull request

Work on a branch, never on `main`. Changes reach `main` only through a pull request whose CI run
has passed.

```bash
# 1. Start from an up-to-date main, on a new branch
git switch main
git pull
git switch -c my-change              # pick a short name for the change

# 2. Make the change, then run the CI checks above until all five pass

# 3. Review and stage what goes into the commit
git status                           # what changed
git diff                             # the changes themselves
git add path/to/file CHANGELOG.md    # stage files by name (`git add -A` stages everything)
git diff --staged                    # exactly what will be committed

# 4. Commit, with a conventional prefix: feat, fix, docs, refactor, test, chore
git commit -m "fix(transfer): keep the origin's moons in the correction forecast"

# 5. Push the branch (`-u` only the first time; later, plain `git push`)
git push -u origin my-change
```

Then open a pull request into `main` on GitHub (the push prints a link to do so), wait for its CI
run to pass, and merge it there.

```bash
# 6. After the merge: back to main, and delete the finished branch
git switch main
git pull
git branch -d my-change              # local; -d refuses if the branch is not merged
git push origin --delete my-change   # remote, unless GitHub already deleted it
```

### Fixing a commit

```bash
git commit --amend                   # fold the staged changes into the last commit
git reset --soft HEAD~1              # undo the last commit, keep its changes staged
git reset --hard HEAD~1              # drop the last commit and its changes: cannot be undone
git push --force-with-lease          # after amending or resetting a commit already pushed
```

Rewrite only commits on your own branch. Once a commit is on `main`, undo it with a new commit
(`git revert <commit>`) instead, and never force-push `main`.
