# PLAN — M30 Phase 8 Step 2: Link

## Goal
Add `Window.add_link` — a real, standalone clickable label. Fulfills
an explicit commitment from Phase 1's own hit-test fix: a bare Text
node deliberately never independently claims a hit, so a real
standalone clickable label needs its own dedicated NodeKind.

## Steps
1. Confirm no official MD3 Link token page exists (directory
   listing).
2. Design: add a genuine new `NodeKind::Link(TextState)` to
   engine-core (reusing TextState verbatim, not a new struct). By not
   matching Text's own `=> false` hit-test arm, it falls through to
   the existing catch-all for free hit-testing independence.
3. Add the enum variant, fix every exhaustive match the compiler
   surfaces (engine-render's paint_node, engine-py's kind_name).
4. Write a direct engine-core unit test proving the real contrast:
   identical geometry with a Text child (defers) vs a Link child
   (claims the hit).
5. Implement `add_link` in `window_factory.rs` (primary color, Body
   Large label, reusing existing constants).
6. Add `.pyi` stub.
7. Write `tests/test_link.py`, reproducing the Text-vs-Link contrast
   through the real Python FFI.
8. Write `examples/link.py`, headless-CI-safe.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
10. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
    + republish the Build Tracker artifact.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (424 pytest
passed/1 skipped, 58 examples, showcase demo, engine-core 150 tests
up from 149).
