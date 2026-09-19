# LOG — M31 Phase 2: Tab-Key Indentation Capture

- Located the real existing chokepoint before writing any code:
  `Tree::dispatch_text_field_key`'s own `Key::Tab | Key::Escape =>
  None` catch-all arm -- the identical real "first refusal, `None`
  means not mine" contract already established for `ArrowLeft`/
  `ArrowRight`/`Home`/`End`/`Enter`, confirmed via direct read rather
  than assumed to need a new mechanism.
- Split it into a real `Key::Tab` arm: a *multiline* field inserts a
  literal `\t` (via `Self::delete_selection(state)` first, the same
  shared helper `Space`/`Enter` already use for an active selection,
  then `state.content.insert(state.cursor, '\t')`), returning
  `Some(Changed(field))`; a single-line field returns `None`
  unchanged, falling through to the existing `move_focus` handling
  exactly as before. `Escape` kept its own separate, always-`None`
  arm.
- Updated two stale doc comments that described Tab as *always*
  falling through to focus traversal: `dispatch_text_field_key`'s own
  doc comment and `Tree::dispatch`'s `KeyPressed` call-site comment.
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check`/`cargo test --release` all clean.
- Wrote 2 new engine-core unit tests: a real Tab keypress on a
  multiline field inserts `\t` and keeps focus; a real Tab keypress
  with an active selection replaces it rather than inserting beside
  it. Both passed on the first run, alongside the pre-existing
  single-line "Tab still moves focus" test, re-confirmed unchanged
  (`engine-core` 165, up from 163).
- Rebuilt the Python extension. **Ran a real, direct empirical
  end-to-end script before writing any pytest suite**: a real Tab
  keypress on a focused multiline Code Editor inserts `\t` and stays
  focused; the identical keypress on a focused single-line `TextField`
  still moves focus away. Both passed.
- Extended `tests/test_code_editor.py` with 2 new tests (multiline Tab
  indents in place and keeps focus; single-line `TextField` still
  loses focus on Tab and inserts nothing) -- a second-focusable-node
  fixture was needed for the single-line case (a lone focusable node
  trivially wraps focus back onto itself, a real test-fixture finding,
  not an implementation bug). Both passed after that fix.
- Extended `examples/code_editor.py` with a real Tab-key indentation
  step appended to its existing live-edit sequence, rather than a new
  example file -- a real enhancement to Code Editor's own existing
  coverage. Clean on the first run once the expected text (a real `\t`
  lands *before* the line's own existing leading spaces, not replacing
  them) was corrected.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-core` 165, up from 163),
  `maturin develop --release`, `pytest tests/` (500 passed, 1 skipped,
  up from 498 -- 2 new, zero regressions), all 68 examples (including
  the updated `examples/code_editor.py`) and the showcase demo re-run
  clean, `mypy --strict` clean against `examples/code_editor.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 33%, Phase 2 heading
  ✅, Step 1 marked done) -- verified the parser's own reported item
  count before/after (191, unchanged, since no bullets were added or
  removed, only an existing one filled in), regenerated and
  republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
