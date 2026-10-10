//! Input dispatch: an `InputEvent` through hover, press, focus, keys, and scrolling to its outcome.

use super::*;

/// 0.5.6 (#165): what places a text field's caret: cursor, selection anchor,
/// and the text's length.
type CaretSnapshot = (usize, Option<usize>, usize);

impl Tree {
    /// M4 Phase 1 step 1's one real top-level entry point: `engine-
    /// platform` translates a raw `winit` event into `InputEvent` and
    /// calls this. Every *mechanical* consequence (hover, focus
    /// movement, text editing, scrolling, §2 Design Principle 6) happens
    /// here, inside `engine-core`; the *meaning-dependent* outcomes
    /// (`DispatchOutcome`) are left for the caller to interpret --
    /// `engine-py::dispatch.rs`'s `run_dispatch_outcome`, not a generic
    /// trait (`input.rs`'s own doc comment has the real correction) --
    /// `Tree` has no idea what activating a node means, only that it
    /// happened.
    pub fn dispatch(&mut self, root: NodeId, event: InputEvent, now: Instant) -> DispatchOutcome {
        // 0.5.6 (#165): a keystroke or press restarts the caret's blink so it
        // is solid while you type; and what moved the focused field's caret is
        // noted for `caret_move`.
        let restart_blink = matches!(
            event,
            InputEvent::KeyPressed { .. }
                | InputEvent::TextInput(_)
                | InputEvent::ImePreedit(..)
                | InputEvent::PointerPressed { .. }
        );
        let before = self.focused_caret();
        if let Some(field) = self.focused
            && let Some(NodeKind::TextField(state)) = self.nodes.get_mut(field).map(|n| &mut n.kind)
        {
            state.last_inserted = None;
            if restart_blink {
                state.caret_on = true;
                state.caret_epoch = None;
            }
        }
        let outcome = self.dispatch_event(root, event, now);
        let after = self.focused_caret();
        if let (Some(before), Some(after)) = (&before, &after)
            && before.0 == after.0
            && before.1 != after.1
        {
            let inserted = match self.nodes.get(after.0).map(|n| &n.kind) {
                Some(NodeKind::TextField(state)) => state.last_inserted.clone(),
                _ => None,
            };
            self.caret_changes.push((after.0, inserted));
        } else if let (None, Some(after)) | (Some(_), Some(after)) = (&before, &after)
            && before.as_ref().is_none_or(|b| b.0 != after.0)
        {
            // Focus arrived in a field: the caret appeared, solid.
            self.caret_changes.push((after.0, None));
            if let Some(NodeKind::TextField(state)) =
                self.nodes.get_mut(after.0).map(|n| &mut n.kind)
            {
                state.caret_on = true;
                state.caret_epoch = None;
            }
        }
        outcome
    }

    /// 0.5.6 (#165): the focused text field's id and what places its caret.
    fn focused_caret(&self) -> Option<(NodeId, CaretSnapshot)> {
        let field = self.focused?;
        match &self.nodes.get(field)?.kind {
            NodeKind::TextField(state) => Some((
                field,
                (state.cursor, state.selection_anchor, state.content.len()),
            )),
            _ => None,
        }
    }

    /// 0.5.6 (#165): the text fields whose caret moved, or whose text changed,
    /// since the last call, each with the bytes the edit inserted, if it did.
    pub fn take_caret_changes(&mut self) -> Vec<(NodeId, Option<std::ops::Range<usize>>)> {
        std::mem::take(&mut self.caret_changes)
    }

