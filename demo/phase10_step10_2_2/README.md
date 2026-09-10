# Demo: Phase 10 Step 10.2.2 -- Texture Fill

```bash
./demo/phase10_step10_2_2/run_texture_fill_demo.sh
```

**What this closes.** The second of six gaps `PLAN.md` scheduled after
Step 10.2's own follow-ups: `FillStyle::Texture` -- a real enum variant
since Step 10.1 -- had no rendering support for any shape kind.

**What's real now.** All four shape kinds render a real bindless texture:

- `Rectangle`/`Circle` extend their own existing per-vertex `GpuRectStyle`/
  `GpuEllipseStyle` style-buffer record with one more trailing field,
  `texture_index`, and sample the bindless texture array directly inside
  `sdf_rect_styled.frag`/`sdf_ellipse.frag`'s own new `fill_kind == 2`
  branch -- `frag_uv` (already known from each shape's own SDF math) maps
  onto the shape's own bounding box to produce a real `[0, 1]` UV, no new
  descriptor bindings needed (the same bindless sampler/texture array
  binding 0/2 every pipeline's descriptor set already carries).
- `Polygon`/`Path` have no per-vertex style record at all, so they reuse
  the EXISTING `PipelineKind::TexturedQuad`/`bindless_textured.frag`
  pipeline directly -- no new shader, no new pipeline: sampling a texture
  is not new math the way gradient evaluation was. A new `bounding_box_
  uvs` helper computes each shape's own real bounding box once, at
  flatten time, and a new `RenderingCanvas::draw_textured_polygon`
  carries real per-vertex UVs instead of `draw_flat_polygon`'s zeroed
  ones.

**A parameter-list refactor made along the way.** `draw_styled_rectangle`/
`draw_ellipse` were about to grow a fourth trailing fill-selection
parameter (`texture_index`, alongside `fill_kind`/`gradient_word_index`
from Step 10.2.1). Bundled all three into one new `StyleFill` value
instead, so the signature doesn't grow again the next time a fill kind
is added.

**What this demo proves, not just "didn't crash."** A real four-quadrant
flag texture (red/green/blue/yellow, pure 0/255 channel values so sRGB
round-trips exactly) fills a rectangle, a circle, a hexagon, and a square
path -- each one's own upper-left and lower-right quadrant probed and
confirmed against the texture's own known content, proving the UV
mapping is correct for every shape kind, not just "some texture
appeared." Visually confirmed: all four shapes show the same clean
four-quadrant flag, correctly oriented and scaled to each shape's own
bounds.

**Verified.** 5 new `tre-engine` unit tests (143 total): `bounding_box_
uvs`' own real normalization and degenerate-zero-extent fallback, and
`flatten_into` wiring for all three real code paths (`Rectangle` forcing
the styled pipeline even with no border/radii/smoothing, `Circle`'s own
`fill_kind`/`texture_index`, `Polygon` routing through `TexturedQuad`
with real per-vertex UVs and no style-buffer write at all). Every
pre-existing demo re-run and confirmed bit-for-bit unchanged, including
`bindless_textures_demo` (confirming the shared `TexturedQuad` pipeline
and bindless descriptor bindings are untouched). `cargo fmt`/`clippy -D
warnings`/`build`/`test` clean across the whole workspace.
