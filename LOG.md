# LOG — M30 Phase 4 Step 2: Snackbar

- Verified real MD3 Snackbar tokens via WebFetch against
  `_md-comp-snackbar.scss`: `inverse_surface` fill, `corner-extra-
  small` (4dp), elevation level 3, 48dp single-line height,
  `inverse_on_surface` supporting text (Body Medium), `inverse_primary`
  action label (Label Large), 24dp icon.
- `inverse-primary` had no direct hex in that file — traced through
  `_md-sys-color.scss` (maps to `primary80` tone) and
  `_md-ref-palette.scss` (`primary80` = `#D0BCFF`, confirmed against
  the already-real `primary40` = `#6750A4`). Added
  `Md3Baseline::INVERSE_PRIMARY`.
- Grepped `engine-core`/`engine-py` for any timer/scheduler primitive
  — zero hits. Confirmed real auto-dismiss-after-duration is out of
  scope, the app's own responsibility (Design Principle 6).
- Re-checked `Chip`'s `removable` icon (decorative only, not
  independently clickable) and `Segmented Button`'s `Vec<Node>` return
  (Phase 1 Step 4) before deciding: a real snackbar action must be
  independently clickable, so `add_snackbar` returns
  `(container, action, close)`, matching the multi-node precedent, not
  the decorative-icon one.
- Implemented `add_snackbar`/`open_snackbar`/`close_snackbar` in
  `crates/engine-py/src/window_factory.rs`. `open_snackbar` reuses
  `open_dialog`'s own synthetic-zero-size-anchor technique, placed at
  the desktop bottom-left corner instead of the origin.
- Fixed a real compile error along the way: `Size<Dimension>` doesn't
  implement `Default` in the pinned taffy version (`Dimension` itself
  has no `Default` impl) — used `auto()` explicitly for the flex-grow
  text child's width instead of `..Default::default()`.
- Added `.pyi` stubs for all three methods.
- Wrote `tests/test_snackbar.py` (10 tests, all passed first run),
  including real click-dispatch tests proving `action`/`close` each
  reach their own independently-registered handler and that clicking
  close doesn't also fire the action's handler.
- Wrote `examples/snackbar.py` (headless-CI-safe), ran clean, `mypy
  --strict` clean.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (318
  passed, 1 skipped), all 45 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
