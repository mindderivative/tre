# Plan: Phase 9, Step 9.2 -- Zero-Allocation & Balance Assertions in CI

## Goal

Build IMPLEMENTATION.md Phase 9 Step 9.2's real CI gates: (1) a
zero-allocation debug guard (TECHNICAL.md Section 3.4) enforced as a
hard CI gate, and (2) the `PushLayer`/`PopLayer` balance-assertion gate
(Phase 2 Step 2.2).

## Real discrepancies found during pre-work, confirmed with the project
## owner before proceeding

1. **The zero-allocation debug guard never existed.** TECHNICAL.md
   Section 3.4 fully specifies a custom `#[global_allocator]` wrapper
   checking a thread-local "render tick active" flag; nothing
   implementing it exists anywhere (`global_allocator`/`GlobalAlloc`/
   `thread_local` all return zero hits). Confirmed: **build it for
   real.**
2. **REVIEW.md finding #134** (main_loop_demo allocates ~20+
   times/frame; no `reset()`/reuse API exists on `RenderingCanvas`/
   `SubCanvas`/`FrameArena`/`ScatterArena`) would make the guard fail
   immediately if wired to the real main loop. Confirmed: **fix finding
   #134 first**, then wire the guard to the real loop.
3. **The criterion/cargo-bench performance suite (TECHNICAL.md Section
   9.2) never existed either** -- no `criterion` dependency, no
   `benches/` directory anywhere. Confirmed: **build a minimal
   criterion bench** verifying the $\le 0.50\text{ ms}$ CPU budget, gated
   in CI.
4. **New finding, surfaced only by actually building the guard and
   investigating the real RHI submit path:** `VulkanDevice::begin_frame`
   (`crates/tre-rhi-vulkan/src/lib.rs`) allocates a fresh `Box<dyn
   RhiCommandBuffer>` every frame, even though the underlying Vulkan
   `vk::CommandBuffer` handle is already reused (per `begin_frame`'s own
   comment). A real fix means changing `RhiDevice::begin_frame`/
   `submit_and_present`'s ownership model (`Box<dyn RhiCommandBuffer>`
   by value) -- rippling through all 31 demo call sites, genuine
   trait-boundary redesign, not a bug-fix-sized change (the same shape
   finding #134 itself has, and was legitimately deferred with that
   exact reasoning). **Not fixed here** -- disclosed as a new REVIEW.md
   finding. The zero-alloc guard's real CI-enforced scope is therefore
   the CPU-side span this step actually owns and fixed (canvas
   recording -> sort/batch -> ring-buffer write), not RHI submission --
   an honest boundary, not a silent narrowing, matching how Step 9.1
   scoped its own small-N radix-sort-fallback and placeholder-glyph
   boundaries.

## Scope decisions

1. **`ScatterArena<T>::reset`/`drain_into`** (`tre-memory`): `reset`
   zeroes `len` (O(1), no allocation, `T: Copy` needs no drop glue --
   matches existing reasoning in this file). `drain_into(&mut self, out:
   &mut Vec<T>)` clears `out` (keeps capacity) and copies the written
   prefix into it, then resets `len` -- combines extraction and reset
   into one call, avoiding a separate always-allocating `into_vec()` in
   the reused-arena path. `into_vec` stays, unchanged, for existing
   callers/tests.
2. **`RenderingCanvas::reset`** (`tre-engine`): clears every internal
   `Vec` (`.clear()`, keeps capacity) except `state_stack`, which is
   cleared and given back exactly one identity entry; resets the shared
   `next_depth_id` counter to 0 (matches today's real per-frame-fresh-
   canvas semantics exactly, since a reused canvas must not let the
   Depth ID counter grow unbounded across a long-running session).
   Available on `SubCanvas` automatically via its existing `DerefMut`.
3. **`RenderingCanvas::stitch_into`/`SubCanvas::stitch_into` become
   non-consuming** (`&self` instead of `self`): the method only ever
   copies data out into the arena's reserved slices (`copy_from_slice`,
   never a move) -- it never needed ownership, the consuming signature
   was incidental. This is what actually enables reusing the same
   `SubCanvas` across frames instead of constructing/dropping one every
   frame; also removes the existing `std::mem::take` Drop-workaround in
   `SubCanvas::stitch_into`, since there's no partial-move-out-of-a-
   `Drop`-type problem left to work around.
