# Comprehensive Engineering Implementation Plan

This document outlines the phased execution strategy for constructing the High-Performance UI Rendering Engine. It synthesizes the system architecture, technical constraints, and design philosophy into actionable implementation steps, providing deep technical context for each subsystem.

## Phase 0: Walking Skeleton (Added in September 2026 Documentation Review)

Phases 1 through 5 build platform abstraction, RHI backends, memory pools, geometry, typography, and multi-threaded recording -- five phases of pure plumbing with no visible pixel output and no end-to-end validation of the architecture's shape. That is a long integration-risk window: interface mismatches between `Canvas`, the IR, and the RHI are cheapest to catch before typography and SVG tessellation are built on top of assumptions that turn out wrong.

* **Implementation Tasks:**

  1. Stand up a single-backend (Vulkan only), single-threaded, minimal `RhiDevice`/`RhiSwapchain` pair -- enough to open one window and clear it to a color.

  2. Implement a stub `RenderingCanvas::draw_rounded_rect` that records exactly one `UiDrawCommand` into a fixed-size array (no ring buffer, no arena, no multi-threading yet).

  3. Implement a trivial pass-through of the sort/flatten stage for the single-command case (no real radix sort needed yet -- one element sorts itself).

  4. Wire that one command through `RhiCommandBuffer::draw_indexed` to the swapchain and present it.

  5. Confirm the full loop -- `Canvas` call in, pixel out -- runs and holds a stable frame time before starting Phase 1's deeper investment.

* **Technical Rationale:** A thin vertical slice validates the shape of the Canvas to IR to RHI contract end-to-end while it is still cheap to change. Every subsequent phase adds depth to a pipeline already proven to connect correctly, rather than five phases of isolated subsystem work converging for the first time in Phase 6.

### Status: Complete (2026-09-04)

Implemented in `crates/tre-engine` (the Phase 0 `Canvas`/IR types) and `crates/tre-rhi-vulkan` (the Vulkan backend, via `ash`), with a runnable proof at `crates/tre-rhi-vulkan/examples/walking_skeleton.rs`. Windowing uses `winit` -- a Phase-0-only expedient; Phase 1's Step 1.1 replaces it with the documented native per-platform bridges. Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` all clean across the workspace, 120 frames presented with zero Vulkan validation-layer errors (`VK_LAYER_KHRONOS_validation`), and a screenshot confirming the rendered rect's color and position match what `Canvas::draw_rounded_rect` was called with.

This walking skeleton did exactly what Phase 0's own rationale says it should: it surfaced real interface gaps and real bugs before Phase 1-5 could build on top of them. Recorded in full in REVIEW.md's "Phase 0 Implementation" entry; summary:

* **ARCHITECTURE.md Section 6 left `RhiBuffer`, `RhiTexture`, `RhiPipelineState`, and `RhiSwapchain` referenced (as `&dyn Rhi*`) but never defined.** Defined now in `tre-engine`, using an opaque-`u64`-handle pattern (mirroring how Vulkan itself represents every object) specifically so `RhiDevice`/`RhiCommandBuffer` implementations never need `std::any::Any` downcasting to recover their own concrete state from a trait object -- which TECHNICAL.md Section 9.1 bans from the per-frame path.
* **`RhiDevice::begin_frame`/`submit_and_present` had no `Result` return type in ARCHITECTURE.md's sketch**, contradicting DESIGN.md Section 2.6's explicit requirement that device-loss/swapchain-out-of-date conditions be "surfaced as a recoverable error" at exactly those calls. Both now return `Result<_, EngineError>`.
* **A `u32` RGBA color hex literal does not pack the way it visually reads.** `0xE0_A0_40_FFu32` stored little-endian places `0xFF` at the lowest memory address, not `0xE0` -- the reverse of what an `R8G8B8A8` vertex attribute expects. Added `tre_engine::rgba8(r, g, b, a)` so no caller has to reason about this by hand; a screenshot during implementation caught the resulting pink-instead-of-amber rectangle.
* **Three real Vulkan lifecycle bugs**, each caught by running with `VK_LAYER_KHRONOS_validation` enabled or by a SIGSEGV backtrace, not by inspection: freeing a command buffer immediately after submitting it (still pending); reusing one `render_finished` semaphore across frames while the swapchain's present operation -- which the engine's fence never tracks -- might still reference it (fixed with one semaphore per swapchain image); and Rust's struct-field drop order (declaration order, not reverse) destroying a window's surface before the swapchain built on it, and destroying a device before the buffers/pipeline built on it.

