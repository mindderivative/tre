# Rendering Engine Technical Requirements & Specifications

## 1. Performance & Latency Budgets

To satisfy ultra-responsive user interfaces across high-refresh displays ($60\text{ Hz}$ to $240\text{ Hz}$), the rendering engine enforces strict runtime numerical budgets.

| **Metric** | **Target Limit (Typical)** | **Hard Maximum (Worst-Case)** |
|---|---|---|
| **Frame Delivery Target** | $240\text{ Hz}$ ($4.16\text{ ms}$) | $60\text{ Hz}$ ($16.66\text{ ms}$) | 
| **CPU Frame Processing Time** | $\le 0.50\text{ ms}$ | $1.00\text{ ms}$ | 
| **GPU Execution Time** | $\le 2.00\text{ ms}$ | $4.00\text{ ms}$ | 
| **Active Steady-State Allocations** | $0\text{ bytes/frame}$ | $0\text{ bytes/frame}$ | 
| **Dynamic VRAM Footprint** | $\le 128\text{ MB}$ | $256\text{ MB}$ (excluding HDR targets) | 
| **Draw Call Reduction Ratio** | $\ge 90\%$ batching efficiency | $80\%$ | 

## 2. Hardware & Platform Requirements

### 2.1 Backend Graphics API Standards

