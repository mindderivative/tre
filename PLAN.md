# PLAN — Branch `0.4.0`: Milestone 2, Migrate to `vello_gpu`

*(Replaces the M1 plan — M1 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Move `tre` from `vello_hybrid` 0.2.0 to upstream `vello_gpu` at the commit M1
pinned (`linebender/vello` `b408cd00`), with no behavior change. Partial
redraw itself is M3–M5; this milestone only changes what `tre` builds on.

## Steps

1. The manifests: one `[workspace.dependencies]` table pins `vello_gpu`,
   `vello_common`, and `glifo` to the commit and `wgpu` to 30; then `tre`'s
   own code updated for `wgpu` 30 and `vello_gpu`'s API, with `render()`
   called as `TargetInit::Clear(ClearSettings::Viewport)` -- today's full clear.
2. Verification: every Rust test and pixel test, pytest, every example, and
   `frame_budget.rs` unchanged; CI builds the wheel with the Git dependency on
   all three platforms.

## Status

**Step 1 done (2026-09-28).** Step 2, CI on all three platforms, next.
