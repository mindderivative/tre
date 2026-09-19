# PLAN — M38 Phase 2: Code Editor Goal-Column Memory

## Goal
Close the real, previously-documented v1 gap: `Tree::dispatch_text_
field_key`'s `ArrowUp`/`ArrowDown` re-derived a `TextFieldState`'s
own real *column* fresh from wherever the cursor currently sat each
call, rather than remembering the original column across a
consecutive run of vertical moves -- so hopping up through a shorter
line permanently lost the original column, unlike every real desktop
text editor.

## Steps
1. Confirmed the exact gap by direct source read: `move_to_line`
   computed `column` fresh from `content[line_start..cursor]` every
   call; `crates/engine-core/src/tree.rs`'s own pre-existing test
   (`arrow_up_and_down_move_the_cursor_by_line_preserving_its_own_
   real_column`) explicitly documented this as "a real, deliberate
   v1 simplification, not a bug."
2. Added `TextFieldState.goal_column: Option<usize>` (`node.rs`):
   `Some(column)` while a consecutive `ArrowUp`/`ArrowDown` sequence
   is in progress, `None` otherwise. Defaulted to `None` in
   `TextFieldState::new`.
3. New `Tree::real_column` helper (the old fresh-every-call
   computation, now also used to *seed* `goal_column` the first time
   a sequence begins). `Tree::move_to_line` gained a `goal_column:
   usize` parameter (replacing its own internal `column` variable) --
   the caller decides which column to land at, not the callee.
4. `ArrowUp`/`ArrowDown` arms in `dispatch_text_field_key`: seed
   `goal_column` via `get_or_insert_with(|| Self::real_column(...))`
   on first use, read (not clear) it on every further hop in the same
   sequence, pass it straight to `move_to_line`.
5. Reset `goal_column` everywhere else a cursor genuinely moves for a
   different reason: a blanket `if !matches!(key, ArrowUp | ArrowDown)
   { state.goal_column = None }` at the top of `dispatch_text_field_
   key` (covers Backspace/Delete/ArrowLeft/Right/Home/End/Space/
   Enter/Tab in one place); inside the shared `delete_selection`
   helper (covers Backspace/Delete/Space/TextInput/`cut_text_field_
   selection`, the last of which doesn't route through `dispatch_
   text_field_key` at all); `set_text_field_cursor`/`extend_text_
   field_selection` (mouse click/drag); the `InputEvent::TextInput`
   dispatch arm (typing). Also reset on the "collapse an active
   selection" branch inside `ArrowUp`/`ArrowDown` themselves -- a
   selection collapse isn't part of a continuous vertical-navigation
   sequence.
6. Rewrote the existing test to assert the new, corrected round-trip
   behavior (single ArrowUp then ArrowDown now returns exactly to the
   starting cursor position, recalling the real goal column, instead
   of the old test's explicit "not a bug" assertion of the opposite).
   Added two new dedicated Rust tests: a full up-up-down-down round
   trip through a genuinely shorter line proving the goal survives
   the whole sequence and returns to the exact start; a non-vertical-
   move-resets-the-goal test proving an ArrowLeft in between forces
   the next ArrowUp to derive a fresh goal instead of reusing a stale
   one.
7. Constructed a real Python-level reproduction too (checked the
   Python API first rather than assuming none exists, unlike M37's
   VirtualList case where the API genuinely had no path): no Python
   getter exists for a `TextField`'s own raw cursor offset, but the
   same `type_text`-after-navigation-then-`get_text()` positional
   probe this file's own pre-existing `test_arrow_up_navigates_to_
   the_previous_line` already uses can observe it indirectly. Two new
   pytest tests added to `tests/test_code_editor.py`, mirroring the
   two new Rust tests.
8. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 75 examples, showcase demo.
9. `BUILD_TRACKER.md` Phase 2 flipped to done (terse step-bullet note,
   full writeup here in `PLAN.md`/`LOG.md`), Top Metrics row updated
   to 2-of-7, artifact regenerated (38/122/212, unchanged) and
   republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 185 passed, up from 183, +2 new tests;
`pytest tests/`: 558 passed/1 skipped, up from 556, +2 new tests; all
75 examples + showcase demo clean). **M38 Phase 2 -- Code Editor
Goal-Column Memory is now complete. M38 itself remains open: 5 phases
remain (fold-aware cursor navigation, Split Button inner-corner
shape-tightening, Button Group per-child shape change, ScrollView
scrollbar thumb, real scroll+clip for Code Editor).**
