# High-Performance UI Rendering Engine: Design Specification - Tesserae Render Engine (TRE)

## 1. Executive Summary & Purpose

The objective of this project is to build a low-overhead, hardware-accelerated 2D rendering engine API, implemented in **Rust**, designed explicitly as a bridge between high-level User Interface (UI) frameworks and low-level Graphics Hardware Interfaces (RHIs) such as Vulkan, DirectX 12, Metal, and WebGPU.

The project's own high-level UI framework is being built in **Python** and is the engine's first and primary consumer. The engine's public API, however, is deliberately language-agnostic: it is exposed across a stable C-ABI boundary (Section 2.7) so that any UI framework -- a custom C++ GUI toolkit, a modern desktop UI runtime, or a language other than Python entirely -- can bind to it without modifying the engine core. Python is the reference integration, not a special case baked into the engine's design.

Traditional 3D game engines carry substantial state management overhead, frame graph complexities, and heavy pass setups that are ill-suited for UI rendering. Conversely, basic software or naive modern graphics wrappers suffer from high CPU draw-call submission latency, poor glyph crispness under arbitrary scaling, dynamic state switches caused by nested clipping, and limited offscreen compositing flexibility.

This rendering engine serves as a specialized, low-latency target focused on vector path rendering, dynamic typography, native static and animated SVG rendering, fast alpha-compositing, offscreen visual filters, layered scissor/stencil clipping, native multi-platform window management, offscreen headless execution, multi-threaded command recording, linear color-space blending, HDR/wide-gamut display support, real-time geometry batching, and frame-rate independent animation/motion mechanics.

## 2. Core Philosophy & Design Principles

### 2.1 Zero-Allocation Steady State

During active frame rendering, the core loop must perform zero dynamic CPU memory allocations ($0\text{ bytes}$ via `malloc` or `new`). All frame-bound memory—including command buffers, vertex queues, index buffers, push constants, animation timelines, parallel sub-canvases, accessibility tree nodes, and event queues—must be allocated from pre-reserved frame-arena accumulators or ring buffers.

*Boundary Definition:* This strict zero-allocation rule applies exclusively to the `RenderingCanvas` recording, intermediate representation flattening, and RHI submission phases. Complex external subsystems operating during layout (e.g., initial SVG DOM parsing, HarfBuzz text shaping) should utilize custom arena allocators to tightly bound their memory usage outside the active render tick.

### 2.2 Strict Architectural Separation of Concerns

Rendering, event handling, accessibility metadata sync, and layout calculations operate in distinct, decoupled subsystems. Event polling, signal dispatching, accessibility querying, and hit-testing must never block or pollute graphics context state, ensuring that input processing and visual pipeline scheduling maintain clear boundaries.

### 2.3 Predictable, Frame-Rate Independent Throughput

UI rendering, motion physics, and visual filter passes must maintain consistent frame delivery up to $240\text{ Hz}$ ($4.16\text{ ms}$ total frame time budget). The CPU submission phase, multi-threaded command compilation, and layout/motion tick must complete in $t_{\text{CPU}} \le 1.0\text{ ms}$, leaving maximum headroom for UI layout engine calculations and event dispatching.

### 2.4 Deterministic Rendering Order & Batch Flattening

UI elements rely heavily on painter's algorithm order for correct visual hierarchy and transparency compositing. The API decouples drawing commands from physical driver submissions. Commands submitted by the UI layer across main and worker threads are recorded into an intermediate representation, key-sorted to group similar primitive types and states, flattened into contiguous index/vertex streams, and coalesced into as few draw dispatches as possible—aiming for a single GPU draw call per render pass when state parameters permit.

### 2.5 Resolution-Independent Scalability & Dynamic Vector Asset Support

All UI primitives—including rounded rectangles with non-uniform corner radii, stroke paths, drop shadows, text glyphs, and complex static/animated SVG graphics—must render crisp visual output at any scale factor ($1.0\times$ to $4.0\times+$ DPI) without needing CPU re-rasterization or multi-pass texture scaling artifacts.

### 2.6 Explicit Failure Modes & Graceful Degradation

Every subsystem must define what happens when its steady-state assumptions break, not just how it behaves in the common case. The engine never uses a Rust panic for an expected or recoverable failure (per the project-wide panic-free-on-the-happy-path rule); every fallible operation returns `Result<T, EngineError>`, and callers are required to handle the failure path explicitly. Panics are reserved strictly for programmer errors (invariant violations, indexing bugs), and per Section 2.7 are caught at the C-ABI boundary rather than allowed to unwind into the calling UI framework.

At minimum, the following failure modes must have a documented, tested response before a subsystem ships:

