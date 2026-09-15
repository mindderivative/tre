//! `PyWindow` (§8's own sketch, split back out of `App` at exactly the
//! step `App`'s own module doc comment predicted -- §14 step 14, §11.1
//! multi-window). Owns one `Tree`, its root, and its own size/title --
//! everything `App::new`/`App::add_rect` used to hold directly, now per
//! window instead of assumed singular.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{ItemExtent, NodeId, NodeKind, PaintProperties, Tree, VirtualListState};
use peniko::Color;
use pyo3::class::{PyTraverseError, PyVisit};
use pyo3::prelude::*;
use taffy::prelude::{Rect as TaffyRect, Size, Style, auto, length};

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
/// (§8's review note: "storing a long-lived `PyObject` callback... is a
/// new risk class... unless the `#[pyclass]` implements `__traverse__`/
/// `__clear__`" -- `node.rs`'s own module doc comment deferred this
/// exact question for `set_on_click` "until something actually stores
/// one"; this is that something). Confirmed directly against pyo3
/// 0.29.2's own real API before implementing (`tests/test_gc.rs`): no
/// `#[pyclass(gc)]` flag exists or is needed in this version -- a
/// `#[pyclass]` simply implementing `__traverse__`/`__clear__` in its
/// `#[pymethods]` is enough to opt into cyclic GC support, see below.
#[pyclass(unsendable, name = "Window")]
pub struct PyWindow {
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) root: NodeId,
    pub(crate) title: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    materializers: HashMap<NodeId, Py<PyAny>>,
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
        }
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
        Ok(())
    }

    fn __clear__(&mut self) {
        self.materializers.clear();
    }
}
