# PLAN — M40: Smooth Window Resize (Coalesced Swapchain Reconfigure)

## Goal
Fix the real resize-smoothness gap M33 never touched: `GpuState::resize`
reconfigures the wgpu surface (a genuine swapchain rebuild) on every
single `WindowEvent::Resized` event, with zero coalescing -- the real
root cause behind "window trailing behind cursor and stuttering" the
user remembered from TRE v1 and pyCopper.

## Steps
1. Real winit investigation: confirmed via direct source read (`winit
   = "0.30.13"`) that `Resized` is the only public cross-platform
   resize-related `WindowEvent` -- no enter/exit-live-resize signal
   exists anywhere in the public API, resolving the scoping note's own
   open question (frame-count-based coalescing is the only portable
   option).
2. **Real, load-bearing correction made before writing any production
   code:** built a throwaway `wgpu`+`winit` scratch probe, run against
   this session's own real KWin/Wayland compositor, screenshotted via
   `spectacle`. Configured a surface at 2x a fixed window's own size,
   painted a marker filling a known fraction of the oversized buffer.
   **Real result: the marker appeared at native, unscaled size, cropped
   to the window's own top-left corner -- not scaled down.** This
   compositor does not scale an oversized buffer to fit by default; it
   crops to the surface's own declared window geometry. Both TRE v1 and
   pyCopper's own real, working versions of the coarse-bucket technique
   depend on `wp_viewporter`-level scaling support to avoid exactly this
   -- the same fork this milestone's own scoping had already deferred.
   The originally-scoped design (port both projects' bucket-and-settle
   mechanism) would have shipped a real, visibly broken crop.
3. A second scratch probe measured this machine's own real
   `surface.configure()` cost: ~600µs-1ms/call (AMD Radeon 890M, RADV/
   Vulkan, 100 real reconfigures) -- modest enough that no bucketing is
   needed to fix the real root cause.
4. **Real, corrected design:** `InputEvent::Resized`'s handler (`app.
   rs`) no longer calls `GpuState::resize` inline -- it only updates the
   real, live `runtime.width`/`height` `SharedSize` cells (zero added
   lag, unchanged). New `GpuState::needs_resize(width, height) -> bool`;
   the per-frame `RedrawRequested` closure's `take_dirty()` early-return
   is widened to also check it (a pending resize touches no `Tree`
   state, so `take_dirty` alone never sees it), and calls the existing
   `resize` exactly once, right before acquiring the surface texture,
   whenever it's true -- always at the window's true current size. Any
   burst of `Resized` events between two real frames collapses into one
   real reconfigure, eliminating the redundant cost with no compositor-
   scaling assumption and no fork.
5. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `cargo test --workspace --release` (unchanged counts -- a pure
   internal render-loop refactor, no new headless-testable pure-logic
   surface), `maturin develop --release`, full `pytest tests/`
   (including `test_resize.py`'s own 5 tests), all 77 examples, showcase
   demo.
6. `BUILD_TRACKER.md` both phases flipped to done with the full real
   investigative story (the crop finding, the measured cost, the design
   correction); milestone status/Top Metrics to Complete; a new "Just
   closed" trailer. Artifact regenerated (40/129/220, unchanged) and
   republished.

## Status
Complete. Full verification chain green (unchanged test/pytest/example
counts, as expected for an internal render-loop refactor with no new
pure-logic surface). **M40 -- Smooth Window Resize -- is now complete,
both phases.** Real, honestly-stated limit: whether this actually feels
smoother during a genuine live OS drag can't be verified from this
environment (no window-manipulation tool available to simulate one,
and "feels smooth" is inherently a hands-on-the-mouse judgment) --
left for the user to try directly against a real window (any example
run with `app.run(max_frames=None)`).