* **Device loss / swapchain acquire failure:** GPU device removal, driver TDR, or an out-of-date swapchain must be detected at `RhiDevice::begin_frame` and surfaced as a recoverable error, allowing the host application to reinitialize the device and resume rather than crash.
* **Atlas exhaustion beyond LRU capacity:** If Guillotine bin-packing fails and LRU eviction (Section 10.2) cannot free enough contiguous space for an incoming glyph or icon, the engine falls back to a lower-fidelity placeholder (e.g., a bounding-box glyph or solid-color swatch) for that frame rather than stalling the render loop or allocating an emergency atlas.
* **Malformed or pathological SVG input:** The SVG parser and tessellator must reject malformed documents and bound worst-case tessellation cost (see IMPLEMENTATION.md Phase 3.3) rather than trusting input shape.
* **Ring buffer / transient pool starvation:** If a frame's draw volume exceeds the pre-reserved ring buffer or transient render target pool capacity, the engine drops the lowest-priority pending draw commands (overlays first, then standard content by depth) and reports a frame-budget diagnostic, rather than growing memory dynamically mid-frame.
* **Shader compilation / pipeline creation failure:** Detected at startup or asset-load time, never mid-frame; a failed pipeline falls back to a minimal built-in solid-fill shader so the application keeps running in a degraded but visible state.
* **Transient render-target pool miss:** If `Canvas::push_layer` requests an offscreen size/format combination that isn't already resident in the pool (e.g., a first-ever window size, or an animated blur radius that produces a novel dimension this frame), the engine must not perform a dynamic RHI texture allocation inside the render tick (Section 2.1, TECHNICAL.md Section 3.2). Pool entries are bucketed to fixed size breakpoints so a wide range of requests hit an existing entry; a genuine miss borrows the next-larger already-pooled entry for that frame (rendering into a sub-rect) while a correctly-sized target is grown into the pool asynchronously for subsequent frames.

This principle exists specifically to close a gap identified in the September 2026 documentation review: the original draft specified the *happy path* in detail but left every failure boundary implicit.

### 2.7 Implementation Language & Cross-Language Boundary

The engine core is implemented in **Rust**. Rust's ownership model gives the zero-allocation steady state (Section 2.1) and the lock-free multi-threaded canvas recording (Section 6.3) compile-time-checked memory- and data-race-safety, without the runtime GC pauses that would threaten the frame budget in Section 2.3.

The engine is otherwise **language-agnostic** with respect to the UI framework driving it: the `Canvas` API (Section 6) is exposed across a stable C-ABI boundary (`extern "C"` functions, `#[repr(C)]` types only -- see TECHNICAL.md Section 9.4 for the binding mechanism), not a Rust-specific interface. Any UI framework, written in any language capable of calling a C ABI, can bind to the engine on equal footing.

The project's own high-level UI framework -- the engine's first and primary consumer -- is implemented in **Python**, binding to the engine through this same C-ABI boundary. This is a deliberate exercise of the language-agnostic boundary, not a special-cased integration: the Python UI framework receives no engine access that a UI framework written in C++, C#, or any other language could not also obtain through the same bindings.

*FFI Safety:* A Rust panic must never unwind across the C-ABI boundary -- doing so is undefined behavior. Every `extern "C"` entry point wraps its body in `std::panic::catch_unwind` and converts a caught panic into an `EngineError` result code (Section 2.6), so a programmer error inside the engine (e.g., an out-of-bounds index during `RhiDevice::begin_frame`) surfaces to the calling UI framework as a recoverable error rather than crashing the host process or corrupting its stack.

## 3. Target Applications & Integration Scope

The engine is engineered specifically for high-throughput desktop applications and dedicated desktop-class host platforms. To maximize focus on ultra-low latency and modern desktop RHI capabilities, the scope explicitly excludes resource-constrained embedded/automotive platforms and mobile application runtimes.

Primary integration targets include:

* **Desktop Application Toolkits:** High-density enterprise interfaces, creative workstation applications, and professional digital audio workstaitons (DAWs) demanding rapid window resizing, multi-window document layouts, sub-pixel aligned text rendering, dynamic layout updates, low memory footprints, color-managed displays, and fluid dynamic animations.

* **Game Engine UI Overlay Systems:** High-performance in-game HUDs, interactive tools, and overlay menus running atop custom DirectX 12, Vulkan, or Metal pipelines without stepping on primary 3D render states.

* **Automated CI/CD & Server-Side Headless Tooling:** Offscreen UI execution runtimes used for automated visual regression testing, pixel-diffing suites, dynamic video/image export, and remote UI stream generation.

**Reference Integration -- Python UI Framework:** The project's own high-level UI framework (Python, Section 2.7) targets the Desktop Application Toolkit category above and serves as the reference, dogfooding integration that validates the C-ABI boundary end to end.

## 4. Platform, Multi-Windowing & Headless Subsystem

The engine integrates a native multi-windowing platform abstraction layer, managing multiple OS application windows and offscreen targets over shared graphics device resources.

