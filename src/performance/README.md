# `src/performance/` — frame-time switches and the benchmark

> **Purpose:** The `RGA_*` environment switches that trade image quality for frame time
> (anti-aliasing, floor eclipse, vsync), plus the frame-time benchmark and its scripts.
> **Edit here to:** add a switch, change a default, or change how the benchmark measures.
> **Not here:** the eclipse shading the switch controls is in
> [`render/`](../render/README.md) and [`assets/`](../../assets/README.md).

The switches that trade image quality or fidelity for frame time, and the benchmark that measures
them. Everything is chosen by environment variable at launch,
so two configurations can be compared without a rebuild. The API is documented in `mod.rs`.

## What lives here

| File | Job |
| --- | --- |
| `mod.rs` | `PerformancePlugin`: inserts the settings, sets MSAA, adds SMAA/FXAA to cameras, runs the benchmark. `apply_performance_settings_system` re-applies MSAA, the post-process pass, and vsync when the settings change (from the menu's video page). |
| `settings.rs` | `PerformanceSettings` and `AntiAliasing`, parsed from the environment over a base (`from_env_over`), which `main.rs` takes from `config/performance.toml`; a misspelt value is an error. |
| `bench.rs` | The benchmark: warm up, record frame times, print one `BENCH …` line, exit. |
| `constants.rs` | Defaults and variable names, each with the measurement behind it. |
| `scripts/bench.sh` | Runs the benchmark N times per experiment and appends results to a TSV. |
| `scripts/compare.sh` | Interleaved A/B over several configurations, so drift hits all of them equally. |

## The switches

| Variable | Values | Default | What it trades |
| --- | --- | --- | --- |
| `RGA_AA` | `msaa4` `msaa2` `smaa` `fxaa` `off` | `msaa4` | Edge quality against GPU bandwidth. |
| `RGA_FLOOR_ECLIPSE` | `on` `off` | `on` | A faint penumbra on the floor against a third of experiment_3's frame. |
| `RGA_VSYNC` | `on` `off` | `on` | Power against uncapped frame rate. Forced off while benchmarking. |
| `RGA_BENCH` | `on` `off` | `off` | Runs the benchmark instead of an interactive session. |
| `RGA_BENCH_WARMUP` / `RGA_BENCH_SECONDS` | seconds | `3` / `10` | Benchmark timing. |

```sh
RGA_AA=smaa RGA_FLOOR_ECLIPSE=off cargo run --release -- 3     # try a configuration
src/performance/scripts/compare.sh aa 3 3 msaa4: smaa:RGA_AA=smaa fxaa:RGA_AA=fxaa
```

The defaults are the shipped look. The two big wins, cheaper anti-aliasing and no floor eclipse,
change the image, so they stay off until someone has judged the result by eye.

## Rules and invariants

- Settings are parsed once, before the `App` exists, because vsync is fixed when the window is
  created. Nothing reads the environment after that.
- Systems that read `PerformanceSettings` take `Option<Res<…>>` and fall back to `Default`, so a
  headless test app without the plugin behaves like the shipped one.
- A misspelt value exits with status 2, like a bad CLI argument.
- Benchmark hygiene: run on a quiet machine (`top -o cpu` first), compare with `compare.sh` rather
  than back to back, and `cargo build --release` first, since `bench.sh` runs
  `target/release/physics-sandbox` and does not build it.
- Bevy 0.14 sometimes deadlocks on exit on macOS after the result has been printed. `bench.sh`
  kills the run and keeps the result.

## Where the tests are

`tst/performance/`, `#[path]`-included from `settings.rs`, `bench.rs`, and `mod.rs`. The floor
switch's effect on the eclipse uniforms is tested in `tst/render/plugin_tests.rs`
(`the_floor_eclipse_switch_silences_only_the_floor`).

## Adding a switch here

Give it a `RGA_` variable and a default in `constants.rs` that says what was measured, parse it in
`PerformanceSettings::from_lookup` with an error for bad values, and add a row to the switch table
above.
