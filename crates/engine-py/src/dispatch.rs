//! M4 Phase 1 step 3's shared "act on a `DispatchOutcome`" logic --
//! factored out once two real call sites needed it: `app.rs`'s own
//! `App.run()` render-loop `on_input` closure (the real `winit`-driven
//! path), and `window.rs`'s `Window.click()` (a direct, programmatic
//! "click this node" entry point, the same "expose a direct method
//! since real dispatch has nowhere else to originate outside a live
//! window" pattern every prior interaction step used -- `Tree::
//! spawn_ripple`, `Tree::open_overlay`, etc.). One copy of the MD3-value
//! `InteractionConfig` constants and the click-handler lookup, not two.
//!
//! M4 Phase 6 (§16.2): generalized from `run_activation`/
//! `click_handlers: HashMap<NodeId, Py<PyAny>>` (`Click`-only) into
//! `run_dispatch_outcome`/`handlers: HashMap<(NodeId, EventKind),
//! Py<PyAny>>`, once a second and third real event kind
//! (`HoverEnter`/`HoverExit`, §7.3) needed the exact same "look up a
//! registered handler for this node, call it" shape -- the Rule of
//! Three, not premature abstraction: duplicating the original
//! `click_handlers` shape a second and third time would have meant two
//! more `Rc<RefCell<HashMap<...>>>` fields apiece on `Node`/`PyWindow`/
//! `View`, for what is really one underlying concept re-keyed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{CompletionHandle, DispatchOutcome, EventKind, InteractionConfig, NodeId};
use pyo3::prelude::*;

/// The one real shape shared by `Node`/`PyWindow`/`View`'s handler
/// storage -- named here (clippy's own `type_complexity` lint, not just
/// convenience) since this module is the one place that actually
/// interprets it.
pub(crate) type HandlerMap = Rc<RefCell<HashMap<(NodeId, EventKind), Py<PyAny>>>>;

/// M9 Phase 2 (§5): the real registry `Node.animate(..., on_complete=
/// ...)` mints a fresh handle into, and `run_completions` (below)
/// drains -- the same `Rc<RefCell<...>>`-shared-into-every-`Node`
/// shape `HandlerMap`/`SharedTheme` already use. `next_id` is a plain
/// monotonic counter, not `NodeId`-derived: a `CompletionHandle` names
/// one specific *animation*, not a node -- the same node can have
/// several real completions registered (each of its own animatable
/// properties, independently) outstanding at once.
pub(crate) struct CompletionRegistry {
    next_id: u64,
    /// `pub(crate)`, not private: `PyWindow`'s own `__traverse__`/
    /// `__clear__` (real `Py<PyAny>` values -- same cyclic-GC
    /// obligation as `handlers`) need direct access, the same way
    /// `HandlerMap`'s own inner `HashMap` is accessed directly there.
    pub(crate) callbacks: HashMap<CompletionHandle, Py<PyAny>>,
}

impl CompletionRegistry {
    pub(crate) fn new() -> Self {
        Self {
            next_id: 0,
            callbacks: HashMap::new(),
        }
    }

    pub(crate) fn register(&mut self, callback: Py<PyAny>) -> CompletionHandle {
        let handle = CompletionHandle(self.next_id);
        self.next_id += 1;
        self.callbacks.insert(handle, callback);
        handle
    }
}

pub(crate) type SharedCompletions = Rc<RefCell<CompletionRegistry>>;

/// `Tree::dispatch`'s own MD3-value inputs (§1 Locked Decisions keeps
/// `engine-core` itself MD3-agnostic, so these live at the real call
/// sites instead). Real MD3 spec values where this codebase can express
/// them today (hover/focus state-layer opacity, MD3's own 0.08/0.12);
/// `ripple_radius` is a flat approximation, not computed per-node from
/// its own size the way real MD3 ripples cover a surface's diagonal
/// from the press point -- `interaction.rs`'s own doc comment already
/// named the real two-phase/per-node ripple model as separate, later
/// scope, unchanged by this step.
pub(crate) fn interaction_config() -> InteractionConfig {
    InteractionConfig {
        hover_opacity: 0.08,
        hover_duration: std::time::Duration::from_millis(100),
        focus_ring_opacity: 1.0,
        focus_ring_duration: std::time::Duration::from_millis(100),
        ripple_radius: 100.0,
        ripple_opacity: 0.12,
        ripple_duration: std::time::Duration::from_millis(300),
    }
}

