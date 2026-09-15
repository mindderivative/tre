# Plan: Phase 4, Step 4.2.2 -- MSDF Glyph Generation

## Scope decisions (confirmed with the project owner, 2026-09-06)

**Uses `fdsm`**, per the project owner's direction from Step 4.2.1's own
planning round (real pure-Rust reimplementation of msdfgen's published
algorithm -- edge coloring, true/pseudo-distance handling, sign
correction -- following Chlumský's thesis rather than a from-scratch
first attempt at this specific, failure-prone algorithm). `fdsm` pulls in
`nalgebra`/`image`/`num-traits` as its own internal, asset-generation-time
dependencies -- already accepted in that same planning round as the same
category as `usvg` pulling in `kurbo`, distinct from this project's own
`tre-math` hot-path primitives.

**Lives in `tre-text`, not `tre-atlas` or a new crate.** DESIGN.md's own
"Multi-Format Atlases" section is explicit that only glyphs use MSDF
($RGB8$) -- icons/vector decals use plain $RGBA8$ -- so MSDF generation is
a text-domain concern, unlike Step 4.2.1's packer (explicitly shared by
both). `tre-text` already owns the `Contour`/`OutlineSegment` data this
step's input is; adding the rasterizer here keeps glyph-geometry-to-pixels
a single crate's responsibility rather than splitting it across a new
crate boundary for no architectural reason.

**No new shader, no GPU at all this sub-step -- a real, deliberate change
of pace from every prior Phase 3/4 sub-step.** IMPLEMENTATION.md's own
task split puts "rasterize glyphs into an RGB8 buffer" (task 2, this
plan) and "implement the MSDF evaluation shader" (task 3) in the same
step, but they're genuinely separable: task 2 produces a CPU-side pixel
buffer, task 3 consumes it on the GPU. Since no shader or pipeline exists
yet that samples an arbitrary texture at all (every existing pipeline is
flat-color, analytical-SDF, or stencil-and-cover -- none textured), doing
any GPU work here would mean inventing throwaway plumbing Step 4.2.3
would then have to build "for real" anyway. This step's own demo is
therefore the first in Phase 3/4 with no Vulkan involvement at all, and
lives in `crates/tre-text/examples/` (not `tre-rhi-vulkan/examples/`,
since it needs no RHI handle whatsoever) -- verified by rendering a
CPU-side preview image via `fdsm::render::render_msdf` (the same
median-of-channels evaluation `msdfgen`'s own reference tooling uses for
exactly this purpose), independently cross-checked against a hand-rolled
median-of-3 computed directly against the raw MSDF bytes before the
preview is trusted.

**Demo glyph: `'O'`, deliberately a hole-having glyph, not another
hole-free letter.** Step 4.1's own "Explicitly out of scope" section
named true multi-contour hole rendering as something ear-clipping
triangulation can't do, deferred to "however Step 4.2's MSDF approach...
ends up handling it." MSDF handles contours/winding natively via the
distance field itself, with no triangulation involved at all -- this is
the step that actually closes that gap, so the demo should prove it on
exactly the case triangulation couldn't.

