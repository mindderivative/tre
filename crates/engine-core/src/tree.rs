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

use std::collections::HashMap;
use std::time::{Duration, Instant};

use slotmap::{Key, SecondaryMap, SlotMap};
use taffy::prelude::{
    AvailableSpace, Layout, Position, Rect as TaffyRect, Size, Style, TaffyTree, auto, length,
};

use crate::access::AccessNodeData;
use crate::animation::MotionCurve;
use crate::interaction::InteractionState;
#[cfg(test)]
use crate::node::{ItemExtent, VirtualListState};
use crate::node::{Node, NodeId, NodeKind, PaintProperties};
use crate::overlay::OverlayMeta;

pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    taffy_nodes: SecondaryMap<NodeId, taffy::NodeId>,
    taffy: TaffyTree<()>,
    /// §14 step 7: which node `TreeUpdate.focus` reports. Plain data --
    /// see `access.rs`'s module doc comment for why the actual Tab/
    /// Shift-Tab traversal logic isn't wired up yet.
    focused: Option<NodeId>,
    /// §14 step 13 (§11.3): keyed by the overlay root's own `NodeId` --
    /// metadata only, never the node itself, which already lives in
    /// `nodes` like any other.
    overlays: HashMap<NodeId, OverlayMeta>,
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
            overlays: HashMap::new(),
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
        // A detached node is no longer reachable from any root, the
        // same "can't stay meaningfully focused" reasoning `remove`
        // already applies -- if it's reattached later, its own
        // eventual re-focus is whatever caller reattached it decides,
        // not a stale pointer surviving from before.
        if self.focused == Some(child) {
            self.focused = None;
        }
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

    /// `id`'s own on-screen position, accumulated all the way up its
    /// `parent` chain to whichever root `compute_layout` was last called
    /// on (§14 step 13's own real need: `open_overlay` positions an
    /// overlay relative to its anchor's *absolute* bounds, not the
    /// anchor's own parent-relative `Layout::location`). A genuinely new
    /// capability, not previously exposed as a standalone query --
    /// `build_tree_scene`/`build_access_update` only ever compute this
    /// *inline*, during their own full-tree walks, and each keeps its
    /// own separate accumulator; nothing before this let a caller ask
    /// "where is this one node, absolutely" without a full walk.
    pub fn absolute_position(&self, id: NodeId) -> (f64, f64) {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut current = id;
        loop {
            let layout = self.layout(current);
            x += f64::from(layout.location.x);
            y += f64::from(layout.location.y);
            let node = self
                .nodes
                .get(current)
                .expect("absolute_position: NodeId not found in this Tree");
            match node.parent {
                Some(parent) => current = parent,
                None => return (x, y),
            }
        }
    }

    /// Updates `id`'s `layout_style` and keeps `taffy`'s own internal
    /// copy in sync -- unlike `insert` (which hands `taffy` its copy
    /// once, at creation), mutating `Node::layout_style` directly
    /// afterward would silently desync the two; `TaffyTree::set_style`
    /// exists for exactly this "push a style update back in" case
    /// (verified directly in its own source before using it), so this
    /// method is the only place after `insert` that's allowed to touch
    /// `layout_style` -- doing so by hand anywhere else would reintroduce
    /// the exact divergence `insert`'s own doc comment says can't happen.
    pub fn set_layout_style(&mut self, id: NodeId, style: Style) {
        let taffy_node = *self
            .taffy_nodes
            .get(id)
            .expect("set_layout_style: NodeId not found in this Tree");
        self.taffy
            .set_style(taffy_node, style.clone())
            .expect("set_layout_style: taffy rejected the style update");
        self.nodes
            .get_mut(id)
            .expect("set_layout_style: NodeId not found in this Tree")
            .layout_style = style;
    }

    /// §14 step 13 (§11.3): positions `content` with `Position::
    /// Absolute`, its `inset` computed from `anchor`'s current absolute
    /// bounds (so it appears anchored just below-left of `anchor` --
    /// the real "one dropdown menu" placement this step's own test
    /// proves, not an arbitrary choice), then appends it to `root`'s
    /// `children` -- paint order is children-list order (§6), so an
    /// appended overlay paints on top with no separate z-order concept,
    /// exactly per §11.3's own claim. `root`/`anchor` must already have
    /// a computed `Layout` (call `compute_layout` at least once first);
    /// the caller must call `compute_layout` again afterward for
    /// `content`'s own new position/size to resolve.
    pub fn open_overlay(
        &mut self,
        root: NodeId,
        anchor: NodeId,
        content: NodeId,
        meta: OverlayMeta,
    ) {
        let (anchor_x, anchor_y) = self.absolute_position(anchor);
        let anchor_height = f64::from(self.layout(anchor).size.height);

        let mut style = self
            .get(content)
            .expect("open_overlay: content NodeId not found in this Tree")
            .layout_style
            .clone();
        style.position = Position::Absolute;
        style.inset = TaffyRect {
            left: length(anchor_x),
            top: length(anchor_y + anchor_height),
            right: auto(),
            bottom: auto(),
        };
        self.set_layout_style(content, style);

        self.add_child(root, content);
        self.overlays.insert(content, meta);
    }

    /// Closes an overlay opened via `open_overlay`: removes its whole
    /// subtree from the `Tree` (reusing step 12's real recursive
    /// `Tree::remove`, not a second removal path) and drops its
    /// metadata. Returns `true` if `id` was a real, currently-open
    /// overlay.
    pub fn close_overlay(&mut self, id: NodeId) -> bool {
        let had_overlay = self.overlays.remove(&id).is_some();
        let removed = self.remove(id);
        had_overlay && removed
    }

    /// The metadata for a currently-open overlay, if `id` is one --
    /// real bookkeeping a future dismiss-on-outside-click/Escape
    /// dispatch (§4/§11.10, not built yet) will read; proven stored
    /// correctly by this step's own test in the meantime.
    pub fn overlay_meta(&self, id: NodeId) -> Option<&OverlayMeta> {
        self.overlays.get(&id)
    }

    /// §14 step 15 (§11.4): "tabbed grouping... a plain index switch."
    /// Ensures exactly `zone.panels[zone.active_tab]` is attached as a
    /// child of `container`, detaching (via `Tree::detach` -- not
    /// deleting) any other panel in `zone.panels` currently attached.
    /// A detached panel isn't in anyone's `children` list, so
    /// `build_tree_scene`'s existing recursive walk already excludes it
    /// from paint with zero changes needed there, the same "reuse what
    /// already exists, prove it doesn't need touching" pattern step 13's
    /// overlay proof established for append-order.
    pub fn apply_active_tab(&mut self, container: NodeId, zone: &crate::dock::DockZone) {
        let active = zone.panels.get(zone.active_tab).copied();

        for &panel in &zone.panels {
            if Some(panel) != active
                && self
                    .get(container)
                    .is_some_and(|c| c.children.contains(&panel))
            {
                self.detach(container, panel);
            }
        }

        if let Some(active) = active
            && self
                .get(container)
                .is_some_and(|c| !c.children.contains(&active))
        {
            self.add_child(container, active);
        }
    }

    /// §14 step 15 (§11.5): moves a `NodeKind::Splitter` to `position`
    /// (0.0..=1.0 along its parent's own flex axis), resizing its two
    /// flanking siblings to match. A drag is a 1:1, instant mouse-follow,
    /// not a smoothly-eased transition -- so despite `SplitterState.
    /// position` being an `Animated<f64>`, this sets it via an instant
    /// (`Duration::ZERO`) `animate_to` and ticks it immediately, the
    /// same "an instant application needs an explicit tick to actually
    /// materialize" fix step 12 already found for bindings
    /// (`Animated::animate_to` alone never eagerly writes `current`).
    ///
    /// The splitter must be a direct child of some parent, sitting
    /// exactly between its two flanking siblings in that parent's own
    /// `children` order (`[..., left, splitter, right, ...]`) -- panics
    /// otherwise, the same "internal bookkeeping bug, not a runtime
    /// condition" reasoning `add_child` already uses for a malformed
    /// tree. Resizes along whichever axis matches the parent's own
    /// `flex_direction` (row -> width, column -> height), proportioning
    /// the two siblings' *current* combined extent by `position`.
    pub fn set_splitter_position(&mut self, id: NodeId, position: f64, now: Instant) {
        // Everything read here first, as owned values, before any
        // `&mut self` call below -- avoids holding a borrow of
        // `self.nodes` across `set_layout_style`'s own `&mut self`.
        let (parent, siblings) = {
            let node = self
                .nodes
                .get(id)
                .expect("set_splitter_position: NodeId not found in this Tree");
            assert!(
                matches!(node.kind, NodeKind::Splitter(_)),
                "set_splitter_position: {id:?} is not a NodeKind::Splitter"
            );
            let parent = node
                .parent
                .expect("set_splitter_position: a splitter must have a parent");
            let siblings = self
                .nodes
                .get(parent)
                .expect("set_splitter_position: parent NodeId not found in this Tree")
                .children
                .clone();
            (parent, siblings)
        };

        let index = siblings.iter().position(|&c| c == id).expect(
            "set_splitter_position: splitter isn't actually a child of its own recorded parent",
        );
        assert!(
            index > 0 && index + 1 < siblings.len(),
            "set_splitter_position: a splitter must sit between two real siblings, not at either end of its parent's children"
        );
        let left = siblings[index - 1];
        let right = siblings[index + 1];

        let is_row = matches!(
            self.nodes[parent].layout_style.flex_direction,
            taffy::FlexDirection::Row | taffy::FlexDirection::RowReverse
        );

        let (left_w, left_h) = {
            let l = self.layout(left);
            (f64::from(l.size.width), f64::from(l.size.height))
        };
        let (right_w, right_h) = {
            let r = self.layout(right);
            (f64::from(r.size.width), f64::from(r.size.height))
        };

        let position = position.clamp(0.0, 1.0);
        let total = if is_row {
            left_w + right_w
        } else {
            left_h + right_h
        };
        let left_extent = total * position;
        let right_extent = total - left_extent;

        let mut left_style = self.nodes[left].layout_style.clone();
        let mut right_style = self.nodes[right].layout_style.clone();
        if is_row {
            left_style.size.width = length(left_extent as f32);
            right_style.size.width = length(right_extent as f32);
        } else {
            left_style.size.height = length(left_extent as f32);
            right_style.size.height = length(right_extent as f32);
        }
        self.set_layout_style(left, left_style);
        self.set_layout_style(right, right_style);

        let NodeKind::Splitter(state) = &mut self.nodes[id].kind else {
            unreachable!("checked at the top of this function")
        };
        state
            .position
            .animate_to(position, Duration::ZERO, MotionCurve::Linear, now);
        state.position.tick(now);
    }

    /// §14 step 15 (§11.7): materializes/recycles a `NodeKind::
    /// VirtualList`'s real children to match exactly `visible` (the
    /// caller's own already-overscanned logical-index range) -- the
    /// concrete mechanism behind §11.7's own claim that a 100,000-row
    /// list never needs 100,000 real `Node`s.
    ///
    /// An index newly entering `visible` is built by calling
    /// `materialize(index)` for its `(NodeKind, Style, PaintProperties)`,
    /// then inserted and absolutely positioned by this method itself --
    /// `top: index * item_extent`, vertical-list only (the common list/
    /// data-grid case; a horizontal virtual list would need the same
    /// treatment along the other axis, not built since nothing here
    /// needs it yet). `Position::Absolute` here resolves against `list`
    /// itself, `list` being the item's own direct parent -- verified
    /// directly in `taffy`'s own `compute/flexbox.rs` at step 15 Stage A
    /// (no separate "positioned ancestor" walk needed, matching
    /// `open_overlay`'s own precedent). An index leaving `visible` is
    /// dropped via `Tree::remove` -- its real generational `NodeId`
    /// invalidation (`SlotMap`'s own behavior, §5) is what makes a stray
    /// reference to a scrolled-away item fail safely rather than
    /// silently reading whatever a reused slot now holds; nothing extra
    /// is needed for "recycling" beyond this ordinary remove+insert, per
    /// §11.7's own text.
    ///
    /// Panics if `list` isn't a `NodeKind::VirtualList`, the same
    /// "internal bookkeeping bug, not a runtime condition" reasoning
    /// `set_splitter_position` already uses for a malformed call.
    pub fn set_virtual_list_window(
        &mut self,
        list: NodeId,
        visible: std::ops::Range<usize>,
        mut materialize: impl FnMut(usize) -> (NodeKind, Style, PaintProperties),
    ) {
        let item_extent = match &self
            .nodes
            .get(list)
            .expect("set_virtual_list_window: NodeId not found in this Tree")
            .kind
        {
            NodeKind::VirtualList(state) => state.item_extent.value(),
            _ => panic!("set_virtual_list_window: {list:?} is not a NodeKind::VirtualList"),
        };

        let currently_materialized: Vec<(usize, NodeId)> = match &self.nodes[list].kind {
            NodeKind::VirtualList(state) => {
                state.materialized.iter().map(|(&i, &id)| (i, id)).collect()
            }
            _ => unreachable!("checked at the top of this function"),
        };

        for (idx, id) in &currently_materialized {
            if !visible.contains(idx) {
                self.remove(*id);
            }
        }
        if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
            state.materialized.retain(|idx, _| visible.contains(idx));
        }

        let already_materialized: std::collections::BTreeSet<usize> = match &self.nodes[list].kind {
            NodeKind::VirtualList(state) => state.materialized.keys().copied().collect(),
            _ => unreachable!("checked at the top of this function"),
        };

        for idx in visible.clone() {
            if already_materialized.contains(&idx) {
                continue;
            }
            let (kind, mut style, paint) = materialize(idx);
            style.position = Position::Absolute;
            style.inset = TaffyRect {
                left: length(0.0),
                top: length((idx as f64 * item_extent) as f32),
                right: auto(),
                bottom: auto(),
            };
            let id = self.insert(kind, style, paint);
            self.add_child(list, id);
            if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
                state.materialized.insert(idx, id);
            }
        }
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
    fn absolute_position_accumulates_through_nested_parents() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            padding: taffy::prelude::Rect {
                left: length(5.0),
                top: length(5.0),
                right: length(5.0),
                bottom: length(5.0),
            },
            size: Size {
                width: length(300.0),
                height: length(300.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let inner_style = Style {
            display: taffy::Display::Flex,
            flex_direction: FlexDirection::Row,
            size: Size {
                width: length(200.0),
                height: length(200.0),
            },
            ..Default::default()
        };
        let (_, _, inner_paint) = leaf(0.0, 0.0);
        let inner = tree.insert(NodeKind::Container, inner_style, inner_paint);
        tree.add_child(root, inner);

        // A spacer sibling before the real target, so the target's own
        // parent-relative x is nonzero -- proving real accumulation
        // through two levels, not a coincidence of both being at (0,0).
        let (k, s, p) = leaf(40.0, 40.0);
        let spacer = tree.insert(k, s, p);
        tree.add_child(inner, spacer);
        let (k, s, p) = leaf(40.0, 40.0);
        let target = tree.insert(k, s, p);
        tree.add_child(inner, target);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );

        // root padding (5,5) + inner's own location (0,0 within root's
        // content box) + spacer's width (40) = target's absolute x.
        let (x, y) = tree.absolute_position(target);
        assert_eq!(x, 5.0 + 40.0);
        assert_eq!(y, 5.0);
    }

    #[test]
    fn open_overlay_positions_below_its_anchor_and_appends_to_root() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(300.0),
                height: length(300.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (k, s, p) = leaf(80.0, 20.0); // the trigger button, e.g. a menu bar item
        let anchor = tree.insert(k, s, p);
        tree.add_child(root, anchor);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );
        let (anchor_x, anchor_y) = tree.absolute_position(anchor);
        assert_eq!((anchor_x, anchor_y), (0.0, 0.0));

        let (k, s, p) = leaf(120.0, 60.0); // the dropdown menu surface
        let menu = tree.insert(k, s, p);
        tree.open_overlay(
            root,
            anchor,
            menu,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: true,
                dismiss_on_escape: true,
            },
        );

        // A second compute_layout is required after open_overlay, per
        // its own doc comment, for the new absolutely-positioned child
        // to actually resolve.
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );

        let menu_layout = tree.layout(menu);
        assert_eq!(
            (menu_layout.location.x, menu_layout.location.y),
            (anchor_x as f32, anchor_y as f32 + 20.0),
            "the menu must land directly below the anchor's own bottom edge"
        );

        assert_eq!(
            tree.get(root).unwrap().children,
            vec![anchor, menu],
            "the overlay must be appended to root's children, after the anchor"
        );

        let meta = tree
            .overlay_meta(menu)
            .expect("the overlay's metadata must be stored, keyed by its own NodeId");
        assert_eq!(meta.anchor, anchor);
        assert!(meta.dismiss_on_outside_click);
        assert!(meta.dismiss_on_escape);
    }

    #[test]
    fn close_overlay_removes_the_node_and_its_metadata() {
        let mut tree = Tree::new();
        let root_style = Style {
            size: Size {
                width: length(300.0),
                height: length(300.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);
        let (k, s, p) = leaf(80.0, 20.0);
        let anchor = tree.insert(k, s, p);
        tree.add_child(root, anchor);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );

        let (k, s, p) = leaf(120.0, 60.0);
        let menu = tree.insert(k, s, p);
        tree.open_overlay(
            root,
            anchor,
            menu,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: true,
                dismiss_on_escape: true,
            },
        );

        assert!(tree.close_overlay(menu));
        assert!(
            tree.get(menu).is_none(),
            "the overlay node itself must be gone"
        );
        assert!(
            tree.overlay_meta(menu).is_none(),
            "its metadata must be gone too"
        );
        assert_eq!(
            tree.get(root).unwrap().children,
            vec![anchor],
            "root's children must no longer mention the closed overlay"
        );
        assert!(
            !tree.close_overlay(menu),
            "closing an already-closed overlay must report false"
        );
    }

    #[test]
    fn set_splitter_position_resizes_both_flanking_siblings() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            flex_direction: FlexDirection::Row,
            size: Size {
                width: length(210.0),
                height: length(50.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (k, s, p) = leaf(100.0, 50.0);
        let left = tree.insert(k, s, p);
        tree.add_child(root, left);

        let splitter = tree.insert(
            NodeKind::Splitter(crate::node::SplitterState {
                position: crate::animation::Animated::new(0.5),
            }),
            Style {
                size: Size {
                    width: length(10.0),
                    height: length(50.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, splitter);

        let (k, s, p) = leaf(100.0, 50.0);
        let right = tree.insert(k, s, p);
        tree.add_child(root, right);

        let available = Size {
            width: AvailableSpace::Definite(210.0),
            height: AvailableSpace::Definite(50.0),
        };
        tree.compute_layout(root, available);
        assert_eq!(tree.layout(left).size.width, 100.0);
        assert_eq!(tree.layout(right).size.width, 100.0);

        let now = Instant::now();
        tree.set_splitter_position(splitter, 0.75, now);
        tree.compute_layout(root, available);

        assert_eq!(
            tree.layout(left).size.width,
            150.0,
            "the left pane should now hold 75% of the 200px the two panes share"
        );
        assert_eq!(
            tree.layout(right).size.width,
            50.0,
            "the right pane should hold the remaining 25%"
        );
        assert_eq!(
            tree.layout(right).location.x,
            160.0,
            "the right pane must actually have moved -- 150 (left) + 10 (splitter)"
        );

        let NodeKind::Splitter(state) = &tree.get(splitter).unwrap().kind else {
            panic!("expected a Splitter node");
        };
        assert_eq!(
            state.position.current, 0.75,
            "the splitter's own position must reflect the new value immediately, \
             not just the two siblings' sizes"
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

    #[test]
    fn detach_removes_attachment_without_deleting_the_node() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let parent = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let child = tree.insert(k, s, p);
        tree.add_child(parent, child);

        tree.detach(parent, child);

        assert_eq!(
            tree.get(parent).unwrap().children,
            Vec::<NodeId>::new(),
            "the parent must no longer list the detached child"
        );
        assert_eq!(
            tree.get(child).unwrap().parent,
            None,
            "the detached child must no longer point back at its old parent"
        );
        assert!(
            tree.get(child).is_some(),
            "unlike remove(), the child node itself must still exist"
        );

        // A detached, reparented node must still work with every
        // ordinary Tree operation -- proving it's a real, live node,
        // not a half-removed one.
        let (k, s, p) = leaf(10.0, 10.0);
        let new_parent = tree.insert(k, s, p);
        tree.add_child(new_parent, child);
        assert_eq!(tree.get(child).unwrap().parent, Some(new_parent));
    }

    #[test]
    fn apply_active_tab_attaches_exactly_one_panel_at_a_time() {
        use crate::dock::DockZone;

        let mut tree = Tree::new();
        let (k, s, p) = leaf(100.0, 100.0);
        let container = tree.insert(k, s, p);

        let (k, s, p) = leaf(50.0, 50.0);
        let tab_a = tree.insert(k, s, p);
        let (k, s, p) = leaf(50.0, 50.0);
        let tab_b = tree.insert(k, s, p);
        let (k, s, p) = leaf(50.0, 50.0);
        let tab_c = tree.insert(k, s, p);

        let mut zone = DockZone::new(100.0);
        zone.panels = smallvec::smallvec![tab_a, tab_b, tab_c];
        zone.active_tab = 0;

        tree.apply_active_tab(container, &zone);
        assert_eq!(
            tree.get(container).unwrap().children,
            vec![tab_a],
            "only the active tab should be attached"
        );

        zone.active_tab = 2;
        tree.apply_active_tab(container, &zone);
        assert_eq!(
            tree.get(container).unwrap().children,
            vec![tab_c],
            "switching tabs must detach the old one and attach the new one"
        );
        assert!(
            tree.get(tab_a).is_some(),
            "a detached (not removed) tab must still exist, ready to be switched back to"
        );

        zone.active_tab = 0;
        tree.apply_active_tab(container, &zone);
        assert_eq!(
            tree.get(container).unwrap().children,
            vec![tab_a],
            "switching back to a previously-detached tab must reattach the same node"
        );
    }

    fn virtual_list_materializer(idx: usize) -> (NodeKind, Style, PaintProperties) {
        (
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(20.0),
                },
                ..Default::default()
            },
            // Encode the logical index into the color so a rendering
            // test elsewhere could tell items apart -- not exercised by
            // these engine-core unit tests, which only check `NodeId`/
            // position bookkeeping, not paint.
            PaintProperties::new(
                Color::from_rgba8((idx % 256) as u8, 0, 0, 255),
                0.0,
                0.0,
                1.0,
            ),
        )
    }

    #[test]
    fn set_virtual_list_window_only_ever_materializes_the_requested_range() {
        // §11.7's own claim: a 100,000-row list never needs 100,000 real
        // `Node`s -- this is the concrete, provable version of that,
        // not just an architectural assertion.
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(100_000, ItemExtent::Fixed(20.0))),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);

        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.materialized.keys().copied().collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4],
            "only the requested window should be materialized, not item_count"
        );
        assert_eq!(
            tree.get(list).unwrap().children.len(),
            5,
            "materialized nodes must be real children of the list, not tracked separately"
        );

        tree.compute_layout(
            list,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(400.0),
            },
        );
        for idx in 0..5 {
            let id = state_materialized_id(&tree, list, idx);
            let (_, y) = tree.absolute_position(id);
            assert_eq!(
                y,
                idx as f64 * 20.0,
                "item {idx} must be positioned at its own logical offset, index * item_extent"
            );
        }
    }

    fn state_materialized_id(tree: &Tree, list: NodeId, idx: usize) -> NodeId {
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        *state
            .materialized
            .get(&idx)
            .unwrap_or_else(|| panic!("index {idx} expected to be materialized"))
    }

    #[test]
    fn set_virtual_list_window_recycles_scrolled_out_indices_and_invalidates_their_old_node_ids() {
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(100_000, ItemExtent::Fixed(20.0))),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);
        let scrolled_away_ids: Vec<NodeId> = (0..3)
            .map(|idx| state_materialized_id(&tree, list, idx))
            .collect();

        // Scroll down: 0..5 -> 3..8. Indices 0,1,2 leave the window,
        // indices 5,6,7 newly enter it.
        tree.set_virtual_list_window(list, 3..8, virtual_list_materializer);

        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.materialized.keys().copied().collect::<Vec<_>>(),
            vec![3, 4, 5, 6, 7],
            "the window must shift to exactly the newly-visible range"
        );
        assert_eq!(
            tree.get(list).unwrap().children.len(),
            5,
            "still only 5 real children after scrolling, never growing with item_count"
        );

        // The real generational-safety claim (§5, §11.7): a stray
        // reference to a scrolled-away item's old `NodeId` must fail
        // safely, not silently resolve to whatever a reused slot now
        // holds.
        for old_id in scrolled_away_ids {
            assert!(
                tree.get(old_id).is_none(),
                "a scrolled-away item's old NodeId must no longer resolve to anything"
            );
        }
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