/// The real "meaning-dependent" half `Tree::dispatch` leaves for its own
/// caller (§2 Design Principle 6) -- every mechanical consequence
/// (hover, focus movement, ripple-spawn-on-press) already happened
/// inside `dispatch` itself. Interprets both real outcomes today:
/// `Activated` (look up and call a registered `Click` handler) and
/// `HoverChanged` (call the old node's `HoverExit` handler, if any, and
/// the new node's `HoverEnter` handler, if any) -- `DispatchOutcome::
/// None` is a no-op.
pub(crate) fn run_dispatch_outcome(
    handlers: &HandlerMap,
    outcome: DispatchOutcome,
    py: Python<'_>,
) {
    match outcome {
        DispatchOutcome::Activated(node) => call_handler(handlers, node, EventKind::Click, py),
        DispatchOutcome::HoverChanged { old, new } => {
            if let Some(old) = old {
                call_handler(handlers, old, EventKind::HoverExit, py);
            }
            if let Some(new) = new {
                call_handler(handlers, new, EventKind::HoverEnter, py);
            }
        }
        // M14 Phase 3 (§16.7): a real `Slider` drag ending -- reuses
        // the same real `call_handler` every other mechanical outcome
        // already does, registered via `Node.set_on_change`.
        DispatchOutcome::Changed(node) => call_handler(handlers, node, EventKind::Change, py),
        // M4 Phase 7 (§11.3): `SecondaryActivated`'s real meaning is a
        // context menu, handled by `open_context_menu` below -- a
        // separate function, not a new match arm here, since it needs
        // `&mut Tree` access this function's callback-only signature
        // doesn't carry.
        DispatchOutcome::SecondaryActivated(_) | DispatchOutcome::None => {}
    }
}

/// M4 Phase 7 (§11.3): `SecondaryActivated`'s real meaning -- opens
/// `anchor`'s registered context menu, if any, via the existing real
/// `Tree::open_overlay` (§14 step 13). Guards against reopening a menu
/// that's already open (checked via `overlay_meta`) rather than
/// double-`add_child`-ing the same content, which `open_overlay`'s own
/// contract doesn't protect against itself. Deliberately does not wire
/// dismissal (`OverlayMeta.dismiss_on_outside_click`/`dismiss_on_
/// escape`) -- a real, separate, still-open gap (`overlay.rs`'s own
/// doc comment has named it since M3 step 13), not manufactured here
/// just because this phase touches the same struct.
pub(crate) fn open_context_menu(
    tree: &Rc<RefCell<engine_core::Tree>>,
    context_menus: &Rc<RefCell<HashMap<NodeId, NodeId>>>,
    root: NodeId,
    outcome: DispatchOutcome,
) {
    let DispatchOutcome::SecondaryActivated(anchor) = outcome else {
        return;
    };
    let Some(&content) = context_menus.borrow().get(&anchor) else {
        return;
    };
    let mut tree = tree.borrow_mut();
    if tree.overlay_meta(content).is_some() {
        return;
    }
    tree.open_overlay(
        root,
        anchor,
        content,
        engine_core::OverlayMeta {
            anchor,
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
        },
    );
}

/// M9 Phase 2 (§5): `Tree::tick_all`'s own real "meaning-dependent"
/// half -- invokes each just-completed animation's registered `on_
/// complete` callback exactly once. The same "clone out, drop the
/// borrow, *then* call" shape `call_handler` already uses (a callback
/// that itself registers a new `on_complete`, a real plausible
/// pattern, would otherwise panic on a re-entrant `RefCell` borrow),
/// but `HashMap::remove` instead of `get`: a real CSS `transitionend`/
/// JS Promise-style single fire, not a repeating subscription -- once
/// invoked, this exact handle can never fire again.
pub(crate) fn run_completions(
    completions: &SharedCompletions,
    completed: Vec<CompletionHandle>,
    py: Python<'_>,
) {
    for handle in completed {
        let callback = completions.borrow_mut().callbacks.remove(&handle);
        if let Some(callback) = callback
            && let Err(err) = callback.call0(py)
        {
            err.print(py);
        }
    }
}

/// M14 Phase 3 (§16.7): widened to `pub(crate)` -- `Node.set_checked`
/// reuses this directly, since a real `Checkbox` edit isn't mechanical
/// the way a `Slider` drag is (Design Principle 6: `engine-core` never
/// touches `checked` itself), so it has no `Tree::dispatch` outcome to
/// resolve through `run_dispatch_outcome` at all; calling this exact
/// same real lookup-and-invoke helper directly is the one real,
/// consistent way both components' own `Change` firing ends up going
/// through the identical mechanism, not two divergent ones.
pub(crate) fn call_handler(handlers: &HandlerMap, node: NodeId, kind: EventKind, py: Python<'_>) {
    // Cloned out and the borrow dropped *before* calling the handler: a
    // handler that itself registers a new handler (a real, plausible
    // pattern -- rebinding a button's own click behavior from inside a
    // click) would otherwise panic on a re-entrant `RefCell` borrow of
    // this same `handlers` map.
    let handler = handlers
        .borrow()
        .get(&(node, kind))
        .map(|handler| handler.clone_ref(py));
    if let Some(handler) = handler
        && let Err(err) = handler.call0(py)
    {
        // §9's own stated policy: "unhandled exceptions from a callback
        // are caught, logged, and non-fatal" -- `PyErr::print` gives the
        // same real traceback CPython itself would print for an
        // uncaught exception, not just a one-line message.
        err.print(py);
    }
}
