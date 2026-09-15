# Demo: Phase 4, Step 4.3.3 -- Atlas LRU Eviction Policy & Wiring

```bash
./demo/phase4_step4_3_3/run_atlas_eviction_demo.sh
```

**The capstone of the whole Step 4.3 arc**, closing the Phase 1-4
review's finding #114 (atlas LRU eviction was named in Step 4.2's own
original task list but never built). A real 64x64 atlas exactly holds
four 32x32 MSDF glyphs (`'G'`, `'L'`, `'Y'`, `'P'`) with zero leftover
space -- filling it completely reaches exactly 100% capacity, past
DESIGN.md Section 10.2's 85% eviction trigger.

`'G'` is explicitly kept fresh via a real `lookup` at frame 700; `'L'`,
`'Y'`, and `'P'` are left untouched after their own frame-0 insertion.
Requesting a fifth glyph (`'H'`) at frame 700 -- 700 frames past their
last real use, well beyond Section 10.2's own "N >= 600 frames" idle
threshold -- forces a real eviction pass inside `AtlasOwner`: the three
stale glyphs are reclaimed (both their `SwmrSlotTable` entry and their
`AtlasPacker` space), `'G'` survives with its placement unchanged, and
`'H'` lands in real, reused atlas space.

This is verified two ways: first, directly via `lookup`'s own before/
after results (the three stale keys resolve to `None`; `'G'` still
resolves to its original placement; `'H'` resolves to a real, non-
overlapping placement) -- then by uploading the finished atlas as one
real GPU texture and rendering `'G'` and `'H'` through the existing,
unmodified `msdf.frag` pipeline (Step 4.2.3), reading back real pixels
to confirm both glyphs actually drew.

**The cold-start fix.** `SwmrSlotTable::insert` (Step 4.3.1) resets a
freshly-claimed slot's recency to `0` -- which would otherwise make a
glyph inserted this very frame look "not rendered within the last 600
frames" the instant a *later* insert crosses the 85% threshold, evicting
brand-new content before it's ever used. `AtlasOwner`'s `process_insert`
now stamps a fresh entry's recency to its own creation frame immediately
after insertion, treating creation as an access the same way a real LRU
cache does -- without this, `'G'` (inserted at frame 0, same as the
others) would have been wrongly evicted alongside `'L'`/`'Y'`/`'P'` the
moment this demo's own eviction check ran, unless it had happened to be
touched again in between (which, in this demo, it deliberately is, to
also prove the "recently touched" half of the LRU story works).
