# Plan: Phase 2, Step 2.3 -- Generational Garbage Collection (GC)

## Scope decisions (confirmed with the project owner, 2026-09-06)

**Target resource, decided via question:** IMPLEMENTATION.md Step 2.3's literal
targets -- "atlas regions, tessellated SVG caches" -- don't exist in this
codebase yet (the dynamic texture atlas is Phase 4 work; SVG tessellation is
Phase 3/5). The only real, already-growing dynamic-VRAM resource today is
Phase 2 Step 2.2's transient render-target pool, whose `free` list currently
never shrinks. **This step builds the real generational-GC mechanism now,
verified against the transient pool** -- the atlas and SVG cache plug into
the same mechanism later once they exist, rather than this step being
deferred entirely the way DX12/Metal were.

**Threading, decided via question:** TECHNICAL.md Section 3.3 calls for an
"asynchronous GC thread." Every step so far has deliberately stayed
single-threaded (Step 2.2's synchronous submission; the Phase 2 Code
Review's `Mutex`/`Arc<Mutex<_>>`-wrapped state that was built "ready for" a
future thread but never actually threaded). **This step builds a genuine
background OS thread** -- the first real multi-threading in the engine,
matching the doc's literal wording rather than deferring it further.

**Why this is safe despite being genuinely concurrent:** TECHNICAL.md
Section 3.3's own task split already separates concerns in a way that avoids
the hard problem. Task 2/3 (scan, identify stale entries, move them to a
deferred-release queue) run on the background GC thread -- but that thread
never calls a single Vulkan function. It only locks `TransientPool`'s
`Mutex` and moves plain Rust values (`VulkanTexture`, which is `Send`) into
a queue. Task 4 (the actual `vkDestroy*` calls, gated by "GPU has had 3
frames to finish with it") stays on the main thread, at the same
`begin_frame` call site that already runs `grow_pending_transient_targets`.
The main thread is therefore the only thread that ever touches a raw Vulkan
handle for destruction, fully serialized with everything else it already
does -- there is no new class of Vulkan-object-lifetime hazard here, only a
new `Mutex` contention profile (addressed below).

**Deliberate deviation from "lock-free queue":** the deferred-release queue
is a plain `Mutex<VecDeque<DeferredRelease>>`, not a lock-free structure.
The GC thread pushes at most once per ~100ms scan interval (and only when
actually over budget); the main thread checks it once per frame. At that
call frequency a lock-free queue buys negligible real benefit for real
implementation risk (lock-free queues are exactly the kind of code that is
easy to get subtly wrong), and a `Mutex` also makes "peek the front without
removing it" trivial -- checking whether the oldest queued entry has served
its 3-frame grace period requires exactly that, which `tre_memory::
SpscRingBuffer`'s `pop`-only API doesn't support without a real risk of
losing an entry on a full re-push. This mirrors the project's established
pattern of documenting a deliberate deviation from literal task wording
when the literal wording doesn't serve the actual goal (e.g. Step 2.2's
task 4).

**A new genuine, monotonic frame counter is required.** `FrameSync::
frame_index` is a 0..3 *rotating* counter (which segment of the ring buffer
is current) -- not suitable for "how many frames old is this resource."
`FrameSync` gains a second field, `total_frame_count: AtomicU64`,
incremented once per `submit_and_present`, read by both the GC thread (to
judge staleness) and the main thread (to judge grace-period elapsed).

**VRAM accounting is real, not assumed.** Each pooled `VulkanTexture` gains
a `size_bytes: u64` field (the same `VkMemoryRequirements::size` already
queried at creation). `TransientPool` maintains a running `total_free_bytes`
sum, updated whenever an entry enters or leaves the free list. The 85%
trigger (task 2) compares this sum against 85% of TECHNICAL.md Section 1's
$128\text{ MB}$ dynamic-VRAM-footprint target, not a percentage of the whole
device's VRAM (which would trigger far too rarely on a modern desktop GPU
with gigabytes of VRAM, defeating the point of a budget the engine itself
is supposed to police).

## Goal

The transient render-target pool's `free` list actually shrinks: entries
untouched for 600 frames are evicted by a real background thread once the
pool's total free bytes exceeds 85% of the $128\text{ MB}$ budget, and
those evicted resources are physically destroyed by the main thread only
after a further 3-frame grace period -- proven with a demo that genuinely
runs past 600 frames and past the byte threshold, not a shortened stand-in.

## Tasks

1. **`FrameSync` gains `total_frame_count: AtomicU64`**, incremented in
   `submit_and_present` (alongside the existing `frame_index` rotation).
   Both the GC thread and the main thread read it via the same `Arc<
   FrameSync>` already shared with `VulkanRingBuffer`.

2. **`VulkanTexture` gains `last_used_frame: u64` and `size_bytes: u64`.**
   `VulkanTexture::new` (the transient-target constructor) sets
   `last_used_frame` to the current frame count at creation and
   `size_bytes` from its own `VkMemoryRequirements::size`.
   `VulkanTexture::from_pixels` (bindless textures, never pool-managed)
   sets the same fields for struct-completeness but nothing ever reads
   `last_used_frame` for a bindless texture, since it never enters
   `TransientPool::free`.

