//! 0.5.4 (review): pointer and keyboard interaction with text that needs the
//! text renderer: click-to-position in a text input, drag selection in text and
//! terminals, links, multi-click, Shift+Up/Down, the pointer shape over text, and
//! the shaped lines the accessibility tree wants. Moved out of `app.rs`, where it
//! had grown to a third of the file.

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{Cursor, InputEvent, NodeId, NodeKind, PointerButton, Tree};
use engine_render::{TextPlacement, TextRenderer};
use peniko::kurbo::Point;

/// M18 Phase 1/2 (§8, §10, §11.9, §11.10): the real per-glyph hit-test
/// `engine-core` structurally can't do itself (§4) -- shared by
/// `PointerPressed`'s click-to-position and `PointerMoved`'s drag-
/// extend, both of which need the exact same "is `hit` a `TextField`,
/// and if so what real byte offset does `local_point` land on"
/// answer. Mirrors `draw_own`'s own real `TextPlacement { x: 0.0,
/// y: 0.0, max_width: <the node's own real computed layout width> }`
/// exactly -- a hit-test using different placement values than what
/// was actually painted would resolve to the wrong character.
fn text_field_hit_offset(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
) -> Option<usize> {
    // M38 Phase 7 (§5, §8): no longer clones `state` out of the borrow
    // -- `TextFieldState` stopped deriving `Clone` once it gained a
    // real `Animated<f64>` field (`scroll_offset`), the identical real
    // reason `ScrollViewState` never derived it either. Holds `tree.borrow()` for this whole function's body
    // instead, released when it returns, before either real caller's
    // own subsequent `borrow_mut()`.
    let tree = tree.borrow();
    let node = tree.get(hit)?;
    let NodeKind::TextField(state) = &node.kind else {
        return None;
    };
    // The placement the field is painted at (`engine-render`'s `draw_own`):
    // inside its padding, and shifted by how far a multiline field has
    // scrolled. A click is resolved against what is painted, so it must use
    // the same one; before, it used the node's corner and ignored the scroll.
    let layout = tree.layout(hit);
    let pad = layout.padding;
    let at = TextPlacement {
        x: f64::from(pad.left) - state.horizontal_scroll_offset.current,
        y: f64::from(pad.top) - state.scroll_offset.current,
        max_width: (layout.size.width - pad.left - pad.right).max(0.0),
        color: peniko::Color::TRANSPARENT,
    };
    Some(text_renderer.hit_test_position(state, at, local_point))
}

/// 0.5.4 (#112): `text_field_hit_offset`'s sibling for selectable static text:
/// the byte offset `local_point` lands on in a text node whose `selectable` is
/// on, resolved against the placement `draw_own` paints it at (inside its
/// padding). `None` for anything else.
fn static_text_hit_offset(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
) -> Option<usize> {
    let tree = tree.borrow();
    let node = tree.get(hit)?;
    let NodeKind::Text(state) = &node.kind else {
        return None;
    };
    if !state.options.selectable {
        return None;
    }
    let layout = tree.layout(hit);
    let pad = layout.padding;
    let at = TextPlacement {
        x: f64::from(pad.left),
        y: f64::from(pad.top),
        max_width: (layout.size.width - pad.left - pad.right).max(0.0),
        color: peniko::Color::TRANSPARENT,
    };
    Some(text_renderer.hit_test_text(hit, state, at, local_point))
}

/// M32 Phase 6 (§4, §5, §8): `text_field_hit_offset`'s own real
/// `Terminal` sibling -- turns a real local click/drag point into the
/// exact real `(row, col)` cell it lands on, via `engine-render`'s own
/// `terminal_hit_cell` (real font metrics only that crate has, §4).
/// `None` for anything that isn't a real, present `Terminal`, the
/// identical "not the kind this needs" contract `text_field_hit_offset`
/// already has.
fn terminal_hit_cell(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
) -> Option<(u16, u16)> {
    let tree_ref = tree.borrow();
    match tree_ref.get(hit).map(|n| &n.kind) {
        Some(NodeKind::Terminal(state)) => {
            // The cell grid starts inside the node's padding.
            let pad = tree_ref.layout(hit).padding;
            let inside = Point::new(
                local_point.x - f64::from(pad.left),
                local_point.y - f64::from(pad.top),
            );
            Some(text_renderer.terminal_hit_cell(state, inside))
        }
        _ => None,
    }
}

/// The placement `draw_own` paints a text node's glyphs at (inside its padding).
fn text_placement(tree: &Tree, id: NodeId) -> TextPlacement {
    let layout = tree.layout(id);
    let pad = layout.padding;
    TextPlacement {
        x: f64::from(pad.left),
        y: f64::from(pad.top),
        max_width: (layout.size.width - pad.left - pad.right).max(0.0),
        color: peniko::Color::TRANSPARENT,
    }
}

/// 0.5.4 (review): a press that was taken from the pointer (a cancelled touch,
/// or one that became a pan) starts no drag and clicks no link, whatever its
/// release does afterwards.
pub(crate) fn text_press_cancelled(
    text_drag: &mut Option<NodeId>,
    terminal_drag: &mut Option<NodeId>,
    clicks: &mut TextClicks,
) {
    *text_drag = None;
    *terminal_drag = None;
    clicks.link_press = None;
}

