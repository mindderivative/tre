//! `PyWindow` (§8's own sketch, split back out of `App` at exactly the
//! step `App`'s own module doc comment predicted -- §14 step 14, §11.1
//! multi-window). Owns one `Tree`, its root, and its own size/title,
//! per window instead of assumed singular. `window.create(kind, ...)`
//! and the rest of its Python methods live in the `window_*` modules.

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
/// shared the identical way `HandlerMap`/`SharedDockState` already
/// are -- `Cell`, not `RefCell`, since `u32` is `Copy` and
/// every real access is a plain get/set, never a borrow that could
/// outlive a single statement. Closes the real, stated v1 limit M32
/// Phase 2 left open: a live, winit-driven resize used to reach only
/// `WindowRuntime`'s own separate, non-shared `u32` copy, never this
/// `PyWindow`'s own fields -- `App::run`'s own `WindowSetup`/
/// `WindowRuntime` now clone this same `Rc<Cell<u32>>` instead of
/// copying its value once at startup, so a real resize's own `.set()`
/// call is immediately visible to every `self.handles.width`/`self.handles.height`
/// read, live.
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
/// `node.on(...)` (in `node_events.rs`) needs to write into the *same*
/// map from a `Node` Python object that holds no back-reference to this
/// `PyWindow` -- shared the exact way `tree: Rc<RefCell<Tree>>` already
/// is between a `Window` and every `Node` it hands out.
#[pyclass(name = "Window")]
pub struct PyWindow(ThreadBound<WindowState>);
thread_bound_shell!(PyWindow => WindowState);

/// `Window`'s state (M96: behind a `ThreadBound`, see `thread_bound`).
/// 0.4.0 M6: the handles a window shares with `App.run()`'s frame loop --
/// each an `Rc` (or a copy), so the run clones this once rather than
/// copying field by field into its own structs.
#[derive(Clone)]
pub(crate) struct WindowHandles {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) width: SharedSize,
    pub(crate) height: SharedSize,
    pub(crate) handlers: HandlerMap,
    /// M4 Phase 9 (§11.4): real docking state -- `DockLayout` plus
    /// engine-py's own zone-container/drag-handle bookkeeping `Tree`
    /// itself never stores. Plain data, no `Py<PyAny>` involved, the
    /// reason it needs no GC-traversal obligation.
    pub(crate) dock: SharedDockState,
    /// M9 Phase 2 (§5): `Node.animate(..., on_complete=...)`'s own
    /// registry, shared the same way `handlers` is. Holds real
    /// `Py<PyAny>` callbacks (like `handlers`, unlike `dock`) -- needs
    /// the same `__traverse__`/`__clear__` obligation below.
    pub(crate) completions: SharedCompletions,
    /// M30 Phase 9 Step 4 (§5, §8, §10): every real, live `Terminal`
    /// session this `Window` has spawned, keyed by its own real
    /// `NodeId` -- shared with `App.run`'s own per-frame `WindowRuntime`
    /// (`app.rs`'s own real drain loop), the identical real shape
    /// `dock` already has: plain data, no `Py<PyAny>` involved, so no
    /// `__traverse__`/`__clear__` GC obligation either.
    pub(crate) terminals: Rc<RefCell<HashMap<NodeId, TerminalSession>>>,
    /// M94: `window.on(...)` listeners -- the window's own, independent of
    /// which tree it shows.
    pub(crate) window_listeners: WindowListenerMap,
    /// M94: the OS window while `App.run()` has it open -- `None` before
    /// and after. `window.set(title=...)` and `window.get("scale_factor")`
    /// reach it here; `App.run()` fills and clears it.
    pub(crate) os_window: SharedOsWindow,
    /// 0.4.0 M5: whether this window redraws only what changed (the
    /// default) -- `window.set(partial_redraw=False)` turns it off. Shared
    /// with `App.run()`'s frame loop, which reads it every frame.
    pub(crate) partial_redraw: Rc<Cell<bool>>,
    /// 0.4.0 M6: whether the open window's surface allows partial redraw
    /// (it can be copied into) -- `None` until `App.run()` opens it.
    pub(crate) surface_partial: Rc<Cell<Option<bool>>>,
}

pub struct WindowState {
    pub(crate) handles: WindowHandles,
    pub(crate) title: String,
}

/// M94: see `PyWindow::os_window`.
pub(crate) type SharedOsWindow = Rc<RefCell<Option<std::sync::Arc<winit::window::Window>>>>;

#[pymethods]
impl PyWindow {
    #[new]
    #[pyo3(signature = (width=480, height=200, title="tre v2"))]
    fn new(width: u32, height: u32, title: &str) -> PyResult<Self> {
        // 0.4.0 review: a GPU surface can't be zero-sized.
        if width == 0 || height == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "a window needs a positive width and height, got {width}x{height}"
            )));
        }
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
        Ok(Self(ThreadBound::new(WindowState {
            title: title.to_string(),
            handles: WindowHandles {
                tree,
                root,
                width: Rc::new(Cell::new(width)),
                height: Rc::new(Cell::new(height)),
                handlers,
                dock: Rc::new(RefCell::new(dock::DockState::new())),
                completions: Rc::new(RefCell::new(CompletionRegistry::new())),
                terminals: Rc::new(RefCell::new(HashMap::new())),
                window_listeners: Rc::new(RefCell::new(HashMap::new())),
                os_window: Rc::new(RefCell::new(None)),
                partial_redraw: Rc::new(Cell::new(true)),
                surface_partial: Rc::new(Cell::new(None)),
            },
        })))
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
        for (handler, _wants_event) in self.handles.handlers.borrow().values() {
            visit.call(handler)?;
        }
        // M9 Phase 2: `completions` holds real `Py<PyAny>` callbacks --
        // the same cyclic-GC obligation as `handlers`.
        for callback in self.handles.completions.borrow().callbacks.values() {
            visit.call(callback)?;
        }
        // M94: window listeners are stored callbacks too.
        for (handler, _wants_event) in self.handles.window_listeners.borrow().values() {
            visit.call(handler)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        if !self.0.is_owner() {
            return;
        }
        self.handles.handlers.borrow_mut().clear();
        self.handles.completions.borrow_mut().callbacks.clear();
        self.handles.window_listeners.borrow_mut().clear();
    }
}
