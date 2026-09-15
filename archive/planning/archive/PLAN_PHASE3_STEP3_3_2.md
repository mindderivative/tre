# Plan: Phase 3, Step 3.3.2 -- SIMD Path-Morphing Interpolation

## Scope decisions (confirmed with the project owner, 2026-09-06)

Second of Step 3.3's sub-steps (see IMPLEMENTATION.md's "Scope decision"
note under Step 3.3, and Step 3.3.1's own precedent). Covers task 2 only
-- SIMD path-morphing interpolation. Task 3 (stencil-and-cover fallback)
remains deferred to Step 3.3.3.

**Morphs already-flattened `Polygon` vertices, not raw pre-flatten Bezier
control points.** IMPLEMENTATION.md task 2 requires "topological
equivalence (matching number of control points) between keyframes" --
`tre-svg`'s `Polygon` type only holds already-flattened straight-line
points (curves are gone by the time a `Polygon` exists), so "control
points" here means flattened vertices, and "topological equivalence"
means "same vertex count," checked and rejected via `Result` if
mismatched -- not automatically resampled to make them match (arc-length
resampling is a real, separate algorithm, deferred until a caller
actually needs mismatched-topology morphing). This is also how production
shape-morphing tools work in practice (e.g. GSAP's MorphSVG, `flubber`):
resample-then-interpolate-points, which tolerates two keyframes with
genuinely different underlying Bezier authoring, not just coordinate
differences -- a real advantage over requiring literal same-command-type
matching, not just a simpler implementation. The alternative (interpolate
raw Bezier control points, re-flattening every frame) would require a new
pre-flatten path-command type in `tre-svg` alongside `Polygon` and is
more fragile (rejects two shapes that are visually equivalent but
authored with different SVG commands) -- explicitly not built here.

**The SIMD batch-lerp primitive lives in `tre-math`, not `tre-svg`.**
`tre-math`'s own top-of-file doc comment already lists "SIMD-accelerated
path interpolation" among its responsibilities, citing TECHNICAL.md
Sections 2.2/5.4/7.2 -- the same section (5.4) that describes path
morphing. `tre-svg` owns path/polygon-specific concerns (parsing,
flattening, triangulation, topology validation); the actual "interpolate
N point-pairs via `wide::f32x8`" math is domain-agnostic and belongs
alongside `Affine2::compose_batch`, which it mirrors closely: 8-wide
SIMD chunks with a scalar remainder, writing into a caller-provided `out`
slice (DESIGN.md's zero-allocation rule), panicking (not `Result`) on a
length mismatch between `from`/`to`/`out` -- a mismatch there is a
programmer error (the caller controls all three slice lengths directly),
distinct from `tre-svg`'s own topology check, which validates genuinely
untrusted, independently-parsed keyframe data and must report via
`Result` instead.

**No re-flattening needed per animation frame.** Because both keyframes
are flattened once (at parse time, via the existing Step 3.3.1 pipeline)
to the same vertex count, morphing at any `t` is a pure per-vertex lerp
over already-straight-line data -- re-triangulation (via the existing
`triangulate`) is still needed every frame, since the interpolated
shape's actual geometry changes, but curve flattening does not repeat.

**Demo keyframes are both pure straight-line paths (no curves), with the
same literal number of `L` commands.** Two independently-flattened curved
paths could legitimately produce different vertex counts (each curve's
tolerance-based subdivision is independent), which would make the demo's
"topological equivalence" hinge on flattening-tolerance coincidence
rather than the actual property this step demonstrates. Using
straight-line-only keyframes (one point in, one point out, no
subdivision) makes the equal-count guarantee structural, not accidental.

## Goal

Given two independently-parsed SVG keyframe shapes with the same
flattened vertex count, interpolate between them at an arbitrary `t` via
a real SIMD batch operation and render the result -- proven by reading
back actual pixels at an intermediate `t` that differ from both
keyframes' own renders, confirming genuine interpolation happened, not a
snap to one endpoint.

## Tasks

1. **`tre-math::lerp_points_batch(from: &[[f32; 2]], to: &[[f32; 2]], t:
   f32, out: &mut [[f32; 2]])`**: SIMD-batched per-point linear
   interpolation, 8 points at a time via `wide::f32x8::mul_add`
   (`lerp(a, b, t) = (b - a).mul_add(t, a)`), scalar fallback for the
   `len() % 8` remainder -- mirrors `compose_batch`'s exact structure.
   Generalizes the existing private `gather` helper (currently typed to
   `&[Affine2]`) to `gather<T>(items: &[T], field: impl Fn(&T) -> f32)`
   so both functions share it rather than duplicating the same 8-lane
   gather-from-slice logic. Panics (documented, matching `compose_batch`)
   if `from`/`to`/`out` lengths differ.

2. **`tre-svg::SvgError::TopologyMismatch { from_points: usize, to_points:
   usize }`**: new error variant for mismatched keyframe vertex counts.

3. **`tre-svg::morph(from: &Polygon, to: &Polygon, t: f32) ->
   Result<Polygon, SvgError>`** (new `morph.rs` module): validates equal
   vertex counts (returning `TopologyMismatch` if not), then calls
   `tre_math::lerp_points_batch` to produce the interpolated `Polygon`.
   Pure geometry function -- triangulation stays a separate, explicit
   caller step via the existing `triangulate`, matching the crate's
   established parse -> polygon -> triangulate -> vertices pipeline shape
   rather than folding morphing into any of those stages.

4. **New example** (`crates/tre-rhi-vulkan/examples/svg_morph_demo.rs`,
   `demo/phase3_step3_3_2/`): two hand-authored, straight-line-only SVG
   keyframe paths with the same vertex count (e.g. two same-vertex-count
   star-like shapes differing only in coordinates), parsed independently,
   morphed at `t = 0.0, 0.5, 1.0` (and re-triangulated fresh at each,
   since the interpolated shape's geometry genuinely changes),
   rendered through the existing flat-color pipeline, with real pixel
   assertions: `t = 0.0`'s render matches parsing+triangulating `from`
   directly; `t = 1.0`'s matches `to` directly; `t = 0.5`'s interior
   differs from at least one point that is inside one keyframe's shape
   but outside the other's, at that shared coordinate -- proving the
   midpoint frame is a genuine, distinct interpolated shape.

5. **CI**: add `svg_morph_demo` to the `vulkan-validation` job's example
   list.

6. **Unit tests**: `tre-math` -- `lerp_points_batch` against a scalar
   reference across every SIMD remainder length (`0, 1, 7, 8, 9, 16,
   17`, matching `compose_batch`'s own precedent), plus a
   panics-on-mismatched-lengths test. `tre-svg` -- `morph` at `t=0`
   returns exactly `from`, at `t=1` returns exactly `to`, at `t=0.5`
   returns the exact midpoint for a simple hand-computable case, and a
   mismatched-vertex-count pair returns `Err(TopologyMismatch)` rather
   than panicking.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `svg_morph_demo` re-run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- uses the pre-existing flat-color pipeline unmodified.
- All 8 pre-existing Vulkan examples re-run manually, unaffected (this
  step touches no RHI/vertex-format code).
- CI: push, confirm `svg_morph_demo` passes on Mesa lavapipe.

## Explicitly out of scope for this step

- Arc-length resampling to reconcile keyframes with genuinely different
  vertex counts -- a real, separate algorithm; this step validates and
  rejects mismatches via `Result` rather than auto-fixing them.
- Raw Bezier-control-point morphing (re-flattening every frame) -- a
  real, separate technique with a real fragility tradeoff (see scope
  decision above), not what this step builds.
- Multi-keyframe timelines, easing curves, or any animation-clock/frame-
  scheduling concern -- this step proves the per-`t` interpolation
  primitive in isolation, matching the "build the tested primitive
  before its exact consumer exists" precedent already used for
  `SpscRingBuffer`, `tre-math`'s `compose_batch`, and Step 3.3.1's
  tessellator.
- Stencil-and-cover fallback for self-intersecting paths (task 3) --
  Step 3.3.3.
- Wiring into `RenderingCanvas`'s public `Canvas` API -- proven directly
  via a dedicated demo first, per the same precedent.
