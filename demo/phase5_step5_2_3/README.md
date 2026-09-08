# Demo: Phase 5, Step 5.2.3 -- The Capstone: Real Concurrent Recording, Rendered

```bash
./demo/phase5_step5_2_3/run_canvas_sub_canvas_demo.sh
```

Closes Step 5.2 in full. Every mechanism this demo exercises already
exists and is already unit-tested (`SubCanvas`/`create_sub_canvas`,
Step 5.2.1; `tre_memory::ScatterArena`/`FrameArena`/`stitch_into`, Step
5.2.2) -- this is the real-stress, real-GPU proof, the same role
`atlas_concurrency_demo` and `atlas_eviction_demo` already played for
their own features.

`min(available_parallelism() - 1, 4)` real OS worker threads each get
their own `SubCanvas`. Every one draws its own rect; thread `0` draws
inside a `begin_overlay`/`end_overlay` bracket; the last thread also
draws a real atlas-backed MSDF glyph. Every thread calls `stitch_into`
on a shared `FrameArena` as its own last action before it exits -- the
actual "workers stitch themselves" design point Step 5.2.2 was built
around, exercised here for real instead of only in a unit test. The
root canvas draws one more rect directly and stitches into the same
arena.

**A real, honest property this demo surfaces that no single-threaded
demo could.** `canvas_batch_flattening_demo` (Step 5.1.3) always
collapses to exactly 3 batches. Here, the overlay worker's
`begin_overlay`/`end_overlay` markers are hard run-segmentation
barriers, and *which* other threads' plain rects land before vs. after
that marker pair in the concurrently-stitched command array depends on
real thread scheduling -- ARCHITECTURE.md Section 4.2's own "soft
target, not a guarantee" caveat, actually exercised rather than just
cited. Run the demo a few times and watch the batch count for the
plain rects vary between 1 and `worker_count`. What never varies: every
rect's content is accounted for somewhere, and the overlay/text content
never merges into the wrong plane or pipeline.
