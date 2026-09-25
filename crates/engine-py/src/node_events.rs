//! M94: `Node`'s M93 event surface -- `on`/`off` listeners and pointer
//! capture. `listeners.rs` does the routing.

use std::rc::Rc;

use engine_core::node_id_as_u64;
use pyo3::prelude::*;

use crate::dispatch::{HandlerKey, register_listener};
use crate::listeners::EventType;
use crate::node::Node;

#[pymethods]
impl Node {
    /// Registers `handler` for `event` on this node, replacing any earlier
    /// listener for the same event. `handler` receives an `Event`, or
    /// nothing if it takes no parameters. Bubbling events also reach a
    /// node's listeners when they happen on a descendant.
    fn on(&self, event: &str, handler: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        let event = EventType::parse(event)?;
        register_listener(&self.handlers, self.id, event, handler, py)
    }

    /// Removes this node's listener for `event`, if any.
    fn off(&self, event: &str) -> PyResult<()> {
        let event = EventType::parse(event)?;
        self.handlers
            .borrow_mut()
            .remove(&(self.id, HandlerKey::Listener(event)));
        Ok(())
    }

    /// Routes every later pointer event to this node -- wherever the
    /// pointer goes -- until the pointer button is released or
    /// `release_pointer()` is called. Call it from `pointer_down`.
    fn capture_pointer(&self) {
        self.tree.borrow_mut().set_pointer_capture(Some(self.id));
    }

    /// Two handles are equal when they name the same node of the same
    /// window -- `Event.target`/`current` hand out fresh handles, so a
    /// listener compares or looks nodes up by value.
    fn __eq__(&self, other: PyRef<'_, Node>) -> bool {
        self.id == other.id && Rc::ptr_eq(&self.tree, &other.tree)
    }

    fn __hash__(&self) -> u64 {
        node_id_as_u64(self.id) ^ (Rc::as_ptr(&self.tree) as u64)
    }

    /// Ends this node's pointer capture, if it holds it.
    fn release_pointer(&self) {
        let mut tree = self.tree.borrow_mut();
        if tree.pointer_capture() == Some(self.id) {
            tree.set_pointer_capture(None);
        }
    }
}
