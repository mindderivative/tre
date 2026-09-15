# Demo: Phase 6, Step 6.5 -- The Combining Capstone

```bash
./demo/phase6_step6_5/run_canvas_combined_scene_demo.sh
```

Phase 6's own closer: one real recorded `Canvas` scene, submitted as a
single `execute_frame` call, that exercises real clipping, real layer
compositing, and three real pipelines *together* -- proving they
interoperate correctly, not just each in isolation. Named explicitly in
Step 6.1's own original plan (`planning/archive/PLAN_PHASE6_STEP6_1.md`'s
"Scope decisions") as Phase 6's own intended closer, matching Step
5.3.3's own precedent of a final capstone proving previously-separate
pieces together.

No prior demo combines all of this: `canvas_state_stack_demo.rs`
combines `DrawGeometry`+`PushScissor`/`PopScissor`; `canvas_layer_
composite_demo.rs` combines `DrawGeometry`+`PushLayer`/`PopLayer`. This
demo does both, plus real text, in one scene:

1. `push_clip`/`draw_rounded_rect` (deliberately oversized vs. the
   clip)/`pop_clip` -- a rect drawn directly onto the swapchain via
   `PipelineKind::SdfRoundedRect`, real GPU scissor cropping proven.
2. `push_layer`/`draw_text` (real shaped word, real resolved MSDF atlas,
   `PipelineKind::MsdfText`)/`pop_layer` -- text rendered into an
   offscreen layer, then composited back via `PipelineKind::TexturedQuad`.

Three real pipeline ids, three real declared formats, no id reused at
two formats in the same frame -- `SdfRoundedRect` only ever draws
directly onto the swapchain here, `MsdfText` only ever draws inside the
layer, `TexturedQuad` only ever composites.

Three real, independent pixel checks:

- **Clip cropping**: a point inside both the clip and the drawn geometry
  reads real foreground; the corresponding point inside the geometry but
  outside the clip reads real background -- genuinely cropped by a real
  GPU scissor test, not just recorded in the IR.
- **Composited text**: each glyph's own on-screen quad, recomputed
  independently and offset by the layer's own composite origin, is
  scanned for real fill -- proving the text genuinely rendered into the
  offscreen layer and survived the round trip back.
- **Composited empty layer area**: a point inside the layer's own
  composited bounds but with no glyph there reads real background,
  proving the layer was genuinely cleared to transparent.

No new `tre-engine`/RHI surface -- every piece this demo combines
(`execute_frame`'s `DrawGeometry`/`PushScissor`/`PopScissor`/`PushLayer`/
`PopLayer` handling) already existed from Steps 6.2-6.4.2. This step is a
pure integration proof, not new engine work, and passed on its first
real run with no bugs found.
