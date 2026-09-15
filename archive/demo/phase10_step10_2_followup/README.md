# Demo: Phase 10 Step 10.2 Follow-up -- Real Tessellation via `lyon`

```bash
./demo/phase10_step10_2_followup/run_path_and_polygon_demo.sh
```

**Closing the gap Step 10.2 disclosed.** Step 10.2 shipped full
rendering support for `Rectangle`/`Circle`/`Ellipse` and `Polygon` fill,
but disclosed two real, unresolved gaps: `Path` fill had no rendering
path at all (the general triangulator this engine would reuse,
`tre_svg::triangulate`, was unreachable from `tre-engine` without a
circular dependency on `tre-svg` itself), and neither `Polygon` nor
`Path` had any border/stroke rendering. This follow-up closes both, by
adopting [`lyon`](https://github.com/nical/lyon) -- the industry-standard
Rust 2D tessellation library -- as the real tessellation backend for
both `tre-svg` (replacing its own hand-rolled ear-clipping triangulator
and stencil-and-cover GPU fallback) and `tre-engine` (real `Path` fill
and stroke, for the first time).

**What this demo proves.** Two shapes, entirely recorded through
`ShapeRegistry::insert` + `ShapeRegistry::flatten_into`:

- A "donut" `Path` -- two subpaths (an outer 100x100 square, an inner
  40x40 "hole" square), wound in opposite directions so `lyon`'s real
  fill tessellator resolves the inner one as a genuine subtractive hole.
  This is exactly the compound-shape case the project's old hand-rolled
  ear-clipper could never express at all (it only ever handled a single
  simple contour). Real pixel samples confirm the ring itself fills, the
  hole stays background (not filled), and the outer edge's own stroke
  renders in `border_color`.
- A bordered hexagon `Polygon` -- fill via the existing from-center
  triangle fan (unaffected by this change; a regular polygon is already
  star-shaped with respect to its own center, so it never needed a
  general tessellator), with a real border/stroke via `lyon`'s
  `StrokeTessellator` -- previously entirely unbuilt for any shape kind.

**Why `lyon`, not another hand-rolled fix.** The project's own
established "build the primitive, don't reach for a crate" precedent
was deliberately set aside here: 2D path tessellation (fill with
self-intersection/hole/winding-rule handling, plus stroke joins/caps/
miter limits) is a deep, well-solved problem domain with a mature,
actively-maintained, dominant Rust-ecosystem answer, the same category
this project already treats `usvg` (SVG parsing) and `wide` (SIMD) as
being in -- not a core-identity primitive like the sort/atlas/arena work
this project does still hand-roll. The hand-rolled ear-clipper's own
history (three separately hard-won bug fixes, each found only via a real
GPU demo's pixel readback, not its own unit tests) is real evidence this
is exactly that kind of hard-to-get-right domain.

**Verified.** `tre-svg`'s own new `tessellate` module (replacing
`triangulate.rs`/`stencil.rs`) has 5 tests, including a real
self-intersecting pentagram tessellating correctly (the old ear-clipper
rejected it outright) and a real ring-with-a-hole. `tre-engine` gained
matching real fill/stroke tests plus real `ShapeRegistry::flatten_into`
coverage for both `Polygon` and `Path` borders. Every pre-existing SVG/
text-shaping demo (`svg_tessellation_demo`, `svg_morph_demo`,
`text_shaping_demo`) still passes unchanged after migrating to the new
API. `self_intersecting_fill_demo` (renamed from `stencil_and_cover_
demo`, its retired technique's own namesake) proves the same pentagram
from the original stencil-and-cover proof now tessellates directly, no
GPU fallback technique needed. Full workspace `fmt`/`clippy -D
warnings`/`build`/`test` clean.
