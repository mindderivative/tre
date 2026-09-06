# Plan: Phase 3, Step 3.3.1 -- SVG Ingestion & Ear-Clipping Tessellation

## Scope decisions (confirmed with the project owner, 2026-09-06)

IMPLEMENTATION.md's Step 3.3 bundles four largely independent chunks of
work (a tessellator, SIMD path-morphing, a stencil-and-cover fallback for
self-intersecting paths, and untrusted-input hardening) -- a much larger
unit than Steps 3.1/3.2, comparable in total scope to all of Phase 2. Per
the project owner's direction, this is split into sub-steps matching Phase
2's own 2.1-2.4 precedent; this plan covers only the first, 3.3.1: SVG
ingestion and ear-clipping tessellation of simple (non-self-intersecting)
polygons. 3.3.2 (SIMD path morphing), 3.3.3 (stencil-and-cover fallback),
and any remaining hardening beyond what this step already covers will each
get their own plan.

**SVG parsing uses the `usvg` crate, not a hand-rolled XML/path-data
parser.** No SVG parsing crate is named anywhere in this project's docs,
and the project otherwise hand-rolls its own primitives (`tre-math`
instead of `nalgebra`/`glam`, etc.) -- but writing a real SVG 2.0 XML
subset parser (tokenizing the `d` attribute's own mini-language, resolving
`<g>`/`<use>`, converting elliptical arcs to cubic Beziers) is itself a
large, separate undertaking with a long tail of interop edge cases a
mature library has already solved. Evaluated three options the project
owner named: `resvg` was rejected because it fully rasterizes to a bitmap
via `tiny-skia`'s own software rasterizer -- pulling it in would bypass
this project's entire purpose (a from-scratch, GPU-tessellated 2D
renderer) rather than serve it. `oxvg` was rejected because it's a DOM
optimization/linting toolchain (SVGO-equivalent), not built to feed a live
rendering pipeline. `usvg` -- the same project `resvg` is built on --
stops exactly where TRE's own work begins: it resolves the DOM (including
`<use>`/`<g>`/CSS), converts every shape to absolute-coordinate path data
(arcs already converted to cubic Beziers), and does no rasterization at
all. TRE tessellates and renders the geometry it hands back through its
own pipeline, same as it always has.

Pinned to `usvg = "=0.45.1"` (the newest version this workspace's
`rust-version = 1.75` can resolve -- 0.48.1 needs rustc 1.85, confirmed via
`cargo add --dry-run`, the same technique used to pin `wide` in Step 3.1)
with `default-features = false`, dropping the `text`/`system-fonts`/
`memmap-fonts` features (font loading, `rustybuzz` text shaping) entirely
-- text rendering is Phase 4's HarfBuzz integration, a wholly separate
concern this crate shouldn't pull in early. Confirmed by building a scratch
project against it: the no-default-features dependency tree is
`roxmltree` (XML), `tiny-skia-path` (the `Path`/`PathSegment` geometry type
usvg exposes), `kurbo` (curve math), `svgtypes` (attribute value parsing),
`simplecss` (CSS selector support), plus small support crates (`base64`,
`flate2` for `.svgz`, etc.) -- no font/text machinery.

**usvg already hardens against the exact untrusted-input risks
IMPLEMENTATION.md task 4 names, verified by reading its actual source
(`~/.cargo/registry/.../usvg-0.45.1/src/parser/`), not assumed from its
reputation:**
- `Error::ElementsLimitReached` -- a hard cap of 1,000,000 total elements,
  checked during parsing.
- `Error::NodesLimitReached` -- a hard cap of 1024 on nesting depth
  (covers both `<g>` nesting and `<use>` chains, via one shared
  recursion-depth counter threaded through `parse_xml_node`).
- Explicit `<use>` cycle detection (`parse_svg_use_element` in
  `src/parser/svgtree/parse.rs`): direct self-reference, indirect
  reference back to an ancestor `use`, and the specific "sibling `use`
  elements referencing each other's ancestor" case are all detected and
  the offending `<use>` is silently skipped (logged, not an error) rather
  than infinitely recursing.
- All of the above surfaces as `Result<Tree, usvg::Error>` from
  `Tree::from_str`/`from_data` -- never a panic, never an unbounded loop.

