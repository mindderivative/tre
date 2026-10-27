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
//!
//! 0.5.9 (#220): a device can serve several windows. `wgpu` keeps one lost
//! callback and one error handler per device, so those are installed once, by
//! a [`GpuHub`], which keeps a short log of what the device reported. Each
//! window has its own [`GpuWatch`] on the hub: it reads the log from where it
//! joined, so every window hears of a loss, and counts its own submissions for
//! the stall watchdog. [`GpuWatch::install`] is a hub with one watch, as before.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Submissions still running on any device, for a loop that must keep
/// waking while the GPU has work in flight: nothing else polls an idle loop,
/// so a hang in the last frame would otherwise never be seen.
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// Whether any device in this process has a submission still running.
pub fn any_in_flight() -> bool {
    IN_FLIGHT.load(Ordering::Relaxed) > 0
}

/// Wakes whoever waits in [`wait_while_idle`]. Held across the check so a submission that
/// lands between the check and the wait is never missed.
static FLIGHT: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());

/// 0.5.9 (#238): blocks while no submission is in flight and `give_up` is false, with no timer:
/// a host's helper thread that must watch the GPU only while it works sleeps through the rest
/// (an idle loop with no window open wakes nothing). Returns when work is in flight, or when
/// [`notify_flight`] was called and `give_up` is then true.
pub fn wait_while_idle(give_up: &std::sync::atomic::AtomicBool) {
    let mut guard = FLIGHT.0.lock().unwrap_or_else(|e| e.into_inner());
    while IN_FLIGHT.load(Ordering::Relaxed) == 0 && !give_up.load(Ordering::Relaxed) {
        guard = FLIGHT.1.wait(guard).unwrap_or_else(|e| e.into_inner());
    }
}

/// Wakes the threads in [`wait_while_idle`] to look again: a submission was made, or one of
/// them is to give up.
pub fn notify_flight() {
    let _guard = FLIGHT.0.lock().unwrap_or_else(|e| e.into_inner());
    FLIGHT.1.notify_all();
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

/// The most reports a device's log keeps. A window that falls further behind
/// than this (it would have to go unpolled through this many distinct errors,
/// which `MAX_DISTINCT_ERRORS` already caps per window) skips the oldest.
const LOG_LIMIT: usize = 256;

/// What one device has reported, oldest first, shared by every window that
/// draws with it. A report's number is its position since the log began, so a
/// reader can say where it got to.
#[derive(Default)]
struct Log {
    /// The number of the report at the front of `entries`.
    base: u64,
    entries: VecDeque<GpuReport>,
}

impl Log {
    fn push(&mut self, report: GpuReport) {
        self.entries.push_back(report);
        while self.entries.len() > LOG_LIMIT {
            self.entries.pop_front();
            self.base += 1;
        }
    }

    /// The number the next report will get.
    fn end(&self) -> u64 {
        self.base + self.entries.len() as u64
    }

    /// The reports from number `cursor` on (those already dropped are skipped).
    fn since(&self, cursor: u64) -> Vec<GpuReport> {
        let skip = cursor.saturating_sub(self.base) as usize;
        self.entries.iter().skip(skip).cloned().collect()
    }
}

/// One device's side of the watch: the log its two handlers write to.
struct Hub {
    log: Mutex<Log>,
    lost: AtomicBool,
}

/// One window's submissions on its device. Guards the process-wide count:
/// a completion that arrives after the [`GpuWatch`] was dropped finds
/// `outstanding` at zero and subtracts nothing.
struct Submissions {
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

/// A device's health, installed once right after the device is made, and handed
/// out as one [`GpuWatch`] per window that draws with it.
#[derive(Clone)]
pub struct GpuHub {
    hub: Arc<Hub>,
}

impl GpuHub {
    /// Installs the device-lost callback and the uncaptured-error handler on
    /// `device`. This replaces `wgpu`'s default, which panics on any GPU
    /// error, and any handler an earlier hub put there.
    pub fn install(device: &wgpu::Device) -> Self {
        let hub = Arc::new(Hub {
            log: Mutex::new(Log::default()),
            lost: AtomicBool::new(false),
        });
        let lost = hub.clone();
        device.set_device_lost_callback(move |reason, message| {
            let destroyed = matches!(reason, wgpu::DeviceLostReason::Destroyed);
            lost.lost.store(true, Ordering::Relaxed);
            lost.log
                .lock()
                .unwrap()
                .push(GpuReport::Lost { destroyed, message });
        });
        let errors = hub.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            errors.log.lock().unwrap().push(GpuReport::Error {
                message: error.to_string(),
            });
        }));
        Self { hub }
    }

    /// A watch for one more window on this device. It hears of what the device
    /// reports from now on, not of what it reported before.
    pub fn watch(&self) -> GpuWatch {
        let cursor = self.hub.log.lock().unwrap().end();
        GpuWatch {
            hub: self.hub.clone(),
            cursor,
            subs: Arc::new(Submissions {
                outstanding: AtomicUsize::new(0),
                completed: AtomicU64::new(0),
            }),
            pending: VecDeque::new(),
            next_seq: 0,
            watchdog: None,
            stalls: Stalls::default(),
            seen_errors: HashSet::new(),
            lost: false,
        }
    }

    /// Whether the device has been reported lost to anyone (it is known only
    /// once some window has polled the device). A lost device cannot be used.
    pub fn is_lost(&self) -> bool {
        self.hub.lost.load(Ordering::Relaxed)
    }
}

