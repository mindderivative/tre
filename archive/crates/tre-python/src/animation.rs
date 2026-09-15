//! `tre.Timeline` (Phase 13 Step 13.3: `treAnimation`), binding to
//! `tre_animation::Timeline`'s own real sequencer. `tre-animation` keeps
//! its animation targets as opaque `u64`s (no PyO3 dependency, see its
//! own doc comment) -- this module supplies the missing half: each
//! `u64` maps to a real `(Py<PyAny>, attribute name)` pair, and
//! `advance()` applies every sampled value back onto its target object
//! via Python's own `setattr`, which works generically for any
//! `#[pyo3(get, set)]` field on any shape class (Rectangle/Circle/
//! Polygon/Path/Text/Svg all qualify already, reusing Step 12.9's own
//! `scale_x`/`scale_y`/`rotation`/`opacity` exposure as real animation
//! targets for the first time) without this module needing per-shape
//! dispatch code.

use std::collections::HashMap;

use pyo3::prelude::*;
use tre_animation::{tween, Timeline};

use crate::tween::PyEasing;

#[pyclass(name = "Timeline")]
pub struct PyTimeline {
    inner: Timeline,
    /// Maps each `Timeline` entry's opaque target id to the real Python
    /// object + attribute name `advance()` applies its sampled value to.
    properties: HashMap<u64, (Py<PyAny>, String)>,
    next_id: u64,
}

#[pymethods]
impl PyTimeline {
    #[new]
    fn new() -> Self {
        Self {
            inner: Timeline::new(),
            properties: HashMap::new(),
            next_id: 0,
        }
    }

    /// Schedules a real animation of `target.<property>` from its
    /// current value to `to` over `duration` real seconds, starting now
    /// (this timeline's current elapsed time).
    ///
    /// # Errors
    /// Raises `AttributeError` if `target` has no `property` attribute,
    /// or `TypeError` if that attribute isn't a real number.
    #[pyo3(signature = (target, property, to, duration, easing = PyEasing::Linear))]
    fn animate(
        &mut self,
        py: Python<'_>,
        target: Py<PyAny>,
        property: String,
        to: f32,
        duration: f32,
        easing: PyEasing,
    ) -> PyResult<()> {
        let current: f32 = target.bind(py).getattr(property.as_str())?.extract()?;
        let id = self.next_id;
        self.next_id += 1;
        self.inner
            .animate(id, tween(current, to, duration, easing.into()));
        self.properties.insert(id, (target, property));
        Ok(())
    }

    /// Advances this timeline by `dt` real seconds, applying every
    /// sampled value back onto its target's attribute via `setattr`.
    /// Returns whether at least one scheduled animation has not yet
    /// finished.
    ///
    /// # Errors
    /// Raises whatever error `setattr` itself raises (e.g. if a
    /// target's attribute became read-only or the target was mutated
    /// into an incompatible shape between `animate()` and `advance()`).
    fn advance(&mut self, py: Python<'_>, dt: f32) -> PyResult<bool> {
        let (samples, still_running) = self.inner.advance(dt);
        for (id, value) in samples {
            if let Some((target, property)) = self.properties.get(&id) {
                target.bind(py).setattr(property.as_str(), value)?;
            }
        }
        Ok(still_running)
    }

    /// Drops every already-finished scheduled animation, so a
    /// long-lived `Timeline` doesn't accumulate dead entries forever.
    fn prune_finished(&mut self) {
        for id in self.inner.prune_finished() {
            self.properties.remove(&id);
        }
    }

    #[getter]
    fn elapsed(&self) -> f32 {
        self.inner.elapsed()
    }

    #[getter]
    fn active_count(&self) -> usize {
        self.inner.active_count()
    }
}
