//! Every constant the `performance` layer reads.
//!
//! Per `.claude/steering/constants.md` this is the layer's only constants module. Each default's
//! doc says what was measured to choose it.

use crate::performance::settings::AntiAliasing;

/// Anti-aliasing the app ships with: `4×` MSAA.
///
/// **Why this value.** It is the look every scene was tuned against, and switching away from it
/// changes how edges and the `6`-pixel trail lines render. It is also the most expensive option
/// measured: at full screen resolution, SMAA cuts experiment_3's frame time by `22%` and
/// experiment_1's by `27%`, FXAA by `27%` and `38%`. The extra cost is mostly the trails' transparent
/// pass, which is free without MSAA and about `1.5 ms` with it. Whether the cheaper look is
/// acceptable is a visual call, so the default stays until someone makes it.
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::default`], and through it
/// `RGA_AA` when unset.
///
/// **If you change it.** Every scene changes look. `Smaa` is the recommended replacement; `Fxaa` is
/// faster and blurrier.
pub const DEFAULT_ANTI_ALIASING: AntiAliasing = AntiAliasing::Msaa4;

/// Whether the floor slab computes eclipse shadows by default: yes.
///
/// **Why this value.** Physical fidelity is the project's rule, and the floor *can* be shadowed:
/// over two simulated years of experiment_3 the darkest floor point is a `1.3%` dimming from
/// Jupiter's penumbra about `5,400` units out, about three 8-bit steps. Turning it off cuts
/// experiment_3's frame time by `34%`, because the floor is nearly every shaded pixel and the
/// eclipse loop is the largest per-pixel cost left. The measurement is reproducible with
/// `cargo test --release -- --ignored darkest_floor_eclipse`.
///
/// **Used by.** [`crate::render::plugin::update_eclipse_occluders_system`], via
/// [`crate::performance::settings::PerformanceSettings::floor_eclipse`].
///
/// **If you change it.** Only the floor loses its shadows; every sphere still casts and receives
/// them.
pub const DEFAULT_FLOOR_ECLIPSE: bool = true;

/// Whether presentation waits for the display's refresh by default: yes.
///
/// **Why this value.** Rendering past the refresh rate burns power and heat for frames nobody
/// sees. The benchmark turns it off, because a capped frame rate measures the display rather than
/// the app.
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::present_mode`].
///
/// **If you change it.** The app renders as fast as it can, typically `200` to `500` fps, and the
/// fans notice. Windowed mode on macOS is capped by the compositor regardless.
pub const DEFAULT_VSYNC: bool = true;

/// Environment variable selecting the anti-aliasing mode: `msaa4`, `msaa2`, `smaa`, `fxaa`, `off`.
///
/// **Why this value.** The `RGA_` prefix keeps the app's switches apart from Bevy's own
/// (`BEVY_ASSET_ROOT`) and wgpu's (`WGPU_BACKEND`).
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::from_lookup`].
///
/// **If you change it.** Update `src/performance/README.md` and the scripts in
/// `src/performance/scripts/`, which set it by name.
pub const ANTI_ALIASING_VAR: &str = "RGA_AA";

/// Environment variable toggling the floor's eclipse shadows: `on` or `off`.
///
/// **Why this value.** See [`ANTI_ALIASING_VAR`] for the prefix.
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::from_lookup`].
///
/// **If you change it.** Update the README and scripts, as for [`ANTI_ALIASING_VAR`].
pub const FLOOR_ECLIPSE_VAR: &str = "RGA_FLOOR_ECLIPSE";

/// Environment variable toggling vsync: `on` or `off`. Ignored while benchmarking.
///
/// **Why this value.** See [`ANTI_ALIASING_VAR`] for the prefix.
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::from_lookup`].
///
/// **If you change it.** Update the README and scripts, as for [`ANTI_ALIASING_VAR`].
pub const VSYNC_VAR: &str = "RGA_VSYNC";

/// Environment variable that turns the frame-time benchmark on: any value but `off` or `0`.
///
/// **Why this value.** See [`ANTI_ALIASING_VAR`] for the prefix.
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::from_lookup`].
///
/// **If you change it.** `src/performance/scripts/bench.sh` sets it by name.
pub const BENCH_VAR: &str = "RGA_BENCH";

/// Environment variable overriding [`DEFAULT_BENCH_WARMUP_SECONDS`].
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::from_lookup`].
///
/// **If you change it.** Update the README.
pub const BENCH_WARMUP_VAR: &str = "RGA_BENCH_WARMUP";

/// Environment variable overriding [`DEFAULT_BENCH_SECONDS`].
///
/// **Used by.** [`crate::performance::settings::PerformanceSettings::from_lookup`].
///
/// **If you change it.** Update the README.
pub const BENCH_SECONDS_VAR: &str = "RGA_BENCH_SECONDS";

/// Seconds the benchmark runs before it starts recording frames.
///
/// **Why this value.** Shader compilation, asset loading, and the first trail samples all land in
/// the first second or two, and their frames are tens of milliseconds long. Three seconds clears
/// them with margin.
///
/// **Used by.** [`crate::performance::bench`].
///
/// **If you change it.** Too short and the mean includes startup hitches; too long only wastes
/// time.
pub const DEFAULT_BENCH_WARMUP_SECONDS: f64 = 3.0;

/// Seconds of frames the benchmark records before it reports and exits.
///
/// **Why this value.** Ten seconds is `2,000` to `5,000` frames on the shipped scenes, enough for a
/// stable p99. Runs of this length agreed to within `1%` of each other on a quiet machine.
///
/// **Used by.** [`crate::performance::bench`].
///
/// **If you change it.** Shorter runs make p95 and p99 noisy before the mean moves.
pub const DEFAULT_BENCH_SECONDS: f64 = 10.0;
