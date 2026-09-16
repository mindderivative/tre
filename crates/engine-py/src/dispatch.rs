//! M4 Phase 1 step 3's shared "act on a `DispatchOutcome`" logic --
//! factored out once two real call sites needed it: `app.rs`'s own
//! `App.run()` render-loop `on_input` closure (the real `winit`-driven
//! path), and `window.rs`'s `Window.click()` (a direct, programmatic
//! "click this node" entry point, the same "expose a direct method
//! since real dispatch has nowhere else to originate outside a live
//! window" pattern every prior interaction step used -- `Tree::
//! spawn_ripple`, `Tree::open_overlay`, etc.). One copy of the MD3-value
//! `InteractionConfig` constants and the click-handler lookup, not two.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{DispatchOutcome, InteractionConfig, NodeId};
use pyo3::prelude::*;

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
/// inside `dispatch` itself; this is only ever reached for
/// `DispatchOutcome::Activated`, and its only job is to look up and call
/// a registered `Node.set_on_click` handler, if any.
pub(crate) fn run_activation(
    click_handlers: &Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>,
    outcome: DispatchOutcome,
    py: Python<'_>,
) {
    let DispatchOutcome::Activated(node) = outcome else {
        return;
    };
    // Cloned out and the borrow dropped *before* calling the handler: a
    // handler that itself calls `set_on_click` (a real, plausible
    // pattern -- rebinding a button's own click behavior from inside a
    // click) would otherwise panic on a re-entrant `RefCell` borrow of
    // this same `click_handlers` map.
    let handler = click_handlers
        .borrow()
        .get(&node)
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
