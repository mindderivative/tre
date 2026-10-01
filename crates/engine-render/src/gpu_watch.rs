//! 0.5.1 (#65): what the GPU tells us about its own health -- a lost device,
//! a GPU error, a frame that never finishes -- collected where the loop can
//! deliver it to the app on its own thread.
//!
//! What `wgpu` 30 does by default, measured for #65: a validation error
//! panics the process (its default handler), and a lost device is reported
//! only through a callback that fires **only when the device is polled** --
//! neither `destroy()` nor an empty `submit` triggers it. After the device
//! is gone, further calls silently do nothing, so that callback is the only
//! signal. A software adapter has no driver timeout, so a shader that never
//! ends there is never reported lost at all: only a watchdog notices.
//!
//! [`GpuWatch`] installs the two handlers on a device, remembers how many
//! submissions are still running (`on_submitted_work_done`), polls the
//! device without blocking, and runs an opt-in stall watchdog. Neither
//! handler touches Python or the loop: both queue a [`GpuReport`] that
//! [`GpuWatch::poll`] hands back.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Submissions still running on any device, for a loop that must keep
/// waking while the GPU has work in flight: nothing else polls an idle loop,
/// so a hang in the last frame would otherwise never be seen.
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// Whether any device in this process has a submission still running.
pub fn any_in_flight() -> bool {
    IN_FLIGHT.load(Ordering::Relaxed) > 0
}

/// The most distinct GPU errors reported per device; the rest are dropped,
/// so a draw failing in a different way every frame can't flood the app.
const MAX_DISTINCT_ERRORS: usize = 64;

/// Something the GPU reported.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuReport {
    /// The device was lost: a driver fault, or a hang the driver's own
    /// timeout caught. `destroyed` is true when it was `Device::destroy`,
    /// false for a fault (then `message` says what the driver reported).
    Lost { destroyed: bool, message: String },
    /// A GPU error the device reported; the draw that caused it is skipped.
    /// Each distinct message is reported once.
    Error { message: String },
    /// A submitted frame hasn't completed after `seconds` (the opt-in
    /// watchdog). Reported once per stuck submission; the work can't be
    /// cancelled.
    Stalled { seconds: f64 },
}

struct Shared {
    reports: Mutex<Vec<GpuReport>>,
    /// Submissions not yet completed. Guards the process-wide count: a
    /// completion that arrives after `GpuWatch` was dropped finds this at
    /// zero and subtracts nothing.
    outstanding: AtomicUsize,
    /// The highest submission number the GPU has finished.
    completed: AtomicU64,
}

/// A stall check that can be tested without a GPU: given the oldest
/// submission still running, decide whether to report it.
#[derive(Default)]
struct Stalls {
    /// The submission already reported, so one stuck frame is one report.
    reported: Option<u64>,
}

impl Stalls {
    fn check(
        &mut self,
        limit: Duration,
        oldest: Option<(u64, Instant)>,
        now: Instant,
    ) -> Option<f64> {
        let (seq, since) = oldest?;
        let waited = now.saturating_duration_since(since);
        if waited < limit || self.reported == Some(seq) {
            return None;
        }
        self.reported = Some(seq);
        Some(waited.as_secs_f64())
    }
}

/// One device's health: install it right after the device is made.
pub struct GpuWatch {
    shared: Arc<Shared>,
    /// Submission numbers and when they were made, oldest first.
    pending: VecDeque<(u64, Instant)>,
    next_seq: u64,
    watchdog: Option<Duration>,
    stalls: Stalls,
    seen_errors: HashSet<String>,
    lost: bool,
}

