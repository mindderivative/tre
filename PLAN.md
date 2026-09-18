# PLAN — M30 Phase 5 Step 4: Tabs

## Goal
Add `Window.add_tabs` — MD3's real Primary Navigation Tab variant, a
row of tabs each with a real 3dp active-indicator bar.

## Steps
1. Verify real Tabs tokens via WebFetch — find the real per-variant
   filenames via a GitHub directory listing (two real variants:
   Primary/Secondary Navigation Tab).
2. Fetch the real Primary variant tokens: surface/corner-none/level0/
   48dp height, indicator primary/3dp/(3,3,0,0) shape, Title Small
   label, optional icon.
3. Trace Title Small's real numeric size/weight — note the real
   coincidence with Label Large's own values, declare distinct
   constants rather than conflate roles.
4. Implement `add_tabs` in `window_factory.rs`.
5. Add `.pyi` stub.
6. Write `tests/test_tabs.py`, including an independent-click test.
7. Write `examples/tabs.py`, headless-CI-safe.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
   + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (364 pytest
passed/1 skipped, 50 examples, showcase demo, 43 Rust test binaries).
A real, confirmed repeat of Navigation Rail's hit-test bug was caught
live by the click-dispatch test (the content wrapper, not the
indicator) and fixed by reusing Node.hit_testable a second time.
