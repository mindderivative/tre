# Demo: Phase 3, Step 3.3.2 -- SIMD Path-Morphing Interpolation

```bash
./demo/phase3_step3_3_2/run_svg_morph_demo.sh
```

**What's actually new here:** Step 3.3.1's tessellator turned real SVG
path data into triangles, but every polygon was a static, one-time
shape. This step adds real animation math: `tre-math::lerp_points_batch`,
a genuine SIMD batch operation (`wide::f32x8`, TECHNICAL.md Section 5.4),
interpolating two keyframe polygons' vertices in lockstep. `tre-svg::morph`
wraps it with the actual, real-world-relevant validation this needs:
rejecting keyframes with different vertex counts via `Result`
(`SvgError::TopologyMismatch`) rather than guessing how to reconcile them.

**Two keyframes, both independently parsed real SVG:** a diamond
(`M 150 50 L 250 150 L 150 250 L 50 150 Z`) and a square
(`M 75 75 L 225 75 L 225 225 L 75 225 Z`) -- deliberately straight-line-only
with the same literal number of `L` commands, so the equal-vertex-count
requirement is structural, not a coincidence of two independently-run
curve-flattening passes.

**Verified by reading back real pixels at three points in the timeline
(`t = 0.0, 0.5, 1.0`)**, using two probe points chosen for maximum
discriminating power:
- `POINT_A` is inside the diamond but outside the square.
- `POINT_B` is outside **both** keyframes, but inside the *exact*
  vertex-wise midpoint quadrilateral -- the strongest possible proof that
  `t=0.5` renders a genuinely different silhouette, not a blend confined
  to either endpoint's own footprint.

The rendered `t=0.5` frame is a visibly distinct tilted quadrilateral --
not a hand-authored shape, but the direct output of a real SIMD
interpolation between two independently-authored keyframes.

**No new shader was needed** -- the interpolated polygon is re-triangulated
fresh each frame via the existing `triangulate`, then rendered through
the pre-existing flat-color pipeline, exactly like Step 3.3.1.

**Explicitly out of scope:** arc-length resampling for mismatched vertex
counts, raw Bezier-control-point morphing, easing curves, and animation
timelines -- see `planning/archive/PLAN_PHASE3_STEP3_3_2.md`.
