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

## Status

**In progress.** Phase 1 Step 2 (dependencies) next.
