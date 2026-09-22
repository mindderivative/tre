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

use engine_core::{
    CompletionHandle, DispatchOutcome, EventKind, InputEvent, InteractionConfig, NodeId, NodeKind,
    Tree,
};
use peniko::kurbo::Point;
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};

use crate::event::{Event, changed_value_to_py};

/// M16 Phase 2 (§3, §9) real finding, not anticipated in `PLAN.md`:
/// `App::run`'s own top is *not* the one guaranteed place a `tracing`
/// subscriber needs to be live. `Window.click`/`Node.set_checked`/
/// `View.click` (and every other synthetic, no-live-window-needed
/// dispatch entry point this whole project's own test suite relies
/// on, deliberately, since M4 Phase 1 step 3) are all real,
/// independently callable without `App::run()` ever running --
/// confirmed the hard way, by a real pytest failure: two tests using
/// exactly those entry points captured empty stderr even though the
/// real event fired, because no subscriber had been installed yet in
/// that pytest process (cross-file test *order* had been silently
/// doing the installing until then, via whichever test file happened
/// to call `App.run()` first). `try_init` is already idempotent and
/// cheap (confirmed by M16 Phase 1's own multi-call test) -- calling
/// it again here, at the one real place every uncaught-callback-
/// exception log actually funnels through, is the correct fix: any
/// caller of `log_uncaught_exception` gets a real, working subscriber
/// regardless of whether `App::run()` was ever reached, not just
/// real apps that happen to call it first.
pub(crate) fn ensure_tracing_subscriber() {
    // M16 Phase 2 real finding, caught only by actually running an
    // example, not by reading the docs: `tracing_subscriber::fmt::
    // try_init()` (the *free function*) specially wires `EnvFilter::
    // from_default_env()` for you (confirmed via direct source read),
    // but `fmt()` (the *builder*, needed here for `.with_writer`) does
    // *not* -- its own default `filter` field is a flat `LevelFilter::
    // INFO` (`Subscriber::DEFAULT_MAX_LEVEL`, confirmed via direct
    // source read), completely ignoring `RUST_LOG`. Switching to the
    // builder for the stderr fix above silently regressed `RUST_LOG`
    // support entirely -- every example started emitting real INFO
    // events unconditionally, caught by manually re-running one after
    // this phase's own earlier stderr fix, not anticipated in advance.
    // `.with_env_filter(EnvFilter::from_default_env())` restores the
    // exact behavior the free function gave for free.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();
}

/// §9's own stated policy -- "unhandled
/// exceptions from a callback are caught, logged via `tracing::
/// error!`, and non-fatal." `PyErr` itself has no single method that
/// both formats the *full* real traceback (frames included, not just
/// the exception's own type/message -- `PyErr`'s own `Display` impl
/// only gives the latter, confirmed via direct source read) and
/// returns it as a `String` rather than writing straight to `sys.
/// stderr` (`PyErr::print`/`display`, both real, both print-only, no
/// string-returning sibling). `err.traceback(py)`'s own real `.format()`
/// (`pyo3::types::PyTracebackMethods`, confirmed via direct source
/// read -- its own doc example is literally `format!("{}{}",
/// traceback.format()?, err)`) is the one real way to get the same
/// full text `PyErr::print` would have shown, as an owned `String` a
/// `tracing::error!` field can actually carry. Falls back to the
/// exception's own plain `Display` (type + message, no frames) if
/// either the traceback is missing (a real, possible case -- an
/// exception constructed but never actually raised) or formatting it
/// itself fails, rather than losing the event entirely.
fn log_uncaught_exception(err: &PyErr, py: Python<'_>) {
    ensure_tracing_subscriber();
    let traceback = match err.traceback(py) {
        Some(tb) => match tb.format() {
            Ok(formatted) => format!("{formatted}{err}"),
            Err(_) => err.to_string(),
        },
        None => err.to_string(),
    };
    tracing::error!(%traceback, "uncaught exception in a Python callback");
}

