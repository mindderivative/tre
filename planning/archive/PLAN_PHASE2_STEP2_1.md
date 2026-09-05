# Plan: Phase 2, Step 2.1 -- Vulkan Bindless Texture Arrays

## Scope decision (confirmed with project owner 2026-09-04/2026-09-05)

Corresponds to IMPLEMENTATION.md's Step 2.1. Tasks 2 (DirectX 12) and 3 (Metal)
are deferred entirely, per the standing precedent re-confirmed when Phase 2
was first planned: neither backend exists, and neither can be built or
verified without a Windows/macOS machine. This step implements task 1
(Vulkan) only -- the `VK_KHR_dynamic_rendering` half of task 1 is already
done (Phase 0 built the whole render loop on dynamic rendering; there has
never been a `VkRenderPass`/`VkFramebuffer` in this codebase). What remains,
and what this step actually builds, is the second half: "a universal
pipeline layout that exposes an unbounded array of textures `texture2D
textures[]` via `VK_EXT_descriptor_indexing`."

**Starting point, stated plainly:** there is currently no way to get a real
image onto the GPU as a sampled texture at all. `acquire_transient_target`
creates *empty* render targets (written to, never sampled from). There is no
`create_texture`-from-pixels, no `VkSampler`, no descriptor set of any kind
in `tre-rhi-vulkan`, and `RhiCommandBuffer::bind_texture` is a Phase 0 stub
(`unimplemented!("Phase 4 ... out of Phase 0's scope")`). This step builds
all of that from nothing.

**What "bindless" means for this step, precisely, and what it doesn't:**
DESIGN.md Section 8.1.2 describes the *end-state* batching win -- "a single
draw call samples from multiple atlases dynamically based on a texture
index passed in the vertex data" -- but that requires a per-vertex texture
index and the atlas-packing system that decides what goes in `UiVertex`'s
already-fully-packed 32 bytes, neither of which exists yet (that's
Phase 3/4's `Canvas`-to-RHI renderer, which doesn't exist yet either -- the
IR's `UiDrawCommand::texture_handle` field is currently always `0` and
nothing consumes it). This step proves the RHI primitive IMPLEMENTATION.md
Step 2.1 actually asks for: **one persistent, unbounded descriptor-indexed
texture array, bound once, sampled by index, with zero descriptor-set
rebinding between draws that use different textures.** The index is passed
as a push constant (one per draw call), not per-vertex -- a real, working,
spec-correct use of `VK_EXT_descriptor_indexing`, just scoped at the
draw-call granularity rather than the vertex granularity. Wiring a per-vertex
index into the canonical `UiVertex`/sort-key format is explicitly Phase 3/4
work, once the atlas/batching system exists to make that wiring meaningful.

**Design mirrors IMPLEMENTATION.md's exact wording:** "an unbounded array of
textures `texture2D textures[]`" is a *separate*-image-and-sampler bindless
layout (one unbounded array of `SAMPLED_IMAGE`, one fixed shared `SAMPLER`
binding) -- not an array of `COMBINED_IMAGE_SAMPLER`. This is also the more
flexible modern convention and avoids needing one sampler per texture.

**Capacity is clamped to the real device limit, not assumed:**
`VK_EXT_descriptor_indexing`'s `UPDATE_AFTER_BIND`/`VARIABLE_DESCRIPTOR_COUNT`
machinery still requires declaring a maximum array size at descriptor-set-
layout-creation time. ARCHITECTURE.md Section 4.1's sort key already commits
to a 12-bit (4,096-slot) texture ID field, so 4,096 is the natural target --
but it must be checked against
`VkPhysicalDeviceDescriptorIndexingPropertiesEXT::maxDescriptorSetUpdateAfterBindSampledImages`
at runtime and clamped down if a real device (e.g. Mesa lavapipe in CI, a
software rasterizer with no reason to advertise generous bindless limits)
reports less. This is exactly the same "don't assume, query and degrade"
discipline Phase 2 Step 2 applied to the validation layer.

