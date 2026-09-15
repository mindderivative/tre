# Plan: Phase 10 Step 10.2.1 — Gradient Fill (Linear + Radial)

**Status: Complete (2026-09-09).** See `documentation/IMPLEMENTATION.md`'s
own "Step 10.2.1: Gradient Fill" write-up for the full account of what
shipped, `documentation/REVIEW.md` finding #167 for the real
coordinate-space bug found and fixed by this step's own demo, and
`demo/phase10_step10_2_1/README.md` for the verification summary. This
file is the original plan, archived unchanged from the combined
`PLAN.md` roadmap (Steps 10.2.1–10.2.6) once this sub-step's own real
work began — see that roadmap's remaining sections for Steps 10.2.2–
10.2.6, still active in `PLAN.md`.

## Investigation

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

## Scope decisions

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
  them, consistent with how shapes themselves are scoped. **Implemented
  as `Result<GradientId, GradientError>` instead** — `EngineError`'s own
  every existing variant is a real RHI/GPU failure class; this is
  caller-input validation, checked entirely on the CPU, the same category
  `tre_svg::SvgError` already occupies.
- New pipeline: `PipelineKind::GradientFill` (its own vertex/fragment shader
  pair) for Polygon/Path — a new pipeline rather than branching inside
  `walking_skeleton.frag`, matching this codebase's own established
  "separate pipeline per real style variant" precedent (`SdfRectStyled`
  next to `SdfRoundedRect`, `SdfEllipse` next to nothing before it).
  **Implemented reusing the EXISTING `bindless_textured.vert`** instead of
  a new vertex shader — that shader already outputs `frag_uv` and already
  declares a `texture_index` push constant, exactly what this pipeline
  needed, matching `msdf.frag`'s own precedent for reusing an existing
  vertex stage.

## Tasks (as implemented)

1. `GradientDef`/`GradientKind`/`GradientStop`/`GradientError`, in
   `tre-engine`'s `shapes.rs`.
2. `ShapeRegistry::create_gradient` — validates stop count/ordering/
   position range and (for `Radial`) a positive radius; stores the
   definition once, on the CPU, in a new `gradients: Vec<GradientDef>`
   registry field (append-only, no generational reuse).
3. Extended `GpuRectStyle`/`GpuEllipseStyle` with `fill_kind`/
   `gradient_word_index`; `sdf_rect_styled.frag`/`sdf_ellipse.frag` each
   gained a duplicated `eval_gradient` function.
4. New `gradient_fill.frag` (paired with the existing `bindless_textured.
   vert`) + `PipelineKind::GradientFill`.
5. Wired all four `flatten_*` functions' `FillStyle::Gradient` arm, via a
   shared `resolve_style_fill` (`Rectangle`/`Circle`) and `draw_polygon_
   fill` (`Polygon`/`Path`).
6. Tests: gradient stop validation, `GpuGradientStyle` byte-layout
   round-trips, `flatten_into` wiring for all three real code paths, and
   a real GPU demo (`gradient_fill_demo.rs`) with an independent Rust
   reference of the exact gradient math.

**One real bug found and fixed during implementation, not in this
original plan:** a coordinate-space mismatch between `GradientDef`'s own
public top-left-relative local space and `frag_uv`'s internal
center-relative convention — see REVIEW.md finding #167 for the full
account.
