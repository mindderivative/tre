# Plan: Phase 10 Step 10.2.2 — Texture Fill

**Status: Complete (2026-09-09).** See `documentation/IMPLEMENTATION.md`'s
own "Step 10.2.2: Texture Fill" write-up for the full account of what
shipped, `documentation/REVIEW.md` findings #168 (Polygon/Path reused
the existing `TexturedQuad` pipeline directly, a better path than this
plan's own original suggestion) and #169 (a `StyleFill` parameter-list
consolidation made along the way), and `demo/phase10_step10_2_2/
README.md` for the verification summary. This file is the original
plan, archived unchanged from the combined `PLAN.md` roadmap (Steps
10.2.1–10.2.6) once this sub-step's own real work began — see that
roadmap's remaining sections for Steps 10.2.3–10.2.6, still active in
`PLAN.md`.

## Investigation

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

## Scope decisions

- Extends 10.2.1's `fill_kind` field to a third value (`2 = texture`) rather
  than inventing a parallel mechanism — `GpuRectStyle`/`GpuEllipseStyle`
  gain one more trailing `u32` (`texture_index`); Polygon/Path route
  through `PipelineKind::GradientFill`'s own shader pair extended with a
  texture branch (renaming it, e.g., to a more general `StyledFill`
  pipeline naming decision made at implementation time) rather than a
  fourth pipeline — avoids a combinatorial pipeline explosion across
  (shape kind × fill kind). **Implemented differently:** Polygon/Path
  reuse the EXISTING `PipelineKind::TexturedQuad`/`bindless_textured.
  frag` pipeline directly instead — texture sampling needed no new GLSL
  at all, unlike gradient evaluation, so extending `GradientFill` would
  have been unnecessary work (REVIEW.md finding #168).
- No filtering/wrap-mode configuration this pass (uses the existing single
  shared bindless sampler, same as `TexturedQuad` today) — a real,
  disclosed simplification, not silently different per shape.
  **Implemented as planned.**
- **Not in this plan, added during implementation:** `draw_styled_
  rectangle`/`draw_ellipse`'s `fill_kind`/`gradient_word_index`/
  `texture_index` fields were bundled into one new `StyleFill` struct
  rather than left as ever-growing trailing `u32` parameters
  (REVIEW.md finding #169).

## Tasks (as implemented)

1. `bounding_box_uvs`, a small pure function in `tre-engine`'s
   `shapes.rs`, with real unit tests (normalization, degenerate
   zero-extent fallback).
2. Extended `GpuRectStyle`/`GpuEllipseStyle` with `texture_index`;
   `sdf_rect_styled.frag`/`sdf_ellipse.frag` each gained a `fill_kind ==
   2` texture-sampling branch, declaring the same bindless sampler/
   texture-array bindings `bindless_textured.frag` already uses.
3. Wired `FillStyle::Texture` at all four `flatten_*` call sites, via
   `resolve_style_fill` (`Rectangle`/`Circle`) and `draw_polygon_fill`
   (`Polygon`/`Path`, reusing `PipelineKind::TexturedQuad` via a new
   `RenderingCanvas::draw_textured_polygon`).
4. Tests + a real GPU demo (`texture_fill_demo.rs`): a real four-quadrant
   flag texture sampled correctly across all four shape kinds,
   pixel-verified against the texture's own known content at known
   probe points.