**Texture creation stays fully synchronous, matching every other RHI
operation built so far:** `create_texture` uploads through a staging buffer
(reusing the existing `upload_buffer` helper) and a one-time command buffer
that is submitted and *waited on* before returning -- blocking, not
pipelined. This matches Phase 2 Step 1's explicit "frame submission stays
synchronous" scope decision; an async/staged upload path is future work, not
silently pretended to exist here.

## Goal

`tre-rhi-vulkan` can create a real sampled texture from CPU pixel data,
register it into one persistent bindless descriptor array, and sample it
from a shader via an index selected per draw call at runtime -- with the
descriptor set bound exactly once and never rebound between draws that
reference different textures. Proven by a new example that uploads several
distinct real textures and draws each in its own draw call, verified both
by zero validation-layer errors and by inspecting actual output pixels.

## Tasks

1. **`RhiTexture` gains `fn bindless_index(&self) -> Option<u32>`** (`None`
   for transient render targets, which stay out of the bindless registry
   this step -- see "explicitly out of scope" below). `RhiDevice` gains
   `fn create_texture(&self, width: u32, height: u32, format: TextureFormat,
   pixels: &[u8]) -> Box<dyn RhiTexture>`.

2. **`VulkanDevice::new` adds `VK_EXT_descriptor_indexing`** to
   `REQUIRED_DEVICE_EXTENSIONS` (a hard requirement, per TECHNICAL.md
   Section 2.1 -- unlike the validation layer, this is not gracefully
   degraded) and chains
   `vk::PhysicalDeviceDescriptorIndexingFeaturesEXT` (requesting
   `shader_sampled_image_array_non_uniform_indexing`,
   `descriptor_binding_partially_bound`,
   `descriptor_binding_variable_descriptor_count`,
   `descriptor_binding_update_unused_while_pending`,
   `runtime_descriptor_array`) onto the existing `push_next` chain alongside
   `dynamic_rendering_feature`. Queries
   `PhysicalDeviceDescriptorIndexingPropertiesEXT` to compute
   `bindless_capacity = min(4096, max_descriptor_set_update_after_bind_sampled_images)`.

3. **Create, once, in `VulkanDevice::new`:** a `vk::Sampler` (linear
   filtering, clamp-to-edge -- a reasonable default for UI textures/atlases);
   a descriptor pool with `UPDATE_AFTER_BIND_BIT`; a descriptor set layout
   with binding 0 = `SAMPLED_IMAGE` array of `bindless_capacity` (flags
   `PARTIALLY_BOUND | UPDATE_AFTER_BIND | VARIABLE_DESCRIPTOR_COUNT`),
   binding 1 = a single fixed `SAMPLER`; one descriptor set allocated with
   `DescriptorSetVariableDescriptorCountAllocateInfo` requesting the full
   `bindless_capacity` at binding 0. A `Mutex<BindlessRegistry>` (free-list
   + next-index bump allocator, same shape as the existing transient pool's
   `Mutex`-guarded state) tracks which indices are live.

4. **`create_pipeline` becomes the one place the universal layout is
   defined:** add the bindless descriptor set layout to
   `vk::PipelineLayoutCreateInfo`, and extend the push constant range from
   8 bytes (`vec2 screen_size`) to 12 bytes (`vec2 screen_size, uint
   texture_index`). Existing shaders (`walking_skeleton.vert`/`.frag`,
   unchanged) simply don't declare the extra 4 bytes or the descriptor set
   -- a pipeline layout may expose resources a given shader doesn't consume,
   so `walking_skeleton`/`multi_window`/`headless`/`input_demo` keep working
   exactly as before, unmodified.

5. **Implement `VulkanTexture::from_pixels`** (image usage
   `SAMPLED | TRANSFER_DST`, `DEVICE_LOCAL` memory -- otherwise identical
   memory-type-selection code to the existing `VulkanTexture::new`): stage
   via `upload_buffer`, then a temporary one-time command buffer records
   `UNDEFINED -> TRANSFER_DST_OPTIMAL`, `cmd_copy_buffer_to_image`,
   `TRANSFER_DST_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL`, submitted and waited
   on before the temporary command buffer is freed. Registers the resulting
   image view into the bindless registry (one `vkUpdateDescriptorSets` call)
   and stores the assigned index on the `VulkanTexture`. `Drop for
   VulkanTexture` releases the index back to the registry's free list
   (guarded by `Option`, since transient-pool textures never get one) in
   addition to its existing view/image/memory teardown.

