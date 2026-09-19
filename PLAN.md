# PLAN — M38 Phase 1: Tessellated-Path Caching for Remaining Shapes

## Goal
Extend M34 Phase 1's own `GeometryCache` pattern (equality-keyed BezPath
reuse, mirroring `TextRenderer::shaped_layout`) beyond `Rect`/`Splitter`
to every other `NodeKind` that still tessellates a curve fresh every
frame -- one of the seven real, currently-open gaps the user chose to
close via "Let's knock out the known gaps."

## Steps
1. Widened `GeometryCache` (`geometry_cache.rs`) with `CircleParams`/
   `ArcParams` param types and three new cache maps/methods:
   `circle_primary`/`circle_secondary` (two independent per-node slots
   -- `RadioButton`'s ring+dot and `Switch`'s handle each need one, no
   single node ever needs both, confirmed via direct call-site read
   before sharing the slot) and `arc` (for `CircularProgress`'s sweep).
   Generalized the private `get_or_build` helper from a
   `RectPathParams`-only signature to `get_or_build<P: PartialEq>` so
   all four param types share one real implementation. Widened
   `evict_stale` to retain-filter the three new maps too. Four new
   unit tests (8/8 passing in the module).
2. Rewired `engine-render::paint_node`'s arms: `Checkbox`'s box fill
   and `Switch`'s track fill/outline stroke reuse the *existing*
   `rounded_rect_fill`/`rounded_rect_border` directly (their geometry
   is byte-for-byte identical to `Rect`/`Splitter`'s, confirmed by
   direct comparison, not assumed); `RadioButton`'s ring/dot and
   `Switch`'s handle now call `circle_primary`/`circle_secondary`;
   `CircularProgress`'s arc stroke now calls `arc`.
3. **Real correction, mid-phase, the identical discipline M35 Phase
   2's own rotation-field correction established:** the phase's own
   scoping note named `Terminal` as one of the five target
   `NodeKind`s, but the first pass skipped it on an unverified
   assumption ("plain rects, nothing to cache"). Direct read of
   `NodeKind::Terminal`'s own paint arm (`lib.rs:574-597`) found this
   was wrong -- its background is a real `RoundedRect::new(0.0, 0.0,
   w, h, radius).to_path(0.1)` fill, tessellated fresh every frame,
   identical to what `rounded_rect_fill` already caches elsewhere.
   Fixed by routing it through `rounded_rect_fill` too (its own
   per-cell glyph/cursor painting, `TextRenderer::draw_terminal`, is
   `TextRenderer`'s own cache, unrelated to `GeometryCache`).
4. **Two more real, previously-uncached sites found by the same
   direct-read discipline while verifying `Terminal`, both fixed by
   pure reuse of the already-tested `rounded_rect_fill` (zero new
   cache code needed):**
   - `TextField`'s own box fill had the identical fresh-`RoundedRect`-
     per-frame pattern -- routed through `rounded_rect_fill` too.
   - The universal interaction state-layer/ripple bounds (the `if let
     Some(interaction) = &node.interaction` block, which runs once per
     frame for *every* interactive node regardless of kind -- buttons,
     list items, icon buttons, anything with hover/ripple) built its
     own fresh `RoundedRect` every frame under the *same* `(id, w, h,
     radius)` as that node's own box fill -- now shares the identical
     cache slot via `rounded_rect_fill(id, w, h, radius)`. In practice
     this is the single hottest redundant-tessellation site in the
     whole render loop, found only by checking real call sites rather
     than trusting the original five-`NodeKind` scoping list.
   - The `VirtualList`/`Carousel`/`ScrollView`/general-`clip_children`
     scroll-clip path built its own fresh `RoundedRect` clip every
     frame too -- routed through `rounded_rect_fill(id, w, h,
     clip_radius)`, safe to share the slot since a node has either a
     box fill or a clip at a given `(w, h, radius)`, never conflicting
     params for the same id in the same frame.
5. Full verification chain, run twice (once after the five-`NodeKind`
   pass, again after the three additional discoveries): cargo
   check/clippy -D warnings/fmt, `cargo test --workspace --release`,
   `maturin develop --release`, full `pytest tests/`, all 75 examples,
   showcase demo.
6. `BUILD_TRACKER.md` Phase 1 flipped to done with a terse step-bullet
   note (full writeup here in `PLAN.md`/`LOG.md`, per the established
   "BUILD_TRACKER.md is the durable index, not the essay" convention),
   Top Metrics row updated to 1-of-7, artifact regenerated (38/122/212
   -- unchanged item count, pure status-flip) and republished.

## Status
Complete. Full verification chain green both runs (`cargo test
--workspace --release` clean, `engine-core` unchanged at 183,
`geometry_cache` module 8/8 including the 4 new tests; `pytest tests/`
556 passed/1 skipped both times, unchanged -- pure internal
optimization, no Python-facing API change; all 75 examples + showcase
demo clean both times). **M38 Phase 1 -- Tessellated-Path Caching for
Remaining Shapes is now complete. M38 itself remains open: 6 phases
remain (goal-column memory, fold-aware cursor navigation, Split Button
inner-corner shape-tightening, Button Group per-child shape change,
ScrollView scrollbar thumb, real scroll+clip for Code Editor).**
