# LOG — M30 Phase 8 Step 1: Popover

- Fetched real Rich Tooltip tokens via `_md-comp-rich-tooltip.scss`:
  `surface_container` fill, `corner-medium` (reused
  `CARD_CORNER_RADIUS`'s own confirmed 12dp value directly), level2
  elevation (reused `MENU_PANEL_ELEVATION`'s own confirmed 2.0
  value), Title Small subhead (reused `TAB_LABEL_FONT_SIZE`/`_WEIGHT`
  — the same real numeric coincidence Tabs already found), Body
  Medium supporting text (reused `DIALOG_BODY_FONT_SIZE`/`_WEIGHT`).
- Decided lifecycle: reuse `Window.open_menu`/`close_menu` directly
  (Tooltip's own precedent), no new dedicated open/close pair --
  "persistent" means immune to hover-exit dismissal, not immune to
  outside-click.
- Implemented `add_popover` in
  `crates/engine-py/src/window_factory.rs`, mirroring Dialog's own
  headline+body column layout without the scrim (Popover isn't
  modal).
- Added `.pyi` stub.
- Wrote `tests/test_popover.py` — caught and fixed a misleading test
  name before writing: my draft called the outside-click test
  "stays open" when the assertion actually proves the OPPOSITE (it
  dismisses on outside click, matching Menu). Renamed and re-worded
  for accuracy. All 5 tests passed on the first run.
- Wrote `examples/popover.py` (headless-CI-safe) — clean on the
  first run, `mypy --strict` clean too.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (420
  passed, 1 skipped), all 57 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md` (Phase 8 heading now 🚧, Step 1 closed),
  regenerated and republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