```
+-------------------------------------------------------------------------+
|                Multi-Window & Platform Abstraction Layer                |
+--------------------+--------------------+-------------------------------+
|  Windows (Win32)   |   Linux (Wayland)  |   macOS (AppKit/Metal)        |
|  - HWND / WGL      |   - wl_surface     |   - NSWindow / CAMetalLayer   |
|  - DirectComposition|  - X11 (XCB fallback)|  - Cocoa Event Dispatch      |
+--------------------+--------------------+-------------------------------+
|                      Multi-Window Lifecycle Manager                     |
|  - Shared GPU Device Context & Global Atlas Resources                   |
|  - Per-Window Swapchains, Scissor Contexts, and Render Targets          |
+-------------------------------------------------------------------------+
|                      Headless / Offscreen Subsystem                     |
|  - Zero-window display surface creation                                 |
|  - Software / GPU Framebuffer Readback & Surface Export                 |
+-------------------------------------------------------------------------+


```

### 4.1 Multi-Window Architecture

* **Shared Resource Context:** All native window surfaces created by the engine share a unified graphics context (`RhiDevice`), GPU dynamic atlas textures, pipeline state objects, and font engines, minimizing memory fragmentation across multi-window desktop applications.

* **Independent Swapchain Pipeline:** Each window maintains its own hardware swapchain (`RhiSwapchain`), resolution/DPI scale factors, dirty-region invalidation tracking, color space configuration, and render pass commands. Swapchains explicitly support per-pixel alpha compositing for borderless/transparent windows and unconstrained tearing (`DXGI_PRESENT_ALLOW_TEARING` / `VK_PRESENT_MODE_IMMEDIATE_KHR`) for low-latency game overlays.

* **Multi-Window Synchronization:** Windows can render synchronously on a single frame tick or independently update dirty surfaces on demand during resize operations.

### 4.2 Platform Implementations

1. Windows (Win32): Direct integration with native `HWND` handle creation, multi-monitor DPI context hooks (`PerMonitorV2`), raw input handling (`WM_INPUT`, `WM_POINTER`), and `IDXGISwapChain3` surface attachments for DirectX 12 and Vulkan backend targets.

2. **Linux (Wayland & X11):**

   * *Wayland (Primary):* Native protocol bindings using `wl_surface`, `xdg_shell`, `zwp_text_input_v3` for high-dpi text composition, multi-surface windowing, and sub-surface compositing.

   * *X11 (Fallback):* XCB window handling and EGL/GLX surface binding for legacy Linux desktop environments.

3. **macOS (AppKit & Cocoa):** Native `NSWindow` / `NSView` wrappers with layer-backed `CAMetalLayer` targets, supporting Retina Display hidpi scaling, multi-window keying, native swipe gestures, and window resize loop hooks.

### 4.3 Headless Rendering Mode

* **Zero-Window Execution:** Enables creation of offscreen render contexts without allocating native display servers or OS windows.

* **Frame Snapshotting & Export:** Directly reads frame memory back into CPU staging memory, outputting raw uncompressed RGBA pixel buffers or encoded dynamic image frames for visual regression testing and server-side layout engines.

## 5. Event, Signal & Accessibility Architecture (Decoupled from Rendering)

Input processing, window state listeners, multi-window focus changes, accessibility updates, and UI signal propagation are decoupled from graphics execution. The rendering system is purely a consumer of state produced by the UI framework and input subsystems.

```
+-------------------------------------------------------------------+
|                        OS Event Pump & Drivers                   |
|          (Multi-Window Mouse, Touch, Keyboard, Focus, Resize)     |
+-------------------------------------------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                  Decoupled Input & Signal Engine                  |
|   +-----------------------------------------------------------+   |
|   | Multi-Window Input Event Queue (Lockless Ring Buffer)    |   |
|   +-----------------------------------------------------------+   |
|   | Spatial Hit Testing & Scene Tree Node Focus Manager       |   |
|   +-----------------------------------------------------------+   |
|   | Accessibility Metadata Bridge (UIA/NSAccess/AT-SPI2)   |   |
|   +-----------------------------------------------------------+   |
|   | Signal / Slot Dispatcher (Asynchronous State Mutations)   |   |
|   +-----------------------------------------------------------+   |
+-------------------------------------------------------------------+
                                 |
                                 v  (Mutates UI Framework State Only)
+-------------------------------------------------------------------+
|                       High-Level UI Framework                     |
|           (Layout Recalculation, DOM / Widget Tree Node Update)    |
+-------------------------------------------------------------------+
                                 |
                                 v  (Pushes Draw Calls & Metadata via Canvas)
+-------------------------------------------------------------------+
|                      Rendering Engine Pipeline                    |
+-------------------------------------------------------------------+


```

### 5.1 Input & Signal Queue

