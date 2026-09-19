# PLAN — M31 Phase 3: Tab/Space Indicators

## Goal
Real, visible glyphs for space/tab in `Window.add_code_editor`, at
paint time only — `state.content` itself never touched.

## Steps
1. Confirmed the real design constraint before writing any code: a
   naive character substitution (`·`/`→` for space/tab) would desync
   `state.cursor`/`selection_anchor`'s own real byte offsets from the
   substituted `Layout`'s byte space, since the replacement glyphs are
   multi-byte in UTF-8 while space/tab are one byte each — a real
   correctness bug (broken caret/selection, or an out-of-bounds/
   mid-character byte offset reaching `Tree::dispatch`), not just a
   cosmetic detail.
2. Designed a real, deterministic bidirectional byte-offset map
   (`to_display_offset`/`from_display_offset`, `engine-render::text`)
   — correct because the substitution is exactly one real char in for
   one real char out, never expanding/collapsing multiple characters.
3. Added `TextFieldState.show_whitespace: bool` (default `false`,
   `multiline`'s own exact precedent), set `true` specifically by
   `Window.add_code_editor` — not exposed on `add_text_field`.
4. Wired `TextRenderer::draw_field` to remap `state.cursor`/`anchor`/
   `caret_at` through the map before querying the substituted
   `Layout` (skipped while a real IME preedit is simultaneously
   active, a vanishingly rare combination — the preedit splice keeps
   its own already-correct unsubstituted offsets then).
5. Wired `TextRenderer::hit_test_position` to build the identical
   substituted `Layout` for a real click and map the *returned*
   display-space byte offset back into real content-space before
   returning it.
6. Wrote 2 new `engine-render` unit tests (a real round-trip of every
   char boundary in a mixed ASCII/space/tab/multi-byte string; a
   direct substitution-formula check) and 1 new integration test
   (`hit_test_position` on a substituted field resolves a far-right
   click to the real, short content length, not the longer substituted
   one — exactly the bug class a broken remap would produce).
7. Ran a real, direct empirical script before writing any pytest:
   `get_text()` on a code editor with live-typed spaces/tabs stays
   completely unsubstituted.
8. Extended `tests/test_code_editor.py` (1 new test) and
   `examples/code_editor.py` (docstring + a real render-loop re-run
   now exercising the substitution paint path with live content)
   rather than new files.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 68 examples, showcase demo, mypy
   --strict.
10. Update `BUILD_TRACKER.md` — verified the parser's own reported
    item count before/after (191, unchanged), regenerate + republish
    the Build Tracker artifact.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-render`
7 unit tests up from 5, 10 tests in `text_field_paint.rs` up from 9,
`pytest tests/` 501 passed/1 skipped up from 500, all 68 examples,
showcase demo). Real space/tab characters genuinely paint as visible
`·`/`→` glyphs while `state.content`/cursor/selection/click-to-position
all stay byte-for-byte correct against the original content, proven
by direct round-trip and out-of-bounds-resolution tests, not assumed.
