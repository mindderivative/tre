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
    AccessibilityNode, AccessibilityNodeId, AccessibilityRole, LayerDesc, RenderingCanvas,
    ScissorRect, TextureFormat,
};

/// Computes the `(x, y, width, height)` a real drop-shadow's own
/// `canvas.layer(..., blur=True)` call should use for a shape at
/// `(x, y, width, height)`, cast with `(offset_x, offset_y)` and
/// `blur_margin` pixels of extra room on every side for the Dual-Kawase
/// blur to spread into (Phase 13 Step 13.4: shadows). Binds directly to
/// `tre_engine::shadow_layer_bounds` -- pure geometry, no new rendering
/// path: the shadow itself is drawn using the SAME `canvas.layer(blur=
/// True)` mechanism already real and exposed since Phase 12 Step 12.5.
#[pyfunction]
#[pyo3(signature = (x, y, width, height, offset_x, offset_y, blur_margin = 24.0))]
#[allow(
    clippy::too_many_arguments,
    reason = "one field per real shadow-bounds input; a real \
         caller almost always writes tre.shadow_layer_bounds(x, y, w, h, ox, oy)"
)]
pub fn shadow_layer_bounds(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    offset_x: f32,
    offset_y: f32,
    blur_margin: f32,
) -> (i32, i32, u32, u32) {
    tre_engine::shadow_layer_bounds(x, y, width, height, offset_x, offset_y, blur_margin)
}

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

impl From<AccessibilityRole> for PyAccessibilityRole {
    fn from(role: AccessibilityRole) -> Self {
        match role {
            AccessibilityRole::Generic => Self::Generic,
            AccessibilityRole::Button => Self::Button,
            AccessibilityRole::TextLabel => Self::TextLabel,
            AccessibilityRole::Image => Self::Image,
        }
    }
}

/// One real tagged accessibility node, as returned by
/// `Canvas.accessibility_nodes()` (Phase 18 Step 18.3) -- a plain data
/// mirror of `tre_engine::AccessibilityNode`, real world-space bounds
/// already resolved by `tag_accessibility_node`. Pass a list of these
/// straight to `A11yBridge.publish(...)` once per rendered frame.
#[pyclass(name = "AccessibilityNode")]
#[derive(Clone, Copy)]
pub struct PyAccessibilityNode {
    #[pyo3(get)]
    pub node_id: u64,
    #[pyo3(get)]
    pub x: f32,
    #[pyo3(get)]
    pub y: f32,
    #[pyo3(get)]
    pub width: f32,
    #[pyo3(get)]
    pub height: f32,
    #[pyo3(get)]
    pub role: PyAccessibilityRole,
}

impl From<AccessibilityNode> for PyAccessibilityNode {
    fn from(node: AccessibilityNode) -> Self {
        Self {
            node_id: node.node_id.0,
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            role: node.role.into(),
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

    /// Every node tagged so far this frame via `tag_accessibility_node`
    /// (Phase 18 Step 18.3) -- pass this straight to
    /// `A11yBridge.publish(...)` once per rendered frame.
    fn accessibility_nodes(&self) -> Vec<PyAccessibilityNode> {
        self.inner
            .accessibility_nodes()
            .iter()
            .copied()
            .map(PyAccessibilityNode::from)
            .collect()
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
