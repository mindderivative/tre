# PLAN — M32 Phase 1: Bundled Monospace Font

## Goal
Replace the fixed analytic `terminal_cell_size` estimate (`font_size *
0.6`/`* 1.3`) with real, measured monospace font metrics for both
`Terminal` and `Code Editor`, by bundling a real, already-vetted
monospace face rather than continuing to approximate with `Roboto`.

## Steps
1. Located a real, directly reusable asset: the sibling `pyCopper`
   project already bundles `HackNerdFontMono-Regular.ttf` (MIT
   License, Hack project, 2018 Source Foundry Authors) for its own
   `Terminal` widget, for the identical real reason (broad glyph
   coverage avoiding "tofu" in TUI/prompt content).
2. Confirmed the font's own real embedded family name via direct read
   of its `name` table (`fontTools.ttLib`), not assumed from the
   filename: "Hack Nerd Font Mono".
3. Copied the font + its license file into
   `crates/engine-render/assets/fonts/`, documented in that
   directory's own README alongside the three existing fonts.
4. Registered it in `TextRenderer::new()` (`include_bytes!` +
   `collection.register_fonts`), added `MONOSPACE_FONT_FAMILY` const.
5. Added `TextRenderer::monospace_cell_size(font_family, font_size)`:
   shapes a single "M" glyph through the existing `build_field_layout`
   (zero new shaping logic) and reads back real `Layout::width()`/
   `height()` — for a genuinely monospace face this gives the exact
   real per-cell width/height. Memoized by `(font_family, font_size)`.
6. `draw_terminal` now calls this instead of `engine_core::
   terminal_cell_size`.
7. Removed `engine_core::terminal_cell_size` entirely (both real call
   sites migrated off it, would otherwise be dead code) — `engine-core`
   stays font-agnostic per the crate-boundary rule (§4); the real
   metric now lives where real font access actually exists
   (`engine-render`), which `engine-py` already depends on.
8. `engine-py::add_terminal`/`add_code_editor` now build a throwaway
   `TextRenderer` to measure real cell/line metrics at node-creation
   time (a real, one-time cost per call, not a per-frame one) and both
   always shape with the real bundled monospace face instead of
   `"Roboto"`.
9. Added `Window.get_monospace_cell_size(font_size) -> (f32, f32)`,
   exposing the same real metric to Python app code — replacing
   `examples/code_editor_folding.py`'s own `FONT_SIZE * 1.3`
   approximation with the real measured value, and fixing both
   gutter examples' sibling `Text` node to use the real
   `MONOSPACE_FONT_FAMILY` (now exported from the `tre` package)
   instead of `"Roboto"`, preserving M31 Phase 1's "lines up by
   construction" invariant now that Code Editor's own real font
   changed.
10. Real Rust unit tests: proved the bundled face has genuinely
    uniform glyph advance (unlike Roboto, a real contrast case) and
    that `monospace_cell_size` scales with `font_size` and is cached.
11. Real, direct empirical script before pytest: `get_monospace_cell_size`
    scales correctly, a real terminal spawns and a real code editor's
    content round-trips with the new font.
12. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all 69 examples, showcase demo,
    mypy --strict on the three touched examples.
13. Update `BUILD_TRACKER.md` (parser count verified), regenerate +
    republish the Build Tracker artifact, update memory, commit.

## Status
Complete. All steps done; full verification chain green (`engine-render`
gains 2 new unit tests, `pytest tests/` 507 passed/1 skipped, up from
506, all 69 examples, showcase demo, mypy --strict clean on the three
touched examples).
