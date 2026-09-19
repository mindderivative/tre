# PLAN — M35 Phase 3: Button Groups, closing M35

## Goal
Close M35 with Phase 3: a real MD3 Standard Button Group -- an
invisible container that, when a child is pressed, grows it and
shrinks its immediate neighbors, a real live width-reflow mechanic.

## Steps
1. Real anatomy verified from the local MD3 spec mirror
   (`COMPONENT_BUTTON_GROUPS.md`) before writing any code: Standard
   variant reflows siblings on press; Connected variant explicitly
   replaces the already-built `Segmented Button` (M30 Phase 6) --
   deliberately out of scope here, `add_segmented_button` already
   covers it.
2. Investigated `Tree::sync_carousel_layouts` (M30 Phase 9 Step 5) as
   the real architectural precedent this phase's own scoping note
   promised: one container-level marker drives every child's own real
   `layout_style`, `compute_layout` running taffy a second time so the
   new insets land. Confirmed `Tree.pressed: Option<(PointerButton,
   NodeId)>` already exists and is already tracked -- no new
   interaction wiring needed at all.
3. New `PaintProperties.button_group_reflow: Option<(f64, f64)>`
   (grow_px, gap_px) -- a plain field, not a new `NodeKind` (mirrors
   `clip_children`'s own "universal opt-in flag" shape, M32 Phase 3).
4. New `Tree::sync_button_group_layouts`, wired into `compute_layout`
   right after the carousel sync. Real, deliberately simple formula
   (no discrete numeric token exists for the grow amount): pressed
   child grows by `grow_px`, split evenly back out of its immediate
   neighbors, row's total width provably constant.
5. Two real, decisive Rust unit tests proving the exact math: nothing
   pressed leaves widths unchanged; pressing the middle of three 80px
   buttons (grow=12) yields exactly 92/74/74 with total width
   unchanged (240px) -- both passed on the first run.
6. `Window.add_button_group(labels, width, height, variant, x, y) ->
   (group, buttons)` -- each child built via a direct `self.
   add_button(...)` call (plain Rust method call within the same impl
   block), reparented under the group via `Tree::try_add_child`.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, real empirical script before pytest, `tests/
   test_button_group.py` (6 tests), `examples/button_group.py`, full
   pytest suite, all examples, showcase demo, mypy --strict (one real
   fix: a missing return-type annotation, caught by mypy itself).
8. `BUILD_TRACKER.md` (Phase 3 closed, M35 itself closed, all 3
   phases; Top Metrics row at 100%), artifact republish, memory
   update, commit, **push** (the full milestone now closes).

## Status
Complete. Full verification chain green (`pytest tests/` 549 passed/1
skipped, 6 new, zero regressions; all 74 examples + showcase demo
clean; mypy --strict clean). **M35 -- MD3 Expressive Catalog: Toolbars,
Split Button, Button Groups is now fully complete, all 3 phases.**
