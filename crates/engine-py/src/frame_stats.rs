//! 0.5.4 (#116): what a window's frames cost, for the app to read.
//!
//! The live loop times each frame it draws in stages and hands the numbers to
//! a `FrameStats`, which keeps the last few hundred and summarizes them. This
//! module is the pure part (records in, summary out), so what it reports is
//! tested without a window.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// A window's statistics, shared so that a handle on another thread can read
/// them (0.5.4, #135) while the loop writes: every writer is the loop's own
/// thread, one short lock a frame.
pub(crate) type SharedStats = Arc<Mutex<FrameStats>>;

pub(crate) fn lock(stats: &SharedStats) -> MutexGuard<'_, FrameStats> {
    stats.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How many drawn frames are kept for the summary.
pub(crate) const KEPT: usize = 240;

/// What a frame redrew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Redraw {
    /// Nothing changed; the kept frame was presented again (an expose, or
    /// something animating elsewhere).
    Nothing,
    /// The whole window.
    Full,
    /// This many damage rects, together `area` of the window (0 to 1).
    Partial { rects: usize, area: f64 },
}

/// One drawn frame's costs, by stage.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameRecord {
    /// Which drawn frame this was, from 1.
    pub(crate) index: u64,
    /// When it began.
    pub(crate) at: Instant,
    /// Animation ticks and timers.
    pub(crate) tick: Duration,
    /// Layout (and virtual lists' materializing).
    pub(crate) layout: Duration,
    /// Working out what changed (the damage walk).
    pub(crate) prepare: Duration,
    /// Waiting for the swapchain to hand over an image: where a paced loop
    /// waits for the display.
    pub(crate) acquire: Duration,
    /// Building the scene, rendering, and submitting.
    pub(crate) draw: Duration,
    /// Presenting.
    pub(crate) present: Duration,
    /// The whole frame, start to finish.
    pub(crate) total: Duration,
    pub(crate) redraw: Redraw,
    /// How many shader passes ran.
    pub(crate) shader_passes: usize,
    /// How many nodes the tree holds.
    pub(crate) nodes: usize,
    /// The surface's size in pixels.
    pub(crate) size: (u32, u32),
    /// 0.5.4 (#135): how long the GPU took on the frame's main submission,
    /// where the adapter can say. It is read back a few frames later, so a
    /// record has `None` until `FrameStats::set_gpu` fills it in.
    pub(crate) gpu: Option<Duration>,
}

impl FrameRecord {
    /// The app's own cost: everything but the waits for the display
    /// (acquiring an image and presenting it).
    pub(crate) fn cpu(&self) -> Duration {
        self.total.saturating_sub(self.acquire + self.present)
    }
}

/// A figure over a set of frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Spread {
    pub(crate) mean: f64,
    pub(crate) max: f64,
    pub(crate) p95: f64,
}

/// What the kept frames add up to.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Summary {
    pub(crate) count: usize,
    /// Drawn frames a second over the kept span; zero for fewer than two.
    pub(crate) fps: f64,
    pub(crate) total_ms: Spread,
    pub(crate) cpu_ms: Spread,
    pub(crate) tick_ms: f64,
    pub(crate) layout_ms: f64,
    pub(crate) prepare_ms: f64,
    pub(crate) acquire_ms: f64,
    pub(crate) draw_ms: f64,
    pub(crate) present_ms: f64,
    /// Mean GPU time over the kept frames that have it; `None` if none do.
    pub(crate) gpu_ms: Option<f64>,
    pub(crate) redrew_nothing: usize,
    pub(crate) redrew_full: usize,
    pub(crate) redrew_partial: usize,
}

