//! `PyWindow`'s virtual-list and canvas plumbing (review follow-through,
//! M28 Phase 2, §4/§8): `add_virtual_list`/`set_virtual_list_window`/
//! `add_canvas`/`redraw_canvas` -- the two `NodeKind`s whose real
//! content is supplied by a Python callback (`materializers`/
//! `canvas_draws`) rather than built once at creation time, the one
//! real thing that sets this pair apart from every plain `add_*` in
//! `window_factory.rs`. See its own doc comment for why this was split
//! out.

use std::cell::RefCell;
use std::collections::BTreeMap;

use engine_core::{ItemExtent, NodeKind, PaintProperties, VirtualListState};
use peniko::Color;
use pyo3::prelude::*;
use taffy::prelude::{Size, Style, auto, length};

use crate::error::EngineError;
use crate::node::Node;
use crate::window::{PyWindow, positioned_style};

/// M12 Phase 2 (§11.7): the per-item height lookup `set_virtual_list_
/// window`'s own materializer closure needs -- extracted as an owned
/// snapshot *before* the closure is built (the closure runs while
/// `Tree::set_virtual_list_window` already holds `&mut self`'s own
/// `Tree`, so it can't also borrow `tree` live from inside itself, the
/// same reason the pre-M12 code already extracted a plain `item_extent`
/// scalar up front). `Variable`'s own per-item height is the real
/// difference between two adjacent resolved cumulative offsets, not a
/// separately-stored value -- `add_virtual_list`'s own eager resolution
/// (M12 Phase 2) always resolves every index up to and including
/// `item_count`, so both `idx` and `idx + 1` are guaranteed present.
enum ResolvedItemHeights {
    Fixed(f64),
    Variable(BTreeMap<usize, f64>),
}

impl ResolvedItemHeights {
    fn of(&self, idx: usize) -> f64 {
        match self {
            ResolvedItemHeights::Fixed(v) => *v,
            ResolvedItemHeights::Variable(offsets) => {
                let start = offsets.get(&idx).unwrap_or_else(|| {
                    panic!("ResolvedItemHeights::of: item {idx}'s own offset must be resolved")
                });
                let end = offsets.get(&(idx + 1)).unwrap_or_else(|| {
                    panic!("ResolvedItemHeights::of: item {idx}'s own end offset must be resolved")
                });
                end - start
            }
        }
    }
}

