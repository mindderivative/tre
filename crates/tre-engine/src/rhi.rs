//! The Render Hardware Interface trait surface (ARCHITECTURE.md
//! Section 6): `RhiDevice`/`RhiCommandBuffer`/`RhiSwapchain`/
//! `RhiBuffer`/`RhiTexture`/`RhiPipelineState`, `PipelineRegistry`,
//! and the two free functions (`execute_frame`, `submit_frame`) that
//! drive them -- split out of `lib.rs` (REVIEW.md #205).

use crate::{
    u16_to_texture_format, CommandType, EngineError, FlattenedFrame, PipelineKind, ScissorRect,
    TextureFormat, FULL_WINDOW_CLIP,
};

/// An acquired swapchain image, handed from `RhiSwapchain::acquire_next_image`
/// through `RhiDevice::begin_frame` to the caller and back to
/// `RhiDevice::submit_and_present`.
///
/// `target_view_handle` is a backend-specific opaque handle (e.g. a Vulkan
/// `vk::ImageView` reinterpreted via `ash::vk::Handle::as_raw`). This is
/// deliberately an opaque integer, not a trait-object downcast: passing an
/// already-opaque handle through a generic interface is not the "dynamic
/// type inspection" TECHNICAL.md Section 9.1 bans from the per-frame path
/// -- no runtime type identification happens anywhere in this exchange,
/// only a backend re-interpreting a handle it produced itself moments
/// earlier. This mirrors how Vulkan itself represents every object as an
/// opaque `u64`.
#[derive(Debug, Clone, Copy)]
pub struct AcquiredImage {
    pub index: u32,
    pub target_view_handle: u64,
    /// The raw target image itself (distinct from its view), needed for
    /// layout-transition barriers around dynamic rendering.
    pub target_image_handle: u64,
    /// Opaque handle of the semaphore `acquire_next_image` signaled;
    /// `RhiDevice::submit_and_present`'s queue submit waits on it.
    pub image_available_semaphore_handle: u64,
    /// Opaque handle of the *per-swapchain-image* semaphore
    /// `RhiDevice::submit_and_present`'s queue submit signals, and
    /// `RhiSwapchain::present` waits on before showing this image.
    /// Per-image, not shared across frames: reusing one semaphore for
    /// every frame's present is a real hazard the Vulkan validation layer
    /// catches (VUID-vkQueueSubmit-pSignalSemaphores-00067) -- the CPU-side
    /// fence this engine waits on covers the queue submit's completion,
    /// not the separate, asynchronous present operation's.
    pub render_finished_semaphore_handle: u64,
}

/// A GPU buffer (vertex, index, or uniform). ARCHITECTURE.md Section 6
/// references `&dyn RhiBuffer` in `RhiCommandBuffer` but never defines
/// this trait's own methods -- defined here using the same opaque-handle
/// pattern as `AcquiredImage`.
///
/// `Send + Sync` (REVIEW.md #203/#204's own fix, same real need as
/// `RhiPipelineState`'s identical bound): `tre-python`'s renderer holds
/// a `Box<dyn RhiDynamicRingBuffer>` (a `RhiBuffer` subtrait) across a
/// `Python::detach` call, and `BufferBinding.buffer: &dyn RhiBuffer`
/// needs this bound at the *base* trait -- adding it only to
/// `RhiDynamicRingBuffer` would leave `&dyn RhiBuffer` itself still
/// non-`Send` after the coercion `BufferBinding` performs, since
/// auto-trait properties belong to the trait object's own static type,
/// not to whatever concrete type was coerced from.
pub trait RhiBuffer: Send + Sync {
    fn raw_handle(&self) -> u64;
}

/// A GPU buffer paired with the byte offset to bind it at
/// (IMPLEMENTATION.md Phase 8 Step 8.1.2) -- lets [`execute_frame`]
/// accept either a one-shot-uploaded whole-frame buffer (`offset: 0`)
/// or a real per-frame `RhiDynamicRingBuffer`-backed segment (whatever
/// offset that buffer's own `write` call returned) through the same
/// parameter, without `execute_frame` itself needing to know which.
/// Bundled rather than left as two separate parameters -- matching this
/// crate's own `GlyphAtlasContext` precedent for `draw_text` -- since a
/// buffer and its own offset are always meant to travel together; two
/// bare `u32` parameters next to each other would risk a caller
/// transposing a vertex offset and an index offset with no compiler
/// error either way.
#[derive(Clone, Copy)]
pub struct BufferBinding<'a> {
    pub buffer: &'a dyn RhiBuffer,
    pub offset: u32,
}

/// A GPU texture (atlas page or offscreen render target). Referenced but
/// undefined by ARCHITECTURE.md Section 6; defined here.
///
/// Exposes every raw handle a backend needs to reconstruct its own
/// concrete texture type from a `Box<dyn RhiTexture>` -- e.g.
/// `RhiDevice::release_transient_target` receives one back from a caller
/// and must recover enough to store/eventually destroy it. This is the
/// same opaque-handle-reinterpretation pattern `AcquiredImage` already
/// uses (multiple named `u64` fields, not one), not a downcast:
/// TECHNICAL.md Section 9.1 bans dynamic type inspection in the per-frame
/// path, but a backend re-interpreting a handle it produced itself
/// moments earlier is not that.
///
/// `Send + Sync` (Phase 12 Step 12.3): the same real, necessary fix
/// already applied to [`RhiBuffer`]/[`RhiPipelineState`] at Phase 10 Step
/// 10.4 -- a `tre-python` type holding a `Box<dyn RhiTexture>` (the new
/// `TextAtlas`'s own live GPU atlas texture) needs to be `Send`/`Sync`
/// itself for `Python::detach` to be usable at all; without this bound,
/// it silently can't be.
pub trait RhiTexture: Send + Sync {
    /// The image view -- what a shader binds/samples.
    fn raw_handle(&self) -> u64;
    /// The underlying image (distinct from its view), needed for layout
    /// barriers and for destroying the image itself.
    fn image_handle(&self) -> u64;
    /// The backing device memory, needed to free it on destruction.
    fn memory_handle(&self) -> u64;
    /// The texture's actual dimensions -- may be *larger* than what a
    /// caller requested from `RhiDevice::acquire_transient_target` on a
    /// transient-pool cache miss (TECHNICAL.md Section 3.2's next-larger
    /// fallback, DESIGN.md Section 2.6), so callers must consult this
    /// rather than assume it matches their request.
    fn dimensions(&self) -> (u32, u32);
    fn format(&self) -> TextureFormat;
    /// This texture's slot in the RHI's persistent bindless texture array
    /// (IMPLEMENTATION.md Step 2.1), usable directly as
    /// `RhiCommandBuffer::bind_texture`'s `bindless_index` argument. `None`
    /// for a transient render target (`RhiDevice::acquire_transient_target`)
    /// -- those are written to, not sampled from, and are not registered
    /// into the bindless array this step (see the Step 2.1 plan's "out of
    /// scope" section).
    fn bindless_index(&self) -> Option<u32>;
    /// This texture's real GPU allocation size in bytes (its own
    /// `VkMemoryRequirements::size`, or backend equivalent). Added
    /// IMPLEMENTATION.md Step 2.3 so `RhiDevice::release_transient_target`
    /// can maintain the transient pool's total-free-bytes accounting (the
    /// generational GC's 85%-of-budget trigger) without re-querying it.
    fn size_bytes(&self) -> u64;
}

