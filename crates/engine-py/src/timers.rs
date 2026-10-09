//! 0.5.6 (#163): `Window.after(ms, fn)`, `Window.every(ms, fn)` and the
//! `TimerHandle` that cancels them.
//!
//! A window keeps a queue of timers on its own clock (`clock::now`), so
//! `Window.advance(ms)` moves them like it moves animations. They run on the
//! loop's thread, in the frame, right after the tick. A loop with nothing
//! else to do sleeps through the wait; a small thread (`TimerWake`) wakes it
//! at the earliest deadline, so a snackbar's countdown costs one wake-up and
//! not sixty redraws a second.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use engine_platform::EventLoopWaker;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::thread_bound::{ThreadBound, thread_bound_shell};

/// One pending timer.
struct Entry {
    id: u64,
    due: Instant,
    /// `Some` for `every`; the timer is rescheduled instead of removed.
    period: Option<Duration>,
    callback: Py<PyAny>,
}

#[derive(Default)]
pub(crate) struct TimerQueue {
    next_id: u64,
    entries: Vec<Entry>,
}

pub(crate) type SharedTimers = Rc<RefCell<TimerQueue>>;

/// A timer taken out of the queue to run.
struct Due {
    id: u64,
    at: Instant,
    callback: Py<PyAny>,
}

impl TimerQueue {
    fn add(&mut self, due: Instant, period: Option<Duration>, callback: Py<PyAny>) -> u64 {
        self.next_id += 1;
        self.entries.push(Entry {
            id: self.next_id,
            due,
            period,
            callback,
        });
        self.next_id
    }

    fn cancel(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        self.entries.len() != before
    }

    fn contains(&self, id: u64) -> bool {
        self.entries.iter().any(|entry| entry.id == id)
    }

    /// When the first timer is due.
    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.entries.iter().map(|entry| entry.due).min()
    }

    /// Every timer due at `now`, earliest first (ties in creation order).
    fn due(&self, py: Python<'_>, now: Instant) -> Vec<Due> {
        let mut due: Vec<Due> = self
            .entries
            .iter()
            .filter(|entry| entry.due <= now)
            .map(|entry| Due {
                id: entry.id,
                at: entry.due,
                callback: entry.callback.clone_ref(py),
            })
            .collect();
        due.sort_by_key(|timer| (timer.at, timer.id));
        due
    }

    /// Settles timer `id` after it fired at `now`: `after` is removed, `every`
    /// is set to its next slot on the grid it started on (a tick that was
    /// missed is skipped, not replayed). False if it was cancelled meanwhile.
    fn settle(&mut self, id: u64, now: Instant) -> bool {
        let Some(position) = self.entries.iter().position(|entry| entry.id == id) else {
            return false;
        };
        match self.entries[position].period {
            None => {
                self.entries.remove(position);
            }
            Some(period) => {
                let entry = &mut self.entries[position];
                let mut next = entry.due + period;
                if next <= now {
                    // Skip the slots already gone by, whole periods at a time.
                    let behind = now.duration_since(next).as_nanos() / period.as_nanos().max(1);
                    next += period.saturating_mul(u32::try_from(behind).unwrap_or(u32::MAX));
                    while next <= now {
                        next += period;
                    }
                }
                entry.due = next;
            }
        }
        true
    }

    /// The window opens: timers set before it counted on the clock it had
    /// then; from here on it runs on the real one. Keeps what each has left.
    pub(crate) fn rebase(&mut self, from: Instant, to: Instant) {
        for entry in &mut self.entries {
            entry.due = to + entry.due.saturating_duration_since(from);
        }
    }

    /// The callbacks, for the cyclic collector.
    pub(crate) fn callbacks(&self) -> impl Iterator<Item = &Py<PyAny>> {
        self.entries.iter().map(|entry| &entry.callback)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Runs every timer due at `now`, once each, and returns whether any ran.
/// A timer a callback adds runs from the next pass; one it cancels does not
/// run. An exception is logged and the timers go on (an `every` keeps
/// repeating).
pub(crate) fn run_due(timers: &SharedTimers, now: Instant, py: Python<'_>) -> bool {
    let due = timers.borrow().due(py, now);
    let mut ran = false;
    for timer in due {
        // The queue is not borrowed while Python runs.
        if !timers.borrow_mut().settle(timer.id, now.max(timer.at)) {
            continue;
        }
        ran = true;
        if let Err(err) = timer.callback.call0(py) {
            crate::dispatch::log_uncaught_exception(&err, py);
        }
    }
    ran
}

/// What the loop's wait is woken by: the earliest timer deadline of any window.
pub(crate) struct Alarm {
    state: Mutex<AlarmState>,
    changed: Condvar,
}

struct AlarmState {
    deadline: Option<Instant>,
    stop: bool,
}

impl Alarm {
    /// Wakes the loop no later than `at` (an earlier deadline wins).
    pub(crate) fn arm(&self, at: Instant) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.deadline.is_none_or(|current| at < current) {
            state.deadline = Some(at);
            self.changed.notify_all();
        }
    }
}

