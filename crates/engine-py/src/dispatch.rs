//! The shared input pipeline and "act on a `DispatchOutcome`" logic --
//! one copy used by both `App.run()`'s live, `winit`-driven loop
//! (`app.rs`) and `window.simulate(...)` (`window_events.rs`): the
//! `HandlerMap` every stored Python callback lives in, `process_input`,
//! `run_dispatch_outcome`, and the clipboard helpers.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{
    CompletionHandle, DispatchOutcome, InputEvent, Key, NodeId, NodeKind, PointerButton, Tree,
};
use pyo3::prelude::*;

use crate::dock::{self, SharedDockState};
use crate::event::{Event, NodeContext, changed_value_to_py};
use crate::listeners::{self, EventType, WindowEventType, WindowListenerMap};
use crate::terminal::TerminalSession;

/// M16 Phase 2 (§3, §9) real finding, not anticipated in `PLAN.md`:
/// `App::run`'s own top is *not* the one guaranteed place a `tracing`
/// subscriber needs to be live. `window.simulate(...)` (and every
/// other no-live-window-needed dispatch entry point this whole
/// project's own test suite relies on) is callable without
/// `App::run()` ever running --
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
pub(crate) fn log_uncaught_exception(err: &PyErr, py: Python<'_>) {
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

/// A window's stored Python callbacks, shared by its `Node` handles --
/// named here (clippy's `type_complexity` lint) since this module is the
/// one place that interprets it. The `bool` is `wants_event`'s answer for
/// the callback, arity-sniffed once at registration: a listener that
/// declares a parameter gets the `Event`, one that doesn't is called
/// plain.
pub(crate) type HandlerMap = Rc<RefCell<HashMap<(NodeId, HandlerKey), (Py<PyAny>, bool)>>>;

/// M94: what a `HandlerMap` entry is registered for -- a `node.on(...)`
/// listener (`Listener`, routed by `listeners.rs` with the M93
/// propagation model) or one of a node's own callbacks. M100 removed the
/// legacy, non-bubbling `set_on_*` handlers that shared this map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum HandlerKey {
    Listener(EventType),
    /// M96: a node's own callbacks -- a canvas's `draw`, a virtual list's
    /// `materialize` and `size_hint` -- stored here for the same GC and
    /// pruning every listener gets.
    Draw,
    Materialize,
    SizeHint,
}

/// M54 Phase 2 (§8, §16.2): arity-sniffs `handler` at registration time
/// -- `true` when it declares at least one real *required* positional
/// parameter (it wants the `Event` argument), `false` for a
/// zero-argument handler. Uses Python's
/// own `inspect.signature` -- the general, correct way to introspect
/// an arbitrary callable (a plain function, a bound method, anything
/// with `__call__`), not `__code__.co_argcount` (which only exists on
/// plain functions, not every callable this codebase's own real
/// handlers can be). A parameter with a real default value (`lambda
/// i=i: ...`, the usual way to close over a loop index) counts as
/// *not required*, the same real distinction Python's own
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

/// M94: `node.on(event, handler)`'s own registration -- arity-sniffed
/// by `wants_event_payload`, under a `Listener` key.
pub(crate) fn register_listener(
    handlers: &HandlerMap,
    node: NodeId,
    event: EventType,
    handler: Py<PyAny>,
    py: Python<'_>,
) -> PyResult<()> {
    let wants_event = wants_event_payload(py, &handler)?;
    handlers
        .borrow_mut()
        .insert((node, HandlerKey::Listener(event)), (handler, wants_event));
    Ok(())
}

/// M94: how `Window.on` registers a window-level listener -- the same
/// arity-sniffing as node listeners, into the window's own map.
pub(crate) fn wants_event(py: Python<'_>, handler: &Py<PyAny>) -> PyResult<bool> {
    wants_event_payload(py, handler)
}

/// M9 Phase 2 (§5): the real registry `Node.animate(..., on_complete=
/// ...)` mints a fresh handle into, and `run_completions` (below)
/// drains -- the same `Rc<RefCell<...>>`-shared-into-every-`Node`
/// shape `HandlerMap` already uses. `next_id` is a plain
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

/// M54 Phase 2 (§8, §16.2): `Changed`'s own real `new_value`, read
/// fresh from `tree` -- deliberately *not* carried on `DispatchOutcome`
/// itself (`ChangedValue` only ever holds the pre-mutation value,
/// engine-core's own doc comment on it explains why). Covers the one
/// `NodeKind` `Tree::dispatch` produces a `Changed` outcome for --
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
        _ => Ok(None),
    }
}

