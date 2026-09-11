//! `Canvas`, redesigned around what `tre-engine` actually does well
//! (Phase 12 Step 12.5), not a 1:1 port of `RenderingCanvas`'s ~20
//! methods. `ShapeRegistry` stays the primary way to describe *what
//! exists*; `Canvas` is the real scene-assembly/compositing context
//! shapes flatten into -- `push_clip`/`push_layer` (as real Python
//! context managers, so a caller can't mismatch a push with a missing
//! pop) and `tag_accessibility_node`, with a renderer's own
//! `flatten_into(canvas, registry)` as the real seam letting more than
//! one registry -- interleaved with clips/layers -- share one canvas
//! before a single `render_canvas` call submits it.
//!
//! `save()`/`restore()` (Step 10.4's own original scope) are unchanged:
//! a `with tre.Canvas() as canvas:` block still calls them on
//! enter/exit.

use pyo3::prelude::*;
use tre_engine::{
    AccessibilityNodeId, AccessibilityRole, LayerDesc, RenderingCanvas, ScissorRect, TextureFormat,
};

/// `tre_engine::AccessibilityRole`, bound directly.
#[pyclass(name = "AccessibilityRole", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyAccessibilityRole {
    Generic,
    Button,
    TextLabel,
    Image,
}

impl From<PyAccessibilityRole> for AccessibilityRole {
    fn from(role: PyAccessibilityRole) -> Self {
        match role {
            PyAccessibilityRole::Generic => Self::Generic,
            PyAccessibilityRole::Button => Self::Button,
            PyAccessibilityRole::TextLabel => Self::TextLabel,
            PyAccessibilityRole::Image => Self::Image,
        }
    }
}

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

    /// Real scissor clipping. Pushes `(x, y, width, height)` onto this
    /// canvas's clip stack immediately (not deferred to `__enter__` --
    /// `with canvas.clip(...):`'s own expression evaluation already
    /// happens exactly once, synchronously, right before the block
    /// starts) and returns a guard whose `__exit__` pops it -- a real
    /// caller cannot mismatch this push with a missing pop the way bare
    /// `push_clip`/`pop_clip` methods would allow.
    fn clip(
        slf: Py<Self>,
        py: Python<'_>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> PyResult<PyClipGuard> {
        slf.borrow_mut(py).inner.push_clip(&ScissorRect {
            x,
            y,
            width,
            height,
        });
        Ok(PyClipGuard { canvas: slf })
    }

    /// Real offscreen compositing (`push_layer`), with an optional
    /// Dual-Kawase blur of the layer's own content -- see
    /// `tre_engine::LayerDesc`'s own doc comment for the real "own-
    /// content blur, not backdrop blur" scope. Same immediate-push,
    /// guard-pops-on-exit contract as [`PyCanvas::clip`].
    #[pyo3(signature = (x, y, width, height, blur=false))]
    fn layer(
        slf: Py<Self>,
        py: Python<'_>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        blur: bool,
    ) -> PyResult<PyLayerGuard> {
        slf.borrow_mut(py).inner.push_layer(&LayerDesc {
            x,
            y,
            width,
            height,
            format: TextureFormat::Rgba8Unorm,
            blur,
        });
        Ok(PyLayerGuard { canvas: slf })
    }

    /// Tags a real, AT-SPI/UIA-consumable accessibility node at
    /// `(x, y, width, height)` (this canvas's own local space,
    /// transformed by whatever `save()`/`clip()`/`layer()` scope is
    /// currently active) -- not a context manager, since there is no
    /// matching "pop" operation. `node_id` is a caller-assigned, stable
    /// identifier for this UI element across frames.
    fn tag_accessibility_node(
        &mut self,
        node_id: u64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        role: PyAccessibilityRole,
    ) {
        self.inner.tag_accessibility_node(
            AccessibilityNodeId(node_id),
            x,
            y,
            width,
            height,
            role.into(),
        );
    }
}

/// [`PyCanvas::clip`]'s own returned context manager -- `__exit__` pops
/// the clip [`PyCanvas::clip`] already pushed.
#[pyclass(name = "ClipGuard")]
pub struct PyClipGuard {
    canvas: Py<PyCanvas>,
}

#[pymethods]
impl PyClipGuard {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (exc_type=None, exc_value=None, traceback=None))]
    fn __exit__(
        &self,
        py: Python<'_>,
        exc_type: Option<Bound<'_, PyAny>>,
        exc_value: Option<Bound<'_, PyAny>>,
        traceback: Option<Bound<'_, PyAny>>,
    ) {
        let _ = (exc_type, exc_value, traceback);
        self.canvas.borrow_mut(py).inner.pop_clip();
    }
}

/// [`PyCanvas::layer`]'s own returned context manager -- `__exit__` pops
/// the layer [`PyCanvas::layer`] already pushed.
#[pyclass(name = "LayerGuard")]
pub struct PyLayerGuard {
    canvas: Py<PyCanvas>,
}

#[pymethods]
impl PyLayerGuard {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (exc_type=None, exc_value=None, traceback=None))]
    fn __exit__(
        &self,
        py: Python<'_>,
        exc_type: Option<Bound<'_, PyAny>>,
        exc_value: Option<Bound<'_, PyAny>>,
        traceback: Option<Bound<'_, PyAny>>,
    ) {
        let _ = (exc_type, exc_value, traceback);
        self.canvas.borrow_mut(py).inner.pop_layer();
    }
}
