# Plan: Phase 4, Step 4.3.3 -- Atlas LRU Eviction Policy & Wiring

## Scope decisions (confirmed with the project owner, 2026-09-06)

**Third and closing sub-step of Step 4.3.** 4.3.1 gave `SwmrSlotTable` the
ability to forget an entry and track recency; 4.3.2 gave `AtlasPacker`
the ability to forget a placement and report how full it is. This
sub-step is the actual policy DESIGN.md Section 10.2 describes -- wiring
both into `AtlasOwner` so a full atlas evicts stale entries instead of
dropping new requests forever (the Phase 1-4 review's finding #114,
finally closed) -- plus a real demo proving eviction and re-insertion
both work correctly end to end, the way every other Step 4.2/4.3
primitive has been proven.

**`AtlasInsertRequest` needs a frame number too, not just `lookup` --
a refinement discovered during this planning pass, not anticipated by
4.3.1's own plan.** 4.3.1 assumed only `AtlasOwnerHandle::lookup` would
need a `current_frame: u64` parameter (to stamp recency via
`get_and_touch` on every read). But *deciding whether to evict* has to
happen somewhere, and the only thread that can make that decision is the
atlas owner's own background thread -- which otherwise has no notion of
"what frame is it right now" at all. Threading `current_frame` through
`AtlasInsertRequest` as well gives the owner a frame number at exactly
the moment it's relevant: right before attempting to pack a new request,
which is also the natural moment to ask "am I over capacity, and if so,
should I make room first."

**Eviction is checked proactively, before every insert attempt, gated
only by capacity -- not "only when this particular insert would
otherwise fail."** DESIGN.md's own wording ("when atlas space capacity
exceeds 85%, the engine runs... an LRU garbage collection pass") reads as
a standing capacity check, not a last-resort fallback for a request that
just failed. `process_insert` now calls a new `maybe_evict_stale_entries`
before its existing packing logic; a request that would have fit anyway
still benefits from staying under the high-water mark for whatever comes
next.

**A freshly-inserted entry must not look stale the instant it's
created -- the "cold start" hazard.** `SwmrSlotTable::insert` (4.3.1)
resets a slot's `last_used` to `0` on every fresh claim, whether it's a
genuinely new key or a reused tombstone. Left unaddressed, this means a
glyph inserted this very frame reads as "not rendered within the last N
frames" the instant a *later* insert crosses the 85% threshold, evicting
brand-new content before it's ever had a chance to be used. Fixed by
having the owner call `slots.get_and_touch(key, current_frame)` itself
immediately after a successful `slots.insert` -- stamping "used as of
right now" at creation time, the same way a real LRU cache treats
insertion as an access. No change needed to 4.3.1's already-committed
`SwmrSlotTable` code for this; it's purely how the owner uses the
existing API.

**The packed `generation` counter stays at `0` -- reconsidered from what
REVIEW.md's finding #114 disposition speculated ("Step 4.3.3 is where a
real eviction actually bumps it").** On reflection during this planning
pass: nothing in this codebase reads or depends on `generation` changing
-- no caller of `AtlasOwnerHandle::lookup` or `unpack_slot_value` acts on
it today. Bumping it meaningfully *per key* across an eviction-then-
reinsertion cycle would require a small persistent map from key to its
own insertion count that survives the key's own table entry being
tombstoned and reused -- real bookkeeping with a real cost, built for a
signal nothing currently consumes. Per this project's own "don't build
what the task doesn't need" discipline, this stays deferred: `generation`
remains `0` from every real caller this step too, exactly as 4.2.1
originally scoped it, until an actual consumer of a changing generation
value exists to justify the bookkeeping.

## Goal

A full atlas degrades gracefully instead of dropping requests forever:
once `AtlasPacker::used_fraction()` exceeds 85%, `AtlasOwner` evicts every
entry whose recency (`SwmrSlotTable`'s `last_used`, stamped via
`get_and_touch` on every real read and on insertion itself) is more than
600 frames stale, reclaiming their atlas space via `AtlasPacker::remove`
and their table slots via `SwmrSlotTable::remove`, before attempting to
pack the request that triggered the check. Proven by a real demo
(`atlas_eviction_demo`) that fills a small real atlas past capacity,
confirms stale entries are genuinely evicted (`lookup` returns `None`)
while recently-touched entries survive, and confirms a post-eviction
insertion lands in real, reused atlas space and renders correctly via one
real GPU draw call and pixel readback.

## Tasks

1. **`AtlasKey: From<u64>`** (the reverse of the existing `From<AtlasKey>
   for u64`): reconstructs a key from `SwmrSlotTable::scan_older_than`'s
   raw `u64` output. Safe unconditionally -- `scan_older_than` only ever
   yields a raw key that was a real, successfully-inserted (non-reserved)
   entry.

