# LOG — M39 Phase 4: Terminal Cell Text Attributes

- User's own explicit instruction: "Start" continued into M39 Phase 4
  (item 4 from the gap-sweep answer, fourth in the user's own chosen
  order): `TerminalCell` real text-attribute plumbing.
- Real verification first, before writing any code: direct source
  read of the vendored `vt100 = "0.16.2"` crate confirmed `vt100::
  Cell` already exposes `bold()`/`dim()`/`italic()`/`underline()`/
  `inverse()` as real, already-parsed booleans (`~/.cargo/registry/
  src/.../vt100-0.16.2/src/cell.rs:148-176`). Confirmed via grep across
  the whole crate: zero real hits for "strike" anywhere -- strikethrough
  genuinely has no source data to render, a real constraint of the
  vendored VT parser itself, not an engine-side choice; kept excluded
  exactly as this milestone's own scoping note already stated.
- `TerminalCell` (`node.rs`) widened with `dim`/`italic`/`underline`/
  `inverse: bool`. `blank()` defaults all four `false`. Fixed one real,
  mechanical compile break this caused: `engine-render/tests/
  terminal_selection.rs`'s own pre-existing `TerminalCell { ... }`
  struct literal needed the four new fields added.
- `engine-py::terminal.rs`'s `screen_cell_to_terminal_cell` reads all
  four straight off the real `vt100::Cell` -- pure plumbing, no new
  logic of its own.
- `engine-render::text.rs`'s `draw_terminal`, the real per-attribute
  paint logic -- the substantial part of this phase:
  - New `terminal_cell_effective_colors(cell: &TerminalCell) -> (Color,
    Color)`: resolves a real `inverse` cell's fg/bg swap once, shared
    by both the background-run loop (which groups/paints by effective
    bg) and the glyph-run loop (which needs effective fg) -- one real
    mechanism, not duplicated. **Real, stated v1 fallback:** a swapped
    foreground landing on `Color::TRANSPARENT` (the cell's own real
    background was never set -- `vt100::Color::Default`'s own real
    sentinel) falls back to `Color::BLACK` rather than a genuinely
    invisible glyph, which would have been a real, visible correctness
    bug (inverse text silently vanishing), not a defensible edge case.
  - `dim` folded directly into the glyph run's own effective ink color
    via `with_opacity(fg, 0.6)`, computed once inside a small local
    `ink` closure reused by both the run-start and run-extend checks
    -- this naturally breaks a run at a dim/non-dim boundary since the
    two resolved colors are simply no longer equal, no separate
    run-grouping key field needed.
  - `italic`: real investigation into how to actually slant a glyph
    when the bundled monospace face (`Hack Nerd Font Mono`) has no
    real italic variant of its own. Found `glifo::GlyphRunBuilder::
    glyph_transform(transform: Affine)`, whose own doc comment states
    exactly this use case: "Use `Affine::skew` with a horizontal-only
    skew to simulate italic text." Used `kurbo::Affine::skew`'s own
    doc-example angle (20°) rather than inventing a different one --
    `Affine::skew`'s own doc comment gives the formula for a Y-up
    coordinate system; this codebase's own screen space is Y-down
    (confirmed already, repeatedly, this session -- most recently for
    the Time Picker Dial's own angle math), so the sign is flipped
    (`-(20f64.to_radians().tan())`) to lean the glyph the same real
    visual direction (top toward positive x) in Y-down space.
  - `underline`: a real, analytic drawn rule (a filled `Rect`) near
    the row's own bottom, positioned at `0.85 * cell_height` -- a real,
    stated v1 approximation of the font's own true underline-position/
    thickness metric (real values a `skrifa` `OS/2`/`post`-table read
    could recover, not attempted here), the identical "analytic grid,
    not exact font metrics" scope `draw_terminal`'s own cell-background/
    cursor positioning already operates at.
- **Real bug #1 found and fixed in my own first test draft:** the
  first version of `terminal_cell_attributes.rs`'s `build_scene`
  omitted `tree.compute_layout(...)` (mirroring `terminal_selection.
  rs`'s own existing helper, which also omits it) -- fine for that
  file's own tests, which only ever compare relative pixel diffs
  between two renders, but MY new tests also assert an absolute
  baseline color for the container's own outer background rect, which
  depends on `w`/`h` from the node's own real `taffy`-computed layout.
  Without `compute_layout`, that layout stays `0x0`, so the outer
  background rect painted nothing at all (`[0,0,0,0]`, fully
  transparent) instead of the expected opaque black -- three tests
  failed on their own sanity-check assertion before ever reaching the
  real attribute comparison. Root-caused by comparing against `shape_
  morph_paint.rs`'s own test harness, which DOES call `compute_
  layout`; fixed by adding the identical call to my own `build_scene`.
- **Real bug #2 found and fixed:** the first `dim` test picked pixel
  `(4, 4)` to compare, a point that turned out to be off the glyph's
  own real ink entirely (pure background in both the dim and non-dim
  render, so naturally, uninterestingly identical -- not evidence dim
  wasn't working, just evidence that specific pixel was never on
  either render's own glyph stroke). Fixed by switching to a real
  whole-buffer diff (`assert_ne!(data_plain, data_dim)`), the identical
  real technique `italic`'s own test already used successfully and
  this whole project has already established as the honest choice
  when a single font-dependent pixel coordinate can't be predicted
  reliably in advance (M39 Phase 1's own horizontal-scroll paint test
  used the same reasoning).
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`
  (one real lint fix along the way: `clippy::manual_slice_fill` on my
  own test's `for c in &mut state.cells { *c = cell; }`, replaced with
  `state.cells.fill(cell)`); `cargo fmt` + `cargo fmt --check`; `cargo
  test --workspace --release` (`engine-core`: 213 passed, +1; new
  `engine-render` `terminal_cell_attributes` suite: 5 passed;
  pre-existing `terminal_selection` suite re-run to confirm zero
  regression: 2 passed, unchanged); `maturin develop --release` (no
  Python-facing API changed this phase); `pytest tests/` (581 passed,
  1 skipped, unchanged from the prior phase, as expected); all 77
  examples + showcase demo clean.
- `BUILD_TRACKER.md` Phase 4's own heading, step bullet, milestone
  status line ("Phase 4 of 5 done", up from "Phase 3"), and Top
  Metrics row (80%, up from 60%) all updated together. Parser
  re-confirmed balanced (39 milestones, 127 phases, 218 items,
  unchanged); artifact regenerated and republished. **M39 Phase 4 is
  now complete. Phase 5 -- the milestone's own final phase --
  remains: `Tree::tick_all` Active-Set Optimization.**
