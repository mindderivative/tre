//! `PyWindow` (§8's own sketch, split back out of `App` at exactly the
//! step `App`'s own module doc comment predicted -- §14 step 14, §11.1
//! multi-window). Owns one `Tree`, its root, and its own size/title --
//! everything `App::new`/`App::add_rect` used to hold directly, now per
//! window instead of assumed singular.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use peniko::Color;
use pyo3::class::{PyTraverseError, PyVisit};
use pyo3::prelude::*;
use taffy::prelude::{Rect as TaffyRect, Size, Style, length};

use crate::dispatch::{CompletionRegistry, HandlerMap, SharedCompletions};
use crate::dock::{self, SharedDockState};
use crate::listeners::WindowListenerMap;
use crate::terminal::TerminalSession;
use crate::thread_bound::{ThreadBound, thread_bound_shell};

const PADDING: f32 = 16.0;
const GAP: f32 = 16.0;

/// M33 Phase 2 (§4, §5, §8): a `Window`'s own real width/height,
/// shared the identical way `SharedTheme`/`HandlerMap`/`context_menus`
/// already are -- `Cell`, not `RefCell`, since `u32` is `Copy` and
/// every real access is a plain get/set, never a borrow that could
/// outlive a single statement. Closes the real, stated v1 limit M32
/// Phase 2 left open: a live, winit-driven resize used to reach only
/// `WindowRuntime`'s own separate, non-shared `u32` copy, never this
/// `PyWindow`'s own fields -- `App::run`'s own `WindowSetup`/
/// `WindowRuntime` now clone this same `Rc<Cell<u32>>` instead of
/// copying its value once at startup, so a real resize's own `.set()`
/// call is immediately visible to every interactive `add_*` factory
/// method's own `self.width`/`self.height` read, live.
pub(crate) type SharedSize = Rc<Cell<u32>>;

/// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): the real, atomically
/// swappable "what a live `Window` currently dispatches against and
/// paints" bundle -- `Window.show_view` writes a whole new one of these
/// in a single `RefCell` replace, and `WindowRuntime`'s own per-frame/
/// per-input closures (`app.rs`) re-sync their existing plain `tree`/
/// `root`/`handlers`/`context_menus` fields from it at the top of every
/// real invocation, instead of every one of this crate's dozens of
/// pre-existing `runtime.tree`/`.root`/`.handlers`/`.context_menus`
/// call sites needing to be rewritten through an extra layer of
/// indirection.
///
/// **Real, load-bearing correctness finding, not covered by this
/// milestone's own original plan text (which named only a `(Tree,
/// NodeId)` pair):** `engine_core::NodeId` is a `slotmap` generational
/// key (`crates/engine-core/src/node.rs`), unique only *within* the
/// `Tree` that allocated it -- two independent `View`s' own root nodes
/// can (and, confirmed by how `slotmap` allocates keys, routinely do)
/// collide on the identical raw value. `HandlerMap`/`context_menus` are
/// keyed by `(NodeId, EventKind)`/`NodeId` alone, with no per-`Tree`
/// namespacing -- sharing one persistent map across a `show_view`
/// switch would silently cross-wire a different `View`'s old callback
/// onto a colliding `NodeId` in the new one. `tree`/`root`/`handlers`/
/// `context_menus` are therefore swapped together, atomically, as one
/// unit. `theme`/`completions` deliberately stay outside it -- see
/// `Window.show_view`'s own doc comment for why.
pub(crate) struct ActiveTree {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) handlers: HandlerMap,
}

pub(crate) type SharedActiveTree = Rc<RefCell<ActiveTree>>;

/// `unsendable` (owns `Rc<RefCell<Tree>>`, §9) -- named `Window` to
/// Python, matching `Node`'s own "renamed to match what Python actually
/// sees" precedent (`tre.Window`, not `tre.PyWindow`); kept as the
/// `PyWindow` identifier on the Rust side since that's the name §8/§11.1
/// use throughout `ARCHITECTURE.md`.
///
/// `materializers` is §11.7's own "materialize item N" callback storage
/// and `handlers` (M4 Phase 1 step 3, §11.10; re-keyed by `(NodeId,
/// EventKind)` at M4 Phase 6, §16.2) is `Node.set_on_click`/
/// `set_on_hover_enter`/`set_on_hover_exit`'s -- both real cases of §8's
/// own review note: "storing a long-lived `PyObject` callback... is a
/// new risk class... unless the `#[pyclass]` implements `__traverse__`/
/// `__clear__`," which `node.rs`'s own module doc comment deferred
/// exactly this long, "until something actually stores one." Confirmed
/// directly against pyo3 0.29.2's own real API before implementing
/// (`tests/test_gc.rs`): no `#[pyclass(gc)]` flag exists or is needed in
/// this version -- a `#[pyclass]` simply implementing `__traverse__`/
/// `__clear__` in its `#[pymethods]` is enough to opt into cyclic GC
/// support, see below.
///
/// `handlers` is an `Rc<RefCell<...>>`, not a plain field, because
/// `Node.set_on_click`/etc (in `node.rs`) need to write into the *same*
/// map from a `Node` Python object that holds no back-reference to this
/// `PyWindow` -- shared the exact way `tree: Rc<RefCell<Tree>>` already
/// is between a `Window` and every `Node` it hands out.
#[pyclass(name = "Window")]
pub struct PyWindow(ThreadBound<WindowState>);
thread_bound_shell!(PyWindow => WindowState);

