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

*Implementation status (Phase 4 Step 4.2.4, 2026-09-06):* Section 2.3's shared-atlas concurrency model above is now real -- `tre_atlas::AtlasOwner` is a second dedicated background OS thread (the GC thread's own precedent, generalized), draining a real `tre_memory::MpscRingBuffer<AtlasInsertRequest>` and publishing into a real `tre_memory::SwmrSlotTable`, exactly the two structures this section's own code snippet already specified. `SubCanvas` (Section 2.2) itself is still not built -- this step's own demo uses ordinary `std::thread::spawn` producer threads as a deliberate stand-in, proving the concurrency primitives under genuine concurrent stress without claiming per-window worker threads now exist. Since the atlas owner's own work (Guillotine packing, MSDF generation) is pure CPU, it shares the GC thread's other property too: neither of this engine's two real background threads touches a Vulkan call directly.

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
* **Why not a general concurrent hash map:** every mutation -- insertion and, as of Phase 4 Step 4.3.1, removal too (tombstone-based, for the LRU eviction policy Section 10.2 describes) -- comes from exactly one writer thread; there is still no *concurrent* remove or update from multiple threads, only the single atlas owner ever mutating the table while any number of readers `get`/`get_and_touch` it lock-free. A general-purpose concurrent map (e.g. `dashmap`) would pay for guarantees (multi-writer safety, in-place removal safe from any thread) this workload never needs; a fixed-capacity array of `AtomicU64` slots, sized to the atlas's own slot budget (TECHNICAL.md Section 3.3), is both simpler and allocation-free, consistent with the zero-allocation steady state (DESIGN.md Section 2.1).

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

*Confirmed by Phase 5 Step 5.1.1 (2026-09-07):* `color` must already be premultiplied by whatever effective alpha the caller wants -- not just carry a reduced alpha channel with RGB left at full brightness. Section 6.1's premultiplied-alpha blend state is only correct if its *input* is already premultiplied, and the existing `sdf_rounded_rect.frag` shader (Step 3.2) only ever multiplies `frag_color.rgb` by the SDF's own coverage term, never by `frag_color.a`; a vertex color with `a < 255` but RGB left un-scaled produces an over-bright premultiplied value once coverage is folded in, which the GPU silently clamps back to fully opaque. `RenderingCanvas::draw_rounded_rect`'s own `premultiply_alpha` helper scales all four channels together for exactly this reason -- confirmed by a real GPU render during that step's own implementation, not just reasoned through in the abstract. Any future primitive that writes `UiVertex::color` directly (`draw_text`, `draw_path`, `draw_svg` -- Step 5.1.2 onward) needs to follow the same convention.

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

*Updated for Phase 9 Step 9.1 (2026-09-09):* the "linear $\mathcal{O}(N)$ 4-pass Radix Sort" this section has always specified is real as of this step (`tre-engine`'s `radix_sort_by_key`) -- `flatten_run` had used `std::sort_unstable_by_key` (a comparison sort) since Step 5.1.3 first built it, an undisclosed gap this step's own pre-work found and closed, not a design change. See TECHNICAL.md Section 4's own matching annotation and REVIEW.md for the full account, including the adversarial and randomized-differential test coverage that verifies it.

### 4.2 Batch Flattening
After sorting, a linear sweep consolidates commands:
1. If Command $A$ and Command $B$ share the exact same top 44 bits of their `sortKey` (Layer + Pipeline + Texture) and have identical `clipBounds`.
2. The index offsets for Command $B$ are rewritten relative to Command $A$.
3. They are dispatched to the RHI as a single `DrawIndexed` execution.

*Known limitation -- clip bounds are not key-encoded (documented in the September 2026 documentation review):* `clipBounds` is compared explicitly in step 1 above rather than folded into the 64-bit key itself, because there is no spare bit budget for a clip-group field without shrinking Layer, Pipeline, or Depth below their required ranges. This preserves *correctness* -- two commands are never merged unless they truly share both state and clip rect -- but it makes the *single-digit draw call* target a soft one, not a structural guarantee: if z-index resolution interleaves commands with different clip rects that would otherwise batch, the flattening pass emits more (still-correct) batches than the theoretical minimum. This is a performance-only risk, not a correctness bug, and is deliberately left unresolved pending real profiling data -- a clip-bucketing secondary pass is the natural future fix if measurement shows it matters in practice.

*Real implementation's conservative barrier policy (IMPLEMENTATION.md Step 5.1.3):* the Rust implementation resolves the above soft target by treating every non-`DrawGeometry` IR command (`PushScissor`/`PopScissor`/`PushLayer`/`PopLayer`) as a hard barrier -- sorting and merging happen independently within each maximal run of consecutive `DrawGeometry` commands, never across one. This is a strictly stronger guarantee than the two-part "same top 44 bits and identical `clipBounds`" criterion alone requires (every command within one such run already shares identical `clipBounds` by construction, since nothing changed the clip stack mid-run), so it costs nothing in correctness; it costs a small, deliberately-accepted amount of the theoretical merge opportunity, exactly the risk this section already calls out as acceptable pending real measurement. `PushLayer`/`PopLayer` must be hard barriers regardless (reordering a draw across an offscreen-compositing redirect would draw it into the wrong render target); treating `PushScissor`/`PopScissor` the same way, rather than trying to reorder across them since each command's own `clipBounds` is self-contained, is the conservative choice this implementation makes today.

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

*Updated for Phase 6 Step 6.1 (2026-09-08):* a real `PipelineRegistry` (`tre-engine`) maps a `UiDrawCommand::pipeline_state_id` to its real `RhiPipelineState` object -- generic over the trait below, no new RHI surface. Not part of this section's own canonical trait sketch since it lives one layer above the RHI itself (an IR-to-pipeline-object lookup a future executor consults, not something a backend implements) -- named here because it exists specifically to replace the two demos' own hardcoded `if pipeline_state_id == PIPELINE_MSDF_TEXT` dispatch, the same kind of "real design supersedes the original sketch" fact this section already tracks for `bind_texture`/`acquire_transient_target` above. See `planning/archive/PLAN_PHASE6_STEP6_1.md`.

*Updated for Phase 6 Step 6.2 (2026-09-08):* `execute_draw_geometry_batches` (`tre-engine`) is the real consumer the previous note anticipated -- drives every `DrawGeometry` batch in a `FlattenedFrame` through `set_pipeline`/`bind_texture`/`bind_vertex_buffer`/`bind_index_buffer`/`draw_indexed` below, resolving each command's pipeline via `PipelineRegistry`, with `PushScissor`/`PushLayer` markers skipped (Steps 6.3/6.4's own job). Not itself part of the canonical trait sketch for the same reason `PipelineRegistry` isn't -- a free function driving the traits below, not one of them. See `planning/archive/PLAN_PHASE6_STEP6_2.md`.