2. **`AtlasInsertRequest` gains `current_frame: u64`**; `AtlasOwnerHandle::
   request_insert` and `AtlasOwnerHandle::lookup` both take a
   `current_frame: u64` parameter (`lookup`'s call becomes
   `slots.get_and_touch(key, current_frame)` instead of the plain `get`
   used since Step 4.2.4). Every existing call site (both examples,
   `owner.rs`'s own unit tests) updated accordingly -- a real signature
   break, not additive, since there is exactly one real implementation of
   each to update, not a published external API.

3. **`maybe_evict_stale_entries(packer, slots, current_frame)`**, called
   at the top of `process_insert` before its existing packing logic:
   - No-ops immediately if `packer.used_fraction() < 0.85` (the named
     constant DESIGN.md Section 10.2 specifies).
   - Otherwise computes `cutoff = current_frame.saturating_sub(600)`
     (`EVICTION_MIN_IDLE_FRAMES`, DESIGN.md's own "N >= 600 frames"),
     collects every `(raw_key, packed_value)` from
     `slots.scan_older_than(cutoff, ...)` whose recency is below it, then
     for each: unpacks `packed_value` into its `PackedRect`,
     `slots.remove(key)`s the table entry, and `packer.remove(rect)`s the
     atlas space -- table removal and packer reclamation as one paired
     operation per evicted entry, since leaving either one done without
     the other would either leak atlas space or resurrect a key with no
     backing space.
   - Evicts every stale entry found in one pass (matching DESIGN.md's own
     "runs... a garbage collection pass" phrasing), not just enough to
     drop back under 85% -- simpler, and the O(capacity) cost of the scan
     itself already dominates whatever the eviction count turns out to
     be.

4. **`process_insert` stamps recency at creation time**: immediately
   after a successful `slots.insert(key, packed)`, call
   `slots.get_and_touch(key, request.current_frame)` -- closing the
   cold-start hazard described above.

5. **New example** (`crates/tre-rhi-vulkan/examples/atlas_eviction_demo.rs`,
   `demo/phase4_step4_3_3/`): a small, real atlas sized so a modest,
   hand-countable sequence of real MSDF glyph insertions crosses the 85%
   threshold. Inserts an initial set of glyphs at frame 0; explicitly
   "touches" (via `lookup`) a chosen subset at a later frame to keep them
   fresh, leaving the rest untouched; advances the frame counter past 600
   idle frames for the untouched set; requests one more real glyph
   insertion at the new frame, which should trigger eviction of the
   stale set and succeed by reusing their reclaimed space. Verifies: the
   evicted keys' `lookup` now returns `None`; the touched keys' `lookup`
   still resolves to their original, unchanged placement; the new
   glyph's placement is confirmed real via one real GPU draw call and
   pixel readback (the shared `pixel_helpers::bgra_pixel_at` from the
   Phase 1-4 review's own unification, not a fresh hand-rolled closure).
   Exact atlas/glyph sizing and exact frame numbers TBD during
   implementation -- the shape above is fixed, the constants aren't.

6. **Unit tests** (`tre-atlas`'s `owner` module, extending its existing
   style):
   - Eviction does not run at all below the 85% threshold -- inserting a
     few glyphs into a spacious atlas at a far-future frame number
     (well past 600) leaves every earlier entry resolvable, proving
     capacity, not just age, gates eviction.
   - A freshly-inserted entry is not evicted by a same-frame or
     immediately-following insert that happens to cross 85% -- the
     cold-start fix, directly tested rather than only reasoned about.
   - A real eviction: fill a small table/atlas past 85%, advance frames
     past the 600-frame threshold for a subset, insert one more entry --
     confirm the stale subset's keys are gone from both `slots` (`lookup`
     returns `None`) and `packer` (their old rects are reusable again),
     while a touched/fresh entry survives unchanged.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `atlas_eviction_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- reuses the existing, unmodified MSDF rendering pipeline
  (Step 4.2.3's `msdf.frag`), so this confirms no regression, not new RHI
  surface area.
- All 16 pre-existing examples (15 plus `atlas_eviction_demo` itself is
  the 16th -- the other 15 unaffected by this step's changes beyond the
  `lookup`/`request_insert` signature update) re-run for real against
  actual Vulkan hardware.
- CI: add `atlas_eviction_demo` to the `vulkan-validation` job's example
  list; push, confirm green.

## Explicitly out of scope for this sub-step

- Bumping `AtlasKey`'s packed generation counter -- see "Scope decisions"
  above; deferred indefinitely until a real consumer exists, not merely
  to this step.
- Free-rectangle merging/coalescing in `AtlasPacker` -- still deferred
  from 4.2.1/4.3.2; not revisited here even though eviction is exactly
  the kind of workload that could eventually expose fragmentation.
- Any change to `MpscRingBuffer` or the request-queue's own capacity/
  backpressure behavior -- eviction only ever runs synchronously inside
  the owner's existing `process_insert` path, not as a separate queued
  message type.
- Multi-threaded eviction stress testing -- Step 4.2.4 already proved
  the owner's core concurrency primitives under real multi-thread stress;
  this step's own new logic (`maybe_evict_stale_entries`) runs entirely
  on the single owner thread, with no new concurrent-access surface to
  stress beyond what 4.3.1's own `SwmrSlotTable` tests already cover.