#[pymethods]
impl PyWindow {
    /// §14 step 15 (§11.7): creates a `NodeKind::VirtualList` of
    /// `item_count` logical rows. Exactly one of `item_extent` (every
    /// row the same fixed height) or `size_hint` (M12 Phase 2: a real
    /// `Callable[[int], float]`, one row's own real height) must be
    /// given -- a real `ValueError` otherwise (neither, or both).
    /// `materialize` is stored here, keyed by the new node's own
    /// `NodeId` -- not called yet; `set_virtual_list_window` is what
    /// actually invokes it, once per newly-visible index.
    ///
    /// **`size_hint` resolves eagerly, once, right here -- not lazily
    /// per `set_virtual_list_window` call.** A real per-item cumulative
    /// offset structurally requires knowing every preceding item's own
    /// height; rather than a stateful, incrementally-extended lazy
    /// cache (real complexity §11.7's own one-line "size-hint callback"
    /// text doesn't ask for), this calls `size_hint` exactly `item_
    /// count` times immediately, building the complete real cumulative-
    /// offset table via `Tree::set_virtual_list_resolved_offsets`
    /// before this method ever returns. A real, deliberate, stated
    /// tradeoff, not a hidden cost: `size_hint` is a plain, cheap
    /// arithmetic call (unlike `materialize`, which builds a real
    /// `Node`), but this is a genuine `O(item_count)` cost at list-
    /// creation time `item_extent`'s own `Fixed` path never pays.
    ///
    /// `height` is a real, meaningful viewport height (M8 Phase 3,
    /// §11.7/§11.8) -- `Window.scroll`/a real dispatched mouse wheel
    /// (`Tree::dispatch`'s own `InputEvent::Scroll` arm) clamp the
    /// list's own `scroll_offset` against exactly this value. Before
    /// that phase it was left `auto()`, which taffy resolves against
    /// *content* size -- and every materialized item is `Position::
    /// Absolute` (resolved from its own `inset`, not counted toward the
    /// parent's own intrinsic size, the same real fact `open_overlay`
    /// already established), so `auto()` never gave a real viewport
    /// height at all. Defaults to this `Window`'s own real height, the
    /// same fallback shape `width` already uses.
    #[pyo3(signature = (item_count, materialize, item_extent=None, size_hint=None, width=None, height=None))]
    // Every real caller uses keyword arguments exclusively (confirmed
    // via grep) -- Python's own kwarg ergonomics are the reason pyo3
    // methods with several optional parameters are a normal shape here,
    // not a real code smell a struct would meaningfully fix.
    #[allow(clippy::too_many_arguments)]
    fn add_virtual_list(
        &self,
        item_count: usize,
        materialize: Py<PyAny>,
        item_extent: Option<f64>,
        size_hint: Option<Py<PyAny>>,
        width: Option<f32>,
        height: Option<f32>,
        py: Python<'_>,
    ) -> PyResult<Node> {
        let (extent, resolved_offsets) = match (item_extent, size_hint) {
            (Some(v), None) => (ItemExtent::Fixed(v), None),
            (None, Some(hint)) => {
                let mut offsets = Vec::with_capacity(item_count + 1);
                let mut cumulative = 0.0;
                for idx in 0..item_count {
                    offsets.push((idx, cumulative));
                    let item_height: f64 = hint.call1(py, (idx,))?.extract(py)?;
                    cumulative += item_height;
                }
                offsets.push((item_count, cumulative));
                (ItemExtent::Variable, Some(offsets))
            }
            (Some(_), Some(_)) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "add_virtual_list: pass exactly one of item_extent or size_hint, not both",
                ));
            }
            (None, None) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "add_virtual_list: pass exactly one of item_extent or size_hint",
                ));
            }
        };

        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(item_count, extent)),
            Style {
                size: Size {
                    width: length(width.unwrap_or(self.width as f32)),
                    height: length(height.unwrap_or(self.height as f32)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        if let Some(offsets) = resolved_offsets {
            tree.set_virtual_list_resolved_offsets(id, offsets);
        }
        drop(tree);
        self.materializers.borrow_mut().insert(id, materialize);
        Ok(self.wrap_node(id))
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
        &self,
        list: PyRef<'_, Node>,
        start: usize,
        end: usize,
        py: Python<'_>,
    ) -> PyResult<()> {
        let materialize = self
            .materializers
            .borrow()
            .get(&list.id)
            .ok_or(EngineError::NotAVirtualList)?
            .clone_ref(py);

        let mut tree = self.tree.borrow_mut();
        let item_heights = match &tree
            .get(list.id)
            .expect("set_virtual_list_window: Node holds a NodeId missing from its own Tree")
            .kind
        {
            NodeKind::VirtualList(state) => match &state.item_extent {
                ItemExtent::Fixed(v) => ResolvedItemHeights::Fixed(*v),
                ItemExtent::Variable => {
                    ResolvedItemHeights::Variable(state.resolved_offsets.clone())
                }
            },
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
                            height: length(item_heights.of(idx) as f32),
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

    /// M5 Phase 3 (§11.10/§11.11): creates a `NodeKind::Canvas` of the
    /// given size. `draw` is stored here, keyed by the new node's own
    /// `NodeId` -- not called yet; `redraw_canvas` is what actually
    /// invokes it, mirroring `add_virtual_list`/`materialize`'s own
    /// "store now, invoke later" shape exactly.
    #[pyo3(signature = (width, height, draw, x=None, y=None))]
    fn add_canvas(
        &self,
        width: f64,
        height: f64,
        draw: Py<PyAny>,
        x: Option<f32>,
        y: Option<f32>,
    ) -> Node {
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Canvas(engine_core::CanvasState::new()),
            positioned_style(
                Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                x,
                y,
            ),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        drop(tree);
        self.canvas_draws.borrow_mut().insert(id, draw);
        self.wrap_node(id)
    }

    /// The real "draw callback" invocation entry point (§11.10/§11.11):
    /// calls `canvas`'s own stored `draw(ctx: CanvasContext) -> None`
    /// callback exactly once, then replaces `canvas`'s entire real
    /// content (`Tree::set_canvas_content`) with whatever `ctx`
    /// collected. Simpler than `set_virtual_list_window`'s own
    /// per-index error-deferral -- this is exactly one call, not N, so
    /// a raised exception propagates as a real `PyErr` directly, no
    /// error slot needed.
    fn redraw_canvas(&self, canvas: PyRef<'_, Node>, py: Python<'_>) -> PyResult<()> {
        let draw = self
            .canvas_draws
            .borrow()
            .get(&canvas.id)
            .ok_or(EngineError::NotACanvas)?
            .clone_ref(py);

        {
            let tree = self.tree.borrow();
            if !matches!(
                tree.get(canvas.id)
                    .expect("redraw_canvas: Node holds a NodeId missing from its own Tree")
                    .kind,
                NodeKind::Canvas(_)
            ) {
                return Err(EngineError::NotACanvas.into());
            }
        }

        let ctx = Py::new(py, crate::canvas::CanvasContext::default())?;
        draw.call1(py, (ctx.clone_ref(py),))?;

        let ctx = ctx.borrow(py);
        self.tree.borrow_mut().set_canvas_content(
            canvas.id,
            ctx.commands.clone(),
            ctx.hit_test.clone(),
        );
        Ok(())
    }
}