pub(crate) type SharedAlarm = Rc<RefCell<Option<Arc<Alarm>>>>;

/// Arms the alarm for the first timer in `timers`, if there is one.
pub(crate) fn arm_for(timers: &SharedTimers, alarm: &SharedAlarm) {
    if let (Some(next), Some(alarm)) = (timers.borrow().next_due(), alarm.borrow().as_ref()) {
        alarm.arm(next);
    }
}

/// The thread that sleeps until the alarm's deadline and wakes the loop.
pub(crate) struct TimerWake {
    pub(crate) alarm: Arc<Alarm>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TimerWake {
    pub(crate) fn start(waker: EventLoopWaker) -> Self {
        let alarm = Arc::new(Alarm {
            state: Mutex::new(AlarmState {
                deadline: None,
                stop: false,
            }),
            changed: Condvar::new(),
        });
        let shared = alarm.clone();
        let thread = std::thread::Builder::new()
            .name("tre-timer-wake".into())
            .spawn(move || {
                let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if state.stop {
                        return;
                    }
                    match state.deadline {
                        None => {
                            state = shared
                                .changed
                                .wait(state)
                                .unwrap_or_else(|e| e.into_inner());
                        }
                        Some(deadline) => {
                            let now = Instant::now();
                            if deadline <= now {
                                state.deadline = None;
                                drop(state);
                                waker.wake();
                                state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                            } else {
                                state = shared
                                    .changed
                                    .wait_timeout(state, deadline - now)
                                    .unwrap_or_else(|e| e.into_inner())
                                    .0;
                            }
                        }
                    }
                }
            })
            .ok();
        Self { alarm, thread }
    }
}

impl Drop for TimerWake {
    fn drop(&mut self) {
        {
            let mut state = self.alarm.state.lock().unwrap_or_else(|e| e.into_inner());
            state.stop = true;
            self.alarm.changed.notify_all();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Adds a timer to a window's queue, on the window's clock, and arms the
/// loop's alarm for it.
pub(crate) fn schedule(
    timers: &SharedTimers,
    alarm: &SharedAlarm,
    now: Instant,
    spec: (f64, bool),
    callback: Py<PyAny>,
    py: Python<'_>,
) -> PyResult<TimerHandle> {
    let (ms, repeat) = spec;
    let what = if repeat { "every" } else { "after" };
    if !callback.bind(py).is_callable() {
        return Err(PyValueError::new_err(format!(
            "{what}: fn must be callable"
        )));
    }
    let min = if repeat { 1.0 } else { 0.0 };
    if !(ms.is_finite() && (min..=31_536_000_000.0).contains(&ms)) {
        return Err(PyValueError::new_err(format!(
            "{what}: ms must be from {min} to a year of milliseconds, got {ms}"
        )));
    }
    let delay = Duration::from_secs_f64(ms / 1000.0);
    let id = timers
        .borrow_mut()
        .add(now + delay, repeat.then_some(delay), callback);
    arm_for(timers, alarm);
    Ok(TimerHandle(ThreadBound::new(TimerState {
        timers: timers.clone(),
        id,
    })))
}

pub struct TimerState {
    timers: SharedTimers,
    id: u64,
}

/// What `Window.after` and `Window.every` return: cancels the timer.
#[pyclass(name = "TimerHandle")]
pub struct TimerHandle(ThreadBound<TimerState>);
thread_bound_shell!(TimerHandle => TimerState);

#[pymethods]
impl TimerHandle {
    /// Stops the timer. Returns whether it was still pending: `False` for an
    /// `after` that already ran or a timer already cancelled.
    fn cancel(&self) -> bool {
        self.timers.borrow_mut().cancel(self.id)
    }

    /// Whether the timer is still pending: an `every` until cancelled, an
    /// `after` until it has run.
    #[getter]
    fn active(&self) -> bool {
        self.timers.borrow().contains(self.id)
    }
}
