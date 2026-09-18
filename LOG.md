# LOG — M30 Phase 6 Step 2: Accordion

- Checked the curated icon set — only 8 icons existed (home, search,
  menu, close, check, arrow_back, add, settings), no chevron. Fetched
  the real `expand_more` SVG path data verbatim from Google's own CDN
  (`https://fonts.gstatic.com/.../expand_more/default/24px.svg`,
  viewBox `0 -960 960 960`), added as the ninth curated icon in
  `crates/engine-md3/src/icons.rs`.
- Investigated whether `Node.animate("transform", ...)` supports
  rotation — confirmed via direct read of `extract_translate_scale`
  that it only ever composes translate+scale, no rotation capability
  exposed to Python at all. Designed the honest workaround: a uniform
  negative scale (`-1.0`) is mathematically identical to a 180°
  rotation for the point-symmetric `expand_more` glyph.
- Decided scope: only the header gets real new anatomy — the
  collapsible content region has no distinctive MD3 styling of its
  own, so no dedicated `add_accordion_content` method; the app
  composes it from any existing container.
- Implemented `add_accordion_header` in
  `crates/engine-py/src/window_factory.rs`, reusing List Item's own
  anatomy (`MENU_ITEM_*`) exactly.
- Added `.pyi` stub.
- Wrote `tests/test_accordion.py` — discovered `Node.get()` only
  supports scalar properties and `"transform"` isn't among them (no
  pixel/value readback), so simplified the test to proving the
  animate call succeeds end to end rather than reading the resulting
  matrix. All 5 tests passed on the first run, including the header's
  independent-click test (no decorative wrapper needed this time).
- Wrote `examples/accordion.py` — discovered `Node.remove()` is full,
  irreversible destruction (`Tree::remove`), not a detach; this
  codebase has no Python-level "hide without destroying" for an
  arbitrary node. Switched the content region's own show/hide to
  `Node.animate("opacity", ...)` instead, avoiding the destroy
  pitfall entirely.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean (the new icon's real parse-as-BezPath coverage came free from
  the existing `every_curated_icon_parses_as_a_real_bezpath` test),
  `cargo test --workspace --release` (all green), `maturin develop
  --release`, `pytest tests/` (390 passed, 1 skipped), all 53
  examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