This satisfies the bulk of task 4's stated concern already, via a mature,
widely-deployed library rather than a hand-rolled reimplementation of the
same protections. What usvg does *not* cap is the raw input byte size
(before parsing even starts) or the *total resolved path point count*
across the whole tree (a depth- and element-count-bounded document can
still resolve to a very large number of path vertices if, e.g., many
sibling `<path>` elements each have thousands of points) -- both real,
cheap additions this step adds on top: a byte-size ceiling checked before
calling into `usvg` at all, and a point-count ceiling checked by walking
the already-parsed tree before tessellation begins, both returning
`Result`, matching this project's `EngineError` pattern rather than
panicking.

**A new crate, `tre-svg`, not code inside `tre-engine`.** Matches this
project's existing precedent of a new capability domain getting its own
crate (`tre-math` for the vector math engine) -- keeps `usvg`'s dependency
tree (`roxmltree`, `kurbo`, etc.) out of `tre-engine`'s graph, and keeps
`tre-engine`'s `forbid(unsafe_code)`/zero-dependency-bloat policy for the
per-frame path unaffected by a parsing-time-only dependency. Matches
DESIGN.md's own architecture diagram, which already draws "SVG
Tessellation" as a sibling box next to "Rendering Canvas API" and "Dynamic
Texture Atlas," not folded into either.

**Ear-clipping only this step, for simple (non-self-intersecting,
`NonZero`-fillable) polygons -- not the general case.** IMPLEMENTATION.md
task 1 names "ear-clipping or trapezoidal mapping" (either satisfies it);
task 3 already carves out the harder case (self-intersecting paths needing
`EvenOdd`) as its own, separate stencil-and-cover fallback -- a strong
signal the two were always meant to be separate mechanisms, which maps
cleanly onto splitting them into separate sub-steps. This step's
tessellator handles the common case (the vast majority of real-world
icon/logo paths); a path this algorithm can't safely triangulate is
reported via `Result`, not guessed at -- Step 3.3.3 is where that fallback
path gets built.