/// A window's frame history.
#[derive(Default)]
pub(crate) struct FrameStats {
    kept: VecDeque<FrameRecord>,
    /// Drawn frames since the window opened (or the last reset).
    drawn: u64,
    /// Passes of the loop that drew nothing: nothing changed, no image taken.
    skipped: u64,
    /// Whether the adapter can time the GPU (it does not change on reset).
    gpu_timing: bool,
    /// 0.5.4 (#135): the last profiled frame's node costs, and which frame.
    profile: Option<(u64, engine_render::FrameProfile)>,
    /// 0.5.4 (#135): a trace being written, and why one stopped on its own.
    trace: Option<crate::trace::Trace>,
    trace_error: Option<String>,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn spread(mut values: Vec<f64>) -> Spread {
    if values.is_empty() {
        return Spread::default();
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.sort_by(|a, b| a.total_cmp(b));
    let max = *values.last().expect("not empty");
    // Nearest-rank 95th percentile.
    let rank = ((values.len() as f64) * 0.95).ceil() as usize;
    let p95 = values[rank.clamp(1, values.len()) - 1];
    Spread { mean, max, p95 }
}

impl FrameStats {
    pub(crate) fn skipped_one(&mut self) {
        self.skipped += 1;
    }

    /// Notes a drawn frame; returns its index.
    pub(crate) fn drew(&mut self, mut record: FrameRecord) -> FrameRecord {
        self.drawn += 1;
        record.index = self.drawn;
        if self.kept.len() == KEPT {
            self.kept.pop_front();
        }
        self.kept.push_back(record);
        if let Some(trace) = &mut self.trace
            && let Err(why) = trace.frame(&record)
        {
            self.trace_failed(why);
        }
        record
    }

    pub(crate) fn drawn(&self) -> u64 {
        self.drawn
    }

    pub(crate) fn skipped(&self) -> u64 {
        self.skipped
    }

    pub(crate) fn last(&self) -> Option<&FrameRecord> {
        self.kept.back()
    }

    pub(crate) fn reset(&mut self) {
        *self = Self {
            gpu_timing: self.gpu_timing,
            // A trace goes on across a reset of the numbers.
            trace: self.trace.take(),
            trace_error: self.trace_error.take(),
            ..Self::default()
        };
    }

    pub(crate) fn set_gpu_timing(&mut self, available: bool) {
        self.gpu_timing = available;
    }

    pub(crate) fn gpu_timing(&self) -> bool {
        self.gpu_timing
    }

    /// Starts a trace on `out`. `false` if one is already running.
    pub(crate) fn start_trace(
        &mut self,
        out: Box<dyn std::io::Write + Send>,
    ) -> std::io::Result<bool> {
        if self.trace.is_some() {
            return Ok(false);
        }
        self.trace = Some(crate::trace::Trace::begin(out)?);
        self.trace_error = None;
        Ok(true)
    }

    /// Ends the trace: the frames it holds, or why it stopped earlier.
    pub(crate) fn stop_trace(&mut self) -> std::io::Result<u64> {
        if let Some(why) = self.trace_error.take() {
            return Err(std::io::Error::other(why));
        }
        match self.trace.take() {
            Some(trace) => trace.finish(),
            None => Ok(0),
        }
    }

    /// A write to the trace failed: it stops, and `stop_trace` says why.
    fn trace_failed(&mut self, why: std::io::Error) {
        self.trace = None;
        self.trace_error = Some(why.to_string());
    }

    pub(crate) fn set_profile(&mut self, frame: u64, profile: engine_render::FrameProfile) {
        self.profile = Some((frame, profile));
    }

    pub(crate) fn profile(&self) -> Option<&(u64, engine_render::FrameProfile)> {
        self.profile.as_ref()
    }

    /// The GPU time of frame `index`, once it has been read back. A frame that
    /// has left the kept history, or been reset away, is ignored.
    pub(crate) fn set_gpu(&mut self, index: u64, gpu: Duration) {
        if let Some(record) = self.kept.iter_mut().rev().find(|r| r.index == index) {
            record.gpu = Some(gpu);
            let record = *record;
            if let Some(trace) = &mut self.trace
                && let Err(why) = trace.gpu(&record, gpu)
            {
                self.trace_failed(why);
            }
        }
    }

    pub(crate) fn summary(&self) -> Summary {
        let n = self.kept.len();
        if n == 0 {
            return Summary::default();
        }
        let mean = |f: fn(&FrameRecord) -> Duration| -> f64 {
            self.kept.iter().map(|r| ms(f(r))).sum::<f64>() / n as f64
        };
        let span = match (self.kept.front(), self.kept.back()) {
            (Some(first), Some(last)) => last.at.saturating_duration_since(first.at),
            _ => Duration::ZERO,
        };
        let fps = if n >= 2 && span > Duration::ZERO {
            (n - 1) as f64 / span.as_secs_f64()
        } else {
            0.0
        };
        let (mut nothing, mut full, mut partial) = (0, 0, 0);
        for r in &self.kept {
            match r.redraw {
                Redraw::Nothing => nothing += 1,
                Redraw::Full => full += 1,
                Redraw::Partial { .. } => partial += 1,
            }
        }
        let gpus: Vec<f64> = self.kept.iter().filter_map(|r| r.gpu).map(ms).collect();
        let gpu_ms = (!gpus.is_empty()).then(|| gpus.iter().sum::<f64>() / gpus.len() as f64);
        Summary {
            gpu_ms,
            count: n,
            fps,
            total_ms: spread(self.kept.iter().map(|r| ms(r.total)).collect()),
            cpu_ms: spread(self.kept.iter().map(|r| ms(r.cpu())).collect()),
            tick_ms: mean(|r| r.tick),
            layout_ms: mean(|r| r.layout),
            prepare_ms: mean(|r| r.prepare),
            acquire_ms: mean(|r| r.acquire),
            draw_ms: mean(|r| r.draw),
            present_ms: mean(|r| r.present),
            redrew_nothing: nothing,
            redrew_full: full,
            redrew_partial: partial,
        }
    }
}

/// 0.5.4 (#135): the way to read a window's statistics from another thread.
/// `Window.stats_handle()` makes one on the loop's thread; any thread can then
/// call `read()`, which does not wait for the loop (the statistics are shared
/// and each frame takes one short lock to add itself). Like `LoopHandle` it is
/// the one kind of `tre` object a background thread may hold.
#[pyclass(frozen, name = "StatsHandle")]
pub struct StatsHandle {
    stats: SharedStats,
}

impl StatsHandle {
    pub(crate) fn new(stats: SharedStats) -> Self {
        Self { stats }
    }
}

#[pymethods]
impl StatsHandle {
    /// The same dict as `Window.frame_stats()`, except that `profile` is always
    /// `None` here: it names `Node`s, which belong to the loop's thread (read it
    /// there). `reset=True` clears the history after reading it.
    #[pyo3(signature = (reset=false))]
    fn read<'py>(&self, py: Python<'py>, reset: bool) -> PyResult<Bound<'py, PyDict>> {
        let mut stats = lock(&self.stats);
        let dict = stats_dict(py, &stats)?;
        if reset {
            stats.reset();
        }
        Ok(dict)
    }
}