## Phase 1: Platform, Windowing, & Input Abstraction

### Step 1.1: Multi-Window & Headless OS Layer

* **Implementation Tasks:**

  1. Construct the `RhiDevice` singleton to manage global shared resources (PSO caches, dynamic atlases, font engines).

  2. Implement native OS surface bridges:

     * **Windows:** Register window classes, handle `WM_NCCALCSIZE` for custom title bars, and wire `HWND` to [DXGI](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/d3d10-graphics-programming-guide-dxgi) -- via the `windows` crate's `Win32` bindings (published under that name on crates.io; "windows-rs" is the name of the upstream GitHub project).

     * **Linux:** Implement `xdg_wm_base` for [Wayland](https://wayland.freedesktop.org/) and fallback to XCB for X11 -- via the `wayland-client`/`wayland-protocols` crates, with `x11rb` for the XCB fallback.

     * **macOS:** Initialize `NSApplication`, configure `CAMetalLayer` with `displaySyncEnabled` to match monitor refresh rates -- via the `objc2`/`objc2-app-kit`/`objc2-metal` crate bindings.

  3. Build the `RhiSwapchain` module to manage per-window surface lifecycle. Map OS-specific DPI events (e.g., `WM_DPICHANGED`) to trigger instantaneous swapchain resizing and global UI scale factor adjustments.

  4. Implement a Headless Mode utilizing virtual framebuffers. Map GPU memory to CPU staging buffers using transfer flags (e.g., `VK_IMAGE_USAGE_TRANSFER_SRC_BIT`), enabling automated CI/CD and server-side visual regression testing.

* **Technical Rationale:** A shared `RhiDevice` with per-window swapchains prevents VRAM fragmentation and resource duplication (like fonts and icons) across multi-window desktop applications.

### Step 1.2: Decoupled Event & Signal Pipeline

* **Implementation Tasks:**

  1. Implement a Single-Producer Multi-Consumer (SPMC) lock-free ring buffer for capturing OS window events.

  2. Translate platform-specific input (e.g., `WM_POINTERDOWN`, `NSEventTypeLeftMouseDown`) to agnostic engine structures (e.g., `InputEvent::PointerDown`).

  3. Implement event payload coalescing. For instance, if multiple high-frequency mouse move events occur between frames, squash them into a single `PointerMove` event to save layout evaluation time.

  4. Ensure the event pump executes entirely outside the graphics pipeline timeline, exposing a polling/drain interface to the UI framework.

* **Technical Rationale:** Graphics execution must never block on hit-testing or OS input hooks. Decoupling guarantees the $0.50\text{ ms}$ CPU frame submission budget is isolated from layout and logic stalls.

## Phase 2: Core Hardware Abstraction (RHI) & Memory Management

### Step 2.1: Modern Graphics API Backends

* **Implementation Tasks:**

  1. **Vulkan 1.2:** Implement backend utilizing [`VK_KHR_dynamic_rendering`](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_KHR_dynamic_rendering.html) (eliminating `VkRenderPass` and `VkFramebuffer` overhead). Define a universal pipeline layout that exposes an unbounded array of textures `texture2D textures[]` via [`VK_EXT_descriptor_indexing`](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_EXT_descriptor_indexing.html). Implemented in the `tre-rhi-vulkan` crate via the `ash` raw-bindings crate.

  2. **DirectX 12:** Implement backend targeting Feature Level 12_0. Construct a Root Signature that passes vertex data via Root Constants or Root SRVs, utilizing [Resource Binding Tier 3](https://learn.microsoft.com/en-us/windows/win32/direct3d12/hardware-support) for bindless descriptor tables. Implemented in the `tre-rhi-dx12` crate via the `windows` crate's `Win32::Graphics::Direct3D12` bindings.

  3. **Metal:** Implement backend utilizing [Argument Buffers Tier 2](https://developer.apple.com/documentation/metal/buffers/about_argument_buffers), enabling dynamic arrays of texture resources directly in the shader. Implemented in the `tre-rhi-metal` crate via the `objc2-metal` crate.

* **Technical Rationale:** Leveraging dynamic rendering and bindless arrays eliminates pipeline permutation explosion and state-switch overhead, which is critical for UI rendering where widgets constantly alternate between textures, vectors, and text.

### Step 2.2: Zero-Allocation Ring Buffers & Transient Pools

* **Implementation Tasks:**

  1. Construct a triple-buffered `DynamicRingBuffer` ($16\text{ MB} - 32\text{ MB}$) using host-coherent, write-combined mapped memory (`VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT`).

  2. Implement CPU-side hardware fence waits (e.g., `vkWaitForFences`) before writing to frame $N$, ensuring the GPU has completely finished reading the segment from $N-3$.

  3. Enforce strict alignment: $64\text{ bytes}$ for CPU thread boundaries (prevent false sharing) and $256\text{ bytes}$ minimum alignment for RHI dynamic offsets.

  4. Build a `FxHashMap`-backed (or `ahash`) Transient Render Pool for offscreen textures keyed by `(Width, Height, Format)`, with width/height rounded up to fixed bucket boundaries (TECHNICAL.md Section 3.2) so nearby requests share an entry -- these are internal engine keys with no untrusted input, so `std::collections::HashMap`'s default SipHash buys nothing but cycles on this per-`push_layer` hot path. Hook this into `Canvas::push_layer` for immediate zero-allocation acquisition, falling back to the next-larger pooled entry on a genuine miss (DESIGN.md Section 2.6).

  5. **Debug-mode balance assertion (added in the September 2026 documentation review):** track `PushLayer`/`PopLayer` calls as a depth counter per `Canvas`; assert the counter is exactly zero at frame boundary. An unbalanced push (a widget that acquires a transient target and never releases it) otherwise starves the pool silently over many frames rather than failing loudly at the point of the actual bug.

* **Technical Rationale:** Writing directly to mapped memory prevents dynamic staging allocations. The transient pool ensures complex multi-pass widget effects (like glassmorphism) require $0\text{ bytes}$ of dynamic allocation during the active frame tick. The balance assertion turns a slow VRAM leak into an immediate, attributable debug-build failure.

### Step 2.3: Generational Garbage Collection (GC)

* **Implementation Tasks:**

  1. Embed a `u64 last_frame_used` timestamp into the metadata of all dynamic VRAM resources (atlas regions, tessellated SVG caches).

  2. Implement an asynchronous GC thread that scans resource pools when VRAM capacity hits $85\%$.

  3. Identify resources older than $N = 600$ frames. Remove their CPU-side handles and move their GPU handles to a deferred release lock-free queue.

  4. At the end of every frame, check the deferred release queue. Physically destroy hardware resources only if $N_{\text{current}} - N_{\text{evicted}} > 3$ frames.

* **Technical Rationale:** Prevents dynamic VRAM from ballooning past the $128\text{ MB}$ budget while ensuring that resources currently being executed by the GPU are never prematurely destroyed.

### Step 2.4: GPU API Validation in Debug & CI Builds

* **Implementation Tasks:**

  1. **Vulkan:** Enable `VK_LAYER_KHRONOS_validation` (via the enabled-layer list passed to instance creation) in debug and CI builds only, with a `VK_EXT_debug_utils` messenger callback routing validation messages into the engine's own logging and failing the CI job on any `VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT` message.

  2. **DirectX 12:** Call `ID3D12Debug::EnableDebugLayer()` before device creation in debug/CI builds. Gate the much heavier `ID3D12Debug1::SetEnableGPUBasedValidation` behind an explicit opt-in (env var or Cargo feature) rather than always-on, since GPU-based validation materially slows frame time and would corrupt the CI's own performance-regression numbers (Section 9.2) if left on unconditionally.

  3. **Metal:** Set `MTL_DEBUG_LAYER=1` (and `MTL_SHADER_VALIDATION=1`) on the CI test process for macOS runners, routing validation output into the same CI-failing log check as the other two backends.

  4. Gate all three behind the same debug/profile `cfg` used by the zero-allocation guard (TECHNICAL.md Section 3.4), so none of this exists in a shipped release binary.

* **Technical Rationale:** The CPU-side gates already in place (zero-allocation guard, `clippy`, the batching-equivalence pixel-diff test) validate everything on the Rust side of the `RhiDevice`/`RhiCommandBuffer` trait boundary, but the RHI backend crates are the one place `unsafe` FFI into the raw graphics APIs happens (TECHNICAL.md Section 9.1) -- exactly the code Rust's own type system cannot check. Native validation layers are the vendor-provided tool for catching resource-state, barrier, and synchronization misuse at that boundary, at zero cost in the shipped binary.

## Phase 3: Geometry Pipeline & Vector Math Engine

### Step 3.1: Compact UI Vertex & Matrix Math

* **Implementation Tasks:**

  1. Implement the `UiVertex` format exactly as defined in ARCHITECTURE.md Section 3.1 (the canonical 32-byte layout, added in the September 2026 documentation review) -- do not redeclare the field layout here.

  2. Implement SIMD-accelerated $3 \times 3$ affine transformation matrices using the [`wide`](https://docs.rs/wide) crate's `f32x8` vector type and its `mul_add` method (hardware FMA where the target has it, a separate multiply+add otherwise) to batch-multiply local node transforms down the UI scene graph tree -- no raw `core::arch` intrinsics or `unsafe` needed for this, since `wide`'s public API is safe and portable across the AVX2 (x86_64) and NEON (ARM64) targets (TECHNICAL.md Section 2.2).

  3. Add a compile-time assertion (`const _: () = assert!(std::mem::size_of::<UiVertex>() == 32);`, per ARCHITECTURE.md Section 3.1) validating the struct layout across all target triples.

* **Technical Rationale:** Capping the vertex struct at $32\text{ bytes}$ minimizes PCIe bus transfer times and maximizes GPU L2 cache coherency.

### Step 3.2: Analytical SDF Rounded Rectangles

* **Implementation Tasks:**

  1. Construct a fragment shader that evaluates the exact [2D Signed Distance Field](https://iquilezles.org/articles/distfunctions2d/) formula:

     $$
     d(\mathbf{p}) = \Vert{}\max(\mathbf{q}, 0)\Vert{} + \min(\max(q_x, q_y), 0) - r
     $$

  2. Configure the CPU generator to emit exactly 4 vertices and 6 indices for *every* rectangle.

  3. Compute perfect anti-aliasing in the fragment shader using hardware screen-space derivatives:

     $$
     \text{Alpha} = \text{clamp}(0.5 - \frac{d(\mathbf{p})}{\text{fwidth}(d(\mathbf{p}))}, 0.0, 1.0)
     $$

* **Technical Rationale:** Dramatically cuts CPU tessellation overhead and vertex buffer sizes while achieving flawless anti-aliasing at any zoom level, entirely bypassing the need for CPU-side triangle math for borders and radii.

### Step 3.3: SVG Tessellation & Keyframe Morphing

* **Implementation Tasks:**

  1. Integrate a robust ear-clipping/trapezoidal tessellator for static complex SVG paths, caching the resulting vertex soup to the `DynamicRingBuffer`.

  2. Implement path-morphing interpolation using the `wide` crate's `f32x8` vector type (TECHNICAL.md Section 5.4) -- one source-level implementation that compiles to genuine 256-bit AVX2 operations on x86_64 and to a pair of emulated 128-bit NEON operations on ARM64. Ensure topological equivalence (matching number of control points) between keyframes.

  3. Implement the stencil-and-cover fallback rendering method for path intersections that fail simple ear-clipping (e.g., self-intersecting paths with `EvenOdd` fill rules).

  4. **Harden the parser against untrusted input (added in the September 2026 documentation review):** enforce hard caps on `<use>` reference recursion depth, total resolved path point count per document, and group nesting depth; reject and report (via `Result<T, EngineError>`, never a panic or an unbounded loop) any document exceeding those caps before tessellation begins. This applies whenever an application path loads SVG that did not ship as a first-party asset -- if a given integration only ever loads trusted, build-time-bundled SVG, that assumption must be stated explicitly in that integration's own documentation rather than assumed silently here.

* **Technical Rationale:** Caching static tessellations prevents frame-over-frame CPU thrashing, while SIMD accelerates dynamic vector animations to maintain the $240\text{ Hz}$ throughput target. Input hardening prevents a malformed or adversarial SVG document from producing unbounded tessellation cost or unbounded recursion -- a denial-of-service risk for any application that loads SVG from outside its own build.

## Phase 4: Dynamic Typography & Texture Atlasing

### Step 4.1: HarfBuzz & FreeType Integration

* **Implementation Tasks:**

  1. Integrate [HarfBuzz](https://harfbuzz.github.io/) to evaluate OpenType features, handle bi-directional text (RTL/LTR), and generate shaped glyph clusters -- via the `harfbuzz_rs` binding crate, or raw FFI where a maintained binding lags upstream.

  2. Implement a Font Fallback cascade (e.g., primary font -> system UI font -> emoji font).

  3. Extract vector control points for required glyphs using [FreeType](https://freetype.org/) (`FT_Outline_Decompose`) to feed the MSDF generator -- via the `freetype` binding crate (the `freetype-rs` project's published crates.io name).

### Step 4.2: MSDF Rasterizer & Atlas Packing

* **Implementation Tasks:**

  1. Implement a 2D MaxRects Guillotine bin-packing algorithm. Maintain a list of free rectangles; upon insertion of a new glyph, find the best fit and split the remaining space horizontally or vertically. This algorithm has exactly one caller in the whole engine: the single atlas owner (ARCHITECTURE.md Section 2.3) -- it is never called concurrently and needs no internal synchronization of its own.

  2. Rasterize glyphs into an $RGB8$ buffer using [Multi-channel Signed Distance Fields (MSDF)](https://github.com/Chlumsky/msdfgen) (evaluating edge colorings to preserve sharp corners) at a fixed $32 \times 32\text{px}$ resolution.

  3. Implement the MSDF evaluation shader per the canonical formula in TECHNICAL.md Section 5.3 (median-of-channels signed distance, screen-space-derivative anti-aliasing) -- do not re-derive it here.

  4. **Multi-window atlas concurrency (ARCHITECTURE.md Section 2.3, TECHNICAL.md Section 8):** implement the bounded MPSC ring buffer that carries `AtlasInsertRequest`s from any window's tessellation phase to the atlas owner, and the fixed-capacity single-writer/multi-reader `AtomicU64` slot table the owner publishes completed `(rect, generation)` pairs into. The owner's own internal bookkeeping (the `LastFrameUsed` map driving LRU eviction, Section 10.2) is touched only by that one single-threaded owner and can safely use a plain `FxHashMap`/`ahash` map for speed -- it is a different structure from the cross-thread-visible `AtomicU64` slot table above, and swapping its hasher does not, by itself, make it safe for concurrent access.

* **Technical Rationale:** Using MSDF preserves sharp corners that traditional single-channel SDFs ruin. The Guillotine packer ensures highly efficient use of the $4096 \times 4096$ GPU atlas space.

## Phase 5: Multi-Threaded Canvas API & Metadata

### Step 5.1: The Canvas Command Recorder

* **Implementation Tasks:**

  1. Build the `RenderingCanvas` API with Drawing Contexts (`draw_rect`, `draw_text`, `push_layer`, `save`, `restore`).

  2. Define the lightweight Intermediate Representation (IR) struct (`UiDrawCommand`), containing a `kind` enum, 64-bit sort key, clip bounds, and geometry offsets.

  3. Implement Overlay routing logic: When `begin_overlay()` is called, assign commands a Layer ID $\ge 10000$ and reset the active clip stack to the native window bounds.

### Step 5.2: Multi-Threading & Lock-Free Sub-Canvases

* **Implementation Tasks:**

  1. Allocate a fixed-size `CommandArena` for every worker thread created by `create_sub_canvas()`.

  2. Thread $i$ writes $N_i$ commands to its local arena without any locking.

  3. During the flattening phase, the main thread computes the final global array location using atomic operations:
     `let write_offset = global_command_counter.fetch_add(n_i, Ordering::Relaxed);`

  4. Worker threads bulk-copy (`copy_from_slice`) their local arenas to the global arena at their acquired offset.

* **Technical Rationale:** The lock-free atomic merge ensures that stitching time remains practically zero (sub-microsecond), maximizing the benefits of multi-threaded UI traversal.

### Step 5.3: Spatial Accessibility Tagging (a11y)

* **Implementation Tasks:**

  1. Implement `Canvas::tag_accessibility_node(node_id, bounds, role_flags)`.

  2. Construct a metadata extractor that runs in parallel with the RHI submission phase.

  3. Map the extracted boundaries directly to OS-level accessibility trees (e.g., implementing [`IRawElementProviderSimple`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationcore/nn-uiautomationcore-irawelementprovidersimple) for Windows UIA, and bridging to [AT-SPI2](https://gnome.pages.gitlab.gnome.org/at-spi2-core/) on Linux).

* **Technical Rationale:** Accessibility tools receive pixel-perfect representations of the visual output without forcing the layout engine to run a separate, redundant accessibility tree calculation.

## Phase 6: Sorting, Batching, & RHI Execution

### Step 6.1: The 64-Bit Radix Batching Engine

* **Implementation Tasks:**

  1. Generate the 64-bit key for every IR command per the canonical bit layout in ARCHITECTURE.md Section 4.1 (Layer 16 / Pipeline 16 / Texture 12 / Depth 20 bits).

  2. Implement a 4-pass Radix Sort ($\mathcal{O}(N)$) utilizing local thread histograms, with pass widths matching the field boundaries above (not a fixed 16-bit digit per pass, since Texture and Depth are no longer 16 bits each).

  3. Calculate prefix sums over the histograms and scatter the `UiDrawCommand` array into a double-buffered secondary array.

  4. **Added in the September 2026 documentation review:** add a debug-build assert on Depth ID assignment that fires before the 20-bit field would overflow (ARCHITECTURE.md Section 4.1).

* **Technical Rationale:** Radix sort guarantees deterministic sub-millisecond sorting times even for extreme outliers (e.g., sorting $50,000$ draw commands in under $0.2\text{ ms}$).

### Step 6.2: Dynamic Index Stitching

* **Implementation Tasks:**

  1. Implement a linear sweep pass over the sorted IR array. Identify contiguous blocks of commands where Layer, Pipeline, and Texture bits are identical.

  2. Consolidate these commands by applying relative offsets to the index buffer: $idx_{\text{global}} = idx_{\text{local}} + \text{vertex}_{\text{offset}}$.

  3. Emit a single `RhiCommandBuffer::draw_indexed` call for the entire aggregated batch, drastically lowering driver submission overhead.

## Phase 7: Color Management & Compositing

### Step 7.1: Linear sRGB Conversions & HDR

* **Implementation Tasks:**

  1. Configure RHI swapchains for HDR execution (`VK_FORMAT_R16G16B16A16_SFLOAT`).

  2. Implement hardware-accelerated sRGB to Linear conversion directly in vertex/fragment shaders for color correctness prior to alpha blending, using the canonical piecewise formula defined once in TECHNICAL.md Section 6.2 -- do not re-derive it here.

  3. Implement the canonical identity-below-white, soft-knee-above-white tone mapping curve (TECHNICAL.md Section 6.3) as a final post-process step when the display's reported HDR headroom is less than the content's authored range. Do not use a full-range filmic curve like [ACES](https://docs.unity3d.com/Packages/com.unity.render-pipelines.core@17.0/manual/tonemapping.html#aces) by default -- this is a desktop UI engine, not a photo/film/video-editing tool, and ACES's deliberate contrast and desaturation shaping would visibly shift exact UI/brand colors that must render unchanged. Expose ACES (or another filmic curve) only as an explicit, opt-in per-`Canvas` style choice for creative-workstation/DAW integrations (DESIGN.md Section 3) that specifically want it for embedded video/image preview content.

* **Technical Rationale:** Blending in sRGB space causes dark fringes around anti-aliased geometry. Doing this math on the GPU in linear space ensures pristine transparency intersections. The tone-mapping curve choice (task 3) is a separate concern from the blend-space conversion (task 2): getting linear blending right prevents dark fringes on every frame; getting the tone-mapping curve right prevents the engine's own HDR support from being the thing that makes a UI's colors inconsistent.

### Step 7.2: Visual Filters (PushLayer Blurs)

* **Implementation Tasks:**

  1. Implement the Dual-Kawase blur algorithm for fast backdrop filtering.

  2. Execute a chain of downsample passes (averaging 4 surrounding pixels per step) followed by upsample passes.

  3. Map `Canvas::pop_layer` logic to redirect rendering out of the transient target, inject the Kawase down/up passes, and blend the final result back into the parent swapchain.

* **Technical Rationale:** The Dual-Kawase approach slashes memory bandwidth by iteratively reducing texture sizes, massively outperforming large-radius, single-pass Gaussian blurs.

## Phase 8: Main Event Loop & The 8-Stage Render Pipeline

### Step 8.1: Loop Orchestration & Frame Timing

* **Implementation Tasks:**

  1. Initialize microsecond-precision monotonic clocks (e.g., `QueryPerformanceCounter` on Windows, `clock_gettime` on Linux).

  2. Implement frame-rate independent spring physics evaluation and lerp decay to calculate motion deltas:

     $$
     x(t + \Delta t) = x_{\text{target}} + (x(t) - x_{\text{target}}) \cdot e^{-\lambda \Delta t}
     $$

  3. Stitch the entire engine together following a strictly enforced 8-stage sequence:
     *Wait Fences* $\rightarrow$ *Drain Events* $\rightarrow$ *Multi-Thread Canvas* $\rightarrow$ *Sub-Canvas Stitch* $\rightarrow$ *Tessellation/Atlas Check* $\rightarrow$ *Radix Sort & Batch* $\rightarrow$ *Ring Buffer Packing* $\rightarrow$ *RHI Submit & Present.*

## Architectural Decision Matrix

| Architecture Choice | Alternative Considered | Selected Decision | Rationale | 
| ----- | ----- | ----- | ----- | 
| **Implementation Language** | C++ | Rust, with a language-agnostic C-ABI boundary | Rust's ownership/borrow-checker gives compile-time memory- and data-race-safety for the zero-allocation, lock-free multi-threaded design (Sections 2, 5) without a runtime GC. A stable C-ABI boundary (TECHNICAL.md Section 9.4) keeps the engine's public surface usable by any UI framework language -- the project's own being Python. | 
| **Command Storage** | Struct-of-Arrays (SoA) | Array-of-Structs (AoS) Linear Arena | Contiguous memory allocation in a dynamic linear arena maximizes CPU L1/L2 cache locality during batch sorting. | 
| **Command Sorting** | Comparison QuickSort ($\mathcal{O}(N \log N)$) | 4-Pass Radix Sort ($\mathcal{O}(N)$) | Guarantees deterministic sub-millisecond sorting times even when UI trees contain over $10,000$ active nodes. | 
| **Font Pipeline** | Standard Raster Glyph Atlas | [MSDF (Multi-channel SDF)](https://github.com/Chlumsky/msdfgen) | Single $32\text{px}$ atlas can scale up to $200\text{pt}$ dynamically without re-rasterizing on CPU or incurring memory bloating. | 
| **Rounded Rectangles** | CPU Triangle Tessellation | Analytical Fragment SDF Shader | Reduces vertex memory usage by $>90\%$, simplifies clipping math, and maintains pristine anti-aliasing on high-DPI screens. | 
| **Buffer Access** | Staging Buffer Copy Queue | Mapped Ring Buffers | Eliminates dynamic GPU queue copy commands, lowering CPU submission latency under $0.5\text{ ms}$. | 
| **Texture Binding** | Traditional Descriptor Sets | Bindless Arrays / Descriptor Indexing | Allows a single draw call to sample from diverse texture/font atlases by passing a bindless handle inside the 32-byte vertex. | 
| **Sub-Canvas Merge** | Mutex / Spinlocks | `AtomicUsize::fetch_add` | Guarantees lock-free stitching of worker thread rendering arenas, achieving near-zero cost merging overhead at frame lock-in. | 
| **Multi-Window Atlas Concurrency** | Mutex around the shared atlas | Lock-free MPSC request queue + single-writer/multi-reader `AtomicU64` slot table | No window ever blocks on another window's atlas traffic; the Guillotine packer still has exactly one writer (an unavoidable sequential step), but nothing waits on it (ARCHITECTURE.md Section 2.3). | 
| **SIMD Abstraction** | Hand-written duplicate AVX2 (x86_64) and NEON (ARM64) intrinsic code | [`wide`](https://docs.rs/wide) crate's portable `f32x4`/`f32x8` types | One shared, safe source-level implementation compiles to native AVX2 on x86_64 and emulates 256-bit ops as paired NEON ops on ARM64, eliminating hand-maintained per-architecture duplicate code and the `unsafe` it would otherwise require (TECHNICAL.md Section 2.2). | 
| **Hash Map Implementation** | `std::collections::HashMap` (default SipHash) | `FxHashMap` (`rustc-hash`) / `ahash` | Internal engine keys (pool dimensions, atlas glyph keys) receive no untrusted input, so SipHash's DoS resistance buys nothing; a faster non-cryptographic hash removes overhead from two per-frame hot-path lookups. | 
| **Tone Mapping Curve** | ACES filmic | Identity-below-white, Reinhard-style compression above white | ACES's cinematic contrast/desaturation shaping is wrong for a UI engine, where exact brand/UI colors must reach the screen unchanged; the chosen curve leaves all standard content untouched and only compresses genuinely-HDR content (TECHNICAL.md Section 6.3). | 

## Phase 9: Testing & Validation Strategy (Added in September 2026 Documentation Review)

Prior drafts specified only performance regression testing (TECHNICAL.md Section 9.2). Performance and correctness are different failure classes and need separate coverage.

### Step 9.1: Correctness Test Suite

* **Implementation Tasks:**

  1. Unit-test the radix sort against adversarial key distributions (all-identical keys, reverse-sorted input, keys clustered at field boundaries, maximum Depth ID values) -- not just random/typical distributions.

  2. Unit-test the Guillotine atlas packer for fragmentation behavior and correct LRU eviction ordering under sustained insert/evict churn, including the failure path added in DESIGN.md Section 2.6 (eviction insufficient, placeholder glyph fallback engaged).

  3. Build a batching-equivalence test: render a scene both through the full batched pipeline and through a naive one-draw-call-per-primitive reference path, and pixel-diff the two outputs. A mismatch indicates a batching or sort-key bug, not a performance regression, and must fail the build.

  4. Fuzz-test the SVG parser and tessellator (Phase 3.3 hardening) with malformed and adversarial documents, asserting bounded tessellation time and memory regardless of input.

* **Technical Rationale:** The performance suite alone cannot catch a batching pass that is fast but wrong (e.g., silently dropping or misordering a draw command). Pixel-diff equivalence testing is the only test that directly validates the "batched output looks identical to unbatched output" invariant the entire architecture depends on.

### Step 9.2: Zero-Allocation & Balance Assertions in CI

* **Implementation Tasks:**

  1. Run the full test suite under the zero-allocation debug guard (TECHNICAL.md Section 3.4) and fail the build on any allocation observed during a render tick.

  2. Run the full test suite under the `PushLayer`/`PopLayer` balance assertion (Phase 2, Step 2.2) and fail the build on any nonzero depth at frame boundary.

* **Technical Rationale:** These are cheap, deterministic, always-on gates that turn two of the engine's most important invariants -- zero steady-state allocation and balanced transient resource acquisition -- into build failures instead of production incidents.

## Phase 10: Cross-Language Bindings & Python UI Framework Integration (Added with the Rust/Python Language Decision)

### Step 10.1: The `tre-ffi` C-ABI Crate

* **Implementation Tasks:**

  1. Define the complete public surface of the engine as `#[repr(C)]` opaque handles and `extern "C"` functions in a dedicated `tre-ffi` crate, per the ABI shape rules in TECHNICAL.md Section 9.4 -- every other crate in the workspace is still linked into the shipped `cdylib`/`staticlib` (Section 9.2), but none of them export their own `extern "C"` symbols; `tre-ffi` is the sole exporter.

  2. Wrap every exported function body in `std::panic::catch_unwind`, translating any caught panic into the corresponding `EngineError` result code (DESIGN.md Section 2.6) rather than allowing it to unwind across the boundary.

  3. Build both `cdylib` (for Python/PyO3 and any future dynamic-language binding) and `staticlib` (for a C++ host linking the engine directly) output targets from the same `tre-ffi` crate, demonstrating that the boundary is not Python-specific.

* **Technical Rationale:** Concentrating the entire FFI surface in one crate makes the language boundary auditable in a single place, and building both `cdylib` and `staticlib` targets from day one is the cheapest available proof that the engine is genuinely UI-framework-language-agnostic rather than Python-agnostic in name only.

### Step 10.2: Python UI Framework Bindings

* **Implementation Tasks:**

  1. Generate the Python extension module via [PyO3](https://pyo3.rs/), wrapping `tre-ffi`'s C-ABI -- not calling into `tre-engine` internals directly -- so the Python bindings exercise the identical boundary any other language would use.

  2. Provide Pythonic ergonomics at the binding layer only: context managers for `Canvas.save()`/`restore()` scope pairs, Python exceptions raised from `EngineError` codes, and buffer-protocol views over headless frame readback buffers (DESIGN.md Section 4.3) to avoid an extra copy into Python.

  3. Release the GIL (`Python::allow_threads`) around any engine call that can block on a GPU fence (e.g., `RhiDevice::begin_frame`), so the Python UI framework's own threads are not serialized behind engine waits.

  4. Add the Python-binding test suite as a required CI job (TECHNICAL.md Section 9.2), exercising the correctness suite (Phase 9, Step 9.1) through Python rather than duplicating it.

* **Technical Rationale:** Routing the Python bindings through `tre-ffi` rather than a Python-specific shortcut into the Rust internals keeps the engine honest about its language-agnostic claim (DESIGN.md Section 2.7) -- if the Python UI framework ever needed something the public C ABI didn't expose, that would signal the ABI itself is incomplete, not a reason to add a side channel.
