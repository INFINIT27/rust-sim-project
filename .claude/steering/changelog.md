---
inclusion: always
---

# SOP: every change is recorded in `CHANGELOG.md` the day it is made

## Rule

Before finishing a change that alters behaviour, structure, or the documented
contract, add or extend today's entry in the root
[`CHANGELOG.md`](../../CHANGELOG.md) **in the same change**. A behavioural change
with no changelog line is an incomplete change, the same way a new source
directory with no `README.md` is (see
[`source-directory-docs.md`](source-directory-docs.md)).

Record:

- a new module, plugin, system, public item, or dependency — `Added`
- changed behaviour, defaults, units, tuning constants, or file layout —
  `Changed`
- a deleted item, prop, or requirement clause superseded by an amendment —
  `Removed`
- a defect fixed, including one recorded as a known gap in a directory
  `README.md` — `Fixed`
- documentation, spec, or steering changes that stand alone — `Docs`

Do **not** record: formatting-only edits, comment typos, `target/` churn,
dependency lockfile refreshes with no version change, or work-in-progress that
was reverted within the same change.

## Format

One `##` heading per calendar day, ISO `YYYY-MM-DD`, newest day first, directly
under the file's intro. Within a day, `###` subsections in the fixed order
`Added`, `Changed`, `Removed`, `Fixed`, `Docs` — omit the ones with nothing in
them. Entries are bullets in the imperative-free past tense, one change each.

```markdown
## 2026-09-06

### Changed

- `SceneConfig::time_scale` default lowered to `1e6` so the fall takes ~3.5
  minutes: `5e6` advanced ~a day per frame, which stepped straight through the
  graze and lost the interesting part of the acceleration curve.
```

Append to today's existing heading rather than opening a second one for the same
date. Dates are the day the work was done, in the author's local time; never
back-date an entry to a day that already has one, and never invent a day's
history that cannot be sourced from the tree or from git.

## What a good entry says

State the change *and* the reason, in one or two sentences. The diff already
says what the code now does; the changelog exists for the reader six months out
who wants to know why it changed and whether the reasoning still holds. Prefer

> Scene lengths are world units at `1 unit = 1e9 m` rather than metres —
> storing metres directly puts `f32` spacing at ~2 km by `3e10 m`.

over

> Changed units.

Cite the governing clause where one exists: a requirement number, an amendment
id (`A1`), or a design-doc section.

## No release headings until there is a release

The crate is unpublished and unversioned, so the file is dated days only. When
versions start being cut, add the version and tag alongside the date on the
heading for the day of the cut (`## 2026-10-01 — v0.1.0`); do not retrofit
version headings onto earlier days.

## Checklist before finishing a change

- [ ] Today's `##` heading exists in `CHANGELOG.md` (created, not duplicated).
- [ ] Every behavioural, structural, or contract change in this diff has a
      bullet under the right `###` subsection.
- [ ] Each bullet gives a reason, not just a restatement of the diff.
- [ ] Affected directory `README.md` files are updated too.
