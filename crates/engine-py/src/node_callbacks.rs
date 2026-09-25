//! M96: the two kinds whose content comes from Python -- a `canvas`, drawn
//! by its `draw(painter)` callback, and a `virtual_list`, whose visible rows
//! its `materialize(index)` callback builds. The callbacks live in the
//! window's handler map (`HandlerKey::Draw`/`Materialize`/`SizeHint`), so the
//! collector sees them and a freed node's go with it.
//!
//! A virtual list made by `window.create` is kept materialized by `tre`:
//! whenever layout runs (`layout`), every row its viewport meets is built,
//! and every row that left it is detached -- freed, unless the framework
//! keeps a handle to reuse it.

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{ItemExtent, NodeId, NodeKind, Tree};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};

use crate::canvas::CanvasContext;
use crate::dispatch::{HandlerKey, HandlerMap, log_uncaught_exception};
use crate::error::EngineError;
use crate::node::Node;
use crate::node_handles;

type SharedTree = Rc<RefCell<Tree>>;

/// `id`'s callback stored under `key`, if any.
pub(crate) fn callback(
    handlers: &HandlerMap,
    id: NodeId,
    key: HandlerKey,
    py: Python<'_>,
) -> Option<Py<PyAny>> {
    handlers
        .borrow()
        .get(&(id, key))
        .map(|(callback, _)| callback.clone_ref(py))
}

/// Calls `canvas`'s draw callback with a fresh painter and replaces the
/// canvas's content with what it drew.
pub(crate) fn redraw(
    tree: &SharedTree,
    handlers: &HandlerMap,
    canvas: NodeId,
    py: Python<'_>,
) -> PyResult<()> {
    if !matches!(
        tree.borrow().get(canvas).map(|n| &n.kind),
        Some(NodeKind::Canvas(_))
    ) {
        return Err(EngineError::NotACanvas.into());
    }
    let draw = callback(handlers, canvas, HandlerKey::Draw, py).ok_or(EngineError::NotACanvas)?;
    let painter = Py::new(py, CanvasContext::default())?;
    draw.call1(py, (painter.clone_ref(py),))?;
    let painter = painter.borrow(py);
    tree.borrow_mut().set_canvas_content(
        canvas,
        painter.commands.clone(),
        painter.hit_test.clone(),
    );
    Ok(())
}

/// A `size_hint` list's row offsets, from calling `size_hint` once per row
/// -- when they're missing (just created, or `item_count` changed).
fn resolve_offsets(
    tree: &SharedTree,
    handlers: &HandlerMap,
    list: NodeId,
    py: Python<'_>,
) -> PyResult<()> {
    let count = match tree.borrow().get(list).map(|n| &n.kind) {
        Some(NodeKind::VirtualList(state))
            if matches!(state.item_extent, ItemExtent::Variable)
                && !state.resolved_offsets.contains_key(&state.item_count) =>
        {
            state.item_count
        }
        _ => return Ok(()),
    };
    let Some(size_hint) = callback(handlers, list, HandlerKey::SizeHint, py) else {
        return Ok(());
    };
    let mut offsets = Vec::with_capacity(count + 1);
    let mut top = 0.0;
    for idx in 0..count {
        offsets.push((idx, top));
        let height: f64 = size_hint.call1(py, (idx,))?.extract(py)?;
        if !(height >= 0.0 && height.is_finite()) {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "size_hint({idx}) must return a non-negative number, got {height}"
            )));
        }
        top += height;
    }
    offsets.push((count, top));
    tree.borrow_mut()
        .set_virtual_list_resolved_offsets(list, offsets);
    Ok(())
}

/// Builds every visible, unbuilt row of every virtual list with a
/// `materialize` callback, and detaches rows that left the viewport.
/// Returns whether any row changed, so layout runs again. A callback that
/// raises, or returns something other than a node of this window, is
/// logged and its row left empty -- layout runs outside any one call.
pub(crate) fn sync_virtual_lists(tree: &SharedTree, handlers: &HandlerMap, py: Python<'_>) -> bool {
    let lists: Vec<NodeId> = handlers
        .borrow()
        .keys()
        .filter(|(_, key)| *key == HandlerKey::Materialize)
        .map(|(id, _)| *id)
        .collect();
    let mut changed = false;
    for list in lists {
        if tree.borrow().get(list).is_none() {
            continue;
        }
        if let Err(err) = resolve_offsets(tree, handlers, list, py) {
            log_uncaught_exception(&err, py);
            continue;
        }
        let (visible, built) = {
            let tree = tree.borrow();
            let built = match tree.get(list).map(|n| &n.kind) {
                Some(NodeKind::VirtualList(state)) => {
                    state.materialized.keys().copied().collect::<Vec<_>>()
                }
                _ => continue,
            };
            (tree.virtual_list_visible(list), built)
        };
        let released = tree
            .borrow_mut()
            .virtual_list_release_outside(list, visible.clone());
        for row in released {
            changed = true;
            tree.borrow_mut().detach_collectible(row);
            node_handles::collect(tree, handlers, row);
        }
        let Some(materialize) = callback(handlers, list, HandlerKey::Materialize, py) else {
            continue;
        };
        for idx in visible.filter(|idx| !built.contains(idx)) {
            // The returned handle is held until the row is attached: a
            // freshly created row has no other, and would be freed first.
            let row = materialize.call1(py, (idx,)).and_then(|row| {
                let row = row.into_bound(py).cast_into::<Node>().map_err(|_| {
                    pyo3::exceptions::PyTypeError::new_err(format!(
                        "materialize({idx}) must return a Node made by this window"
                    ))
                })?;
                if !Rc::ptr_eq(&row.borrow().tree, tree) {
                    return Err(EngineError::ForeignNode.into());
                }
                Ok(row)
            });
            match row {
                Ok(row) => {
                    let id = row.borrow().id;
                    if tree.borrow_mut().virtual_list_adopt(list, idx, id) {
                        changed = true;
                    }
                }
                Err(err) => log_uncaught_exception(&err, py),
            }
        }
    }
    changed
}

/// Lays out `root`'s tree at `size`, materializing virtual lists' visible
/// rows -- and laying out again when that changed any.
pub(crate) fn layout(
    tree: &SharedTree,
    root: NodeId,
    size: Size<AvailableSpace>,
    handlers: &HandlerMap,
    py: Python<'_>,
) {
    tree.borrow_mut().compute_layout(root, size);
    if sync_virtual_lists(tree, handlers, py) {
        tree.borrow_mut().compute_layout(root, size);
    }
}

#[pymethods]
impl Node {
    /// M96: runs this canvas's `draw` callback now, replacing what it
    /// shows with what the callback draws.
    fn redraw(&self, py: Python<'_>) -> PyResult<()> {
        self.check_alive()?;
        redraw(&self.tree, &self.handlers, self.id, py)
    }
}
