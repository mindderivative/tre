# LOG — M30 Phase 5 Step 4: Tabs

- Fetched the real GitHub directory listing for
  `tokens/versions/v0_192` filtered to "tab" — found MD3 has exactly
  two real Tab variants, each its own file:
  `_md-comp-primary-navigation-tab.scss`/
  `_md-comp-secondary-navigation-tab.scss`. Primary in scope,
  Secondary deliberately out of scope.
- Fetched the real Primary-variant tokens: `surface` fill,
  `corner-none`, `level0`, 48dp height; active indicator `primary`
  fill, 3dp height, real shape `(3px 3px 0px 0px)` (rounded only on
  top corners); active label/icon `primary`, inactive
  `on_surface_variant`; label type role Title Small.
- Traced Title Small's real numeric values (0.875rem = 14px,
  weight-medium = 500) — noted the real coincidence with Label
  Large's own numbers but declared distinct `TAB_LABEL_FONT_SIZE`/
  `_WEIGHT` constants rather than conflating the two roles.
- Implemented `add_tabs` in `crates/engine-py/src/window_factory.rs`.
  Cleaned up a redundant `row_style` shadow before compiling.
- `tests/test_tabs.py`'s own independent-click test failed on first
  run: assumed the 3dp indicator's own geometry (flush against the
  tab's bottom edge) would avoid Navigation Rail's own hit-test bug,
  but the `content` wrapper Rect around the icon/label (used purely
  for its own flex-centering layout) intercepted the click instead —
  a real, confirmed repeat of the same bug class, just from a
  different decorative Rect.
- Fixed identically to Navigation Rail:
  `tree.set_hit_testable(content, false)` — a direct, real
  confirmation the `Node.hit_testable` capability generalizes beyond
  its original use case. Corrected the method's own doc comment,
  which had prematurely claimed no opt-out was needed.
- Re-ran `tests/test_tabs.py` after the fix — all 9 passed.
- Added `.pyi` stub.
- Wrote `examples/tabs.py` (headless-CI-safe), applying the
  `Callable` return-type annotation proactively — clean on the first
  run (`mypy --strict` too).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (364
  passed, 1 skipped), all 50 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