4. **`FrameArena::flatten_into`** (`tre-engine`), new: a non-consuming
   sibling of `flatten()`/`flatten_unbatched()` that drains each
   internal `ScatterArena` into a caller-provided, reused
   `FlattenedFrame`'s own fields (via `drain_into`) and runs the same
   sort/merge logic as `segment_and_flatten`, writing into that
   `FlattenedFrame`'s `commands`/`indices` (cleared, not reallocated)
   and a caller-provided reused scratch buffer, instead of allocating
   fresh output `Vec`s every call. The existing consuming `flatten()`/
   `flatten_unbatched()` stay unchanged (still real, still used by every
   single-shot/test call site) -- this is a pure addition sharing the
   sort/merge core via a small internal helper, not a breaking change.
5. **`main_loop_demo.rs` rewritten to build every per-frame structure
   once, before the loop**, and `reset()`/`flatten_into()` each frame
   instead of reconstructing: root canvas, each worker's `SubCanvas`
   (created once via `create_sub_canvas()`, never dropped/recreated),
   `FrameArena` (plain owned value, not `Arc` -- `std::thread::scope`
   lets spawned closures borrow it directly, so the existing `Arc::new`/
   `Arc::try_unwrap` per frame goes away entirely, itself a real
   allocation removed), and the output `FlattenedFrame`/sort-scratch
   buffer.
6. **The debug alloc guard lives in `tre-engine`** (`alloc_guard.rs`):
   `DebugAllocGuard` (a `GlobalAlloc` wrapper around `std::alloc::
   System`) and `RenderTickGuard` (an RAII scope guard flipping a
   thread-local flag). `#[global_allocator]` itself is installed by the
   *binary* that opts in (`main_loop_demo.rs`), never by the library
   crate -- `#[global_allocator]` is whole-binary-scoped, so a library
   crate cannot install one for every consumer. Each thread that should
   be checked (main thread + every worker thread recording a
   `SubCanvas`) starts its own `RenderTickGuard`, since the flag is
   thread-local by design -- a worker thread's own allocations are
   invisible to the main thread's guard otherwise. A violation disarms
   the flag *before* panicking, so the panic machinery's own formatting/
   unwind allocations don't recursively re-trigger the check.
7. **A minimal `criterion` bench** (`tre-engine/benches/frame_
   processing.rs`): benchmarks one representative frame's real
   record-then-`flatten()` cost. CI parses the reported per-iteration
   time and fails the job if it exceeds $0.50\text{ ms}$ -- a real, if
   minimal, hard gate, not merely a number nobody checks.
