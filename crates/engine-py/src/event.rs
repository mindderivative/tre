//! M54 Phase 2 (§8, §16.2): the real `Event` payload object
//! `call_handler`'s own newly arity-sniffed callback receives, when it
//! declared it wants one -- ARCHITECTURE.md §16.2's own original sketch
//! (`source`/`kind`/kind-specific `data`), reconciled against what's
//! actually available at each real dispatch site (`M54_PLAN`/`LOG.md`'s
//! own investigation) rather than reimplemented verbatim.
//!
//! M56 (§8, §16.2): `Event.node`, a real live `Node` handle, alongside
//! the plain opaque `source: u64` id M54 shipped -- the deferred design
//! question both M54 and M55 named and left open ("apps already close
//! over the specific `Node` they attached a handler to") is real for
//! *that* handler, but not for one shared generically across several
//! nodes, which has no way to know which one just fired without this.
//! Re-entrancy is exactly as safe as this crate's own established
//! "clone the `Rc`s out, drop the borrow, *then* call" discipline
//! already guarantees everywhere else (`call_handler`'s own doc
//! comment): every `Event` is fully built, `Py<Node>` included, before
//! the real Python handler ever runs, and building a `Node` costs only
//! cheap `Rc` clones, never a `Tree` borrow of its own.
//!
//! `#[pyo3(get)]` field access (`event.kind`, `event.position`, ...),
//! not this crate's own usual explicit-getter-method convention
//! (`Node.get_checked()`/`get_text()`): those exist on `Node` because
//! its own fields are live, mutable `Tree` state that can change out
//! from under a stale read -- an `Event` is an immutable snapshot,
//! handed to exactly one callback invocation and never touched again,
//! the same real distinction a plain Python `dataclass`/`namedtuple`
//! is for. Plain attribute access is the honest, idiomatic fit here,
//! not a stylistic inconsistency. (`Event.node` is the one field this
//! doesn't quite apply to -- `Node` itself is a live handle, not inert
//! data, but it's still handed out once and never mutated by `Event`
//! itself, so exposing it as a plain attribute stays consistent.)

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{ChangedValue, EventKind, NodeId, PointerButton, Tree, node_id_as_u64};
use pyo3::prelude::*;

use crate::dispatch::{HandlerMap, SharedCompletions};
use crate::node::{Node, NodeState};
use crate::window::SharedTheme;

/// M56 (§8, §16.2): the 5 real handles every `Node` needs besides its
/// own `id` -- the identical bundle `Node`'s own struct already carries
/// (`node.rs`), and `PyWindow`/`View`/`WindowRuntime` each already hold
/// as their own fields. Named once here since this is the third-plus
/// real place this exact 5-tuple needs to travel together through a
/// plain function signature (`Event::click`/`hover`/`change`/`focus_
/// transition`, `dispatch::run_dispatch_outcome`, `dispatch::fire_
/// focus_transition`) -- the Rule of Three, the same real threshold
/// `HandlerMap`'s own doc comment already used to justify factoring
/// out a shared type, not speculative abstraction. Borrowed, not
/// owned, since most real callers already hold these as borrows/refs
/// too, so cloning into a `Node` only happens once, right where one is
/// actually being built.
pub(crate) struct NodeContext<'a> {
    pub(crate) tree: &'a Rc<RefCell<Tree>>,
    pub(crate) handlers: &'a HandlerMap,
    pub(crate) context_menus: &'a Rc<RefCell<HashMap<NodeId, NodeId>>>,
    pub(crate) theme: &'a SharedTheme,
    pub(crate) completions: &'a SharedCompletions,
}

/// `EventKind`'s own real Python-facing name -- lowercase, unprefixed
/// (`"click"`, not `"on_click"` -- that prefix is `view.rs`'s own YAML
/// *attribute name* convention, a different real thing: which
/// declarative binding this maps to, not the event's own kind).
fn kind_name(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Click => "click",
        EventKind::HoverEnter => "hover_enter",
        EventKind::HoverExit => "hover_exit",
        EventKind::Change => "change",
        EventKind::FocusEnter => "focus_enter",
        EventKind::FocusExit => "focus_exit",
    }
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
    }
}

