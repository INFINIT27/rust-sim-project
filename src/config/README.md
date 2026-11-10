# `src/config/` — the configuration files' loader

> **Purpose:** Reads `config/*.toml` once at startup and turns each file into the resources the
> other layers already run on (key bindings, fleet, camera tuning, crosshair, compass, arrival
> pop-up, trails, performance switches), and writes a file back when the menu saves it.
> **Edit here to:** add a setting to a file, add a new file, change how a file is validated, or
> change where the directory is looked for.
> **Not here:** the settings' *defaults* are in each owning layer's `constants.rs`. The files
> themselves are [`config/`](../../config/README.md). The `RGA_*` environment variables are
> [`src/performance/`](../performance/README.md).

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | `Config` (every configured resource), `Config::load`, `Config::insert_into`, `ConfigError`, the pure `parse`, and `config_dir`. `ConfigPlugin`, `ConfigDir`, `save_requested_system` and `write_file`: saving a page when the menu asks. |
| `files.rs` | One `…File` type per file (`SpacecraftFile`, `CameraFile`, `UiFile`, `TrailsFile`, `PerformanceFile`, `ControlsFile`): what the file may contain, its defaults, `into_runtime`, which checks the values and builds the runtime type, and `from_runtime`, which builds the file to save. |
| `constants.rs` | `CONFIG_DIR_VAR` (`RGA_CONFIG`), `CONFIG_DIR_NAME` (`config`), the six file names, and `SAVED_FILE_HEADER`. |

## Rules and invariants

- **Every file and every key is optional.** A `…File` defaults each field from the runtime type's
  own `Default`, so the app with no `config/` directory runs exactly as before, and a file need
  list only what it changes.
- **Anything wrong is an error, never a silent default.** Unknown keys are rejected
  (`deny_unknown_fields`), and `into_runtime` range-checks every value. The message names the
  file and the key; `main.rs` prints it and exits with status 2, like a bad argument.
- **Read once, at startup; written by the menu.** Nothing watches the files. The menu changes the
  running settings directly, and its Save posts a `SaveTarget` that `save_requested_system`
  (`ConfigPlugin`) writes back as that page's file, after `SAVED_FILE_HEADER`. A saved file is
  generated, so hand-written comments in it are not kept.
- **Inserted before the plugins.** Each owning plugin `init_resource`s its settings, which keeps a
  resource already there, so `Config::insert_into` runs first in `main.rs`.
- **The shipped files equal the defaults.** A test loads `config/` and compares it with every
  `Default`, so a default changed in code without its file (or the reverse) fails.
- **The file format belongs to this layer.** The runtime types carry no `serde` derives; each file
  has its own type here, so the file can use readable units (g, specific impulse) without the
  runtime types changing.
- **This layer sits above all others**, beside `main.rs`. It imports `input`, `ui`, `render`,
  `spacecraft`, and `performance`; none of them imports it.

## Tests

`tst/config/config_tests.rs`, `#[path]`-included into `mod.rs`.

## Adding a setting or a file

- **A setting in an existing file:** add the field to its `…File` with a doc comment, default it
  from the runtime type, check it in `into_runtime`, and add it to the shipped file in `config/`
  with a comment.
- **A new file:** its name goes in `constants.rs`, its type in `files.rs`, its resource in
  `Config` (loaded in `load`, inserted in `insert_into`), and the file in `config/` with its row
  in `config/README.md`. The runtime type must be a resource its plugin `init_resource`s, so the
  configured one survives.
