# LOG — M30 Phase 8 Steps 6-7: Main Menu submenus, closing Phase 8

- Confirmed no new overlay kind is needed — a submenu is just another
  real `build_menu`/`open_menu` pair, opened with the parent item's
  own returned `Node` as the anchor, reusing `Tree::open_overlay`
  exactly as every other overlay in this catalog already does.
- Fetched `chevron_right`'s real SVG path data verbatim from the same
  Material Symbols CDN this catalog's icon module already cites,
  confirmed the same `viewBox="0 -960 960 960"` convention. Added as
  the twelfth curated icon in `crates/engine-md3/src/icons.rs`.
- Extended `add_menu_item` in `crates/engine-py/src/window_factory.rs`
  with a `submenu: bool = false` param, inserted between `icon` and
  `width`. When true, reserves `MENU_ITEM_ICON_SIZE +
  MENU_ITEM_ICON_GAP` from `label_width` and adds a real trailing
  `chevron_right` icon child. Confirmed backward-compatible by
  grepping every real caller first — all keyword-only.
- Updated `python/tre/_core.pyi`'s `add_menu_item` stub.
- Wrote `tests/test_main_menu_submenus.py` (5 tests originally) — all
  passed on the first run, though (as later understood) none of them
  actually exercised the real bug scenario below (none opened both a
  parent menu *and* its own submenu simultaneously via `open_menu`).
- Wrote `examples/main_menu_submenus.py`, a full click-driven
  reproduction (`window.click(trigger)` → `window.click(export_item)`
  → `window.click(pdf_item)`).
- **Ran the example — it genuinely FAILED**:
  `AssertionError: a real click on a submenu item must reach its own
  registered handler`. Not predicted in advance.
- Root-caused by direct read of `Tree::dismiss_overlays_outside`:
  `open_overlay` always positions content anchor-relative-*below*, so
  a submenu anchored to an item inside an already-open parent menu
  genuinely sits outside that parent menu's own bounds. The dismiss
  filter only ever checked a *candidate* overlay's own bounds against
  the press point — a press genuinely inside the submenu was wrongly
  read as "outside" the *parent* menu, dismissing it and consuming the
  press via `dispatch`'s own early-return before the submenu item's
  own handler ever ran.
- Fixed: added an `inside_any_overlay` check to the filter — a press
  inside *any* currently-open overlay can no longer dismiss a
  *different* one, since the user is still interacting with the
  overlay system as a whole. For every existing single-overlay caller
  (Menu, Tooltip, Search View, Popover) this is a true no-op.
- Re-ran `cargo check`/`clippy -D warnings`/`fmt --check` clean, then
  the full `cargo test --workspace --release` — all green, zero
  regressions, confirming the no-op claim for the single-overlay case.
- Wrote a new, dedicated `engine-core` unit test,
  `a_press_inside_a_nested_submenu_does_not_dismiss_its_own_parent_menu`,
  building a genuine two-overlay nested scene from scratch (not
  reusing the single-overlay `overlay_scene` helper): a `trigger`
  anchoring `parent_menu`, a real child `parent_item` filling
  `parent_menu` exactly, anchoring `submenu` immediately below —
  `submenu` starts exactly where `parent_menu` ends. A press at
  `submenu`'s own center is inside `submenu` but outside
  `parent_menu`. Passed on the first run, confirming the fix directly.
  `engine-core` test count: 151, up from 150.
- Ran `maturin develop --release` to rebuild the Python extension with
  the fix, then re-ran `examples/main_menu_submenus.py` — now exits
  cleanly, `selected format 'PDF'`. Re-ran
  `tests/test_main_menu_submenus.py` — all 5 original tests still pass.
- Added the real Python-level "parent menu survives a submenu click"
  test that was earlier deliberately deferred (uncertain of the real
  geometric outcome before the fix) — now resolvable:
  `test_the_parent_menu_survives_a_real_click_on_its_own_submenu_item`,
  a full `window.click()`-driven reproduction (not `open_menu` called
  directly) proving the parent menu's own survival *behaviorally*: a
  second click on `export_item` after the submenu click still reaches
  its own handler, only possible if `export_item` (a real child of
  `file_menu`) is still genuinely attached to the tree. Passed.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (`engine-core` 151, up
  from 150), `maturin develop --release`, `mypy --strict` clean
  against `examples/main_menu_submenus.py`, `pytest tests/` (446
  passed, 1 skipped, up from 440 — 6 in `test_main_menu_submenus.py`,
  up from the original 5), all 62 examples clean, showcase demo clean.
- Step 7 (`.pyi` stubs) confirmed already satisfied incrementally —
  every Step 1-6 method already has a real stub entry — closing Phase
  8 entirely, all 7 steps.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 90%, Phase 8 heading
  icon, Step 6/7 lines, "Just closed"/"Up next" trailer), regenerated
  and republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
