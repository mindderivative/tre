# Demo: Phase 5, Step 5.1.1 -- Canvas Drawing-Context State Stack

```bash
./demo/phase5_step5_1_1/run_canvas_state_stack_demo.sh
```

The first sub-step of Phase 5's Canvas work: `RenderingCanvas` gains a
real hierarchical Drawing Context state stack -- `save`/`restore`
(transform + alpha) and a separate `push_clip`/`pop_clip` scissor stack,
per DESIGN.md's own architecture diagram naming them as two distinct
mechanisms -- wired into the one real primitive the Canvas already had
(`draw_rounded_rect`, Step 3.2).

Three rects prove three different things:

- **Rect A** (`save()`/`transform()`/`restore()`): drawn at local
  `(0,0,50,50)`, translated by `(70,60)`. Its transformed world position
  is confirmed filled with real rendered pixels; its raw, untransformed
  local position is confirmed still background -- proving the transform
  moved it rather than drawing an extra copy.
- **Rect B** (`save()`/`set_alpha(0.5)`/`restore()`): a genuine, visible
  partial blend against the background -- neither the fully-opaque
  foreground color nor untouched background.
- **Rect C** (`push_clip()`/`pop_clip()`): checked at the IR level (its
  recorded `UiDrawCommand::clip_bounds`) and, since Phase 6 Step 6.3,
  with a real GPU scissor test -- its own drawn geometry is deliberately
  larger than its own clip rect on every side, so a real `set_scissor`
  call genuinely has something to crop: a pixel inside both the geometry
  and the clip reads real foreground, the corresponding pixel inside the
  geometry but outside the clip reads real background, proving the
  content was actually cropped on real hardware, not just recorded in
  the IR.

**A real bug found along the way, not just designed around in the
abstract.** The first draft of the alpha-scaling logic reduced only the
vertex color's alpha byte, leaving RGB at full brightness -- which
rendered every `set_alpha()` call completely invisible, not merely
imprecise, because `sdf_rounded_rect.frag`'s own output formula never
multiplies `frag_color.rgb` by `frag_color.a`, only by the SDF's own
coverage term. `UiVertex::color` must already be premultiplied by
whatever effective alpha a caller wants; this demo's own first real GPU
run caught it (Rect B rendered pixel-identical to fully opaque), fixed by
scaling all four channels together instead of just one. See REVIEW.md
finding #118 and this step's own `LOG.md` for the full account.
