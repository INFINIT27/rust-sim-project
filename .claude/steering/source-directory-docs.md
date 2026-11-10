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
Browser, GitHub, and the VS Code explorer. Keep it to a screen or less. Open it
with the lookup summary below, then include these six things:

1. **What this directory is**: one sentence a newcomer can act on. The lookup
   summary's **Purpose** line covers this. Add more only if the directory needs
   background.
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

## The lookup summary: always the first paragraph

The `README.md` files are the repository's index. Someone (or an agent) looking
for where a change goes reads the root directory map, then only the **first
paragraph** of each candidate `README.md`, and reads further only in the one that
matches. That works only if the first paragraph always has the same shape and
always answers the routing question, so it is fixed:

```markdown
# `src/<dir>/` — short title

> **Purpose:** what this directory owns, in one or two sentences.
> **Edit here to:** the typical changes that belong here, as a list of tasks.
> **Not here:** the nearby concerns that live elsewhere, each with a link to the
> directory that owns it.
```

- It comes **directly after the `#` heading**, on line 3, before any history or
  rationale. Background prose goes after it.
- It is one blockquote paragraph (every line starts with `> `), so the extraction
  command below can find where it ends.
- **Edit here to** names tasks ("change the landing law"), not files. The file
  table answers "which file".
- **Not here** is the most valuable line. It catches the wrong guess a reader is
  most likely to make (for example, "the clock keys are in `input/`, not
  `time/`"). For a `tst/` mirror it points at the `src/` directory under test.
- Keep it to about eight wrapped lines. If it runs longer, the directory probably
  does more than one job.
- When the directory's responsibilities change (a file moves in or out, a
  concern moves to another layer), update the summary **and** the matching row in
  the root `README.md` map in the same commit.

The root `README.md` is the one exception. Its first paragraph describes the
project, and its directory map is the top of the index.

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
- The root `README.md` holds the directory map: one row per directory that has a
  `README.md` (each `tst/` mirror is covered by the single `tst/` row), saying
  what to go there to change. When you add a directory, add its row too. The root
  stays to setup and that map; anything more detailed belongs in a directory's own
  `README.md`.
- A `README.md` that only restates the directory's name is not compliant. If
  there is genuinely nothing to say about a directory, that is a signal the
  directory should not exist.

## Checklist for a new source directory

- [ ] `README.md` exists in the new directory, opens with the lookup summary
      (**Purpose** / **Edit here to** / **Not here**), and covers all six sections
      above.
- [ ] The **Not here** lines of neighbouring directories still point to the
      right place now that this directory exists.
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

Every `README.md` except the root must open with the lookup summary:

```bash
for f in $(git ls-files '*README.md' | grep -v '^README.md$'); do
  sed -n 3p "$f" | grep -q '^> \*\*Purpose:\*\*' || echo "$f"
done
```

Empty output means compliant. Any path printed is a `README.md` whose first
paragraph is not the summary.

To read the whole index (every heading and summary, and nothing else):

```bash
for f in $(git ls-files '*README.md' | grep -v '^README.md$'); do
  awk 'NR==1 {print; next} /^>/ {print; seen=1; next} seen {exit}' "$f"; echo
done
```
