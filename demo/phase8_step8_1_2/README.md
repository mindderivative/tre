# Demo: Phase 8, Step 8.1.2 -- The Full 8-Stage Continuous Main Loop

```bash
./demo/phase8_step8_1_2/run_main_loop_demo.sh
```

The first demo to combine all 8 of Step 8.1's own named pipeline
stages -- *Wait Fences -> Drain Events -> Multi-Thread Canvas ->
Sub-Canvas Stitch -> Tessellation/Atlas Check -> Radix Sort & Batch ->
Ring Buffer Packing -> RHI Submit & Present* -- inside one real,
continuous, windowed loop. Every mechanism here was already proven
individually (Step 5.2.3's multi-threaded stitching, Step 4.3.1's atlas
recency tracking, Step 8.1.1's `FrameClock`/`spring_decay`); this demo
is the real-stress proof that they compose correctly run together,
every frame, not just once.

An amber rect eases in from the left edge toward the right, driven by
`FrameClock::tick()` feeding `spring_decay` every frame -- Step 8.1.1's
own two primitives, built with no consumer at the time, get their first
real one here. Two white rects near the top and one text glyph are
drawn every frame by 2 real OS worker threads (`std::thread::scope`,
spawned fresh each frame, matching Step 5.2.3's own proven recipe), then
stitched into a shared arena alongside the root canvas's own animated
rect.

**The real, previously-hidden bug this step fixed:** `execute_frame`
hardcoded a `0` byte offset at its `bind_vertex_buffer`/`bind_index_
buffer` calls, so it could never bind a real per-frame `RhiDynamic
RingBuffer`-backed segment -- only ever a one-shot-uploaded whole-frame
buffer. Every prior demo used `upload_buffer` for exactly this reason.
This demo instead calls the ring buffer's own `write()` every frame and
passes the real offset it returns straight into the now-fixed
`execute_frame`.

Runs for `${TRE_MAIN_LOOP_FRAMES:-90}` frames by default (env var to
change), or close the window to exit early.

**Verification, adapted to a real RHI constraint this step's own
investigation found:** `VulkanSwapchain` (a real window/compositor
surface) has no pixel-readback method the way `HeadlessSwapchain` does,
so this demo can't assert rendered pixels the way the headless demos
do. Instead it records the real per-frame `dt` sequence `FrameClock`
actually produced and the `x` position it actually drew each frame,
then independently replays `spring_decay` over that exact `dt` sequence
after the loop ends and asserts it reproduces the exact position
sequence drawn -- proving the formula was applied correctly every
single frame, not just that something moved on screen. On this
session's own real GPU and display: 90 frames presented, zero Vulkan
validation-layer errors, `x` moved from `40.0` to `559.89` against a
`560.0` target.

**What's explicitly out of scope** (see `documentation/IMPLEMENTATION.md`'s
Step 8.1.2 write-up for the full reasoning): live, mid-run atlas growth
(the atlas is fully pre-seeded before the loop starts -- its owner
thread's only readback API, `join()`, stops the thread); a persistent,
reused worker-thread pool (real OS threads are spawned fresh every
frame instead); SVG tessellation/morphing in the loop; Windows/macOS.
