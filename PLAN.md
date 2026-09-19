# PLAN — M39 Phase 4: Terminal Cell Text Attributes

## Goal
Close a real, previously-stated v1 gap: `TerminalCell` only ever
carried `bold` -- underline/italic/dim/inverse were real, named
omissions since M30 Phase 9 Step 4. `vt100::Cell` already parses all
four; this phase plumbs them through and paints each for real.

## Steps
1. Real verification first: direct source read of the vendored `vt100
   = "0.16.2"` crate confirmed `vt100::Cell::dim()/italic()/
   underline()/inverse()` all exist as real, already-parsed booleans
   (`~/.cargo/registry/.../vt100-0.16.2/src/cell.rs`), and confirmed
   via grep that the crate has zero real strikethrough support
   anywhere -- strikethrough stays a real, stated v1 omission, not an
   oversight.
2. `TerminalCell` (`node.rs`) widened with `dim`/`italic`/`underline`/
   `inverse: bool`; `blank()` defaults all four `false`.
3. `engine-py::terminal.rs`'s `screen_cell_to_terminal_cell` reads all
   four straight off the real `vt100::Cell`.
4. `engine-render::text.rs`'s `draw_terminal`, real per-attribute
   paint logic:
   - New `terminal_cell_effective_colors(cell) -> (fg, bg)` helper --
     resolves a real `inverse` cell's fg/bg swap once, shared by both
     the background-run loop and the glyph-run loop. Real, stated v1
     fallback: a swapped foreground landing on `Color::TRANSPARENT`
     (the cell's own background was never set) falls back to
     `Color::BLACK` rather than a genuinely invisible glyph.
   - `dim` folded directly into the glyph run's own effective ink
     color (`with_opacity(fg, 0.6)`), so it naturally breaks a run
     from an adjacent non-dim cell rather than needing a separate
     run-grouping key field.
   - `italic`: real investigation found `kurbo::Affine::skew` plus
     `glifo::GlyphRunBuilder::glyph_transform` -- that builder method's
     own doc comment literally says "Use `Affine::skew` with a
     horizontal-only skew to simulate italic text," the bundled
     monospace face's own real answer to having no italic variant.
     20° (kurbo's own doc example angle), sign flipped for this
     codebase's own real y-down screen space.
   - `underline`: a real, analytic drawn rule near the row's own
     bottom (`0.85 * cell_height`), a real, stated v1 approximation of
     the font's own true underline-position metric, not a
     `skrifa`-derived exact one.
5. Real tests: 1 new `engine-core` unit test (`TerminalCell::blank()`'s
   four new fields all default `false`); 5 new real pixel-readback
   tests, `engine-render/tests/terminal_cell_attributes.rs` (dim,
   italic, underline, and inverse's own two real cases -- unset
   background falling back to legible ink, and an explicit background
   swapping cleanly), mirroring `terminal_selection.rs`'s own
   established render-to-texture-then-readback harness verbatim.
   **Found and fixed two real bugs in my own first test draft** -- see
   `LOG.md`.
6. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release` (no
   Python-facing API changed this phase), full `pytest tests/`, all 77
   examples, showcase demo.
7. `BUILD_TRACKER.md` Phase 4 flipped fully to done (heading, step
   bullet, milestone status line, Top Metrics row). Artifact
   regenerated (39/127/218, unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 213 passed, +1; `engine-render`'s new
`terminal_cell_attributes` suite 5 passed; pre-existing
`terminal_selection` suite unaffected, 2 passed; `pytest tests/`: 581
passed/1 skipped, unchanged -- no Python-facing API touched this
phase; all 77 examples + showcase demo clean). **M39 Phase 4 --
Terminal Cell Text Attributes -- is now complete. Phase 5 of M39
remains: `Tree::tick_all` Active-Set Optimization (the milestone's own
final phase).**
