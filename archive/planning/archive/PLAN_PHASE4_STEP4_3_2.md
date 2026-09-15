# Plan: Phase 4, Step 4.3.2 -- AtlasPacker Free-Rectangle Reclamation

## Scope decisions (confirmed with the project owner, 2026-09-06)

**Second of Step 4.3's three sub-steps** (`planning/archive/PLAN_PHASE4_STEP4_3_1.md`
built the first): give `tre_atlas::AtlasPacker` a way to give back a
rectangle it previously handed out, so an evicted glyph's atlas space can
actually be reused by a later insertion instead of being lost forever.
4.3.1 gave `SwmrSlotTable` the ability to forget an entry; this sub-step
gives the packer the matching ability to forget a placement. Step 4.3.3
wires both together into the real 85%-capacity/N-frame eviction policy
DESIGN.md Section 10.2 describes.

**No free-rectangle merging/coalescing -- an extension of Step 4.2.1's own
original scope decision, not a new one.** `AtlasPacker`'s very first
sub-step already accepted fragmentation over a long insertion sequence as
"a deliberate, documented simplification... revisit only if a concrete
fragmentation problem shows up in a later step's real usage." Adding
removal without also adding merging is the direct continuation of that
same choice: `remove` pushes the freed rectangle straight onto
`free_rects`, unmerged, even if it happens to be adjacent to another free
rectangle. Real coalescing (detecting and merging adjacent free
rectangles) is its own well-known, nontrivial packing-algorithm problem,
not a small addition -- and DESIGN.md's own eviction policy is already an
approximate LRU heuristic, not a defragmenter, so a real fragmentation
problem (if one ever shows up in Step 4.3.3's own demo or beyond) is the
right trigger to revisit this, not a reason to build it speculatively now.

**`remove` trusts its caller; no internal "currently placed" registry.**
`AtlasPacker::insert` already trusts its caller not to double-count
overlapping requests -- it has no registry of what it's handed out, only
of what's still free. `remove(rect)` extends the same trust boundary: it
expects `rect` to be a value this exact packer instance previously
returned from a successful `insert` call, and does not (and cannot,
without adding a full placement registry purely to guard against caller
bugs) verify that. This is safe specifically because of
ARCHITECTURE.md's "single atlas owner" design -- exactly one execution
context ever touches one `AtlasPacker`, so there is no concurrent-misuse
surface to defend against, only a single-threaded caller-correctness
contract, the same shape `insert` already has.

**A capacity-fraction query belongs to the packer, not the policy.**
DESIGN.md Section 10.2's "when atlas space capacity exceeds 85%" trigger
needs a way to ask "how full am I," but *deciding* to evict at that
threshold, and *which* entries to evict, is Step 4.3.3's policy, wired
into `AtlasOwner`. This sub-step adds `used_fraction() -> f64` -- a plain
read of the packer's own bookkeeping (total atlas area vs. currently-free
area), with no threshold or eviction logic of its own -- the same
"primitive supplies the fact, policy acts on it" split this whole Step
4.3 breakdown already follows.

## Goal

`AtlasPacker` gains `remove(&mut self, rect: PackedRect)` (returns
previously-placed space to the free list, unmerged) and `used_fraction(&self)
-> f64` (a plain read of how much of the atlas's total area is currently
free vs. placed), proven via unit tests in this module's own existing
style: hand-computed worked examples and overlap-invariant checks across
a real insert/remove/insert sequence, not just isolated logic checks.

## Tasks

1. **`remove(&mut self, rect: PackedRect)`**: pushes `rect` onto
   `free_rects` as-is. No merging with adjacent free rectangles (see
   "Scope decisions" above). Documented caller contract: `rect` must be a
   value this exact packer previously returned from `insert`; passing
   back an arbitrary or already-removed rectangle is a caller bug this
   method has no way to detect, not a condition it reports.

2. **`used_fraction(&self) -> f64`**: `1.0 - (sum of all free_rects'
   areas) / (atlas width * height)`. Requires tracking the atlas's own
   total area at construction (`AtlasPacker::new`'s `width`/`height`
   already fully determine it; store it directly rather than
   recomputing it from the initial single free rectangle, which becomes
   impossible to recover once that rectangle has been split). Returns a
   plain `f64` in `[0.0, 1.0]`; no threshold comparison or eviction
   decision lives here.

3. **Unit tests**, extending this module's own existing worked-example
   style:
   - `remove` then a matching-size `insert` succeeds and reuses the
     freed space where it previously wouldn't have fit -- a fully
     packed atlas, one placement removed, an identically-sized request
     now succeeds, and the full non-overlap invariant still holds
     across everything still placed.
   - An exact-fill-then-free-then-refill worked example (mirroring
     `two_rectangles_can_exactly_tile_a_small_atlas`'s existing style):
     a small atlas tiled exactly by two rectangles, one removed, a third
     request of that exact freed size succeeds, a fourth of any size
     still fails (the atlas is genuinely full again).
   - **No accidental merging**: remove two rectangles that happen to be
     adjacent (share an edge), then confirm a request sized to their
     *combined* area still fails (proving `free_rects` genuinely holds
     them as two separate entries, not silently merged), while a
     request fitting in either piece alone still succeeds -- an
     explicit test documenting the accepted no-merge behavior as a
     property, not an oversight left untested.
   - `used_fraction` returns exact, hand-computed values across a real
     insert/remove/insert sequence (e.g. a 100x100 atlas: `0.0` when
     empty, an exact fraction after a known-size insertion, back to that
     same fraction after removing a different, later insertion of equal
     size).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace -- `tre-atlas` keeps its `#![forbid(unsafe_code)]` posture
  (this sub-step needs no `unsafe`, same as the rest of the crate).
- No new example/demo this sub-step, matching 4.3.1's own precedent --
  `AtlasPacker`'s own capability is proven via unit tests alone; the real
  end-to-end proof (a live eviction happening during a real GPU render)
  is Step 4.3.3's job, once there's an actual policy and a reason to
  build a demo around it.
- `atlas_packing_demo` and `atlas_concurrency_demo` (the two existing
  examples that exercise `AtlasPacker`/`AtlasOwner` directly) re-run for
  real against actual Vulkan hardware as a regression check, even though
  neither yet calls `remove`/`used_fraction` -- matching 4.3.1's own
  practice of actually running the relevant examples rather than only
  asserting they're unaffected.
- CI: no new example to add; push, confirm green.

## Explicitly out of scope for this sub-step

- Free-rectangle merging/coalescing -- deliberately deferred, per "Scope
  decisions" above; revisit only if a concrete fragmentation problem
  shows up in real usage.
- Any change to `AtlasOwner`, `SwmrSlotTable` wiring, or a real eviction
  policy (the 85%-capacity trigger, the N-frame staleness threshold,
  actually calling `remove`/`scan_older_than`/`used_fraction` together)
  -- Step 4.3.3.
- Bumping `AtlasKey`'s packed generation counter on a reused rectangle's
  next insertion -- Step 4.3.3, once there's a real caller that reuses a
  slot for a genuinely different key.
- Any registry of "currently placed" rectangles for `remove` to validate
  against -- see "Scope decisions" above; not needed given the
  single-atlas-owner design, and would cost real bookkeeping to guard
  against a caller bug that can't otherwise occur.
