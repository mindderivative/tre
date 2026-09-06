# Rendering Engine Architecture & System Blueprint

## 1. Subsystem Decomposition & Topography

The engine architecture is highly decoupled, operating strictly as a bridge between a high-level UI Layout Framework and the underlying hardware drivers. It is divided into four primary domains: the **Platform & Event Layer**, the **Core Processing Engine**, the **Dynamic Batching Engine**, and the **Render Hardware Interface (RHI)**.

*Implemented in the `tre-platform` crate -- Linux only (Wayland/X11) for now; Windows/macOS bridges are later steps. As of Phase 1 Step 1 (2026-09-05), `PlatformConnection` owns native multi-window creation (one shared connection per backend, TECHNICAL.md Section 9.2). As of Phase 1 Step 2 (2026-09-05), it also owns the real OS input/event pump: pointer and keyboard events are translated into `tre_engine::InputEvent`s and pushed through `tre_engine::InputEventQueue` (TECHNICAL.md Section 8), which coalesces high-frequency pointer motion before a caller drains it.*

```text
+-----------------------------------------------------------------------------------+
|                        PLATFORM & EVENT LAYER (Decoupled)                         |
|   +--------------------------+  +--------------------------+  +-----------------+ |
|   | Multi-Window Swapchains  |  |  OS Input / Event Pump   |  | OS a11y Bridge  | |
|   +--------------------------+  +--------------------------+  +-----------------+ |
+-----------------------------------------------------------------------------------+
                                         | (Events pushed to UI Framework)
                                         v
+-----------------------------------------------------------------------------------+
|                             CORE ENGINE MIDDLEWARE                                |
|   +-----------------------+  +--------------------------+  +--------------------+ |
|   | Rendering Canvas API  |  | Dynamic Texture Atlas    |  | SVG Tessellation   | |
|   | (Sub-Canvas Threads)  |  | (Guillotine + LRU GC)    |  | & MSDF Engine      | |
|   +-----------------------+  +--------------------------+  +--------------------+ |
|                                        |                                          |
|                                        v (Intermediate Representation Array)      |
|   +-----------------------------------------------------------------------------+ |
|   |                   Dynamic Command Sort & Batching Engine                    | |
|   |         (64-bit Radix Sort -> State Grouping -> Index Merging)              | |
|   +-----------------------------------------------------------------------------+ |
+-----------------------------------------------------------------------------------+
                                         | (Batched Draw Calls)
                                         v
+-----------------------------------------------------------------------------------+
|                        RENDER HARDWARE INTERFACE (RHI)                            |
|   +------------------------+  +------------------------+  +---------------------+ |
|   |  RHI Command Buffer    |  |  Transient Render Pool |  | Dynamic Ring Buffer | |
|   +------------------------+  +------------------------+  +---------------------+ |
|               |                            |                         |            |
|               v                            v                         v            |
|   [ Vulkan 1.2 Native ]           [ DirectX 12 Native ]         [ Metal Native ]    |
+-----------------------------------------------------------------------------------+
```

---

## 2. Multi-Window & Threading Architecture

To support complex desktop applications, the engine separates global GPU resources from window-specific surfaces.

*Implementation status (Phase 2 Step 2.3, 2026-09-06):* the `SubCanvas` worker threads (Section 2.2) and the shared-atlas concurrency model (Section 2.3) below remain design, not yet built -- neither `SubCanvas` nor the dynamic texture atlas exists in the codebase yet. The engine's actual first real OS thread is `tre-rhi-vulkan`'s background GC thread (TECHNICAL.md Section 3.3), which evicts stale entries from the transient render-target pool. It deliberately never touches a Vulkan call itself -- only the main thread destroys GPU objects, after a grace period -- so it doesn't yet exercise the harder cross-thread-GPU-object-lifetime questions this section's future worker threads will eventually raise.

