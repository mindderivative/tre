# Plan: Phase 5, Step 5.2.1 -- SubCanvas & Thread-Local Recording

## Scope decisions

**Step 5.2 ("Multi-Threading & Lock-Free Sub-Canvases") is split into
three sub-steps**, the same way Steps 3.3/4.2/4.3/5.1 were each split
into independently-plannable, independently-testable chunks:

- **5.2.1 (this plan):** `Canvas::create_sub_canvas()` and thread-local
  recording -- a worker thread gets its own independent `SubCanvas`
  sharing exactly one thing with the root canvas and every sibling
  sub-canvas: the global Depth ID counter that keeps every command's
  sort key unique across the whole frame regardless of which thread
  recorded it.
- **5.2.2 (next):** the real lock-free stitching primitive
  (`tre-memory` gains a new "reserve a disjoint output range via one
  atomic `fetch_add`, write into it from any thread, no lock" type,
  ARCHITECTURE.md Section 2.2/TECHNICAL.md Section 8's own literal
  design) and wiring it into a real multi-source `flatten`.
- **5.2.3 (last):** the capstone -- real OS worker threads recording a
  nontrivial scene concurrently, merged via 5.2.2's primitive, rendered
  and read back as real GPU pixels, the same "prove it under genuine
  concurrent stress" rigor `atlas_concurrency_demo` already established
  for Step 4.2.4.

**Why this split, and why thread-local recording goes first.**
Nothing about the stitching primitive (5.2.2) is meaningfully testable
without real `SubCanvas` output to stitch, and the capstone (5.2.3)
needs both built -- the same dependency chain that ordered 5.1's own
three sub-steps.

**`SubCanvas` reuses `RenderingCanvas` directly instead of duplicating
its drawing API.** DESIGN.md Section 6.3/TECHNICAL.md Section 8 both
describe a `SubCanvas` recording the exact same primitives
(`draw_rect`, `draw_text`, `save`/`restore`, clip, overlay) a normal
`Canvas` does -- "thread-local linear arenas" holding the identical
`UiDrawCommand`/`UiVertex`/`u32` data this crate already has. Rather
than inventing a second, parallel type with its own copy of every
drawing method, `SubCanvas` is a thin wrapper around a private
`RenderingCanvas` (`Deref`/`DerefMut` to it), so every existing method
-- `save`, `push_clip`, `begin_overlay`, `draw_rounded_rect`,
`draw_text` -- works on a `SubCanvas` for free, unchanged.

**Depth ID must become genuinely shared across threads; Layer ID does
not.** Step 5.1.3 made Depth ID a single, monotonically increasing
`u32` counter specifically *because* no two `DrawGeometry` commands in
one frame may ever share a full sort key -- `flatten_run`'s
`sort_unstable_by_key` safety and its adjacent-merge logic both depend
on that uniqueness. A plain per-canvas counter starting at `0` in every
worker thread would immediately violate it: two sub-canvases' first
commands would both claim Depth ID `0` and collide once merged. Depth
ID is therefore promoted from a private `u32` field to a shared
`Arc<AtomicU32>`, cloned into every `SubCanvas` `create_sub_canvas()`
produces, so `fetch_add`'s own atomicity is what guarantees uniqueness
regardless of which thread calls it or in what order. Layer ID has no
such requirement -- it only distinguishes standard content from the
overlay plane, so two different sub-canvases both drawing standard
content at Layer `0` (or both calling `begin_overlay(OverlayLayerPriority
(0))`) is completely correct; `overlay_stack` (and `state_stack`/
`clip_stack`) stay exactly what they already are, private and
thread-local, with no sharing at all.

