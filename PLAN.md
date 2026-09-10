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

**Status: Complete (2026-09-09) -- archived to
`planning/archive/PLAN_PHASE10_STEP10_2_3.md`** (with real
implementation notes on top of this original plan, including a real,
disclosed pivot away from this plan's own primary path -- `VK_EXT_
blend_operation_advanced` turned out not to be implemented by RADV, this
project's own real dev GPU/driver; the real implementation uses
`VK_KHR_dynamic_rendering_local_read` instead, at the user's explicit
direction). See `documentation/IMPLEMENTATION.md`'s own write-up,
REVIEW.md findings #170/#171/#172, and `demo/phase10_step10_2_3/`. See
the archive file for the original plan text, kept there as historical
record.

---

## Step 10.2.4 — SDF Fidelity: Exact Ellipse Distance Field & Corner-Smoothing Reconciliation

**Status: Complete (2026-09-09) -- archived to
`planning/archive/PLAN_PHASE10_STEP10_2_4.md`** (with real
implementation notes on top of this original plan, including a real,
disclosed correction to this plan's own expectation about where the old
ellipse SDF approximation's real error would show up). See
`documentation/IMPLEMENTATION.md`'s own write-up, REVIEW.md findings
#173/#174, and `demo/phase10_step10_2_4/`. See the archive file for the
original plan text, kept there as historical record.

---

## Step 10.2.5 — Rounded Stroke Caps on Partial-Arc Circles/Ellipses

**Status: Complete (2026-09-09) -- archived to
`planning/archive/PLAN_PHASE10_STEP10_2_5.md`** (with real
implementation notes on top of this original plan). See
`documentation/IMPLEMENTATION.md`'s own write-up, REVIEW.md finding
#175, and `demo/phase10_step10_2_5/`. See the archive file for the
original plan text, kept there as historical record.

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
