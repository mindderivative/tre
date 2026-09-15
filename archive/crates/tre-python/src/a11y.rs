//! `tre.A11yBridge` (GUI Readiness recommendation 7, Phase 18 Step
//! 18.3) -- binds `tre_a11y::A11yBridge` directly onto the real Linux
//! AT-SPI2 accessibility bus. A single process: no handoff file, no
//! second binary. `crates/tre-rhi-vulkan/examples/
//! canvas_accessibility_single_process_check.rs` empirically confirmed
//! (this same round, on this real desktop) that a real Vulkan-linked
//! process publishing its own real `A11yBridge` and being independently
//! queried by a genuinely separate verifier process works correctly --
//! this project's own earlier `canvas_accessibility_demo`/
//! `canvas_accessibility_verify` split existed for an unrelated reason
//! (matching real AT-SPI2 deployment practice: a real screen reader is
//! always a separate process from the app it inspects), not because a
//! real *application* -- the role `tre-python` fills here, not the AT's
//! -- can't publish its own tree from a single process. See
//! `documentation/IMPLEMENTATION.md`'s Phase 18 entry for the full,
//! corrected account of why the older framing was wrong.

use pyo3::prelude::*;
use tre_a11y::A11yBridge;
use tre_engine::AccessibilityNode;

use crate::canvas::PyAccessibilityNode;

/// A real connection to this machine's Linux AT-SPI2 accessibility bus.
/// `unsendable` (matching `PyClipboard`/`PyTrayIcon`'s own established
/// precedent for platform-connection state): stays pinned to whichever
/// Python thread created it.
#[pyclass(name = "A11yBridge", unsendable)]
pub struct PyA11yBridge {
    inner: A11yBridge,
}

#[pymethods]
impl PyA11yBridge {
    /// Connects to the real Linux accessibility bus. Infallible,
    /// matching `tre_a11y::A11yBridge::connect`'s own real contract:
    /// gracefully degrades to a permanently-inactive adapter (`publish`
    /// becomes a real no-op) when no AT-SPI2 registry is reachable,
    /// rather than raising.
    #[new]
    fn new(app_name: &str, toolkit_name: &str, toolkit_version: &str) -> Self {
        Self {
            inner: A11yBridge::connect(app_name, toolkit_name, toolkit_version),
        }
    }

    /// Publishes one frame's tagged accessibility nodes -- call this
    /// once per rendered frame, right alongside the real render call,
    /// with `canvas.accessibility_nodes()`'s own current list. Never
    /// blocks on D-Bus I/O (`tre_a11y::A11yBridge::publish`'s own
    /// already-proven contract).
    fn publish(&self, nodes: Vec<PyAccessibilityNode>) {
        let nodes: Vec<AccessibilityNode> = nodes
            .into_iter()
            .map(|n| AccessibilityNode {
                node_id: tre_engine::AccessibilityNodeId(n.node_id),
                x: n.x,
                y: n.y,
                width: n.width,
                height: n.height,
                role: n.role.into(),
            })
            .collect();
        self.inner.publish(&nodes);
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyA11yBridge>()?;
    Ok(())
}
