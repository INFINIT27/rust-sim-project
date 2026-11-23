//! The GPU pass timer: how long each group of the particle kernels takes on the GPU, from timestamp
//! queries.
//!
//! The frame-time benchmark (`crate::performance`) times whole frames on the CPU, which cannot say
//! where a GPU frame goes. With the timer on ([`GPU_PROFILE_ENV`]), [`GpuParticles`] encodes each
//! group of kernels ([`GpuPhase`]) as its own compute pass and has the GPU write a timestamp at the
//! start and the end of each (`wgpu::Features::TIMESTAMP_QUERY`). The timestamps are resolved into a
//! buffer at the end of the frame, read back a frame or two later without waiting, and added up
//! per phase. Frames that start while the last read-back is still in flight are not timed.
//!
//! **What it costs.** Off, nothing: the advance encodes one pass a substep, as before. On, the
//! kernels are split over up to nine passes a substep. `wgpu` already places a barrier between
//! dispatches that write the same storage buffers, so the split adds little, but the times are of
//! the timed encoding.
//!
//! **What it does not see.** Copies between passes (the sort's copy back, the read-backs) and the
//! renderer's draw. Gaps between passes, [`GpuProfile::idle_ms`], are the GPU waiting for the
//! CPU's next submission, or running something else (the draw).
//!
//! [`GpuParticles`]: crate::compute::particles::GpuParticles

use std::fmt;

use crate::compute::constants::{GPU_PROFILE_ENV, GPU_PROFILE_WINDOW_FRAMES};
use crate::compute::particles::{map_async, MapState, Mapping};

/// A group of the advance's kernels that the timer times as one pass, in the order they run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuPhase {
    /// The start of the advance and the Morton sort: `begin`, `bounds`, `morton`, every bitonic
    /// stage, and `gather`.
    Sort,
    /// `restore_sph`, after the sorted particles are copied back.
    Restore,
    /// A tick's `tick` and `drift`: every particle moves.
    Drift,
    /// The tree's boxes and moments, leaves up (`refit_leaves`, `refit_level`).
    Refit,
    /// The cell list: clear, count, prefix sum, scatter.
    Grid,
    /// `activate`, the fluid's density (`density_*`) and `limits_first`.
    Density,
    /// `forces_pairs`: every pair, for direct gravity or the all-pairs neighbour search.
    PairForces,
    /// `gravity_group`: the cooperative Barnes–Hut walk.
    GravityWalk,
    /// `forces_tree`: the tree walk (gravity when not cooperative, and the tree's neighbours).
    TreeWalk,
    /// `forces_grid`: the neighbours from the cell list.
    GridContacts,
    /// `close_kick`, `limits`, `plan` and `open_kick`.
    Kick,
}

/// How many phases there are.
pub const GPU_PHASES: usize = 11;

impl GpuPhase {
    /// Every phase, in the order they run.
    pub const ALL: [Self; GPU_PHASES] = [
        Self::Sort,
        Self::Restore,
        Self::Drift,
        Self::Refit,
        Self::Grid,
        Self::Density,
        Self::PairForces,
        Self::GravityWalk,
        Self::TreeWalk,
        Self::GridContacts,
        Self::Kick,
    ];

