# Demo: Phase 10 Step 10.2.1 -- Gradient Fill (Linear + Radial)

```bash
./demo/phase10_step10_2_1/run_gradient_fill_demo.sh
```

**What this closes.** The first of six gaps `PLAN.md` scheduled after
Step 10.2's own lyon/premultiply follow-ups: `FillStyle::Gradient` --
a real enum variant since Step 10.1 -- had no rendering support for any
shape kind, panicking loudly (`unimplemented!`) wherever it was used.

**What's real now.** `ShapeRegistry::create_gradient(GradientDef) ->
Result<GradientId, GradientError>` defines a real, validated (empty/too-
many/out-of-range/out-of-order stops, non-positive radial radius all
rejected with a real `Err`, never silently clamped) linear-or-radial
gradient. `Rectangle`/`Circle`/`Polygon`/`Path` all render it for real:

- `Rectangle`/`Circle` route through their own existing per-vertex
  `GpuRectStyle`/`GpuEllipseStyle` style-buffer record, extended with two
  new fields (`fill_kind`, `gradient_word_index`) pointing at an
  independently word-indexed `GpuGradientStyle` record in the same
  buffer.
- `Polygon`/`Path` have no per-vertex style record at all, so they route
  through an entirely new `PipelineKind::GradientFill` pipeline
  (`gradient_fill.frag`, reusing the existing `bindless_textured.vert`
  and its own `texture_index` push constant, repurposed as a gradient
  word index -- the same per-draw mechanism `TexturedQuad` already uses).

**A real coordinate-space bug found and fixed while building this
demo, not assumed away.** `sdf_rect_styled.frag`/`sdf_ellipse.frag`'s own
`frag_uv` is CENTER-relative (an internal shader convention for
symmetric SDF math), but a `GradientDef`'s own points are authored in
`Rectangle`/`Circle`'s PUBLIC local space (bounding-box top-left at the
origin -- the same convention their own `corner_radius`/`border`
thinking already uses). The first real run of this exact demo caught the
mismatch directly (a rectangle gradient that should have shown a mid-red
tone at one probe instead showed pure, unmixed red) -- fixed by
`build_gpu_gradient_style` subtracting each shape's own local-space
center offset (`[half_width, half_height]` for `Rectangle`, `radius` for
`Circle`) before writing the record, so gradient authors keep thinking in
the same top-left-relative coordinates as every other shape property.

**What this demo proves, not just "didn't crash."** A bordered rectangle
with a real red-to-blue linear gradient, a bordered circle with a real
white-to-green radial gradient, and a hexagon with a real red-to-green
linear gradient through the separate `GradientFill` pipeline -- every
probed pixel compared against an independent Rust reference
implementation of the exact same premultiplied, linear-space gradient
math the real shaders perform (mirroring `translucent_flat_fill_demo.
rs`'s own established discipline), not just "some color changed."
Border/gradient composition is proven too: both the rectangle and the
circle keep their own solid `border_color` at the edge while the
interior renders the gradient.

**Verified.** 11 new `tre-engine` unit tests (gradient validation,
GPU-record encoding, `flatten_into` wiring for all three real code
paths). Every pre-existing demo re-run and confirmed bit-for-bit
unchanged. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the whole workspace.
