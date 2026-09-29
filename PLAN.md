# PLAN — Branch `0.4.0`: Milestone 6 Phase 2, the Review's Decisions

*(Replaces the M5 plan — M5 is complete, and M6 Phase 1 is the pre-release
review. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Act on every open item of the pre-release review
([report](https://claude.ai/artifact/X4Xo6Zwk4spCRxnDTWQMk3)), as the user
decided (2026-09-28): "1. Unify them 2. Move into a per-window 3. raise an
exception 4. Yes 5. Fix them all".

## Steps

1. MSRV and release pins: `rust-version` from the real dependency floor,
   an `msrv` CI job; third-party release actions pinned to commit SHAs.
2. No GPU adapter raises a Python exception from `App.run()` instead of
   exiting the process; a zero-sized window is rejected at construction.
3. One tree walk: the paint walk and the damage walk share one traversal
   (visibility, culling, opacity, clip narrowing, child order).
4. A per-window renderer in `engine-render` owning the frame renderer,
   caches, persistent target and tracker, with one frame method -- used by
   `app.rs`, the pixel tests, and the benchmark.
5. Cheaper terminal and canvas fingerprints. (Done with a fast hash, not
   a content counter: a counter would be instrumentation, which the
   tracker avoids.)
6. A lost surface is recreated; a frame with no damage skips the present
   when the tree asked for it (not when the OS did).
7. The window's shared fields in one struct instead of three copies;
   `get("partial_redraw_active")` reports the fallback.
8. The older GPU tests use `tests/support`'s shared setup and readback.
9. Images drawn from `vello_gpu`'s image atlas, if it keeps in-place frame
   updates; otherwise recorded why not.
10. `engine-core`'s `tree.rs` split by concern.

Each step: the full chain, docs, tracker, a local commit.
