# PLAN — M30 Phase 5 Step 3: Top App Bar

## Goal
Add `Window.add_top_app_bar` — MD3's real Small Top App Bar variant,
a 64dp header with an optional leading icon and zero-or-more trailing
icons.

## Steps
1. Verify real Top App Bar tokens via WebFetch — 404 on the naive
   filename; found the real per-variant filenames via a GitHub
   directory listing (`_md-comp-top-app-bar-small.scss` etc).
2. Fetch the real Small-variant tokens: surface/level0/64dp/Title
   Large headline/leading+trailing icon colors.
3. Trace Title Large's real numeric size/weight through
   `_md-sys-typescale.scss`/`_md-ref-typeface.scss`.
4. Design: reuse Icon Button's own exact anatomy for leading/trailing
   actions (Rect container + centered Icon child) to avoid any
   Navigation Rail-style hit-test risk by construction.
5. Implement `add_top_app_bar` in `window_factory.rs`.
6. Add `.pyi` stub.
7. Write `tests/test_top_app_bar.py`, including independent-click
   proofs for leading and trailing icons.
8. Write `examples/top_app_bar.py`, headless-CI-safe.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
10. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
    + republish the Build Tracker artifact.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (355 pytest
passed/1 skipped, 49 examples, showcase demo, 43 Rust test binaries).
Both independent-click tests passed on the first run.
