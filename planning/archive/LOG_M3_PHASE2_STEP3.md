# Log: M3 Phase 2, Step 3 — Taffy Layout + Frame-Time CI Benchmark (§14 step 3)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 2, step 3 of 4.

## What happened

**Decided `slotmap` for `NodeId` rather than hand-rolling a generational
struct:** while adding `taffy` to `engine-core`, its build output showed
it pulls in `slotmap 1.1.1` transitively. `slotmap::new_key_type!`
generates exactly the `{ index, generation }` shape §5's own `NodeId`
doc comment describes, with checked `get`/`remove` built in — adopting it
costs zero new entries in the dependency graph (confirmed via
`cargo add slotmap --dry-run` resolving to the identical 1.1.1) and no
correctness gap versus hand-rolling the same thing.

**Implemented §5's data model in `engine-core/src/node.rs`, deliberately
narrower than the full spec** (documented inline, same discipline as
`MotionCurve::Linear`-only in step 2): `Node` omits `access`/
`interaction` (AccessKit is step 7, ripple is §7.3/step 9); `PaintProperties`
omits `transform`/`shape` (both need `Interpolate` impls nothing needs
yet); `NodeKind` carries only `Rect`/`Container`.

**`Tree` (`src/tree.rs`) wraps two parallel structures — a
`SlotMap<NodeId, Node>` and a `taffy::TaffyTree<()>`, linked by a
`SecondaryMap<NodeId, taffy::NodeId>`** — rather than implementing
taffy's custom-tree traits to compute directly against this crate's own
arena. Simpler, and provably correct by construction: `insert` is the
only place either structure is created, so they can't diverge. Four unit
tests, including a deterministic 3-child row layout (fixed 100px
children, no gap) asserting exact x-positions (0, 100, 200) — a
hand-computable answer, not just "did it not panic."

**`engine-render::build_tree_scene` composes layout and paint for the
first time:** walks a `Tree` from a root, accumulating each ancestor's
`taffy::Layout::location` (parent-relative) into an absolute position,
painting every `NodeKind::Rect`. Proved with a headless integration test
(`tests/layout_tree.rs`): two same-size children side by side in a row,
distinct colors, sampling one pixel inside each child's own laid-out box
— if layout were ignored, the second color would simply overwrite the
first everywhere. Both pixels landed correctly.

**Added the frame-time CI benchmark §6 has promised since M2**
(`tests/frame_budget.rs`): 300 nodes in a taffy flex-wrap grid, every
node with a genuinely active color animation, timing the real per-frame
sequence (`tick_all` → `compute_layout` → `build_tree_scene` → encode →
submit) across 30 iterations after 3 warm-up iterations (vello_hybrid
compiles GPU pipelines lazily on first use — a real one-time cost that
would otherwise dominate iteration 0). Median in release: **0.58ms** —
comfortably inside both the 16.6ms (60Hz) enforced target and the 8.3ms
(120Hz) stretch goal.

**Real finding, handled, not just noted:** the same benchmark measured
**~36ms median in a debug build** — over 60x slower, and well past the
16.6ms budget. This is expected (no inlining, no vectorization, bounds
checks everywhere in unoptimized Rust) and not a regression to chase:
nobody ships a debug build, and §6's budget is a claim about real
runtime performance. Shrinking the node count until debug mode happened
to pass would have produced a number meaning nothing — the same mistake
as a benchmark tuned to its own outcome. Instead the test is `#[ignore]`d
from the default `cargo test --workspace` run and documented to run
explicitly with `--release`; this is the command CI's frame-budget job
is expected to run.

**Extended the real windowed demo** (`tests/rect_window.rs`, step 1/2's
single hand-ticked rect) to a real `Tree` of 4 rects laid out in a row by
`taffy` (with padding and inter-item gap), each with its own
`PaintProperties` animating color on a staggered duration (600ms to
1200ms) — genuinely demonstrating several independent `Animated<T>`
instances advancing through one `Tree::tick_all` call, not one value
copy-pasted four times.

## Verification

```
$ cargo test --workspace
    ...
running 10 tests (engine-core)
test animation::tests::... (6 tests) ... ok
test tree::tests::insert_creates_a_parentless_node ... ok
test tree::tests::add_child_links_both_structures ... ok
test tree::tests::tick_all_reports_active_and_advances_every_node ... ok
test tree::tests::compute_layout_positions_row_children_left_to_right ... ok
test result: ok. 10 passed; 0 failed

     Running unittests src/lib.rs (engine_render)
test tests::rect_scene_renders_expected_pixels ... ok

     Running tests/animated_rect.rs (engine_render)
test animated_color_and_opacity_render_the_interpolated_value_mid_flight ... ok

     Running tests/frame_budget.rs (engine_render)
test frame_pipeline_fits_the_16_6ms_budget ... ignored, perf benchmark -- \
    meaningless in a debug build, run with: cargo test -p engine-render \
    --test frame_budget --release -- --ignored --nocapture

     Running tests/layout_tree.rs (engine_render)
test two_row_children_paint_at_their_own_laid_out_positions ... ok

     Running tests/rect_window.rs (harness = false)
engine-render §14 step 3: first frame presented, 420x120, 4 laid-out rects animating independently
engine-render §14 step 3: animation ran for 0.21s across 60 frames
engine-render §14 step 3: exited cleanly after 60 frames

$ cargo test -p engine-render --test frame_budget --release -- --ignored --nocapture
engine-render §14 step 3 frame budget: 300 nodes, median 0.580ms, max 0.986ms \
    (60Hz target 16.6ms, 120Hz stretch 8.3ms)
test frame_pipeline_fits_the_16_6ms_budget ... ok

$ cargo clippy --workspace --all-targets   # clean
$ cargo fmt --check                        # clean
```

## Next

`BUILD_TRACKER.md` M3 Phase 2 updated (step 3 of 4 done). Next: step 4,
wiring `parley` for text — a real typography spike per §14's own text
("don't stop at one static label... at least two type roles... plus one
non-trivial string"). This is also the first step nothing before it has
touched at all: no font/text code exists anywhere in the v2 tree yet.