* **Lock-Free Event Ingestion:** Native OS window events across all open windows are pushed directly to a thread-safe ring queue without acquiring lock primitives.

* **Decoupled Processing:** The UI framework drains the event queue during its logic tick, evaluating spatial hit tests against scene tree bounds and emitting signals (e.g., `OnClick`, `OnHover`, `OnValueChange`).

* **Zero GPU Thread Interference:** Input processing and signal routing execute completely off the rendering thread. The graphics pipeline never performs direct event dispatching or hit testing during command stream compilation.

### 5.2 Accessibility (a11y) & Spatial Boundary Metadata Integration

* **Spatial Boundary Tagging:** As drawing commands are recorded into the `Canvas`, the UI framework can attach accessibility annotations and bounding boxes (`Canvas::tag_accessibility_node(node_id, bounds, role_flags)`).

* **OS Accessibility Bridge:** Bounding boxes and node descriptors are extracted directly during command stream traversal and exported to native OS accessibility systems (Windows UI Automation, macOS NSAccessibility, Linux Wayland AT-SPI2).

* **Zero Layout Re-evaluation:** Screen readers query spatial metadata generated directly from the visual rendering queue, guaranteeing $100\%$ alignment between what is visually rendered on screen and what is reported to assistive technology.

## 6. Rendering Canvas & Parallel Recording API

The **Rendering Canvas** serves as the explicit API boundary and bridge between the high-level UI framework and the low-level rendering pipeline. It abstracts state management, clip stack hierarchy, geometry submission, overlay registration, offscreen compositing layers, and vector pass generation.

```
+-------------------------------------------------------------------+
|                       UI Framework Target                         |
|   +-----------------------------------------------------------+   |
|   | Main Canvas / Thread           Worker Thread 1 .. N       |   |
|   | (Root Transforms, Layout)      (Parallel Sub-Canvases)    |   |
|   +-----------------------------------------------------------+   |
+-------------------------------------------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                         Rendering Canvas                          |
|  - Drawing Context State Stack (Matrix Transform, Alpha, Blend)   |
|  - Primitive Recording API (DrawRect, DrawText, DrawPath, DrawSVG) |
|  - Compositing Layer Stack (PushLayer / PopLayer - Blurs, Effects)|
|  - Dynamic Scissor / Mask Clip Stack (PushClip, PopClip)          |
|  - Parallel Sub-Canvas Aggregator (Lock-Free Command Merge)      |
|  - Accessibility Metadata Recorder (TagAccessibilityNode)        |
+-------------------------------------------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|                Intermediate Representation (IR) Builder           |
|            (Command Sorting, Batching & RHI Submission)           |
+-------------------------------------------------------------------+


```

### 6.1 Canvas Core Responsibilities

* **Primary Interface Entry Point:** The UI Framework obtains a reference to a `Canvas` instance tied to a native window surface or offscreen target.

* **Imperative Drawing Context:** Exposes dynamic drawing primitives:

  * `Canvas::draw_rounded_rect(&Rect, &CornerRadii, &Paint)`

  * `Canvas::draw_text(&DynamicTextLayout, &Point, &Paint)`

  * `Canvas::draw_path(&Path, &StrokeAndFillStyle)`

  * `Canvas::draw_svg(&SvgDocumentHandle, &Rect)`

* **Hierarchical State Stack:** Manages dynamic coordinate space transformations, global alpha multipliers, and clip boundaries via `Canvas::save()` and `Canvas::restore()`.

* **Overlay Layer Management:** Exposes specialized scope operators (`Canvas::begin_overlay(OverlayLayerPriority)`) to route popups and modal menus directly into dedicated rendering planes.

### 6.2 Dynamic Compositing Layers & Visual Filters (`PushLayer` / `PopLayer`)

* **Offscreen Render Target Allocation:** Complex UI widgets, glassmorphism containers, dynamic drop shadows, or group opacity nodes execute offscreen passes via scope wrappers:

  * `Canvas::push_layer(&LayerDesc)`

  * `Canvas::pop_layer()`

* **Transient Render Target Pooling:** To maintain the zero-allocation steady state, offscreen targets requested via `PushLayer` are acquired from a pre-allocated GPU transient texture pool. Targets are dynamically resized or recycled at frame boundaries to prevent VRAM fragmentation and costly pipeline stalls.

* **Visual Filter Pipeline:**

  * *Group Opacity:* Applies a single collective alpha multiplier to a tree of overlapping elements without causing intra-element transparency overlap artifacts.

  * *Backdrop & Gaussian Blurs:* Supports dynamic real-time dual-kawase or downscaled Gaussian backdrop blurs for modern translucent macOS Vibrancy and Windows Acrylic/Mica dynamic UI panel effects.

  * *Blend Modes:* Supports advanced layer blending operations including Multiply, Screen, Overlay, Soft Light, and Color Dodge.

