# LOG — M38 Phase 2: Code Editor Goal-Column Memory

- User's own explicit instruction: "Let's knock out the known gaps"
  -- M38's own second, still-cheap/bounded phase, per the milestone's
  own scoping order.
- Confirmed the real gap by direct source read before writing any
  code: `Tree::move_to_line` (`crates/engine-core/src/tree.rs`)
  re-derived its own real *column* fresh from `content[line_start..
  cursor]` on every call -- it never remembered the original column
  from before an intermediate hop landed on a shorter line. The
  file's own pre-existing test explicitly labeled this "a real,
  deliberate v1 simplification, not a bug," matching exactly what
  `BUILD_TRACKER.md`'s own M38 scoping note named as Phase 2's real
  target.
- Added `TextFieldState.goal_column: Option<usize>` (`crates/engine-
  core/src/node.rs`): `Some(column)` while a consecutive `ArrowUp`/
  `ArrowDown` run is in progress, `None` otherwise (the default, set
  in `TextFieldState::new`).
- New `Tree::real_column(content, cursor)` helper -- the exact old
  fresh-every-call computation, kept as a real, named function since
  it's still needed to *seed* `goal_column` the first time a sequence
  begins. `Tree::move_to_line` itself changed from computing its own
  `column` internally to taking a `goal_column: usize` parameter --
  the caller now decides which column to land at.
- `ArrowUp`/`ArrowDown` arms in `dispatch_text_field_key`: `let goal
  = *state.goal_column.get_or_insert_with(|| Self::real_column(&state.
  content, state.cursor));` seeds the goal only the first time (when
  `None`), then reads the same already-set value on every further
  consecutive hop -- `move_to_line` receives `goal`, never `state.
  cursor`'s own current column directly. Compiled clean on the first
  `cargo check` -- confirmed Rust's disjoint closure field capture
  (the closure borrows `state.content`/`state.cursor` while the
  method receiver mutably borrows the separate `state.goal_column`
  field, both of the same `state: &mut TextFieldState`) works exactly
  as expected here, no workaround needed.
- Reset `goal_column` everywhere else a cursor genuinely moves for a
  reason other than a consecutive vertical hop -- enumerated by
  direct grep for every real `state.cursor = `/`+=` site in `tree.rs`
  before writing any reset, not guessed:
  1. A blanket `if !matches!(key, Key::ArrowUp | Key::ArrowDown) {
     state.goal_column = None; }` at the very top of `dispatch_text_
     field_key`, before the real per-key `match` -- covers
     `Backspace`/`Delete`/`ArrowLeft`/`ArrowRight`/`Home`/`End`/
     `Space`/`Enter`/`Tab` in one place rather than nine separate
     edits.
  2. Inside the shared `delete_selection` helper (only on its real
     "a selection actually existed and was removed" `true` path) --
     covers `Backspace`/`Delete`/`Space`/`TextInput`'s own selection-
     replace path, and `cut_text_field_selection`, the one real
     caller that doesn't route through `dispatch_text_field_key` at
     all (a separate public method), previously missed by the
     blanket reset above.
  3. `set_text_field_cursor`/`extend_text_field_selection` (real
     mouse click/drag-select) and the `InputEvent::TextInput`
     dispatch arm (a real inserted character/IME commit) -- both
     mutate `state.cursor` outside `dispatch_text_field_key` entirely.
  4. The "collapse an active selection" branch inside `ArrowUp`/
     `ArrowDown` themselves (pressing an unshifted arrow while a
     selection is active) -- a selection collapse is a real, distinct
     cursor move, not part of a continuous vertical-navigation
     sequence, so it resets the goal too rather than inheriting
     whatever was set before the selection existed.
- Rewrote the existing test (`arrow_up_and_down_move_the_cursor_by_
  line_preserving_its_own_real_column`): removed its own second
  `ArrowUp` step and its comment explicitly asserting the old "not a
  bug" behavior; its final `ArrowDown` assertion changed from `8`
  (the old buggy landing) to `14` (a full round trip back to the
  exact original starting cursor position, since the real goal column
  5 is now correctly remembered through "hi"'s own shorter line).
- Added two new, decisive Rust tests directly proving the fix:
  `arrow_up_and_down_remember_a_real_goal_column_through_a_shorter_
  line` -- a genuine up-up-down-down round trip through
  "alphabet\nhi\nbanana" that lands back on the *exact* original
  starting byte offset, with an explicit contrast in its own
  assertion message showing what byte the old buggy behavior would
  have produced instead (2, landing on 'p') versus the real fix (6,
  landing on 'e'); `a_non_vertical_move_resets_the_remembered_goal_
  column` -- proves an `ArrowLeft` in the middle of a vertical run
  forces the next `ArrowUp` to derive a genuinely fresh goal (1) from
  wherever the cursor now sits, not the stale original (6).
- Real, honest verification-surface check done properly this time,
  unlike the reflexive "no Python API exposes this" assumption M37
  made for `VirtualList` (which turned out to be correct there, but
  only after checking): grepped `python/tre/_core.pyi` for a cursor
  getter first -- confirmed none exists, so `state.cursor` itself
  can't be read directly from Python. But the *effect* of where the
  cursor landed is still observable: `test_code_editor.py`'s own pre-
  existing `test_arrow_up_navigates_to_the_previous_line` already
  proves this exact technique (navigate, then `type_text` a marker,
  then read `get_text()` to see exactly where it landed). Reused it
  for two new real Python-level tests: `test_arrow_up_and_down_
  remember_a_real_goal_column_through_a_shorter_line` (asserts
  `get_text() == "alphabXet\nhi\nbanana"`, proving the marker landed
  at "alphabet"'s own real column 6, not "hi"'s clamped column 2's
  equivalent position `"alXphabet..."`) and `test_a_non_vertical_
  move_resets_the_remembered_goal_column` (asserts `"aXlphabet\nhi\n
  banana"` after an interrupting `ArrowLeft`). Both passed on the
  first run.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 185
  passed, up from 183, exactly the 2 new tests, zero regressions);
  `maturin develop --release` rebuilt; `pytest tests/` 558 passed/1
  skipped, up from 556, exactly the 2 new tests; all 75 examples and
  the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md`: Phase 2 flipped `⬜` -> `✅` with a
  terse step-bullet note; milestone status line and Top Metrics row
  updated to "Phase 2 of 7 done" / 29%. Verified the parser's own
  reported item count unchanged before/after (38/122/212 both times).
  Regenerated and republished the Build Tracker artifact.
  **This closes M38 Phase 2. M38 itself remains open -- 5 phases
  remain (fold-aware cursor navigation, Split Button inner-corner
  shape-tightening, Button Group per-child shape change on
  press/select, ScrollView scrollbar thumb, real scroll+clip for Code
  Editor with caret-follow).**
