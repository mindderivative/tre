# Plan: Phase 10 Steps 10.2.1 – 10.2.6 — Finishing Full Shape Rendering Support

## User request (verbatim)

> Create a plan for 10.2.1 - 10.2.6 for the real gaps still open. Update the
> TRE Build Tracker and other documentation. We are going to finish out 10.2
> completely.

Sub-numbering, not a renumbering: 10.2.1–10.2.6 sit between the already-shipped
10.2 and the already-planned 10.3 (`tre-ffi`)/10.4 (Python bindings) — the same
dotted-decimal convention Phase 3's own 3.3.1–3.3.3 already established. No
existing step number changes.

## The six real gaps, as disclosed to the user and confirmed against the
## real, current code before writing this plan

1. `FillStyle::Gradient(GradientId)` — a real enum variant since Step 10.1,
   still `unimplemented!()` at all four `flatten_*` call sites in
   `crates/tre-engine/src/shapes.rs` (confirmed: lines ~775, 834, 926, 1091,
   each `let FillStyle::Solid(color) = shape.fill else { panic!(...) }`).
2. `FillStyle::Texture(u32)` — same enum, same panic sites.
3. `BlendMode` (`Normal`/`Multiply`/`Screen`/`Overlay`/`SoftLight`/
   `ColorDodge`) — a real, already-threaded `PrimitiveCommon::blend_mode`
   field since Step 10.1, read by nothing (`ShapeRegistry::flatten_into`
   never branches on it; every pipeline's blend state is `Normal` today).
4. `sd_ellipse`'s disclosed approximate SDF (`sdf_ellipse.frag`) — exact only
   when `radius.x == radius.y`; and `sdf_rect_styled.frag`'s
   `corner_smoothing` superellipse blend, explicitly not claimed to match
   any specific reference squircle algorithm.
5. Rounded stroke caps on a partial-arc `Circle`/`Ellipse` (`arc_length <
   360`) — `sdf_ellipse.frag`'s sector cutoff is hard-edged
   (`d = max(d, 0.001)` past the sweep), no cap geometry at the two cuts.
6. The shape system's zero-allocation claim (ARCHITECTURE.md Section 7.5) is
   architecturally sound but not proven live the way `main_loop_demo`'s own
   claim is — `RenderTickGuard` is wired into that one demo only.

## Sequencing rationale

10.2.1 and 10.2.2 share one new piece of infrastructure (a per-shape
"fill kind" dispatch: solid vs. gradient vs. texture) and are sequenced
together so that infrastructure is designed once, not twice. 10.2.3 (blend
modes) is a pipeline-level, shader-independent axis and is sequenced after
the fill-kind shaders stabilize, so its own new pipeline variants are built
against final shader code, not code still in flux. 10.2.4 and 10.2.5 both
touch `sdf_ellipse.frag`; 10.2.4 (the exact SDF) is sequenced first so 10.2.5
builds its cap geometry on the corrected distance field, not the outgoing
approximation. 10.2.6 runs last by design — its own new demo should prove
every new code path landed by 10.2.1–10.2.5 is zero-alloc too, not just
today's already-shipped Step 10.2 code.

---

## Step 10.2.1 — Gradient Fill (Linear + Radial)

**Status: Complete (2026-09-09) -- archived to
`planning/archive/PLAN_PHASE10_STEP10_2_1.md`** (with real
implementation notes on top of this original plan). See
`documentation/IMPLEMENTATION.md`'s own write-up, REVIEW.md finding
#167 (a real coordinate-space bug found and fixed by this step's own
demo), and `demo/phase10_step10_2_1/`. Kept below unchanged as
reference context for Steps 10.2.2 onward.

### Investigation

- `FillStyle::Gradient(GradientId)` and `GradientId(pub u32)` already exist
  (`shapes.rs`); nothing defines what a `GradientId` actually points to yet
  — this step adds the missing definition-and-storage half.