### 2.1 Shared Context & Swapchains
* **Global `RhiDevice`:** A single logical GPU device instance is instantiated at startup. It owns all global resources: the MSDF font engine, the dynamic texture atlas, static pipeline states (PSOs), and the SVG path cache.
* **Per-Window Contexts:** Each native window (Win32 `HWND`, Wayland `wl_surface`, macOS `NSWindow`) holds a dedicated `RhiSwapchain`, a local event queue, and its own multi-threaded command arena. 
* **Headless Contexts:** For CI/CD and server rendering, virtual swapchains are created that read back framebuffers directly to CPU staging memory without OS surface dependencies.

### 2.2 Sub-Canvas Multi-Threading
Drawing complex UIs (like large lists or data grids) is parallelized via the `Canvas` API:
1. **Main Thread:** The UI framework evaluates layout and pushes root transformations.
2. **Worker Threads:** The UI framework spawns `SubCanvas` instances. Each worker thread processes a subset of the UI tree, executing bounds-checking and generating `UiDrawCommand` structs into thread-local linear arenas.
3. **Lock-Free Stitching:** During the engine's "Flattening Phase," thread-local IR arrays are atomic-added (`AtomicUsize::fetch_add`) into the main window's global frame arena, entirely avoiding mutex contention.

### 2.3 Shared Atlas Concurrency Model

The global `RhiDevice` (Section 2.1) owns one dynamic texture atlas shared by every window. Because windows can render on independent per-window timelines (Section 2.1's "Headless Contexts" and DESIGN.md Section 4.1's "Multi-Window Synchronization"), two windows can simultaneously discover a missing glyph and both need to mutate that one atlas -- DESIGN.md Section 10.3 states the principle; this is the concrete data-structure design.

The Guillotine free-rectangle list itself has exactly one writer, always. Everything *around* the packer is lock-free:

```rust
// One MPSC producer slot per pending request; the atlas owner is the sole consumer.
struct AtlasInsertRequest {
    key: AtlasKey,              // (font_id, glyph_id) or icon identifier
    raster_source: RasterSourceHandle,
}

// The published result every window's render thread reads without locking.
// One slot per resident atlas key; the atlas owner is the sole writer.
struct AtlasSlot {
    // Packed (u,v,w,h) rect + generation counter, or a sentinel for "not yet resident."
    // Readers: `slot.load(Ordering::Acquire)`. Writer: `slot.store(new_value, Ordering::Release)`.
    packed: AtomicU64,
}
```

* **Request side (many producers, one consumer):** any window's tessellation phase that finds a missing key pushes an `AtlasInsertRequest` onto a bounded, pre-allocated MPSC ring buffer (TECHNICAL.md Section 8) and continues immediately -- it never waits for the atlas owner.
* **Packing (the one genuinely sequential step):** the atlas owner drains the MPSC queue, performs the Guillotine insertion and MSDF rasterization, and is the only code in the engine ever allowed to touch the free-rectangle list.
* **Publish side (one writer, many readers):** the atlas owner stores the new `(rect, generation)` into that key's `AtlasSlot` with `Ordering::Release`. Any window's render thread reads the same slot with `Ordering::Acquire` when resolving a glyph for batching; if the slot still holds the "not yet resident" sentinel, that window uses the placeholder-glyph fallback (DESIGN.md Section 2.6) for this frame and re-reads the slot next frame.
* **Why not a general concurrent hash map:** the access pattern here is add-only from exactly one writer -- no concurrent removes, no concurrent updates to an existing key. A general-purpose concurrent map (e.g. `dashmap`) would pay for guarantees (multi-writer safety, in-place removal) this workload never needs; a fixed-capacity array of `AtomicU64` slots, sized to the atlas's own slot budget (TECHNICAL.md Section 3.3), is both simpler and allocation-free, consistent with the zero-allocation steady state (DESIGN.md Section 2.1).

---

## 3. Core Data Structures & Memory Layout

### 3.1 Compact Vertex Structure
To strictly respect PCIe memory bandwidth and remain within the $128\text{ MB}$ dynamic VRAM footprint budget, UI geometry utilizes a dense 32-byte format. This is the canonical definition (established in the September 2026 documentation review) -- TECHNICAL.md Section 5.1 and IMPLEMENTATION.md Section 3.1 reference this struct rather than restating its fields.

