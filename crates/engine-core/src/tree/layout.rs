//! Layout: computing it (with scroll views and virtual lists synced first), reading it, and composing each node's transform.

use super::*;

/// 0.5.3 (#98): the style `taffy` lays a node out with.
///
/// `taffy` rounds every box to whole pixels, so a text node given
/// `width=66.43` (what `measure_text` says its text needs) was laid out 66
/// wide, and the text, which needs 66.43, wrapped. A text node's explicit
/// `width`, `min_width` and `max_width` are therefore rounded **up** before
/// `taffy` sees them: a width the text fits in still fits after the rounding.
/// The node's own `layout_style` keeps the value that was set (so `get`
/// reads it back exactly); only the laid-out size is the whole pixel above.
/// Percentages, `auto` and every other kind are untouched, and so are
/// heights (a text node's height doesn't wrap anything).
pub(super) fn laid_out_style(kind: &NodeKind, style: &Style) -> Style {
    if !matches!(kind, NodeKind::Text(_)) {
        return style.clone();
    }
    fn up(dimension: taffy::style::Dimension) -> taffy::style::Dimension {
        match dimension.expand() {
            taffy::style::ExpandedDimension::Length(v) if v.is_finite() && v > 0.0 => {
                taffy::style::Dimension::length(v.ceil())
            }
            _ => dimension,
        }
    }
    // `min_size`/`max_size` are `LengthPercentageAuto`, `size` a `Dimension`.
    fn up_limit(limit: taffy::style::LengthPercentageAuto) -> taffy::style::LengthPercentageAuto {
        match limit.expand() {
            taffy::style::ExpandedLengthPercentageAuto::Length(v) if v.is_finite() && v > 0.0 => {
                taffy::style::LengthPercentageAuto::length(v.ceil())
            }
            _ => limit,
        }
    }
    let mut out = style.clone();
    out.size.width = up(out.size.width);
    out.min_size.width = up_limit(out.min_size.width);
    out.max_size.width = up_limit(out.max_size.width);
    out
}

/// 0.5.6.2 (#178): a watched node's size change, `(node, old, new)` with `old`
/// `None` the first time.
pub type LayoutChange = (NodeId, Option<(f32, f32)>, (f32, f32));

/// 0.5.6.2 (#178): resolve-and-lay-out-again passes a calc size gets before
/// `settle_calc_sizes` gives up on it.
const CALC_PASSES: usize = 4;

/// The size a percentage of `parent` is taken of, as taffy takes it for a plain
/// `"50%"`: the parent's content box for a node in the flow, its padding box for
/// an absolute one (measured: a 300x200 parent with `padding=10` gives 280x180
/// and 300x200).
fn calc_basis(parent: &Layout, position: taffy::style::Position) -> (f32, f32) {
    let (border, padding) = (parent.border, parent.padding);
    let (mut width, mut height) = (
        parent.size.width - border.left - border.right,
        parent.size.height - border.top - border.bottom,
    );
    if position != taffy::style::Position::Absolute {
        width -= padding.left + padding.right + parent.scrollbar_size.width;
        height -= padding.top + padding.bottom + parent.scrollbar_size.height;
    }
    (width.max(0.0), height.max(0.0))
}

/// Whether two sizes are the same length, to a hundredth of a pixel.
fn same_length(a: taffy::style::Dimension, b: taffy::style::Dimension) -> bool {
    use taffy::style::ExpandedDimension::Length;
    matches!((a.expand(), b.expand()), (Length(a), Length(b)) if (a - b).abs() < 0.01)
}

/// 0.5.6.2 (#176, #177): measures `state`'s text, wrapped within `max_width`
/// when given, as `(width, height)` -- what a text shaper's `measure` returns.
/// `Send` so a `Tree` stays one.
pub type TextMeasure = Box<dyn FnMut(&crate::node::TextState, Option<f32>) -> (f32, f32) + Send>;