/// A triple-buffered, host-mapped dynamic buffer (TECHNICAL.md Section
/// 3.1) for per-frame vertex/index/uniform data written directly by the
/// CPU, without a staging upload. A distinct trait from the plain
/// `RhiBuffer` rather than additional methods bolted onto it, since
/// callers use a fundamentally different pattern: bump-allocate into the
/// current frame's segment every frame, rather than upload-once-and-keep.
pub trait RhiDynamicRingBuffer: RhiBuffer {
    /// Bump-allocates `bytes.len()` (rounded up to the RHI's minimum
    /// dynamic-offset alignment) from the current frame's segment and
    /// copies `bytes` into it, returning the byte offset `bytes` was
    /// written at -- usable directly as a
    /// `RhiCommandBuffer::bind_vertex_buffer`/`bind_index_buffer` offset.
    /// Returns `None` if the segment has no room left this frame
    /// (DESIGN.md Section 2.6: ring-buffer starvation is reported, never
    /// grown dynamically mid-frame).
    ///
    /// # Note (REVIEW.md finding #142)
    /// `Option<u32>`, not `Result<u32, EngineError>`, is a narrower
    /// contract than DESIGN.md Section 2.6's own blanket "every fallible
    /// operation returns `Result<T, EngineError>`" rule -- and that
    /// section's own "ring buffer / transient pool starvation" bullet
    /// describes graceful degradation (dropping the lowest-priority
    /// pending draws and reporting a frame-budget diagnostic), not a bare
    /// `None` for the caller to do whatever it likes with.
    /// `main_loop_demo.rs` (Step 8.1.2), the first real per-frame caller,
    /// currently `.expect()`s this -- i.e. starvation crashes the
    /// process today, not graceful degradation. Implementing the real
    /// policy is future work belonging with the overlay-priority/
    /// depth-sorting machinery, not a signature tweak; flagged here so a
    /// future reader doesn't assume this already matches policy.
    fn write(&self, bytes: &[u8]) -> Option<u32>;
}

/// A compiled graphics pipeline state object. Referenced but undefined by
/// ARCHITECTURE.md Section 6; defined here.
pub trait RhiPipelineState: Send + Sync {
    fn raw_handle(&self) -> u64;
    /// Opaque handle of this pipeline's layout, needed by
    /// `RhiCommandBuffer::set_pipeline` implementations that push
    /// constants/descriptors keyed by layout (e.g. `vkCmdPushConstants`).
    /// Same opaque-handle pattern as `AcquiredImage` -- not a downcast.
    fn layout_handle(&self) -> u64;
}

/// Maps a [`UiDrawCommand::pipeline_state_id`] to the real
/// [`RhiPipelineState`] object it names (IMPLEMENTATION.md Phase 6 Step
/// 6.1) -- replaces the hardcoded `if pipeline_state_id ==
/// PIPELINE_MSDF_TEXT {...} else {...}` branch
/// `canvas_batch_flattening_demo.rs`/`canvas_sub_canvas_demo.rs`
/// currently each duplicate. A backend builds its real pipeline objects
/// exactly as it does today (`VulkanDevice::create_pipeline`, unchanged)
/// and registers them here once at startup; this type owns no
/// Vulkan-specific knowledge at all, generic purely over the
/// `RhiPipelineState` trait. `HashMap` over a fixed-size array: only two
/// real entries exist today (`PipelineKind::SdfRoundedRect`/`MsdfText`)
/// against a 16-bit id space far too sparse for an array to make sense,
/// and frame-time `get()` is not a measured hot path (Step 6.2's real
/// executor is the first thing that will call it at all).
pub struct PipelineRegistry {
    pipelines: std::collections::HashMap<u16, Box<dyn RhiPipelineState>>,
}

impl PipelineRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pipelines: std::collections::HashMap::new(),
        }
    }

    /// Registers `pipeline` under `id` (typically `PipelineKind::* as
    /// u16`, though any `u16` is accepted -- the registry itself has no
    /// opinion on where ids come from).
    ///
    /// # Panics
    /// Panics if `id` is already registered -- two pipelines silently
    /// sharing one id is a programmer error, not a recoverable runtime
    /// condition, matching this crate's established `pop_layer`/
    /// `restore`-style precedent for unbalanced/invalid caller state.
    pub fn register(&mut self, id: u16, pipeline: Box<dyn RhiPipelineState>) {
        assert!(
            self.pipelines.insert(id, pipeline).is_none(),
            "PipelineRegistry: id {id} was already registered"
        );
    }

    /// Resolves `id` to its registered pipeline, or `None` if nothing is
    /// registered under it -- distinct from `register`'s panic-on-
    /// duplicate above, since a command referencing an unknown pipeline
    /// id is a real runtime condition a caller (Step 6.2's executor)
    /// should be able to detect and report, not necessarily a programmer
    /// error caught at registration time.
    #[must_use]
    pub fn get(&self, id: u16) -> Option<&dyn RhiPipelineState> {
        self.pipelines.get(&id).map(std::convert::AsRef::as_ref)
    }
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A per-window presentation surface. Referenced (as `&dyn RhiSwapchain`)
/// but never defined by ARCHITECTURE.md Section 6; defined here with the
/// minimum needed to make `RhiDevice::begin_frame`/`submit_and_present`
/// actually implementable without `Any`-downcasting (TECHNICAL.md Section
/// 9.1's per-frame-loop ban).
///
/// `Send + Sync` (Architecture review: RHI trait-object generalization,
/// REVIEW.md finding #216) -- the same real, necessary reason already
/// applied to `RhiBuffer`/`RhiTexture`/`RhiPipelineState`: `tre-python`'s
/// renderer types hold their own swapchain behind `Box<dyn RhiSwapchain>`
/// across a `Python::detach` call, which requires the boxed trait object
/// itself to be `Send`.
pub trait RhiSwapchain: Send + Sync {
    fn extent(&self) -> (u32, u32);

    /// Opaque handle (e.g. a Vulkan `vk::ImageView` reinterpreted via
    /// `ash::vk::Handle::as_raw`, same pattern as `AcquiredImage`'s own
    /// handles -- not a downcast) of this swapchain's own stencil image
    /// view, sized to match its `extent()`. IMPLEMENTATION.md Step 3.3.3:
    /// every swapchain owns its own stencil image, mirroring how it
    /// already owns its own color image(s), since different swapchains
    /// (e.g. two windows in `multi_window`) can have different extents.
    fn stencil_view_handle(&self) -> u64;

    /// The same stencil image's underlying `vk::Image` (distinct from its
    /// view, exactly like `AcquiredImage::target_image_handle` vs.
    /// `target_view_handle`) -- needed for the layout-transition barrier
    /// `RhiDevice::begin_frame` issues before rendering can use it.
    fn stencil_image_handle(&self) -> u64;

    /// Phase 10 Step 10.2.3: `true` only when THIS swapchain's own color
    /// image(s) were created with `VK_IMAGE_USAGE_INPUT_ATTACHMENT_BIT` --
    /// a real, disclosed, per-swapchain capability query, distinct from
    /// `RhiDevice::local_read_blend_supported()`'s own device-wide query.
    /// A headless swapchain's single persistent image always declares
    /// this flag (a core, always-safe-to-declare usage on a manually
    /// allocated image), but a real windowed swapchain's images come
    /// from `vkCreateSwapchainKHR`, whose `imageUsage` must be a subset
    /// of that specific surface's own `VkSurfaceCapabilitiesKHR::
    /// supportedUsageFlags` -- unlike `INPUT_ATTACHMENT`'s always-
    /// guaranteed presence on a manually allocated image, a given
    /// platform's presentable surface is not spec-guaranteed to support
    /// it, so `VulkanSwapchain::new` queries it for real rather than
    /// assuming. `VulkanDevice::begin_frame` requires BOTH this AND
    /// `local_read_blend_supported()` before choosing `RENDERING_LOCAL_
    /// READ_KHR` for the active color attachment -- so a
    /// `PipelineKind::FlatColorBlend` draw against a window whose
    /// surface doesn't support this fails closed to ordinary
    /// `COLOR_ATTACHMENT_OPTIMAL` rendering (with blend modes then
    /// unavailable, exactly as if the device itself lacked the
    /// extension) rather than hitting a validation error or driver-
    /// defined behavior.
    ///
    /// Defaults to `false` (`/review-project` Architecture finding #247,
    /// 2026-09-13): the same default-delegating pattern
    /// `acquire_next_image_with_timeout` and every `begin_frame_with_*`
    /// addition already use, so a backend without this capability
    /// compiles without an explicit override and degrades to the same
    /// `Normal`-blend fallback path a device lacking the extension takes.
    fn supports_local_read_input_attachment(&self) -> bool {
        false
    }

