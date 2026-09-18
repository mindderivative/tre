# PLAN — M30 Phase 5 Step 1: Navigation Rail

## Goal
Add `Window.add_navigation_rail` — the real desktop counterpart to
Navigation Bar. Built as a plain composition (app-owned selection
state), returning one real Node per item.

## Steps
1. Verify real MD3 Navigation Rail tokens via WebFetch against
   Material Web's own `_md-comp-navigation-rail.scss`.
2. Catch and correct a self-made error: re-verify container-color
   with a second, more targeted fetch after the first summary looked
   suspicious (misattributed the indicator's own color to the
   container).
3. Trace Label Medium's real numeric weights (500/700) through
   `_md-sys-typescale.scss` and `_md-ref-typeface.scss`.
4. Decide architecture: plain composition (Segmented Button/Filter
   Chip precedent), returns `Vec<Node>` (Segmented Button precedent).
5. Implement `add_navigation_rail` in `window_factory.rs`.
6. Write `tests/test_navigation_rail.py` — hits a real, confirmed
   click-dispatch bug: the decorative indicator pill (an interior
   Rect) eats clicks meant for its parent item.
7. Investigate and fix the real engine-core gap: add
   `Node.hit_testable: bool` (purely additive) + `Tree::
   set_hit_testable`, generalizing Phase 1's Text/Icon hit-test
   exemption. Add a direct `engine-core` unit test proving it.
8. Add `.pyi` stub.
9. Write `examples/navigation_rail.py`, headless-CI-safe.
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
11. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 5 heading now
    🚧, step line), regenerate + republish the Build Tracker artifact.
12. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (335 pytest
passed/1 skipped, 47 examples, showcase demo, engine-core 149 tests).