*Updated for Phase 6 Step 6.3 (2026-09-08):* renamed to `execute_frame` -- real `PushScissor`/`PopScissor` execution added, calling `set_scissor` below via a runtime clip stack (a `PopScissor` command's own `clip_bounds` carries no restore data). A new `full_window: &ScissorRect` parameter substitutes the real framebuffer extent for `FULL_WINDOW_CLIP`'s own CPU-side-only sentinel wherever it would otherwise reach `set_scissor` -- passing that sentinel's `u32::MAX` fields literally would be an invalid scissor rect on real hardware. See `planning/archive/PLAN_PHASE6_STEP6_3.md`.

*Updated for Phase 6 Step 6.4.1 (2026-09-08):* `RhiCommandBuffer` gains `begin_render_to_texture`/`end_render_to_texture` (real render-to-texture, with correct `UNDEFINED`/`COLOR_ATTACHMENT_OPTIMAL`/`SHADER_READ_ONLY_OPTIMAL` layout transitions) and `resume_swapchain_rendering`; `RhiDevice` gains `register_bindless`/`deregister_bindless`, letting an already-rendered-into texture become sample-able without a full `create_texture`-style pixel upload. Unlike `PipelineRegistry`/`execute_frame`, these genuinely belong in this section's own canonical sketch -- real RHI trait surface, not a layer above it -- but are documented here via annotation rather than rewriting the code block below, matching this section's own established pattern for every prior addition. Scoped to single-level layer use only (resuming the swapchain, not an *outer* layer's own target) -- true nested layers are real, separate future work with no real scene to prove them against yet. See `planning/archive/PLAN_PHASE6_STEP6_4_1.md` and REVIEW.md finding #128 for a real bug this step's own first run found: Vulkan's dynamic viewport/scissor state persists across `cmd_begin_rendering` calls, so `resume_swapchain_rendering` must explicitly restore both, not just re-begin rendering and assume they carried over correctly.

*Updated for Phase 6 Step 6.4.2 (2026-09-08):* `execute_frame` gains a `device: &dyn RhiDevice` parameter and real `PushLayer`/`PopLayer` handling, closing out Step 6.2's own `PushLayer`/`PopLayer`-markers-skipped note above -- `PushLayer` decodes a `TextureFormat` back out of its own `pipeline_state_id` field and `acquire_transient_target`s/`begin_render_to_texture`s into it; `PopLayer` `end_render_to_texture`s, `register_bindless`s, `resume_swapchain_rendering`s (re-applying the current clip stack afterward, since that call unconditionally resets the GPU scissor to the full swapchain extent), then draws its own already-baked composite-quad geometry using the just-registered bindless index rather than the IR's own `NO_TEXTURE` placeholder, and finally `deregister_bindless`/`release_transient_target`s. A new `PipelineKind::TexturedQuad` names the composite pipeline id -- the first time anything in `tre-engine` itself (not a hand-written demo) emits a bindless-textured draw. `LayerDesc` gained `x`/`y` (Step 6.1's original struct had no compositing position at all; `push_layer` had silently hardcoded it to the origin). Still single-level only, matching Step 6.4.1's own scope -- a nested `PushLayer` panics. See `planning/archive/PLAN_PHASE6_STEP6_4_2.md`.

*Updated for Phase 7 Step 7.2.1 (2026-09-08, REVIEW.md finding #148):* `RhiCommandBuffer` gains `begin_render_to_texture_no_end` -- identical to `begin_render_to_texture` except it never calls `cmd_end_rendering` first, paired with a plain `end_render_to_texture` call immediately before it. Fixes a real bug found while chaining consecutive render-to-texture passes (a Dual-Kawase downsample/upsample sequence): `end_render_to_texture(previous)` directly into `begin_render_to_texture(next)` calls `cmd_end_rendering` twice for one active rendering scope, a genuine Vulkan validation error. This method is real, tested-compiling, and used by `dual_kawase_blur_demo.rs`; it was previously omitted from this section's own trait-sketch annotations even though IMPLEMENTATION.md's Step 7.2.1 write-up documents it correctly. See REVIEW.md finding #130 for the separate bindless-sampling bug this step's own investigation was actually chasing when it found this fix along the way -- closed for real the same day (see the next annotation).

*Updated for the REVIEW.md finding #152 fix (2026-09-08):* `RhiCommandBuffer::begin_render_to_texture`/`begin_render_to_texture_no_end` both gain two new parameters, `logical_width: u32, logical_height: u32` -- the caller's own intended size (exactly what it already passed to `RhiDevice::acquire_transient_target`), used only to set the command buffer's own NDC-mapping state, never for viewport/scissor/render area (still driven by the texture's real physical size, unchanged). Fixes a real, shipped bug in Step 6.4.2's own `PushLayer`/`PopLayer` compositing: `acquire_transient_target`'s documented "oversized borrow" fallback can return a texture larger than requested, and using that real (rather than intended) size for NDC mapping silently confined a layer's own content to a small corner of the oversized image -- the same underlying mechanism finding #130 (above) turned out to have, reached this time through the production path. Every real call site updated: `execute_frame`'s `PushLayer` handling, `render_to_texture_demo.rs`, `dual_kawase_nonbindless_experiment.rs`, `dual_kawase_blur_demo.rs`. See REVIEW.md finding #152 for the full account, including why leaving viewport/scissor/render area untouched is still correct (the resulting "stretch" is exactly undone by a normalized-UV read later).

*Updated for Phase 7 Step 7.2.2 (2026-09-08):* `RhiCommandBuffer` gains `apply_layer_blur(&mut self, device: &dyn RhiDevice, source: &dyn RhiTexture, width: u32, height: u32) -> Box<dyn RhiTexture>` -- one purpose-built method (matching `begin_render_to_texture_no_end`'s own precedent: added narrowly for the one real need it serves, not a speculative primitive family) encapsulating `dual_kawase_blur_demo.rs`'s own proven non-bindless Dual-Kawase chain as real, reusable engine capability. `LayerDesc` gains `blur: bool`, threaded through the IR via `PopLayer`'s own otherwise-inert `texture_handle` field; `execute_frame`'s `PopLayer` handling calls `apply_layer_blur` and composites its returned texture instead of the raw one whenever set. `VulkanDevice` gains a lazily-initialized `blur_resources` cache (mirroring `transient_pool`'s own `Arc<Mutex<..>>` pattern) holding the blur-specific descriptor sets/pipelines/sampler/unit-quad buffers, created once on first real use; `VulkanCommandBuffer` holds its own `Arc` clone plus copied `instance`/`physical_device`/`stencil_format` fields, since `apply_layer_blur`'s own `device: &dyn RhiDevice` parameter is a trait object with no way back to `VulkanDevice`'s concrete fields. See REVIEW.md findings #152 (this step's own prerequisite fix) and #153 (a real vertex/index-buffer-restoration bug this step's own first GPU run caught) for the full account, and REVIEW.md finding #130's own updated write-up for a real, separate, still-unexplained bindless-sampling defect this step's own pre-work newly disclosed (sidestepped entirely by reusing the demo's already-proven non-bindless mechanism, not investigated further here).

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
    /// IMPLEMENTATION.md Step 3.3.3: every swapchain owns its own
    /// stencil image (sized to its own extent, since different
    /// swapchains -- e.g. `multi_window`'s two windows -- can differ).
    /// `stencil_view_handle`/`stencil_image_handle` mirror
    /// `AcquiredImage`'s view-vs-image split: the view for the
    /// `RenderingInfo` attachment, the image for the layout-transition
    /// barrier `RhiDevice::begin_frame` issues before rendering.
    fn stencil_view_handle(&self) -> u64;
    fn stencil_image_handle(&self) -> u64;
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
* **Stencil Test:** Disabled by default, same reasoning as depth. IMPLEMENTATION.md Step 3.3.3's stencil-and-cover fallback (for self-intersecting paths ear-clipping cannot triangulate) is the one deliberate exception -- its two pipelines (`create_stencil_and_cover_pipelines`) enable stencil test/write to encode a per-pixel winding count or even-odd parity, while depth test/write stay disabled exactly as above. Every pipeline, including the ordinary default ones described here, declares a stencil-compatible `PipelineRenderingCreateInfo` regardless of whether it enables the test -- the same "declared everywhere, unused by pipelines that don't reference it" precedent as the bindless descriptor set and push-constant range, needed because every swapchain now always has a stencil buffer attached.

---

## 7. UI Primitive Shape System (Phase 10 Steps 10.1-10.2)

**Status: Complete (2026-09-09), extended (2026-09-09, Step 10.2).**
Every struct/trait/enum below is real, shipped code in
`crates/tre-engine/src/shapes.rs`, matching this section's own text
exactly (no drift found during implementation). Step 10.1 shipped the
data model plus real rendering for one narrow case (uniform-radius,
borderless `Rectangle`); Step 10.2 made real rendering support MUCH
broader -- `Rectangle` (any corner radii, real borders, corner
smoothing) and `Circle`/`Ellipse` (borders, partial-arc sweep) both
render for real now, `Polygon`/`Star` fill for real (no border/stroke
yet), and `Path`'s Bezier-flattening math is real and tested even though
no `Path` rendering path (fill or stroke) exists yet. See Section 7.5's
own "Implementation status" note for the current, itemized disposition
of every field. The per-step plans
(`planning/archive/PLAN_PHASE10_STEP10_1.md`,
`planning/archive/PLAN_PHASE10_STEP10_2.md`) are the authoritative task
breakdowns; this section remains the canonical struct/trait/enum
reference every other document points to, matching this document's own
established "define once here, reference elsewhere" convention (Sections
3.1, 4.1).

**Why a retained-mode layer at all.** Every existing drawing entry point
(`RenderingCanvas::draw_rounded_rect`, `draw_text`, `draw_path`) is
immediate-mode: the caller re-issues every draw call every frame, and
`RenderingCanvas` itself is rebuilt from scratch each frame (Phase 9
Step 9.2's own `reset()`-based reuse only avoids reallocating that
per-frame `Vec` storage, it does not let a caller skip re-recording
unchanged content). An external UI framework driving this engine --
Python via Phase 10 Step 10.4's direct PyO3 binding, or any other
language via Step 10.3's `tre-ffi` C-ABI (DESIGN.md Section 2.7's own
"Two Real Paths") -- wants the opposite shape of API: create a shape
once, hold a stable handle to it, mutate a handful of
properties as the UI framework's own layout/animation system runs, and
let the engine decide what actually needs re-recording this frame. This
section's `ShapeRegistry` is that layer -- a retained store of shape
*descriptions*, each translated into the exact same immediate-mode
`RenderingCanvas` calls (and therefore the exact same IR/radix-sort/
batch/RHI pipeline, Phases 5 through 9) every other caller already uses,
whenever it is dirty. It is a convenience layer over the existing
pipeline, never a second one.

### 7.1 Shared Layout Properties

Every concrete shape embeds one `PrimitiveCommon` by composition, not
inheritance -- Rust has no struct inheritance, and this project's own
established style (`CanvasState`, `LayerDesc`) already prefers a shared
data struct embedded by value over a deep trait hierarchy:

```rust
/// Fields every shape primitive carries, regardless of kind -- embedded
/// by value in `Rectangle`/`Circle`/`Polygon`/`Path` (Section 7.3), not
/// inherited. `Primitive::common`/`common_mut` (below) give generic code
/// (the per-frame flattening pass, hit-testing) uniform access without
/// matching every concrete shape variant.
#[derive(Debug, Clone, Copy)]
pub struct PrimitiveCommon {
    pub transform: Transform2D,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub visibility: Visibility,
    pub hit_testable: bool,
}

/// A decomposed, animation-friendly transform -- deliberately *not* the
/// existing `tre_math::Affine2` (Section 3.1's compact 6-float matrix).
/// `Affine2` is the pipeline's own canonical, composable representation,
/// but a raw matrix's rotation component cannot be cleanly interpolated
/// (a lerp between two matrices is not a lerp between two rotations);
/// `position`/`scale`/`rotation` can each be animated independently and
/// combined into a real `Affine2` once per frame during flattening
/// (`to_affine2`, Section 7.4) -- `Affine2::from_translation(position)
/// .compose(&Affine2::from_rotation(rotation).compose(&Affine2::
/// from_scale(scale[0], scale[1])))`, standard translate*rotate*scale
/// order, using `Affine2::compose`'s own existing method (Section 3.1),
/// not a new matrix implementation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub position: Vec2,
    pub scale: Vec2,
    /// Radians, matching `Affine2::from_rotation`'s own convention
    /// (Section 3.1) -- not degrees.
    pub rotation: f32,
}

/// A plain 2D point/extent -- a *type alias* for the `[f32; 2]` this
/// codebase already uses everywhere a 2D point appears
/// (`UiVertex::position`, `tre_math::lerp_points_batch`'s own
/// signature), not a new nominal struct. Deliberate: introducing a
/// distinct `Vec2` type would need its own `Add`/`Sub`/`Mul` impls and
/// would stop being interchangeable with every existing `[f32; 2]` call
/// site for no real benefit -- this section only reaches for a genuinely
/// new nominal type where the existing convention has no equivalent
/// (`CornerRadii`, Section 7.2).
pub type Vec2 = [f32; 2];

/// TECHNICAL.md Section 3.4/DESIGN.md Section 6.2's own long-named
/// "Visual Filter Pipeline" concept, concretized here as the actual
/// enum a shape's `blend_mode` field holds -- `LayerDesc`'s own doc
/// comment (Section 5) already named blend mode as belonging to "a
/// later phase [that] implements those visual filters"; this is that
/// phase's own data-model piece. **No rendering support exists for any
/// non-`Normal` variant yet** -- see this section's own closing
/// "Implementation status" note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    ColorDodge,
}

/// Layout-participation state, distinct from `opacity == 0.0` (which
/// still occupies layout space and still hit-tests) -- `Collapsed`
/// mirrors the common "display: none" UI-framework concept: no layout
/// space, no hit-testing, no recording into the IR at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapsed,
}

/// Every concrete shape (Section 7.3) implements this so generic code
/// can reach `PrimitiveCommon` without a `match` over every variant --
/// an ordinary object-safe accessor trait, not a marker for dynamic
/// dispatch: every real call site uses the `ShapePrimitive` enum
/// (Section 7.3), never `dyn Primitive`, per TECHNICAL.md Section 9.1's
/// "no dynamic type inspection in hot paths" rule.
pub trait Primitive {
    fn common(&self) -> &PrimitiveCommon;
    fn common_mut(&mut self) -> &mut PrimitiveCommon;
}
```

### 7.2 Fill, Stroke & Color

```rust
/// `UiVertex::color`'s own existing packed-`u32` convention (Section
/// 3.1, `rgba8`'s own doc comment) -- a type alias, not a new struct,
/// for the same reason `Vec2` is one: every existing color call site
/// already speaks this exact representation.
pub type Color = u32;

/// What a shape's interior is painted with. `Gradient` and `Texture`
/// reference subsystems at two different real-vs-planned points:
/// `Texture` is real today (the existing bindless texture-handle system,
/// Phase 2 Step 2.1/Phase 4 Step 4.2.4) -- a shape's fill can bind an
/// already-uploaded bindless texture index immediately. `Gradient` has
/// **no evaluator anywhere in this codebase** -- DESIGN.md's own
/// architecture diagram names a "Dynamic Gradient & Pattern Fill
/// Evaluator" as a future box, never built; `GradientId` is a real,
/// stable handle *type* a shape can reference now, satisfying the data
/// model, but resolving one to actual pixels is separate, disclosed,
/// not-yet-scheduled future work.
#[derive(Debug, Clone, Copy)]
pub enum FillStyle {
    Solid(Color),
    Gradient(GradientId),
    Texture(u32),
}

/// Opaque handle into the not-yet-built gradient evaluator's own future
/// table -- exists so `FillStyle::Gradient` is a real, stable type today
/// rather than a placeholder that would need a breaking change once the
/// evaluator lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientId(pub u32);

/// `Rectangle::corner_radius`'s own per-corner field type -- clockwise
/// from top-left, matching the field order every CSS-derived UI
/// framework already expects. A genuinely new nominal type (not a
/// `Vec2`-style alias): unlike a generic 2D point/extent, "four corner
/// radii in a fixed clockwise order" is a real, distinct concept with
/// its own indexing convention worth naming.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    /// The common case -- one radius applied to all four corners,
    /// exactly what today's real `draw_rounded_rect` already supports
    /// (Phase 3 Step 3.2). A `Rectangle` using only this constructor
    /// needs no new rendering work at all to flatten correctly.
    #[must_use]
    pub const fn uniform(radius: f32) -> Self {
        Self { top_left: radius, top_right: radius, bottom_right: radius, bottom_left: radius }
    }
}
```

### 7.3 The Concrete Shapes

```rust
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub common: PrimitiveCommon,
    pub size: Vec2,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    pub corner_radius: CornerRadii,
    /// Squircle interpolation factor, `0.0` (pure circular-arc rounding,
    /// today's real shader) to `1.0` (full squircle) -- see this
    /// section's closing "Implementation status" note.
    pub corner_smoothing: f32,
}

