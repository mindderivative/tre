# Plan: Phase 6, Step 6.4.1 -- Real RHI Render-to-Texture Capability

## Scope decisions (confirmed with the project owner, 2026-09-08)

**Step 6.4 ("Real PushLayer/PopLayer Execution") is split into two
sub-steps**, confirmed with the project owner after a real-code
investigation found it needed genuinely new, cross-cutting RHI trait
surface, not just engine-layer wiring like Steps 6.1-6.3:

- **6.4.1 (this plan):** the real RHI capability -- begin/end rendering
  into an arbitrary `RhiTexture` mid-command-buffer, with correct layout
  transitions, plus registering an already-rendered-into texture into
  the bindless array so it can be sampled afterward. Proven directly,
  driven by hand-written calls in a new demo -- no `Canvas`/IR
  involvement yet.
- **6.4.2 (next):** wires `Canvas::push_layer`/`pop_layer`'s real IR
  commands to this capability inside `execute_frame`, including
  threading `LayerDesc::format` through the IR (currently silently
  dropped) and a real, composited capstone driven by an actual recorded
  scene instead of hand-written calls.

**Confirmed, real, previously-unverified facts this plan depends on**
(an Explore-agent investigation read the actual source for each; nothing
here is assumed):
- `VulkanTexture` (what `acquire_transient_target` returns) is already
  created with `vk::ImageUsageFlags::COLOR_ATTACHMENT | SAMPLED`
  (`tre-rhi-vulkan/src/lib.rs:2878`) and a real `vk::ImageView`
  (`:2932-2947`) -- fully sufficient as a render-attachment target
  already; no texture-creation change needed.
- Nothing anywhere renders into an acquired transient target today --
  `gc_demo.rs`/`memory_pools_demo.rs` (the only two real callers of
  `acquire_transient_target`) immediately release what they acquire,
  exercising only the pool's own bucketing/GC.
- The only real `cmd_begin_rendering`/`RenderingInfo` construction in
  the whole codebase is inside `VulkanDevice::begin_frame`
  (`:1794-1836`), hard-wired to the swapchain's own color *and* stencil
  views -- a transient target has no stencil image at all, so a layer's
  own rendering scope cannot reuse that exact code path unmodified; it
  needs its own, stencil-free `RenderingInfo`.