/// The size taffy gets for a text leaf (`id`) whose sides are `FitContent`: the
/// text's own width and wrapped height, or zero on a side that doesn't fit (a
/// text node has no size of its own). `known` is a side the layout has already
/// decided (from the node's `width`/`height`, or its parent), `available` the
/// space its parent offers.
fn fit_size(
    nodes: &Nodes,
    measure: &mut Option<TextMeasure>,
    id: NodeId,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    let Some(node) = nodes.get(id) else {
        return Size::ZERO;
    };
    let NodeKind::Text(state) = &node.kind else {
        return Size::ZERO;
    };
    let fit = node.fit_content;
    let Some(measure) = measure.as_mut().filter(|_| fit.any()) else {
        return Size::ZERO;
    };
    let width = if !fit.width {
        0.0
    } else if let Some(width) = known.width {
        width
    } else {
        // One line (or one per paragraph), then narrowed to the offered space
        // when the text wraps. Min-content is the longest word: a width of
        // zero breaks at every opportunity.
        let widest = measure(state, None).0.ceil();
        match available.width {
            _ if !state.options.wrap => widest,
            AvailableSpace::MinContent => measure(state, Some(0.0)).0.ceil(),
            AvailableSpace::MaxContent => widest,
            AvailableSpace::Definite(space) => widest.min(space.max(0.0)),
        }
    };
    let height = if !fit.height {
        0.0
    } else if let Some(height) = known.height {
        height
    } else {
        let wrap_at = if fit.width { Some(width) } else { known.width };
        measure(state, wrap_at.filter(|_| state.options.wrap)).1
    };
    Size { width, height }
}

impl Tree {
    /// 0.5.6.2 (#176, #177): sets how text is measured for `FitContent`. A
    /// window installs its text shaper here; without one a fit side is zero.
    pub fn set_text_measure(&mut self, measure: TextMeasure) {
        self.text_measure = Some(measure);
    }

