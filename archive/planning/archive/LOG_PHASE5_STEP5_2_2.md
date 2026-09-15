# Log: Phase 5, Step 5.2.2 -- Real Lock-Free Stitching

## No design bugs found -- every core decision held up on the first real run

`ScatterArena<T>`'s atomic bump-reservation, `stitch_into`'s copy-and-
rebase logic, and `FrameArena::flatten`'s reuse of Step 5.1.3's real
sort/merge algorithm all compiled and passed every new test -- including
both real concurrency stress tests -- on the first attempt. The riskiest
call in `PLAN.md` (workers stitching themselves rather than a
coordinator-only sequential merge, which would have needed no atomics
at all) needed no revision once implemented.

## Two small, purely mechanical clippy fixes (not design bugs)

- `clippy::needless_pass_by_value` on `segment_and_flatten`'s `indices`
  parameter: extracting the shared segmentation-sort-merge loop out of
  `RenderingCanvas::flatten` left `indices: Vec<u32>` as a by-value
  parameter that the function body only ever borrows (`&indices`,
  passed straight through to `flatten_run`) -- never actually owns or
  mutates. Changed to `indices: &[u32]`; both call sites
  (`RenderingCanvas::flatten`, `FrameArena::flatten`) pass a reference
  instead.
- `clippy::drop_non_drop` in two `ScatterArena` tests: an explicit
  `drop(slice)` call on a `ScatterSlice`, which has no `Drop` impl of
  its own (nothing needs cleanup for a `T: Copy` payload) -- the call
  was a habit carried over from testing types that *do* need explicit
  early drops (like `MpscRingBuffer`'s own queued-item cleanup). Removed
  both calls; NLL already ends the borrow at last use, letting the
  following `arena.into_vec()` call (which needs `self` by value)
  compile without it.

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace`: all clean, zero warnings, zero failures across every
  crate.
- `tre-memory` now has 30 unit tests (up from 25): 5 new ones for
  `ScatterArena` -- sequential `start_index()` correctness, writes
  through a reserved slice landing correctly in `into_vec()`,
  overflow reporting `None` rather than growing, `into_vec()`
  truncating to the actually-reserved prefix, and a real 8-thread/500-
  item-each (4,000 total) concurrent stress test confirming every
  `(thread_id, i)` pair appears in the final result exactly once, with
  no corruption or overlap across any pair of concurrently-granted
  ranges.
- `tre-engine` now has 44 unit tests (up from 41): the exact-value
  rebasing test is the most direct proof available -- two sub-canvases
  sharing one root's Depth ID counter (avoiding a sort-key tie that
  would make merge order implementation-defined), each drawing one
  rect, stitched in sequence; the resulting `frame.indices` matches
  `[0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]` exactly -- the second source's
  own locally-recorded `[0,1,2,0,2,3]` correctly shifted by +4, the
  first source's actual vertex count. The overflow test confirms
  `stitch_into` reports `false` cleanly against a deliberately
  undersized `FrameArena`. The concurrency test spawns 4 real
  `std::thread::spawn` threads, each holding its own `SubCanvas` (via
  5.2.1's own `create_sub_canvas`), each drawing one rect and then
  calling `stitch_into` on a shared `FrameArena` as its own very last
  action before the thread exits -- exactly the "worker performs its
  own stitch" pattern the whole sub-step's design rests on, not a
  simulated stand-in. The resulting frame has all 16 vertices and one
  correctly-merged 24-index command, confirmed via a scheduling-order-
  independent search (each thread's rect found somewhere in the merged
  vertices at its own expected position) rather than an assumption
  about thread completion order.
- All 19 pre-existing examples re-run manually against real Vulkan
  hardware: zero regressions from the `flatten()` refactor (extracting
  `segment_and_flatten` changed zero observable behavior for any
  existing single-canvas caller, confirmed both by the unchanged 41
  pre-existing `tre-engine` tests and this full example sweep).
- No new demo this sub-step, matching 5.2.1's own precedent: the real
  end-to-end GPU proof (real worker threads, real merged render, real
  pixel readback of a scene multiple threads contributed to) is Step
  5.2.3's capstone job.
- Docs: added a `ScatterArena`/`FrameArena` cross-reference to
  TECHNICAL.md Section 8's "Lock-Free Aggregation" bullet, matching the
  established "Implemented as of..." pattern already used for
  `MpscRingBuffer`/`SwmrSlotTable` -- the docs never named a concrete
  type for "the global arena" before this.