```rust
#[repr(C, align(16))]
struct UiVertex {
    position: [f32; 2],  // 8 bytes:  Screen-Space X, Y
    uv: [f32; 2],        // 8 bytes:  Texture coordinates or SDF bounds
    color: u32,          // 4 bytes:  Packed RGBA8 (sRGB converted to Linear in shader)
    params: [f32; 3],    // 12 bytes: Shader params (Corner Radii, Stroke Width, etc.)
}                        // 32 Bytes Total

const _: () = assert!(std::mem::size_of::<UiVertex>() == 32);
```

`#[repr(C, align(16))]` fixes field order and padding to match a plain C struct, so the layout is deterministic across compilers and matches the GPU-side vertex input layout exactly -- this type does not itself cross the Python FFI boundary (TECHNICAL.md Section 9.4), but the same determinism requirement applies to any type that does.

### 3.2 Intermediate Representation (IR)
Instead of executing RHI commands instantly, the `Canvas` records lightweight structs representing draw intents:

```rust
#[repr(u8)]
enum CommandType {
    DrawGeometry,
    PushScissor,
    PopScissor,
    PushLayer,
    PopLayer,
}

#[repr(C)]
struct UiDrawCommand {
    kind: CommandType,           // `type` is a reserved keyword in Rust
    sort_key: u64,               // 64-bit Radix Sort Key
    pipeline_state_id: u16,
    texture_handle: u32,         // Bindless array index or atlas handle
    element_count: u32,          // Index count
    vertex_offset: u32,          // Offset into the dynamic ring buffer
    clip_bounds: ScissorRect,
}
```

---

## 4. Sorting, Batching & The 64-Bit Key

The engine's defining performance characteristic is its ability to reduce thousands of UI elements into single-digit draw calls via dynamic index restitching. This section is the canonical definition of the sort key (established in the September 2026 documentation review) -- TECHNICAL.md Section 4 and IMPLEMENTATION.md Section 6.1 reference this section rather than restating it; update it here first if the layout ever changes.

### 4.1 The 64-Bit Sort Key
Every `DrawGeometry` command generates a strict 64-bit integer key evaluated via a linear $\mathcal{O}(N)$ 4-pass Radix Sort.

$$\text{SortKey} = (\text{Layer ID} \ll 48) \mid (\text{Pipeline ID} \ll 32) \mid (\text{Texture ID} \ll 20) \mid (\text{Depth ID})$$

* **Layer ID (Bits 63:48, 16 bits):** Isolates depth planes. Standard content uses $0-9999$. Overlays, modal backdrops, and popups use $10000+$.
* **Pipeline ID (Bits 47:32, 16 bits):** Groups commands by shader execution state (e.g., SDF Rect, MSDF Text, Gaussian Blur).
* **Texture/Bindless ID (Bits 31:20, 12 bits):** Groups calls by active atlas. 4,096 concurrent slots -- comfortably above the low dozens of atlases (MSDF, R8 fallback, RGBA icon, dynamic SVG cache) the engine actually maintains.
* **Depth ID (Bits 19:0, 20 bits):** The original depth-first traversal index (post z-index resolution -- see below), ensuring overlapping alpha-blended elements composite in the exact order specified by the UI framework. Widened from 16 to 20 bits in the September 2026 documentation review: 1,048,576 slots gives a safe margin over the $>10{,}000\text{ node}$ trees the Architectural Decision Matrix targets, versus the previous field's thin 6.5x headroom. If a single frame's node count would still overflow 20 bits, the Canvas asserts in debug builds and, in release builds, splits the offending layer's content into two sequential sub-frame passes rather than wrapping the counter and silently corrupting paint order.

*Z-Index Resolution Order (added in the September 2026 documentation review):* Depth ID is assigned *after* z-index resolution, not from raw traversal order -- an element with an explicit z-index override gets the Depth ID matching its resolved paint position, not its position in the widget tree. This keeps Depth ID a true "final paint order" index in all cases, including the ones DESIGN.md Section 7.1 describes where z-index breaks strict depth-first order.

