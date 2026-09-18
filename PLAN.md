# PLAN — M30 Phase 8 Step 4: Pagination

## Goal
Add `Window.add_pagination` — MD3 has no official page; a plain
composition of page-number indicators plus prev/next controls.

## Steps
1. Confirm no official MD3 Pagination page.
2. Check curated icons: only "arrow_back" exists, no "forward". Fetch
   the real "arrow_forward" SVG path data verbatim from Google's CDN,
   add as the eleventh curated icon.
3. Design: page items reuse the 40dp circular footprint
   (SEARCH_ICON_BUTTON_SIZE), selected = primary/on_primary (Date
   Picker's own pattern), unselected = transparent/on_surface_variant
   (Segmented Button/Chip/Nav Rail's own pattern). Prev/next reuse
   Icon Button's own anatomy.
4. Implement `add_pagination` in `window_factory.rs`.
5. Fix a real borrow-checker error: a tree-capturing closure shared
   across two call sites with intervening code between them (the
   pages loop) -- converted to a plain, non-capturing fn taking
   &mut Tree explicitly.
6. Add `.pyi` stub.
7. Write `tests/test_pagination.py`, including independent-click
   tests for pages and prev/next.
8. Write `examples/pagination.py`, headless-CI-safe.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
10. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
    + republish the Build Tracker artifact.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (436 pytest
passed/1 skipped, 60 examples, showcase demo, 43 Rust test binaries).