### 6.3 Multi-Threaded Parallel Canvas Recording

* **Thread-Local Sub-Canvases:** For complex interfaces with high widget counts, the UI framework can split command recording across worker threads using light child sub-canvases (`Canvas::create_sub_canvas()`).

* **Lock-Free Merging:** Sub-canvases record draw commands into thread-local arenas. At frame lock-in, sub-canvas command streams are stitched into the main frame IR array via atomic memory fence operations without thread lock contention.

## 7. Scene Hierarchy, Ordering, Layering & Overlay Subsystem

User interfaces require structured visual hierarchy, spatial containment, strict ordering rules, and escaping mechanism for top-level floating elements.

```
+-------------------------------------------------------------------+
|                      UI Framework Scene Graph                     |
|  (Parent Node -> Child Nodes with Local Transforms & Local Z-Index) |
+-------------------------------------------------------------------+
                                 |
                                 v Matrix & Z-Index Flattening
+-------------------------------------------------------------------+
|                     Rendering Layer Allocator                     |
|   +-----------------------------------------------------------+   |
|   | Standard Content Plane (Layer 0 .. 1000)                  |   |
|   |  - Parent/Child Ordered Draw Commands                     |   |
|   |  - Local Scissor Boundaries Enforced                      |   |
|   +-----------------------------------------------------------+   |
|   | Overlay & Floating Plane (Layer 10000+)                   |   |
|   |  - Context Menus, Tooltips, Dropdowns, Modal Dialogs      |   |
|   |  - Unconstrained by Parent Clipping Bounds                |   |
|   +-----------------------------------------------------------+   |
+-------------------------------------------------------------------+


```

### 7.1 Scene Hierarchy & Transformation Propagation

* **Parent-Child Relationships:** The UI framework expresses visual elements as hierarchical trees. As nodes are traversed, the local affine matrices are multiplied down the hierarchy to compute the final world transform:

  $$
  \mathbf{M}_{\text{world\_child}} = \mathbf{M}_{\text{world\_parent}} \times \mathbf{M}_{\text{local\_child}}
  $$

* **Z-Index & Painter's Order:** Ordering is determined by a combination of explicit Z-index property values and implicit depth-first traversal order (traversal index). Explicit Z-index overrides default parent-child stacking without breaking world transformation inheritance.

### 7.2 Native Overlay & Popup Management

* **Clipping Escape Mechanism:** UI elements like dropdown menus, tooltips, context menus, and modal dialogs are logically nested under child widgets in the UI tree, but visually escape parent bounding box scissor clips.

* **Dedicated Overlay Rendering Layers:** The engine provides a dedicated Overlay subsystem at the rendering level. When an element is recorded within an overlay scope, its draw commands and bounding scissors are decoupled from the parent container's scissor stack and assigned to high-priority global overlay depth layers ($\text{Layer ID} \ge 10000$).

* **Overlay Backdrops & Modal Dimming:** Supports automatic rendering of full-viewport modal backdrop layers with dynamic alpha attenuation and blur passes prior to rendering modal content.

## 8. Drawing, Dynamic Batching & Draw Flattening Engine

To achieve maximum rendering throughput and low latency, the engine implements aggressive geometry flattening and dynamic state batching to reduce draw dispatches towards a theoretical target of **a single draw call per frame per window layer**.

```
Submit Sequence (UI Traversal Across Threads):
 [Rect: P1, Tex 0] -> [Text: P2, Atlas A] -> [Rect: P1, Tex 0] -> [Rect: P1, Tex 0] (Overlay)

Radix Key Sorting & Flattening:
 +-----------------------------------------------------------------------+
 | Batch 0 (Layer Standard, Pipeline Rect P1, Texture 0):                |
 |  -> Rect 1 Vertices + Rect 2 Vertices concatenated                    |
 |  -> Unified Index Buffer Stitching                                    |
 |  -> Dispatched in 1 Draw Call (vkCmdDrawIndexed)                     |
 +-----------------------------------------------------------------------+
 | Batch 1 (Layer Standard, Pipeline Text P2, Atlas A):                  |
 |  -> Text Glyphs concatenated                                         |
 |  -> Dispatched in 1 Draw Call                                        |
 +-----------------------------------------------------------------------+
 | Batch 2 (Layer Overlay, Pipeline Rect P1, Texture 0):                 |
 |  -> Overlay Rect Vertices (Scissor: Full Window)                     |
 |  -> Dispatched in 1 Draw Call                                        |
 +-----------------------------------------------------------------------+


```

### 8.1 Primitive Type Coalescing & Batching Strategy

1. **Texture Atlas Unification:** Icons, vector glyphs, MSDF typography, and small UI images are packed into single global GPU atlas textures. Elements using the same atlas share a single texture handle, enabling cross-widget batching.

