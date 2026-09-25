//! M96: node lifetime. Counts the live Python `Node` handles to each node,
//! and frees a detached subtree -- one made by `window.create` or detached by
//! `node.remove()` -- once no handle points anywhere into it
//! (`Tree::collect_unreferenced`), pruning its listeners with it. A node
//! attached to a window is never freed this way.
//!
//! The counts live here rather than in `Tree` so that making a handle never
//! needs a `Tree` borrow: many factories build their `Node` while still
//! holding one. They're per-thread because a handle's state is only ever
//! created and dropped on its owning thread (`thread_bound`), and keyed by the
//! tree's address, which can't be reused while any handle keeps that tree
//! alive. A collection that finds the tree borrowed waits in `PENDING` for
//! the next `reclaim`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{NodeId, Tree};

use crate::dispatch::HandlerMap;
use crate::node::NodeState;
use crate::thread_bound;

type TreeKey = usize;

struct PendingCollect {
    tree: Rc<RefCell<Tree>>,
    handlers: HandlerMap,
    id: NodeId,
}

thread_local! {
    static COUNTS: RefCell<HashMap<(TreeKey, NodeId), u32>> = RefCell::new(HashMap::new());
    static PENDING: RefCell<Vec<PendingCollect>> = const { RefCell::new(Vec::new()) };
}

fn tree_key(tree: &Rc<RefCell<Tree>>) -> TreeKey {
    Rc::as_ptr(tree) as TreeKey
}

pub(crate) fn retain(state: &NodeState) {
    reclaim();
    COUNTS.with(|counts| {
        *counts
            .borrow_mut()
            .entry((tree_key(&state.tree), state.id))
            .or_insert(0) += 1;
    });
}

pub(crate) fn release(state: &NodeState) {
    let last = COUNTS.with(|counts| {
        let mut counts = counts.borrow_mut();
        let key = (tree_key(&state.tree), state.id);
        let Some(count) = counts.get_mut(&key) else {
            return false;
        };
        *count -= 1;
        if *count > 0 {
            return false;
        }
        counts.remove(&key);
        true
    });
    if last {
        collect(&state.tree, &state.handlers, state.id);
    }
}

/// Whether any live handle points at `id` in `tree`.
fn is_referenced(tree: TreeKey, id: NodeId) -> bool {
    COUNTS.with(|counts| counts.borrow().contains_key(&(tree, id)))
}

/// Frees `id`'s collectible subtree if nothing references it any more.
pub(crate) fn collect(tree: &Rc<RefCell<Tree>>, handlers: &HandlerMap, id: NodeId) {
    let key = tree_key(tree);
    let Ok(mut borrowed) = tree.try_borrow_mut() else {
        PENDING.with(|pending| {
            pending.borrow_mut().push(PendingCollect {
                tree: tree.clone(),
                handlers: handlers.clone(),
                id,
            });
        });
        return;
    };
    let freed = borrowed.collect_unreferenced(id, |node| is_referenced(key, node));
    drop(borrowed);
    prune(handlers, &freed);
}

/// Drops every listener registered on a freed node. The callbacks are
/// dropped only after the map's borrow is released, since dropping one can
/// run Python code.
pub(crate) fn prune(handlers: &HandlerMap, freed: &[NodeId]) {
    if freed.is_empty() {
        return;
    }
    let removed: Vec<_> = {
        let Ok(mut map) = handlers.try_borrow_mut() else {
            return;
        };
        let keys: Vec<_> = map
            .keys()
            .filter(|(node, _)| freed.contains(node))
            .copied()
            .collect();
        keys.into_iter()
            .filter_map(|key| map.remove(&key))
            .collect()
    };
    drop(removed);
}

/// The owning thread's safe point: finishes drops other threads handed back
/// (`thread_bound::reclaim`) and retries collections that found their tree
/// borrowed.
pub(crate) fn reclaim() {
    thread_bound::reclaim();
    let pending = PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    for item in pending {
        collect(&item.tree, &item.handlers, item.id);
    }
}
