# LOG — M30 Phase 5 Step 3: Top App Bar

- WebFetch against `_md-comp-top-app-bar.scss` 404'd. Fetched the
  real GitHub directory listing for `tokens/versions/v0_192` and
  found the real per-variant filenames:
  `_md-comp-top-app-bar-small.scss`/`-medium.scss`/`-large.scss`/
  `-small-centered.scss`.
- Fetched the real Small-variant tokens: `surface` fill, `level0`
  elevation, 64dp height, Title Large headline (`on_surface`), 24dp
  leading icon (`on_surface`), 24dp trailing icon
  (`on_surface_variant` -- a real, confirmed asymmetry from leading).
- Traced Title Large's real numeric values: `title-large-size` =
  1.375rem = 22px, `title-large-weight` = `weight-regular` = 400
  (via `_md-sys-typescale.scss`/`_md-ref-typeface.scss`).
- Designed to reuse Icon Button's own exact anatomy (Rect container +
  centered, deferring Icon child) for leading/trailing actions,
  avoiding any Navigation Rail-style hit-test risk by construction.
- Implemented `add_top_app_bar` in
  `crates/engine-py/src/window_factory.rs`. Cleaned up a redundant
  `let bar_style = ...; let mut bar_style = bar_style;` shadow before
  compiling.
- Added `.pyi` stub.
- Wrote `tests/test_top_app_bar.py` (8 tests, all passed first run),
  including independent-click proofs for the leading icon and each
  trailing icon.
- Wrote `examples/top_app_bar.py` (headless-CI-safe), ran clean,
  `mypy --strict` clean on the first run.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (355
  passed, 1 skipped), all 49 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