/// `Window`'s state (M96: behind a `ThreadBound`, see `thread_bound`).
pub struct WindowState {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) title: String,
    pub(crate) width: SharedSize,
    pub(crate) height: SharedSize,
    /// M27 Phase 3: wrapped in a `RefCell` (previously a plain
    /// `HashMap`) so `add_virtual_list` can be `&self` like every other
    /// `add_*` method -- the real, concrete need Phase 1's own stated
    /// residual limitation predicted: a real click handler that
    /// navigates to a new screen and builds a `Canvas`/`VirtualList`
    /// there (an entirely ordinary pattern) needs this, confirmed by
    /// hitting the exact predicted "Already borrowed" panic while
    /// building the showcase demo's own motion screen.
    pub(crate) materializers: RefCell<HashMap<NodeId, Py<PyAny>>>,
    pub(crate) handlers: HandlerMap,
    /// M4 Phase 9 (§11.4): real docking state -- `DockLayout` plus
    /// engine-py's own zone-container/drag-handle bookkeeping `Tree`
    /// itself never stores. Plain data, no `Py<PyAny>` involved, the
    /// same reason `context_menus` needs no GC-traversal obligation.
    pub(crate) dock: SharedDockState,
    /// M9 Phase 2 (§5): `Node.animate(..., on_complete=...)`'s own
    /// registry, shared the same way `theme` is. Holds real `Py<PyAny>`
    /// callbacks (like `handlers`, unlike `context_menus`/`dock`/
    /// `theme`) -- needs the same `__traverse__`/`__clear__` obligation
    /// below.
    pub(crate) completions: SharedCompletions,
    /// M30 Phase 9 Step 4 (§5, §8, §10): every real, live `Terminal`
    /// session this `Window` has spawned, keyed by its own real
    /// `NodeId` -- shared with `App.run`'s own per-frame `WindowRuntime`
    /// (`app.rs`'s own real drain loop), the identical real shape
    /// `context_menus`/`dock`/`theme` already have: plain data, no
    /// `Py<PyAny>` involved, so no `__traverse__`/`__clear__` GC
    /// obligation either.
    pub(crate) terminals: Rc<RefCell<HashMap<NodeId, TerminalSession>>>,
    /// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): the real, swappable
    /// "currently shown" bundle -- initialized to mirror this `Window`'s
    /// own `tree`/`root`/`handlers`/`context_menus` at construction
    /// time (`new`/`from_view`), and never read directly by any `add_*`
    /// factory below (those keep using the plain fields above
    /// unchanged) -- only `App::run`'s own `WindowSetup`/`WindowRuntime`
    /// and `show_view` (below) ever touch it.
    pub(crate) active: SharedActiveTree,
    /// M94: `window.on(...)` listeners -- the window's own, independent of
    /// which tree it shows.
    pub(crate) window_listeners: WindowListenerMap,
    /// M94: the OS window while `App.run()` has it open -- `None` before
    /// and after. `window.set(title=...)` and `window.get("scale_factor")`
    /// reach it here; `App.run()` fills and clears it.
    pub(crate) os_window: SharedOsWindow,
}

/// M94: see `PyWindow::os_window`.
pub(crate) type SharedOsWindow = Rc<RefCell<Option<std::sync::Arc<winit::window::Window>>>>;

/// Real review finding: every `add_*`/`build_shell` method below used
/// to build an identical 6-field `Node` struct literal by hand (the
/// same shared state every `Node` this `Window` hands out always
/// carries) -- factored out once, so a future new shared field (the
/// exact class of thing `completions`, M9 Phase 2, once was) only
/// needs updating here, not at every one of the 11 call sites this
/// used to be duplicated across. A plain, non-`#[pymethods]` `impl`
/// block -- `pyo3` has no way to expose a method taking a raw
/// `NodeId` as a Python-callable argument, and this helper is only
/// ever called from Rust, never from Python.
impl PyWindow {}

