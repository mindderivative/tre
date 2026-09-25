//! M87 (tre issue #6): running a Python callable on a live `App`'s
//! event-loop thread, queued from any other thread.
//!
//! `App`/`Window`/`View`/`Node` are all thread-bound (`thread_bound`), so a
//! background thread -- a file watcher driving hot reload, a network
//! client, a subprocess reader -- can't touch them, and once `App.run()`
//! starts, Python otherwise only runs inside input handlers. `LoopHandle`
//! is the one `Send + Sync` object a background thread can hold:
//! `call_soon(fn)` queues `fn` and wakes the loop, and the event-loop
//! thread runs it at the top of its next frame, with the GIL held, where
//! it can call `view.reconcile(spec=...)` like any input handler could.
//!
//! The same shape `TerminalSession`'s PTY reader thread already uses
//! (`terminal.rs`): a shared queue plus an `EventLoopWaker`
//! (`engine-platform`, M31 Phase 6) installed from `run_windowed_multi`'s
//! `setup` closure. `PlatformEvent::Wake` wakes even an idle
//! `ControlFlow::Wait` loop and requests a redraw of every window, which
//! is what gets the queue drained.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use engine_platform::EventLoopWaker;
use pyo3::prelude::*;

#[derive(Default)]
struct Shared {
    queue: VecDeque<Py<PyAny>>,
    /// `None` outside `App.run()` -- a callable queued then just waits
    /// for the next run's first frame.
    waker: Option<EventLoopWaker>,
}

/// The queue one `App` drains. Cheap to clone (one `Arc`); every clone
/// is the same queue.
#[derive(Clone, Default)]
pub(crate) struct CallQueue(Arc<Mutex<Shared>>);

impl CallQueue {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues `callable` and wakes the loop if one is running. The lock
    /// is released before waking, so nothing here ever holds it across
    /// a call out of this module.
    pub(crate) fn push(&self, callable: Py<PyAny>) {
        let waker = {
            let mut shared = self.lock();
            shared.queue.push_back(callable);
            shared.waker.clone()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Installed by `App.run()`'s `setup` closure, cleared when `run()`
    /// returns.
    pub(crate) fn set_waker(&self, waker: Option<EventLoopWaker>) {
        self.lock().waker = waker;
    }

    fn wake(&self) {
        let waker = self.lock().waker.clone();
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Runs every queued callable in FIFO order on the calling (event-
    /// loop) thread; returns how many ran. The whole queue is taken
    /// under the lock and the lock released *before* any Python runs, so
    /// a callable that itself calls `call_soon` can't deadlock -- its new
    /// entry simply runs on the next frame. A raising callable is
    /// reported like an uncaught input-handler exception and doesn't
    /// stop the rest. After running anything, the loop is woken once
    /// more: a callable run during window A's frame may have changed
    /// window B's tree after B's frame already ran, and the extra wake
    /// makes every window re-check its dirty flag.
    pub(crate) fn drain(&self, py: Python<'_>) -> usize {
        let callables: Vec<Py<PyAny>> = self.lock().queue.drain(..).collect();
        for callable in &callables {
            if let Err(err) = callable.call0(py) {
                crate::dispatch::log_uncaught_exception(&err, py);
            }
        }
        if !callables.is_empty() {
            self.wake();
        }
        callables.len()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.lock().queue.len()
    }
}

/// A thread-safe handle to a running `App`'s event loop, from
/// `App.thread_handle()`. The one `tre` object a background thread may
/// use.
#[pyclass(frozen, name = "LoopHandle")]
pub struct LoopHandle {
    queue: CallQueue,
}

impl LoopHandle {
    pub(crate) fn new(queue: CallQueue) -> Self {
        Self { queue }
    }
}

#[pymethods]
impl LoopHandle {
    /// Queues `callback` (called with no arguments) to run on the
    /// `App`'s event-loop thread, and wakes the loop -- including an idle
    /// one. Safe from any thread, before, during, or after `App.run()`:
    /// a callback queued outside a run waits for the next run's first
    /// frame. Raises `TypeError` if `callback` isn't callable.
    fn call_soon(&self, callback: Bound<'_, PyAny>) -> PyResult<()> {
        if !callback.is_callable() {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "call_soon() needs a callable, got {}",
                callback.get_type().name()?
            )));
        }
        self.queue.push(callback.unbind());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_runs_queued_callables_in_fifo_order_and_empties_the_queue() {
        Python::attach(|py| {
            let order = pyo3::types::PyList::empty(py);
            let globals = pyo3::types::PyDict::new(py);
            globals.set_item("order", &order).unwrap();
            let queue = CallQueue::default();
            for code in [c"lambda: order.append(1)", c"lambda: order.append(2)"] {
                queue.push(py.eval(code, Some(&globals), None).unwrap().unbind());
            }
            assert_eq!(queue.len(), 2);
            assert_eq!(queue.drain(py), 2);
            assert_eq!(order.extract::<Vec<i32>>().unwrap(), vec![1, 2]);
            assert_eq!(queue.len(), 0);
            assert_eq!(queue.drain(py), 0, "an empty queue drains nothing");
        });
    }

    #[test]
    fn a_raising_callable_does_not_stop_the_rest_of_the_queue() {
        Python::attach(|py| {
            let order = pyo3::types::PyList::empty(py);
            let globals = pyo3::types::PyDict::new(py);
            globals.set_item("order", &order).unwrap();
            let queue = CallQueue::default();
            queue.push(py.eval(c"lambda: 1 / 0", None, None).unwrap().unbind());
            queue.push(
                py.eval(c"lambda: order.append('ran')", Some(&globals), None)
                    .unwrap()
                    .unbind(),
            );
            assert_eq!(queue.drain(py), 2);
            assert_eq!(
                order.len(),
                1,
                "the callable after the raising one must still run"
            );
        });
    }

    #[test]
    fn a_callable_queued_during_a_drain_runs_on_the_next_drain_not_this_one() {
        Python::attach(|py| {
            let queue = CallQueue::default();
            let handle = Py::new(py, LoopHandle::new(queue.clone())).unwrap();
            let globals = pyo3::types::PyDict::new(py);
            globals.set_item("handle", &handle).unwrap();
            globals
                .set_item("hits", pyo3::types::PyList::empty(py))
                .unwrap();
            queue.push(
                py.eval(
                    c"lambda: handle.call_soon(lambda: hits.append(1))",
                    Some(&globals),
                    None,
                )
                .unwrap()
                .unbind(),
            );
            assert_eq!(queue.drain(py), 1);
            assert_eq!(queue.len(), 1, "re-queued, not run in the same drain");
            assert_eq!(queue.drain(py), 1);
        });
    }

    #[test]
    fn call_soon_rejects_a_non_callable() {
        Python::attach(|py| {
            let handle = LoopHandle::new(CallQueue::default());
            let err = handle
                .call_soon(py.eval(c"42", None, None).unwrap())
                .expect_err("an int isn't callable");
            assert!(err.is_instance_of::<pyo3::exceptions::PyTypeError>(py));
            assert!(err.to_string().contains("int"));
        });
    }
}