    /// A short name, for the overlay and the benchmark's line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sort => "sort",
            Self::Restore => "restore",
            Self::Drift => "drift",
            Self::Refit => "refit",
            Self::Grid => "grid",
            Self::Density => "density",
            Self::PairForces => "pairs",
            Self::GravityWalk => "gravity",
            Self::TreeWalk => "tree",
            Self::GridContacts => "contacts",
            Self::Kick => "kick",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Whether the timer is asked for, by the [`GPU_PROFILE_ENV`] variable: `on`, `1`, `true` or `yes`
/// (in any case). Off when unset.
///
/// # Examples
///
/// ```
/// use physics_sandbox::gpu_profile_enabled_by;
///
/// assert!(!gpu_profile_enabled_by(None));
/// assert!(gpu_profile_enabled_by(Some("ON")));
/// assert!(!gpu_profile_enabled_by(Some("off")));
/// ```
pub fn gpu_profile_enabled_by(value: Option<&str>) -> bool {
    matches!(
        value
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Some("on" | "1" | "true" | "yes")
    )
}

/// [`gpu_profile_enabled_by`] the process environment.
pub fn gpu_profile_enabled() -> bool {
    gpu_profile_enabled_by(std::env::var(GPU_PROFILE_ENV).ok().as_deref())
}

/// Means per timed frame over a run of frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuProfile {
    /// Frames timed.
    pub frames: u32,
    /// Milliseconds of GPU time in each phase, a frame, by [`GpuPhase::index`].
    pub phase_ms: [f64; GPU_PHASES],
    /// Milliseconds from the start of a frame's first timed pass to the end of its last.
    pub span_ms: f64,
    /// Substeps a frame that ran untimed, past the query set's room.
    pub untimed_substeps: f64,
    /// Advances whose timing was read back, for the three means below.
    pub advances: u32,
    /// Substeps queued an advance (each its own submission; the ones past the end of the frame's
    /// time return at once, but still cost their dispatches).
    pub queued_substeps: f64,
    /// Ticks that ran an advance.
    pub ticks: f64,
    /// The fraction of the particles active in a tick, over every tick (`activate`'s count).
    pub active_fraction: f64,
}

impl GpuProfile {
    /// Milliseconds a frame in `phase`.
    pub fn phase(&self, phase: GpuPhase) -> f64 {
        self.phase_ms[phase.index()]
    }

    /// Milliseconds a frame inside the timed passes.
    pub fn passes_ms(&self) -> f64 {
        self.phase_ms.iter().sum()
    }

    /// Milliseconds a frame between the timed passes: the GPU waiting for the next submission, or
    /// doing other work (copies, the draw).
    pub fn idle_ms(&self) -> f64 {
        (self.span_ms - self.passes_ms()).max(0.0)
    }

    /// Rows for the overlay: the totals, then each phase that took time, longest first.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "GPU a frame: {:.1} ms in passes, {:.1} ms between ({} frames)",
            self.passes_ms(),
            self.idle_ms(),
            self.frames
        )];
        if self.advances > 0 {
            lines.push(format!(
                "ticks {:.1} of {:.1} queued, {:.1}% of particles active a tick",
                self.ticks,
                self.queued_substeps,
                100.0 * self.active_fraction
            ));
        }
        let mut phases: Vec<GpuPhase> = GpuPhase::ALL
            .into_iter()
            .filter(|&phase| self.phase(phase) > 0.0)
            .collect();
        phases.sort_by(|a, b| self.phase(*b).total_cmp(&self.phase(*a)));
        let passes = self.passes_ms();
        for phase in phases {
            let ms = self.phase(phase);
            lines.push(format!(
                "  {:<9} {:>7.2} ms  {:>3.0}%",
                phase.name(),
                ms,
                if passes > 0.0 {
                    100.0 * ms / passes
                } else {
                    0.0
                }
            ));
        }
        if self.untimed_substeps > 0.0 {
            lines.push(format!(
                "  ({:.1} substeps a frame untimed)",
                self.untimed_substeps
            ));
        }
        lines
    }
}

/// One line, `GPU_PROFILE key=value ...`, as the benchmark prints it beside its `BENCH` line.
impl fmt::Display for GpuProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GPU_PROFILE frames={} span_ms={:.3} passes_ms={:.3} idle_ms={:.3}",
            self.frames,
            self.span_ms,
            self.passes_ms(),
            self.idle_ms()
        )?;
        for phase in GpuPhase::ALL {
            write!(f, " {}_ms={:.3}", phase.name(), self.phase(phase))?;
        }
        write!(
            f,
            " untimed_substeps={:.1} queued_substeps={:.1} ticks={:.1} active_pct={:.2}",
            self.untimed_substeps,
            self.queued_substeps,
            self.ticks,
            100.0 * self.active_fraction
        )
    }
}

