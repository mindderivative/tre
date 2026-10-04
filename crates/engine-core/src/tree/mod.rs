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

use std::collections::HashSet;
use std::time::Instant;

use slotmap::{Key as SlotMapKey, KeyData, SecondaryMap};

use self::nodes::Nodes;
pub use self::nodes::Touched;
use taffy::prelude::{
    AvailableSpace, Layout, Position, Rect as TaffyRect, Size, Style, TaffyTree, auto, length,
};

use crate::access::AccessNodeData;
use crate::animation::CompletionHandle;
#[cfg(test)]
use crate::canvas::CanvasState;
use crate::canvas::{CustomHitTest, DrawCommand};
use crate::input::{ChangedValue, DispatchOutcome, InputEvent, Key, PointerButton, ScrollDelta};
use crate::node::{
    ImageState, Node, NodeId, NodeKind, PaintProperties, SCROLLBAR_GRAB_SLOP, SCROLLBAR_MARGIN,
    SCROLLBAR_THICKNESS, TextFieldState,
};
#[cfg(test)]
use crate::node::{ItemExtent, TerminalCell, TerminalState, VirtualListState};
use crate::overlay::{OverlayMeta, Placement};
#[cfg(test)]
use peniko::kurbo::BezPath;
use peniko::kurbo::{Affine, ParamCurveNearest, Point, Rect};

mod access;
pub use access::{AccessLine, TextPart};
mod dispatch;
mod focus;
mod layers;
mod layout;
mod nodes;
mod scroll;
pub use scroll::KEY_SCROLL_LINE;
#[cfg(test)]
mod tests;
mod text_editing;

/// `Tree::move_focus`'s own direction -- Tab vs. Shift-Tab (§10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDirection {
    Next,
    Previous,
}

