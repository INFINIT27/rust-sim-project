//! The frame-time benchmark: warm up, record every frame's wall-clock length, print one summary
//! line, and exit.
//!
//! The line starts with `BENCH ` so scripts can grep it out of Bevy's log. `src/performance/scripts/`
//! has the runners that collect it.

use std::time::Instant;

use bevy::prelude::*;

use crate::performance::settings::BenchConfig;

#[derive(Resource)]
pub(crate) struct BenchRun {
    config: BenchConfig,
    started: Option<Instant>,
    last: Option<Instant>,
    frames_ms: Vec<f64>,
    reported: bool,
}

impl BenchRun {
    pub(crate) fn new(config: BenchConfig) -> Self {
        Self {
            config,
            started: None,
            last: None,
            frames_ms: Vec::new(),
            reported: false,
        }
    }
}

/// Frame-time summary of a finished run, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BenchSummary {
    pub frames: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

impl BenchSummary {
    /// `None` for an empty run, which has no percentiles.
    pub fn from_frames(frames_ms: &[f64]) -> Option<Self> {
        if frames_ms.is_empty() {
            return None;
        }
        let mut sorted = frames_ms.to_vec();
        sorted.sort_by(f64::total_cmp);
        let at = |p: f64| sorted[((sorted.len() - 1) as f64 * p) as usize];
        Some(Self {
            frames: sorted.len(),
            mean_ms: sorted.iter().sum::<f64>() / sorted.len() as f64,
            p50_ms: at(0.5),
            p95_ms: at(0.95),
            p99_ms: at(0.99),
            max_ms: sorted[sorted.len() - 1],
        })
    }
}

impl std::fmt::Display for BenchSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BENCH frames={} mean_ms={:.3} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} max_ms={:.3} fps={:.1}",
            self.frames,
            self.mean_ms,
            self.p50_ms,
            self.p95_ms,
            self.p99_ms,
            self.max_ms,
            1e3 / self.mean_ms
        )
    }
}

/// Runs in `Last`, so a frame's length is measured between the same point of consecutive frames.
pub(crate) fn record_frame_system(mut run: ResMut<BenchRun>, mut exit: EventWriter<AppExit>) {
    if run.reported {
        return;
    }

    let now = Instant::now();
    let started = *run.started.get_or_insert(now);
    let elapsed = now - started;

    if elapsed > run.config.warmup {
        if let Some(last) = run.last {
            run.frames_ms.push((now - last).as_secs_f64() * 1e3);
        }
    }
    run.last = Some(now);

    if elapsed > run.config.warmup + run.config.window {
        if let Some(summary) = BenchSummary::from_frames(&run.frames_ms) {
            println!("{summary}");
            run.reported = true;
            exit.send(AppExit::Success);
        }
    }
}

#[cfg(test)]
#[path = "../../tst/performance/bench_tests.rs"]
mod tests;
