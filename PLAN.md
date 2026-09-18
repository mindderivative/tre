# PLAN — M30 Phase 6 Step 1: List / ListItem

## Goal
Add `Window.add_list_item`/`add_list` — a plain, non-virtualized list
for small real collections, distinct from VirtualList (already real,
stays the choice for large ones).

## Steps
1. Reuse List Item's own token file, already investigated once (Menu,
   Phase 2 Step 4) -- MENU_ITEM_* constants apply directly.
2. Fetch the real two-line variant tokens (72dp height, Body Medium
   supporting text) not previously needed by Menu.
3. Design to proactively apply the Navigation Rail/Tabs hit-test
   lesson: the two-line headline+supporting-text wrapper gets
   set_hit_testable(false) immediately, not found the hard way again.
4. Implement `add_list_item`/`add_list` in `window_factory.rs`,
   mirroring build_menu's own detach-then-reparent mechanism for
   add_list.
5. Add `.pyi` stubs for both.
6. Write `tests/test_list.py`, including independent-click tests for
   both one-line and two-line items.
7. Write `examples/list.py`, headless-CI-safe.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 6 heading now
   🚧, step line), regenerate + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (385 pytest
passed/1 skipped, 52 examples, showcase demo, 43 Rust test binaries).
Both independent-click tests passed on the first run -- the proactive
hit-test fix worked. Self-caught a real pre-layout height-summing bug
before compiling, fixed with auto() sizing.