pub struct Tree {
    nodes: Nodes,
    /// 0.5.4 (#125): counts the times taffy computed a layout, so a consumer
    /// can tell that no node moved or changed size since it last looked.
    layout_epoch: u64,
    taffy_nodes: SecondaryMap<NodeId, taffy::NodeId>,
    taffy: TaffyTree<()>,
    /// §14 step 7: which node `TreeUpdate.focus` reports. Moved by
    /// `move_focus` (M4 Phase 1 step 1, §10) in real Tab/Shift-Tab tree
    /// order, not left as inert plain data anymore.
    focused: Option<NodeId>,
    /// M4 Phase 1 step 1 (§7.3): the node `update_hover` last reported
    /// as hit, so a repeated `PointerMoved` over the same node is a
    /// no-op rather than re-triggering a hover transition every frame.
    hovered: Option<NodeId>,
    /// M4 Phase 1 step 1 (§11.10 dispatch): the `(button, node)` a
    /// `PointerPressed` last hit, so `PointerReleased` can tell a real
    /// click (same button, same node) from a drag-off (different node,
    /// no node, or a different button released) apart. A single slot,
    /// not one per button -- this minimal mouse-only model never needs
    /// to track two buttons held down at once.
    pressed: Option<(PointerButton, NodeId)>,
    /// M4 Phase 3 (§11.5): the node currently being pointer-dragged, if
    /// any -- a `ScrollView`/`VirtualList` whose scrollbar thumb a
    /// primary-button `PointerPressed` grabbed, read by every
    /// subsequent `PointerMoved` until a primary-button `PointerRelease
    /// d` clears it (wherever that happens, not conditioned on still
    /// hitting the node -- a real mouse-up always ends a drag, matching
    /// real OS drag semantics). `update_drag` is what actually branches
    /// on which real kind this is.
    dragging: Option<NodeId>,
    /// §14 step 13 (§11.3): keyed by the overlay root's own `NodeId` --
    /// metadata only, never the node itself, which already lives in
    /// `nodes` like any other.
    overlays: Vec<(NodeId, OverlayMeta)>,
    /// M96: layers an outside press or Escape asked to dismiss, since the
    /// last `take_dismissals`.
    dismissals: Vec<NodeId>,
    /// M29 Phase 1 (§5, §6): coarse, whole-tree "does the next frame
    /// need real paint/GPU work at all" signal -- set `true` by every
    /// real mutating method below (deliberately conservative: a method
    /// that sets it even when its own specific call turned out to be a
    /// no-op costs nothing, since idle frames are the case this exists
    /// to skip, not frames already doing real input/animation work) and
    /// by `tick_all` whenever it reports `any_active`. Starts `true` so
    /// the very first frame always paints. Read via `take_dirty`, never
    /// this field directly, so "read" and "reset" can never drift apart.
    dirty: bool,
    /// 0.5.4 (#112): the text node whose static selection is showing, if any.
    /// One at a time, as on a desktop.
    static_selection: text_editing::StaticSelection,
    /// 0.5.4 (#153): the shaped lines of text nodes, for their accessibility runs.
    text_lines: std::collections::HashMap<NodeId, Vec<access::AccessLine>>,
    /// 0.5.4 (#103): the nodes the last full scan found animating, and the
    /// `animations_started` count it was taken at. See `tick_all`.
    animating: Vec<NodeId>,
    /// 0.5.4 (#105): the root and available space the last `compute_layout`
    /// answered, to skip a repeat of the same question.
    last_layout: Option<(NodeId, Size<AvailableSpace>)>,
    scanned_at: Option<u64>,
    scroll_view_count: usize,
    virtual_list_count: usize,
    /// 0.5.4 (#150): how many `Image` and `Svg` nodes the tree holds, so a
    /// per-frame consumer of them can skip scanning a tree that has none.
    image_count: usize,
    svg_count: usize,
    /// M94: the node holding pointer capture (`set_pointer_capture`).
    pointer_capture: Option<NodeId>,
    /// M96: detached subtree roots that are freed once nothing outside the
    /// tree references anything in their subtree (`detach_collectible`,
    /// `collect_unreferenced`). A root leaves the set when it's attached
    /// again. Content detached any other way (a plain `detach`, inactive
    /// dock panels) is never collected.
    collectible: HashSet<NodeId>,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

/// The ordinary hit-test default (§11.10): a node-local point is
/// "inside" if it falls within the node's own untransformed layout box,
/// `(0, 0)` to `(width, height)` -- shared by every `NodeKind` with no
/// `CustomHitTest` override (M5 Phase 3).
fn rect_contains(layout: &Layout, local_point: Point) -> bool {
    let bounds = Rect::new(
        0.0,
        0.0,
        f64::from(layout.size.width),
        f64::from(layout.size.height),
    );
    bounds.contains(local_point)
}

impl Tree {
    pub fn new() -> Self {
        Self {
            nodes: Nodes::new(),
            layout_epoch: 0,
            animating: Vec::new(),
            static_selection: Default::default(),
            text_lines: Default::default(),
            last_layout: None,
            scanned_at: None,
            taffy_nodes: SecondaryMap::new(),
            taffy: TaffyTree::new(),
            focused: None,
            hovered: None,
            pressed: None,
            dragging: None,
            overlays: Vec::new(),
            dismissals: Vec::new(),
            dirty: true,
            scroll_view_count: 0,
            virtual_list_count: 0,
            image_count: 0,
            svg_count: 0,
            pointer_capture: None,
            collectible: HashSet::new(),
        }
    }

    /// M96: detaches `id` from its parent, if it has one, and makes it a
    /// collectible root: it stays alive, and reattachable, until
    /// `collect_unreferenced` finds nothing referencing its subtree.
    pub fn detach_collectible(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes.get(id).and_then(|n| n.parent) {
            self.detach(parent, id);
        }
        self.collectible.insert(id);
    }

    /// M96: frees `id` and its whole subtree, returning every freed id.
    pub fn destroy(&mut self, id: NodeId) -> Vec<NodeId> {
        let mut freed = Vec::new();
        let mut stack = vec![id];
        while let Some(next) = stack.pop() {
            if let Some(node) = self.nodes.get(next) {
                freed.push(next);
                stack.extend(node.children.iter().copied());
            }
        }
        self.remove(id);
        freed
    }

    /// M96: frees the collectible subtree `id` belongs to if `referenced`
    /// is false for every node in it -- the caller's own record of which
    /// nodes it still holds handles to. Returns every freed id; nothing is
    /// freed when `id`'s root isn't collectible.
    pub fn collect_unreferenced(
        &mut self,
        id: NodeId,
        referenced: impl Fn(NodeId) -> bool,
    ) -> Vec<NodeId> {
        if !self.nodes.contains_key(id) {
            return Vec::new();
        }
        let root = self.root_of(id);
        if !self.collectible.contains(&root) {
            return Vec::new();
        }
        let mut stack = vec![root];
        while let Some(next) = stack.pop() {
            if referenced(next) {
                return Vec::new();
            }
            if let Some(node) = self.nodes.get(next) {
                stack.extend(node.children.iter().copied());
            }
        }
        self.destroy(root)
    }

