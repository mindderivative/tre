# PLAN — M30 Phase 6 Step 3: Tree View (closes Phase 6)

## Goal
Add `Window.add_tree_node` — the identical grounding Accordion
already established, applied recursively: a per-node row with real
depth-based indentation.

## Steps
1. Design: reuse Accordion's own header anatomy verbatim (MENU_ITEM_*/
   ACCORDION_CHEVRON_ICON), add real per-depth left indentation
   (24dp/level) and a leaf flag that omits the chevron for childless
   nodes.
2. Implement `add_tree_node` in `window_factory.rs`.
3. Add `.pyi` stub.
4. Write `tests/test_tree_view.py`, including a dedicated deep-
   nesting (depth=3) independent-click test to confirm the added
   padding doesn't reintroduce a hit-test problem.
5. Write `examples/tree_view.py`, headless-CI-safe, a small file-
   browser-shaped tree.
6. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
7. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 6 heading now
   ✅, Step 3 and Step 4 both closed), regenerate + republish the
   Build Tracker artifact. Closes Phase 6 entirely.
8. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (399 pytest
passed/1 skipped, 54 examples, showcase demo, 43 Rust test binaries).
Phase 6 (Lists & Disclosure) is now fully complete, all 4 steps.