    /// # Errors
    /// Returns [`EngineError::SwapchainOutOfDate`] if the surface no longer
    /// matches the window (DESIGN.md Section 2.6) or
    /// [`EngineError::DeviceLost`] on any other acquisition failure.
    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError>;

    /// Identical to [`Self::acquire_next_image`] except the wait for the
    /// next available image is bounded by `timeout_ns` nanoseconds
    /// instead of blocking indefinitely (REVIEW.md finding #235, Option
    /// 3: under Wayland, a live resize drag can make the real, unbounded
    /// wait here block for hundreds of milliseconds to seconds on a
    /// single frame). Default implementation ignores `timeout_ns` and
    /// defers to [`Self::acquire_next_image`]'s own unbounded wait, for
    /// any backend that hasn't implemented a real bounded acquire.
    ///
    /// # Errors
    /// Same as [`Self::acquire_next_image`], plus
    /// [`EngineError::AcquireTimedOut`] if no image became available
    /// within `timeout_ns`.
    fn acquire_next_image_with_timeout(
        &self,
        timeout_ns: u64,
    ) -> Result<AcquiredImage, EngineError> {
        let _ = timeout_ns;
        self.acquire_next_image()
    }

    /// Waits on `image.render_finished_semaphore_handle` before showing
    /// the image (DESIGN.md Section 2.6 -- surfaces failures rather than
    /// stalling or panicking).
    ///
    /// # Errors
    /// Returns [`EngineError::SwapchainOutOfDate`] if the surface no longer
    /// matches the window, or [`EngineError::DeviceLost`] on any other
    /// presentation failure.
    fn present(&self, image: AcquiredImage) -> Result<(), EngineError>;

    /// Reads this swapchain's own most recently presented frame back as
    /// tightly packed `BGRA8` host memory (Architecture review: RHI
    /// trait-object generalization, REVIEW.md finding #216) -- lets a
    /// caller like `tre-python`'s `HeadlessRenderer` store its swapchain
    /// behind `Box<dyn RhiSwapchain>` instead of the concrete headless
    /// type, closing the one capability that previously forced it to
    /// stay concrete.
    ///
    /// # Errors
    /// Returns [`EngineError::PixelReadbackUnsupported`] on a swapchain
    /// with no CPU-visible staging buffer to read back from (a real
    /// windowed swapchain presents straight to the window surface and
    /// has none) -- only a headless swapchain, built with exactly this
    /// use in mind, supports it.
    fn read_pixels_bgra8(&self) -> Result<Vec<u8>, EngineError>;
}

/// Configures [`RhiDevice::begin_frame_with_options`]'s own bounded-
/// acquire and viewport-crop axes -- two independent, freely composable
/// knobs (`/review-project` Architecture finding, 2026-09-13:
/// `VulkanDevice::begin_frame_impl` already treated these as orthogonal
/// internal parameters; this type just exposes that same composability
/// on the public trait, where four separate, mutually-exclusive methods
/// previously forced a caller to choose one axis at a time).
#[derive(Debug, Clone, Copy, Default)]
pub struct BeginFrameOptions {
    /// `None` (the default) waits indefinitely for an image, matching
    /// plain [`RhiDevice::begin_frame`]. `Some(timeout_ns)` bounds the
    /// wait instead (REVIEW.md finding #235, Option 3) -- lets a caller
    /// skip rendering this tick and try again next iteration rather than
    /// freezing the whole render loop when a resize makes acquire slow.
    /// Returns [`EngineError::AcquireTimedOut`] if no image becomes
    /// available within it.
    pub timeout_ns: Option<u64>,
    /// `None` (the default) renders across the swapchain's own full
    /// extent, unchanged. `Some((width, height))` confines the GPU
    /// viewport/scissor/render area to that size instead, rendering 1:1,
    /// undistorted, into just that sub-rectangle of a possibly-larger
    /// buffer (REVIEW.md finding #238's shipped resize-drag fix) --
    /// pair with a real compositor-side viewport source crop of the
    /// identical size (`tre_platform::PlatformConnection::
    /// set_viewport_source_crop`) for the two to compose into content
    /// that fills the real window with zero resample anywhere.
    pub crop_size: Option<(u32, u32)>,
}

