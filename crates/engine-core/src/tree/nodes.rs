//! 0.5.4 (#125): the tree's nodes, with every mutable access recorded.
//!
//! Change tracking for the damage walk must not depend on every write path
//! remembering to say it wrote. So the slot map is private to this wrapper,
//! and the only ways to reach a node mutably (`get_mut`, `IndexMut`,
//! `iter_mut`, `values_mut`) note the node as touched: a write that skips the
//! note cannot compile. Noting is conservative: asking for `&mut` counts as a
//! touch even if nothing is written, which costs only a needless re-check of
//! that one node.

use std::cell::{Cell, RefCell};
use std::ops::{Index, IndexMut};

use slotmap::SlotMap;

use crate::node::{Node, NodeId};

/// Past this many touches between two drains, the list is dropped and
/// "everything" is recorded instead, so a tree nobody drains cannot grow it
/// without bound.
const MAX_TOUCHED: usize = 8192;

/// The nodes touched since the last `take_touched`.
#[derive(Debug, Default)]
pub struct Touched {
    /// Every node may have changed (a pass over all nodes was made, or too
    /// many were touched to list).
    pub all: bool,
    /// The touched nodes, possibly repeated and possibly since removed.
    pub ids: Vec<NodeId>,
}

pub(super) struct Nodes {
    map: SlotMap<NodeId, Node>,
    // Interior mutability only so the damage tracker can drain these from a
    // shared reference to the tree; every write to them is through `&mut self`.
    touched: RefCell<Vec<NodeId>>,
    all_touched: Cell<bool>,
}

impl Nodes {
    pub(super) fn new() -> Self {
        Self {
            map: SlotMap::with_key(),
            touched: RefCell::new(Vec::new()),
            all_touched: Cell::new(false),
        }
    }

    fn note(&mut self, id: NodeId) {
        if *self.all_touched.get_mut() {
            return;
        }
        let touched = self.touched.get_mut();
        if touched.len() >= MAX_TOUCHED {
            touched.clear();
            *self.all_touched.get_mut() = true;
        } else {
            touched.push(id);
        }
    }

    fn note_all(&mut self) {
        self.touched.get_mut().clear();
        *self.all_touched.get_mut() = true;
    }

    /// Hands over what was touched since the last call, and starts again.
    pub(super) fn take_touched(&self) -> Touched {
        Touched {
            all: self.all_touched.take(),
            ids: self.touched.take(),
        }
    }

    pub(super) fn insert_with_key(&mut self, f: impl FnOnce(NodeId) -> Node) -> NodeId {
        self.map.insert_with_key(f)
    }

    pub(super) fn remove(&mut self, id: NodeId) -> Option<Node> {
        self.map.remove(id)
    }

    pub(super) fn get(&self, id: NodeId) -> Option<&Node> {
        self.map.get(id)
    }

    pub(super) fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        if self.map.contains_key(id) {
            self.note(id);
        }
        self.map.get_mut(id)
    }

    pub(super) fn contains_key(&self, id: NodeId) -> bool {
        self.map.contains_key(id)
    }

    pub(super) fn len(&self) -> usize {
        self.map.len()
    }

    pub(super) fn iter(&self) -> slotmap::basic::Iter<'_, NodeId, Node> {
        self.map.iter()
    }

    pub(super) fn values(&self) -> slotmap::basic::Values<'_, NodeId, Node> {
        self.map.values()
    }

    pub(super) fn iter_mut(&mut self) -> slotmap::basic::IterMut<'_, NodeId, Node> {
        self.note_all();
        self.map.iter_mut()
    }

    pub(super) fn values_mut(&mut self) -> slotmap::basic::ValuesMut<'_, NodeId, Node> {
        self.note_all();
        self.map.values_mut()
    }
}

impl Index<NodeId> for Nodes {
    type Output = Node;

    fn index(&self, id: NodeId) -> &Node {
        &self.map[id]
    }
}

impl IndexMut<NodeId> for Nodes {
    fn index_mut(&mut self, id: NodeId) -> &mut Node {
        self.note(id);
        &mut self.map[id]
    }
}

impl<'a> IntoIterator for &'a mut Nodes {
    type Item = (NodeId, &'a mut Node);
    type IntoIter = slotmap::basic::IterMut<'a, NodeId, Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a> IntoIterator for &'a Nodes {
    type Item = (NodeId, &'a Node);
    type IntoIter = slotmap::basic::Iter<'a, NodeId, Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
