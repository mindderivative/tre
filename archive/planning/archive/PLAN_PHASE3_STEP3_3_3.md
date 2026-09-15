# Plan: Phase 3, Step 3.3.3 -- Stencil-and-Cover Fallback Rendering

## Scope decisions (confirmed with the project owner, 2026-09-06)

Third and last of Step 3.3's sub-steps -- covers task 3: "Implement the
stencil-and-cover fallback rendering method for path intersections that
fail simple ear-clipping (e.g., self-intersecting paths with `EvenOdd`
fill rules)."

**Both fill rules (`NonZero` and `EvenOdd`), not just `EvenOdd`.** The
task's own example names `EvenOdd` specifically (a single `INVERT`
stencil op per fan triangle, no per-triangle winding needed), but the
project owner chose full support for both. `NonZero` needs two-sided
stencil ops (`INCR_WRAP` for front-facing fan triangles, `DECR_WRAP` for
back-facing ones) -- real, separate GPU pipeline state from `EvenOdd`'s
single `INVERT` op, but the *cover* pass (testing `stencil != 0`,
resetting to `0` on pass) is identical for both fill rules; only the
*stencil* pass's op configuration differs. Note on correctness: which
physical winding direction gets `INCR_WRAP` vs `DECR_WRAP` does not
affect the `NonZero` test's correctness -- `!= 0` is symmetric under
swapping which side increments vs decrements, as long as front and back
get opposite non-identity ops. The classic proof case (a self-intersecting
pentagram, task's own kind of example) renders *differently* under the
two rules -- `EvenOdd` leaves the central pentagon unfilled (crossed an
even number of times), `NonZero` fills it (winding number 2, nonzero) --
giving a real, decisive, textbook pixel check for both.

**Stencil support becomes a permanent part of the shared RHI surface
(`begin_frame`/`create_pipeline`), not a demo-local, self-contained
proof.** Checked the current code first: `begin_frame`'s dynamic-rendering
setup has no stencil attachment at all today, and it is the one shared
per-frame path all 9 existing examples already go through. Per the
project owner's direction, this step makes stencil a first-class,
always-present part of the universal per-frame rendering setup --
matching the existing "declare it everywhere, unused shaders/pipelines
ignore it" precedent already used for the bindless descriptor set and the
12-byte push-constant range. Concretely: every `VulkanSwapchain`/
`HeadlessSwapchain` gains its own stencil image (sized to its own
extent, mirroring how each already owns its own color image), `begin_frame`
always attaches it (`AttachmentLoadOp::CLEAR` once per frame; individual
shapes never need a mid-frame clear, since the cover pass's own
`pass_op = ZERO` keeps the buffer clean between shapes), and
`create_pipeline`'s existing pipelines each get a small, additive,
non-breaking internal change (declaring a matching `stencilAttachmentFormat`
in `PipelineRenderingCreateInfo`) so dynamic rendering's attachment-format
compatibility rules are satisfied -- `create_pipeline`'s *public signature*
and every existing call site stay unchanged; only its internal pipeline-info
construction gains one line. This is real, if contained, shared-surface
risk: every existing example must be re-verified after this change, not
just the new one.

**No window-resize handling needed.** Checked: this project has no
swapchain-resize/recreate logic today, so the per-swapchain stencil
image, created once alongside the swapchain's color image, needs no
resize-time recreation path either.

**A genuinely self-intersecting pentagram, not a strawman shape.** The
demo path connects five circle points in `0, 2, 4, 1, 3` order (the
classic pentagram construction) -- its own boundary crosses itself five
times. `triangulate()` is expected to genuinely return
`Err(SvgError::NotSimplePolygon)` on it (confirmed as part of this step's
own verification, not assumed), proving this is a real case ear-clipping
cannot handle, motivating the fallback this step builds -- not a shape
chosen to make the fallback look necessary.

**No new shader.** Both the stencil pass (color writes masked off,
`ALWAYS`-passing stencil test that only *writes*) and the cover pass
(normal color writes, a `NOT_EQUAL 0` stencil *test* that also resets to
`0` on pass) reuse the existing `walking_skeleton` flat-color vertex/
fragment shader pair -- the difference between this technique and every
prior step's rendering is entirely in pipeline *state* (blend/stencil/
color-write configuration across two separate `VkPipeline` objects), not
shader code.

