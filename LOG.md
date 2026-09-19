# LOG — M32 Phase 1: Bundled Monospace Font

- Grepped `BUILD_TRACKER.md` for its own `**Not scoped**`/"real,
  deliberately deferred"/"real, stated v1" notes across M29/M30/M31 to
  catalogue real hardening targets before scoping M32 at all: a
  bundled monospace font, window resize handling, general scroll/clip,
  Terminal Ctrl+C/scrollback/mouse selection. Scoped M32's own 6
  phases, cheapest/most-grounded first, matching M31's own discipline.
- Investigated the font gap specifically: the sibling `pyCopper`
  project already bundles a real, MIT-licensed monospace face
  (`HackNerdFontMono-Regular.ttf`) for its own `Terminal` widget.
  Confirmed the license file's own real MIT grant (Hack project, 2018
  Source Foundry Authors) and the font's own real embedded family name
  via direct `fontTools` read of its `name` table: "Hack Nerd Font
  Mono" -- not assumed from the filename.
- Copied the font + license into `crates/engine-render/assets/fonts/`,
  documented in that directory's own README (matching the existing
  Roboto/Noto Sans Arabic entries' format).
- Added `HACK_NERD_FONT_MONO`/`MONOSPACE_FONT_FAMILY` to `text.rs`,
  registered in `TextRenderer::new()` alongside the existing 3 fonts.
- Added `TextRenderer::monospace_cell_size(font_family, font_size)`:
  reuses the existing private `build_field_layout` to shape a single
  "M" and reads back real `Layout::width()`/`height()` -- zero new
  shaping logic, real metrics instead of a guess. Memoized by
  `(font_family, font_size)` since `draw_terminal` calls it every
  frame.
- `draw_terminal` now uses the real measured cell size.
- Removed `engine_core::terminal_cell_size` entirely (both real call
  sites migrated off it) -- confirmed via grep it had exactly those 2
  real callers, nothing else. `engine-core` stays font-agnostic per
  the crate-boundary rule; the real metric now lives in
  `engine-render`, which `engine-py` already depends on (confirmed via
  `app.rs`'s own existing `use engine_render::...`).
- `engine-py::add_terminal`/`add_code_editor` now build a throwaway
  `TextRenderer` to measure real metrics at node-creation time (a
  real, one-time cost per call, not per-frame) and both always shape
  with the real bundled monospace face instead of `"Roboto"`.
- Added `Window.get_monospace_cell_size(font_size)`, exposing the
  identical real metric to Python. Added `tre.MONOSPACE_FONT_FAMILY`
  (a plain Python constant mirroring the Rust-side one) so app code
  doesn't have to hardcode the literal string.
- Fixed `examples/code_editor_gutter.py`/`code_editor_folding.py`'s
  own sibling gutter `Text` nodes, which had hardcoded `font_family=
  "Roboto"` -- now `MONOSPACE_FONT_FAMILY`, preserving M31 Phase 1's
  own real "lines up by construction" invariant (identical
  `shaped_layout` inputs) now that Code Editor's real font changed.
  Replaced `code_editor_folding.py`'s own `LINE_HEIGHT = FONT_SIZE *
  1.3` approximation with the real `get_monospace_cell_size` value.
- Updated `add_code_editor`'s own Rust doc comment and the `.pyi`
  stub's docstring, both of which had stale "no bundled monospace
  font"/"no syntax highlighting, no line-number gutter" language left
  over from before M31 closed those gaps -- corrected while already
  editing this exact text, not a separate pass.
- Real Rust unit tests (`engine-render`, 2 new): proved the bundled
  face has genuinely uniform glyph advance across visually
  different-width glyphs ("M" vs "i"), with a direct contrast case
  (`Roboto`, real and proportional) proving the test isn't vacuous;
  proved `monospace_cell_size` scales with `font_size` and returns an
  identical cached value on a repeat call.
- Full Rust verification chain green on the first pass after one
  mechanical fix (a `let Self { .. }` destructuring pattern needed the
  new cache field named): `cargo check`/`clippy -D warnings`/
  `fmt --check`/`cargo test --release` all clean.
- Rebuilt the Python extension (`maturin develop --release`). Ran a
  real, direct empirical script before writing any pytest:
  `get_monospace_cell_size` returns real positive values that
  genuinely double when `font_size` doubles (8.43/16.30 @14pt vs
  16.86/32.59 @28pt -- exactly linear); a real terminal spawns; a real
  code editor's content round-trips exactly.
- Added `tests/test_terminal.py::
  test_get_monospace_cell_size_returns_real_positive_values_that_scale_with_font_size`
  (synchronous, no `App.run()` -- the established "never add a second
  real App.run() in this pytest process" rule stayed intact; this
  file already has its one real `App.run()` call in the shell-response
  test).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-render` +2 unit tests),
  `maturin develop --release`, `pytest tests/` 507 passed/1 skipped (1
  new, up from 506, zero regressions), all 69 examples and the
  showcase demo re-run clean, `mypy --strict` clean against the three
  touched examples (`code_editor.py`, `code_editor_gutter.py`,
  `code_editor_folding.py`).
- Updated `BUILD_TRACKER.md` (Top Metrics row, Phase 1 heading and
  Step 1) -- verified the parser's own reported item count before/
  after, regenerated and republished the Build Tracker artifact.
