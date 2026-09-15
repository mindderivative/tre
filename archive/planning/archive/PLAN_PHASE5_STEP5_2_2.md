# Plan: Phase 5, Step 5.2.2 -- Real Lock-Free Stitching

## Scope decisions

**Second of Step 5.2's three sub-steps.** 5.2.1 built `SubCanvas` and
proved concurrent thread-local recording is safe (a real 4-thread
stress test, zero Depth ID collisions) but deliberately added no way
to get a sub-canvas's recorded data back out. This sub-step is that
missing half: a real lock-free primitive that lets any thread --
worker or coordinator -- merge its own local `vertices`/`indices`/
`commands` into one shared destination without a lock, plus wiring
that destination into a real `FlattenedFrame`.

**Workers do their own copy -- this is the one design choice that
actually justifies calling it "lock-free," not just "safe."**
IMPLEMENTATION.md's own task list is specific: "Worker threads
bulk-copy... to the global arena at their acquired offset" -- not "the
coordinator copies on their behalf." If only the coordinator ever
performed the merge (sequentially, after joining every thread), a
plain `usize` running total would do the identical job with zero
atomics and no `unsafe` anywhere -- correct, but it wouldn't be the
primitive DESIGN.md/TECHNICAL.md Section 8 actually describe, and it
would defeat the entire "stitching time near-zero, overlapping with
still-running recording" value proposition Section 6.3 states as the
reason this exists. This sub-step builds the real thing: a new
`tre_memory::ScatterArena<T>` whose `reserve(&self, count)` is safe to
call concurrently from any number of threads, each getting back a
disjoint, exclusively-owned `&mut [T]` slice to write into -- so a
`SubCanvas`, as its very last action before its worker thread exits,
can stitch its own data into the shared destination itself.

**`ScatterArena<T>` follows `MpscRingBuffer`'s own established
unsafe pattern exactly, not a new one.** `tre-memory` already solves
"hand a `T` from one thread to another without a lock" via per-slot
`UnsafeCell<MaybeUninit<T>>` plus an explicit, justified `unsafe impl
Send + Sync`. `ScatterArena<T>` is the same technique applied to
*ranges* instead of single slots: a fixed-capacity `Box<[UnsafeCell
<MaybeUninit<T>>]>`, one atomic `AtomicUsize` bump counter
(`fetch_add(count)` reserves `[start, start+count)`), and no ring
wraparound or per-slot sequence number at all -- unlike
`MpscRingBuffer`, nothing here is ever popped or reused within a
frame, so the mechanics are strictly simpler: every reservation is
permanently exclusive until the whole arena is consumed once, at
frame's end, via `into_vec(self)`. `T: Copy` (every real payload --
`UiVertex`, `u32`, `UiDrawCommand` -- already is), no `Default` bound
needed (unlike a defaults-prefilled design, matching `MpscRingBuffer`'s
own choice of `MaybeUninit` over pre-filling).

**Bulk-copying "with `copy_from_slice`" is imprecise -- the real
operation is copy-and-rebase, not a byte-for-byte memcpy.** A raw
index value is an absolute offset into `vertices`; a command's
`vertex_offset` is an absolute offset into `indices`. Concatenating
several sources' arrays means every downstream source's own index
values and `vertex_offset`s must be shifted by however much space the
*previous* sources actually claimed -- discovered only once that
source's own `reserve` call returns its actual `start` position, which
depends on however many other threads' reservations landed first.
`stitch_into` therefore performs its three reservations in a fixed
order (vertices, then indices rebased by the vertices reservation's
own `start`, then commands rebased by the indices reservation's own
`start`) -- vertices alone are a literal bulk copy; indices and
commands are not.

