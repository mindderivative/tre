# PLAN — M30 Phase 8 Step 3: SpinBox

## Goal
Add `Window.add_spin_box` — a real numeric increment control,
deliberately named to avoid MD3's own "Stepper" naming collision
(pyCopper's own prior finding).

## Steps
1. Confirm no official MD3 page for SpinBox/Stepper naming.
2. Check the curated icon set: only "add" exists, no decrement
   glyph. Fetch the real "remove" (minus) SVG path data verbatim
   from Google's CDN, add as the tenth curated icon.
3. Design: reuse TextField for the numeric field (Time Input's own
   surface_container_highest/corner-small convention), reuse Icon
   Button's own anatomy for decrement/increment (40dp, add/remove
   icons).
4. Implement `add_spin_box` in `window_factory.rs`.
5. Add `.pyi` stub.
6. Write `tests/test_spin_box.py`, including a real typing/focus
   round-trip and independent-click tests for both buttons.
7. Write `examples/spin_box.py`, headless-CI-safe.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
   + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (429 pytest
passed/1 skipped, 59 examples, showcase demo, 43 Rust test binaries).
