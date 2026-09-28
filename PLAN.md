# PLAN — Branch `0.4.0`: Milestone 5, Partial Redraw End to End, Measured

*(Replaces the M4 plan — M4 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

The frame renders only what changed: `DamageTracker`'s rects clear and
redraw inside the persistent target, the rest kept from the last frame. On
by default with a switch to turn it off (D4), made safe by a test requiring
identical pixels from partial and full redraw.

## Steps

1. Integration: per window, `DamageTracker` runs before the scene is built;
   `Damage::None` skips rendering, `Rects` renders the scene clipped to the
   rects with `TargetInit::Clear(ClearSettings::Rects)` and culls nodes
   outside them, `Full` renders as now; `reset` when the target is recreated;
   the off switch.
2. Correctness: pixel tests that partial redraw matches full redraw across
   every example's animations.
3. Measurement against `v0.3.5.1`: a small animation in a large window, and
   no regression for a full-window change.

## Status

**Planned (2026-09-28).** Waiting on the user to start M5.