### 4.2 Batch Flattening
After sorting, a linear sweep consolidates commands:
1. If Command $A$ and Command $B$ share the exact same top 44 bits of their `sortKey` (Layer + Pipeline + Texture) and have identical `clipBounds`.
2. The index offsets for Command $B$ are rewritten relative to Command $A$.
3. They are dispatched to the RHI as a single `DrawIndexed` execution.

*Known limitation -- clip bounds are not key-encoded (documented in the September 2026 documentation review):* `clipBounds` is compared explicitly in step 1 above rather than folded into the 64-bit key itself, because there is no spare bit budget for a clip-group field without shrinking Layer, Pipeline, or Depth below their required ranges. This preserves *correctness* -- two commands are never merged unless they truly share both state and clip rect -- but it makes the *single-digit draw call* target a soft one, not a structural guarantee: if z-index resolution interleaves commands with different clip rects that would otherwise batch, the flattening pass emits more (still-correct) batches than the theoretical minimum. This is a performance-only risk, not a correctness bug, and is deliberately left unresolved pending real profiling data -- a clip-bucketing secondary pass is the natural future fix if measurement shows it matters in practice.

*If and when this is implemented, take the spare bits from Pipeline ID, never from Depth ID.* Depth ID was deliberately widened from 16 to 20 bits earlier in this same section specifically because 16 bits (a 6.5x margin over the >10,000-node target) was judged too thin -- reusing that exact bit budget for a clip-group field would silently reintroduce the problem that widening was meant to fix. Pipeline ID, by contrast, has real slack: 16 bits (65,536 shader pipeline states) is far beyond what a UI-focused engine's realistic pipeline-family count needs (SDF rect, MSDF text, plain texture, a handful of blur/blend variants -- low tens, not tens of thousands). Trimming Pipeline ID to roughly 10-12 bits (1,024-4,096 states, still generous headroom) frees 4-6 bits for a clip-group field (16-64 concurrent clip groups per frame), which comfortably covers the low number of distinct clip rects realistically active in one frame. The exact split should still be set from measured pipeline-state counts at the time this is implemented, not fixed here in advance.

---

## 5. Visual Compositing & Offscreen Layers

Modern UIs require grouped opacity and background blurs (glassmorphism). The architecture handles this via the `Transient Render Pool`.

1. **`Canvas::push_layer` Invocation:** When requested, the engine calculates the screen-space bounding box of the layer.
2. **Checkout:** It checks out an appropriately sized offscreen $RGBA16F$ (HDR) texture from the transient pool (a hash map of previously allocated, currently unused VRAM surfaces).
3. **Redirection:** Subsequent batched draw calls are redirected into this offscreen target.
4. **`Canvas::pop_layer` Invocation:** The offscreen texture is applied as a shader input (e.g., dual-kawase blur pass) to the main swapchain, and the texture is returned to the transient pool for reuse in the next frame.

---

## 6. Hardware Abstraction Layer (RHI) Interfaces

The RHI maps the batched IR commands directly to Vulkan, DirectX 12, or Metal concepts.

*Dynamic Dispatch Note (updated for the Rust implementation, originally added in the September 2026 documentation review):* `RhiDevice` and `RhiCommandBuffer` are Rust traits, invoked through `&dyn RhiDevice` / `&mut dyn RhiCommandBuffer` trait objects rather than monomorphized generics. This is a deliberate, bounded exception to the project's "no dynamic dispatch (`dyn Trait`) in tight loops -- prefer generics or enum dispatch" coding standard -- every trait-object call here (`set_pipeline`, `bind_texture`, `draw_indexed`, etc.) happens once per *batch*, not once per primitive or per vertex, and batch counts are single-digit to low-hundreds per frame (Section 4). The vtable-indirection overhead is amortized across every vertex in that batch and is negligible against the $\le 0.50\text{ ms}$ CPU budget. Using `dyn Trait` here also keeps the three backend implementations (Vulkan/DX12/Metal) out of the core engine crate's generic parameter list, avoiding a build that must be recompiled per backend. This exception does not extend to any per-primitive or per-vertex code path -- those must remain statically dispatched (generics or `#[inline]` enum matches), per the standing rule.