/// The real "meaning-dependent" half `Tree::dispatch` leaves for its own
/// caller (§2 Design Principle 6) -- every mechanical consequence
/// (hover, focus movement) already happened inside `dispatch` itself.
/// Delivers each outcome to `node.on(...)` listeners (`listeners.rs`):
/// `Activated`/`SecondaryActivated` as `click`/`secondary_click`,
/// `HoverChanged` as the hover pair, `FocusChanged` as the focus pair,
/// and `Changed` (a `text_input` edit) as `change` --
/// `DispatchOutcome::None` is a no-op.
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
/// immutably, released before any real callback runs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_dispatch_outcome(
    handlers: &HandlerMap,
    tree: &Rc<RefCell<Tree>>,
    completions: &SharedCompletions,
    outcome: &DispatchOutcome,
    event: Option<&InputEvent>,
    py: Python<'_>,
) {
    let ctx = NodeContext {
        tree,
        handlers,
        completions,
    };
    match outcome {
        DispatchOutcome::Activated(node) => {
            deliver_click(&ctx, EventType::Click, *node, event, py);
        }
        DispatchOutcome::HoverChanged { old, new } => {
            let (old, new) = (*old, *new);
            // `PointerMoved` carries the pointer's position; M94's
            // `PointerLeft` (the pointer left the window) has none.
            let point = match event {
                Some(InputEvent::PointerMoved { position }) => Some(*position),
                _ => None,
            };
            listeners::route_hover(&ctx, old, new, point, py);
        }
        // M14 Phase 3, M54 Phase 2: a text input edit `Tree::dispatch`
        // detected -- `old_value` comes from `engine-core`'s snapshot;
        // `deliver_change` reads the new value fresh from `tree`.
        DispatchOutcome::Changed { node, old_value } => {
            let node = *node;
            match changed_value_to_py(py, old_value) {
                Ok(old) => deliver_change(&ctx, node, Some(old), py),
                Err(err) => log_uncaught_exception(&err, py),
            }
        }
        // M55 (§10, §16.2): a real click-to-focus or Tab-navigation
        // transition `Tree::dispatch` itself detected -- reuses the
        // shared `fire_focus_transition` (below), the identical real
        // implementation AccessKit's own `Action::Focus` handling
        // (`app.rs`) calls directly, since that path never reaches
        // `Tree::dispatch`/this function at all.
        DispatchOutcome::FocusChanged { old, new } => {
            fire_focus_transition(handlers, tree, completions, *old, *new, py);
        }
        // M4 Phase 7 (§11.3): a secondary click -- what it means (a
        // context menu, say) is the listener's to decide.
        DispatchOutcome::SecondaryActivated(node) => {
            deliver_click(&ctx, EventType::SecondaryClick, *node, event, py);
        }
        DispatchOutcome::None => {}
    }
}

/// M94: `click`/`secondary_click` listeners, bubbling from `node`. A
/// pointer activation carries its position and button; a keyboard or
/// accessibility activation has neither.
fn deliver_click(
    ctx: &NodeContext<'_>,
    event_type: EventType,
    node: NodeId,
    event: Option<&InputEvent>,
    py: Python<'_>,
) {
    let (point, button) = match event {
        Some(InputEvent::PointerReleased { position, button }) => (Some(*position), Some(*button)),
        _ => (None, None),
    };
    listeners::deliver(ctx, py, event_type, node, point, |e| {
        e.button = button.map(|b| crate::event::button_name(b).to_string());
        listeners::stamp_modifiers(e);
    });
}

/// M94: the `change` listener on a `text_input` whose text the user just
/// changed -- text-only, per M93, and non-bubbling. `old` is the text
/// before.
pub(crate) fn deliver_change(
    ctx: &NodeContext<'_>,
    node: NodeId,
    old: Option<Py<PyAny>>,
    py: Python<'_>,
) {
    let is_text_input = matches!(
        ctx.tree.borrow().get(node).map(|n| &n.kind),
        Some(NodeKind::TextField(_))
    );
    if !is_text_input {
        return;
    }
    let new = match read_new_changed_value(&ctx.tree.borrow(), node, py) {
        Ok(new) => new,
        Err(err) => {
            log_uncaught_exception(&err, py);
            return;
        }
    };
    listeners::deliver(ctx, py, EventType::Change, node, None, |e| {
        e.old_value = old;
        e.new_value = new;
    });
}

/// M100: a window's state the input pipeline reaches beyond its tree --
/// docking, window listeners, and terminal sessions.
pub(crate) struct WindowIo<'a> {
    pub(crate) dock: &'a SharedDockState,
    pub(crate) listeners: &'a WindowListenerMap,
    pub(crate) terminals: &'a SharedTerminals,
    /// 0.5.0 M3: the window itself -- its OS window, for moving and
    /// resizing it from a press, and its state.
    pub(crate) window: &'a crate::window::WindowHandles,
}

