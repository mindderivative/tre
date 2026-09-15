# Demo: Phase 7, Step 7.2.2 -- Wire Dual-Kawase Blur to push_layer/pop_layer

```bash
./demo/phase7_step7_2_2/run_layer_blur_demo.sh
```

Proves a real, GPU-accelerated Dual-Kawase blur is now a working
`Canvas`-level capability, not just RHI-level plumbing (Step 7.2.1) --
`RenderingCanvas::push_layer(&LayerDesc { blur: true, .. })`, a draw
call, `pop_layer()` are the *only* calls this demo makes; `execute_frame`
alone drives every real RHI call, including the blur chain itself
(`RhiCommandBuffer::apply_layer_blur`), from the resulting IR.
Own-content blur only -- it blurs the layer's own newly-drawn content,
not whatever is visually behind it on the swapchain; a true backdrop
blur needs a real swapchain snapshot/copy capability this engine
doesn't have yet, real, disclosed future work.

**Design, briefly:** `LayerDesc.blur` (a fixed-depth `bool` toggle, no
tunable radius/quality yet) rides `PopLayer`'s own otherwise-inert
`texture_handle` IR field. `execute_frame`'s `PopLayer` handling, on
seeing it set, calls the new `RhiCommandBuffer::apply_layer_blur` --
one purpose-built method (matching `begin_render_to_texture_no_end`'s
own precedent) that internally chains the exact same non-bindless
downsample/upsample mechanism `dual_kawase_blur_demo.rs` already proved
(Step 7.2.1, REVIEW.md finding #130's real fix), graduated into real,
reusable engine capability with its own lazily-created, cached
pipelines/descriptors (`VulkanDevice::blur_resources`) -- no caller
needs any awareness that blur pipelines exist at all. The blurred
result is released and composited exactly like an un-blurred layer
already was; the `blur: false` path is completely untouched.

**Two real bugs found by actually running this on real GPU hardware,
not caught by design review:**
- REVIEW.md finding #152 (this step's own prerequisite, fixed first):
  `acquire_transient_target`'s documented "oversized borrow" fallback
  silently broke the *existing*, already-shipped `PushLayer`/`PopLayer`
  compositing whenever it fired -- found while checking this step would
  build on solid ground, fixed at the shared `begin_render_to_texture`
  trait level before this step's own design was finalized.
- REVIEW.md finding #153: `apply_layer_blur`'s own internal hops left
  its own tiny unit-quad vertex/index buffers bound on the command
  buffer, with nothing restoring the frame's real, shared ones before
  the composite draw immediately following -- a real out-of-bounds
  index read this demo's own first GPU run caught via Vulkan
  validation. Fixed by having `execute_frame` re-bind the frame's real
  buffers right after `apply_layer_blur` returns.

A third real finding, disclosed but not chased further here: a scratch
test during this step's own pre-work found that REVIEW.md finding
#152's general fix does *not* fully explain finding #130's original
symptom -- the bindless array genuinely still cannot sample a texture
while rendering into a *different* offscreen target, a real, separate,
still-unexplained defect. This step's own blur chain is unaffected,
since it reuses the demo's already-proven non-bindless mechanism
throughout, and its own final composite step samples while rendering
into the *swapchain* -- the one condition already proven to work.

**This demo proves the fix directly, not via a proxy.** It reads back
real GPU pixels at two points, matching `dual_kawase_blur_demo.rs`'s
own methodology exactly: deep in the drawn square's own original
interior (must stay mostly foreground, not washed to uniform gray) and
just outside its own original hard edge (must show a genuine partial
blend -- neither pure background, proving real blur spread, nor pure
foreground, proving the blend is genuine). Both assertions pass
consistently across repeated real runs against actual GPU hardware.

REVIEW.md findings #130, #152, and #153 have the full technical
accounts. IMPLEMENTATION.md Step 7.2.2 has the complete design writeup.

**Scope note:** own-content blur only, fixed chain depth, no tunable
radius/quality yet -- true backdrop blur and runtime-tunable blur
quality remain real, separate future work, unchanged from Step 7.2.1's
own original scope decisions.
