# PLAN — M39 Phase 3: Shape-Morphed Border Inset Fix

## Goal
Fix M38 Phase 4's own stated v1 gap: a border stroked while a real
shape morph is active strokes the raw vertex silhouette centered, no
inset, so it can sit up to half its own width outside the fill's own
edge.

## Steps
1. Real investigation first: does `kurbo` already expose a polygon-
   inset/offset operation? Direct source read of `kurbo = "0.13.1"`'s
   own vendored `offset.rs` found `pub fn offset_cubic(c: CubicBez,
   d: f64, tolerance: f64, result: &mut BezPath)` -- a single-cubic
   Bézier offset-curve algorithm, genuinely the wrong tool for
   `ShapeKey`'s own vertices-only straight-line-segment shapes (`shape_
   morph.rs`'s own module doc comment: "always straight-line segments
   between the interpolated vertices"), not merely unused. Confirmed
   via grep: no general polygon-offset/inset operation exists
   anywhere in this codebase's own kurbo usage.
2. New `ShapeKey::inset_path(amount) -> BezPath` (`shape_morph.rs`): a
   real, new, self-contained straight-edge polygon inset. For each
   real edge, computes its inward normal (resolved per-edge by
   comparing against the real polygon centroid, not an assumed CW/CCW
   winding -- `ShapeKey`'s own vertices can come from any caller-
   supplied path with no guaranteed winding, and `interpolate`'s own
   alignment search can reorder them further), offsets the edge line
   by `amount`, then finds each new vertex as the real intersection of
   its two adjacent offset edges (a real miter join). New private
   `line_intersection` helper. Real, stated v1 scope limit: correct
   for the real border widths this codebase actually uses against
   MD3-scale shapes, not proven robust against a self-intersecting
   inset.
3. `engine-render`'s border block (`lib.rs`) now strokes `node.paint.
   shape.current.inset_path(inset)` instead of `.to_path()` for the
   real "active shape morph" branch -- the fill geometry itself is
   untouched, only the border path changed.
4. Real tests: 4 new `engine-core` unit tests in `shape_morph.rs` --
   a hand-derived case (a 10x10 square inset by 2.0 must produce
   exactly `(2,2)-(8,2)-(8,8)-(2,8)`, independently traced by hand
   through the real algorithm before the assertion was written, not
   just picked because it looked plausible), a reversed-winding twin
   proving the centroid-based normal resolution doesn't depend on
   winding direction, a zero-amount true-no-op case, and a degenerate
   2-point case. New real pixel-readback test in `engine-render/tests/
   shape_morph_paint.rs` -- a 20px border on a 60x60 shape centered in
   a 100x100 box: before this phase, the border would have bled 10px
   past the shape's own `x=20` edge (empirically the exact real bug);
   after, a point in that bled zone is proven plain background, a
   point in the real inset border band is proven the border color, and
   the shape's own interior is proven unaffected.
5. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release` (no
   Python-facing API changed this phase, rebuilt anyway per the
   standing chain), full `pytest tests/`, all 77 examples, showcase
   demo.
6. `BUILD_TRACKER.md` Phase 3 flipped fully to done (heading, step
   bullet, milestone status line, Top Metrics row all updated
   together, matching Phase 2's own precedent for a single-step
   phase). Artifact regenerated (39/127/218, unchanged) and
   republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 212 passed, +4 from this step;
`engine-render`'s `shape_morph_paint` integration suite 3 passed, +1;
`pytest tests/`: 581 passed/1 skipped, unchanged -- no Python-facing
API touched this phase; all 77 examples + showcase demo clean).
**M39 Phase 3 -- Shape-Morphed Border Inset Fix -- is now complete.
Phases 4-5 of M39 remain: Terminal Cell Text Attributes, `Tree::
tick_all` Active-Set Optimization.**
