# LOG — M31 Phase 1: Line-Number Gutter

- Read pyCopper's own real `CodeEditor` widget in full — a genuinely
  more advanced widget than TRE's own current multiline `TextField`
  approach (its own dedicated engine-level class, bundled monospace
  font, real Pygments syntax highlighting, built-in gutter). Confirmed
  the real "never wraps" property TRE's own `field_max_width` already
  gives multiline mode (`f32::MAX`), the load-bearing fact this whole
  phase's own design leans on.
- Investigated the real open question the scoping note left ("a real
  per-line Y-offset read-back or an assumed fixed line-height") by
  direct source read, before writing any code: `TextRenderer::draw`
  (plain `Text`) and `TextRenderer::draw_field` (`TextField`) both
  build their real `parley::Layout` through the exact same private
  `shaped_layout` method — confirmed via direct read, not assumed.
  This means a gutter composed as an ordinary sibling `Text` node
  (same font settings as the editor, wide enough never to wrap) lines
  up with the editor's own real per-line Y positions *by construction*.
  No new engine-py/engine-core capability needed at all.
- Also confirmed the "fixed line-height assumed in the app" concern
  isn't actually fragile here: since multiline mode never wraps, every
  real line shares the identical font-metric line height by
  definition (a real, exact property, not an approximation) — there is
  no per-line variance a fixed assumption could get wrong.
- Proved the claim precisely, not just plausibly, with a new
  `engine-render` unit test comparing real `parley::Layout::lines()`'s
  own `block_min_coord` geometry between a Text-shaped and a
  TextField-shaped layout built from the same content/font — the two
  are byte-for-byte identical, confirmed by direct assertion against
  real `Layout` data, white-box (inside `text.rs`'s own `#[cfg(test)]`
  module, calling the private `shaped_layout` both node kinds share).
  Passed on the first run.
- Wired the real live-update half: `editor.set_on_change(...)`
  (already real and Python-facing since M14 Phase 3) recomputes the
  gutter's own content from `editor.get_text().count("\n") + 1` on
  every real keystroke edit — a plain Python callback, no new
  dispatch mechanism needed.
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check`/`cargo test --release` all clean
  (`engine-render` 5, up from 4).
- Rebuilt the Python extension (no Rust-level engine-py/engine-core
  changes were needed for this phase, only the new `engine-render`
  proof test — rebuilt anyway to be safe).
- Wrote `tests/test_code_editor_gutter.py` (4 tests: single-line/
  multi-line seeding, a real Enter growing the gutter live, a real
  Backspace merging two lines and shrinking it live) — checked for a
  filename collision first. All 4 passed on the first run.
- Wrote `examples/code_editor_gutter.py` — also checked for a filename
  collision first. Clean on the first run: a real 2-line buffer seeds
  the gutter at "1\n2", a real Enter mid-buffer grows it live to
  "1\n2\n3".
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-render` 5, up from 4),
  `maturin develop --release`, `pytest tests/` (498 passed, 1 skipped,
  up from 494 — 4 new, zero regressions, no second real `App.run()`
  introduced anywhere in this phase's own test suite, the real hazard
  M30 Phase 9 Step 5's own investigation found), all 68 examples
  (including the new `examples/code_editor_gutter.py`) and the
  showcase demo re-run clean, `mypy --strict` clean against
  `examples/code_editor_gutter.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 17%, Phase 1 heading
  ✅, Step 1 marked done) — verified the parser's own reported item
  count before/after (191, unchanged, since no bullets were added or
  removed, only an existing one filled in), regenerated and
  republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
