//! The accessibility tree (§10) built from the node tree.

use super::*;

impl Tree {
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

    /// 0.5.4 (#102): [`build_access_update`](Self::build_access_update) with
    /// every node's bounds multiplied by `scale`: layout is in logical
    /// pixels, and assistive technology wants the window's physical ones.
    pub fn build_access_update_scaled(&self, root: NodeId, scale: f64) -> accesskit::TreeUpdate {
        let mut update = self.build_access_update(root);
        if scale != 1.0 {
            for (_, node) in &mut update.nodes {
                if let Some(b) = node.bounds() {
                    node.set_bounds(accesskit::Rect {
                        x0: b.x0 * scale,
                        y0: b.y0 * scale,
                        x1: b.x1 * scale,
                        y1: b.y1 * scale,
                    });
                }
            }
        }
        update
    }

    pub(super) fn collect_access_nodes(
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
        let (sx, sy) = self.scroll_shift(id);
        let x = offset_x + f64::from(layout.location.x) + sx;
        let y = offset_y + f64::from(layout.location.y) + sy;
        let w = f64::from(layout.size.width);
        let h = f64::from(layout.size.height);

        let mut access_node = accesskit::Node::new(node.access.role);
        if let Some(label) = &node.access.label {
            access_node.set_label(label.clone());
        }
        if let Some(description) = &node.access.description {
            access_node.set_description(description.clone());
        }
        for action in node.access.offered_actions() {
            access_node.add_action(action);
        }
        if node.access.states.disabled {
            access_node.set_disabled();
        }
        // M15 Phase 1 (§5, §16.7): the same real, automatic derivation
        // -- `content` is the one real source of truth, never mirrored
        // into a second copy here.
        if let NodeKind::TextField(state) = &node.kind {
            access_node.set_value(state.content.clone());
        }
        // M94: what a framework set explicitly -- applied last, so it
        // wins over anything derived from a built-in kind above.
        let access = &node.access;
        match &access.value {
            Some(crate::access::AccessValue::Text(text)) => access_node.set_value(text.clone()),
            Some(crate::access::AccessValue::Number(n)) => access_node.set_numeric_value(*n),
            None => {}
        }
        if let Some(min) = access.value_min {
            access_node.set_min_numeric_value(min);
        }
        if let Some(max) = access.value_max {
            access_node.set_max_numeric_value(max);
        }
        if let Some(step) = access.value_step {
            access_node.set_numeric_value_step(step);
        }
        if let Some(checked) = access.checked {
            access_node.set_toggled(checked.into());
        }
        if let Some(selected) = access.selected {
            access_node.set_selected(selected);
        }
        if let Some(expanded) = access.expanded {
            access_node.set_expanded(expanded);
        }
        if let Some(level) = access.level {
            access_node.set_level(level);
        }
        if let Some(live) = access.live {
            access_node.set_live(live);
        }
        if access.hidden {
            access_node.set_hidden();
        }
        access_node.set_bounds(accesskit::Rect {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        });
        let visible_children = || {
            node.children
                .iter()
                .copied()
                .filter(|&child| self.nodes.get(child).is_some_and(|c| c.visible))
        };
        access_node.set_children(visible_children().map(to_access_id).collect::<Vec<_>>());

        out.push((to_access_id(id), access_node));

        for child in visible_children() {
            self.collect_access_nodes(child, x, y, out);
        }
    }
}