    /// M29 Phase 1 (§5, §6): reads and clears the dirty flag in one
    /// step (`std::mem::replace`) -- a caller that read `true`, then
    /// crashed or skipped acting on it before ever resetting the flag,
    /// would otherwise leave every subsequent frame permanently
    /// "still dirty from before" or, the opposite bug, a separate
    /// read-then-a-separate-reset pair could race a real mutation
    /// landing in between. One atomic-with-respect-to-this-`&mut self`
    /// operation instead.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.dirty, false)
    }

    /// Marks the tree as needing a frame -- for a caller that took the
    /// flag but couldn't draw that frame (0.4.0 review: a surface that
    /// had to be reconfigured first), so the next one does.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Creates a new, parentless node (attach it under another with
    /// `add_child`, or leave it as a root passed to `compute_layout`).
    pub fn insert(
        &mut self,
        kind: NodeKind,
        layout_style: Style,
        paint: PaintProperties,
    ) -> NodeId {
        self.dirty = true;
        // M65 (§5, §6): the real, single increment choke point for the
        // per-kind existence counters `compute_layout`'s own sync
        // functions consult -- see their own shared doc comment.
        match &kind {
            NodeKind::ScrollView(_) => self.scroll_view_count += 1,
            NodeKind::VirtualList(_) => self.virtual_list_count += 1,
            NodeKind::Image(_) => self.image_count += 1,
            NodeKind::Svg(_) => self.svg_count += 1,
            _ => {}
        }
        let taffy_node = self
            .taffy
            .new_leaf(layout::laid_out_style(&kind, &layout_style))
            .expect("TaffyTree::new_leaf is infallible for a leaf with no children");
        let id = self.nodes.insert_with_key(|id| Node {
            id,
            parent: None,
            children: Vec::new(),
            kind,
            layout_style,
            paint,
            access: AccessNodeData::default(),
            hit_testable: true,
            sticky: None,
            cursor: None,
            window_region: crate::node::WindowRegion::Default,
            shader: None,
            visible: true,
            z_index: 0,
        });
        self.taffy_nodes.insert(id, taffy_node);
        id
    }

    /// M30 Phase 5 Step 1 (§5, §7): opts `id` out of independently
    /// claiming a hit in `hit_test_at` -- see `Node::hit_testable`'s own
    /// doc comment (`NodeKind::Text`'s own hardcoded exemption, widened
    /// into an opt-in flag for a decorative `Rect` layer). A no-op call
    /// (`id` already at `hit_testable`) is harmless; panics if `id` is
    /// stale/foreign to this `Tree`, the same real contract every
    /// other single-node setter here already has.
    pub fn set_hit_testable(&mut self, id: NodeId, hit_testable: bool) {
        self.nodes
            .get_mut(id)
            .expect("set_hit_testable: NodeId not found in this Tree")
            .hit_testable = hit_testable;
    }

