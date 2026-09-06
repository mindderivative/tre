# Demo: Phase 2, Step 2.3 -- Generational Garbage Collection

```bash
./demo/phase2_step2_3/run_gc_demo.sh
```

This step introduces the engine's **first genuine background OS thread**.
Before it, `tre-rhi-vulkan`'s transient render-target pool (Phase 2
Step 2.2) never shrank -- every distinct size ever requested stayed
resident for the process's entire lifetime. This demo proves a real
generational GC now reclaims it.

**What actually happens:** the demo tries to check 25 distinct sizes into
the pool, then runs ordinary `begin_frame`/`submit_and_present` cycles --
no draw calls, no shortened stand-in constants. A background thread wakes
roughly every 100ms and, once the pool is over budget, evicts entries
untouched for the real 600 frames IMPLEMENTATION.md Step 2.3 specifies
(up to 64 per scan -- a throughput cap, not a "stop once under budget"
one, added by the follow-up code review below). The main thread physically
destroys evicted entries in `begin_frame`, but only after a further real
3-frame grace period -- the same call site that already handles Step 2.2's
pool-growth. The demo polls `transient_pool_stats()`'s
`evictions`/`destroyed` counters (not a fixed sleep) until both are
nonzero, then asserts the frame count that took was genuinely past 600.

```
admission cap reached after 22 sizes (~128 MB) -- stopping early, as expected once past the GC trigger threshold
checked 22 distinct sizes into the transient pool (~128 MB total)
frame 1261: evictions=45, destroyed=45
gc_demo: 1261 frames, evictions=45, destroyed=45
gc demo exited cleanly
```

(Real output from a local run -- exact frame count varies by a few hundred
with real OS scheduling jitter; multiple consecutive runs all completed in
under a second.)

**Why the demo stops at 22 of its 25 candidate sizes, and why eviction
counts don't match check-in counts 1:1 -- both real, explained behavior,
not bugs:** a follow-up code review (`documentation/REVIEW.md`'s "Phase 2
Step 2.3 Code Review," finding #80) added a real admission cap --
`acquire_transient_target` now refuses to cold-allocate a genuinely novel
size once the pool's idle free bytes reach the full 128 MB budget, so
callers can't grow the pool without bound. This demo deliberately requests
enough distinct sizes to clear the GC's 85% (~108.8 MB) trigger well before
hitting that cap, and stops gracefully the moment it does. Separately
(finding #77, from the original implementation), Step 2.2's
`acquire_transient_target` queues newly-cold-allocated buckets for a
*second* allocation at the next frame's pool-growth check regardless of
whether anything ever re-requests that size -- this demo's "acquire,
immediately release, never re-request" access pattern leaves that second
allocation equally idle, so the GC thread correctly finds and evicts both
copies of every checked-in size.

**Why introducing a real thread here is safe:** the background thread
never calls a single Vulkan function. It only locks a `Mutex` around plain
Rust bookkeeping (`TransientPool`'s free list, `total_free_bytes`) and
moves values into a queue once they're judged stale. Every actual
`vkDestroy*` call still happens on the main thread, in `begin_frame`, after
the grace period -- the same thread that already does every other piece of
GPU work. Verified by re-running all six pre-existing examples with
`VK_LAYER_KHRONOS_validation` enabled after adding this thread: zero
validation errors, including the checks that would catch unsynchronized
cross-thread Vulkan misuse if this split weren't sound.
