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

/// One shaped line of a text node, for its accessibility runs (0.5.4, #153).
/// The text renderer makes these; the tree has no text layout of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessLine {
    /// The bytes of the node's content on this line.
    pub start: usize,
    pub end: usize,
    /// The line's box in the node's own coordinates.
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    /// For each character of `content[start..end]`: its left edge, measured
    /// from the line's `x0`, and its advance.
    pub chars: Vec<(f32, f32)>,
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

/// A stretch of text that is one accessibility run: inside one link (or none)
/// and on one line.
struct Piece<'a> {
    start: usize,
    end: usize,
    /// Index of the link-boundary segment it came from.
    segment: usize,
    link: Option<&'a str>,
    /// The shaped line it is on, with the lines' own indices.
    line: Option<&'a AccessLine>,
}

/// Cuts a text node into runs: each link-boundary segment further cut at the
/// shaped lines, when there are any that fit the content.
fn text_pieces<'a>(state: &'a crate::TextState, lines: Option<&'a [AccessLine]>) -> Vec<Piece<'a>> {
    let usable = lines.filter(|lines| {
        !lines.is_empty()
            && lines.iter().all(|l| {
                l.start < l.end
                    && l.end <= state.content.len()
                    && state.content.is_char_boundary(l.start)
                    && state.content.is_char_boundary(l.end)
                    && l.chars.len() == state.content[l.start..l.end].chars().count()
            })
    });
    let mut out = Vec::new();
    for (segment, (start, end, link)) in text_segments(state).into_iter().enumerate() {
        match usable {
            None => out.push(Piece {
                start,
                end,
                segment,
                link,
                line: None,
            }),
            Some(lines) => {
                for line in lines {
                    let (a, b) = (start.max(line.start), end.min(line.end));
                    if a < b {
                        out.push(Piece {
                            start: a,
                            end: b,
                            segment,
                            link,
                            line: Some(line),
                        });
                    }
                }
            }
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
        let lines = self.text_lines.get(&id).map(Vec::as_slice);
        let pieces = text_pieces(state, lines);
        // (byte range, run id) per run, to place the selection.
        let mut runs: Vec<(usize, usize, accesskit::NodeId)> = Vec::new();
        let mut top = Vec::new();
        // A link's runs, until its last piece.
        let mut link_runs: Vec<(accesskit::NodeId, accesskit::Rect)> = Vec::new();
        for (index, piece) in pieces.iter().enumerate() {
            let run_id = text_part_id(id, index * 2);
            let text = &state.content[piece.start..piece.end];
            let mut run = accesskit::Node::new(accesskit::Role::TextRun);
            run.set_value(text);
            run.set_character_lengths(
                text.chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<u8>>(),
            );
            run.set_text_direction(accesskit::TextDirection::LeftToRight);
            let run_bounds = match piece.line {
                Some(line) => {
                    let skip = state.content[line.start..piece.start].chars().count();
                    let chars = &line.chars[skip..skip + text.chars().count()];
                    let base = chars.first().map_or(0.0, |c| c.0);
                    let width = chars.last().map_or(0.0, |c| c.0 + c.1 - base);
                    run.set_character_positions(
                        chars.iter().map(|c| c.0 - base).collect::<Vec<f32>>(),
                    );
                    run.set_character_widths(chars.iter().map(|c| c.1).collect::<Vec<f32>>());
                    accesskit::Rect {
                        x0: x + line.x0 + f64::from(base),
                        y0: y + line.y0,
                        x1: x + line.x0 + f64::from(base) + f64::from(width),
                        y1: y + line.y1,
                    }
                }
                None => bounds,
            };
            run.set_bounds(run_bounds);
            let before = state.content[..piece.start].chars().next_back();
            let starts: Vec<usize> = text
                .char_indices()
                .scan(before, |prev, (i, c)| {
                    let start = !c.is_whitespace() && prev.is_some_and(char::is_whitespace);
                    *prev = Some(c);
                    Some(start.then_some(text[..i].chars().count()))
                })
                .flatten()
                .collect();
            if !starts.is_empty() {
                run.set_word_starts(starts.into_iter().map(|i| i as u8).collect::<Vec<u8>>());
            }
            runs.push((piece.start, piece.end, run_id));
            out.push((run_id, run));
            match piece.link {
                Some(_) => link_runs.push((run_id, run_bounds)),
                None => top.push(run_id),
            }
            // A link node closes after the last piece of its segment.
            let last_of_segment = pieces
                .get(index + 1)
                .is_none_or(|n| n.segment != piece.segment);
            if let (Some(href), true) = (piece.link, last_of_segment) {
                let link_id = text_part_id(id, piece.segment * 2 + 1);
                let (first, last) = (link_runs[0].1, link_runs[link_runs.len() - 1].1);
                let mut node = accesskit::Node::new(accesskit::Role::Link);
                let label_start = pieces
                    .iter()
                    .find(|p| p.segment == piece.segment)
                    .map_or(0, |p| p.start);
                node.set_label(&state.content[label_start..piece.end]);
                node.set_url(href);
                node.add_action(accesskit::Action::Click);
                node.set_bounds(accesskit::Rect {
                    x0: first.x0.min(last.x0),
                    y0: first.y0,
                    x1: first.x1.max(last.x1),
                    y1: last.y1,
                });
                node.set_children(link_runs.iter().map(|r| r.0).collect::<Vec<_>>());
                out.push((link_id, node));
                top.push(link_id);
                link_runs.clear();
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

    /// Hands the tree the shaped lines of its text nodes, for their runs'
    /// bounds and character positions (the text renderer makes them; see
    /// [`AccessLine`]). Replaces any it had, so call it just before
    /// [`build_access_update`](Self::build_access_update); text it has no lines
    /// for gets one run per link segment, over the node's box.
    pub fn set_text_access_lines(
        &mut self,
        lines: std::collections::HashMap<NodeId, Vec<AccessLine>>,
    ) {
        self.text_lines = lines;
    }

    /// The text nodes that are exposed as text containers, which are the ones
    /// [`set_text_access_lines`](Self::set_text_access_lines) wants lines for.
    pub fn text_nodes_needing_access_lines(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|(_, n)| {
                n.visible
                    && !n.access.hidden
                    && matches!(&n.kind, NodeKind::Text(s) if exposes_text(s))
            })
            .map(|(id, _)| id)
            .collect()
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
        let lines = self.text_lines.get(&owner).map(Vec::as_slice);
        if part.is_multiple_of(2) {
            let piece = text_pieces(state, lines).into_iter().nth(part / 2)?;
            return Some(TextPart::Run {
                owner,
                start: piece.start,
                end: piece.end,
            });
        }
        let (start, end, link) = *text_segments(state).get(part / 2)?;
        Some(TextPart::Link {
            owner,
            href: link?.to_owned(),
            start,
            end,
        })
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