/// Unifies circle and ellipse: `radius[0] == radius[1]` is a circle,
/// otherwise an ellipse -- one struct, no separate `Circle`/`Ellipse`
/// types, matching how a UI framework caller almost always wants "the
/// same shape, sometimes with unequal axes," not two APIs to learn.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub common: PrimitiveCommon,
    pub radius: Vec2,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    /// Degrees, `0.0..=360.0` -- a progress-wheel/pie-chart partial
    /// sweep starting at 12 o'clock, clockwise. `360.0` (the default)
    /// is a full circle/ellipse.
    pub arc_length: f32,
}

/// Covers triangle/hexagon/N-gon and star shapes with one struct: a
/// regular `sides`-gon when `star_points` is `None`, an alternating
/// inner/outer-radius star when it is `Some`.
#[derive(Debug, Clone, Copy)]
pub struct Polygon {
    pub common: PrimitiveCommon,
    pub sides: u32,
    pub radius: f32,
    /// Uniform rounding applied to every vertex -- distinct from
    /// `Rectangle::corner_radius`'s per-corner `CornerRadii`, since a
    /// regular polygon's own symmetry makes a single value both
    /// sufficient and simpler to reason about.
    pub vertex_radius: f32,
    pub star_points: Option<u32>,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
}

/// One instruction in a `Path`'s command list -- deliberately a small,
/// closed set matching SVG path-data's own real primitives (the same
/// vocabulary `tre-svg`'s existing parser already consumes, Phase 3 Step
/// 3.3.1), not a speculative superset.
#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    MoveTo(Vec2),
    LineTo(Vec2),
    QuadraticTo { control: Vec2, to: Vec2 },
    CubicTo { control1: Vec2, control2: Vec2, to: Vec2 },
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// The one shape whose own data model already names a real, deliberate
/// heap allocation (`commands: Vec<PathCommand>`) -- consistent with
/// DESIGN.md Section 2.1's own zero-allocation *boundary*, which
/// exempts "complex external subsystems" (its own named example:
/// SVG/path data) from the per-frame steady-state rule as long as they
/// use arena-bounded allocation outside the active render tick.
/// `commands` is recorded once (or replaced wholesale on a real edit,
/// not appended to every frame), the same shape `tre-svg`'s existing
/// parsed-path storage already has.
#[derive(Debug, Clone)]
pub struct Path {
    pub common: PrimitiveCommon,
    pub commands: Vec<PathCommand>,
    pub fill: FillStyle,
    pub border_color: Color,
    pub border_thickness: f32,
    pub stroke_line_cap: LineCap,
    pub stroke_line_join: LineJoin,
}