/// 0.5.4 (#131): the link under `local_point` in text node `hit`, if it has
/// link spans and the point is on a character of one.
fn link_under(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
) -> Option<String> {
    let tree = tree.borrow();
    let node = tree.get(hit)?;
    let NodeKind::Text(state) = &node.kind else {
        return None;
    };
    if !state.options.spans.iter().any(|s| s.link.is_some()) {
        return None;
    }
    let at = text_placement(&tree, hit);
    let offset = text_renderer.text_offset_under(hit, state, at, local_point)?;
    tree.text_link_at(hit, offset).map(str::to_owned)
}

/// 0.5.4 (#154): Shift+Up and Shift+Down in static text move the selection's
/// moving end a line, which needs the shaped layout (so it runs here, with the
/// text renderer, not in the tree). At the first or last line the end goes to
/// that line's start or end, and from there on into the neighbouring selectable
/// text. Returns whether it moved the selection.
pub(crate) fn text_key_input(
    event: &InputEvent,
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
) -> bool {
    let InputEvent::KeyPressed {
        key: key @ (engine_core::Key::ArrowUp | engine_core::Key::ArrowDown),
        shift: true,
    } = event
    else {
        return false;
    };
    let forward = *key == engine_core::Key::ArrowDown;
    let (anchor, target) = {
        let tree = tree.borrow();
        if tree
            .focused()
            .is_some_and(|f| matches!(tree.get(f).map(|n| &n.kind), Some(NodeKind::TextField(_))))
        {
            return false;
        }
        let Some((anchor, (owner, focus))) = tree.static_selection_ends() else {
            return false;
        };
        let Some(NodeKind::Text(state)) = tree.get(owner).map(|n| &n.kind) else {
            return false;
        };
        let at = text_placement(&tree, owner);
        let moved = text_renderer.move_focus_lines(
            owner,
            state,
            at,
            if anchor.0 == owner { anchor.1 } else { focus },
            focus,
            if forward { 1 } else { -1 },
        );
        // No line to go to: the text's edge, then the next text.
        let target = if moved == focus {
            tree.static_selection_vertical_edge(forward)
                .unwrap_or((owner, focus))
        } else {
            (owner, moved)
        };
        (anchor, target)
    };
    tree.borrow_mut().select_across(anchor, target)
}

/// 0.5.4 (#153): the shaped lines of every text node the accessibility tree
/// will expose as text, keyed by node.
pub(crate) fn text_access_lines(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
) -> std::collections::HashMap<NodeId, Vec<engine_core::AccessLine>> {
    let tree = tree.borrow();
    tree.text_nodes_needing_access_lines()
        .into_iter()
        .filter_map(|id| {
            let NodeKind::Text(state) = &tree.get(id)?.kind else {
                return None;
            };
            let lines = text_renderer.access_lines(id, state, text_placement(&tree, id));
            Some((id, lines))
        })
        .collect()
}

/// 0.5.4 (#131): the pointer shape text asks for under `position`: a pointer
/// over a link, an I-beam over selectable text, else `None` (the node's own
/// `cursor`, if it set one, is checked first by the caller).
pub(crate) fn text_cursor_at(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    root: NodeId,
    position: Point,
) -> Option<Cursor> {
    let (hit, local) = tree.borrow().hit_test_local(root, position)?;
    let (own, selectable) = {
        let tree = tree.borrow();
        let node = tree.get(hit)?;
        let NodeKind::Text(state) = &node.kind else {
            return None;
        };
        (node.cursor, state.options.selectable)
    };
    if own.is_some() {
        return None;
    }
    if link_under(tree, text_renderer, hit, local).is_some() {
        return Some(Cursor::Pointer);
    }
    selectable.then_some(Cursor::Text)
}

/// A click that landed on a link span (0.5.4, #131).
pub(crate) struct LinkClick {
    pub(crate) node: NodeId,
    pub(crate) href: String,
    pub(crate) position: Point,
}

/// What `text_pointer_input` remembers between events about links and
/// multi-clicks (0.5.4, #131).
#[derive(Default)]
pub(crate) struct TextClicks {
    /// The link a primary press landed on, until its release.
    link_press: Option<(NodeId, String)>,
    /// The last primary press: when and where, and how many in a row.
    last: Option<(std::time::Instant, Point, u8)>,
}

impl TextClicks {
    /// Counts a primary press at `position`: 1, or 2 and 3 for presses in
    /// quick succession on the same spot, then back to 1.
    fn register(&mut self, now: std::time::Instant, position: Point) -> u8 {
        const WINDOW: std::time::Duration = std::time::Duration::from_millis(500);
        const SLOP: f64 = 4.0;
        let count = match self.last {
            Some((at, was, n))
                if now.saturating_duration_since(at) <= WINDOW
                    && (position - was).hypot() <= SLOP =>
            {
                n % 3 + 1
            }
            _ => 1,
        };
        self.last = Some((now, position, count));
        count
    }
}