/// The one real shape shared by `Node`/`PyWindow`/`View`'s handler
/// storage -- named here (clippy's own `type_complexity` lint, not just
/// convenience) since this module is the one place that actually
/// interprets it.
///
/// M54 Phase 2 (§8, §16.2): the stored value widened from a bare
/// `Py<PyAny>` to `(Py<PyAny>, bool)` -- the `bool` is `wants_event_
/// payload`'s own real, one-time answer for this handler, arity-
/// sniffed once at registration (`Node.set_on_click`/etc.), not
/// re-inspected on every real call. Backward compatible with every
/// pre-existing zero-argument handler by construction: `call_handler`
/// only ever calls `handler.call1(py, (event,))` when this is `true`.
pub(crate) type HandlerMap = Rc<RefCell<HashMap<(NodeId, EventKind), (Py<PyAny>, bool)>>>;

/// M54 Phase 2 (§8, §16.2): arity-sniffs `handler` at registration time
/// -- `true` when it declares at least one real *required* positional
/// parameter (it wants the new `Event` argument), `false` for the
/// 133+ pre-existing zero-argument handlers this project's own
/// `tests`/`examples` already register, confirmed via exhaustive grep
/// before this change (M54's own scoping investigation). Uses Python's
/// own `inspect.signature` -- the general, correct way to introspect
/// an arbitrary callable (a plain function, a bound method, anything
/// with `__call__`), not `__code__.co_argcount` (which only exists on
/// plain functions, not every callable this codebase's own real
/// handlers can be). A parameter with a real default value (`lambda
/// i=i: ...`, used pervasively for closing over a loop index --
/// `examples/segmented_button.py`, `tests/test_date_picker.py`, etc.)
/// counts as *not required*, the same real distinction Python's own
/// call semantics already make -- `VAR_POSITIONAL`/`VAR_KEYWORD`/
/// `KEYWORD_ONLY` parameters are skipped too, since none of them make
/// a plain positional `Event` argument mandatory.
fn wants_event_payload(py: Python<'_>, handler: &Py<PyAny>) -> PyResult<bool> {
    let inspect = py.import("inspect")?;
    let signature = inspect.call_method1("signature", (handler,))?;
    let empty = inspect.getattr("Parameter")?.getattr("empty")?;
    let parameters = signature.getattr("parameters")?.call_method0("values")?;
    for param in parameters.try_iter()? {
        let param = param?;
        let kind: String = param.getattr("kind")?.getattr("name")?.extract()?;
        if matches!(
            kind.as_str(),
            "VAR_POSITIONAL" | "VAR_KEYWORD" | "KEYWORD_ONLY"
        ) {
            continue;
        }
        let default = param.getattr("default")?;
        if !default.eq(&empty)? {
            continue;
        }
        return Ok(true);
    }
    Ok(false)
}

/// `Node.set_on_click`/`set_on_hover_enter`/`set_on_hover_exit`/
/// `set_on_change` (`node.rs`) and `view.rs`'s equivalent declarative
/// registration path all funnel through this one real place to insert
/// into a `HandlerMap` -- arity-sniffs once here, not duplicated at
/// each of those five real call sites.
pub(crate) fn register_handler(
    handlers: &HandlerMap,
    key: (NodeId, EventKind),
    handler: Py<PyAny>,
    py: Python<'_>,
) -> PyResult<()> {
    let wants_event = wants_event_payload(py, &handler)?;
    handlers.borrow_mut().insert(key, (handler, wants_event));
    Ok(())
}

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

/// Real review finding: `window.rs`'s `click`/`hover`/`scroll`/
/// `right_click` and `view.rs`'s `click`/`hover`/`right_click` each
/// built this identical "compute layout, then find a node's real
/// center point" block by hand -- 7 near-copies differing only in
/// which root to lay out from and which `AvailableSpace` to lay out
/// against (`Window`'s own real, fixed size vs. `View`'s own
/// `MaxContent`, since it has no window size of its own). Factored
/// out here, the same already-established real home for logic shared
/// between `window.rs` and `view.rs` (`interaction_config`/
/// `run_dispatch_outcome`/`open_context_menu`, all just below).
pub(crate) fn node_center(
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    available: Size<AvailableSpace>,
    node: NodeId,
) -> Point {
    let mut tree = tree.borrow_mut();
    tree.compute_layout(root, available);
    let (x, y) = tree.absolute_position(node);
    let layout = tree.layout(node);
    Point::new(
        x + f64::from(layout.size.width) / 2.0,
        y + f64::from(layout.size.height) / 2.0,
    )
}

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

