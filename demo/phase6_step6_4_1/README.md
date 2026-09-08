# Demo: Phase 6, Step 6.4.1 -- Real RHI Render-to-Texture Capability

```bash
./demo/phase6_step6_4_1/run_render_to_texture_demo.sh
```

The first time this project has ever rendered into an offscreen target
and sampled the result back, on real Vulkan hardware. No `Canvas`/IR
involvement at all -- `push_layer`/`pop_layer` stay real but inert
markers until Step 6.4.2 wires them to this same capability; every RHI
call here is hand-written, proving the capability itself in isolation:

1. Acquire a transient target (`Rgba16Float`, ARCHITECTURE.md Section 5's
   own prescription for offscreen layers).
2. `begin_render_to_texture`/`end_render_to_texture` redirect rendering
   into it -- a real rounded rect, via the existing, unmodified
   `sdf_rounded_rect` pipeline (built against the layer's own
   `R16G16B16A16_SFLOAT` format, not the swapchain's).
3. `register_bindless` makes the rendered-into texture sample-able.
4. `resume_swapchain_rendering` returns to the swapchain `begin_frame`
   originally set up, preserving what it already had drawn.
5. The layer is composited back as a textured quad via the existing,
   unmodified bindless-textured pipeline and the default
   premultiplied-alpha blend state every pipeline already carries -- no
   special-casing needed for a correct "over" composite.
6. `deregister_bindless` then `release_transient_target` return the
   texture to the pool, exactly as any other transient-target user must.

Two pixels prove the round trip is real, not just "didn't crash":

- **Composited interior** (the rounded rect's own deep interior,
  composited onto the swapchain): real, exactly opaque foreground --
  proving content genuinely rendered into the offscreen target and
  survived the trip back.
- **Composited transparent area** (inside the composited region's own
  bounds, but outside the rect's own rounded footprint): real
  background, showing through exactly -- proving the layer target was
  actually cleared to transparent (not opaque or garbage) and that the
  default blend state composites it correctly with no special-casing.

**A real bug found by this demo's own first real run, not designed
around in the abstract.** Vulkan's dynamic viewport/scissor state is a
persistent property of the command buffer, not scoped to one
`cmd_begin_rendering` instance -- `begin_render_to_texture` correctly
sets its own viewport for the layer's smaller size, but the first draft
of `resume_swapchain_rendering` never restored it, so the composite
draw silently rendered through the *layer's* stale, smaller viewport
instead of the swapchain's own. The validation layer never flagged it
(a smaller-than-target viewport isn't itself invalid); only checking the
real composited pixel caught it, exactly the discipline this demo exists
to enforce. Fixed by having `resume_swapchain_rendering` explicitly
restore both viewport and scissor to the real swapchain extent, rather
than relying on a caller to remember to. See REVIEW.md finding #128 for
the full account.