*Filled in during the Phase 0 walking skeleton implementation (2026-09-04, IMPLEMENTATION.md Phase 0 status note):* this section originally left `RhiBuffer`, `RhiTexture`, `RhiPipelineState`, and `RhiSwapchain` referenced (as `&dyn Rhi*` parameters) but undefined, and gave `begin_frame`/`submit_and_present` no way to report failure. Both gaps surfaced immediately on trying to actually implement a Vulkan backend against this sketch -- exactly the kind of interface mismatch Phase 0 exists to catch. The definitions below are the real, working, validated design (`crates/tre-engine/src/lib.rs`), not a plan.

*Updated for Phase 2 Step 1 (2026-09-05):* `create_dynamic_ring_buffer`/`acquire_transient_target`/`release_transient_target` were `unimplemented!()` stubs through Phase 0 and 1; they are real as of this step (`crates/tre-rhi-vulkan`'s `VulkanRingBuffer`/`VulkanTexture`, TECHNICAL.md Sections 3.1/3.2). Frame submission itself stays fully synchronous (one frame in flight at a time) -- the ring buffer's 3-segment structure and the transient pool's deferred-growth queue are real and correct, but genuine overlapping multi-frame-in-flight GPU/CPU submission is deferred to a future step, per `planning/archive/PLAN_PHASE2_STEP1.md`'s scope decision.

*Updated for Phase 2 Step 2.1 (2026-09-05):* `RhiCommandBuffer::bind_texture` was a Phase 0 `unimplemented!()` stub ("Phase 4 (bindless atlas textures) -- out of Phase 0's scope"); it is real as of this step, backed by `tre-rhi-vulkan`'s persistent bindless descriptor array (`VK_EXT_descriptor_indexing`, TECHNICAL.md Section 2.1/9.2). `RhiTexture` gained `bindless_index`, and `RhiDevice` gained `create_texture` (uploading real CPU pixel data as a sampled, bindless-registered texture -- distinct from `acquire_transient_target`'s empty render targets). The bindless index selects a texture per *draw call* (a push constant), not per vertex -- the per-vertex indexing DESIGN.md Section 8.1.2 describes for cross-atlas batching needs the `Canvas`-to-RHI renderer Phase 3/4 builds, which doesn't exist yet. See `planning/archive/PLAN_PHASE2_STEP2_1.md`.

