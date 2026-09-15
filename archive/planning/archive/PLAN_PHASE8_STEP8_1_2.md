# Plan: Phase 8, Step 8.1.2 -- The Full 8-Stage Continuous Main Loop

## Goal

Wire the entire engine into one real, continuously-running, windowed
loop that executes IMPLEMENTATION.md Step 8.1's own strictly-enforced
8-stage sequence every frame -- *Wait Fences -> Drain Events ->
Multi-Thread Canvas -> Sub-Canvas Stitch -> Tessellation/Atlas Check ->
Radix Sort & Batch -> Ring Buffer Packing -> RHI Submit & Present* --
and fix the one real, previously-undiscovered blocker found while
scoping Step 8.1.1 (`planning/archive/PLAN_PHASE8_STEP8_1_1.md`):
`execute_frame` hardcodes a zero vertex/index buffer offset, so it
cannot accept a real per-frame `RhiDynamicRingBuffer`-backed buffer at
all today.

## Real investigation, this session

Every stage below already has one real, individually-proven mechanism
somewhere in the codebase; no demo has ever combined all 8 together in
one continuous loop. Confirmed by re-reading the real, current
implementation (not from memory) before writing this plan:

1. **Wait Fences is already inside `begin_frame`,** not a separate call
   the loop needs to add. `VulkanDevice::begin_frame`
   (`crates/tre-rhi-vulkan/src/lib.rs:1740`) waits on and resets the
   frame's fence, grows pending transient targets, and drains the
   deferred-release queue -- all *before* acquiring the swapchain image.
   Every windowed demo already gets this for free by calling
   `begin_frame` first.

2. **Drain Events is `PlatformConnection::poll_events`**
   (`crates/tre-platform/src/lib.rs:101`), already used exactly this way
   by all 3 existing windowed demos (`walking_skeleton.rs:105`,
   `multi_window.rs`, `input_demo.rs`) to detect
   `InputEvent::CloseRequested` and break the loop.

3. **Multi-Thread Canvas / Sub-Canvas Stitch already has a real, proven
   per-frame recipe** -- `canvas_sub_canvas_demo.rs` (Step 5.2.3's
   capstone): `root.create_sub_canvas()` per worker,
   `std::thread::scope` to spawn real OS threads, each thread calling
   `sub.stitch_into(&arena)` as its own last action, then
   `root.stitch_into(&arena)`. This demo only does it once
   (single-frame, headless); nothing stops repeating the identical
   recipe every loop iteration -- no new engine code needed for this
   stage, only new orchestration code in the demo.

4. **Tessellation/Atlas Check has a real gap beyond `execute_frame`'s
   offset, found by reading `tre-atlas`'s own `AtlasOwner` in full
   (`crates/tre-atlas/src/owner.rs`), not assumed.** The atlas owner's
   background thread (`run_owner_loop`) owns the shared atlas pixel
   buffer entirely privately -- the *only* way to ever read it back is
   `AtlasOwner::join(self) -> Vec<u8>`, which **consumes the owner and
   stops its thread**. There is no live "peek at the atlas's current
   pixels while the owner thread keeps running" API. Every existing
   demo that draws text (`canvas_sub_canvas_demo.rs`,
   `canvas_draw_text_demo.rs`) pre-seeds every glyph it will ever need,
   calls `join()` once, and uploads that one static buffer as a GPU
   texture *before* any drawing -- none of them ever re-upload the atlas
   after that. A live loop that could add genuinely new glyphs mid-run
   and see them appear on screen needs a new `AtlasOwnerHandle` API
   (e.g. a snapshot/dirty-generation read) that does not exist and is
   real, separate engineering, not just wiring -- explicitly out of
   scope below.

5. **Radix Sort & Batch is `FrameArena::flatten()`**, already fully
   proven by Step 5.1.3/5.2.3 -- no new code needed.