/// The one type every shape's own real, concrete storage and every
/// per-frame flattening call site actually holds -- `enum` dispatch,
/// not `Box<dyn Primitive>`: matching a fixed, closed set of variants
/// costs one branch and no heap allocation or vtable indirection, the
/// same reasoning `CommandType`/`UiDrawCommand` (Section 3.2) already
/// establishes for the IR itself, and required by TECHNICAL.md Section
/// 9.1's "no dynamic type inspection in hot paths" rule for anything
/// touched during a render tick.
#[derive(Debug, Clone)]
pub enum ShapePrimitive {
    Rectangle(Rectangle),
    Circle(Circle),
    Polygon(Polygon),
    Path(Path),
}
```

### 7.4 Retained State & the Shape Registry

```rust
/// A stable-until-removed handle into `ShapeRegistry` -- the type an
/// external UI framework actually holds across many frames: wrapped as
/// a `tre-ffi` opaque handle for non-Python languages (Phase 10 Step
/// 10.2), or held directly inside a PyO3 `#[pyclass]` for Python (Step
/// 10.3, DESIGN.md Section 2.7's "Two Real Paths") -- never the raw
/// struct exposed to either. `generation` is what makes a stale handle
/// (one whose slot was removed and reused) detectable rather than
/// silently resolving to a different, unrelated shape -- the same real
/// hazard `tre_memory::SwmrSlotTable`'s own per-slot generation counter
/// exists to prevent for atlas glyph slots (Step 4.3.1), applied here
/// to a different concrete storage shape (see this subsection's own
/// closing note on why `SwmrSlotTable` itself isn't reused directly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId {
    index: u32,
    generation: u32,
}