6. **Implement `RhiCommandBuffer::bind_texture`** for real: stores
   `bindless_index` on `VulkanCommandBuffer` (the `slot` parameter is
   unused/asserted-zero this step -- only one array binding exists; a second
   slot for e.g. a separate mask-atlas array is future work, not built
   speculatively here). `set_pipeline` binds the persistent bindless
   descriptor set once via `cmd_bind_descriptor_sets` (it never changes, so
   binding it every `set_pipeline` call is correct and cheap). `draw_indexed`
   includes `texture_index` in its existing push-constant write, using a
   sentinel (`u32::MAX`) meaning "no texture, use vertex color" when
   `bind_texture` was never called for this draw -- so nothing about the
   Phase 0 flat-color rendering path breaks by default.

7. **New shader pair + example**
   (`crates/tre-rhi-vulkan/shaders/bindless_textured.{vert,frag}`,
   `examples/bindless_textures_demo.rs`): synthesizes several small, visually
   distinct RGBA8 pixel buffers in Rust (solid colors or a checkerboard --
   no image-decoding dependency needed), uploads each via `create_texture`,
   and issues one `draw_indexed` per texture -- each a full quad positioned
   in a different screen region -- through the *same* bound pipeline and
   *same* bound descriptor set, varying only the push-constant
   `texture_index` between draws. Captures a headless PNG (reusing the
   existing `headless` example's output path) and asserts specific pixel
   colors match each uploaded texture's known content, proving real sampling
   happened, not just that the draw calls didn't crash.

8. **CI**: add `bindless_textures_demo` to the `vulkan-validation` job's
   example list in `.github/workflows/ci.yml`.

## Verification plan

- Local: `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across
  the workspace.
- Local: all five existing examples still run unmodified and correctly --
  proving the new descriptor set/extended push constants didn't disturb the
  non-textured path.
- Local: `bindless_textures_demo` runs under
  `VK_LAYER_KHRONOS_validation` (loaded automatically per Phase 2 Step 2)
  with zero validation errors, and its own pixel-color assertions pass
  against the captured PNG.
- CI: push, `gh run list --branch main --limit 1` / `gh run view` to confirm
  all jobs including the extended `vulkan-validation` job pass for real on
  Mesa lavapipe -- the actual test of whether `bindless_capacity`'s
  device-limit clamp is doing real work, since a software rasterizer is
  exactly the kind of implementation likely to report a lower
  `maxDescriptorSetUpdateAfterBindSampledImages` than a desktop GPU.
- If lavapipe's real limit turns out to be surprising (very low, or the
  extension/features aren't actually supported at all despite being listed
  as required), that becomes a documented finding, not a silently patched
  assumption -- matching how Phase 2 Step 2's CI gap was handled.

## Explicitly out of scope for this step

- DirectX 12 (Resource Binding Tier 3 root signatures) and Metal (Argument
  Buffers Tier 2) -- neither backend exists; deferred with them.
- Per-vertex texture indexing and the atlas/batching system DESIGN.md
  Section 8.1.2 describes -- that's Phase 3/4's `Canvas`-to-RHI renderer,
  which doesn't exist yet. This step's index is per-draw-call.
- Registering transient render targets (`acquire_transient_target`'s output)
  into the bindless array -- real, but not needed until `PushLayer`
  offscreen compositing (Phase 3+) actually wants to sample a previously
  rendered layer as an input texture.
- Async/pipelined texture upload -- `create_texture` blocks on a
  `queue_wait_idle`-equivalent fence wait, matching every other RHI
  operation's current synchronous scope.
- A second bindless array/slot (e.g. a separate mask-atlas array) -- only
  one array exists; `bind_texture`'s `slot` parameter is accepted but
  unused.