**The concurrency cap (TECHNICAL.md Section 8: "`available_parallelism()`
minus one") is enforced by an exact, panic-safe live count, not a
best-effort one.** `create_sub_canvas()` increments a shared
`Arc<AtomicUsize>` via a compare-exchange loop (checking the cap
*before* committing the increment, not fixing it up after), and
`SubCanvas`'s own `Drop` impl decrements it -- exact even if a caller
panics mid-use and that panic is later caught (TECHNICAL.md Section
9.4's own `catch_unwind` FFI boundary means a caught panic doesn't
necessarily end the process, so a merely-approximate live count could
permanently wedge future `create_sub_canvas()` calls after one caught
panic). The real `std::thread::available_parallelism()` sets the cap
for normal use; a `#[cfg(test)]`-only constructor overrides it with a
small, deterministic value so the cap's own panic behavior is testable
without depending on the test runner's actual core count (which could,
in principle, make every real `create_sub_canvas()` call panic
immediately on a genuinely single-core CI runner).

**No data extraction yet -- `SubCanvas` cannot be flattened or
consumed for its recorded data in this sub-step.** `RenderingCanvas::
flatten(self)` takes `self` by value; `Deref`/`DerefMut` only forward
`&self`/`&mut self` methods, so a bare `SubCanvas` genuinely cannot
call it today, and this sub-step doesn't add an escape hatch (no
`SubCanvas::finish()`) either -- extracting a sub-canvas's data is
exactly 5.2.2's job (the real stitching primitive), not this one's.
Until then, dropping a `SubCanvas` (ordinary scope exit, or an explicit
`drop()`) is the only way to end one, which is sufficient to prove and
test the live-count release.

**Zero new dependencies.** `Arc`, `AtomicU32`, `AtomicUsize`, and
`std::thread::available_parallelism` are all `std` -- no new crate for
either `tre-engine` (which stays `#![forbid(unsafe_code)]`, since
sharing plain, already-`Send`/`Sync` atomics via `Arc` needs no
`unsafe` of its own) or the workspace.

## Goal

`RenderingCanvas::create_sub_canvas(&self) -> SubCanvas` produces an
independently-recordable `SubCanvas` sharing the root's Depth ID
counter and cap-enforcement bookkeeping, usable from a real spawned OS
thread with the exact same drawing API (`save`/`restore`/`transform`/
`set_alpha`/`push_clip`/`pop_clip`/`begin_overlay`/`end_overlay`/
`draw_rounded_rect`/`draw_text`) the root canvas already has -- proven
by a real multi-thread stress test in which every thread's recorded
Depth IDs are collected and confirmed collision-free, the
`available_parallelism() - 1` cap is confirmed exact (both that the
next call past it panics, and that dropping a `SubCanvas` frees its
slot for reuse), and Layer ID is confirmed *not* required to be unique
across sub-canvases.

## Tasks

1. **Promote `next_depth_id` from `u32` to `Arc<AtomicU32>`** on
   `RenderingCanvas`. Update `next_sort_key` to `self.next_depth_id.
   fetch_add(1, Ordering::Relaxed)` -- drops the now-redundant
   `.expect("next_depth_id overflowed u32")` panic (`compute_sort_key`'s
   own debug assert already catches the real, far lower 20-bit Depth ID
   overflow threshold; a raw `u32` wrap at 4 billion was never the
   meaningful check).

2. **New `RenderingCanvas` fields**: `max_sub_canvases: usize` (set once
   at construction, copied by value into every `SubCanvas`, never
   mutated after); `live_sub_canvases: Arc<AtomicUsize>` (shared,
   starts at `0`). `RenderingCanvas::new()` sets `max_sub_canvases` from
   `std::thread::available_parallelism()` minus one (with a documented
   sane fallback if the query itself errors).

3. **`pub struct SubCanvas`**: wraps a private `RenderingCanvas` plus
   its own clone of `live_sub_canvases`; `Deref`/`DerefMut` to
   `RenderingCanvas` so every existing drawing method works unchanged;
   `Drop` decrements `live_sub_canvases`.

