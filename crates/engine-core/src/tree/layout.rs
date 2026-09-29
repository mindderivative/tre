//! Layout: computing it (with scroll views and virtual lists synced first), reading it, and composing each node's transform.

use super::*;

impl Tree {
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
        // M36 Phase 1 (§5, §7, §11.7): the identical real "container-
        // level state drives one real child's own real layout_style,
        // then taffy runs once more so it actually lands" shape the
        // button-group sync above already establishes, applied
        // to a real, general `ScrollView`'s own scroll offset. A no-op
        // call (`false`) whenever no `NodeKind::ScrollView` exists
        // anywhere in this `Tree` -- every other real `compute_layout`
        // caller pays nothing extra.
        if self.sync_scroll_view_layouts() {
            self.taffy
                .compute_layout(root_taffy, available_space)
                .expect("compute_layout: taffy layout computation failed (scroll view sync pass)");
        }
        // M37 (§5, §7, §11.7): the real fix for the hit-test-after-
        // scroll bug M36's own investigation found in `VirtualList` --
        // the identical real "bake into layout_style, then taffy runs
        // once more" shape every sync above already establishes. A
        // no-op call (`false`) whenever no `NodeKind::VirtualList` has
        // anything materialized yet.
        if self.sync_virtual_list_layouts() {
            self.taffy
                .compute_layout(root_taffy, available_space)
                .expect("compute_layout: taffy layout computation failed (virtual list sync pass)");
        }
        // M96: anchored layers go where they fit, once their sizes are known.
        if self.place_layers(root) {
            self.taffy
                .compute_layout(root_taffy, available_space)
                .expect("compute_layout: taffy layout computation failed (layer placement pass)");
        }
    }

    /// M36 Phase 1 (§5, §7, §11.7): the real, general scrollable-
    /// viewport mechanism, grounded directly in the sibling `pyCopper`
    /// project's own `ScrollViewElement.perform_layout`/`child_origin`.
    /// Unlike `VirtualList`'s own separate paint-time-only translate
    /// (before M37), bakes
    /// the one real child's own current scroll-shifted position
    /// directly into `layout_style` every frame, so `Tree::hit_test_at`
    /// (which reads `self.layout(child)`, not a second paint-only
    /// transform) agrees with `engine-render's paint walk` by
    /// construction -- this phase's own real investigation found that
    /// guarantee does *not* hold for `VirtualList` today (a real, pre-
    /// existing, separate bug this milestone surfaces but does not
    /// fix). A `ScrollView` with no real children yet, or whose one
    /// real child was just removed, is a true no-op for that node --
    /// nothing to scroll.
    pub(super) fn sync_scroll_view_layouts(&mut self) -> bool {
        // M65 (§5, §6): the real O(1) check -- the full scan below now
        // only ever runs when at least one real `ScrollView` exists.
        if self.scroll_view_count == 0 {
            return false;
        }
        let views: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, node)| matches!(node.kind, NodeKind::ScrollView(_)))
            .map(|(id, _)| id)
            .collect();
        for view in views {
            let Some(&child) = self.nodes[view].children.first() else {
                continue;
            };

            let outer = self.layout(view);
            let viewport_w = f64::from(outer.size.width);
            let viewport_h = f64::from(outer.size.height);
            let child_layout = self.layout(child);
            let content_w = f64::from(child_layout.size.width);
            let content_h = f64::from(child_layout.size.height);

            let NodeKind::ScrollView(state) = &self.nodes[view].kind else {
                unreachable!("checked by the filter above")
            };
            let horizontal = state.horizontal;
            let max_scroll = if horizontal {
                (content_w - viewport_w).max(0.0)
            } else {
                (content_h - viewport_h).max(0.0)
            };
            // Real, honest re-clamp on every real layout pass, the
            // identical "content may have shrunk since last frame"
            // discipline pyCopper's own `_clamped_scroll` already
            // applies on every real `perform_layout` call -- a hot-
            // reload or app-driven content change that shortens the
            // real scrollable extent must not leave a stale offset
            // pointing past the new real end.
            let clamped = state.scroll.current.clamp(0.0, max_scroll);
            if let NodeKind::ScrollView(state) = &mut self.nodes[view].kind {
                state.scroll.current = clamped;
            }

            let mut style = self.nodes[child].layout_style.clone();
            style.position = Position::Absolute;
            style.inset = if horizontal {
                TaffyRect {
                    left: length(-clamped as f32),
                    top: length(0.0),
                    right: auto(),
                    bottom: auto(),
                }
            } else {
                TaffyRect {
                    left: length(0.0),
                    top: length(-clamped as f32),
                    right: auto(),
                    bottom: auto(),
                }
            };
            self.set_layout_style(child, style);
        }
        true
    }

    /// M37 (§5, §7, §11.7): the real fix for a genuine, previously
    /// undiscovered bug M36's own investigation found (documented in
    /// `BUILD_TRACKER_ARCHIVE_M1-M50.md`, M36): `VirtualList`'s real
    /// scroll offset used to be applied *only* as an extra
    /// `engine-render's paint walk` translate, never reflected back
    /// into `layout_style` -- so a real point-based hit-test at a
    /// materialized item's own genuine post-scroll screen position
    /// resolved to the *wrong* item, silently never caught because the
    /// synthetic click helper of the time computed its target from the
    /// identical stale, pre-scroll `self.layout(node)` `Tree::
    /// hit_test_at` itself reads, so the two coincidentally agreed
    /// without either reflecting the real, live, post-scroll visual
    /// position. Mirrors `sync_scroll_view_layouts`'s own exact
    /// bug-free shape: bakes each real
    /// materialized child's own current scroll-adjusted position
    /// directly into `layout_style.inset.top` every frame, which both
    /// `engine-render's paint walk` and `Tree::hit_test_at` now read
    /// correctly, by construction, with zero second, separate
    /// transform either has to independently agree with. A `VirtualList`
    /// with nothing materialized yet is a true no-op.
    pub(super) fn sync_virtual_list_layouts(&mut self) -> bool {
        // M65 (§5, §6): the real O(1) check -- the full scan below now
        // only ever runs when at least one real `VirtualList` exists.
        if self.virtual_list_count == 0 {
            return false;
        }
        let lists: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, node)| matches!(node.kind, NodeKind::VirtualList(_)))
            .map(|(id, _)| id)
            .collect();
        for list in lists {
            let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
                unreachable!("checked by the filter above")
            };
            let scroll = state.scroll_offset.current;
            let materialized: Vec<(usize, NodeId)> =
                state.materialized.iter().map(|(&i, &id)| (i, id)).collect();

            for (idx, child) in materialized {
                let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
                    unreachable!("checked by the filter above")
                };
                let top = state.offset_of(idx) - scroll;

                let mut style = self.nodes[child].layout_style.clone();
                style.position = Position::Absolute;
                style.inset = TaffyRect {
                    left: length(0.0),
                    top: length(top as f32),
                    right: auto(),
                    bottom: auto(),
                };
                self.set_layout_style(child, style);
            }
        }
        true
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
    /// translate(layout.location) * own_transform` product `draw_own`/
    /// `hit_test_at` already compose (M5 Phase 1/2), not just a pure
    /// accumulated translation. §14 step 13's own real need: `open_overlay`
    /// positions an overlay relative to its anchor's *absolute* bounds,
    /// not the anchor's own parent-relative `Layout::location` -- and
    /// every caller wants the transform-aware answer.
    ///
    /// An `Affine` only composes correctly root-to-node, the opposite
    /// order of the old bottom-up accumulation -- so this collects the
    /// chain from `id` up to the root first, then walks it in reverse.
    /// None of this method's real callers run once per node per frame
    /// (an overlay opens once per interaction, a drag reads this once
    /// per pointer move), so the extra composition cost here is not the
    /// same concern it would be for `draw_own`/`hit_test_at`'s own
    /// per-frame walks.
    pub fn absolute_position(&self, id: NodeId) -> (f64, f64) {
        let p = self.composed_transform(id) * Point::ORIGIN;
        (p.x, p.y)
    }

    /// The root-to-`id` composition of every node's layout offset and own
    /// transform -- shared by `absolute_position` and `window_to_local`.
    pub(super) fn composed_transform(&self, id: NodeId) -> Affine {
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
                * node
                    .paint
                    .local_transform(f64::from(layout.size.width), f64::from(layout.size.height));
        }
        composed
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
        self.dirty = true;
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

    /// M94: maps a window-space point into `id`'s own local space, through
    /// the same composed layout-and-transform chain `hit_test_at` and
    /// `draw_own` use -- how a bubbling pointer event reports `x`/`y`
    /// relative to each node its listeners run on. Needs a computed layout.
    pub fn window_to_local(&self, id: NodeId, point: Point) -> Point {
        self.composed_transform(id).inverse() * point
    }

    /// M94: `window_to_local`'s inverse -- a point in `id`'s own local
    /// space, in window space.
    pub fn local_to_window(&self, id: NodeId, point: Point) -> Point {
        self.composed_transform(id) * point
    }
}
