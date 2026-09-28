# PLAN — Branch `0.4.0`: Milestone 1, Scope and Fork Setup

*(The `0.4.0` line's first plan. Its tracker, `BUILD_TRACKER.md`, restarts at
M1; the 0.3 line is archived in `BUILD_TRACKER_ARCHIVE_0.3.md`.)*

## Goal

Real GPU-level partial redraw through a `vello_hybrid` fork (issue #4): a
patched renderer that can skip the full clear and clear a scissored rect, a
persistent offscreen target, and dirty-region tracking (M2–M5), then the
release (M6).

## Steps

**Phase 1 — Decisions** (nothing is built before the user confirms these)
1. Re-verify issue #4's patch points against current `vello_hybrid`; fork
   0.2.0 (still the latest release) or upstream `main`.
2. The user's decisions: fork mechanics (a `mindderivative/vello` fork via
   `[patch.crates-io]` at a pinned revision, or vendored), dirty-region
   granularity (one rect or several), and partial redraw on by default or
   opt-in for `0.4.0`.

**Phase 2 — Fork setup**
1. The fork created and pinned, building unchanged, every test passing.

## Status

**Phase 1 done (2026-09-28).** Decisions: pinned upstream `vello_gpu` commit, Git dependency, a small set of dirty rects with a full-redraw fallback, on by default with a switch. Phase 2 Step 1 next.