- The per-shape GPU style buffer (`crates/tre-engine/src/gpu_style.rs`,
  `RhiDevice::shape_style_buffer`, bindless set binding 1) is a plain
  `readonly buffer { uint words[]; }` — appending a new, independently
  word-indexed record type (`GpuGradientStyle`) costs nothing structural;
  the existing bump allocator doesn't care that records have different
  sizes (`gpu_style.rs`'s own doc comment already says so).
- `GpuRectStyle`/`GpuEllipseStyle` currently assume `frag_color` (solid,
  vertex-interpolated) is the only fill source. Both records gain two new
  trailing `u32` words: `fill_kind` (0 = solid via `frag_color`, 1 =
  gradient) and `gradient_word_index` (a second style-buffer word index,
  valid only when `fill_kind == 1`) — additive, so existing solid-fill
  callers are unaffected (`fill_kind` defaults to 0).
- Polygon/Path fill (`PipelineKind::FlatColor`, `walking_skeleton.frag`) has
  no per-shape style record today (`params` is unused, `uv` is zeroed) —
  a gradient fill here needs local-space position at the fragment stage
  (repurpose the currently-zeroed `uv` slot to carry it) plus a
  `gradient_word_index`, carried the same way `TexturedQuad`'s
  `texture_index` already is: a push constant (constant across one draw,
  matching how gradient fills already can't currently batch across
  different gradients any more than textured draws batch across different
  textures today).

### Scope decisions

- Linear and radial gradients only (conic/angular explicitly out of scope,
  matching the original Step 10.2 plan's own disclosure).
- Up to 8 stops (position `0.0..=1.0` + `Color`), a fixed cap matching this
  codebase's own "small, fixed, disclosed limit" precedent (e.g. `tre-svg`'s
  own input caps) — more than 8 stops returns a real `Result` error from
  the new gradient-creation API, not silent truncation.
- Stop colors stored packed sRGB (same `rgba8` convention as everywhere
  else); interpolation happens in LINEAR space in the shader (`srgb_to_
  linear` per endpoint, then `mix`), matching this codebase's own
  established blending discipline (ARCHITECTURE.md Section 6.1) — not
  interpolated in sRGB space, which would produce the well-known "muddy
  midpoint" artifact.
- New public API: `ShapeRegistry::create_gradient(GradientDef) -> Result<GradientId, EngineError>`
  (a new registry-owned table, mirroring `ShapeRegistry::insert`'s own
  generational-handle precedent) rather than a global/static table —
  keeps gradients scoped to the registry that owns the shapes referencing
  them, consistent with how shapes themselves are scoped.
- New pipeline: `PipelineKind::GradientFill` (its own vertex/fragment shader
  pair) for Polygon/Path — a new pipeline rather than branching inside
  `walking_skeleton.frag`, matching this codebase's own established
  "separate pipeline per real style variant" precedent (`SdfRectStyled`
  next to `SdfRoundedRect`, `SdfEllipse` next to nothing before it).

### Tasks

1. `GradientDef`/`GradientKind` (`Linear { start: Vec2, end: Vec2 }` /
   `Radial { center: Vec2, radius: f32 }`) + up to 8 `(f32, Color)` stops,
   in `tre-engine`.
2. `ShapeRegistry::create_gradient` — validates stop count/ordering,
   bump-allocates a `GpuGradientStyle` record (header: kind tag, start/end
   or center/radius, real stop count; then up to 8 `(f32 position, u32
   color)` pairs) into the shape style buffer once per unique gradient
   per frame (or once per `create_gradient` call if gradients are meant to
   be stable across frames — resolve during implementation against how
   `ShapeRegistry`'s own per-frame vs. per-registry lifetime already
   works for other data).
3. Extend `GpuRectStyle`/`GpuEllipseStyle` with `fill_kind`/
   `gradient_word_index`; update `sdf_rect_styled.frag`/`sdf_ellipse.frag`
   with a gradient-evaluation branch (shared GLSL `vec3 eval_gradient(...)`
   snippet, kept in lockstep by hand like every other style-buffer field
   already is).
4. New `gradient_fill.vert`/`gradient_fill.frag` + `PipelineKind::
   GradientFill`, registered everywhere `PipelineRegistry` is built.
5. Wire all four `flatten_*` functions' `FillStyle::Gradient` arm.
6. Tests: gradient stop validation (empty/too-many/unordered), linear/radial
   evaluation at known `t` values (a CPU-side Rust reference mirroring the
   shader's own math, same pattern `translucent_flat_fill_demo.rs` already
   established for verifying shader math independently), a real GPU demo
   with pixel samples along a gradient's own axis confirming a real,
   monotonic color change (not just "didn't crash").

---

## Step 10.2.2 — Texture Fill

**Status: Complete (2026-09-09) -- archived to
`planning/archive/PLAN_PHASE10_STEP10_2_2.md`** (with real
implementation notes on top of this original plan). See
`documentation/IMPLEMENTATION.md`'s own write-up, REVIEW.md findings
#168/#169, and `demo/phase10_step10_2_2/`. Kept below unchanged as
reference context for Steps 10.2.3 onward.

### Investigation

- `FillStyle::Texture(u32)` already exists; the `u32` is the same bindless
  texture-array index space `PipelineKind::TexturedQuad`/
  `bindless_textured.frag` already use (`layout(set = 0, binding = 2)
  uniform texture2D bindless_textures[]`) — no new descriptor infrastructure
  needed, only UV generation and a fill-kind branch, reusing the exact
  sentinel convention (`0xFFFFFFFF` = no texture) `bindless_textured.frag`
  already established.
- UV mapping is shape-kind-specific: `Rectangle`/`Circle`/`Ellipse` map
  local coordinates to `[0,1]` via their own known half-extent/radius (no
  new bookkeeping); `Polygon`/`Path` need each shape's own local bounding
  box, computed once at flatten time (a small, real addition — matches the
  original Step 10.2 plan's own "bounding-box-mapped UVs" note).

### Scope decisions

- Extends 10.2.1's `fill_kind` field to a third value (`2 = texture`) rather
  than inventing a parallel mechanism — `GpuRectStyle`/`GpuEllipseStyle`
  gain one more trailing `u32` (`texture_index`); Polygon/Path route
  through `PipelineKind::GradientFill`'s own shader pair extended with a
  texture branch (renaming it, e.g., to a more general `StyledFill`
  pipeline naming decision made at implementation time) rather than a
  fourth pipeline — avoids a combinatorial pipeline explosion across
  (shape kind × fill kind).
- No filtering/wrap-mode configuration this pass (uses the existing single
  shared bindless sampler, same as `TexturedQuad` today) — a real,
  disclosed simplification, not silently different per shape.

### Tasks

1. Bounding-box computation for Polygon/Path (`generate_polygon_points`'s
   output, `flatten_path`'s output) — a small, pure function, real unit
   tests against known shapes.
2. Extend style records + shaders with the texture branch (sampling via
   `nonuniformEXT`, matching `bindless_textured.frag`'s own real code).
3. Wire `FillStyle::Texture` at all four `flatten_*` call sites.
4. Tests + a real GPU demo: a texture (e.g. the existing atlas-packing
   demo's own checkerboard/solid-color test texture) sampled correctly
   across all four shape kinds, pixel-verified against the source texture's
   own known content at known UV coordinates.

---

## Step 10.2.3 — Non-`Normal` Blend Modes

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

---

## Step 10.2.4 — SDF Fidelity: Exact Ellipse Distance Field & Corner-Smoothing Reconciliation

### Investigation

- `sd_ellipse` (`sdf_ellipse.frag`) is a standard scaled-circle
  approximation, exact only when `radius.x == radius.y`. A real "exact"
  ellipse SDF (Inigo Quilez's own published derivation) is not actually a
  closed-form quartic in practice — it's typically an iterative
  (Newton-style) refinement on the ellipse's implicit parametrization; the
  real, current formula must be verified against IQ's own published
  article before implementation (this codebase's own standing discipline:
  verify real external algorithms via direct research, not from memory,
  before integrating — exactly how the `lyon` migration's own API
  assumptions were verified this session).
- `corner_smoothing`'s superellipse blend (`sdf_rect_styled.frag`) is a
  real, legitimate, monotonic smoothing technique (superellipse/"squircle"
  blending is itself a standard real technique, not an ad hoc hack) — the
  disclosed gap is that it was never checked against any specific
  reference (e.g. Figma's own published squircle parametrization), not
  that it's known wrong.

### Scope decisions

- Ellipse: replace `sd_ellipse` with a verified-correct exact (or
  near-machine-precision iterative) formula, with a NEW unit test
  comparing against analytically-known exact distances at reference points
  (e.g. along the major/minor axis, where the exact distance is trivially
  `d - a`/`d - b` for a point at distance `d > a`/`b` from center) — proving
  BOTH the new formula's correctness AND, for honesty, the OLD
  approximation's real error at the same points (matching this session's
  own `translucent_flat_fill_demo.rs` precedent of proving a fix against
  an independent reference, not just asserting "looks fine").
- Corner smoothing: research a specific real reference formula (e.g.
  Figma's own published squircle article) and make an honest,
  evidence-based call at implementation time — either match it if a simple
  closed form exists, or formally verify and document the existing
  superellipse blend's real deviation from that reference within a
  quantified tolerance, whichever the research actually supports. Not
  presupposing a rewrite is needed; presupposing only that the current
  disclosed uncertainty gets resolved one way or the other, honestly.

### Tasks

1. Research IQ's real published exact/near-exact ellipse SDF article;
   confirm the formula before writing GLSL against it.
2. Implement + a CPU-side Rust reference of the same formula for testing
   (mirroring `translucent_flat_fill_demo.rs`'s "independent reference"
   pattern), with reference-point unit tests.
3. Research a real squircle/corner-smoothing reference formula; make and
   document the scope call above.
4. Re-run `shape_full_rendering_demo` and every other consumer of these two
   shaders to confirm no visual regression at `radius.x == radius.y` /
   `smoothing == 0` (the two cases where old and new must be bit-identical
   or near-identical).

---

## Step 10.2.5 — Rounded Stroke Caps on Partial-Arc Circles/Ellipses

### Investigation

- `sdf_ellipse.frag`'s sector cutoff (`if (relative > arc_sweep_angle) { d
  = max(d, 0.001); }`) hard-clips the fill/stroke past the arc's own sweep
  -- no cap geometry, so a partial-arc progress-ring-style `Circle`'s two
  cut edges are flat, not rounded, even when `stroke_line_cap` (already a
  real field, borrowed from `Path`'s own convention conceptually) would
  imply otherwise.
- A real alternative considered and rejected: route partial-arc circles
  through `lyon`'s own tessellated stroke path (flatten the arc into a
  polyline, let `lyon`'s real `LineCap::Round` handle the caps) — this
  session's own precedent for "don't hand-roll what lyon already solves."
  Rejected here specifically because `draw_flat_polygon`'s tessellated
  triangles have NO antialiasing (confirmed this session: hard triangle
  edges only), and circles/rings are a highly AA-sensitive, extremely
  common real UI element (progress indicators) — trading the SDF
  pipeline's existing `fwidth`-based smooth edges for jagged tessellated
  ones would be a real visual regression for a common case, not a neutral
  implementation-detail swap. An analytic SDF cap keeps the existing
  antialiasing.

### Scope decisions

- Real analytic rounded caps: at each of the arc's two cut angles, union
  (`min()`) the existing sector-clipped ellipse SDF with two small circle
  SDFs of radius `border_thickness / 2`, centered at the point where the
  stroke band's own centerline meets that cut angle on the ellipse
  boundary — the standard 2D "rounded line/capsule" SDF technique, applied
  at the two cut points instead of a straight segment's two ends.
- Disclosed approximation carried over from `sd_ellipse` itself for a true
  (non-circular) `Ellipse`: the cap-center placement uses the LOCAL
  boundary point at each cut angle (exact for a `Circle`, a real,
  consistent approximation for a non-uniform-radius `Ellipse`, in the same
  spirit as `sd_ellipse`'s own existing disclosed approximation, now
  narrowed by 10.2.4 to only this one remaining case).
- Only applies when `border_thickness > 0.0` and `arc_sweep_angle < TAU`
  (a full ellipse or a borderless partial arc needs no cap geometry at
  all — matches the shader's own existing early-out for a full sweep).

### Tasks

1. Derive and implement the two-cap-circle SDF union in `sd_ellipse`'s
   sector-cutoff branch.
2. New tests: a partial-arc `Circle` with a real border, pixel-sampled at
   both cut edges, confirming a real rounded (not flat) transition — a
   direct visual/pixel proof, matching this session's own established
   demo discipline.
3. Re-verify `shape_full_rendering_demo`'s own existing arc-wedge exclusion
   test still passes unchanged (a full sweep and a borderless arc must be
   bit-identical to before).

---

## Step 10.2.6 — Zero-Allocation Live Verification for the Shape System

### Investigation

- `tre_memory::RenderTickGuard`/`DebugAllocGuard` (Step 9.2) already prove
  `main_loop_demo`'s own per-frame span is genuinely zero-allocation, after
  a documented warm-up frame. `ShapeRegistry`/`RenderingCanvas::flatten_
  into` reuse the exact same already-proven `reset()`/`draw_*`/
  `flatten_into` machinery Step 9.2 verified — but no demo has ever wrapped
  a shape-registry-driven scene in the guard itself (ARCHITECTURE.md
  Section 7.5's own disclosed gap).

### Scope decisions

- New demo (not a retrofit of `shape_full_rendering_demo`, to keep that
  demo's own existing, stable pixel-correctness assertions untouched):
  builds a representative mixed scene — `Rectangle`/`Circle`/`Polygon`/
  `Path`, with borders, and (since this step runs last) gradients,
  textures, and non-`Normal` blend modes too, so every new code path from
  10.2.1–10.2.5 is covered by the same zero-allocation proof, not just the
  original Step 10.2 surface.
- Proves the REAL, dynamic-update case, not a static strawman: mutates
  shape properties (position, color, gradient stops) across repeated
  frames inside the guard, matching this project's own standing discipline
  of testing what real UI usage actually does (continuous updates), not
  just a frame replayed unchanged.

### Tasks

1. New `shape_registry_zero_alloc_demo.rs`, modeled on `main_loop_demo.rs`'s
   own established warm-up-then-guard pattern.
2. A representative, mutating mixed scene exercising every shape kind and
   (once landed) every new fill/blend feature.
3. `RenderTickGuard`-wrapped `flatten_into`/`canvas.flatten()` across many
   repeated frames; a real, hard assertion of zero allocations post-warm-up.
4. Update ARCHITECTURE.md Section 7.5 to close the disclosed gap once
   proven, matching `main_loop_demo`'s own precedent exactly.

---

## Documentation & tracking (every sub-step)

- REVIEW.md: a new finding/decision entry per sub-step for any real issue
  found during implementation (matching this project's own standing
  practice — never silently fixed, never silently deferred).
- ARCHITECTURE.md Section 7.5's "Implementation status" note updated as
  each gap closes.
- IMPLEMENTATION.md: a new `#### Step 10.2.X` write-up per sub-step,
  "Status: Complete" only once real, tested, demoed.
- `demo/phase10_step10_2_X/` per sub-step, matching this project's own
  established per-step demo-folder convention.
- TRE Build Tracker (Artifact `714bd0d7-87d1-4615-9ae3-6b99c44cd378`):
  six new rows under Phase 10, each `PLANNED` until its own sub-step
  actually lands, then flipped to `DONE` with a real one-line summary —
  never batch-flipped ahead of real, verified work.
- This `PLAN.md` gets archived to `planning/archive/PLAN_PHASE10_STEP10_2_X.md`
  as each sub-step's own real work begins in earnest (matching this
  project's established one-active-plan convention), with a fresh `PLAN.md`
  scoped to the next sub-step.

## Verification plan (every sub-step)

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace`, a full regression sweep of every
pre-existing GPU demo (not just the new one), and the new sub-step's own
real GPU demo with real pixel assertions — the same bar every prior real
step in this project has been held to, no exceptions for "just a smaller
sub-step."
