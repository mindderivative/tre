# RHI Trait & Backends

The Render Hardware Interface (RHI) is `tre-engine`'s abstraction boundary: `rhi.rs` defines a set of backend-agnostic traits (`RhiDevice`, `RhiCommandBuffer`, `RhiSwapchain`, `RhiBuffer`, `RhiTexture`, `RhiPipelineState`) plus two free functions that drive them (`execute_frame`, `submit_frame`). `tre-rhi-vulkan` is the one real, complete implementation today; `tre-rhi-dx12`/`tre-rhi-metal` are unimplemented placeholders.

Every opaque handle these traits expose (`raw_handle`, `image_handle`, `layout_handle`, ...) is a plain `u64` a backend produced itself moments earlier and re-interprets -- e.g. a Vulkan `vk::ImageView` via `ash::vk::Handle::as_raw`. This is deliberate: passing an already-opaque handle through a generic interface is not the "dynamic type inspection" the workspace bans from the per-frame path (TECHNICAL.md Section 9.1); no runtime type identification happens anywhere in this exchange.

## The trait surface

### `RhiBuffer` / `RhiDynamicRingBuffer` / `BufferBinding`

```rust
pub trait RhiBuffer: Send + Sync {
    fn raw_handle(&self) -> u64;
}

pub trait RhiDynamicRingBuffer: RhiBuffer {
    fn write(&self, bytes: &[u8]) -> Option<u32>;  // None if the segment has no room this frame
}

pub struct BufferBinding<'a> {
    pub buffer: &'a dyn RhiBuffer,
    pub offset: u32,
}
```

`RhiDynamicRingBuffer` is triple-buffered and host-mapped -- per-frame vertex/index/uniform data written directly by the CPU with no staging upload. It's a distinct trait from plain `RhiBuffer` (not extra methods bolted on) because callers use a fundamentally different pattern: bump-allocate into the current frame's segment every frame, rather than upload-once-and-keep. `write` returns `None` on starvation rather than growing the buffer -- see the note below on this being a narrower contract than the rest of the engine's `Result<T, EngineError>` convention.

!!! note "REVIEW.md finding #142 -- an intentionally incomplete contract"
    `write`'s `Option<u32>` return doesn't match DESIGN.md's blanket "every fallible operation returns `Result<T, EngineError>`" rule, and that section's own "ring buffer starvation" bullet describes graceful degradation (dropping lowest-priority draws, reporting a frame-budget diagnostic) -- not a bare `None`. Today's only real per-frame caller (`main_loop_demo.rs`) `.expect()`s this, meaning starvation crashes the process. Implementing the real graceful-degradation policy is disclosed future work belonging with the overlay-priority/depth-sorting machinery, not a signature tweak.

### `RhiTexture`

```rust
pub trait RhiTexture: Send + Sync {
    fn raw_handle(&self) -> u64;      // the image view -- what a shader binds/samples
    fn image_handle(&self) -> u64;    // the underlying image, for layout barriers
    fn memory_handle(&self) -> u64;   // backing device memory, freed on destruction
    fn dimensions(&self) -> (u32, u32);
    fn format(&self) -> TextureFormat;
    fn bindless_index(&self) -> Option<u32>;  // None for a transient render target
    fn size_bytes(&self) -> u64;
}
```

`dimensions()` may be **larger** than what a caller originally requested from `acquire_transient_target` -- the transient pool's "next-larger-size" cache-miss fallback can hand back an oversized texture, so callers must consult this rather than assume it matches their request.

### `RhiPipelineState` / `PipelineRegistry`

```rust
pub trait RhiPipelineState: Send + Sync {
    fn raw_handle(&self) -> u64;
    fn layout_handle(&self) -> u64;
}

pub struct PipelineRegistry { /* private: HashMap<u16, Box<dyn RhiPipelineState>> */ }
impl PipelineRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, id: u16, pipeline: Box<dyn RhiPipelineState>); // panics on duplicate id
    pub fn get(&self, id: u16) -> Option<&dyn RhiPipelineState>;
}
```