6. **Ring Buffer Packing's real gap is exactly what Step 8.1.1's own
   plan predicted, now confirmed precisely.** `VulkanRingBuffer::write`
   (`crates/tre-rhi-vulkan/src/lib.rs:3953`) derives which of the 3
   frame-in-flight segments to write into from `self.frame_sync.
   frame_index` and returns a real, non-zero absolute byte offset once
   the ring has advanced past its first segment. `execute_frame`
   (`crates/tre-engine/src/lib.rs:2423`) calls `cmd_buffer.
   bind_vertex_buffer(vertex_buffer, 0)` / `bind_index_buffer(index_
   buffer, 0)` with a hardcoded literal `0` -- confirmed by direct
   read, not memory -- even though `RhiCommandBuffer::bind_vertex_
   buffer`/`bind_index_buffer` (`crates/tre-engine/src/lib.rs:2284-
   2285`) already accept an `offset: u32` parameter for exactly this
   purpose. `memory_pools_demo.rs`, the ring buffer's one real non-test
   caller, only proves segment rotation with no draw calls at all.

7. **RHI Submit & Present is `RhiDevice::submit_and_present`**
   (`crates/tre-engine/src/lib.rs:2265`), already proven by every
   existing demo.

## Scope decisions

1. **Fix `execute_frame` to accept real per-frame buffer offsets,
   rather than inventing a second executor.** Add two new parameters,
   `vertex_offset: u32` and `index_offset: u32`, used in place of the
   hardcoded `0` literals at its two `bind_vertex_buffer`/`bind_index_
   buffer` call sites. A minimal, surgical signature change: every
   other line of `execute_frame` is unaffected. All 12 real call sites
   (7 `tre-engine` unit tests using `FakeCommandBuffer`, 5 demos) pass
   `0, 0` to preserve their exact current behavior -- confirmed via
   `grep -rn "execute_frame("` before writing this plan, not assumed.
   Only the new demo below passes real, non-zero ring-buffer-derived
   offsets.

2. **The new continuous loop lives in one new demo
   (`main_loop_demo.rs`), not inside `tre-engine` itself.** Matches
   every prior "stitch together already-proven mechanisms" step's own
   precedent (`canvas_sub_canvas_demo.rs`, `canvas_combined_scene_
   demo.rs`): the engine's job is to expose correct primitives, not to
   own one specific application loop shape. A future real UI framework
   embedding this engine will write its own loop; this demo proves the
   *engine* supports the 8-stage sequence correctly, matching Step
   0/Phase 6's own "thin vertical slice, not the final shape" role.

3. **FrameClock and `spring_decay` (Step 8.1.1) get their first real
   consumer here, as originally deferred.** Each frame, `FrameClock::
   tick()` supplies a real `dt`; one shape's position is animated
   toward a moving target via `spring_decay`, giving the two primitives
   built zero-consumer in 8.1.1 a real, visible proof in the same step
   that first needs them, rather than leaving them unconsumed
   indefinitely.

4. **The atlas is fully pre-seeded before the loop starts, matching
   every existing demo's own real, proven pattern -- live mid-run atlas
   growth is explicitly out of scope (see below).** The loop's
   "Tessellation/Atlas Check" stage is real and exercised every frame
   (`draw_text`'s own `atlas_context.atlas.lookup` call, which also
   real-touches `SwmrSlotTable`'s recency tracking, Step 4.3.1's own
   eviction-resistance mechanism) -- it is simply never a cache *miss*
   during this demo's run, exactly like every prior demo that draws
   text.

5. **Multi-threading uses `std::thread::scope` fresh every frame,
   matching Step 5.2.3's own proven recipe exactly -- a persistent,
   reused worker-thread pool is explicitly out of scope.** Real OS
   thread spawn/join every frame is not how a production frame loop
   would be tuned, but IMPLEMENTATION.md's own task 3 asks only to
   "stitch the entire engine together following a strictly enforced
   8-stage sequence," not to optimize thread lifecycle -- inventing a
   pooled-thread design now would be scope no prior step asked for, on
   top of a mechanism (`create_sub_canvas`/`stitch_into`) never before
   exercised in a repeating loop at all.

