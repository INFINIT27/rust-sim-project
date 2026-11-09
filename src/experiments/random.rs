//! A small deterministic random-number generator, for randomised initial conditions.
//!
//! [`Rng`] is SplitMix64: a 64-bit counter advanced by a fixed increment and passed
//! through an avalanche of shifts and multiplies. It is the generator Java's
//! `SplittableRandom` and Rust's `rand` crate both use to seed other generators, and
//! it is about ten lines.
//!
//! # Why not the `rand` crate
//!
//! The crate's dependency list is three entries long (`bevy`, `bevy_egui`, and
//! `proptest` for tests), and every one of them is there because it does something
//! that would be unreasonable to write. A uniform `f32` in a range is not that. Ten
//! lines with a cited algorithm and a test that the distribution is not obviously
//! broken is a smaller liability than a dependency, at this size.
//!
//! That reasoning inverts the moment anything here needs a *distribution* — normal,
//! Poisson, sampling without replacement — or cryptographic quality. At that point
//! take `rand`; do not grow this file into a statistics library.
//!
//! # What it guarantees, and what it does not
//!
//! - **Deterministic.** The same seed yields the same sequence, on every platform and
//!   every run. That is what makes a randomised experiment reproducible: pass
//!   `--seed 42` twice and you get the same scene, which is the difference between an
//!   observation and an anecdote.
//! - **Not cryptographic.** The state is recoverable from a couple of outputs. Nothing
//!   here is a secret, and nothing here should ever become one.
//! - **Not a statistical instrument.** It is uniform enough to place three bodies. It
//!   has not been run through a test suite like TestU01, and the tests here check
//!   determinism, range, and a coarse spread — not randomness in any formal sense.

/// A deterministic SplitMix64 generator.
///
/// # Examples
///
/// ```
/// use physics_sandbox::Rng;
///
/// // The same seed always gives the same sequence.
/// let mut a = Rng::new(42);
/// let mut b = Rng::new(42);
/// assert_eq!(a.next_u64(), b.next_u64());
///
/// // Values land inside the requested range.
/// let mut rng = Rng::new(7);
/// for _ in 0..100 {
///     let value = rng.next_f32_in(0.4, 1.3);
///     assert!((0.4..=1.3).contains(&value));
/// }
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

/// The SplitMix64 increment: the odd 64-bit approximation of `2^64 / φ`.
///
/// **Not tunable.** It is part of the published algorithm, in the same category as the
/// `0.5` in `½·m·v²`: changing it does not configure the generator, it makes it a
/// different — and unstudied — one. That is why it stays here rather than moving to
/// `constants.rs`, and why it carries the marker the constants audit looks for. See
/// `.claude/steering/constants.md`, "Algorithm and format constants".
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15; // ALGORITHM CONSTANT: SplitMix64

impl Rng {
    /// A generator seeded with `seed`.
    ///
    /// Every seed is valid, including zero: SplitMix64 has no degenerate seeds,
    /// because the state is advanced by addition rather than by a multiply that could
    /// map zero to zero.
    #[inline]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 bits.
    ///
    /// Advances the counter, then avalanches it. The shift and multiply constants are
    /// the published SplitMix64 values; they are not tunable, and changing one makes
    /// this a different generator rather than a configured one.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);

        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next value as an `f32` in `[0.0, 1.0)`.
    ///
    /// Built from the top 24 bits, which is exactly `f32`'s mantissa width — taking
    /// the low bits instead would waste the generator's best-mixed output and, for
    /// some generators, its worst-mixed.
    #[inline]
    pub fn next_unit_f32(&mut self) -> f32 {
        // 2^-24, so the result is in [0, 1) with every representable step equally
        // likely rather than clustered near zero. **Not tunable**: 24 is `f32`'s
        // mantissa width, a property of the format rather than a choice.
        const SCALE: f32 = 1.0 / (1u32 << 24) as f32; // ALGORITHM CONSTANT: f32 mantissa
        ((self.next_u64() >> 40) as u32) as f32 * SCALE
    }

    /// The next value as an `f32` in `[low, high)`, or exactly `low` when the range is
    /// empty or unusable.
    ///
    /// Returning `low` rather than `NaN` or a panic for a reversed or non-finite range
    /// matches the crate's benign-in-release contract: a caller with a bad range gets a
    /// deterministic, in-range answer, and the `debug_assert!` names the problem in a
    /// debug build.
    #[inline]
    pub fn next_f32_in(&mut self, low: f32, high: f32) -> f32 {
        debug_assert!(
            low.is_finite() && high.is_finite() && low <= high,
            "next_f32_in: need a finite non-empty range, got [{}, {}]",
            low,
            high
        );

        if !low.is_finite() || !high.is_finite() || high <= low {
            return low;
        }

        low + self.next_unit_f32() * (high - low)
    }
}

#[cfg(test)]
#[path = "../../tst/experiments/random_tests.rs"]
mod tests;
