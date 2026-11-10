# `tst/config/` — tests for the configuration loader

> **Purpose:** Tests for [`src/config/`](../../src/config/README.md): the shipped files, defaults
> for anything left out, and errors that name the file and key.
> **Edit here to:** test a new setting or file, or a validation rule.
> **Not here:** the code under test is [`src/config/`](../../src/config/README.md); the files are
> [`config/`](../../config/README.md).

| File | Tests |
| --- | --- |
| `config_tests.rs` | 17 tests, `#[path]`-included into `src/config/mod.rs`: the shipped files equal the built-in defaults; every file is optional and an empty one changes nothing; a key changes only its own setting; misspelt keys and out-of-range values are errors naming the key; a replaced fleet, optional craft keys, and engine limits in g; an invalid craft named; a bad file on disk named; the environment overriding `performance.toml`; and configured resources surviving their plugins' `init_resource`. Saving: every page written by `write_file` loads back as what was saved (the built-in fleet exactly), a bad `controls.toml` is an error naming the entry, and `save_requested_system` writes the requested page and reports where. |

Tests that write files use `target/config-tests/<test>/`, which each test empties first.
