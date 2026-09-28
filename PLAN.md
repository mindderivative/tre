# PLAN — Branch `0.4.0`: Milestone 3, A Persistent Offscreen Target and Blit

*(Replaces the M2 plan — M2 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

A window's swapchain image keeps nothing from the previous frame, so partial
redraw needs a texture that does: render into a persistent per-window
texture and copy it to the swapchain image each frame. No visible change.

## Steps

1. The per-window target texture, recreated on resize and scale-factor
   change, rendered into and copied to the acquired swapchain image.
2. Pixel tests that the copied output matches the direct render exactly, and
   no regression in `frame_budget.rs`.

## Status

**Planned (2026-09-28).** Waiting on the user to start M3.