**Curve flattening is real, hand-rolled work, not something borrowed from
usvg/kurbo.** `usvg::Path::data()` exposes `tiny_skia_path::PathSegment`s
(`MoveTo`/`LineTo`/`QuadTo`/`CubicTo`/`Close`, arcs already pre-converted
to cubics by usvg's own parser) -- straight-line segments an ear-clipping
triangulator needs, not curves. Converting `QuadTo`/`CubicTo` into a
polyline via recursive de Casteljau subdivision (flatness-tolerance-based,
not a fixed segment count, so a large curve gets more segments than a
small one) is exactly the kind of tessellation-core algorithm this project
owns itself, matching the `wide`-not-`nalgebra` precedent: use an external
library for the plumbing (XML parsing, DOM resolution) it makes no sense
to reinvent, but keep the actual geometry algorithm this phase exists to
build in-house.

**Output feeds the existing flat-color pipeline -- no new shader needed
this step.** A triangulated polygon has no SDF to evaluate; it's exactly
the primitive `walking_skeleton.{vert,frag}`'s flat vertex-color path
already renders (per-vertex `UiVertex::color`, no `uv`/`params`
involvement). This step's tessellator emits plain `UiVertex` triangles
directly usable by that existing pipeline, so the demo proving this step
needs no shader work at all -- a genuine change of pace from Step 3.2,
similar to how Step 3.1 was a genuine change of pace from the GPU-heavy
Phase 2 steps.

## Goal

Given real SVG path data (parsed via `usvg`, not hand-authored `UiVertex`
arrays), produce a correct triangle mesh for simple polygonal shapes and
render it through the existing flat-color pipeline -- proven by reading
back actual rendered pixels, plus unit tests checking the tessellator's
triangle output against a known-correct area (the shoelace formula) for
hand-picked polygons a human can verify by hand.

## Tasks

1. **New `tre-svg` crate** (`crates/tre-svg`), added to the workspace
   `Cargo.toml`. Depends on `usvg = "=0.45.1"` (`default-features =
   false`). `#![forbid(unsafe_code)]`, matching `tre-engine`/`tre-math`'s
   existing policy -- nothing in ear-clipping or curve flattening needs
   `unsafe`.

2. **`parse_svg(source: &[u8], max_bytes: usize, max_points: usize) ->
   Result<Vec<Polygon>, SvgError>`** (exact name/shape TBD during
   implementation): rejects `source.len() > max_bytes` before calling
   `usvg::Tree::from_str`, maps `usvg::Error` to a new `SvgError` enum
   (never panics), walks the parsed tree collecting every `Path` node's
   fill data, flattens curves to polylines (recursive de Casteljau,
   flatness-tolerance-based), and rejects the whole document via
   `SvgError::TooManyPoints` if the running total point count exceeds
   `max_points` -- checked incrementally during the walk, not after fully
   resolving a pathological document first.

3. **Ear-clipping triangulator**: `triangulate(polygon: &Polygon) ->
   Result<Vec<[u32; 3]>, SvgError>` (indices into the polygon's point
   list) for a single-contour, non-self-intersecting polygon. Returns
   `SvgError::NotSimplePolygon` (or similar) for input it can't safely
   handle -- detected via a real ear-validity check (no other polygon
   vertex inside the candidate ear triangle), not assumed. Multi-contour
   paths (holes) are walked as independent polygons this step -- true hole
   support (subtracting an inner contour from an outer one) is real,
   separate work, out of scope here (see below).

4. **`UiVertex` emission**: convert triangulated output into
   `Vec<UiVertex>`/`Vec<u32>` (flat color per path, `uv`/`params` zeroed,
   matching every pre-Step-3.2 flat-color quad's convention), ready for
   the exact same `upload_buffer`/`draw_indexed` path every existing
   example already uses.

5. **New example** (`crates/tre-rhi-vulkan/examples/svg_tessellation_demo.rs`,
   `demo/phase3_step3_3_1/`): parses a small, hand-authored SVG string
   containing one simple non-convex polygon path (e.g. a five-pointed star
   -- non-convex, so ear-clipping's real behavior is actually exercised,
   not just the trivial convex case), tessellates it via `tre-svg`,
   renders through the existing flat-color pipeline, and reads back real
   pixels: a point deep inside the star is the fill color, a point in one
   of the star's concave notches (outside the polygon despite being inside
   its bounding box) is the background -- proving the triangulation is
   topologically correct, not just "some triangles got drawn somewhere."

6. **Unit tests** in `tre-svg`: ear-clipping triangle count/total-area
   checks (shoelace formula) against hand-picked polygons (a square, an
   L-shape, the star used in the demo); curve-flattening tolerance tests
   (a quarter-circle cubic Bezier approximation stays within a known error
   bound of the true circle); the byte-size and point-count hardening
   caps, exercised with a deliberately oversized/many-point synthetic
   input (not usvg's own depth/element caps -- those are already usvg's
   own, separately-maintained, separately-tested code, not this step's to
   re-verify).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace, including `tre-svg`'s own `#![forbid(unsafe_code)]`.
- `svg_tessellation_demo` re-run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- it uses the pre-existing flat-color pipeline unmodified, so
  this is mostly confirming no regression, not new Vulkan surface area.
- All 8 pre-existing Vulkan examples re-run manually, unaffected (this
  step touches no RHI/vertex-format code at all).
- CI: add `svg_tessellation_demo` to the `vulkan-validation` job's example
  list; push, confirm green.

## Explicitly out of scope for this step

- SIMD path-morphing interpolation (IMPLEMENTATION.md task 2) -- Step
  3.3.2.
- Stencil-and-cover fallback for self-intersecting/`EvenOdd` paths (task
  3) -- Step 3.3.3.
- True multi-contour hole support (subtracting an inner contour from an
  outer one via a bridge-edge or similar technique) -- a real, separate
  algorithm; this step triangulates each contour independently, which is
  wrong for shapes with holes (a hole would render filled, not cut out).
  Documented explicitly rather than silently producing wrong output on
  such input -- `parse_svg` in this step's scope only handles
  single-contour paths correctly.
- Stroke rendering, gradients, patterns, clip-paths, masks, filters, and
  any other `usvg`-modeled paint/compositing feature beyond plain solid
  fill -- this step proves the tessellation primitive, not a
  general-purpose SVG renderer.
- Wiring into `RenderingCanvas`'s public `Canvas` API (a `draw_svg`/
  `DrawPath` IR command) -- proven directly via a dedicated demo first,
  matching this project's own "prove the primitive before its real
  consumer exists" precedent (`SpscRingBuffer`, `tre-math`'s
  `compose_batch`).
- Tightening usvg's own 1024-depth/1,000,000-element caps further --
  those are real, already-enforced limits; revisit only if a concrete
  reason (e.g. a memory budget for icon-sized assets) emerges.
