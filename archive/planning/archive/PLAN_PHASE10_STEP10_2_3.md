# Plan: Phase 10 Step 10.2.3 — Non-`Normal` Blend Modes

**Status: Complete (2026-09-09), via a real, disclosed pivot away from
this plan's own primary path.** See `documentation/IMPLEMENTATION.md`'s
own "Step 10.2.3: Non-Normal Blend Modes" write-up for the full
technical account of what shipped, `documentation/REVIEW.md` finding
#170 (the plan-invalidating discovery and the pivot, made at the user's
explicit direction) and findings #171-172 (two real regressions this
step's own GPU demo-regression sweep caught and fixed same-day), and
`demo/phase10_step10_2_3/README.md` for the verification summary. This
file is the original plan, archived unchanged below from the combined
`PLAN.md` roadmap (Steps 10.2.1–10.2.6) once this sub-step's own real
work began — see that roadmap's remaining sections for Steps
10.2.4–10.2.6, still active in `PLAN.md`.

**What actually shipped, in one paragraph.** This plan's own primary
path — `VK_EXT_blend_operation_advanced`, mapping each blend mode
directly onto a hardware `VkBlendOp` — is NOT implemented by RADV, this
project's own real dev GPU/driver (confirmed via direct `vulkaninfo`
inspection and independently corroborated via Mesa's own release
notes), invalidating this plan's own "Investigation" section below
before any implementation code was written. Presented to the user as a
genuine three-way fork; the user chose the real, portable alternative
explicitly: "use the alternative, it sounds like the designed way to do
it, VK_KHR_dynamic_rendering_local_read." The real implementation reads
the destination pixel a preceding draw already wrote via a real
`VK_DESCRIPTOR_TYPE_INPUT_ATTACHMENT` descriptor and GLSL `subpassLoad`
(a genuine framebuffer read — the technique this plan's own
"Investigation" section explicitly argued was unnecessary), computes
the blend formula itself in one shared `PipelineKind::FlatColorBlend`
pipeline (a runtime shader branch on blend mode, not one `VkPipeline`
per blend mode as this plan originally scoped), and writes the
already-composited result with hardware blending disabled.

---

## Original plan, as written (now superseded by the pivot above)

### Investigation

- Vulkan blend state is baked into each `VkPipeline` at creation
  (`crates/tre-rhi-vulkan/src/lib.rs`'s `dynamic_states` only covers
  `VIEWPORT`/`SCISSOR` today) — a shape's `blend_mode` therefore selects a
  *pipeline variant*, not a shader branch; fill-kind (10.2.1/10.2.2) and
  blend mode are genuinely orthogonal axes (fragment color math vs.
  fixed-function blend equation).
- `VK_EXT_blend_operation_advanced` maps `Multiply`/`Screen`/`Overlay`/
  `SoftLight`/`ColorDodge` directly onto hardware advanced-blend `VkBlendOp`
  values (`MULTIPLY_EXT`/`SCREEN_EXT`/`OVERLAY_EXT`/`SOFTLIGHT_EXT`/
  `COLORDODGE_EXT`) — this is the REAL, correct answer this project's own
  finding (which assumed "needs new RHI framebuffer-read capability") did
  not have researched at the time it was written. `ash` 0.38 exposes these
  as ordinary `vk::BlendOp` values; the extension itself must be verified
  present and enabled at `VulkanDevice::new` (a real, disclosed capability
  query, not assumed) before this path can be used.
- No framebuffer-read shader trick needed if the extension is present —
  a real, better technical path than the original Step 10.2 plan's own
  finding assumed, discovered only by researching the extension directly
  (not from memory) before writing this plan, matching this project's
  standing "verify real behavior before committing to an approach"
  discipline.

**Superseded (REVIEW.md finding #170):** the extension is absent from
this project's own real dev GPU's advertised extension list, so this
entire "no framebuffer-read needed" premise turned out to be wrong for
the hardware this project actually verifies against — the real
implementation DOES use a framebuffer read, via
`VK_KHR_dynamic_rendering_local_read`.

### Scope decisions

- Primary path: `VK_EXT_blend_operation_advanced`, queried for real support
  at device creation; if genuinely unavailable on the running GPU/driver,
  fail closed to `Normal` blending with a clear, disclosed, non-panicking
  degradation (an `EngineError`-surfaced capability flag, not a silent
  behavior change) — this project's own established RHI-capability-gap
  discipline (e.g. `RhiCommandBuffer` trait methods that return `Result`
  for genuinely-optional capabilities).
- One `VkPipeline` per (shape-rendering shader × non-`Normal` `BlendMode`)
  combination, added to `PipelineRegistry` under new `PipelineKind` values
  — mechanical given `PipelineRegistry`'s already-generic
  `HashMap<u16, Box<dyn RhiPipelineState>>` design; exact ID-space encoding
  (e.g. `base_pipeline_id + blend_mode as u16 * NUM_BASE_PIPELINES`, or a
  separate `blend_mode` field carried on `DrawGeometry` and combined with
  the shape's own base pipeline id at lookup time) is an implementation-time
  decision, not fixed here.

**Implemented differently, in both bullets above.** The primary path was
never usable (see above), so the fail-closed fallback became the ONLY
`Normal`-blending behavior, unconditionally, and the real path taken is
`VK_KHR_dynamic_rendering_local_read` instead, gated behind
`RhiDevice::local_read_blend_supported` (mirroring the exact same
capability-query discipline this bullet already called for, just against
a different extension) — real hardware without it still falls back to
`Normal` blending, exactly as planned. Only ONE new pipeline
(`PipelineKind::FlatColorBlend`) was needed, not one per blend mode per
shape shader: the blend mode itself became a cheap runtime shader branch
selected via a repurposed push-constant value, and the scope was
additionally narrowed (a real, disclosed decision made during
implementation, not written here originally) to `Polygon`/`Path` solid
fill only — not `Rectangle`/`Circle`, not gradient/texture fill.

### Tasks

1. Research + confirm `VK_EXT_blend_operation_advanced` availability
   assumptions against the actual CI/dev GPU (`vulkaninfo` or equivalent
   real query) before committing further — a real go/no-go gate for the
   primary path.
2. Device-creation-time capability query + a real, tested fallback path.
3. Pipeline-variant construction for each real (shape pipeline × blend
   mode) combination actually needed.
4. `ShapeRegistry::flatten_into` selects the blend-mode-appropriate
   pipeline id per shape's own `blend_mode` field (already threaded, never
   read).
5. Tests + a real GPU demo: two overlapping shapes under each non-`Normal`
   `BlendMode`, pixel-verified against an independent Rust reference of
   each blend equation (same "compute the correct answer independently,
   compare against real GPU output" discipline `translucent_flat_fill_
   demo.rs` established this session).

**Task 1's own go/no-go gate is exactly what caught the pivot** — real
`vulkaninfo` research at the start of implementation returned "not
available," triggering the fork presented to and resolved by the user
(REVIEW.md finding #170), rather than proceeding on an unverified
assumption. Tasks 2-5 were all still done, against the real alternative
extension instead: a device-creation-time capability query (task 2); the
new pipeline layout/descriptor set/pipeline (task 3, one pipeline, not
one per blend mode); `shapes.rs`'s `draw_polygon_fill` dispatch on
`blend_mode` (task 4); and a real GPU demo, `blend_mode_demo.rs`, with
six polygon swatches (one per `BlendMode`) pixel-verified against an
independent Rust reference of the real W3C blend formulas (task 5).