/// `ChangedValue`'s own real conversion to a Python value -- the one
/// place a `Tree::dispatch`-detected `Changed` outcome's mechanical
/// `old_value` becomes a real Python object. `Node.set_checked`/
/// `set_selected`/`set_on`/`set_text` and `Window.cut`'s own direct
/// (non-`Tree::dispatch`) `Change` firing never go through this at all
/// -- their own `old`/`new` values are already real Python-native
/// `bool`/`str` at the point they fire, constructed directly there.
pub(crate) fn changed_value_to_py(py: Python<'_>, value: &ChangedValue) -> PyResult<Py<PyAny>> {
    match value {
        ChangedValue::Text(text) => Ok(text.into_pyobject(py)?.unbind().into_any()),
        ChangedValue::Number(n) => Ok(n.into_pyobject(py)?.unbind().into_any()),
        ChangedValue::Time { hour, minute } => {
            Ok((*hour, *minute).into_pyobject(py)?.unbind().into_any())
        }
    }
}

/// The real payload object every arity-sniffed handler that declared it
/// wants one receives as its own one real argument. Every field beyond
/// `kind`/`source` is `None` when this event's own real `kind` has
/// nothing to say about it -- never fabricated (a `Click` from a real
/// keyboard `Enter`/`Space` activation genuinely has no pointer
/// position/button; a `HoverEnter`/`HoverExit` genuinely has no
/// old/new value).
#[pyclass(name = "Event")]
pub struct Event {
    #[pyo3(get)]
    kind: String,
    /// A stable, opaque `u64` identity for the node this event fired
    /// on -- `engine_core::node_id_as_u64`, the exact same real
    /// `slotmap::KeyData::as_ffi()` encoding this crate already uses
    /// for `accesskit`/`engine-render`'s own GPU texture cache keys
    /// (`tree.rs`'s own `to_access_id`/`node_id_as_u64`), reused here
    /// for a third real foreign-handle consumer rather than inventing
    /// a fourth id scheme. Kept unchanged since M54 shipped it
    /// (`AskUserQuestion`, M56 scoping) -- `node` (below) is the real,
    /// live counterpart for the case this alone can't serve. `0` for a
    /// window event (M94), which has no node.
    #[pyo3(get)]
    source: u64,
    /// M56 (§8, §16.2): the real, live `Node` this event fired on --
    /// additive alongside `source`, not a replacement. The one real thing
    /// `source`'s bare id can't serve: a single handler registered
    /// generically across several nodes has no way to know *which* one
    /// just fired without this. M94: `None` only for a window event, whose
    /// `node` getter raises instead (see `Event::node`).
    node: Option<Py<Node>>,
    #[pyo3(get)]
    position: Option<(f64, f64)>,
    #[pyo3(get)]
    pub(crate) button: Option<String>,
    #[pyo3(get)]
    pub(crate) old_value: Option<Py<PyAny>>,
    #[pyo3(get)]
    pub(crate) new_value: Option<Py<PyAny>>,
    // M94: the M93 target-API fields. `type`/`target` are set for every
    // event (legacy ones too); the rest only where the event has
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
    #[pyo3(get)]
    pub(crate) scale_factor: Option<f64>,
    /// `focus`/`blur`: the node on the other side of the move -- losing
    /// focus for `focus`, gaining it for `blur`.
    #[pyo3(get)]
    pub(crate) related_target: Option<Py<Node>>,
    /// `focus`: whether focus arrived by keyboard or assistive technology
    /// rather than a pointer press (`listeners::note_input_modality`).
    #[pyo3(get)]
    pub(crate) focus_visible: Option<bool>,
    pub(crate) stopped: bool,
    pub(crate) cancelled: bool,
    pub(crate) cancellable: bool,
}