/// A frame's costs as a Python dict, in milliseconds.
pub(crate) fn record_dict<'py>(py: Python<'py>, r: &FrameRecord) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("frame", r.index)?;
    d.set_item("tick_ms", ms(r.tick))?;
    d.set_item("layout_ms", ms(r.layout))?;
    d.set_item("prepare_ms", ms(r.prepare))?;
    d.set_item("acquire_ms", ms(r.acquire))?;
    d.set_item("draw_ms", ms(r.draw))?;
    d.set_item("present_ms", ms(r.present))?;
    d.set_item("total_ms", ms(r.total))?;
    d.set_item("cpu_ms", ms(r.cpu()))?;
    d.set_item("gpu_ms", r.gpu.map(ms))?;
    let (redraw, rects, area) = match r.redraw {
        Redraw::Nothing => ("nothing", 0, 0.0),
        Redraw::Full => ("full", 1, 1.0),
        Redraw::Partial { rects, area } => ("partial", rects, area),
    };
    d.set_item("redraw", redraw)?;
    d.set_item("damage_rects", rects)?;
    d.set_item("damage_area", area)?;
    d.set_item("shader_passes", r.shader_passes)?;
    d.set_item("nodes", r.nodes)?;
    d.set_item("width", r.size.0)?;
    d.set_item("height", r.size.1)?;
    Ok(d)
}

fn spread_dict<'py>(py: Python<'py>, s: Spread) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("mean", s.mean)?;
    d.set_item("p95", s.p95)?;
    d.set_item("max", s.max)?;
    Ok(d)
}