* **Vulkan:** Version 1.2+ minimum.

  * Required Extensions: `VK_KHR_dynamic_rendering`, `VK_EXT_descriptor_indexing` (for bindless textures).

  * Hardware Requirements: Push constants ($128\text{ bytes}$ minimum), scalar block layout support.

  * **Implementation status (Phase 2 Step 2.1, 2026-09-05):** `VK_EXT_descriptor_indexing` is a real, enforced hard requirement in `tre-rhi-vulkan` (`REQUIRED_DEVICE_EXTENSIONS`), not gracefully degraded like the debug-only validation layer (TECHNICAL.md Section 9.2). `VulkanDevice::new` requests `shaderSampledImageArrayNonUniformIndexing`, `descriptorBindingSampledImageUpdateAfterBind`, `descriptorBindingPartiallyBound`, `descriptorBindingVariableDescriptorCount`, `descriptorBindingUpdateUnusedWhilePending`, and `runtimeDescriptorArray`, and creates one persistent descriptor set: a fixed shared sampler at binding 0, an unbounded `SAMPLED_IMAGE` array at binding 1 (`VARIABLE_DESCRIPTOR_COUNT` must be on the highest-numbered binding, per spec -- a real bug caught by the validation layer during this step's own development, see `planning/archive/LOG_PHASE2_STEP2_1.md`). The array's real size is `min(4096, maxDescriptorSetUpdateAfterBindSampledImages)`, clamped at runtime, not assumed. The set is bound once per pipeline bind; selecting a texture is a per-draw-call push constant (`RhiCommandBuffer::bind_texture`), not a per-vertex index -- see `planning/archive/PLAN_PHASE2_STEP2_1.md` for why per-vertex indexing is explicitly deferred to Phase 3/4.

* **DirectX 12:** Feature Level 12_0 minimum.

  * Required Features: Resource Binding Tier 3 (for unbounded descriptor tables/bindless).

* **Metal:** Metal 2.4+. *(Verify the exact macOS floor against Apple's current Metal Feature Set tables before implementation -- Argument Buffers Tier 2 requires a substantially newer macOS release than the original Metal 2.0 launch on 10.14 Mojave; do not build against an unverified version pairing.)*

  * Required Features: Argument Buffers Tier 2.

### 2.2 Host Architecture & CPU Requirements

* **CPU Architectures:** 64-bit OS required. x86_64 (AVX2 enabled) and ARM64 (NEON instruction set).

* **SIMD Requirements:** Vector math, matrix multiplication, and path tessellation must utilize 128-bit/256-bit SIMD. The primary implementation path is the [`wide`](https://docs.rs/wide) crate's portable vector types (`f32x4`, `f32x8`), which select the appropriate SSE2/AVX2 backend on x86_64 or NEON backend on ARM64 at compile time, transparently emulating `f32x8` as two `f32x4` NEON operations on targets with no native 256-bit width -- this gives the engine one shared code path across architectures instead of hand-maintained duplicate intrinsic implementations per platform (see Section 5.4). Raw `core::arch::{x86_64, aarch64}` intrinsics behind runtime feature detection (`is_x86_feature_detected!` / `is_aarch64_feature_detected!`) are a fallback only for an operation `wide` doesn't expose, not the default path.

* **Memory Alignment Rules:**

  * All dynamic CPU ring buffers must maintain $64\text{ bytes}$ cache-line alignment to prevent false sharing across thread boundaries.

  * Uniform dynamic offsets sent to the RHI must comply with standard $256\text{ bytes}$ alignment requirements ($Alignment_{\text{min}} = 256\text{ bytes}$).

### 2.3 OS Windowing & Accessibility APIs

* **Windows:** Win32 API (`HWND`), DXGI for swapchains, and UI Automation (UIA) COM interfaces for accessibility.

* **Linux:** Wayland primary (`wl_surface`, `xdg_shell`, `zwp_text_input_v3`), X11 fallback via XCB. Accessibility bridged via AT-SPI2 over D-Bus.

* **macOS:** Cocoa/AppKit (`NSWindow`, `CAMetalLayer`), NSAccessibility protocols.

## 3. Memory Subsystem & Zero-Allocation Strategy

To achieve the $0\text{ byte}$ dynamic allocation rule during the active frame tick, the engine utilizes strict pre-allocation strategies.

### 3.1 Triple-Buffered CPU/GPU Ring Arenas

```
+-------------------------------------------------------------------------+
|                  CPU-Visible Ring Buffer (Host Coherent)                |
|  +--------------------+--------------------+-------------------------+  |
|  | Frame N (In Read)  | Frame N+1 (Mapped) | Frame N+2 (Reserved)    |  |
|  +--------------------+--------------------+-------------------------+  |
+-------------------------------------------------------------------------+


```

* **Capacity:** $16\text{ MB} - 32\text{ MB}$ total capacity.

* **Mapping:** CPU maps the target segment asynchronously via `Write-Combined` memory and writes sequential geometry/command data.

* **Fencing:** CPU threads must wait on a hardware `Fence` before writing to a ring buffer segment to ensure the GPU has finished reading previous frames.

### 3.2 Transient Render Target Pool

* Requests for offscreen buffers (e.g., `Canvas::push_layer`) must *never* invoke dynamic RHI texture creation during the render loop.

* **Pool Mechanism:** A hash-map-backed pool of pre-allocated GPU render targets keyed by `(Width, Height, Format)`, with widths and heights rounded up to fixed bucket boundaries so nearby requests share a pool entry rather than each unique pixel dimension demanding its own. The pool uses `FxHashMap` (the `rustc-hash` crate) or `ahash`'s `AHashMap` in place of `std::collections::HashMap`'s default SipHash -- these keys are internal, engine-generated values with no untrusted input reaching them, so SipHash's DoS resistance buys nothing here and only costs cycles on a per-`push_layer` hot-path lookup. Targets requested are checked out and returned to the pool upon `Canvas::pop_layer`; see DESIGN.md Section 2.6 for the pool-miss fallback.

### 3.3 Generational Garbage Collection & Deferred Release

* Resources (textures, meshes) track usage via a `u64 last_frame_used` timestamp.

* **LRU Threshold:** When VRAM pool capacity hits $85\%$, resources unused for $N \ge 600$ frames are marked for eviction.

* **Deferred Release Queue:** Evicted resources are pushed to a lock-free queue and physically destroyed only after $N_{\text{current}} - N_{\text{evicted}} > 3$ frames to guarantee they are no longer in-flight on the GPU.

* **Implementation status (Phase 2 Step 2.3, 2026-09-06):** real and verified in `tre-rhi-vulkan`, against the one dynamic-VRAM resource that exists today (the transient render-target pool, Section 3.2) -- the atlas/SVG-cache targets this section originally describes don't exist yet (Phase 3/4/5), and will plug into the same mechanism once they do. Genuinely asynchronous: a real background `std::thread` (the engine's first) scans `TransientPool` roughly every 100ms and evicts entries older than the $N = 600$ frame threshold once `TransientPool::total_free_bytes` crosses 85% of Section 1's $128\text{ MB}$ dynamic-VRAM-footprint target -- but that thread never calls a single Vulkan function; it only locks plain Rust state and moves values into a queue. The actual `vkDestroy*` calls, gated by the $N_{\text{current}} - N_{\text{evicted}} > 3$ grace period, happen on the main thread, in `VulkanDevice::begin_frame`, alongside the existing transient-pool-growth check. One deliberate deviation from this section's "lock-free queue": the deferred-release queue is a plain `Mutex<VecDeque<_>>`, not a lock-free structure -- contention is negligible at this call frequency, and a `Mutex` makes peeking the front entry without consuming it trivial, which the grace-period check needs and `tre_memory::SpscRingBuffer`'s `pop`-only API doesn't support safely. See `planning/archive/PLAN_PHASE2_STEP2_3.md` for the full reasoning. A dedicated second review pass (2026-09-06) found and fixed a real use-after-destroy on shutdown (the deferred-release queue wasn't cleared before device teardown, unlike the transient pool itself), added `saturating_sub` throughout the byte accounting, and added a real (if imperfect -- idle bytes only, not total) admission-side cap on pool growth, since the GC alone can only reclaim idle entries, not gate new ones. See `documentation/REVIEW.md`'s "Phase 2 Step 2.3 Code Review" section.

### 3.4 Zero-Allocation Enforcement

Added in the September 2026 documentation review. The $0\text{ bytes/frame}$ budget (Section 1) is a hard constraint, not an aspiration, and must be mechanically enforced rather than assumed:

* **Debug/Profile Build Guard:** In debug and profiling build configurations, a custom `#[global_allocator]` wrapper around the system allocator checks a thread-local "render tick active" flag set by a scope guard at the start of `RenderingCanvas` recording and cleared after RHI submission. Any allocation observed while the flag is set triggers an immediate assert with a captured call stack. (Corrected in the Phase 2 Step 1 documentation pass: earlier revisions of this section described this as overriding `operator new`/`operator delete`/`malloc`/`free`, leftover C++ phrasing from before the Rust migration -- Rust has no equivalent operator to override, and the actual mechanism is the standard library's `GlobalAlloc` trait.)
* **CI Gate:** The headless CI performance regression suite (Section 9.2) runs every commit's render loop under this guard and fails the build on any violation -- the same gate that checks the $\le 0.50\text{ ms}$ CPU budget must also check for zero allocation events, since a passing timing budget with an undetected allocation is a false pass.
* **Release Build Behavior:** The guard and its thread-local check compile out entirely in release builds (`cfg(not(debug_assertions))`, Rust's equivalent of C's `NDEBUG`-gated code) to avoid any steady-state overhead from the enforcement mechanism itself.

## 4. Draw Sorting & 64-Bit Batching Key

Command sorting uses a strictly defined 64-bit integer key evaluated via a 4-pass Radix Sort ($\mathcal{O}(N)$). The canonical bit-field breakdown, rationale, and batch-flattening rules are defined once in ARCHITECTURE.md Section 4.1 (added in the September 2026 documentation review to remove drift risk from four independent restatements) -- this section states only the numeric budget each field must satisfy.

* **Layer ID -- 16 bits:** up to 65,536 layers; standard content uses $0$-$9999$, overlay/modal content uses $\ge 10000$.
* **Pipeline ID -- 16 bits:** up to 65,536 distinct shader pipeline states.
* **Texture/Bindless ID -- 12 bits:** up to 4,096 concurrently bound atlas/bindless slots -- ample headroom over the low dozens of atlases the engine actually maintains.
* **Depth ID -- 20 bits:** up to 1,048,576 submission-order slots per frame. Widened from 16 to 20 bits in the September 2026 documentation review: 16 bits (65,536 slots) left only a 6.5x margin over the Architectural Decision Matrix's stated $>10{,}000\text{ node}$ target, with no documented overflow behavior. A debug-build assert fires if a frame's node count would overflow this field; see ARCHITECTURE.md Section 4.1 for the overflow fallback.

## 5. Primitive & Vector Pipeline Specifications

### 5.1 Compressed Vertex Format

To maximize PCIe bandwidth, the primary UI vertex structure is tightly packed into $32\text{ bytes}$ -- that size is the hard budget this document owns; the canonical field-by-field struct definition lives in ARCHITECTURE.md Section 3.1 (added in the September 2026 documentation review to remove drift risk from three independent restatements). A compile-time `static_assert(sizeof(UiVertex) == 32)` enforces the budget across all target compilers.

### 5.2 Analytical Rounded Rectangles

* Rendered via Signed Distance Fields (SDF) evaluated in the fragment shader to avoid CPU tessellation.

* **Formula Implementation:** $d(\mathbf{p}) = \Vert{}\max(\mathbf{q}, 0)\Vert{} + \min(\max(q_x, q_y), 0) - r$

* **Vertices:** Always emits exactly $4\text{ vertices}$ ($6\text{ indices}$) per rectangle, regardless of corner radius complexity.

* **Implementation status (Phase 3 Step 3.2, 2026-09-06):** real, in a dedicated `sdf_rounded_rect.{vert,frag}` shader pair (`crates/tre-rhi-vulkan/shaders/`). $\mathbf{q}$ in the formula above is $\mathbf{q} = |\mathbf{p}| - \mathbf{b} + r$, the standard box-SDF construction (Inigo Quilez), where $\mathbf{b}$ is the box's own half-extent (not shrunk by $r$) and $\mathbf{p}$ is the fragment's position relative to the rect's center. `RenderingCanvas::draw_rounded_rect` supplies $\mathbf{p}$ by repurposing `UiVertex::uv` (Section 5.1's "Texture coordinates or SDF bounds" field) as each corner's center-relative offset, in the same pixel units as `position` -- linear interpolation across the quad's two triangles reproduces the exact $\mathbf{p}$ at every fragment. `UiVertex::params` carries $(r, b_x, b_y)$. Only a single uniform corner radius is supported (clamped to `[0, min(b_x, b_y)]` at construction); independent per-corner radii are a real, separate technique deferred until DESIGN.md's `CornerRadii`-taking `Canvas` API exists to need them. Verified by reading back real rendered pixels (`crates/tre-rhi-vulkan/examples/sdf_rounded_rect_demo.rs`): exact foreground at the interior, exact background outside the rounding arc, and a genuine partial-alpha blend confirmed near the rounded corner specifically -- a perfectly axis-aligned *flat* edge's entire 1px AA ramp can fall exactly between two pixel centers with no fractional-coverage sample inside it, so a flat edge is not a reliable place to look for one.

### 5.3 Text & MSDF Engine

* **Atlas Generation:** Glyphs are rasterized into a Multi-channel Signed Distance Field (MSDF) at a fixed $32 \times 32\text{ pixel}$ resolution.

* **Atlas Format:** $RGB8$ for MSDF channels, $R8$ for standard fallback glyphs.

* **Shader Evaluation:** Anti-aliasing calculated dynamically using screen-space derivatives (`fwidth`):

  $$
  \text{sigDist} = \text{median}(R, G, B) - 0.5
  $$

  $$
  \text{opacity} = \text{clamp}\left(\frac{\text{sigDist}}{\Vert{}\nabla(\text{sigDist})\Vert{}} + 0.5, 0.0, 1.0\right)
  $$

  *This is the canonical MSDF opacity formula -- IMPLEMENTATION.md Section 4.2 references it rather than restating it.*

* **Implementation status (Phase 4 Step 4.1, 2026-09-06):** the upstream half of this section -- shaping, fallback, and outline extraction, feeding whatever this section's own MSDF rasterizer eventually consumes -- is real, in a new `tre-text` crate, built as an all-pure-Rust font stack rather than the literal HarfBuzz/FreeType C libraries IMPLEMENTATION.md Step 4.1 names (`rustybuzz`, a faithful port of HarfBuzz's own shaping algorithm, and `skrifa`, Google Fonts' `fontations` project, in place of each respectively -- see `planning/archive/PLAN_PHASE4_STEP4_1.md`). Bidi + script run segmentation (`unicode-bidi`/`unicode-script`) is hand-rolled at this layer since `rustybuzz::shape` itself only shapes one already-uniform run; font fallback is a real `fontconfig`-driven cascade (Linux only this step); outline extraction returns raw, unscaled control points in the same `MoveTo`/`LineTo`/`QuadTo`/`CurveTo`/`Close` shape `FT_Outline_Decompose` would have produced. This section's own MSDF generation, atlas packing, and the opacity formula above are still not built -- Step 4.2.

### 5.4 SVG Path Tessellation & Morphing

* **Tessellation Constraints:** Static paths must be triangulated (via ear-clipping or trapezoidal mapping) and cached into a read-only vertex pool to prevent per-frame re-tessellation CPU overhead.

* **Dynamic Morphing:** Keyframed SMIL/CSS path morphing evaluates topological interpolation using the `wide` crate's `f32x8` vector type (Section 2.2) before pushing updated vertices to the dynamic ring buffer. On x86_64 with AVX2 this compiles to genuine 256-bit operations; on ARM64, where NEON has no native 256-bit width, `wide` transparently emulates `f32x8` as a pair of 128-bit NEON operations -- the algorithm's source code is identical on both architectures, with the performance difference (not a correctness or code-path difference) confined entirely inside the `wide` crate's backend selection.

* **Implementation status (Phase 3 Step 3.3.1, 2026-09-06):** the ear-clipping half of the tessellation constraint above is real, in a new `tre-svg` crate. SVG DOM/XML parsing itself uses the `usvg` crate (not hand-rolled) -- it resolves `<use>`/`<g>`/CSS and converts every shape to absolute-coordinate path data (arcs already converted to cubic Beziers) but performs no rasterization, so this project's own tessellation work starts exactly where `usvg`'s ends. Curve flattening (cubic/quadratic Bezier to polyline, via tolerance-based recursive de Casteljau subdivision) and the ear-clipping triangulator itself are both hand-rolled. A correct ear-clipping validity check needs BOTH "no remaining vertex strictly inside the candidate ear triangle" AND "no remaining edge properly crosses the ear's diagonal" -- two real bugs found via a non-convex five-pointed star demo (neither caught by simpler convex-shape unit tests) showed that either check alone has a genuine blind spot the other closes: a vertex can sit exactly on an edge without being strictly "inside" a triangle while an edge through it still crosses the boundary, and conversely a vertex can lie fully inside a triangle while both of its own edges terminate exactly at two of that triangle's corners, never registering as a "proper" crossing. Handles only simple, single-contour, non-self-intersecting fills -- true holes, self-intersecting paths (`EvenOdd` fill rule), path morphing (this section's other half), and the stencil-and-cover fallback are all deferred to later sub-steps (IMPLEMENTATION.md Step 3.3.2/3.3.3).

* **Implementation status (Phase 3 Step 3.3.2, 2026-09-06):** the dynamic morphing half above is real -- `tre_math::lerp_points_batch` (not `tre-svg`; the primitive is domain-agnostic SIMD point interpolation, mirroring `Affine2::compose_batch`'s exact 8-wide-chunk-plus-scalar-remainder structure) interpolates two equal-length `[f32; 2]` slices via `wide::f32x8::mul_add`. "Topological equivalence between keyframes" is enforced at the `tre-svg::morph` layer, one level above the raw SIMD primitive: for already-flattened `Polygon`s (curves are gone by that point), it means equal vertex counts, checked and reported via `Result` (`SvgError::TopologyMismatch`) rather than the raw primitive's own contract, which panics on a length mismatch like `compose_batch` does -- a mismatch reaching the SIMD primitive itself would be a programmer error, since `morph` is expected to have already validated it. Mismatched-vertex-count keyframes are rejected, not automatically resampled to reconcile them (a real, separate arc-length-resampling technique, not yet built). Verified against two independently-parsed, straight-line-only SVG keyframes with a real per-vertex SIMD interpolation, re-triangulated fresh at each `t` since the shape's geometry genuinely changes every frame even though curve flattening does not repeat.

* **Implementation status (Phase 3 Step 3.3.3, 2026-09-06):** the self-intersecting-path/`EvenOdd` case named above is real -- a two-pass stencil-and-cover technique (`VulkanDevice::create_stencil_and_cover_pipelines`), supporting both `NonZero` and `EvenOdd` fill rules (`tre_engine::FillRule`). Stencil support is now a permanent part of the shared per-frame RHI surface: every swapchain owns its own stencil image (sized to its own extent), `begin_frame` always attaches it, and every pipeline (including ordinary flat-color/SDF ones) declares a compatible stencil format, matching the "declared everywhere, unused where not referenced" precedent already used for the bindless descriptor set. `tre-svg`'s `fan_triangles`/`bounding_box` supply the CPU-side geometry -- an anchor-based fan that makes no validity assumption at all, unlike `triangulate`, since overlap and self-intersection are exactly what the GPU's stencil accumulation resolves correctly. Fixing this step's own verification demo surfaced a genuine gap in Step 3.3.1's `triangulate`: its ear-validity checks only ever compare a candidate diagonal against the boundary still remaining *during* clipping, which does not by itself guarantee catching every self-intersecting *original* polygon (a classic pentagram clipped cleanly with no diagonal ever conflicting, silently producing a wrong triangulation instead of an error) -- fixed with an explicit, global `has_self_intersection` pre-check run once before clipping starts.

## 6. Color Management & HDR Specifications

### 6.1 Swapchain Formats

* **Standard Dynamic Range (SDR):** `VK_FORMAT_B8G8R8A8_SRGB` / `DXGI_FORMAT_B8G8R8A8_UNORM_SRGB`.

* **High Dynamic Range (HDR) / Wide Gamut:** 16-bit float swapchains required for zero-banding gradients and extended headroom. `VK_FORMAT_R16G16B16A16_SFLOAT` / `DXGI_FORMAT_R16G16B16A16_FLOAT`.

### 6.2 Linear Compositing

* The UI Framework provides colors in $sRGB$. The RHI pipeline must convert these to $Linear$ space *before* blending to eliminate dark transparency fringes.

* **Conversion Formula:**

  $$
  C_{\text{linear}} = \begin{cases} \frac{C_{\text{srgb}}}{12.92}, & C_{\text{srgb}} \le 0.04045 \\ \left(\frac{C_{\text{srgb}} + 0.055}{1.055}\right)^{2.4}, & C_{\text{srgb}} > 0.04045 \end{cases}
  $$

*This is the canonical sRGB <-> Linear conversion formula -- DESIGN.md Section 11.1 and IMPLEMENTATION.md Section 7.1 reference it rather than restating it.*

### 6.3 HDR-to-SDR Tone Mapping

This is a UI engine, not a photo, film, or video-editing application: standard (at-or-below-white) UI content -- buttons, panels, text, brand colors -- must reach the screen bit-for-bit identical to its authored sRGB value. Only the content DESIGN.md Section 11.2 explicitly calls out as intentionally exceeding standard white (audio meter peaks, HDR video preview frames, high-brightness indicators) needs compressing into the display's actual headroom.

**Canonical formula** (identity at or below white; Reinhard-style compression of the excess above it), for linear light value $L$ where $L = 1.0$ is standard SDR white:

$$
f(L) = \begin{cases} L, & L \le 1.0 \\[4pt] 1.0 + \dfrac{L - 1.0}{1 + (L - 1.0)/W}, & L > 1.0 \end{cases}
$$

$W$ is the display's reported HDR headroom in SDR-white multiples (e.g., $W = 3.0$ for a display capable of 3x SDR peak brightness), read at runtime from the brightness metadata hooks DESIGN.md Section 11.2 already exposes -- never a fixed constant. The function is continuous and monotonic across the $L = 1.0$ boundary (no visible seam between standard and HDR content), and as $L \to \infty$, $f(L) \to 1.0 + W$: output asymptotically approaches but never hard-clips at the display's actual headroom, avoiding both banding and an abrupt ceiling.

*Why not ACES:* ACES filmic tone mapping reshapes contrast and desaturates highlights across the *entire* input range, including everything at or below white -- correct for cinematic footage, where no pixel is supposed to reach the screen as an exact, specific value, but wrong for a UI engine, where a button's authored color must not shift. ACES (or another filmic curve) remains available as an explicit, opt-in per-`Canvas` style choice for creative-workstation/DAW integrations (DESIGN.md Section 3) that specifically want it for embedded video/image preview content, but it is never the default tone-mapping path.

*This is the canonical HDR tone-mapping formula -- DESIGN.md Section 11.2 and IMPLEMENTATION.md Section 7.1 reference it rather than restating it.*

## 7. Math & Timing Specifications

### 7.1 Microsecond Monotonic Clock

* The engine must use hardware-backed, monotonically increasing timers (e.g., `QueryPerformanceCounter` on Windows, `clock_gettime(CLOCK_MONOTONIC)` on Linux).

* Time delta ($\Delta t$) passed to the animation evaluation tick must have precision to at least $1 \mu s$ ($0.000001\text{ s}$).

### 7.2 SIMD Affine Matrix Hierarchy

Transforms use $3 \times 3$ affine matrices. Given 2D translation $(t_x, t_y)$, rotation $(\theta)$, and scale $(s_x, s_y)$:

$$
\mathbf{M} = \begin{bmatrix} s_x \cos\theta & -s_y \sin\theta & t_x \\ s_x \sin\theta &  s_y \cos\theta & t_y \\ 0 & 0 & 1 \end{bmatrix}
$$

Matrix multiplications for parent-child world transforms must be batched and executed via the `wide` crate's portable SIMD types (Section 2.2) during the scene graph flattening phase.

* **Implementation status (Phase 3 Step 3.1, 2026-09-06):** real in `tre-math`. `Affine2` stores exactly the six meaningful values of the matrix above (the bottom row is always `[0, 0, 1]` for any genuine affine transform, so storing it would waste memory and SIMD lanes for nothing) -- constructible from translation/rotation/scale individually or combined in one call matching the formula above exactly. `compose_batch` processes 8 parent-child pairs at a time via `wide::f32x8::mul_add`, with a scalar fallback for the remainder, writing into a caller-provided output slice rather than allocating (TECHNICAL.md Section 1's zero-allocation steady state). No scene-graph tree exists yet to call it with real parent-child data -- this step builds and proves the primitive against synthetic test data (11 unit tests, including a SIMD-vs-scalar-reference comparison across every remainder length relative to the 8-wide chunk size), matching this project's established "build the tested primitive before its exact consumer exists" precedent. Verified on x86_64/AVX2 only; the NEON (ARM64) path compiles from the same source but isn't run on real ARM hardware here.

## 8. Threading & Concurrency Limits

* **Event Queue:** Single-Producer Single-Consumer (SPSC) lock-free ring buffer for OS input events. Corrected in the September 2026 documentation review: the engine has exactly one consumer (the UI framework's logic tick draining the queue, per DESIGN.md Section 5.1). SPSC is simpler and faster than SPMC -- no consumer-side CAS races or ABA hazards -- and should only be upgraded to SPMC if a second, genuinely independent consumer thread is added, at which point that consumer and its purpose must be named here explicitly.

  *Implemented as of Phase 1 Step 2 (2026-09-05): the generic, allocation-once-at-construction primitive is `tre_memory::SpscRingBuffer<T>`. `tre-platform`'s Wayland/X11 backends push translated events into `tre_engine::InputEventQueue`, a producer-side wrapper around a `SpscRingBuffer<InputEvent>` that also owns pointer-move coalescing (the staged, not-yet-published value lives in a plain struct field, never in an already-published ring slot, so the coalescing logic stays sound if a real second consumer thread is introduced later without needing a redesign). `PlatformConnection::poll_events` is this step's non-blocking drain -- producer and consumer are still the same call stack for now (`IMPLEMENTATION.md` Step 1.2's scope decision defers genuine thread separation), so real concurrent access to the ring buffer is exercised only by `tre-memory`'s own unit tests, not by this integration.*

* **Canvas Recording:** Worker threads can spawn `SubCanvas` instances. Max concurrent sub-canvases constrained to `std::thread::available_parallelism()` minus one. (`std::thread::hardware_concurrency()` is the C++ name for this query; Rust's equivalent in `std::thread` is `available_parallelism`, stabilized since Rust 1.59 and returning an `io::Result<NonZeroUsize>`.)

* **Lock-Free Aggregation:** Sub-canvases are stitched into the main command arena using atomic pointer increments (`AtomicUsize::fetch_add`) to reserve output space in the global intermediate representation array, requiring zero mutex locks.

* **Multi-Window Atlas Concurrency:** DESIGN.md Section 10.3 requires that no window ever blocks on another window's atlas insertion. Two lock-free primitives implement this:

  * A bounded, pre-allocated **MPSC (multi-producer, single-consumer) ring buffer** carries atlas-insertion requests from any window's tessellation phase (producers) to the single atlas owner (the sole consumer). This generalizes the SPSC ring buffer already used for OS input events (above) to the multi-producer case, since here multiple window threads genuinely are independent producers, unlike the input-event queue's single OS-event-pump producer.
  * A fixed-capacity **single-writer/multi-reader (SWMR) open-addressed table** publishes completed atlas coordinates: the atlas owner is the only writer (`Ordering::Release` store into a slot), and any window's rendering thread reads (`Ordering::Acquire` load) without ever taking a lock or performing a CAS. This is a purpose-built structure, not a general-purpose concurrent hash map (e.g. not `dashmap`) -- it only ever adds entries (never removes them in place; eviction is handled by the existing generation-counter/deferred-release mechanism, Section 3.3), which is exactly the access pattern a plain atomic-publish table is sufficient for. Swapping the *hasher* used elsewhere for hot-path lookups (`FxHashMap`/`ahash`, Section 3.2) does not solve this problem by itself -- it only makes single-threaded lookups faster, not concurrent ones safe.

## 9. Toolchain & Development Environment

### 9.1 Language & Compiler Targets

* **Rust Edition:** 2021 minimum, MSRV (Minimum Supported Rust Version) pinned at 1.75+ via `rust-toolchain.toml`. Strictly leveraging `const` generics and compile-time `const fn` evaluation for memory alignments, and `#[repr(C)]` layouts for every type that crosses the FFI boundary (Section 9.4).

* **No dynamic type inspection in hot paths:** no `std::any::Any` downcasting or reflection-style type inspection within the per-frame rendering loop -- the Rust-equivalent of the prior "no RTTI" rule. The Section 6 (ARCHITECTURE.md) `dyn Trait` exception is for RHI method dispatch only, never for runtime type identification.

* **No unwinding past the FFI boundary:** A Rust panic must never propagate past an `extern "C"` function back into the calling UI framework (Section 9.4) -- doing so is undefined behavior per the Rust reference. This requires the *opposite* of the commonly-recommended `panic = "abort"` release-profile setting: the `tre-ffi` crate and its entire dependency graph build with the default `panic = "unwind"` strategy specifically so that `std::panic::catch_unwind` has stack unwinding available to catch. Every exported `extern "C"` entry point wraps its body in `catch_unwind`, converting any caught panic into an `EngineError` result code (DESIGN.md Section 2.6) before returning to the caller. `panic = "abort"` must never be set on any profile used to build the shipped `cdylib`/`staticlib` -- it would make every `catch_unwind` call a silent no-op, turning a recoverable panic back into a process-terminating crash and defeating the entire purpose of this rule.

* **`unsafe` policy:** `unsafe` is permitted only inside the RHI backend crates (raw graphics API FFI), the ring-buffer/arena allocators (Section 3, including the atlas's MPSC/SWMR concurrency primitives in Section 8), the `tre-ffi` crate (raw handle/pointer conversion and manual buffer-ownership transfer across the C-ABI boundary, Section 9.4), and the `tre-platform` crate (added Phase 1 Step 1: `raw-window-handle` construction, and XCB FFI via `x11rb`'s `allow-unsafe-code` feature for a real `xcb_connection_t*`). Every other crate in the workspace, including `tre-engine` itself, carries `#![forbid(unsafe_code)]`, so the four permitted locations above are the complete, closed set -- not merely the ones called out for emphasis. The vector-math/SIMD crate (Section 2.2) is *not* on this list: since it is built on the `wide` crate's safe public API rather than raw `core::arch` intrinsics, it requires no `unsafe` of its own. (If a future gap forces a raw intrinsic `wide` doesn't cover, add that crate back to this list explicitly at that point -- per the same reasoning that closed the list in the first place, don't leave an implicit, undocumented exception.) Every `unsafe` block requires an adjacent `// SAFETY:` comment stating the invariant being upheld, and `#![deny(unsafe_op_in_unsafe_fn)]` is set workspace-wide.

* **Toolchains:**

  * `rustc` / `cargo` via `rustup`, stable channel, pinned per-workspace.

  * `x86_64-pc-windows-msvc` target, MSVC-linked (Windows)

  * `x86_64-apple-darwin` / `aarch64-apple-darwin` targets, Xcode-provided linker (macOS)

  * `x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu` targets (Linux)

### 9.2 Build System & CI/CD

* **Build System:** Cargo workspace (Cargo 1.75+): a core `tre-engine` crate, one crate per RHI backend (`tre-rhi-vulkan`, `tre-rhi-dx12`, `tre-rhi-metal`, platform-gated via `#[cfg(target_os = ...)]`), a `tre-platform` crate owning native window creation (ARCHITECTURE.md Section 1's "Platform & Event Layer" -- added Phase 1 Step 1, Linux-only for now via `wayland-client`/`x11rb`), and a `tre-ffi` crate that owns the entire C-ABI surface (Section 9.4). `tre-engine` and the RHI backend crates are still statically linked (as `rlib`s) into the final `cdylib`/`staticlib` -- every crate's code ships inside that one binary -- but `tre-ffi` is the *only* crate whose items are `pub` and re-exported as `extern "C"` symbols; every other crate's items stay `pub(crate)`-or-narrower from `tre-ffi`'s perspective. This achieves the same "hide RHI backend symbols from the UI framework" goal that CMake-based linker visibility flags (`-fvisibility=hidden` / `.def` export lists) would otherwise be used for, without relying on a linker feature.

* **Code Formatting & Linting:** Enforcement via `rustfmt` (workspace `rustfmt.toml`) and `clippy` with `-D warnings` in CI, including `clippy::pedantic` selectively enabled for the core rendering crates.

* **Continuous Integration:** CI pipelines must include hardware-accelerated headless rendering instances to run automated performance regression tests (`cargo bench` via `criterion`), verifying the $\le 0.50\text{ ms}$ CPU frame processing budget on target hardware, and must also run under the zero-allocation debug guard (Section 3.4) as a hard gate, not merely a benchmark. `cargo clippy` and `cargo test` -- including the Python-binding integration tests (Section 9.4) -- run on every commit alongside the performance suite.

* **GPU API Validation:** Debug and CI builds of each RHI backend run with the native graphics API's own validation enabled -- `VK_LAYER_KHRONOS_validation` for Vulkan, the D3D12 debug layer (`ID3D12Debug::EnableDebugLayer`) for DirectX 12, and `MTL_DEBUG_LAYER=1` for Metal -- with any validation error failing the CI job. This is the standard, vendor-provided tool for catching resource-state, barrier, and synchronization misuse at exactly the point where the `unsafe` policy above concentrates raw FFI into the graphics APIs, a class of bug the CPU-side gates (zero-allocation guard, `clippy`, batching-equivalence tests) cannot see. All validation layers compile out entirely in release builds -- see IMPLEMENTATION.md Phase 2 Step 2.4.

  *Implemented for Vulkan as of Phase 2 Step 2 (2026-09-05):* `VulkanDevice::new` queries `vkEnumerateInstanceLayerProperties`/`vkEnumerateInstanceExtensionProperties` first and only requests `VK_LAYER_KHRONOS_validation`/`VK_EXT_debug_utils` if both are actually installed (graceful degradation, not a hard requirement -- a contributor without the package still gets a working `cargo run`), gated by `cfg(debug_assertions)`. The `VK_EXT_debug_utils` messenger callback calls `std::process::abort()` on an `ERROR`-severity message -- verified by deliberately triggering it, not assumed: an earlier version used `std::process::exit()`, which hung indefinitely instead of terminating (a driver's `atexit` handler appears to deadlock trying to reacquire a lock the still-on-the-stack Vulkan call that triggered the callback is holding). `abort()` skips `atexit` entirely and reliably terminates. CI's new `vulkan-validation` job (`.github/workflows/ci.yml`) installs a software Vulkan ICD (`mesa-vulkan-drivers`/lavapipe, since hosted runners have no GPU) and runs every example under `xvfb-run` (no display server either), genuinely exercising this gate on every push rather than relying on a human remembering to check manually -- proven by deliberately reintroducing a real error (a zero-byte buffer) and confirming the CI job actually failed with the expected `VUID-VkBufferCreateInfo-size-00912` message before reverting it. DirectX 12/Metal validation remain unimplemented, deferred with those backends.

### 9.3 Shader Authoring & Cross-Compilation

Added in the September 2026 documentation review -- no prior draft specified how a single shader source reaches three backend shading languages:

* **Single Source, HLSL:** All engine shaders (SDF rect, MSDF text, blur passes) are authored once in HLSL (Shader Model 6.6+).
* **Vulkan:** Cross-compiled to SPIR-V via DXC (`-spirv` target), consumed directly by `VK_KHR_dynamic_rendering` pipelines.
* **DirectX 12:** Compiled to DXIL via DXC natively -- no cross-compilation step required.
* **Metal:** SPIR-V output is translated to Metal Shading Language via SPIRV-Cross (or a maintained equivalent) as part of the asset build step, not at runtime.
* **Build Integration:** Shader compilation is a Cargo `build.rs` build-script step producing backend-specific binary blobs (embedded via `include_bytes!` or loaded from `OUT_DIR` at startup) at build time; runtime shader compilation is never performed in a shipping build, and a compilation failure at build time fails `cargo build` rather than surfacing as a runtime pipeline-creation error (see DESIGN.md Section 2.6 for what happens if a pipeline nonetheless fails to create at load time, e.g., due to a driver-specific rejection).

### 9.4 Cross-Language FFI & Python Bindings

Added alongside the Rust engine / Python UI framework language decision. The engine's entire public surface is a `#[repr(C)]`, `extern "C"` API defined once in the `tre-ffi` crate (Section 9.2); every language binding -- including the project's own Python UI framework -- is written against this single boundary, so no binding gets privileged access the C ABI does not expose to any other language.

* **ABI shape:** Opaque handle types (`TreCanvasHandle`, `TreDeviceHandle`, etc.) as `#[repr(transparent)]` pointer wrappers. No Rust `enum` with data, `Option<T>`, `Result<T, E>`, `String`, `Vec<T>`, or trait object crosses the boundary directly -- each has a C-compatible shadow representation (tagged-union struct, raw pointer + length pair, integer result code, or out-parameter, respectively).
* **Error propagation:** Every fallible `extern "C"` function returns an `EngineError` integer result code (DESIGN.md Section 2.6) rather than a Rust `Result`; out-parameters carry the success value.
* **Memory ownership:** Any buffer the engine allocates and hands across the boundary (e.g., a headless frame readback buffer, DESIGN.md Section 4.3) is freed by an explicit `tre_*_free` function, never by the caller's allocator -- Rust's allocator and the host language's allocator (e.g., CPython's) are never assumed compatible.
* **Python binding mechanism:** The Python UI framework binds via [PyO3](https://pyo3.rs/), generating a native extension module that wraps the `tre-ffi` C-ABI surface with Pythonic ergonomics (context managers for `Canvas.save()`/`restore()` scope pairs; Python exceptions raised from `EngineError` codes at the Python layer only -- the Rust/C boundary itself never uses exceptions). The GIL is released (`Python::allow_threads`) for the duration of any blocking engine call (e.g., an `RhiDevice::begin_frame` fence wait) so Python-side multi-threading is not serialized behind engine calls. The `catch_unwind` panic guard (Section 9.1) wraps the *entire* PyO3-facing call, including the `allow_threads` scope, not just the inner blocking wait -- so a panic occurring while the GIL is released is only converted to an `EngineError` after PyO3 has reacquired the GIL on scope exit, never while it is still released.
* **Binding-parity testing:** `cargo test`'s FFI test target and the Python package's own test suite both exercise the same `tre-ffi` entry points; Section 9.2's CI gate treats a Python-binding test failure as a build failure, not an advisory warning, since the Python UI framework is a first-class consumer of this boundary, not a side project.
