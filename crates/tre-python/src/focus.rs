//! `tre.FocusableNode`/`tre.FocusManager` (Phase 19 Step 19.3) -- thin
//! PyO3 wrappers over `tre_engine::focus`'s real in-app widget focus
//! and Tab-order traversal, mirroring `a11y.rs`'s own `register()`
//! pattern.

use pyo3::prelude::*;
use tre_engine::{AccessibilityNodeId, FocusManager, FocusableNode};

/// A plain data mirror of `tre_engine::FocusableNode`, as returned by
/// `Canvas.focusable_nodes()`. Pass a list of these to
/// `FocusManager.focus_next`/`focus_previous`.
#[pyclass(name = "FocusableNode")]
#[derive(Clone, Copy)]
pub struct PyFocusableNode {
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
    pub tab_index: Option<i32>,
}

impl From<FocusableNode> for PyFocusableNode {
    fn from(node: FocusableNode) -> Self {
        Self {
            node_id: node.node_id.0,
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            tab_index: node.tab_index,
        }
    }
}

impl From<PyFocusableNode> for FocusableNode {
    fn from(node: PyFocusableNode) -> Self {
        Self {
            node_id: AccessibilityNodeId(node.node_id),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            tab_index: node.tab_index,
        }
    }
}

/// `tre.FocusManager` -- persistent in-app widget focus state (Phase 19
/// Step 19.2). NOT `unsendable`: pure in-memory logic, no OS/platform
/// connection handle, unlike `A11yBridge`/`TrayIcon`/`Clipboard`.
#[pyclass(name = "FocusManager")]
pub struct PyFocusManager {
    inner: FocusManager,
}

#[pymethods]
impl PyFocusManager {
    #[new]
    fn new() -> Self {
        Self {
            inner: FocusManager::new(),
        }
    }

    fn focused(&self) -> Option<u64> {
        self.inner.focused().map(|id| id.0)
    }

    fn set_focus(&mut self, node_id: Option<u64>) {
        self.inner.set_focus(node_id.map(AccessibilityNodeId));
    }

    fn focus_next(&mut self, nodes: Vec<PyFocusableNode>) -> Option<u64> {
        let nodes: Vec<FocusableNode> = nodes.into_iter().map(FocusableNode::from).collect();
        self.inner.focus_next(&nodes).map(|id| id.0)
    }

    fn focus_previous(&mut self, nodes: Vec<PyFocusableNode>) -> Option<u64> {
        let nodes: Vec<FocusableNode> = nodes.into_iter().map(FocusableNode::from).collect();
        self.inner.focus_previous(&nodes).map(|id| id.0)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFocusableNode>()?;
    m.add_class::<PyFocusManager>()?;
    Ok(())
}
