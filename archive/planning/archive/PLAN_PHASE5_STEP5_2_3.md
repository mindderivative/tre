# Plan: Phase 5, Step 5.2.3 -- The Capstone: Real Concurrent Recording, Rendered

## Scope decisions

**Third and closing sub-step of Step 5.2, closing Phase 5 Step 5.2 in
full.** Every mechanism this capstone needs already exists and is
already unit-tested: `SubCanvas`/`create_sub_canvas` (5.2.1),
`tre_memory::ScatterArena`/`FrameArena`/`stitch_into` (5.2.2). This
sub-step adds essentially no new engine mechanism -- it proves the
whole chain end to end with real OS threads and a real GPU render,
matching `atlas_concurrency_demo`'s (Step 4.2.4) and
`atlas_eviction_demo`'s (Step 4.3.3) own precedent that a capstone's
job is proof under genuine stress, not new production code.

**One small, genuinely new addition: `RenderingCanvas::
max_sub_canvases(&self) -> usize`.** Nothing has needed to *read back*
the configured concurrency cap before now -- 5.2.1/5.2.2's own tests
all used the `#[cfg(test)]`-only cap override to work with a known,
fixed number. A real demo (or any real caller) deciding how many
worker threads to actually spawn needs to ask "how many can I have?"
first, rather than guessing and risking `create_sub_canvas()`'s own
panic. This is the first sub-step whose real consumer needs this
value, matching the project's own "build the tested primitive when its
exact consumer needs it" precedent -- a one-line accessor, not new
mechanism.

**The demo spawns `available_parallelism() - 1` real worker threads,
not a fixed number -- and asserts at least one is available rather
than silently degrading.** Hardcoding a small fixed thread count (e.g.
always 2) would be simpler but would understate what the cap actually
means and wouldn't adapt to real hardware; querying the exact same
value `RenderingCanvas::new()` computed internally (now readable via
`max_sub_canvases()`) is both more honest to the feature and
automatically portable across whatever machine or CI runner actually
runs it. Clamped to a demo-friendly maximum of 4 (so the on-screen
layout stays simple and legible regardless of how many cores the real
machine has) via `.clamp(1, 4)` -- the `assert!` on the lower bound is
a deliberate, loud failure on the one genuinely degenerate case (a
reported single-core machine, `max_sub_canvases() == 0`) rather than a
silently-smaller, less meaningful demo.