/// One window's view of its device's health.
pub struct GpuWatch {
    hub: Arc<Hub>,
    /// The next report in the device's log this window has not read.
    cursor: u64,
    subs: Arc<Submissions>,
    /// Submission numbers and when they were made, oldest first.
    pending: VecDeque<(u64, Instant)>,
    next_seq: u64,
    watchdog: Option<Duration>,
    stalls: Stalls,
    seen_errors: HashSet<String>,
    lost: bool,
}

impl GpuWatch {
    /// A device used by one window: a [`GpuHub`] and its only watch. This
    /// replaces `wgpu`'s default error handler, which panics on any GPU error.
    pub fn install(device: &wgpu::Device) -> Self {
        GpuHub::install(device).watch()
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
        self.subs.outstanding.fetch_add(1, Ordering::Relaxed);
        IN_FLIGHT.fetch_add(1, Ordering::Relaxed);
        notify_flight();
        let subs = self.subs.clone();
        queue.on_submitted_work_done(move || {
            subs.completed.fetch_max(seq, Ordering::Relaxed);
            let dropped =
                subs.outstanding
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
        let completed = self.subs.completed.load(Ordering::Relaxed);
        while self
            .pending
            .front()
            .is_some_and(|&(seq, _)| seq <= completed)
        {
            self.pending.pop_front();
        }
        let mut out = Vec::new();
        let news = {
            let log = self.hub.log.lock().unwrap();
            let news = log.since(self.cursor);
            self.cursor = log.end();
            news
        };
        for report in news {
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
        self.subs.outstanding.load(Ordering::Relaxed) > 0
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
        let left = self.subs.outstanding.swap(0, Ordering::Relaxed);
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

    fn lost() -> GpuReport {
        GpuReport::Lost {
            destroyed: false,
            message: "x".into(),
        }
    }

    fn error(n: u32) -> GpuReport {
        GpuReport::Error {
            message: format!("e{n}"),
        }
    }

    #[test]
    fn a_reader_hears_what_came_after_its_cursor_and_every_reader_hears_it() {
        let mut log = Log::default();
        log.push(error(0));
        let early = 0; // a window that was there from the start
        let late = log.end(); // a window that joined after the first report
        log.push(lost());
        assert_eq!(log.since(early), vec![error(0), lost()]);
        assert_eq!(
            log.since(late),
            vec![lost()],
            "a late window hears only what is new"
        );
        assert!(log.since(log.end()).is_empty());
    }

    #[test]
    fn a_full_log_drops_its_oldest_and_a_slow_reader_skips_to_what_is_left() {
        let mut log = Log::default();
        for n in 0..(LOG_LIMIT as u32 + 10) {
            log.push(error(n));
        }
        assert_eq!(log.entries.len(), LOG_LIMIT);
        assert_eq!(log.base, 10);
        let heard = log.since(0);
        assert_eq!(heard.len(), LOG_LIMIT);
        assert_eq!(heard[0], error(10), "the first ten were dropped");
        assert_eq!(log.end(), LOG_LIMIT as u64 + 10);
        assert_eq!(log.since(log.end() - 1), vec![error(LOG_LIMIT as u32 + 9)]);
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