/// A window's terminal sessions, by node.
pub(crate) type SharedTerminals = Rc<RefCell<HashMap<NodeId, TerminalSession>>>;

/// M94: the one input pipeline, shared by `App.run()`'s live loop and
/// `Window.simulate`: resolve the listener target before dispatch, let
/// `Tree::dispatch` do everything mechanical, deliver the raw event to
/// `node.on(...)` listeners, then the outcome to listeners. The caller has already computed layout. M99: a
/// docking drag rides the same pipeline -- the pointer's moves report
/// `dock_target` and the primary button's release drops the panel and
/// reports `dock_drop`. M100: so do a focused terminal's keys, which go
/// to its PTY instead of the tree, a wheel over a terminal, and the
/// Ctrl shortcuts a text input or terminal handles itself -- so
/// `simulate` drives every one of them exactly as a live key does.
pub(crate) fn process_input(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    event: &InputEvent,
    py: Python<'_>,
) -> DispatchOutcome {
    // 0.5.4 (#113): fingers and trackpad pinches have a pipeline of their own,
    // which feeds the pointer events back through this one.
    if matches!(
        event,
        InputEvent::Touch { .. } | InputEvent::TrackpadPinch { .. }
    ) {
        crate::touch::process(ctx, io, root, event, py);
        return DispatchOutcome::None;
    }
    // 0.5.4 (#114): files are queued, and delivered together by `files::flush`.
    if matches!(
        event,
        InputEvent::FileHovered { .. }
            | InputEvent::FileHoverCancelled
            | InputEvent::FileDropped { .. }
    ) {
        crate::files::queue(io, event);
        return DispatchOutcome::None;
    }
    if send_to_focused_terminal(ctx, io, event) {
        return DispatchOutcome::None;
    }
    if resize_from_border(ctx, io, event) {
        return DispatchOutcome::None;
    }
    // 0.5.4 (#154): Ctrl+Shift+Left/Right move the selection in static text by
    // word. The tree's own Shift+arrow handling moves by character, so this
    // comes first and, when it moves the selection, the key goes no further.
    if let InputEvent::KeyPressed {
        key: key @ (engine_core::Key::ArrowLeft | engine_core::Key::ArrowRight),
        shift: true,
    } = event
        && listeners::modifiers().ctrl
    {
        let mut tree = ctx.tree.borrow_mut();
        let in_input = tree
            .focused()
            .is_some_and(|f| matches!(tree.get(f).map(|n| &n.kind), Some(NodeKind::TextField(_))));
        if !in_input && tree.extend_static_selection_by(*key, true) {
            return DispatchOutcome::None;
        }
    }
    listeners::note_input_modality(event);
    let shifted = shift_wheel(event);
    let delivered = shifted.as_ref().unwrap_or(event);
    let target = listeners::target_before(&ctx.tree.borrow(), root, delivered);
    let outcome =
        ctx.tree
            .borrow_mut()
            .dispatch(root, delivered.clone(), crate::clock::now(ctx.tree));
    // 0.5.0 M3: a cancelled press's release reaches no listener; a new
    // press starts afresh.
    let swallowed = match delivered {
        InputEvent::PointerPressed { .. } => {
            io.window.press_cancelled.set(false);
            false
        }
        InputEvent::PointerReleased { .. } => io.window.press_cancelled.replace(false),
        _ => false,
    };
    if !swallowed {
        listeners::route_input(ctx, target, delivered, py);
    }
    window_drag(ctx, io, delivered, target, py);
    // M96: layers an outside press or Escape asked to dismiss.
    let dismissed = ctx.tree.borrow_mut().take_dismissals();
    for layer in dismissed {
        listeners::deliver(ctx, py, listeners::EventType::Dismiss, layer, None, |_| {});
    }
    dock_drag(ctx, io.dock, io.listeners, root, event, py);
    run_dispatch_outcome(
        ctx.handlers,
        ctx.tree,
        ctx.completions,
        &outcome,
        Some(event),
        py,
    );
    shortcuts(ctx, io, root, event, py);
    keyboard_scroll(ctx, event);
    system_menu_key(io, event);
    listeners::fire_scroll_changes(ctx, py);
    outcome
}

