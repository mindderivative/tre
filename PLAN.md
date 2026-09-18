# PLAN — M30 Phase 5 Step 2: Navigation Drawer

## Goal
Add `Window.add_navigation_drawer`/`open_navigation_drawer`/
`close_navigation_drawer` — the real desktop counterpart to Bottom
App Bar's navigation role, docked to the left edge, in Standard and
Modal variants (Side Sheet's own real behavioral fork).

## Steps
1. Verify real Navigation Drawer item/destination tokens via WebFetch
   (container tokens already known from Side Sheet's own earlier
   investigation, since both share the same source file).
2. Trace Label Large's real active/inactive weights through
   `_md-sys-typescale.scss`/`_md-ref-typeface.scss` — confirm they
   match Navigation Rail's own already-found 500/700 pair.
3. Design to proactively avoid Navigation Rail's own hit-test bug:
   make the indicator pill itself the returned, interactive node
   (icon+label side by side inside it), not a decorative layer nested
   inside a separate outer item wrapper.
4. Implement `add_navigation_drawer` (reusing SIDE_SHEET_* container
   constants, mirrored corner rounding for the left-docked edge) and
   `open_navigation_drawer`/`close_navigation_drawer` (Side Sheet's
   own exact lifecycle pattern).
5. Add `.pyi` stubs for all three methods.
6. Write `tests/test_navigation_drawer.py`, including a third
   independent OverlayMeta.modal proof and an independent-click test
   for destinations (watching whether it passes first try, unlike
   Navigation Rail's).
7. Write `examples/navigation_drawer.py`, headless-CI-safe,
   demonstrating both variants — apply the Callable return-type
   annotation proactively this time (Navigation Rail's own mypy
   finding).
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
   + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (347 pytest
passed/1 skipped, 48 examples, showcase demo, 43 Rust test binaries).
The proactive design avoided Navigation Rail's hit-test bug entirely
-- the independent-click test passed on the first run.