/// A real pointer press, move, or release over a text field or a terminal:
/// click-to-position, drag-to-select, and the end of a drag. Needs the text
/// renderer's font metrics, which `process_input` doesn't have, so it runs
/// here, after it. Takes the tree, the root, and the two drag trackers
/// rather than the whole window runtime, so a test can drive it without a
/// window.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text_pointer_input(
    event: &InputEvent,
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    text: &mut TextRenderer,
    text_drag: &mut Option<NodeId>,
    terminal_drag: &mut Option<NodeId>,
    clicks: &mut TextClicks,
    now: std::time::Instant,
) -> Option<LinkClick> {
    let mut link_click = None;
    match *event {
        InputEvent::PointerPressed {
            position,
            button: PointerButton::Primary,
        } => {
            // M18 Phase 1 (§8, §10, §11.9, §11.10): widened
            // from `hit_test` to `hit_test_local` -- the
            // extra local-space point is exactly what a
            // real click-to-position hit-test needs below.
            // The hit is taken in its own statement: held in the `if let`'s
            // scrutinee, the tree's `Ref` would still be alive in its body,
            // and the `borrow_mut` there panicked (0.4.4, 0.5.0).
            let hit = tree.borrow().hit_test_local(root, position);
            // 0.5.4 (#112): a press anywhere but the selected static text
            // clears its selection.
            let on_selectable = hit
                .is_some_and(|(id, point)| static_text_hit_offset(tree, text, id, point).is_some());
            if !on_selectable {
                tree.borrow_mut().clear_text_selection();
            }
            let count = clicks.register(now, position);
            clicks.link_press = hit
                .and_then(|(id, point)| link_under(tree, text, id, point).map(|href| (id, href)));
            if let Some((hit, local_point)) = hit {
                if let Some(offset) = static_text_hit_offset(tree, text, hit, local_point) {
                    // 0.5.4 (#131): a double click selects the word, a triple
                    // click the line; a single press starts a drag.
                    if count >= 2 {
                        let (start, end) =
                            static_text_range(tree, text, hit, local_point, count >= 3)
                                .unwrap_or((offset, offset));
                        tree.borrow_mut().set_text_selection(hit, start, end);
                    } else {
                        tree.borrow_mut().set_text_selection(hit, offset, offset);
                        *text_drag = Some(hit);
                    }
                } else if let Some(offset) = text_field_hit_offset(tree, text, hit, local_point) {
                    tree.borrow_mut().set_text_field_cursor(hit, offset);
                    // M18 Phase 2 (§8, §10): a real press
                    // on a TextField always ARMS drag
                    // tracking -- whether it turns into a
                    // real selection depends entirely on
                    // whether a genuine PointerMoved to a
                    // different position follows before
                    // release (below); a plain click never
                    // does, so `selection_anchor` stays
                    // `None` exactly as `set_text_field_
                    // cursor` already left it.
                    *text_drag = Some(hit);
                } else if let Some((row, col)) = terminal_hit_cell(tree, text, hit, local_point) {
                    // M32 Phase 6 (§4, §5, §8): a real press
                    // on a `Terminal` -- the identical real
                    // "collapsed selection, arm drag
                    // tracking" shape `TextField`'s own
                    // press handling just above already
                    // has.
                    tree.borrow_mut()
                        .set_terminal_selection_start(hit, row, col);
                    *terminal_drag = Some(hit);
                }
            }
        }
        InputEvent::PointerMoved { position } => {
            // M18 Phase 2 (§8, §10): the real drag-select
            // half of click-to-position. Lives here, not
            // inside `Tree::dispatch`'s own existing
            // `self.dragging`/`update_drag` mechanism
            // (scrollbar thumbs) -- that mechanism is pure
            // geometry with zero rendering knowledge, but
            // this needs the identical real per-glyph
            // hit-test `PointerPressed` above already uses,
            // which only `engine-render` can do (§4).
            //
            // **Real, stated scope boundary:** a real drag
            // that leaves the field's own bounds mid-drag
            // simply stops updating the selection until it
            // re-enters (`hit_test_local` returning a
            // different node, or `None`, is a genuine
            // no-op below) -- it does not clamp to the
            // field's own nearest edge the way some real
            // desktop editors do. A further, real,
            // un-scoped refinement beyond this phase.
            if let Some(field) = *text_drag {
                let hit = tree.borrow().hit_test_local(root, position);
                if let Some((hit, local_point)) = hit {
                    let static_drag = matches!(
                        tree.borrow().get(field).map(|n| &n.kind),
                        Some(NodeKind::Text(_))
                    );
                    if static_drag {
                        // 0.5.4 (#152): a drag that started in static text goes on
                        // into any other selectable text, selecting what lies
                        // between; over anything else it keeps what it had.
                        if let Some(offset) = static_text_hit_offset(tree, text, hit, local_point) {
                            tree.borrow_mut().extend_text_selection(hit, offset);
                        }
                    } else if hit == field
                        && let Some(offset) = text_field_hit_offset(tree, text, hit, local_point)
                    {
                        tree.borrow_mut().extend_text_field_selection(hit, offset);
                    }
                }
            }
            // M32 Phase 6 (§4, §5, §8): `text_drag`'s own
            // real `Terminal` sibling -- the identical real
            // "still over the same node, extend" shape.
            if let Some(terminal) = *terminal_drag {
                let hit = tree.borrow().hit_test_local(root, position);
                if let Some((hit, local_point)) = hit
                    && hit == terminal
                    && let Some((row, col)) = terminal_hit_cell(tree, text, hit, local_point)
                {
                    tree.borrow_mut().extend_terminal_selection(hit, row, col);
                }
            }
        }
        InputEvent::PointerReleased {
            button: PointerButton::Primary,
            ..
        } => {
            // M18 Phase 2 (§8, §10): a real mouse-up always
            // ends any in-progress text drag, wherever it
            // happens -- the same "not conditioned on still
            // hitting the original node" real mouse-up
            // semantics `Tree::dispatch`'s own `self.
            // dragging = None` already established for
            // its own drags (M4 Phase 3).
            *text_drag = None;
            // M32 Phase 6 (§4, §5, §8): `text_drag`'s own
            // real `Terminal` sibling -- the real selection
            // itself stays visible (`TerminalState.
            // selection_start`/`end` are untouched here),
            // only the drag-tracking itself ends.
            *terminal_drag = None;
            // 0.5.4 (#131): a release on the link the press landed on is a
            // click on it (a drag that selected text across it is not).
            if let InputEvent::PointerReleased { position, .. } = *event
                && let Some((node, href)) = clicks.link_press.take()
            {
                let still = tree.borrow().hit_test_local(root, position);
                if let Some((hit, point)) = still
                    && hit == node
                    && link_under(tree, text, hit, point).as_deref() == Some(href.as_str())
                    && tree.borrow().static_selected_text().is_none()
                {
                    link_click = Some(LinkClick {
                        node,
                        href,
                        position,
                    });
                }
            }
        }
        _ => {}
    }
    link_click
}

