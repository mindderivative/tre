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
clean. cargo 373, pytest 451 + 1 skipped.

**M9 Phase 1 (2026-09-28).** Worktree tre-0.3.5.2 + branch 0.3.5.3 removed
(content on main); merged local release branches pruned; rect_window.rs
keeps its surface-bound adapter.

**Manual check (2026-09-29).** Wayland (KDE): all good, active = True.
X11 (XWayland): all good, active = True. macOS, Windows: can't be
checked by hand (user); CI only.

**M10, issue #21 (2026-09-29).** User: "before that check your issues".
Back/forward mouse buttons: PointerButton::Back/Forward, "back"/"forward",
no click. cargo 373, pytest 452 + 1 skipped.

**Released (2026-09-29).** PR #22 merged (97d0b4a), v0.4.1 tagged, 24
assets, PyPI 22 wheels + sdist, #21 closed, Tesserae told.
Next: Tesserae's report.