`PipelineRegistry` maps a `UiDrawCommand::pipeline_state_id` (typically a `PipelineKind as u16`) to the real pipeline object -- replacing what used to be a hardcoded `if pipeline_state_id == PIPELINE_MSDF_TEXT` branch duplicated across demos. `register` panics on a duplicate id (a programmer error); `get` returns `None` for an unknown id (a real runtime condition the executor can detect and report).

### `RhiSwapchain`

```rust
pub trait RhiSwapchain {
    fn extent(&self) -> (u32, u32);
    fn stencil_view_handle(&self) -> u64;
    fn stencil_image_handle(&self) -> u64;
    fn supports_local_read_input_attachment(&self) -> bool;
    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError>;
    fn acquire_next_image_with_timeout(&self, timeout_ns: u64) -> Result<AcquiredImage, EngineError>;
    fn present(&self, image: AcquiredImage) -> Result<(), EngineError>;
    fn read_pixels_bgra8(&self) -> Result<Vec<u8>, EngineError>;
}
```

`acquire_next_image_with_timeout` is the bounded-wait counterpart to `acquire_next_image` (REVIEW.md finding #235) -- returns `EngineError::AcquireTimedOut` if no image becomes available within `timeout_ns`, letting a caller skip a tick's render rather than block indefinitely. `read_pixels_bgra8` reads the last-presented frame back as tightly-packed BGRA8; only `HeadlessSwapchain` has a real implementation (a manually allocated CPU-visible staging buffer) -- a real windowed `VulkanSwapchain` returns `EngineError::PixelReadbackUnsupported` since it presents straight to the surface with no such buffer.

`supports_local_read_input_attachment` is a **per-swapchain** capability query, distinct from `RhiDevice::local_read_blend_supported`'s device-wide one: a real windowed swapchain's images come from `vkCreateSwapchainKHR`, whose `imageUsage` must be a subset of that specific surface's own supported usage flags -- unlike a manually-allocated headless target, a given platform's presentable surface is not spec-guaranteed to support `INPUT_ATTACHMENT` usage. `VulkanDevice::begin_frame` requires **both** this and the device-wide query before selecting the local-read rendering path -- a `PipelineKind::FlatColorBlend` draw against a window whose surface doesn't support it fails closed to ordinary `COLOR_ATTACHMENT_OPTIMAL` rendering (blend modes unavailable) rather than hitting a validation error or driver-defined behavior.

### `RhiDevice`

```rust
pub trait RhiDevice {
    fn create_dynamic_ring_buffer(&self, capacity: usize) -> Result<Box<dyn RhiDynamicRingBuffer>, EngineError>;
    fn shape_style_buffer(&self) -> &dyn RhiDynamicRingBuffer;
    fn local_read_blend_supported(&self) -> bool;

    fn acquire_transient_target(&self, width: u32, height: u32, format: TextureFormat)
        -> Result<Box<dyn RhiTexture>, EngineError>;
    fn release_transient_target(&self, texture: Box<dyn RhiTexture>);

    fn create_texture(&self, width: u32, height: u32, format: TextureFormat, pixels: &[u8])
        -> Result<Box<dyn RhiTexture>, EngineError>;
    fn register_bindless(&self, texture: &dyn RhiTexture) -> Result<u32, EngineError>;
    fn deregister_bindless(&self, bindless_index: u32);
    fn create_custom_pipeline(&self, fragment_source: &str, color_format: TextureFormat)
        -> Result<Box<dyn RhiPipelineState>, EngineError>;

    fn begin_frame(&self, swapchain: &dyn RhiSwapchain)
        -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;
    fn begin_frame_with_logical_size(&self, swapchain: &dyn RhiSwapchain, logical_size: (u32, u32))
        -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;
    fn begin_frame_with_options(&self, swapchain: &dyn RhiSwapchain, options: BeginFrameOptions)
        -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;
    fn submit_and_present(&self, cmd_buffer: Box<dyn RhiCommandBuffer>,
        swapchain: &dyn RhiSwapchain, image: AcquiredImage) -> Result<(), EngineError>;
}

pub struct BeginFrameOptions {
    pub timeout_ns: Option<u64>,       // bounded acquire; None = unbounded wait
    pub crop_size: Option<(u32, u32)>, // confines GPU viewport/scissor/render area
}
```

- **`create_dynamic_ring_buffer`** returns `Result<_, EngineError>` (`DeviceLost` on allocation failure) rather than a bare `Box` -- an addition beyond the original architecture sketch, matching the rest of this trait's fallible-by-`Result` convention.
- **`shape_style_buffer`** is the backend's *one* persistent shape-style storage buffer, bound once at device construction to the bindless descriptor set (see [Canvas & Intermediate Representation](canvas-and-ir.md#gpu-side-style-records-gpu_style)) -- a distinct method rather than a second `create_dynamic_ring_buffer` call, since a caller-created ring buffer has no way to reach that fixed binding.
- **`local_read_blend_supported`** is queried once, at device construction, never assumed -- `ShapeRegistry::flatten_into` falls back to plain `Normal` blending when it's `false`, rather than attempting a pipeline/descriptor set that was never created. This is the exact flag the finding #208 CI investigation is about: it must reflect real device+validation-layer capability, not just driver advertisement (see `crates/tre-rhi-vulkan/src/device.rs`'s `is_local_read_rejected_by_layer`).
- **`register_bindless`/`deregister_bindless`** let an already-GPU-resident texture (typically one just rendered into) get a bindless slot without a pixel upload -- distinct from `create_texture`'s upload-and-register-in-one-call. A texture from `acquire_transient_target` is *not* bindless-registered by default ("written to, not sampled from" is the common case); this is the explicit opt-in for compositing a rendered-into transient target back as a sampled quad. Deregister **before** releasing a bindless-registered transient target, not after -- `release_transient_target`'s own safety guard rejects any texture whose `bindless_index()` is still `Some`.
- **`create_custom_pipeline`** compiles caller-supplied GLSL fragment-shader source at runtime (via `shaderc`) and pairs it with the device's fixed vertex-shader/bindless-descriptor contract (Phase 13 Step 13.8's custom shader API) -- moved onto this trait from a `VulkanDevice`-only inherent method (Architecture review: RHI trait-object generalization, REVIEW.md finding #216) so `tre-python`'s renderer can call it through `Box<dyn RhiDevice>` instead of needing the concrete backend type.
- **`begin_frame`/`submit_and_present`** return `Result<_, EngineError>` (`DeviceLost`/`SwapchainOutOfDate`) -- an addition beyond the original architecture sketch, which omitted error handling entirely; DESIGN.md explicitly requires device-loss conditions to be detected and surfaced as a recoverable error.
- **`begin_frame_with_logical_size`** is identical to `begin_frame` except the returned command buffer's pixel-to-NDC projection uses `logical_size` instead of `swapchain.extent()` -- part of the Wayland resize-drag fix (REVIEW.md finding #235; see [Platform Integration](../platform-integration.md#window-resizing-on-wayland)).
- **`begin_frame_with_options`** unifies what used to be two separate, mutually-exclusive methods (`begin_frame_with_timeout`/`begin_frame_with_viewport_crop`) into one call governed by `BeginFrameOptions`'s two independent, freely composable fields: `timeout_ns` (a bounded acquire, returning `EngineError::AcquireTimedOut` on expiry) and `crop_size` (confines the viewport/scissor/render area, the same mechanism `begin_frame_with_logical_size` uses). `BeginFrameOptions::default()` behaves identically to plain `begin_frame`. Default trait implementation ignores `options` and defers to `begin_frame`, for any backend that hasn't implemented this (REVIEW.md finding #244).

### `RhiCommandBuffer`

```rust
pub trait RhiCommandBuffer {
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState);
    fn set_scissor(&mut self, rect: &ScissorRect);
    fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_texture(&mut self, slot: u32, bindless_index: u32);
    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32);

    fn insert_blend_read_barrier(&mut self);

    fn begin_render_to_texture(&mut self, texture: &dyn RhiTexture, logical_width: u32, logical_height: u32);
    fn end_render_to_texture(&mut self, texture: &dyn RhiTexture);
    fn begin_render_to_texture_no_end(&mut self, texture: &dyn RhiTexture, logical_width: u32, logical_height: u32);
    fn resume_swapchain_rendering(&mut self);

    fn apply_layer_blur(&mut self, device: &dyn RhiDevice, source: &dyn RhiTexture, width: u32, height: u32)
        -> Box<dyn RhiTexture>;

    fn raw_handle(&self) -> u64;
}
```

- **`insert_blend_read_barrier`** issues `VK_KHR_dynamic_rendering_local_read`'s real mechanism: a by-region barrier making the current color attachment's already-written pixels visible as input-attachment reads to the *next* draw in the same rendering scope. Called before **every** `FlatColorBlend` draw, never just once per frame -- each such draw must see the very latest framebuffer state, including ordinary draws that ran since the last blend-mode draw. A real, disclosed no-op when `local_read_blend_supported()` is `false` (never called in that case at all, since `FlatColorBlend` itself is never selected without the capability).
- **`begin_render_to_texture`/`end_render_to_texture`** bracket rendering into one offscreen texture at a time (the RHI capability `PushLayer`/`PopLayer` execution is built on); `resume_swapchain_rendering` returns to the swapchain `begin_frame` originally set up. Scoped to exactly one level of redirection -- true nested layers (resuming an *outer* layer's own target) are real, separate future work with no real scene to prove it against yet.
- **`begin_render_to_texture_no_end`** exists specifically for chaining multiple render-to-texture passes back to back (the Dual-Kawase blur's downsample/upsample levels) -- paired with a plain `end_render_to_texture(previous)` immediately before it, **not** a second `begin_render_to_texture` call, which would end a rendering scope a second time (a real validation error hit during development). A first, combined "end-and-begin in one call" design was tried and reverted: it left no point at which `register_bindless` could safely run, and calling it while a render pass was active produced fully wrong (not merely invalid) sampled output on real hardware -- caught only by an actual GPU pixel check, not by validation or design review.
- **`apply_layer_blur`** is one opaque, purpose-built operation (real Dual-Kawase blur, own-content only -- blurs `source`'s own rendered pixels, not whatever is visually behind it) rather than several smaller primitives a caller would orchestrate itself, since the real mechanism needs a custom pipeline layout incompatible with this trait's otherwise-universal bindless layout assumption.

## Driving the traits: `execute_frame` / `submit_frame`

```rust
pub fn execute_frame(
    frame: &FlattenedFrame,
    registry: &PipelineRegistry,
    vertex_buffer: BufferBinding<'_>,
    index_buffer: BufferBinding<'_>,
    full_window: &ScissorRect,
    device: &dyn RhiDevice,
    cmd_buffer: &mut dyn RhiCommandBuffer,
);

pub fn submit_frame<F>(device: &dyn RhiDevice, swapchain: &dyn RhiSwapchain, record: F) -> Result<(), EngineError>
where F: FnOnce(&mut dyn RhiCommandBuffer);

pub fn submit_frame_with_logical_size<F>(device: &dyn RhiDevice, swapchain: &dyn RhiSwapchain,
    logical_size: (u32, u32), record: F) -> Result<(), EngineError>
where F: FnOnce(&mut dyn RhiCommandBuffer);

pub fn submit_frame_with_viewport_crop<F>(device: &dyn RhiDevice, swapchain: &dyn RhiSwapchain,
    logical_size: (u32, u32), record: F) -> Result<(), EngineError>
where F: FnOnce(&mut dyn RhiCommandBuffer);
```

`execute_frame` is the real, generic frame executor: it walks a `FlattenedFrame`'s commands, resolves each `DrawGeometry`'s pipeline via `registry`, and issues the matching `RhiCommandBuffer` calls -- including switching render targets on `PushLayer`/`PopLayer` and inserting blend-read barriers where needed. The vertex/index buffer binding happens exactly once, outside the per-command loop, since it's command-buffer state invariant across the whole frame (rebinding it per command was pure redundant driver overhead). `submit_frame` is the thin, generic wrapper tying `begin_frame` → a caller-supplied recording closure → `submit_and_present` into one call. `submit_frame_with_logical_size` and `submit_frame_with_viewport_crop` are the same wrapper built on `begin_frame_with_logical_size`/`begin_frame_with_options` respectively -- `submit_frame_with_viewport_crop` is what `tre-python`'s windowed renderer actually calls for the Wayland coarse-resize-drag fix (see [Platform Integration](../platform-integration.md#window-resizing-on-wayland)).

## Backends

| Crate | Status |
|---|---|
| `tre-rhi-vulkan` | The one real, complete backend. Built on [`ash`](https://docs.rs/ash) (raw Vulkan bindings) -- see below. |
| `tre-rhi-dx12` | A 9-line placeholder: module doc only, no implementation. Windows-only (the `windows` crate dependency is target-gated), builds as a no-op elsewhere. |
| `tre-rhi-metal` | A 9-line placeholder, same shape as `tre-rhi-dx12`. macOS-only (built on `objc2-metal`), builds as a no-op elsewhere. |

Both placeholder backends are counted among the three crates permitted to contain `unsafe` (raw DX12/Metal FFI), alongside `tre-rhi-vulkan`.

### `tre-rhi-vulkan`

The Vulkan implementation of every trait above, plus real device/instance setup. Highlights, verified against source over the course of this project's own CI investigations:

- **Instance/device setup** enables `VK_LAYER_KHRONOS_validation` + `VK_EXT_debug_utils` only when both are actually installed (`debug_validation_available`), so a contributor without the validation layers package installed can still `cargo run` -- debug builds only, compiled out entirely in release.
- **`vulkan_debug_callback`** aborts the process (`std::process::abort()`, deliberately not `std::process::exit()`, to avoid an `atexit`-handler deadlock) on any `VK_EXT_debug_utils` ERROR-severity message -- a real, deliberate safety policy, not a bug. One narrow, well-documented exception exists for a confirmed validation-layer/driver version-skew false positive around `VK_KHR_dynamic_rendering_local_read` on older CI runners (see `documentation/REVIEW.md` finding #208) -- `local_read_blend_supported()` is itself corrected to `false` the moment the validation layer signals it can't check that extension, so the engine simply stops exercising the path that layer can't validate, rather than pattern-matching every message it might produce.
- **The bindless texture array** is a fixed-capacity descriptor array (`min(4096, maxDescriptorSetUpdateAfterBindSampledImages)`, real device-limit-clamped, not assumed), backed by a `BindlessRegistry` free-list tracking which slots are live.
- **The transient render-target pool** (TECHNICAL.md Section 3.2) is keyed by `(width, height, format)` after power-of-two bucket rounding, with a background GC thread performing generational, deferred release (a 3-frame grace period) so a texture isn't destroyed while a prior frame might still be reading it.
- **`FrameSync`** tracks one frame in flight at a time, fully synchronous (`begin_frame` waits before recording) -- still uses a rotating index for real, historical reasons documented in its own doc comment (a once-broken-then-fixed synchronization bug).
- A dedicated upload command pool separate from the per-frame recording pool (Vulkan requires external synchronization on a pool for allocate/free, and sharing would need its own locking), and the `BlendReadResources`/`blend_read` descriptor set backing `PipelineKind::FlatColorBlend` -- gated behind `local_read_supported`, never constructed on a device/environment that can't validate the extension. Atlas integration (`AtlasOwnerHandle`, see [Text, SVG & Atlas](text-svg-atlas.md)) lives in `tre-engine`/`tre-atlas`, not this backend -- `tre-rhi-vulkan` only samples the resulting texture bindlessly like any other.