8. **Balance-assertion gate (task 2): already real, confirmed, not
   newly built.** The 4 `debug_assert_eq!`/`debug_assert!` checks in
   `flatten()`/`flatten_unbatched()`/`FrameArena`'s stitch path already
   exist, are already covered by 4 passing `should_panic` tests, and
   CI's `test`/`vulkan-validation` jobs already run in debug mode
   (`cargo test`/`cargo run`, no `--release`) -- an unbalanced stack
   anywhere already fails CI today. This task needs documentation
   (naming it explicitly as this step's own gate) and one more explicit
   regression test exercising the balance check inside the *reused*
   canvas path (new this step), not new enforcement machinery.

## Tasks

1. `crates/tre-memory/src/scatter.rs`: `ScatterArena::reset`/
   `drain_into` + unit tests.
2. `crates/tre-engine/src/lib.rs`: `RenderingCanvas::reset`;
   `stitch_into` (`RenderingCanvas` + `SubCanvas`) changed to `&self`;
   `FrameArena::flatten_into` (shares sort/merge core with
   `segment_and_flatten`); unit tests for all three, including a reused-
   canvas-across-multiple-flattens regression test and a balance-
   assertion test specifically on the reused path.
3. `crates/tre-engine/src/alloc_guard.rs` (new module): `DebugAllocGuard`
   (`GlobalAlloc` impl) + `RenderTickGuard` (RAII flag scope). Real unit
   tests: allocating outside a tick is fine; allocating inside a tick
   panics with the expected message; the flag is thread-local (a
   background thread's own tick doesn't affect the main thread's
   allocations, and vice versa); a violation's panic doesn't itself
   deadlock/double-panic (already-disarmed-before-panicking behavior
   under test).
4. `crates/tre-rhi-vulkan/examples/main_loop_demo.rs`: rewritten per
   scope decision 5, with `#[global_allocator] static ALLOCATOR:
   tre_engine::DebugAllocGuard = ...;` installed and `RenderTickGuard`
   wrapping the CPU-side per-frame span (canvas recording -> sort/batch
   -> ring-buffer write) on the main thread and inside each worker's
   `scope.spawn` closure.
5. `crates/tre-engine/benches/frame_processing.rs` (new): the
   criterion bench per scope decision 7; `Cargo.toml` `[dev-dependencies]`
   addition + `[[bench]]` entry.
6. `.github/workflows/ci.yml`: new step(s) in the `test` (or a new
   `zero-alloc-validation`) job running `main_loop_demo` under the guard
   as a hard gate, and a step running the criterion bench and failing if
   the reported time exceeds $0.50\text{ ms}$.
7. `documentation/DESIGN.md`/`TECHNICAL.md`/`ARCHITECTURE.md`: mark
   Section 3.4 and the balance-assertion gate real/implemented, with
   dated annotations matching this project's own established
   "annotate, don't rewrite the canonical block" convention.
   `documentation/IMPLEMENTATION.md` Phase 9 Step 9.2: real write-up.
   `documentation/REVIEW.md`: new finding for `begin_frame`'s per-frame
   `Box::new` (disclosed, not fixed), plus closing finding #134 as fixed
   here.
8. `demo/phase9_step9_2/`: README + note that this step's "demo" *is*
   `main_loop_demo.rs` re-run under the new guard (no new example
   binary needed -- the guard is infrastructure, not a visual feature).

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- New `tre-memory`/`tre-engine` unit tests pass, including the alloc-
  guard's own violation-detection and thread-local-isolation tests.
- `main_loop_demo` re-run manually under the real guard: passes clean
  (zero violations) for the CPU-side span this step fixes; confirms the
  disclosed `begin_frame` gap is the *only* remaining violation if the
  guard's scope is (as an experiment, not the shipped default) widened
  to include RHI submission -- proving the disclosure is accurate, not
  guessed.
- Full manual regression sweep of all 31 pre-existing Vulkan demos,
  since `stitch_into`'s signature changed (consuming -> borrowing) --
  every existing call site updated and re-run.
- `cargo bench` runs clean; CI's parsed-time gate is exercised (real
  pass, and a deliberate temporary regression confirms it can actually
  fail the job, matching this project's own "prove the gate can fail,
  not just that it can pass" precedent from Step 2.4/finding-verification
  work).
- Commit; push only on explicit "push it"; `gh run watch` after any push
  (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- Fixing `begin_frame`'s per-frame `Box<dyn RhiCommandBuffer>`
  allocation -- confirmed as genuine trait-boundary redesign work,
  disclosed as a new REVIEW.md finding, not fixed here.
- A persistent, reused worker-thread *pool* for Multi-Thread Canvas --
  still real, separate future work (Step 8.1.2's own disclosed
  boundary, unchanged); this step reuses each worker's `SubCanvas`
  *data*, not the OS thread itself, which continues to be freshly
  spawned every frame via `std::thread::scope`, matching the existing,
  disclosed scope line exactly.
- Extending the criterion bench beyond one representative frame shape,
  or building a full benchmark suite across every demo scene --
  TECHNICAL.md Section 9.2's own larger, separate scope; this step
  builds the minimal real gate Step 9.2's task list actually names.
- Live, mid-run atlas growth -- unrelated, already-disclosed, separate
  gap (Step 8.1.2).
