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
        let mut children: Vec<accesskit::NodeId> = visible_children().map(to_access_id).collect();
        // 0.5.4 (#151): text with a selection or links is a text container.
        let mut text_nodes = Vec::new();
        if let NodeKind::Text(state) = &node.kind
            && !node.access.hidden
            && exposes_text(state)
        {
            children = self.text_access(id, state, (x, y, w, h), &mut access_node, &mut text_nodes);
        }
        access_node.set_children(children);

        out.push((to_access_id(id), access_node));
        out.append(&mut text_nodes);

        for child in visible_children() {
            self.collect_access_nodes(child, x, y, out);
        }
    }
}

/// Bit 63 marks the ids of the accessibility nodes a text node makes for
/// itself (its runs and links): a real node's id has a slot version there,
/// which never gets that large.
const TEXT_PART: u64 = 1 << 63;
const OWNER_MASK: u64 = 0x7FFF_FFFF_FFFF;

/// The id of part `part` of text node `owner`'s accessibility subtree.
fn text_part_id(owner: NodeId, part: usize) -> accesskit::NodeId {
    accesskit::NodeId(
        TEXT_PART | ((to_access_id(owner).0 & OWNER_MASK) << 16) | (part as u64 & 0xFFFF),
    )
}

/// Whether a text node needs the text-container treatment: something to
/// select, or links to follow.
fn exposes_text(state: &crate::TextState) -> bool {
    state.options.selectable
        || state.options.selection.is_some()
        || state.options.spans.iter().any(|s| s.link.is_some())
}

/// A text node's content cut at link boundaries: `(start, end, link)` byte
/// ranges that cover it, each either wholly inside one link or outside any.
fn text_segments(state: &crate::TextState) -> Vec<(usize, usize, Option<&str>)> {
    let content = &state.content;
    let snap = |offset: usize| Tree::char_boundary(content, offset);
    let mut cuts = vec![0, content.len()];
    for span in state.options.spans.iter().filter(|s| s.link.is_some()) {
        cuts.push(snap(span.start));
        cuts.push(snap(span.end));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let link_at = |offset: usize| {
        state
            .options
            .spans
            .iter()
            .rev()
            .find(|s| s.link.is_some() && snap(s.start) <= offset && offset < snap(s.end))
            .and_then(|s| s.link.as_deref())
    };
    let mut out: Vec<(usize, usize, Option<&str>)> = Vec::new();
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let link = link_at(start);
        match out.last_mut() {
            Some(last) if last.2 == link => last.1 = end,
            _ => out.push((start, end, link)),
        }
    }
    out
}

/// What a synthetic accessibility id of a text node stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum TextPart {
    /// A run of text: bytes `start..end` of the node's content.
    Run {
        owner: NodeId,
        start: usize,
        end: usize,
    },
    /// A link: its `href` and bytes.
    Link {
        owner: NodeId,
        href: String,
        start: usize,
        end: usize,
    },
}

impl Tree {
    /// Builds the container's own properties and its run and link nodes.
    fn text_access(
        &self,
        id: NodeId,
        state: &crate::TextState,
        (x, y, w, h): (f64, f64, f64, f64),
        container: &mut accesskit::Node,
        out: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) -> Vec<accesskit::NodeId> {
        if container.role() == accesskit::Role::Unknown {
            container.set_role(accesskit::Role::Label);
        }
        let bounds = accesskit::Rect {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        };
        // (byte range, run id) per run, to place the selection.
        let mut runs: Vec<(usize, usize, accesskit::NodeId)> = Vec::new();
        let mut top = Vec::new();
        for (index, (start, end, link)) in text_segments(state).into_iter().enumerate() {
            let run_id = text_part_id(id, index * 2);
            let text = &state.content[start..end];
            let mut run = accesskit::Node::new(accesskit::Role::TextRun);
            run.set_value(text);
            run.set_character_lengths(
                text.chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<u8>>(),
            );
            run.set_bounds(bounds);
            run.set_text_direction(accesskit::TextDirection::LeftToRight);
            runs.push((start, end, run_id));
            match link {
                Some(href) => {
                    let link_id = text_part_id(id, index * 2 + 1);
                    let mut node = accesskit::Node::new(accesskit::Role::Link);
                    node.set_label(text);
                    node.set_url(href);
                    node.add_action(accesskit::Action::Click);
                    node.set_bounds(bounds);
                    node.set_children(vec![run_id]);
                    out.push((link_id, node));
                    out.push((run_id, run));
                    top.push(link_id);
                }
                None => {
                    out.push((run_id, run));
                    top.push(run_id);
                }
            }
        }
        if let Some((a, b)) = state.options.selection {
            let position = |offset: usize| {
                let offset = Tree::char_boundary(&state.content, offset);
                let (start, _, run) = runs
                    .iter()
                    .find(|(s, e, _)| *s <= offset && offset < *e)
                    .or_else(|| runs.last())?;
                Some(accesskit::TextPosition {
                    node: *run,
                    character_index: state.content[*start..offset.max(*start)].chars().count(),
                })
            };
            if let (Some(anchor), Some(focus)) = (position(a), position(b)) {
                container.set_text_selection(accesskit::TextSelection { anchor, focus });
            }
        }
        if state.options.selectable {
            container.add_action(accesskit::Action::SetTextSelection);
        }
        top
    }

    /// What the synthetic accessibility id `id` stands for, if it is the run
    /// or link of one of this tree's text nodes (0.5.4, #151).
    pub fn resolve_text_part(&self, id: accesskit::NodeId) -> Option<TextPart> {
        if id.0 & TEXT_PART == 0 {
            return None;
        }
        let key = (id.0 >> 16) & OWNER_MASK;
        let part = (id.0 & 0xFFFF) as usize;
        let (owner, node) = self.nodes.iter().find(|(nid, node)| {
            matches!(&node.kind, NodeKind::Text(s) if exposes_text(s))
                && to_access_id(*nid).0 & OWNER_MASK == key
        })?;
        let NodeKind::Text(state) = &node.kind else {
            return None;
        };
        let (start, end, link) = *text_segments(state).get(part / 2)?;
        match (part % 2, link) {
            (0, _) => Some(TextPart::Run { owner, start, end }),
            (_, Some(href)) => Some(TextPart::Link {
                owner,
                href: href.to_owned(),
                start,
                end,
            }),
            _ => None,
        }
    }

    /// A screen reader's `SetTextSelection`: selects the range it names in
    /// the one text node both ends are in. `false` if it names anything else.
    pub fn set_text_selection_from_access(&mut self, selection: &accesskit::TextSelection) -> bool {
        let locate = |tree: &Tree, position: &accesskit::TextPosition| {
            let TextPart::Run { owner, start, end } = tree.resolve_text_part(position.node)? else {
                return None;
            };
            let NodeKind::Text(state) = &tree.nodes.get(owner)?.kind else {
                return None;
            };
            let text = &state.content[start..end];
            let offset = text
                .char_indices()
                .nth(position.character_index)
                .map_or(text.len(), |(i, _)| i);
            Some((owner, start + offset))
        };
        let (Some((a_owner, anchor)), Some((f_owner, focus))) = (
            locate(self, &selection.anchor),
            locate(self, &selection.focus),
        ) else {
            return false;
        };
        self.select_across((a_owner, anchor), (f_owner, focus))
    }
}
