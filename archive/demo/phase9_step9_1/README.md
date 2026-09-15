# Demo: Phase 9, Step 9.1 -- Correctness Test Suite

```bash
./demo/phase9_step9_1/run_batching_equivalence_demo.sh
```

**Step 9.1's real payoff, on real GPU hardware.** Most of this step's
work is unit and property tests (`tre-engine`, `tre-atlas`, `tre-svg`)
with no visual output of their own -- see `documentation/IMPLEMENTATION.md`
Step 9.1 for the full account. This demo is the one part of the step
that renders real pixels: it records the identical scene (four
non-overlapping SDF rects sharing the same Layer/Pipeline/Texture/clip
state) twice, once through `RenderingCanvas::flatten()` (the real,
production batched path -- merges into one draw call) and once through
the new `RenderingCanvas::flatten_unbatched()` (the same real radix
sort, merge step skipped -- four draw calls), renders both through the
real, unmodified `execute_frame`, and asserts the two framebuffers are
byte-for-byte identical.

**Why this test exists.** The existing performance suite can catch a
batching pass that is *slow*; it cannot catch one that is *fast but
wrong* -- silently dropping or misordering a draw command would still
pass a performance check while producing a visibly broken frame.
Pixel-diff equivalence directly validates the one invariant the entire
batching architecture depends on: batching is a pure performance
optimization, never a visual one.

**Two real discrepancies found during this step's own pre-work
investigation** (documented in full in `documentation/REVIEW.md`
findings #154-155):

1. TECHNICAL.md Section 4 / ARCHITECTURE.md Section 4.1 documented a
   4-pass radix sort for the 64-bit draw-command sort key; the real
   `flatten_run` had always used `std::sort_unstable_by_key` (a
   comparison sort) instead, unchanged since Step 5.1.3. Resolved by
   building the real thing: `radix_sort_by_key` (`tre-engine`), a
   genuine 4-pass, 16-bit-digit LSD radix sort with no per-call heap
   allocation, now the real sort underneath every `flatten()` call --
   including this demo's own batched render.

2. DESIGN.md Section 2.6 documented an atlas-exhaustion placeholder-
   glyph fallback that was never built; the real `AtlasOwner::process_
   insert` silently drops the request instead. Resolved by correcting
   the documentation to match reality and adding a dedicated regression
   test proving the drop path never wedges the atlas or corrupts a
   subsequent normal insert.

**Verified.** Ran 3 times, byte-for-byte identical across the whole
framebuffer every time. Added to `ci.yml`'s `vulkan-validation` job.
A full manual regression sweep of all 31 pre-existing Vulkan demos was
also run after the sort-algorithm swap underneath `flatten()`/
`execute_frame`'s entire draw pipeline -- 30 passed; `canvas_
accessibility_verify` failed only on the same pre-existing,
already-documented environmental limitation as REVIEW.md finding #126
(no AT-SPI registry daemon in this sandbox), unrelated to this step.