impl GpuWatch {
    /// Installs the device-lost callback and the uncaptured-error handler on
    /// `device`. This replaces `wgpu`'s default, which panics on any GPU
    /// error.
    pub fn install(device: &wgpu::Device) -> Self {
        let shared = Arc::new(Shared {
            reports: Mutex::new(Vec::new()),
            outstanding: AtomicUsize::new(0),
            completed: AtomicU64::new(0),
        });
        let lost = shared.clone();
        device.set_device_lost_callback(move |reason, message| {
            let destroyed = matches!(reason, wgpu::DeviceLostReason::Destroyed);
            lost.reports
                .lock()
                .unwrap()
                .push(GpuReport::Lost { destroyed, message });
        });
        let errors = shared.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            errors.reports.lock().unwrap().push(GpuReport::Error {
                message: error.to_string(),
            });
        }));
        Self {
            shared,
            pending: VecDeque::new(),
            next_seq: 0,
            watchdog: None,
            stalls: Stalls::default(),
            seen_errors: HashSet::new(),
            lost: false,
        }
    }

    /// Turns the stall watchdog on (`Some(seconds)`) or off (`None`). Off by
    /// default.
    pub fn set_watchdog(&mut self, seconds: Option<f64>) {
        self.watchdog = seconds
            .filter(|s| s.is_finite() && *s > 0.0)
            .map(Duration::from_secs_f64);
    }

    /// Call right after `queue.submit`: counts the submission as running
    /// until the GPU reports its work done.
    pub fn submitted(&mut self, queue: &wgpu::Queue) {
        self.next_seq += 1;
        let seq = self.next_seq;
        self.pending.push_back((seq, Instant::now()));
        self.shared.outstanding.fetch_add(1, Ordering::Relaxed);
        IN_FLIGHT.fetch_add(1, Ordering::Relaxed);
        let shared = self.shared.clone();
        queue.on_submitted_work_done(move || {
            shared.completed.fetch_max(seq, Ordering::Relaxed);
            let dropped =
                shared
                    .outstanding
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
            if dropped.is_ok() {
                IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
            }
        });
    }

    /// Polls the device without blocking, which is what fires the lost and
    /// completion callbacks, then returns what the GPU reported since the
    /// last call: a loss first, each distinct error once, and a stall if the
    /// watchdog is on and a submission has run too long.
    pub fn poll(&mut self, device: &wgpu::Device) -> Vec<GpuReport> {
        let _ = device.poll(wgpu::PollType::Poll);
        self.report(Instant::now())
    }

    /// The part of [`poll`](Self::poll) after the device was polled, with the
    /// time passed in, so the watchdog is testable.
    fn report(&mut self, now: Instant) -> Vec<GpuReport> {
        let completed = self.shared.completed.load(Ordering::Relaxed);
        while self
            .pending
            .front()
            .is_some_and(|&(seq, _)| seq <= completed)
        {
            self.pending.pop_front();
        }
        let mut out = Vec::new();
        for report in std::mem::take(&mut *self.shared.reports.lock().unwrap()) {
            match report {
                GpuReport::Lost { .. } => {
                    self.lost = true;
                    tracing::error!(?report, "the GPU device was lost");
                    out.push(report);
                }
                GpuReport::Error { ref message } => {
                    if self.seen_errors.len() < MAX_DISTINCT_ERRORS
                        && self.seen_errors.insert(message.clone())
                    {
                        tracing::warn!(%message, "GPU error (reported once)");
                        out.push(report);
                    }
                }
                GpuReport::Stalled { .. } => out.push(report),
            }
        }
        if let Some(limit) = self.watchdog
            && !self.lost
            && let Some(seconds) = self.stalls.check(limit, self.pending.front().copied(), now)
        {
            tracing::warn!(seconds, "a submitted GPU frame has not completed");
            out.push(GpuReport::Stalled { seconds });
        }
        out
    }

    /// Whether a submission is still running on this device.
    pub fn in_flight(&self) -> bool {
        self.shared.outstanding.load(Ordering::Relaxed) > 0
    }

    /// Whether the device has been reported lost.
    pub fn is_lost(&self) -> bool {
        self.lost
    }
}

impl Drop for GpuWatch {
    fn drop(&mut self) {
        // Work still running when the device goes away may never report
        // done; give the process-wide count back so an idle loop can sleep.
        let left = self.shared.outstanding.swap(0, Ordering::Relaxed);
        IN_FLIGHT.fetch_sub(left, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, secs: f64) -> Instant {
        base + Duration::from_secs_f64(secs)
    }

    #[test]
    fn a_stall_is_reported_once_after_the_limit() {
        let base = Instant::now();
        let mut stalls = Stalls::default();
        let limit = Duration::from_secs(5);
        let oldest = Some((7, base));
        assert_eq!(stalls.check(limit, oldest, at(base, 4.9)), None, "not yet");
        let waited = stalls
            .check(limit, oldest, at(base, 5.0))
            .expect("at the limit");
        assert!((waited - 5.0).abs() < 1e-6);
        assert_eq!(
            stalls.check(limit, oldest, at(base, 9.0)),
            None,
            "once per submission"
        );
    }

    #[test]
    fn the_next_stuck_submission_is_reported_again() {
        let base = Instant::now();
        let mut stalls = Stalls::default();
        let limit = Duration::from_secs(1);
        assert!(
            stalls
                .check(limit, Some((1, base)), at(base, 2.0))
                .is_some()
        );
        assert!(
            stalls
                .check(limit, Some((2, at(base, 2.0))), at(base, 4.0))
                .is_some()
        );
    }

    #[test]
    fn nothing_running_is_never_a_stall() {
        let mut stalls = Stalls::default();
        assert_eq!(
            stalls.check(Duration::from_secs(1), None, Instant::now()),
            None
        );
    }
}
