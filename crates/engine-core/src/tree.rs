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
use crate::animation::{CompletionHandle, MotionCurve};
#[cfg(test)]
use crate::canvas::CanvasState;
use crate::canvas::{CustomHitTest, DrawCommand};
use crate::input::{DispatchOutcome, InputEvent, Key, PointerButton, ScrollDelta};
use crate::interaction::InteractionState;
#[cfg(test)]
use crate::node::{CheckboxState, ItemExtent, SliderState, VirtualListState};
use crate::node::{ImageState, Node, NodeId, NodeKind, PaintProperties, TextFieldState};
use crate::overlay::OverlayMeta;
#[cfg(test)]
use peniko::kurbo::BezPath;
use peniko::kurbo::{Affine, ParamCurveNearest, Point, Rect};

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
    /// M4 Phase 3 (§11.5), widened M14 Phase 2 (§7.3): the node
    /// currently being pointer-dragged, if any -- a `NodeKind::
    /// Splitter` or (M14 Phase 2) a `NodeKind::Slider`, set on a
    /// primary-button `PointerPressed` that hits one, read by every
    /// subsequent `PointerMoved` until a primary-button `PointerRelease
    /// d` clears it (wherever that happens, not conditioned on still
    /// hitting the node -- a real mouse-up always ends a drag, matching
    /// real OS drag semantics). `update_drag` is what actually branches
    /// on which real kind this is.
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

    /// M6 Phase 1 (§8): the checked counterpart to `add_child`, for the
    /// one caller that can't structurally guarantee it won't form a
    /// cycle -- Python's own `Node.add_child`. Every existing internal
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
        let mut current = Some(parent);
        while let Some(id) = current {
            if id == child {
                return false;
            }
            current = self.nodes.get(id).and_then(|n| n.parent);
        }

        if let Some(current_parent) = self.nodes[child].parent {
            self.detach(current_parent, child);
        }
        self.add_child(parent, child);
        true
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

    /// `id`'s own real, on-screen position -- transform-aware since M6
    /// Phase 4 (§8): composes the identical `parent *
    /// translate(layout.location) * own_transform` product `paint_node`/
    /// `hit_test_at` already compose (M5 Phase 1/2), not just a pure
    /// accumulated translation. §14 step 13's own real need: `open_overlay`
    /// positions an overlay relative to its anchor's *absolute* bounds,
    /// not the anchor's own parent-relative `Layout::location` --
    /// `splitter_geometry`'s drag math and every `engine-py` synthetic-
    /// point entry point (`Window`/`View`'s `.click()`/`.hover()`/
    /// `.right_click()`) have the exact same real need, confirmed via
    /// grep as this method's only real callers (a small, fully
    /// enumerated set, unlike `add_child`'s ~80 -- every one of them
    /// wants the transform-aware answer, so this rewrites the method in
    /// place rather than adding a parallel checked sibling the way M6
    /// Phase 1/M5 Phase 2 did for `add_child`/`hit_test`).
    ///
    /// An `Affine` only composes correctly root-to-node, the opposite
    /// order of the old bottom-up accumulation -- so this collects the
    /// chain from `id` up to the root first, then walks it in reverse.
    /// None of this method's real callers run once per node per frame
    /// (an overlay opens once per interaction, a drag reads this once
    /// per pointer move), so the extra composition cost here is not the
    /// same concern it would be for `paint_node`/`hit_test_at`'s own
    /// per-frame walks.
    pub fn absolute_position(&self, id: NodeId) -> (f64, f64) {
        let mut chain = vec![id];
        let mut current = id;
        while let Some(parent) = self
            .nodes
            .get(current)
            .expect("absolute_position: NodeId not found in this Tree")
            .parent
        {
            chain.push(parent);
            current = parent;
        }

        let mut composed = Affine::IDENTITY;
        for &node_id in chain.iter().rev() {
            let layout = self.layout(node_id);
            let node = self
                .nodes
                .get(node_id)
                .expect("absolute_position: NodeId not found in this Tree");
            composed = composed
                * Affine::translate((f64::from(layout.location.x), f64::from(layout.location.y)))
                * node.paint.transform.current;
        }

        let p = composed * Point::ORIGIN;
        (p.x, p.y)
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

    /// M10 Phase 3 (§11.4): `open_overlay`'s own missing "cover an
    /// arbitrary rect" counterpart -- `open_overlay` only ever places
    /// content anchor-relative-below (`dock.rs`'s own doc comment names
    /// this exact gap: a drop-zone highlight needs to cover a target
    /// zone's own real computed bounds, not sit below some anchor).
    /// Sets `Position::Absolute` with `inset.left`/`top` from `rect`'s
    /// own origin and an explicit `style.size` matching `rect`'s own
    /// width/height -- unlike `open_overlay`, which leaves `content`'s
    /// existing size untouched, this method always resizes `content` to
    /// exactly cover `rect`, since that's the whole point of a
    /// highlight. Deliberately does *not* attach `content` anywhere or
    /// touch `self.overlays` -- a rect-covering highlight has no anchor
    /// and no outside-click/Escape dismissal (`OverlayMeta` doesn't fit
    /// it); attach/detach lifecycle is the caller's own responsibility,
    /// driven by whatever real state (e.g. drag-in-progress) decides
    /// when it should be visible. `root`/whatever produced `rect` must
    /// already have a computed `Layout`; the caller must call `compute_
    /// layout` again afterward for `content`'s own new position/size to
    /// resolve, exactly like `open_overlay`.
    pub fn position_overlay_over(&mut self, content: NodeId, rect: Rect) {
        let mut style = self
            .get(content)
            .expect("position_overlay_over: content NodeId not found in this Tree")
            .layout_style
            .clone();
        style.position = Position::Absolute;
        style.inset = TaffyRect {
            left: length(rect.x0),
            top: length(rect.y0),
            right: auto(),
            bottom: auto(),
        };
        style.size = Size {
            width: length(rect.width()),
            height: length(rect.height()),
        };
        self.set_layout_style(content, style);
    }

    /// Closes an overlay opened via `open_overlay`: detaches its whole
    /// subtree from its own parent (the same real `Tree::detach`
    /// mechanism `Node.set_context_menu` already uses to keep content
    /// "alive, parentless, ready for `add_child` elsewhere later") and
    /// drops its metadata. Returns `true` if `id` was a real,
    /// currently-open overlay.
    ///
    /// **M10 Phase 1 (§11.3): real finding, corrected before this
    /// phase's own dismissal wiring shipped, not after.** Originally
    /// used `Tree::remove` (full, irreversible destruction) -- this
    /// genuinely broke the single most realistic real use of dismissal:
    /// right-click a context menu open, dismiss it (outside click or
    /// Escape), right-click the *same* anchor again. `Node.set_
    /// context_menu` registers one specific, app-owned content `NodeId`
    /// meant to be reopened repeatedly, not recreated per click --
    /// destroying it on the very first dismissal left `dispatch::
    /// open_context_menu`'s own stored `content` id dangling, panicking
    /// the next real reopen attempt (`open_overlay`'s own `self.get(
    /// content).expect(...)`). Detach, not destroy, is the same
    /// contract `set_context_menu` already committed to for exactly
    /// this reason -- a caller that genuinely wants an overlay's own
    /// content destroyed can still call `Tree::remove` on it directly
    /// afterward.
    pub fn close_overlay(&mut self, id: NodeId) -> bool {
        let had_overlay = self.overlays.remove(&id).is_some();
        if !had_overlay {
            return false;
        }
        if let Some(parent) = self.get(id).and_then(|node| node.parent) {
            self.detach(parent, id);
        }
        true
    }

    /// The metadata for a currently-open overlay, if `id` is one --
    /// real bookkeeping the real dismiss-on-outside-click/Escape
    /// dispatch (below, M10 Phase 1) reads, alongside `dispatch::
    /// open_context_menu`'s own re-open guard.
    pub fn overlay_meta(&self, id: NodeId) -> Option<&OverlayMeta> {
        self.overlays.get(&id)
    }

    /// M10 Phase 1 (§11.3): closes every currently-open overlay whose
    /// own `OverlayMeta.dismiss_on_outside_click` is true and whose
    /// whole visible subtree, *and* whose own anchor, don't contain
    /// `point` -- `hit_test` scoped to the overlay's own `content`
    /// `NodeId` is `None` exactly when the point is genuinely outside
    /// it (safe to call with a non-root `NodeId` here: every overlay's
    /// own `content` is always a direct child of the tree's own true
    /// root, which always has identity location/transform, `PLAN.md`).
    /// The anchor is excluded too -- a real, found-while-testing bug
    /// fix: a press that lands back on the anchor itself (e.g. right-
    /// clicking the same trigger a second time, `dispatch::open_
    /// context_menu`'s own re-open guard already handles that safely)
    /// must not be treated as an outside click, or the very press meant
    /// to interact with the anchor would dismiss its own overlay first
    /// and never reach `SecondaryActivated` at all. Returns whether
    /// anything was actually dismissed, so `dispatch`'s own
    /// `PointerPressed` arm knows whether to consume that press.
    fn dismiss_overlays_outside(&mut self, point: Point) -> bool {
        let to_dismiss: Vec<NodeId> = self
            .overlays
            .iter()
            .filter(|(content, meta)| {
                meta.dismiss_on_outside_click
                    && self.hit_test(**content, point).is_none()
                    && self.hit_test(meta.anchor, point).is_none()
            })
            .map(|(&content, _)| content)
            .collect();
        let dismissed_any = !to_dismiss.is_empty();
        for content in to_dismiss {
            self.close_overlay(content);
        }
        dismissed_any
    }

    /// M10 Phase 1 (§11.3): closes every currently-open overlay whose
    /// own `OverlayMeta.dismiss_on_escape` is true, unconditionally --
    /// `Key::Escape` always means "close it," no position check needed,
    /// unlike outside-click dismissal above.
    fn dismiss_escapable_overlays(&mut self) {
        let to_dismiss: Vec<NodeId> = self
            .overlays
            .iter()
            .filter(|(_, meta)| meta.dismiss_on_escape)
            .map(|(&content, _)| content)
            .collect();
        for content in to_dismiss {
            self.close_overlay(content);
        }
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
        // M9 Phase 1 (§5): kind-specific fields (`SplitterState.
        // position`, ticked manually here, outside `Tree::tick_all`'s
        // own real per-node walk since M8 Phase 2's own confirmed
        // finding) don't participate in the central completion queue --
        // a real, stated scope boundary, not silently dropped
        // functionality (nothing attaches `on_complete` to a splitter's
        // own position animation).
        state.position.tick(now, &mut Vec::new());
    }

    /// M14 Phase 2 (§5, §7.3): moves a `NodeKind::Slider`'s own real
    /// `thumb_position` to `position` (clamped `0.0..=1.0`) -- mirrors
    /// `set_splitter_position`'s own instant (`Duration::ZERO`)
    /// `animate_to` + immediate `tick` shape exactly (a drag is a 1:1
    /// mouse-follow, not a smoothly-eased transition), with no sibling-
    /// resize step: a slider doesn't resize anything else, only itself.
    /// Panics if `id` isn't a real `NodeKind::Slider` in this `Tree`,
    /// the same "internal bug, not a runtime condition" contract
    /// `set_splitter_position`/`scroll_virtual_list_by` already use.
    pub fn set_slider_position(&mut self, id: NodeId, position: f64, now: Instant) {
        let position = position.clamp(0.0, 1.0);
        let NodeKind::Slider(state) = &mut self.nodes[id].kind else {
            panic!("set_slider_position: {id:?} is not a NodeKind::Slider");
        };
        state
            .thumb_position
            .animate_to(position, Duration::ZERO, MotionCurve::Linear, now);
        state.thumb_position.tick(now, &mut Vec::new());
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
    ///
    /// M14 Phase 2 (§7.3): widened with a real `Slider` branch -- much
    /// simpler geometry (no flanking siblings; the node's own real
    /// absolute position/width *is* the whole track), horizontal-only
    /// for now (the same "not built since nothing here needs it yet"
    /// scope limit `set_virtual_list_window`'s own vertical-only
    /// restriction already established).
    fn update_drag(&mut self, point: Point, now: Instant) {
        let Some(dragging) = self.dragging else {
            return;
        };
        match &self.nodes[dragging].kind {
            NodeKind::Splitter(_) => self.update_splitter_drag(dragging, point, now),
            NodeKind::Slider(_) => self.update_slider_drag(dragging, point, now),
            _ => {}
        }
    }

    fn update_splitter_drag(&mut self, splitter: NodeId, point: Point, now: Instant) {
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

    /// M14 Phase 2 (§7.3): the slider's own real, live-follows-the-
    /// cursor drag math -- the node's own real absolute x and current
    /// computed width are the whole track, no flanking siblings
    /// involved. A no-op if the slider has zero real width (nothing to
    /// divide a fraction of, the same guard `update_splitter_drag`
    /// already has for zero combined sibling extent).
    fn update_slider_drag(&mut self, slider: NodeId, point: Point, now: Instant) {
        let (x, _y) = self.absolute_position(slider);
        let width = f64::from(self.layout(slider).size.width);
        if width <= 0.0 {
            return;
        }
        let fraction = ((point.x - x) / width).clamp(0.0, 1.0);
        self.set_slider_position(slider, fraction, now);
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
    /// `top: state.offset_of(idx)` (M12 Phase 1: `idx * item_extent` for
    /// `Fixed`, a real resolved cumulative offset for `Variable`),
    /// vertical-list only (the common list/data-grid case; a horizontal
    /// virtual list would need the same treatment along the other axis,
    /// not built since nothing here needs it yet). `Position::Absolute` here resolves against `list`
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
        match &self
            .nodes
            .get(list)
            .expect("set_virtual_list_window: NodeId not found in this Tree")
            .kind
        {
            NodeKind::VirtualList(_) => {}
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
            let top = match &self.nodes[list].kind {
                NodeKind::VirtualList(state) => state.offset_of(idx),
                _ => unreachable!("checked at the top of this function"),
            };
            style.position = Position::Absolute;
            style.inset = TaffyRect {
                left: length(0.0),
                top: length(top as f32),
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

    /// M8 Phase 3 (§11.7): moves `id`'s own real `scroll_offset` by
    /// `delta_y` real pixels, clamped to `[0.0, max_offset]` --
    /// `max_offset` is `id`'s own real content extent (`VirtualList
    /// State::total_extent`, M12 Phase 1: `item_extent * item_count`
    /// for `Fixed`, the real resolved sum for `Variable`) minus its own
    /// real, computed viewport height (`Tree::layout`), floored at
    /// `0.0` (a list whose content is shorter than its own viewport
    /// can't scroll at all, correctly).
    /// A positive `delta_y` increases the offset (content moves up,
    /// later items come into view) -- this crate's own chosen, stated
    /// convention (`PLAN.md`), not one `winit`'s own docs pin down.
    /// Exposed as its own real method, the same "a direct method
    /// `dispatch` reuses internally" shape `set_splitter_position`/
    /// `spawn_ripple` already use -- panics if `id` isn't a real
    /// `NodeKind::VirtualList` in this `Tree`, the same "internal bug,
    /// not a runtime condition" contract those methods use too.
    pub fn scroll_virtual_list_by(&mut self, id: NodeId, delta_y: f64) {
        let node = self
            .nodes
            .get(id)
            .expect("scroll_virtual_list_by: NodeId not found in this Tree");
        let NodeKind::VirtualList(state) = &node.kind else {
            panic!("scroll_virtual_list_by: {id:?} is not a NodeKind::VirtualList");
        };
        let content_extent = state.total_extent();
        let viewport_height = f64::from(self.layout(id).size.height);
        let max_offset = (content_extent - viewport_height).max(0.0);

        let NodeKind::VirtualList(state) = &mut self.nodes[id].kind else {
            unreachable!("checked above")
        };
        state.scroll_offset.current =
            (state.scroll_offset.current + delta_y).clamp(0.0, max_offset);
    }

    /// M12 Phase 1 (§11.7): the real way a caller supplies resolved
    /// cumulative offsets for a `Variable`-extent list -- `engine-core`
    /// itself never computes a cumulative sum from raw per-item heights
    /// (no size-hint callback lives here, §4); it only stores what it's
    /// given, mirroring `materialized`'s own "resolve once, cache"
    /// shape. `offsets`'s own key `item_count` (one past the last real
    /// item) is where the real total content extent belongs -- see
    /// `VirtualListState::offset_of`/`total_extent`'s own doc comments.
    /// Panics if `list` isn't a real `NodeKind::VirtualList` in this
    /// `Tree`, the same "internal bug, not a runtime condition"
    /// contract `scroll_virtual_list_by` already uses.
    pub fn set_virtual_list_resolved_offsets(
        &mut self,
        list: NodeId,
        offsets: impl IntoIterator<Item = (usize, f64)>,
    ) {
        let NodeKind::VirtualList(state) = &mut self
            .nodes
            .get_mut(list)
            .expect("set_virtual_list_resolved_offsets: NodeId not found in this Tree")
            .kind
        else {
            panic!("set_virtual_list_resolved_offsets: {list:?} is not a NodeKind::VirtualList");
        };
        state.resolved_offsets.extend(offsets);
    }

    /// The central tick's per-`Tree` entry point (§5): ticks every
    /// node's `PaintProperties` and (§14 step 9) its `InteractionState`
    /// if it has one, returning `true` if any is still mid-animation. A
    /// naive whole-tree walk, not the "active set only" scoped version
    /// §5 describes -- see `PaintProperties::tick` for why that scoping
    /// is deliberately deferred past this step.
    /// M9 Phase 1 (§5): also returns the real set of `CompletionHandle`s
    /// that finished on exactly this tick, across every node -- one
    /// shared `Vec` threaded through the whole walk (`PaintProperties::
    /// tick`/`InteractionState::tick`'s own `completed` parameter),
    /// not a per-node allocation. `engine-py`'s own per-frame render
    /// loop is what actually drains this and invokes a real Python
    /// callback for each one (M9 Phase 2) -- this method itself stays
    /// pyo3-agnostic, just plumbing the real data out.
    pub fn tick_all(&mut self, now: Instant) -> (bool, Vec<CompletionHandle>) {
        let mut any_active = false;
        let mut completed = Vec::new();
        for node in self.nodes.values_mut() {
            if node.paint.tick(now, &mut completed) {
                any_active = true;
            }
            if let Some(interaction) = &mut node.interaction
                && interaction.tick(now, &mut completed)
            {
                any_active = true;
            }
            // M14 Phase 1 (§7.3): `check_progress` is meant to animate
            // toward a real target once the app sets `checked` (unlike
            // `SplitterState.position`/`VirtualListState.scroll_offset`,
            // driven directly, never eased -- confirmed by direct read
            // this loop never touched kind-specific payloads before this
            // phase), so it needs the same central ticking `node.paint`
            // already gets, not a second, separate mechanism.
            if let NodeKind::Checkbox(state) = &mut node.kind
                && state.check_progress.tick(now, &mut completed)
            {
                any_active = true;
            }
            // M14 Phase 2 (§7.3): real finding while designing this --
            // `thumb_position` is *also* exposed to `Node.animate()`
            // (unlike `SplitterState.position`, never exposed there at
            // all), so a real, app-triggered eased move (not a drag)
            // needs this same central ticking, or a nonzero-duration
            // `animate()` call would set an active animation that never
            // progresses. The drag path itself already ticks manually
            // (`set_slider_position`'s own `Duration::ZERO` + immediate
            // tick), so this is a true no-op for that path -- `tick`
            // returns `false` immediately once `self.active` is `None`.
            if let NodeKind::Slider(state) = &mut node.kind
                && state.thumb_position.tick(now, &mut completed)
            {
                any_active = true;
            }
        }
        (any_active, completed)
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

    /// M7 Phase 3 (§7.1/§7.3): updates every already-opted-in node's
    /// `InteractionState::tint` to `tint` -- the real "apply a newly
    /// (re)resolved theme color to whatever's already on screen"
    /// mechanism both `engine-py::Window.set_theme` (a node that opted
    /// in before the theme was set) and real live theme switching (a
    /// node that was already themed, now needs the *other* scheme's
    /// color) share. A node that never opted into `InteractionState`
    /// (`interaction: None`) is left untouched, matching `interaction_
    /// mut`'s own "only a node that opts in pays the cost" contract --
    /// this never lazily creates one.
    pub fn set_all_interaction_tints(&mut self, tint: peniko::Color) {
        for node in self.nodes.values_mut() {
            if let Some(interaction) = node.interaction.as_mut() {
                interaction.tint = tint;
            }
        }
    }

    /// M20 Phase 1 (§7.1, §7.3): `set_all_interaction_tints`'s own real
    /// sibling for a genuinely different kind of field -- `mark_tint`/
    /// `track_tint` live directly on `CheckboxState`/`SliderState`
    /// (every real instance always has one), not on the optional
    /// `InteractionState` every node may or may not opt into. A
    /// separate method, not a widened `set_all_interaction_tints`,
    /// keeps that already-tested method's own real, documented
    /// behavior (touches only `node.interaction`) unchanged, matching
    /// this codebase's own "distinct real behaviors, distinct methods"
    /// precedent (`set_text_field_cursor`/`extend_text_field_
    /// selection`). No opt-in gate, unlike `set_all_interaction_
    /// tints`: every matching node unconditionally gets the real
    /// resolved color, since these fields aren't an optional
    /// capability to begin with.
    pub fn set_all_component_tints(&mut self, tint: peniko::Color) {
        for node in self.nodes.values_mut() {
            match &mut node.kind {
                NodeKind::Checkbox(state) => state.mark_tint = tint,
                NodeKind::Slider(state) => state.track_tint = tint,
                // M20 Phase 2 (§7.1, §7.3): `TextField`'s own real
                // sibling, closing the milestone's own real mechanism.
                NodeKind::TextField(state) => state.text_tint = tint,
                _ => {}
            }
        }
    }

    /// M5 Phase 3 (§11.10, §11.11): replaces a `NodeKind::Canvas`
    /// node's entire real content -- both what `paint_node` draws and
    /// what `hit_test_at` tests against. The one, ordinary (non-
    /// callback) `Tree` mutation `engine-py::Window.redraw_canvas`
    /// calls after invoking the app's Python draw callback exactly
    /// once and collecting its result -- see `canvas.rs`'s own module
    /// doc comment for why the callback itself never reaches this far.
    /// Returns `None` if `id` doesn't exist or isn't a `Canvas`.
    pub fn set_canvas_content(
        &mut self,
        id: NodeId,
        commands: Vec<DrawCommand>,
        hit_test: Option<CustomHitTest>,
    ) -> Option<()> {
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
    pub fn image_nodes(&self) -> impl Iterator<Item = (NodeId, &ImageState)> {
        self.nodes.iter().filter_map(|(id, node)| match &node.kind {
            NodeKind::Image(state) => Some((id, state)),
            _ => None,
        })
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
    /// **Since M5 Phase 3:** a `NodeKind::Canvas` with a `CustomHitTest`
    /// set (`Tree::set_canvas_content`) overrides the rect test below
    /// for that node only, per §11.10's own text; a `Canvas` with none
    /// (or any other `NodeKind`) keeps the ordinary rect test.
    pub fn hit_test(&self, root: NodeId, point: Point) -> Option<NodeId> {
        self.hit_test_at(root, point, Affine::IDENTITY)
            .map(|(id, _local_point)| id)
    }

    /// M18 Phase 1 (§8, §10, §11.9, §11.10): `hit_test`'s own sibling,
    /// additionally returning the click's own local-space point on the
    /// hit node -- needed by a caller (`TextRenderer::hit_test_position`)
    /// that has to turn a click into a byte offset within text painted
    /// at that node's own local origin, the same real transform-aware
    /// coordinate `hit_test_at` already computes internally for its own
    /// rect/custom-hit-test containment check but never exposed before
    /// this. Reuses `hit_test_at`'s own composition math verbatim, not a
    /// second way of computing it.
    pub fn hit_test_local(&self, root: NodeId, point: Point) -> Option<(NodeId, Point)> {
        self.hit_test_at(root, point, Affine::IDENTITY)
    }

    /// See `hit_test`'s own doc comment for the composition formula and
    /// why it has to match `paint_node`'s exactly. `parent_transform` is
    /// the caller's already-composed transform for `id`'s *parent*.
    fn hit_test_at(
        &self,
        id: NodeId,
        point: Point,
        parent_transform: Affine,
    ) -> Option<(NodeId, Point)> {
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
        // space via the inverse of its composed transform -- both the
        // rect default and any `CustomHitTest` below test against this
        // same local point, exactly the local-space coordinates
        // `paint_node` draws into under the identical `composed`
        // transform (M5 Phase 1).
        let local_point = composed.inverse() * point;

        let hit = match &node.kind {
            NodeKind::Canvas(state) => match &state.hit_test {
                Some(CustomHitTest::Circle { cx, cy, radius }) => {
                    (local_point - Point::new(*cx, *cy)).hypot() <= *radius
                }
                Some(CustomHitTest::Path { path, tolerance }) => {
                    path.segments()
                        .map(|seg| seg.nearest(local_point, 0.1).distance_sq)
                        .fold(f64::INFINITY, f64::min)
                        .sqrt()
                        <= *tolerance
                }
                None => rect_contains(layout, local_point),
            },
            _ => rect_contains(layout, local_point),
        };
        hit.then_some((id, local_point))
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

    /// M15 Phase 2 (§8, §10): real keyboard-driven `TextField` editing
    /// -- the `Tree`'s own real mutator (unlike `CheckboxState.
    /// checked`, a real keystroke is mechanical, not app-defined
    /// meaning, so `engine-core` is the one real owner here, mirroring
    /// `set_slider_position`'s own "engine-core owns the real
    /// mutation" shape). `content`/`cursor` stay on real UTF-8 char
    /// boundaries throughout via `char_indices` -- grapheme-cluster
    /// and BiDi-visual-order movement are `parley::editing::Selection`
    /// 's own richer job, deliberately not reused here (`engine-core`
    /// has no `parley` dependency at all, §4).
    ///
    /// Returns `None` for a key this method doesn't claim (`Tab`/
    /// `Escape`) -- the caller falls through to the generic handling
    /// for those. Every other key returns `Some`: `Changed(field)` for
    /// a real content edit, `None` (the outcome, not the `Option`) for
    /// pure cursor movement or a genuine no-op (e.g. `Backspace` at
    /// `cursor == 0`) -- `EventKind::Change` (M14 Phase 3) only ever
    /// means "the bound value actually changed," and cursor position
    /// isn't the bound value.
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
    fn dispatch_text_field_key(
        &mut self,
        field: NodeId,
        key: Key,
        shift: bool,
    ) -> Option<DispatchOutcome> {
        let NodeKind::TextField(state) = &mut self.nodes[field].kind else {
            return None;
        };
        match key {
            Key::Backspace => {
                if Self::delete_selection(state) {
                    return Some(DispatchOutcome::Changed(field));
                }
                if state.cursor == 0 {
                    return Some(DispatchOutcome::None);
                }
                let prev = state.content[..state.cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                state.content.replace_range(prev..state.cursor, "");
                state.cursor = prev;
                Some(DispatchOutcome::Changed(field))
            }
            Key::Delete => {
                if Self::delete_selection(state) {
                    return Some(DispatchOutcome::Changed(field));
                }
                if state.cursor >= state.content.len() {
                    return Some(DispatchOutcome::None);
                }
                let next = state.content[state.cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| state.cursor + i)
                    .unwrap_or(state.content.len());
                state.content.replace_range(state.cursor..next, "");
                Some(DispatchOutcome::Changed(field))
            }
            Key::ArrowLeft => {
                if !shift && let Some(anchor) = state.selection_anchor.take() {
                    state.cursor = state.cursor.min(anchor);
                    return Some(DispatchOutcome::None);
                }
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                }
                if state.cursor > 0 {
                    state.cursor = state.content[..state.cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
                Some(DispatchOutcome::None)
            }
            Key::ArrowRight => {
                if !shift && let Some(anchor) = state.selection_anchor.take() {
                    state.cursor = state.cursor.max(anchor);
                    return Some(DispatchOutcome::None);
                }
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                }
                if state.cursor < state.content.len() {
                    state.cursor = state.content[state.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| state.cursor + i)
                        .unwrap_or(state.content.len());
                }
                Some(DispatchOutcome::None)
            }
            Key::Home => {
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                } else {
                    state.selection_anchor = None;
                }
                state.cursor = 0;
                Some(DispatchOutcome::None)
            }
            Key::End => {
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                } else {
                    state.selection_anchor = None;
                }
                state.cursor = state.content.len();
                Some(DispatchOutcome::None)
            }
            // A real space keypress reaches `KeyPressed` (`Key::Space`,
            // matched by `translate_key` before `TextInput` would ever
            // fire for it, §8) rather than `TextInput` -- so a focused
            // `TextField` must claim it here as a real inserted space,
            // not fall through to `Key::Enter | Key::Space =>
            // Activated`'s own generic button-activation meaning.
            Key::Space => {
                Self::delete_selection(state);
                state.content.insert(state.cursor, ' ');
                state.cursor += 1;
                Some(DispatchOutcome::Changed(field))
            }
            // A single-line field: `Enter` is consumed (no activation,
            // matching `Space`'s own reasoning above) but deliberately
            // doesn't insert a newline either -- real, stated,
            // single-line scope, not a general multiline text area.
            Key::Enter => Some(DispatchOutcome::None),
            Key::Tab | Key::Escape => None,
        }
    }

    /// M15 Phase 3 (§16.7): deletes a real, active selection (`anchor
    /// != cursor`) and leaves `cursor` at the deleted range's own
    /// start -- returns `true` if it did, `false` (a true no-op) if no
    /// real selection was active. Shared by every real edit that must
    /// replace a selection rather than naively act at a bare cursor.
    fn delete_selection(state: &mut TextFieldState) -> bool {
        let Some(anchor) = state.selection_anchor else {
            return false;
        };
        if anchor == state.cursor {
            state.selection_anchor = None;
            return false;
        }
        let (start, end) = if anchor < state.cursor {
            (anchor, state.cursor)
        } else {
            (state.cursor, anchor)
        };
        state.content.replace_range(start..end, "");
        state.cursor = start;
        state.selection_anchor = None;
        true
    }

    /// M17 Phase 1 (§8): a pure, real read of a `TextField`'s own
    /// currently selected text -- `None` if `field` isn't a real,
    /// present `TextField`, or its selection is empty/collapsed
    /// (`anchor == cursor`), the same "not a real selection"
    /// definition `delete_selection` already uses. Never touches a
    /// real OS clipboard itself -- `engine-core` has no platform
    /// access at all (§4's crate-boundary rule); the real, external
    /// clipboard write is `engine-py`'s own job, this only ever
    /// answers "what text a real copy would grab."
    pub fn text_field_selected_text(&self, field: NodeId) -> Option<String> {
        let NodeKind::TextField(state) = &self.nodes.get(field)?.kind else {
            return None;
        };
        let anchor = state.selection_anchor?;
        if anchor == state.cursor {
            return None;
        }
        let (start, end) = if anchor < state.cursor {
            (anchor, state.cursor)
        } else {
            (state.cursor, anchor)
        };
        Some(state.content[start..end].to_string())
    }

    /// M17 Phase 1 (§8): `text_field_selected_text`'s own real cut
    /// counterpart -- reads the same real selected text, then deletes
    /// it via the identical shared `delete_selection` helper `Backspace`
    /// /`Delete`/`Space`/`TextInput` already use (M15 Phase 3), so a
    /// real cut is genuinely indistinguishable from "select text, read
    /// it, then delete it" -- not a second, parallel selection-removal
    /// mechanism.
    pub fn cut_text_field_selection(&mut self, field: NodeId) -> Option<String> {
        let text = self.text_field_selected_text(field)?;
        let NodeKind::TextField(state) = &mut self.nodes[field].kind else {
            return None;
        };
        Self::delete_selection(state);
        Some(text)
    }

    /// M18 Phase 1 (§8, §10, §11.9, §11.10): the real, pure `engine-core`
    /// half of click-to-position -- `engine-render`'s `TextRenderer::
    /// hit_test_position` (the real per-glyph shaping `engine-core` has
    /// no visibility into, §4) computes *which byte offset* a click
    /// landed on; this method is the plain mutation that applies it,
    /// the identical "engine-core owns the real mutation" split `set_
    /// slider_position`/`dispatch_text_field_key` already established.
    /// A plain click always collapses any active selection -- real
    /// desktop-editor behavior, matching every non-shift cursor movement
    /// `dispatch_text_field_key` already has (M15 Phase 2/3).
    ///
    /// `offset` is clamped to a real UTF-8 char boundary within `0..=
    /// content.len()` -- defensive: `parley::editing::Cursor::from_point`
    /// 's own result should already land on one, but this method must
    /// stay correct even if a future caller doesn't go through it.
    /// Returns whether `field` was actually a real `TextField` -- a
    /// no-op on any other kind or a stale/missing `NodeId`, mirroring
    /// `dock::start_drag`'s own "press on the wrong thing, nothing
    /// happens" precedent.
    pub fn set_text_field_cursor(&mut self, field: NodeId, offset: usize) -> bool {
        let Some(NodeKind::TextField(state)) = self.nodes.get_mut(field).map(|n| &mut n.kind)
        else {
            return false;
        };
        state.cursor = Self::char_boundary(&state.content, offset);
        state.selection_anchor = None;
        true
    }

    /// Shared by `set_text_field_cursor` and `extend_text_field_
    /// selection` (M18 Phase 1/2): clamps a raw byte offset to
    /// `0..=content.len()` and snaps down to the nearest real UTF-8
    /// char boundary -- defensive in both callers (`parley::editing::
    /// Cursor::from_point`'s own result should already land on one),
    /// factored out once a second real caller needed it, not
    /// duplicated.
    fn char_boundary(content: &str, offset: usize) -> usize {
        let clamped = offset.min(content.len());
        (0..=clamped)
            .rev()
            .find(|&i| content.is_char_boundary(i))
            .unwrap_or(0)
    }

    /// M18 Phase 2 (§8, §10): `set_text_field_cursor`'s own real drag-
    /// extend sibling -- a genuinely different operation, not the same
    /// method with a flag (the same "two distinct real behaviors, two
    /// methods" shape `text_field_selected_text`/`cut_text_field_
    /// selection` already established). Seeds `selection_anchor` at the
    /// *current* `cursor` only if one isn't already active -- the
    /// identical `get_or_insert`-at-first-move pattern shift-arrow
    /// selection already uses (M15 Phase 3) -- then moves `cursor`,
    /// growing the real selection instead of repeatedly collapsing it
    /// the way `set_text_field_cursor` deliberately does for a plain
    /// click.
    pub fn extend_text_field_selection(&mut self, field: NodeId, offset: usize) -> bool {
        let Some(NodeKind::TextField(state)) = self.nodes.get_mut(field).map(|n| &mut n.kind)
        else {
            return false;
        };
        if state.selection_anchor.is_none() {
            state.selection_anchor = Some(state.cursor);
        }
        state.cursor = Self::char_boundary(&state.content, offset);
        true
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
                // M10 Phase 1 (§11.3): a real press outside every open
                // dismiss_on_outside_click overlay's own subtree closes
                // it and consumes this press -- skips the normal hit/
                // ripple registration below entirely, matching
                // Android's own real "outside touch dismisses, doesn't
                // pass through" convention (`PLAN.md`).
                if self.dismiss_overlays_outside(position) {
                    self.pressed = None;
                    return DispatchOutcome::None;
                }
                let hit = self.hit_test(root, position);
                if let Some(node) = hit {
                    self.pressed = Some((button, node));
                    // M4 Phase 3 (§11.5), widened M14 Phase 2 (§7.3):
                    // pressing a splitter or a slider with the primary
                    // button starts a real drag -- reuses this same
                    // hit-test result, not a second one.
                    if button == PointerButton::Primary
                        && matches!(
                            self.nodes.get(node).map(|n| &n.kind),
                            Some(NodeKind::Splitter(_)) | Some(NodeKind::Slider(_))
                        )
                    {
                        self.dragging = Some(node);
                    }
                    // M18 Phase 1 (§8, §10): a real click-to-focus,
                    // scoped specifically to `TextField` -- before this,
                    // `PointerPressed` never touched `self.focused` at
                    // all anywhere (focus was Tab-driven, or explicit
                    // via `set_focus_to`'s own AT-SPI/test callers), a
                    // real, bigger-than-scoped finding surfaced while
                    // investigating this phase. Every real text field in
                    // every real desktop app focuses itself on click;
                    // this is not a generic click-to-focus for every
                    // node kind, which would be real, separate scope
                    // creep beyond what this phase needs. Reuses `set_
                    // focus_to` verbatim -- the real focus-ring
                    // transition it already drives is exactly correct
                    // here too, not a second mechanism.
                    if button == PointerButton::Primary
                        && matches!(
                            self.nodes.get(node).map(|n| &n.kind),
                            Some(NodeKind::TextField(_))
                        )
                    {
                        self.set_focus_to(
                            node,
                            config.focus_ring_opacity,
                            config.focus_ring_duration,
                            now,
                        );
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

                // M14 Phase 3 (§16.7): a real `Slider` drag genuinely
                // ending is this node's own real, meaningful edit --
                // takes priority over the ordinary same-node-press-
                // release `Activated`/`SecondaryActivated` logic above
                // (a slider's own real interaction is its value
                // settling, not a click). Checked *before* `self.
                // dragging` is cleared below.
                let outcome = if button == PointerButton::Primary
                    && matches!(
                        self.dragging.map(|id| &self.nodes[id].kind),
                        Some(NodeKind::Slider(_))
                    ) {
                    DispatchOutcome::Changed(self.dragging.expect("checked by matches! above"))
                } else {
                    outcome
                };

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
            InputEvent::KeyPressed { key, shift } => {
                // M15 Phase 2 (§8, §10): a focused `TextField` gets
                // first refusal on most keys -- its own real "Enter"/
                // "Space" meaning (insert a character) is genuinely
                // different from the generic button-activation meaning
                // below, so this can't simply run after it. `Tab`/
                // `Escape` still fall through unchanged (`dispatch_
                // text_field_key` returns `None` for those two,
                // meaning "not mine to handle") -- a focused field must
                // still lose focus on Tab and still dismiss overlays on
                // Escape, the same as any other focused node.
                if let Some(field) = self.focused
                    && matches!(
                        self.nodes.get(field).map(|n| &n.kind),
                        Some(NodeKind::TextField(_))
                    )
                    && let Some(outcome) = self.dispatch_text_field_key(field, key, shift)
                {
                    return outcome;
                }
                match key {
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
                    // M10 Phase 1 (§11.3): closes every real, currently-
                    // open dismiss_on_escape overlay -- a mechanical
                    // consequence handled entirely here, the same shape
                    // ripple-spawn/hover-update already use, no new
                    // outcome variant.
                    Key::Escape => {
                        self.dismiss_escapable_overlays();
                        DispatchOutcome::None
                    }
                    // M15 Phase 2: real, but only ever meaningful when a
                    // `TextField` is focused -- handled above via `
                    // dispatch_text_field_key` in that case. Reaching
                    // here means no `TextField` is focused at all, a
                    // true no-op.
                    Key::Backspace
                    | Key::Delete
                    | Key::ArrowLeft
                    | Key::ArrowRight
                    | Key::Home
                    | Key::End => DispatchOutcome::None,
                }
            }
            InputEvent::KeyReleased { .. } => DispatchOutcome::None,
            // M15 Phase 2 (§8, §10): a real, produced character
            // keypress -- only meaningful when a `TextField` is
            // focused (a true no-op otherwise, the same "mechanism
            // only" shape every other real dispatch already follows).
            InputEvent::TextInput(text) => {
                let Some(field) = self.focused else {
                    return DispatchOutcome::None;
                };
                let NodeKind::TextField(state) = &mut self.nodes[field].kind else {
                    return DispatchOutcome::None;
                };
                // M15 Phase 3 (§16.7): typing over a real, active
                // selection replaces it -- the same real desktop-editor
                // behavior `Backspace`/`Delete`/`Space` already apply,
                // via the identical shared helper.
                Self::delete_selection(state);
                state.content.insert_str(state.cursor, &text);
                state.cursor += text.len();
                // M17 Phase 2 (§8): a real insertion -- whether from a
                // plain keypress or a real IME `Commit` (both reach
                // this same arm) -- always clears any stale preedit.
                // Defensive: `winit`'s own real behavior already keeps
                // plain `KeyboardInput`/`Ime` events mutually exclusive
                // during composition, so `preedit` should already be
                // `None` here in practice, but a real commit is exactly
                // the moment composition ends either way.
                state.preedit = None;
                DispatchOutcome::Changed(field)
            }
            // M4 Phase 8 (§11.7/§11.8 groundwork): a true no-op today,
            // deliberately -- wiring this to VirtualList's window
            // movement needs the still-open real scrollable-viewport
            // gap (clipping + scroll offset), not manufactured here
            // ahead of that need. Real translation from a genuine
            // winit::WindowEvent::MouseWheel already reaches this far
            // (engine-platform); this is where it stops for now.
            // M8 Phase 3 (§11.7): closes M4 Phase 8's own stated gap --
            // hit-tests at the event's own position (the same real
            // mechanism PointerPressed/PointerReleased already use),
            // walks up the hit node's own parent chain for the nearest
            // NodeKind::VirtualList (a scroll gesture can land on any
            // materialized child, not just the list's own root pixel --
            // real browser/OS scroll-bubbling behavior), and moves that
            // list's own real scroll offset. A mechanical consequence
            // handled entirely here, the same shape ripple-spawn-on-
            // press/hover-update already use -- still DispatchOutcome::
            // None, nothing for the app layer to be told happened.
            InputEvent::Scroll { delta, position } => {
                if let Some(hit) = self.hit_test(root, position) {
                    let mut current = Some(hit);
                    while let Some(id) = current {
                        let node = &self.nodes[id];
                        if matches!(node.kind, NodeKind::VirtualList(_)) {
                            let delta_y = match delta {
                                ScrollDelta::Lines(_, y) => y * 20.0,
                                ScrollDelta::Pixels(_, y) => y,
                            };
                            self.scroll_virtual_list_by(id, delta_y);
                            break;
                        }
                        current = node.parent;
                    }
                }
                DispatchOutcome::None
            }
            // M7 Phase 3 (§7.1): plumbing only, see `InputEvent::
            // ThemeChanged`'s own doc comment -- `engine-py` handles
            // this directly on the raw event, the same way it already
            // does for dock-drag `PointerPressed`/`PointerReleased`.
            InputEvent::ThemeChanged { .. } => DispatchOutcome::None,
            // M17 Phase 1 (§8): plumbing only, the identical shape --
            // `engine-core` has no clipboard access at all, so the real
            // work (reading `Tree::text_field_selected_text`/`cut_
            // text_field_selection` and the actual OS clipboard I/O)
            // happens in `engine-py`'s own raw-event handling, not here.
            InputEvent::Copy | InputEvent::Cut | InputEvent::PasteRequested => {
                DispatchOutcome::None
            }
            // M17 Phase 2 (§8): a real, mechanical mutation (unlike
            // Copy/Cut/PasteRequested above, this needs no OS access --
            // `engine-platform` already extracted the real preedit text
            // from `winit::event::Ime::Preedit` before this ever
            // reaches here), but never a `Change`: a composition
            // preview isn't committed content, nothing has actually
            // been typed yet.
            InputEvent::ImePreedit(text) => {
                if let Some(field) = self.focused
                    && let Some(NodeKind::TextField(state)) =
                        self.nodes.get_mut(field).map(|n| &mut n.kind)
                {
                    state.preedit = if text.is_empty() { None } else { Some(text) };
                }
                DispatchOutcome::None
            }
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
        // M14 Phase 1 (§7.3): the real, automatic derivation
        // ARCHITECTURE.md already promised -- "a well-known field name
        // on a NodeKind payload... derives its corresponding
        // AccessStates flag... the app sets one property and the
        // accessibility tree stays correct for free." Reads `checked`
        // directly from `NodeKind::Checkbox` -- deliberately not
        // mirrored into a second `AccessStates` field first, which
        // would just be two copies of the same fact that could drift.
        if let NodeKind::Checkbox(state) = &node.kind {
            access_node.set_toggled(state.checked.into());
        }
        // M15 Phase 1 (§5, §16.7): the same real, automatic derivation
        // -- `content` is the one real source of truth, never mirrored
        // into a second copy here.
        if let NodeKind::TextField(state) = &node.kind {
            access_node.set_value(state.content.clone());
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

/// M22 Phase 1 (§5): a stable `u64` for a `NodeId` -- `to_access_id`'s
/// own real `KeyData::as_ffi()` encoding, reused here for a different
/// foreign-handle consumer (`engine-render`'s own GPU texture cache
/// keys a real, persistent `vello_hybrid::TextureId` per `Image` node
/// by this exact value, rather than inventing a second id scheme).
pub fn node_id_as_u64(id: NodeId) -> u64 {
    id.data().as_ffi()
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

    /// M6 Phase 1 (§8): the trivial cycle case -- a node can't become
    /// its own child.
    #[test]
    fn try_add_child_rejects_a_node_as_its_own_child() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let a = tree.insert(k, s, p);

        assert!(
            !tree.try_add_child(a, a),
            "attaching a node under itself must be rejected"
        );
        assert_eq!(
            tree.get(a).unwrap().children,
            Vec::<NodeId>::new(),
            "a rejected attach must leave the tree completely untouched"
        );
    }

    /// The real, multi-level case: attaching an ancestor as a child of
    /// its own descendant would corrupt the tree into a cycle no
    /// traversal could ever terminate on.
    #[test]
    fn try_add_child_rejects_an_ancestor_as_a_descendants_child() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let grandparent = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let parent = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let child = tree.insert(k, s, p);
        tree.add_child(grandparent, parent);
        tree.add_child(parent, child);

        assert!(
            !tree.try_add_child(child, grandparent),
            "attaching grandparent as child's own child must be rejected -- grandparent is \
             already child's ancestor"
        );
        assert_eq!(
            tree.get(child).unwrap().children,
            Vec::<NodeId>::new(),
            "a rejected attach must leave the tree completely untouched"
        );
        assert_eq!(
            tree.get(grandparent).unwrap().parent,
            None,
            "grandparent's own real parent (none) must be unchanged"
        );
    }

    /// The real, closely-related corruption risk found by reading
    /// `add_child` closely (PLAN.md): re-parenting an already-attached
    /// node must move it, not duplicate its parent pointer -- the same
    /// "`add_child` has no dedup" bug class M4 Phase 7/9 each already
    /// found once, now guarded against for `try_add_child`'s own
    /// general-purpose reparenting.
    #[test]
    fn try_add_child_moves_an_already_attached_node_instead_of_duplicating_it() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(10.0, 10.0);
        let old_parent = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let new_parent = tree.insert(k, s, p);
        let (k, s, p) = leaf(10.0, 10.0);
        let child = tree.insert(k, s, p);
        tree.add_child(old_parent, child);

        assert!(tree.try_add_child(new_parent, child));

        assert_eq!(
            tree.get(old_parent).unwrap().children,
            Vec::<NodeId>::new(),
            "the old parent must no longer list the moved child"
        );
        assert_eq!(
            tree.get(new_parent).unwrap().children,
            vec![child],
            "the new parent must list the moved child exactly once"
        );
        assert_eq!(
            tree.get(child).unwrap().parent,
            Some(new_parent),
            "the child's own parent pointer must point at its new parent"
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

    /// M6 Phase 4 (§8): the real, post-M5-review gap this phase closes --
    /// `absolute_position` must report a node's *actual* transformed
    /// canvas position, not its plain untransformed layout position, for
    /// a node inside an ancestor with a real `transform` -- the same
    /// claim `hit_test`/`paint_node` already correctly make (M5 Phase
    /// 1/2), now true here too.
    #[test]
    fn absolute_position_follows_an_ancestor_transform() {
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

        // Before any transform: chip sits at its plain local (0,0) --
        // default block layout places the sole child at its parent's
        // origin, matching every other transform test's own established
        // baseline (M5 Phase 1/2).
        assert_eq!(tree.absolute_position(chip), (0.0, 0.0));

        // camera's own transform: scale(2.0) about the origin, then
        // translate by (20, 20) -- chip's local (0,0) maps to canvas
        // (20, 20), not (0, 0).
        tree.get_mut(camera).unwrap().paint.transform.current =
            Affine::translate((20.0, 20.0)) * Affine::scale(2.0);

        assert_eq!(
            tree.absolute_position(chip),
            (20.0, 20.0),
            "absolute_position must report chip's real, transformed canvas position, not \
             its plain untransformed layout position"
        );
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
    fn position_overlay_over_covers_an_arbitrary_rect_independent_of_any_anchor() {
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
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );

        // A dock zone's own container -- some arbitrary rect nowhere
        // near the origin, proving this isn't secretly anchor-relative.
        let (k, s, p) = leaf(120.0, 80.0);
        let highlight = tree.insert(k, s, p);
        tree.position_overlay_over(highlight, Rect::new(140.0, 60.0, 260.0, 140.0));

        // Not attached anywhere -- `position_overlay_over` only sets
        // layout_style, matching its own doc comment ("deliberately
        // does not attach content anywhere").
        assert!(tree.get(highlight).unwrap().parent.is_none());

        tree.add_child(root, highlight);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );

        let (x, y) = tree.absolute_position(highlight);
        let size = tree.layout(highlight).size;
        assert_eq!(
            (x, y),
            (140.0, 60.0),
            "must land exactly at the rect's own origin"
        );
        assert_eq!(
            (size.width, size.height),
            (120.0, 80.0),
            "must be resized to exactly cover the rect, not keep its own prior size"
        );
    }

    #[test]
    fn close_overlay_detaches_the_node_and_drops_its_metadata() {
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
            tree.get(menu).is_some(),
            "M10 Phase 1: the overlay node itself must survive closing -- detached, not \
             destroyed, so `set_context_menu`'s own registered content can be reopened"
        );
        assert!(
            tree.get(menu).unwrap().parent.is_none(),
            "a closed overlay must be genuinely detached from its former parent"
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

    /// M10 Phase 1 (§11.3): a root + anchor (0,0)-(80,20) + a real open
    /// overlay `menu`, positioned by `open_overlay` itself directly
    /// below the anchor -- (0,20)-(120,80) -- with both `dismiss_on_*`
    /// flags set as given, ready for a real dispatch.
    fn overlay_scene(
        dismiss_on_outside_click: bool,
        dismiss_on_escape: bool,
    ) -> (Tree, NodeId, NodeId, NodeId) {
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
                dismiss_on_outside_click,
                dismiss_on_escape,
            },
        );
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(300.0),
            },
        );
        (tree, root, anchor, menu)
    }

    fn dispatch_config() -> InteractionConfig {
        InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        }
    }

    #[test]
    fn a_real_press_outside_a_dismiss_on_outside_click_overlay_closes_it() {
        let (mut tree, root, _, menu) = overlay_scene(true, true);
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(250.0, 250.0), // well outside the menu's own (0,20)-(120,80)
                button: PointerButton::Primary,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::None);
        assert!(
            tree.overlay_meta(menu).is_none(),
            "a real outside press must genuinely close the overlay, not just hide it"
        );
        assert!(
            !tree.get(root).unwrap().children.contains(&menu),
            "the closed overlay must no longer be displayed, detached from root"
        );
    }

    #[test]
    fn a_real_press_inside_the_overlay_leaves_it_open_and_dispatches_normally() {
        let (mut tree, root, _, menu) = overlay_scene(true, true);
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(50.0, 50.0), // inside the menu's own (0,20)-(120,80)
                button: PointerButton::Primary,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::None);
        assert!(
            tree.get(menu).is_some(),
            "a press genuinely inside the overlay must never dismiss it"
        );
        assert_eq!(
            tree.pressed,
            Some((PointerButton::Primary, menu)),
            "an in-bounds press must still register normally -- dismissal must not swallow it"
        );
    }

    #[test]
    fn a_real_press_back_on_the_anchor_does_not_dismiss_its_own_overlay() {
        // A real regression found while verifying Phase 1 end to end
        // (Python's own `test_right_clicking_the_same_anchor_twice_
        // does_not_crash`): right-clicking the same anchor a second
        // time must not have its own press treated as an "outside
        // click" against the overlay it's about to (safely, no-op)
        // reopen -- the anchor itself is genuinely excluded.
        let (mut tree, root, anchor, menu) = overlay_scene(true, true);
        let anchor_center = {
            let (x, y) = tree.absolute_position(anchor);
            let layout = tree.layout(anchor);
            Point::new(
                x + f64::from(layout.size.width) / 2.0,
                y + f64::from(layout.size.height) / 2.0,
            )
        };
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: anchor_center,
                button: PointerButton::Secondary,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert!(
            tree.get(menu).is_some(),
            "a press back on the overlay's own anchor must not dismiss it"
        );
    }

    #[test]
    fn dismiss_on_outside_click_false_survives_a_real_outside_press() {
        let (mut tree, root, _, menu) = overlay_scene(false, true);
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(250.0, 250.0),
                button: PointerButton::Primary,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert!(
            tree.get(menu).is_some(),
            "dismiss_on_outside_click: false must never be dismissed by an outside press"
        );
    }

    #[test]
    fn a_real_escape_dispatch_closes_every_dismiss_on_escape_overlay() {
        let (mut tree, root, _, menu) = overlay_scene(true, true);
        let outcome = tree.dispatch(
            root,
            InputEvent::KeyPressed {
                key: Key::Escape,
                shift: false,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::None);
        assert!(
            tree.overlay_meta(menu).is_none(),
            "a real Escape dispatch must genuinely close a dismiss_on_escape overlay"
        );
        assert!(
            !tree.get(root).unwrap().children.contains(&menu),
            "the closed overlay must no longer be displayed, detached from root"
        );
    }

    #[test]
    fn dismiss_on_escape_false_survives_a_real_escape_dispatch() {
        let (mut tree, root, _, menu) = overlay_scene(true, false);
        tree.dispatch(
            root,
            InputEvent::KeyPressed {
                key: Key::Escape,
                shift: false,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert!(
            tree.get(menu).is_some(),
            "dismiss_on_escape: false must never be dismissed by an Escape dispatch"
        );
    }

    /// M10 Phase 1 (§11.3): the exact real scenario that motivated
    /// `close_overlay`'s own detach-not-destroy fix -- close an
    /// overlay via a real Escape dispatch, then reopen the *same*
    /// content node again via `open_overlay`. Must not panic (a
    /// destroyed content `NodeId` would make `open_overlay`'s own
    /// `self.get(content).expect(...)` fail), and the reopened overlay
    /// must be for real (attached to root again).
    #[test]
    fn a_dismissed_overlays_own_content_can_be_reopened() {
        let (mut tree, root, anchor, menu) = overlay_scene(true, true);
        tree.dispatch(
            root,
            InputEvent::KeyPressed {
                key: Key::Escape,
                shift: false,
            },
            &dispatch_config(),
            Instant::now(),
        );
        assert!(
            tree.overlay_meta(menu).is_none(),
            "sanity: must be closed first"
        );

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

        assert!(
            tree.overlay_meta(menu).is_some(),
            "the same content NodeId must be reopenable after a real dismissal"
        );
        assert!(
            tree.get(root).unwrap().children.contains(&menu),
            "the reopened overlay must be genuinely attached to root again"
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

    /// M14 Phase 2 (§5, §7.3): a real `Slider` scene -- a real, definite
    /// width/height it drags along, mirroring `splitter_scene`'s own
    /// real-geometry shape (no `Style::default()` auto-sizing, which
    /// gives `update_slider_drag` nothing real to divide a fraction of).
    fn slider_scene() -> (Tree, NodeId, NodeId, Size<AvailableSpace>) {
        let mut tree = Tree::new();
        let root_style = Style {
            size: Size {
                width: length(200.0),
                height: length(20.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let slider = tree.insert(
            NodeKind::Slider(SliderState::new(0.0)),
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(20.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0x63, 0x50, 0xA4, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, slider);

        let available = Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(20.0),
        };
        tree.compute_layout(root, available);
        (tree, root, slider, available)
    }

    #[test]
    fn dispatch_drag_on_a_slider_moves_thumb_position_live_as_the_pointer_moves() {
        let (mut tree, root, slider, _available) = slider_scene();
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

        // Press anywhere on the slider (x=50, well inside [0,200)).
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(50.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );

        // Drag to x=100: fraction = 100/200 = 0.5.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(100.0, 10.0),
            },
            &config,
            now,
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        assert_eq!(
            state.thumb_position.current, 0.5,
            "the thumb must live-follow the cursor to its first drag position"
        );

        // Keep dragging, to x=170: fraction = 170/200 = 0.85 -- proves
        // this isn't a one-shot snap, the thumb keeps following.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(170.0, 10.0),
            },
            &config,
            now,
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        assert_eq!(state.thumb_position.current, 0.85);
    }

    #[test]
    fn dispatch_a_slider_drag_clamps_to_the_real_0_to_1_range() {
        let (mut tree, root, slider, _available) = slider_scene();
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
                position: Point::new(50.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        // Well past the slider's own right edge.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(10_000.0, 10.0),
            },
            &config,
            now,
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        assert_eq!(
            state.thumb_position.current, 1.0,
            "must clamp to 1.0, not overshoot"
        );

        // Well past the left edge, including negative.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(-500.0, 10.0),
            },
            &config,
            now,
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        assert_eq!(
            state.thumb_position.current, 0.0,
            "must clamp to 0.0, not go negative"
        );
    }

    #[test]
    fn dispatch_release_ends_a_slider_drag_so_further_pointer_moves_dont_move_it() {
        let (mut tree, root, slider, _available) = slider_scene();
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
                position: Point::new(50.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(100.0, 10.0),
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(100.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        let position_after_release = state.thumb_position.current;

        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(180.0, 10.0),
            },
            &config,
            now,
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        assert_eq!(
            state.thumb_position.current, position_after_release,
            "a pointer move after release must not still be tracked as a drag"
        );
    }

    /// M14 Phase 3 (§16.7): the real, mechanical half of a slider's own
    /// `Change` -- `Tree::dispatch` itself must produce `DispatchOutcome
    /// ::Changed(slider)` on the real release that ends a real drag, the
    /// same "engine-core only knows THAT it happened" contract
    /// `Activated`/`HoverChanged` already have.
    #[test]
    fn dispatch_release_ending_a_real_slider_drag_produces_changed() {
        let (mut tree, root, slider, _available) = slider_scene();
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
                position: Point::new(50.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(100.0, 10.0),
            },
            &config,
            now,
        );
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(100.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::Changed(slider),
            "a real release ending a real slider drag must produce Changed(slider), not \
             Activated or None"
        );
    }

    /// M14 Phase 3 (§16.7): a release with *no* drag in progress (a
    /// plain click elsewhere, or a release on a real `Slider` that was
    /// never actually pressed to start a drag) must never produce a
    /// spurious `Changed` -- the same "no real mechanical fact, no
    /// outcome" contract every other `DispatchOutcome` variant already
    /// has.
    #[test]
    fn dispatch_release_with_no_slider_drag_in_progress_never_produces_changed() {
        let (mut tree, root, _slider, _available) = slider_scene();
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

        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(500.0, 500.0),
                button: PointerButton::Primary,
            },
            &config,
            now,
        );
        assert_eq!(outcome, DispatchOutcome::None);
    }

    #[test]
    fn tick_all_animates_thumb_position_toward_a_real_target() {
        let mut tree = Tree::new();
        let (_, style, paint) = leaf(200.0, 20.0);
        let slider = tree.insert(NodeKind::Slider(SliderState::new(0.0)), style, paint);

        let now = Instant::now();
        let NodeKind::Slider(state) = &mut tree.get_mut(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        state
            .thumb_position
            .animate_to(1.0, Duration::from_millis(100), MotionCurve::Linear, now);

        let (any_active, _) = tree.tick_all(now + Duration::from_millis(50));
        assert!(
            any_active,
            "a mid-flight thumb_position animation must report as active"
        );
        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider");
        };
        assert!(
            state.thumb_position.current > 0.0 && state.thumb_position.current < 1.0,
            "thumb_position must be genuinely mid-animation at the halfway point, got {}",
            state.thumb_position.current
        );

        let (any_active, _) = tree.tick_all(now + Duration::from_millis(200));
        assert!(
            !any_active,
            "the animation must be finished well past its own duration"
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

    /// A list with a real, definite viewport height, much shorter than
    /// its own real content extent (`item_extent * item_count`) -- the
    /// exact shape `scroll_virtual_list_by`'s own clamping needs to be
    /// real, not vacuous (`Style::default()`'s own auto-sizing, used by
    /// this module's other `VirtualList` tests, doesn't give a
    /// meaningful "viewport" to clamp against).
    fn scrollable_list(item_count: usize) -> (Tree, NodeId) {
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(item_count, ItemExtent::Fixed(20.0))),
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.compute_layout(
            list,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(100.0),
            },
        );
        (tree, list)
    }

    #[test]
    fn scroll_virtual_list_by_moves_and_clamps_the_offset_at_both_ends() {
        // 20 items * 20px = 400px of real content, in a 100px-tall
        // viewport -- max_offset = 400 - 100 = 300.0.
        let (mut tree, list) = scrollable_list(20);

        tree.scroll_virtual_list_by(list, 50.0);
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(state.scroll_offset.current, 50.0);

        // Past the real upper bound -- must clamp to max_offset, not
        // overshoot into content that doesn't exist.
        tree.scroll_virtual_list_by(list, 1000.0);
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.scroll_offset.current, 300.0,
            "must clamp to the real max_offset (content_extent - viewport_height), not overshoot"
        );

        // Past the real lower bound -- must clamp to 0.0, not go
        // negative.
        tree.scroll_virtual_list_by(list, -10_000.0);
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.scroll_offset.current, 0.0,
            "must clamp to 0.0, not go negative"
        );
    }

    #[test]
    fn scroll_virtual_list_by_cannot_scroll_a_list_shorter_than_its_own_viewport() {
        // 3 items * 20px = 60px of real content, in a 100px-tall
        // viewport -- there's nothing to reveal, so max_offset must be
        // 0.0, not negative.
        let (mut tree, list) = scrollable_list(3);
        tree.scroll_virtual_list_by(list, 50.0);
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.scroll_offset.current, 0.0,
            "a list shorter than its own viewport must not be scrollable at all"
        );
    }

    /// M12 Phase 1 (§11.7): a real, non-uniform set of resolved offsets
    /// -- rows of height 10, 30, 15, 25, matching neither a uniform
    /// spacing nor a simple arithmetic sequence, so a test passing here
    /// can't be an accident of `Fixed`-shaped math still secretly being
    /// used underneath.
    fn variable_offsets() -> Vec<(usize, f64)> {
        // Heights: [10, 30, 15, 25] -> cumulative offsets [0, 10, 40, 55],
        // total extent (index 4, one past the last item) = 80.
        vec![(0, 0.0), (1, 10.0), (2, 40.0), (3, 55.0), (4, 80.0)]
    }

    #[test]
    fn offset_of_and_total_extent_match_the_uniform_formula_for_fixed() {
        let state = VirtualListState::new(10, ItemExtent::Fixed(20.0));
        for idx in 0..10 {
            assert_eq!(
                state.offset_of(idx),
                idx as f64 * 20.0,
                "Fixed must keep computing idx * item_extent exactly as before this phase"
            );
        }
        assert_eq!(
            state.total_extent(),
            200.0,
            "Fixed must keep computing item_count * item_extent exactly as before this phase"
        );
    }

    #[test]
    fn offset_of_and_total_extent_use_real_resolved_offsets_for_variable() {
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.set_virtual_list_resolved_offsets(list, variable_offsets());

        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(state.offset_of(0), 0.0);
        assert_eq!(state.offset_of(1), 10.0);
        assert_eq!(state.offset_of(2), 40.0);
        assert_eq!(
            state.offset_of(3),
            55.0,
            "must reflect the real, non-uniform spacing"
        );
        assert_eq!(
            state.total_extent(),
            80.0,
            "total_extent must be the real resolved offset one past the last item, not \
             item_count * some average extent"
        );
    }

    #[test]
    #[should_panic(expected = "item 2's own offset must be resolved")]
    fn offset_of_panics_on_an_unresolved_variable_index() {
        let state = VirtualListState::new(4, ItemExtent::Variable);
        // Nothing has been resolved -- querying any index must panic
        // with a clear message, not silently return a wrong answer
        // (e.g. 0.0, which could be mistaken for a real, resolved offset).
        state.offset_of(2);
    }

    #[test]
    fn set_virtual_list_window_positions_items_at_their_real_non_uniform_offsets() {
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.set_virtual_list_resolved_offsets(list, variable_offsets());

        tree.set_virtual_list_window(list, 0..4, virtual_list_materializer);
        tree.compute_layout(
            list,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(400.0),
            },
        );

        let expected = [0.0, 10.0, 40.0, 55.0];
        for (idx, &expected_y) in expected.iter().enumerate() {
            let id = state_materialized_id(&tree, list, idx);
            let (_, y) = tree.absolute_position(id);
            assert_eq!(
                y, expected_y,
                "item {idx} must be positioned at its own real, non-uniform resolved offset, \
                 not idx * some uniform extent"
            );
        }
    }

    #[test]
    fn scroll_virtual_list_by_clamps_against_the_real_non_uniform_total_extent() {
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(30.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.compute_layout(
            list,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(30.0),
            },
        );
        tree.set_virtual_list_resolved_offsets(list, variable_offsets());

        // Real content extent 80.0, viewport 30.0 -> max_offset = 50.0,
        // not the wrong 4 * some uniform guess a Fixed-shaped formula
        // would produce.
        tree.scroll_virtual_list_by(list, 1000.0);
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.scroll_offset.current, 50.0,
            "must clamp to the real non-uniform total_extent minus viewport height"
        );
    }

    #[test]
    fn dispatch_scroll_over_a_virtual_lists_child_updates_its_real_scroll_offset() {
        let (mut tree, list) = scrollable_list(20);
        tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);
        tree.compute_layout(
            list,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(100.0),
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
        // Item 1's own real slot (y in [20, 40)) -- a real scroll
        // gesture over a materialized *child*, not the list's own root
        // pixel, must still bubble up to the list's own scroll offset.
        let outcome = tree.dispatch(
            list,
            InputEvent::Scroll {
                delta: ScrollDelta::Lines(0.0, 2.0),
                position: Point::new(100.0, 30.0),
            },
            &config,
            Instant::now(),
        );

        assert_eq!(outcome, DispatchOutcome::None);
        let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
            panic!("expected a VirtualList");
        };
        assert_eq!(
            state.scroll_offset.current, 40.0,
            "2.0 lines * this crate's own 20px-per-line convention == 40.0px"
        );
    }

    #[test]
    fn dispatch_scroll_that_hits_nothing_is_a_true_no_op() {
        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(50.0, 50.0);
        let root = tree.insert(kind, style, paint);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(50.0),
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
        // Hits `root` itself (a plain Rect, no VirtualList ancestor at
        // all) -- must not panic, and there's nothing real to assert
        // changed, since nothing in this tree can scroll.
        let outcome = tree.dispatch(
            root,
            InputEvent::Scroll {
                delta: ScrollDelta::Lines(0.0, 2.0),
                position: Point::new(25.0, 25.0),
            },
            &config,
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::None);
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

        let (still_active, _completed) = tree.tick_all(start + Duration::from_millis(500));
        assert!(still_active);
        let node = tree.get(id).unwrap();
        assert!((node.paint.opacity.current - 0.5).abs() < 0.01);

        let (still_active, _completed) = tree.tick_all(start + Duration::from_secs(2));
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

        let (still_active, _completed) = tree.tick_all(start + Duration::from_millis(100));
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

        let (still_active, _completed) = tree.tick_all(start + Duration::from_secs(1));
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

    /// M7 Phase 3 (§7.1): `set_all_interaction_tints` must update every
    /// node that already opted into `InteractionState`, and must leave
    /// a node that never opted in exactly as `None` -- never lazily
    /// creating one just to give it a tint, matching `interaction_mut`'s
    /// own "only a node that opts in pays the cost" contract.
    #[test]
    fn set_all_interaction_tints_updates_only_already_opted_in_nodes() {
        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(10.0, 10.0);
        let opted_in = tree.insert(kind, style, paint);
        tree.interaction_mut(opted_in);

        let (kind, style, paint) = leaf(10.0, 10.0);
        let never_opted_in = tree.insert(kind, style, paint);

        let real_color = peniko::Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);
        tree.set_all_interaction_tints(real_color);

        assert_eq!(
            tree.get(opted_in)
                .unwrap()
                .interaction
                .as_ref()
                .unwrap()
                .tint,
            real_color,
            "an already-opted-in node must pick up the new tint"
        );
        assert!(
            tree.get(never_opted_in).unwrap().interaction.is_none(),
            "a node that never opted into InteractionState must not have one lazily created \
             just to give it a tint"
        );
    }

    /// M7 Phase 3 (§7.1): `ThemeChanged` is plumbing only, the identical
    /// "true no-op" contract `Scroll` already established -- `engine-py`
    /// handles the real color-resolution/tint-push side effect directly
    /// on the raw event, not through `Tree::dispatch`'s own return value.
    #[test]
    fn theme_changed_dispatches_to_a_true_no_op() {
        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(10.0, 10.0);
        let root = tree.insert(kind, style, paint);
        tree.interaction_mut(root);
        let tint_before = tree.get(root).unwrap().interaction.as_ref().unwrap().tint;

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let outcome = tree.dispatch(
            root,
            InputEvent::ThemeChanged { dark: true },
            &config,
            Instant::now(),
        );

        assert_eq!(outcome, DispatchOutcome::None);
        assert_eq!(
            tree.get(root).unwrap().interaction.as_ref().unwrap().tint,
            tint_before,
            "Tree::dispatch itself must never touch a tint on ThemeChanged -- that's \
             engine-py's own job, via set_all_interaction_tints"
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

    /// M5 Phase 3 (§11.10): a `Canvas` node's `CustomHitTest::Circle`
    /// genuinely overrides the default rect test, not just narrows it --
    /// a point inside the node's own 100x100 rect but outside the
    /// circle must miss entirely (the canvas is this tree's only node,
    /// so there's nothing else for it to fall through to).
    #[test]
    fn canvas_custom_circle_hit_test_overrides_the_default_rect() {
        let mut tree = Tree::new();
        let mut state = CanvasState::new();
        state.hit_test = Some(CustomHitTest::Circle {
            cx: 50.0,
            cy: 50.0,
            radius: 10.0,
        });
        let canvas = tree.insert(
            NodeKind::Canvas(state),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.compute_layout(
            canvas,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );

        assert_eq!(
            tree.hit_test(canvas, Point::new(50.0, 50.0)),
            Some(canvas),
            "the circle's own center must hit"
        );
        assert_eq!(
            tree.hit_test(canvas, Point::new(5.0, 5.0)),
            None,
            "a point well inside the node's 100x100 rect but far outside the 10px-radius \
             circle must miss -- the rect default must not apply as a fallback"
        );
    }

    /// The path half of the same claim (§11.10's own "a bezier curve
    /// within N pixels of the point" example) -- a straight diagonal
    /// `BezPath` (a degenerate, zero-curvature case of the same real
    /// `ParamCurveNearest` math a true bezier would use) with a 5px
    /// tolerance.
    #[test]
    fn canvas_custom_path_hit_test_uses_real_distance_to_path() {
        let mut tree = Tree::new();
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((100.0, 100.0));

        let mut state = CanvasState::new();
        state.hit_test = Some(CustomHitTest::Path {
            path,
            tolerance: 5.0,
        });
        let canvas = tree.insert(
            NodeKind::Canvas(state),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.compute_layout(
            canvas,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );

        // Distance from (50, 52) to the line y=x is |50-52|/sqrt(2) ~= 1.41px.
        assert_eq!(
            tree.hit_test(canvas, Point::new(50.0, 52.0)),
            Some(canvas),
            "a point ~1.4px from the diagonal, within the 5px tolerance, must hit"
        );
        // Distance from (10, 90) to the line y=x is |10-90|/sqrt(2) ~= 56.6px.
        assert_eq!(
            tree.hit_test(canvas, Point::new(10.0, 90.0)),
            None,
            "a point ~56.6px from the diagonal, well outside the 5px tolerance (though \
             still inside the node's own rect), must miss"
        );
    }

    /// M11 Phase 1 (§11.10, §11.11): the non-degenerate case the test
    /// above's own doc comment names but doesn't exercise -- a real
    /// `quad_to` curve, not a straight line. `move_to(0,0)`, `quad_to`
    /// control `(50,100)`, end `(100,0)` bulges to a true midpoint of
    /// `(50,50)` (the standard quadratic-bezier weighted-control-point
    /// formula), far from the naive chord's own midpoint `(50,0)`.
    #[test]
    fn canvas_custom_path_hit_test_uses_the_real_curve_not_the_straight_chord_between_its_endpoints()
     {
        let mut tree = Tree::new();
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.quad_to((50.0, 100.0), (100.0, 0.0));

        let mut state = CanvasState::new();
        state.hit_test = Some(CustomHitTest::Path {
            path,
            tolerance: 5.0,
        });
        let canvas = tree.insert(
            NodeKind::Canvas(state),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.compute_layout(
            canvas,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );

        // (50, 50) sits right on the true curve's own real midpoint --
        // 50px from the naive straight chord (0,0)-(100,0), so this
        // would wrongly MISS if hit-testing only ever saw a straight
        // line between the curve's endpoints.
        assert_eq!(
            tree.hit_test(canvas, Point::new(50.0, 50.0)),
            Some(canvas),
            "a point on the real curve's own midpoint, far from the straight chord between \
             its endpoints, must hit -- proving this is real curve-aware distance, not a \
             straight-line approximation"
        );
        // (50, 2) sits ~2px from the naive straight chord -- it would
        // wrongly HIT under a chord-only distance, but the real curve
        // passes through (50, 50) here, ~48px away, well outside the
        // 5px tolerance.
        assert_eq!(
            tree.hit_test(canvas, Point::new(50.0, 2.0)),
            None,
            "a point near the straight chord but far from the real curve must miss -- the \
             adversarial case a chord-only (not curve-aware) distance would get wrong"
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

    /// M14 Phase 1 (§7.3): `check_progress` must animate toward a real
    /// target the same way `PaintProperties`' own fields already do --
    /// confirming `tick_all`'s new `NodeKind::Checkbox` arm actually
    /// runs, not just that it compiles.
    #[test]
    fn tick_all_animates_check_progress_toward_a_real_target() {
        let mut tree = Tree::new();
        let (_, style, paint) = leaf(20.0, 20.0);
        let checkbox = tree.insert(NodeKind::Checkbox(CheckboxState::new(false)), style, paint);

        let now = Instant::now();
        let NodeKind::Checkbox(state) = &mut tree.get_mut(checkbox).unwrap().kind else {
            panic!("expected a Checkbox");
        };
        state
            .check_progress
            .animate_to(1.0, Duration::from_millis(100), MotionCurve::Linear, now);

        let (any_active, _) = tree.tick_all(now + Duration::from_millis(50));
        assert!(
            any_active,
            "a mid-flight check_progress animation must report as active"
        );
        let NodeKind::Checkbox(state) = &tree.get(checkbox).unwrap().kind else {
            panic!("expected a Checkbox");
        };
        assert!(
            state.check_progress.current > 0.0 && state.check_progress.current < 1.0,
            "check_progress must be genuinely mid-animation at the halfway point, got {}",
            state.check_progress.current
        );

        let (any_active, _) = tree.tick_all(now + Duration::from_millis(200));
        assert!(
            !any_active,
            "the animation must be finished well past its own duration"
        );
        let NodeKind::Checkbox(state) = &tree.get(checkbox).unwrap().kind else {
            panic!("expected a Checkbox");
        };
        assert_eq!(
            state.check_progress.current, 1.0,
            "must reach the real target exactly"
        );
    }

    /// M14 Phase 1 (§7.3): the real, automatic `checked` -> `Toggled`
    /// derivation ARCHITECTURE.md promises -- reading straight from
    /// `NodeKind::Checkbox`'s own real `checked` field, not a second,
    /// separately-set copy.
    #[test]
    fn build_access_update_reports_the_real_toggled_state_for_a_checkbox() {
        let mut tree = Tree::new();
        let (_, style, paint) = leaf(20.0, 20.0);
        let checkbox = tree.insert(NodeKind::Checkbox(CheckboxState::new(true)), style, paint);
        tree.compute_layout(
            checkbox,
            Size {
                width: AvailableSpace::Definite(20.0),
                height: AvailableSpace::Definite(20.0),
            },
        );

        let update = tree.build_access_update(checkbox);
        let (_, node) = &update.nodes[0];
        assert_eq!(
            node.toggled(),
            Some(accesskit::Toggled::True),
            "a checked checkbox must report Toggled::True"
        );
    }

    /// M15 Phase 1 (§5, §16.7): `TextFieldState::new`'s own real
    /// contract -- `cursor` seeds at `content`'s own real end, a real
    /// text field's own expected initial-cursor-at-end convention, not
    /// `0`.
    #[test]
    fn text_field_state_new_seeds_the_cursor_at_the_end_of_the_initial_content() {
        let state = TextFieldState::new("hello", "Roboto", 400.0, 16.0);
        assert_eq!(state.content, "hello");
        assert_eq!(
            state.cursor, 5,
            "cursor must start at content's own real end (byte offset 5 for \"hello\")"
        );
        assert_eq!(state.selection_anchor, None);
    }

    /// A `TextField`'s own real, automatic accessibility derivation --
    /// mirrors `build_access_update_reports_the_real_toggled_state_for_
    /// a_checkbox`'s own shape: reads `content` directly from `NodeKind
    /// ::TextField`, not a second, separately-set copy.
    #[test]
    fn build_access_update_reports_the_real_value_role_and_focus_action_for_a_text_field() {
        use crate::access::{Action, Role};

        let mut tree = Tree::new();
        let (_, style, paint) = leaf(120.0, 32.0);
        let field = tree.insert(
            NodeKind::TextField(TextFieldState::new("hello", "Roboto", 400.0, 16.0)),
            style,
            paint,
        );
        tree.set_access(
            field,
            AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
        );
        tree.compute_layout(
            field,
            Size {
                width: AvailableSpace::Definite(120.0),
                height: AvailableSpace::Definite(32.0),
            },
        );

        let update = tree.build_access_update(field);
        let (_, node) = &update.nodes[0];
        assert_eq!(node.role(), Role::TextInput);
        assert_eq!(
            node.value(),
            Some("hello"),
            "a TextField's own real content must reach the accessibility tree's value, \
             not a stale or missing one"
        );
        assert!(
            node.supports_action(Action::Focus),
            "a TextField must be a real Focus target for Tab/screen-reader reachability"
        );
    }

    /// M15 Phase 2 (§8, §10): mirrors `slider_scene`'s own shape -- a
    /// real `TextField`, already the `Tree`'s own real focused node
    /// (every real editing test needs that, so seeding it here avoids
    /// repeating a `set_focus_to` call in every single test below).
    fn text_field_scene(content: &str) -> (Tree, NodeId, NodeId) {
        let mut tree = Tree::new();
        let (_, root_style, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);

        let field = tree.insert(
            NodeKind::TextField(TextFieldState::new(content, "Roboto", 400.0, 16.0)),
            Style {
                size: Size {
                    width: length(120.0),
                    height: length(24.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, field);
        tree.set_focus_to(field, 1.0, Duration::ZERO, Instant::now());
        (tree, root, field)
    }

    fn field_state(tree: &Tree, field: NodeId) -> &TextFieldState {
        let NodeKind::TextField(state) = &tree.get(field).unwrap().kind else {
            panic!("expected a TextField node");
        };
        state
    }

    fn dispatch_key(tree: &mut Tree, root: NodeId, key: Key) -> DispatchOutcome {
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        tree.dispatch(
            root,
            InputEvent::KeyPressed { key, shift: false },
            &config,
            Instant::now(),
        )
    }

    /// M15 Phase 3 (§16.7): `dispatch_key`'s own real `shift`-held
    /// sibling, for selection-extension tests.
    fn dispatch_shift_key(tree: &mut Tree, root: NodeId, key: Key) -> DispatchOutcome {
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        tree.dispatch(
            root,
            InputEvent::KeyPressed { key, shift: true },
            &config,
            Instant::now(),
        )
    }

    #[test]
    fn text_input_with_no_focused_field_is_a_true_no_op() {
        let mut tree = Tree::new();
        let (_, root_style, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let outcome = tree.dispatch(
            root,
            InputEvent::TextInput("a".to_string()),
            &config,
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::None);
    }

    #[test]
    fn text_input_inserts_at_the_real_cursor_and_advances_it() {
        let (mut tree, root, field) = text_field_scene("hllo");
        // Real cursor starts at content's own end (`TextFieldState::
        // new`'s own contract) -- move it to byte offset 1 (after "h")
        // first via a real ArrowLeft x3 from the end, so the insert
        // below lands in the middle, not just appended.
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_key(&mut tree, root, Key::ArrowRight);

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let outcome = tree.dispatch(
            root,
            InputEvent::TextInput("e".to_string()),
            &config,
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        let state = field_state(&tree, field);
        assert_eq!(state.content, "hello");
        assert_eq!(
            state.cursor, 2,
            "cursor must advance past the real inserted text"
        );
    }

    #[test]
    fn backspace_removes_the_real_char_before_the_cursor() {
        let (mut tree, root, field) = text_field_scene("hello");
        let outcome = dispatch_key(&mut tree, root, Key::Backspace);
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        let state = field_state(&tree, field);
        assert_eq!(state.content, "hell");
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn backspace_at_the_real_start_is_a_genuine_no_op() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        let outcome = dispatch_key(&mut tree, root, Key::Backspace);
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "no real edit happened, so this must not report Changed"
        );
        assert_eq!(field_state(&tree, field).content, "hello");
    }

    #[test]
    fn delete_removes_the_real_char_after_the_cursor() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        let outcome = dispatch_key(&mut tree, root, Key::Delete);
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        let state = field_state(&tree, field);
        assert_eq!(state.content, "ello");
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn delete_at_the_real_end_is_a_genuine_no_op() {
        let (mut tree, root, field) = text_field_scene("hello");
        let outcome = dispatch_key(&mut tree, root, Key::Delete);
        assert_eq!(outcome, DispatchOutcome::None);
        assert_eq!(field_state(&tree, field).content, "hello");
    }

    #[test]
    fn arrow_keys_move_the_real_cursor_without_reporting_changed() {
        let (mut tree, root, field) = text_field_scene("hi");
        assert_eq!(field_state(&tree, field).cursor, 2);

        let outcome = dispatch_key(&mut tree, root, Key::ArrowLeft);
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "pure cursor movement is not a bound-value change"
        );
        assert_eq!(field_state(&tree, field).cursor, 1);

        dispatch_key(&mut tree, root, Key::ArrowRight);
        assert_eq!(field_state(&tree, field).cursor, 2);
    }

    #[test]
    fn home_and_end_jump_the_real_cursor_to_the_real_edges() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        assert_eq!(field_state(&tree, field).cursor, 0);
        dispatch_key(&mut tree, root, Key::End);
        assert_eq!(field_state(&tree, field).cursor, 5);
    }

    #[test]
    fn a_focused_text_field_inserts_a_real_space_instead_of_activating() {
        let (mut tree, root, field) = text_field_scene("ab");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_key(&mut tree, root, Key::ArrowRight);
        let outcome = dispatch_key(&mut tree, root, Key::Space);
        assert_eq!(
            outcome,
            DispatchOutcome::Changed(field),
            "Space on a focused TextField must be a real inserted character, not Activated"
        );
        assert_eq!(field_state(&tree, field).content, "a b");
    }

    #[test]
    fn enter_on_a_focused_text_field_is_consumed_without_inserting_or_activating() {
        let (mut tree, root, field) = text_field_scene("hi");
        let outcome = dispatch_key(&mut tree, root, Key::Enter);
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a single-line TextField must not activate on Enter, matching Space's own reasoning"
        );
        assert_eq!(
            field_state(&tree, field).content,
            "hi",
            "Enter must not insert a newline into a single-line field either"
        );
    }

    #[test]
    fn tab_still_moves_focus_away_from_a_focused_text_field() {
        let (mut tree, root, field) = text_field_scene("hi");
        assert_eq!(tree.focused(), Some(field));
        dispatch_key(&mut tree, root, Key::Tab);
        assert_ne!(
            tree.focused(),
            Some(field),
            "Tab must still move focus away from a focused TextField"
        );
    }

    #[test]
    fn backspace_removes_a_real_multi_byte_utf8_character_whole() {
        // "café" -- "é" is a real 2-byte UTF-8 scalar; a naive
        // byte-at-a-time backspace would corrupt it into invalid UTF-8.
        let (mut tree, root, field) = text_field_scene("café");
        let outcome = dispatch_key(&mut tree, root, Key::Backspace);
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        assert_eq!(field_state(&tree, field).content, "caf");
    }

    #[test]
    fn shift_arrow_extends_a_real_selection_from_the_cursor() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home); // cursor -> 0

        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);

        let state = field_state(&tree, field);
        assert_eq!(
            state.selection_anchor,
            Some(0),
            "the anchor must stay at the real position the selection started from"
        );
        assert_eq!(
            state.cursor, 2,
            "the cursor is the selection's own real focus end"
        );
    }

    #[test]
    fn a_bare_arrow_after_a_real_selection_collapses_to_its_near_edge() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he" (0..2)

        // A bare (non-shift) Left must collapse to the selection's own
        // real left edge (0), not move one more char left from the
        // focus end -- real desktop-editor behavior.
        dispatch_key(&mut tree, root, Key::ArrowLeft);
        let state = field_state(&tree, field);
        assert_eq!(state.cursor, 0);
        assert_eq!(
            state.selection_anchor, None,
            "collapsing must clear the selection"
        );
    }

    #[test]
    fn a_bare_arrow_after_a_real_selection_collapses_to_its_far_edge_when_moving_right() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he" (0..2)

        dispatch_key(&mut tree, root, Key::ArrowRight);
        let state = field_state(&tree, field);
        assert_eq!(state.cursor, 2);
        assert_eq!(state.selection_anchor, None);
    }

    #[test]
    fn shift_home_and_shift_end_extend_the_real_selection_to_the_real_edges() {
        let (mut tree, root, field) = text_field_scene("hello");
        // cursor starts at content's own end (5).
        dispatch_shift_key(&mut tree, root, Key::Home);
        let state = field_state(&tree, field);
        assert_eq!(state.selection_anchor, Some(5));
        assert_eq!(state.cursor, 0);

        dispatch_key(&mut tree, root, Key::End); // collapse, cursor -> end, clears selection
        dispatch_shift_key(&mut tree, root, Key::Home);
        assert_eq!(field_state(&tree, field).selection_anchor, Some(5));
    }

    #[test]
    fn backspace_deletes_a_real_active_selection_instead_of_one_char() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

        let outcome = dispatch_key(&mut tree, root, Key::Backspace);
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        let state = field_state(&tree, field);
        assert_eq!(state.content, "llo");
        assert_eq!(state.cursor, 0);
        assert_eq!(state.selection_anchor, None);
    }

    #[test]
    fn delete_deletes_a_real_active_selection_instead_of_one_char() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

        let outcome = dispatch_key(&mut tree, root, Key::Delete);
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        assert_eq!(field_state(&tree, field).content, "llo");
    }

    #[test]
    fn typing_over_a_real_selection_replaces_it() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        let outcome = tree.dispatch(
            root,
            InputEvent::TextInput("HI".to_string()),
            &config,
            Instant::now(),
        );
        assert_eq!(outcome, DispatchOutcome::Changed(field));
        let state = field_state(&tree, field);
        assert_eq!(state.content, "HIllo");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn space_over_a_real_selection_replaces_it() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

        dispatch_key(&mut tree, root, Key::Space);
        assert_eq!(field_state(&tree, field).content, " llo");
    }

    #[test]
    fn a_collapsed_zero_width_selection_is_not_treated_as_real() {
        // Shift+Right then Shift+Left back to the same spot leaves a
        // real selection_anchor set, but anchor == cursor -- a real
        // editor treats this as "no selection," not an empty delete.
        let (mut tree, root, field) = text_field_scene("hi");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowLeft);
        assert_eq!(field_state(&tree, field).cursor, 0);

        let outcome = dispatch_key(&mut tree, root, Key::Backspace);
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a zero-width selection at the real start must still be a genuine no-op"
        );
        assert_eq!(field_state(&tree, field).content, "hi");
    }

    #[test]
    fn text_field_selected_text_reads_the_real_selected_range() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);

        assert_eq!(tree.text_field_selected_text(field), Some("he".to_string()));
        assert_eq!(
            field_state(&tree, field).content,
            "hello",
            "a pure read must never mutate the field's own real content"
        );
    }

    #[test]
    fn text_field_selected_text_is_none_with_no_real_selection() {
        let (tree, _root, field) = text_field_scene("hello");
        assert_eq!(
            tree.text_field_selected_text(field),
            None,
            "a freshly created field has no real selection at all"
        );
    }

    #[test]
    fn text_field_selected_text_is_none_for_a_non_text_field_node() {
        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(10.0, 10.0);
        let rect = tree.insert(kind, style, paint);
        assert_eq!(tree.text_field_selected_text(rect), None);
    }

    #[test]
    fn cut_text_field_selection_reads_and_deletes_the_real_selection() {
        let (mut tree, root, field) = text_field_scene("hello");
        dispatch_key(&mut tree, root, Key::Home);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);
        dispatch_shift_key(&mut tree, root, Key::ArrowRight);

        let cut = tree.cut_text_field_selection(field);
        assert_eq!(cut, Some("he".to_string()));
        let state = field_state(&tree, field);
        assert_eq!(
            state.content, "llo",
            "the real selected range must actually be removed, not just read"
        );
        assert_eq!(state.cursor, 0);
        assert_eq!(state.selection_anchor, None);
    }

    #[test]
    fn cut_text_field_selection_with_no_real_selection_is_a_true_no_op() {
        let (mut tree, _root, field) = text_field_scene("hello");
        assert_eq!(tree.cut_text_field_selection(field), None);
        assert_eq!(field_state(&tree, field).content, "hello");
    }

    fn dispatch_ime_preedit(tree: &mut Tree, root: NodeId, text: &str) -> DispatchOutcome {
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        tree.dispatch(
            root,
            InputEvent::ImePreedit(text.to_string()),
            &config,
            Instant::now(),
        )
    }

    #[test]
    fn ime_preedit_sets_the_real_focused_fields_own_preview_without_touching_content() {
        let (mut tree, root, field) = text_field_scene("hi");
        let outcome = dispatch_ime_preedit(&mut tree, root, "n");
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a composition preview is not a real content change"
        );
        let state = field_state(&tree, field);
        assert_eq!(state.preedit, Some("n".to_string()));
        assert_eq!(
            state.content, "hi",
            "a real preedit must never touch committed content"
        );
    }

    #[test]
    fn ime_preedit_with_an_empty_string_clears_a_real_active_preview() {
        let (mut tree, root, field) = text_field_scene("hi");
        dispatch_ime_preedit(&mut tree, root, "n");
        assert_eq!(field_state(&tree, field).preedit, Some("n".to_string()));

        dispatch_ime_preedit(&mut tree, root, "");
        assert_eq!(
            field_state(&tree, field).preedit,
            None,
            "an empty preedit string is winit's own real 'cleared' convention"
        );
    }

    #[test]
    fn ime_preedit_with_no_focused_field_is_a_true_no_op() {
        let mut tree = Tree::new();
        let (_, root_style, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);
        let outcome = dispatch_ime_preedit(&mut tree, root, "n");
        assert_eq!(outcome, DispatchOutcome::None);
    }

    #[test]
    fn a_real_text_input_commit_clears_a_stale_preedit() {
        let (mut tree, root, field) = text_field_scene("hi");
        dispatch_ime_preedit(&mut tree, root, "n");
        assert_eq!(field_state(&tree, field).preedit, Some("n".to_string()));

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        tree.dispatch(
            root,
            InputEvent::TextInput("\u{5462}".to_string()),
            &config,
            Instant::now(),
        );
        let state = field_state(&tree, field);
        assert_eq!(state.content, "hi\u{5462}");
        assert_eq!(
            state.preedit, None,
            "a real Commit (reaching TextInput) must clear the stale preview"
        );
    }

    /// M18 Phase 1 (§8, §10): a real click-to-focus, scoped to
    /// `TextField` -- `text_field_scene` is deliberately NOT reused here
    /// (it pre-focuses via `set_focus_to`), since this test's own claim
    /// is that a plain `PointerPressed` genuinely does the focusing.
    #[test]
    fn pointer_press_on_a_text_field_moves_focus_there() {
        let mut tree = Tree::new();
        let root_style = Style {
            size: Size {
                width: length(120.0),
                height: length(24.0),
            },
            ..Default::default()
        };
        let (_, _, root_paint) = leaf(0.0, 0.0);
        let root = tree.insert(NodeKind::Container, root_style, root_paint);
        let field = tree.insert(
            NodeKind::TextField(TextFieldState::new("hi", "Roboto", 400.0, 16.0)),
            Style {
                size: Size {
                    width: length(120.0),
                    height: length(24.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, field);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(120.0),
                height: AvailableSpace::Definite(24.0),
            },
        );
        assert_eq!(tree.focused(), None, "must start genuinely unfocused");

        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(10.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            Instant::now(),
        );
        assert_eq!(
            tree.focused(),
            Some(field),
            "a real click on a TextField must move real focus there, the same as every real \
             desktop text field"
        );
    }

    #[test]
    fn pointer_press_on_a_non_text_field_does_not_move_focus() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(100.0, 100.0);
        let root = tree.insert(k, s, p);
        let config = InteractionConfig {
            hover_opacity: 0.08,
            hover_duration: Duration::from_millis(100),
            focus_ring_opacity: 1.0,
            focus_ring_duration: Duration::from_millis(100),
            ripple_radius: 50.0,
            ripple_opacity: 0.12,
            ripple_duration: Duration::from_millis(300),
        };
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(10.0, 10.0),
                button: PointerButton::Primary,
            },
            &config,
            Instant::now(),
        );
        assert_eq!(
            tree.focused(),
            None,
            "clicking an ordinary Rect must never move focus -- this is scoped to TextField, \
             not a generic click-to-focus for every node kind"
        );
    }

    /// M18 Phase 1 (§8, §11.9, §11.10): `hit_test_local`'s own real
    /// claim -- it must report the SAME local point `paint_node`
    /// painted into, under a real ancestor transform, not just the same
    /// hit `NodeId` `hit_test` already proved (`hit_test_follows_an_
    /// ancestor_translate_transform`, above).
    #[test]
    fn hit_test_local_reports_the_click_in_the_hit_nodes_own_local_space() {
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

        assert_eq!(
            tree.hit_test_local(root, Point::new(25.0, 25.0)),
            Some((chip, Point::new(25.0, 25.0))),
            "with no transform applied, the local point must equal the canvas point"
        );

        // Same real (100, 100) translate `hit_test_follows_an_ancestor_
        // translate_transform` already proves moves `chip` to canvas
        // (100,100)-(150,150) -- a click at canvas (125, 125) must
        // report chip's own LOCAL point as (25, 25), the inverse-
        // transformed coordinate `paint_node` itself painted at, not
        // the raw canvas point.
        tree.get_mut(camera).unwrap().paint.transform.current = Affine::translate((100.0, 100.0));
        assert_eq!(
            tree.hit_test_local(root, Point::new(125.0, 125.0)),
            Some((chip, Point::new(25.0, 25.0))),
        );
    }

    #[test]
    fn set_text_field_cursor_moves_the_cursor_and_clears_a_selection() {
        let (mut tree, _root, field) = text_field_scene("hello");
        {
            let NodeKind::TextField(state) = &mut tree.get_mut(field).unwrap().kind else {
                panic!("expected a TextField node");
            };
            state.selection_anchor = Some(0);
            state.cursor = 5;
        }
        let moved = tree.set_text_field_cursor(field, 2);
        assert!(moved);
        let state = field_state(&tree, field);
        assert_eq!(state.cursor, 2);
        assert_eq!(
            state.selection_anchor, None,
            "a plain click-driven cursor move must collapse any active selection"
        );
    }

    #[test]
    fn set_text_field_cursor_clamps_beyond_content_length() {
        let (mut tree, _root, field) = text_field_scene("hi");
        assert!(tree.set_text_field_cursor(field, 999));
        assert_eq!(field_state(&tree, field).cursor, 2);
    }

    #[test]
    fn set_text_field_cursor_snaps_to_a_real_char_boundary() {
        // "h" + a 3-byte character -- byte offset 2 lands inside it.
        let (mut tree, _root, field) = text_field_scene("h\u{5462}");
        assert!(tree.set_text_field_cursor(field, 2));
        let cursor = field_state(&tree, field).cursor;
        assert!(
            field_state(&tree, field).content.is_char_boundary(cursor),
            "a mid-character offset must snap back to a real char boundary, not corrupt state"
        );
        assert_eq!(
            cursor, 1,
            "must snap DOWN to the boundary before the requested offset"
        );
    }

    #[test]
    fn set_text_field_cursor_on_a_non_text_field_is_a_true_no_op() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(100.0, 100.0);
        let root = tree.insert(k, s, p);
        assert!(!tree.set_text_field_cursor(root, 0));
    }

    #[test]
    fn extend_text_field_selection_seeds_the_anchor_at_the_current_cursor_on_first_move() {
        let (mut tree, _root, field) = text_field_scene("hello");
        tree.set_text_field_cursor(field, 2);
        assert_eq!(field_state(&tree, field).selection_anchor, None);

        let extended = tree.extend_text_field_selection(field, 5);
        assert!(extended);
        let state = field_state(&tree, field);
        assert_eq!(
            state.selection_anchor,
            Some(2),
            "the anchor must seed at wherever the cursor already was, the real click position"
        );
        assert_eq!(state.cursor, 5);
    }

    #[test]
    fn extend_text_field_selection_keeps_growing_without_moving_the_anchor() {
        let (mut tree, _root, field) = text_field_scene("hello world");
        tree.set_text_field_cursor(field, 0);
        tree.extend_text_field_selection(field, 3);
        tree.extend_text_field_selection(field, 7);
        tree.extend_text_field_selection(field, 11);
        let state = field_state(&tree, field);
        assert_eq!(
            state.selection_anchor,
            Some(0),
            "repeated drag-move calls must never re-seed or move the anchor"
        );
        assert_eq!(state.cursor, 11);
    }

    #[test]
    fn extend_text_field_selection_clamps_beyond_content_length() {
        let (mut tree, _root, field) = text_field_scene("hi");
        tree.set_text_field_cursor(field, 0);
        assert!(tree.extend_text_field_selection(field, 999));
        assert_eq!(field_state(&tree, field).cursor, 2);
    }

    #[test]
    fn extend_text_field_selection_on_a_non_text_field_is_a_true_no_op() {
        let mut tree = Tree::new();
        let (k, s, p) = leaf(100.0, 100.0);
        let root = tree.insert(k, s, p);
        assert!(!tree.extend_text_field_selection(root, 0));
    }

    /// M20 Phase 1 (§7.1, §7.3): `set_all_component_tints`'s own real
    /// claim -- a `Checkbox` and a `Slider` both pick up the real
    /// resolved tint on their own distinct fields, and an unrelated
    /// `NodeKind` (a plain `Rect`) is left completely untouched, the
    /// same "only a real match, never a lazily-created capability"
    /// contract `set_all_interaction_tints`'s own test already proves
    /// for a different field.
    #[test]
    fn set_all_component_tints_updates_checkbox_and_slider_and_leaves_others_untouched() {
        let mut tree = Tree::new();
        let checkbox = tree.insert(
            NodeKind::Checkbox(CheckboxState::new(false)),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0xFF), 0.0, 0.0, 1.0),
        );
        let slider = tree.insert(
            NodeKind::Slider(SliderState::new(0.0)),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0xFF), 0.0, 0.0, 1.0),
        );
        // M20 Phase 2 (§7.1, §7.3): TextField's own real sibling case.
        let text_field = tree.insert(
            NodeKind::TextField(TextFieldState::new("hi", "Roboto", 400.0, 16.0)),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0xFF), 0.0, 0.0, 1.0),
        );
        let (kind, style, paint) = leaf(10.0, 10.0);
        let rect = tree.insert(kind, style, paint);

        let real_color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);
        tree.set_all_component_tints(real_color);

        let NodeKind::Checkbox(state) = &tree.get(checkbox).unwrap().kind else {
            panic!("expected a Checkbox node");
        };
        assert_eq!(state.mark_tint, real_color);

        let NodeKind::Slider(state) = &tree.get(slider).unwrap().kind else {
            panic!("expected a Slider node");
        };
        assert_eq!(state.track_tint, real_color);

        let NodeKind::TextField(state) = &tree.get(text_field).unwrap().kind else {
            panic!("expected a TextField node");
        };
        assert_eq!(state.text_tint, real_color);

        assert!(
            matches!(tree.get(rect).unwrap().kind, NodeKind::Rect),
            "an unrelated NodeKind must be left completely untouched"
        );
    }

    /// M22 Phase 1 (§5): `NodeKind::Image` round-trips through
    /// `Tree::insert`/`Tree::get` exactly like every other kind -- no
    /// special-cased storage, the same "common core + per-kind payload"
    /// shape §1 Locked Decisions already establishes.
    #[test]
    fn nodekind_image_round_trips_through_insert_and_get() {
        let mut tree = Tree::new();
        let image_data = peniko::ImageData {
            data: peniko::Blob::from(vec![0xFFu8, 0x00, 0x00, 0xFF]),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 1,
            height: 1,
        };
        let (_, style, paint) = leaf(10.0, 10.0);
        let id = tree.insert(
            NodeKind::Image(ImageState::new(image_data.clone())),
            style,
            paint,
        );

        let NodeKind::Image(state) = &tree.get(id).unwrap().kind else {
            panic!("expected an Image node");
        };
        assert_eq!(state.image, image_data);
    }
}