/// One caller-assigned tween/animation this project's own future
/// animation-timeline system (DESIGN.md Section 12.3's spring/lerp-decay
/// math, Phase 8 Step 8.1.1's real `spring_decay` primitive) is actively
/// driving against a shape's property -- opaque here, since owning and
/// stepping the animation itself is that system's job, not the shape
/// registry's; a non-empty `active_animations` on a `ShapeSlot` is only
/// ever a *signal* ("this shape needs re-flattening this frame even
/// though nothing external marked it `layout_dirty`"), matching
/// `tre_math`'s own established "stateless evaluation library, the
/// caller owns state" boundary (DESIGN.md Section 12.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(pub u64);

/// One `ShapeRegistry` slot -- the shape itself plus the UI-framework-
/// facing state hooks that decide whether this frame's flattening pass
/// needs to touch it at all.
#[derive(Debug, Clone)]
pub struct ShapeSlot {
    pub shape: ShapePrimitive,
    pub active_animations: Vec<AnimationId>,
    pub layout_dirty: bool,
    /// A shape-local clip rect, in the same coordinate space
    /// `Canvas::push_clip`'s existing `ScissorRect` (Section 3.2) uses
    /// -- reuses that exact real type rather than a new `Rect`, matching
    /// this whole section's own "alias/reuse an existing type unless a
    /// genuinely new concept requires one" discipline.
    pub clip_bounds: Option<ScissorRect>,
}

