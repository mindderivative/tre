//! Phase 9 Step 9.2's real zero-allocation debug guard (TECHNICAL.md
//! Section 3.4): a custom `#[global_allocator]` wrapper around the
//! system allocator, checking a thread-local "render tick active" flag
//! set by [`RenderTickGuard`]. Lives here, not `tre-engine`, because
//! `tre-engine` carries `#![forbid(unsafe_code)]` -- implementing
//! `GlobalAlloc` requires `unsafe`, and this crate is one of the four
//! workspace locations TECHNICAL.md Section 9.1 permits it in
//! ("the ring-buffer/arena allocators" category this whole crate
//! already belongs to).
//!
//! `#[global_allocator]` is installed by the *binary* that opts in
//! (e.g. `main_loop_demo.rs`), never by this library crate itself: the
//! attribute is whole-binary-scoped, and a library crate declaring one
//! unconditionally would force it onto every downstream consumer,
//! including ones that never asked for this check.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static RENDER_TICK_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// A `GlobalAlloc` wrapper around [`System`] that panics on any
/// allocation observed while the calling thread's own
/// [`RenderTickGuard`] is active. Debug/profile builds only --
/// `cfg(not(debug_assertions))` makes every check a no-op, matching
/// TECHNICAL.md Section 3.4's own "Release Build Behavior" bullet: zero
/// steady-state overhead from the enforcement mechanism itself in a
/// release build.
pub struct DebugAllocGuard;

impl DebugAllocGuard {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[cfg(debug_assertions)]
    fn check() {
        if RENDER_TICK_ACTIVE.with(Cell::get) {
            // Disarm *before* panicking: panic!()'s own message
            // formatting and unwind machinery may themselves allocate,
            // and if the flag were still set, that allocation would
            // re-enter this very check and panic again before the first
            // panic has even started unwinding -- a double panic
            // aborts the process instead of reporting the real
            // violation this guard exists to catch.
            RENDER_TICK_ACTIVE.with(|flag| flag.set(false));
            panic!(
                "heap allocation observed while a RenderTickGuard was active (TECHNICAL.md \
                 Section 3.4) -- the 0 bytes/frame zero-allocation budget was violated"
            );
        }
    }

    #[cfg(not(debug_assertions))]
    fn check() {}
}

impl Default for DebugAllocGuard {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: every method below delegates directly to `System`, the real,
// already-sound global allocator, forwarding its arguments unchanged --
// `check()` only ever reads/writes a thread-local `bool` and, on
// violation, panics; it never touches the pointer/layout arguments or
// otherwise affects the allocator contract `System` itself upholds.
unsafe impl GlobalAlloc for DebugAllocGuard {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::check();
        // SAFETY: `layout` is this method's own parameter, forwarded
        // unchanged under the identical caller contract `System::alloc`
        // itself requires.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::check();
        // SAFETY: identical reasoning to `alloc` above.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::check();
        // SAFETY: identical reasoning to `alloc` above.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::check();
        // SAFETY: identical reasoning to `alloc` above.
        unsafe { System.alloc_zeroed(layout) }
    }
}

/// RAII scope guard marking "a render tick is active on this thread"
/// (TECHNICAL.md Section 3.4) -- construct one at the start of the CPU
/// work a frame's zero-allocation budget covers and let it drop at the
/// end. The tracked flag is thread-local by design: a multi-threaded
/// recording scheme (Step 5.2's `SubCanvas` workers) needs its own guard
/// started on *each* thread whose allocations should be checked -- the
/// main thread's own guard has no visibility into a worker thread's
/// allocations, and vice versa.
///
/// No-op in release builds (`cfg(not(debug_assertions))`) -- matches
/// `DebugAllocGuard::check`'s own release behavior exactly, so
/// constructing and dropping this type costs nothing once compiled
/// without `debug_assertions`.
pub struct RenderTickGuard {
    _private: (),
}

