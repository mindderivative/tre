# LOG — Branch `0.3.5`: Milestone 101

- M100 complete, pushed at `07be406`; Tesserae sent the updated
  `_removed.py` to rerun its gate. The user: "Push and send Tesserae the
  update, then start M101".
- Tesserae reran its gate against `07be406`: zero RemovedErrors.
- Baseline measured at `v0.3.4`: 81 s release build, 11.70 MB wheel,
  0.493 ms frame-budget median, 59,691 lines of Rust source.

## Done

1. Phase 1 Step 1: the legacy handler path, `EventKind`, `ActiveTree`,
   the `materializers` map, and `Tree::set_focused` removed; cargo 326,
   pytest 435, 15 examples, docs clean.
2. Phase 1 Step 2: `raw-window-handle`, the test-only `smallvec`, and
   `vello_hybrid`'s `text` feature removed; every test still passes.
3. Phase 2: against `v0.3.4` -- Rust source 59,691 → 27,483 lines, public
   API 193 → 46 members, wheel 11.70 → 10.32 MB, build 81 → 67 s; frame
   budget unchanged within noise.

## Status

**Complete (2026-09-27).** M102 next.