/// The whole picture as a Python dict: counts, the last frame, and the summary
/// of the kept ones.
pub(crate) fn stats_dict<'py>(py: Python<'py>, stats: &FrameStats) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("frames", stats.drawn())?;
    d.set_item("skipped", stats.skipped())?;
    d.set_item("gpu_timing", stats.gpu_timing())?;
    d.set_item("profile", py.None())?;
    match stats.last() {
        Some(last) => d.set_item("last", record_dict(py, last)?)?,
        None => d.set_item("last", py.None())?,
    }
    let s = stats.summary();
    let recent = PyDict::new(py);
    recent.set_item("count", s.count)?;
    recent.set_item("fps", s.fps)?;
    recent.set_item("total_ms", spread_dict(py, s.total_ms)?)?;
    recent.set_item("cpu_ms", spread_dict(py, s.cpu_ms)?)?;
    recent.set_item("gpu_ms", s.gpu_ms)?;
    let stages = PyDict::new(py);
    stages.set_item("tick", s.tick_ms)?;
    stages.set_item("layout", s.layout_ms)?;
    stages.set_item("prepare", s.prepare_ms)?;
    stages.set_item("acquire", s.acquire_ms)?;
    stages.set_item("draw", s.draw_ms)?;
    stages.set_item("present", s.present_ms)?;
    recent.set_item("stage_ms", stages)?;
    let redraws = PyDict::new(py);
    redraws.set_item("nothing", s.redrew_nothing)?;
    redraws.set_item("full", s.redrew_full)?;
    redraws.set_item("partial", s.redrew_partial)?;
    recent.set_item("redraws", redraws)?;
    d.set_item("recent", recent)?;
    Ok(d)
}

/// The last profiled frame as a Python dict (0.5.4, #135): the frame's number,
/// how many nodes the paint walk reached and the time in them, the time
/// `by_kind` and the `slowest` nodes, each as the `Node` itself while it is
/// still in the tree.
pub(crate) fn profile_dict<'py>(
    py: Python<'py>,
    handles: &crate::window::WindowHandles,
    frame: u64,
    profile: &engine_render::FrameProfile,
) -> PyResult<Bound<'py, PyDict>> {
    use crate::node::{Node, NodeState};
    let d = PyDict::new(py);
    d.set_item("frame", frame)?;
    d.set_item("reached", profile.reached)?;
    d.set_item("ms", ms(profile.time))?;
    let kinds = PyDict::new(py);
    for (kind, cost) in &profile.by_kind {
        let k = PyDict::new(py);
        k.set_item("reached", cost.reached)?;
        k.set_item("drawn", cost.drawn)?;
        k.set_item("ms", ms(cost.time))?;
        kinds.set_item(kind, k)?;
    }
    d.set_item("by_kind", kinds)?;
    let slowest = pyo3::types::PyList::empty(py);
    for cost in &profile.slowest {
        let n = PyDict::new(py);
        let alive = handles.tree.borrow().get(cost.id).is_some();
        if alive {
            n.set_item(
                "node",
                Node::from(NodeState {
                    id: cost.id,
                    tree: handles.tree.clone(),
                    handlers: handles.handlers.clone(),
                    completions: handles.completions.clone(),
                }),
            )?;
        } else {
            n.set_item("node", py.None())?;
        }
        n.set_item("kind", cost.kind)?;
        n.set_item("drawn", cost.drawn)?;
        n.set_item("ms", ms(cost.time))?;
        slowest.append(n)?;
    }
    d.set_item("slowest", slowest)?;
    Ok(d)
}