impl RenderTickGuard {
    /// Marks a render tick active on the calling thread.
    ///
    /// # Panics
    /// In debug builds, panics if a `RenderTickGuard` is already active
    /// on this thread -- guards do not nest. An accidental double-
    /// `begin()` almost certainly means a prior guard's own `Drop` was
    /// skipped (e.g. by `std::mem::forget`) or two independent call
    /// sites both think they own "the" render tick for this thread,
    /// either of which is a programmer error (DESIGN.md Section 2.6),
    /// not a legitimate use.
    #[must_use]
    pub fn begin() -> Self {
        #[cfg(debug_assertions)]
        {
            if RENDER_TICK_ACTIVE.with(Cell::get) {
                // Same reasoning as `DebugAllocGuard::check`'s own
                // disarm-before-panic: a plain `assert!` here would
                // panic while the flag is still `true`, and the panic
                // machinery's own allocations (backtrace capture,
                // payload boxing) would then be caught by `check` as a
                // false "heap allocation observed" violation instead of
                // this assert's own real message -- disarming first
                // avoids that entirely.
                RENDER_TICK_ACTIVE.with(|flag| flag.set(false));
                panic!(
                    "RenderTickGuard::begin() called while another RenderTickGuard is already \
                     active on this thread -- guards do not nest"
                );
            }
            RENDER_TICK_ACTIVE.with(|flag| flag.set(true));
        }
        Self { _private: () }
    }
}

impl Drop for RenderTickGuard {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        RENDER_TICK_ACTIVE.with(|flag| flag.set(false));
    }
}

#[cfg(test)]
mod tests {
    use super::{DebugAllocGuard, RenderTickGuard};

    // Installed only for this crate's own test binary (`cfg(test)`
    // scopes it out of the normal library build entirely) -- proves
    // `DebugAllocGuard` genuinely intercepts real heap allocations
    // end to end, not a simulated/mocked check. Outside any active
    // `RenderTickGuard`, every method is a pure passthrough to
    // `System`, so this has zero effect on every other pre-existing
    // test in this crate that never touches `RenderTickGuard` at all.
    #[global_allocator]
    static ALLOCATOR: DebugAllocGuard = DebugAllocGuard::new();

    #[test]
    fn allocating_outside_a_tick_is_fine() {
        let v: Vec<u32> = (0..100).collect();
        assert_eq!(v.len(), 100);
    }

    #[test]
    #[should_panic(expected = "heap allocation observed while a RenderTickGuard was active")]
    fn allocating_inside_a_tick_panics() {
        let _guard = RenderTickGuard::begin();
        let _v: Vec<u32> = Vec::with_capacity(16);
    }

    #[test]
    fn no_allocation_inside_a_tick_does_not_panic() {
        let capacity_prebuilt: Vec<u32> = Vec::with_capacity(16);
        let guard = RenderTickGuard::begin();
        let mut reused = capacity_prebuilt;
        reused.push(1);
        reused.push(2);
        drop(guard);
        assert_eq!(reused, vec![1, 2]);
    }

    #[test]
    #[should_panic(expected = "guards do not nest")]
    fn nested_begin_on_the_same_thread_panics() {
        let _outer = RenderTickGuard::begin();
        let _inner = RenderTickGuard::begin();
    }

    #[test]
    fn a_background_threads_own_tick_is_invisible_to_the_main_threads_allocations() {
        // The thread-local flag must not leak across threads: a
        // background thread's own active RenderTickGuard must not make
        // this (the test-runner) thread's own, entirely unrelated
        // allocations panic.
        let handle = std::thread::spawn(|| {
            let _guard = RenderTickGuard::begin();
            std::thread::sleep(std::time::Duration::from_millis(50));
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        let v: Vec<u32> = (0..50).collect();
        assert_eq!(v.len(), 50);
        handle
            .join()
            .expect("background thread panicked unexpectedly");
    }

    #[test]
    fn a_violations_panic_does_not_leave_the_flag_stuck_active() {
        // Real regression coverage for the disarm-before-panic design:
        // if check() panicked *without* first clearing the flag, the
        // panic's own formatting/unwind allocations would recursively
        // re-trigger it, and (having survived that, however it survived)
        // any *later*, unrelated allocation on this same thread after
        // catching the panic would incorrectly panic too.
        let result = std::panic::catch_unwind(|| {
            let _guard = RenderTickGuard::begin();
            let _v: Vec<u32> = Vec::with_capacity(4);
        });
        assert!(
            result.is_err(),
            "the allocation inside the tick must have panicked"
        );
        // The guard's own Drop never ran (the panic unwound through
        // Vec::with_capacity before RenderTickGuard was ever dropped),
        // but check()'s own disarm-before-panic must have already
        // cleared the flag -- this allocation, on the same thread right
        // after catching that panic, must not panic again.
        let v: Vec<u32> = (0..10).collect();
        assert_eq!(v.len(), 10);
    }
}