**CPU-side fan/bbox helpers live in `tre-svg`, reusing the existing
`Polygon` type.** `fan_triangles(polygon: &Polygon) -> Vec<[u32; 3]>`
(anchor at vertex 0, fan to every edge -- always succeeds, no validity
check needed, since overlap/self-intersection is exactly what the GPU's
stencil accumulation is designed to handle correctly) and
`bounding_box(polygon: &Polygon) -> ([f32; 2], [f32; 2])` (for the cover
pass's quad) sit alongside `triangulate`/`morph`, giving a clean
"`parse_svg` -> `Polygon` -> try `triangulate`, fall back to
`fan_triangles` + stencil-and-cover" shape at the call-site level.

## Goal

Render a real, genuinely self-intersecting path (which `triangulate`
provably cannot handle) correctly under both `NonZero` and `EvenOdd` fill
rules via a real two-pass stencil-and-cover GPU technique -- proven by
reading back actual pixels showing the two fill rules produce their
textbook-different results (the pentagram's center filled under
`NonZero`, empty under `EvenOdd`), and by confirming all 9 pre-existing
examples still render correctly after this step's shared-RHI-surface
changes.

## Tasks

1. **`tre-engine`**: add `pub enum FillRule { NonZero, EvenOdd }`
   (shared, backend-agnostic type, matching `TextureFormat`'s placement).
   Extend the `RhiSwapchain` trait with a new opaque-handle accessor for
   the per-swapchain stencil image view (matching `AcquiredImage`'s
   existing opaque-`u64`-handle pattern), implemented by both
   `VulkanSwapchain` and `HeadlessSwapchain`.

2. **`tre-rhi-vulkan` -- stencil image plumbing**: `VulkanDevice::new`
   queries and stores a supported combined depth/stencil format
   (`D24_UNORM_S8_UINT` preferred, `D32_SFLOAT_S8_UINT` fallback -- the
   spec guarantees at least one is supported; detected, not assumed,
   matching this project's established capability-query precedent).
   `VulkanSwapchain::new`/`HeadlessSwapchain::new` each create their own
   stencil image + view sized to their own extent (mirroring their
   existing color-image creation code, reusing the existing
   `allocate_and_bind_image` helper). `begin_frame` transitions the
   stencil image's layout and attaches it via `RenderingInfo`'s stencil
   attachment (`AttachmentLoadOp::CLEAR` every frame, stencil cleared to
   `0`).

3. **`tre-rhi-vulkan` -- `create_pipeline`**: internal-only change
   (`PipelineRenderingCreateInfo::stencil_attachment_format`, no public
   signature change) so every existing pipeline stays compatible with the
   now-always-attached stencil buffer without touching call sites.

4. **`tre-rhi-vulkan` -- new pipeline pair**:
   `create_stencil_and_cover_pipelines(vertex_spv, fragment_spv,
   color_format, fill_rule: FillRule) -> Result<(VulkanPipelineState,
   VulkanPipelineState), EngineError>`. Stencil-pass PSO: color writes
   masked off, stencil test `ALWAYS`, `EvenOdd` uses a single `INVERT` op
   for both front/back faces, `NonZero` uses two-sided
   `INCR_WRAP`(front)/`DECR_WRAP`(back). Cover-pass PSO: normal color
   writes, stencil test `NOT_EQUAL` against `0`, `pass_op = ZERO`
   (cleans the buffer for the next shape), `fail_op = KEEP` -- identical
   for both fill rules.

5. **`tre-svg`**: `fan_triangles(polygon: &Polygon) -> Vec<[u32; 3]>` and
   `bounding_box(polygon: &Polygon) -> ([f32; 2], [f32; 2])`.

6. **New example** (`crates/tre-rhi-vulkan/examples/stencil_and_cover_demo.rs`,
   `demo/phase3_step3_3_3/`): parses a genuine self-intersecting pentagram
   path, confirms `triangulate()` returns `Err(NotSimplePolygon)`, then
   renders it twice (once per fill rule) via the new stencil-and-cover
   pipeline pair: stencil pass draws `fan_triangles`' geometry, cover
   pass draws a flat-colored quad over `bounding_box`'s extent. Reads
   back real pixels: a point in the pentagram's outer star-point regions
   is filled under both fill rules; a point in the central pentagon is
   filled under `NonZero` but exactly background under `EvenOdd` -- the
   textbook proof both fill rules are implemented correctly and
   genuinely differently.

7. **CI**: add `stencil_and_cover_demo` to the `vulkan-validation` job's
   example list.

8. **Unit tests**: `tre-svg` -- `fan_triangles`/`bounding_box` against
   hand-computable shapes (including a self-intersecting one, to confirm
   `fan_triangles` genuinely makes no validity assumption, unlike
   `triangulate`).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `stencil_and_cover_demo` re-run under `VK_LAYER_KHRONOS_validation`,
  zero errors.
- **All 9 pre-existing Vulkan examples re-run manually** -- this step
  touches the shared `begin_frame`/`create_pipeline` surface for the
  first time in Phase 3, a real regression risk every prior 3.3.x
  sub-step didn't carry.
- CI: push, confirm `stencil_and_cover_demo` passes on Mesa lavapipe
  (which must support at least one of the two depth/stencil formats
  queried, per spec guarantee).

## Explicitly out of scope for this step

- Wiring stencil-and-cover into `RenderingCanvas`'s public `Canvas` API
  or any automatic `triangulate`-fails-so-fall-back-automatically
  orchestration -- proven directly via a dedicated demo first, matching
  every prior sub-step's precedent.
- Window-resize-time stencil image recreation -- no resize support
  exists in this project yet.
- Antialiasing the stencil-and-cover result's edges (this technique
  produces a hard-edged fill; real implementations often combine it with
  MSAA or a signed-distance post-process, both separate, real techniques
  not built here).
- Any change to `tre-svg::triangulate`'s own behavior or error type --
  `SvgError::NotSimplePolygon` already exists and already documents this
  step as its intended remedy.
