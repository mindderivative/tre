# PLAN — M31 Phase 5: Code Folding

## Goal
Real code folding for `Window.add_code_editor`. Flagged in its own
scoping note as "the most speculative phase in this milestone" — no
real reference implementation existed to design it from.

## Steps
1. Checked pyCopper's own real `CodeEditor` before designing anything
   and confirmed it explicitly excludes code folding too ("code
   folding... out of scope for this pass"). Paused and asked the user
   directly via `AskUserQuestion`, matching the established discipline
   for genuinely large, ungrounded builds (Terminal, Carousel). The
   user chose "Full real folding (Recommended)."
2. Designed real content-hiding by reusing the identical `display_
   content`-splice pattern IME preedit (M17 Phase 2) already
   established: each real folded byte range collapses into one
   visible "⋯" (U+22EF) marker (`engine-render::text::
   elide_folded_ranges`).
3. Designed a real, segment-based bidirectional byte-offset map
   (`to_display_offset_folded`/`from_display_offset_folded`) — a
   deliberate, real v1 clamp for an offset landing inside a fold
   (resolves to right after that fold's own marker).
4. Restructured `draw_field`'s own offset handling into one shared
   `to_display` closure that chains folding, then whitespace
   substitution (M31 Phase 3) — so cursor/selection/caret/syntax spans
   (M31 Phase 4) all stay correct together, whichever real combination
   of the three paint transforms is active.
5. Added `TextFieldState.folded_ranges: Vec<Range<usize>>` (empty
   default) and `Node.set_folded_ranges` in engine-py, the identical
   real contract `set_syntax_spans` already has.
6. Wrote 5 new `engine-render` unit tests (elision, malformed-range
   skipping, round-trip mapping, in-fold clamping, marker-click
   resolution) and 2 new integration tests (a real pixel-diff proof
   folded content paints differently; `hit_test_position` past a fold
   resolves to a real content offset) — all passed on the first run.
7. Ran a real, direct empirical script before writing any pytest: real
   fold ranges set without raising, content stays unsubstituted, a
   non-`TextField` node rejects the call, and folding + syntax
   highlighting + whitespace indicators compose cleanly through a real
   render loop.
8. Extended `tests/test_code_editor.py` (4 new tests) and added
   `examples/code_editor_folding.py` (a real gutter toggle affordance,
   composed entirely from existing primitives, folding/unfolding a
   real function body through two real clicks) — both checked for
   filename collisions first.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 69 examples, showcase demo, mypy
   --strict.
10. Update `BUILD_TRACKER.md` — verified the parser's own reported
    item count before/after (191, unchanged), regenerate + republish
    the Build Tracker artifact.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-render`
12 unit tests up from 7, 13 tests in `text_field_paint.rs` up from 11,
`pytest tests/` 506 passed/1 skipped up from 503, all 69 examples,
showcase demo). Real content folding genuinely hides folded text and
shows a real "⋯" marker instead, with cursor/selection/syntax-span
byte offsets all staying correct against the real display layout,
confirmed by direct Rust-level and pixel-diff tests, not assumed. A
real, stated v1 limitation: cursor navigation is not fold-aware.