/// M54 Phase 2 (§8, §16.2): `Changed`'s own real `new_value`, read
/// fresh from `tree` -- deliberately *not* carried on `DispatchOutcome`
/// itself (`ChangedValue` only ever holds the pre-mutation value,
/// engine-core's own doc comment on it explains why). Covers exactly
/// the three real `NodeKind`s `Tree::dispatch` can produce a `Changed`
/// outcome for (`tree.rs`'s own producer sites, confirmed via grep) --
/// `None` for any other kind, matching `Event`'s own "never fabricate
/// a field this event's real kind has nothing to say about" contract.
pub(crate) fn read_new_changed_value(
    tree: &Tree,
    node: NodeId,
    py: Python<'_>,
) -> PyResult<Option<Py<PyAny>>> {
    let Some(n) = tree.get(node) else {
        return Ok(None);
    };
    match &n.kind {
        NodeKind::TextField(state) => Ok(Some(
            state.content.clone().into_pyobject(py)?.unbind().into_any(),
        )),
        NodeKind::Slider(state) => Ok(Some(
            state
                .thumb_position
                .current
                .into_pyobject(py)?
                .unbind()
                .into_any(),
        )),
        NodeKind::TimePickerDial(state) => Ok(Some(
            (state.hour, state.minute)
                .into_pyobject(py)?
                .unbind()
                .into_any(),
        )),
        _ => Ok(None),
    }
}

