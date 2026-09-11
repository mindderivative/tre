//! `tre.Clock` (Phase 13 Step 13.1: `treTime`) -- a thin Python wrapper
//! over `tre_engine::FrameClock`, the engine's own real, hardware-backed
//! monotonic frame timer. Deliberately not a new timing implementation:
//! `FrameClock` already correctly handles "first tick has no prior frame
//! to measure a delta from" (returns `0.0`), and is real elsewhere in
//! the engine (`main_loop_demo.rs`) -- this just gives Python callers
//! the same real `Instant`-backed timer, plus a new `elapsed()` (total
//! time since creation) that `treTween`/`treAnimation` need to sample a
//! tween/timeline against, distinct from `tick()`'s own per-frame delta.

use pyo3::prelude::*;
use tre_engine::FrameClock;

/// A real monotonic timer: `tick()` returns the delta seconds since the
/// previous `tick()` call (`0.0` on the first call), `elapsed()` returns
/// the total real seconds since this `Clock` was constructed.
#[pyclass(name = "Clock")]
pub struct PyClock {
    inner: FrameClock,
}

#[pymethods]
impl PyClock {
    #[new]
    fn new() -> Self {
        Self {
            inner: FrameClock::new(),
        }
    }

    /// Real elapsed seconds since the previous `tick()` call on this
    /// clock (`0.0` on the first call).
    fn tick(&mut self) -> f32 {
        self.inner.tick()
    }

    /// Real total elapsed seconds since this `Clock` was constructed,
    /// independent of how many times -- or how recently -- `tick()` has
    /// been called.
    fn elapsed(&self) -> f32 {
        self.inner.elapsed()
    }
}
