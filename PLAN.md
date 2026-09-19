# PLAN — M31 Phase 4: Syntax Highlighting

## Goal
Real per-token coloring for `Window.add_code_editor`, via
`RangedBuilder::push(StyleProperty::Brush(color), range)`. App-side
tokenization only — no engine-bundled lexer.

## Steps
1. Investigated the real open technical question the scoping note
   left: whether `vello_hybrid` reads per-style-run brush data
   directly, or whether `draw_field`'s paint loop needed
   restructuring. Resolved by direct source read: `vello_hybrid`'s own
   `Scene::glyph_run`/`fill_glyphs` never reads a brush at all (color
   stays the existing `scene.set_paint` mechanism) — but the real,
   load-bearing finding was that a `parley::Run` does *not* necessarily
   split at every style boundary; each individual `parley::Glyph`
   instead carries its own real `style_index` into `Layout::styles()`.
2. Added `TextFieldState.syntax_spans: Vec<(Range<usize>, Color)>`
   (empty default), and `Node.set_syntax_spans` in engine-py.
3. Extended `shaped_layout` to push a real default `Brush` (`at.color`)
   covering the whole content, then a real per-span override for each
   real syntax span — both now part of the real shaping-cache key.
4. **First design attempt was wrong, caught live by the very first
   pixel test written for this phase:** matched color per-*run* via
   `Run::text_range()`, assuming the `Brush` push always forced a run
   split. A real three-way pixel-diff test (no spans / whole-content
   span / first-character-only span) failed immediately — "half" and
   "whole" rendered pixel-identical. Root-caused via `parley::Cluster::
   first_style`'s own real source and fixed by reading each glyph's own
   real `style_index` directly instead, batching consecutive
   same-color glyphs into one `fill_glyphs` call (mirrors
   `draw_terminal`'s own real run-batching). The test then passed.
5. Wired the real per-span offset remapping through the identical
   `to_display_offset` machinery M31 Phase 3 already built, so
   whitespace substitution and syntax highlighting stay correct
   together.
6. Ran a real, direct empirical script before writing any pytest: real
   spans set without raising, `get_text()` stays unsubstituted, a
   non-`TextField` node correctly rejects the call.
7. Extended `tests/test_code_editor.py` (2 new tests) and
   `examples/code_editor.py` (a real, minimal app-side keyword
   tokenizer) rather than new files.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 68 examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` — verified the parser's own reported item
   count before/after (191, unchanged), regenerate + republish the
   Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-render`
7 unit tests unchanged, 11 tests in `text_field_paint.rs` up from 10 —
including the real pixel-diff test that caught and proved the fix for
a genuine bug, not merely plausible-sounding code — `pytest tests/`
503 passed/1 skipped up from 501, all 68 examples, showcase demo). Real
per-token syntax coloring genuinely paints only its own real byte
range, confirmed by direct pixel comparison, not assumed.