/// The retained-mode shape store -- a hand-built generational slot
/// arena, not `Box<dyn Primitive>`s in a `Vec` (dynamic dispatch,
/// TECHNICAL.md Section 9.1) and not the pre-existing `tre_memory::
/// ScatterArena`/`SwmrSlotTable` (both real but the wrong shape for this
/// job: `ScatterArena` is single-write-then-consumed-once per frame,
/// `SwmrSlotTable`'s own value slot is a bare `u64`, not an owned,
/// in-place-mutable `ShapeSlot`). A new, small, purpose-built primitive
/// -- matching this project's own established "hand-build the
/// concurrency/memory primitive rather than reach for a crate"
/// precedent (`ScatterArena`, `SwmrSlotTable`, `MpscRingBuffer`,
/// `SpscRingBuffer`), not a dependency on the `slotmap` crate or
/// similar.
pub struct ShapeRegistry {
    slots: Vec<Option<ShapeSlot>>,
    generations: Vec<u32>,
    free_list: Vec<u32>,
    /// Added during implementation (2026-09-09), not part of this
    /// section's original planning sketch -- an `O(1)` live-shape count
    /// for `len()`/`is_empty()` (`clippy::pedantic`'s own
    /// `len_without_is_empty` convention this crate already follows
    /// elsewhere), maintained incrementally on `insert`/`remove` rather
    /// than recomputed by scanning `slots` on every call. The only real
    /// difference found between this section's own planned shape and
    /// the shipped one.
    live_count: usize,
}
```

### 7.5 Efficiency & FFI Design

* **Neither real cross-language path sees raw struct layout, by two
  different mechanisms (revised 2026-09-09, renumbered 2026-09-09 when
  Step 10.2 was inserted for shape rendering).** Every field above stays
  ordinary (non-`#[repr(C)]`) Rust, exactly like `RenderingCanvas`
  itself today. Non-Python languages reach `ShapeId` through Phase 10
  Step 10.3's `tre-ffi` crate, exactly as originally designed: an
  opaque handle plus `extern "C"` getter/setter functions per field
  (the same pattern already used for `RhiTexture`/`RhiCommandBuffer`/
  `AcquiredImage`, Section 6), never a transmuted pointer into a
  `Rectangle`. **Python, per DESIGN.md Section 2.7's "Two Real Paths"
  correction, does not go through `tre-ffi` at all** -- Phase 10 Step
  10.4's `tre-python` crate wraps `ShapeId`/`Rectangle`/`Circle`/
  `Polygon`/`Path` directly as PyO3 `#[pyclass]` types, with
  `#[pymethods]` as the real field-access boundary instead of `extern
  "C"` getters/setters; PyO3 itself, not this project's own code,
  guarantees the Python interpreter never sees or transmutes the raw
  Rust layout. Either way, `Vec<PathCommand>`, `Option<ScissorRect>`,
  and enum-with-data fields (`FillStyle`, `ShapePrimitive` itself)
  exist exactly as written above -- none of those are FFI-safe by
  value under `tre-ffi`'s own C-ABI rules, and none need to be, since
  neither real path ever crosses either boundary by raw value.
