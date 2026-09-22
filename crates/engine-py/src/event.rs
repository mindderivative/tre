//! M54 Phase 2 (§8, §16.2): the real `Event` payload object
//! `call_handler`'s own newly arity-sniffed callback receives, when it
//! declared it wants one -- ARCHITECTURE.md §16.2's own original sketch
//! (`source`/`kind`/kind-specific `data`), reconciled against what's
//! actually available at each real dispatch site (`M54_PLAN`/`LOG.md`'s
//! own investigation) rather than reimplemented verbatim. Deliberately
//! holds no `Rc<RefCell<Tree>>`/`HandlerMap` of its own -- every field
//! is plain, already-owned data (mirroring `CanvasContext`'s own
//! "no `__traverse__`/`__clear__` needed" reasoning, `canvas.rs`), so a
//! handler that itself mutates the `Tree` it came from (a real,
//! expected pattern this crate already supports everywhere else) can
//! never re-enter through this object -- it simply has no live handle
//! to re-enter through.
//!
//! `#[pyo3(get)]` field access (`event.kind`, `event.position`, ...),
//! not this crate's own usual explicit-getter-method convention
//! (`Node.get_checked()`/`get_text()`): those exist on `Node` because
//! its own fields are live, mutable `Tree` state that can change out
//! from under a stale read -- an `Event` is an immutable snapshot,
//! handed to exactly one callback invocation and never touched again,
//! the same real distinction a plain Python `dataclass`/`namedtuple`
//! is for. Plain attribute access is the honest, idiomatic fit here,
//! not a stylistic inconsistency.

use engine_core::{ChangedValue, EventKind, PointerButton, node_id_as_u64};
use pyo3::prelude::*;

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
fn button_name(button: PointerButton) -> &'static str {
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
    /// a fourth id scheme. Deliberately not a live `Node` handle this
    /// milestone (`AskUserQuestion`, M54 scoping) -- real apps already
    /// close over the specific `Node` they attached a handler to.
    #[pyo3(get)]
    source: u64,
    #[pyo3(get)]
    position: Option<(f64, f64)>,
    #[pyo3(get)]
    button: Option<String>,
    #[pyo3(get)]
    old_value: Option<Py<PyAny>>,
    #[pyo3(get)]
    new_value: Option<Py<PyAny>>,
}

impl Event {
    pub(crate) fn click(
        node: engine_core::NodeId,
        position: Option<(f64, f64)>,
        button: Option<PointerButton>,
    ) -> Self {
        Self {
            kind: kind_name(EventKind::Click).to_string(),
            source: node_id_as_u64(node),
            position,
            button: button.map(button_name).map(str::to_string),
            old_value: None,
            new_value: None,
        }
    }

    pub(crate) fn hover(kind: EventKind, node: engine_core::NodeId, position: (f64, f64)) -> Self {
        debug_assert!(matches!(kind, EventKind::HoverEnter | EventKind::HoverExit));
        Self {
            kind: kind_name(kind).to_string(),
            source: node_id_as_u64(node),
            position: Some(position),
            button: None,
            old_value: None,
            new_value: None,
        }
    }

    pub(crate) fn change(
        node: engine_core::NodeId,
        old_value: Option<Py<PyAny>>,
        new_value: Option<Py<PyAny>>,
    ) -> Self {
        Self {
            kind: kind_name(EventKind::Change).to_string(),
            source: node_id_as_u64(node),
            position: None,
            button: None,
            old_value,
            new_value,
        }
    }

    /// M55 (§10, §16.2): `Event::hover`'s own real `Focus` sibling --
    /// mirrors its exact shape, but `position` stays `None`: unlike a
    /// hover transition (always produced by a real `PointerMoved`), a
    /// focus transition can come from a real keyboard Tab press, a
    /// real AccessKit `Action::Focus` request, or a real `Node.focus()`
    /// call, none of which carry a pointer position at all -- `None`
    /// rather than fabricating one for the one real case (click-to-
    /// focus) that happens to have one.
    pub(crate) fn focus_transition(kind: EventKind, node: engine_core::NodeId) -> Self {
        debug_assert!(matches!(kind, EventKind::FocusEnter | EventKind::FocusExit));
        Self {
            kind: kind_name(kind).to_string(),
            source: node_id_as_u64(node),
            position: None,
            button: None,
            old_value: None,
            new_value: None,
        }
    }
}
