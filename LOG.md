# LOG — Branch `0.4.0`: Milestone 5 (complete)

- M4 complete: `DamageTracker` (compare painted rects and fingerprints,
  merge to 4 rects or full) with 18 tests; cargo 348.

## Status

**Step 1 done (2026-09-28).** Each window's frame computes its damage
after layout: `None` re-presents the kept frame, `Rects` renders a scene
clipped to the rects with `ClearSettings::Rects`, `Full` as before. Resets
on target recreate, font registration, and a failed surface acquire.
`window.set(partial_redraw=False)` switches it off. Seven GPU tests find
partial redraw byte-identical to full; a live animation renders 1 full
frame then 59 partial ones. cargo 355, pytest 431 + 1 skipped, 20 examples,
mkdocs strict, mypy strict.

**Step 2 done (2026-09-28).** Nine animation tests tick real animations
through 12 frames, comparing a partial-redraw window with a full-redraw
one byte for byte every frame: opacity + corner radius, retargeted fill,
scale + shadow, shadows, translation, rotation, a group's border, path
morph + trim, scroll offset. Shrinking every rect by 3 px fails 9 of 16,
so the tests see a too-small rect. cargo 364.

**Step 3 done, M5 complete (2026-09-28).** `tests/partial_redraw_bench.rs`
(ignored): 1920x1080, 576 cards, GPU wait included, Radeon 890M.
v0.3.5.1 measured with a port in a removed worktree, 5 alternating runs.
Small animation: 0.56-1.00 ms vs 2.16-4.78 ms (3.7-4.9x less, every
pair). Whole window: 2.30-5.34 ms vs 2.18-5.19 ms (within noise), after
fixing what the first run found -- 117-180 ms from the cubic pair merge
over 577 rects; past 64 rects the merge now takes their bounding box.
Two new damage tests. cargo 366.

**Pre-release review (2026-09-28).** Four-lens workflow, 8 agents; 33
findings, 32 confirmed, 20 fixed. Headline: partial redraw culled by
layout box and skipped subtrees, so shadows and overflowing children kept
stale pixels -- now one shared painted extent. Also size clamps, image
limits, surface Outdated recovery, walk perf, CI token scope. cargo 370,
pytest 433 + 1 skipped. Report: https://claude.ai/artifact/X4Xo6Zwk4spCRxnDTWQMk3

**Phase 2 Step 1 (2026-09-28).** MSRV 1.90 (1.89 failed on ordered-float
5.5.0), msrv CI job, maturin-action and action-gh-release pinned to SHAs.

**Step 2 (2026-09-28).** GPU setup failures raise RuntimeError from
App.run (platform created-callback returns bool); zero-size Window raises.

**Step 3 (2026-09-28).** walk.rs: one traversal, Painter and Recorder
visitors; draw_own unchanged; all pixel tests unchanged.

**Step 4 (2026-09-28).** WindowRenderer (prepare/draw/reset/resize) owns
the render state; app, pixel tests and bench drive it; engine-py drops
its direct vello_gpu dependency.

**Step 5 (2026-09-28).** foldhash fingerprints instead of SipHash (a
counter would be instrumentation); walk beside a 200x60 terminal + 2000
commands: 0.32-0.77 ms -> 0.07-0.10 ms.

**Step 6 (2026-09-28).** Lost surfaces recreated; platform passes
os_requested to on_frame; no present for loop-requested, undamaged,
non-animating frames; OS redraws re-present the kept frame.

**Step 7 (2026-09-28).** WindowHandles (11 shared handles, one struct);
get("partial_redraw_active").

**Step 8 (2026-09-28).** 21 test files on support::device / read_texture
(-1,142 +103 lines); rect_window keeps its own (real surface).

**Step 9 (2026-09-28).** Images stay on tre's own textures: atlas has no
public in-place write, unwraps allocation (panics), 4096 pages.

Next: Step 10, split tree.rs.
