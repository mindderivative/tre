# LOG — Branch `0.4.1`: Milestone 7

## Status

**Scaffolded (2026-09-28).** Branch `0.4.1` off `main` at `53bb1bb`,
versions 0.4.1. User: "yes" to 0.4.1 = the overhead fix (M7), a
redrawn-areas overlay (M8), housekeeping and release (M9) with a manual
cross-platform check before tagging.

**M7 Step 1 (2026-09-28).** Bench split into prepare/draw/gpu + idle
workload. Whole-window overhead = damage walk 0.10-0.12 ms (the rest of
v0.4.0's reported gap was noise). Copy of the kept frame = 0.25 ms.

**M7 complete (2026-09-28).** User: "Do your recommendation" -- Steps 2-3
decided against (0.05 ms gain / no buffer age).

**M8 complete (2026-09-28).** show_damage overlay: SrcOver render into the
surface after the frame's submit; GPU test proves the kept frame stays
clean. cargo 373, pytest 451 + 1 skipped. Next: M9.
