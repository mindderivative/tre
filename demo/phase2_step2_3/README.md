# Demo: Phase 2, Step 2.3 -- Generational Garbage Collection

```bash
./demo/phase2_step2_3/run_gc_demo.sh
```

This step introduces the engine's **first genuine background OS thread**.
Before it, `tre-rhi-vulkan`'s transient render-target pool (Phase 2
Step 2.2) never shrank -- every distinct size ever requested stayed
resident for the process's entire lifetime. This demo proves a real
generational GC now reclaims it.

**What actually happens:** the demo checks 25 distinct sizes into the pool
(~240 MB total, comfortably past the documented 85%-of-128MB trigger
threshold), then runs ordinary `begin_frame`/`submit_and_present` cycles --
no draw calls, no shortened stand-in constants. A background thread wakes
roughly every 100ms and, once the pool is over budget, evicts every entry
untouched for the real 600 frames IMPLEMENTATION.md Step 2.3 specifies. The
main thread physically destroys evicted entries in `begin_frame`, but only
after a further real 3-frame grace period -- the same call site that
already handles Step 2.2's pool-growth. The demo polls
`transient_pool_stats()`'s `evictions`/`destroyed` counters (not a fixed
sleep) until both are nonzero, then asserts the frame count that took was
genuinely past 600.

```
checked 25 distinct sizes into the transient pool (~240 MB total)
frame 1304: evictions=50, destroyed=50
gc_demo: 1304 frames, evictions=50, destroyed=50
gc demo exited cleanly
```

(Real output from a local run -- exact frame count varies by a few hundred
with real OS scheduling jitter; five consecutive runs all completed in
0.2-0.3 seconds.)

**Why 50 evictions from 25 checked-in sizes, not 25 -- a real discovery,
not a bug:** Step 2.2's `acquire_transient_target` queues newly-cold-
allocated buckets for a *second* allocation at the next frame's pool-growth
check, regardless of whether anything ever asks for that size again. This
demo's access pattern (acquire, immediately release, never re-request)
is exactly the pattern that leaves that second allocation equally idle --
so the GC thread correctly finds and evicts both copies of all 25 sizes.
Confirmed real by reading `acquire_transient_target`'s existing code (see
`documentation/REVIEW.md`'s "Phase 2 Step 2.3 Implementation," finding
#77); the GC evicted exactly what was genuinely stale in the pool.

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
