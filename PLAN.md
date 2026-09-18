# PLAN — M30 Phase 4 Step 3: Side Sheet (closes Phase 4)

## Goal
Add `Window.add_side_sheet`/`open_side_sheet`/`close_side_sheet` —
the real desktop counterpart to Bottom Sheet, in Standard (embedded,
non-overlay) and Modal (floating, blocking) variants.

## Steps
1. Verify real MD3 Side Sheet tokens via WebFetch — 404, confirming
   no dedicated token file exists (matching Menu's own earlier
   finding). Fall back to Navigation Drawer's own real tokens, the
   structurally closest MD3 component.
2. Decide the real behavioral fork: Standard is a plain layout
   participant (Card's own precedent — attach immediately, optional
   x/y), Modal is a real blocking overlay (Dialog's own precedent —
   scrim + OverlayMeta.modal, unattached until opened).
3. Implement `add_side_sheet` (builds both variants from one method,
   corner_radii_override for the real corner-large-end shape) and
   `open_side_sheet`/`close_side_sheet` (modal-only lifecycle,
   real explicit no-op for standard sheets).
4. Add `.pyi` stubs for all three methods.
5. Write `tests/test_side_sheet.py`, including a real click-dispatch
   proof that the modal variant blocks background clicks (reusing
   Dialog's own OverlayMeta.modal capability against a second,
   independently-built overlay) and a real re-parenting proof for the
   standard variant.
6. Write `examples/side_sheet.py`, headless-CI-safe, demonstrating
   both variants.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
8. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 4 heading now
   ✅, step line, Step 4 .pyi-stubs line), regenerate + republish the
   Build Tracker artifact. Closes Phase 4 entirely.
9. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (327 pytest
passed/1 skipped, 46 examples, showcase demo, 43 Rust test binaries).
Phase 4 (Overlay-Dependent) is now fully complete, all 4 steps.
