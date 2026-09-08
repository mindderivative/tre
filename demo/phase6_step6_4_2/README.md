# Demo: Phase 6, Step 6.4.2 -- Wiring `push_layer`/`pop_layer` to Real Render-to-Texture

```bash
./demo/phase6_step6_4_2/run_canvas_layer_composite_demo.sh
```

Reproduces Step 6.4.1's own `render_to_texture_demo`'s scene and pixel
coordinates, but recorded entirely through `Canvas`/`execute_frame` --
`push_layer`/draw/`pop_layer` are the only calls that build the scene;
no hand-written RHI calls anywhere in this demo:

1. `canvas.push_layer(&LayerDesc { .. })` records a `PushLayer` marker
   and pushes it onto the canvas's own layer stack.
2. `canvas.draw_rounded_rect(..)`, called while the layer is still open,
   records an ordinary `DrawGeometry` command in the layer's own local
   coordinate space -- nothing about `draw_rounded_rect` itself is
   layer-aware; it's `execute_frame`'s own command-stream ordering that
   makes this land inside the layer's texture.
3. `canvas.pop_layer()` records a `PopLayer` command carrying a real,
   already-baked composite-quad geometry sized to the `LayerDesc`'s own
   on-screen `x`/`y`/`width`/`height`.
4. `execute_frame` walks the flattened IR: `PushLayer` acquires a
   transient target and redirects rendering into it, the `DrawGeometry`
   in between draws the rounded rect into it, `PopLayer` ends that
   render, registers the result bindless, resumes swapchain rendering
   (re-applying the current clip stack afterward), and draws the
   composite quad using the just-registered bindless index -- not the
   IR's own `NO_TEXTURE` placeholder.

Same two real pixel checks `render_to_texture_demo` proved, at the same
swapchain coordinates, since this demo reproduces its exact scene:

- **Composited interior** (the rounded rect's own deep interior,
  composited onto the swapchain): real, exactly opaque foreground --
  proving content genuinely recorded via `Canvas`, rendered into the
  offscreen target by `execute_frame`'s own `PushLayer` handling, and
  survived the round trip back via its own `PopLayer` handling.
- **Composited transparent area**: real background, showing through
  exactly -- proving the layer target was actually cleared to
  transparent and the default blend state composites it correctly.

**Two real design gaps found before any code was written, not
discovered afterward.** `LayerDesc` had no compositing position at all
-- `push_layer` had silently hardcoded it to the origin, never
exercised by any real caller until this step. `RenderingCanvas::
layer_depth` was a bare balance counter, unable to recover a popped
layer's own width/height/format -- became `layer_stack: Vec<LayerDesc>`.

**A real bug found and fixed before any test ran.** `segment_and_flatten`
only rewrote `vertex_offset`/copied indices for commands inside a
`DrawGeometry` run -- every marker, `PopLayer` included, passed through
unchanged. `PopLayer`'s own new composite-quad geometry would have kept
a `vertex_offset` pointing into the canvas's raw, pre-flatten indices
instead of the flattened buffer the RHI index buffer is actually built
from. Fixed by rebasing any boundary command whose `element_count > 0`
the same way `DrawGeometry` commands already are. See REVIEW.md finding
#129 for the full account.