/// 0.5.0 M3 (issue #28): which edge or corner of the window's resize border
/// `position` is on, if any -- `window.set(resize_border=N)`, the
/// framework's own border, live only while the window is undecorated (the
/// OS's decorations have their own edges) and neither maximized nor
/// fullscreen (nothing to resize). In layout pixels, like the pointer.
pub(crate) fn border_direction(
    window: &crate::window::WindowHandles,
    position: peniko::kurbo::Point,
) -> Option<winit::window::ResizeDirection> {
    let border = window.resize_border.get();
    // On macOS an undecorated window keeps the OS's own resizing (M4).
    if border <= 0.0
        || engine_platform::titlebar::OVERLAY_TITLEBAR
        || window.decorations.get()
        || window.maximized.get()
        || window.fullscreen.get()
    {
        return None;
    }
    // 0.5.4 (#102): `position` and the border are logical pixels.
    let (width, height) = window.logical_size();
    edge_at(border, width, height, position)
}

/// 0.5.0 M3: the edge or corner of a `w` x `h` window that `position` is
/// within `border` pixels of, if any -- a corner where two edges meet.
fn edge_at(
    border: f64,
    w: f64,
    h: f64,
    position: peniko::kurbo::Point,
) -> Option<winit::window::ResizeDirection> {
    use winit::window::ResizeDirection as Dir;
    let (x, y) = (position.x, position.y);
    let (west, east) = (x < border, x >= w - border);
    let (north, south) = (y < border, y >= h - border);
    Some(match (north, south, west, east) {
        (true, _, true, _) => Dir::NorthWest,
        (true, _, _, true) => Dir::NorthEast,
        (_, true, true, _) => Dir::SouthWest,
        (_, true, _, true) => Dir::SouthEast,
        (true, ..) => Dir::North,
        (_, true, ..) => Dir::South,
        (_, _, true, _) => Dir::West,
        (_, _, _, true) => Dir::East,
        _ => return None,
    })
}

/// 0.5.0 M3: a primary press on the resize border resizes the window from
/// that edge or corner (`winit`'s `drag_resize_window`) and reaches no node,
/// so a splitter or scrollbar at the edge doesn't also start its own drag;
/// its release, if it comes, is swallowed too. Whether it was taken.
fn resize_from_border(ctx: &NodeContext<'_>, io: &WindowIo<'_>, event: &InputEvent) -> bool {
    let InputEvent::PointerPressed {
        position,
        button: PointerButton::Primary,
    } = *event
    else {
        return false;
    };
    let Some(direction) = border_direction(io.window, position) else {
        return false;
    };
    if let Some(window) = io.window.os_window.borrow().as_ref() {
        let _ = window.drag_resize_window(direction);
    }
    ctx.tree.borrow_mut().cancel_press();
    io.window.press_cancelled.set(true);
    true
}

/// 0.5.0 M3 (issue #28): a primary press on a drag region -- a framework's
/// own title bar (`window_region`) -- moves the window. Runs after the
/// press reached its listeners, so one that took the pointer capture counts
/// as interactive. The move is `winit`'s `drag_window()`, which must follow
/// the press at once; the press then ends with `pointer_cancel` rather than
/// a `click`, since the platform may never deliver its release. Without an
/// open window (`simulate`) there's nothing to move, but the press still
/// ends the same way, so the decision is testable.
fn window_drag(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    event: &InputEvent,
    target: Option<NodeId>,
    py: Python<'_>,
) {
    let InputEvent::PointerPressed { position, button } = *event else {
        return;
    };
    // A fullscreen window has no frame to move, maximize, or show a menu
    // for: its drag region's presses are ordinary ones.
    let menu = button == PointerButton::Secondary && io.window.system_menu.get();
    if (button != PointerButton::Primary && !menu)
        || io.window.fullscreen.get()
        || !starts_window_drag(ctx, target)
    {
        return;
    }
    // A secondary press opens the OS's window menu there, as on a native
    // title bar -- after its `pointer_down` listeners -- when the framework
    // opted in with `system_menu` (off by default). `winit` shows it on
    // Windows and Wayland and ignores it elsewhere.
    if menu {
        if let Some(window) = io.window.os_window.borrow().as_ref() {
            window.show_window_menu(winit::dpi::PhysicalPosition::new(position.x, position.y));
        }
        end_press(ctx, io, target, position, py);
        return;
    }
    // A second press soon after, near the first, is a double-click: it
    // toggles maximize, as a native title bar's does, instead of moving.
    let now = crate::clock::now(ctx.tree);
    let double = io.window.last_drag_press.get().is_some_and(|(then, at)| {
        now.saturating_duration_since(then) <= engine_platform::double_click_time()
            && (at - position).hypot() <= DOUBLE_CLICK_SLOP
    });
    io.window
        .last_drag_press
        .set((!double).then_some((now, position)));
    // 0.5.0 M4: what the double-click does is the user's setting on macOS.
    use engine_platform::titlebar::TitleBarDoubleClick as Action;
    let action = double.then(engine_platform::titlebar::title_bar_double_click);
    match (io.window.os_window.borrow().as_ref(), action) {
        (Some(window), Some(Action::Maximize)) => window.set_maximized(!window.is_maximized()),
        (Some(window), Some(Action::Minimize)) => window.set_minimized(true),
        (Some(window), None) => {
            let _ = window.drag_window();
        }
        (None, Some(Action::Maximize)) => io.window.maximized.set(!io.window.maximized.get()),
        (None, Some(Action::Minimize)) => io.window.minimized.set(true),
        (_, Some(Action::Nothing)) | (None, None) => {}
    }
    end_press(ctx, io, target, position, py);
}

