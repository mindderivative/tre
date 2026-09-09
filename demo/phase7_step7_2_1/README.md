# Demo: Phase 7, Step 7.2.1 -- Real Dual-Kawase Blur RHI Capability

```bash
./demo/phase7_step7_2_1/run_dual_kawase_blur_demo.sh
```

Proves the real, hand-written RHI-level Dual-Kawase blur chain works
end to end on real GPU hardware -- REVIEW.md finding #130, closed
2026-09-08 after four separate investigation sessions.

**What it does:** draws a small opaque white square into a full-size
transient offscreen target (L0), downsamples it twice via a real 5-tap
Kawase filter (L0 -> L1 -> L2, half then quarter size), upsamples it
back twice via a real 8-tap Kawase filter (L2 -> U1 -> U0, half then
full size again), then composites U0 onto the swapchain. Every
downsample/upsample/composite pass samples through a plain,
conventionally-bound `COMBINED_IMAGE_SAMPLER` descriptor rather than the
engine's persistent bindless texture array -- a real, independently-
reasonable design matching how other engines (Windows Acrylic/Mica,
macOS's own backdrop materials, in-app game-engine post-process chains)
implement this exact kind of same-frame offscreen backdrop blur.

**The bug this closes, precisely:** this chain used to read back the
composited result's own background clear color everywhere, including
where the square's own blurred content should have appeared. Three
separate investigation sessions blamed the bindless texture array
itself, following real, reproducible (if ultimately misleading)
evidence -- a single-hop non-bindless experiment worked where every
bindless attempt failed. **The bindless array was never actually the
cause.** Converting the real 5-hop chain to that same non-bindless
approach still failed past the second hop. Bisecting hop by hop and
substituting one variable at a time (source texture, destination,
pipeline object) isolated the real defect to `RhiCommandBuffer::
draw_indexed` (`tre-rhi-vulkan/src/lib.rs`): it unconditionally performs
its own, second `cmd_push_constants` call using the render target's own
real dimensions, silently overwriting a manually-pushed `screen_size`
push constant right before the draw executes. This is normally
invisible -- except `RhiDevice::acquire_transient_target`'s own
documented "oversized borrow" fallback can (and, here, does) hand back a
texture *larger* than requested when no free bucket of the exact
requested size exists yet, which is exactly what happens the first time
this chain's later, smaller transient targets are acquired after its
earlier, larger ones have already been released. With the wrong,
oversized dimensions silently substituted into the vertex shader's NDC
mapping, the actual draw ends up confined to a small corner of the real
backing image, leaving its true center at the untouched clear color --
precisely the "reads back background" symptom chased under three
different framings.

**The fix:** every non-bindless pass in `dual_kawase_blur_demo.rs`
issues its draw via a raw `cmd_draw_indexed` call instead of
`RhiCommandBuffer::draw_indexed`, so the wrapper's own redundant,
potentially-stale push-constant call never executes for these passes.
The non-bindless sampling design itself is kept unchanged -- it remains
a real, reasonable choice, just never what was actually broken.

**This demo proves the fix directly, not via a proxy.** It reads back
real GPU pixels at two points: deep in the square's own original
interior (must stay mostly foreground, not washed to uniform gray by
the blur) and just outside the square's own original hard edge (must
show a genuine partial blend -- neither pure background, proving real
blur spread outward, nor pure foreground, proving the blend is genuine
and not a hard-edged artifact). Both assertions pass consistently across
repeated real runs against actual GPU hardware, with clean resource
cleanup (zero leaked Vulkan objects) every time.

`dual_kawase_blur_demo.rs`'s own header doc comment has the complete
technical account of all four investigation sessions. REVIEW.md finding
#130 has the full narrative, including every hypothesis tested and
ruled out along the way.

**Scope note:** this is RHI-level plumbing only -- proving the blur
chain itself works with hand-written Vulkan calls, not a `Canvas`-level
capability. Wiring this to `push_layer`/`pop_layer` so any layer can
request a real backdrop blur is Step 7.2.2, real and separate future
work, not attempted here.
