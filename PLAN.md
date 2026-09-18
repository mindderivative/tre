# PLAN — M30 Phase 8 Steps 6-7: Main Menu submenus, closing Phase 8

## Goal
Add nested submenu support to `add_menu_item`/the existing Menu overlay
mechanism, then close Phase 8's own `.pyi` stub step (already satisfied
incrementally) — closing Phase 8 entirely, all 7 steps.

## Steps
1. Confirm no new overlay kind is needed: submenus reuse
   `build_menu`/`open_menu`/`Tree::open_overlay` directly, anchored to
   the parent item's own returned `Node`.
2. Add the twelfth curated icon, `chevron_right`, to
   `engine-md3/src/icons.rs` (verbatim from the real Material Symbols
   CDN).
3. Extend `add_menu_item` with a `submenu: bool = false` param
   (inserted between `icon` and `width`) that adds a real trailing
   chevron child — confirmed backward-compatible by grepping every
   real caller first.
4. Update `python/tre/_core.pyi`'s `add_menu_item` stub.
5. Write `tests/test_main_menu_submenus.py` and
   `examples/main_menu_submenus.py`.
6. Run the example — **found a real, confirmed bug live**: a click on
   a submenu item never reached its own handler.
7. Root-cause via direct read of `Tree::dismiss_overlays_outside`:
   `open_overlay` always positions content anchor-relative-below, so a
   submenu anchored to an item inside an already-open parent menu sits
   outside that parent menu's own bounds — the dismiss filter only
   checked a candidate overlay's own bounds, so a press inside the
   submenu was wrongly read as "outside" the parent, dismissing it and
   swallowing the press.
8. Fix: add an `inside_any_overlay` check to the filter — a press
   inside any open overlay can no longer dismiss a different one.
   Confirmed a true no-op for every existing single-overlay caller via
   the full pre-existing `cargo test --workspace --release` run.
9. Write a new, dedicated `engine-core` unit test proving the fix
   directly: a genuine two-overlay nested scene (not reusing the
   single-overlay `overlay_scene` helper), a press inside the inner
   overlay but outside the outer one, asserting the outer overlay
   survives.
10. Rebuild via `maturin develop --release`; re-run
    `tests/test_main_menu_submenus.py` and
    `examples/main_menu_submenus.py` — both now pass/exit cleanly.
11. Add the real Python-level "parent menu survives a submenu click"
    test that was earlier deliberately deferred pending the fix — now
    resolvable, proven behaviorally through a real `window.click()`
    reproduction (not `open_menu` called directly).
12. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
13. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 8 heading icon,
    Step 6 and Step 7 lines, "Just closed"/"Up next" trailer),
    regenerate + republish the Build Tracker artifact.
14. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-core`
151 tests up from 150, 446 pytest passed/1 skipped up from 440, all 62
examples, showcase demo, 43 Rust test binaries). Phase 8 fully closed,
all 7 steps.
