# `tst/performance/` — tests for the performance switches

> **Purpose:** Tests for [`src/performance/`](../../src/performance/README.md): switch
> parsing, the `BENCH` line, and the ignored floor-eclipse measurement behind its default.
> **Edit here to:** add or fix a test of a switch or the benchmark summary.
> **Not here:** the switches and the benchmark, in `src/performance/`.

Mirrors `src/performance/`. Each file is a `#[path]`-included unit-test module compiled inside its
subject, so it uses `super::*` and `crate::…` paths.

## What lives here

| File | Subject | Shape of the tests |
| --- | --- | --- |
| `settings_tests.rs` | `settings.rs` | Parsing through `from_lookup`, never the real environment: defaults, every value, misspellings as errors, benchmark timing, vsync forced off while benchmarking. |
| `bench_tests.rs` | `bench.rs` | The summary statistics and the exact `BENCH` line the scripts grep for. |
| `floor_eclipse_tests.rs` | `mod.rs` | `#[ignore]`d measurement: the darkest eclipse on experiment_3's floor over two simulated years, the figure behind `DEFAULT_FLOOR_ECLIPSE`. |

## Conventions specific to this directory

- **No test touches the process environment.** Tests run in parallel and `std::env::set_var` is
  process-wide, so parsing is tested through `PerformanceSettings::from_lookup` with a map.
- **Frame times are not tested here.** They depend on the machine, so the scripts in
  `src/performance/scripts/` measure them instead.
