# `config/` — settings read at startup

> **Purpose:** The TOML files the app reads when it starts: the key bindings, the spacecraft fleet,
> camera tuning, on-screen UI sizes, motion trails, collision detail, and the performance switches.
> **Edit here to:** change a setting without rebuilding. Edit a file and relaunch, or change it
> in the menu (`Esc` > Settings or Configuration) and press Save there.
> **Not here:** the code that reads these files is [`src/config/`](../src/config/README.md). The
> built-in values they mirror are in each layer's `constants.rs`. Per-experiment settings (scene,
> starting camera, time scale) are in [`src/projects/interstellar/`](../src/projects/interstellar/README.md).

Every file, and every key in it, is optional: delete one and the built-in value is used. The
shipped files hold exactly the built-in values, so they are a list of what can be changed.

The menu's Save writes to `user/` in here, which git ignores; a file there is read instead of the
shipped one, so delete it to go back. It is written from what is running, so comments you added are
not kept. Reset on a menu page puts that page back to the built-in values (save afterwards to
make it stick).

| File | Settings |
| --- | --- |
| `spacecraft.toml` | Which craft `R` launches (`selected`), whether a flight lands or loops round its destination and returns (`trip`), and the fleet to choose from: each craft's name, mass, size, colour, launch altitude, and propulsion (least-energy transfer, direct, coasting, or a staged rocket that carries its propellant) with its engine and g limit, or its stages. |
| `camera.toml` | Movement speed, the boost and precision multipliers, look sensitivity. |
| `ui.toml` | `[crosshair]` size and colour, `[compass]` size and placement, `[arrival]` pop-up width. |
| `trails.toml` | Whether trails show at launch, their length, sample spacing and count, and fade. |
| `collision.toml` | How much detail collisions are simulated in: how many particles a planet built from particles is made of (`particles_per_body`, `1` to `200000`, applied at launch; above about `200` it wants the GPU), and how many bodies the debris of an instant merge is split into (`debris_fragments`, `0` to `512`). And how the GPU steps particles, applied at once: `gravity` (`"direct"` or `"barnes-hut"`), `neighbours` (`"all-pairs"`, `"tree"` or `"grid"`), and Barnes–Hut's `opening_angle` (`0.1` to `1.0`); the default is the exact pair, fastest up to tens of thousands of particles. And what planets built from particles are made of, applied at launch: `planet_model`, `"fluid"` (the default: an iron core under a basalt mantle, by smoothed-particle hydrodynamics) or `"rubble"` (basalt particles held by springs and friction). Also under `Esc` > Configuration > Collisions. |
| `wormhole.toml` | The shape of `experiment_9`, applied at once: `throat_units` (`0.1` to `100`), `half_length` and `lensing_width` in throat radii (`0.005` to `5`, `0.014` to `0.43`). Also under `Esc` > Configuration > Wormhole. |
| `performance.toml` | Anti-aliasing, the floor's eclipse shadows, vsync, and the black hole's ray-marcher (on or off, its quality, its stars). The `RGA_AA`, `RGA_FLOOR_ECLIPSE`, `RGA_VSYNC`, `RGA_BLACK_HOLE`, `RGA_BLACK_HOLE_QUALITY` and `RGA_BLACK_HOLE_STARS` environment variables override them for one run. |
| `controls.toml` | Which key each action is on (`pause = "KeyP"`). `Esc` and the mouse cannot be rebound. |

- **A mistake stops the app** with the file, the key, and what is wrong, rather than running with
  a setting that silently did nothing. A misspelt key counts as a mistake.
- **The GPU physics has no file:** it is on whenever there is a GPU, and `RGA_GPU_PHYSICS=off`
  turns it off for one run. The menu's first page says where the physics runs.
- **Where the app looks:** the directory named by `RGA_CONFIG` if set; otherwise this directory
  (found from the crate root, so `cargo run` and `target/release/physics-sandbox` both use it);
  otherwise `config/` in the working directory.
- **An experiment can override a setting** it cares about: the solar-system scenes set their own
  trail length.