/// 0.5.4 (#131): the word (or, `line`, the line) at `local_point` in selectable
/// static text, as a byte range.
fn static_text_range(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
    line: bool,
) -> Option<(usize, usize)> {
    let tree = tree.borrow();
    let NodeKind::Text(state) = &tree.get(hit)?.kind else {
        return None;
    };
    let at = text_placement(&tree, hit);
    Some(text_renderer.text_range_at(hit, state, at, local_point, line))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::PaintProperties;
    use peniko::Color;
    use taffy::prelude::{AvailableSpace, Size, Style, length};

    /// A window with one 200x30 text field at its top-left corner and one
    /// 200x60 terminal under it; a real pointer press on either runs
    /// `text_pointer_input`, which needs the tree *not* borrowed while it
    /// writes. `simulate` never reaches it, so only a real window did.
    type Scene = (
        std::rc::Rc<std::cell::RefCell<Tree>>,
        engine_core::NodeId,
        engine_core::NodeId,
        engine_core::NodeId,
    );

    fn field_and_terminal() -> Scene {
        field_and_terminal_padded(0.0, 0.0)
    }

    /// The same, with `padding` left and top on the field and the terminal.
    fn field_and_terminal_padded(pad_left: f32, pad_top: f32) -> Scene {
        let mut tree = Tree::new();
        let sized = |w: f32, h: f32| Style {
            size: Size {
                width: length(w),
                height: length(h),
            },
            padding: taffy::geometry::Rect {
                left: length(pad_left),
                top: length(pad_top),
                right: length(0.0),
                bottom: length(0.0),
            },
            ..Default::default()
        };
        let paint = || PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
        let root = tree.insert(
            NodeKind::Container,
            Style {
                flex_direction: taffy::FlexDirection::Column,
                size: Size {
                    width: length(300.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            paint(),
        );
        let field = tree.insert(
            NodeKind::TextField(Box::new(engine_core::TextFieldState::new(
                "hello world",
                "Roboto",
                400.0,
                16.0,
            ))),
            sized(200.0, 30.0),
            paint(),
        );
        let terminal = tree.insert(
            NodeKind::Terminal(Box::new(engine_core::TerminalState::new(
                20,
                4,
                engine_render::MONOSPACE_FONT_FAMILY,
                14.0,
            ))),
            sized(200.0, 60.0),
            paint(),
        );
        tree.add_child(root, field);
        tree.add_child(root, terminal);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(200.0),
            },
        );
        (
            std::rc::Rc::new(std::cell::RefCell::new(tree)),
            root,
            field,
            terminal,
        )
    }

    fn pointer(
        event: engine_core::InputEvent,
        tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
        root: engine_core::NodeId,
        drags: &mut (Option<engine_core::NodeId>, Option<engine_core::NodeId>),
    ) {
        let mut text = engine_render::TextRenderer::new();
        super::text_pointer_input(
            &event,
            tree,
            root,
            &mut text,
            &mut drags.0,
            &mut drags.1,
            &mut super::TextClicks::default(),
            std::time::Instant::now(),
        );
    }

    fn press(position: Point) -> engine_core::InputEvent {
        engine_core::InputEvent::PointerPressed {
            position,
            button: engine_core::PointerButton::Primary,
        }
    }

    /// A real click in a text field puts the caret there and arms a drag.
    /// It panicked with `RefCell already borrowed` (0.4.4 and 0.5.0).
    #[test]
    fn a_press_in_a_text_field_positions_the_caret_and_arms_a_drag() {
        let (tree, root, field, _) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(60.0, 10.0)), &tree, root, &mut drags);
        assert_eq!(drags.0, Some(field), "the drag is armed");
        let tree = tree.borrow();
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            unreachable!()
        };
        assert!(state.cursor > 0, "the caret moved to the click");
        assert_eq!(
            state.selection_anchor, None,
            "a plain click selects nothing"
        );
    }

    /// Dragging from a press across the field selects what it passes over.
    #[test]
    fn a_drag_in_a_text_field_extends_the_selection() {
        let (tree, root, field, _) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(20.0, 10.0)), &tree, root, &mut drags);
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(120.0, 10.0),
            },
            &tree,
            root,
            &mut drags,
        );
        {
            let tree = tree.borrow();
            let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
                unreachable!()
            };
            assert!(state.selection_anchor.is_some(), "a selection began");
            assert_ne!(Some(state.cursor), state.selection_anchor);
        }
        pointer(
            engine_core::InputEvent::PointerReleased {
                position: Point::new(120.0, 10.0),
                button: engine_core::PointerButton::Primary,
            },
            &tree,
            root,
            &mut drags,
        );
        assert_eq!(drags, (None, None), "a release ends the drag");
    }

    /// The same for a terminal: a press starts a selection, a drag grows it.
    #[test]
    fn a_press_and_drag_in_a_terminal_select_cells() {
        let (tree, root, _, terminal) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(10.0, 40.0)), &tree, root, &mut drags);
        assert_eq!(drags.1, Some(terminal), "the drag is armed");
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(90.0, 60.0),
            },
            &tree,
            root,
            &mut drags,
        );
        let tree = tree.borrow();
        let Some(NodeKind::Terminal(state)) = tree.get(terminal).map(|n| &n.kind) else {
            unreachable!()
        };
        assert!(state.selection_start.is_some());
        assert_ne!(
            state.selection_start, state.selection_end,
            "the selection grew"
        );
    }

    /// 0.5.4 (#112): a 300x40 selectable text node and, under it, a plain one.
    fn static_text_scene() -> (
        std::rc::Rc<std::cell::RefCell<Tree>>,
        engine_core::NodeId,
        engine_core::NodeId,
        engine_core::NodeId,
    ) {
        let mut tree = Tree::new();
        let paint = || PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
        let text_node = |selectable: bool| {
            NodeKind::Text(engine_core::TextState {
                content: "Select this sentence".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 16.0,
                align: engine_core::TextAlign::Start,
                line_height: None,
                options: engine_core::TextOptions {
                    selectable,
                    ..Default::default()
                },
            })
        };
        let sized = |h: f32| Style {
            size: Size {
                width: length(300.0),
                height: length(h),
            },
            ..Default::default()
        };
        let root = tree.insert(
            NodeKind::Container,
            Style {
                flex_direction: taffy::FlexDirection::Column,
                size: Size {
                    width: length(300.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            paint(),
        );
        let selectable = tree.insert(text_node(true), sized(40.0), paint());
        let plain = tree.insert(text_node(false), sized(40.0), paint());
        tree.add_child(root, selectable);
        tree.add_child(root, plain);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(200.0),
            },
        );
        (
            std::rc::Rc::new(std::cell::RefCell::new(tree)),
            root,
            selectable,
            plain,
        )
    }

    /// 0.5.4 (#131): a 300x40 selectable text "Visit the docs now" whose
    /// "docs" (bytes 10..14) is a link, and a plain one under it.
    fn link_scene() -> (
        std::rc::Rc<std::cell::RefCell<Tree>>,
        engine_core::NodeId,
        engine_core::NodeId,
        engine_core::NodeId,
    ) {
        let (tree, root, linked, plain) = static_text_scene();
        if let Some(NodeKind::Text(state)) = tree.borrow_mut().get_mut(linked).map(|n| &mut n.kind)
        {
            state.content = "Visit the docs now".to_string();
            state.options.spans = vec![engine_core::TextSpan {
                start: 10,
                end: 14,
                link: Some("https://example.com/docs".to_string()),
                ..Default::default()
            }];
        }
        (tree, root, linked, plain)
    }

    /// Runs `events` through `text_pointer_input` with one renderer and one
    /// click tracker, returning the link clicks.
    fn run(
        events: &[engine_core::InputEvent],
        tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
        root: engine_core::NodeId,
        clicks: &mut super::TextClicks,
        drags: &mut (Option<engine_core::NodeId>, Option<engine_core::NodeId>),
        start: std::time::Instant,
    ) -> Vec<(engine_core::NodeId, String)> {
        let mut text = engine_render::TextRenderer::new();
        let mut out = Vec::new();
        for (i, event) in events.iter().enumerate() {
            let now = start + std::time::Duration::from_millis(10 * i as u64);
            if let Some(click) = super::text_pointer_input(
                event,
                tree,
                root,
                &mut text,
                &mut drags.0,
                &mut drags.1,
                clicks,
                now,
            ) {
                out.push((click.node, click.href));
            }
        }
        out
    }

    fn release(position: Point) -> engine_core::InputEvent {
        engine_core::InputEvent::PointerReleased {
            position,
            button: engine_core::PointerButton::Primary,
        }
    }

    /// Where "docs" is: a point inside it, found by the renderer itself.
    fn on_docs(tree: &std::rc::Rc<std::cell::RefCell<Tree>>, id: engine_core::NodeId) -> Point {
        let mut text = engine_render::TextRenderer::new();
        (0..300)
            .map(|x| Point::new(f64::from(x), 10.0))
            .find(|p| super::link_under(tree, &mut text, id, *p).is_some())
            .map(|p| Point::new(p.x + 3.0, p.y))
            .expect("the link is somewhere on the line")
    }

    #[test]
    fn a_press_and_release_on_a_link_clicks_it() {
        let (tree, root, linked, _) = link_scene();
        let at = on_docs(&tree, linked);
        let clicks = run(
            &[press(at), release(at)],
            &tree,
            root,
            &mut super::TextClicks::default(),
            &mut (None, None),
            std::time::Instant::now(),
        );
        assert_eq!(
            clicks,
            vec![(linked, "https://example.com/docs".to_string())]
        );
    }

    #[test]
    fn a_drag_from_one_selectable_text_into_another_selects_across_them() {
        let (tree, root, first, second) = static_text_scene();
        if let Some(NodeKind::Text(state)) = tree.borrow_mut().get_mut(second).map(|n| &mut n.kind)
        {
            state.options.selectable = true;
        }
        let mut state = (super::TextClicks::default(), (None, None));
        run(
            &[
                press(Point::new(60.0, 10.0)),
                engine_core::InputEvent::PointerMoved {
                    position: Point::new(60.0, 50.0),
                },
                release(Point::new(60.0, 50.0)),
            ],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            std::time::Instant::now(),
        );
        let (a, b) = selection_of(&tree, first).expect("the first holds the start");
        let (c, d) = selection_of(&tree, second).expect("the second holds the end");
        assert_eq!(
            a.max(b),
            "Select this sentence".len(),
            "to the end of the first"
        );
        assert_eq!(c.min(d), 0, "from the start of the second");
        assert!(a.min(b) > 0 && c.max(d) > 0);
        let copied = tree.borrow().static_selected_text().expect("copyable");
        assert!(
            copied.contains('\n'),
            "the two texts are joined by a newline: {copied:?}"
        );
        // Dragging on over something that is not selectable keeps the selection.
        run(
            &[
                press(Point::new(60.0, 10.0)),
                engine_core::InputEvent::PointerMoved {
                    position: Point::new(60.0, 50.0),
                },
                engine_core::InputEvent::PointerMoved {
                    position: Point::new(60.0, 150.0),
                },
                release(Point::new(60.0, 150.0)),
            ],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        );
        assert_eq!(
            tree.borrow().static_selected_text().as_deref(),
            Some(copied.as_str())
        );
    }

    fn shift(key: engine_core::Key) -> engine_core::InputEvent {
        engine_core::InputEvent::KeyPressed { key, shift: true }
    }

    #[test]
    fn shift_up_and_down_move_the_selections_end_a_line_then_to_the_edge_then_on() {
        let (tree, _root, first, second) = static_text_scene();
        {
            let mut tree = tree.borrow_mut();
            for (id, content) in [
                (first, "word ".repeat(40)),
                (second, "next paragraph".to_string()),
            ] {
                if let Some(NodeKind::Text(state)) = tree.get_mut(id).map(|n| &mut n.kind) {
                    state.content = content;
                    state.options.selectable = true;
                }
            }
            tree.set_text_selection(first, 8, 8);
        }
        let mut text = engine_render::TextRenderer::new();
        let focus = |tree: &std::rc::Rc<std::cell::RefCell<Tree>>| {
            tree.borrow().static_selection_ends().unwrap().1
        };
        // Down: a line further on (well past the first 8 bytes).
        assert!(super::text_key_input(
            &shift(engine_core::Key::ArrowDown),
            &tree,
            &mut text
        ));
        let (node, one_line) = focus(&tree);
        assert_eq!(node, first);
        assert!(one_line > 20, "moved a line, not a character: {one_line}");
        // Up undoes it, to about where it was.
        assert!(super::text_key_input(
            &shift(engine_core::Key::ArrowUp),
            &tree,
            &mut text
        ));
        let (_, back) = focus(&tree);
        assert!(back.abs_diff(8) <= 3, "back near the start column: {back}");
        // Up on the first line goes to the text's start; again, nowhere (first text).
        super::text_key_input(&shift(engine_core::Key::ArrowUp), &tree, &mut text);
        assert_eq!(focus(&tree), (first, 0));
        // Down to the last line, to its end, then into the next text's end.
        for _ in 0..10 {
            super::text_key_input(&shift(engine_core::Key::ArrowDown), &tree, &mut text);
        }
        assert_eq!(focus(&tree), (second, "next paragraph".len()));
        assert!(tree.borrow().static_selected_text().unwrap().contains('\n'));
        // Other keys and an unshifted arrow are not ours.
        assert!(!super::text_key_input(
            &engine_core::InputEvent::KeyPressed {
                key: engine_core::Key::ArrowDown,
                shift: false
            },
            &tree,
            &mut text
        ));
        assert!(!super::text_key_input(
            &shift(engine_core::Key::ArrowLeft),
            &tree,
            &mut text
        ));
    }

    #[test]
    fn a_press_taken_from_the_pointer_neither_drags_nor_clicks_a_link() {
        let (tree, root, linked, _) = link_scene();
        let at = on_docs(&tree, linked);
        let mut text = engine_render::TextRenderer::new();
        let (mut clicks, mut drags) = (super::TextClicks::default(), (None, None));
        let now = std::time::Instant::now();
        let mut feed =
            |event: engine_core::InputEvent,
             clicks: &mut super::TextClicks,
             drags: &mut (Option<engine_core::NodeId>, Option<engine_core::NodeId>)| {
                super::text_pointer_input(
                    &event,
                    &tree,
                    root,
                    &mut text,
                    &mut drags.0,
                    &mut drags.1,
                    clicks,
                    now,
                )
                .map(|c| c.href)
            };
        // Pressed on the link, then the press is cancelled (a pan began): the
        // release over the same link is not a click.
        feed(press(at), &mut clicks, &mut drags);
        assert!(drags.0.is_some(), "the press armed a drag");
        super::text_press_cancelled(&mut drags.0, &mut drags.1, &mut clicks);
        assert_eq!(drags, (None, None));
        assert_eq!(feed(release(at), &mut clicks, &mut drags), None);
        // And an uncancelled press and release still clicks it (a fresh click
        // count, so it is not the second half of a double click).
        clicks = super::TextClicks::default();
        feed(press(at), &mut clicks, &mut drags);
        assert!(feed(release(at), &mut clicks, &mut drags).is_some());
    }

    #[test]
    fn a_press_off_the_link_or_released_away_from_it_is_not_a_click() {
        let (tree, root, linked, _) = link_scene();
        let at = on_docs(&tree, linked);
        let off = Point::new(2.0, 10.0);
        let mut state = (super::TextClicks::default(), (None, None));
        let now = std::time::Instant::now();
        let a = run(
            &[press(off), release(off)],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            now,
        );
        let b = run(
            &[press(at), release(off)],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            now,
        );
        let c = run(
            &[press(off), release(at)],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            now,
        );
        assert!(
            a.is_empty() && b.is_empty() && c.is_empty(),
            "{a:?} {b:?} {c:?}"
        );
    }

    #[test]
    fn dragging_a_selection_across_a_link_does_not_click_it() {
        let (tree, root, linked, _) = link_scene();
        let at = on_docs(&tree, linked);
        let from = Point::new(2.0, 10.0);
        let mut state = (super::TextClicks::default(), (None, None));
        // Press on the link, drag left over text, release on the link.
        let clicks = run(
            &[
                press(at),
                engine_core::InputEvent::PointerMoved { position: from },
                engine_core::InputEvent::PointerMoved { position: at },
                release(at),
            ],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            std::time::Instant::now(),
        );
        // The drag returned to where it started, so nothing is selected and
        // it is a click; moving it somewhere else selects, which is not.
        assert_eq!(clicks.len(), 1);
        let clicks = run(
            &[
                press(at),
                engine_core::InputEvent::PointerMoved { position: from },
                release(at),
            ],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        );
        assert!(clicks.is_empty(), "a selection was made: {clicks:?}");
        assert!(selection_of(&tree, linked).is_some());
    }

    #[test]
    fn a_double_click_selects_a_word_and_a_triple_click_the_line() {
        let (tree, root, linked, _) = link_scene();
        let at = on_docs(&tree, linked);
        let mut state = (super::TextClicks::default(), (None, None));
        let now = std::time::Instant::now();
        run(
            &[press(at), release(at), press(at)],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            now,
        );
        let (a, b) = selection_of(&tree, linked).expect("a word is selected");
        let content = "Visit the docs now";
        assert_eq!(&content[a.min(b)..a.max(b)], "docs");
        run(
            &[release(at), press(at)],
            &tree,
            root,
            &mut state.0,
            &mut state.1,
            now + std::time::Duration::from_millis(30),
        );
        let (a, b) = selection_of(&tree, linked).expect("the line is selected");
        assert_eq!((a.min(b), a.max(b)), (0, content.len()));
    }

    #[test]
    fn clicks_far_apart_or_slow_count_from_one_again() {
        let mut clicks = super::TextClicks::default();
        let now = std::time::Instant::now();
        let ms = std::time::Duration::from_millis;
        let p = Point::new(10.0, 10.0);
        assert_eq!(clicks.register(now, p), 1);
        assert_eq!(clicks.register(now + ms(100), p), 2);
        assert_eq!(clicks.register(now + ms(200), p), 3);
        assert_eq!(clicks.register(now + ms(300), p), 1, "a fourth starts over");
        assert_eq!(
            clicks.register(now + ms(400), Point::new(60.0, 10.0)),
            1,
            "moved"
        );
        assert_eq!(
            clicks.register(now + ms(2000), Point::new(60.0, 10.0)),
            1,
            "slow"
        );
    }

    #[test]
    fn text_asks_for_a_pointer_over_a_link_and_an_i_beam_over_selectable_text() {
        let (tree, root, linked, plain) = link_scene();
        let mut text = engine_render::TextRenderer::new();
        let on_link = on_docs(&tree, linked);
        let cursor = |p: Point, text: &mut engine_render::TextRenderer| {
            super::text_cursor_at(&tree, text, root, p)
        };
        assert_eq!(
            cursor(on_link, &mut text),
            Some(engine_core::Cursor::Pointer)
        );
        assert_eq!(
            cursor(Point::new(2.0, 10.0), &mut text),
            Some(engine_core::Cursor::Text)
        );
        // The plain, unselectable text asks for nothing.
        let _ = plain;
        assert_eq!(cursor(Point::new(2.0, 50.0), &mut text), None);
        // A node's own cursor wins.
        tree.borrow_mut().get_mut(linked).unwrap().cursor = Some(engine_core::Cursor::Help);
        assert_eq!(cursor(on_link, &mut text), None);
    }

    fn selection_of(
        tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
        id: engine_core::NodeId,
    ) -> Option<(usize, usize)> {
        match tree.borrow().get(id).map(|n| &n.kind) {
            Some(NodeKind::Text(state)) => state.options.selection,
            _ => None,
        }
    }

    #[test]
    fn a_press_and_drag_in_selectable_text_select_and_copy_what_it_passes() {
        let (tree, root, selectable, _) = static_text_scene();
        let mut drags = (None, None);
        pointer(press(Point::new(2.0, 10.0)), &tree, root, &mut drags);
        assert_eq!(drags.0, Some(selectable), "the drag is armed");
        assert_eq!(
            selection_of(&tree, selectable),
            Some((0, 0)),
            "a click selects nothing"
        );
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(45.0, 10.0),
            },
            &tree,
            root,
            &mut drags,
        );
        let (anchor, focus) = selection_of(&tree, selectable).expect("a selection");
        assert_eq!(anchor, 0);
        assert!(focus > 2, "the drag reached into the text: {focus}");
        let copied = tree.borrow().static_selected_text().expect("selected text");
        assert_eq!(copied, "Select this sentence"[..focus]);
        pointer(
            engine_core::InputEvent::PointerReleased {
                position: Point::new(45.0, 10.0),
                button: engine_core::PointerButton::Primary,
            },
            &tree,
            root,
            &mut drags,
        );
        assert_eq!(drags, (None, None));
        assert_eq!(
            selection_of(&tree, selectable),
            Some((anchor, focus)),
            "it stays"
        );
    }

    #[test]
    fn a_press_elsewhere_clears_the_selection() {
        let (tree, root, selectable, plain) = static_text_scene();
        let mut drags = (None, None);
        pointer(press(Point::new(2.0, 10.0)), &tree, root, &mut drags);
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(60.0, 10.0),
            },
            &tree,
            root,
            &mut drags,
        );
        assert!(tree.borrow().static_selected_text().is_some());
        pointer(
            engine_core::InputEvent::PointerReleased {
                position: Point::new(60.0, 10.0),
                button: engine_core::PointerButton::Primary,
            },
            &tree,
            root,
            &mut drags,
        );
        // A press on the plain (not selectable) text below.
        pointer(press(Point::new(20.0, 60.0)), &tree, root, &mut drags);
        assert_eq!(selection_of(&tree, selectable), None);
        assert_eq!(tree.borrow().static_selected_text(), None);
        assert_eq!(selection_of(&tree, plain), None, "plain text never selects");
        assert_eq!(drags.0, None, "and starts no drag");
    }

    #[test]
    fn text_that_is_not_selectable_ignores_a_press_and_drag() {
        let (tree, root, _, plain) = static_text_scene();
        let mut drags = (None, None);
        pointer(press(Point::new(2.0, 60.0)), &tree, root, &mut drags);
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(80.0, 60.0),
            },
            &tree,
            root,
            &mut drags,
        );
        assert_eq!(selection_of(&tree, plain), None);
    }

    fn state_of_field(
        tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
        field: engine_core::NodeId,
    ) -> usize {
        let tree = tree.borrow();
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            unreachable!()
        };
        state.cursor
    }

    /// 0.5.1 (#53): with `padding`, a click lands on the character under it:
    /// the same glyph, clicked where it now is, gives the same offset.
    #[test]
    fn a_click_in_a_padded_text_input_lands_on_the_character_under_it() {
        let (plain, root, field, _) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(60.0, 10.0)), &plain, root, &mut drags);
        let expected = state_of_field(&plain, field);
        assert!(expected > 0, "the click is inside the text");

        let (padded, root, field, _) = field_and_terminal_padded(40.0, 10.0);
        let mut drags = (None, None);
        pointer(
            press(Point::new(60.0 + 40.0, 10.0 + 10.0)),
            &padded,
            root,
            &mut drags,
        );
        assert_eq!(state_of_field(&padded, field), expected);
    }

    /// The same for a terminal: the cell under the pointer, inside the padding.
    #[test]
    fn a_click_in_a_padded_terminal_lands_on_the_cell_under_it() {
        fn cell(
            tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
            terminal: engine_core::NodeId,
        ) -> Option<(u16, u16)> {
            let tree = tree.borrow();
            let Some(NodeKind::Terminal(state)) = tree.get(terminal).map(|n| &n.kind) else {
                unreachable!()
            };
            state.selection_start
        }
        let (plain, root, _, terminal) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(60.0, 50.0)), &plain, root, &mut drags);
        let expected = cell(&plain, terminal);
        assert!(expected.is_some());

        let (padded, root, _, terminal) = field_and_terminal_padded(20.0, 10.0);
        let mut drags = (None, None);
        pointer(
            press(Point::new(60.0 + 20.0, 50.0 + 10.0)),
            &padded,
            root,
            &mut drags,
        );
        assert_eq!(cell(&padded, terminal), expected);
    }

    /// Found while doing #53: the hit test ignored a multiline field's scroll,
    /// so a click in a scrolled field landed on the line it would have under
    /// an unscrolled one. It must resolve against what is painted.
    #[test]
    fn a_click_in_a_scrolled_multiline_field_lands_on_the_line_under_it() {
        fn click_at_the_top(scroll: f64) -> usize {
            let mut tree = Tree::new();
            let content = (0..40)
                .map(|i| format!("line{i}"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut state = engine_core::TextFieldState::new(content, "Roboto", 400.0, 14.0);
            state.multiline = true;
            state.scroll_offset.current = scroll;
            let field = tree.insert(
                NodeKind::TextField(Box::new(state)),
                Style {
                    size: Size {
                        width: length(200.0),
                        height: length(100.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
            );
            tree.compute_layout(
                field,
                Size {
                    width: AvailableSpace::Definite(200.0),
                    height: AvailableSpace::Definite(100.0),
                },
            );
            let tree = std::rc::Rc::new(std::cell::RefCell::new(tree));
            let mut drags = (None, None);
            pointer(press(Point::new(5.0, 5.0)), &tree, field, &mut drags);
            state_of_field(&tree, field)
        }
        let unscrolled = click_at_the_top(0.0);
        let scrolled = click_at_the_top(100.0);
        assert!(
            scrolled > unscrolled,
            "scrolled content: line {scrolled} vs {unscrolled}"
        );
    }
}
