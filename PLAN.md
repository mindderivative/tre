# PLAN — M30 Phase 8 Step 1: Popover

## Goal
Add `Window.add_popover` — grounded in MD3's real Rich Tooltip
anatomy per pyCopper's own prior research, reused per the tracker's
own scope note but with token values re-verified fresh.

## Steps
1. Fetch real Rich Tooltip tokens via `_md-comp-rich-tooltip.scss`:
   surface_container, corner-medium (12dp, reuse CARD_CORNER_RADIUS),
   level2 elevation (reuse MENU_PANEL_ELEVATION), Title Small subhead
   (reuse TAB_LABEL_FONT_SIZE/_WEIGHT), Body Medium supporting text
   (reuse DIALOG_BODY_FONT_SIZE/_WEIGHT).
2. Decide lifecycle: reuse open_menu/close_menu directly (Tooltip's
   own precedent) rather than new dedicated methods -- "persistent"
   means no hover-exit dismissal, not immunity to outside-click.
3. Implement `add_popover` in `window_factory.rs`, mirroring Dialog's
   own headline+body column layout without the scrim.
4. Add `.pyi` stub.
5. Write `tests/test_popover.py`, including an explicit test proving
   the outside-click dismissal behavior (fixed a misleading test name
   before writing -- the popover does dismiss on outside click,
   "persistent" only means immune to hover-exit).
6. Write `examples/popover.py`, headless-CI-safe.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
8. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 8 heading now
   🚧, step line), regenerate + republish the Build Tracker artifact.
9. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (420 pytest
passed/1 skipped, 57 examples, showcase demo, 43 Rust test binaries).
