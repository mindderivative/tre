//! A minimal `RenderingCanvas` binding (IMPLEMENTATION.md Phase 10 Step
//! 10.4 task 2's "context managers for `Canvas.save()`/`restore()` scope
//! pairs"). Deliberately narrow for this first pass: the
//! `ShapeRegistry`-driven retained-mode path
//! ([`crate::shapes::PyShapeRegistry`] +
//! [`crate::renderer::PyHeadlessRenderer`]) is this binding's primary,
//! fully-real surface; direct immediate-mode drawing through a raw
//! `Canvas` (the other ~20 `RenderingCanvas` methods -- `draw_rounded_
//! rect`, `draw_ellipse`, `push_clip`, layers, text, ...) is real,
//! disclosed follow-up work, not yet exposed here.

use pyo3::prelude::*;
use tre_engine::RenderingCanvas;

/// A `with tre.Canvas() as canvas:` block calls `save()` on entry and
/// `restore()` on exit -- a real Python context manager over the exact
/// same push/pop transform-and-clip stack `RenderingCanvas::save`/
/// `restore` already implement, not a separate Python-side reimplementation.
#[pyclass(name = "Canvas")]
pub struct PyCanvas {
    pub(crate) inner: RenderingCanvas,
}

#[pymethods]
impl PyCanvas {
    #[new]
    fn new() -> Self {
        Self {
            inner: RenderingCanvas::new(),
        }
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.inner.save();
        slf
    }

    #[pyo3(signature = (exc_type=None, exc_value=None, traceback=None))]
    fn __exit__(
        &mut self,
        exc_type: Option<Bound<'_, PyAny>>,
        exc_value: Option<Bound<'_, PyAny>>,
        traceback: Option<Bound<'_, PyAny>>,
    ) {
        let _ = (exc_type, exc_value, traceback);
        self.inner.restore();
    }
}
