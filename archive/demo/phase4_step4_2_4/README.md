# Demo: Phase 4, Step 4.2.4 -- Multi-Window Atlas Concurrency

```bash
./demo/phase4_step4_2_4/run_atlas_concurrency_demo.sh
```

**The capstone of the whole Step 4.2 arc.** Every piece built across
4.2.1-4.2.3 (the Guillotine packer, real MSDF generation, the evaluation
shader) is exercised together here, for the first time under genuine
concurrent stress: 3 real producer threads request MSDF atlas space for
the 6 letters of `"GLYPHS"` concurrently, through a real bounded MPSC
ring buffer; a single, real, dedicated background `AtlasOwner` thread
(the same precedent Phase 2 Step 2.3's generational GC thread already
established) drains requests, performs real packing and generation for
each, and publishes results into a real single-writer/multi-reader
publish table -- all before the finished shared atlas is uploaded as one
real GPU texture and every letter rendered in a single draw call.

**Two new generic concurrency primitives, both in `tre-memory`.**
`tre_memory::MpscRingBuffer<T>` (Dmitry Vyukov's well-known bounded MPMC
ring buffer design, simplified for a single consumer) and
`tre_memory::SwmrSlotTable<K>` (a fixed-capacity, open-addressed publish
table, needing zero `unsafe`) -- both living alongside the pre-existing
`SpscRingBuffer` in the one crate TECHNICAL.md's `unsafe` policy already
groups them under, not duplicated into `tre-atlas`.

**`tre-atlas` still never depends on `tre-text`.** `AtlasInsertRequest`
carries a boxed `RasterSource` trait object (`size()` + `rasterize()`);
this demo's own `GlyphRasterSource` is the glue calling
`tre_text::generate_msdf` underneath, kept entirely on the demo side so
the atlas crate stays exactly as content-agnostic as Step 4.2.1 first
established (shared by glyphs *and*, eventually, icons).

**Verified by more than "a slot exists."** After every producer thread
rejoins, this demo confirms every returned placement is non-overlapping
*and* byte-identical, row for row, to an independently regenerated MSDF
for that exact glyph -- proving the owner thread's copy into the shared
CPU buffer is correct, not just that some bytes landed somewhere. Only
then is the buffer uploaded to the GPU and every letter's own on-screen
quad checked for real, non-background pixels.

**Two real, if minor, findings along the way** (documented in
`documentation/REVIEW.md`'s "Phase 4 Step 4.2.4 Implementation"
section): this demo's first draft polled for results using
`thread::yield_now()` in a tight loop, which is only ever a scheduler
*hint* -- under this environment's scheduler it let the polling loop burn
through its whole retry budget without the OS ever actually switching to
the atlas owner thread. Switched to a real `sleep`-based backoff, which
genuinely waits. Separately, the demo's first attempt at verifying each
letter actually rendered checked only its bounding-box *center* pixel --
the same "an open-counter glyph like 'G' can have background sitting
exactly at its own bbox center" gotcha 'L' already taught this project in
Step 4.1, re-learned here and fixed by scanning each letter's whole
on-screen quad for *any* non-background pixel instead.
