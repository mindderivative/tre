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

use slotmap::{Key as SlotMapKey, KeyData, SecondaryMap, SlotMap};
use taffy::prelude::{
    AvailableSpace, Layout, Position, Rect as TaffyRect, Size, Style, TaffyTree, auto, length,
};

use crate::access::AccessNodeData;
use crate::animation::MotionCurve;
use crate::input::{DispatchOutcome, InputEvent, Key, PointerButton};
use crate::interaction::InteractionState;
#[cfg(test)]
use crate::node::{ItemExtent, VirtualListState};
use crate::node::{Node, NodeId, NodeKind, PaintProperties};
use crate::overlay::OverlayMeta;
use peniko::kurbo::{Affine, Point, Rect};

/// `Tree::move_focus`'s own direction -- Tab vs. Shift-Tab (§10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDirection {
    Next,
    Previous,
}

/// `Tree::dispatch`'s own MD3-value inputs, kept entirely out of
/// `engine-core` itself (§1 Locked Decisions: "keep `engine-core`
/// MD3-agnostic") -- a caller (eventually `engine-md3`'s own named
/// presets, the same `MotionCurve`/`engine_md3::motion::STANDARD` split
/// already used elsewhere) supplies the actual numbers; `Tree` only
/// knows how to animate toward whatever it's given. No `Default` impl,
/// deliberately -- a caller/test must state real values, not inherit an
/// implicit one that would itself be an unstated MD3 opinion.
pub struct InteractionConfig {
    pub hover_opacity: f64,
    pub hover_duration: Duration,
    pub focus_ring_opacity: f64,
    pub focus_ring_duration: Duration,
    pub ripple_radius: f64,
    pub ripple_opacity: f64,
    pub ripple_duration: Duration,
}

pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
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
    /// M4 Phase 3 (§11.5): the `NodeKind::Splitter` currently being
    /// dragged, if any -- set on a primary-button `PointerPressed` that
    /// hits a splitter, read by every subsequent `PointerMoved` until a
    /// primary-button `PointerReleased` clears it (wherever that
    /// happens, not conditioned on still hitting the splitter -- a real
    /// mouse-up always ends a drag, matching real OS drag semantics).
    dragging: Option<NodeId>,
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
            hovered: None,
            pressed: None,
            dragging: None,
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
        let (left, right, is_row, total) = self.splitter_geometry(id);

        let position = position.clamp(0.0, 1.0);
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
            unreachable!("checked by splitter_geometry")
        };
        state
            .position
            .animate_to(position, Duration::ZERO, MotionCurve::Linear, now);
        state.position.tick(now);
    }

    /// Shared by `set_splitter_position` and `update_drag` (M4 Phase 3,
    /// §11.5): resolves a splitter's own flanking-siblings geometry --
    /// which two real siblings it sits between, which axis its parent's
    /// `flex_direction` puts them on, and their current combined extent
    /// along that axis (which stays constant while dragging -- the two
    /// siblings only ever trade extent between each other). Panics under
    /// the same "internal bookkeeping bug, not a runtime condition"
    /// reasoning `add_child` already uses for a malformed tree.
    fn splitter_geometry(&self, id: NodeId) -> (NodeId, NodeId, bool, f64) {
        let node = self
            .nodes
            .get(id)
            .expect("splitter_geometry: NodeId not found in this Tree");
        assert!(
            matches!(node.kind, NodeKind::Splitter(_)),
            "splitter_geometry: {id:?} is not a NodeKind::Splitter"
        );
        let parent = node
            .parent
            .expect("splitter_geometry: a splitter must have a parent");
        let siblings = &self
            .nodes
            .get(parent)
            .expect("splitter_geometry: parent NodeId not found in this Tree")
            .children;

        let index = siblings.iter().position(|&c| c == id).expect(
            "splitter_geometry: splitter isn't actually a child of its own recorded parent",
        );
        assert!(
            index > 0 && index + 1 < siblings.len(),
            "splitter_geometry: a splitter must sit between two real siblings, not at either end of its parent's children"
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
        let total = if is_row {
            left_w + right_w
        } else {
            left_h + right_h
        };

        (left, right, is_row, total)
    }

    /// M4 Phase 3 (§11.5): "on drag, `position`'s tick handler mutates
    /// its two adjacent siblings' `layout_style`" made real -- converts
    /// `point`'s coordinate along the dragged splitter's own parent flex
    /// axis into a 0.0..=1.0 fraction (relative to the left sibling's
    /// own current absolute start and the flanking siblings' combined
    /// extent from `splitter_geometry`), then calls the *existing*
    /// `set_splitter_position` with it -- one real mechanism, reused,
    /// not reimplemented for the drag case. A no-op if `self.dragging`
    /// isn't currently set or the flanking siblings have zero combined
    /// extent (nothing to divide a fraction of).
    fn update_drag(&mut self, point: Point, now: Instant) {
        let Some(splitter) = self.dragging else {
            return;
        };
        let (left, _right, is_row, total) = self.splitter_geometry(splitter);
        if total <= 0.0 {
            return;
        }
        let (left_x, left_y) = self.absolute_position(left);
        let coord = if is_row { point.x } else { point.y };
        let start = if is_row { left_x } else { left_y };
        let fraction = ((coord - start) / total).clamp(0.0, 1.0);
        self.set_splitter_position(splitter, fraction, now);
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

    /// §11.10: pointer-to-node resolution, reverse paint order (topmost
    /// first -- the last child in `children`-list order paints on top,
    /// §6, so it's tested first here too; this is exactly what naturally
    /// reaches an appended overlay, §11.3, before ordinary background
    /// content, with zero special-casing).
    ///
    /// **Transform-aware since M5 Phase 2** (§11.9's own composed
    /// transform, landed M5 Phase 1): delegates to `hit_test_at`, which
    /// composes the same `parent * translate(layout.location) *
    /// own_transform` product `engine-render::paint_node` composes
    /// during paint -- if this formula and that one ever diverge,
    /// hit-testing and rendering will disagree about where a node is.
    /// Deliberately does NOT reuse `absolute_position` (pure
    /// translation, used by overlay placement/splitter-drag geometry/
    /// several `engine-py` synthetic-point entry points -- all
    /// explicitly out of this phase's scope, unchanged).
    ///
    /// **Still narrowed:** no `NodeKind::Canvas` custom hit-test
    /// override -- `Canvas` doesn't exist yet (M5 Phase 3 adds both).
    pub fn hit_test(&self, root: NodeId, point: Point) -> Option<NodeId> {
        self.hit_test_at(root, point, Affine::IDENTITY)
    }

    /// See `hit_test`'s own doc comment for the composition formula and
    /// why it has to match `paint_node`'s exactly. `parent_transform` is
    /// the caller's already-composed transform for `id`'s *parent*.
    fn hit_test_at(&self, id: NodeId, point: Point, parent_transform: Affine) -> Option<NodeId> {
        let node = self.nodes.get(id)?;
        let layout = self.layout(id);
        let composed = parent_transform
            * Affine::translate((f64::from(layout.location.x), f64::from(layout.location.y)))
            * node.paint.transform.current;

        for &child in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_at(child, point, composed) {
                return Some(hit);
            }
        }

        // Map the caller's canvas-space `point` into this node's local
        // space via the inverse of its composed transform, then test it
        // against the untransformed local layout box -- exactly the
        // local-space rect `paint_node` draws into under the identical
        // `composed` transform (M5 Phase 1).
        let local_point = composed.inverse() * point;
        let bounds = Rect::new(
            0.0,
            0.0,
            f64::from(layout.size.width),
            f64::from(layout.size.height),
        );
        bounds.contains(local_point).then_some(id)
    }

    /// The concrete fulfillment of §7.3's own text: "hover needs no new
    /// dispatch mechanism -- it falls out of hit-testing, run every
    /// pointer-move... entirely inside `engine-core`." Only animates a
    /// node that already opted into `InteractionState` (Design
    /// Principle 6: "only a node that opts in pays the cost") -- unlike
    /// `interaction_mut`, this never lazily creates one just because a
    /// node happened to be hovered. `hover_opacity`/`duration` are
    /// caller-supplied, not hardcoded: `engine-core` stays MD3-agnostic
    /// (§1 Locked Decisions) -- the real MD3 hover value is
    /// `engine-md3`'s to supply, the same generic/preset split
    /// `MotionCurve`/`engine_md3::motion::STANDARD` already uses.
    ///
    /// Returns the newly-hovered node (`None` if the pointer left every
    /// hit-testable node). A repeated call with the same result is a
    /// no-op -- it doesn't retrigger the same animation every frame.
    pub fn update_hover(
        &mut self,
        root: NodeId,
        point: Point,
        hover_opacity: f64,
        duration: Duration,
        now: Instant,
    ) -> Option<NodeId> {
        let hit = self.hit_test(root, point);
        if hit == self.hovered {
            return hit;
        }
        if let Some(old) = self.hovered
            && let Some(node) = self.nodes.get_mut(old)
            && let Some(state) = node.interaction.as_mut()
        {
            state
                .hover_opacity
                .animate_to(0.0, duration, MotionCurve::Linear, now);
        }
        if let Some(new) = hit
            && let Some(node) = self.nodes.get_mut(new)
            && let Some(state) = node.interaction.as_mut()
        {
            state
                .hover_opacity
                .animate_to(hover_opacity, duration, MotionCurve::Linear, now);
        }
        self.hovered = hit;
        hit
    }

    /// §10's own minimal keyboard focus model: Tab/Shift-Tab moves
    /// `focused` in tree order, wrapping at both ends. "Interactive"
    /// means `access.actions` is non-empty -- the real, already-existing
    /// signal (M3 step 7's own button test sets `Action::Click`), not a
    /// new field manufactured for this step. Animates `focus_ring` the
    /// same opt-in-only way `update_hover` animates `hover_opacity`, for
    /// the same Design Principle 6 reason.
    pub fn move_focus(
        &mut self,
        root: NodeId,
        direction: FocusDirection,
        focus_ring_opacity: f64,
        duration: Duration,
        now: Instant,
    ) {
        let mut order = Vec::new();
        self.collect_interactive(root, &mut order);

        let old = self.focused;
        let new = if order.is_empty() {
            None
        } else {
            let next_index = match old.and_then(|f| order.iter().position(|&n| n == f)) {
                Some(idx) => match direction {
                    FocusDirection::Next => (idx + 1) % order.len(),
                    FocusDirection::Previous => (idx + order.len() - 1) % order.len(),
                },
                None => match direction {
                    FocusDirection::Next => 0,
                    FocusDirection::Previous => order.len() - 1,
                },
            };
            Some(order[next_index])
        };
        self.transition_focus(new, focus_ring_opacity, duration, now);
    }

    /// M4 Phase 2 (§10): the direct-target counterpart to `move_focus`'s
    /// own tab-order computation -- what a platform-driven focus request
    /// actually needs ("a screen reader focusing a node directly"
    /// dispatches `accesskit::Action::Focus`, §10's own text), since
    /// jumping straight to a specific node is a different operation from
    /// "move to the next/previous interactive node in tree order."
    /// `node` need not itself have a non-empty `access.actions` --
    /// unlike `move_focus`'s own Tab-order, a platform accessibility
    /// client names the exact node it wants focused, the same way a
    /// mouse click names the exact node it hit regardless of that node's
    /// own `access.actions` (`Tree::activate`'s own doc comment makes
    /// the identical "don't invent a stricter rule for one path than the
    /// other" argument).
    pub fn set_focus_to(
        &mut self,
        node: NodeId,
        focus_ring_opacity: f64,
        duration: Duration,
        now: Instant,
    ) {
        if !self.nodes.contains_key(node) {
            return;
        }
        self.transition_focus(Some(node), focus_ring_opacity, duration, now);
    }

    /// Shared by `move_focus`/`set_focus_to`: animates the previously-
    /// focused node's `focus_ring` out and the newly-focused one's in,
    /// opt-in-only (Design Principle 6) exactly like `update_hover`
    /// animates `hover_opacity` -- factored out once a second real
    /// caller needed the identical transition logic, not duplicated.
    fn transition_focus(
        &mut self,
        new: Option<NodeId>,
        focus_ring_opacity: f64,
        duration: Duration,
        now: Instant,
    ) {
        let old = self.focused;
        self.focused = new;
        if old == new {
            return;
        }
        if let Some(old) = old
            && let Some(node) = self.nodes.get_mut(old)
            && let Some(state) = node.interaction.as_mut()
        {
            state
                .focus_ring
                .animate_to(0.0, duration, MotionCurve::Linear, now);
        }
        if let Some(new) = new
            && let Some(node) = self.nodes.get_mut(new)
            && let Some(state) = node.interaction.as_mut()
        {
            state
                .focus_ring
                .animate_to(focus_ring_opacity, duration, MotionCurve::Linear, now);
        }
    }

    /// M4 Phase 2 (§10): the direct, non-`InputEvent` counterpart to a
    /// mouse click's own `Activated` outcome -- what `accesskit::
    /// Action::Click` from a platform accessibility client actually
    /// needs, since there's no press/release pair or on-screen point
    /// involved, just a named target node. Deliberately does **not**
    /// check `access.actions` first -- the real mouse-click path
    /// (`dispatch`'s `PointerPressed`/`PointerReleased` handling)
    /// doesn't gate on it either (hit-testing alone decides *which*
    /// node; whether anything is actually registered to react is the
    /// caller's own lookup, e.g. `engine-py`'s `click_handlers`) --
    /// keeping both activation paths symmetric.
    pub fn activate(&self, node: NodeId) -> DispatchOutcome {
        if self.nodes.contains_key(node) {
            DispatchOutcome::Activated(node)
        } else {
            DispatchOutcome::None
        }
    }

    /// Pre-order walk collecting every node whose `access.actions` is
    /// non-empty, in tree order -- `move_focus`'s own Tab-order.
    fn collect_interactive(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        if !node.access.actions.is_empty() {
            out.push(id);
        }
        for &child in &node.children {
            self.collect_interactive(child, out);
        }
    }

    /// M4 Phase 1 step 1's one real top-level entry point: `engine-
    /// platform` translates a raw `winit` event into `InputEvent` and
    /// calls this. Every *mechanical* consequence (hover, focus
    /// movement, ripple-spawn-on-press, §2 Design Principle 6) happens
    /// here, inside `engine-core`; the one *meaning-dependent* outcome
    /// (`DispatchOutcome::Activated`) is left for the caller's own
    /// `AppHandler` impl to interpret -- `Tree` has no idea what
    /// activating a node means, only that it happened.
    ///
    /// Ripple stays the existing single-shot press+release
    /// approximation (`InteractionState::spawn_ripple`) for this step --
    /// upgrading to real two-phase press/hold/release timing is a real,
    /// separate scope (PLAN.md), not bundled into "make real events
    /// reach the tree at all."
    pub fn dispatch(
        &mut self,
        root: NodeId,
        event: InputEvent,
        config: &InteractionConfig,
        now: Instant,
    ) -> DispatchOutcome {
        match event {
            InputEvent::PointerMoved { position } => {
                // M4 Phase 6 (§7.3): captured before `update_hover` runs
                // -- it mutates `self.hovered` internally and returns
                // only the new value, so the *old* value has to be read
                // here to report a real transition afterward.
                let old_hovered = self.hovered;
                let new_hovered = self.update_hover(
                    root,
                    position,
                    config.hover_opacity,
                    config.hover_duration,
                    now,
                );
                // M4 Phase 3 (§11.5): live-follows-the-cursor while a
                // splitter drag is active -- a no-op otherwise.
                if self.dragging.is_some() {
                    self.update_drag(position, now);
                }
                if old_hovered != new_hovered {
                    DispatchOutcome::HoverChanged {
                        old: old_hovered,
                        new: new_hovered,
                    }
                } else {
                    DispatchOutcome::None
                }
            }
            InputEvent::PointerPressed { position, button } => {
                let hit = self.hit_test(root, position);
                if let Some(node) = hit {
                    self.pressed = Some((button, node));
                    // M4 Phase 3 (§11.5): pressing a splitter with the
                    // primary button starts a real drag -- reuses this
                    // same hit-test result, not a second one.
                    if button == PointerButton::Primary
                        && matches!(
                            self.nodes.get(node).map(|n| &n.kind),
                            Some(NodeKind::Splitter(_))
                        )
                    {
                        self.dragging = Some(node);
                    }
                    if let Some(state) = self.interaction_mut(node) {
                        state.spawn_ripple(
                            Point::new(position.x, position.y),
                            config.ripple_radius,
                            config.ripple_opacity,
                            config.ripple_duration,
                            now,
                        );
                    }
                } else {
                    self.pressed = None;
                }
                DispatchOutcome::None
            }
            InputEvent::PointerReleased { position, button } => {
                let hit = self.hit_test(root, position);
                let outcome = match self.pressed {
                    // M4 Phase 7 (§11.3): a same-node press/release pair
                    // means something different per button -- Primary
                    // activates (existing, unchanged), Secondary opens a
                    // context menu (its own real outcome now), Middle
                    // has no real meaning yet, matching Middle's own
                    // stated "no real MD3 desktop meaning" status
                    // elsewhere in this module.
                    Some((pressed_button, pressed_node))
                        if pressed_button == button && Some(pressed_node) == hit =>
                    {
                        match button {
                            PointerButton::Primary => DispatchOutcome::Activated(pressed_node),
                            PointerButton::Secondary => {
                                DispatchOutcome::SecondaryActivated(pressed_node)
                            }
                            PointerButton::Middle => DispatchOutcome::None,
                        }
                    }
                    _ => DispatchOutcome::None,
                };
                self.pressed = None;
                // M4 Phase 3 (§11.5): a real mouse-up always ends a
                // drag, wherever it happens -- not conditioned on still
                // hitting the splitter (the pointer can leave a thin
                // splitter's own hit region mid-drag and the drag must
                // still track it until release, matching real OS drag
                // semantics).
                if button == PointerButton::Primary {
                    self.dragging = None;
                }
                outcome
            }
            InputEvent::KeyPressed { key, shift } => match key {
                Key::Tab => {
                    let direction = if shift {
                        FocusDirection::Previous
                    } else {
                        FocusDirection::Next
                    };
                    self.move_focus(
                        root,
                        direction,
                        config.focus_ring_opacity,
                        config.focus_ring_duration,
                        now,
                    );
                    DispatchOutcome::None
                }
                Key::Enter | Key::Space => match self.focused {
                    Some(node) => DispatchOutcome::Activated(node),
                    None => DispatchOutcome::None,
                },
                // No overlay-dismiss consumer exists yet to route this
                // to (§11.3's own "dismissed on outside-click or
                // Escape" isn't wired) -- explicitly deferred, not
                // silently dropped.
                Key::Escape => DispatchOutcome::None,
            },
            InputEvent::KeyReleased { .. } => DispatchOutcome::None,
            // M4 Phase 8 (§11.7/§11.8 groundwork): a true no-op today,
            // deliberately -- wiring this to VirtualList's window
            // movement needs the still-open real scrollable-viewport
            // gap (clipping + scroll offset), not manufactured here
            // ahead of that need. Real translation from a genuine
            // winit::WindowEvent::MouseWheel already reaches this far
            // (engine-platform); this is where it stops for now.
            InputEvent::Scroll { .. } => DispatchOutcome::None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ScrollDelta;
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

    /// A 210x50 root, `Row`: left pane [0,100), splitter [100,110),
    /// right pane [110,210) -- the same real geometry `set_splitter_
    /// position_resizes_both_flanking_siblings` already uses, factored
    /// out once `dispatch`'s own drag tests needed it too.
    fn splitter_scene() -> (Tree, NodeId, NodeId, NodeId, NodeId, Size<AvailableSpace>) {
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
        (tree, root, left, splitter, right, available)
    }

    #[test]
    fn dispatch_drag_on_a_splitter_resizes_flanking_siblings_live_as_the_pointer_moves() {
        let (mut tree, root, left, _splitter, right, available) = splitter_scene();
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        // Press on the splitter itself (x=105, inside [100,110)).
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(105.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );

        // Drag to x=130: fraction = 130/200 = 0.65 of the shared 200px.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(130.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        assert_eq!(
            tree.layout(left).size.width,
            130.0,
            "the left pane must live-follow the cursor to its first drag position"
        );

        // Keep dragging, to x=160: fraction = 160/200 = 0.8 -- proves
        // this isn't a one-shot snap, the pane keeps following.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(160.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        assert_eq!(
            tree.layout(left).size.width,
            160.0,
            "the left pane must keep following a second drag position, not just the first"
        );
        assert_eq!(tree.layout(right).size.width, 40.0);
    }

    #[test]
    fn dispatch_release_ends_the_drag_so_further_pointer_moves_dont_resize() {
        let (mut tree, root, left, _splitter, _right, available) = splitter_scene();
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(105.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(130.0, 25.0),
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(130.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        let width_after_release = tree.layout(left).size.width;

        // A further pointer move, with no press held, must not keep
        // resizing the pane -- the drag genuinely ended at release.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(180.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        assert_eq!(
            tree.layout(left).size.width,
            width_after_release,
            "a pointer move after release must not still be tracked as a drag"
        );
    }

    #[test]
    fn dispatch_pressing_a_non_splitter_node_never_starts_a_drag() {
        let (mut tree, root, left, _splitter, _right, available) = splitter_scene();
        let original_left_width = tree.layout(left).size.width;
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        // Press and drag starting on the left pane itself, not the
        // splitter -- must never move the splitter it happens to share
        // a parent with.
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(50.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(180.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        assert_eq!(
            tree.layout(left).size.width,
            original_left_width,
            "dragging from a plain node must never move an unrelated splitter"
        );
    }

    #[test]
    fn dispatch_a_non_primary_button_press_on_a_splitter_never_starts_a_drag() {
        let (mut tree, root, left, _splitter, _right, available) = splitter_scene();
        let original_left_width = tree.layout(left).size.width;
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(105.0, 25.0),
                button: PointerButton::Secondary,
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(180.0, 25.0),
            },
            &config,
            now,
        );
        tree.compute_layout(root, available);
        assert_eq!(
            tree.layout(left).size.width,
            original_left_width,
            "a non-primary-button press on a splitter must never start a drag"
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

    /// A 100x100 root with two overlapping 60x60 children at the same
    /// position -- `second` is added later, so it's `first`'s topmost
    /// sibling per §6's own children-list-order-is-paint-order rule, and
    /// `hit_test` must pick it.
    fn overlapping_siblings() -> (Tree, NodeId, NodeId, NodeId) {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let zero_inset = TaffyRect {
            left: length(0.0),
            top: length(0.0),
            right: auto(),
            bottom: auto(),
        };

        let (k, s, p) = leaf(60.0, 60.0);
        let mut absolute = s.clone();
        absolute.position = Position::Absolute;
        absolute.inset = zero_inset;
        let first = tree.insert(k, absolute, p);
        tree.add_child(root, first);

        let (k, s, p) = leaf(60.0, 60.0);
        let mut absolute = s.clone();
        absolute.position = Position::Absolute;
        absolute.inset = zero_inset;
        let second = tree.insert(k, absolute, p);
        tree.add_child(root, second);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );
        (tree, root, first, second)
    }

    #[test]
    fn hit_test_reaches_the_topmost_of_two_overlapping_siblings() {
        let (tree, root, _first, second) = overlapping_siblings();
        let hit = tree.hit_test(root, Point::new(30.0, 30.0));
        assert_eq!(
            hit,
            Some(second),
            "the later (topmost-painted) sibling must win a point both overlap"
        );
    }

    #[test]
    fn hit_test_misses_entirely_outside_every_nodes_bounds() {
        let (tree, root, _first, _second) = overlapping_siblings();
        // root itself spans exactly [0,100)x[0,100) (it's a real,
        // hit-testable node too, by design -- every node is a valid hit
        // target by its own bounds, matching §11.10's own text), so the
        // only genuinely-outside point is beyond root's own extent.
        assert_eq!(tree.hit_test(root, Point::new(150.0, 150.0)), None);
    }

    #[test]
    fn hit_test_reaches_an_appended_overlay_over_background_content() {
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

        // Background content covering the whole canvas.
        let (k, s, p) = leaf(300.0, 300.0);
        let background = tree.insert(k, s, p);
        tree.add_child(root, background);

        // A small anchor button sitting on top of the background, at
        // the canvas's own top-left -- an ordinary toolbar-button shape,
        // not something that itself covers the whole canvas (unlike
        // `background`), so `open_overlay`'s own "below the anchor's
        // bottom edge" placement lands the menu somewhere still on
        // screen and still overlapping `background`.
        let zero_inset = TaffyRect {
            left: length(0.0),
            top: length(0.0),
            right: auto(),
            bottom: auto(),
        };
        let (k, s, p) = leaf(80.0, 20.0);
        let mut anchor_style = s;
        anchor_style.position = Position::Absolute;
        anchor_style.inset = zero_inset;
        let anchor = tree.insert(k, anchor_style, p);
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
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );

        // (10, 25) sits inside both the full-canvas background AND the
        // overlay menu (positioned just below the anchor button's
        // bottom edge, at y=20, per `open_overlay`) -- the point this
        // test exists to prove: the overlay wins, not the background
        // it's stacked above.
        let hit = tree.hit_test(root, Point::new(10.0, 25.0));
        assert_eq!(
            hit,
            Some(menu),
            "an appended overlay must win hit-testing over the background it covers, \
             the same append-order-is-paint-order convention step 13 already proved for paint"
        );
    }

    /// M5 Phase 2 (§11.10): a real, non-identity ancestor `transform`
    /// must shift where its child is actually hit -- the same claim
    /// M5 Phase 1's `engine-render` pixel test proved for painting, one
    /// layer down. Uses a pure-translate transform, not scale, so the
    /// expected hit point is exact integer arithmetic, not an
    /// approximation.
    #[test]
    fn hit_test_follows_an_ancestor_translate_transform() {
        let mut tree = Tree::new();
        let root_style = Style {
            size: Size {
                width: length(200.0),
                height: length(200.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (_, camera_style, camera_paint) = leaf(200.0, 200.0);
        let camera = tree.insert(NodeKind::Container, camera_style, camera_paint);
        tree.add_child(root, camera);

        let (k, s, p) = leaf(50.0, 50.0);
        let chip = tree.insert(k, s, p);
        tree.add_child(camera, chip);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(200.0),
            },
        );

        // Before any transform: `chip` occupies local/canvas (0,0)-(50,50)
        // (default block layout places the sole child at its parent's
        // origin) -- (25,25) hits it. `camera` itself is a real,
        // full-canvas (200x200) hit-testable node too (every node is a
        // valid hit target by its own bounds, matching the existing
        // `hit_test_misses_entirely_outside_every_nodes_bounds` test's
        // own comment) -- (150,150) misses `chip` but still lands on
        // `camera`'s own bounds, not `None`.
        assert_eq!(tree.hit_test(root, Point::new(25.0, 25.0)), Some(chip));
        assert_eq!(tree.hit_test(root, Point::new(150.0, 150.0)), Some(camera));

        // `camera`'s own transform translates by (100, 100) -- this
        // moves `camera` itself as well as `chip` (§11.9's own "exactly
        // like nested <g transform> in SVG": a node's own transform
        // applies to itself, not just its descendants, proven at the
        // paint level by M5 Phase 1). `chip` now occupies canvas
        // (100,100)-(150,150); `camera`'s own footprint is now entirely
        // off the original (0,0)-(200,200) canvas on its near edges.
        tree.get_mut(camera).unwrap().paint.transform.current = Affine::translate((100.0, 100.0));

        assert_eq!(
            tree.hit_test(root, Point::new(25.0, 25.0)),
            Some(root),
            "chip's and camera's old, untransformed footprint must no longer hit either of \
             them -- the point now falls through to root's own untransformed full-canvas \
             bounds, the real background beneath"
        );
        assert_eq!(
            tree.hit_test(root, Point::new(125.0, 125.0)),
            Some(chip),
            "chip's new, transformed position must hit it"
        );
    }

    /// The scale half of the same claim: a non-uniform composed
    /// transform (translate * scale) must also grow/shrink the
    /// hit-testable area, not just move it.
    #[test]
    fn hit_test_follows_an_ancestor_scale_and_translate_transform() {
        let mut tree = Tree::new();
        let root_style = Style {
            size: Size {
                width: length(200.0),
                height: length(200.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (_, camera_style, camera_paint) = leaf(200.0, 200.0);
        let camera = tree.insert(NodeKind::Container, camera_style, camera_paint);
        tree.add_child(root, camera);

        let (k, s, p) = leaf(40.0, 40.0);
        let chip = tree.insert(k, s, p);
        tree.add_child(camera, chip);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(200.0),
            },
        );

        // scale(2.0) about the origin, then translate by (20, 20):
        // chip's local (0,0)-(40,40) box maps to canvas (20,20)-(100,100).
        // This same transform also moves `camera`'s own 200x200 bounds
        // to canvas (20,20)-(420,420) -- larger and shifted, not the
        // original (0,0)-(200,200) -- so a point that misses both now
        // falls through to `root`'s own untransformed full-canvas
        // bounds, not `None` (mirroring the translate-only test above).
        tree.get_mut(camera).unwrap().paint.transform.current =
            Affine::translate((20.0, 20.0)) * Affine::scale(2.0);

        assert_eq!(
            tree.hit_test(root, Point::new(60.0, 60.0)),
            Some(chip),
            "a point inside the scaled-up transformed box must hit chip"
        );
        assert_eq!(
            tree.hit_test(root, Point::new(10.0, 10.0)),
            Some(root),
            "a point outside both chip's and camera's new transformed footprint (though \
             inside their old untransformed one) must fall through to root's own \
             untransformed bounds, not silently miss"
        );
    }

    #[test]
    fn update_hover_only_animates_nodes_that_already_opted_into_interaction_state() {
        use std::time::Duration;

        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (k, s, p) = leaf(50.0, 50.0);
        let opted_in = tree.insert(k, s, p);
        tree.add_child(root, opted_in);
        tree.interaction_mut(opted_in); // opts in, per `InteractionState::new`'s own 0.0 default

        let (k, s, p) = leaf(50.0, 50.0);
        let never_opted_in = tree.insert(k, s, p);
        tree.add_child(root, never_opted_in);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(50.0),
            },
        );

        let now = Instant::now();
        tree.update_hover(
            root,
            Point::new(25.0, 25.0),
            0.08,
            Duration::from_millis(100),
            now,
        );
        let hover_target = tree
            .get(opted_in)
            .unwrap()
            .interaction
            .as_ref()
            .unwrap()
            .hover_opacity
            .active
            .as_ref()
            .map(|a| a.to);
        assert_eq!(
            hover_target,
            Some(0.08),
            "the opted-in hovered node must have a real hover animation registered toward 0.08"
        );

        tree.update_hover(
            root,
            Point::new(75.0, 25.0),
            0.08,
            Duration::from_millis(100),
            now,
        );
        assert!(
            tree.get(never_opted_in).unwrap().interaction.is_none(),
            "hovering a node that never opted into InteractionState must not create one -- \
             Design Principle 6, only a node that opts in pays the cost"
        );
    }

    #[test]
    fn update_hover_fades_the_old_node_out_and_the_new_one_in_on_a_real_change() {
        use std::time::Duration;

        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (k, s, p) = leaf(50.0, 50.0);
        let a = tree.insert(k, s, p);
        tree.add_child(root, a);
        tree.interaction_mut(a);

        let (k, s, p) = leaf(50.0, 50.0);
        let b = tree.insert(k, s, p);
        tree.add_child(root, b);
        tree.interaction_mut(b);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(50.0),
            },
        );

        let start = Instant::now();
        let hover = tree.update_hover(
            root,
            Point::new(25.0, 25.0),
            0.08,
            Duration::from_millis(100),
            start,
        );
        assert_eq!(hover, Some(a));
        tree.tick_all(start + Duration::from_millis(100));
        let a_hover_after_settling = tree
            .get(a)
            .unwrap()
            .interaction
            .as_ref()
            .unwrap()
            .hover_opacity
            .current;
        assert!(
            (a_hover_after_settling - 0.08).abs() < 0.001,
            "A must have settled at the real hover target, got {a_hover_after_settling}"
        );

        let now = start + Duration::from_millis(200);
        let hover = tree.update_hover(
            root,
            Point::new(75.0, 25.0),
            0.08,
            Duration::from_millis(100),
            now,
        );
        assert_eq!(hover, Some(b));
        tree.tick_all(now + Duration::from_millis(100));

        let a_hover = tree
            .get(a)
            .unwrap()
            .interaction
            .as_ref()
            .unwrap()
            .hover_opacity
            .current;
        let b_hover = tree
            .get(b)
            .unwrap()
            .interaction
            .as_ref()
            .unwrap()
            .hover_opacity
            .current;
        assert!(
            a_hover.abs() < 0.001,
            "A must have faded back out once the pointer left it, got {a_hover}"
        );
        assert!(
            (b_hover - 0.08).abs() < 0.001,
            "B must have faded in to the real hover target, got {b_hover}"
        );
    }

    #[test]
    fn move_focus_cycles_only_through_interactive_nodes_in_tree_order_wrapping_at_both_ends() {
        use std::time::Duration;

        use crate::access::{AccessNodeData, Action, Role};

        let mut tree = Tree::new();
        let (k, s, p) = leaf(0.0, 0.0);
        let root = tree.insert(k, s, p);

        let (k, s, p) = leaf(10.0, 10.0);
        let button_a = tree.insert(k, s, p);
        tree.set_access(
            button_a,
            AccessNodeData::new(Role::Button).with_action(Action::Click),
        );
        tree.add_child(root, button_a);

        // A plain, non-interactive node in between -- must be skipped
        // entirely by Tab, not just left unfocused.
        let (k, s, p) = leaf(10.0, 10.0);
        let decoration = tree.insert(k, s, p);
        tree.add_child(root, decoration);

        let (k, s, p) = leaf(10.0, 10.0);
        let button_b = tree.insert(k, s, p);
        tree.set_access(
            button_b,
            AccessNodeData::new(Role::Button).with_action(Action::Click),
        );
        tree.add_child(root, button_b);

        assert_eq!(tree.focused(), None);
        let now = Instant::now();
        let duration = Duration::from_millis(100);

        tree.move_focus(root, FocusDirection::Next, 1.0, duration, now);
        assert_eq!(
            tree.focused(),
            Some(button_a),
            "Tab from nothing focused lands on the first interactive node"
        );

        tree.move_focus(root, FocusDirection::Next, 1.0, duration, now);
        assert_eq!(
            tree.focused(),
            Some(button_b),
            "Tab skips the non-interactive decoration node entirely"
        );

        tree.move_focus(root, FocusDirection::Next, 1.0, duration, now);
        assert_eq!(
            tree.focused(),
            Some(button_a),
            "Tab wraps back to the first interactive node at the end"
        );

        tree.move_focus(root, FocusDirection::Previous, 1.0, duration, now);
        assert_eq!(
            tree.focused(),
            Some(button_b),
            "Shift-Tab wraps backward past the first node to the last"
        );

        assert!(
            tree.get(decoration).unwrap().interaction.is_none(),
            "the non-interactive node must never be touched by focus movement at all"
        );
    }

    #[test]
    fn dispatch_activates_only_a_same_node_primary_press_and_release_pair() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (k, s, p) = leaf(50.0, 50.0);
        let a = tree.insert(k, s, p);
        tree.add_child(root, a);
        let (k, s, p) = leaf(50.0, 50.0);
        let b = tree.insert(k, s, p);
        tree.add_child(root, b);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(50.0),
            },
        );

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        // Press and release over the same node (A) -- a real click.
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a press alone never activates"
        );
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::Activated(a),
            "press and release over the same node with the primary button must activate it"
        );

        // Press over A, release over B -- a drag-off, not a click.
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(75.0, 25.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "releasing over a different node than was pressed must not activate anything"
        );

        // M4 Phase 7 (§11.3): press and release over the same node with
        // the secondary button now produces its own real outcome --
        // this section used to assert `None` here with a comment
        // predicting "reserved for a future context-menu mechanism,"
        // which this phase is.
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Secondary,
            },
            &config,
            now,
        );
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Secondary,
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::SecondaryActivated(a),
            "a same-node secondary-button press/release pair must produce SecondaryActivated"
        );

        // Middle-button press/release still has no real meaning --
        // unlike Secondary (this phase), nothing in this codebase names
        // a real use for Middle yet.
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Middle,
            },
            &config,
            now,
        );
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(25.0, 25.0),
                button: PointerButton::Middle,
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a same-node middle-button press/release pair must still produce no real outcome"
        );

        // Enter/Space on the currently-focused node also activates it.
        tree.set_focused(Some(b));
        let outcome = tree.dispatch(
            root,
            InputEvent::KeyPressed {
                key: Key::Enter,
                shift: false,
            },
            &config,
            now,
        );
        assert_eq!(outcome, DispatchOutcome::Activated(b));
    }

    #[test]
    fn dispatch_reports_hover_changed_only_on_a_real_transition() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let (k, s, p) = leaf(50.0, 50.0);
        let a = tree.insert(k, s, p);
        tree.add_child(root, a);
        let (k, s, p) = leaf(50.0, 50.0);
        let b = tree.insert(k, s, p);
        tree.add_child(root, b);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(50.0),
            },
        );

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        // Moving onto A for the first time: None -> Some(a).
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(25.0, 25.0),
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::HoverChanged {
                old: None,
                new: Some(a)
            },
            "hitting a node for the first time must report a real transition"
        );

        // Moving again within A: no transition, no outcome.
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(30.0, 30.0),
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a repeated move within the same already-hovered node must not report a transition"
        );

        // Moving from A to B: Some(a) -> Some(b).
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(75.0, 25.0),
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::HoverChanged {
                old: Some(a),
                new: Some(b)
            },
            "moving directly from one hovered node to another must report both halves"
        );

        // Moving off every node: Some(b) -> None.
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(500.0, 500.0),
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::HoverChanged {
                old: Some(b),
                new: None
            },
            "leaving every hit-testable node must still report the exit half"
        );
    }

    #[test]
    fn dispatch_scroll_is_a_true_no_op() {
        let mut tree = Tree::new();
        let root_style = Style {
            display: taffy::Display::Flex,
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);
        let (k, s, p) = leaf(50.0, 50.0);
        let a = tree.insert(k, s, p);
        tree.add_child(root, a);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(50.0),
            },
        );

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let now = Instant::now();

        // M4 Phase 8 (§11.7/§11.8 groundwork): real translation reaches
        // Tree::dispatch, but a Scroll event must be a genuine no-op --
        // no outcome, and no side effect on any other tracked state --
        // matching this phase's own explicit "plumbing only" scope.
        let before_hovered = tree.hovered;
        let outcome = tree.dispatch(
            root,
            InputEvent::Scroll {
                delta: ScrollDelta::Lines(0.0, 3.0),
                position: Point::new(25.0, 25.0),
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a Scroll event must produce no real outcome yet"
        );
        assert_eq!(
            tree.hovered, before_hovered,
            "a Scroll event must not touch hover state as a side effect"
        );

        let outcome = tree.dispatch(
            root,
            InputEvent::Scroll {
                delta: ScrollDelta::Pixels(0.0, -40.0),
                position: Point::new(25.0, 25.0),
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a pixel-delta Scroll event must also produce no real outcome yet"
        );
    }

    #[test]
    fn access_id_round_trips_through_to_access_id_and_from_access_id() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let node = tree.insert(k, s, p);

        let access_id = to_access_id(node);
        let round_tripped = from_access_id(access_id);
        assert_eq!(
            round_tripped, node,
            "KeyData::as_ffi/from_ffi's own documented guarantee: round-tripping \
             through the opaque accesskit::NodeId must return an equal key"
        );
    }

    #[test]
    fn from_access_id_on_a_stale_or_foreign_id_fails_safely_not_silently() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let node = tree.insert(k, s, p);
        let access_id = to_access_id(node);

        tree.remove(node);

        // The real generational-safety claim (§5), applied to an
        // accessibility-client-supplied id: a stale accesskit::NodeId
        // for a since-removed node must not resolve to whatever now
        // occupies that slot.
        let stale = from_access_id(access_id);
        assert!(
            tree.get(stale).is_none(),
            "a stale accesskit::NodeId must round-trip to a NodeId that fails \
             this Tree's own generation check, not one that silently resolves"
        );
    }

    #[test]
    fn activate_produces_activated_for_a_real_node_and_none_for_an_unknown_one() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let node = tree.insert(k, s, p);

        assert_eq!(tree.activate(node), DispatchOutcome::Activated(node));

        tree.remove(node);
        assert_eq!(
            tree.activate(node),
            DispatchOutcome::None,
            "activating a NodeId no longer in this Tree must be a safe no-op, \
             not a panic or a stale Activated outcome"
        );
    }

    #[test]
    fn set_focus_to_jumps_directly_to_the_named_node_and_animates_focus_ring() {
        use std::time::Duration;

        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let a = tree.insert(k, s, p);
        tree.interaction_mut(a);
        let (k, s, p) = leaf(10.0, 10.0);
        let b = tree.insert(k, s, p);
        tree.interaction_mut(b);

        let now = Instant::now();
        let duration = Duration::from_millis(100);

        // Unlike move_focus, set_focus_to doesn't need `a`/`b` to have
        // any access.actions at all -- it's a direct target, the same
        // way a mouse click names its target regardless of that node's
        // own access.actions.
        tree.set_focus_to(a, 1.0, duration, now);
        assert_eq!(tree.focused(), Some(a));
        tree.tick_all(now + duration);
        assert_eq!(
            tree.get(a)
                .unwrap()
                .interaction
                .as_ref()
                .unwrap()
                .focus_ring
                .current,
            1.0
        );

        tree.set_focus_to(b, 1.0, duration, now);
        assert_eq!(
            tree.focused(),
            Some(b),
            "set_focus_to must jump straight to the named node, not compute a \
             tab-order neighbor"
        );
        tree.tick_all(now + duration + duration);
        let a_ring = tree
            .get(a)
            .unwrap()
            .interaction
            .as_ref()
            .unwrap()
            .focus_ring
            .current;
        let b_ring = tree
            .get(b)
            .unwrap()
            .interaction
            .as_ref()
            .unwrap()
            .focus_ring
            .current;
        assert_eq!(
            a_ring, 0.0,
            "the previously-focused node's ring must animate out"
        );
        assert_eq!(b_ring, 1.0, "the newly-focused node's ring must animate in");
    }

    #[test]
    fn set_focus_to_an_unknown_node_is_a_safe_no_op() {
        use std::time::Duration;

        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let real = tree.insert(k, s, p);
        tree.set_focused(Some(real));
        let (k, s, p) = leaf(10.0, 10.0);
        let ghost = tree.insert(k, s, p);
        tree.remove(ghost);

        tree.set_focus_to(ghost, 1.0, Duration::from_millis(100), Instant::now());
        assert_eq!(
            tree.focused(),
            Some(real),
            "focusing an unknown NodeId must leave the real current focus untouched"
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
