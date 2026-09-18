# LOG — M30 Phase 4 Step 1: Dialog

- Investigated `Tree::dismiss_overlays_outside` and its one call site in
  `dispatch`'s `PointerPressed` arm directly — confirmed the real gap:
  a non-dismissing overlay currently lets background clicks fall
  straight through, since blocking and dismissing were never separable.
- Added `OverlayMeta.modal: bool` (`crates/engine-core/src/overlay.rs`)
  and `Tree::press_blocked_by_modal_overlay` (`tree.rs`), wired into
  `dispatch`'s `PointerPressed` arm. Updated all 9 pre-existing
  `OverlayMeta { ... }` construction sites with `modal: false`.
- Two new `engine-core` unit tests, both passed first try:
  `a_press_outside_a_modal_overlay_is_consumed_without_dismissing_it`,
  `modal_false_still_lets_an_outside_press_reach_the_background`.
- Verified real MD3 Dialog tokens via WebFetch against
  `_md-comp-dialog.scss`: `surface_container_high` panel,
  `corner-extra-large` (28dp), elevation level 3, Headline Small
  (24sp/400, `on_surface`), Body Medium (14sp/400, `on_surface_variant`).
- Implemented `add_dialog`/`open_dialog`/`close_dialog` in
  `crates/engine-py/src/window_factory.rs`. Caught and fixed three real
  bugs in my own first draft before compiling: no theme resolution at
  all, wrong headline color role (`on_surface_variant` instead of
  `on_surface`), and `x`/`y` kwargs that would have defeated the
  deliberate flex-centering design.
- Added `.pyi` stubs for all three methods in `python/tre/_core.pyi`.
- Wrote `tests/test_dialog.py` (7 tests, all passed first run),
  including a real end-to-end click-dispatch test proving the modal
  actually blocks a background click and un-blocks it after close.
- Wrote `examples/dialog.py` (headless-CI-safe), ran clean, `mypy
  --strict` clean.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (43 binaries green),
  `maturin develop --release`, `pytest tests/` (308 passed, 1 skipped),
  all 44 examples clean, showcase demo clean.
- Found and fixed a real, stale `BUILD_TRACKER.md` Top Metrics row for
  M30 — it had stayed at "0%, not started" through Phases 0-3 actually
  completing. Corrected while updating for this step.
- Updated `BUILD_TRACKER.md`, regenerated and republished the Build
  Tracker artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
