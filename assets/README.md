# `assets/` — the one thing that is not Rust

> **Purpose:** The only runtime file that is not Rust: the fragment shader that draws eclipse
> shadows.
> **Edit here to:** change how eclipse shadows look on the GPU, and make the same change to
> `star_visible_fraction` in `src/render/eclipse.rs`.
> **Not here:** the material and the CPU copy of the shadow math, in
> [`src/render/`](../src/render/README.md). The floor-eclipse switch is in
> [`src/performance/`](../src/performance/README.md).

Bevy resolves `ShaderRef::Path` against this directory, and it exists for exactly one file:

| File | Job |
| --- | --- |
| `shaders/eclipse.wgsl` | The fragment shader that draws eclipse shadows. Mirrors `star_visible_fraction` in [`src/render/eclipse.rs`](../src/render/eclipse.rs). |

## Why there is a shader at all

Shadows here cannot come from a shadow map, and the numbers are not close. A Bevy point light casts
through a cube map whose face spans 90°, so a 1024² map resolves `1.53e-3` radians per texel — which
at Jupiter's distance from the Sun is **17× Jupiter's whole radius**, and 656× Io's. At 4096² it is
still 4.3× Jupiter's radius. Resolving Io's shadow would need about **671,000 texels per cube face**.

So the shadow is *computed* rather than sampled. Every body is a sphere and there is one star, so "how
much of the star can this point see" has a closed form: the overlap area of two discs on the sky. That
is exact at any scale, has no resolution limit, and costs a few instructions per occluder.

## The rule for changing it

**The arithmetic exists twice** — once in Rust as `star_visible_fraction`, where it can be tested, and
once here, where it runs. That is a real hazard, and it is managed rather than avoided:

- The Rust version is the reference. Change it first, and let its tests tell you the answer.
- `eclipse_tests::the_shader_mirrors_the_reference_thresholds` reads this file as **text** and asserts
  its four case boundaries and its two shared constants are the ones Rust uses. It catches a threshold
  edited on one side only, which is the divergence that would actually happen.
- It cannot catch the shader being wrong in a way the source text does not show. **A shader change is
  not verified until someone has looked at it** — `cargo run --release -- solar-system`, fly to
  Jupiter, and watch a moon cross its disc.

## Failure modes worth knowing

A missing or misnamed shader is a **runtime** error, not a compile error: the app logs an
asset-not-found and the bodies draw wrongly or not at all. A WGSL syntax error or a bind-group
mismatch is also runtime, and Bevy logs it loudly — so `cargo run` with the log in view is part of
verifying any change here, and a silent log is the first piece of evidence that a change is sound.

`MAX_OCCLUDERS` and the `array<vec4<f32>, 16>` length are written here **and** in
`MAX_ECLIPSE_OCCLUDERS`, because WGSL cannot import a Rust constant. A mismatch either truncates
silently or overflows the uniform, so a test pins the pair.

## Getting the path right

Bevy resolves this directory **relative to the executable**, not to the working directory. So
`./target/release/physics-sandbox` looks for `target/release/assets/` and finds nothing, while
`cargo run` works because it sets the working directory to the crate root. Every shader load in this
project went through `cargo run` for its whole life, so that gap stayed hidden until someone ran the
binary directly.

`point_bevy_at_the_assets_directory` in `src/main.rs` closes it by setting `BEVY_ASSET_ROOT` before the
app is built. Two things follow for anyone adding an asset:

- **Test by running the binary, not just `cargo run`.** A missing asset is an `ERROR` log line and a
  material that still draws — the window does not tell you, and neither does the test suite.
- **A binary copied out of this tree falls back to exe-relative lookup**, which is correct: the
  override only applies when the compile-time source directory still exists and still has an `assets/`
  in it. Ship the directory next to the executable.
