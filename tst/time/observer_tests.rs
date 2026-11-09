//! Unit tests for the [`crate::time::observer`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/time/observer.rs`
//! via `#[path]`, not as a standalone integration test.
//!
//! The observer clock is small, and its whole reason to exist is the distinction
//! between its two readings — raw for FPS, capped for motion — so that is what most
//! of these test. The FPS cases came from `ui/overlay.rs`'s old private
//! `fps_from_dt` helper, which this type replaced.

use super::*;

#[test]
fn a_new_clock_starts_at_zero() {
    let clock = ObserverClock::new();

    assert_eq!(clock.elapsed_seconds(), 0.0);
    assert_eq!(clock.raw_step_seconds(), 0.0);
    assert_eq!(clock.fps(), 0.0);
    assert!(clock.is_valid());
    assert_eq!(clock, ObserverClock::default());
}

#[test]
fn advancing_records_the_delta_and_accumulates_real_time() {
    let mut clock = ObserverClock::new();

    // Binary-exact frames (1/64 and 1/32 s) so the elapsed assertion can be exact:
    // `0.02f32 as f64` is not `0.02f64`, and a test that ignores that fails for
    // reasons that have nothing to do with the clock.
    clock.advance(1.0 / 64.0);
    assert_eq!(clock.raw_step_seconds(), 0.015625);
    assert_eq!(clock.elapsed_seconds(), 0.015625);

    clock.advance(1.0 / 32.0);
    assert_eq!(clock.raw_step_seconds(), 0.03125);
    assert_eq!(clock.elapsed_seconds(), 0.046875);
}

#[test]
fn a_stall_is_honest_for_fps_and_capped_for_motion() {
    // The two readings exist for exactly this case: the FPS row must show that a
    // frame took half a second, and the camera must not move as if it had.
    let mut clock = ObserverClock::new();
    clock.advance(0.5);

    assert_eq!(clock.raw_step_seconds(), 0.5);
    assert_eq!(clock.motion_step_seconds(), MAX_REAL_STEP_SECONDS);
    assert_eq!(clock.fps(), 2.0);
}

#[test]
fn a_normal_frame_reads_the_same_both_ways() {
    let mut clock = ObserverClock::new();
    clock.advance(1.0 / 60.0);

    assert_eq!(clock.motion_step_seconds(), clock.raw_step_seconds());
    assert!((clock.fps() - 60.0).abs() < 1.0e-3);
}

#[test]
fn an_unusable_delta_is_sanitised_to_zero() {
    let mut clock = ObserverClock::new();

    clock.advance(f32::NAN);
    assert_eq!(clock.raw_step_seconds(), 0.0);
    clock.advance(-1.0);
    assert_eq!(clock.raw_step_seconds(), 0.0);
    clock.advance(f32::INFINITY);
    assert_eq!(clock.raw_step_seconds(), 0.0);

    assert_eq!(clock.elapsed_seconds(), 0.0);
    assert!(clock.is_valid());
}

#[test]
fn fps_is_zero_rather_than_infinite_on_the_first_tick() {
    // Bevy reports `delta_seconds() == 0.0` before a real frame has elapsed. One
    // frame of `0.0` in a readout beats one frame of `inf`.
    let mut clock = ObserverClock::new();

    clock.advance(0.0);
    assert_eq!(clock.fps(), 0.0);

    // And the same for a delta too small to invert meaningfully.
    clock.advance(1.0e-7);
    assert_eq!(clock.fps(), 0.0);
}

#[test]
fn the_observer_clock_is_independent_of_the_simulation_clock() {
    // The property the whole two-clock split exists for: nothing about pausing or
    // rescaling the experiment can touch the observer's time, because they share no
    // state at all. Asserted here as a compile-and-run fact rather than left to the
    // README.
    use crate::time::simulation::SimulationClock;

    let mut simulation = SimulationClock::new(1.0e6);
    let mut observer = ObserverClock::new();

    simulation.set_paused(true);

    for _ in 0..5 {
        simulation.advance(1.0 / 64.0);
        observer.advance(1.0 / 64.0);
    }

    assert_eq!(
        simulation.elapsed_seconds(),
        0.0,
        "the experiment is frozen"
    );
    assert_eq!(
        observer.elapsed_seconds(),
        5.0 / 64.0,
        "but the observer keeps moving, which is what lets you fly around it"
    );
}