/// Sums over timed frames, from which [`GpuProfile`]'s means are taken.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Totals {
    frames: u32,
    phase_ns: [f64; GPU_PHASES],
    span_ns: f64,
    untimed: u64,
    advances: u32,
    queued: f64,
    ticks: f64,
    active: f64,
    particle_ticks: f64,
}

impl Totals {
    /// Add one frame: its spans' phases in order, and their timestamps, a begin and an end each, in
    /// ticks of `period_ns` nanoseconds. A span whose end is before its begin (a timestamp the
    /// driver did not write) counts as nothing.
    pub(crate) fn add_frame(
        &mut self,
        phases: &[GpuPhase],
        stamps: &[u64],
        period_ns: f64,
        untimed: u32,
    ) {
        let mut first = u64::MAX;
        let mut last = 0;
        for (phase, pair) in phases.iter().zip(stamps.chunks_exact(2)) {
            let (begin, end) = (pair[0], pair[1]);
            if end < begin || begin == 0 {
                continue;
            }
            self.phase_ns[phase.index()] += (end - begin) as f64 * period_ns;
            first = first.min(begin);
            last = last.max(end);
        }
        if last > first {
            self.span_ns += (last - first) as f64 * period_ns;
        }
        self.frames += 1;
        self.untimed += u64::from(untimed);
    }

    /// Add one advance's timing: substeps queued, ticks run, and particles active summed over the
    /// ticks, of `count` particles.
    pub(crate) fn add_advance(&mut self, queued: u32, ticks: u32, active: u32, count: usize) {
        self.advances += 1;
        self.queued += f64::from(queued);
        self.ticks += f64::from(ticks);
        self.active += f64::from(active);
        self.particle_ticks += f64::from(ticks) * count as f64;
    }

    /// The means.
    pub(crate) fn profile(&self) -> GpuProfile {
        let frames = f64::from(self.frames.max(1));
        let advances = f64::from(self.advances.max(1));
        let mut phase_ms = [0.0; GPU_PHASES];
        for (ms, ns) in phase_ms.iter_mut().zip(self.phase_ns) {
            *ms = ns * 1e-6 / frames;
        }
        GpuProfile {
            frames: self.frames,
            phase_ms,
            span_ms: self.span_ns * 1e-6 / frames,
            untimed_substeps: self.untimed as f64 / frames,
            advances: self.advances,
            queued_substeps: self.queued / advances,
            ticks: self.ticks / advances,
            active_fraction: if self.particle_ticks > 0.0 {
                self.active / self.particle_ticks
            } else {
                0.0
            },
        }
    }
}

/// The most passes one frame can time: two timestamps each, in one query set of `wgpu`'s largest.
pub(crate) const MAX_SPANS: usize = wgpu::QUERY_SET_MAX_QUERIES as usize / 2;

/// The timer's query set and read-back, and the sums so far.
pub(crate) struct GpuProfiler {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    state: MapState,
    period_ns: f64,
    /// Whether the frame being encoded is timed, and its spans' phases so far.
    recording: bool,
    spans: Vec<GpuPhase>,
    untimed: u32,
    /// The spans of the frame whose timestamps are being read back.
    in_flight: Vec<GpuPhase>,
    in_flight_untimed: u32,
    /// The last [`GPU_PROFILE_WINDOW_FRAMES`] frames, published as `recent` when full.
    window: Totals,
    recent: GpuProfile,
    /// Everything since [`GpuProfiler::reset`].
    total: Totals,
}