/// The Render Hardware Interface device trait (ARCHITECTURE.md Section 6).
///
/// `begin_frame`/`submit_and_present` return `Result<_, EngineError>`,
/// which ARCHITECTURE.md's original sketch omitted -- DESIGN.md Section
/// 2.6 explicitly requires device-loss/swapchain-out-of-date conditions to
/// be "detected at `RhiDevice::begin_frame` and surfaced as a recoverable
/// error," which is impossible with a bare, infallible return type. This
/// is exactly the kind of interface mismatch Phase 0 exists to catch
/// while it's still cheap to change (IMPLEMENTATION.md Phase 0 rationale).
///
/// `Send + Sync` (Architecture review: RHI trait-object generalization,
/// REVIEW.md finding #216) -- the same real, necessary reason already
/// applied to `RhiBuffer`/`RhiTexture`/`RhiPipelineState`, now extended
/// to the device itself: `tre-python`'s renderer types hold their own
/// device behind `Box<dyn RhiDevice>` across a `Python::detach` call
/// (needs `Send`), and `render_parallel`'s own worker threads each take
/// a shared `&dyn RhiDevice` reference concurrently (needs `Sync`).
pub trait RhiDevice: Send + Sync {
    // Resource Management
    /// `capacity` is the ring buffer's TOTAL size in bytes (TECHNICAL.md
    /// Section 3.1's $16\text{-}32\text{MB}$), divided evenly across the
    /// 3 frame-in-flight segments -- not the per-segment size.
    ///
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] if the backing GPU buffer/
    /// memory allocation fails (REVIEW.md finding #189: previously an
    /// internal `.expect()`, panicking on a real, recoverable
    /// out-of-memory condition instead of letting a caller handle it).
    fn create_dynamic_ring_buffer(
        &self,
        capacity: usize,
    ) -> Result<Box<dyn RhiDynamicRingBuffer>, EngineError>;
    /// Phase 10 Step 10.2: the backend's one persistent shape-style
    /// storage buffer -- bound once, at device construction, to the
    /// bindless descriptor set's binding 1 (see
    /// `documentation/ARCHITECTURE.md` Section 7's "Shape Style Buffer").
    /// `shapes::GpuShapeStyle` records are bump-allocated into it via the
    /// same `RhiDynamicRingBuffer::write` contract `create_dynamic_ring_
    /// buffer`'s own vertex/index buffers already use -- a distinct
    /// method (not a second `create_dynamic_ring_buffer` call) because
    /// this buffer's identity is fixed at device construction and bound
    /// into the bindless set then; a caller-created ring buffer has no
    /// way to reach that binding.
    fn shape_style_buffer(&self) -> &dyn RhiDynamicRingBuffer;
    /// Phase 10 Step 10.2.3: whether this real device supports
    /// `VK_KHR_dynamic_rendering_local_read` (queried once, at device
    /// construction, never assumed) -- the real capability gate for
    /// non-`Normal` `BlendMode` rendering (`PipelineKind::
    /// FlatColorBlend`). A real, disclosed capability query, not a
    /// silent assumption: `ShapeRegistry::flatten_into` falls back to
    /// plain `Normal` blending when this is `false`, rather than
    /// attempting to use a pipeline/descriptor set that was never
    /// created. See `PipelineKind::FlatColorBlend`'s own doc comment for
    /// why the originally-planned `VK_EXT_blend_operation_advanced` path
    /// was abandoned instead of gated the same way.
    ///
    /// Defaults to `false` (`/review-project` Architecture finding #247,
    /// 2026-09-13) -- see `RhiSwapchain::supports_local_read_input_
    /// attachment`'s own identical rationale: a backend that never
    /// overrides this is treated exactly like a device without the
    /// capability, and `FlatColorBlend` is never selected for it.
    fn local_read_blend_supported(&self) -> bool {
        false
    }
    /// # Errors
    /// Returns [`EngineError::TransientPoolBudgetExceeded`] if a genuinely
    /// novel size would need cold-allocating while the pool's idle free
    /// bytes are already at or past the dynamic-VRAM budget (Phase 2
    /// Step 2.3 Code Review finding #80) -- a reuse of an already-pooled
    /// size (the common case) never fails this way.
    fn acquire_transient_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<Box<dyn RhiTexture>, EngineError>;
    fn release_transient_target(&self, texture: Box<dyn RhiTexture>);
    /// Uploads `pixels` (tightly packed, row-major, matching `format`'s
    /// byte layout) as a new GPU-resident sampled texture and registers it
    /// into the RHI's persistent bindless texture array (IMPLEMENTATION.md
    /// Step 2.1), so `texture.bindless_index()` can immediately be passed to
    /// `RhiCommandBuffer::bind_texture`. Unlike `acquire_transient_target`,
    /// this is a genuine one-time GPU upload, not a pool checkout -- callers
    /// own the returned texture for as long as they need it and simply drop
    /// it when done (`Drop` tears down the GPU resources and frees the
    /// bindless slot).
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidTextureData`] if `pixels.len()` doesn't
    /// match `width * height * bytes_per_pixel(format)`, or `width`/`height`
    /// is zero. Returns [`EngineError::BindlessArrayExhausted`] if the
    /// bindless texture array has no free slots left. Added in Phase 2 Code
    /// Review findings #66/#67 -- both were previously unconditional panics.
    fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Result<Box<dyn RhiTexture>, EngineError>;

    /// Registers an already-created texture -- typically one just
    /// rendered into via [`RhiCommandBuffer::begin_render_to_texture`]/
    /// [`RhiCommandBuffer::end_render_to_texture`] -- into the RHI's
    /// persistent bindless texture array, returning the allocated slot
    /// so it can be passed directly to [`RhiCommandBuffer::bind_texture`]
    /// (IMPLEMENTATION.md Phase 6 Step 6.4.1). Unlike
    /// [`RhiDevice::create_texture`], this performs no pixel upload at
    /// all -- `texture` is already GPU-resident; this only allocates a
    /// bindless slot and points it at the texture's own existing view.
    /// Returns the raw index rather than mutating `texture.bindless_
    /// index()` itself: `RhiTexture` exposes no setter (deliberately --
    /// every other method on it is a read of state fixed at construction
    /// time), so the caller is responsible for remembering the returned
    /// index for as long as it needs it, exactly as it already must for
    /// any other value this trait returns.
    ///
    /// A texture returned by [`RhiDevice::acquire_transient_target`] is
    /// *not* bindless-registered by default ("written to, not sampled
    /// from" is the common case that never needs a slot at all) -- this
    /// is the explicit opt-in for the one real case that does:
    /// compositing a rendered-into transient target back as a sampled
    /// quad.
    ///
    /// # Errors
    /// Returns [`EngineError::BindlessArrayExhausted`] if the bindless
    /// texture array has no free slots left -- the same failure mode
    /// [`RhiDevice::create_texture`] can hit, for the same underlying
    /// array.
    fn register_bindless(&self, texture: &dyn RhiTexture) -> Result<u32, EngineError>;

    /// Reverses [`RhiDevice::register_bindless`] -- frees `bindless_index`
    /// (that call's own return value) back to the registry's free list.
    /// Takes the raw index alone, not a texture reference: the free-list
    /// release itself only ever needed the index (the descriptor slot's
    /// contents are simply overwritten, harmlessly, whenever it's next
    /// allocated to something else). Callers that bindless-register a
    /// texture acquired via [`RhiDevice::acquire_transient_target`] must
    /// call this before [`RhiDevice::release_transient_target`], not
    /// after: that function's own safety guard (Phase 2 Code Review
    /// finding #70) rejects any texture whose `bindless_index()` is
    /// `Some`, since a *genuinely* `create_texture`-sourced texture
    /// reaching it that way would otherwise be pooled as if it had
    /// `COLOR_ATTACHMENT` usage it never actually has -- deregistering
    /// first, here, keeps that guard's own logic completely untouched.
    /// This crate's own `register_bindless` never mutates a transient
    /// target's own `bindless_index()` field at all (see that method's
    /// own doc comment), so that guard's check is unaffected by this
    /// method's use either way.
    fn deregister_bindless(&self, bindless_index: u32);

    // Command Submission
    /// # Errors
    /// Returns [`EngineError::DeviceLost`] on GPU device removal or driver
    /// TDR, or [`EngineError::SwapchainOutOfDate`] if `swapchain` no longer
    /// matches its window -- surfaced here per DESIGN.md Section 2.6.
    fn begin_frame(
        &self,
        swapchain: &dyn RhiSwapchain,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;

    /// Identical to [`Self::begin_frame`] except the returned command
    /// buffer's pixel-to-NDC projection uses `logical_size` instead of
    /// `swapchain.extent()` -- the GPU viewport/scissor/render area still
    /// cover the swapchain's own real, full extent unchanged (REVIEW.md
    /// finding #235). This is the real fix pyCopper's own
    /// `LESSONS_LEARNED.md` describes for resize trailing: while a
    /// swapchain is deliberately held at a coarser size than the real
    /// window during an active drag (avoiding a `VkSwapchainKHR` rebuild
    /// on every pixel), content is still placed as if the render target
    /// were `logical_size` -- pre-stretched across the oversized buffer
    /// -- so that Wayland's own compositor-side scale-to-fit (the same
    /// mechanism finding #234 fixed as a bug when it was unintentional)
    /// exactly cancels that pre-stretch. Net effect: pixel-exact
    /// geometry with no visible squash/distortion, at the cost of one
    /// compositor-side resample (a real, disclosed softening, most
    /// visible in text) -- pyCopper's own test suite documents the
    /// identical tradeoff. Default implementation ignores `logical_size`
    /// and defers to [`Self::begin_frame`]'s own behavior (NDC always
    /// matches the swapchain's real extent), for any backend that hasn't
    /// implemented this.
    ///
    /// # Errors
    /// Same as [`Self::begin_frame`].
    fn begin_frame_with_logical_size(
        &self,
        swapchain: &dyn RhiSwapchain,
        logical_size: (u32, u32),
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        let _ = logical_size;
        self.begin_frame(swapchain)
    }

    /// Identical to [`Self::begin_frame`] except governed by `options`,
    /// whose two fields are independent, freely composable axes -- not
    /// alternatives forcing a choice, the way this trait's own previous
    /// surface (`begin_frame_with_timeout`/`begin_frame_with_viewport_crop`,
    /// REVIEW.md findings #235/#238) required (`/review-project`
    /// Architecture finding, 2026-09-13: `VulkanDevice::begin_frame_impl`
    /// already treated the timeout and the crop as orthogonal internal
    /// parameters; only the public trait forced a caller to pick one
    /// method or the other, which is why the bounded-acquire path ended
    /// up with no real caller left once viewport-crop shipped -- neither
    /// `resize_test.rs`'s own `Coarse`/`Timeout` A/B-test strategies nor
    /// any other real caller had a way to ask for both at once even
    /// though nothing about the underlying mechanism prevented it).
    /// `BeginFrameOptions::default()` behaves identically to plain
    /// [`Self::begin_frame`] (unbounded wait, full-extent viewport) --
    /// see that type's own field docs for what each one does. Default
    /// implementation ignores `options` and defers to
    /// [`Self::begin_frame`], for any backend that hasn't implemented
    /// this.
    ///
    /// # Errors
    /// Same as [`Self::begin_frame`], plus
    /// [`EngineError::AcquireTimedOut`] if `options.timeout_ns` is `Some`
    /// and no image became available within it.
    fn begin_frame_with_options(
        &self,
        swapchain: &dyn RhiSwapchain,
        options: BeginFrameOptions,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError> {
        let _ = options;
        self.begin_frame(swapchain)
    }

    /// # Errors
    /// Returns [`EngineError::DeviceLost`] or
    /// [`EngineError::SwapchainOutOfDate`] under the same conditions as
    /// [`RhiDevice::begin_frame`].
    fn submit_and_present(
        &self,
        cmd_buffer: Box<dyn RhiCommandBuffer>,
        swapchain: &dyn RhiSwapchain,
        image: AcquiredImage,
    ) -> Result<(), EngineError>;

    /// Compiles `fragment_source` (real GLSL) at runtime and pairs it
    /// with this device's own fixed vertex-shader/bindless-descriptor
    /// contract into a real pipeline (Phase 13 Step 13.8's custom shader
    /// API) -- moved onto this trait from a `VulkanDevice`-only inherent
    /// method (Architecture review: RHI trait-object generalization,
    /// REVIEW.md finding #216) so `tre-python`'s renderer can call it
    /// through `Box<dyn RhiDevice>` instead of needing the concrete
    /// backend type. `color_format` names the render target's format
    /// using this crate's own backend-agnostic [`TextureFormat`], not a
    /// raw backend format code.
    ///
    /// **Real, disclosed v1 scope** (unchanged from the inherent method
    /// this replaces): `fragment_source` must declare `layout(location =
    /// 0) in vec4 frag_color;`, `layout(location = 1) in vec2 frag_uv;`,
    /// optionally `layout(location = 2) in vec3 frag_params;`,
    /// `layout(location = 0) out vec4 out_color;`, and the identical
    /// 12-byte `PushConstants { vec2 screen_size; uint texture_index; }`
    /// block every other real pipeline in this workspace already shares
    /// -- no custom vertex shader, no arbitrary vertex attributes, no
    /// descriptor sets beyond the engine's own shared bindless set.
    ///
    /// # Errors
    /// Returns [`EngineError::ShaderCompilationFailed`] with the
    /// backend's own real compiler diagnostic if `fragment_source` fails
    /// to compile. Returns [`EngineError::PipelineCreationFailed`] if the
    /// (successfully compiled) shader still fails real pipeline creation.
    fn create_custom_pipeline(
        &self,
        fragment_source: &str,
        color_format: TextureFormat,
    ) -> Result<Box<dyn RhiPipelineState>, EngineError>;
}

/// The Render Hardware Interface command-buffer trait (ARCHITECTURE.md
/// Section 6), with one addition beyond the original sketch: `raw_handle`,
/// needed so `RhiDevice::submit_and_present` can recover the concrete
/// backend's submittable handle from a `Box<dyn RhiCommandBuffer>` -- via
/// the same opaque-handle pattern as `AcquiredImage`, not downcasting.
pub trait RhiCommandBuffer {
    // State Tracking
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState);
    fn set_scissor(&mut self, rect: &ScissorRect);

    // Bindings (Leveraging Bindless where available)
    fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_texture(&mut self, slot: u32, bindless_index: u32);

    // Execution
    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32);

    /// Phase 10 Step 10.2.3: a by-region pipeline barrier making the
    /// current color attachment's own already-written pixels visible as
    /// input-attachment reads to the NEXT draw in this same rendering
    /// scope -- `VK_KHR_dynamic_rendering_local_read`'s own real
    /// mechanism (`execute_frame`'s own doc comment on why this is
    /// called before every `PipelineKind::FlatColorBlend` draw, never
    /// only once per frame: each such draw must see whatever the very
    /// latest framebuffer state is, including ordinary draws that ran
    /// since the last blend-mode draw). A real, disclosed no-op on a
    /// device where `RhiDevice::local_read_blend_supported` is `false`
    /// -- never called in that case, since `FlatColorBlend` itself is
    /// never selected without that capability.
    ///
    /// Defaults to a no-op (`/review-project` Architecture finding #247,
    /// 2026-09-13): by this method's own contract above it is never
    /// called on a device whose `local_read_blend_supported` is `false`,
    /// and that is exactly the default for any backend that doesn't
    /// override both -- so an unoverridden no-op here can never be
    /// reached with real work to do.
    fn insert_blend_read_barrier(&mut self) {}

    // Offscreen render targets (IMPLEMENTATION.md Phase 6 Step 6.4.1) --
    // real render-to-texture, the RHI capability real `PushLayer`/
    // `PopLayer` execution (Step 6.4.2) is built on. Scoped to exactly
    // one level of redirection: `begin_render_to_texture`/
    // `end_render_to_texture` bracket rendering into one texture at a
    // time, and `resume_swapchain_rendering` returns to the swapchain
    // `RhiDevice::begin_frame` originally set up -- resuming an *outer*
    // layer's own target (true nested layers) is real, separate future
    // work with no real scene to prove it against yet.
    /// Ends whatever rendering scope is currently active and begins a
    /// new one targeting `texture`, cleared to transparent black, with
    /// no stencil attachment (transient targets don't have one).
    ///
    /// `logical_width`/`logical_height` are the caller's own *intended*
    /// size -- exactly what it originally passed to `RhiDevice::
    /// acquire_transient_target` -- not necessarily `texture`'s own real
    /// physical size. REVIEW.md finding #152: `acquire_transient_
    /// target`'s documented "oversized borrow" fallback can hand back a
    /// texture larger than requested, and every subsequent `draw_indexed`
    /// call's own NDC-mapping push constant must be computed against the
    /// caller's *intended* size, not the texture's real one, or content
    /// recorded assuming the smaller size (every `PushLayer` inner draw,
    /// baked at `Canvas` record time before the real texture is ever
    /// acquired) silently confines itself to a small corner of the
    /// oversized image. Viewport/scissor/render area stay driven by the
    /// texture's own real size regardless -- content simply draws
    /// "stretched" to fill it, a stretch exactly undone later when it's
    /// sampled back through a normalized `(0,0)`-`(1,1)` UV read and
    /// redrawn at its own real, requested on-screen size (`PopLayer`'s
    /// own composite quad). No other caller-side change is needed.
    fn begin_render_to_texture(
        &mut self,
        texture: &dyn RhiTexture,
        logical_width: u32,
        logical_height: u32,
    );
    /// Ends the rendering scope `begin_render_to_texture` began and
    /// transitions `texture` to a layout suitable for sampling
    /// afterward (e.g. via `RhiDevice::register_bindless` then
    /// `bind_texture`).
    fn end_render_to_texture(&mut self, texture: &dyn RhiTexture);
    /// Begins a new rendering scope targeting `texture`, cleared to
    /// transparent black -- identical to `begin_render_to_texture`
    /// except it never calls `cmd_end_rendering` first, because nothing
    /// is currently active to end. `logical_width`/`logical_height` carry
    /// the same meaning as `begin_render_to_texture`'s own parameters of
    /// the same name -- see that method's own doc comment.
    ///
    /// Exists specifically for chaining multiple render-to-texture
    /// passes back to back (IMPLEMENTATION.md Step 7.2.1's own
    /// Dual-Kawase downsample/upsample levels), used together with a
    /// plain `end_render_to_texture(previous)` call immediately before
    /// it -- **not** `begin_render_to_texture(texture)` directly, which
    /// would call `cmd_end_rendering` a second time for the one scope
    /// `end_render_to_texture` already ended, a real Vulkan validation
    /// error found by actually running Step 7.2.1's own first demo.
    ///
    /// A first, combined `chain_render_to_texture(ending, beginning)`
    /// design (one call doing both the end-and-barrier and the next
    /// begin) was tried and reverted during this same step's own
    /// implementation: it left no point between "the previous texture is
    /// in a sampling-ready layout" and "the next render pass is already
    /// active" to call `RhiDevice::register_bindless` -- every proven
    /// working caller of that method (Step 6.4.1's own single-level
    /// flow) calls it with *no* render pass active, and calling it while
    /// one *is* active (as the combined design forced) produced fully
    /// transparent/wrong sampled output on real hardware, not a
    /// validation error -- caught only by a real GPU pixel check, not
    /// design review. This split keeps every call's own preconditions
    /// identical to the already-proven single-level usage.
    fn begin_render_to_texture_no_end(
        &mut self,
        texture: &dyn RhiTexture,
        logical_width: u32,
        logical_height: u32,
    );
    /// Resumes rendering into the swapchain image `RhiDevice::begin_frame`
    /// originally set up, preserving whatever it already had drawn --
    /// unlike `begin_render_to_texture`, this never clears.
    fn resume_swapchain_rendering(&mut self);

    /// Applies a real Dual-Kawase blur to `source` -- a texture already
    /// `end_render_to_texture`'d (sampling-ready), `width`/`height` its
    /// own real, intended/logical size (the same convention as
    /// `begin_render_to_texture`'s own `logical_width`/`logical_height`
    /// above) -- and returns a *new*, separately-owned, already
    /// sampling-ready blurred texture. IMPLEMENTATION.md Step 7.2.2:
    /// backs `LayerDesc::blur`; own-content blur only (blurs `source`'s
    /// own already-rendered pixels, not whatever is visually behind it),
    /// a fixed chain depth matching Step 7.2.1's own proven demo.
    ///
    /// Deliberately one opaque, purpose-built operation rather than
    /// several smaller primitives the caller would orchestrate itself --
    /// the real mechanism (a non-bindless downsample/upsample chain,
    /// REVIEW.md finding #130's own real fix) needs a custom pipeline
    /// layout/descriptor set incompatible with every other trait method
    /// here, which deliberately assumes the universal bindless layout;
    /// exposing that mismatch to callers has no benefit over hiding it
    /// entirely behind one call, matching this trait's own precedent for
    /// `begin_render_to_texture_no_end` (added narrowly for the one real
    /// need it served, not as a speculative primitive family).
    ///
    /// The caller owns the returned texture exactly as if it had called
    /// `RhiDevice::acquire_transient_target` itself -- release it the
    /// same way once done. `source` itself is untouched (still owned by
    /// the caller, still sampling-ready) -- this does not consume or
    /// release it.
    ///
    /// # Errors
    /// Returns [`EngineError::TransientPoolBudgetExceeded`] (or another
    /// `acquire_transient_target` failure) if any of this method's own
    /// internal hops cannot acquire a transient target (REVIEW.md finding
    /// #189: previously an internal `.expect()` per hop, panicking on a
    /// real, recoverable pool-exhaustion condition instead of letting a
    /// caller handle it). Any hop already acquired before a later one
    /// fails is released back to the pool before returning `Err` -- this
    /// method never leaks a transient target on a failure path.
    fn apply_layer_blur(
        &mut self,
        device: &dyn RhiDevice,
        source: &dyn RhiTexture,
        width: u32,
        height: u32,
    ) -> Result<Box<dyn RhiTexture>, EngineError>;

    fn raw_handle(&self) -> u64;
}

