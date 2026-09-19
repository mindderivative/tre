# PLAN — M30 Phase 9 Step 3: Code Editor

## Goal
Add `Window.add_code_editor` — closing the real, stated single-line-
only gap `TextField` always had (`Enter` consumed, never inserted a
newline), so a genuinely multiline editing surface exists.

## Steps
1. Read pyCopper's own real `CodeEditor` widget for its real design
   (line-number gutter, syntax highlighting via optional Pygments,
   monospace font default, never-wraps layout, Tab-capture for
   indentation) and its own stated v1 deferrals.
2. Confirmed via direct source read: `TextField` is deliberately
   single-line only (`Tree::dispatch_text_field_key`'s own
   `Key::Enter => Some(DispatchOutcome::None)` comment).
3. Design: reuse `NodeKind::TextField` directly with a new
   `TextFieldState.multiline: bool` field (default `false`), not a
   new `NodeKind` -- `engine-core`'s own editing model has no M3
   chrome to strip, unlike pyCopper's own real reason for building
   alongside `TextFieldElement` rather than subclassing it.
4. Confirmed via direct source read: `engine_core::Key` has no
   `ArrowUp`/`ArrowDown` variants at all -- real, load-bearing new
   infrastructure needed for line navigation, not just new match arms.
5. Design line navigation as pure string/column logic (no `parley`
   access needed, respecting §4's crate boundary) rather than real
   pixel-based cursor geometry -- correct for a genuinely monospace
   editor, though this codebase has no bundled monospace font yet (a
   real, separate, stated gap).
6. Add `Key::ArrowUp`/`ArrowDown` to the enum; thread through
   `engine_platform::translate_key` (real winit mapping) and
   `Window.press_key` (synthetic-testing string vocabulary); fix the
   two exhaustive `match key` compile errors this raised.
7. Implement `Enter` (multiline: insert `\n`), `Home`/`End` (multiline:
   per-line via `Tree::line_start`/`line_end`), and `ArrowUp`/`Down`
   (multiline: `Tree::move_to_line`, column-preserving) in
   `dispatch_text_field_key`.
8. Write 7 new `engine-core` unit tests -- caught and fixed one real
   math error in the process (a second consecutive `ArrowUp` does NOT
   preserve the original column across an intervening shorter line,
   since this v1 has no persistent goal-column memory; verified via a
   Python simulation before trusting the Rust assertion).
9. Fix `engine-render`'s own layout: `break_all_lines(Some(f32::MAX))`
   and `None` are the identical real no-wrap value (confirmed via
   direct source read) -- new `field_max_width` helper picks it when
   `state.multiline`, at the two real call sites (`draw_field`/
   `hit_test_position`).
10. Write a new, GPU-free `engine-render` test using
    `hit_test_position` proving a real `\n` produces a real second,
    vertically-stacked layout line.
11. Implement `Window.add_code_editor` in `window_factory.rs`, reusing
    `add_text_field`'s exact real pattern (theme resolution, access
    role) plus `multiline: true` -- deliberately does NOT expose
    `font_family` (this catalog's only registered fonts are Roboto/
    Noto Sans Arabic, confirmed via direct source read of
    `TextRenderer::new`'s "system font discovery is deliberately OFF").
12. Add `.pyi` stub.
13. Write `tests/test_code_editor.py` and `examples/code_editor.py` --
    checked for a filename collision first this time
    ([[feedback_check_before_new_example_file]]).
14. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
15. Update `BUILD_TRACKER.md` (Top Metrics row, Step 3 line,
    "Just closed"/"Up next" trailer), regenerate + republish the
    Build Tracker artifact.
16. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-core`
158 up from 151, `engine-render` `text_field_paint` 9 up from 8, 474
pytest passed/1 skipped up from 465, all 65 examples, showcase demo,
44 Rust test binaries).