/// The real "meaning-dependent" half `Tree::dispatch` leaves for its own
/// caller (§2 Design Principle 6) -- every mechanical consequence
/// (hover, focus movement, ripple-spawn-on-press) already happened
/// inside `dispatch` itself. Interprets every real outcome today:
/// `Activated` (look up and call a registered `Click` handler),
/// `HoverChanged` (call the old node's `HoverExit` handler, if any, and
/// the new node's `HoverEnter` handler, if any), and `Changed` (a real
/// `Slider`/`TextField`/`TimePickerDial` edit `Tree::dispatch` itself
/// detected) -- `DispatchOutcome::None` is a no-op.
///
/// M54 Phase 2 (§8, §16.2): widened to also take `event: Option<&
/// InputEvent>` -- the real, found-while-implementing-Phase-1
/// correction to the original plan: `Activated`'s own `Click` position/
/// button and `HoverChanged`'s own position need no new `engine-core`
/// data at all, since every real caller that dispatched a genuine
/// `InputEvent` already holds it in scope right here (a `PointerRelease
/// d`/`KeyPressed` for `Activated`, a `PointerMoved` for `HoverChanged`)
/// -- extracted by pattern-matching it directly. `Option` (not a bare
/// `&InputEvent`) accounts for the one real caller with no originating
/// `InputEvent` at all: `app.rs`'s real AccessKit `Action::Click`
/// handling calls `Tree::activate` directly, a semantic action a
/// screen reader requested, not a mechanical pointer/keyboard event --
/// `None` there correctly yields no position/button, the same honest
/// "don't fabricate" contract a keyboard-triggered `Click` already
/// gets. `tree` is also new here, needed only for `Changed`'s own
/// `new_value` (`read_new_changed_value`, above) -- borrowed
/// immutably, released before any real callback runs, the same
/// discipline `call_handler` itself already established for
/// `handlers`.
pub(crate) fn run_dispatch_outcome(
    handlers: &HandlerMap,
    tree: &Rc<RefCell<Tree>>,
    outcome: &DispatchOutcome,
    event: Option<&InputEvent>,
    py: Python<'_>,
) {
    match outcome {
        DispatchOutcome::Activated(node) => {
            let node = *node;
            let (position, button) = match event {
                Some(InputEvent::PointerReleased { position, button }) => {
                    (Some((position.x, position.y)), Some(*button))
                }
                // A real keyboard `Enter`/`Space` activation, or a real
                // AccessKit-driven activation with no originating
                // pointer/keyboard event at all -- no position/button
                // exists to report; `None` rather than a fabricated
                // `(0.0, 0.0)`/synthetic button.
                _ => (None, None),
            };
            call_handler(handlers, node, EventKind::Click, py, |_py| {
                Ok(Event::click(node, position, button))
            });
        }
        DispatchOutcome::HoverChanged { old, new } => {
            let (old, new) = (*old, *new);
            let position = match event {
                Some(InputEvent::PointerMoved { position }) => (position.x, position.y),
                // `update_hover` (`engine-core`) has exactly one real
                // caller, `Tree::dispatch`'s own `PointerMoved` arm --
                // confirmed via grep before this change -- so a
                // `HoverChanged` outcome paired with anything but a
                // real `PointerMoved` event is a real internal-
                // consistency bug, not a reachable user-facing
                // condition (unlike `Activated`, no real caller ever
                // produces `HoverChanged` with `event: None`).
                _ => unreachable!(
                    "DispatchOutcome::HoverChanged is only ever produced by PointerMoved dispatch"
                ),
            };
            if let Some(old) = old {
                call_handler(handlers, old, EventKind::HoverExit, py, |_py| {
                    Ok(Event::hover(EventKind::HoverExit, old, position))
                });
            }
            if let Some(new) = new {
                call_handler(handlers, new, EventKind::HoverEnter, py, |_py| {
                    Ok(Event::hover(EventKind::HoverEnter, new, position))
                });
            }
        }
        // M14 Phase 3 (§16.7), widened M54 Phase 2: a real `Slider`/
        // `TextField`/`TimePickerDial` edit `Tree::dispatch` itself
        // detected -- reuses the same real `call_handler` every other
        // mechanical outcome already does, registered via `Node.
        // set_on_change`. `old_value` comes straight from `engine-
        // core`'s own real snapshot (Phase 1); `new_value` is read
        // fresh from `tree` here, after `dispatch()` has already
        // returned.
        DispatchOutcome::Changed { node, old_value } => {
            let node = *node;
            call_handler(handlers, node, EventKind::Change, py, |py| {
                let old = Some(changed_value_to_py(py, old_value)?);
                let new = read_new_changed_value(&tree.borrow(), node, py)?;
                Ok(Event::change(node, old, new))
            });
        }
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
    outcome: &DispatchOutcome,
) {
    let DispatchOutcome::SecondaryActivated(anchor) = outcome else {
        return;
    };
    let anchor = *anchor;
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
            modal: false,
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
            log_uncaught_exception(&err, py);
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
/// M54 Phase 2 (§8, §16.2): widened with `make_event` -- called only
/// when a handler is genuinely found *and* it arity-sniffed as wanting
/// one (`wants_event_payload`, at registration) -- so building a real
/// `Event` (which can itself borrow `tree`, `read_new_changed_value`)
/// never happens on a dispatch nothing is even listening for. Returns
/// `PyResult<Event>` rather than a bare `Event`: constructing one can
/// itself fail (`changed_value_to_py`'s own `into_pyobject` calls are
/// fallible in principle, matching pyo3's own general contract) -- a
/// construction failure is logged the identical "uncaught exception,
/// non-fatal" way any other callback failure already is here, not a
/// silent swallow or a panic.
pub(crate) fn call_handler(
    handlers: &HandlerMap,
    node: NodeId,
    kind: EventKind,
    py: Python<'_>,
    make_event: impl FnOnce(Python<'_>) -> PyResult<Event>,
) {
    // Cloned out and the borrow dropped *before* calling the handler: a
    // handler that itself registers a new handler (a real, plausible
    // pattern -- rebinding a button's own click behavior from inside a
    // click) would otherwise panic on a re-entrant `RefCell` borrow of
    // this same `handlers` map.
    let handler = handlers
        .borrow()
        .get(&(node, kind))
        .map(|(handler, wants_event)| (handler.clone_ref(py), *wants_event));
    let Some((handler, wants_event)) = handler else {
        return;
    };
    let result = if wants_event {
        match make_event(py).and_then(|event| Py::new(py, event)) {
            Ok(event) => handler.call1(py, (event,)),
            Err(err) => Err(err),
        }
    } else {
        handler.call0(py)
    };
    if let Err(err) = result {
        // §9's own stated policy: "unhandled exceptions from a callback
        // are caught, logged via `tracing::error!`, and non-fatal" --
        // `log_uncaught_exception` (M16 Phase 2) carries the same full
        // real traceback `PyErr::print` used to write straight to
        // stderr, now as a real structured `tracing` event instead.
        log_uncaught_exception(&err, py);
    }
}

/// M53 Phase 2 (§8, §10, §11.3): `App::run`'s own real, winit-driven
/// `InputEvent::Copy` logic, factored out once a second real call site
/// needed it -- `Window.copy_to_system_clipboard` (`window_input.rs`),
/// the real, non-hermetic sibling this milestone adds so a context-
/// menu "Copy" item's own `on_click` callback has something real to
/// call. `engine-core` itself never touches a real clipboard (§4), so
/// this is the one shared place with both `Tree` and real `arboard`
/// access. A clipboard failure (no real clipboard service reachable, a
/// real, possible condition in some headless environments) is logged
/// and non-fatal, returning `false` -- the same "real, expected,
/// gracefully-handled" policy this crate already established for
/// no-GPU/no-display (M16 Phase 2). Returns `true` only on a genuine,
/// complete real write.
pub(crate) fn copy_focused_selection_to_clipboard(tree: &Rc<RefCell<Tree>>) -> bool {
    let selected = tree
        .borrow()
        .focused()
        .and_then(|field| tree.borrow().text_field_selected_text(field));
    let Some(text) = selected else {
        return false;
    };
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(%err, "failed to write to the real OS clipboard");
            false
        }
    }
}

/// `copy_focused_selection_to_clipboard`'s own real Cut sibling --
/// writes to the real clipboard *first*, using a pure read
/// (`text_field_selected_text`, not the mutating `cut_text_field_
/// selection`), and only actually removes the real selection once that
/// write genuinely succeeds. A failed clipboard write must never
/// silently destroy the user's own selected text with no way to
/// recover it. Fires `Change` on a genuine cut, the same way a direct,
/// non-`Tree::dispatch` mutation always does elsewhere in this crate
/// (`Node.set_checked`/`set_text`).
pub(crate) fn cut_focused_selection_to_clipboard(
    tree: &Rc<RefCell<Tree>>,
    handlers: &HandlerMap,
    py: Python<'_>,
) -> bool {
    let field_and_text = tree.borrow().focused().and_then(|field| {
        tree.borrow()
            .text_field_selected_text(field)
            .map(|text| (field, text))
    });
    let Some((field, text)) = field_and_text else {
        return false;
    };
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
        Ok(()) => {
            let old = read_new_changed_value(&tree.borrow(), field, py);
            tree.borrow_mut().cut_text_field_selection(field);
            call_handler(handlers, field, EventKind::Change, py, |py| {
                let old = old?;
                let new = read_new_changed_value(&tree.borrow(), field, py)?;
                Ok(Event::change(field, old, new))
            });
            true
        }
        Err(err) => {
            tracing::warn!(
                %err,
                "failed to write to the real OS clipboard -- selection left untouched"
            );
            false
        }
    }
}

