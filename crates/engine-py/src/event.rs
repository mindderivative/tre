//! M54 Phase 2 (§8, §16.2): the real `Event` payload object a
//! `node.on(...)`/`window.on(...)` listener receives, when it declared
//! it wants one (arity-sniffed at registration, `dispatch.rs`). M94
//! reshaped it around the M93 target API (`type`, `target`, `current`,
//! and per-event fields).
//!
//! Re-entrancy is exactly as safe as this crate's own established
//! "clone the `Rc`s out, drop the borrow, *then* call" discipline
//! already guarantees everywhere else: every `Event` is fully built,
//! `Py<Node>` handles included, before the real Python handler ever
//! runs, and building a `Node` costs only cheap `Rc` clones, never a
//! `Tree` borrow of its own.
//!
//! `#[pyo3(get)]` field access (`event.type`, `event.x`, ...), not
//! `Node`'s `get(name)`: a node's properties are live, mutable `Tree`
//! state that can change out from under a stale read -- an `Event` is
//! an immutable snapshot, handed to one dispatch and never touched
//! again, the same real distinction a plain Python `dataclass` is for.
//! (`target`/`current` are live `Node` handles, not inert data, but
//! they're still handed out once and never mutated by `Event` itself.)

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{ChangedValue, NodeId, PointerButton, Tree};
use pyo3::prelude::*;

use crate::dispatch::{HandlerMap, SharedCompletions};
use crate::node::{Node, NodeState};

/// M56 (§8, §16.2): the shared handles every `Node` needs besides its
/// own `id` -- the bundle `Node`'s own struct already carries
/// (`node.rs`), and `PyWindow`/`WindowRuntime` each already hold as
/// their own fields. Named once here since these travel together
/// through every delivery path's plain function signature
/// (`dispatch::run_dispatch_outcome`, `dispatch::fire_focus_transition`,
/// `listeners.rs`). Borrowed, not owned, since most real callers
/// already hold these as borrows/refs too, so cloning into a `Node`
/// only happens once, right where one is actually being built.
pub(crate) struct NodeContext<'a> {
    pub(crate) tree: &'a Rc<RefCell<Tree>>,
    pub(crate) handlers: &'a HandlerMap,
    pub(crate) completions: &'a SharedCompletions,
}

/// `PointerButton`'s own real Python-facing name, lowercased from its
/// Rust variant name -- no existing cross-boundary convention to match
/// (`PointerButton` has never reached Python before this), so this is
/// simply the plainest honest mapping.
pub(crate) fn button_name(button: PointerButton) -> &'static str {
    match button {
        PointerButton::Primary => "primary",
        PointerButton::Secondary => "secondary",
        PointerButton::Middle => "middle",
        PointerButton::Back => "back",
        PointerButton::Forward => "forward",
    }
}

/// `ChangedValue`'s own real conversion to a Python value -- the one
/// place a `Tree::dispatch`-detected `Changed` outcome's mechanical
/// `old_value` becomes a real Python object. A cut's own direct
/// (non-`Tree::dispatch`) `change` firing never goes through this --
/// it reads its `old` value straight from the tree.
pub(crate) fn changed_value_to_py(py: Python<'_>, value: &ChangedValue) -> PyResult<Py<PyAny>> {
    match value {
        ChangedValue::Text(text) => Ok(text.into_pyobject(py)?.unbind().into_any()),
    }
}

