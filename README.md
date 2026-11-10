# physics-sandbox

A desktop 3D physics sandbox in Rust and [Bevy](https://bevyengine.org/). Its main experiment is
**the solar system under Newtonian gravity**: the Sun, the eight planets, Earth's Moon, and
Jupiter's four Galilean moons, built from published measurements and drawn at true scale. Nothing
is on rails, and every path comes from the forces between the bodies.

## Setup

1. Install a stable Rust toolchain (rustc 1.87+) via [rustup](https://rustup.rs/):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Make sure your platform's graphics stack works: Metal on macOS, Vulkan plus X11/Wayland on
   Linux, DX12 or Vulkan on Windows.

## Run the solar system

```bash
cargo run --release -- solar-system
```

The first build compiles Bevy and takes a few minutes. Later builds are incremental.

The window opens full-screen. At true scale the planets are smaller than a pixel from the opening
view, so you will see the Sun and the orbit trails. Fly toward a planet to see it.

| Key | Action |
| --- | --- |
| Left-click, then mouse | Capture the cursor and look around |
| `W` `A` `S` `D`, `Space` / `Left Ctrl` | Fly forward/left/back/right, up/down |
| `C` | Cycle camera speed (normal, fast, slow) |
| `F`, then mouse / scroll | Lock onto the body at the crosshair, orbit it, zoom in/out |
| `R`, then `R` | Rocket: aim at the departure body and press `R`, then at the destination and press `R`. It flies under real gravity and its path is drawn |
| `G` | Pick the next spacecraft in the fleet: by default crewed transfer, full thrust, coasting probe; see below |
| `P` / `]` / `[` | Pause, or double/halve the time scale |
| `Esc` | Close a pop-up, or open the menu: settings (rebind any key, video), configuration, exit |
| `Cmd`-`Q` / `Alt`-`F4` | Quit |

### The spacecraft

By default `R` flies a **crewed transfer**: a `15 t` ship with an Aerojet 260 engine (`26.2 MN`, the
largest solid motor ever fired) throttled so the crew never feels more than `3 g`. It flies the way
real missions do, on the **least-energy** route: a `3 g` burn onto the cheapest coasting arc to the
target (found among every time of flight from where the planets are now), an unpowered cruise with
small correction burns, then a `3 g` braking burn and a soft landing at about `10 m/s`. Earth to the
Moon takes about 4.6 days, Earth to Mars about 17 months, Earth to Neptune 17 years. When it lands,
a pop-up shows the time of travel, top speed, speed change, engine time, energy and propellant
used, peak g, and touchdown speed.

`G` cycles through the rest of the fleet. The built-in fleet also has the same ship at
**full thrust** (`178 g`, uncrewed), which burns straight at the target, flips, and brakes
(Mercury to Jupiter in 13 hours at 12% of light speed), and a **coasting probe** that leaves at
up to Voyager 2's `15.4 km/s` and can use gravity assists. The craft are plain data (`src/spacecraft/config.rs`), and the fleet is read from
`config/spacecraft.toml` at launch: edit it to change a craft or add your own.

### Settings

The files in [`config/`](config/README.md) are read when the app starts: the spacecraft fleet,
camera speed and look sensitivity, crosshair, compass and pop-up sizes, trails, and the performance
switches. Edit one and relaunch; no rebuild is needed. Every file and key is optional, and a
misspelt key or out-of-range value stops the app with a message naming it.

The engine is idealised: it never runs out of propellant, and the mass never changes, so the
propellant reported is what it would have burned. The simulation is Newtonian throughout.

## Other experiments

```bash
cargo run --release -- --list   # every experiment
cargo run --release -- --help   # arguments and all in-window keys
cargo run --release -- moons    # Jupiter and the Galilean moons
cargo run --release -- scale    # the planets side by side at true size
```

Running with no argument starts `experiment_1`, the two-body free fall.

## Tests

```bash
cargo test
```

These run headlessly, so you need no window or GPU.

## Where the details are

Every source directory has a `README.md` that covers its design, invariants, and known gaps. The
table below is the **index**: one row per directory. To decide where a change goes, start here,
then read only the first paragraph of the candidate `README.md`. That paragraph always says what
the directory is for (**Purpose**), what to change there (**Edit here to**), and where nearby
concerns live instead (**Not here**).

| Directory | Go here to change |
| --- | --- |
| [`src/`](src/README.md) | Crate root, app wiring and the window, key bindings, constants shared by layers |
| [`src/core/`](src/core/README.md) | The world model: `Scene`, `Prop`, `Body`, units (1 world unit = 10⁹ m), the step loop, gravity |
| [`src/core/transfer/`](src/core/transfer/README.md) | The coasting-path search between two bodies, gravity assists, the prediction the spacecraft use |
| [`src/collision/`](src/collision/README.md) | Hulls, contact detection, elastic bounces, black-hole absorption |
| [`src/time/`](src/time/README.md) | The simulation and observer clocks, `TimeStep`, time scaling |
| [`src/experiments/`](src/experiments/README.md) | The experiments, planet and moon data, starting cameras, command-line arguments |
| [`src/spacecraft/`](src/spacecraft/README.md) | The craft, flight planning, steering and landing, the flight report |
| [`src/render/`](src/render/README.md) | The Scene→ECS mirror, the frame pipeline, lighting, eclipse shadows, the grid, trails, missions |
| [`src/input/`](src/input/README.md) | The camera, what each key does, input modes |
| [`src/ui/`](src/ui/README.md) | On-screen panels: the overlay, crosshair, compass, menu, picker, arrival pop-up |
| [`src/physics_utils/`](src/physics_utils/README.md) | Physical constants and Newtonian formulas |
| [`src/physics_utils/relativity/`](src/physics_utils/relativity/README.md) | Special-relativity formulas and `c` |
| [`src/performance/`](src/performance/README.md) | The `RGA_*` frame-time switches and the benchmark |
| [`src/config/`](src/config/README.md) | Loading the settings files: what each may contain, its validation, where the directory is found |
| [`config/`](config/README.md) | The settings files themselves: key bindings, fleet, camera, UI sizes, trails, performance. Edit and relaunch, or save from the menu |
| [`assets/`](assets/README.md) | The eclipse-shadow shader |
| [`tst/`](tst/README.md) | How tests are laid out and wired in. `tst/<layer>/` holds the tests for `src/<layer>/` and has its own `README.md` |

To print every directory's summary at once (the whole index, without opening any file in full):

```bash
for f in $(git ls-files '*README.md' | grep -v '^README.md$'); do
  awk 'NR==1 {print; next} /^>/ {print; seen=1; next} seen {exit}' "$f"; echo
done
```

The history of changes is in [`CHANGELOG.md`](CHANGELOG.md). Contribution rules are in
[`CONTRIBUTING.md`](CONTRIBUTING.md) and [`.claude/steering/`](.claude/steering/).

## Troubleshooting

- **The window fails to open, or you get a wgpu backend error:** Bevy prints the cause on stderr.
  On Linux, check Vulkan with `vulkaninfo` and make sure X11 or Wayland is running. On Windows,
  update your GPU drivers.
- **Mouse-look stops responding:** the window has probably lost focus. Switch back to it and
  left-click.
