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

use slotmap::{Key, SecondaryMap, SlotMap};
use taffy::prelude::{AvailableSpace, Layout, Size, Style, TaffyTree};

use crate::access::AccessNodeData;
use crate::interaction::InteractionState;
use crate::node::{Node, NodeId, NodeKind, PaintProperties};

pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    taffy_nodes: SecondaryMap<NodeId, taffy::NodeId>,
    taffy: TaffyTree<()>,
    /// §14 step 7: which node `TreeUpdate.focus` reports. Plain data --
    /// see `access.rs`'s module doc comment for why the actual Tab/
    /// Shift-Tab traversal logic isn't wired up yet.
    focused: Option<NodeId>,
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
            focused: None,
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
            access: AccessNodeData::default(),
            interaction: None,
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

    /// Recursively removes `id` and its whole subtree (§14 step 12,
    /// §16.4's own reconciliation need: a widget whose `id` disappeared
    /// from a reloaded view must actually leave the tree, not just its
    /// root node). Detaches `id` from its parent's `children` list
    /// first, if it has one -- an orphaned root removal (the subtree's
    /// own top node had no parent) is also valid, matching `insert`'s
    /// own "a parentless node is fine" contract.
    ///
    /// `taffy::TaffyTree::remove` only detaches a single node and
    /// orphans its children (checked directly in its own doc comment
    /// and source) -- it does not recurse, so this method walks the
    /// subtree itself and calls it once per node, deepest first,
    /// rather than relying on it for more than one node at a time.
    ///
    /// Returns `true` if `id` was present and removed, `false` if it
    /// wasn't in this `Tree` at all (a no-op, not an error -- matching
    /// `set_access`'s own "id not found is a silent no-op" contract).
    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get(id) else {
            return false;
        };
        let children: Vec<NodeId> = node.children.clone();
        let parent = node.parent;

        for child in children {
            self.remove(child);
        }

        if let Some(parent) = parent
            && let Some(parent_node) = self.nodes.get_mut(parent)
        {
            parent_node.children.retain(|&c| c != id);
        }

        if let Some(taffy_node) = self.taffy_nodes.remove(id) {
            let _ = self.taffy.remove(taffy_node);
        }
        self.nodes.remove(id);
        if self.focused == Some(id) {
            self.focused = None;
        }

        true
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
    /// node's `PaintProperties` and (§14 step 9) its `InteractionState`
    /// if it has one, returning `true` if any is still mid-animation. A
    /// naive whole-tree walk, not the "active set only" scoped version
    /// §5 describes -- see `PaintProperties::tick` for why that scoping
    /// is deliberately deferred past this step.
    pub fn tick_all(&mut self, now: Instant) -> bool {
        let mut any_active = false;
        for node in self.nodes.values_mut() {
            if node.paint.tick(now) {
                any_active = true;
            }
            if let Some(interaction) = &mut node.interaction
                && interaction.tick(now)
            {
                any_active = true;
            }
        }
        any_active
    }

    /// Opts one node into accessibility -- every node defaults to
    /// `AccessNodeData::default()` (`Role::Unknown`, no label/actions,
    /// effectively invisible to a screen reader) until a caller sets
    /// something real here (§14 step 7).
    pub fn set_access(&mut self, id: NodeId, access: AccessNodeData) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.access = access;
        }
    }

    /// Opts one node into interaction state (ripple/hover/focus, §7.3),
    /// lazily creating it on first use -- mirrors `set_access`'s "every
    /// node defaults to nothing until a caller opts in" shape. Returns
    /// `None` only if `id` doesn't exist in this `Tree`.
    pub fn interaction_mut(&mut self, id: NodeId) -> Option<&mut InteractionState> {
        let node = self.nodes.get_mut(id)?;
        Some(node.interaction.get_or_insert_with(InteractionState::new))
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    pub fn set_focused(&mut self, id: Option<NodeId>) {
        self.focused = id;
    }

    /// Builds a fresh `accesskit::TreeUpdate` from the current `Node`
    /// tree (§10: "built fresh... every frame, not maintained as a
    /// separate parallel structure that can drift out of sync"). `root`
    /// must already have a computed layout (`compute_layout`) -- bounds
    /// come from the same taffy `Layout` `engine-render` paints from,
    /// accumulated the same way `build_tree_scene`'s own walk does, so
    /// the accessible tree's geometry can never disagree with what's
    /// actually on screen.
    pub fn build_access_update(&self, root: NodeId) -> accesskit::TreeUpdate {
        let mut nodes = Vec::new();
        self.collect_access_nodes(root, 0.0, 0.0, &mut nodes);
        let root_id = to_access_id(root);
        let focus = self.focused.map(to_access_id).unwrap_or(root_id);
        accesskit::TreeUpdate {
            nodes,
            tree: Some(accesskit::TreeInfo::new(root_id)),
            tree_id: accesskit::TreeId::ROOT,
            focus,
        }
    }

    fn collect_access_nodes(
        &self,
        id: NodeId,
        offset_x: f64,
        offset_y: f64,
        out: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        let node = self
            .nodes
            .get(id)
            .expect("build_access_update: NodeId not found in this Tree");
        let layout = self.layout(id);
        let x = offset_x + f64::from(layout.location.x);
        let y = offset_y + f64::from(layout.location.y);
        let w = f64::from(layout.size.width);
        let h = f64::from(layout.size.height);

        let mut access_node = accesskit::Node::new(node.access.role);
        if let Some(label) = &node.access.label {
            access_node.set_label(label.clone());
        }
        if let Some(description) = &node.access.description {
            access_node.set_description(description.clone());
        }
        for &action in &node.access.actions {
            access_node.add_action(action);
        }
        if node.access.states.disabled {
            access_node.set_disabled();
        }
        access_node.set_bounds(accesskit::Rect {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        });
        let children: Vec<accesskit::NodeId> =
            node.children.iter().copied().map(to_access_id).collect();
        access_node.set_children(children);

        out.push((to_access_id(id), access_node));

        for &child in &node.children {
            self.collect_access_nodes(child, x, y, out);
        }
    }
}

