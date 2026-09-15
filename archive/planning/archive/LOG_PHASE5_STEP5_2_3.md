# Log: Phase 5, Step 5.2.3 -- The Capstone: Real Concurrent Recording, Rendered

## Two real bugs found and fixed during the demo's own first-draft development

### Bug 1: a second, independent root canvas broke Depth ID sharing

The first draft called `create_sub_canvas()` on one `RenderingCanvas`
(`root`) but drew the root's own rect on a *different*
`RenderingCanvas::new()` (`root_canvas`) before stitching that one.
Since Depth ID is only unique *within one shared counter*
(`Arc<AtomicU32>`, Step 5.2.1), and `root_canvas` had its own
completely independent counter, its rect's Depth ID could -- and did --
collide with a worker's. The real GPU render immediately surfaced this
as 4 batches instead of the expected 3: a plain rect that should have
merged with the others sat alone instead, because after sorting, its
tied sort key put it in an unpredictable position relative to the
command it should have merged with.

**Fix:** draw the root's own rect directly on `root` itself, and defer
`root.stitch_into(&arena)` (which consumes `root` by value) until
*after* every `create_sub_canvas()` call is done borrowing it. This is
also what naturally shares the one Depth ID counter across the root and
every worker, which is the actual point.

### Bug 2 (a real design gap, not a coding mistake): concurrent stitching interacts with marker-based hard barriers to make batch count scheduling-dependent

After fixing Bug 1, the demo still occasionally reported 4 batches
instead of 3 -- not from a collision this time, but from a genuinely
different, correct cause: `begin_overlay`/`end_overlay`'s
`PushScissor`/`PopScissor` markers are hard run-segmentation barriers
(Step 5.1.3's own design, unchanged and correct). Since concurrent
`stitch_into` calls land in the shared `commands` array in whatever
order their `fetch_add` reservations happen to complete in -- not
submission order -- the overlay worker's marker pair can end up
*between* two otherwise-mergeable plain rects from two *other* workers,
splitting them into separate runs that never merge. This is real,
correct behavior, not a bug in the engine -- it's ARCHITECTURE.md
Section 4.2's own "soft target, not a guarantee" caveat about batch
count, made concretely observable for the first time because every
earlier demo's recording was single-threaded (where this interleaving
simply cannot happen).

**Fix:** not a code fix in the engine -- a fix to the demo's own
verification, which had been asserting something the architecture
never actually promised. Revised to assert what's genuinely guaranteed
regardless of scheduling: every plain rect's own 6 indices are
accounted for *somewhere* in the final frame (1 batch or several,
doesn't matter), and the overlay rect and text glyph each always stay
their own single, never-merging batch (since nothing else shares their
Layer ID or pipeline, there's nothing for them to ever mismerge with,
regardless of scheduling).

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace`: all clean, zero warnings, zero failures across every
  crate.
- The new demo, `canvas_sub_canvas_demo`, was run 15+ times in a row on
  the real development machine (24 real cores, so `worker_count` = 4,
  the demo's own capped maximum, every run) specifically to observe the
  batch-count variability directly rather than just reason about it in
  the abstract -- confirmed genuinely varying between 1 and 2 separate
  plain-rect batches across runs, while every pixel-level and content
  assertion held on every single run with zero exceptions.
- Real GPU pixel verification: every worker thread's own rect (drawn on
  a different real OS thread) renders white at its own correct
  position; the root's own directly-drawn rect renders correctly
  alongside them; the gap between adjacent rects stays background,
  proving they're genuinely distinct shapes; the real MSDF text glyph
  (drawn on a worker thread, not the main thread) renders real,
  non-background fill.
- All 18 pre-existing examples re-run manually against real Vulkan
  hardware: zero regressions. This sub-step's one new engine change
  (`RenderingCanvas::max_sub_canvases`) is a read-only accessor with no
  behavioral effect on any existing code path.
- `std::thread::scope` (not `std::thread::spawn`) was required once the
  demo needed to borrow local, non-`'static` data (`font`/`shaped`/
  `handle`) from worker-thread closures -- scoped threads guarantee
  every spawned thread is joined (and any panic re-raised) before the
  scope itself returns, which is exactly the lifetime this demo already
  needed and is simpler than cloning everything into each thread.
- Confirmed `RenderingCanvas::max_sub_canvases()` reports the same real
  `available_parallelism() - 1` value regardless of which fresh
  `RenderingCanvas::new()` instance asks (used once, early, just to
  decide `worker_count`, entirely separate from the actual `root` used
  for real recording).
- This closes Step 5.2 (5.2.1, 5.2.2, 5.2.3) in full. Added
  `canvas_sub_canvas_demo` to the `vulkan-validation` CI job.
