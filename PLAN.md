# PLAN — Branch `0.3.5`: Milestone 101, Consolidation and Size Pass

*(Replaces the M100 plan — M100 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

The "no duplicate code" goal, measured rather than asserted: remove what
M98–M100 left unreachable, fold duplicated logic, prune dependencies, and
record the before-and-after size with no performance regression.

## Baseline

`v0.3.4`, the last release before the removals (M97–M100): lines of Rust,
public Python API count, wheel size, release build time, and the
frame-budget benchmark, measured on this machine the same way as the
after figures.

## Steps

**Phase 1 — Dead code and duplication**
1. Unreachable code: the legacy handler path (`HandlerMap`'s per-`EventKind`
   handlers, `call_handler`, the legacy `Event` constructors, and
   `run_dispatch_outcome`'s legacy half) now that nothing registers a
   legacy handler; `DispatchOutcome`/`InputEvent` variants and `Tree`
   methods nothing produces or calls; per-kind branches and comments for
   kinds that no longer exist; then fold duplicated logic.
2. Unused Cargo dependencies and features, checked crate by crate against
   the code.

**Phase 2 — Measurement**
1. The after figures against the baseline, and the frame-budget benchmark
   rerun.

Each step: the full standing chain, the tracker, a local commit, memory.

## Status

Phase 1 Step 1 done (2026-09-27). Step 2, the dependencies, next.