**Fixed parameters matching this project's own architecture, not
invented:** `32x32` px output (IMPLEMENTATION.md task 2's own stated
resolution), a `4.0`px distance range (the standard `msdfgen` default,
also what `fdsm`'s own worked example in its README uses), and the
`0.03` edge-coloring angle-threshold value from that same README example
(a `sin`-of-angle corner-detection parameter internal to the published
algorithm `fdsm` already implements -- not something this project
re-derives). A uniform (not per-axis) fit-to-target-box transform, since
non-uniform scaling would distort corners/curves and defeat MSDF's whole
"preserve sharp corners" purpose (`Similarity2`, the transform type
`fdsm`'s own example uses, is uniform-scale-only by construction).

## Goal

Given a real glyph's already-extracted outline (`tre_text::Contour`,
Step 4.1), produce a real `32x32` RGB8 multi-channel signed distance
field via `fdsm`, correctly handling a glyph with a true hole (`'O'`) --
proven by an independent, hand-computed median-of-3 check against the raw
distance-field bytes at chosen interior/hole/exterior points, and by a
CPU-rendered preview image visually showing a correctly-filled ring, not
a solid disc (which is what a triangulation-based or naively-signed
approach would wrongly produce for a shape with a hole).

## Tasks

1. **`fdsm` dependency** added to `tre-text`'s `[dependencies]` (pinned to
   the version confirmed compatible with this workspace's `rust-version =
   1.75` via `cargo add --dry-run`, the same technique used throughout
   this project); `png` added to `tre-text`'s `[dev-dependencies]` for
   this step's own demo output only.

2. **Contour conversion**
   (`to_fdsm_contour(contour: &Contour) -> fdsm::shape::Contour`, exact
   name/shape TBD during implementation): walks a `tre_text::Contour`'s
   `OutlineSegment`s the same way `flatten_contour` already does
   elsewhere (tracking a "current point" across `MoveTo`/`LineTo`/
   `QuadTo`/`CubicTo`), emitting one `fdsm::bezier::Segment::line`/`quad`/
   `cubic` per segment with explicit start-and-end points (`fdsm`'s
   `Segment`, unlike this project's own `OutlineSegment`, carries its own
   endpoints rather than relying on an implicit running "current point").
   Real correctness risk named explicitly: `fdsm`'s `Contour` has no
   `Close` marker at all, so a contour whose flattened points don't
   already end exactly back at its own start needs one final explicit
   `Segment::line` closing it -- get this wrong and the shape silently
   has a gap, not an error.

3. **`generate_msdf(contours: &[Contour], size: u32, range_px: f64) ->
   MsdfBitmap`** (exact name/shape TBD): converts every contour, computes
   the glyph's own bounding box directly from the already-extracted
   points (no new dependency on `skrifa`'s own bbox query), builds a
   uniform `Similarity2`-based fit transform centering the glyph within
   `size x size` pixels with `range_px` margin on all sides, applies it,
   runs `Shape::edge_coloring_simple` -> `.prepare()` -> `generate_msdf`
   -> `correct_sign_msdf` (all `fdsm`), and returns a plain
   `MsdfBitmap { width: u32, height: u32, pixels: Vec<u8> }` (RGB8,
   `into_raw()` from `fdsm`'s own `image::RgbImage`) -- the exact
   `RGB8` buffer IMPLEMENTATION.md task 2 calls for, ready for Step
   4.2.3's texture upload.

4. **Unit tests** in `tre-text`: a small, font-independent hand-built
   contour (e.g. a unit square, no font/glyph involved) generating a
   plausible MSDF (a deep-interior sample's median channel value
   comfortably above the `0.5` inside/outside threshold, a
   clearly-exterior sample comfortably below it); a real glyph with a
   hole (`'O'`, extracted via `tre_text::glyph_outline` against a real
   installed font, same discipline as Step 4.1's own font-dependent
   tests) confirming the *hole's own interior* median value sits on the
   *outside* side of the threshold while the ring material around it
   sits *inside* -- the actual property a triangulation-only approach
   cannot express at all.

5. **New example**
   (`crates/tre-text/examples/msdf_generation_demo.rs`,
   `demo/phase4_step4_2_2/`): discovers a real cascade font (reusing
   `tre_text::FontCascade`), extracts `'O'`'s real outline, generates its
   `32x32` MSDF, independently computes (by hand, in the demo's own code,
   not reused from `fdsm`'s private internals) the median-of-3 at a
   deep-interior point, a hole-interior point, and a clearly-exterior
   point, asserts each lands on the correct side of the `0.5` threshold,
   then renders a human-viewable preview (`fdsm::render::render_msdf`
   upsampled well beyond `32x32`, e.g. to `320x320`) to a PNG for visual
   inspection -- a correctly-filled ring, not a solid disc.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace, including `tre-text`'s own `#![forbid(unsafe_code)]` (`fdsm`
  itself is a safe-Rust dependency; nothing in this step's own new code
  needs `unsafe`).
- `msdf_generation_demo` run directly (`cargo run -p tre-text --example
  msdf_generation_demo`), no `xvfb-run`/Vulkan validation layer needed --
  this sub-step touches no RHI code at all. CI: add it as a plain `cargo
  run` step, likely in the existing `test` or `build` job rather than
  `vulkan-validation`, since it needs no virtual display or Vulkan ICD.
- All 13 pre-existing Vulkan examples re-run manually, unaffected (no RHI
  code touched).

## Explicitly out of scope for this sub-step

- The MSDF evaluation shader and any GPU rendering of the generated
  bitmap at all (task 3, IMPLEMENTATION.md's canonical formula in
  TECHNICAL.md Section 5.3) -- Step 4.2.3, which is also where the
  anti-aliasing gap flagged after Step 4.1's demo actually gets resolved.
- Multi-window atlas concurrency (task 4) -- Step 4.2.4.
- Uploading the generated `MsdfBitmap` into a real GPU texture or the
  Step 4.2.1 atlas packer's own space -- proven directly via this
  sub-step's own CPU-only demo first, matching this project's "prove the
  primitive before its real consumer exists" precedent; Step 4.2.3 is
  where a generated MSDF actually reaches the GPU.
- Non-uniform/anisotropic fitting, variable output resolutions other than
  the fixed `32x32`, and any caching/reuse of previously-generated MSDFs
  -- this step generates one bitmap per call, on demand; atlas-level
  bookkeeping is Step 4.2.1's (already built) and 4.2.4's (concurrency)
  job, not this one's.