/// A stable, deterministic mapping from this crate's own generational
/// `NodeId` (a `slotmap` key, opaque by design) to accesskit's
/// `NodeId(u64)` -- `KeyData::as_ffi` is `slotmap`'s own purpose-built
/// conversion for exactly this "hand a key to a foreign API" case.
fn to_access_id(id: NodeId) -> accesskit::NodeId {
    accesskit::NodeId(id.data().as_ffi())
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
    fn remove_drops_a_whole_subtree_and_detaches_from_its_parent() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let root = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let child = tree.insert(k, s, p);
        tree.add_child(root, child);
        let (k, s, p) = leaf(5.0, 5.0);
        let grandchild = tree.insert(k, s, p);
        tree.add_child(child, grandchild);

        // A sibling that must survive the removal, to prove this isn't
        // just clearing the whole tree.
        let (k, s, p) = leaf(10.0, 10.0);
        let sibling = tree.insert(k, s, p);
        tree.add_child(root, sibling);

        let removed = tree.remove(child);
        assert!(removed, "remove must report true for a real, present id");

        assert!(
            tree.get(child).is_none(),
            "the removed node itself must be gone"
        );
        assert!(
            tree.get(grandchild).is_none(),
            "the whole subtree must be gone, not just its root"
        );
        assert!(
            tree.get(sibling).is_some(),
            "an unrelated sibling must survive"
        );
        assert_eq!(
            tree.get(root).unwrap().children,
            vec![sibling],
            "the parent's own children list must no longer mention the removed node"
        );
    }

    #[test]
    fn remove_of_an_unknown_id_is_a_harmless_no_op() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let real = tree.insert(k, s, p);
        tree.remove(real); // consume the only real id so it's now stale

        assert!(
            !tree.remove(real),
            "removing an already-removed id must report false, not panic"
        );
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

    /// Proves `interaction_mut`/`tick_all` actually compose (§14 step
    /// 9) -- not just that `InteractionState::tick` works in isolation
    /// (already covered in `interaction.rs`'s own tests), but that
    /// `Tree::tick_all` genuinely reaches a node's interaction state
    /// during its whole-tree walk, the same claim
    /// `tick_all_reports_active_and_advances_every_node` proves for
    /// `PaintProperties`.
    #[test]
    fn tick_all_also_advances_a_nodes_interaction_state() {
        use std::time::Duration;

        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(10.0, 10.0);
        let id = tree.insert(kind, style, paint);
        let start = Instant::now();

        tree.interaction_mut(id).unwrap().spawn_ripple(
            peniko::kurbo::Point::new(5.0, 5.0),
            50.0,
            1.0,
            Duration::from_millis(200),
            start,
        );

        let still_active = tree.tick_all(start + Duration::from_millis(100));
        assert!(
            still_active,
            "a mid-flight ripple should keep tick_all reporting active"
        );
        let radius = tree.get(id).unwrap().interaction.as_ref().unwrap().ripples[0]
            .radius
            .current;
        assert!(
            (radius - 25.0).abs() < 0.01,
            "ripple radius should be ~halfway to 50.0, got {radius}"
        );

        let still_active = tree.tick_all(start + Duration::from_secs(1));
        assert!(!still_active);
        assert!(
            tree.get(id)
                .unwrap()
                .interaction
                .as_ref()
                .unwrap()
                .ripples
                .is_empty(),
            "the finished ripple should have been pruned by tick_all"
        );
    }

    #[test]
    fn build_access_update_exposes_a_button_with_the_right_role_label_and_bounds() {
        use crate::access::AccessNodeData;
        use crate::access::{Action, Role};

        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(120.0, 40.0);
        let button = tree.insert(kind, style, paint);
        tree.set_access(
            button,
            AccessNodeData::new(Role::Button)
                .with_label("Save")
                .with_action(Action::Click),
        );
        tree.compute_layout(
            button,
            Size {
                width: AvailableSpace::Definite(120.0),
                height: AvailableSpace::Definite(40.0),
            },
        );

        let update = tree.build_access_update(button);
        assert_eq!(
            update.nodes.len(),
            1,
            "expected exactly the one button node"
        );
        let (id, node) = &update.nodes[0];
        assert_eq!(*id, to_access_id(button));
        assert_eq!(node.role(), Role::Button);
        assert_eq!(node.label(), Some("Save"));
        assert!(
            node.supports_action(Action::Click),
            "expected the button's Click action to be reported"
        );
        let bounds = node.bounds().expect("a laid-out node must report bounds");
        assert_eq!(
            (bounds.x0, bounds.y0, bounds.x1, bounds.y1),
            (0.0, 0.0, 120.0, 40.0)
        );

        // `focus` must be a valid value even though nothing was
        // explicitly focused -- §10: "if no specific node has keyboard
        // focus, this must be set to the root."
        assert_eq!(update.focus, to_access_id(button));
        assert_eq!(update.tree_id, accesskit::TreeId::ROOT);
    }
}
