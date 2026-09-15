# Demo: Phase 3, Step 3.3.1 -- SVG Ingestion & Ear-Clipping Tessellation

```bash
./demo/phase3_step3_3_1/run_svg_tessellation_demo.sh
```

**What's actually new here:** every prior demo's geometry was hand-authored
directly as `UiVertex` arrays or a `draw_rounded_rect` call. This step
adds a real ingestion path: a new `tre-svg` crate parses genuine SVG text
via the `usvg` crate (which resolves the DOM -- `<use>`/`<g>`/CSS -- and
converts every shape into absolute-coordinate path data, but performs no
rasterization of its own), then hand-rolls the actual tessellation this
project owns: Bezier curve flattening (recursive de Casteljau
subdivision) and ear-clipping triangulation. The output is plain
`UiVertex` triangles, rendered through the pre-existing flat-color
pipeline -- a triangle soup has no SDF to evaluate, so this step needed
no new shader at all.

**Verified by reading back actual pixels:** the star's center is exactly
the fill color, and a point in one of its concave notches (inside the
star's bounding box, but outside the actual polygon) is exactly the
background -- proving the triangulation is topologically correct, not
just "some triangles got drawn somewhere."

**Two real bugs found and fixed while building this demo** (see
REVIEW.md's "Phase 3 Step 3.3.1 Implementation" section for the full
account): a symmetric square/L-shape triangulate correctly by pure area
and triangle-count checks even when the *individual* triangles are
wrong, because (1) an internal index-remapping bug silently returned
triangle indices valid against a reversed working copy rather than the
caller's own point array, and (2) the ear-validity check needed BOTH "no
remaining vertex strictly inside the candidate triangle" AND "no
remaining edge properly crosses the diagonal" -- either check alone
missed a real class of invalid ear. This non-convex star is exactly the
shape that exposed both: a symmetric shape's total area can stay
suspiciously close to correct even while specific triangles are wrong,
which is why this step's own `tre-svg` unit tests were strengthened to
also check that a known concave-notch point is covered by no triangle,
not just that the total area and triangle count come out right.

**Only simple, single-contour, non-self-intersecting fills this step** --
holes, strokes, gradients, and self-intersecting paths are all explicitly
out of scope; see `planning/archive/PLAN_PHASE3_STEP3_3_1.md`.