    /// Attaches `child` under `parent` in both structures at once, so
    /// they can never diverge. Panics if either id is stale/foreign to
    /// this `Tree` -- an internal bookkeeping bug, not a recoverable
    /// runtime condition (unlike the GPU/display absence this codebase
    /// exits gracefully for elsewhere).
    ///
    /// A `child` that's already attached is *moved* (`try_add_child`),
    /// never left under two parents -- the "`add_child` has no dedup"
    /// bug class found three times (overlays, M4; docking, M4; docking
    /// again, issue #14), closed here rather than per caller.
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        if self
            .nodes
            .get(child)
            .expect("add_child: child NodeId not found in this Tree")
            .parent
            .is_some()
        {
            self.try_add_child(parent, child);
            return;
        }
        // M96: below any open overlay, so content never paints over one.
        let index = self.content_len(parent);
        self.attach_at(parent, index, child);
    }

    /// Attaches `child` under `parent` at `index`, in both structures.
    fn attach_at(&mut self, parent: NodeId, index: usize, child: NodeId) {
        self.dirty = true;
        let parent_taffy = *self
            .taffy_nodes
            .get(parent)
            .expect("add_child: parent NodeId not found in this Tree");
        let child_taffy = *self
            .taffy_nodes
            .get(child)
            .expect("add_child: child NodeId not found in this Tree");
        self.taffy
            .insert_child_at_index(parent_taffy, index, child_taffy)
            .expect("add_child: taffy rejected the parent/child pair");
        self.nodes[parent].children.insert(index, child);
        self.nodes[child].parent = Some(parent);
        self.collectible.remove(&child);
    }

    /// M96: how many of `parent`'s children come before its open
    /// overlays -- its content, which open overlays always sit above.
    fn content_len(&self, parent: NodeId) -> usize {
        self.nodes[parent]
            .children
            .iter()
            .take_while(|child| !self.is_overlay(**child))
            .count()
    }

    /// M96: `parent`'s children other than its open overlays, in order.
    pub fn content_children(&self, parent: NodeId) -> &[NodeId] {
        match self.nodes.get(parent) {
            Some(node) => &node.children[..self.content_len(parent)],
            None => &[],
        }
    }

    fn is_overlay(&self, id: NodeId) -> bool {
        self.overlays.iter().any(|(content, _)| *content == id)
    }

    /// M6 Phase 1 (§8): the checked counterpart to `add_child`, for the
    /// one caller that can't structurally guarantee it won't form a
    /// cycle -- Python's own `node.add_child`. Every existing internal
    /// caller of `add_child` already knows it can't (attaching a
    /// freshly-inserted node, or a reparent already proven disjoint),
    /// and keeps calling the cheaper, infallible `add_child` directly;
    /// `add_child` itself and its ~80 existing call sites are untouched.
    ///
    /// Returns `false` (no mutation at all) if attaching `child` under
    /// `parent` would create a cycle -- `child` is `parent` itself, or
    /// `child` is already an ancestor of `parent` -- detected by walking
    /// up from `parent` via `Node::parent` links (the same walk
    /// `absolute_position` already uses) and checking whether `child`
    /// appears in that chain; `parent == child` is caught for free as
    /// the walk's own first iteration.
    ///
    /// If `child` already has a different parent, detaches it first
    /// (`Tree::detach`) before attaching -- the same "`add_child` has no
    /// dedup" bug class M4 Phase 7 (overlay) and M4 Phase 9 (docking)
    /// each already found and fixed once for their own specific caller;
    /// this is the first general-purpose, arbitrary-reparenting entry
    /// point, and the one most likely to hit it a third time.
    pub fn try_add_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        let end = self.content_len(parent);
        let end = if self.content_children(parent).contains(&child) {
            end - 1
        } else {
            end
        };
        self.insert_child(parent, end, child)
    }

    /// M96: attaches `child` under `parent` so it ends up at `index` in
    /// `parent`'s children -- the keyed-reorder primitive. A child that
    /// already has a parent (the same one or another) is *moved*: it keeps
    /// its `NodeId`, state, and running animations, and keeps focus as long
    /// as it stays under the same root. `index` counts the children as they
    /// are once `child` has been taken out of its old place; past the end
    /// it appends. Returns `false`, changing nothing, if `child` is
    /// `parent` or one of its ancestors.
    pub fn insert_child(&mut self, parent: NodeId, index: usize, child: NodeId) -> bool {
        if self.ancestors(parent).any(|id| id == child) {
            return false;
        }
        self.dirty = true;
        let root_before = self.root_of(child);
        if let Some(old_parent) = self.nodes[child].parent {
            self.unlink(old_parent, child);
        }
        let index = index.min(self.content_len(parent));
        self.attach_at(parent, index, child);
        if self.root_of(child) != root_before {
            self.forget_interaction_in(child);
        }
        true
    }

    /// M96: `id`'s children in paint order -- bottom first, by `z_index`,
    /// equal values keeping child order. Borrows the child list unless a
    /// child has a nonzero `z_index`.
    pub fn children_in_paint_order(&self, id: NodeId) -> std::borrow::Cow<'_, [NodeId]> {
        let Some(node) = self.nodes.get(id) else {
            return std::borrow::Cow::Borrowed(&[]);
        };
        let z = |child: &NodeId| self.nodes.get(*child).map_or(0, |n| n.z_index);
        if node.children.iter().all(|child| z(child) == 0) {
            return std::borrow::Cow::Borrowed(&node.children);
        }
        let mut ordered = node.children.clone();
        ordered.sort_by_key(z);
        std::borrow::Cow::Owned(ordered)
    }

    /// `id` and every ancestor up to its root, innermost first.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(Some(id), |&id| self.nodes.get(id).and_then(|n| n.parent))
            .filter(|&id| self.nodes.contains_key(id))
    }

    /// The top of `id`'s tree: `id` itself when it has no parent.
    pub fn root_of(&self, id: NodeId) -> NodeId {
        self.ancestors(id).last().unwrap_or(id)
    }

    /// Takes `child` out of `parent`'s children in both structures, and
    /// nothing else -- `detach` and `insert_child` decide what else a
    /// detach or a move means.
    fn unlink(&mut self, parent: NodeId, child: NodeId) {
        let parent_taffy = *self
            .taffy_nodes
            .get(parent)
            .expect("detach: parent NodeId not found in this Tree");
        let child_taffy = *self
            .taffy_nodes
            .get(child)
            .expect("detach: child NodeId not found in this Tree");
        self.taffy
            .remove_child(parent_taffy, child_taffy)
            .expect("detach: taffy rejected removing this parent/child pair");
        self.nodes[parent].children.retain(|&c| c != child);
        self.nodes[child].parent = None;
    }

    /// Clears focus, hover, press, and pointer capture held by `id` or a
    /// descendant -- for a subtree that left the tree it was reachable in,
    /// where none of them can stay meaningful.
    fn forget_interaction_in(&mut self, id: NodeId) {
        let inside = |tree: &Self, held: Option<NodeId>| {
            held.is_some_and(|held| tree.ancestors(held).any(|a| a == id))
        };
        if inside(self, self.focused) {
            self.focused = None;
        }
        if inside(self, self.hovered) {
            self.hovered = None;
        }
        if inside(self, self.pointer_capture) {
            self.pointer_capture = None;
        }
        if inside(self, self.pressed.map(|(_, node)| node)) {
            self.pressed = None;
        }
    }

    /// The inverse of `add_child`: detaches `child` from `parent`
    /// *without* deleting it (unlike `Tree::remove`, which deletes the
    /// whole subtree) -- `child` stays alive, parentless, ready for
    /// `add_child` elsewhere later. §14 step 15's own real need: docking
    /// (§11.4) tabbed grouping switches which panel is currently
    /// attached to a zone, without discarding the panels that aren't
    /// showing right now. `taffy::TaffyTree::remove_child` exists for
    /// exactly this -- verified directly in its own doc comment ("not
    /// removed from the tree entirely, simply no longer attached to its
    /// previous parent") before using it.
    pub fn detach(&mut self, parent: NodeId, child: NodeId) {
        self.dirty = true;
        self.unlink(parent, child);
        // A detached node is no longer reachable from any root, the
        // same "can't stay meaningfully focused" reasoning `remove`
        // already applies -- if it's reattached later, its own
        // eventual re-focus is whatever caller reattached it decides,
        // not a stale pointer surviving from before.
        self.forget_interaction_in(child);
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// 0.5.4 (#146): every SVG node whose document has text is given the
    /// document `reparse` makes from its source -- the fonts changed, so its
    /// outlines must. `None` keeps a node's document. Marks the tree dirty
    /// when any changed.
    pub fn reparse_svgs(
        &mut self,
        mut reparse: impl FnMut(
            &crate::svg::SvgState,
        ) -> Option<std::sync::Arc<crate::svg::SvgDocument>>,
    ) {
        let mut changed = false;
        for node in self.nodes.values_mut() {
            if let NodeKind::Svg(state) = &mut node.kind
                && state.document.has_text
                && let Some(document) = reparse(state)
            {
                state.document = document;
                changed = true;
            }
        }
        if changed {
            self.mark_dirty();
        }
    }

    /// 0.5.4 (#125): the nodes handed out mutably since the last call (see
    /// `Touched`), and a fresh start. For the damage tracker: a node absent
    /// from this was not written to.
    pub fn take_touched(&self) -> Touched {
        self.nodes.take_touched()
    }

    /// 0.5.4 (#125): how many times a layout has been computed. While it is
    /// unchanged, no node's position or size has.
    pub fn layout_epoch(&self) -> u64 {
        self.layout_epoch
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.dirty = true;
        self.nodes.get_mut(id)
    }

    /// Recursively removes `id` and its whole subtree (§14 step 12: a
    /// removed node must take its descendants with it, not just its
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
        self.dirty = true;
        let Some(node) = self.nodes.get(id) else {
            return false;
        };
        let children: Vec<NodeId> = node.children.clone();
        let parent = node.parent;
        // M65 (§5, §6): the real, single decrement choke point --
        // mirrors `insert`'s own increment above exactly. Read while
        // `node` is still borrowed, before `self.nodes.remove(id)`
        // below invalidates it, and before the recursive removes just
        // below (each of which independently re-enters this same
        // function and does its own decrement for its own child).
        match &node.kind {
            NodeKind::ScrollView(_) => self.scroll_view_count -= 1,
            NodeKind::VirtualList(_) => self.virtual_list_count -= 1,
            NodeKind::Image(_) => self.image_count -= 1,
            NodeKind::Svg(_) => self.svg_count -= 1,
            _ => {}
        }

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
        self.collectible.remove(&id);
        // Review follow-through (M28 Phase 1, §11.3): `close_overlay` is
        // the only other place that ever cleared a `self.overlays`
        // entry -- a caller removing the same content through this
        // general-purpose method instead (or removing one of its
        // ancestors, reached via the recursion above) left it behind
        // forever. A plain `HashMap::remove` is a no-op for the (vast
        // majority of) ids that were never overlay content, so this
        // costs nothing on the common path.
        self.overlays.retain(|(content, _)| *content != id);
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.pointer_capture == Some(id) {
            self.pointer_capture = None;
        }
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        // 0.5.4 (#131, #153): a removed text node keeps no selection or lines.
        self.text_lines.remove(&id);
        if self.static_selection.involves(id) {
            self.clear_text_selection();
        }

        true
    }

    /// One node's animatable values, ticked: whether any is still running.
    fn tick_node(node: &mut Node, now: Instant, completed: &mut Vec<CompletionHandle>) -> bool {
        let mut active = node.paint.tick(now, completed);
        // M95: a path's data (morphing) and stroke trim.
        if let NodeKind::Path(state) = &mut node.kind
            && state.tick(now, completed)
        {
            active = true;
        }
        // M96: a text input's animatable `fill` (its text color), and a
        // scroll view's animatable `scroll_offset`.
        if let NodeKind::TextField(state) = &mut node.kind
            && state.text_tint.tick(now, completed)
        {
            active = true;
        }
        if let NodeKind::ScrollView(state) = &mut node.kind
            && state.scroll.tick(now, completed)
        {
            active = true;
        }
        // 0.5.4 (#136): a virtual list's offset animates too (a fling).
        if let NodeKind::VirtualList(state) = &mut node.kind
            && state.scroll_offset.tick(now, completed)
        {
            active = true;
        }
        active
    }

    /// Advances every running animation to `now`. A full pass over the
    /// tree happens only when an animation has started somewhere since the
    /// last one (`animations_started`); otherwise only the nodes that pass
    /// found animating are ticked, so the cost follows the animations, not
    /// the tree's size (0.5.4, #103).
    /// 0.5.4 (#139): makes `id` sticky, `inset` pixels from the start edge of the
    /// scroller it sits in, or ordinary again with `None`. A no-op for a missing
    /// node. See `Node::sticky`.
    pub fn set_sticky(&mut self, id: NodeId, inset: Option<f64>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.sticky = inset;
            self.dirty = true;
        }
    }

    /// 0.5.4 (#116): how many nodes the tree holds.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn tick_all(&mut self, now: Instant) -> (bool, Vec<CompletionHandle>) {
        let mut completed = Vec::new();
        let started = crate::animation::animations_started();
        let full = self.scanned_at != Some(started);
        let mut still = Vec::new();
        if full {
            for (id, node) in &mut self.nodes {
                if Self::tick_node(node, now, &mut completed) {
                    still.push(id);
                }
            }
            self.scanned_at = Some(started);
        } else {
            for &id in &self.animating {
                if let Some(node) = self.nodes.get_mut(id)
                    && Self::tick_node(node, now, &mut completed)
                {
                    still.push(id);
                }
            }
        }
        self.animating = still;
        let any_active = !self.animating.is_empty();
        // M29 Phase 1 (§5, §6): a mid-flight animation is itself a real
        // reason to redraw next frame -- `any_active` was already the
        // exact signal this needs, just never fed into a redraw
        // decision before now.
        if any_active {
            self.dirty = true;
        }
        (any_active, completed)
    }

    /// Opts one node into accessibility -- every node defaults to
    /// `AccessNodeData::default()` (`Role::Unknown`, no label/actions,
    /// effectively invisible to a screen reader) until a caller sets
    /// something real here (§14 step 7).
    pub fn set_access(&mut self, id: NodeId, access: AccessNodeData) {
        self.dirty = true;
        if let Some(node) = self.nodes.get_mut(id) {
            node.access = access;
        }
    }

    /// M5 Phase 3 (§11.10, §11.11): replaces a `NodeKind::Canvas`
    /// node's entire real content -- both what `draw_own` draws and
    /// what `hit_test_at` tests against. The one, ordinary (non-
    /// callback) `Tree` mutation `engine-py` makes after invoking the
    /// app's Python `draw` callback and collecting its result -- see `canvas.rs`'s own module
    /// doc comment for why the callback itself never reaches this far.
    /// Returns `None` if `id` doesn't exist or isn't a `Canvas`.
    pub fn set_canvas_content(
        &mut self,
        id: NodeId,
        commands: Vec<DrawCommand>,
        hit_test: Option<CustomHitTest>,
    ) -> Option<()> {
        self.dirty = true;
        let node = self.nodes.get_mut(id)?;
        match &mut node.kind {
            NodeKind::Canvas(state) => {
                state.commands = commands;
                state.hit_test = hit_test;
                Some(())
            }
            _ => None,
        }
    }

    /// M22 Phase 1 (§5): every real `Image` node currently in this
    /// tree -- `engine-render::FrameRenderer::sync_image_textures`'s
    /// own real need, to know which GPU textures must exist before a
    /// real `render()` call, without `engine-render` ever reaching
    /// into this `Tree`'s own private `nodes` map directly (§4's
    /// crate-boundary rule).
    /// 0.5.4 (#150): whether the tree holds any `Image` node.
    pub fn has_images(&self) -> bool {
        debug_assert_eq!(
            self.image_count > 0,
            self.nodes
                .values()
                .any(|n| matches!(n.kind, NodeKind::Image(_))),
            "a node's kind was changed after it was made: the tree counts nodes by kind"
        );
        self.image_count > 0
    }

    /// 0.5.4 (#150): whether the tree holds any `Svg` node.
    pub fn has_svgs(&self) -> bool {
        debug_assert_eq!(
            self.svg_count > 0,
            self.nodes
                .values()
                .any(|n| matches!(n.kind, NodeKind::Svg(_))),
            "a node's kind was changed after it was made: the tree counts nodes by kind"
        );
        self.svg_count > 0
    }

    pub fn image_nodes(&self) -> impl Iterator<Item = (NodeId, &ImageState)> {
        self.nodes.iter().filter_map(|(id, node)| match &node.kind {
            NodeKind::Image(state) => Some((id, state)),
            _ => None,
        })
    }

    /// 0.5.4 (#147): every raster image the tree's SVG nodes were given, for
    /// the renderer to upload as textures.
    pub fn svg_bitmaps(&self) -> Vec<std::sync::Arc<crate::svg::SvgBitmap>> {
        self.nodes
            .values()
            .filter_map(|node| match &node.kind {
                NodeKind::Svg(state) => Some(state.images.0.iter().map(|(_, b)| b.clone())),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// 0.5.1 (#68): whether giving `id` `shader` would make a shader depend
    /// on itself. A shader's inputs are nodes; an input that is not an image
    /// node is sampled through its own shader, which has inputs in turn.
    /// (An image node as an input means its image pixels, so it ends the
    /// chain.)
    pub fn shader_cycle(&self, id: NodeId, shader: &crate::Shader) -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut stack: Vec<NodeId> = shader.inputs().iter().map(|(_, n)| *n).collect();
        while let Some(next) = stack.pop() {
            let Some(node) = self.nodes.get(next) else {
                continue;
            };
            if matches!(node.kind, NodeKind::Image(_)) {
                continue;
            }
            if next == id {
                return true;
            }
            if !seen.insert(next) {
                continue;
            }
            if let Some(inner) = &node.shader {
                stack.extend(inner.inputs().iter().map(|(_, n)| *n));
            }
        }
        false
    }

    /// 0.5.1 (#67): whether any node has a shader -- the renderer's cheap
    /// check before it plans shader passes.
    pub fn has_shaders(&self) -> bool {
        self.nodes.values().any(|node| node.shader.is_some())
    }

    /// M15 Phase 2 (§8, §10): real keyboard-driven `TextField` editing
    /// -- the `Tree`'s own real mutator (a real keystroke is
    /// mechanical, not app-defined meaning, so `engine-core` is the one
    /// real owner here). `content`/`cursor` stay on real UTF-8 char
    /// boundaries throughout via `char_indices` -- grapheme-cluster
    /// and BiDi-visual-order movement are `parley::editing::Selection`
    /// 's own richer job, deliberately not reused here (`engine-core`
    /// has no `parley` dependency at all, §4).
    ///
    /// Returns `None` for a key this method doesn't claim (`Escape`
    /// always; `Tab` only for a *single-line* field -- M31 Phase 2
    /// widens a *multiline* field to claim `Tab` too, inserting a real
    /// `\t`) -- the caller falls through to the generic handling for
    /// those. Every other key returns `Some`: `Changed(field)` for
    /// a real content edit, `None` (the outcome, not the `Option`) for
    /// pure cursor movement or a genuine no-op (e.g. `Backspace` at
    /// `cursor == 0`) -- a `change` event only ever means "the text
    /// actually changed," and cursor position isn't the text.
    ///
    /// M15 Phase 3 (§16.7) adds real `shift`-driven selection: an
    /// arrow/`Home`/`End` key held with `shift` extends the selection
    /// from wherever it started (`selection_anchor` seeded from the
    /// pre-move cursor the first time, left alone on every further
    /// extend); the same key *without* `shift`, while a real selection
    /// is active, collapses to that selection's own near edge (real
    /// desktop-editor behavior) instead of moving one more character
    /// past the focus end. `Backspace`/`Delete`/a real inserted
    /// character (`Space` here, `TextInput` below) all delete a real,
    /// active selection first via the shared `delete_selection` helper
    /// -- replacing the selection, the same real behavior every desktop
    /// text editor has, not `engine-core` inventing a special case per
    /// key.
    /// M38 Phase 7 (§5, §8): VS Code's own real, cited default line-
    /// height multiplier (`editor.lineHeight`: `0` means "compute from
    /// `fontSize`", real default `fontSize * 1.35` on non-macOS) --
    /// `engine-core` has no real font-shaping access to measure an
    /// exact value itself (§4's crate-boundary rule), so this is a
    /// real, honest approximation for the caret-follow heuristic below,
    /// not an exact metric.
    const CODE_EDITOR_LINE_HEIGHT_RATIO: f64 = 1.35;

    /// M39 Phase 1 (§5, §8): a real, cited monospace character-advance-
    /// width estimate for the horizontal half of caret-follow, for the
    /// identical real reason `CODE_EDITOR_LINE_HEIGHT_RATIO` exists --
    /// `engine-core` has no font-shaping access to measure one exactly
    /// (§4). Not an external citation this time: this codebase's own
    /// real, historical precedent (found via `git log -p` on `engine-
    /// render/src/text.rs`, predating M32 Phase 1's switch to real
    /// measured `monospace_cell_size`) already used exactly this ratio
    /// for `Terminal`'s own analytic cell-width estimate before a real
    /// bundled monospace face existed to measure -- reused verbatim
    /// rather than re-derived.
    const CODE_EDITOR_CHAR_WIDTH_RATIO: f64 = 0.6;
}

/// A stable, deterministic mapping from this crate's own generational
/// `NodeId` (a `slotmap` key, opaque by design) to accesskit's
/// `NodeId(u64)` -- `KeyData::as_ffi` is `slotmap`'s own purpose-built
/// conversion for exactly this "hand a key to a foreign API" case.
/// `NodeId`'s own generational key data, encoded as the opaque 64-bit
/// integer `accesskit::NodeId` wraps -- `SlotMapKey::data().as_ffi()`
/// is real, documented `slotmap` API for exactly this ("pass slot map
/// keys as opaque handles to foreign code... confidently use them...
/// without worrying about unsafe behavior").
pub fn to_access_id(id: NodeId) -> accesskit::NodeId {
    accesskit::NodeId(id.data().as_ffi())
}

/// The reverse of [`to_access_id`] -- M4 Phase 2's real need: an
/// `accesskit::ActionRequest.target_node` (a platform accessibility
/// client naming which node it wants activated/focused, §10) arrives as
/// this same opaque integer and has to become a real `NodeId` again to
/// call `Tree::activate`/`set_focus_to` with. `KeyData::from_ffi`'s own
/// doc comment states the guarantee this relies on directly: "passing
/// it to `from_ffi` will return a key equal to the original" -- a
/// stale/foreign id round-trips to *some* `NodeId` value that simply
/// fails this `Tree`'s own generation check later (behaves as "not
/// found"), not undefined behavior, the same generational-safety
/// property §5 already relies on everywhere else.
pub fn from_access_id(id: accesskit::NodeId) -> NodeId {
    NodeId::from(KeyData::from_ffi(id.0))
}

/// M22 Phase 1 (§5): a stable `u64` for a `NodeId` -- `to_access_id`'s
/// own real `KeyData::as_ffi()` encoding, reused here for a different
/// foreign-handle consumer (`engine-render`'s own GPU texture cache
/// keys a real, persistent `vello_gpu::TextureId` per `Image` node
/// by this exact value, rather than inventing a second id scheme).
pub fn node_id_as_u64(id: NodeId) -> u64 {
    id.data().as_ffi()
}