/// The real payload object every arity-sniffed handler that declared it
/// wants one receives as its own one real argument. Every field beyond
/// `kind`/`source` is `None` when this event's own real `kind` has
/// nothing to say about it -- never fabricated (a `click` from a real
/// keyboard `Enter`/`Space` activation genuinely has no pointer
/// position/button; a `pointer_enter`/`pointer_leave` genuinely has no
/// old/new value).
#[pyclass(name = "Event")]
pub struct Event {
    #[pyo3(get)]
    pub(crate) button: Option<String>,
    #[pyo3(get)]
    pub(crate) old_value: Option<Py<PyAny>>,
    #[pyo3(get)]
    pub(crate) new_value: Option<Py<PyAny>>,
    // M94: the M93 target-API fields. `type`/`target` are set for every
    // event; the rest only where the event has
    // something to say, never fabricated.
    #[pyo3(get, name = "type")]
    event_type: String,
    #[pyo3(get)]
    target: Option<Py<Node>>,
    /// The node whose listener is running -- updated at each step of
    /// bubbling, as are `x`/`y`, which are local to it.
    #[pyo3(get)]
    pub(crate) current: Option<Py<Node>>,
    #[pyo3(get)]
    pub(crate) x: Option<f64>,
    #[pyo3(get)]
    pub(crate) y: Option<f64>,
    #[pyo3(get)]
    pub(crate) window_x: Option<f64>,
    #[pyo3(get)]
    pub(crate) window_y: Option<f64>,
    #[pyo3(get)]
    pub(crate) delta_x: Option<f64>,
    #[pyo3(get)]
    pub(crate) delta_y: Option<f64>,
    #[pyo3(get)]
    pub(crate) key: Option<String>,
    #[pyo3(get)]
    pub(crate) repeat: Option<bool>,
    #[pyo3(get)]
    pub(crate) shift: Option<bool>,
    #[pyo3(get)]
    pub(crate) ctrl: Option<bool>,
    #[pyo3(get)]
    pub(crate) alt: Option<bool>,
    #[pyo3(get)]
    pub(crate) meta: Option<bool>,
    #[pyo3(get)]
    pub(crate) text: Option<String>,
    #[pyo3(get)]
    pub(crate) action: Option<String>,
    #[pyo3(get)]
    pub(crate) value: Option<Py<PyAny>>,
    #[pyo3(get)]
    pub(crate) width: Option<f64>,
    #[pyo3(get)]
    pub(crate) height: Option<f64>,
    #[pyo3(get)]
    pub(crate) dark: Option<bool>,
    /// 0.5.0 M2: the window's `maximized` event -- whether it now is.
    #[pyo3(get)]
    pub(crate) maximized: Option<bool>,
    /// 0.5.0 M2: the window's `active` event -- whether it now has focus.
    #[pyo3(get)]
    pub(crate) active: Option<bool>,
    /// 0.5.0 M4: the window's `titlebar_inset` event -- the new
    /// `(height, width)`.
    #[pyo3(get)]
    pub(crate) titlebar_inset: Option<(f64, f64)>,
    #[pyo3(get)]
    pub(crate) scale_factor: Option<f64>,
    /// `focus`/`unfocus`: the node on the other side of the move -- losing
    /// focus for `focus`, gaining it for `unfocus`.
    #[pyo3(get)]
    pub(crate) related_target: Option<Py<Node>>,
    /// `focus`: whether focus arrived by keyboard or assistive technology
    /// rather than a pointer press (`listeners::note_input_modality`).
    #[pyo3(get)]
    pub(crate) focus_visible: Option<bool>,
    /// M99: `dock_target`/`dock_drop`: the dock zone -- `"left"`,
    /// `"right"`, `"top"`, `"bottom"`, or `"center"` -- or `None` when the
    /// pointer is over no zone.
    #[pyo3(get)]
    pub(crate) side: Option<String>,
    /// M99: `dock_drop`: the panel whose drag ended.
    #[pyo3(get)]
    pub(crate) panel: Option<Py<Node>>,
    pub(crate) stopped: bool,
    pub(crate) cancelled: bool,
    pub(crate) cancellable: bool,
}

#[pymethods]
impl Event {
    /// Ends propagation: no listener on a further ancestor runs. A no-op
    /// on an event that doesn't bubble.
    fn stop(&mut self) {
        self.stopped = true;
    }

    /// Prevents a cancellable event's default -- today only the window's
    /// `close_requested`, which then leaves the window open.
    fn cancel(&mut self) -> PyResult<()> {
        if !self.cancellable {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "`{}` can't be cancelled -- only `close_requested` can",
                self.event_type
            )));
        }
        self.cancelled = true;
        Ok(())
    }
}

impl Event {
    /// M56 (§8, §16.2): builds a real, live `Node` handle (`Event.target`,
    /// `current`, ...) from the same shared handles every `Node` a
    /// `Window` hands out carries (`node.rs`'s own module doc comment).
    /// Fallible (`Py::new`) -- the same reason every other real place
    /// this crate builds a fresh pyclass instance inside a dispatch path
    /// is fallible too.
    pub(crate) fn build_node(
        py: Python<'_>,
        id: NodeId,
        ctx: &NodeContext<'_>,
    ) -> PyResult<Py<Node>> {
        Py::new(
            py,
            Node::from(NodeState {
                id,
                tree: ctx.tree.clone(),
                handlers: ctx.handlers.clone(),
                completions: ctx.completions.clone(),
            }),
        )
    }

    /// M94: an event with only its names and node set -- every other
    /// field `None`, for the constructor to fill in what applies.
    fn blank(event_type: &str, target: Option<Py<Node>>) -> Self {
        Self {
            target,
            button: None,
            old_value: None,
            new_value: None,
            event_type: event_type.to_string(),
            current: None,
            x: None,
            y: None,
            window_x: None,
            window_y: None,
            delta_x: None,
            delta_y: None,
            key: None,
            repeat: None,
            shift: None,
            ctrl: None,
            alt: None,
            meta: None,
            text: None,
            action: None,
            value: None,
            width: None,
            height: None,
            dark: None,
            maximized: None,
            active: None,
            titlebar_inset: None,
            scale_factor: None,
            related_target: None,
            focus_visible: None,
            side: None,
            panel: None,
            stopped: false,
            cancelled: false,
            cancellable: false,
        }
    }

    /// M94: a `node.on(...)` listener event aimed at `target`.
    pub(crate) fn for_node(
        py: Python<'_>,
        event_type: &str,
        target: NodeId,
        ctx: &NodeContext<'_>,
    ) -> PyResult<Self> {
        Ok(Self::blank(
            event_type,
            Some(Self::build_node(py, target, ctx)?),
        ))
    }

    /// M94: a `window.on(...)` listener event -- no node at all.
    pub(crate) fn for_window(event_type: &str) -> Self {
        Self::blank(event_type, None)
    }
}
