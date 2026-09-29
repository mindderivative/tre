# PLAN — Branch `0.4.1`: Milestone 7, Partial Redraw's Fixed Costs

*(Replaces the M6 plan — `v0.4.0` is released. Every step is in
`BUILD_TRACKER.md`.)*

## Goal

`v0.4.0`'s one worse number: with partial redraw on, a change covering
most of the window costs 0.2-1.8 ms more a frame than with it off -- the
damage tracker fingerprints every node before settling on a full redraw.
Remove that, and see whether a small partial frame's copy to the surface
is worth cutting too.

## Steps

1. Measure where a frame's time goes -- damage walk, scene, render, copy --
   for the small and whole-window workloads of `partial_redraw_bench`.
2. Stop the damage walk once the frame is known to be a full redraw; the
   next frame compares afresh (one extra full frame when a whole-window
   change ends, instead of a wasted walk on every frame of it).
3. Measure the copy to the surface in a small partial frame, and cut it if
   it's a real share.

Each step: the full chain, docs, tracker, a local commit.
