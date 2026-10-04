//! Hit-testing, hover, press, focus movement, activation, and pointer capture.

use super::*;

impl Tree {
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
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
    /// own_transform` product `engine-render's paint walk` composes
    /// during paint -- if this formula and that one ever diverge,
    /// hit-testing and rendering will disagree about where a node is.
    /// (`absolute_position` composes the same product, via
    /// `composed_transform`.)
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
    /// why it has to match `draw_own`'s exactly. `parent_transform` is
    /// the caller's already-composed transform for `id`'s *parent*.
    pub(super) fn hit_test_at(
        &self,
        id: NodeId,
        point: Point,
        parent_transform: Affine,
    ) -> Option<(NodeId, Point)> {
        let node = self.nodes.get(id)?;
        if !node.visible {
            return None;
        }
        let layout = self.layout(id);
        let (sx, sy) = self.scroll_shift(id);
        let composed = parent_transform
            * Affine::translate((
                f64::from(layout.location.x) + sx,
                f64::from(layout.location.y) + sy,
            ))
            * node
                .paint
                .local_transform(f64::from(layout.size.width), f64::from(layout.size.height));

        for &child in self.children_in_paint_order(id).iter().rev() {
            if let Some(hit) = self.hit_test_at(child, point, composed) {
                return Some(hit);
            }
        }

        // Map the caller's canvas-space `point` into this node's local
        // space via the inverse of its composed transform -- both the
        // rect default and any `CustomHitTest` below test against this
        // same local point, exactly the local-space coordinates
        // `draw_own` draws into under the identical `composed`
        // transform (M5 Phase 1).
        let local_point = composed.inverse() * point;
        // 0.5.1 (#53): a canvas's painter coordinates start at its padding,
        // and a custom hit shape is in those coordinates.
        let painter_point = {
            let pad = layout.padding;
            local_point - peniko::kurbo::Vec2::new(f64::from(pad.left), f64::from(pad.top))
        };

        // M30 Phase 5 Step 1 (§5, §7): `Node::hit_testable`'s own real
        // opt-out, checked before the per-`NodeKind` match below --
        // `false` short-circuits straight to "no hit" here exactly the
        // way `NodeKind::Text` already does unconditionally,
        // generalized to any node a caller has explicitly opted out
        // (children were already checked above, so this only ever
        // affects whether *this* node itself claims the point).
        let hit = if !node.hit_testable {
            false
        } else {
            match &node.kind {
                NodeKind::Canvas(state) => match &state.hit_test {
                    Some(CustomHitTest::Circle { cx, cy, radius }) => {
                        (painter_point - Point::new(*cx, *cy)).hypot() <= *radius
                    }
                    Some(CustomHitTest::Path { path, tolerance }) => {
                        path.segments()
                            .map(|seg| seg.nearest(painter_point, 0.1).distance_sq)
                            .fold(f64::INFINITY, f64::min)
                            .sqrt()
                            <= *tolerance
                    }
                    None => rect_contains(layout, local_point),
                },
                // M30 Phase 1 (§5, §7): a bare `Text` label never claims a
                // hit itself -- it's decorative content inside a clickable
                // parent (a button's centered label, sized to fill the
                // button's box, would otherwise sit over the parent and
                // take its clicks as `hit_test`'s target). `TextField` is
                // unaffected, a distinct `NodeKind` with its own real
                // click-to-focus need.
                // 0.5.4 (#112): unless it opted in to being selected, which
                // needs the press to reach it.
                NodeKind::Text(state) => {
                    state.options.selectable && rect_contains(layout, local_point)
                }
                _ => rect_contains(layout, local_point),
            }
        };
        hit.then_some((id, local_point))
    }

    /// The concrete fulfillment of §7.3's own text: "hover needs no new
    /// dispatch mechanism -- it falls out of hit-testing, run every
    /// pointer-move... entirely inside `engine-core`." Hover draws
    /// nothing itself -- a framework styles it from `pointer_enter`/
    /// `pointer_leave` listeners.
    ///
    /// Returns the newly-hovered node (`None` if the pointer left every
    /// hit-testable node). A repeated call with the same result is a
    /// no-op.
    pub fn update_hover(&mut self, root: NodeId, point: Point) -> Option<NodeId> {
        let hit = self.hit_test_input(root, point);
        self.set_hovered(hit)
    }

    /// M94: `update_hover`'s own transition half, split out so
    /// `InputEvent::PointerLeft` can clear hover without a hit-test.
    /// Returns `hit`.
    pub fn set_hovered(&mut self, hit: Option<NodeId>) -> Option<NodeId> {
        self.dirty = true;
        if hit == self.hovered {
            return hit;
        }
        self.hovered = hit;
        hit
    }

    /// M38 Phase 5 (§5, §7): the single real chokepoint every real
    /// `self.pressed` mutation goes through.
    pub(super) fn set_pressed(&mut self, new: Option<(PointerButton, NodeId)>) {
        let old = self.pressed;
        if old.map(|(_, id)| id) == new.map(|(_, id)| id) {
            self.pressed = new;
            return;
        }
        self.pressed = new;
    }

