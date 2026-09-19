# LOG — M31 Phase 5: Code Folding

- Checked pyCopper's own real `CodeEditor` before designing anything,
  the same real discipline every prior phase this milestone already
  used -- confirmed it explicitly excludes code folding too ("code
  folding... out of scope for this pass"), the first phase this whole
  milestone with no real reference implementation to design from.
- Paused and asked the user directly via `AskUserQuestion`, matching
  the established discipline for genuinely large, ungrounded builds
  (Terminal, Carousel). The user chose "Full real folding
  (Recommended)" -- a real fold-state model, a real gutter toggle
  affordance, and real content-hiding.
- Designed real content-hiding by reusing the identical `display_
  content`-splice pattern IME preedit (M17 Phase 2) already
  established: each real folded byte range collapses into one visible
  "⋯" (U+22EF MIDLINE HORIZONTAL ELLIPSIS) marker -- the same real
  "something is hidden here" convention VS Code/Sublime Text both use,
  never a silent vanish.
- Designed a real, segment-based bidirectional byte-offset map
  (`to_display_offset_folded`/`from_display_offset_folded`,
  `engine-render::text`) -- a real, deliberate v1 clamp for a real
  offset landing strictly inside a fold: resolves to right after that
  fold's own real marker, since a position inside genuinely hidden
  content can't be usefully distinguished.
- Restructured `draw_field`'s own offset handling into one shared
  `to_display` closure chaining folding first, then whitespace
  substitution (M31 Phase 3) -- so cursor/selection/caret/syntax spans
  (M31 Phase 4) all stay correct together, whichever real combination
  of the three real paint transforms is active. Mirrored the identical
  reverse chain in `hit_test_position`.
- Added `TextFieldState.folded_ranges: Vec<Range<usize>>` to
  `engine-core::node` (empty default, every existing construction site
  unchanged) -- `engine-core` never interprets the ranges itself,
  deciding which real lines are foldable/currently folded is the app's
  own concern, the identical real split `syntax_spans` already has.
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check`/`cargo test --release` all clean
  (after adding `#[allow(clippy::single_range_in_vec_init)]` to the
  handful of real single-element-range test fixtures clippy correctly
  flagged as ambiguous-looking, a real, minor lint, not a bug).
- Wrote 5 new `engine-render` unit tests: real elision collapses each
  range to one marker; a real malformed/overlapping/out-of-bounds
  range is skipped rather than corrupting output; the real fold-aware
  offset map round-trips every real char boundary outside a fold; a
  real offset strictly inside a fold clamps to the identical real
  display position; a real display offset landing on the marker itself
  resolves back to the fold's own real start. All 5 passed on the
  first run (`engine-render` 12 unit tests, up from 7).
- Wrote 2 new integration tests in `text_field_paint.rs`: a real
  folded range paints genuinely different pixels than unfolded (the
  same diff-based proof this file's own hard-to-pin-exact-pixel claims
  already use); a real click far past a folded field's own end
  resolves to `state.content`'s own real length, neither the shorter
  marker-collapsed display length nor an out-of-bounds offset. Both
  passed on the first run (13 tests, up from 11) -- unlike M31 Phase
  4's own first attempt, this design was correct on the first try,
  verified by these same real tests before trusting it.
- Added `Node.set_folded_ranges([(start, end), ...])` to `engine-py`
  -- replaces the whole list every call, the identical real contract
  `set_syntax_spans` already has.
- Rebuilt the Python extension. **Ran a real, direct empirical
  end-to-end script before writing any pytest suite**: real fold
  ranges set on a Code Editor without raising; `get_text()` stays
  completely unsubstituted; a plain `Rect` node correctly rejects the
  call; folding + syntax highlighting + whitespace indicators compose
  cleanly through a real `App.run()` render loop with no crash. All
  passed.
- Extended `tests/test_code_editor.py` with 4 new tests (real ranges
  don't raise and never touch content; a non-`TextField` node rejects
  the call; folding and syntax highlighting compose without raising).
  Deliberately did *not* add a real `App.run()` call to this new test
  -- the real cross-test hazard M30 Phase 9 Step 5's own investigation
  already found and recorded as a durable memory. All 4 passed on the
  first run.
- Wrote `examples/code_editor_folding.py` -- a real gutter toggle
  affordance, composed entirely from existing primitives (a small
  clickable `Rect` per foldable line, positioned using the identical
  real line-height approximation `engine_core::terminal_cell_size`
  already documents), folding/unfolding a real function body through
  two real clicks. Checked for a filename collision first. Clean on
  the first run.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-render` 12 unit tests up from
  7, 13 in `text_field_paint.rs` up from 11), `maturin develop
  --release`, `pytest tests/` (506 passed, 1 skipped, up from 503 --
  4 new, zero regressions, no second real `App.run()` introduced), all
  69 examples (including the new `examples/code_editor_folding.py`)
  and the showcase demo re-run clean, `mypy --strict` clean against
  `examples/code_editor_folding.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 83%, Phase 5 heading
  ✅, Step 1 marked done) -- verified the parser's own reported item
  count before/after (191, unchanged, since no bullets were added or
  removed, only an existing one filled in), regenerated and
  republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
