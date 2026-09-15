# Plan: Phase 10 Step 10.2 — Full Shape Rendering Support

**Renumbering:** the current Step 10.2 ("The tre-ffi C-ABI Crate") becomes
**10.3**; the current Step 10.3 ("Python UI Framework Bindings") becomes
**10.4**. Rationale: shapes are the thing FFI/Python will actually expose —
finishing real rendering for all four primitives before building a
cross-language boundary around them avoids binding an API surface that's
still 3/4 stubbed.

## User request (verbatim)

> Let's get full rendering support for Shapes fully completed. Build the new
> Bezier-flatting requirements, and get the shape creation completely
> functional. This will be the backbone of UI frameworks and a large portion
> of the whole purpose of this project. Use your best judgement for any
> questions, keeping in mind performance, flexibility, and the goal of the
> project (high-performance desktop GUI's). I have given you full permissions
> to complete this task till finished. Make sure to update the documentation
> and resolve issues the best you can.

## Investigation findings (real, checked before writing this plan)

1. **Bezier flattening already exists.** `crates/tre-svg/src/flatten.rs`
   has real `flatten_cubic`/`flatten_quad` (tolerance-based recursive de
   Casteljau, already `pub` and reused by `tre-text`). REVIEW.md finding
   #161 (Step 10.1) was right that nothing wires `shapes::PathCommand` to
   this — but wrong to imply the curve math itself needs building from
   scratch. The real gap is the wiring layer: `PathCommand` → flattened
   point lists → `tre_svg::Polygon` → `triangulate`/`fan_triangles`.
2. **`tre_svg::triangulate`/`fan_triangles`/`to_ui_vertices` are directly
   reusable** for Path and Polygon fill — ear-clipping triangulation
   already exists and already emits `UiVertex`/index buffers.
3. **The hard constraint is `UiVertex`.** `#[repr(C, align(16))]`, hard
   32-byte invariant (`const _: () = assert!(size_of::<UiVertex>() == 32)`,
   ARCHITECTURE.md Section 3.1), `params: [f32; 3]`. `draw_rounded_rect`'s
   own doc comment already says this out loud: "uniform across all 4
   vertices since the vertex format has no per-quad channel." Non-uniform
   corner radii (4 floats) + border (thickness + color) + corner smoothing
   + gradient parameters cannot fit in 3 floats, and growing `UiVertex`
   itself would bloat every pipeline in the system (text, flat tessellated
   fills, blur) that doesn't need any of this data, and would ripple
   through every vertex producer in the codebase.
4. **The fix: reuse the bindless pattern that already exists for
   textures**, not a vertex-format change. `VulkanDevice`'s bindless
   descriptor set (`crates/tre-rhi-vulkan/src/lib.rs` ~line 630) already
   has binding 0 (immutable sampler) and binding 1 (unbounded
   `SAMPLED_IMAGE` array). Adding **binding 2: one `STORAGE_BUFFER`**
   holding a per-frame array of `GpuShapeStyle` records, referenced by a
   style index bit-cast into `UiVertex.params[0]` (`floatBitsToUint` in
   the shader), gives effectively unlimited per-shape style data with
   zero change to `UiVertex`'s size/layout and zero impact on pipelines
   that don't use it. This is the same "index into a big GPU-resident
   array instead of duplicating data per-vertex" idea the bindless texture
   array already uses — real precedent in this exact codebase, and
   strictly better for GPU bandwidth than duplicating radii/thickness
   across 4 vertices per shape.
5. **`PipelineRegistry`/`RhiPipelineState`/`execute_frame`
   (`tre-engine`) are already generic** (`HashMap<u16, Box<dyn
   RhiPipelineState>>`), so registering new pipeline kinds (ellipse SDF,
   gradient-aware tessellated fill, stroke) is straightforward and
   precedented — no executor changes needed beyond dispatch, which already
   works for arbitrary registered ids.
6. **Stencil-and-cover (`create_stencil_and_cover_pipelines`) is a real,
   working two-pass technique** (Step 3.3.3) but is driven manually by its
   demo, not through the single-`DrawGeometry`-command `execute_frame`
   model — wiring a two-pass, per-shape-stencil-clear technique into the
   generic single-command IR is real, separate scope. Explicitly deferred
   (see "Out of scope" below), not silently dropped.
7. **`BlendMode` (Multiply/Screen/Overlay/SoftLight/ColorDodge) has no
   renderer support today** — Vulkan fixed-function blend state only
   expresses a subset of these exactly; the rest need framebuffer-read
   shader support (a distinct, real RHI capability) this project doesn't
   have yet. Explicitly deferred.

## Scope decisions (my judgment calls, per "use your best judgement")

**In scope — real, working, tested:**
- New bindless `STORAGE_BUFFER` binding + per-frame `GpuShapeStyle` bump
  buffer (persistent-mapped, reset each frame like the existing dynamic
  ring buffers).
- **Rectangle**: non-uniform 4-corner radii, real single-pass border (SDF
  offset-distance stroke — exact, since a box SDF is a true distance
  field, not an approximation), corner smoothing (superellipse-blend
  approximation, explicitly *not* claimed to be bit-identical to any
  specific reference implementation's squircle algorithm).
- **Circle/Ellipse**: new dedicated SDF pipeline (own shader pair), exact
  circle SDF, IQ's ellipse SDF approximation for non-uniform radii, same
  border technique, `arc_length` via an angular-sector cutoff (hard-edged
  at the cut — no rounded stroke caps on partial arcs this pass,
  disclosed).
- **Polygon** (regular N-gon and star): CPU-side procedural vertex
  generation (exact trig, no tessellation-quality tradeoffs), fan
  triangulation via `tre_svg::fan_triangles` (valid because a regular/star
  polygon generated this way is star-shaped w.r.t. its own center by
  construction), border via a new shared stroke tessellator.
- **Path**: `PathCommand` → `tre_svg::flatten_cubic`/`flatten_quad` →
  `tre_svg::Polygon` subpaths → `triangulate()` for fill; the same stroke
  tessellator for stroking, honoring `stroke_line_cap`/`stroke_line_join`.
- **Stroke tessellator** (shared by Polygon + Path): per-segment offset
  quads; Bevel and Round joins (both robust for any angle); Miter with a
  bounded miter-limit that falls back to Bevel past the limit (standard
  technique); Butt/Square/Round caps for open paths.
- **Gradients**: `FillStyle::Gradient(GradientId)` becomes real — linear
  and radial, up to 8 stops, stored in the same style buffer, evaluated in
  linear color space (converted from each stop's sRGB before interpolating
  — consistent with this codebase's existing `srgb_to_linear` discipline)
  for both SDF shapes and tessellated fills.
- **Texture fill**: `FillStyle::Texture(u32)` wired to the existing
  bindless texture array via bounding-box-mapped UVs, for all four
  primitive kinds.
- **Hit-testing**: `ShapeRegistry::hit_test(point) -> Option<ShapeId>`,
  topmost-first, honoring `hit_testable`/`Visibility`/transform —
  currently a dead field on every shape; this is the actual "backbone of
  UI frameworks" need (routing clicks/hover) the user's message names.

**Explicitly out of scope this pass (disclosed, not silently dropped):**
- `BlendMode` beyond `Normal` — needs new RHI framebuffer-read capability.
- Self-intersecting `Path` fills (stencil-and-cover fallback) — needs
  generic multi-pass-per-shape wiring into `execute_frame`, real separate
  scope. `triangulate()`'s `NotSimplePolygon` case will surface as a clear,
  documented panic/error from the flattening pass, not silent wrong output.
- Conic/angular gradients.
- Rounded stroke caps on partial-arc `Circle`s (`arc_length < TAU`).

## Tasks

1. `GpuShapeStyle` GPU buffer: Rust struct (`tre-engine`), Vulkan storage
   buffer + descriptor binding 2 (`tre-rhi-vulkan`), per-frame bump
   allocator, wired into `RenderingCanvas`'s existing per-frame reset path
   (`RenderingCanvas::reset()` from Step 9.2).
2. Rectangle: extend `sdf_rounded_rect.frag`/`.vert` for non-uniform radii
   + border + corner smoothing, sourced from the style buffer via a
   bit-cast style index in `params[0]`. New `Canvas` method for the
   styled path; old `draw_rounded_rect` untouched for existing callers.
3. Circle/Ellipse: new `sdf_ellipse.vert`/`.frag`, `PipelineKind::SdfEllipse`,
   registration in every demo/example that builds a `PipelineRegistry`.
4. Gradient evaluation: shared GLSL gradient-stop-array eval, wired into
   both SDF shaders and a new gradient-aware tessellated-fill shader.
5. Stroke tessellator module (`tre-engine` or `tre-svg`, TBD by what it
   needs to depend on) — joins, caps, miter limit.
6. Polygon: procedural vertex generation + fill + stroke wiring in
   `ShapeRegistry::flatten_into`.
7. Path: `PathCommand` flattening → fill + stroke wiring in
   `ShapeRegistry::flatten_into`.
8. Texture fill wiring for all four primitives.
9. Hit-testing: point-in-rect/ellipse/polygon/path, `ShapeRegistry::hit_test`.
10. Tests: unit tests for stroke tessellation, polygon generation, gradient
    stop evaluation, hit-testing, style-buffer bump allocation; a real GPU
    demo exercising all four primitives with borders/gradients/textures/
    hit-testing, pixel-verified where feasible (same discipline as
    `shape_registry_demo`).
11. Documentation: ARCHITECTURE.md Section 7 rewrite, IMPLEMENTATION.md
    Step 10.2 write-up (and renumber 10.2→10.3, 10.3→10.4 throughout),
    TECHNICAL.md (new pipelines, new descriptor binding), REVIEW.md new
    findings for any real issues hit + the explicit out-of-scope list,
    Build Tracker update.

## Verification plan

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo fmt --all -- --check`, full demo regression sweep, new
GPU demo run + pixel sanity checks, CI green (existing two known-red gates
excepted).
