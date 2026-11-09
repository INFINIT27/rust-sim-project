//! Unit tests for [`crate::performance::bench`].

use super::*;

#[test]
fn an_empty_run_has_no_summary() {
    assert_eq!(BenchSummary::from_frames(&[]), None);
}

#[test]
fn the_summary_is_order_independent_and_uses_nearest_lower_rank() {
    let frames: Vec<f64> = (1..=100).rev().map(f64::from).collect();
    let summary = BenchSummary::from_frames(&frames).unwrap();
    assert_eq!(summary.frames, 100);
    assert_eq!(summary.mean_ms, 50.5);
    assert_eq!(summary.p50_ms, 50.0);
    assert_eq!(summary.p95_ms, 95.0);
    assert_eq!(summary.p99_ms, 99.0);
    assert_eq!(summary.max_ms, 100.0);
}

#[test]
fn the_report_line_is_what_the_scripts_grep_for() {
    let line = BenchSummary::from_frames(&[4.0, 4.0]).unwrap().to_string();
    assert_eq!(
        line,
        "BENCH frames=2 mean_ms=4.000 p50_ms=4.000 p95_ms=4.000 p99_ms=4.000 max_ms=4.000 fps=250.0"
    );
}
