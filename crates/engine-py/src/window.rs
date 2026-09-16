//! `PyWindow` (§8's own sketch, split back out of `App` at exactly the
//! step `App`'s own module doc comment predicted -- §14 step 14, §11.1
//! multi-window). Owns one `Tree`, its root, and its own size/title --
//! everything `App::new`/`App::add_rect` used to hold directly, now per
//! window instead of assumed singular.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{
    InputEvent, ItemExtent, Key, NodeId, NodeKind, PaintProperties, PointerButton, Tree,
    VirtualListState,
};
use peniko::Color;
use peniko::kurbo::Point;
use pyo3::class::{PyTraverseError, PyVisit};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Rect as TaffyRect, Size, Style, auto, length};

use crate::dispatch::{interaction_config, run_activation};
use crate::error::EngineError;
use crate::node::Node;

const PADDING: f32 = 16.0;
const GAP: f32 = 16.0;

/// `unsendable` (owns `Rc<RefCell<Tree>>`, §9) -- named `Window` to
/// Python, matching `Node`'s own "renamed to match what Python actually
/// sees" precedent (`tre.Window`, not `tre.PyWindow`); kept as the
/// `PyWindow` identifier on the Rust side since that's the name §8/§11.1
/// use throughout `ARCHITECTURE.md`.
///
/// `materializers` is §11.7's own "materialize item N" callback storage
/// and `click_handlers` (M4 Phase 1 step 3, §11.10) is `Node.
/// set_on_click`'s -- both real cases of §8's own review note: "storing
/// a long-lived `PyObject` callback... is a new risk class... unless the
/// `#[pyclass]` implements `__traverse__`/`__clear__`," which `node.rs`'s
/// own module doc comment deferred exactly this long, "until something
/// actually stores one." Confirmed directly against pyo3 0.29.2's own
/// real API before implementing (`tests/test_gc.rs`): no `#[pyclass(gc)]`
/// flag exists or is needed in this version -- a `#[pyclass]` simply
/// implementing `__traverse__`/`__clear__` in its `#[pymethods]` is
/// enough to opt into cyclic GC support, see below.
///
/// `click_handlers` is an `Rc<RefCell<...>>`, not a plain field, because
/// `Node.set_on_click` (in `node.rs`) needs to write into the *same*
/// map from a `Node` Python object that holds no back-reference to this
/// `PyWindow` -- shared the exact way `tree: Rc<RefCell<Tree>>` already
/// is between a `Window` and every `Node` it hands out.
#[pyclass(unsendable, name = "Window")]
pub struct PyWindow {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) title: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    materializers: HashMap<NodeId, Py<PyAny>>,
    pub(crate) click_handlers: Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>,
}

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
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        Self {
            tree: Rc::new(RefCell::new(tree)),
            root,
            title: title.to_string(),
            width,
            height,
            materializers: HashMap::new(),
            click_handlers: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// §14 step 6's own "node creation" -- one shape (a colored rect, a
    /// child of this window's implicit root row) is the real minimal
    /// slice; unchanged by the `PyWindow` split, just moved here with
    /// `App` itself.
    fn add_rect(&self, background: (u8, u8, u8, u8), width: f32, height: f32) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(width),
                    height: length(height),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Node {
            id,
            tree: self.tree.clone(),
            click_handlers: self.click_handlers.clone(),
        }
    }

    /// M4 Phase 1 step 3 (§11.10): a direct, programmatic "click this
    /// node" entry point -- the same "expose a direct method since real
    /// pointer dispatch has nowhere else to originate outside a live
    /// window" pattern every prior interaction step used (`Tree::
    /// spawn_ripple`, `Tree::open_overlay`, etc.), and this step's own
    /// real, no-window-needed way to prove `set_on_click` actually
    /// fires. Computes layout first (so `node`'s own bounds are current
    /// -- the real render loop does this every frame; nothing else does
    /// for a `Window` with no render loop attached), then dispatches a
    /// primary-button press+release pair at `node`'s own real center
    /// point -- exactly what a real mouse click there would produce.
    fn click(&mut self, node: PyRef<'_, Node>, py: Python<'_>) {
        let point = {
            let mut tree = self.tree.borrow_mut();
            tree.compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(self.width as f32),
                    height: AvailableSpace::Definite(self.height as f32),
                },
            );
            let (x, y) = tree.absolute_position(node.id);
            let layout = tree.layout(node.id);
            Point::new(
                x + f64::from(layout.size.width) / 2.0,
                y + f64::from(layout.size.height) / 2.0,
            )
        };

        let now = std::time::Instant::now();
        let config = interaction_config();
        // Each `dispatch` call's own `self.tree.borrow_mut()` is a
        // short-lived temporary, released before `run_activation` runs
        // -- a click handler that itself touches this same `Tree` (e.g.
        // animating the very node it's attached to, a real, plausible
        // pattern) would otherwise panic on a re-entrant borrow.
        let press = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerPressed {
                position: point,
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        run_activation(&self.click_handlers, press, py);

        let release = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::PointerReleased {
                position: point,
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        run_activation(&self.click_handlers, release, py);
    }

    /// M4 Phase 2 (§10): `click()`'s own keyboard counterpart -- the
    /// real, no-window-needed way to test Tab/Shift-Tab focus movement
    /// and Enter/Space activation from Python, neither of which had a
    /// Python-facing entry point before this. `key` is one of `"tab"`/
    /// `"enter"`/`"space"`/`"escape"` -- `engine_core::Key`'s own
    /// deliberately minimal vocabulary (§10), not a general key-code
    /// mapping nothing here needs yet.
    #[pyo3(signature = (key, shift=false))]
    fn press_key(&mut self, key: &str, shift: bool, py: Python<'_>) -> PyResult<()> {
        let key = match key {
            "tab" => Key::Tab,
            "enter" => Key::Enter,
            "space" => Key::Space,
            "escape" => Key::Escape,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "press_key: unknown key {other:?} -- expected one of \"tab\", \"enter\", \
                     \"space\", \"escape\""
                )));
            }
        };
        let outcome = self.tree.borrow_mut().dispatch(
            self.root,
            InputEvent::KeyPressed { key, shift },
            &interaction_config(),
            std::time::Instant::now(),
        );
        run_activation(&self.click_handlers, outcome, py);
        Ok(())
    }

    /// §14 step 15 (§11.7): creates a `NodeKind::VirtualList` of
    /// `item_count` logical rows, each `item_extent` px tall (only
    /// `ItemExtent::Fixed` is exposed -- see `engine_core::ItemExtent`'s
    /// own doc comment for why the variable-height variant isn't built
    /// yet). `materialize` is stored here, keyed by the new node's own
    /// `NodeId` -- not called yet; `set_virtual_list_window` is what
    /// actually invokes it, once per newly-visible index.
    #[pyo3(signature = (item_count, item_extent, materialize, width=None))]
    fn add_virtual_list(
        &mut self,
        item_count: usize,
        item_extent: f64,
        materialize: Py<PyAny>,
        width: Option<f32>,
    ) -> Node {
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(
                item_count,
                ItemExtent::Fixed(item_extent),
            )),
            Style {
                size: Size {
                    // A real scrollable viewport (clipping, an actual
                    // scroll offset/transform, §11.8/§11.9) isn't built
                    // yet -- nothing in this step's own scope needs it,
                    // so the list's own box height is left `auto()`
                    // rather than manufacturing a viewport concept ahead
                    // of a step that needs one. `auto()` doesn't affect
                    // item positioning: absolutely positioned children
                    // (every materialized item) are resolved from their
                    // own `inset`, not their parent's size, the same
                    // real taffy fact `open_overlay` already established.
                    width: length(width.unwrap_or(self.width as f32)),
                    height: auto(),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.materializers.insert(id, materialize);
        Node {
            id,
            tree: self.tree.clone(),
            click_handlers: self.click_handlers.clone(),
        }
    }

    /// §14 step 15 (§11.7): the "materialize item N" FFI entry point --
    /// invokes `list`'s own stored `materialize(index: int) -> (r, g, b,
    /// a)` callback once for every index in `start..end` not already
    /// materialized, and drops (`Tree::remove`, real generational
    /// `NodeId` invalidation) whatever was materialized outside that
    /// range. `list` must be a `Node` this same `Window` created via
    /// `add_virtual_list` -- raises `ValueError` otherwise (it either
    /// isn't a `VirtualList` at all, or belongs to a different `Window`
    /// with no recorded callback for it).
    ///
    /// A materializer callback raising propagates as a real `PyErr` --
    /// caught via an error slot the closure writes into, since
    /// `Tree::set_virtual_list_window`'s own closure signature is
    /// deliberately infallible (§4: `engine-core` stays `pyo3`-agnostic,
    /// so it can't know about `PyErr`). Not transactional: an index
    /// materialized earlier in the same call before a later index raises
    /// stays in the tree -- acceptable for this step's own scope
    /// (proving the callback mechanism and measuring its real GIL
    /// overhead below), not a general rollback guarantee.
    fn set_virtual_list_window(
        &mut self,
        list: PyRef<'_, Node>,
        start: usize,
        end: usize,
        py: Python<'_>,
    ) -> PyResult<()> {
        let materialize = self
            .materializers
            .get(&list.id)
            .ok_or(EngineError::NotAVirtualList)?
            .clone_ref(py);

        let mut tree = self.tree.borrow_mut();
        let item_extent = match &tree
            .get(list.id)
            .expect("set_virtual_list_window: Node holds a NodeId missing from its own Tree")
            .kind
        {
            NodeKind::VirtualList(state) => {
                let ItemExtent::Fixed(v) = state.item_extent;
                v
            }
            _ => return Err(EngineError::NotAVirtualList.into()),
        };

        let error: RefCell<Option<PyErr>> = RefCell::new(None);
        tree.set_virtual_list_window(list.id, start..end, |idx| {
            let outcome = materialize
                .call1(py, (idx,))
                .and_then(|result| result.extract::<(u8, u8, u8, u8)>(py));
            match outcome {
                Ok((r, g, b, a)) => (
                    NodeKind::Rect,
                    Style {
                        size: Size {
                            width: auto(),
                            height: length(item_extent as f32),
                        },
                        ..Default::default()
                    },
                    PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
                ),
                Err(e) => {
                    // First failure wins; later indices in this same
                    // call still need *some* real tuple to return, so
                    // this is an inert, fully transparent placeholder,
                    // not a value anyone is meant to see on screen.
                    error.borrow_mut().get_or_insert(e);
                    (
                        NodeKind::Rect,
                        Style::default(),
                        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 0.0),
                    )
                }
            }
        });

        match error.into_inner() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// §11.7's own claim, matching `App::run`'s existing `PyWindow::
    /// __traverse__` reference in step 14's module doc comment: every
    /// stored `PyObject` a window keeps must be visible to CPython's
    /// cyclic GC, or a materializer closure that captures this very
    /// `Window` (a plausible, real pattern -- e.g. a bound method) forms
    /// a reference cycle the refcounting GC alone can never collect.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for materializer in self.materializers.values() {
            visit.call(materializer)?;
        }
        for click_handler in self.click_handlers.borrow().values() {
            visit.call(click_handler)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.materializers.clear();
        self.click_handlers.borrow_mut().clear();
    }
}
