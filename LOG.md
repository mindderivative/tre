# LOG — M30 Phase 8 Step 5: Status Bar

- Confirmed no official MD3 Status Bar page via the same directory-
  listing technique used throughout this milestone.
- Re-read `build_shell`'s real signature and found it already accepts
  a pre-built `status_bar: Option<PyRef<'_, Node>>` (since §14 step
  13, AppShell) — no new shell-level wiring needed, only real, styled
  bar content.
- Designed: 24dp thin bar, `surface_container` fill, Label Small text
  reusing `BADGE_LABEL_FONT_SIZE`/`_WEIGHT` (Badge's own earlier real
  finding, Phase 3 Step 1).
- Implemented `add_status_bar` in
  `crates/engine-py/src/window_factory.rs`, including
  `flex_shrink: 0.0` so the fixed-height bar doesn't shrink inside
  `build_shell`'s own flex-column layout.
- Added `.pyi` stub.
- Wrote `tests/test_status_bar.py` (4 tests) — all passed on the
  first run, including a real test proving the returned Node passes
  directly into `build_shell`'s own `status_bar` parameter.
- Wrote `examples/status_bar.py` — revised a first draft that used a
  pointless "harmless no-op animate call" as its own proof; replaced
  with a real, honest demonstration composing a menu bar, toolbar,
  and status bar together into one real AppShell. Clean on the first
  run, `mypy --strict` clean too.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green,
  unchanged), `maturin develop --release`, `pytest tests/` (440
  passed, 1 skipped), all 61 examples clean, showcase demo clean.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