* **Zero allocation in the hot path stays real by construction, not yet
  verified live under the real guard.** Creating or removing a shape
  touches `ShapeRegistry`'s own `free_list` (amortized `O(1)`, no
  allocation once the registry has grown to its steady-state slot count
  -- the same "grow once, reuse after" discipline Phase 9 Step 9.2
  already established for `FrameArena`'s own scratch buffers).
  *Mutating* an existing shape's fields is a plain in-place write. Only
  the per-frame *flattening* pass (`ShapeRegistry::flatten_into`)
  touches `RenderingCanvas`, and it does so through the exact same
  already-zero-allocation-verified `reset()`/`draw_*`/`flatten_into`
  path Step 9.2 built and proved with a real, self-checking
  `RenderTickGuard` (TECHNICAL.md Section 3.4). This section adds no new
  steady-state allocation source of its own by construction -- but,
  disclosed honestly: `RenderTickGuard` is only wired into
  `main_loop_demo.rs` today, not into `shape_registry_demo.rs` or any
  other automated check, so this claim is architecturally sound but not
  yet *proven* the same way `main_loop_demo`'s own zero-allocation claim
  is. Wiring a shape-registry-driven scene into `main_loop_demo` (or an
  equivalent guarded demo) to close that gap is real, separate future
  work, not done as part of this step.