/// `copy_focused_selection_to_clipboard`'s own real Paste sibling --
/// reads the real OS clipboard, then dispatches the resulting text
/// exactly like a real typed character (`InputEvent::TextInput`, M15
/// Phase 2's own existing mechanism, reused completely, no new
/// insertion path). `Tree::dispatch` already resolves "which field, if
/// any, is currently focused" internally for `TextInput` -- the same
/// real behavior a genuine Ctrl+V already has, so this never needs its
/// own focused-field check first. Returns whether the real clipboard
/// *read* succeeded, not whether the text landed anywhere -- the
/// identical real distinction `Window.paste`'s own hermetic sibling
/// doesn't need to make (it's handed the text directly), but a genuine
/// OS read can genuinely fail on its own.
pub(crate) fn paste_clipboard_into_focused(
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    handlers: &HandlerMap,
    py: Python<'_>,
) -> bool {
    match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
        Ok(text) => {
            let event = InputEvent::TextInput(text);
            let outcome = tree.borrow_mut().dispatch(
                root,
                event.clone(),
                &interaction_config(),
                std::time::Instant::now(),
            );
            run_dispatch_outcome(handlers, tree, &outcome, Some(&event), py);
            true
        }
        Err(err) => {
            tracing::warn!(%err, "failed to read the real OS clipboard");
            false
        }
    }
}
