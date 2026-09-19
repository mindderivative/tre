# LOG — M31 Phase 3: Tab/Space Indicators

- Identified the real design constraint before writing any code: `·`
  (U+00B7) and `→` (U+2192), real substitute glyphs for space/tab, are
  multi-byte in UTF-8 while space/tab are one byte each -- a naive
  character substitution into `display_content` would silently desync
  `state.cursor`/`selection_anchor`'s own real byte offsets from the
  substituted `Layout`'s own byte space. A real, confirmed correctness
  risk, not a cosmetic detail: a broken caret/selection position, or
  an out-of-bounds/mid-character byte offset reaching `Tree::
  dispatch`'s own real cursor-mutation code.
- Designed a real, deterministic bidirectional byte-offset map
  (`to_display_offset`/`from_display_offset`, new free functions in
  `engine-render::text`) -- correct because the substitution is
  exactly one real char in for one real char out, never expanding or
  collapsing multiple characters, so a simple linear char-by-char scan
  suffices in both directions.
- Added `TextFieldState.show_whitespace: bool` to `engine-core::node`
  (default `false`, the identical `multiline`-established pattern --
  every existing construction site stays byte-for-byte unchanged).
- Wired `TextRenderer::draw_field`: after the existing real IME-preedit
  splice, applies `substitute_whitespace` to `display_content` and
  remaps `state.cursor`/`selection_anchor`/`caret_at` through `to_
  display_offset` -- but only when no real preedit is simultaneously
  active (a vanishingly rare real combination; the preedit splice's
  own already-correct unsubstituted offsets are left alone then,
  a real, honest, stated v1 simplification).
- Wired `TextRenderer::hit_test_position`: builds the identical
  substituted layout for a real click, then maps the *returned*
  display-space byte offset back into `state.content`'s own real byte
  space via `from_display_offset` before returning it.
- Set `TextFieldState.show_whitespace = true` in `Window.
  add_code_editor` specifically -- not exposed on `add_text_field`,
  since an ordinary form input showing visible dots for its own real
  spaces would be unasked-for visual noise.
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check`/`cargo test --release` all clean.
- Wrote 2 new `engine-render` unit tests: a real round-trip of every
  char boundary in a mixed ASCII/space/tab/multi-byte-UTF-8 string
  through both offset-mapping directions; a direct check that only
  space/tab get substituted, never any other real character. Both
  passed on the first run (`engine-render` 7 unit tests, up from 5).
- Wrote 1 new `hit_test_position` integration test in `text_field_
  paint.rs`: a real click far past a short substituted field's own end
  ("a b", 3 real bytes) resolves to `state.content.len()` (3), not the
  longer substituted string's own length (4, for "a·b") -- exactly the
  class of bug a broken remap would produce. Passed on the first run
  (10 tests in `text_field_paint.rs`, up from 9).
- Rebuilt the Python extension. **Ran a real, direct empirical
  end-to-end script before writing any pytest suite**: a code editor
  seeded with real space/tab content, then live-typed more of the
  same, reads back completely unsubstituted via `get_text()` both
  times. Passed.
- Extended `tests/test_code_editor.py` with 1 new test (a real
  space/tab-containing buffer, edited live, still reads back
  completely unsubstituted). Passed on the first run.
- Extended `examples/code_editor.py`'s own docstring and let its
  existing real `App.run()` call exercise the new whitespace-
  substitution paint path directly (the buffer it edits live already
  contains real tabs from the M31 Phase 2 step) -- clean on the first
  run, no crash.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-render` 7 unit tests up from
  5, 10 in `text_field_paint.rs` up from 9), `maturin develop
  --release`, `pytest tests/` (501 passed, 1 skipped, up from 500 --
  1 new, zero regressions), all 68 examples (including the updated
  `examples/code_editor.py`) and the showcase demo re-run clean,
  `mypy --strict` clean against `examples/code_editor.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 50%, Phase 3 heading
  ✅, Step 1 marked done) -- verified the parser's own reported item
  count before/after (191, unchanged, since no bullets were added or
  removed, only an existing one filled in), regenerated and
  republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
