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

/// `unsendable` (owns `Rc<RefCell<Tree>>`, §9) -- named `Window` to
/// Python, matching `Node`'s own "renamed to match what Python actually
/// sees" precedent (`tre.Window`, not `tre.PyWindow`); kept as the
/// `PyWindow` identifier on the Rust side since that's the name §8/§11.1
/// use throughout `ARCHITECTURE.md`.
///
/// `handlers` holds every stored Python callback -- `node.on(...)`
/// listeners and a canvas's `draw`, a virtual list's `materialize` and
/// `size_hint` -- a real case of §8's own review note: "storing a long-lived `PyObject` callback... is a
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
        Self(ThreadBound::new(WindowState {
            tree,
            root,
            title: title.to_string(),
            width: Rc::new(Cell::new(width)),
            height: Rc::new(Cell::new(height)),
            handlers,
            dock: Rc::new(RefCell::new(dock::DockState::new())),
            completions: Rc::new(RefCell::new(CompletionRegistry::new())),
            terminals: Rc::new(RefCell::new(HashMap::new())),
            window_listeners: Rc::new(RefCell::new(HashMap::new())),
            os_window: Rc::new(RefCell::new(None)),
        }))
    }

    /// §11.7's own claim, matching `App::run`'s existing `PyWindow::
    /// __traverse__` reference in step 14's module doc comment: every
    /// stored `PyObject` a window keeps must be visible to CPython's
    /// cyclic GC, or a callback that captures this very
    /// `Window` (a plausible, real pattern -- e.g. a bound method) forms
    /// a reference cycle the refcounting GC alone can never collect.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        // M96: the collector may run on any thread; elsewhere, report
        // nothing (see `thread_bound`).
        if !self.0.is_owner() {
            return Ok(());
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
        Ok(())
    }

    fn __clear__(&mut self) {
        if !self.0.is_owner() {
            return;
        }
        self.handlers.borrow_mut().clear();
        self.completions.borrow_mut().callbacks.clear();
        self.window_listeners.borrow_mut().clear();
    }
}