2. **Bindless Textures / Descriptor Indexing:** To prevent batch-breaking when alternating between monochrome text ($R8$) and color vectors ($RGBA8$), the RHI utilizes bindless texture arrays or descriptor indexing. This allows a single draw call to sample from multiple multi-format atlases dynamically based on a texture index passed in the vertex data.

   *Shader Unification (added in the September 2026 documentation review):* Batching across atlas formats only works if the fragment shader itself is unified. `PipelineStateId` selects a *shader family* (e.g., "Standard 2D" vs. "Blur" vs. "Custom"), not a per-format variant. Within the "Standard 2D" family, a small shader-mode tag packed into a spare `params` lane tells the fragment shader whether to evaluate an analytical SDF rounded-rect, sample a plain RGBA8 texture, or run the MSDF median/`fwidth` opacity calculation (TECHNICAL.md Section 5.3). This keeps text, icons, and rounded rects on one pipeline state and one draw call at the cost of a branch per fragment, which is materially cheaper than a state switch and resolves an ambiguity between this batching claim and the sort key's per-pipeline granularity.

3. **Shader Uniform Instance Streams:** Dynamic properties (color, corner radii, border thickness, matrix transforms) are packed directly into per-vertex parameter fields or uniform instance arrays, allowing visually distinct widgets (e.g., buttons, cards, panels) to share a single graphics pipeline state (`PipelineStateID`).

4. **Dynamic Vertex & Index Stitching:** Contiguous quads and triangle meshes submitted across distinct UI tree nodes are flattened into a single contiguous mapped vertex buffer segment. Index offsets are adjusted in real-time ($idx_{\text{global}} = idx_{\text{local}} + vertex_{\text{offset}}$), merging hundreds of individual widget draw calls into a unified index stream.

5. **State Transition Minimization:** Draw commands are key-sorted using a 64-bit sort key. State changes (pipeline binds, texture updates, scissor rect modifications) are evaluated during a linear sweep, emitting execution batch boundaries only when hardware states physically change.

## 9. SVG & Animated SVG Vector Pipeline

The engine includes native parsing, tessellation, dynamic keyframe morphing, and rendering capabilities for standard SVG and Animated SVG graphics.

```
+-------------------------------------------------------------------+
|                    SVG Document Parser & Asset Manager            |
|         (Parses SVG 2.0 XML / Binary Vector Representations)      |
+-------------------------------------------------------------------+
                                 |
                                 v
+-------------------------------------------------------------------+
|               SVG Geometry & Path Animation Engine                |
|   +-----------------------------------------------------------+   |
|   | Path Vector Tessellator (Analytical Bézier Curves & Arc)  |   |
|   +-----------------------------------------------------------+   |
|   | Parametric Path Morphing & Interpolation (SMIL / CSS)     |   |
|   +-----------------------------------------------------------+   |
|   | Dynamic Gradient & Pattern Fill Evaluator                 |   |
+-----------------------------------------------------------+---+
                                 |
                                 v
+-------------------------------------------------------------------+
|               Rendering Canvas Draw Execution                     |
+-------------------------------------------------------------------+


```

### 9.1 SVG Capability Scope

* **Static Vector Graphics (SVG 2.0 Subset):** Native rendering of complex path shapes, linear/radial multi-stop gradients, stroke dashing, fill rules (`NonZero`, `EvenOdd`), groups (`<g>`), and embedded viewports.

* **Animated SVG Support:** Supports CSS/SMIL keyframe vector animations, path morphing (interpolating control points between compatible topological path segments), dynamic opacity/transform transitions, and color ramp animations evaluated on frame ticks.

* **Tessellation & GPU Evaluation:** Static SVG paths are tessellated into dynamic triangle strips or evaluated analytically via GPU shaders for zero-overhead crisp scaling. Dynamic path morphing executes using SIMD path interpolation algorithms prior to vertex buffer uploads.

## 10. GPU Resource Lifecycle & Dynamic Atlas Management

### 10.1 Dynamic Atlas Allocation

* **Multi-Format Atlases:** Maintains unified GPU dynamic atlases for monochrome alpha glyphs ($R8$), MSDF glyphs ($RGB8$), and color UI vector icons ($RGBA8$).

* **Guillotine Bin-Packing:** Dynamic insertion of newly encountered text glyphs and dynamically generated vector decals into atlas textures using a high-efficiency 2D Guillotine rectangle bin-packing algorithm.

### 10.2 LRU Eviction & Generational GC

* **Generational Age Tracking:** Every entry in the dynamic atlas and dynamic path tessellation cache maintains a frame timestamp tag ($\text{LastFrameUsed}$).

* **Least-Recently-Used (LRU) Garbage Collection:** When an atlas space capacity exceeds $85\%$, the engine runs an asynchronous LRU garbage collection pass, evicting stale glyphs and unreferenced path meshes that have not been rendered within the last $N$ frames (e.g., $N \ge 600$ frames).

