//! `PyWindow`'s docking methods -- thin wrappers over `dock.rs`, which
//! does the work. M99 reduced docking to D10's bare bones: the framework
//! draws the handle and the target highlight; `tre` docks, drags, and
//! reports (`dock_target`/`dock_drop` window events, delivered from
//! `dispatch::process_input`).

use std::rc::Rc;

use pyo3::prelude::*;

use crate::dock;
use crate::error::EngineError;
use crate::node::Node;
use crate::window::PyWindow;

#[pymethods]
impl PyWindow {
    /// Makes `container` the zone for `side` (`"left"`, `"right"`,
    /// `"top"`, `"bottom"`, `"center"`).
    fn add_dock_zone(&self, side: &str, container: PyRef<'_, Node>, size: f64) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::add_dock_zone(&self.handles.dock, side, container.id, size);
        Ok(())
    }

    /// Docks `panel` into `side`'s zone and shows it.
    fn dock_panel(&self, side: &str, panel: PyRef<'_, Node>) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::dock_panel(&self.handles.dock, &self.handles.tree, side, panel.id)
    }

    /// M99: shows the `index`th panel docked in `side`'s zone (was
    /// `set_active_tab`).
    fn set_active_panel(&self, side: &str, index: usize) -> PyResult<()> {
        let side = dock::parse_dock_side(side)?;
        dock::set_active_panel(&self.handles.dock, &self.handles.tree, side, index)
    }

    /// M99: starts dragging `panel`, a docked panel -- call it from the
    /// framework's own drag handle's `pointer_down`. While the pointer
    /// moves, `dock_target` reports the zone under it; the button's
    /// release moves the panel there and reports `dock_drop`.
    fn start_panel_drag(&self, panel: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.handles.tree, &panel.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        dock::start_drag(&self.handles.dock, &self.handles.tree, panel.id)
    }

    /// M105 (issue #16): takes `panel` out of its zone and off the tree;
    /// `dock_panel` can dock it again later.
    fn undock_panel(&self, panel: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.handles.tree, &panel.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        dock::undock_panel(&self.handles.dock, &self.handles.tree, panel.id)
    }
}
