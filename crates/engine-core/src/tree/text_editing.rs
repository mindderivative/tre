//! Text fields' and terminals' editing and selection.

use super::*;

impl Tree {
    /// M38 Phase 7 (§5, §8): real caret-follow -- if `field`'s own real
    /// caret would currently sit outside its own visible viewport
    /// (given its current `scroll_offset`), scrolls just enough to
    /// bring it back in, the same real "the editor auto-scrolls to
    /// keep the cursor visible" behavior every real code editor
    /// already has. A true no-op for a single-line field (never
    /// scrolls at all, `scroll_offset` stays `0.0` forever) or one
    /// whose real content already fits its own viewport.
    ///
    /// **Real, honest v1 approximation, stated directly:** the real
    /// per-line pixel height used here (`Self::CODE_EDITOR_LINE_
    /// HEIGHT_RATIO * state.font_size`) is an estimate, not an exact
    /// measured value -- `engine-core` cannot load a real font to
    /// measure one (§4). This can leave the caret slightly closer to
    /// (or further from) the real viewport edge than an exactly-
    /// measured line height would land it -- a real, acceptable
    /// imprecision in a heuristic, not a correctness bug: whatever
    /// `scroll_offset` this computes is applied identically to both
    /// the real clip and the real glyph positions at paint time
    /// (`engine-render::draw_field`), so the actually-*painted* result
    /// stays internally consistent regardless of how precisely this
    /// guessed the ideal scroll target.
    pub(super) fn scroll_text_field_caret_into_view(&mut self, field: NodeId) {
        let Some(node) = self.nodes.get(field) else {
            return;
        };
        let NodeKind::TextField(state) = &node.kind else {
            return;
        };
        if !state.multiline {
            return;
        }
        // 0.5.1 (#53): the viewport is the content box, inside the padding.
        let layout = self.layout(field);
        let pad = layout.padding;
        let viewport_height = f64::from(layout.size.height - pad.top - pad.bottom).max(0.0);
        let viewport_width = f64::from(layout.size.width - pad.left - pad.right).max(0.0);
        let NodeKind::TextField(state) = &self.nodes[field].kind else {
            unreachable!("checked above")
        };
        let line_height = f64::from(state.font_size) * Self::CODE_EDITOR_LINE_HEIGHT_RATIO;
        if line_height <= 0.0 {
            return;
        }
        let caret_line = state.content[..state.cursor].matches('\n').count() as f64;
        let total_lines = state.content.matches('\n').count() as f64 + 1.0;
        let max_scroll = (total_lines * line_height - viewport_height).max(0.0);
        let caret_top = caret_line * line_height;
        let caret_bottom = caret_top + line_height;

        let mut scroll = state.scroll_offset.current;
        if caret_top < scroll {
            scroll = caret_top;
        } else if caret_bottom > scroll + viewport_height {
            scroll = caret_bottom - viewport_height;
        }
        let scroll = scroll.clamp(0.0, max_scroll);

        // M39 Phase 1 (§5, §8): the identical real "scroll just enough
        // to reveal the caret" logic as the vertical case just above,
        // along the horizontal axis instead -- `engine-render::text::
        // field_max_width` never wraps a `multiline` field's own real
        // lines, so a long line can overflow the box horizontally the
        // same way tall content overflows it vertically.
        let char_width = f64::from(state.font_size) * Self::CODE_EDITOR_CHAR_WIDTH_RATIO;
        let h_scroll = if char_width <= 0.0 {
            0.0
        } else {
            let caret_column = Self::real_column(&state.content, state.cursor) as f64;
            let longest_line = state
                .content
                .split('\n')
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0) as f64;
            let max_h_scroll = (longest_line * char_width - viewport_width).max(0.0);
            let caret_left = caret_column * char_width;
            let caret_right = caret_left + char_width;

            let mut h_scroll = state.horizontal_scroll_offset.current;
            if caret_left < h_scroll {
                h_scroll = caret_left;
            } else if caret_right > h_scroll + viewport_width {
                h_scroll = caret_right - viewport_width;
            }
            h_scroll.clamp(0.0, max_h_scroll)
        };

