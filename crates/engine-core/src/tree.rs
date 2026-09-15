//! `Tree`: owns every `Node` plus the `taffy::TaffyTree` that computes
//! their layout (§5, §14 step 3). Two parallel structures, deliberately
//! -- not `taffy`'s custom-tree traits (`TraversePartialTree`/
//! `LayoutPartialTree`) computing directly against this crate's own
//! arena. The custom-tree route avoids the double bookkeeping below, but
//! costs getting those traits' contract right on a first pass; the
//! simpler two-structure approach is provably correct by construction
//! (each `Node` <-> `taffy::NodeId` pair is created together, in
//! `insert`, and never diverges) and can be revisited if profiling ever
//! shows the parallel bookkeeping itself is the frame-budget cost --
//! which the CI benchmark this step adds would be exactly what catches
//! that.

use std::time::Instant;

use slotmap::{SecondaryMap, SlotMap};
use taffy::prelude::{AvailableSpace, Layout, Size, Style, TaffyTree};

use crate::node::{Node, NodeId, NodeKind, PaintProperties};

pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    taffy_nodes: SecondaryMap<NodeId, taffy::NodeId>,
    taffy: TaffyTree<()>,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            taffy_nodes: SecondaryMap::new(),
            taffy: TaffyTree::new(),
        }
    }

    /// Creates a new, parentless node (attach it under another with
    /// `add_child`, or leave it as a root passed to `compute_layout`).
    pub fn insert(
        &mut self,
        kind: NodeKind,
        layout_style: Style,
        paint: PaintProperties,
    ) -> NodeId {
        let taffy_node = self
            .taffy
            .new_leaf(layout_style.clone())
            .expect("TaffyTree::new_leaf is infallible for a leaf with no children");
        let id = self.nodes.insert_with_key(|id| Node {
            id,
            parent: None,
            children: Vec::new(),
            kind,
            layout_style,
            paint,
        });
        self.taffy_nodes.insert(id, taffy_node);
        id
    }

    /// Attaches `child` under `parent` in both structures at once, so
    /// they can never diverge. Panics if either id is stale/foreign to
    /// this `Tree` -- an internal bookkeeping bug, not a recoverable
    /// runtime condition (unlike the GPU/display absence this codebase
    /// exits gracefully for elsewhere).
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        let parent_taffy = *self
            .taffy_nodes
            .get(parent)
            .expect("add_child: parent NodeId not found in this Tree");
        let child_taffy = *self
            .taffy_nodes
            .get(child)
            .expect("add_child: child NodeId not found in this Tree");
        self.taffy
            .add_child(parent_taffy, child_taffy)
            .expect("add_child: taffy rejected the parent/child pair");

        self.nodes[parent].children.push(child);
        self.nodes[child].parent = Some(parent);
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Computes layout for `root`'s whole subtree. §5's `layout_style`
    /// lives on `Node`, not duplicated here -- `insert`/`add_child` are
    /// this module's only two places a `taffy::NodeId` is minted or
    /// linked, so `taffy`'s own tree always matches what `insert` was
    /// given.
    pub fn compute_layout(&mut self, root: NodeId, available_space: Size<AvailableSpace>) {
        let root_taffy = *self
            .taffy_nodes
            .get(root)
            .expect("compute_layout: root NodeId not found in this Tree");
        self.taffy
            .compute_layout(root_taffy, available_space)
            .expect("compute_layout: taffy layout computation failed");
    }

    /// The computed box for `id`, after `compute_layout` has run for a
    /// root that contains it. Panics under the same "internal bug, not a
    /// runtime condition" reasoning as `add_child`.
    pub fn layout(&self, id: NodeId) -> &Layout {
        let taffy_node = *self
            .taffy_nodes
            .get(id)
            .expect("layout: NodeId not found in this Tree");
        self.taffy
            .layout(taffy_node)
            .expect("layout: no computed layout yet for this node -- call compute_layout first")
    }

    /// The central tick's per-`Tree` entry point (§5): ticks every
    /// node's `PaintProperties` and returns `true` if any is still
    /// mid-animation. A naive whole-tree walk, not the "active set
    /// only" scoped version §5 describes -- see `PaintProperties::tick`
    /// for why that scoping is deliberately deferred past this step.
    pub fn tick_all(&mut self, now: Instant) -> bool {
        let mut any_active = false;
        for node in self.nodes.values_mut() {
            if node.paint.tick(now) {
                any_active = true;
            }
        }
        any_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::Color;
    use taffy::prelude::{FlexDirection, length};

    fn leaf(width: f32, height: f32) -> (NodeKind, Style, PaintProperties) {
        (
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(width),
                    height: length(height),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(255, 0, 0, 255), 0.0, 0.0, 1.0),
        )
    }

    #[test]
    fn insert_creates_a_parentless_node() {
        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(10.0, 10.0);
        let id = tree.insert(kind, style, paint);

        let node = tree.get(id).expect("just-inserted node must be present");
        assert_eq!(node.id, id);
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
    }

    #[test]
    fn add_child_links_both_structures() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let parent = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let child = tree.insert(k, s, p);

        tree.add_child(parent, child);

        assert_eq!(tree.get(parent).unwrap().children, vec![child]);
        assert_eq!(tree.get(child).unwrap().parent, Some(parent));
    }

    /// Three fixed-size children in a row, via `FlexDirection::Row` with
    /// no gap -- a deterministic layout with a known, hand-computable
    /// answer, so this proves `taffy`'s layout output is actually wired
    /// through `Tree`, not just that it doesn't panic.
    #[test]
    fn compute_layout_positions_row_children_left_to_right() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            flex_direction: FlexDirection::Row,
            size: Size {
                width: length(300.0),
                height: length(100.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let mut children = Vec::new();
        for _ in 0..3 {
            let (kind, style, paint) = leaf(100.0, 100.0);
            let child = tree.insert(kind, style, paint);
            tree.add_child(root, child);
            children.push(child);
        }

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(100.0),
            },
        );

        for (i, &child) in children.iter().enumerate() {
            let layout = tree.layout(child);
            assert_eq!(layout.size.width, 100.0);
            assert_eq!(layout.size.height, 100.0);
            assert_eq!(layout.location.x, i as f32 * 100.0);
            assert_eq!(layout.location.y, 0.0);
        }
    }

    #[test]
    fn tick_all_reports_active_and_advances_every_node() {
        use std::time::Duration;

        let mut tree = Tree::new();
        let (kind, style, mut paint) = leaf(10.0, 10.0);
        let start = Instant::now();
        paint.opacity.animate_to(
            0.0,
            Duration::from_secs(1),
            crate::animation::MotionCurve::Linear,
            start,
        );
        let id = tree.insert(kind, style, paint);

        let still_active = tree.tick_all(start + Duration::from_millis(500));
        assert!(still_active);
        let node = tree.get(id).unwrap();
        assert!((node.paint.opacity.current - 0.5).abs() < 0.01);

        let still_active = tree.tick_all(start + Duration::from_secs(2));
        assert!(!still_active);
        assert_eq!(tree.get(id).unwrap().paint.opacity.current, 0.0);
    }
}
