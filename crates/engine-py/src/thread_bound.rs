//! M96: `ThreadBound<T>`, the thread-checked shell every `tre` object Python
//! holds is built on (issue #10).
//!
//! The state behind a `Window`, `Node`, `App`, and the rest is `Rc`-shared
//! and so can't cross threads. `#[pyclass(unsendable)]` expressed that, but
//! pyo3 then leaks an object -- and panics in `__clear__` -- when Python's
//! cyclic collector frees it on another thread, which any allocation on a
//! background thread can make happen. A class that isn't `unsendable` must
//! be `Send + Sync`, so each one wraps its state in a `ThreadBound`:
//!
//! - **Use** from another thread panics, exactly as `unsendable` did: every
//!   access goes through `Deref`, which checks the thread.
//! - **Drop** from another thread never touches the state there. It's moved,
//!   untouched, into a queue the owning thread drains at its next safe point
//!   (`reclaim`), where it's dropped normally.
//! - **`__traverse__`/`__clear__`** check `is_owner` first and do nothing
//!   elsewhere, which only makes the collector more conservative.

use std::any::Any;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, ThreadId};

pub(crate) struct ThreadBound<T: 'static> {
    owner: ThreadId,
    value: ManuallyDrop<T>,
}

// SAFETY: the value is only ever reached on `owner`: `Deref`/`DerefMut`
// panic anywhere else, and `Drop` elsewhere moves it -- never reading or
// dropping it -- into `ORPHANS`, which only `owner` drains.
unsafe impl<T: 'static> Send for ThreadBound<T> {}
unsafe impl<T: 'static> Sync for ThreadBound<T> {}

impl<T: 'static> ThreadBound<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            owner: thread::current().id(),
            value: ManuallyDrop::new(value),
        }
    }

    /// Whether the calling thread is the one that created this value.
    pub(crate) fn is_owner(&self) -> bool {
        thread::current().id() == self.owner
    }

    fn check(&self) {
        assert!(
            self.is_owner(),
            "{} belongs to the thread that created it -- reach it from another thread \
             through App.thread_handle().call_soon(...)",
            std::any::type_name::<T>()
        );
    }
}

impl<T: 'static> Deref for ThreadBound<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.check();
        &self.value
    }
}

impl<T: 'static> DerefMut for ThreadBound<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.check();
        &mut self.value
    }
}

impl<T: 'static> Drop for ThreadBound<T> {
    fn drop(&mut self) {
        // SAFETY: `value` is taken exactly once, here, and never used again.
        let value = unsafe { ManuallyDrop::take(&mut self.value) };
        if self.is_owner() {
            drop(value);
        } else {
            ORPHANS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((self.owner, Orphan(Box::new(value))));
            ORPHANS_PENDING.store(true, Ordering::Release);
        }
    }
}

/// Implements `Deref`/`DerefMut` from a `#[pyclass]` shell
/// `$shell(ThreadBound<$state>)` to its state, so its methods read fields
/// exactly as before.
macro_rules! thread_bound_shell {
    ($shell:ty => $state:ty) => {
        impl std::ops::Deref for $shell {
            type Target = $state;

            fn deref(&self) -> &$state {
                &self.0
            }
        }

        impl std::ops::DerefMut for $shell {
            fn deref_mut(&mut self) -> &mut $state {
                &mut self.0
            }
        }
    };
}
pub(crate) use thread_bound_shell;

/// A value dropped on the wrong thread, waiting for its owner.
struct Orphan(#[allow(dead_code)] Box<dyn Any>);

// SAFETY: an `Orphan` is only moved between threads, never read; it's
// dropped only by its owning thread, in `reclaim`.
unsafe impl Send for Orphan {}

static ORPHANS: Mutex<Vec<(ThreadId, Orphan)>> = Mutex::new(Vec::new());
static ORPHANS_PENDING: AtomicBool = AtomicBool::new(false);

/// Drops every value another thread handed back to this one. One atomic
/// load when there's nothing to do. Called at the event loop's frame start
/// and wherever new handles are made, so a queued value never waits long.
pub(crate) fn reclaim() {
    if !ORPHANS_PENDING.load(Ordering::Acquire) {
        return;
    }
    let me = thread::current().id();
    let mine: Vec<Orphan> = {
        let mut orphans = ORPHANS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mine, others): (Vec<_>, Vec<_>) =
            orphans.drain(..).partition(|(owner, _)| *owner == me);
        ORPHANS_PENDING.store(!others.is_empty(), Ordering::Release);
        *orphans = others;
        mine.into_iter().map(|(_, orphan)| orphan).collect()
    };
    // Dropped outside the lock: a value's own drop may hand back more.
    drop(mine);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn a_drop_on_another_thread_is_finished_by_the_owner() {
        let marker = Rc::new(());
        let bound = ThreadBound::new(marker.clone());
        assert_eq!(Rc::strong_count(&marker), 2);
        thread::spawn(move || drop(bound)).join().unwrap();
        assert_eq!(Rc::strong_count(&marker), 2, "not dropped off-thread");
        reclaim();
        assert_eq!(Rc::strong_count(&marker), 1, "dropped by the owner");
    }

    #[test]
    fn use_on_another_thread_panics() {
        let bound = std::sync::Arc::new(ThreadBound::new(1_u8));
        let other = bound.clone();
        let result = thread::spawn(move || **other).join();
        assert!(result.is_err());
        assert_eq!(**bound, 1);
    }
}
