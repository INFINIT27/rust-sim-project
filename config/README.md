# `config/` — settings read at startup

> **Purpose:** The TOML files the app reads when it starts: the key bindings, the spacecraft fleet,
> camera tuning, on-screen UI sizes, motion trails, and the performance switches.
> **Edit here to:** change a setting without rebuilding. Edit a file and relaunch, or change it
> in the menu (`Esc` > Settings or Configuration) and press Save there.
> **Not here:** the code that reads these files is [`src/config/`](../src/config/README.md). The
> built-in values they mirror are in each layer's `constants.rs`. Per-experiment settings (scene,
> starting camera, time scale) are in [`src/experiments/`](../src/experiments/README.md).

Every file, and every key in it, is optional: delete one and the built-in value is used. The
shipped files hold exactly the built-in values, so they are a list of what can be changed.

The menu's Save rewrites a file from what is running, so comments you added to that file are
not kept. Reset on a menu page puts that page back to the built-in values (save afterwards to
make it stick).

| File | Settings |
| --- | --- |
| `spacecraft.toml` | The fleet `G` cycles through: each craft's name, mass, size, colour, launch altitude, and propulsion (least-energy transfer, direct, or coasting) with its engine and g limit. |
| `camera.toml` | Movement speed, the boost and precision multipliers, look sensitivity. |
| `ui.toml` | `[crosshair]` size and colour, `[compass]` size and placement, `[arrival]` pop-up width. |
| `trails.toml` | Whether trails show at launch, their length, sample spacing and count, and fade. |
| `performance.toml` | Anti-aliasing, the floor's eclipse shadows, vsync. The `RGA_AA`, `RGA_FLOOR_ECLIPSE` and `RGA_VSYNC` environment variables override them for one run. |
| `controls.toml` | Which key each action is on (`pause = "KeyP"`). `Esc` and the mouse cannot be rebound. |

- **A mistake stops the app** with the file, the key, and what is wrong, rather than running with
  a setting that silently did nothing. A misspelt key counts as a mistake.
- **Where the app looks:** the directory named by `RGA_CONFIG` if set; otherwise this directory
  (found from the crate root, so `cargo run` and `target/release/physics-sandbox` both use it);
  otherwise `config/` in the working directory.
- **An experiment can override a setting** it cares about: the solar-system scenes set their own
  trail length.