* **Implementation status, itemized against real rendering support
  (revised 2026-09-09, Step 10.2):**
  - **`Rectangle`: fully real.** Any `corner_radius` (uniform or not),
    a real border (`border_color`/`border_thickness`), and
    `corner_smoothing` (a real, if approximate, superellipse blend --
    see the Shape Style Buffer subsection below) all render correctly,
    via a new `sdf_rect_styled.frag` pipeline
    (`RenderingCanvas::draw_styled_rectangle`) alongside the original
    uniform-radius `draw_rounded_rect` path (kept, unmodified, as the
    cheaper common case).
  - **`Circle`/`Ellipse`: fully real.** A new `sdf_ellipse.frag`
    pipeline (`RenderingCanvas::draw_ellipse`) renders the exact circle
    case exactly and the non-uniform-radius ellipse case via a real,
    disclosed *approximate* SDF (exact only when `radius.x ==
    radius.y`); border and `arc_length` (a hard-edged angular sector
    cutoff, no rounded stroke caps at the cut) are both real.
  - **`Polygon`/`Star`: fill is real**, via procedural boundary-point
    generation (`generate_polygon_points`) and a from-center triangle
    fan (`fan_from_center`) -- valid because a regular/star polygon
    generated this way is star-shaped with respect to its own center by
    construction. **Border/stroke rendering is not built** (no stroke
    tessellator exists yet).
  - **`Path`: Bezier-flattening is real and tested**
    (`shapes::flatten_path`, reusing the exact tolerance-based recursive
    de Casteljau algorithm `tre_svg::flatten_cubic`/`flatten_quad`
    already use for SVG curves -- duplicated, not shared, since
    `tre-svg` already depends on `tre-engine`, so the reverse dependency
    this would need is circular; see REVIEW.md's own Phase 10 Step 10.2
    finding). **No `Path` rendering path exists at all** -- fill needs a
    general (non-star-shaped) triangulator this crate cannot reach for
    the same circular-dependency reason, and stroke needs a tessellator
    not yet built. `flatten_path`'s real output is used today by
    `ShapeRegistry::hit_test`'s own `Path` case, which needs no GPU
    rendering plumbing.
  - **`FillStyle::Gradient`/`Texture`: still not built, for any shape
    kind.** **Non-`Normal` `BlendMode`: still not built** -- the
    Vulkan fixed-function blend state a subset could use, and the
    framebuffer-read capability the rest would need, are both real,
    separate, not-yet-scheduled RHI work.
  - **Hit-testing is real for all four shape kinds**
    (`ShapeRegistry::hit_test`) -- the actual concrete need
    `hit_testable` (present since Step 10.1, read by nothing until now)
    exists for. Exact point-in-shape tests: the same non-uniform
    rounded-box SDF `sdf_rect_styled.frag` evaluates, on the CPU, for
    `Rectangle`; an exact closed-form ellipse-membership test (not the
    GPU's own SDF approximation) plus the same arc-sector convention for
    `Circle`; the standard even-odd ray-casting algorithm (PNPOLY) for
    `Polygon` and (XORed across subpaths) `Path`.

  See IMPLEMENTATION.md Step 10.2's own "Explicitly out of scope" list
  for the authoritative version of this same disclosure.
* **The Shape Style Buffer (new, Step 10.2).** `UiVertex`'s hard
  32-byte layout (`params: [f32; 3]`, Section 3.1) has no room for
  non-uniform corner radii, a border color/thickness, or corner
  smoothing -- `draw_rounded_rect`'s own doc comment already says this
  out loud ("uniform across all 4 vertices since the vertex format has
  no per-quad channel"). Rather than grow `UiVertex` itself (which would
  bloat every pipeline in the system -- text, flat tessellated fills,
  blur -- that doesn't need any of this data), Step 10.2 reuses the
  bindless pattern Step 2.1 already established for textures: a new
  binding-1 `STORAGE_BUFFER` on the same bindless descriptor set (the
  texture array itself moved from binding 1 to binding 2 to keep the
  spec-required "`VARIABLE_DESCRIPTOR_COUNT` binding must be the
  highest-numbered one" invariant), holding per-shape `GpuRectStyle`/
  `GpuEllipseStyle` records (`crates/tre-engine/src/gpu_style.rs`) that
  a single word-index -- carried NUMERICALLY (not bit-cast, see below)
  in one existing `UiVertex.params` float slot -- references. The
  underlying buffer is a `VulkanRingBuffer` (the same triple-buffered,
  persistent-mapped type the vertex/index ring buffer already uses),
  bump-allocated into fresh each frame via the same
  `RhiDynamicRingBuffer::write` contract, exposed through a new
  `RhiDevice::shape_style_buffer()` method. This is real, general
  infrastructure -- any future per-shape data (a future gradient
  evaluator's stop tables, say) has a ready home in a new record type
  using the same mechanism, without another `UiVertex` conversation.
  **A real GPU bug found and fixed while building this (REVIEW.md's own
  finding):** the word index was originally bit-cast (`f32::from_bits`)
  into the float slot -- for small indices this produces a *subnormal*
  float, and real GPU hardware was observed to silently flush it to
  `0.0` (denormal flush-to-zero, common ALU/interpolation behavior),
  so the shader read the wrong style record entirely. Fixed by carrying
  the index as a plain numeric value (`as f32`/`uint(...)`) instead --
  every real word index is a small integer, exactly representable as a
  normal `f32`, with no denormal ever in play.

*Future consideration -- opaque pre-pass (not implemented; profile before building):* Depth-test-off means the GPU gets no early-Z rejection, so overdraw-heavy scenes (e.g. a dense data grid with thousands of large, fully-opaque cell backgrounds) pay full fragment cost for content later fragments completely cover. A front-to-back, depth-tested pre-pass restricted to batches provably fully opaque (nothing SDF-antialiased or alpha-sampled can participate) could reclaim that cost via early-Z, at the price of a second pipeline state, a second command-buffer pass, and careful ordering against the existing Depth-ID-driven painter's-algorithm pass so the two agree on what's already covered. Do not build this speculatively: profile a representative overdraw-heavy scene first and confirm GPU time -- not CPU submission time -- is the actual bottleneck before spending the complexity budget here.