- `RhiTexture::bindless_index()` returns `None` for every transient
  target by original design ("written to, not sampled from",
  `tre-engine`'s own doc comment) -- compositing a layer back as a
  sampled quad is blocked without registering it, and no existing
  method does this after the fact; `create_texture`'s own registration
  step (allocate a bindless slot, write a `WriteDescriptorSet` pointing
  it at the texture's `view`, `tre-rhi-vulkan/src/lib.rs:3293-3319`) is
  small and genuinely separable from that method's own pixel-upload
  work -- confirmed by reading it directly, not assumed reusable.
- The plain bindless-textured pipeline (`bindless_textured.vert` +
  `bindless_textured.frag`, used by `bindless_textures_demo.rs`) does a
  pure passthrough texture sample with no MSDF-specific math -- directly
  reusable as the compositing shader with no change.
- `VulkanDevice::create_pipeline`'s default blend state
  (`tre-rhi-vulkan/src/lib.rs:863-871`) is already real, unconditional
  premultiplied-alpha "over" blending (`ONE`/`ONE_MINUS_SRC_ALPHA` on
  both color and alpha channels) for every pipeline -- if a layer's own
  target is rendered with this same default state (cleared to
  transparent black first), compositing it back needs no special-casing
  at all; the existing default is already exactly right.

**Deliberately scoped to single-level layer use only, not nested
layers.** Nothing anywhere calls `push_layer`/`pop_layer` today, so
supporting a `push_layer` nested inside another `push_layer`'s own
bracket -- which needs a real "resume rendering into the *outer* layer
without re-clearing its already-drawn content" operation, distinct from
"begin fresh, cleared rendering into a brand-new target" -- has no real
scene to prove it against yet and would be speculative work. This
step's own capability only needs to resume the swapchain (whose own
content, from `begin_frame`, must never be re-cleared) after exactly one
texture-targeted rendering scope ends -- nested-layer support is real,
separate future work once `Canvas` actually records a scene that needs
it (deferred the same way this project deferred Windows/macOS
accessibility, blur filters, and HDR -- named honestly, not silently
dropped).

**Both layout-transition barriers this step needs have statically known
old/new layouts, given this scope -- no per-texture "current layout"
tracking field needed on `RhiTexture`.** Acquiring a target for layer
rendering always transitions from `UNDEFINED` (discarding whatever
content a reused pooled texture happened to have, which is correct and
intentional -- the target is cleared to transparent immediately after
regardless) to `COLOR_ATTACHMENT_OPTIMAL`; ending layer rendering always
transitions from `COLOR_ATTACHMENT_OPTIMAL` (guaranteed, since nothing
else touches the image layout between these two calls within one
command buffer) to `SHADER_READ_ONLY_OPTIMAL`.

## Goal

Two new real `RhiCommandBuffer` methods --
`begin_render_to_texture(&mut self, texture: &dyn RhiTexture)` and
`end_render_to_texture(&mut self, texture: &dyn RhiTexture)` -- plus
`resume_swapchain_rendering(&mut self)`, and a new `RhiDevice` method to
register an already-created texture into the bindless array after the
fact (`register_bindless` or similar) paired with `release_transient_
target` correctly deregistering it. A new, minimal demo -- driven
entirely by hand-written calls, no `Canvas` involved -- proves the full
real round trip on real Vulkan hardware: acquire a transient target,
render a real shape into it via `begin_render_to_texture`/`end_render_
to_texture`, register it bindless, resume swapchain rendering, composite
it back as a textured quad via the existing bindless-textured pipeline
and default blend state, read back real pixels confirming the
composited content appears correctly, then release the target.

## Tasks

1. **`RhiCommandBuffer::begin_render_to_texture`** (`tre-engine` trait +
   `tre-rhi-vulkan` impl): issues the real `UNDEFINED -> COLOR_ATTACHMENT_
   OPTIMAL` barrier for `texture`'s own image, ends whatever rendering
   scope is currently active (`cmd_end_rendering`), and begins a new one
   (`cmd_begin_rendering`) targeting `texture`'s own view, cleared to
   transparent black (`(0,0,0,0)`), with no stencil attachment -- a
   stencil-free `RenderingInfo`, distinct from `begin_frame`'s own.

2. **`RhiCommandBuffer::end_render_to_texture`**: ends the texture-
   targeted rendering scope (`cmd_end_rendering`) and issues the real
   `COLOR_ATTACHMENT_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL` barrier.

3. **`RhiCommandBuffer::resume_swapchain_rendering`**: re-begins
   rendering into the original swapchain image `begin_frame` set up.
   Requires `VulkanCommandBuffer` to stash the swapchain's own real
   attachment info (color view, stencil view, extent) at `begin_frame`
   time, since nothing currently persists it past that one function's
   own local scope -- a real, small change to `begin_frame` itself, not
   just a new method.

4. **A new `RhiDevice` method to register an existing texture into the
   bindless array** (exact name/signature TBD during implementation,
   e.g. `register_bindless(&self, texture: &mut dyn RhiTexture) ->
   Result<(), EngineError>`): factors the real allocate-slot-and-write-
   descriptor logic `VulkanTexture::from_pixels` already has
   (`tre-rhi-vulkan/src/lib.rs:3293-3319`) into a form callable on an
   already-existing `VulkanTexture` (the transient target, post-
   rendering), not just at pixel-upload time. `release_transient_target`
   updated to deregister the bindless slot (if one was allocated) before
   returning the texture to the pool, so repeated layer use across many
   frames cannot leak bindless slots.

5. **New demo** (`crates/tre-rhi-vulkan/examples/render_to_texture_demo.rs`,
   `demo/phase6_step6_4_1/`): acquires a transient target (e.g.
   `200x150`, `Rgba16Float` per ARCHITECTURE.md Section 5's own
   prescription for offscreen layers), uses `begin_render_to_texture`/
   `end_render_to_texture` to render one real shape into it via the
   existing, unmodified `sdf_rounded_rect` pipeline, registers it
   bindless, calls `resume_swapchain_rendering`, then composites it back
   onto the swapchain as a textured quad via the existing bindless-
   textured pipeline (a hand-built quad covering the composited region --
   no `Canvas` involvement, this step proves the RHI capability in
   isolation). Reads back real swapchain pixels confirming the
   composited shape appears at its expected position with real,
   non-background color -- the actual, first real proof this project has
   ever rendered into an offscreen target and sampled the result back.
   Releases the transient target at the end.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- New demo run under `VK_LAYER_KHRONOS_validation`, zero errors --
  validation is exactly the right check for real layout-transition
  correctness (a wrong old/new layout, or a missing barrier, is
  precisely the kind of thing this layer catches that a passing pixel
  readback alone would not).
- All pre-existing examples re-run manually end to end, zero
  regressions (this step touches shared RHI trait/impl code --
  `begin_frame`'s own stashing addition and the two demos calling it,
  `canvas_batch_flattening_demo`/`canvas_sub_canvas_demo`/`canvas_state_
  stack_demo`, must still render identically).
- CI: add the new demo to the `vulkan-validation` job's example list.

## Explicitly out of scope for this sub-step

- Any `Canvas`/IR involvement -- `push_layer`/`pop_layer` stay real IR
  markers with no RHI effect (`execute_frame`'s own no-op match arm)
  until Step 6.4.2.
- Nested layer support (resuming an *outer layer's* own rendering
  without re-clearing it, as opposed to resuming the swapchain) -- see
  "Scope decisions" above for why this is a real, deliberate deferral,
  not an oversight.
- Blur or any other visual filter on the composited layer -- Phase 7
  Step 7.2 (Step 6.1's plan), unchanged.
- Any change to `tre-rhi-dx12`/`tre-rhi-metal` beyond whatever keeps
  their existing 9-line stubs compiling against the widened
  `RhiCommandBuffer`/`RhiDevice` traits -- both are placeholder-only,
  never implemented, same precedent as every other RHI trait addition
  so far.