#[pymethods]
impl PyWindow {
    #[new]
    #[pyo3(signature = (width=480, height=200, title="tre v2"))]
    fn new(width: u32, height: u32, title: &str) -> Self {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Row,
                padding: TaffyRect {
                    left: length(PADDING),
                    right: length(PADDING),
                    top: length(PADDING),
                    bottom: length(PADDING),
                },
                gap: Size {
                    width: length(GAP),
                    height: length(GAP),
                },
                size: Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        );
        let tree = Rc::new(RefCell::new(tree));
        let handlers: HandlerMap = Rc::new(RefCell::new(HashMap::new()));
        let active = Rc::new(RefCell::new(ActiveTree {
            tree: tree.clone(),
            root,
            handlers: handlers.clone(),
        }));
        Self(ThreadBound::new(WindowState {
            tree,
            root,
            title: title.to_string(),
            width: Rc::new(Cell::new(width)),
            height: Rc::new(Cell::new(height)),
            materializers: RefCell::new(HashMap::new()),
            handlers,
            dock: Rc::new(RefCell::new(dock::DockState::new())),
            completions: Rc::new(RefCell::new(CompletionRegistry::new())),
            terminals: Rc::new(RefCell::new(HashMap::new())),
            active,
            window_listeners: Rc::new(RefCell::new(HashMap::new())),
            os_window: Rc::new(RefCell::new(None)),
        }))
    }

    /// §11.7's own claim, matching `App::run`'s existing `PyWindow::
    /// __traverse__` reference in step 14's module doc comment: every
    /// stored `PyObject` a window keeps must be visible to CPython's
    /// cyclic GC, or a materializer closure that captures this very
    /// `Window` (a plausible, real pattern -- e.g. a bound method) forms
    /// a reference cycle the refcounting GC alone can never collect.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        // M96: the collector may run on any thread; elsewhere, report
        // nothing (see `thread_bound`).
        if !self.0.is_owner() {
            return Ok(());
        }
        for materializer in self.materializers.borrow().values() {
            visit.call(materializer)?;
        }
        for (handler, _wants_event) in self.handlers.borrow().values() {
            visit.call(handler)?;
        }
        // M9 Phase 2: `completions` holds real `Py<PyAny>` callbacks --
        // the same cyclic-GC obligation as `handlers`.
        for callback in self.completions.borrow().callbacks.values() {
            visit.call(callback)?;
        }
        // M94: window listeners are stored callbacks too.
        for (handler, _wants_event) in self.window_listeners.borrow().values() {
            visit.call(handler)?;
        }
        // M42 Phase 2: after a real `show_view` switch, `self.active`'s
        // own `handlers` can be a *different* `HandlerMap` than
        // `self.handlers` above (the newly-shown `View`'s own) -- its
        // callbacks are already reachable via that `View`'s own
        // `__traverse__` too, but only for as long as that `View`'s own
        // Python wrapper object stays alive. If it doesn't (a real app
        // dropped its own reference after switching away), this Rust-
        // level `Rc` clone is still the only thing keeping those
        // callbacks alive -- traversing it directly here closes that
        // real, if narrow, gap rather than leaving a cycle CPython's own
        // collector could never find.
        //
        // **Real, load-bearing bug caught by this crate's own existing
        // `test_window_participates_in_cyclic_gc_when_a_click_handler_
        // captures_it_back` regression test, not found by inspection
        // alone:** for an ordinary `Window` that never called `show_
        // view` (the common case, `active.handlers` still the *same*
        // `Rc` as `self.handlers` above), an unconditional second loop
        // here calls `visit.call` on the identical `Py<PyAny>` object a
        // second time within this same `tp_traverse` invocation.
        // CPython's cyclic collector counts each `visit.call` as one
        // real outgoing reference when subtracting internal refs from
        // an object's total refcount -- reporting the same real,
        // single reference twice makes a genuine cycle look like it
        // still has an external referent, so it survives collection
        // (confirmed: this exact regression test started failing before
        // this `Rc::ptr_eq` guard was added). Skipped entirely unless
        // `active`'s handlers are genuinely a *different* map.
        let active_handlers = self.active.borrow().handlers.clone();
        if !Rc::ptr_eq(&self.handlers, &active_handlers) {
            for (handler, _wants_event) in active_handlers.borrow().values() {
                visit.call(handler)?;
            }
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        if !self.0.is_owner() {
            return;
        }
        self.materializers.borrow_mut().clear();
        self.handlers.borrow_mut().clear();
        self.completions.borrow_mut().callbacks.clear();
        self.active.borrow().handlers.borrow_mut().clear();
        self.window_listeners.borrow_mut().clear();
    }
}