        let NodeKind::TextField(state) = &mut self.nodes[field].kind else {
            unreachable!("checked above")
        };
        state.scroll_offset.current = scroll;
        state.horizontal_scroll_offset.current = h_scroll;
    }

    pub(super) fn dispatch_text_field_key(
        &mut self,
        field: NodeId,
        key: Key,
        shift: bool,
    ) -> Option<DispatchOutcome> {
        let NodeKind::TextField(state) = &mut self.nodes[field].kind else {
            return None;
        };
        // M38 Phase 2 (§5, §8): every key except `ArrowUp`/`ArrowDown`
        // itself ends a real goal-column sequence -- those two arms
        // manage `goal_column` themselves (seed it on the first hop,
        // leave it alone on every further one), matching real desktop-
        // editor behavior: only a *consecutive* run of vertical moves
        // remembers the original column.
        if !matches!(key, Key::ArrowUp | Key::ArrowDown) {
            state.goal_column = None;
        }
        match key {
            Key::Backspace => {
                // M54 Phase 1 (§8, §16.2): the real pre-edit content,
                // snapshotted before either real mutation path below --
                // `DispatchOutcome::Changed`'s own `old_value` needs it
                // captured here, the one real place it's still whole.
                let old_content = state.content.clone();
                if Self::delete_selection(state) {
                    return Some(DispatchOutcome::Changed {
                        node: field,
                        old_value: ChangedValue::Text(old_content),
                    });
                }
                if state.cursor == 0 {
                    return Some(DispatchOutcome::None);
                }
                let prev = state.content[..state.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i);
                state.content.replace_range(prev..state.cursor, "");
                state.cursor = prev;
                Some(DispatchOutcome::Changed {
                    node: field,
                    old_value: ChangedValue::Text(old_content),
                })
            }
            Key::Delete => {
                let old_content = state.content.clone();
                if Self::delete_selection(state) {
                    return Some(DispatchOutcome::Changed {
                        node: field,
                        old_value: ChangedValue::Text(old_content),
                    });
                }
                if state.cursor >= state.content.len() {
                    return Some(DispatchOutcome::None);
                }
                let next = state.content[state.cursor..]
                    .char_indices()
                    .nth(1)
                    .map_or(state.content.len(), |(i, _)| state.cursor + i);
                state.content.replace_range(state.cursor..next, "");
                Some(DispatchOutcome::Changed {
                    node: field,
                    old_value: ChangedValue::Text(old_content),
                })
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
                        .map_or(0, |(i, _)| i);
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
                        .map_or(state.content.len(), |(i, _)| state.cursor + i);
                }
                Some(DispatchOutcome::None)
            }
            // M30 Phase 9 Step 3 (§8, §10): `Home`/`End` jump to the
            // whole buffer's own start/end for a single-line field
            // (real, unchanged, byte-for-byte), but to the *current
            // line's* own start/end for `Code Editor`'s real multiline
            // mode -- the real, expected desktop-editor convention, not
            // "jump to the file's own start" every keystroke.
            Key::Home => {
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                } else {
                    state.selection_anchor = None;
                }
                let target = if state.multiline {
                    Self::line_start(&state.content, state.cursor)
                } else {
                    0
                };
                state.cursor = Self::snap_out_of_fold(target, &state.content, &state.folded_ranges);
                Some(DispatchOutcome::None)
            }
            Key::End => {
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                } else {
                    state.selection_anchor = None;
                }
                let target = if state.multiline {
                    Self::line_end(&state.content, state.cursor)
                } else {
                    state.content.len()
                };
                state.cursor = Self::snap_out_of_fold(target, &state.content, &state.folded_ranges);
                Some(DispatchOutcome::None)
            }
            // M30 Phase 9 Step 3 (§8, §10): a true no-op for a single-
            // line field (consumed, matching `Enter`'s own established
            // "consumed but no-op" precedent just below) -- `Code
            // Editor`'s own real, load-bearing need: move by line,
            // preserving the caret's own real *character* column
            // (`Self::move_to_line`), the correct, expected behavior
            // for a genuinely monospace editor, not a pixel-accurate
            // approximation (this codebase has no bundled monospace
            // font yet, a real, separate, stated gap -- `add_code_
            // editor`'s own doc comment). Mirrors `ArrowLeft`/`Right`'s
            // own established "collapse an active selection, don't
            // also move further" convention for consistency, not a
            // second, differently-shaped rule.
            Key::ArrowUp => {
                if !state.multiline {
                    return Some(DispatchOutcome::None);
                }
                if !shift && let Some(anchor) = state.selection_anchor.take() {
                    state.cursor = state.cursor.min(anchor);
                    state.goal_column = None;
                    return Some(DispatchOutcome::None);
                }
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                }
                let goal = *state
                    .goal_column
                    .get_or_insert_with(|| Self::real_column(&state.content, state.cursor));
                if let Some(target) = Self::move_to_line(&state.content, state.cursor, true, goal) {
                    state.cursor =
                        Self::snap_out_of_fold(target, &state.content, &state.folded_ranges);
                }
                Some(DispatchOutcome::None)
            }
            Key::ArrowDown => {
                if !state.multiline {
                    return Some(DispatchOutcome::None);
                }
                if !shift && let Some(anchor) = state.selection_anchor.take() {
                    state.cursor = state.cursor.max(anchor);
                    state.goal_column = None;
                    return Some(DispatchOutcome::None);
                }
                if shift {
                    state.selection_anchor.get_or_insert(state.cursor);
                }
                let goal = *state
                    .goal_column
                    .get_or_insert_with(|| Self::real_column(&state.content, state.cursor));
                if let Some(target) = Self::move_to_line(&state.content, state.cursor, false, goal)
                {
                    state.cursor =
                        Self::snap_out_of_fold(target, &state.content, &state.folded_ranges);
                }
                Some(DispatchOutcome::None)
            }
            // A real space keypress reaches `KeyPressed` (`Key::Space`,
            // matched by `translate_key` before `TextInput` would ever
            // fire for it, §8) rather than `TextInput` -- so a focused
            // `TextField` must claim it here as a real inserted space,
            // not fall through to `Key::Enter | Key::Space =>
            // Activated`'s own generic button-activation meaning.
            Key::Space => {
                let old_content = state.content.clone();
                Self::delete_selection(state);
                state.content.insert(state.cursor, ' ');
                state.cursor += 1;
                Some(DispatchOutcome::Changed {
                    node: field,
                    old_value: ChangedValue::Text(old_content),
                })
            }
            // A single-line field: `Enter` is consumed (no activation,
            // matching `Space`'s own reasoning above) but deliberately
            // doesn't insert a newline either -- real, stated,
            // single-line scope, not a general multiline text area.
            // M30 Phase 9 Step 3 (§8, §10): `Code Editor`'s own real
            // multiline mode inserts a genuine `\n` instead, the same
            // real `delete_selection`-then-insert shape `Space` above
            // already establishes.
            Key::Enter => {
                if state.multiline {
                    let old_content = state.content.clone();
                    Self::delete_selection(state);
                    state.content.insert(state.cursor, '\n');
                    state.cursor += 1;
                    Some(DispatchOutcome::Changed {
                        node: field,
                        old_value: ChangedValue::Text(old_content),
                    })
                } else {
                    Some(DispatchOutcome::None)
                }
            }
            // M31 Phase 2 (§8, §10): a focused *multiline* field claims
            // `Tab` first -- the identical real "first refusal, `None`
            // means not mine" contract every other key in this method
            // already establishes, not a second, differently-shaped
            // mechanism. Inserts a literal `\t`, not N spaces:
            // tabs-vs-spaces is real app-level policy (Design Principle
            // 6), not engine-core's to decide. A single-line field
            // still returns `None` here, byte-for-byte its own prior
            // real behavior -- `Tab` on a single-line `TextField`
            // remains ordinary focus traversal, matching real desktop
            // form convention (a single-line input was never a place a
            // real indentation character belongs). `Escape` stays the
            // real, unconditional way out of a focused field either
            // way, mirroring pyCopper's own real "Escape still
            // defocuses before per-element delivery" precedent, so Tab
            // capture never traps the keyboard.
            Key::Tab => {
                if state.multiline {
                    let old_content = state.content.clone();
                    Self::delete_selection(state);
                    state.content.insert(state.cursor, '\t');
                    state.cursor += 1;
                    Some(DispatchOutcome::Changed {
                        node: field,
                        old_value: ChangedValue::Text(old_content),
                    })
                } else {
                    None
                }
            }
            Key::Escape => None,
            // 0.4.2 M12: a text field doesn't page; the scroll view around
            // it does.
            Key::PageUp | Key::PageDown => None,
        }
    }

    /// M15 Phase 3 (§16.7): deletes a real, active selection (`anchor
    /// != cursor`) and leaves `cursor` at the deleted range's own
    /// start -- returns `true` if it did, `false` (a true no-op) if no
    /// real selection was active. Shared by every real edit that must
    /// replace a selection rather than naively act at a bare cursor.
    pub(super) fn delete_selection(state: &mut TextFieldState) -> bool {
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
        state.goal_column = None;
        true
    }

    /// M30 Phase 9 Step 3 (§8, §10): the byte offset of `cursor`'s own
    /// current line's start -- just past the nearest `\n` before it,
    /// or `0` at the buffer's own real start. Shared by `Key::Home`'s
    /// own multiline arm and `move_to_line` below.
    pub(super) fn line_start(content: &str, cursor: usize) -> usize {
        content[..cursor].rfind('\n').map_or(0, |i| i + 1)
    }

    /// `line_start`'s own real end-of-line sibling -- the nearest
    /// `\n`'s own byte offset at or after `cursor`, or `content.len()`
    /// at the buffer's own real end.
    pub(super) fn line_end(content: &str, cursor: usize) -> usize {
        content[cursor..]
            .find('\n')
            .map_or(content.len(), |i| cursor + i)
    }

    /// M38 Phase 3 (§5, §8): a real cursor position, after `Home`/
    /// `End`/`ArrowUp`/`ArrowDown` compute where it would land,
    /// snapped out of any real folded range it would otherwise land
    /// strictly inside -- mirrors `engine-render::text::to_display_
    /// offset_folded`'s own identical "resolves to right after that
    /// fold's own real marker" convention (its own doc comment), so
    /// cursor navigation and paint now agree on what landing inside a
    /// genuinely hidden region really means, closing the real,
    /// previously-stated v1 gap `TextFieldState.folded_ranges`'s own
    /// doc comment named directly. A position exactly *at* a fold's
    /// own `start` or `end` is left alone -- both are real, visible
    /// boundaries, not hidden content. Malformed ranges (out of
    /// order, overlapping, out of `content`'s own bounds) are skipped,
    /// the same real defensive normalization `engine-render`'s own
    /// `fold_segments` already applies -- `engine-core` never
    /// validates `folded_ranges` itself.
    pub(super) fn snap_out_of_fold(
        cursor: usize,
        content: &str,
        folded: &[std::ops::Range<usize>],
    ) -> usize {
        let content_len = content.len();
        let mut result = cursor;
        let mut consumed = 0;
        for range in folded {
            if range.start < consumed || range.end <= range.start || range.end > content_len {
                continue;
            }
            if range.start < result && result < range.end {
                result = range.end;
            }
            consumed = range.end;
        }
        result
    }

    /// A cursor's own real *character* column (not byte offset) on
    /// whatever line it currently sits on -- the fresh-each-call
    /// computation `move_to_line` used before M38 Phase 2, now also
    /// used to *seed* `TextFieldState::goal_column` the first time a
    /// real `ArrowUp`/`ArrowDown` sequence begins.
    pub(super) fn real_column(content: &str, cursor: usize) -> usize {
        let line_start = Self::line_start(content, cursor);
        content[line_start..cursor].chars().count()
    }

    /// `Key::ArrowUp`/`ArrowDown`'s own real line-navigation logic:
    /// moves `cursor` to the adjacent line (`up`, or the next one),
    /// landing at `goal_column` (a real *character* column, UTF-8-safe
    /// the same way `ArrowLeft`/`ArrowRight`'s own `char_indices`
    /// stepping already is), clamped to the target line's own real
    /// length if it's shorter -- the same real "land at end of a
    /// shorter line" convention every desktop text editor already has.
    /// M38 Phase 2 (§5, §8): `goal_column` is the caller's own real
    /// "goal column" memory (`TextFieldState::goal_column`), not
    /// necessarily `cursor`'s own current column -- a consecutive
    /// `ArrowUp`/`ArrowDown` sequence keeps landing at the *original*
    /// column even after an intermediate shorter line clamped the
    /// real cursor to something smaller, the real behavior a fuller
    /// desktop editor already has and this v1 didn't track before.
    /// Returns `None` (a true no-op) at the buffer's own first/last
    /// line, where there's genuinely nowhere to go.
    pub(super) fn move_to_line(
        content: &str,
        cursor: usize,
        up: bool,
        goal_column: usize,
    ) -> Option<usize> {
        let line_start = Self::line_start(content, cursor);
        let (target_start, target_end) = if up {
            if line_start == 0 {
                return None;
            }
            let prev_end = line_start - 1;
            (Self::line_start(content, prev_end), prev_end)
        } else {
            let line_end = Self::line_end(content, cursor);
            if line_end == content.len() {
                return None;
            }
            let next_start = line_end + 1;
            (next_start, Self::line_end(content, next_start))
        };
        Some(
            content[target_start..target_end]
                .char_indices()
                .nth(goal_column)
                .map_or(target_end, |(i, _)| target_start + i),
        )
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
        // M95: an obscured (password) field's text never leaves it --
        // copy and cut both read through here.
        if state.obscured {
            return None;
        }
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
        self.dirty = true;
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
    /// the identical "engine-core owns the real mutation" split
    /// `dispatch_text_field_key` already established.
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
        self.dirty = true;
        let Some(NodeKind::TextField(state)) = self.nodes.get_mut(field).map(|n| &mut n.kind)
        else {
            return false;
        };
        state.cursor = Self::char_boundary(&state.content, offset);
        state.selection_anchor = None;
        state.goal_column = None;
        self.scroll_text_field_caret_into_view(field);
        true
    }

    /// Shared by `set_text_field_cursor` and `extend_text_field_
    /// selection` (M18 Phase 1/2): clamps a raw byte offset to
    /// `0..=content.len()` and snaps down to the nearest real UTF-8
    /// char boundary -- defensive in both callers (`parley::editing::
    /// Cursor::from_point`'s own result should already land on one),
    /// factored out once a second real caller needed it, not
    /// duplicated.
    pub(super) fn char_boundary(content: &str, offset: usize) -> usize {
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
        self.dirty = true;
        let Some(NodeKind::TextField(state)) = self.nodes.get_mut(field).map(|n| &mut n.kind)
        else {
            return false;
        };
        if state.selection_anchor.is_none() {
            state.selection_anchor = Some(state.cursor);
        }
        state.cursor = Self::char_boundary(&state.content, offset);
        state.goal_column = None;
        self.scroll_text_field_caret_into_view(field);
        true
    }

    /// M53 Phase 1 (§8, §10, §11.3): a real "Select All" (Ctrl+A).
    /// `selection_anchor` at the real start (`0`, always a char
    /// boundary), `cursor` at the real end (`content.len()`, likewise) --
    /// matching every real desktop text field's own Ctrl+A convention:
    /// the whole content becomes selected, cursor lands at the end, not
    /// the start. Returns whether `field` was actually a real
    /// `TextField` -- a no-op on any other kind or a stale/missing
    /// `NodeId`, the same real contract every sibling method here
    /// already has.
    pub fn select_all_text_field(&mut self, field: NodeId) -> bool {
        self.dirty = true;
        let Some(NodeKind::TextField(state)) = self.nodes.get_mut(field).map(|n| &mut n.kind)
        else {
            return false;
        };
        state.selection_anchor = Some(0);
        state.cursor = state.content.len();
        state.goal_column = None;
        self.scroll_text_field_caret_into_view(field);
        true
    }

    /// M32 Phase 6 (§4, §5, §8): a real press on a `Terminal`'s own
    /// cell grid -- sets both `selection_start`/`selection_end` to the
    /// same real `(row, col)`, the identical "a plain click collapses
    /// any active selection" real semantics `set_text_field_cursor`
    /// already established (a collapsed `start == end` is treated as
    /// "no real selection" by `terminal_selected_text` below, mirroring
    /// `text_field_selected_text`'s own `anchor == cursor -> None`).
    /// Whether this turns into a real drag-selection depends entirely
    /// on a genuine `PointerMoved` to a different cell following before
    /// release, the identical real shape `TextField`'s own drag-select
    /// already has. Returns whether `id` was actually a real `Terminal`
    /// -- a no-op on any other kind or a stale/missing `NodeId`.
    pub fn set_terminal_selection_start(&mut self, id: NodeId, row: u16, col: u16) -> bool {
        self.dirty = true;
        let Some(NodeKind::Terminal(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind) else {
            return false;
        };
        state.selection_start = Some((row, col));
        state.selection_end = Some((row, col));
        true
    }

    /// `set_terminal_selection_start`'s own real drag-extend sibling --
    /// moves only `selection_end`, growing the real selection instead
    /// of collapsing it, the identical real split `extend_text_field_
    /// selection` already has from `set_text_field_cursor`.
    pub fn extend_terminal_selection(&mut self, id: NodeId, row: u16, col: u16) -> bool {
        self.dirty = true;
        let Some(NodeKind::Terminal(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind) else {
            return false;
        };
        if state.selection_start.is_none() {
            state.selection_start = Some((row, col));
        }
        state.selection_end = Some((row, col));
        true
    }

    /// M32 Phase 6 (§4, §5, §8): a pure, real read of a `Terminal`'s
    /// own currently selected text -- `None` if `id` isn't a real,
    /// present `Terminal`, or its selection is empty/collapsed (`start
    /// == end`), the identical "not a real selection" definition
    /// `text_field_selected_text` already uses. Never touches a real OS
    /// clipboard itself (`engine-core` has no platform access at all,
    /// §4) -- this only ever answers "what text a real copy would
    /// grab." Real *linear* selection (reading order: row by row, left
    /// to right within each row), the same real default every terminal
    /// emulator uses, not a rectangular block-select. Each real row's
    /// own trailing whitespace is trimmed, joined by `"\n"` (a real
    /// fixed-width grid pads every row with blanks that were never
    /// really "selected" text).
    pub fn terminal_selected_text(&self, id: NodeId) -> Option<String> {
        let NodeKind::Terminal(state) = &self.nodes.get(id)?.kind else {
            return None;
        };
        let start = state.selection_start?;
        let end = state.selection_end?;
        if start == end {
            return None;
        }
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let (start_row, start_col) = start;
        let (end_row, end_col) = end;

        let mut lines = Vec::with_capacity(usize::from(end_row - start_row) + 1);
        for row in start_row..=end_row {
            let col_start = if row == start_row { start_col } else { 0 };
            let col_end = if row == end_row { end_col } else { state.cols };
            let line: String = (col_start..col_end)
                .map(|col| state.cell(row, col).ch)
                .collect();
            lines.push(line.trim_end().to_string());
        }
        Some(lines.join("\n"))
    }
}

/// 0.5.4 (#112, #152): the selection in static text. Each selected text node
/// holds its own range in `TextOptions::selection` (what paints and what a
/// screen reader reads); this is what ties them into one selection, which can
/// run from a point in one text to a point in another.
#[derive(Clone, Debug, Default)]
pub(super) struct StaticSelection {
    /// Where the selection started, and where its moving end is.
    anchor: Option<(NodeId, usize)>,
    focus: Option<(NodeId, usize)>,
    /// The nodes holding a range of it, in document order.
    nodes: Vec<NodeId>,
}

/// 0.5.4 (#112): selection in static text (`TextOptions::selectable`).
impl Tree {
    fn text_len(&self, id: NodeId) -> Option<usize> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Text(state) => Some(state.content.len()),
            _ => None,
        }
    }

    /// Selectable, visible text nodes under `root` in document order, plus the
    /// `extra` nodes whatever their `selectable`.
    fn selectable_texts(&self, root: NodeId, extra: [NodeId; 2]) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            if !node.visible {
                continue;
            }
            if let NodeKind::Text(state) = &node.kind
                && (state.options.selectable || extra.contains(&id))
            {
                out.push(id);
            }
            stack.extend(node.children.iter().rev());
        }
        out
    }

    /// Selects `anchor..focus` (bytes, either order, clamped to character
    /// boundaries) in a text node, clearing any other selection first.
    /// `false`, changing nothing, if `id` isn't a text node.
    pub fn set_text_selection(&mut self, id: NodeId, anchor: usize, focus: usize) -> bool {
        self.select_across((id, anchor), (id, focus))
    }

    /// Selects from `anchor` to `focus`, each a text node and a byte offset
    /// (clamped to a character boundary). When they are in different nodes,
    /// every selectable text between them in document order is selected whole,
    /// and the two ends from their offset to the node's end, or its start to the
    /// offset. `false`, changing nothing, unless both are text nodes of one tree.
    pub fn select_across(&mut self, anchor: (NodeId, usize), focus: (NodeId, usize)) -> bool {
        let snap = |tree: &Self, (id, offset): (NodeId, usize)| match &tree.nodes.get(id)?.kind {
            NodeKind::Text(state) => Some((id, Self::char_boundary(&state.content, offset))),
            _ => None,
        };
        let (Some(anchor), Some(focus)) = (snap(self, anchor), snap(self, focus)) else {
            return false;
        };
        let root = self.root_of(anchor.0);
        if self.root_of(focus.0) != root {
            return false;
        }
        let order = self.selectable_texts(root, [anchor.0, focus.0]);
        let position = |id: NodeId| order.iter().position(|n| *n == id);
        let (Some(ia), Some(ifo)) = (position(anchor.0), position(focus.0)) else {
            return false;
        };
        // The ranges each node gets.
        let mut ranges: Vec<(NodeId, (usize, usize))> = Vec::new();
        if anchor.0 == focus.0 {
            ranges.push((anchor.0, (anchor.1, focus.1)));
        } else {
            let (first, last) = if ia < ifo {
                (anchor, focus)
            } else {
                (focus, anchor)
            };
            let (lo, hi) = (ia.min(ifo), ia.max(ifo));
            for &id in &order[lo..=hi] {
                let len = self.text_len(id).unwrap_or(0);
                let range = if id == first.0 {
                    (first.1, len)
                } else if id == last.0 {
                    (0, last.1)
                } else {
                    (0, len)
                };
                ranges.push((id, range));
            }
        }
        let keep: Vec<NodeId> = ranges.iter().map(|(id, _)| *id).collect();
        let previous = std::mem::take(&mut self.static_selection.nodes);
        for id in previous.into_iter().filter(|id| !keep.contains(id)) {
            if let Some(NodeKind::Text(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind) {
                state.options.selection = None;
            }
        }
        for (id, range) in ranges {
            if let Some(NodeKind::Text(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind) {
                state.options.selection = Some(range);
            }
        }
        self.static_selection = StaticSelection {
            anchor: Some(anchor),
            focus: Some(focus),
            nodes: keep,
        };
        self.dirty = true;
        true
    }

    /// Moves the focus end of the selection to `focus` in text node `id`,
    /// keeping its anchor (a drag), even if that is another text node; starts
    /// a selection at `focus` if there is none.
    pub fn extend_text_selection(&mut self, id: NodeId, focus: usize) -> bool {
        match self.static_selection.anchor {
            Some(anchor) if self.text_len(anchor.0).is_some() => {
                self.select_across(anchor, (id, focus))
            }
            _ => self.set_text_selection(id, focus, focus),
        }
    }

    /// Makes text node `id` the owner of the static selection if it has one,
    /// or releases it if it was the owner and has none: for code that sets
    /// `TextOptions::selection` directly on the node.
    pub fn adopt_text_selection(&mut self, id: NodeId) {
        let selection = match self.nodes.get(id).map(|n| &n.kind) {
            Some(NodeKind::Text(state)) => state.options.selection,
            _ => None,
        };
        match selection {
            Some((a, f)) => {
                // Sets exactly this node's range, clearing the rest.
                let previous = std::mem::take(&mut self.static_selection.nodes);
                for other in previous.into_iter().filter(|n| *n != id) {
                    if let Some(NodeKind::Text(state)) =
                        self.nodes.get_mut(other).map(|n| &mut n.kind)
                    {
                        state.options.selection = None;
                    }
                }
                self.static_selection = StaticSelection {
                    anchor: Some((id, a)),
                    focus: Some((id, f)),
                    nodes: vec![id],
                };
            }
            None if self.static_selection.nodes.contains(&id) => self.clear_text_selection(),
            None => {}
        }
    }

    /// Clears the selection in static text, wherever it is.
    pub fn clear_text_selection(&mut self) {
        let StaticSelection { nodes, .. } = std::mem::take(&mut self.static_selection);
        if nodes.is_empty() {
            return;
        }
        for id in nodes {
            if let Some(NodeKind::Text(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind) {
                state.options.selection = None;
            }
        }
        self.dirty = true;
    }

    /// 0.5.4 (#131): the link at byte `offset` of text node `id` (the last
    /// span covering it that has one), if any.
    pub fn text_link_at(&self, id: NodeId, offset: usize) -> Option<&str> {
        let NodeKind::Text(state) = &self.nodes.get(id)?.kind else {
            return None;
        };
        state
            .options
            .spans
            .iter()
            .rev()
            .find(|s| s.link.is_some() && s.start <= offset && offset < s.end)
            .and_then(|s| s.link.as_deref())
    }

    /// 0.5.4 (#131): Ctrl+A in static text: selects all of the text node the
    /// selection started in. `false` if there is no selection.
    pub fn select_all_static_text(&mut self) -> bool {
        let Some((owner, _)) = self.static_selection.anchor else {
            return false;
        };
        let Some(len) = self.text_len(owner) else {
            return false;
        };
        self.set_text_selection(owner, 0, len)
    }

    /// 0.5.4 (#131): Shift+Left, Shift+Right, Shift+Home or Shift+End in
    /// static text: moves the selection's focus end one character (or to the
    /// start or end of its text), keeping its anchor. `false` if there is no
    /// selection.
    pub fn extend_static_selection(&mut self, key: crate::Key) -> bool {
        let (Some(anchor), Some((owner, focus))) =
            (self.static_selection.anchor, self.static_selection.focus)
        else {
            return false;
        };
        let Some(NodeKind::Text(state)) = self.nodes.get(owner).map(|n| &n.kind) else {
            return false;
        };
        let content = &state.content;
        let focus = Self::char_boundary(content, focus);
        let moved = match key {
            crate::Key::ArrowLeft => content[..focus]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i),
            crate::Key::ArrowRight => content[focus..]
                .chars()
                .next()
                .map_or(focus, |c| focus + c.len_utf8()),
            crate::Key::Home => 0,
            crate::Key::End => content.len(),
            _ => return false,
        };
        self.select_across(anchor, (owner, moved))
    }

    /// The text a Copy of the static selection would take, the nodes' pieces
    /// joined by newlines: `None` for no selection or an empty one.
    pub fn static_selected_text(&self) -> Option<String> {
        let pieces: Vec<String> = self
            .static_selection
            .nodes
            .iter()
            .filter_map(|id| self.text_selected_text(*id))
            .collect();
        (!pieces.is_empty()).then(|| pieces.join("\n"))
    }

    /// The selected text of text node `id`, if any is selected.
    pub fn text_selected_text(&self, id: NodeId) -> Option<String> {
        let NodeKind::Text(state) = &self.nodes.get(id)?.kind else {
            return None;
        };
        let (a, b) = state.options.selection?;
        let (start, end) = (a.min(b), a.max(b));
        let start = Self::char_boundary(&state.content, start);
        let end = Self::char_boundary(&state.content, end);
        (start < end).then(|| state.content[start..end].to_string())
    }
}