/// The real, generic frame executor (IMPLEMENTATION.md Phase 6 -- Step
/// 6.2 built the `DrawGeometry` half, Step 6.3 the real `PushScissor`/
/// `PopScissor` half; renamed from `execute_draw_geometry_batches` at
/// Step 6.3 since its scope is no longer just draw batches). Drives
/// every command in `frame.commands` through the RHI: a `DrawGeometry`
/// resolves its own `pipeline_state_id` via `registry` (Step 6.1's
/// [`PipelineRegistry`]) instead of a hardcoded per-pipeline branch,
/// exactly as Step 6.2 left it; a `PushScissor`/`PopScissor` applies a
/// real `RhiCommandBuffer::set_scissor` via a runtime clip stack, since
/// a `PopScissor` command's own `clip_bounds` carries no restore data
/// (`push_clip`/`pop_clip`'s own real source).
///
/// `full_window` is the real framebuffer extent, in real pixels -- the
/// only thing the caller (not `tre-engine`, which has no notion of
/// framebuffer size) knows. It stands in for [`FULL_WINDOW_CLIP`]'s own
/// `u32::MAX`-sized sentinel wherever that sentinel would otherwise
/// reach a real `set_scissor` call (`begin_overlay`'s own `PushScissor`
/// command carries the raw sentinel directly; the clip stack emptying
/// after a `PopScissor` represents the same "no active clip" concept) --
/// passing the raw sentinel to a real GPU call would be an invalid,
/// out-of-bounds scissor rect. `RhiDevice::begin_frame` already applies
/// a real, correct full-framebuffer scissor before returning the command
/// buffer, so a frame with no `PushScissor` at all needs no extra call
/// here to stay correct.
///
/// `PushLayer`/`PopLayer` commands drive real transient-target
/// acquisition and compositing (Step 6.4.2), built on Step 6.4.1's RHI
/// capability: `PushLayer` decodes `desc.format` back out of
/// `command.pipeline_state_id` (`u16_to_texture_format`, reversing
/// `push_layer`'s own `texture_format_to_u16`), `device.
/// acquire_transient_target`s a target sized to `command.clip_bounds`'
/// `width`/`height`, and `cmd_buffer.begin_render_to_texture`s into it.
/// `PopLayer` ends that render -- then, if the popped `LayerDesc`'s own
/// `blur` flag was set (smuggled through `command.texture_handle`,
/// `1`/`0`, `pop_layer`'s own doc comment; Step 7.2.2), calls
/// `cmd_buffer.apply_layer_blur` and releases the original, now-
/// unneeded layer texture, compositing the *returned* blurred one
/// instead. Either way, `device.register_bindless`s whichever texture
/// is actually being composited, `cmd_buffer.resume_swapchain_
/// rendering`s, then draws the `PopLayer` command's own baked
/// composite-quad geometry (`element_count`/`vertex_offset`, `pop_
/// layer`'s own doc comment) against the pipeline `command.pipeline_
/// state_id` names (`PipelineKind::TexturedQuad`) -- binding the
/// just-registered index directly rather than trusting `command.
/// texture_handle` (which carries the blur flag here, never a texture
/// reference), the same substitution `PushScissor` already does for
/// `FULL_WINDOW_CLIP`, below. `resume_swapchain_rendering` unconditionally resets the GPU
/// scissor to the full swapchain extent (Step 6.4.1's own REVIEW.md
/// #128 fix), so `PopLayer` re-applies `clip_stack`'s current top
/// afterward -- otherwise a layer popped from inside an active
/// `push_clip` would incorrectly escape that clip for its own composite
/// draw. Finally `device.deregister_bindless`/`release_transient_target`
/// return the target to the pool, mirroring `render_to_texture_demo.rs`'s
/// own hand-written sequence exactly, just driven by the IR instead of
/// hand-written calls. Scoped to one level: a nested `PushLayer` (while
/// another is already active) panics -- see `# Panics`.
///
/// `vertex_buffer`/`index_buffer` are [`BufferBinding`]s the caller
/// already populated -- via a one-shot backend upload helper (`offset:
/// 0`), or via a real per-frame `RhiDynamicRingBuffer::write` call
/// (whatever offset it returned). Either way, "Buffer Packing"
/// (DESIGN.md's own frame-lifecycle item 7) is a distinct stage this
/// function deliberately does not perform -- it only binds at whatever
/// offset the caller's own packing already produced (IMPLEMENTATION.md
/// Phase 8 Step 8.1.2).
///
/// `clip_stack` is caller-owned scratch, cleared (not reallocated) at the
/// top of every call -- REVIEW.md finding #222: a fresh `Vec` here on
/// every rendered frame that clips anything was the last remaining
/// per-frame heap allocation in this function. Pass the same `Vec` back
/// in on every frame from the same render loop so its capacity is
/// actually reused; a brand-new empty `Vec` per call defeats the point
/// (though it's still correct either way, just not zero-allocation).
///
/// # Errors
/// Returns whatever `device.acquire_transient_target`/`register_
/// bindless`/`cmd_buffer.apply_layer_blur` themselves return (REVIEW.md
/// finding #141, fixed: this function previously had no `Result` return
/// type at all and `.expect()`-ed all three, panicking on exactly the
/// failures `EngineError::TransientPoolBudgetExceeded`'s own doc comment
/// calls "recoverable: a caller can release outstanding textures, wait
/// for the GC thread to catch up, and retry" -- a real, undisclosed gap,
/// not a documented limit, as finding #141's own writeup first
/// established before this fix closed it for real). Any transient
/// target already acquired before a later failure is released back to
/// the pool first -- a `PushLayer`'s own texture if `apply_layer_blur`
/// or `register_bindless` fails after it, or the just-blurred
/// replacement if only `register_bindless` fails -- so this function
/// never leaks one on a failure path.
///
/// # Panics
/// Panics if a `DrawGeometry` or `PopLayer` command's
/// `pipeline_state_id` was never registered in `registry`; if a
/// `PushLayer` is encountered while another is already active (true
/// nested layers are real, separate future work -- no real scene needs
/// them yet, matching `RhiCommandBuffer::resume_swapchain_rendering`'s
/// own single-level scope); or if a `PopLayer` is encountered with no
/// active `PushLayer`. Every pipeline a `Canvas` can emit is always
/// registered before a frame is rendered, so an unresolved id is a
/// static setup bug, not a transient, recoverable-mid-frame condition
/// (matching this crate's established `pop_layer`/`restore`/
/// `PipelineRegistry::register`-style precedent for invalid caller
/// state, not `EngineError`'s own device/resource failure modes) --
/// unlike the three failures above, none of these three is a real,
/// recoverable condition an `Err` return would let a caller act on.
#[allow(
    clippy::too_many_arguments,
    reason = "8 parameters, one over the lint's default threshold, after REVIEW.md finding \
              #222 added `clip_stack` -- every existing parameter is already load-bearing (see \
              this doc comment's own account of each one), and bundling them into a struct \
              would only formalize an already-obvious grouping without reducing real call-site \
              complexity, matching this crate's own precedent for allowing this lint where a \
              function's real arity reflects genuine caller-supplied inputs, not accidental \
              sprawl"
)]
#[allow(
    clippy::too_many_lines,
    reason = "REVIEW.md finding #141: widening this function to `Result<(), EngineError>` \
              added an explicit `match`/release-before-`Err` cleanup block at each of its two \
              `PopLayer` failure points (mirroring finding #189/#240's identical discipline \
              inside `apply_layer_blur` itself), pushing this over the lint's default line \
              threshold by a handful of lines -- splitting `PopLayer`'s own handling into a \
              separate function would only relocate this same logic, not reduce it, and this \
              function's real complexity already comes from `CommandType`'s four variants, not \
              accidental sprawl"
)]
pub fn execute_frame(
    frame: &FlattenedFrame,
    registry: &PipelineRegistry,
    vertex_buffer: BufferBinding<'_>,
    index_buffer: BufferBinding<'_>,
    full_window: &ScissorRect,
    device: &dyn RhiDevice,
    cmd_buffer: &mut dyn RhiCommandBuffer,
    clip_stack: &mut Vec<ScissorRect>,
) -> Result<(), EngineError> {
    // REVIEW.md finding #135: bound once, here, rather than inside the
    // loop below -- `vertex_buffer`/`index_buffer` are this whole call's
    // own parameters, invariant for every command in `frame`, and a
    // vertex/index buffer binding is command-buffer state that persists
    // across `PopLayer`'s own render-target switch (`begin_render_to_
    // texture`/`resume_swapchain_rendering` never touch it -- unlike
    // viewport/scissor, finding #128), so rebinding it per command was
    // pure redundant driver overhead, not a correctness requirement.
    cmd_buffer.bind_vertex_buffer(vertex_buffer.buffer, vertex_buffer.offset);
    cmd_buffer.bind_index_buffer(index_buffer.buffer, index_buffer.offset);

    // REVIEW.md finding #222: `clip_stack` is now caller-owned, persistent
    // scratch reused across frames (cleared, not reallocated) -- matching
    // this crate's own zero-allocation-reuse convention elsewhere
    // (`FrameArena`'s scratch fields, `ShapeRegistry::polygon_points_
    // scratch`) instead of a fresh `Vec` on every one of ~30 call sites'
    // own per-frame call.
    clip_stack.clear();
    // The layer's own requested (logical) width/height ride alongside
    // the texture itself -- REVIEW.md finding #152: `PopLayer`'s own
    // `apply_layer_blur` call (Step 7.2.2) needs the *requested* size,
    // not whatever the acquired texture's own real dimensions are.
    let mut active_layer: Option<(Box<dyn RhiTexture>, u32, u32)> = None;
    for command in &frame.commands {
        match command.kind {
            CommandType::DrawGeometry => {
                let pipeline = registry.get(command.pipeline_state_id).unwrap_or_else(|| {
                    panic!(
                        "execute_frame: no pipeline registered for id {}",
                        command.pipeline_state_id
                    )
                });
                cmd_buffer.set_pipeline(pipeline);
                cmd_buffer.bind_texture(0, command.texture_handle);
                // Phase 10 Step 10.2.3: a `PipelineKind::FlatColorBlend`
                // draw reads the destination pixel a PRECEDING draw
                // already wrote (`VK_KHR_dynamic_rendering_local_read`)
                // -- the barrier runs before EVERY such draw, not once
                // per frame, since each one must see whatever the very
                // latest framebuffer state is, including ordinary draws
                // that ran since the last blend-mode draw.
                if command.pipeline_state_id == PipelineKind::FlatColorBlend as u16 {
                    cmd_buffer.insert_blend_read_barrier();
                }
                cmd_buffer.draw_indexed(command.element_count, command.vertex_offset, 0);
            }
            CommandType::PushScissor => {
                let resolved = if command.clip_bounds == FULL_WINDOW_CLIP {
                    *full_window
                } else {
                    command.clip_bounds
                };
                clip_stack.push(resolved);
                cmd_buffer.set_scissor(&resolved);
            }
            CommandType::PopScissor => {
                clip_stack.pop();
                let restored = clip_stack.last().copied().unwrap_or(*full_window);
                cmd_buffer.set_scissor(&restored);
            }
            CommandType::PushLayer => {
                assert!(
                    active_layer.is_none(),
                    "execute_frame: nested PushLayer is not supported yet"
                );
                let format = u16_to_texture_format(command.pipeline_state_id);
                let texture = device.acquire_transient_target(
                    command.clip_bounds.width,
                    command.clip_bounds.height,
                    format,
                )?;
                // REVIEW.md finding #152: pass the *requested* size, not
                // whatever `texture` itself reports -- `acquire_transient_
                // target`'s oversized-borrow fallback can return something
                // larger, and `begin_render_to_texture`'s own doc comment
                // is explicit that its `logical_width`/`logical_height`
                // parameters must be the caller's original intent.
                cmd_buffer.begin_render_to_texture(
                    &*texture,
                    command.clip_bounds.width,
                    command.clip_bounds.height,
                );
                active_layer = Some((
                    texture,
                    command.clip_bounds.width,
                    command.clip_bounds.height,
                ));
            }
            CommandType::PopLayer => {
                let (texture, layer_width, layer_height) = active_layer
                    .take()
                    .expect("execute_frame: PopLayer with no active PushLayer");
                cmd_buffer.end_render_to_texture(&*texture);

                // `pop_layer`'s own doc comment: `texture_handle` carries
                // the popped `LayerDesc`'s own `blur` flag here, never a
                // real bindless index (Step 7.2.2).
                let composited_texture = if command.texture_handle != 0 {
                    // REVIEW.md finding #141: propagates now instead of
                    // `.expect()`-ing -- `texture` is still held (not yet
                    // released) at this point, so a failure here releases
                    // it back to the pool before returning `Err`, the same
                    // "don't leak on the newly-reachable failure path"
                    // discipline finding #189/#240 already applied inside
                    // `apply_layer_blur`'s own internal hops.
                    let blurred = match cmd_buffer.apply_layer_blur(
                        device,
                        &*texture,
                        layer_width,
                        layer_height,
                    ) {
                        Ok(blurred) => blurred,
                        Err(e) => {
                            device.release_transient_target(texture);
                            return Err(e);
                        }
                    };
                    device.release_transient_target(texture);
                    // REVIEW.md finding #153: `apply_layer_blur`'s own
                    // internal hops rebind the command buffer's vertex/
                    // index buffers to its own small, unit-quad ones --
                    // finding #135's own "bound once, at the top"
                    // invariant otherwise leaves this frame's real
                    // vertex/index buffers un-bound for the composite
                    // draw below, a real bug a real GPU run caught
                    // immediately (an out-of-bounds index read, since the
                    // composite quad's own real `vertex_offset` doesn't
                    // exist in `apply_layer_blur`'s own tiny buffer).
                    // Restored here, the same "undo whatever
                    // this call disturbed" responsibility `resume_
                    // swapchain_rendering`'s own scissor-restore already
                    // established.
                    cmd_buffer.bind_vertex_buffer(vertex_buffer.buffer, vertex_buffer.offset);
                    cmd_buffer.bind_index_buffer(index_buffer.buffer, index_buffer.offset);
                    blurred
                } else {
                    texture
                };
                // `composited_texture` (either the just-blurred texture or
                // the original layer's own, if no blur was requested) is
                // still held at this point -- release it back to the pool
                // before propagating, for the identical leak-avoidance
                // reason as the `apply_layer_blur` failure path above.
                let bindless_index = match device.register_bindless(&*composited_texture) {
                    Ok(index) => index,
                    Err(e) => {
                        device.release_transient_target(composited_texture);
                        return Err(e);
                    }
                };
                cmd_buffer.resume_swapchain_rendering();
                let restored = clip_stack.last().copied().unwrap_or(*full_window);
                cmd_buffer.set_scissor(&restored);

                let pipeline = registry.get(command.pipeline_state_id).unwrap_or_else(|| {
                    panic!(
                        "execute_frame: no pipeline registered for id {}",
                        command.pipeline_state_id
                    )
                });
                cmd_buffer.set_pipeline(pipeline);
                cmd_buffer.bind_texture(0, bindless_index);
                cmd_buffer.draw_indexed(command.element_count, command.vertex_offset, 0);

                device.deregister_bindless(bindless_index);
                device.release_transient_target(composited_texture);
            }
        }
    }
    Ok(())
}