    /// 0.5.6.2 (#176, #177): sets which sides of text node `id` take the text's
    /// own size. Marks the node for layout.
    pub fn set_fit_content(&mut self, id: NodeId, fit: crate::node::FitContent) {
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };
        if node.fit_content == fit {
            return;
        }
        node.fit_content = fit;
        self.relayout(id);
    }

    /// 0.5.6.2 (#176, #177): marks `id` for layout again: its text, font or
    /// fit changed, and a fit side's size depends on all of those.
    pub fn relayout(&mut self, id: NodeId) {
        self.dirty = true;
        if let Some(&taffy_node) = self.taffy_nodes.get(id) {
            let _ = self.taffy.mark_dirty(taffy_node);
        }
    }

    /// One taffy pass over `root`, with text leaves measured through the
    /// `Tree`'s text measure.
    fn run_taffy(&mut self, root: taffy::NodeId, available: Size<AvailableSpace>) {
        let nodes = &self.nodes;
        let measure = &mut self.text_measure;
        self.taffy
            .compute_layout_with_measure(root, available, |inputs, _, id, style| {
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, space| match id {
                        Some(&mut id) => fit_size(nodes, measure, id, known, space),
                        None => Size::ZERO,
                    },
                )
            })
            .expect("compute_layout: taffy layout computation failed");
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
        // 0.5.4 (#105): taffy rounds every node's layout on each call even
        // when it computed nothing, which at 3,000 nodes is most of a
        // scrolled frame. A root whose cache is intact, asked the same
        // question as last time, has nothing to redo.
        let asked = (root, available_space);
        let clean =
            self.last_layout == Some(asked) && !self.taffy.dirty(root_taffy).unwrap_or(true);
        if !clean {
            self.run_taffy(root_taffy, available_space);
            self.last_layout = Some(asked);
            self.layout_epoch += 1;
        }
        // M36 Phase 1 (§5, §7, §11.7): the identical real "container-
        // level state drives one real child's own real layout_style,
        // then taffy runs once more so it actually lands" shape the
        // button-group sync above already establishes, applied
        // to a real, general `ScrollView`'s own scroll offset. A no-op
        // call (`false`) whenever no `NodeKind::ScrollView` exists
        // anywhere in this `Tree` -- every other real `compute_layout`
        // caller pays nothing extra.
        if self.sync_scroll_view_layouts() {
            self.run_taffy(root_taffy, available_space);
            self.layout_epoch += 1;
        }
        // M37 (§5, §7, §11.7): the real fix for the hit-test-after-
        // scroll bug M36's own investigation found in `VirtualList` --
        // the identical real "bake into layout_style, then taffy runs
        // once more" shape every sync above already establishes. A
        // no-op call (`false`) whenever no `NodeKind::VirtualList` has
        // anything materialized yet.
        if self.sync_virtual_list_layouts() {
            self.run_taffy(root_taffy, available_space);
            self.layout_epoch += 1;
        }
        // 0.5.6.2 (#178): sizes that mix units, resolved against their parents' boxes.
        if !self.calc_nodes.is_empty() {
            self.settle_calc_sizes(root_taffy, available_space);
        }
        // M96: anchored layers go where they fit, once their sizes are known.
        if self.place_layers(root) {
            self.run_taffy(root_taffy, available_space);
            self.layout_epoch += 1;
        }
    }

    /// 0.5.6.2 (#178): makes one side of `id`'s size a `calc` (or, with `None`,
    /// an ordinary size again). The style's size on that side is the caller's to
    /// set, to `auto` for a new calc side: layout runs once with it, then
    /// `compute_layout` resolves the calc against the parent's box and writes
    /// the result into the style as pixels.
    pub fn set_calc_size(&mut self, id: NodeId, height: bool, calc: Option<crate::node::CalcLen>) {
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };
        let side = if height {
            &mut node.calc_size.height
        } else {
            &mut node.calc_size.width
        };
        if *side == calc {
            return;
        }
        *side = calc;
        if calc.is_some() && !self.calc_nodes.contains(&id) {
            self.calc_nodes.push(id);
        }
        self.relayout(id);
    }

    /// 0.5.6.2 (#178): the calc sizes `compute_layout` gave up on since this was
    /// last called, as `(node, is_height)`. A calc side that is still changing
    /// after several resolve-and-lay-out-again passes has no fixed point -- its
    /// parent is sized by it (`child = parent - 16`, `parent = child`) -- so it
    /// is returned to `auto` rather than drifting every frame.
    pub fn take_calc_conflicts(&mut self) -> Vec<(NodeId, bool)> {
        std::mem::take(&mut self.calc_conflicts)
    }

    /// 0.5.6.2 (#178): starts (or, with `false`, stops) reporting `id`'s laid-out
    /// size through `take_layout_changes`. A watched node's first laid-out size
    /// is reported too.
    pub fn watch_layout(&mut self, id: NodeId, on: bool) {
        if !on {
            self.layout_watch.remove(&id);
        } else if self.nodes.get(id).is_some() {
            self.layout_watch.entry(id).or_insert(None);
        }
    }

    /// 0.5.6.2 (#178): every watched node whose laid-out size has changed since
    /// this was last asked, as `(node, old, new)` with `old` `None` the first
    /// time -- by comparison, as `take_scroll_changes` does, so no cause is
    /// missed: a `set`, a parent's resize, a text edit, a calc size resolving.
    /// Reads the last computed layout; nodes that have none yet wait, and nodes
    /// since removed are dropped.
    pub fn take_layout_changes(&mut self) -> Vec<LayoutChange> {
        let mut changes = Vec::new();
        if self.layout_watch.is_empty() {
            return changes;
        }
        self.layout_watch
            .retain(|&id, _| self.nodes.get(id).is_some());
        let mut watched: Vec<NodeId> = self.layout_watch.keys().copied().collect();
        // A fixed order, so two nodes that changed together report the same way
        // every time.
        watched.sort_by_key(|&id| node_id_as_u64(id));
        for id in watched {
            // A node whose layout is stale, or was never computed (taffy hands
            // out a zero-sized placeholder for it), waits for the next layout.
            let Some(layout) = self
                .taffy_nodes
                .get(id)
                .filter(|&&t| !self.taffy.dirty(t).unwrap_or(true))
                .and_then(|&t| self.taffy.layout(t).ok())
            else {
                continue;
            };
            let size = (layout.size.width, layout.size.height);
            if let Some(reported) = self.layout_watch.get_mut(&id)
                && *reported != Some(size)
            {
                changes.push((id, *reported, size));
                *reported = Some(size);
            }
        }
        changes
    }

    /// Resolves every calc size against its parent's laid-out box and lays out
    /// again while any of them changed, up to `CALC_PASSES` times.
    fn settle_calc_sizes(&mut self, root: taffy::NodeId, available: Size<AvailableSpace>) {
        for pass in 0..=CALC_PASSES {
            let changed = self.resolve_calc_sizes();
            if changed.is_empty() {
                return;
            }
            if pass == CALC_PASSES {
                for (id, height) in changed {
                    self.set_calc_size(id, height, None);
                    if let Some(node) = self.nodes.get(id) {
                        let mut style = node.layout_style.clone();
                        if height {
                            style.size.height = taffy::style::Dimension::auto();
                        } else {
                            style.size.width = taffy::style::Dimension::auto();
                        }
                        self.set_layout_style(id, style);
                    }
                    tracing::warn!(
                        side = if height { "height" } else { "width" },
                        "a calc size has no fixed point (its parent is sized by it): left `auto`"
                    );
                    self.calc_conflicts.push((id, height));
                }
            }
            self.run_taffy(root, available);
            self.layout_epoch += 1;
        }
    }

    /// Writes each calc side's current size into its style, from its parent's
    /// last layout; returns the sides that changed.
    fn resolve_calc_sizes(&mut self) -> Vec<(NodeId, bool)> {
        self.calc_nodes
            .retain(|&id| self.nodes.get(id).is_some_and(|n| n.calc_size.any()));
        let mut changed = Vec::new();
        for id in self.calc_nodes.clone() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let Some(parent) = node.parent else {
                continue;
            };
            let Some(parent_layout) = self
                .taffy_nodes
                .get(parent)
                .and_then(|&t| self.taffy.layout(t).ok())
            else {
                continue;
            };
            let basis = calc_basis(parent_layout, node.layout_style.position);
            let mut style = node.layout_style.clone();
            let mut moved = Vec::new();
            for (height, calc) in [(false, node.calc_size.width), (true, node.calc_size.height)] {
                let Some(calc) = calc else {
                    continue;
                };
                let want = taffy::style::Dimension::length(calc.resolve(if height {
                    basis.1
                } else {
                    basis.0
                }));
                let side = if height {
                    &mut style.size.height
                } else {
                    &mut style.size.width
                };
                if !same_length(*side, want) {
                    *side = want;
                    moved.push((id, height));
                }
            }
            if !moved.is_empty() {
                self.set_layout_style(id, style);
                changed.extend(moved);
            }
        }
        changed
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
        let mut changed = false;
        for view in views {
            let Some(max_scroll) = self.max_scroll(view) else {
                continue;
            };
            let child = self.nodes[view].children[0];
            let NodeKind::ScrollView(state) = &self.nodes[view].kind else {
                unreachable!("checked by the filter above")
            };
            // Real, honest re-clamp on every real layout pass, the
            // identical "content may have shrunk since last frame"
            // discipline pyCopper's own `_clamped_scroll` already
            // applies on every real `perform_layout` call -- a hot-
            // reload or app-driven content change that shortens the
            // real scrollable extent must not leave a stale offset
            // pointing past the new real end.
            let clamped = state.scroll.current.clamp(0.0, max_scroll);
            // Only when it moved: a write through `&mut self.nodes[view]` counts
            // the scroll view as touched, which re-walks it and its content.
            if state.scroll.current != clamped
                && let NodeKind::ScrollView(state) = &mut self.nodes[view].kind
            {
                state.scroll.current = clamped;
            }

            // 0.5.4 (#105): the offset is no longer baked into the child's
            // inset; it is a paint-time shift (`scroll_shift`), so a scroll
            // changes no layout and taffy has nothing to redo.
            let mut style = self.nodes[child].layout_style.clone();
            style.position = Position::Absolute;
            style.inset = TaffyRect {
                left: length(0.0),
                top: length(0.0),
                right: auto(),
                bottom: auto(),
            };
            if self.nodes[child].layout_style != style {
                self.set_layout_style(child, style);
                changed = true;
            }
        }
        changed
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
        let mut changed = false;
        for list in lists {
            let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
                unreachable!("checked by the filter above")
            };
            let materialized: Vec<(usize, NodeId)> =
                state.materialized.iter().map(|(&i, &id)| (i, id)).collect();

            for (idx, child) in materialized {
                let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
                    unreachable!("checked by the filter above")
                };
                // 0.5.4 (#105): the row's place in the list, not on screen;
                // the scroll is a paint-time shift (`scroll_shift`).
                let top = state.offset_of(idx);

                let mut style = self.nodes[child].layout_style.clone();
                style.position = Position::Absolute;
                style.inset = TaffyRect {
                    left: length(0.0),
                    top: length(top as f32),
                    right: auto(),
                    bottom: auto(),
                };
                if self.nodes[child].layout_style != style {
                    self.set_layout_style(child, style);
                    changed = true;
                }
            }
        }
        changed
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

    /// 0.5.4 (#105): how far `id`'s parent scrolls it, as the translation to
    /// add to its layout location when painting and hit-testing: the single
    /// child of a `ScrollView` moves by minus the view's offset, and a
    /// `VirtualList`'s rows by minus its scroll. Layout places children as
    /// if unscrolled, so scrolling never invalidates it. Every reader of a
    /// node's position goes through here.
    pub fn scroll_shift(&self, id: NodeId) -> (f64, f64) {
        let Some(node) = self.nodes.get(id) else {
            return (0.0, 0.0);
        };
        let Some(parent) = node.parent else {
            return (0.0, 0.0);
        };
        let (mut dx, mut dy) = match self.nodes.get(parent).map(|n| &n.kind) {
            Some(NodeKind::ScrollView(state)) => {
                let offset = state.scroll.current;
                if state.horizontal {
                    (-offset, 0.0)
                } else {
                    (0.0, -offset)
                }
            }
            Some(NodeKind::VirtualList(state)) => (0.0, -state.scroll_offset.current),
            _ => (0.0, 0.0),
        };
        // 0.5.4 (#139): a sticky node also holds itself in view.
        if let Some(inset) = node.sticky {
            let (sx, sy) = self.sticky_shift(id, inset);
            dx += sx;
            dy += sy;
        }
        (dx, dy)
    }

    /// 0.5.4 (#139): how far sticky node `id` moves, along the nearest scroller's
    /// axis, to stay `inset` pixels inside that scroller's start edge: how far
    /// its natural place (in the scrolled content) has gone past the edge,
    /// no further than keeps it inside its own parent's box -- CSS's `position:
    /// sticky`. `(0, 0)` with no scroller around it, for a direct child of the
    /// scroller (it IS the content), or while it has not reached the edge.
    /// Transforms between the node and the scroller are not accounted for, and a
    /// sticky node inside another sticky one ignores the outer's shift.
    fn sticky_shift(&self, id: NodeId, inset: f64) -> (f64, f64) {
        // The node's place along the axis in the scroller's content, summed up
        // the chain to the scroller.
        let mut chain = Vec::new();
        let mut current = Some(id);
        let (horizontal, offset) = loop {
            let Some(at) = current else {
                return (0.0, 0.0);
            };
            let Some(node) = self.nodes.get(at) else {
                return (0.0, 0.0);
            };
            match &node.kind {
                NodeKind::ScrollView(state) if at != id => {
                    break (state.horizontal, state.scroll.current);
                }
                NodeKind::VirtualList(state) if at != id => {
                    break (false, state.scroll_offset.current);
                }
                _ => {}
            }
            chain.push(at);
            current = node.parent;
        };
        // `chain` runs from the node up to the scroller's direct child.
        if chain.len() < 2 {
            return (0.0, 0.0);
        }
        let axis = |id: NodeId| {
            let layout = self.layout(id);
            if horizontal {
                (f64::from(layout.location.x), f64::from(layout.size.width))
            } else {
                (f64::from(layout.location.y), f64::from(layout.size.height))
            }
        };
        let natural: f64 = chain.iter().map(|&n| axis(n).0).sum();
        let (local, extent) = axis(id);
        let parent_extent = axis(chain[1]).1;
        let room = (parent_extent - (local + extent)).max(0.0);
        let shift = (offset + inset - natural).clamp(0.0, room);
        if horizontal {
            (shift, 0.0)
        } else {
            (0.0, shift)
        }
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
            let (sx, sy) = self.scroll_shift(node_id);
            composed = composed
                * Affine::translate((
                    f64::from(layout.location.x) + sx,
                    f64::from(layout.location.y) + sy,
                ))
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
        let node = self
            .nodes
            .get_mut(id)
            .expect("set_layout_style: NodeId not found in this Tree");
        self.taffy
            .set_style(taffy_node, laid_out_style(&node.kind, &style))
            .expect("set_layout_style: taffy rejected the style update");
        node.layout_style = style;
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