```rust
/// An acquired swapchain image, threaded from `RhiSwapchain::acquire_next_image`
/// through `RhiDevice::begin_frame` to the caller and back to
/// `RhiDevice::submit_and_present`. Every `_handle` field is a
/// backend-specific opaque integer (e.g. a Vulkan handle reinterpreted via
/// `ash::vk::Handle::as_raw`) -- deliberately, so concrete `RhiDevice`/
/// `RhiCommandBuffer`/`RhiSwapchain` implementations never need
/// `std::any::Any` downcasting to recover their own state from a trait
/// object, which TECHNICAL.md Section 9.1 bans from the per-frame path.
/// This mirrors how Vulkan itself represents every object as an opaque
/// `u64`; no runtime type identification happens anywhere in the exchange.
struct AcquiredImage {
    index: u32,
    target_view_handle: u64,
    target_image_handle: u64,
    image_available_semaphore_handle: u64,
    /// Per-swapchain-image, not shared across frames: reusing one
    /// semaphore for every frame's present is a real hazard the Vulkan
    /// validation layer catches (`VUID-vkQueueSubmit-pSignalSemaphores-00067`)
    /// -- the CPU-side fence `begin_frame` waits on covers the queue
    /// submit's completion, not the separate, asynchronous present
    /// operation's.
    render_finished_semaphore_handle: u64,
}

trait RhiBuffer {
    fn raw_handle(&self) -> u64;
}

/// `image_handle`/`memory_handle` added in Phase 2 Step 1: a backend
/// needs all three opaque handles (view, image, backing memory) to
/// reconstruct its own concrete texture type from a `Box<dyn RhiTexture>`
/// -- e.g. `RhiDevice::release_transient_target` receives one back from a
/// caller and must recover enough to store/eventually destroy it. Same
/// opaque-handle pattern as `AcquiredImage`, not a downcast.
trait RhiTexture {
    fn raw_handle(&self) -> u64;
    fn image_handle(&self) -> u64;
    fn memory_handle(&self) -> u64;
    fn dimensions(&self) -> (u32, u32);
    fn format(&self) -> TextureFormat;
    /// Added Phase 2 Step 2.1: this texture's slot in the RHI's persistent
    /// bindless texture array, or `None` for a transient render target
    /// (which is written to, not sampled from, and isn't registered into
    /// the array).
    fn bindless_index(&self) -> Option<u32>;
    /// Added Phase 2 Step 2.3: this texture's real GPU allocation size in
    /// bytes, so `RhiDevice::release_transient_target` can maintain the
    /// transient pool's total-free-bytes accounting (the generational
    /// GC's 85%-of-budget trigger, TECHNICAL.md Section 3.3) without
    /// re-querying it.
    fn size_bytes(&self) -> u64;
}

trait RhiPipelineState {
    fn raw_handle(&self) -> u64;
    /// Opaque handle of this pipeline's layout, needed by
    /// `RhiCommandBuffer::set_pipeline` implementations that push
    /// constants/descriptors keyed by layout (e.g. `vkCmdPushConstants`).
    fn layout_handle(&self) -> u64;
}

/// TECHNICAL.md Section 3.1's triple-buffered dynamic ring buffer. A
/// distinct trait from `RhiBuffer` (Phase 2 Step 1), not extra methods
/// added to it, since callers use a fundamentally different pattern:
/// bump-allocate into the current frame's segment every frame, rather
/// than upload-once-and-keep-forever.
trait RhiDynamicRingBuffer: RhiBuffer {
    /// Bump-allocates from the current frame's segment, returning the
    /// byte offset written at (usable directly as a
    /// `bind_vertex_buffer`/`bind_index_buffer` offset), or `None` if the
    /// segment has no room left this frame (DESIGN.md Section 2.6:
    /// starvation is reported, never grown dynamically mid-frame).
    fn write(&self, bytes: &[u8]) -> Option<u32>;
}

trait RhiSwapchain {
    fn extent(&self) -> (u32, u32);
    fn acquire_next_image(&self) -> Result<AcquiredImage, EngineError>;
    /// Waits on `image.render_finished_semaphore_handle` before showing
    /// the image (DESIGN.md Section 2.6 -- surfaces failures rather than
    /// stalling or panicking).
    fn present(&self, image: AcquiredImage) -> Result<(), EngineError>;
}

trait RhiDevice {
    // Resource Management -- real as of Phase 2 Step 1 (TECHNICAL.md
    // Sections 3.1/3.2): `create_dynamic_ring_buffer` returns a distinct
    // `RhiDynamicRingBuffer` trait (bump-allocate-per-frame is a
    // different usage pattern than a plain upload-once `RhiBuffer`), and
    // `acquire_transient_target` takes a `TextureFormat` so the pool can
    // key on `(Width, Height, Format)` as Section 3.2 requires.
    fn create_dynamic_ring_buffer(&self, capacity: usize) -> Box<dyn RhiDynamicRingBuffer>;
    /// Added Phase 2 Step 2.3 Code Review finding #80: returns `Result` --
    /// a genuinely novel size that would need cold-allocating while the
    /// pool's idle free bytes are already at the dynamic-VRAM budget is a
    /// real, recoverable admission failure, not a panic. The common case
    /// (reusing an already-pooled size) never fails this way.
    fn acquire_transient_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<Box<dyn RhiTexture>, EngineError>;
    fn release_transient_target(&self, texture: Box<dyn RhiTexture>);
    /// Added Phase 2 Step 2.1: uploads real CPU pixel data as a new
    /// GPU-resident sampled texture and registers it into the persistent
    /// bindless array, so `bindless_index()` can be passed straight to
    /// `RhiCommandBuffer::bind_texture`. A genuine one-time upload, unlike
    /// `acquire_transient_target`'s pool checkout. Returns `Result` (Phase
    /// 2 Code Review findings #66/#67): a mismatched `pixels` length/zero
    /// dimensions and bindless-array exhaustion are both real, recoverable
    /// failure conditions, not programmer-error panics.
    fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Result<Box<dyn RhiTexture>, EngineError>;

    // Command Submission -- both return `Result` (added during Phase 0;
    // the original sketch didn't), since DESIGN.md Section 2.6 requires
    // device-loss/swapchain-out-of-date conditions to be "detected at
    // `RhiDevice::begin_frame` and surfaced as a recoverable error," which
    // an infallible return type cannot do.
    fn begin_frame(
        &self,
        swapchain: &dyn RhiSwapchain,
    ) -> Result<(Box<dyn RhiCommandBuffer>, AcquiredImage), EngineError>;
    fn submit_and_present(
        &self,
        cmd_buffer: Box<dyn RhiCommandBuffer>,
        swapchain: &dyn RhiSwapchain,
        image: AcquiredImage,
    ) -> Result<(), EngineError>;
}

trait RhiCommandBuffer {
    // State Tracking
    fn set_pipeline(&mut self, pipeline: &dyn RhiPipelineState);
    fn set_scissor(&mut self, rect: &ScissorRect);

    // Bindings (Leveraging Bindless where available)
    fn bind_vertex_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_index_buffer(&mut self, buffer: &dyn RhiBuffer, offset: u32);
    fn bind_texture(&mut self, slot: u32, bindless_index: u32);

    // Execution
    fn draw_indexed(&mut self, index_count: u32, start_index: u32, base_vertex: i32);

    /// Needed so `RhiDevice::submit_and_present` can recover the concrete
    /// backend's submittable handle from a `Box<dyn RhiCommandBuffer>` --
    /// via the same opaque-handle pattern as `AcquiredImage`, not
    /// downcasting.
    fn raw_handle(&self) -> u64;
}
```

