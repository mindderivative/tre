# LOG — M30 Phase 6 Step 1: List / ListItem

- Reused List Item's own token file, already investigated once (Menu,
  Phase 2 Step 4) — `MENU_ITEM_*` constants (56dp height, 24dp icon,
  16dp leading space, 12dp icon gap) apply directly.
- Fetched the real two-line variant tokens via WebFetch against
  `_md-comp-list.scss`: 72dp two-line container height (vs. 56dp
  one-line, confirmed), supporting text `on_surface_variant` Body
  Medium, trailing supporting text (metadata) same role. No divider
  token in the list's own file.
- Designed to proactively apply the Navigation Rail/Tabs hit-test
  lesson: the two-line headline+supporting-text wrapper
  (NodeKind::Container) gets `tree.set_hit_testable(text_block,
  false)` immediately on insertion, before ever running a test.
- Implemented `add_list_item`/`add_list` in
  `crates/engine-py/src/window_factory.rs`, mirroring `build_menu`'s
  own detach-then-reparent mechanism for `add_list`.
- Self-caught a real bug before compiling: a first draft of `add_list`
  summed each item's own `Tree::layout(...).size.height` to compute
  the frame's own height — a mistake, since an item's `Layout` is
  only meaningful after a real `compute_layout` pass has run, which
  this method has no guarantee of. Fixed to use `auto()` for the
  frame's own height, matching AppShell's own `content` region
  pattern (flex_grow, no explicit height).
- Added `.pyi` stubs for both methods.
- Wrote `tests/test_list.py` (11 tests) — all passed on the first
  run, including both one-line and two-line independent-click tests,
  confirming the proactive hit-test fix worked without needing a
  failing run first.
- Wrote `examples/list.py` (headless-CI-safe) — fixed a real mypy
  --strict inline-lambda type-inference finding by switching to the
  established factory-function pattern (Navigation Rail/Drawer/Tabs'
  own precedent).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (385
  passed, 1 skipped), all 52 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md` (Phase 6 heading now 🚧, Step 1 closed),
  regenerated and republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
