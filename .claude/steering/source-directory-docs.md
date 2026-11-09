---
inclusion: always
---

# SOP: every source directory carries a `README.md`

## Rule

Whenever you introduce a new directory that contains source code, add a
`README.md` to it in the same change. A directory with code and no `README.md`
is an incomplete change, the same way a new public function with no doc comment
is.

This applies to every directory holding code, including:

- each layer directory under `src/` (`src/core/`, `src/render/`, …)
- the `src/` and `tst/` roots themselves
- each mirror directory under `tst/`

It does not apply to generated or tool-owned directories (`target/`,
`proptest-regressions/`), which are not authored code.

## Why

The module tree is the first thing a newcomer reads and the last thing anyone
updates. Rustdoc on `mod.rs` covers the *API*, but it is only visible after a
`cargo doc` build and it is written for a caller who already knows why the
module exists. A `README.md` renders inline in the file browser and in the IDE
explorer, so the question "what is this directory and may I put my new file
here?" is answered before anyone opens a `.rs` file.

The rule is deliberately mechanical: "does this directory have a `README.md`?"
is checkable in one command, so it does not depend on reviewer diligence.

## Required content

Name the file exactly `README.md` — that is what renders automatically in Code
Browser, GitHub, and the VS Code explorer. Keep it to a screen or less, and
include these six things:

1. **What this directory is** — one sentence a newcomer can act on.
2. **What lives here** — a one-line-per-file table, with each file's job.
3. **What its constants are** — the directory's `constants.rs` (see
   [`constants.md`](constants.md)) listed in the file table, and, where the
   directory has tunable values, a table of them with the reason for each value.
   The knobs a directory exposes are part of what it is.
4. **Rules and invariants** — the constraints that make this directory a unit,
   especially any that are enforced by convention rather than by the compiler
   (import restrictions, numeric types, ordering guarantees). Cite the
   requirement or design-doc clause where one exists.
5. **Where the tests are** — the mirroring `tst/` path, and how the tests are
   wired in.
6. **Adding a file here** — what a new file in this directory must satisfy, so
   the next person does not have to infer it.

## Do not duplicate rustdoc

The `README.md` and the module's `//!` rustdoc have different audiences, and
duplicated prose drifts apart. Split them:

- `README.md` — orientation. Why the directory exists, what belongs in it, how
  it relates to its siblings.
- `mod.rs` rustdoc — API reference. What the exported items are and how to call
  them.

Cross-link rather than restate. When a fact belongs in both, put it in the
rustdoc (it is closer to the code and testable via doctests) and reference it
from the `README.md`.

## Keeping them true

- Renaming, moving, splitting, or deleting a file means updating the affected
  `README.md` tables **in the same commit**. A stale table is worse than no
  table, because it is trusted.
- The root `README.md` holds a directory map: one row per directory, linking its
  `README.md`. When you add a directory, add its row too. The root stays to setup
  and that map; anything more detailed belongs in a directory's own `README.md`.
- A `README.md` that only restates the directory's name is not compliant. If
  there is genuinely nothing to say about a directory, that is a signal the
  directory should not exist.

## Checklist for a new source directory

- [ ] `README.md` exists in the new directory and covers all six sections
      above.
- [ ] `constants.rs` exists if the directory has any tunable value, and every
      constant in the directory lives in it — see [`constants.md`](constants.md).
- [ ] The corresponding `tst/` mirror directory exists and has its own
      `README.md`.
- [ ] The root `README.md` directory map includes the new directory.
- [ ] The parent module declares the new module, and the crate root re-exports
      any new public items.
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings`, and
      `cargo doc --no-deps` are clean.

## Related SOPs

- [`constants.md`](constants.md) — where a directory's constants live, and what
  every one of them must document.
- [`changelog.md`](changelog.md) — recording the change that added the directory.

## Audit

Every directory containing a `.rs` file must contain a `README.md`:

```bash
comm -23 \
  <(find src tst -name '*.rs' -exec dirname {} \; | sort -u) \
  <(find src tst -name 'README.md' -exec dirname {} \; | sort -u)
```

Empty output means compliant. Any line printed is a directory missing its doc.
