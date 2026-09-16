//! `Node` (§8's `PyNode`, renamed to match what Python actually sees --
//! `tre.Node`, not `tre.PyNode`) -- narrower than §8's own full sketch:
//! `animate()`/`get()` plus, as of M4 Phase 1 step 3, `set_on_click`.
//! Still no `add_child` (no cycle to reject, so `EngineError::
//! CycleRejected` isn't implemented yet either) -- additive when its own
//! later build-order step needs it.
//!
//! `set_on_click`'s callback storage (`click_handlers`) is an
//! `Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>` *shared* with the owning
//! `PyWindow` -- created once in `window.rs`, cloned into every `Node`
//! that `Window` hands out, the exact same sharing shape `tree:
//! Rc<RefCell<Tree>>` already uses. This is what actually resolves §8's
//! own review note (a Python callback stored in a Rust struct is a real
//! GC-cycle risk unless the owning `#[pyclass]` implements `__traverse__`/
//! `__clear__`) without needing `Node` to hold a back-reference to its
//! own `PyWindow`: `PyWindow::__traverse__` already visits everything in
//! this same shared map (see `window.rs`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{Action, MotionCurve, NodeId, NodeKind, Tree};
use peniko::Color;
use pyo3::prelude::*;

use crate::error::EngineError;

#[pyclass(unsendable)]
pub struct Node {
    pub(crate) id: NodeId,
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) click_handlers: Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>,
}

#[pymethods]
impl Node {
    /// Starts (or retargets, §5's own `animate_to` semantics) an
    /// animation on one property. Registers the work and returns
    /// immediately -- never blocks waiting for the animation to finish
    /// (§8's own design rule). Dispatch is two-level per §8's review
    /// note: `PaintProperties`' own fields first (universal, every kind
    /// has them), then the node's `NodeKind` payload's fields if it has
    /// one -- `Text`'s `TextState` has none yet (§14 step 4 added no
    /// `Animated` fields to it), so that second level currently always
    /// falls through to `UnknownProperty`, which is the honest, correct
    /// behavior today, not a gap.
    #[pyo3(signature = (property, to, duration_ms=0))]
    pub(crate) fn animate(
        &self,
        property: &str,
        to: Bound<'_, PyAny>,
        duration_ms: u64,
    ) -> PyResult<()> {
        let duration = Duration::from_millis(duration_ms);
        let now = Instant::now();
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);

        match property {
            "opacity" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .opacity
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "corner_radius" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .corner_radius
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "elevation" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .elevation
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "background" => {
                let value = extract_color(&to, property)?;
                node.paint
                    .background
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            _ => {
                return Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into());
            }
        }
        Ok(())
    }

    /// Reads a numeric property's current (possibly still-animating)
    /// value -- `animate()`'s missing counterpart, added at §14 step 12
    /// once something (a binding's own applied value, §16.2) actually
    /// needed to be observed from Python rather than only ever written.
    /// `background` isn't included: it isn't a single `f64`, and
    /// nothing yet needs to read it back.
    fn get(&self, property: &str) -> PyResult<f64> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match property {
            "opacity" => Ok(node.paint.opacity.current),
            "corner_radius" => Ok(node.paint.corner_radius.current),
            "elevation" => Ok(node.paint.elevation.current),
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: property.to_string(),
            }
            .into()),
        }
    }

    /// M4 Phase 1 step 3 (§4, §11.10): registers `callback` to run when
    /// this node is *activated* -- a real primary-button click released
    /// over it, or `Enter`/`Space` while it's the keyboard-focused node
    /// (`Tree::dispatch`'s own `DispatchOutcome::Activated`, §2 Design
    /// Principle 6: `Tree` only knows *that* activation happened, this
    /// is where it's given meaning). Also adds `Action::Click` to this
    /// node's own `access.actions` if it isn't already there -- the same
    /// real signal `Tree::move_focus` already keys "interactive" off
    /// (§10), so a node this is called on becomes Tab-reachable for
    /// free, not just mouse-clickable; §10's own "keyboard operability
    /// ships from day one" stance applied to the one call site that
    /// actually makes a node interactive for the first time.
    pub(crate) fn set_on_click(&self, callback: Py<PyAny>) {
        self.click_handlers.borrow_mut().insert(self.id, callback);
        if let Some(node) = self.tree.borrow_mut().get_mut(self.id)
            && !node.access.actions.contains(&Action::Click)
        {
            node.access.actions.push(Action::Click);
        }
    }
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Rect => "Rect",
        NodeKind::Container => "Container",
        NodeKind::Text(_) => "Text",
        NodeKind::Splitter(_) => "Splitter",
        NodeKind::VirtualList(_) => "VirtualList",
    }
}

fn type_name_of(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "<unknown type>".to_string())
}

fn extract_f64(to: &Bound<'_, PyAny>, property: &str) -> Result<f64, EngineError> {
    to.extract::<f64>().map_err(|_| EngineError::TypeMismatch {
        property: property.to_string(),
        expected: "a float",
        actual: type_name_of(to),
    })
}

fn extract_color(to: &Bound<'_, PyAny>, property: &str) -> Result<Color, EngineError> {
    to.extract::<(u8, u8, u8, u8)>()
        .map(|(r, g, b, a)| Color::from_rgba8(r, g, b, a))
        .map_err(|_| EngineError::TypeMismatch {
            property: property.to_string(),
            expected: "an (r, g, b, a) tuple of 0-255 ints",
            actual: type_name_of(to),
        })
}
