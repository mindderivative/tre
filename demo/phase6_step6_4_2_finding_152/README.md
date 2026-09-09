# Demo: REVIEW.md Finding #152 -- PushLayer/PopLayer Oversized-Borrow Fix

```bash
./demo/phase6_step6_4_2_finding_152/run_layer_oversize_regression_demo.sh
```

Proves a real, currently-shipped bug in Step 6.4.2's own `PushLayer`/
`PopLayer` compositing is fixed -- found while checking whether Step
7.2.2 (wiring Dual-Kawase blur into `push_layer`/`pop_layer`) would be
building on solid ground.

**The bug, precisely:** `RhiDevice::acquire_transient_target`'s own
documented "oversized borrow" fallback can hand back a texture *larger*
than requested, whenever no free bucket of the exact requested size
exists yet but a larger, already-freed one does. `RhiCommandBuffer::
begin_render_to_texture`/`begin_render_to_texture_no_end` used to feed
that texture's own *real* dimensions into `draw_indexed`'s NDC-mapping
push constant -- so a layer's own inner content, recorded at `Canvas`
record time against the `LayerDesc`'s own smaller, *requested* size,
would have its vertex positions mapped against the wrong, larger size,
confining the actual draw to a small corner of the oversized image.
From the outside, this looked like the layer's own content simply
vanishing from the composited frame -- no error, no validation warning,
just missing content. Same underlying mechanism as REVIEW.md finding
#130's real root cause, reached through the standard, production
`PushLayer`/`PopLayer` path instead of a hand-rolled demo -- meaning
this bug has been live since Step 6.4.2 first shipped.

**What this demo does:** push+pop a first, larger layer (200x150, a
fresh transient-pool allocation), letting it release back to the pool at
frame end, then push+pop a second, smaller, never-before-requested layer
(50x40) in a later frame. The pool has no exact bucket for the second
size yet, but does have the first layer's own freed, larger one -- so it
hands that back instead. The second layer's own content (a rect nearly
filling its own local bounds) must still composite correctly.

**The fix:** `begin_render_to_texture`/`begin_render_to_texture_no_end`
now take explicit `logical_width`/`logical_height` parameters -- the
caller's own *intended* size, always already known (it's exactly what
was passed to `acquire_transient_target`) -- used only to set the
NDC-mapping push-constant source. Viewport/scissor/render area stay
driven by the texture's own real size, unchanged: content simply draws
"stretched" to fill it, a stretch exactly undone later when `PopLayer`'s
own composite quad samples it back through a normalized `(0,0)`-`(1,1)`
UV read and redraws it at its own real, requested on-screen size. No UV
rescaling or dynamic vertex-buffer rewriting needed anywhere.

**This demo proves the fix directly, not via a proxy.** It reads back
real GPU pixels at the second layer's own deep interior (must be real
foreground, not the background clear color -- this is the exact
regression) and at a point just outside the second layer's own
composited bounds (must be real background, proving the composite quad
stayed confined to its own requested on-screen size). Both assertions
pass consistently across repeated real runs against actual GPU hardware.

REVIEW.md finding #152 has the full technical account, including the
proof that leaving viewport/scissor/render area on the texture's real
size is still correct.
