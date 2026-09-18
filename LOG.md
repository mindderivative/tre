# LOG — M30 Phase 5 Step 2: Navigation Drawer

- Verified real Navigation Drawer item tokens via WebFetch against
  `_md-comp-navigation-drawer.scss`: active indicator 336x56dp,
  `corner-full`, `secondary_container` fill; active icon/label
  `on_secondary_container`, inactive `on_surface_variant`; label
  Label Large.
- Traced the active/inactive label weights through
  `_md-sys-typescale.scss` (label-large-weight = weight-medium,
  label-large-weight-prominent = weight-bold) into
  `_md-ref-typeface.scss` — confirmed they match Navigation Rail's
  own already-found 500/700 pair exactly; reused
  `NAV_RAIL_LABEL_WEIGHT_ACTIVE`/`_INACTIVE` directly instead of
  re-declaring.
- Designed proactively to avoid Navigation Rail's own hit-test bug:
  the indicator pill itself is the returned, interactive node (icon
  and label sit side by side *inside* it, spanning the item's entire
  clickable anatomy) — no separate outer wrapper, no
  `Tree::set_hit_testable` opt-out needed here.
- Implemented `add_navigation_drawer`/`open_navigation_drawer`/
  `close_navigation_drawer` in
  `crates/engine-py/src/window_factory.rs`, reusing `SIDE_SHEET_*`
  container constants directly (confirmed via direct re-fetch that
  Navigation Drawer and Side Sheet share the same real token source),
  mirroring `corner_radii_override` for the left-docked edge.
- Fixed a real bug in my own first draft before compiling: used a
  non-existent `.patch_from()` method trying to merge two `Style`
  values for the Standard variant's optional x/y positioning — fixed
  by setting `position`/`inset` fields directly on the already-built
  `panel_style` instead, mirroring `positioned_style`'s own logic
  inline.
- Fixed a real clippy `too_many_arguments` finding (9 params) with
  `#[allow(clippy::too_many_arguments)]`, matching this file's own
  established precedent for multi-option MD3 factory methods.
- Removed an unused `NAV_DRAWER_HORIZONTAL_MARGIN` constant caught by
  a real compiler warning — the margin is already implicit via
  `align_items: CENTER`, no explicit constant needed.
- Added `.pyi` stubs for all three methods.
- Wrote `tests/test_navigation_drawer.py` (12 tests) — all passed on
  the first run, including the independent-click test that failed
  for Navigation Rail, confirming the proactive design choice worked.
- Wrote `examples/navigation_drawer.py` (headless-CI-safe, both
  variants), applying the `Callable` return-type annotation
  proactively (Navigation Rail's own mypy --strict finding) — clean
  on the first run.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (347
  passed, 1 skipped), all 48 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
