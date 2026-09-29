//! Overlays and layers: opening, placing, and dismissing them, and hit-testing past them.

use super::*;

impl Tree {
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
        self.dirty = true;
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

        let top = self.nodes[root].children.len();
        self.attach_at(root, top, content);
        self.overlays.retain(|(open, _)| *open != content);
        self.overlays.push((content, meta));
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
        self.dirty = true;
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
    /// subtree from its own parent (`Tree::detach`, which keeps content
    /// "alive, parentless, ready for `add_child` elsewhere later") and
    /// drops its metadata. Returns `true` if `id` was a real,
    /// currently-open overlay.
    ///
    /// **M10 Phase 1 (§11.3):** detach, not destroy -- the same content
    /// (a context menu, say) is meant to be reopened repeatedly, not
    /// recreated per open. A caller that genuinely wants an overlay's
    /// own content destroyed can still call `Tree::remove` on it
    /// directly afterward.
    pub fn close_overlay(&mut self, id: NodeId) -> bool {
        self.dirty = true;
        let Some(index) = self.overlays.iter().position(|(content, _)| *content == id) else {
            return false;
        };
        self.overlays.remove(index);
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
        self.overlays
            .iter()
            .find(|(content, _)| *content == id)
            .map(|(_, meta)| meta)
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
    pub(super) fn dismiss_overlays_outside(&mut self, point: Point) -> bool {
        // M30 Phase 8 Step 6 (§11.3): a real, confirmed bug submenus
        // found live, not assumed in advance -- a submenu opened
        // *inside* a real parent menu (with a menu-item `Node` as its
        // own anchor) genuinely sits
        // outside the parent menu's own bounds (`open_overlay` always
        // positions content anchor-relative-below, so a submenu grows
        // past whatever real vertical space the parent menu's own
        // bounds occupy). A real click on a submenu item used to
        // dismiss the *parent* menu first -- its own `dismiss_on_
        // outside_click` filter only ever checked the point against
        // that ONE overlay's own bounds, found it genuinely outside,
        // and consumed the click before it ever reached the submenu
        // item at all. The real, minimal fix: a point inside *any*
        // currently-open overlay is never "outside" for the purposes
        // of dismissing a *different* overlay -- the user is still
        // interacting with the real overlay system as a whole. A true
        // no-op for the single-overlay case: with only one overlay open,
        // "inside any overlay" and "inside this overlay" are the
        // identical real condition.
        let inside_any_overlay = self
            .overlays
            .iter()
            .any(|(content, _)| self.hit_test(*content, point).is_some());
        let to_dismiss: Vec<NodeId> = self
            .overlays
            .iter()
            .filter(|(content, meta)| {
                meta.dismiss_on_outside_click
                    && !inside_any_overlay
                    && self.hit_test(*content, point).is_none()
                    && !self.anchor_hit(meta, point)
            })
            .map(|(content, _)| *content)
            .collect();
        let dismissed_any = !to_dismiss.is_empty();
        for content in to_dismiss {
            self.close_overlay(content);
        }
        dismissed_any
    }

    /// M30 Phase 4 Step 1 (§11.3): `dismiss_overlays_outside`'s own
    /// real sibling for the other real half of "outside interaction"
    /// -- a press outside a real modal overlay (`OverlayMeta.modal`)
    /// must never reach the background, whether or not it also
    /// dismisses the overlay. Read-only (unlike `dismiss_overlays_
    /// outside`, which mutates `self.overlays`/detaches content) --
    /// this only ever *reports* whether the press should be consumed;
    /// `dispatch`'s own `PointerPressed` arm is what actually consumes
    /// it, the same real split that arm already has for the dismiss
    /// case.
    pub(super) fn press_blocked_by_modal_overlay(&self, point: Point) -> bool {
        self.overlays.iter().any(|(content, meta)| {
            meta.modal && self.hit_test(*content, point).is_none() && !self.anchor_hit(meta, point)
        })
    }

    /// Whether `point` lands on `meta`'s anchor.
    pub(super) fn anchor_hit(&self, meta: &OverlayMeta, point: Point) -> bool {
        meta.anchor
            .is_some_and(|anchor| self.hit_test(anchor, point).is_some())
    }

    /// M96: a press at `point` outside open layers -- from the top down,
    /// each dismissible layer it misses asks to be dismissed, stopping at
    /// the layer it lands in (so a press inside a submenu leaves its
    /// parent menu open) or at a modal one. Returns whether it asked any,
    /// so the press is consumed, as a legacy outside press is.
    pub(super) fn report_outside_press(&mut self, point: Point) -> bool {
        let mut asked = Vec::new();
        for (content, meta) in self.overlays.iter().rev() {
            if self.hit_test(*content, point).is_some() || self.anchor_hit(meta, point) {
                break;
            }
            if meta.dismissible {
                asked.push(*content);
            }
            if meta.modal {
                break;
            }
        }
        let any = !asked.is_empty();
        self.dismissals.extend(asked);
        any
    }

    /// M96: shows `node` as a layer over `root`'s content -- on top of every
    /// layer already open -- positioned absolutely: against `meta.anchor`
    /// at every layout (`place_layers`), or at its own `x`/`y` without
    /// one. A node attached elsewhere moves. Returns `false`, changing
    /// nothing, if `node` is `root` or one of its ancestors.
    pub fn show_layer(&mut self, root: NodeId, node: NodeId, meta: OverlayMeta) -> bool {
        if self.ancestors(root).any(|id| id == node) {
            return false;
        }
        self.dirty = true;
        self.overlays.retain(|(open, _)| *open != node);
        if let Some(parent) = self.nodes[node].parent {
            self.unlink(parent, node);
        }
        let mut style = self.nodes[node].layout_style.clone();
        style.position = Position::Absolute;
        self.set_layout_style(node, style);
        let top = self.nodes[root].children.len();
        self.attach_at(root, top, node);
        self.overlays.push((node, meta));
        true
    }

    /// M96: closes the layer `node`, detaching it, still alive and
    /// collectible (`detach_collectible`), and returns what it was opened
    /// with -- `restore_focus` for the caller to hand focus back to.
    pub fn hide_layer(&mut self, node: NodeId) -> Option<OverlayMeta> {
        let index = self.overlays.iter().position(|(open, _)| *open == node)?;
        let (_, meta) = self.overlays.remove(index);
        self.detach_collectible(node);
        Some(meta)
    }

    /// M96: whether `id` is an open layer or legacy overlay.
    pub fn is_layer(&self, id: NodeId) -> bool {
        self.is_overlay(id)
    }

    /// M96: the topmost open modal layer, if any.
    pub(super) fn top_modal(&self) -> Option<usize> {
        self.overlays.iter().rposition(|(_, meta)| meta.modal)
    }

    /// M96: the node input at `point` is aimed at -- `hit_test`, except
    /// that an open modal layer blocks everything beneath it, so a point
    /// outside it (and outside every layer above it) hits nothing.
    pub fn hit_test_input(&self, root: NodeId, point: Point) -> Option<NodeId> {
        let Some(modal) = self.top_modal() else {
            return self.hit_test(root, point);
        };
        self.overlays[modal..]
            .iter()
            .rev()
            .find_map(|(content, _)| self.hit_test(*content, point))
    }

    /// M96: the subtree Tab moves through -- the topmost modal layer while
    /// one is open, else the layer holding focus, else `root` without its
    /// layers. Each open layer is its own focus scope (R8).
    pub(super) fn focus_scope(&self, root: NodeId) -> (NodeId, bool) {
        if let Some(modal) = self.top_modal() {
            return (self.overlays[modal].0, false);
        }
        let holding = self
            .focused
            .and_then(|focused| self.ancestors(focused).find(|id| self.is_overlay(*id)));
        match holding {
            Some(layer) => (layer, false),
            None => (root, true),
        }
    }

    /// M96: places every anchored layer against its anchor, preferring
    /// `placement`, flipping to the opposite side when that side lacks
    /// the room and the other has more, then shifting along both axes to
    /// stay inside `root`. Records the side used as `placed`. Returns
    /// whether any layer moved, so layout runs again.
    pub(super) fn place_layers(&mut self, root: NodeId) -> bool {
        let window = {
            let size = self.layout(root).size;
            (f64::from(size.width), f64::from(size.height))
        };
        let mut moved = false;
        for index in 0..self.overlays.len() {
            let (layer, meta) = self.overlays[index];
            let (Some(anchor), Some(preferred)) = (meta.anchor, meta.placement) else {
                continue;
            };
            if self.nodes.get(anchor).is_none() {
                continue;
            }
            let (ax, ay) = self.absolute_position(anchor);
            let anchor_size = self.layout(anchor).size;
            let (aw, ah) = (f64::from(anchor_size.width), f64::from(anchor_size.height));
            let size = self.layout(layer).size;
            let (w, h) = (f64::from(size.width), f64::from(size.height));
            // Room on each side of the anchor.
            let room = |side: Placement| match side {
                Placement::Below => window.1 - (ay + ah),
                Placement::Above => ay,
                Placement::Start => ax,
                Placement::End => window.0 - (ax + aw),
            };
            let needed = |side: Placement| match side {
                Placement::Below | Placement::Above => h,
                Placement::Start | Placement::End => w,
            };
            let side = if room(preferred) < needed(preferred)
                && room(preferred.opposite()) > room(preferred)
            {
                preferred.opposite()
            } else {
                preferred
            };
            let (x, y) = match side {
                Placement::Below => (ax, ay + ah),
                Placement::Above => (ax, ay - h),
                Placement::Start => (ax - w, ay),
                Placement::End => (ax + aw, ay),
            };
            let fit = |at: f64, extent: f64, limit: f64| at.min(limit - extent).max(0.0);
            let (x, y) = (fit(x, w, window.0) as f32, fit(y, h, window.1) as f32);
            let mut style = self.nodes[layer].layout_style.clone();
            let inset = TaffyRect {
                left: length(x),
                top: length(y),
                right: auto(),
                bottom: auto(),
            };
            if style.inset != inset {
                style.inset = inset;
                self.set_layout_style(layer, style);
                moved = true;
            }
            self.overlays[index].1.placed = Some(side);
        }
        moved
    }

    /// M96: the layers an outside press or Escape asked to dismiss since
    /// the last call -- `engine-py` delivers each a `dismiss` event.
    pub fn take_dismissals(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.dismissals)
    }

    /// M10 Phase 1 (§11.3): closes every currently-open overlay whose
    /// own `OverlayMeta.dismiss_on_escape` is true, unconditionally --
    /// `Key::Escape` always means "close it," no position check needed,
    /// unlike outside-click dismissal above.
    pub(super) fn dismiss_escapable_overlays(&mut self) {
        let to_dismiss: Vec<NodeId> = self
            .overlays
            .iter()
            .filter(|(_, meta)| meta.dismiss_on_escape)
            .map(|(content, _)| *content)
            .collect();
        for content in to_dismiss {
            self.close_overlay(content);
        }
        // M96: Escape asks only the topmost dismissible layer.
        if let Some((content, _)) = self
            .overlays
            .iter()
            .rev()
            .find(|(_, meta)| meta.dismissible)
        {
            self.dismissals.push(*content);
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
        self.dirty = true;
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
}