    /// §10's own minimal keyboard focus model: Tab/Shift-Tab moves
    /// `focused` in tree order, wrapping at both ends, over the nodes in
    /// the Tab order (`AccessNodeData::in_tab_order`).
    ///
    /// M55: returns the real `(old, new)` transition, `Some` only on a
    /// genuine change -- `Tree::dispatch`'s own `KeyPressed`/`Key::Tab`
    /// arm reads it to produce a real `DispatchOutcome::FocusChanged`.
    pub fn move_focus(
        &mut self,
        root: NodeId,
        direction: FocusDirection,
    ) -> Option<(Option<NodeId>, Option<NodeId>)> {
        self.dirty = true;
        let mut order = Vec::new();
        let (scope, skip_layers) = self.focus_scope(root);
        self.collect_interactive(scope, skip_layers, &mut order);
        // M94: positive `tab_index` values first, ascending; then the
        // rest in tree order (the sort is stable).
        order.sort_by_key(|&id| match self.nodes[id].access.tab_index {
            index if index > 0 => (0, index),
            _ => (1, 0),
        });

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
        self.transition_focus(new)
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
    /// M55 (§10, §16.2): the return, widened from `()`, is the real
    /// `(old, new)` transition -- `Some` only on a genuine change,
    /// `None` when `node` was already focused or doesn't exist (the
    /// same "no real fact to report" contract `HoverChanged` already
    /// has). Every real caller here and in `engine-py` (the one real
    /// non-`Tree::dispatch` mutation path: AccessKit's `Action::Focus`,
    /// `app.rs`) reads it to fire a real `FocusEnter`/`FocusExit` --
    /// `Tree::dispatch`'s own callers get theirs a different way, via
    /// `DispatchOutcome::FocusChanged`, since this method's own return
    /// only reaches a direct caller, not `dispatch`'s own outcome.
    pub fn set_focus_to(&mut self, node: NodeId) -> Option<(Option<NodeId>, Option<NodeId>)> {
        self.dirty = true;
        if !self.nodes.contains_key(node) {
            return None;
        }
        self.transition_focus(Some(node))
    }

    /// M94: `set_focus_to`'s counterpart that leaves nothing focused --
    /// how a simulated `blur` clears focus through the same transition
    /// (and `(old, new)` report) as every other focus change.
    pub fn clear_focus(&mut self) -> Option<(Option<NodeId>, Option<NodeId>)> {
        self.dirty = true;
        self.transition_focus(None)
    }

    /// Shared by `move_focus`/`set_focus_to`: animates the previously-
    /// focused node's `focus_ring` out and the newly-focused one's in,
    /// opt-in-only (Design Principle 6) exactly like `update_hover`
    /// animates `hover_opacity` -- factored out once a second real
    /// caller needed the identical transition logic, not duplicated.
    ///
    /// M55: returns the real `(old, new)` transition, `Some` only when
    /// `old != new` -- the one real chokepoint both `move_focus`/
    /// `set_focus_to` already shared, so this is also the one real
    /// place to compute it, mirroring `update_hover`'s own analogous
    /// role for `self.hovered`.
    pub(super) fn transition_focus(
        &mut self,
        new: Option<NodeId>,
    ) -> Option<(Option<NodeId>, Option<NodeId>)> {
        let old = self.focused;
        self.focused = new;
        if old == new {
            return None;
        }
        // 0.4.2 M12 (issue #24): a newly focused node is scrolled into
        // view, however focus got there -- Tab, a click, `focus()`, or
        // assistive technology.
        if let Some(node) = new {
            self.scroll_into_view(node);
        }
        Some((old, new))
    }

    /// M94: `node` or its nearest ancestor that a framework marked
    /// `focusable`.
    pub(super) fn focusable_ancestor(&self, node: NodeId) -> Option<NodeId> {
        let mut current = Some(node);
        while let Some(id) = current {
            let n = self.nodes.get(id)?;
            if n.access.focusable == Some(true) {
                return Some(id);
            }
            current = n.parent;
        }
        None
    }

    /// Pre-order walk collecting every node whose `access.actions` is
    /// non-empty, in tree order -- `move_focus`'s own Tab-order.
    pub(super) fn collect_interactive(&self, id: NodeId, skip_layers: bool, out: &mut Vec<NodeId>) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        if !node.visible {
            return;
        }
        if node.access.in_tab_order() {
            out.push(id);
        }
        for &child in &node.children {
            if !(skip_layers && self.is_overlay(child)) {
                self.collect_interactive(child, skip_layers, out);
            }
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

    /// M94: the node currently holding pointer capture, if any
    /// (`set_pointer_capture`).
    pub fn pointer_capture(&self) -> Option<NodeId> {
        self.pointer_capture
    }

    /// M94: routes every later pointer event to `node` until it is
    /// released -- by `None` here, by the pointer button's release
    /// (`engine-py`'s router releases it after delivering `pointer_up`),
    /// or by `node` leaving the tree. Only Python listener routing reads
    /// it; the engine's own built-in widget dispatch is unaffected.
    pub fn set_pointer_capture(&mut self, node: Option<NodeId>) {
        self.pointer_capture = node.filter(|&id| self.nodes.contains_key(id));
    }

    /// 0.5.0 M3 (issue #28): ends the current press without a click and
    /// releases any pointer capture -- for a press that started a window
    /// move or resize, whose release the platform may never deliver.
    /// Returns the node that was pressed.
    pub fn cancel_press(&mut self) -> Option<NodeId> {
        let pressed = self.pressed.map(|(_, id)| id);
        self.set_pressed(None);
        self.pointer_capture = None;
        pressed
    }
}