### 6.1 Default Pipeline State (PSO) Configuration

Added in the September 2026 documentation review -- the standard 2D content pipeline's blend/depth configuration was previously implicit:

* **Depth Test:** Disabled. Paint order is fully determined by the Depth ID field of the sort key (Section 4.1), not a GPU depth test, since 2D UI compositing requires exact painter's-algorithm ordering rather than depth-buffer occlusion.
* **Depth Write:** Disabled, for the same reason.
* **Blending:** Enabled, premultiplied-alpha "over" compositing, evaluated in linear color space (DESIGN.md Section 11.1 / TECHNICAL.md Section 6.2). Premultiplied alpha is required for correct results when `PushLayer` offscreen composites (Section 5) are later blended back into a parent target.
* **Culling:** Disabled (or front-and-back both drawn) -- 2D quads have no meaningful winding-order culling benefit and disabling it removes a class of "invisible rect" bugs from incorrect vertex winding.

*Future consideration -- opaque pre-pass (not implemented; profile before building):* Depth-test-off means the GPU gets no early-Z rejection, so overdraw-heavy scenes (e.g. a dense data grid with thousands of large, fully-opaque cell backgrounds) pay full fragment cost for content later fragments completely cover. A front-to-back, depth-tested pre-pass restricted to batches provably fully opaque (nothing SDF-antialiased or alpha-sampled can participate) could reclaim that cost via early-Z, at the price of a second pipeline state, a second command-buffer pass, and careful ordering against the existing Depth-ID-driven painter's-algorithm pass so the two agree on what's already covered. Do not build this speculatively: profile a representative overdraw-heavy scene first and confirm GPU time -- not CPU submission time -- is the actual bottleneck before spending the complexity budget here.