#[pymethods]
impl Event {
    /// The node the event fired on (legacy name for `target`). A window
    /// event has none, so this raises rather than returning `None` --
    /// keeping the long-standing `node: Node` type exact.
    #[getter]
    fn node(&self, py: Python<'_>) -> PyResult<Py<Node>> {
        self.node.as_ref().map(|n| n.clone_ref(py)).ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err(format!(
                "a `{}` window event has no node -- window events have no target",
                self.event_type
            ))
        })
    }

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
    /// M56 (§8, §16.2): builds the real, live `Node` handle `Event.node`
    /// carries -- the exact same 6-field construction pattern every
    /// real `add_*` factory already uses (`node.rs`'s own module doc
    /// comment: "created once there, cloned into every `Node` handed
    /// out"), reused here rather than reinvented. Fallible (`Py::new`)
    /// -- the same reason every other real place this crate builds a
    /// fresh pyclass instance inside a dispatch path is fallible too.
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
                context_menus: ctx.context_menus.clone(),
                theme: ctx.theme.clone(),
                completions: ctx.completions.clone(),
            }),
        )
    }

    /// M94: an event with only its names and node set -- every other
    /// field `None`, for the constructor to fill in what applies.
    fn blank(
        kind: &str,
        event_type: &str,
        source: u64,
        node: Option<Py<Node>>,
        py: Python<'_>,
    ) -> Self {
        Self {
            kind: kind.to_string(),
            source,
            target: node.as_ref().map(|n| n.clone_ref(py)),
            node,
            position: None,
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
            scale_factor: None,
            related_target: None,
            focus_visible: None,
            stopped: false,
            cancelled: false,
            cancellable: false,
        }
    }

    /// M94: a legacy event, `type` equal to its legacy `kind`.
    fn legacy(
        py: Python<'_>,
        kind: EventKind,
        node: NodeId,
        ctx: &NodeContext<'_>,
    ) -> PyResult<Self> {
        let name = kind_name(kind);
        Ok(Self::blank(
            name,
            name,
            node_id_as_u64(node),
            Some(Self::build_node(py, node, ctx)?),
            py,
        ))
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
            event_type,
            node_id_as_u64(target),
            Some(Self::build_node(py, target, ctx)?),
            py,
        ))
    }

    /// M94: a `window.on(...)` listener event -- no node at all.
    pub(crate) fn for_window(py: Python<'_>, event_type: &str) -> Self {
        Self::blank(event_type, event_type, 0, None, py)
    }

    pub(crate) fn click(
        py: Python<'_>,
        node: NodeId,
        ctx: &NodeContext<'_>,
        position: Option<(f64, f64)>,
        button: Option<PointerButton>,
    ) -> PyResult<Self> {
        let mut event = Self::legacy(py, EventKind::Click, node, ctx)?;
        event.position = position;
        event.button = button.map(button_name).map(str::to_string);
        Ok(event)
    }

    /// M94: `position` is `None` for a hover exit caused by the pointer
    /// leaving the window, which has no position inside it.
    pub(crate) fn hover(
        py: Python<'_>,
        kind: EventKind,
        node: NodeId,
        ctx: &NodeContext<'_>,
        position: Option<(f64, f64)>,
    ) -> PyResult<Self> {
        debug_assert!(matches!(kind, EventKind::HoverEnter | EventKind::HoverExit));
        let mut event = Self::legacy(py, kind, node, ctx)?;
        event.position = position;
        Ok(event)
    }

    pub(crate) fn change(
        py: Python<'_>,
        node: NodeId,
        ctx: &NodeContext<'_>,
        old_value: Option<Py<PyAny>>,
        new_value: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let mut event = Self::legacy(py, EventKind::Change, node, ctx)?;
        event.old_value = old_value;
        event.new_value = new_value;
        Ok(event)
    }

    /// M55 (§10, §16.2): `Event::hover`'s own real `Focus` sibling --
    /// `position` stays `None`: a focus transition can come from a real
    /// keyboard Tab press, a real AccessKit `Action::Focus` request, or a
    /// real `Window.focus()`/`View.focus()` call, none of which carry a
    /// pointer position at all.
    pub(crate) fn focus_transition(
        py: Python<'_>,
        kind: EventKind,
        node: NodeId,
        ctx: &NodeContext<'_>,
    ) -> PyResult<Self> {
        debug_assert!(matches!(kind, EventKind::FocusEnter | EventKind::FocusExit));
        Self::legacy(py, kind, node, ctx)
    }
}