4. **`RenderingCanvas::create_sub_canvas(&self) -> SubCanvas`**:
   compare-exchange loop against `live_sub_canvases`/`max_sub_canvases`
   (panics with a clear message, naming both the configured limit and
   that it was exceeded, if the cap would be violated -- checked before
   the increment commits, not fixed up afterward); the new `SubCanvas`'s
   inner `RenderingCanvas` gets a fresh, empty `state_stack`/
   `clip_stack`/`overlay_stack`/`vertices`/`indices`/`commands` (matching
   `RenderingCanvas::new()`'s own identity-state seeding) but shares
   `Arc::clone(&self.next_depth_id)`, `self.max_sub_canvases` (copied),
   and `Arc::clone(&self.live_sub_canvases)`.

5. **`#[cfg(test)]`-only constructor** (e.g. `RenderingCanvas::
   new_with_sub_canvas_cap(cap: usize)`) overriding `max_sub_canvases`
   directly, so the cap's panic behavior is deterministically testable
   regardless of the test runner's real core count.

6. **Unit tests**: a real multi-thread stress test (`std::thread::
   spawn`, matching `atlas_concurrency_demo`'s and `MpscRingBuffer`'s
   own established "real OS threads, not a synthetic simulation"
   precedent) -- several real threads each get a `SubCanvas` via
   `create_sub_canvas()`, each records several `draw_rounded_rect`
   calls, each reports its own commands' Depth IDs back (via the
   thread's own return value, joined on the main thread); asserts the
   union across every thread has zero duplicate Depth IDs and the
   expected total count. Also: the deterministic-cap constructor's
   `create_sub_canvas()` panics exactly on the call past the cap, not
   before; dropping a `SubCanvas` frees its slot for a subsequent
   `create_sub_canvas()` to reuse; two different sub-canvases both
   using Layer `0` (and both calling `begin_overlay(OverlayLayerPriority
   (0))`) is not an error and produces no Depth-ID-style collision
   concern, since Layer ID was never required to be unique; a
   `SubCanvas`'s own `save`/`push_clip`/`draw_rounded_rect` calls work
   identically to the root canvas's (a brief smoke test via delegation,
   not re-proving `draw_rounded_rect`'s own already-tested correctness).

7. **Docs**: IMPLEMENTATION.md Step 5.2.1 subsection; REVIEW.md entry;
   ARCHITECTURE.md/TECHNICAL.md only if a new convention surfaces beyond
   what's already planned here.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- No demo this sub-step -- matching Steps 4.3.1/4.3.2's own precedent
  (a `SubCanvas` alone has no way to be rendered yet; the real
  end-to-end GPU proof lands in the 5.2.3 capstone, exactly as Step
  4.3.3 was for 4.3.1/4.3.2). Proven by real concurrent unit tests
  instead.
- All pre-existing examples re-run manually as a regression check,
  since `next_sort_key`'s internal change (still producing the exact
  same values for any single-threaded caller) touches a shared code
  path every example goes through.

## Explicitly out of scope for this sub-step

- The real lock-free stitching primitive and any way to merge a
  `SubCanvas`'s recorded data back into the root canvas or a
  `FlattenedFrame` -- Step 5.2.2.
- `SubCanvas::finish()` or any other data-extraction method -- also
  5.2.2, since extraction and stitching are the same real problem.
- Balance-checking a `SubCanvas`'s own `state_stack`/`clip_stack`/
  `overlay_stack` before it's dropped/extracted (`RenderingCanvas::
  flatten`'s own debug asserts do this today, but nothing calls
  `flatten` on a `SubCanvas` yet) -- deferred alongside 5.2.2's
  extraction method, which is what will need this check.
- Nested sub-canvases (a `SubCanvas` spawning its own child
  `SubCanvas`) -- only the root `RenderingCanvas` exposes
  `create_sub_canvas()`.
- A real GPU demo -- Step 5.2.3.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code, or to
  `tre-memory` -- this sub-step is `tre-engine`-only.
