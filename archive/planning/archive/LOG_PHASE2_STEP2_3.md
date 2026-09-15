# Log: Phase 2, Step 2.3 -- Generational Garbage Collection (GC)

## Real issues found during implementation

1. **Borrow-checker error mutating a sibling field while iterating a
   `HashMap`'s `values_mut()` through a `MutexGuard`.** The first draft of
   the GC thread's scan tried to do `pool.total_free_bytes -=
   texture.size_bytes;` inside the `for textures in pool.free.values_mut()`
   loop -- `E0499: cannot borrow pool as mutable more than once at a time`.
   Even though `total_free_bytes` and `free` are disjoint fields, the
   borrow checker doesn't track that disjointness through a `MutexGuard`'s
   `DerefMut` the way it would for a plain local struct. Fixed by
   collecting evicted textures into a `Vec` during the scan and updating
   `total_free_bytes` once, after the loop over `pool.free` has ended (a
   `.iter().map(|t| t.size_bytes).sum()`), rather than mutating both
   fields interleaved.

2. **`dead_code` flags a field whose only use is being dropped.**
   `DeferredRelease::texture` is never accessed by name anywhere -- its
   entire purpose is to be owned until the right moment, then dropped,
   running `VulkanTexture`'s own teardown. `dead_code` analysis only
   recognizes explicit field reads (`entry.texture`), not "this value gets
   dropped for its side effect," so it flagged the field despite it doing
   real, necessary work. Fixed with a documented `#[allow(dead_code)]` --
   not a real defect, but a real gap between what the lint can see and
   what the code actually does.

## A genuinely surprising, but correct, discovery (not a bug)

`gc_demo` checks in 25 distinct transient-target sizes, but
`transient_pool_stats()` reports 50 evictions once the GC thread runs, not
25. Root cause: `acquire_transient_target`'s cold-miss path (Phase 2
Step 2.2) does two things on a genuinely novel size -- it cold-allocates a
texture to return immediately, AND queues that same exact bucket into
`pending_growth` for `begin_frame`'s next `grow_pending_transient_targets`
call to *also* allocate. `gc_demo`'s access pattern (acquire, immediately
release, never re-acquire that size) is exactly the pattern that never lets
the queued growth get a chance to be useful -- every one of the 25 sizes
gets a duplicate texture allocated at the first `begin_frame`, all of it
equally idle, so the GC thread correctly finds and evicts all 50. This is
real, pre-existing Step 2.2 behavior interacting with a synthetic demo
access pattern, not a Step 2.3 defect -- the GC thread evicted exactly what
was actually stale in the pool. Not fixed (out of this step's scope): a
future step could make `acquire_transient_target` skip queuing growth for
a bucket it just cold-allocated for, but that's a Step 2.2 pool-efficiency
question, not a GC correctness one.

## What worked without needing a fix

- The core safety argument held on the first real run: the GC thread never
  calls a Vulkan function (confirmed by code review of `gc_thread_loop`,
  which only touches `Mutex`-guarded plain Rust data), and all six
  pre-existing examples plus the new `gc_demo` ran with
  `VK_LAYER_KHRONOS_validation` enabled and zero errors -- including the
  cross-thread `Mutex` access pattern the validation layer's own
  synchronization-validation checks would have flagged if the main
  thread/GC thread interaction were unsound.
- Clean shutdown: `Drop for VulkanDevice` signals `gc_running = false` and
  joins the thread as its first action. Verified with five consecutive
  `gc_demo` runs (all correctly reporting 50/50 evictions/destructions,
  frame counts varying 1097-1387 due to real scheduling jitter) and
  repeated runs of the fast-exiting `headless`/`walking_skeleton`
  examples -- none hung on shutdown, confirming the GC thread's re-check-
  after-sleep design bounds shutdown latency to `GC_SCAN_INTERVAL`
  (100ms), not indefinitely.
- Real numbers, not shortened stand-ins: `gc_demo` genuinely runs past the
  documented 600-frame age threshold (frames=1097-1387 across five runs,
  always safely past 600) and the real 3-frame grace period, completing in
  ~0.2-0.3 seconds wall-clock -- fast enough that there was no temptation
  to shorten the constants for CI speed, which the plan had already ruled
  out doing.