## Tasks

1. `tre-engine`: add `vertex_offset: u32`/`index_offset: u32` parameters
   to `execute_frame`, replacing the two hardcoded `0` literals at its
   `bind_vertex_buffer`/`bind_index_buffer` call sites. Update all 12
   existing call sites (7 unit tests, 5 demos) to pass `0, 0`.
2. `tre-engine`: one new unit test on the existing `FakeCommandBuffer`
   test double, asserting a non-zero `vertex_offset`/`index_offset`
   passed to `execute_frame` is recorded verbatim on the resulting
   `BindVertexBuffer`/`BindIndexBuffer` calls -- proving the fix, not
   just that it compiles.
3. New demo, `crates/tre-rhi-vulkan/examples/main_loop_demo.rs`: a real
   windowed, `CloseRequested`-aware, env-var-frame-capped loop (matching
   `walking_skeleton.rs`/`multi_window.rs`/`input_demo.rs`'s own
   precedent) that, every frame: drains events; spawns worker
   sub-canvases via `thread::scope` and stitches them into a shared
   `FrameArena` alongside the root canvas's own draws (one of which is
   animated via `FrameClock::tick()` + `spring_decay` toward a moving
   target); flattens the arena; writes the flattened vertex/index bytes
   into a real `RhiDynamicRingBuffer` via `write()`; calls the now-fixed
   `execute_frame` with the real offsets `write()` returned; and
   presents via `submit_and_present`. Real pixel assertions on at least
   one frame (matching every prior visual demo's own discipline):
   the animated shape's position after N frames must differ from its
   starting position by an amount consistent with `spring_decay`'s own
   formula, not just "something rendered."
4. Update `documentation/IMPLEMENTATION.md` -- add a "Step 8.1.2"
   write-up below the existing "Step 8.1" heading, matching the
   6.4/7.2/8.1.1 split precedent, including today's Tessellation/Atlas
   Check and thread-pool scope decisions as real, named future work
   (matching this project's own deferral discipline for Windows/macOS,
   blur filters, and HDR).

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- The new `FakeCommandBuffer`-based unit test passes, proving the
  offset fix directly.
- `main_loop_demo` runs end to end under `xvfb-run` (matching CI's
  `vulkan-validation` job's own existing pattern for windowed demos),
  zero Vulkan validation-layer errors, real pixel assertions pass.
- All 5 existing demos whose `execute_frame` call site changed
  signature are re-run manually, zero regressions.
- New demo added to `ci.yml`'s `vulkan-validation` job, matching every
  prior windowed demo's own precedent.

## Explicitly out of scope

- **Live, mid-run atlas growth** (a new `AtlasOwnerHandle` API to read
  the atlas's current pixels while its background thread keeps
  running, rather than only via the terminal, thread-stopping `join()`)
  -- real, separate future engineering; named here as real, not
  silently dropped, matching this project's own deferral discipline.
- **A persistent, reused worker-thread pool** for Multi-Thread Canvas --
  `std::thread::scope` fresh every frame, matching Step 5.2.3's own
  proven recipe, is what this step builds; pooling is real future
  performance work, not named by IMPLEMENTATION.md's own task list.
- **SVG tessellation/morphing inside the loop** -- no demo has ever
  combined it with multi-threading + atlas + ring-buffer packing
  together, and folding it in now would be new, unproven surface
  beyond what this step's own investigation covers.
- **Windows/macOS** -- this project's standing Linux/Vulkan-first
  deferral, unchanged from every earlier phase.
- **A damped, oscillating spring simulation** -- `spring_decay` (Step
  8.1.1) is deliberately pure exponential decay; this step is its first
  real consumer, not a reason to extend the primitive itself.