/// Tells the window's `frame` listener, if it has one, about a drawn frame.
pub(crate) fn announce(
    handles: &crate::window::WindowHandles,
    record: &FrameRecord,
    py: Python<'_>,
) {
    use crate::listeners::{WindowEventType, deliver_window};
    if !handles
        .window_listeners
        .borrow()
        .contains_key(&WindowEventType::Frame)
    {
        return;
    }
    let Ok(stats) = record_dict(py, record) else {
        return;
    };
    let stats = stats.into_any().unbind();
    deliver_window(&handles.window_listeners, py, WindowEventType::Frame, |e| {
        e.stats = Some(stats);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(at: Instant, total_ms: u64, wait_ms: u64, redraw: Redraw) -> FrameRecord {
        let d = Duration::from_millis;
        FrameRecord {
            index: 0,
            at,
            tick: d(1),
            layout: d(2),
            prepare: d(3),
            acquire: d(wait_ms),
            draw: d(4),
            present: d(0),
            total: d(total_ms),
            redraw,
            shader_passes: 0,
            nodes: 10,
            size: (100, 100),
            gpu: None,
        }
    }

    #[test]
    fn nothing_recorded_is_an_empty_summary() {
        let stats = FrameStats::default();
        assert_eq!(stats.summary(), Summary::default());
        assert!(stats.last().is_none());
        assert_eq!((stats.drawn(), stats.skipped()), (0, 0));
    }

    #[test]
    fn cpu_cost_leaves_out_the_waits_for_the_display() {
        let r = record(Instant::now(), 16, 10, Redraw::Full);
        assert_eq!(r.cpu(), Duration::from_millis(6));
        // Waits longer than the whole frame never go negative.
        assert_eq!(
            record(Instant::now(), 5, 10, Redraw::Full).cpu(),
            Duration::ZERO
        );
    }

    #[test]
    fn frames_are_numbered_and_the_summary_averages_them() {
        let (mut stats, t0) = (FrameStats::default(), Instant::now());
        for i in 0..4u64 {
            let at = t0 + Duration::from_millis(i * 20);
            let r = stats.drew(record(
                at,
                10 + i * 10,
                0,
                Redraw::Partial {
                    rects: 1,
                    area: 0.1,
                },
            ));
            assert_eq!(r.index, i + 1);
        }
        let s = stats.summary();
        assert_eq!(s.count, 4);
        assert!((s.total_ms.mean - 25.0).abs() < 1e-9, "10, 20, 30, 40");
        assert!((s.total_ms.max - 40.0).abs() < 1e-9);
        assert!((s.total_ms.p95 - 40.0).abs() < 1e-9);
        assert!((s.fps - 50.0).abs() < 1e-6, "3 gaps of 20 ms: {}", s.fps);
        assert!((s.tick_ms - 1.0).abs() < 1e-9 && (s.draw_ms - 4.0).abs() < 1e-9);
        assert_eq!(
            (s.redrew_partial, s.redrew_full, s.redrew_nothing),
            (4, 0, 0)
        );
    }

    #[test]
    fn the_95th_percentile_is_nearest_rank() {
        let (mut stats, t0) = (FrameStats::default(), Instant::now());
        // 100 frames of 1 ms and 5 of 100 ms: the 95th of 105 is still 1 ms
        // (rank 100), the max is not.
        for i in 0..105u64 {
            let total = if i < 100 { 1 } else { 100 };
            stats.drew(record(
                t0 + Duration::from_millis(i),
                total,
                0,
                Redraw::Full,
            ));
        }
        let s = stats.summary();
        assert!((s.total_ms.p95 - 1.0).abs() < 1e-9, "{}", s.total_ms.p95);
        assert!((s.total_ms.max - 100.0).abs() < 1e-9);
    }

    #[test]
    fn only_the_last_few_hundred_are_kept_but_all_are_counted() {
        let (mut stats, t0) = (FrameStats::default(), Instant::now());
        for i in 0..(KEPT as u64 + 50) {
            stats.drew(record(t0 + Duration::from_millis(i), 1, 0, Redraw::Full));
        }
        assert_eq!(stats.drawn(), KEPT as u64 + 50);
        assert_eq!(stats.summary().count, KEPT);
        assert_eq!(stats.last().unwrap().index, KEPT as u64 + 50);
    }

    #[test]
    fn skipped_passes_are_counted_and_reset_clears_everything() {
        let mut stats = FrameStats::default();
        stats.skipped_one();
        stats.skipped_one();
        stats.drew(record(Instant::now(), 1, 0, Redraw::Nothing));
        assert_eq!((stats.drawn(), stats.skipped()), (1, 2));
        stats.reset();
        assert_eq!((stats.drawn(), stats.skipped()), (0, 0));
        assert!(stats.last().is_none());
    }

    #[test]
    fn a_single_frame_has_no_rate() {
        let mut stats = FrameStats::default();
        stats.drew(record(Instant::now(), 7, 0, Redraw::Full));
        assert_eq!(stats.summary().fps, 0.0);
        assert_eq!(stats.summary().count, 1);
    }
}