/// 0.5.0 M3: a press the window took -- to move, maximize, or show its
/// menu -- ends without a click, releasing any pointer capture, and tells
/// the pressed node with `pointer_cancel`; its release, if the platform
/// delivers one, reaches no listener.
fn end_press(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    target: Option<NodeId>,
    position: peniko::kurbo::Point,
    py: Python<'_>,
) {
    let pressed = ctx.tree.borrow_mut().cancel_press();
    io.window.press_cancelled.set(true);
    if let Some(node) = pressed.or(target) {
        listeners::deliver(
            ctx,
            py,
            EventType::PointerCancel,
            node,
            Some(position),
            |e| {
                listeners::stamp_modifiers(e);
            },
        );
    }
}

/// 0.5.0 M3: on Windows, Alt+Space opens an undecorated window's menu, which
/// the OS gives only a decorated one. (On Linux the compositor usually owns
/// the shortcut; macOS has no window menu.)
fn system_menu_key(io: &WindowIo<'_>, event: &InputEvent) {
    if !cfg!(target_os = "windows")
        || !matches!(
            event,
            InputEvent::KeyPressed {
                key: Key::Space,
                ..
            }
        )
        || !listeners::modifiers().alt
        || io.window.decorations.get()
        || !io.window.system_menu.get()
    {
        return;
    }
    if let Some(window) = io.window.os_window.borrow().as_ref() {
        window.show_window_menu(winit::dpi::PhysicalPosition::new(0.0, 0.0));
    }
}

/// 0.5.0 M3: how far apart, in pixels, a double-click's two presses may be.
const DOUBLE_CLICK_SLOP: f64 = 4.0;

/// 0.5.0 M3: the custom-windowing design's Q1 rule. Walking up from the
/// pressed node: a `window_region="drag"` node moves the window, a
/// `"none"` one doesn't, and so doesn't an interactive one met first --
/// focusable, a text input or terminal, with a `click` listener, or holding
/// the pointer capture. Hover and `pointer_down` listeners don't count, so a
/// tooltip's anchor or a context menu's target in a title bar still drags,
/// and nothing above the drag region is looked at.
fn starts_window_drag(ctx: &NodeContext<'_>, target: Option<NodeId>) -> bool {
    let Some(target) = target else {
        return false;
    };
    let tree = ctx.tree.borrow();
    let handlers = ctx.handlers.borrow();
    let capture = tree.pointer_capture();
    for id in tree.ancestors(target) {
        let Some(node) = tree.get(id) else {
            return false;
        };
        match node.window_region {
            engine_core::WindowRegion::Drag => return true,
            engine_core::WindowRegion::NoDrag => return false,
            engine_core::WindowRegion::Default => {}
        }
        let interactive = node.access.focusable == Some(true)
            || matches!(node.kind, NodeKind::TextField(_) | NodeKind::Terminal(_))
            || handlers.contains_key(&(id, HandlerKey::Listener(EventType::Click)))
            || capture == Some(id);
        if interactive {
            return false;
        }
    }
    false
}

/// 0.4.3 M17: with Shift held, a wheel that has no horizontal part scrolls
/// horizontally, as in a browser, GTK, or Qt -- most mice have no other way
/// to scroll a horizontal view. Core's `Scroll` carries no modifiers, so
/// the wheel is turned here, before dispatch; the listeners get it as
/// turned. A wheel that already has a horizontal part is left alone (macOS
/// turns Shift+wheel itself), and a terminal's scrollback (`shortcuts`)
/// reads the wheel as it came.
fn shift_wheel(event: &InputEvent) -> Option<InputEvent> {
    use engine_core::ScrollDelta;
    let InputEvent::Scroll { delta, position } = event else {
        return None;
    };
    if !listeners::modifiers().shift {
        return None;
    }
    let delta = match *delta {
        ScrollDelta::Lines(x, y) if x == 0.0 && y != 0.0 => ScrollDelta::Lines(y, 0.0),
        ScrollDelta::Pixels(x, y) if x == 0.0 && y != 0.0 => ScrollDelta::Pixels(y, 0.0),
        _ => return None,
    };
    Some(InputEvent::Scroll {
        delta,
        position: *position,
    })
}

