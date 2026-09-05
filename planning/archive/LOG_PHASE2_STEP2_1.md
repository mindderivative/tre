# Log: Phase 2, Step 2.1 -- Vulkan Bindless Texture Arrays

## Real bugs found during implementation (all caught by the Vulkan
validation layer, actually running the new example, not by code review)

1. **Missing `descriptorBindingSampledImageUpdateAfterBind` feature.**
   First run failed immediately at `vkCreateDescriptorSetLayout`:

   ```
   pCreateInfo->pNext<VkDescriptorSetLayoutBindingFlagsCreateInfo>.pBindingFlags[0]
   includes VK_DESCRIPTOR_BINDING_UPDATE_AFTER_BIND_BIT but pBindings[0].descriptorType
   is VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE but descriptorBindingSampledImageUpdateAfterBind
   was not enabled.
   ```

   `PLAN.md`'s feature list (`shader_sampled_image_array_non_uniform_indexing`,
   `descriptor_binding_partially_bound`,
   `descriptor_binding_variable_descriptor_count`,
   `descriptor_binding_update_unused_while_pending`,
   `runtime_descriptor_array`) was incomplete: `VK_EXT_descriptor_indexing`
   actually splits "may I use `UPDATE_AFTER_BIND` at all" into
   per-descriptor-type feature bits, and the general
   `descriptor_binding_update_unused_while_pending`/`partially_bound`/etc.
   flags don't imply the type-specific one. Fixed by also requesting
   `descriptor_binding_sampled_image_update_after_bind(true)`.

2. **`VARIABLE_DESCRIPTOR_COUNT` on the wrong binding.** Second run failed
   at the same call:

   ```
   pBindingFlags[0] (binding 0) includes VK_DESCRIPTOR_BINDING_VARIABLE_DESCRIPTOR_COUNT_BIT
   but can only be on the last binding element (binding 1).
   ```

   The original layout put the unbounded texture array at binding 0 and the
   fixed immutable sampler at binding 1 (matching IMPLEMENTATION.md's prose
   order, "an unbounded array of textures ... [and] a separate ... shared
   sampler"). Vulkan requires `VARIABLE_DESCRIPTOR_COUNT` to be on the
   *highest-numbered* binding in the set, unconditionally -- prose order and
   binding-number order aren't the same thing. Fixed by swapping: binding 0
   is now the fixed sampler, binding 1 is the unbounded array. Both the
   Rust-side layout/pool/write-descriptor code and
   `bindless_textured.frag`'s `layout(set = 0, binding = ...)` declarations
   were updated together; a mismatch between them would have been a third,
   silent bug (wrong binding sampled) rather than a caught validation error,
   since GLSL binding numbers and Rust `vk::DescriptorSetLayoutBinding`
   numbers aren't checked against each other by anything short of the
   pipeline actually producing correct pixels -- which the pixel-color
   assertions below did.

## A real design lesson found while writing the demo (not a bug in the RHI
itself, but a bug in the demo's first draft that reveals a genuine subtlety
of the API just built)

The demo's fourth quad was originally drawn by simply never calling
`bind_texture` for it, on the assumption that "no `bind_texture` call" means
"no texture, fall back to vertex color." That's true only for the FIRST
draw of a fresh command buffer (whose `texture_index` starts at the
sentinel) -- `bind_texture`'s bound index is ordinary command-buffer state
that persists across draws until explicitly changed, exactly like the
pipeline, vertex buffer, or scissor rect already do. Since the demo had
already called `bind_texture` for the `blue` texture immediately before,
skipping the call for draw 4 left `blue`'s index still bound, and the
"yellow" quad rendered as blue instead. Caught immediately by the pixel
assertion (`expected BGRA [0, 255, 255, 255], got [255, 0, 0, 255]`) -- not
a crash, a silently-wrong-but-plausible render, which is exactly the class
of bug pixel-content assertions exist to catch that "it didn't crash" would
have missed entirely. Fixed by having the demo explicitly rebind the
sentinel (`bind_texture(0, u32::MAX)`) before its fourth draw. No RHI code
changed -- the behavior was correct and intentional (stateful binding is
how every real graphics API's command buffer works); the demo's assumption
was wrong.

## What worked without needing a fix

- The core bindless mechanism itself: one descriptor set, bound exactly
  once per pipeline bind, three real uploaded textures sampled correctly by
  three separate draw calls that never rebind it -- confirmed both by zero
  further validation errors and by the pixel-exact PNG readback.
- The runtime capacity clamp (`min(4096, max_descriptor_set_update_after_
  bind_sampled_images)`) -- this machine's driver reported a limit well
  above 4,096, so the target capacity was used unclamped locally; whether
  Mesa lavapipe in CI reports something lower is exactly what the CI push
  in this step's verification plan checks for.
- The one-time staging-buffer upload path (`VulkanTexture::from_pixels`) --
  worked correctly on the first attempt once the descriptor-layout issues
  above were fixed, including the `UNDEFINED -> TRANSFER_DST_OPTIMAL ->
  SHADER_READ_ONLY_OPTIMAL` layout transitions and the blocking fence wait.
- `create_pipeline`'s extended universal layout (12-byte push constant
  range, bindless set at index 0) did not disturb any of the five
  pre-existing examples -- all five were re-run manually after this step's
  changes and still produce correct output with zero validation errors,
  confirming a pipeline layout can expose resources a given shader simply
  doesn't declare/consume.