impl GpuProfiler {
    /// A timer on `device`, or `None` when the device cannot write timestamps.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let queries = (2 * MAX_SPANS) as u32;
        let bytes = u64::from(queries) * 8;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("particle pass timer"),
            ty: wgpu::QueryType::Timestamp,
            count: queries,
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle pass timer resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle pass timer read-back"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            query_set,
            resolve,
            readback,
            state: MapState::new(),
            period_ns: f64::from(queue.get_timestamp_period()),
            recording: false,
            spans: Vec::new(),
            untimed: 0,
            in_flight: Vec::new(),
            in_flight_untimed: 0,
            window: Totals::default(),
            recent: GpuProfile::default(),
            total: Totals::default(),
        })
    }

    /// Start timing a frame, unless the last frame's timestamps are still being read back.
    pub(crate) fn begin_frame(&mut self) {
        self.poll();
        self.recording = self.state.get() == Mapping::Idle;
        self.spans.clear();
        self.untimed = 0;
    }

    /// Whether `passes` more passes can be timed in this frame.
    pub(crate) fn has_room(&self, passes: usize) -> bool {
        self.recording && self.spans.len() + passes <= MAX_SPANS
    }

    /// The timestamp writes for one more pass of `phase`. Only after [`has_room`](Self::has_room).
    pub(crate) fn span(&mut self, phase: GpuPhase) -> wgpu::ComputePassTimestampWrites<'_> {
        let begin = 2 * self.spans.len() as u32;
        self.spans.push(phase);
        wgpu::ComputePassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        }
    }

    /// Note a substep of a timed frame that ran untimed, past the room.
    pub(crate) fn note_untimed(&mut self) {
        if self.recording {
            self.untimed += 1;
        }
    }

    /// End the frame: resolve its timestamps and start reading them back.
    pub(crate) fn end_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !std::mem::take(&mut self.recording) || self.spans.is_empty() {
            return;
        }
        let queries = 2 * self.spans.len() as u32;
        let bytes = u64::from(queries) * 8;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("particle pass timer resolve"),
        });
        encoder.resolve_query_set(&self.query_set, 0..queries, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, bytes);
        queue.submit(Some(encoder.finish()));
        map_async(&self.readback, bytes, &self.state);
        self.in_flight = std::mem::take(&mut self.spans);
        self.in_flight_untimed = self.untimed;
    }

    /// Take a read-back that has arrived (the caller polls the device).
    pub(crate) fn poll(&mut self) {
        if self.state.get() != Mapping::Ready {
            return;
        }
        let bytes = self.in_flight.len() as u64 * 16;
        let stamps: Vec<u64> = {
            let view = self.readback.slice(..bytes).get_mapped_range();
            view.chunks_exact(8)
                .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8])))
                .collect()
        };
        self.readback.unmap();
        self.state.set(Mapping::Idle);
        for totals in [&mut self.window, &mut self.total] {
            totals.add_frame(
                &self.in_flight,
                &stamps,
                self.period_ns,
                self.in_flight_untimed,
            );
        }
        self.in_flight.clear();
        if self.window.frames >= GPU_PROFILE_WINDOW_FRAMES {
            self.recent = std::mem::take(&mut self.window).profile();
        }
    }

    /// Add an advance's timing read back from the GPU (see [`Totals::add_advance`]).
    pub(crate) fn add_advance(&mut self, queued: u32, ticks: u32, active: u32, count: usize) {
        for totals in [&mut self.window, &mut self.total] {
            totals.add_advance(queued, ticks, active, count);
        }
    }

    /// The means over the last full window of frames.
    pub(crate) fn recent(&self) -> &GpuProfile {
        &self.recent
    }

    /// The means since [`reset`](Self::reset).
    pub(crate) fn total(&self) -> GpuProfile {
        self.total.profile()
    }

    /// Start the running sums again.
    pub(crate) fn reset(&mut self) {
        self.total = Totals::default();
    }
}

#[cfg(test)]
#[path = "../../tst/compute/profiler_tests.rs"]
mod tests;