/// Wraps the "begin a frame, record into it, submit and present" sandwich
/// every real caller in the workspace was hand-writing identically --
/// found via this project's own `/review-project` process (REVIEW.md
/// #200): ~40 example programs plus `tre-python`'s own `HeadlessRenderer`
/// each repeated
/// `device.begin_frame(swapchain)` -> (some per-frame recording) ->
/// `device.submit_and_present(cmd_buffer, swapchain, image)` verbatim,
/// with only the recording step itself ever differing -- a single
/// [`execute_frame`] call for callers that go through the sorted/batched
/// IR pipeline, or hand-written `set_pipeline`/`bind_vertex_buffer`/
/// `draw_indexed` calls for callers that don't.
///
/// `record` receives the freshly acquired command buffer and does
/// whatever per-frame work this caller needs -- an [`execute_frame`]
/// call, several draw calls, or (Step 6.4.2/7.2.2) a `push_layer`/
/// `pop_layer` render-to-texture sequence entirely encoded as `UiDrawCommand`s
/// `execute_frame` already dispatches. This function only owns the part
/// that never varies: fence-wait/image-acquire (inside `begin_frame`)
/// and submit/present (inside `submit_and_present`), both real,
/// swapchain-agnostic RHI operations already generic over `&dyn
/// RhiSwapchain` -- headless and windowed callers share this one
/// function with no branching.
///
/// # Errors
/// Returns whatever [`RhiDevice::begin_frame`]/[`RhiDevice::
/// submit_and_present`] themselves return -- see their own docs.
pub fn submit_frame<F>(
    device: &dyn RhiDevice,
    swapchain: &dyn RhiSwapchain,
    record: F,
) -> Result<(), EngineError>
where
    F: FnOnce(&mut dyn RhiCommandBuffer),
{
    let (mut cmd_buffer, image) = device.begin_frame(swapchain)?;
    record(&mut *cmd_buffer);
    device.submit_and_present(cmd_buffer, swapchain, image)
}

