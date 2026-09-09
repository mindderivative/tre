# Demo: Phase 10, Step 10.2 -- Full Shape Rendering Support

```bash
./demo/phase10_step10_2/run_shape_full_rendering_demo.sh
```

**Shapes, made actually usable as the backbone of a UI framework.** Step
10.1 shipped the shape-primitive data model and real rendering for
exactly one narrow case (a uniform-radius, borderless `Rectangle`). This
step makes real rendering support much broader: `Rectangle` (any corner
radii, real borders, corner smoothing) and `Circle`/`Ellipse` (borders,
a real partial-arc sweep) both render for real now, `Polygon`/`Star`
fill for real, and `Path`'s Bezier-flattening math is real and tested.

**What this demo proves.** One scene, entirely recorded through
`ShapeRegistry::insert` + `ShapeRegistry::flatten_into` (never a
hand-written RHI call): a `Rectangle` with a *sharp* (radius 0)
top-left/bottom-right corner pair, a *rounded* (radius 40) top-right/
bottom-left pair, and a real 8px border; plus a bordered, 270-degree
(three-quarter) `Circle`. Real pixel samples assert:

- The sharp corner renders right up to the corner -- no rounding.
- The rounded corner is genuinely excised by its own 40px arc, at a
  point that sits inside the rectangle's raw bounding box but outside
  the rounded corner's real curve -- a render using only one shared
  radius could not pass both checks at once.
- The border band renders `border_color`; the interior renders
  `fill_color` -- on both shapes.
- The circle's own excluded (northwest) wedge, beyond its 270-degree
  sweep, renders background, not fill -- proving `arc_length` is a real
  cutoff, not just accepted and ignored.

**A real GPU bug found and fixed while building this.** The style-buffer
word index a vertex uses to reference its own `GpuRectStyle`/
`GpuEllipseStyle` record was originally bit-cast into a `UiVertex.params`
float slot -- for small indices this produces a *subnormal* float, and
real hardware silently flushed it to `0.0` (a well-documented GPU
optimization), so the fragment shader read the wrong shape's style data
entirely. Found by observing wrong pixels (the circle's own visible
radius, ~10px against a requested 50px, exactly matched what reading the
*rectangle's* style data as the circle's own fields would produce), not
by inspection. Fixed by carrying the index numerically instead --
`crates/tre-engine/src/gpu_style.rs`'s own doc comment has the full
account; REVIEW.md finding #162 records it.

**What's disclosed, not silently missing.** `Polygon`/`Path` border
(stroke) rendering has no tessellator built yet; `Path` *fill* rendering
has no rendering path at all (a real architectural blocker -- the
triangulator this engine would reuse, `tre_svg::triangulate`, is
unreachable from `tre-engine` without a circular crate dependency,
REVIEW.md finding #163); `FillStyle::Gradient`/`Texture` and non-`Normal`
`BlendMode` remain unbuilt for any shape kind. See
`documentation/ARCHITECTURE.md` Section 7.5's own "Implementation
status" note for the full, itemized disposition.

**Verified.** 25 new `tre-engine` unit tests (119 total, up from 94) and
5 new `tre-math` tests (`Affine2::invert`) -- style-buffer byte-layout
round-trips, the corrected numeric word-index encoding, polygon/star
point generation, Bezier flattening, and real hit-testing for all four
shape kinds (including a rounded-corner exclusion test, an arc-wedge
exclusion test, and a star-polygon concave-notch exclusion test). This
demo's own 8 pixel-correctness assertions ran clean across repeated real
GPU runs; `shape_registry_demo.rs` (Step 10.1's own byte-for-byte
equivalence proof) still passes unchanged, confirming the original
`draw_rounded_rect` path is untouched. Full workspace `fmt`/`clippy -D
warnings`/`build`/`test` clean.
