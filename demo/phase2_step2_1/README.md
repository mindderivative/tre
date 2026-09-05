# Demo: Phase 2, Step 2.1 -- Vulkan Bindless Texture Arrays

```bash
./demo/phase2_step2_1/run_bindless_textures_demo.sh
```

![four columns: red, green, blue, yellow](bindless_textures_output.png)

Before this step, there was no way to get a real image onto the GPU as a
sampled texture at all -- `acquire_transient_target` only created *empty*
render targets, and `RhiCommandBuffer::bind_texture` was a Phase 0
`unimplemented!()` stub. This step builds `RhiDevice::create_texture`
(a real, one-time CPU-pixels-to-GPU upload), registers each texture into
`tre-rhi-vulkan`'s new persistent bindless descriptor array
(`VK_EXT_descriptor_indexing`, IMPLEMENTATION.md Step 2.1), and implements
`bind_texture` for real.

**What the four columns above prove:** the demo uploads three distinct 4x4
solid-color textures (red, green, blue) via `create_texture`, then issues
one draw call per texture -- all four draws share the exact same bound
pipeline and the exact same bound descriptor set (bound once, inside
`VulkanCommandBuffer::set_pipeline`). Between draws, only a 4-byte push
constant (the bindless array index) changes; `vkCmdBindDescriptorSets` is
never called again after the one bind at the start. That "bind once, select
by index" property is the entire point of "bindless" -- traditional
descriptor sets would need a rebind per texture. The fourth (yellow) column
is drawn with the bindless sentinel (`u32::MAX`) explicitly bound, proving
every pre-existing flat-vertex-color draw path (all five earlier examples)
keeps working unchanged through the same, now-larger, pipeline layout.

**The example itself asserts this, not just eyeballs the PNG:** after
rendering, it reads back the actual output pixels and asserts the center of
each column exactly matches the color that texture (or vertex, for yellow)
was uploaded/specified with. Pure `0`/`255` channel values are used
throughout specifically so the render target's sRGB encode/decode round-trips
exactly, keeping the assertions exact rather than approximate.

**Two real bugs were found building this, both by the Vulkan validation
layer on the very first and second runs, not by review** (full detail in
`documentation/REVIEW.md`'s "Phase 2 Step 2.1 Implementation" section and
`planning/archive/LOG_PHASE2_STEP2_1.md`):

- A missing `descriptorBindingSampledImageUpdateAfterBind` feature request
  -- `VK_EXT_descriptor_indexing`'s general binding flags don't imply the
  per-descriptor-type "may I update-after-bind at all" feature.
- `VARIABLE_DESCRIPTOR_COUNT` placed on the wrong binding -- Vulkan requires
  it on the *highest-numbered* binding in the set, not wherever the prose
  in IMPLEMENTATION.md happened to list it first. Fixed by swapping binding
  numbers (fixed sampler at 0, unbounded array at 1) on both the Rust and
  GLSL sides together.

A third issue turned up in the demo's own first draft (not the RHI): it
assumed skipping `bind_texture` reset the bound index back to the sentinel.
It doesn't -- the bound index is ordinary command-buffer state that persists
across draws until explicitly changed, exactly like the pipeline or vertex
buffer. Caught immediately by the pixel-color assertion (the "yellow"
quad silently rendered blue instead), not by a crash.

**CI**: `bindless_textures_demo` was added to
[`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)'s
`vulkan-validation` job, so every push runs it (and its pixel assertions)
against Mesa lavapipe under `xvfb-run` -- the same job that already proves
the other five examples run correctly on a real, if software, Vulkan
implementation.