/// Identical to [`submit_frame`] except it begins the frame via
/// [`RhiDevice::begin_frame_with_logical_size`] instead of
/// [`RhiDevice::begin_frame`] -- REVIEW.md finding #235's real fix for a
/// swapchain deliberately held at a coarser size than the real window
/// during an active resize drag.
///
/// # Errors
/// Same as [`submit_frame`].
pub fn submit_frame_with_logical_size<F>(
    device: &dyn RhiDevice,
    swapchain: &dyn RhiSwapchain,
    logical_size: (u32, u32),
    record: F,
) -> Result<(), EngineError>
where
    F: FnOnce(&mut dyn RhiCommandBuffer),
{
    let (mut cmd_buffer, image) = device.begin_frame_with_logical_size(swapchain, logical_size)?;
    record(&mut *cmd_buffer);
    device.submit_and_present(cmd_buffer, swapchain, image)
}

/// Identical to [`submit_frame`] except it begins the frame via
/// [`RhiDevice::begin_frame_with_options`] with `crop_size` set --
/// see that method's own doc comment for why this is a different
/// tradeoff from [`submit_frame_with_logical_size`], not a drop-in
/// replacement for the same call site. The caller is responsible for
/// pairing this with a matching compositor-side viewport source crop of
/// the identical `crop_size` (e.g.
/// `tre_platform::PlatformConnection::set_viewport_source_crop`) --
/// without it, this alone just renders into a corner of the buffer that
/// the compositor still scales/resamples like any other content.
///
/// # Errors
/// Same as [`submit_frame`].
pub fn submit_frame_with_viewport_crop<F>(
    device: &dyn RhiDevice,
    swapchain: &dyn RhiSwapchain,
    crop_size: (u32, u32),
    record: F,
) -> Result<(), EngineError>
where
    F: FnOnce(&mut dyn RhiCommandBuffer),
{
    let (mut cmd_buffer, image) = device.begin_frame_with_options(
        swapchain,
        BeginFrameOptions {
            crop_size: Some(crop_size),
            ..Default::default()
        },
    )?;
    record(&mut *cmd_buffer);
    device.submit_and_present(cmd_buffer, swapchain, image)
}