    fn dispatch_event(&mut self, root: NodeId, event: InputEvent, now: Instant) -> DispatchOutcome {
        self.dirty = true;
        match event {
            InputEvent::PointerMoved { position } => {
                // M4 Phase 6 (§7.3): captured before `update_hover` runs
                // -- it mutates `self.hovered` internally and returns
                // only the new value, so the *old* value has to be read
                // here to report a real transition afterward.
                let old_hovered = self.hovered;
                let new_hovered = self.update_hover(root, position);
                // M4 Phase 3 (§11.5): live-follows-the-cursor while a
                // scrollbar-thumb drag is active -- a no-op otherwise.
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
                // M96: a press outside dismissible layers asks them to be
                // dismissed, and is consumed like a legacy outside press.
                if self.report_outside_press(position) {
                    self.set_pressed(None);
                    return DispatchOutcome::None;
                }
                // M10 Phase 1 (§11.3): a real press outside every open
                // dismiss_on_outside_click overlay's own subtree closes
                // it and consumes this press -- skips the normal hit/
                // press registration below entirely, matching
                // Android's own real "outside touch dismisses, doesn't
                // pass through" convention (`PLAN.md`).
                if self.dismiss_overlays_outside(position) {
                    self.set_pressed(None);
                    return DispatchOutcome::None;
                }
                // M30 Phase 4 Step 1 (§11.3): the real modal-blocking
                // half `dismiss_overlays_outside` alone can't express
                // -- a press outside a real modal overlay is consumed
                // here even when it doesn't also dismiss anything,
                // the same real "skip the normal hit/press
                // registration below entirely" outcome the dismiss
                // case already has.
                if self.press_blocked_by_modal_overlay(position) {
                    self.set_pressed(None);
                    return DispatchOutcome::None;
                }
                let hit = self.hit_test_input(root, position);
                // M38 Phase 6 (§5, §7, §11.7): a real scrollbar-thumb
                // grab takes priority over the ordinary hit -- the
                // thumb is a paint-only overlay drawn *over* the real
                // scrolled content (`engine-render's paint walk`'s own
                // `NodeKind::ScrollView` arm, mirroring pyCopper's own
                // `paint_foreground` running after children), so a real
                // point-based `hit_test` resolves to whatever content
                // sits underneath, not the thumb itself -- the exact
                // real problem pyCopper's own module doc comment names
                // directly ("the press lands on whatever row is
                // underneath"; it solves this with real event capture,
                // an architecture this codebase doesn't have, so this
                // instead walks the hit node's own ancestor chain for a
                // real `ScrollView` whose thumb the press genuinely
                // grabs). A real grab starts the drag and consumes the
                // press entirely -- the content underneath must not
                // also register a click for the same real press.
                if button == PointerButton::Primary {
                    let mut current = hit;
                    while let Some(id) = current {
                        if matches!(self.nodes[id].kind, NodeKind::ScrollView(_))
                            && self.grabs_scroll_view_thumb(id, position)
                        {
                            let (horizontal, ..) = self
                                .scroll_view_extents(id)
                                .expect("checked by grabs_scroll_view_thumb above");
                            let coord = if horizontal { position.x } else { position.y };
                            let NodeKind::ScrollView(state) = &mut self.nodes[id].kind else {
                                unreachable!("checked above")
                            };
                            let scroll = state.scroll.current;
                            state.thumb_drag_anchor = Some((coord, scroll));
                            self.dragging = Some(id);
                            self.set_pressed(None);
                            return DispatchOutcome::None;
                        }
                        // M47 (§5, §7, §11.7): the identical real grab-
                        // takes-priority-over-content technique above,
                        // for `VirtualList`'s own thumb -- vertical-only,
                        // so no `horizontal` branch is needed the way
                        // `ScrollView`'s own arm above has one.
                        if matches!(self.nodes[id].kind, NodeKind::VirtualList(_))
                            && self.grabs_virtual_list_thumb(id, position)
                        {
                            let NodeKind::VirtualList(state) = &mut self.nodes[id].kind else {
                                unreachable!("checked above")
                            };
                            let scroll = state.scroll_offset.current;
                            state.thumb_drag_anchor = Some((position.y, scroll));
                            self.dragging = Some(id);
                            self.set_pressed(None);
                            return DispatchOutcome::None;
                        }
                        current = self.nodes[id].parent;
                    }
                }
                if let Some(node) = hit {
                    self.set_pressed(Some((button, node)));
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
                    //
                    // M30 Phase 9 Step 4 (§5, §8, §10): widened to
                    // `Terminal` too -- **a real, confirmed bug found
                    // live, not predicted in advance**: a real end-to-
                    // end empirical test (spawn a shell, click the
                    // terminal, type a command) found the click never
                    // actually focused it at all, so the typed command
                    // silently never reached the shell. Every real
                    // terminal emulator focuses itself on click, the
                    // identical real expectation `TextField`'s own
                    // finding already states for text input generally.
                    //
                    // M53 Phase 1 (§8, §10, §11.3): widened to `Pointer
                    // Button::Secondary` too -- a right-click that opens
                    // a context menu must focus the field first, or a
                    // Copy/Cut/Paste menu item would act on whatever
                    // was last *left*-clicked, not the field the user
                    // just right-clicked. Every real desktop text field
                    // focuses itself on right-click too, the identical
                    // real expectation this gate's own `Primary` case
                    // already establishes.
                    // M55 (§10, §16.2): captured so this real click-to-
                    // focus transition can become a real `Focus
                    // Changed` outcome below, instead of the prior
                    // unconditional `DispatchOutcome::None` silently
                    // discarding it.
                    //
                    // M94: widened to any node a framework marked
                    // `focusable` -- the press focuses the nearest such
                    // node, itself or an ancestor, the way a browser
                    // focuses the focusable element containing a click.
                    let focus_target =
                        if !matches!(button, PointerButton::Primary | PointerButton::Secondary) {
                            None
                        } else if matches!(
                            self.nodes.get(node).map(|n| &n.kind),
                            Some(NodeKind::TextField(_)) | Some(NodeKind::Terminal(_))
                        ) {
                            Some(node)
                        } else {
                            self.focusable_ancestor(node)
                        };
                    let focus_transition =
                        focus_target.and_then(|target| self.set_focus_to(target));
                    match focus_transition {
                        Some((old, new)) => DispatchOutcome::FocusChanged { old, new },
                        None => DispatchOutcome::None,
                    }
                } else {
                    self.set_pressed(None);
                    DispatchOutcome::None
                }
            }
            InputEvent::PointerReleased { position, button } => {
                let hit = self.hit_test_input(root, position);
                let outcome = match self.pressed {
                    // M4 Phase 7 (§11.3): a same-node press/release pair
                    // means something different per button -- Primary
                    // activates, Secondary is its own real outcome (a
                    // context menu, say), and Middle and (0.4.1) the side
                    // buttons have no outcome -- listeners still hear their
                    // pointer_down/pointer_up, so no `click`.
                    Some((pressed_button, pressed_node))
                        if pressed_button == button && Some(pressed_node) == hit =>
                    {
                        match button {
                            PointerButton::Primary => DispatchOutcome::Activated(pressed_node),
                            PointerButton::Secondary => {
                                DispatchOutcome::SecondaryActivated(pressed_node)
                            }
                            PointerButton::Middle
                            | PointerButton::Back
                            | PointerButton::Forward => DispatchOutcome::None,
                        }
                    }
                    _ => DispatchOutcome::None,
                };
                self.set_pressed(None);

                // M4 Phase 3 (§11.5): a real mouse-up always ends a
                // drag, wherever it happens -- not conditioned on still
                // hitting the thumb, matching real OS drag semantics.
                if button == PointerButton::Primary {
                    // M38 Phase 6 (§5, §7, §11.7): a real scrollbar
                    // thumb's own drag bookkeeping is genuinely per-
                    // gesture -- mirrors pyCopper's own real `on_
                    // pointer_up`, which pops the identical `drag_from`/
                    // `drag_scroll` state.
                    if let Some(dragging) = self.dragging
                        && let NodeKind::ScrollView(state) = &mut self.nodes[dragging].kind
                    {
                        state.thumb_drag_anchor = None;
                    }
                    self.dragging = None;
                }
                outcome
            }
            InputEvent::KeyPressed { key, shift } => {
                // 0.5.4 (#131): Shift+arrows, Home and End extend a selection
                // in static text, unless a text input has the keys.
                if shift
                    && matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End)
                    && !self.focused.is_some_and(|f| {
                        matches!(
                            self.nodes.get(f).map(|n| &n.kind),
                            Some(NodeKind::TextField(_))
                        )
                    })
                    && self.extend_static_selection(key)
                {
                    return DispatchOutcome::None;
                }
                // M15 Phase 2 (§8, §10): a focused `TextField` gets
                // first refusal on most keys -- its own real "Enter"/
                // "Space" meaning (insert a character) is genuinely
                // different from the generic button-activation meaning
                // below, so this can't simply run after it. `Escape`
                // still falls through unchanged (`dispatch_text_field_
                // key` always returns `None` for it, meaning "not mine
                // to handle") -- a focused field must still dismiss
                // overlays on Escape, the same as any other focused
                // node. M31 Phase 2 (§8, §10): `Tab` used to be in that
                // same "always falls through" group too -- now only a
                // *single-line* field still loses focus on Tab; a
                // *multiline* field claims it first and inserts a real
                // `\t` instead (`dispatch_text_field_key`'s own doc
                // comment has the real reasoning).
                if let Some(field) = self.focused
                    && matches!(
                        self.nodes.get(field).map(|n| &n.kind),
                        Some(NodeKind::TextField(_))
                    )
                    && let Some(outcome) = self.dispatch_text_field_key(field, key, shift)
                {
                    // M38 Phase 7 (§5, §8): real caret-follow -- runs
                    // after every real key this field claimed
                    // (`dispatch_text_field_key` returning `Some` at
                    // all, regardless of which of its own many early-
                    // return arms produced it), the one chokepoint
                    // every real call reaches regardless of internal
                    // control flow, rather than auditing and touching
                    // each of that method's own ~15 individual arms.
                    // A true no-op key still calls this harmlessly (the
                    // caret hasn't moved, so there's nothing to scroll
                    // to).
                    self.scroll_text_field_caret_into_view(field);
                    return outcome;
                }
                match key {
                    Key::Tab => {
                        let direction = if shift {
                            FocusDirection::Previous
                        } else {
                            FocusDirection::Next
                        };
                        // M55 (§10, §16.2): a real Tab/Shift-Tab focus
                        // transition now becomes a real `FocusChanged`
                        // outcome, instead of the prior unconditional
                        // `DispatchOutcome::None` silently discarding
                        // it -- mirrors the identical real fix at the
                        // `PointerPressed` click-to-focus site above.
                        match self.move_focus(root, direction) {
                            Some((old, new)) => DispatchOutcome::FocusChanged { old, new },
                            None => DispatchOutcome::None,
                        }
                    }
                    Key::Enter | Key::Space => match self.focused {
                        Some(node) => DispatchOutcome::Activated(node),
                        None => DispatchOutcome::None,
                    },
                    // M10 Phase 1 (§11.3): closes every real, currently-
                    // open dismiss_on_escape overlay -- a mechanical
                    // consequence handled entirely here, the same shape
                    // hover-update already uses, no new outcome variant.
                    Key::Escape => {
                        self.dismiss_escapable_overlays();
                        DispatchOutcome::None
                    }
                    // M15 Phase 2: real, but only ever meaningful when a
                    // `TextField` is focused -- handled above via `
                    // dispatch_text_field_key` in that case. Reaching
                    // here means none is focused, a true no-op.
                    // `ArrowUp`/`ArrowDown` (M30 Phase 9 Step 3, §10)
                    // join the same real "only meaningful when a
                    // TextField is focused" group -- also handled above
                    // via `dispatch_text_field_key` when one is.
                    Key::Backspace
                    | Key::Delete
                    | Key::ArrowLeft
                    | Key::ArrowRight
                    | Key::ArrowUp
                    | Key::ArrowDown
                    | Key::Home
                    | Key::End
                    | Key::PageUp
                    | Key::PageDown => DispatchOutcome::None,
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
                // 0.5.6 (#162): a read-only field takes no text, and a field with a
                // limit takes what fits (a composition that was in progress ends).
                if state.read_only {
                    state.preedit = None;
                    return DispatchOutcome::None;
                }
                let text = state.fit(&text).to_string();
                if text.is_empty() {
                    state.preedit = None;
                    return DispatchOutcome::None;
                }
                let old_content = state.content.clone();
                // M15 Phase 3 (§16.7): typing over a real, active
                // selection replaces it -- the same real desktop-editor
                // behavior `Backspace`/`Delete`/`Space` already apply,
                // via the identical shared helper.
                Self::delete_selection(state);
                state.last_inserted = Some(state.cursor..state.cursor + text.len());
                state.content.insert_str(state.cursor, &text);
                state.cursor += text.len();
                state.goal_column = None;
                // M17 Phase 2 (§8): a real insertion -- whether from a
                // plain keypress or a real IME `Commit` (both reach
                // this same arm) -- always clears any stale preedit.
                // Defensive: `winit`'s own real behavior already keeps
                // plain `KeyboardInput`/`Ime` events mutually exclusive
                // during composition, so `preedit` should already be
                // `None` here in practice, but a real commit is exactly
                // the moment composition ends either way.
                state.preedit = None;
                self.scroll_text_field_caret_into_view(field);
                DispatchOutcome::Changed {
                    node: field,
                    old_value: ChangedValue::Text(old_content),
                }
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
            // handled entirely here, the same shape hover-update
            // already uses -- still DispatchOutcome::None, nothing for
            // the app layer to be told happened.
            InputEvent::Scroll { delta, position } => {
                // M96: winit's sign scrolls toward the start; every offset
                // below grows toward the end, so it flips once, here. (It
                // used to pass through unflipped, so a real wheel scrolled
                // backwards -- only synthetic input, which used the offset's
                // own sign, was ever tested.)
                let delta = match delta {
                    ScrollDelta::Lines(x, y) => ScrollDelta::Lines(-x, -y),
                    ScrollDelta::Pixels(x, y) => ScrollDelta::Pixels(-x, -y),
                };
                if let Some(hit) = self.hit_test_input(root, position) {
                    let mut current = Some(hit);
                    while let Some(id) = current {
                        let node = &self.nodes[id];
                        // 0.4.3 M17, 0.4.4 M21: a list or view the wheel
                        // can't move -- no part along its axis, nothing to
                        // scroll, or already at that end -- passes it on to
                        // the next one out (`can_scroll`): a plain wheel over
                        // a carousel scrolls the page around it, and so does
                        // one over an inner list at its end, as in a
                        // browser. One that can move takes the whole wheel.
                        if matches!(node.kind, NodeKind::VirtualList(_)) {
                            let delta_y = match delta {
                                ScrollDelta::Lines(_, y) => y * 20.0,
                                ScrollDelta::Pixels(_, y) => y,
                            };
                            if self.can_scroll(id, delta_y) {
                                self.scroll_virtual_list_by(id, delta_y);
                                break;
                            }
                        }
                        // M36 Phase 1 (§5, §7, §11.7): the identical
                        // real "walk up to the nearest scrollable
                        // ancestor" widening, a third time, for a real
                        // general `ScrollView` -- a wheel notch over
                        // any of its scrolled content bubbles to it
                        // exactly the way one over a `VirtualList` row
                        // already does above.
                        if let NodeKind::ScrollView(state) = &node.kind {
                            let delta_along = match delta {
                                ScrollDelta::Lines(x, y) => {
                                    if state.horizontal {
                                        x * 20.0
                                    } else {
                                        y * 20.0
                                    }
                                }
                                ScrollDelta::Pixels(x, y) => {
                                    if state.horizontal {
                                        x
                                    } else {
                                        y
                                    }
                                }
                            };
                            if self.can_scroll(id, delta_along) {
                                self.scroll_scroll_view_by(id, delta_along);
                                break;
                            }
                        }
                        current = self.nodes[id].parent;
                    }
                }
                DispatchOutcome::None
            }
            // M7 Phase 3 (§7.1): plumbing only, see `InputEvent::
            // ThemeChanged`'s own doc comment -- `engine-py` reports
            // the raw event to window listeners.
            InputEvent::ThemeChanged { .. } => DispatchOutcome::None,
            // 0.5.4 (#113): touches and trackpad pinches are recognized and
            // delivered by the window (`engine-py`); the tree has nothing to do.
            InputEvent::Touch { .. } | InputEvent::TrackpadPinch { .. } => DispatchOutcome::None,
            // 0.5.4 (#115): OS preferences are reported by the window (`engine-py`).
            InputEvent::ReducedMotionChanged { .. } | InputEvent::HighContrastChanged { .. } => {
                DispatchOutcome::None
            }
            // 0.5.4 (#114): files are delivered by the window (`engine-py`).
            InputEvent::FileHovered { .. }
            | InputEvent::FileHoverCancelled
            | InputEvent::FileDropped { .. } => DispatchOutcome::None,
            // 0.5.0 M2: plumbing only, like `ThemeChanged`.
            InputEvent::Focused { .. } => DispatchOutcome::None,
            // M32 Phase 2 (§4, §5): unlike `ThemeChanged`, a real
            // mutation happens right here -- `root`'s own `layout_
            // style.size` is a pure taffy concern `engine-core` fully
            // owns (no platform knowledge needed), so there's no
            // reason to defer this to `engine-py` the way `ThemeChanged`
            // has to. `self.dirty` is already set unconditionally at
            // the top of this function, which is exactly what a real
            // resize needs: the next `compute_layout` call picks up
            // this new size and lays out fresh, the identical dirty-
            // tracking mechanism every other real mutation here already
            // rides for free. **Real bug this phase's own first test
            // caught, not predicted in advance:** an initial draft
            // mutated `node.layout_style` directly -- `Tree::layout`
            // still reported the stale size after a fresh `compute_
            // layout`, because taffy keeps its own internal copy of
            // every node's style (fed once at `insert` time) and never
            // reads `Node::layout_style` back out of the tree on its
            // own. Fixed by going through the real, existing `Tree::
            // set_layout_style` -- its own doc comment states plainly
            // that it's "the only place after `insert` that's allowed
            // to touch `layout_style`" for exactly this reason.
            InputEvent::Resized { width, height } => {
                if let Some(mut style) = self.get(root).map(|node| node.layout_style.clone()) {
                    style.size = Size {
                        width: length(width),
                        height: length(height),
                    };
                    self.set_layout_style(root, style);
                }
                DispatchOutcome::None
            }
            // M17 Phase 1 (§8): plumbing only, the identical shape --
            // `engine-core` has no clipboard access at all, so the real
            // work (reading `Tree::text_field_selected_text`/`cut_
            // text_field_selection` and the actual OS clipboard I/O)
            // happens in `engine-py`'s own raw-event handling, not here.
            InputEvent::Copy | InputEvent::Cut | InputEvent::PasteRequested => {
                DispatchOutcome::None
            }
            // M32 Phase 4 (§4, §8): the identical plumbing-only shape
            // just above -- `engine-core` has no PTY/terminal access at
            // all (§4), so deciding whether a focused `Terminal` exists
            // to send this letter's own real ASCII control byte to
            // happens in `engine-py`'s own raw-event handling.
            InputEvent::ControlChar(_) => DispatchOutcome::None,
            // M32 Phase 6 (§4, §5, §8): the identical plumbing-only
            // shape -- reading a focused `Terminal`'s own real selected
            // text (`Tree::terminal_selected_text`) and writing it to
            // the real OS clipboard both happen in `engine-py`'s own
            // raw-event handling.
            InputEvent::TerminalCopyRequested => DispatchOutcome::None,
            // M17 Phase 2 (§8): a real, mechanical mutation (unlike
            // Copy/Cut/PasteRequested above, this needs no OS access --
            // `engine-platform` already extracted the real preedit text
            // from `winit::event::Ime::Preedit` before this ever
            // reaches here), but never a `Change`: a composition
            // preview isn't committed content, nothing has actually
            // been typed yet.
            InputEvent::ImePreedit(text, cursor) => {
                if let Some(field) = self.focused
                    && let Some(NodeKind::TextField(state)) =
                        self.nodes.get_mut(field).map(|n| &mut n.kind)
                {
                    // 0.5.6 (#162): a read-only field takes no composition.
                    if state.read_only {
                        return DispatchOutcome::None;
                    }
                    let was = state.preedit.is_some();
                    let now_has = !text.is_empty();
                    state.preedit = if now_has { Some(text.clone()) } else { None };
                    state.preedit_cursor = now_has.then_some(cursor).flatten();
                    let phase = match (was, now_has) {
                        (false, true) => Some(crate::input::ComposePhase::Start),
                        (true, true) => Some(crate::input::ComposePhase::Update),
                        (true, false) => Some(crate::input::ComposePhase::End),
                        (false, false) => None,
                    };
                    if let Some(phase) = phase {
                        return DispatchOutcome::Composed {
                            node: field,
                            phase,
                            text,
                            cursor,
                        };
                    }
                }
                DispatchOutcome::None
            }
            // M94: plumbing only -- named keys, modifier changes, and
            // scale-factor changes mean nothing to the tree itself;
            // `engine-py` routes them to Python listeners.
            InputEvent::Key { .. }
            | InputEvent::ModifiersChanged(_)
            | InputEvent::ScaleFactorChanged { .. } => DispatchOutcome::None,
            // M94: the pointer left the window, so nothing is hovered --
            // the same transition `PointerMoved` reports when the pointer
            // moves off every node.
            InputEvent::PointerLeft => {
                let old_hovered = self.hovered;
                self.set_hovered(None);
                if old_hovered.is_some() {
                    DispatchOutcome::HoverChanged {
                        old: old_hovered,
                        new: None,
                    }
                } else {
                    DispatchOutcome::None
                }
            }
        }
    }
}