/// 0.4.2 M12 (issue #24): the arrows, Page Up/Down, and Home/End scroll the
/// nearest scroll view around the focused node (`Tree::scroll_view_for_key`)
/// -- unless a node on the way, the scroll view included, uses the key
/// itself: a text input, which keeps its arrows, Home, and End (not Page
/// Up/Down), or any node with its own `key_down` listener, such as a slider
/// built from boxes. (A focused terminal already took every key.)
///
/// 0.4.3 M14: a key pressed with Ctrl, Alt, or Meta held is a shortcut --
/// Ctrl+Page Down switching tabs, Alt+Left going back -- so it scrolls
/// nothing. Shift still scrolls, as it does in a browser. Core's
/// `KeyPressed` carries only Shift; the rest are tracked here.
fn keyboard_scroll(ctx: &NodeContext<'_>, event: &InputEvent) {
    let InputEvent::KeyPressed { key, .. } = *event else {
        return;
    };
    let held = listeners::modifiers();
    if held.ctrl || held.alt || held.meta {
        return;
    }
    let mut tree = ctx.tree.borrow_mut();
    let Some(focused) = tree.focused() else {
        return;
    };
    let Some(view) = tree.scroll_view_for_key(focused, key) else {
        return;
    };
    let handlers = ctx.handlers.borrow();
    let pages = matches!(key, Key::PageUp | Key::PageDown);
    for id in tree.ancestors(focused) {
        let listens = handlers.contains_key(&(id, HandlerKey::Listener(EventType::KeyDown)));
        let text_input = matches!(tree.get(id).map(|n| &n.kind), Some(NodeKind::TextField(_)));
        if listens || (text_input && !pages) {
            return;
        }
        if id == view {
            break;
        }
    }
    drop(handlers);
    tree.scroll_by_key(view, key);
}

/// The focused node, when it's a terminal.
fn focused_terminal(tree: &Tree) -> Option<NodeId> {
    tree.focused()
        .filter(|&id| matches!(tree.get(id).map(|n| &n.kind), Some(NodeKind::Terminal(_))))
}

/// M30 Phase 9 Step 4, M32 Phase 4: a focused terminal claims a key
/// entirely -- its bytes, or a Ctrl+letter's control byte (SIGINT for
/// Ctrl+C), go to the PTY, and the tree never sees it (Tab included, which
/// would otherwise move focus away). Whether it claimed `event`.
fn send_to_focused_terminal(ctx: &NodeContext<'_>, io: &WindowIo<'_>, event: &InputEvent) -> bool {
    let Some(terminal) = focused_terminal(&ctx.tree.borrow()) else {
        return false;
    };
    let bytes = crate::terminal::input_bytes_for(event)
        .or_else(|| crate::terminal::control_byte_for(event).map(|byte| vec![byte]));
    let Some(bytes) = bytes else {
        return false;
    };
    if let Some(session) = io.terminals.borrow_mut().get_mut(&terminal) {
        session.write_input(&bytes);
    }
    true
}

/// M100: what a text input or terminal does with the clipboard shortcuts
/// and a wheel -- moved here from the live loop, so `simulate` reaches
/// them too. Copy, cut, and paste act on the focused text input (paste
/// types the clipboard's text, so `input` and `change` fire as for any
/// typing); Ctrl+A selects all of it; Ctrl+Shift+C copies a focused
/// terminal's selection; a wheel over a terminal scrolls its history.
fn shortcuts(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    event: &InputEvent,
    py: Python<'_>,
) {
    match event {
        InputEvent::Copy => {
            copy_focused_selection_to_clipboard(ctx.tree);
        }
        InputEvent::Cut => {
            cut_focused_selection_to_clipboard(ctx.tree, ctx.handlers, ctx.completions, py);
        }
        InputEvent::PasteRequested => {
            if let Some(text) = read_clipboard() {
                process_input(ctx, io, root, &InputEvent::TextInput(text), py);
            }
        }
        InputEvent::ControlChar('a') => {
            let focused = ctx.tree.borrow().focused();
            let in_input =
                focused.is_some_and(|field| ctx.tree.borrow_mut().select_all_text_field(field));
            if !in_input {
                // 0.5.4 (#131): with no input to select in (nothing focused, or a
                // button or row), the selected static text.
                ctx.tree.borrow_mut().select_all_static_text();
            }
        }
        InputEvent::TerminalCopyRequested => {
            let selected = {
                let tree = ctx.tree.borrow();
                focused_terminal(&tree).and_then(|id| tree.terminal_selected_text(id))
            };
            if let Some(text) = selected {
                write_clipboard(&text);
            }
        }
        // M32 Phase 5: a wheel over a terminal moves its viewport into
        // scrollback -- a positive wheel `y` (away from the user) reveals
        // older history.
        InputEvent::Scroll { delta, position } => {
            let hit = {
                let tree = ctx.tree.borrow();
                tree.hit_test(root, *position).filter(|&id| {
                    matches!(tree.get(id).map(|n| &n.kind), Some(NodeKind::Terminal(_)))
                })
            };
            if let Some(terminal) = hit {
                let lines = match delta {
                    engine_core::ScrollDelta::Lines(_, y) => *y,
                    engine_core::ScrollDelta::Pixels(_, y) => y / 20.0,
                };
                if let Some(session) = io.terminals.borrow_mut().get_mut(&terminal) {
                    session.scroll_by(&mut ctx.tree.borrow_mut(), terminal, lines.round() as i64);
                }
            }
        }
        _ => {}
    }
}