**The scene combines everything Step 5.2 (and 5.1.3) can prove into
one frame, sourced from real concurrent threads instead of one
single-threaded recording -- the actual new thing this capstone
demonstrates.** Reusing `canvas_batch_flattening_demo`'s own
established verification shape (exactly 3 real batches: merged plain
rects, a separate overlay-plane rect, a separate real MSDF text glyph)
but now: every worker-thread rect is recorded on its own real OS
thread and stitched into a shared `FrameArena` as that thread's own
last action (5.2.2's own "workers stitch themselves" design point,
exercised for real here rather than only in a unit test); one worker
thread's rect is wrapped in `begin_overlay`/`end_overlay` (proving
overlay routing survives a worker thread, not just the main thread);
one worker thread also draws the real MSDF text glyph (proving a
second real pipeline's content survives concurrent recording too); the
root canvas draws one more rect directly and stitches itself into the
*same* arena (proving root and worker contributions genuinely co-merge,
not just worker-to-worker).

**Text's atlas glyph is pre-seeded synchronously before any worker
thread is spawned**, matching `canvas_batch_flattening_demo`'s own
simpler single-frame pattern rather than `canvas_draw_text_demo`'s
two-frame miss-then-hit dance -- this capstone's own point is
concurrent *recording and stitching*, not re-proving the atlas's
cache-miss behavior a second time.

## Goal

A real demo spawns `min(available_parallelism() - 1, 4)` worker
threads, each holding its own `SubCanvas`; each records one rect (one
of them inside `begin_overlay`, one of them also drawing a real
atlas-backed MSDF glyph) and stitches itself into a shared `FrameArena`
as its own last action before exiting; the root canvas draws and
stitches one more rect directly; the coordinator joins every thread,
flattens the arena, and renders the result through the existing
`sdf_rounded_rect`/`bindless_textured.vert`+`msdf.frag` pipelines --
proving, with both an IR-level assertion (exactly 3 real batches,
matching Step 5.1.3's own worked-example shape) and real GPU pixel
readback, that a frame recorded across genuinely concurrent threads
renders every shape at its own correct, distinct position regardless
of thread scheduling order.

## Tasks

1. **`RenderingCanvas::max_sub_canvases(&self) -> usize`**: a plain
   accessor for the existing private field, so a real caller can size
   its own worker-thread count against the actual configured cap
   instead of guessing.

2. **New demo** (`crates/tre-rhi-vulkan/examples/canvas_sub_canvas_demo.rs`,
   `demo/phase5_step5_2_3/`):
   - Pre-seed one real glyph's atlas entry synchronously (real cascade
     font, real `AtlasOwner`, polled to resolution) before spawning any
     worker thread.
   - `worker_count = root.max_sub_canvases().clamp(1, 4)` (`assert!`
     that `max_sub_canvases() >= 1` beforehand, with a clear message
     naming the degenerate single-core case).
   - Root canvas draws one rect directly and calls `stitch_into` on a
     shared `Arc<FrameArena>`, sized exactly for `worker_count + 2`
     logical shapes' worth of vertices/indices/commands.
   - Spawn `worker_count` real `std::thread::spawn` threads, each
     already holding its own `SubCanvas` (created on the main thread
     via `create_sub_canvas()` before spawning, then moved in): thread
     `0` draws its rect inside `begin_overlay(OverlayLayerPriority(0))`/
     `end_overlay()`; the last thread also draws the pre-seeded glyph
     via `draw_text`; every thread calls `stitch_into(&arena)` as its
     own last action and asserts it returned `true`.
   - Join every thread, call `arena.flatten()`.
   - IR-level assertion: exactly 3 `DrawGeometry` commands survive --
     one merged plain-rect batch (`element_count == worker_count * 6`,
     covering every non-overlay worker rect plus the root's own rect),
     one standalone overlay-plane batch, one standalone text batch --
     the same 3-batch shape `canvas_batch_flattening_demo` already
     established, now reached via real concurrent recording instead of
     one single-threaded call sequence.
   - Real GPU render: walk `frame.commands`, switching between the
     `sdf_rounded_rect` and `bindless_textured.vert`/`msdf.frag`
     pipelines per command's own `pipeline_state_id` (the same
     multi-pipeline dispatch loop `canvas_batch_flattening_demo`
     already established) -- no new shader/RHI work.
   - Pixel verification: every rect's own center (each worker's, the
     overlay one's, the root's) reads back as real white fill; a
     background check between adjacent rects confirms they're
     genuinely distinct shapes; the text glyph's own quad region shows
     real non-background fill (the same whole-quad-scan precedent
     `atlas_concurrency_demo`/`canvas_draw_text_demo` already
     established).
   - Report the real `worker_count` actually used, so the demo's own
     output is transparent about what ran on this particular machine.

3. **Docs**: IMPLEMENTATION.md Step 5.2.3 subsection, closing Step 5.2
   (5.2.1-5.2.3) in full; REVIEW.md entry.

4. **CI**: add `canvas_sub_canvas_demo` to the `vulkan-validation`
   job's example list.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `canvas_sub_canvas_demo` run under `VK_LAYER_KHRONOS_validation`,
  zero errors, on the actual development machine's own real core count
  (not a fixed/mocked value).
- All pre-existing examples re-run manually as a final regression
  check, closing out Step 5.2 in full.

## Explicitly out of scope for this sub-step

- Any new engine mechanism beyond the one accessor -- everything else
  this demo exercises already exists and is already unit-tested.
- DESIGN.md Section 2.6's prioritized-degradation policy for arena
  overflow -- still deferred (5.2.2's own scope note); this demo sizes
  its arena exactly, so overflow never occurs in practice here.
- Nested sub-canvases, or a `SubCanvas` spawning its own children --
  still out of scope, unchanged since 5.2.1.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code -- reuses the
  existing `sdf_rounded_rect`/`bindless_textured.vert`/`msdf.frag`
  pipelines exactly as `canvas_batch_flattening_demo` already does.