**Arena overflow drops that source's remaining data and reports
incomplete -- it does not grow, and does not yet implement DESIGN.md
Section 2.6's full "drop lowest-priority content first, then report a
diagnostic" policy.** `FrameArena`'s three `ScatterArena`s are sized
once, by the caller, at construction (`with_capacity`) -- matching the
zero-dynamic-allocation-during-a-frame philosophy this project has
followed since Phase 0. `reserve` returning `None` in the real,
per-window `RhiDevice`/pool code already has an established, named
precedent for exactly this situation (DESIGN.md Section 2.6: "the
engine drops the lowest-priority pending draw commands... and reports
a frame-budget diagnostic, rather than growing memory dynamically
mid-frame"), but implementing the actual *priority ordering* that
policy needs (overlays first, then standard content by depth) is a
separate, larger feature. This sub-step's `stitch_into` returns `bool`
(fully stitched or not) so a caller can observe the failure, without
yet building the prioritized-degradation policy itself.

**The single-threaded `flatten()` sort/merge logic gets reused, not
duplicated.** `flatten_run`'s per-run sort-then-merge algorithm
(Step 5.1.3) doesn't care how its input `commands`/`source_indices`
were assembled -- it only needs one combined command list and one
combined index buffer. `FrameArena::flatten(self)` calls
`ScatterArena::into_vec()` on all three arenas and then drives the
*exact same* marker-segmentation-and-`flatten_run` loop
`RenderingCanvas::flatten` already has, refactored into one shared
free function both call -- an internal reorganization, zero behavior
change for any existing single-canvas caller.

**Extracting a `SubCanvas`'s inner data needs `mem::take`, not a
destructuring move.** 5.2.1 left this as an open question: `SubCanvas`
has a manual `Drop` impl (releasing its slot in `live_sub_canvases`),
and Rust forbids partially moving fields out of any type that
implements `Drop`. `SubCanvas::stitch_into` resolves it with
`std::mem::take(&mut self.canvas)` (swapping in a throwaway, about to
be dropped, placeholder `RenderingCanvas` -- its content is never used
again) to extract the real data *before* `self` finishes going out of
scope, letting `SubCanvas`'s own `Drop` still fire normally afterward
and correctly release the slot.

## Goal

`tre_memory::ScatterArena<T>` lets any number of threads concurrently
reserve disjoint output ranges via one atomic `fetch_add` and write
into them without a lock; `tre_engine::FrameArena` bundles three of
them (vertices/indices/commands); `RenderingCanvas::stitch_into`/
`SubCanvas::stitch_into` merge one source's locally-recorded data into
a `FrameArena` with correct index/`vertex_offset` rebasing;
`FrameArena::flatten(self)` produces a real `FlattenedFrame` by
running the existing Step 5.1.3 sort/merge logic over the combined
result -- proven by a real multi-thread test in which several threads
concurrently stitch distinct content into one arena with zero data
corruption, plus deterministic single-threaded tests proving rebasing
correctness and arena-overflow handling.

## Tasks

1. **`tre_memory::ScatterArena<T: Copy>`** (new file
   `crates/tre-memory/src/scatter.rs`): `with_capacity(capacity: usize)
   -> Self`; `reserve(&self, count: usize) -> Option<ScatterSlice<'_,
   T>>` (atomic `fetch_add`-based bump reservation, `None` on would-be
   overflow); `ScatterSlice` exposes `start_index()` and
   `Deref`/`DerefMut` to `&mut [T]`; `into_vec(self) -> Vec<T>`
   (consumes the arena, single-threaded, once every writer is done --
   `assume_init_read`s exactly the written prefix). `unsafe impl<T:
   Send> Send + Sync for ScatterArena<T>`, with a `// SAFETY:` comment
   matching `MpscRingBuffer`'s own justification style: disjoint
   `fetch_add`-reserved ranges are never handed out twice, and no
   range is ever read until `into_vec` is called after every writer
   has finished.

2. **Real concurrent unit tests for `ScatterArena`** in `tre-memory`:
   several real threads each reserve and fully write a distinct chunk
   concurrently; `into_vec()`'s result contains every written value
   exactly once, in a valid (order-doesn't-matter-across-threads)
   arrangement; `reserve` returns `None` once capacity is exhausted;
   `into_vec()` correctly truncates to the actually-reserved prefix
   when capacity was never fully used.

3. **`tre_engine::FrameArena`**: `with_capacity(vertex_capacity,
   index_capacity, command_capacity) -> Self`, wrapping `Arc<
   tre_memory::ScatterArena<_>>` for each of the three arrays.

4. **`RenderingCanvas::stitch_into(self, arena: &FrameArena) -> bool`**:
   reserves+writes vertices (a literal bulk copy), then indices
   (rebased by the vertices reservation's `start_index()`), then
   commands (rebased by the indices reservation's `start_index()`);
   returns `false` if any of the three reservations failed (arena
   full), documenting that a partial-failure stitch leaves that
   source's contribution incomplete rather than atomic/all-or-nothing.

5. **`SubCanvas::stitch_into(mut self, arena: &FrameArena) -> bool`**:
   `std::mem::take`s the inner `RenderingCanvas` out, delegates to its
   `stitch_into`, and lets `self`'s own `Drop` release the
   `live_sub_canvases` slot normally at the end of the call.

6. **Refactor `RenderingCanvas::flatten`'s segmentation-and-merge loop**
   into a shared free function taking plain `vertices`/`indices`/
   `commands` (not `self`), called both by `RenderingCanvas::flatten`
   (unchanged behavior, existing tests must still pass verbatim) and
   the new `FrameArena::flatten(self) -> FlattenedFrame` (calling
   `into_vec()` on all three `ScatterArena`s first).

7. **Unit tests in `tre-engine`**: a single source's `stitch_into`
   correctly rebases indices/`vertex_offset` into an otherwise-empty
   arena (hand-computed expected values, matching this crate's own
   style); two (or three) *sequential* (single-threaded, for
   determinism) sources stitched into one arena produce a
   `FlattenedFrame` whose vertices/commands correctly reflect every
   source's own content at its own correct position, including a
   cross-source merge case (two sources' commands sharing Layer+
   Pipeline+Texture+`clip_bounds` still merge into one batch, proving
   `FrameArena::flatten` reuses the real Step 5.1.3 merge logic, not a
   naive concatenation); an undersized `FrameArena` causes
   `stitch_into` to report `false`; **one real multi-thread test**:
   several real `SubCanvas`es (via 5.2.1's own `create_sub_canvas`)
   each drawing distinct content on their own `std::thread::spawn`
   thread, each calling `stitch_into` itself as its last action before
   the thread exits, joined and flattened on the coordinator -- the
   same "real threads, not a simulation" rigor 5.2.1's own stress test
   and `MpscRingBuffer`'s already established.

8. **Docs**: IMPLEMENTATION.md Step 5.2.2 subsection; REVIEW.md entry;
   TECHNICAL.md Section 8/ARCHITECTURE.md Section 2.2 only if a new
   convention surfaces beyond what's already planned here (the
   `ScatterArena` name itself is new -- the docs never name a specific
   type for "the global arena" -- worth a brief cross-reference if it
   reads as load-bearing enough to record there).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- No demo this sub-step, matching 5.2.1's own precedent -- the real
  end-to-end GPU proof (real worker threads, real merged render, real
  pixel readback) is Step 5.2.3's capstone job.
- All existing `tre-engine` tests (41, from 5.1.1-5.2.1) re-run
  unchanged after the `flatten` refactor, confirming zero behavior
  change for the single-canvas path.
- All pre-existing examples re-run manually as a regression check.

## Explicitly out of scope for this sub-step

- The real GPU demo -- Step 5.2.3.
- DESIGN.md Section 2.6's full prioritized-degradation policy for
  arena overflow (dropping overlays before standard content, by
  depth) -- `stitch_into` reports failure; deciding *what to do* about
  it is deferred.
- Any automatic sizing/estimation of `FrameArena`'s capacities -- the
  caller supplies them explicitly, the same way every other
  pre-allocated pool in this codebase (the transient render-target
  pool, the MPSC ring buffers) is caller-sized, not auto-tuned.
- Transactional (all-or-nothing) reservation across a single source's
  three arrays -- a partial stitch failure leaves that source's
  contribution incomplete, a disclosed, accepted limitation.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code.
