//! Unit and property tests for the [`crate::experiments::random`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside
//! `src/experiments/random.rs` via `#[path]`, not as a standalone integration test.
//!
//! These test the three things the generator actually promises — determinism, range,
//! and a coarse spread — and deliberately not randomness in any formal sense. A
//! hand-rolled generator that claimed statistical quality would need a statistical test
//! suite; this one claims only that it is good enough to place three bodies, and that
//! the same seed always places them identically.

// `super::*` is deliberately *not* glob-imported here: `proptest::prelude` re-exports
// a `Rng` trait from `rand`, and two globs offering the same name is an ambiguity
// error. The explicit import names the `Rng` this file is about.
use super::Rng;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Where proptest writes the seeds of failures it has seen. See `tst/README.md`.
const REGRESSION_SEEDS: &str = "tst/experiments/proptest-regressions/random_tests.txt";

#[test]
fn the_same_seed_gives_the_same_sequence() {
    // The property everything reproducible rests on: `--seed 42` must mean one thing.
    let mut a = Rng::new(42);
    let mut b = Rng::new(42);

    for _ in 0..64 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn different_seeds_give_different_sequences() {
    let mut a = Rng::new(1);
    let mut b = Rng::new(2);

    assert_ne!(a.next_u64(), b.next_u64());
}

#[test]
fn a_zero_seed_is_not_degenerate() {
    // SplitMix64 advances by addition, so zero is an ordinary seed — worth pinning,
    // because a generator that multiplies its state would return zeros forever here and
    // a caller who passed `--seed 0` would get a silently broken experiment.
    let mut rng = Rng::new(0);
    let first = rng.next_u64();
    let second = rng.next_u64();

    assert_ne!(first, 0);
    assert_ne!(first, second);
}

#[test]
fn successive_values_differ() {
    let mut rng = Rng::new(12345);
    let mut previous = rng.next_u64();

    for _ in 0..100 {
        let next = rng.next_u64();
        assert_ne!(next, previous, "the generator repeated a value immediately");
        previous = next;
    }
}

#[test]
fn unit_values_stay_in_the_half_open_unit_interval() {
    let mut rng = Rng::new(7);

    for _ in 0..10_000 {
        let value = rng.next_unit_f32();
        assert!((0.0..1.0).contains(&value), "{value} is outside [0.0, 1.0)");
    }
}

#[test]
fn unit_values_are_spread_across_the_interval() {
    // A coarse spread check: ten buckets, ten thousand draws, every bucket hit. This
    // would catch a generator stuck in a corner of the range — the failure mode that
    // matters here — without pretending to be a randomness test.
    let mut rng = Rng::new(99);
    let mut buckets = [0_u32; 10];

    for _ in 0..10_000 {
        let index = (rng.next_unit_f32() * 10.0) as usize;
        buckets[index.min(9)] += 1;
    }

    for (index, count) in buckets.iter().enumerate() {
        assert!(
            *count > 500,
            "bucket {index} got only {count} of 10,000 draws — the spread is broken"
        );
    }
}

#[test]
fn ranged_values_stay_inside_their_range() {
    let mut rng = Rng::new(4);

    for _ in 0..10_000 {
        let value = rng.next_f32_in(0.4, 1.3);
        assert!(
            (0.4..=1.3).contains(&value),
            "{value} is outside [0.4, 1.3]"
        );
    }
}

#[test]
fn a_single_point_range_returns_that_point() {
    let mut rng = Rng::new(5);
    assert_eq!(rng.next_f32_in(2.5, 2.5), 2.5);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "need a finite non-empty range")]
fn a_reversed_range_panics_in_debug() {
    let mut rng = Rng::new(6);
    let _ = rng.next_f32_in(1.0, 0.0);
}

#[cfg(not(debug_assertions))]
#[test]
fn a_reversed_or_non_finite_range_returns_the_low_bound_in_release() {
    // Benign in release, matching the crate's contract: an in-range deterministic
    // answer beats a `NaN` propagating into a scene.
    let mut rng = Rng::new(6);
    assert_eq!(rng.next_f32_in(1.0, 0.0), 1.0);
    assert_eq!(rng.next_f32_in(f32::NAN, 1.0).is_nan(), true);
    assert_eq!(rng.next_f32_in(0.0, f32::INFINITY), 0.0);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_SEEDS))),
        ..ProptestConfig::default()
    })]

    /// Determinism over the whole seed space, not just the seeds someone thought to
    /// type.
    #[test]
    fn any_seed_is_reproducible(seed in any::<u64>()) {
        let mut a = Rng::new(seed);
        let mut b = Rng::new(seed);

        for _ in 0..8 {
            prop_assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    /// A ranged draw is always inside its range and always finite, for any seed and any
    /// sane range — which is what lets the ring builder use it without checking.
    #[test]
    fn ranged_draws_are_always_in_range(
        seed in any::<u64>(),
        low in -1000.0_f32..1000.0,
        width in 0.0_f32..1000.0,
    ) {
        let high = low + width;
        let mut rng = Rng::new(seed);

        for _ in 0..8 {
            let value = rng.next_f32_in(low, high);
            prop_assert!(value.is_finite());
            prop_assert!(value >= low);
            prop_assert!(value <= high);
        }
    }
}