/// M99: moves a docking drag along with the pointer -- see `dock.rs`.
fn dock_drag(
    ctx: &NodeContext<'_>,
    dock: &SharedDockState,
    window_listeners: &WindowListenerMap,
    root: NodeId,
    event: &InputEvent,
    py: Python<'_>,
) {
    match *event {
        InputEvent::PointerMoved { position } => {
            if let Some(side) = dock::drag_to(dock, ctx.tree, root, position) {
                listeners::deliver_window(window_listeners, py, WindowEventType::DockTarget, |e| {
                    e.side = side.map(|s| dock::side_name(s).to_string());
                });
            }
        }
        InputEvent::PointerReleased {
            position,
            button: PointerButton::Primary,
        } => {
            if let Some((panel, side)) = dock::drop(dock, ctx.tree, root, position) {
                let panel = match Event::build_node(py, panel, ctx) {
                    Ok(panel) => Some(panel),
                    Err(err) => {
                        log_uncaught_exception(&err, py);
                        None
                    }
                };
                listeners::deliver_window(window_listeners, py, WindowEventType::DockDrop, |e| {
                    e.panel = panel;
                    e.side = side.map(|s| dock::side_name(s).to_string());
                });
            }
        }
        _ => {}
    }
}

/// M55 (§10, §16.2): the real focus-change firing logic, shared by
/// every caller that moves focus -- `run_dispatch_outcome`'s own
/// `FocusChanged` arm above (real click-to-focus/Tab navigation,
/// reached through `Tree::dispatch`), `app.rs`'s real AccessKit
/// `Action::Focus` handling (which calls `Tree::set_focus_to` directly,
/// never through `dispatch()`), and layer focus changes. Delivers the
/// `unfocus`/`focus` listener pair via `listeners::route_focus`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fire_focus_transition(
    handlers: &HandlerMap,
    tree: &Rc<RefCell<Tree>>,
    completions: &SharedCompletions,
    old: Option<NodeId>,
    new: Option<NodeId>,
    py: Python<'_>,
) {
    let ctx = NodeContext {
        tree,
        handlers,
        completions,
    };
    listeners::route_focus(&ctx, old, new, py);
}

/// M9 Phase 2 (§5): `Tree::tick_all`'s own real "meaning-dependent"
/// half -- invokes each just-completed animation's registered `on_
/// complete` callback exactly once. The same "clone out, drop the
/// borrow, *then* call" shape listener delivery uses (a callback
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

thread_local! {
    /// M100: one long-lived `arboard::Clipboard` per thread. On X11 the
    /// instance that wrote the clipboard is what serves its content, until
    /// it's dropped -- a fresh instance per call (as before) lost the text
    /// the moment the call returned, unless a clipboard manager copied it.
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

/// Runs `f` on this thread's clipboard, opening it on first use; `None`
/// when no clipboard service can be reached (logged).
fn with_clipboard<R>(f: impl FnOnce(&mut arboard::Clipboard) -> R) -> Option<R> {
    CLIPBOARD.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            match arboard::Clipboard::new() {
                Ok(clipboard) => *slot = Some(clipboard),
                Err(err) => {
                    tracing::warn!(%err, "no OS clipboard service reachable");
                    return None;
                }
            }
        }
        slot.as_mut().map(f)
    })
}

