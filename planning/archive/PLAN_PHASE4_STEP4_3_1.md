# Plan: Phase 4, Step 4.3.1 -- SwmrSlotTable Tombstone Deletion & Recency Tracking

## Scope decisions (confirmed with the project owner, 2026-09-06)

**Step 4.3 ("Atlas LRU Eviction," closing the Phase 1-4 review's finding
#114) is split into three sub-steps**, the same way Step 3.3 and Step 4.2
were each split into independently-plannable, independently-testable
chunks:

- **4.3.1 (this plan):** give `tre_memory::SwmrSlotTable` the two
  primitive capabilities real eviction needs that it doesn't have today --
  removing an entry without breaking lock-free reads, and tracking which
  entries are actually still being used.
- **4.3.2 (next):** give `tre_atlas::AtlasPacker` a way to give back space
  it previously handed out, so an evicted glyph's rectangle can be reused.
- **4.3.3 (last):** wire 4.3.1 and 4.3.2 together in `AtlasOwner` into the
  actual policy DESIGN.md Section 10.2 describes -- evict entries idle for
  N+ frames once the atlas crosses 85% capacity -- plus a real demo
  proving eviction and re-insertion both work end to end.

This mirrors why 4.2 was split: bin-packing, MSDF generation, the
evaluation shader, and concurrency wiring were each independently provable
before being combined. Here, "can a slot be safely removed without
breaking a lock-free reader" and "can the packer reclaim space" are each
fully testable in isolation, before either has to cooperate with a real
eviction policy.

**Why this sub-step first, and why it's not a small addition.** The
Phase 1-4 review's finding #114 treated LRU eviction as one missing
feature; digging in during this planning pass surfaced that
`SwmrSlotTable`'s own module doc comment states an explicit invariant
this whole design currently rests on: "entries are add-only -- never
removed... which is exactly what makes the 'hit an empty slot during
probing means the key was never inserted' early-exit... sound; a table
that removed entries in place would need tombstones instead." Real
eviction requires exactly that: removing an entry from the table while
`get()` keeps running lock-free, with readers on other threads
potentially probing concurrently. That is a genuine (if well-understood)
concurrent-data-structure change, not a policy tweak on top of the
existing structure -- hence giving it a dedicated sub-step of its own
before touching `tre-atlas`'s eviction policy at all.

**Design: classic tombstone deletion, not a rebuild-the-table approach.**
Considered and rejected: swapping in a whole new `SwmrSlotTable` on
eviction (abandoning all prior entries, forcing every window to
re-request everything) -- far coarser than DESIGN.md's per-entry LRU
policy, and wasteful given most entries survive most eviction passes.
Instead: a second reserved sentinel, `TOMBSTONE_KEY` (distinct from the
existing `EMPTY_KEY`), stored into a removed entry's key slot.
`get()`'s probe now stops (returns `None`) only at a genuine `EMPTY_KEY`
-- a tombstone doesn't prove "never inserted," so probing continues past
it, exactly the case the module's own doc comment already anticipated.
`insert()` remembers the first tombstone-or-empty slot it passes while
probing and reuses it if the key isn't found further along -- standard,
textbook linear-probing-with-tombstones, not a novel scheme. Both
`EMPTY_KEY` and the new `TOMBSTONE_KEY` get rejected as real keys the same
way the Phase 1-4 review's finding #99 fix already rejects `EMPTY_KEY`.

**Recency tracking is a new, separate concern from the table's existing
key/value payload, not folded into it.** `SwmrSlotTable`'s existing
`get()` is called by any number of concurrent reader threads with no
writes at all -- that's the whole point of the lock-free design. Marking
an entry "still in use this frame" is inherently a write (bumping a
timestamp) that needs to happen from every one of those same reader
threads, which is a different concurrency shape than the existing
single-writer/multi-reader model for keys and values. Handled as its own
parallel `Box<[AtomicU64]>`, one slot per table slot, each entry updated
via `AtomicU64::fetch_max` (stable, race-safe for "record the highest
frame number any reader has reported" with no ordering coordination
needed beyond `Relaxed` -- multiple concurrent writers racing to record a
recency stamp is harmless by construction, unlike the key/value payload
itself). A new `get_and_touch(key, current_frame) -> Option<value>`
method does both the lookup and the recency stamp in one call; the
existing plain `get()` is untouched, for callers (if any ever exist) that
want a lookup with no recency side effect.

**`tre-atlas` supplies the frame number; `tre-memory` and `tre-atlas`
still know nothing about frames, windows, or the RHI.** The natural
"current frame" already exists as `FrameSync::total_frame_count` in
`tre-rhi-vulkan` (Phase 2 Step 2.3's GC uses the exact same counter for
the exact same "how stale is this" purpose) -- but `tre-atlas` has no
dependency on `tre-rhi-vulkan` and must keep it that way (ARCHITECTURE.md
Section 2.3, and this crate's own established "content-and-backend-
agnostic" precedent from Step 4.2.1 onward). So the frame number is a
plain `u64` parameter threaded in by whichever RHI-side code calls
`AtlasOwnerHandle::lookup` -- a signature change to that method, landing
in Step 4.3.3 once there's an actual policy consuming it. This sub-step
only needs `SwmrSlotTable` itself to accept an opaque `u64` timestamp; it
never needs to know where the number comes from.

## Goal

`SwmrSlotTable<K>` gains `remove(key) -> bool` (tombstone-based, safe
against concurrent `get()`/`get_and_touch()` calls from other threads)
and `get_and_touch(key, frame) -> Option<value>` (records recency without
ever blocking a reader or requiring the single writer to coordinate with
readers), plus a writer-only scan capability so a future eviction policy
can enumerate "every currently-occupied slot and how recently it was
touched" without needing per-key polling. Proven via unit tests
mirroring this module's own existing concurrency-safety tests (a
concurrent-readers-during-a-remove stress test, not just single-threaded
logic checks), not by inventing a new testing style for this module.

## Tasks

1. **`TOMBSTONE_KEY` sentinel**, alongside the existing `EMPTY_KEY`, both
   rejected as real keys by `insert`/`get`/`get_and_touch`/`remove` (the
   same real, non-debug-only rejection the Phase 1-4 review's finding #99
   fix already applies to `EMPTY_KEY` alone, generalized to both reserved
   values).

2. **`remove(&self, key: K) -> bool`**: probes exactly like `get`, and on
   finding the key, stores `TOMBSTONE_KEY` into that slot's key
   (`Ordering::Release`, matching the existing "publish" discipline for
   key stores). Returns `false` if the key isn't present. Documented as
   single-writer-only, same as `insert` -- concurrent `remove`/`insert`
   calls from multiple threads are not supported, only `insert`/`remove`
   vs. `get`/`get_and_touch` concurrently.

3. **Rewrite `insert`'s and `get`'s probe loops** to the standard
   tombstone-aware algorithm: `get`/`get_and_touch` stop (return `None`)
   only at a genuine `EMPTY_KEY`, treating `TOMBSTONE_KEY` as "keep
   probing"; `insert` remembers the first `TOMBSTONE_KEY`-or-`EMPTY_KEY`
   slot seen and reuses it once it's confirmed the key isn't already
   present further along the same probe sequence.

4. **`get_and_touch(&self, key: K, frame: u64) -> Option<u64>`**: identical
   lookup to `get`, plus `self.last_used[index].fetch_max(frame,
   Ordering::Relaxed)` on a hit only (a miss touches nothing). New
   `last_used: Box<[AtomicU64]>` field, capacity-sized, zero-initialized
   alongside `keys`/`values` in `with_capacity`.

5. **A writer-only scan for eviction candidates**: something in the shape
   of `fn scan_older_than(&self, cutoff_frame: u64, visit: impl FnMut(u64,
   u64))` -- iterates every occupied (non-`EMPTY_KEY`, non-`TOMBSTONE_KEY`)
   slot whose `last_used` is below `cutoff_frame`, calling `visit(raw_key,
   value)` for each. Raw `u64` rather than `K` (avoids adding a
   `u64`-back-to-`K` trait bound this module doesn't otherwise need); the
   caller reconstructs its own key type. Exact name/shape TBD during
   implementation. Documented as writer-thread-only, matching `insert`/
   `remove`.

6. **Unit tests**, extending this module's own existing test style
   (already includes a genuine multi-threaded stress test, not just
   single-threaded assertions):
   - `remove` then `get` returns `None`; the table still round-trips
     other, untouched keys correctly.
   - Re-inserting the same key after `remove` succeeds and reuses the
     same physical slot (observable indirectly: capacity doesn't leak
     across repeated insert/remove/insert cycles for one key -- a table
     sized for exactly N live keys must still accept the (N+1)th
     insert-after-remove-of-one-of-the-first-N).
   - A different key whose probe sequence passes through a tombstoned
     slot still resolves correctly (both for the tombstoned key, now
     absent, and the other key, still present).
   - `get_and_touch` records a strictly-increasing `last_used` across
     repeated calls with increasing frame numbers, and a call with a
     *lower* frame number than already recorded does not regress it
     (that's what makes `fetch_max` the right primitive, not a plain
     store).
   - A concurrent stress test: one thread calls `remove` on a live key
     while several reader threads concurrently call `get_and_touch` on a
     mix of keys (including the one being removed) -- no reader ever
     observes a torn/corrupted value, matching the existing
     `concurrent_readers_see_a_fully_published_value_never_a_default`
     test's own style.
   - `scan_older_than` finds exactly the entries expected in a small,
     hand-built table with a mix of fresh and stale `last_used` values,
     and correctly excludes tombstoned/never-inserted slots.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace -- `tre-memory` keeps its zero-`unsafe` posture for this
  module (the tombstone/recency logic needs no raw pointers, same as the
  module's existing code).
- No example/demo this sub-step -- `SwmrSlotTable` has no direct RHI or
  visual surface of its own; Step 4.2.4's own precedent already proved
  this module purely via unit tests, and this sub-step's new
  capabilities aren't wired into `tre-atlas`'s real atlas owner until
  4.3.2/4.3.3. All 15 pre-existing examples re-run manually as a
  regression check even though this sub-step touches no RHI code.
- CI: no new example to add this sub-step (nothing to add to the
  `vulkan-validation` job); the existing `test` job already runs
  `tre-memory`'s unit tests. Push, confirm green.

## Explicitly out of scope for this sub-step

- `AtlasPacker` reclaiming a rectangle back to its free list -- Step
  4.3.2.
- Any real eviction *policy* (the 85%-capacity trigger, the N-frame
  staleness threshold, actually calling `remove`/`scan_older_than` from
  `AtlasOwner`) -- Step 4.3.3. This sub-step only builds the primitives a
  policy will need.
- Changing `AtlasOwnerHandle::lookup`'s signature to accept a frame
  number -- Step 4.3.3, once there's a real caller for it.
- Any change to `AtlasKey`'s packed `(rect, generation)` encoding --
  already covers a generation counter (`pack_slot_value`/
  `unpack_slot_value`, widened in the Phase 1-4 review's finding #105
  fix) for exactly this eventual purpose; Step 4.3.3 is where a real
  eviction actually bumps it.
- `MpscRingBuffer` -- untouched; eviction is entirely something the
  single atlas-owner thread decides and executes on its own, with no new
  cross-thread request type.