3. **`TransientPool` gains `total_free_bytes: u64`.** `release_transient_
   target` (after its existing finding-#70 misuse guard) stamps
   `last_used_frame = current_frame` on the texture being checked in and
   adds its `size_bytes`; `acquire_transient_target`'s hit path subtracts
   the popped texture's `size_bytes` when it leaves the free list.

4. **A `Mutex<VecDeque<DeferredRelease>>` deferred-release queue**
   (`DeferredRelease { texture: VulkanTexture, evicted_at_frame: u64 }`),
   shared via `Arc` between `VulkanDevice` and the GC thread.

5. **The GC thread**, spawned in `VulkanDevice::new` via `std::thread::
   spawn`, holding `Arc` clones of `transient_pool`, `frame_sync`, the
   deferred-release queue, and a `running: Arc<AtomicBool>` shutdown flag:
   every ~100ms, lock `transient_pool`; if `total_free_bytes` is at or
   above 85% of the $128\text{ MB}$ budget, walk every bucket's `Vec<
   VulkanTexture>`, and for each entry older than 600 frames
   (`total_frame_count - last_used_frame > 600`), remove it (`swap_remove`
   -- free-list order is irrelevant), subtract its `size_bytes`, and push
   it onto the deferred-release queue stamped with the current frame.
   Never calls a Vulkan function.

6. **`VulkanDevice::begin_frame` drains the deferred-release queue** (new
   call alongside the existing `grow_pending_transient_targets()`): peek
   the front entry; if `total_frame_count - evicted_at_frame > 3`, pop and
   drop it for real (its own `Drop for VulkanTexture` destroys view/image/
   memory); stop at the first entry that hasn't served its grace period
   yet, since the queue is FIFO-ordered by a monotonically non-decreasing
   eviction frame.

7. **Clean shutdown**: `Drop for VulkanDevice` signals `running = false`
   and `.join()`s the GC thread as its very first action -- before the
   existing pool-clear/`device_wait_idle` steps -- since the GC thread only
   touches `transient_pool`'s `Mutex` and plain data, joining it first
   avoids any race between the GC thread's scan and `Drop`'s own pool
   clear.

8. **`TransientPoolStats` gains `evictions: u64` and `destroyed: u64`**
   (incremented at steps 5 and 6 respectively) so a demo/test can observe
   both phases of the deferred-release design distinctly, not just an
   aggregate.

9. **New example** (`crates/tre-rhi-vulkan/examples/gc_demo.rs`,
   `demo/phase2_step2_3/`): acquires-then-releases ~30 distinct transient
   target sizes (~4MB each, comfortably past the ~$108.8\text{ MB}$ trigger
   threshold) to push the pool over budget, then runs real `begin_frame`/
   `submit_and_present` cycles (no draw calls -- just the frame
   loop itself, to keep 600+ iterations fast) until `total_frame_count`
   has advanced far enough for the GC thread to have evicted the stale
   entries and the main thread to have destroyed them -- polling
   `transient_pool_stats()` for `evictions > 0` then `destroyed > 0` with a
   real timeout, not a fixed sleep. Genuinely exercises all 600 frames and
   the real 3-frame grace period -- no shortened stand-in constant.

10. **Add `tre-memory` as a dependency of `tre-rhi-vulkan`?** No --
    considered and rejected: `SpscRingBuffer`'s `pop`-only API doesn't
    support peeking the front without consuming it (needed for task 6's
    grace-period check), and reaching for it anyway would mean building a
    push-back-on-not-ready workaround that risks losing an entry if the
    queue is momentarily full. The plain `Mutex<VecDeque<_>>` above is the
    correct tool here, not a missed opportunity to reuse Phase 1's ring
    buffer.

## Verification plan

- Local: `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across
  the workspace.
- Local: all five pre-existing Vulkan examples plus `bindless_textures_demo`
  re-run manually with `VK_LAYER_KHRONOS_validation` enabled, zero errors --
  proving the GC thread's existence doesn't introduce a race the validation
  layer (which does catch cross-thread Vulkan misuse, e.g.
  `VUID-vkDestroyDevice-device-05137`-style checks) can detect.
- Local: `gc_demo` genuinely runs past 600 frames and the byte threshold,
  observes `evictions > 0` then `destroyed > 0` via `transient_pool_stats()`
  within a real (generous) timeout, and exits cleanly -- including a clean
  GC-thread shutdown (`Drop for VulkanDevice` joining it without hanging).
- CI: push, `gh run list --branch main --limit 1` / `gh run view` to
  confirm `gc_demo` passes for real on Mesa lavapipe under `xvfb-run` --
  the real test of whether a background thread scanning `Mutex`-guarded
  state behaves correctly on a CI runner's scheduler, not just this
  machine's.

## Explicitly out of scope for this step

- Wiring the atlas or SVG tessellation cache into this mechanism -- neither
  exists yet (Phase 3/4/5 work). This step builds the mechanism and proves
  it against the one real consumer that exists today; a future step
  connects the atlas/cache to it once they're built.
- Any main-thread stall mitigation for the GC scan itself -- the scan holds
  `transient_pool`'s lock for its duration, which could in principle delay
  a same-frame `acquire_transient_target`/`release_transient_target` call.
  Scans are rare (gated by the 85% trigger, at most every ~100ms) and
  CPU-only (no GPU calls), so the realistic delay is microseconds, not
  milliseconds -- accepted as a real but currently negligible tradeoff, not
  measured or mitigated further this step.
- A configurable/shortened eviction-age or grace-period constant for faster
  testing -- `gc_demo` runs the real 600-frame/3-frame values IMPLEMENTATION.md
  and TECHNICAL.md specify, not a test-only stand-in.