/// M100: the OS clipboard's text, or `None` when it holds none or can't
/// be reached (some headless environments have no clipboard service --
/// logged, never raised). `Window.read_clipboard` and every paste use it.
pub(crate) fn read_clipboard() -> Option<String> {
    match with_clipboard(|cb| cb.get_text())? {
        Ok(text) => Some(text),
        Err(arboard::Error::ContentNotAvailable) => None,
        Err(err) => {
            tracing::warn!(%err, "failed to read the OS clipboard");
            None
        }
    }
}

/// M100: puts `text` on the OS clipboard; `false` when it can't be
/// reached (logged, never raised). `Window.write_clipboard` and every
/// copy or cut use it.
pub(crate) fn write_clipboard(text: &str) -> bool {
    match with_clipboard(|cb| cb.set_text(text)) {
        Some(Ok(())) => true,
        Some(Err(err)) => {
            tracing::warn!(%err, "failed to write to the OS clipboard");
            false
        }
        None => false,
    }
}

/// M53 Phase 2 (§8, §10, §11.3): what `InputEvent::Copy` does -- copies
/// the focused text input's selection (`shortcuts`, above, calls it for
/// both a live key and `simulate`). `engine-core` itself never touches
/// a real clipboard (§4), so this is the one shared place with both
/// `Tree` and real `arboard` access. A clipboard failure (no real clipboard service reachable, a
/// real, possible condition in some headless environments) is logged
/// and non-fatal, returning `false` -- the same "real, expected,
/// gracefully-handled" policy this crate already established for
/// no-GPU/no-display (M16 Phase 2). Returns `true` only on a genuine,
/// complete real write.
pub(crate) fn copy_focused_selection_to_clipboard(tree: &Rc<RefCell<Tree>>) -> bool {
    // A focused text input's selection first, else (0.5.4, #112) the selected
    // static text.
    let selected = tree
        .borrow()
        .focused()
        .and_then(|field| tree.borrow().text_field_selected_text(field))
        .or_else(|| tree.borrow().static_selected_text());
    selected.is_some_and(|text| write_clipboard(&text))
}

/// `copy_focused_selection_to_clipboard`'s own real Cut sibling --
/// writes to the real clipboard *first*, using a pure read
/// (`text_field_selected_text`, not the mutating `cut_text_field_
/// selection`), and only actually removes the real selection once that
/// write genuinely succeeds. A failed clipboard write must never
/// silently destroy the user's own selected text with no way to
/// recover it. Fires `change` on a genuine cut, the same way a
/// `Tree::dispatch` edit does.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cut_focused_selection_to_clipboard(
    tree: &Rc<RefCell<Tree>>,
    handlers: &HandlerMap,
    completions: &SharedCompletions,
    py: Python<'_>,
) -> bool {
    let ctx = NodeContext {
        tree,
        handlers,
        completions,
    };
    let field_and_text = tree.borrow().focused().and_then(|field| {
        tree.borrow()
            .text_field_selected_text(field)
            .map(|text| (field, text))
    });
    let Some((field, text)) = field_and_text else {
        return false;
    };
    // A failed write leaves the selection untouched.
    if !write_clipboard(&text) {
        return false;
    }
    let old = read_new_changed_value(&tree.borrow(), field, py);
    let old_for_listeners = old
        .as_ref()
        .ok()
        .and_then(|o| o.as_ref().map(|v| v.clone_ref(py)));
    tree.borrow_mut().cut_text_field_selection(field);
    deliver_change(&ctx, field, old_for_listeners, py);
    true
}

#[cfg(test)]
mod tests {
    use super::edge_at;
    use peniko::kurbo::Point;
    use winit::window::ResizeDirection as Dir;

    #[test]
    fn edge_at_names_each_edge_and_corner_of_the_border() {
        let at = |x, y| edge_at(8.0, 400.0, 300.0, Point::new(x, y));
        assert_eq!(at(200.0, 150.0), None, "inside");
        assert_eq!(at(3.0, 150.0), Some(Dir::West));
        assert_eq!(at(396.0, 150.0), Some(Dir::East));
        assert_eq!(at(200.0, 2.0), Some(Dir::North));
        assert_eq!(at(200.0, 295.0), Some(Dir::South));
        assert_eq!(at(1.0, 1.0), Some(Dir::NorthWest));
        assert_eq!(at(399.0, 1.0), Some(Dir::NorthEast));
        assert_eq!(at(1.0, 299.0), Some(Dir::SouthWest));
        assert_eq!(at(399.0, 299.0), Some(Dir::SouthEast));
        assert_eq!(at(7.9, 150.0), Some(Dir::West), "the border's last pixel");
        assert_eq!(at(8.0, 150.0), None, "just past it");
    }
}
