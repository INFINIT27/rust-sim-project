//! Tests for the GPU pass timer's sums and formatting. Its GPU side is tested in
//! `particles_tests.rs`, with the stepper it times.

use super::*;

#[test]
fn the_timer_is_off_unless_asked_for() {
    assert!(!gpu_profile_enabled_by(None));
    assert!(!gpu_profile_enabled_by(Some("")));
    assert!(!gpu_profile_enabled_by(Some("off")));
    assert!(!gpu_profile_enabled_by(Some("maybe")));
    for on in ["on", "1", "true", "YES", " On "] {
        assert!(gpu_profile_enabled_by(Some(on)), "{on}");
    }
}

#[test]
fn every_phase_has_its_own_index_and_name() {
    for (i, phase) in GpuPhase::ALL.into_iter().enumerate() {
        assert_eq!(phase.index(), i);
    }
    let mut names: Vec<_> = GpuPhase::ALL.iter().map(|phase| phase.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), GPU_PHASES);
}

#[test]
fn a_frame_s_spans_are_summed_by_phase_and_its_span_runs_first_to_last() {
    let mut totals = Totals::default();
    // Two ticks of 2 ns a tick: drift 10..20 and 40..45, kick 25..35. First 10, last 45.
    let phases = [GpuPhase::Drift, GpuPhase::Kick, GpuPhase::Drift];
    let stamps = [10, 20, 25, 35, 40, 45];
    totals.add_frame(&phases, &stamps, 2.0, 3);
    let profile = totals.profile();
    assert_eq!(profile.frames, 1);
    assert!((profile.phase(GpuPhase::Drift) - 30e-6).abs() < 1e-15);
    assert!((profile.phase(GpuPhase::Kick) - 20e-6).abs() < 1e-15);
    assert!((profile.span_ms - 70e-6).abs() < 1e-15);
    assert!((profile.passes_ms() - 50e-6).abs() < 1e-15);
    assert!((profile.idle_ms() - 20e-6).abs() < 1e-15);
    assert_eq!(profile.untimed_substeps, 3.0);
}

#[test]
fn means_are_per_frame_and_per_advance() {
    let mut totals = Totals::default();
    totals.add_frame(&[GpuPhase::Sort], &[1, 1_000_001], 1.0, 0);
    totals.add_frame(&[GpuPhase::Sort], &[1, 3_000_001], 1.0, 0);
    // 100 particles: 10 ticks with 50 active in all, then 30 ticks with 150.
    totals.add_advance(12, 10, 50, 100);
    totals.add_advance(40, 30, 150, 100);
    let profile = totals.profile();
    assert!((profile.phase(GpuPhase::Sort) - 2.0).abs() < 1e-12);
    assert_eq!(profile.advances, 2);
    assert_eq!(profile.queued_substeps, 26.0);
    assert_eq!(profile.ticks, 20.0);
    assert!((profile.active_fraction - 200.0 / 4000.0).abs() < 1e-15);
}

#[test]
fn an_unwritten_timestamp_counts_as_nothing() {
    let mut totals = Totals::default();
    // A begin of zero, and an end before its begin.
    totals.add_frame(&[GpuPhase::Drift, GpuPhase::Kick], &[0, 50, 90, 80], 1.0, 0);
    let profile = totals.profile();
    assert_eq!(profile.passes_ms(), 0.0);
    assert_eq!(profile.span_ms, 0.0);
    assert_eq!(profile.frames, 1);
}

#[test]
fn nothing_measured_reads_as_zero() {
    let profile = Totals::default().profile();
    assert_eq!(profile, GpuProfile::default());
    assert_eq!(profile.idle_ms(), 0.0);
}

#[test]
fn the_overlay_lists_the_phases_that_took_time_longest_first() {
    let mut profile = GpuProfile {
        frames: 30,
        span_ms: 10.0,
        advances: 30,
        queued_substeps: 8.0,
        ticks: 6.0,
        active_fraction: 0.25,
        ..GpuProfile::default()
    };
    profile.phase_ms[GpuPhase::Drift.index()] = 1.0;
    profile.phase_ms[GpuPhase::TreeWalk.index()] = 6.0;
    profile.phase_ms[GpuPhase::Sort.index()] = 2.0;
    let lines = profile.lines();
    assert!(lines[0].contains("9.0 ms in passes") && lines[0].contains("1.0 ms between"));
    assert!(lines[1].contains("ticks 6.0 of 8.0") && lines[1].contains("25.0%"));
    let order: Vec<&str> = lines[2..]
        .iter()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(order, ["tree", "sort", "drift"]);
}

#[test]
fn the_benchmark_line_names_every_phase() {
    let line = GpuProfile::default().to_string();
    assert!(line.starts_with("GPU_PROFILE "));
    for phase in GpuPhase::ALL {
        assert!(line.contains(&format!(" {}_ms=", phase.name())), "{line}");
    }
    assert!(line.contains(" active_pct="));
}