* **Deferred GPU Reclamation:** Evicted memory regions and graphics resource handles are funneled into a deferred release ring buffer, guaranteeing that resource memory is never freed while currently in-flight on the GPU.

### 10.3 Multi-Window Atlas Insertion Concurrency

Because the dynamic texture atlas is a single global resource owned by the shared `RhiDevice` (Section 4.1), and windows may render on independent per-window timelines (Section 4.1's "Multi-Window Synchronization"), two windows can discover a missing glyph or icon in the same tick and both need to mutate the same atlas -- the Guillotine bin-packer's free-rectangle list cannot be safely mutated by two threads at once, so this cannot be solved by making the packer itself concurrent.

* **Single Atlas Owner:** Exactly one execution context -- part of the global `RhiDevice`, never a per-window thread -- performs all Guillotine bin-pack insertions and MSDF rasterization. This is an unavoidable sequential bottleneck for the *packing operation itself*, not a design compromise.
* **Lock-Free Request Path:** A window's tessellation/atlas phase that finds a missing glyph never blocks waiting for the atlas owner. It enqueues an insertion request onto a bounded, pre-allocated lock-free multi-producer queue (TECHNICAL.md Section 8) and continues; if the atlas owner hasn't processed the request in time for that window's own frame, the window falls back to the existing placeholder-glyph degradation path (Section 2.6) for that one frame and re-checks next frame. No window ever stalls on another window's atlas traffic.
* **Lock-Free Publish Path:** Once the atlas owner packs and rasterizes an entry, it publishes the resulting atlas coordinates into a lock-free, single-writer/multi-reader table that every window's rendering thread can read without locking (TECHNICAL.md Section 8 specifies the concurrency primitive). This is a purpose-built structure, not a general concurrent hash map -- see TECHNICAL.md Section 8 for why a hash-map swap alone does not solve this problem.

## 11. Color Management, Gamma & HDR / Wide-Gamut Pipeline

High-density visual interfaces require strict color correctness, gamma-accurate linear compositing, and dynamic support for high-brightness, wide-gamut displays.

### 11.1 sRGB to Linear Blending Pipeline

* **Gamma-Correct Blending:** All color inputs specified by the UI framework in $sRGB$ space are automatically converted into Linear color space prior to alpha compositing in shader pipelines. The canonical piecewise conversion formula is defined once in TECHNICAL.md Section 6.2 (added as part of the September 2026 documentation review's de-duplication pass) — refer there rather than re-deriving it per document.

* **Fringe Elimination:** Blending transparency in Linear space eliminates dark fringe artifacts along semi-transparent vector edges, text glyph outlines, and rounded rectangle anti-aliased borders.

### 11.2 High Dynamic Range (HDR) & Wide Color Gamut (Display P3 / HDR10)

* **Extended Gamut Colorspaces:** Full native support for standard sRGB, DCI-P3, and Extended Rec. 2020 color spaces.

* **Floating-Point Swapchains:** Supports modern 10-bit ($RGBA10A2$) and 16-bit floating-point ($RGBA16F$) swapchain pixel formats, preventing color banding across rich UI gradients.

* **Tone Mapping & Dynamic Headroom:** Exposes display brightness metadata hooks to scale specular UI elements (e.g., audio meter peaks, HDR video preview frames, high-brightness indicators) into Extended Dynamic Range (EDR/HDR) presentation headroom. This is a UI engine, not a photo/film/video-editing tool: the default tone-mapping curve leaves every standard (at-or-below-white) UI color mathematically unchanged and only compresses the genuinely-HDR content above white -- a cinematic filmic curve is deliberately *not* the default, since it would visibly shift exact brand/UI colors. See TECHNICAL.md Section 6.3 for the canonical formula and rationale.

## 12. Animation, Event Loop & Vector Math Engine

Smooth motion, dynamic layouts, multi-window clocks, and user interactions require tight coupling between frame clocks, continuous vector math, and interpolation pipelines.

### 12.1 High-Precision Frame Clock & Timing

* **Microsecond Monotonic Timer:** Hardware-backed high-resolution counter (`std::chrono::high_resolution_clock` or platform ticks like `QueryPerformanceCounter` / `clock_gettime`) providing sub-microsecond timestamp precision $t_{\text{frame}}$.

* **Decoupled Physics & Smooth Interpolation:** Supports fixed-step simulation ticks ($t_{\text{tick}} = 120\text{ Hz}$ or $240\text{ Hz}$) with render time alpha blending:

  $$
  \alpha = \frac{t_{\text{current}} - t_{\text{last\_tick}}}{\Delta t_{\text{fixed}}}
  $$

### 12.2 Vector Math Subsystem

Optimized for SIMD acceleration (AVX2 on x86_64, NEON on ARM64) to handle high-frequency transformation tree updates across multi-window hierarchies:

* **Vector Primitives:** 2D/3D vectors ($Vec2$, $Vec3$, $Vec4$), bounding boxes ($AABB2D$), color transformations ($sRGB \leftrightarrow \text{Linear}$).

* **Transformation Matrices:** $3 \times 3$ Affine Transformation Matrices ($Mat3x2$ / $Mat3x3$) supporting continuous hierarchy matrix stacking:

  $$
  \mathbf{M}_{\text{world}} = \mathbf{M}_{\text{parent}} \times \mathbf{T}(x,y) \times \mathbf{R}(\theta) \times \mathbf{S}(s_x, s_y) \times \mathbf{K}(k_x, k_y)
  $$

  where $\mathbf{T}$ is Translation, $\mathbf{R}$ is Rotation, $\mathbf{S}$ is Scaling, and $\mathbf{K}$ is Skew.

### 12.3 Motion & Easing Engine

* **Cubic Bézier Interpolation:** Analytical evaluation of parametric Bézier curves $B(t) = (1-t)^3 P_0 + 3(1-t)^2 t P_1 + 3(1-t) t^2 P_2 + t^3 P_3$ for custom easing curves (e.g., `ease-in-out`).

* **Harmonic Spring Dynamics:** Frame-rate independent physical spring-mass-damper models parameterizable by stiffness $k$, damping ratio $\zeta$, and mass $m$:

  $$
  F_{\text{spring}} = -k (x - x_{\text{target}}) - c v
  $$

* **Frame-Rate Independent Lerp Decay:** Guarantees uniform motion convergence under variable frame updates:

  $$
  x(t + \Delta t) = x_{\text{target}} + (x(t) - x_{\text{target}}) \cdot e^{-\lambda \Delta t}
  $$

### 12.4 Animation State Ownership

Added in the September 2026 documentation review. Spring and lerp-decay state ($x$, $v$, $x_{\text{target}}$ per animated property) is owned and persisted by the UI framework's node/widget tree, not by the `Canvas` or the rendering engine. The Vector Math Engine is a stateless evaluation library: it is handed current state and $\Delta t$ each tick and returns the next state, but stores nothing itself between frames. This keeps the engine's per-frame memory bounded and predictable and avoids a second, engine-side source of truth for animation state that could drift from the UI framework's own model.

## 13. Visual Pipeline & Render Pass Breakdown

Every frame processed by the engine moves through eight distinct conceptual stages per active window context:

1. **CPU/GPU Synchronization & Resource Acquisition:** The CPU waits on hardware fences to ensure the GPU has finished reading the ring-buffer segment for the upcoming frame index. Staging buffers are prepared, transient render targets are recycled from the pool, and the swapchain image is acquired.

2. **Event Draining & Motion Ticks:** OS events across all windows are popped from the event queue by the UI framework; animation timelines, dynamic SVG keyframes, spring physics, and time updates are evaluated via the vector math engine.

3. **UI Framework Layout & Multi-Threaded Canvas Recording:** The UI framework computes widget layouts, evaluates scene graph matrices, and issues high-level drawing primitives (`draw_rect`, `draw_text`, `draw_path`, `draw_svg`, `push_layer`, `push_clip_rect`, `begin_overlay`, `tag_accessibility_node`) across main and worker thread sub-canvases into the active window's `RenderingCanvas`.

4. **Sub-Canvas Stitching & Layer Pass Resolution:** Parallel sub-canvas streams are merged lock-free into the main IR array; offscreen compositing layers (`PushLayer`) allocate transient render targets from the pool and setup visual filter passes (e.g., backdrop blurs).

5. **Tessellation, Atlas & Accessibility Tagging Phase:** Non-standard vector paths and animated SVG geometries are converted into low-index triangle primitives; missing text glyphs are dynamically rasterized into MSDF texture atlases (triggering LRU eviction if capacity thresholds are reached); spatial bounding metadata is harvested for the OS Accessibility bridge.

6. **Sorting, Flattening & Batching Phase:** Intermediate commands recorded by the `Canvas` are encoded with 64-bit sort keys ($Key = \{Overlay, Layer, Depth, PipelineState, TextureID\}$), radix-sorted to group similar primitive types, and flattened into contiguous index buffers to approach single draw dispatches per layer plane.

7. **Buffer Packing & Color Conversion Phase:** Contiguous dynamic vertex and index memory are mapped directly into ring buffers for zero-copy GPU access, with colors converted to linear space or transformed for HDR display profiles.

8. **Driver Submission & Presentation:** RHI translates batched command streams into explicit graphics command lists (e.g., `vkCmdDrawIndexed`, `ID3D12GraphicsCommandList::DrawIndexedInstanced`) and dispatches them to the hardware queue, signaling fences and presenting the swapchain.
