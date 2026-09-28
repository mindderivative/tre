# PLAN — Branch `0.4.0`: Milestone 4, Dirty-Region Tracking

*(Replaces the M3 plan — M3 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Know which parts of a window changed each frame: a small set of rects,
merged when they overlap, with a full redraw past a size limit (the user's
D3). Nothing uses them yet -- M5 renders only inside them.

## Steps

1. Accumulate each change's painted bounds, before and after: animations,
   property sets, layout and structure changes, canvas redraws, layers,
   including transforms, shadows, and clipping.
2. Tests that every kind of change reports a rect covering it, and that a
   change over most of the window falls back to a full redraw.

## Status

**Planned (2026-09-28).** Waiting on the user to start M4.
