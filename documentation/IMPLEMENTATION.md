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

### Status: Linux complete (2026-09-05); Windows/macOS deferred

Scope decision (confirmed with the project owner): this step was executed for **Linux only** -- both Wayland (primary, via `wayland-client`/`wayland-protocols`) and X11 (fallback, via `x11rb`'s XCB FFI connection, exercised through XWayland on the dev machine). Windows and macOS native bridges are deferred to their own later steps, since this machine can only build, run, and verify Linux -- matching the "verify for real, not just compile" discipline Phase 0 established.

Implemented in a new `tre-platform` crate (task 2's native OS surface bridges, Linux half) and `crates/tre-rhi-vulkan` (tasks 1, 3, 4). Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace; both windowing backends confirmed with real, visible windows (screenshotted); a two-window demo proves genuine `RhiDevice` sharing (one device, two independently-rendering swapchains, zero validation errors); a headless demo proves `HeadlessSwapchain` implements the unmodified `RhiSwapchain` trait and produces a pixel-correct PNG with zero validation errors.

Real bugs and gaps found during implementation (full detail in documentation/REVIEW.md's Phase 1 Step 1 entry):

* **`VulkanDevice::submit_and_present`'s post-render layout transition is hardcoded for a real presentable swapchain** (`COLOR_ATTACHMENT_OPTIMAL -> PRESENT_SRC_KHR`), which is meaningless for `HeadlessSwapchain`'s plain image. Caught by the Vulkan validation layer as a layout mismatch. Worked around in `HeadlessSwapchain::present` for now (its own transition starts from the real, `PRESENT_SRC_KHR`-tagged state rather than the `COLOR_ATTACHMENT_OPTIMAL` it would need for a windowed swapchain); the real fix -- letting each concrete `RhiSwapchain` control its own post-render transition instead of one hardcoded in the shared `RhiDevice` code -- is a genuine interface refinement worth making before more swapchain variants are built on this pattern.
* **A leaked `VkSurfaceKHR`:** `VulkanDevice::new` requires a window purely to probe present support while selecting a physical device, which is awkward for headless mode (which has no real window at all) -- the headless demo initially never destroyed this probe surface. Fixed in the demo; the underlying awkwardness (headless mode needing a throwaway window just to bootstrap a device) is a real API gap, deferred to Phase 2's device-selection work.
* **`VulkanDevice::create_surface`** was extracted from `VulkanDevice::new` as its own public method, so additional windows can get a surface without re-running physical device selection -- required for genuine multi-window support, previously impossible since surface creation was embedded entirely inside `new`.
* **Wayland surfaces with no buffer attached are invisible** (unlike X11, which shows a blank mapped window) -- expected protocol behavior, not a bug, but worth noting since it means a windowing-only smoke test can't be verified by screenshot the way a windowed Vulkan demo can.
* **xdg-shell gives clients no control over top-level window position** -- when the multi-window demo's two unpositioned windows land at the same compositor-chosen spot, they visually overlap. This is inherent to the Wayland protocol (X11 clients can request a position; Wayland toplevels cannot), not an implementation defect -- the two-window demo still proves independent rendering per window (distinct colors, zero validation errors), just not always without moving one window to see both clearly.
* **Disclosed by the Phase 1-4 review (2026-09-06), not by this step's own original status text:** task 3's second half -- mapping DPI/resize events to "trigger instantaneous swapchain resizing" -- was never actually built. `VulkanSwapchain` has only a constructor, no recreate/resize method; `tre-platform`'s `scale_factor()` has no caller outside its own dispatch wrapper; `InputEvent::Resized` is drained by `input_demo` purely to log it; and `acquire_next_image`/`present` correctly surface `EngineError::SwapchainOutOfDate` on `ERROR_OUT_OF_DATE_KHR`/`SUBOPTIMAL_KHR` but nothing anywhere recovers from it. This step's own status text above never disclosed this gap the way it honestly disclosed Windows/macOS deferral -- a real window resize today gets `SwapchainOutOfDate` on every subsequent frame with no recovery path. Real swapchain recreation is genuine future work, tracked here rather than built opportunistically alongside an unrelated review/fix pass; see REVIEW.md's Phase 1-4 review entry for the full finding.

### Step 1.2: Decoupled Event & Signal Pipeline

* **Implementation Tasks:**

  1. Implement the canonical Single-Producer Single-Consumer (SPSC) lock-free ring buffer for capturing OS window events, per TECHNICAL.md Section 8 -- do not restate "SPMC" here; that was corrected to SPSC in the September 2026 documentation review (the engine has exactly one consumer, the UI framework's logic tick) and this line was simply never updated to match since nothing had implemented this step yet to notice the drift.

  2. Translate platform-specific input (e.g., `WM_POINTERDOWN`, `NSEventTypeLeftMouseDown`) to agnostic engine structures (e.g., `InputEvent::PointerDown`).

  3. Implement event payload coalescing. For instance, if multiple high-frequency mouse move events occur between frames, squash them into a single `PointerMove` event to save layout evaluation time.

  4. Ensure the event pump executes entirely outside the graphics pipeline timeline, exposing a polling/drain interface to the UI framework.

* **Technical Rationale:** Graphics execution must never block on hit-testing or OS input hooks. Decoupling guarantees the $0.50\text{ ms}$ CPU frame submission budget is isolated from layout and logic stalls.

### Status: Linux complete (2026-09-05); Windows/macOS deferred

Scope decision (confirmed with the project owner): `tre-platform` is consolidated from Step 1.1's one-connection-per-window design to one shared `PlatformConnection` per backend (`wayland_client::Connection` or `x11rb::xcb_ffi::XCBConnection`), multiplexing multiple windows -- each addressed by an opaque `WindowId` -- over that single connection, matching the "one OS-event-pump producer across all windows" design TECHNICAL.md Section 8 already described. Touch input and genuine cross-thread producer/consumer separation remain out of scope (no touchscreen on this machine; the ring buffer is built genuinely atomic-based so it needs no redesign whenever a real second thread is introduced, but this step still drains it from the same call stack that renders).

Implemented: task 1's SPSC ring buffer as `tre_memory::SpscRingBuffer<T>`; tasks 2-3 as `tre_engine::{InputEvent, WindowId, InputEventQueue}`; task 4 (pointer/keyboard binding) on both backends -- Wayland via `wl_seat` -> `wl_pointer`/`wl_keyboard`, X11 via the existing window's extended event mask; task 5 (coalescing) inside `InputEventQueue`, tested independent of any windowing. All three Step 1.1 examples plus `smoke_test` were updated to the new `PlatformConnection` API, and a new `input_demo` (two windows, `demo/phase1_step2/`) proves input works and routes to the correct window.

Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including new `tre-memory` unit tests for the ring buffer (capacity limits, and a genuinely concurrent 100k-item producer/consumer stress test on real OS threads) and new `tre-engine` unit tests for `InputEventQueue`'s coalescing behavior. All three Vulkan examples plus the new input demo ran against real hardware with `VK_LAYER_KHRONOS_validation` enabled, zero errors. Real pointer motion, button clicks, and key presses were synthesized via the X11 XTEST extension (the same mechanism tools like `xdotool` use) against the real X11 backend and shown to translate correctly, including into the right `WindowId` when two windows were open simultaneously (events routed to window A while A had focus/stacking priority, then to window B after switching, with zero cross-window leakage in either direction).

Real bugs and gaps found during implementation (full detail in documentation/REVIEW.md's Phase 1 Step 2 entry):

* **A genuine data-race hazard was found and avoided at design time, before it was ever built:** the initial design considered implementing pointer-move coalescing by having the producer find-and-overwrite the *most recently published* ring-buffer slot in place. This is unsound whenever the queue holds exactly one unconsumed item, since the consumer could be mid-read of that exact slot concurrently. `InputEventQueue` instead stages the pending move in an ordinary (non-atomic) struct field, only ever calling the underlying `SpscRingBuffer::push` once a value is ready to publish -- so the shared ring buffer itself is never mutated by the coalescing logic, keeping it sound if a real second consumer thread is introduced later.
* **Live compositor-level input synthesis was verified for X11 but not Wayland.** This session's compositor (KWin) does not advertise `org_kde_kwin_fake_input`, and wlroots-specific virtual-pointer/virtual-keyboard protocols do not apply to KWin, so no virtual-input mechanism was available to drive the Wayland backend the way XTEST drove X11. Wayland's pointer/keyboard translation code was verified by careful code review and structural parity with the XTEST-verified X11 implementation (identical event model, identical coalescing path through the shared `InputEventQueue`), not by live synthesized input -- an honest gap, not a silent claim of full parity.
* **Unhinted window placement causes same-position stacking on X11 too, not just Wayland.** Step 1.1 already noted Wayland gives clients no control over toplevel position; the same default-placement behavior was observed on X11 via KWin's XWayland window management when verifying multi-window input routing -- two unpositioned windows can land exactly on top of each other, so whichever is topmost receives pointer input regardless of which window's "own" screen coordinates were targeted. Not a `tre-platform` defect; the verification harness was updated to explicitly raise/focus its target window before synthesizing input, which is a testing concern, not a product one.

## Phase 2: Core Hardware Abstraction (RHI) & Memory Management

### Step 2.1: Modern Graphics API Backends

* **Implementation Tasks:**

  1. **Vulkan 1.2:** Implement backend utilizing [`VK_KHR_dynamic_rendering`](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_KHR_dynamic_rendering.html) (eliminating `VkRenderPass` and `VkFramebuffer` overhead). Define a universal pipeline layout that exposes an unbounded array of textures `texture2D textures[]` via [`VK_EXT_descriptor_indexing`](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_EXT_descriptor_indexing.html). Implemented in the `tre-rhi-vulkan` crate via the `ash` raw-bindings crate.

  2. **DirectX 12:** Implement backend targeting Feature Level 12_0. Construct a Root Signature that passes vertex data via Root Constants or Root SRVs, utilizing [Resource Binding Tier 3](https://learn.microsoft.com/en-us/windows/win32/direct3d12/hardware-support) for bindless descriptor tables. Implemented in the `tre-rhi-dx12` crate via the `windows` crate's `Win32::Graphics::Direct3D12` bindings.

  3. **Metal:** Implement backend utilizing [Argument Buffers Tier 2](https://developer.apple.com/documentation/metal/buffers/about_argument_buffers), enabling dynamic arrays of texture resources directly in the shader. Implemented in the `tre-rhi-metal` crate via the `objc2-metal` crate.

* **Technical Rationale:** Leveraging dynamic rendering and bindless arrays eliminates pipeline permutation explosion and state-switch overhead, which is critical for UI rendering where widgets constantly alternate between textures, vectors, and text.

### Status: Vulkan complete (task 1); DirectX 12/Metal deferred (2026-09-05)

Scope decision (confirmed with the project owner, re-confirming Phase 2's original precedent): tasks 2 (DirectX 12) and 3 (Metal) are deferred entirely -- neither backend exists, and neither can be built or verified without a Windows/macOS machine.

Task 1's `VK_KHR_dynamic_rendering` half was already done as of Phase 0 (there has never been a `VkRenderPass`/`VkFramebuffer` in this codebase). This step (Phase 2 Step 2.1) built the remaining half: a real `VK_EXT_descriptor_indexing`-backed bindless texture array. `RhiTexture` gained `bindless_index()`; `RhiDevice` gained `create_texture` (a genuine one-time GPU upload from CPU pixel data, distinct from `acquire_transient_target`'s empty render targets); `RhiCommandBuffer::bind_texture` -- a Phase 0 `unimplemented!()` stub -- is now real. `VulkanDevice::new` builds one persistent descriptor set (a fixed shared sampler + an unbounded `SAMPLED_IMAGE` array, capacity clamped at runtime to the real device's `maxDescriptorSetUpdateAfterBindSampledImages`, target 4,096 matching ARCHITECTURE.md Section 4.1's sort-key field width), bound exactly once per pipeline bind and never rebound between draws that sample different textures -- selecting a texture is purely a per-draw-call push constant.

Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace. All five pre-existing Vulkan examples were re-run manually after this step's changes (extended pipeline layout, larger push-constant range) and still produce correct output with zero validation-layer errors, confirming the change is additive. A new `bindless_textures_demo` example uploads three distinct real textures and draws each via the same bound pipeline/descriptor set, varying only the push-constant index -- verified both by zero validation errors and by asserting the actual output pixel colors from a headless PNG readback match each texture's known content exactly (not merely that the draw calls didn't crash). Added to the CI `vulkan-validation` job.

Two real bugs were found and fixed during implementation, both caught by the validation layer on the very first and second runs (full detail in `documentation/REVIEW.md`'s Phase 2 Step 2.1 entry and `planning/archive/LOG_PHASE2_STEP2_1.md`): a missing `descriptorBindingSampledImageUpdateAfterBind` feature request, and a `VARIABLE_DESCRIPTOR_COUNT` flag placed on the wrong (non-highest-numbered) binding. A third issue was found in the new demo itself, not the RHI: assuming "skip `bind_texture`" resets to "no texture" -- it doesn't, since the bound index is ordinary persistent command-buffer state, exactly like the pipeline or vertex buffer.

Per-vertex texture indexing (DESIGN.md Section 8.1.2's cross-atlas single-draw-call batching) is explicitly out of scope: it needs the atlas-packing `Canvas`-to-RHI renderer, which is Phase 3/4 work that doesn't exist yet.

### Step 2.2: Zero-Allocation Ring Buffers & Transient Pools

* **Implementation Tasks:**

  1. Construct a triple-buffered `DynamicRingBuffer` ($16\text{ MB} - 32\text{ MB}$) using host-coherent, write-combined mapped memory (`VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT`).

  2. Implement CPU-side hardware fence waits (e.g., `vkWaitForFences`) before writing to frame $N$, ensuring the GPU has completely finished reading the segment from $N-3$.

  3. Enforce strict alignment: $64\text{ bytes}$ for CPU thread boundaries (prevent false sharing) and $256\text{ bytes}$ minimum alignment for RHI dynamic offsets.

  4. Build a `FxHashMap`-backed (or `ahash`) Transient Render Pool for offscreen textures keyed by `(Width, Height, Format)`, with width/height rounded up to fixed bucket boundaries (TECHNICAL.md Section 3.2) so nearby requests share an entry -- these are internal engine keys with no untrusted input, so `std::collections::HashMap`'s default SipHash buys nothing but cycles on this per-`push_layer` hot path. Hook this into `Canvas::push_layer` for immediate zero-allocation acquisition, falling back to the next-larger pooled entry on a genuine miss (DESIGN.md Section 2.6).

  5. **Debug-mode balance assertion (added in the September 2026 documentation review):** track `PushLayer`/`PopLayer` calls as a depth counter per `Canvas`; assert the counter is exactly zero at frame boundary. An unbalanced push (a widget that acquires a transient target and never releases it) otherwise starves the pool silently over many frames rather than failing loudly at the point of the actual bug.

* **Technical Rationale:** Writing directly to mapped memory prevents dynamic staging allocations. The transient pool ensures complex multi-pass widget effects (like glassmorphism) require $0\text{ bytes}$ of dynamic allocation during the active frame tick. The balance assertion turns a slow VRAM leak into an immediate, attributable debug-build failure.

### Status: Complete, with two scoped deviations from the tasks above (2026-09-05)

Scope decision (confirmed with the project owner): this step tackled Step 2.2 before Step 2.1 (graphics backends), since DX12/Metal can't be built or verified on this Linux machine -- Step 2.1 is deferred to its own future step (DX12/Metal to be deferred entirely as empty placeholders when that happens, mirroring Step 1.1's Windows/macOS precedent). Full detail in `planning/archive/PLAN_PHASE2_STEP1.md`/`LOG_PHASE2_STEP1.md`.

Implemented: a real `tre_engine::RhiDynamicRingBuffer` trait and `VulkanRingBuffer` (task 1: host-coherent, persistently-mapped `VkBuffer`, 3 real segments, 256-byte-aligned bump allocation -- task 3's alignment half); a real transient render target pool (`VulkanDevice`'s `Mutex<TransientPool>`, task 4: power-of-two `(width, height, format)` bucketing, next-larger fallback on miss, deferred exact-size growth at the next frame's `begin_frame`); and `RenderingCanvas::push_layer`/`pop_layer` with the debug balance assertion (task 5).

Two deliberate deviations from the literal task wording above:
* **Task 3's "64 bytes for CPU thread boundaries"** is not implemented -- there is no multi-threaded canvas writer yet to need false-sharing protection against (that arrives with Phase 5's `SubCanvas`). Implementing padding for a thread-safety property nothing yet exercises would be unverifiable; deferred until Phase 5 introduces a real concurrent writer.
* **Task 4's "Hook this into `Canvas::push_layer` for immediate zero-allocation acquisition"** was NOT done as literally worded. `RenderingCanvas::push_layer`/`pop_layer` record IR markers and track the balance counter only -- they never call `RhiDevice::acquire_transient_target` directly. This preserves DESIGN.md Section 2.2's "Strict Architectural Separation of Concerns": `Canvas` recording is pure, backend-agnostic IR construction with no RHI device reference, matching how `draw_rounded_rect` already works, and nothing downstream of `Canvas` consumes a transient target yet (the sort/batch/execute pipeline, Phase 6, is what would). The transient pool itself is real and independently proven (`demo/phase2_step1/`); wiring `push_layer` to actually acquire from it is deferred to whichever later phase builds the RHI execution stage that would consume the result.

A real bug was found and fixed via actual execution, not code review: an initial version of the frame-in-flight fence upgrade (built to let the ring buffer track "current segment") mistakenly rotated the SAME fence-wait/signal logic that gates `VulkanDevice`'s single persistent command buffer -- since that command buffer is reused every frame regardless of which ring-buffer segment is current, rotating its fence broke the actual synchronization guarantee (a rotated fence is trivially already-signaled, so it doesn't prove the GPU is done with the command buffer). The Vulkan validation layer caught this immediately once `walking_skeleton`/`multi_window` were re-run (command-buffer-still-in-use errors), not from static analysis. Fixed by keeping a single real fence for command-buffer gating (unchanged Phase 0 semantics) and adding a separate, purely informational rotating counter for the ring buffer's own segment selection. Full detail in `documentation/REVIEW.md`'s Phase 2 Step 1 entry.

Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including new unit tests for `push_layer`/`pop_layer`'s balance assertion. A new `demo/phase2_step1/` example drives real frames through a `HeadlessSwapchain` while writing into the ring buffer and cycling the transient pool, verified against real hardware with `VK_LAYER_KHRONOS_validation` enabled -- zero errors after the fence-design fix above (a real, validation-layer-caught leak of pooled textures never destroyed at device teardown was also found and fixed the same way).

### Step 2.3: Generational Garbage Collection (GC)

* **Implementation Tasks:**

  1. Embed a `u64 last_frame_used` timestamp into the metadata of all dynamic VRAM resources (atlas regions, tessellated SVG caches).

  2. Implement an asynchronous GC thread that scans resource pools when VRAM capacity hits $85\%$.

  3. Identify resources older than $N = 600$ frames. Remove their CPU-side handles and move their GPU handles to a deferred release lock-free queue.

  4. At the end of every frame, check the deferred release queue. Physically destroy hardware resources only if $N_{\text{current}} - N_{\text{evicted}} > 3$ frames.

* **Technical Rationale:** Prevents dynamic VRAM from ballooning past the $128\text{ MB}$ budget while ensuring that resources currently being executed by the GPU are never prematurely destroyed.

### Status: Complete, verified against the transient pool (2026-09-06)

Scope decision (confirmed with the project owner): this step's literal targets -- "atlas regions, tessellated SVG caches" -- don't exist yet (the dynamic texture atlas is Phase 4 work; SVG tessellation is Phase 3/5). Rather than deferring the whole step the way DX12/Metal were, the real generational-GC mechanism was built now and verified against the one dynamic-VRAM resource that already exists and already grows unbounded: Phase 2 Step 2.2's transient render-target pool. The atlas and SVG cache plug into the same mechanism once they exist. The project owner also chose to build task 2's "asynchronous GC thread" as a genuine background OS thread -- the engine's first real multi-threading -- rather than deferring threading further the way every prior step had.

Implemented: `FrameSync` gained a genuinely monotonic `total_frame_count` (distinct from its existing 0..3 ring-buffer-segment counter). `VulkanTexture` gained `last_used_frame`/`size_bytes`; `TransientPool` gained a running `total_free_bytes`. A real background thread (`gc_thread_loop`) wakes roughly every 100ms, and once `total_free_bytes` crosses 85% of the $128\text{ MB}$ budget, evicts every free-list entry older than 600 frames into a deferred-release queue. Crucially, that thread never calls a single Vulkan function -- it only locks `TransientPool`'s `Mutex` and moves plain Rust values. The actual destruction (task 4) happens on the main thread, in `begin_frame`, after the real 3-frame grace period -- the same call site that already runs the Step 2.2 pool-growth check. This split (decide-on-a-thread, destroy-on-the-main-thread) is what makes introducing real concurrency here sound: the main thread remains the only thread that ever touches a raw Vulkan handle for destruction.

One deliberate deviation from task 3's literal "lock-free queue": the deferred-release queue is a plain `Mutex<VecDeque<_>>`. Contention is negligible at this call frequency (the GC thread pushes at most once per ~100ms scan, the main thread checks once per frame), and a `Mutex` makes peeking the front entry without consuming it trivial -- needed for the grace-period check -- which `tre_memory::SpscRingBuffer`'s `pop`-only API doesn't support without real risk of losing an entry.

Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace. All six pre-existing examples re-run manually with `VK_LAYER_KHRONOS_validation` enabled, zero errors -- confirming the new background thread introduces no cross-thread Vulkan misuse the validation layer would catch. A new `gc_demo` example checks 25 distinct transient-target sizes into the pool (~240 MB, comfortably past the 85% trigger), then runs real `begin_frame`/`submit_and_present` cycles -- no shortened stand-in for the 600-frame age threshold or the 3-frame grace period -- until `transient_pool_stats()` reports real evictions and destructions (consistently 50 of each across five runs, in ~0.2-0.3 seconds; the doubling from 25 checked-in sizes to 50 evicted is a real, explained interaction with Step 2.2's pool-growth queuing, not a defect -- see `planning/archive/LOG_PHASE2_STEP2_3.md`). Five consecutive `gc_demo` runs plus repeated fast-exiting example runs confirmed `Drop for VulkanDevice`'s GC-thread shutdown never hangs.

**A dedicated second review pass (2026-09-06), scoped to just this step**, found and fixed two Critical bugs (both agents independently) plus two Should-fix design gaps -- see `documentation/REVIEW.md`'s "Phase 2 Step 2.3 Code Review" section for full detail. Most notably: `Drop for VulkanDevice` cleared the transient pool before device teardown but never extended that same fix to the new `deferred_release` queue, a real use-after-destroy on shutdown -- the exact class of bug Step 2.2 already found once. Also added: `saturating_sub` throughout the byte accounting (an underflow would have poisoned the shared mutex and cascaded into main-thread panics with no diagnostic trail), a real admission-side cap on the transient pool's growth (`acquire_transient_target` now returns `Result`, since the GC alone can only reclaim idle entries, never cap active growth), and a throughput cap on how many entries one GC scan evicts (bounding worst-case lock contention and shutdown latency). `gc_demo` updated accordingly and re-verified: it now hits the new admission cap after 22 of its 25 candidate sizes (~128 MB), already comfortably past the 85% GC-trigger threshold by then, and stops gracefully rather than treating that as an error.

### Step 2.4: GPU API Validation in Debug & CI Builds

* **Implementation Tasks:**

  1. **Vulkan:** Enable `VK_LAYER_KHRONOS_validation` (via the enabled-layer list passed to instance creation) in debug and CI builds only, with a `VK_EXT_debug_utils` messenger callback routing validation messages into the engine's own logging and failing the CI job on any `VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT` message.

  2. **DirectX 12:** Call `ID3D12Debug::EnableDebugLayer()` before device creation in debug/CI builds. Gate the much heavier `ID3D12Debug1::SetEnableGPUBasedValidation` behind an explicit opt-in (env var or Cargo feature) rather than always-on, since GPU-based validation materially slows frame time and would corrupt the CI's own performance-regression numbers (Section 9.2) if left on unconditionally.

  3. **Metal:** Set `MTL_DEBUG_LAYER=1` (and `MTL_SHADER_VALIDATION=1`) on the CI test process for macOS runners, routing validation output into the same CI-failing log check as the other two backends.

  4. Gate all three behind the same debug/profile `cfg` used by the zero-allocation guard (TECHNICAL.md Section 3.4), so none of this exists in a shipped release binary.

* **Technical Rationale:** The CPU-side gates already in place (zero-allocation guard, `clippy`, the batching-equivalence pixel-diff test) validate everything on the Rust side of the `RhiDevice`/`RhiCommandBuffer` trait boundary, but the RHI backend crates are the one place `unsafe` FFI into the raw graphics APIs happens (TECHNICAL.md Section 9.1) -- exactly the code Rust's own type system cannot check. Native validation layers are the vendor-provided tool for catching resource-state, barrier, and synchronization misuse at that boundary, at zero cost in the shipped binary.

### Status: Vulkan complete (task 1 + 4); DirectX 12/Metal deferred (2026-09-05)

Scope decision (confirmed with the project owner): tasks 2-3 (DirectX 12, Metal) are deferred entirely -- neither backend exists yet (Step 2.1 is itself deferred, and when it happens DX12/Metal stay deferred too, per the Step 1.1 Windows/macOS precedent: no machine to build or verify against). This step implements task 1 (Vulkan) and task 4 (release-build gating, folded into the same `cfg(debug_assertions)` gate) for real.

Implemented: `VulkanDevice::new` queries instance layer/extension support and requests `VK_LAYER_KHRONOS_validation`/`VK_EXT_debug_utils` only if both are available (graceful degradation -- a contributor without the package installed still gets a working `cargo run`, rather than validation being a hard requirement); a `VK_EXT_debug_utils` messenger callback prints every message and terminates the process on any `ERROR`-severity one; a new `vulkan-validation` CI job installs a software Vulkan ICD (`mesa-vulkan-drivers`) and runs all five examples under `xvfb-run` (hosted runners have neither a GPU nor a display server).

A real bug was found and fixed via actually triggering the failure path, not by reading the Vulkan/Rust docs and assuming it would work: the callback's first version called `std::process::exit(1)` on an error, which hung indefinitely instead of terminating (confirmed via a hard `timeout` wrapper: exit code 124, not the expected nonzero-and-done). Root cause: `exit()` runs registered `atexit` handlers before terminating, and the GPU driver's own handler appears to deadlock trying to reacquire a lock the still-on-the-stack Vulkan call that triggered the very callback calling `exit()` is holding. Fixed by switching to `std::process::abort()`, which raises `SIGABRT` directly and skips `atexit` entirely -- confirmed via the same test: exit code 134, immediate termination. Full detail in `documentation/REVIEW.md`'s Phase 2 Step 2 entry.

A second, unrelated, pre-existing bug was found while verifying this step's new CI job for the first time: `cargo build`/`clippy`/`test` had been failing on CI since Phase 1 Step 1 (three prior commits, undetected until now), because `libwayland-dev`, `libxcb1-dev`, and `glslc` -- all needed to compile the workspace at all -- were never installed on the GitHub-hosted runners. Fixed as its own commit, separate from this step's actual feature work, since it's a pre-existing regression this step happened to be the first to notice (nothing about Step 2.4 introduced it).

Verified end to end: the new CI job was proven to genuinely catch a real failure, not just exist -- a deliberate zero-byte buffer (a guaranteed `VUID-VkBufferCreateInfo-size-00912` violation) was pushed to a scratch branch, confirmed via `gh run view` to make the real GitHub Actions job fail with the expected validation message and a nonzero exit code, then reverted and confirmed the same job passes clean. All five examples (`walking_skeleton`, `multi_window`, `headless`, `input_demo`, `memory_pools_demo`) now run in CI under a software Vulkan renderer and a virtual display, with validation loading automatically -- no more relying on a human remembering to set `VK_LOADER_LAYERS_ENABLE` manually, which is exactly how both of Phase 2 Step 1's real bugs were originally caught.

## Phase 3: Geometry Pipeline & Vector Math Engine

### Step 3.1: Compact UI Vertex & Matrix Math

* **Implementation Tasks:**

  1. Implement the `UiVertex` format exactly as defined in ARCHITECTURE.md Section 3.1 (the canonical 32-byte layout, added in the September 2026 documentation review) -- do not redeclare the field layout here.

  2. Implement SIMD-accelerated $3 \times 3$ affine transformation matrices using the [`wide`](https://docs.rs/wide) crate's `f32x8` vector type and its `mul_add` method (hardware FMA where the target has it, a separate multiply+add otherwise) to batch-multiply local node transforms down the UI scene graph tree -- no raw `core::arch` intrinsics or `unsafe` needed for this, since `wide`'s public API is safe and portable across the AVX2 (x86_64) and NEON (ARM64) targets (TECHNICAL.md Section 2.2).

  3. Add a compile-time assertion (`const _: () = assert!(std::mem::size_of::<UiVertex>() == 32);`, per ARCHITECTURE.md Section 3.1) validating the struct layout across all target triples.

* **Technical Rationale:** Capping the vertex struct at $32\text{ bytes}$ minimizes PCIe bus transfer times and maximizes GPU L2 cache coherency.

### Status: Complete (2026-09-06)

Tasks 1 and 3 were already done, as a side effect of Phase 0's walking skeleton -- `crates/tre-engine/src/lib.rs`'s `UiVertex` struct and its `const _: () = assert!(std::mem::size_of::<UiVertex>() == 32);` predate this step by several phases. This step implemented the real remaining work, task 2, in the previously-empty `tre-math` crate.

Chosen as Phase 3's opening step deliberately: unlike every Phase 2 step, it has zero GPU/Vulkan dependency, verified entirely by `cargo test -p tre-math` -- no display server, no validation layer, no demo folder with a screenshot. A genuine change of pace after five consecutive GPU-heavy steps.

Implemented: `Affine2` (six `f32` fields, not a dense $3\times3$ -- the bottom row is always `[0, 0, 1]` for a genuine affine transform, so storing it would be pure waste), with constructors matching TECHNICAL.md Section 7.2's formula exactly (`from_translation`, `from_rotation`, `from_scale`, and the combined `from_translation_rotation_scale`), scalar `compose`/`transform_point`, and `compose_batch` -- 8 parent-child pairs at a time via `wide::f32x8::mul_add`, with a scalar-`compose` fallback for the remainder. `compose_batch` writes into a caller-provided `&mut [Affine2]` rather than returning a `Vec`, since its eventual per-frame scene-graph-flattening caller can't allocate (DESIGN.md Section 2.1's zero-allocation steady state) -- the same discipline that shaped Phase 2's entire ring-buffer/transient-pool design, applied here even though the calling code doesn't exist yet.

No scene-graph/node-tree type exists in this codebase, so `compose_batch` operates on plain slices rather than a real tree -- matching the same "build the tested primitive before its exact consumer exists" precedent as `tre_memory::SpscRingBuffer` (built in Phase 1 before any real second thread existed) and Phase 2 Step 1's dynamic ring buffer/transient pool (built before Phase 6's execution stage exists to feed them).

Verified end to end: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace (`tre-math` opts into `clippy::pedantic`, per its own `Cargo.toml`, and required a few targeted `#[allow]`s for genuine false positives -- `similar_names` on `tx`/`ty`-derived local bindings that are this codebase's own field names, not an accidental collision, and `float_cmp` on tests whose inputs involve no rounding at all). 11 unit tests, including a SIMD-vs-scalar-reference comparison across slice lengths `0, 1, 7, 8, 9, 16, 17` -- every remainder case relative to the 8-wide SIMD chunk -- compared with an epsilon tolerance rather than exact equality, since `wide::f32x8::mul_add`'s hardware FMA and a scalar multiply-then-add can legitimately differ in the last bit or two of an `f32`. Verified on x86_64/AVX2 only (this dev machine and the CI runner); the NEON code path compiles from the identical source but is not independently run on real ARM hardware here.

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

### Status: Complete (2026-09-06)

Task 2 (exactly 4 vertices/6 indices per rectangle) predates this step -- `RenderingCanvas::draw_rounded_rect` has emitted that shape since Phase 0. This step implemented the real remaining work, tasks 1 and 3: a new, dedicated `sdf_rounded_rect.{vert,frag}` shader pair evaluating the exact SDF and `fwidth`-based AA formulas above, plus the CPU-side plumbing needed to feed it real per-quad data.

`draw_rounded_rect` gained a `radius: f32` parameter, clamped to `[0.0, min(half_width, half_height)]` before storage. `UiVertex::uv` -- documented since ARCHITECTURE.md Section 3.1 as "Texture coordinates *or SDF bounds*" -- is repurposed for this shader as each corner's offset from the rect's center, in the same pixel units as `position`; linear interpolation across the quad's two triangles reproduces the exact local `(x, y)` offset at every fragment, the standard technique for evaluating a box SDF from a single quad. `UiVertex::params` becomes `[radius, half_width, half_height]`, uniform across all 4 vertices. A real, pre-existing gap was found and fixed along the way: `VulkanDevice::create_pipeline`'s vertex attribute descriptions had only ever declared `position`/`uv`/`color` (locations 0-2) -- `params` has existed in the vertex format since Phase 0 but was never wired as a shader-readable attribute until this step added location 3 (`R32G32B32_SFLOAT`, offset 20) to the one universal pipeline layout every pipeline gets.

Uniform corner radius only, not DESIGN.md's eventual per-corner `CornerRadii` -- the formula above takes a single scalar $r$; four independently-selected radii is a real, separate technique deferred until a `Canvas` API that actually needs it exists. A new, dedicated shader pair rather than a modification to `walking_skeleton` or `bindless_textured`, matching this project's existing one-shader-pair-per-technique precedent; DESIGN.md Section 8.1.2's eventual shader-mode-tag unification across SDF/texture/MSDF stays deferred to Phase 4.

Verified by a new headless example, `sdf_rounded_rect_demo` (`demo/phase3_step3_2/`), reading back real rendered pixels rather than trusting that the shader compiled: a deep-interior point is exactly the foreground color (alpha clamps to exactly 1.0), a point in the bounding box's corner well outside the rounding arc is exactly the background clear color (alpha clamps to exactly 0.0), and a real partial-alpha blend is confirmed near the rounded corner's arc. That last check taught a real, worth-recording lesson during development: this rect's *flat* edges sit at exact integer canvas coordinates, so their entire 1px-wide analytical AA ramp falls exactly between two pixel centers (at half-integer offsets) with no fractional-coverage sample landing inside it -- an initial version of the demo scanned the flat left edge for a blended pixel and found none, not because AA was broken, but because a perfectly pixel-aligned axis-aligned edge is the one case this technique produces a hard transition for. The rounded corner's non-axis-aligned gradient has no such alignment and reliably produces several genuinely partial-alpha pixels, which is also the more representative place to check anyway, since it's the rounding itself this step exists to prove. All 7 pre-existing examples (5 windowed/headless demos across 4 files, `sdf_rounded_rect_demo` makes a 6th) were re-run manually under `VK_LAYER_KHRONOS_validation` after the vertex-attribute and signature changes -- zero errors, only the expected benign performance warning that the older flat-color shaders don't consume the new `location = 3` input. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including 3 new `tre-engine` unit tests for the `uv`/`params` encoding and radius clamping (2 needed a targeted `#[allow(clippy::float_cmp)]` for genuinely exact literal-`f32` arithmetic, the same pattern Step 3.1 established).

### Step 3.3: SVG Tessellation & Keyframe Morphing

* **Implementation Tasks:**

  1. Integrate a robust ear-clipping/trapezoidal tessellator for static complex SVG paths, caching the resulting vertex soup to the `DynamicRingBuffer`.

  2. Implement path-morphing interpolation using the `wide` crate's `f32x8` vector type (TECHNICAL.md Section 5.4) -- one source-level implementation that compiles to genuine 256-bit AVX2 operations on x86_64 and to a pair of emulated 128-bit NEON operations on ARM64. Ensure topological equivalence (matching number of control points) between keyframes.

  3. Implement the stencil-and-cover fallback rendering method for path intersections that fail simple ear-clipping (e.g., self-intersecting paths with `EvenOdd` fill rules).

  4. **Harden the parser against untrusted input (added in the September 2026 documentation review):** enforce hard caps on `<use>` reference recursion depth, total resolved path point count per document, and group nesting depth; reject and report (via `Result<T, EngineError>`, never a panic or an unbounded loop) any document exceeding those caps before tessellation begins. This applies whenever an application path loads SVG that did not ship as a first-party asset -- if a given integration only ever loads trusted, build-time-bundled SVG, that assumption must be stated explicitly in that integration's own documentation rather than assumed silently here.

* **Technical Rationale:** Caching static tessellations prevents frame-over-frame CPU thrashing, while SIMD accelerates dynamic vector animations to maintain the $240\text{ Hz}$ throughput target. Input hardening prevents a malformed or adversarial SVG document from producing unbounded tessellation cost or unbounded recursion -- a denial-of-service risk for any application that loads SVG from outside its own build.

### Scope decision (confirmed with the project owner, 2026-09-06)

This step bundles four largely independent chunks of work (a tessellator, SIMD path-morphing, a stencil-and-cover fallback for self-intersecting paths, and untrusted-input hardening) -- comparable in total scope to all of Phase 2 combined. Split into sub-steps matching Phase 2's own 2.1-2.4 precedent: **3.3.1** (below) covers SVG ingestion and ear-clipping tessellation of simple polygons. 3.3.2 (SIMD path morphing), 3.3.3 (stencil-and-cover fallback), and any remaining hardening beyond what 3.3.1 already covers each get their own future plan and status note.

### Step 3.3.1: SVG Ingestion & Ear-Clipping Tessellation -- Status: Complete (2026-09-06)

Covers task 1 (ear-clipping only, not trapezoidal -- the task names either) for simple, non-self-intersecting, single-contour fills, and the bulk of task 4's hardening. Tasks 2 (SIMD path morphing) and 3 (stencil-and-cover fallback) are explicitly deferred to their own future sub-steps.

**SVG parsing uses the `usvg` crate (new dependency, pinned to `0.45.1` -- the newest version this workspace's `rust-version = 1.75` can resolve -- with `default-features = false` to exclude font/text-shaping machinery, Phase 4's concern), not a hand-rolled XML/path-data parser.** `resvg` (rasterizes via `tiny-skia`'s own software rasterizer, bypassing this project's entire GPU-tessellation purpose) and `oxvg` (a DOM-optimization toolchain, not built to feed a live rendering pipeline) were both evaluated and rejected; `usvg` stops exactly where this project's own work begins -- DOM/`<use>`/`<g>`/CSS resolution and absolute-coordinate path data, with zero rasterization. A real, verified finding from reading `usvg`'s own source (not assumed from its reputation): it already hardens against the bulk of task 4's stated concerns -- a 1024-deep nesting/`<use>`-chain cap (`Error::NodesLimitReached`), a 1,000,000-element cap (`Error::ElementsLimitReached`), and explicit `<use>` cycle detection (direct, indirect, and sibling-reference cases) -- all surfaced as `Result`, never a panic or an unbounded loop. This step's own hardening adds the two things `usvg` does not itself cap: a raw input byte-size ceiling (checked before `usvg` ever sees the data) and a total-resolved-point-count ceiling (checked incrementally while walking the parsed tree, since a depth/element-bounded document can still resolve to an unbounded number of points).

New crate `tre-svg` (matching the `tre-math` precedent of a new capability domain getting its own crate, keeping `usvg`'s dependency tree out of `tre-engine`'s graph): curve flattening (`flatten.rs`, recursive de Casteljau subdivision, tolerance-based with a hard recursion-depth safety cap) and ear-clipping triangulation (`triangulate.rs`) are both hand-rolled, real algorithmic work -- `usvg` supplies only the DOM/XML plumbing. `to_affine2` bridges `usvg`'s per-path `abs_transform()` into `tre-math`'s `Affine2::transform_point` (Phase 3 Step 3.1) -- the field-for-field-identical affine formula, confirmed by reading both crates' actual transform-application code -- the first real consumer of that primitive outside its own test suite.

Two real, non-obvious ear-clipping correctness bugs were found and fixed while building the verification demo (a non-convex five-pointed star), neither caught by the initially-passing square/L-shape unit tests: (1) triangle indices were returned valid against an internal, possibly-reversed working copy of the polygon's points rather than the caller's own array -- fixed by threading an explicit `original_index` remapping through deduplication and reversal; (2) the ear-validity check needs BOTH "no remaining vertex strictly inside the candidate triangle" AND "no remaining edge properly crosses the diagonal" -- each check alone has a real, distinct blind spot the other closes. See `documentation/REVIEW.md`'s "Phase 3 Step 3.3.1 Implementation" section and `planning/archive/LOG_PHASE3_STEP3_3_1.md` for the full account.

Output feeds the pre-existing flat-color pipeline directly (`UiVertex` triangles, `uv`/`params` zeroed) -- no new shader needed, since a plain triangle soup has no SDF to evaluate. Verified by `crates/tre-rhi-vulkan/examples/svg_tessellation_demo.rs` (`demo/phase3_step3_3_1/`): reads back real rendered pixels, confirming the star's interior is filled and a concave notch is not. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including 15 new `tre-svg` unit tests (the five-pointed-star test checks total area against the true shoelace-formula area AND that a known concave-notch point is covered by no triangle -- a regression test for exactly the bug class found above, since pure area/count checks did not catch it). All 7 pre-existing Vulkan examples re-run manually under `VK_LAYER_KHRONOS_validation`, zero errors (this step touches no RHI/vertex-format code).

**Explicitly out of scope for this sub-step:** true multi-contour hole support (each contour is triangulated independently, which is wrong for shapes with holes); stroke rendering, gradients, patterns, clip-paths, masks, and filters; wiring into `RenderingCanvas`'s public `Canvas` API (proven directly via a dedicated demo first, matching the `SpscRingBuffer`/`tre-math` precedent); tightening `usvg`'s own already-enforced caps further.

### Step 3.3.2: SIMD Path-Morphing Interpolation -- Status: Complete (2026-09-06)

Covers task 2. Task 3 (stencil-and-cover fallback) remains deferred to Step 3.3.3.

**Morphs already-flattened `Polygon` vertices, not raw pre-flatten Bezier control points.** For `tre-svg`'s `Polygon` type (curves already gone by the time one exists), "topological equivalence (matching number of control points)" means equal vertex counts, checked and rejected via `Result` (`SvgError::TopologyMismatch`) if mismatched -- not automatically resampled to reconcile them. This matches how production shape-morphing tools (e.g. GSAP's MorphSVG, `flubber`) actually work: resample-then-interpolate-points, tolerating two keyframes with genuinely different underlying Bezier authoring, not just coordinate differences.

The actual SIMD batch-lerp primitive, `tre_math::lerp_points_batch`, lives in `tre-math` -- its own top-of-file doc comment already lists "SIMD-accelerated path interpolation" among its responsibilities, and the new function mirrors `Affine2::compose_batch`'s exact structure (8-wide `wide::f32x8::mul_add` chunks, scalar remainder, writes into a caller-provided `out` slice, panics on a `from`/`to`/`out` length mismatch since that's a programmer error, not `tre-svg`'s own untrusted-data check). The private `gather` helper was generalized from `&[Affine2]`-only to a generic `gather<T>` so both functions share one 8-lane gather implementation. `tre_svg::morph` stays a pure geometry function -- triangulation remains a separate, explicit caller step via the existing `triangulate`, so re-triangulation happens every animation frame (the interpolated shape's geometry genuinely changes) while curve flattening does not repeat.

Verified by `crates/tre-rhi-vulkan/examples/svg_morph_demo.rs` (`demo/phase3_step3_3_2/`): two independently-parsed, straight-line-only SVG keyframes (a diamond and a square, same vertex count by construction) morphed at `t = 0.0, 0.5, 1.0` and re-triangulated fresh each time. Two probe points -- one inside the diamond but outside the square, one outside *both* keyframes but inside their exact vertex-wise midpoint quadrilateral -- pairwise distinguish all three renders, the strongest available proof that `t=0.5` is a genuinely distinct interpolated shape rather than a snap to either endpoint. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including 3 new `tre-math` unit tests (SIMD-remainder comparison against a scalar reference, an epsilon-based endpoint check -- an initial exact-equality version failed on real data since `(b-a).mul_add(1.0, a)` is not always bit-exact to `b`, the same FMA-rounding lesson `compose_batch`'s own tests already documented -- and a panics-on-mismatch test) and 3 new `tre-svg` unit tests (`morph` at `t=0/1/0.5`, mismatched-count rejection). All 8 pre-existing Vulkan examples re-run manually under `VK_LAYER_KHRONOS_validation`, zero errors (this step touches no RHI/vertex-format code).

**Explicitly out of scope for this sub-step:** arc-length resampling to reconcile keyframes with different vertex counts; raw Bezier-control-point morphing (re-flattening every frame); multi-keyframe timelines, easing curves, or animation-clock/frame-scheduling concerns; the stencil-and-cover fallback (task 3, Step 3.3.3); wiring into `RenderingCanvas`'s public `Canvas` API.

### Step 3.3.3: Stencil-and-Cover Fallback Rendering -- Status: Complete (2026-09-06)

Covers task 3, the last of Step 3.3's sub-steps. Both `NonZero` and `EvenOdd` fill rules are supported (the project owner's choice; the task's own example names `EvenOdd` specifically, but `NonZero` was built too).

**Stencil support became a permanent part of the shared RHI surface** (the project owner's choice, over a self-contained demo-local alternative): every `VulkanSwapchain`/`HeadlessSwapchain` now owns its own stencil image (sized to its own extent, mirroring how each already owns its own color image), `VulkanDevice::begin_frame` always attaches it, and `create_pipeline` declares a matching `stencilAttachmentFormat` internally (no public signature change -- existing pipelines simply don't enable the stencil test, matching the bindless-descriptor/push-constant "declared everywhere" precedent). A real Vulkan validation regression was found and fixed while re-verifying all 10 pre-existing examples against this change: a stencil-only image view/layout on a combined depth+stencil format needs `VK_KHR_separate_depth_stencil_layouts` (core in Vulkan 1.2) explicitly enabled at device creation -- not implied by targeting API version 1.2 alone.

New `VulkanDevice::create_stencil_and_cover_pipelines(vertex_spv, fragment_spv, color_format, fill_rule) -> Result<(VulkanPipelineState, VulkanPipelineState), EngineError>` builds a stencil-pass PSO (color writes masked off; `EvenOdd` uses a single `INVERT` stencil op regardless of triangle winding, `NonZero` uses two-sided `INCREMENT_AND_WRAP`/`DECREMENT_AND_WRAP`) and a cover-pass PSO (normal color writes; a `stencil != 0` test that resets to `0` on pass, identical for both fill rules). Both reuse the existing flat-color `walking_skeleton` shader -- no new shader needed, since the technique is entirely pipeline *state*.

New `tre-svg` module `stencil`: `fan_triangles` (anchor-at-vertex-0 fan, always succeeds, no validity check -- overlap and self-intersection are exactly what GPU stencil accumulation is designed to resolve) and `bounding_box` (the cover pass's quad extent).

A second real correctness bug (beyond the RHI regression above) was found while building this step's own verification demo: `triangulate`'s ear-validity checks (Step 3.3.1) only ever compare a candidate diagonal against the *currently remaining* boundary during clipping -- not a global "is this whole polygon simple" check. A classic pentagram (five points connected in `0,2,4,1,3` order) clipped cleanly with no diagonal ever conflicting with a remaining edge, silently producing a plausible-looking but wrong triangulation instead of being rejected. Fixed by adding `has_self_intersection`, an explicit global check (every pair of non-adjacent original edges tested for a proper crossing) that runs once before clipping starts. See `documentation/REVIEW.md`'s "Phase 3 Step 3.3.3 Implementation" section and `planning/archive/LOG_PHASE3_STEP3_3_3.md` for the full account of both bugs.

Verified by `crates/tre-rhi-vulkan/examples/stencil_and_cover_demo.rs` (`demo/phase3_step3_3_3/`): confirms `triangulate` genuinely rejects the pentagram before ever reaching stencil-and-cover, then renders it under both fill rules and reads back real pixels at the textbook-decisive point -- the pentagram's central pentagon (winding number 2, crossed an even number of times) is filled under `NonZero` but empty under `EvenOdd`, independently verified via a Python winding-number/ray-casting reference before any Rust code was written. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including a new `tre-svg` regression test for the pentagram bug. **All 10 pre-existing Vulkan examples re-run manually** under `VK_LAYER_KHRONOS_validation`, zero errors -- the elevated verification bar this step's own plan called for, since it is the first Phase 3 sub-step to touch the shared `begin_frame`/`create_pipeline` surface.

**Explicitly out of scope for this sub-step:** wiring stencil-and-cover into `RenderingCanvas`'s public `Canvas` API or any automatic triangulate-fails-so-fall-back-automatically orchestration; window-resize-time stencil image recreation (no resize support exists in this project); antialiasing the stencil-and-cover result's hard edges (a separate technique, e.g. MSAA or a signed-distance post-process).

## Phase 4: Dynamic Typography & Texture Atlasing

### Step 4.1: HarfBuzz & FreeType Integration

* **Implementation Tasks:**

  1. Integrate [HarfBuzz](https://harfbuzz.github.io/) to evaluate OpenType features, handle bi-directional text (RTL/LTR), and generate shaped glyph clusters -- via the `harfbuzz_rs` binding crate, or raw FFI where a maintained binding lags upstream.

  2. Implement a Font Fallback cascade (e.g., primary font -> system UI font -> emoji font).

  3. Extract vector control points for required glyphs using [FreeType](https://freetype.org/) (`FT_Outline_Decompose`) to feed the MSDF generator -- via the `freetype` binding crate (the `freetype-rs` project's published crates.io name).

### Status: Complete (2026-09-06)

Built as a deliberate all-pure-Rust font stack, not the literal
`harfbuzz_rs`/`freetype` bindings named above -- see
`planning/archive/PLAN_PHASE4_STEP4_1.md` for the project owner's
rationale. `rustybuzz` (a complete, faithful port of HarfBuzz's own
shaping algorithm) replaces HarfBuzz for task 1; `skrifa` (Google Fonts'
`fontations` project) replaces FreeType for task 3's outline extraction.
Neither introduces a C library dependency, so this workspace's only
remaining C ABI boundary is Vulkan itself. A new `tre-text` crate holds
all three tasks, `#![forbid(unsafe_code)]` like every other non-RHI crate.

Task 1 (shaping) is real bidi + script run segmentation
(`unicode-bidi`/`unicode-script`, since `rustybuzz::shape` itself shapes
one direction-and-script-uniform run at a time -- segmenting the input is
this project's own job) feeding `rustybuzz::shape`, producing shaped
glyphs already in correct visual (not logical) order for a mixed-direction
string -- verified against a real mixed Latin/Hebrew string, both via unit
tests on the run-segmentation boundaries and via `text_shaping_demo`
shaping it through a real installed font and confirming the Hebrew run's
glyphs come back in descending (visually reversed) cluster order.

Task 2 (font fallback) is a real `fontconfig`-driven cascade
(`FontCascade::discover`), Linux-only this step (Windows/macOS system font
APIs deferred, matching Phase 1's platform-gating precedent), querying
real installed families (`sans-serif`, `Noto Sans`, `emoji`) rather than a
hardcoded font list. `resolve_run` shapes against the primary font first
and falls through the cascade on any character the current candidate's
charmap doesn't cover -- verified against a real codepoint (the "brain"
emoji, U+1F9E0) confirmed via `fc-query`'s own charset dump to be absent
from likely primary-font resolutions and present in the emoji fallback.

Task 3 (outline extraction) is `skrifa`'s push-based outline API recorded
into an owned `Contour`/`OutlineSegment` structure mirroring
`FT_Outline_Decompose`'s own callback shape, returned as raw, unscaled
(font design-unit) control points -- this step extracts geometry only, no
rasterization at all; Step 4.2's MSDF generator is the consumer. Verified
by extracting a real glyph ('L', deliberately hole-free -- a glyph with a
counter needs multi-contour winding not built this step) from a real
font, flattening it via `tre-svg`'s now-`pub` `flatten_cubic`/
`flatten_quad` (Step 3.3.1, reused rather than reimplemented), rendering
it through the pre-existing, unmodified ear-clipping + flat-color Vulkan
pipeline, and confirming the rendered pixels match an independently
computed (in-demo, not externally pre-verified, since a real font's glyph
shape isn't known in advance the way a hand-authored SVG is) point-in-
polygon check. Extended, at the project owner's request for stronger
evidence of shaping correctness specifically (a single glyph proves
outline extraction but not layout), to also shape and render the real
word `"TEXT"` positioned entirely by `rustybuzz`'s own per-glyph advances
-- seven independently-computed probes (every letter's own bounding-box
center, plus every adjacent inter-letter gap checked against every
letter's outline) all matched the GPU render.

No MSDF rasterization, no atlas, no `RenderingCanvas` wiring, no
multi-contour/hole rendering, and no Windows/macOS font discovery this
step -- see `planning/archive/PLAN_PHASE4_STEP4_1.md`'s "Explicitly out of
scope" section for the full list; all deferred to Step 4.2 or later.

### Step 4.2: MSDF Rasterizer & Atlas Packing

* **Implementation Tasks:**

  1. Implement a 2D MaxRects Guillotine bin-packing algorithm. Maintain a list of free rectangles; upon insertion of a new glyph, find the best fit and split the remaining space horizontally or vertically. This algorithm has exactly one caller in the whole engine: the single atlas owner (ARCHITECTURE.md Section 2.3) -- it is never called concurrently and needs no internal synchronization of its own.

  2. Rasterize glyphs into an $RGB8$ buffer using [Multi-channel Signed Distance Fields (MSDF)](https://github.com/Chlumsky/msdfgen) (evaluating edge colorings to preserve sharp corners) at a fixed $32 \times 32\text{px}$ resolution.

  3. Implement the MSDF evaluation shader per the canonical formula in TECHNICAL.md Section 5.3 (median-of-channels signed distance, screen-space-derivative anti-aliasing) -- do not re-derive it here.

  4. **Multi-window atlas concurrency (ARCHITECTURE.md Section 2.3, TECHNICAL.md Section 8):** implement the bounded MPSC ring buffer that carries `AtlasInsertRequest`s from any window's tessellation phase to the atlas owner, and the fixed-capacity single-writer/multi-reader `AtomicU64` slot table the owner publishes completed `(rect, generation)` pairs into. The owner's own internal bookkeeping (the `LastFrameUsed` map driving LRU eviction, Section 10.2) is touched only by that one single-threaded owner and can safely use a plain `FxHashMap`/`ahash` map for speed -- it is a different structure from the cross-thread-visible `AtomicU64` slot table above, and swapping its hasher does not, by itself, make it safe for concurrent access.

* **Technical Rationale:** Using MSDF preserves sharp corners that traditional single-channel SDFs ruin. The Guillotine packer ensures highly efficient use of the $4096 \times 4096$ GPU atlas space.

### Step 4.2.1: Guillotine Atlas Bin-Packing -- Status: Complete (2026-09-06)

Split from Step 4.2's four largely independent tasks the same way Step 3.3 was split into 3.3.1-3.3.3 (`planning/archive/PLAN_PHASE4_STEP4_2_1.md`): this sub-step covers only task 1, the bin-packer itself. Task 2 (MSDF glyph generation, via `fdsm`, a real pure-Rust reimplementation of msdfgen's own published algorithm), task 3 (the MSDF evaluation shader plus a real anti-aliased glyph rendered end-to-end), and task 4 (multi-window atlas concurrency) are Steps 4.2.2-4.2.4.

A new `tre-atlas` crate (`#![forbid(unsafe_code)]`) -- not folded into `tre-text`, since ARCHITECTURE.md Section 2.3/DESIGN.md Section 10.2 describe the atlas as shared by MSDF glyphs *and* plain-color icon/vector-decal entries, not text-exclusive. `AtlasPacker::insert` maintains a free-rectangle list, selects the tightest-fitting free rectangle via Best Area Fit, and performs a real guillotine split into exactly two non-overlapping leftover rectangles -- chosen by actually building both candidate cuts and comparing their larger resulting piece's area, not approximated from raw leftover dimensions (a worked 100x100/90x10 example in `tre-atlas`'s own tests demonstrates why the shortcut approximation picks the wrong cut). Insertion only this sub-step -- no eviction/removal (not named in this step's task list; LRU reclamation is separate, DESIGN.md Section 10.2 future work) and no free-rectangle merging (a deliberate, documented simplification).

Verified both by unit tests (an overlap-invariant check across a varied insertion sequence, an exactly-fills-the-atlas hand-worked case, and the split-heuristic worked example above) and by `atlas_packing_demo`, which packs 12 varied rectangle sizes into a real 256x256 atlas and renders each through the pre-existing, unmodified flat-color pipeline, reading back real pixels to confirm every placement's own center is its own color and at least one unpacked point stays background.

This sub-step's own demo surfaced a real, previously-invisible engine finding, unrelated to the packer itself: `walking_skeleton.frag` never performs the sRGB-to-linear conversion `UiVertex::color`'s doc comment promises. Confirmed via TECHNICAL.md Section 6's own canonical formula that this is Step 7.1's explicit, already-scheduled job, not a regression -- not fixed here; this step's own demo instead uses only 0/255-channel colors, which round-trip correctly both before and after Step 7.1 lands. (Correction, Phase 1-4 review: this bug has actually been visibly wrong since Phase 0's `walking_skeleton` demo itself, which draws a non-invariant amber rectangle -- it went unnoticed there because that demo's own verification was a qualitative screenshot check, not because its colors were gamma-invariant. See REVIEW.md finding #92's own correction note.)

### Step 4.2.2: MSDF Glyph Generation -- Status: Complete (2026-09-06)

Second of Step 4.2's four sub-steps (`planning/archive/PLAN_PHASE4_STEP4_2_2.md`): task 2, real MSDF rasterization, via `fdsm` -- a pure-Rust reimplementation of `msdfgen`'s own published algorithm (edge coloring, true/pseudo-distance handling, sign correction, following Chlumský's thesis) rather than a from-scratch attempt at this specific, failure-prone technique. Lives in `tre-text` (not `tre-atlas`), since DESIGN.md's "Multi-Format Atlases" section is explicit that only glyphs use MSDF -- icons use plain RGBA8 -- making this a text-domain concern, unlike Step 4.2.1's packer.

`generate_msdf` converts this crate's own already-extracted `Contour`/`OutlineSegment` geometry (Step 4.1) into `fdsm`'s own `Shape`/`Segment` types (synthesizing an explicit closing segment when a contour's own points don't already meet back at their start, since `fdsm`'s `Contour` has no `Close` marker at all), fits it into a fixed `32x32`px box with a uniform (never anisotropic) scale and `4.0`px margin via a hand-built affine transform (also correcting the Y-axis flip between font design space and pixel space, which `fdsm`'s own usage example explicitly leaves uncorrected), then runs `fdsm`'s edge-coloring -> `prepare` -> `generate_msdf` -> `correct_sign_msdf` pipeline and returns a plain RGB8 `MsdfBitmap` -- geometry only, no rasterization beyond the distance field itself.

No GPU, shader, or Vulkan involvement at all this sub-step -- a deliberate change of pace, since no pipeline in this project samples an arbitrary texture yet, and inventing throwaway plumbing here would be redone properly in Step 4.2.3 anyway. Verified entirely on the CPU: unit tests cover the contour-closing logic and a hand-built shape's interior/exterior median values; `msdf_generation_demo` (living in `crates/tre-text/examples/`, the first example in this project needing no RHI at all) generates a real `'O'` glyph's MSDF -- deliberately a glyph with a true hole, the exact case Step 4.1's ear-clipping-based rendering explicitly couldn't handle -- and proves the hole survived via an independent scanline check (exactly two outside-to-inside transitions through the shape's vertical center, a signature no solid shape can produce), plus a human-viewable preview rendered by `fdsm`'s own CPU-side `render_msdf`.

**Disclosed by the Phase 1-4 review (2026-09-06):** the fixed `32x32px` resolution above is a real, previously-undiscussed quality ceiling on glyph *complexity*, not just glyph *scale*. MSDF's whole benefit is resolution-independent magnification -- it cannot invent detail a fixed 32x32 source grid never captured. A genuinely complex glyph (a dense CJK ideograph, a heavily-serifed/ligature glyph with many edges closer together than this grid can distinguish) will have distinct nearby strokes blur/merge in the distance field itself, regardless of how correct `msdf.frag`'s evaluation is. Every glyph this pipeline has actually tested (`'O'`, `'L'`, "GLYPHS") is Latin and comparatively simple, so this gap is real but not yet empirically exercised. Accepted here as a deliberate Latin/simple-script-first limitation; a larger or content-adaptive MSDF resolution for dense scripts is genuine future work, not something to retrofit opportunistically.

### Step 4.2.3: MSDF Evaluation Shader & Real Anti-Aliased Glyph Render -- Status: Complete (2026-09-06)

Third of Step 4.2's four sub-steps (`planning/archive/PLAN_PHASE4_STEP4_2_3.md`): task 3, the real MSDF evaluation shader, and the sub-step that actually resolves the jagged-'X' observation drawn from Step 4.1's demo. A real glyph's MSDF (Step 4.2.2, unmodified) finally reaches the screen with genuine, resolution-independent anti-aliasing.

No new descriptor-set or pipeline-creation code at all -- reuses Step 2.1's already-working bindless-texture infrastructure exactly as-is (`create_texture` -> `bindless_index()` -> `bind_texture` -> `draw_indexed`, the same flow `bindless_textures_demo` already established). The new `msdf.frag` is even paired with `bindless_textured.vert` *unchanged* at pipeline-creation time -- that vertex shader's inputs (position, uv, color) and push constants (`screen_size`, `texture_index`) were already exactly what MSDF sampling needs, and `build.rs` already compiles every shader file independently rather than as fixed vert/frag pairs, so no build changes beyond one new `compile_shader` call were needed.

A new `TextureFormat::Rgba8Unorm` (mapping to `vk::Format::R8G8B8A8_UNORM`, linear, no gamma) was required, not optional: an MSDF texel is a distance encoding, not a color, and sampling it through either existing `_SRGB`-family format would silently corrupt the encoded distance at every value except the two endpoints -- the same defect class Step 4.2.1's Finding #92 documented for actual color data, but worse here since it would corrupt geometry. `msdf.frag` implements TECHNICAL.md Section 5.3's exact canonical formula (median-of-channels signed distance, `fwidth`-based opacity) verbatim, not re-derived, and outputs premultiplied-alpha color matching `sdf_rounded_rect.frag`'s own convention.

Verified by `msdf_rendering_demo`: the same `'O'` glyph from Step 4.2.2, its MSDF uploaded as a real GPU texture and rendered at roughly 7x on-screen magnification -- generous enough that residual jaggedness would be obvious if present. Rather than predicting exact screen coordinates (bilinear texture filtering, already enabled on the existing bindless sampler, makes the precise transition point a function of GPU sampling), the demo scans the glyph's own center row after rendering and classifies each pixel by proximity to white vs. background: real stretches of both, plus genuinely intermediate (neither extreme) pixels at each ring-wall crossing -- the concrete, measurable signature of real sub-pixel anti-aliasing a hard binary edge could never produce. Worked correctly on the very first real GPU run, with no bugs found this sub-step. All 13 pre-existing examples re-run manually, zero validation errors.

**Disclosed by the Phase 1-4 review (2026-09-06):** this verification is magnification-only. Every texture `VulkanTexture::from_pixels` creates (MSDF atlases included) has exactly one mip level, so the shared bindless sampler's `mipmap_mode(LINEAR)` is dead configuration, and `msdf.frag`'s `fwidth`-based opacity formula has no prefiltered data to fall back on once a glyph is minified below its fixed 32x32 texel resolution -- the common case for ordinary UI body text. Naive minification of an MSDF texture doesn't just blur it: because the median-of-3-channel decode is nonlinear, it can locally corrupt the encoded distance near corners/thin strokes, and a real fix needs MSDF-aware mip generation (a naive box filter also corrupts the median encoding), not just enabling more mip levels. Documented here as a known, currently-untested limitation rather than built opportunistically alongside an unrelated review pass -- future work, not a regression in this step's own magnification-only verification.

### Step 4.2.4: Multi-Window Atlas Concurrency -- Status: Complete (2026-09-06)

The closing sub-step of the whole Step 4.2 arc (`planning/archive/PLAN_PHASE4_STEP4_2_4.md`): task 4, wiring 4.2.1-4.2.3 together behind ARCHITECTURE.md Section 2.3/TECHNICAL.md Section 8's real concurrency model, so multiple independent producers can request atlas space concurrently without ever blocking each other or the single atlas owner.

Two new generic primitives in `tre-memory` (matching that crate's own doc comment, written before this step existed, and TECHNICAL.md Section 9.1's `unsafe`-policy grouping): `MpscRingBuffer<T>` (Dmitry Vyukov's well-known bounded MPMC ring buffer design, simplified for a single consumer) and `SwmrSlotTable<K>` (open-addressed, needs zero `unsafe` at all -- both key and value per slot are plain `AtomicU64`s). `tre-atlas` supplies only what's genuinely atlas-specific on top: `AtlasKey`, the `(rect, generation)` `u64` packing, and `AtlasOwner` -- a second real, dedicated background OS thread (Phase 2 Step 2.3's GC thread precedent, generalized) that drains requests, performs the real Guillotine packing and MSDF generation, and publishes results. `tre-atlas` still never depends on `tre-text`: `AtlasInsertRequest` carries a boxed `RasterSource` trait object (`size()`/`rasterize()`), keeping the crate exactly as content-agnostic as Step 4.2.1 first established.

Verified by real concurrent stress throughout, not simulated with one thread standing in for many: `MpscRingBuffer` passed an 8-thread/80,000-item test with zero loss or duplication on the first attempt; `SwmrSlotTable` passed a real two-thread test confirming a reader can never observe a key before its value is fully published; `AtlasOwner` itself passed a real 6-producer-thread round trip. The capstone demo (`atlas_concurrency_demo`) spawns 3 real producer threads requesting MSDF space for the 6 letters of `"GLYPHS"` from a real cascade font concurrently, verifies every placement is non-overlapping and byte-identical to an independently regenerated MSDF for that same glyph, then uploads the finished shared atlas as one real GPU texture and renders every letter in a single draw call via the unmodified `msdf.frag` pipeline (Step 4.2.3) -- reading back real pixels confirming the word actually reads "GLYPHS."

Two real, minor findings surfaced by this step's own demo (neither in the concurrency primitives themselves, both already fixed): `thread::yield_now()` is only a scheduler hint, not a real wait, and a tight polling loop built on it burned through its entire retry budget without this environment's scheduler ever switching to the atlas owner thread -- fixed with a real `sleep`-based backoff. Separately, checking only a glyph's bounding-box center to verify it rendered repeated Step 4.1's own `'L'`-shaped lesson (an open-counter glyph like `'G'` can have background sitting exactly at its own bbox center) -- fixed by scanning each letter's whole on-screen quad for any non-background pixel instead.

**Disclosed by the Phase 1-4 review (2026-09-06), not by this step's own original status text:** task 4's own scope, quoted above, explicitly includes "the `LastFrameUsed` map driving LRU eviction, Section 10.2" -- this was never actually built. `owner.rs`'s `process_insert` silently and permanently drops a request the packer can't currently fit (no eviction/reclamation exists anywhere in `tre-atlas`), and unlike Step 4.2.1's own honest "LRU reclamation is separate, DESIGN.md Section 10.2 future work" disclosure two sub-steps above, this step's status text declared "This closes Step 4.2 ... and, with it, all of Phase 4" without mentioning that omission. DESIGN.md Section 2.6's placeholder-fallback story implies eventual resolution once eviction frees space; without eviction, a full atlas makes that fallback *permanent*, not "for this frame," for any new key requested from that point on. Tracked here as genuine future work (implementing real LRU eviction, not a fix to apply opportunistically alongside an unrelated review pass); see REVIEW.md's Phase 1-4 review entry for the full finding.

This closes Step 4.2 (4.2.1-4.2.4) and, with it, all of Phase 4 as originally scoped (Steps 4.1 and 4.2 both complete). Phase 4 was later extended with Step 4.3 below, added in direct response to the disclosed gap immediately above (the Phase 1-4 review's finding #114).

### Step 4.3: Atlas LRU Eviction

* **Implementation Tasks:**

  1. Give `tre_memory::SwmrSlotTable` real per-key removal that stays sound for concurrent lock-free readers (tombstone deletion, DESIGN.md Section 10.2's eviction eventually needs to invalidate a resident entry without ever blocking a reader), plus per-entry recency tracking (the `LastFrameUsed` concept Section 10.2 names) so a future policy can identify stale entries.

  2. Give `tre_atlas::AtlasPacker` a way to give back a rectangle it previously handed out, so an evicted glyph's atlas space can be reused by a later insertion instead of being lost to fragmentation forever.

  3. Wire both into `AtlasOwner`: the actual policy DESIGN.md Section 10.2 describes -- when atlas capacity exceeds 85%, evict entries idle for $N \ge 600$ frames -- plus a real demo proving eviction and re-insertion both work correctly end to end.

* **Technical Rationale:** Closing the Phase 1-4 review's finding #114 (LRU eviction was named in Step 4.2's own original task list but never built, leaving a full atlas to drop new glyph requests forever instead of degrading gracefully). Split into three sub-steps the same way Step 4.2 itself was, since giving a lock-free table real removal is a genuine concurrent-data-structure change independent of the packer's own space-reclamation problem, and both need to exist before a real eviction policy can combine them.

### Step 4.3.1: SwmrSlotTable Tombstone Deletion & Recency Tracking -- Status: Complete (2026-09-06)

First of Step 4.3's three sub-steps (`planning/archive/PLAN_PHASE4_STEP4_3_1.md`): task 1, giving `SwmrSlotTable` the two primitive capabilities real eviction needs that it didn't have before -- removing an entry without breaking lock-free reads, and tracking which entries are actually still being used. No `tre-atlas`/RHI surface touched at all this sub-step; the new capabilities aren't wired into the real atlas owner until Step 4.3.3.

`SwmrSlotTable` gained a second reserved sentinel, `TOMBSTONE_KEY` (distinct from the existing `EMPTY_KEY`), and classic linear-probing-with-tombstones: `get`/`get_and_touch` now stop only at a genuine `EMPTY_KEY` (proof a key was never inserted) rather than at a tombstone (which proves nothing about whether the key exists further along the same probe sequence), and `insert` reuses the first tombstone-or-empty slot it passes once it's confirmed the key isn't already present elsewhere in its probe chain -- not a novel scheme, the standard textbook technique for open addressing with deletion. `remove(key) -> bool` stores the tombstone; both `insert` and `remove` remain writer-exclusive, same as before, so the lock-free-reader guarantee is unaffected. Recency tracking (`get_and_touch(key, frame) -> Option<value>`) is a genuinely separate mechanism from the key/value payload -- a parallel `last_used: Box<[AtomicU64]>`, updated via `AtomicU64::fetch_max` so any number of concurrent reader threads can safely race to record "still in use as of this frame" with no ordering coordination needed. A new writer-only `scan_older_than(cutoff_frame, visit)` enumerates stale entries for a future eviction policy to act on.

A real bug surfaced and fixed during implementation, not just designed around in the abstract: the first version of `insert`'s tombstone-reuse logic only claimed a remembered tombstone when the probe *also* reached a genuine `EMPTY_KEY` first -- once a table has cycled through enough remove/insert pairs that zero `EMPTY_KEY` slots remain at all (every slot either a live key or a tombstone), the loop exhausted its full capacity without ever triggering that branch and incorrectly reported the table full. Caught immediately by a dedicated unit test (`a_slot_freed_by_remove_does_not_permanently_shrink_capacity`) built specifically for that condition; fixed by checking the remembered tombstone once, after the probe loop ends, regardless of how it ended. Full detail in `planning/archive/LOG_PHASE4_STEP4_3_1.md`.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 12 unit tests in `tre-memory`'s `swmr` module (up from 7), including a new concurrent-stress test (6 reader threads doing 20,000 rounds of `get_and_touch` each while one key is concurrently removed, confirming no reader ever observes a torn or corrupted value) matching this module's own pre-existing testing discipline rather than a new style introduced for this sub-step. No new example this sub-step -- `SwmrSlotTable` has no RHI/visual surface of its own, and Step 4.2.4 already established unit tests alone as sufficient proof for this module. All 15 pre-existing examples remain unaffected (nothing in their dependency graph calls the new methods yet).

### Step 4.3.2: AtlasPacker Free-Rectangle Reclamation -- Status: Complete (2026-09-06)

Second of Step 4.3's three sub-steps (`planning/archive/PLAN_PHASE4_STEP4_3_2.md`): task 2, giving `tre_atlas::AtlasPacker` a way to give back a rectangle it previously handed out, so an evicted glyph's or icon's atlas space becomes real, reusable space rather than being lost forever. 4.3.1 gave `SwmrSlotTable` the ability to forget an entry; this sub-step gives the packer the matching ability to forget a placement. No `AtlasOwner`/RHI surface touched this sub-step -- neither new method is called from anywhere real yet.

`AtlasPacker` gained `remove(rect)` (pushes `rect` back onto the free-rectangle list, unmerged -- a direct extension of Step 4.2.1's own original "no free-rectangle merging" simplification, not a new compromise; real coalescing stays deferred until a concrete fragmentation problem actually shows up in real usage) and `used_fraction() -> f64` (a plain read of how much of the atlas's total area is currently placed vs. free, computed against a `total_area` cached at construction rather than recomputed from a free rectangle that stops existing the moment it's first split). `remove` trusts its caller to only pass back a rectangle this exact packer instance previously returned -- no internal registry of "currently placed" rectangles exists to validate against, the same trust boundary `insert` already has, safe specifically because of ARCHITECTURE.md Section 2.3's single-atlas-owner design. `used_fraction` itself makes no threshold decision -- DESIGN.md Section 10.2's 85%-capacity trigger and the eviction policy that reads this value are Step 4.3.3's job.

No bugs this sub-step -- every design decision locked into the plan (unmerged reclamation, a cached `total_area`, `saturating_sub` guarding a hypothetical caller-bug underflow) held up on the first real test run. Full detail in `planning/archive/LOG_PHASE4_STEP4_3_2.md`.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 16 unit tests in `tre-atlas` (up from 11), including a worked exact-fill/free/refill example mirroring Step 4.2.1's own original testing style, and an explicit test confirming two adjacent freed rectangles do *not* silently merge (the accepted no-merge behavior as a tested property, not just an unstated gap). No new example this sub-step, matching 4.3.1's own precedent. `atlas_packing_demo` and `atlas_concurrency_demo` (the two examples that exercise `AtlasPacker`/`AtlasOwner` directly) re-run for real against actual Vulkan hardware as a regression check: zero validation errors, all existing assertions still pass.

### Step 4.3.3: Atlas LRU Eviction Policy & Wiring -- Status: Complete (2026-09-06)

Third and closing sub-step of Step 4.3 (`planning/archive/PLAN_PHASE4_STEP4_3_3.md`): task 3, wiring 4.3.1's `SwmrSlotTable` (forget an entry, track recency) and 4.3.2's `AtlasPacker` (forget a placement, report capacity) into `AtlasOwner`'s real eviction policy -- closing the Phase 1-4 review's finding #114. This closes Step 4.3 (4.3.1-4.3.3) in full.

`AtlasInsertRequest` gained a `current_frame: u64` field, and both `AtlasOwnerHandle::request_insert` and `::lookup` now take a `current_frame: u64` parameter -- a refinement discovered during this sub-step's own planning, since 4.3.1 only anticipated `lookup` needing one: deciding whether to evict has to happen on the owner's own background thread, which otherwise has no notion of "what frame is it" at all, so the frame number needed threading through the insert path too. `process_insert` now calls a new `maybe_evict_stale_entries` before its existing packing logic: a no-op below DESIGN.md Section 10.2's 85%-capacity threshold (`AtlasPacker::used_fraction()`); otherwise, every entry whose recency is older than `current_frame - 600` (Section 10.2's own "N >= 600 frames") is evicted in one pass, pairing `SwmrSlotTable::remove` with `AtlasPacker::remove` per entry so neither a leaked rect nor a resurrected key with no backing space can result. `AtlasKey` gained `From<u64>` (the reverse of the existing `From<AtlasKey> for u64`) to reconstruct keys from `scan_older_than`'s raw output.

A cold-start hazard was identified and fixed during implementation, not left to be discovered by a failing test: `SwmrSlotTable::insert` resets a freshly claimed slot's recency to `0`, which would otherwise make a glyph inserted this very frame look "not rendered within the last 600 frames" the instant a *later* insert crosses the capacity threshold -- evicting brand-new content before it's ever used. Fixed by having `process_insert` call `slots.get_and_touch(key, request.current_frame)` immediately after a successful insert, treating creation as an access, the same way a real LRU cache does.

The packed `generation` counter was reconsidered from what this project's own REVIEW.md finding #114 disposition had speculated ("Step 4.3.3 is where a real eviction actually bumps it") and deliberately left at `0`: nothing in this codebase reads or depends on it changing, and building the persistent per-key bookkeeping a meaningful bump would require is complexity with no current consumer to justify it -- deferred indefinitely, not merely to this step, per this project's own "don't build what the task doesn't need" discipline.

No bugs beyond the cold-start hazard identified during planning and fixed during implementation -- every other design decision (the exact atlas/frame-number geometry chosen for both the unit tests and the demo) held up on the first real run. Full detail in `planning/archive/LOG_PHASE4_STEP4_3_3.md`.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 19 unit tests in `tre-atlas` (up from 16), including three new tests precisely engineered around the real `0.85`/`600`-frame thresholds (capacity, not just age, gates eviction; a freshly-inserted entry survives the very next capacity-crossing insert; a full evict-then-reinsert scenario spares a touched entry). The new capstone example, `atlas_eviction_demo` (a real 64x64 atlas exactly holding four 32x32 MSDF glyphs, one kept fresh and three left stale, a fifth request 700 frames later triggering real eviction, then a real GPU render of the two survivors through the unmodified `msdf.frag` pipeline), passed on its first real run: zero Vulkan validation errors, all assertions passed. All 16 examples (the 15 pre-existing plus the new one) re-run manually end to end, zero regressions from the `request_insert`/`lookup` signature change. Added to the `vulkan-validation` CI job.

## Phase 5: Multi-Threaded Canvas API & Metadata

### Step 5.1: The Canvas Command Recorder

* **Implementation Tasks:**

  1. Build the `RenderingCanvas` API with Drawing Contexts (`draw_rect`, `draw_text`, `push_layer`, `save`, `restore`).

  2. Define the lightweight Intermediate Representation (IR) struct (`UiDrawCommand`), containing a `kind` enum, 64-bit sort key, clip bounds, and geometry offsets.

  3. Implement Overlay routing logic: When `begin_overlay()` is called, assign commands a Layer ID $\ge 10000$ and reset the active clip stack to the native window bounds.

* **Technical Rationale:** `RenderingCanvas` already existed as a real Phase 0 stub (`draw_rounded_rect`, `push_layer`/`pop_layer`, the exact `UiDrawCommand`/`CommandType` structs ARCHITECTURE.md Section 3.2 specifies) -- this step turns that stub into the real Drawing Context, split into three sub-steps the same way Steps 3.3/4.2/4.3 were: 5.1.1 (the state stack), 5.1.2 (`draw_text`, the first real `tre-engine` -> `tre-text`/`tre-atlas` wiring), 5.1.3 (the real sort key, `begin_overlay`, and real batch flattening).

### Step 5.1.1: Canvas Drawing-Context State Stack -- Status: Complete (2026-09-07)

First of Step 5.1's three sub-steps (`planning/archive/PLAN_PHASE5_STEP5_1_1.md`): task 1's state-stack half, wired into the one real primitive `RenderingCanvas` already had. DESIGN.md Section 6's own architecture diagram lists `save`/`restore` (a "Drawing Context State Stack: Matrix Transform, Alpha, Blend") and `push_clip`/`pop_clip` (a separate "Dynamic Scissor / Mask Clip Stack") as two distinct mechanisms -- `RenderingCanvas` now has both, as genuinely separate stacks, not one bundled state object.

`save`/`restore` snapshot a `CanvasState { transform: Affine2, alpha: f32 }` (blend mode stays deferred alongside `LayerDesc`'s own visual-filter fields); two new mutator methods not named in IMPLEMENTATION.md's own task list but required for `save`/`restore` to have anything real to protect -- `Canvas::transform(&Affine2)` (composes via `Affine2::compose`, Phase 3 Step 3.1, reused directly) and `Canvas::set_alpha(f32)` (multiplies the current effective alpha, so nested group opacity compounds correctly). `push_clip`/`pop_clip` intersect scissor rects on a separate stack and emit real `PushScissor`/`PopScissor` commands -- variants that have existed in `CommandType` since Phase 0 but had never once been emitted before this step. `draw_rounded_rect` itself is rewired to apply the active transform to its emitted vertex positions (not `uv`/`params`, which must stay in local SDF-space regardless of world transform), scale its color by the active alpha, and carry the active clip rect in its `UiDrawCommand`.

A real bug surfaced by actually running the new demo, not caught by unit tests alone: the first `scale_alpha` draft scaled only the vertex color's alpha byte, leaving RGB at full brightness. `sdf_rounded_rect.frag`'s own output (`vec4(frag_color.rgb * coverage, frag_color.a * coverage)`) never multiplies `frag_color.rgb` by `frag_color.a` -- `UiVertex::color` must already be *premultiplied* before it reaches that shader, so an alpha-only reduction produced an over-bright premultiplied value the GPU silently clamped back to fully opaque, rendering every `set_alpha()` call invisible rather than merely imprecise. Fixed by scaling all four channels together (renamed `premultiply_alpha`) and documented directly on `UiVertex`'s own canonical definition (ARCHITECTURE.md Section 3.1) as a convention any future primitive writing `color` must follow. A second, independent bug in the new demo's own test design (a check point that happened to fall inside a different rect's footprint) was found and fixed alongside it. Full detail in `planning/archive/LOG_PHASE5_STEP5_1_1.md`.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 20 unit tests in `tre-engine` (up from 12), covering save/restore/transform/set_alpha composition and push_clip/pop_clip intersection. The new capstone example, `canvas_state_stack_demo`, proves what actually reaches the GPU with real pixels (a transformed world-space placement, a genuine visible partial-alpha blend) and checks `push_clip`/`pop_clip` at the IR level only, since nothing in the render pipeline consumes `clip_bounds` yet (that wiring is Step 5.1.3/Phase 6's job) -- not a GPU scissor test that doesn't exist. `sdf_rounded_rect_demo` (the only pre-existing caller of `draw_rounded_rect`) re-run manually, confirming the rewired logic is a genuine no-op at the default identity/full-alpha/no-clip state. All 17 examples (16 pre-existing plus the new one) re-run manually end to end, zero validation errors.

### Step 5.1.2: Canvas Text Rendering (`draw_text`) -- Status: Complete (2026-09-07)

Second of Step 5.1's three sub-steps (`planning/archive/PLAN_PHASE5_STEP5_1_2.md`): `tre-engine`'s first real cross-crate wiring into `tre-text`/`tre-atlas`. `Canvas::draw_text(shaped, font, font_id, origin, px_size, rgba, atlas_context)` takes a single already-shaped `tre_text::ShapedRun` and a resolved `skrifa::FontRef` -- a concrete, minimal signature in place of DESIGN.md's abstract `DynamicTextLayout`/`Paint` sketch, whose referenced types don't exist anywhere in the codebase, matching 5.1.1's own precedent of adapting doc sketches to what's concretely buildable. Font-fallback cascade resolution and multi-run/bidi paragraph handling stay the caller's job (`tre_text::resolve_run`/`segment_runs` already do this); `draw_text` renders exactly one run against one font.

Per glyph: a real `AtlasOwnerHandle::lookup` decides cache hit or miss. A **hit** emits one real textured `DrawGeometry` command -- a fixed `px_size`-square quad (matching the MSDF atlas entry's own fixed square aspect exactly; true per-glyph bounding-box-accurate sizing is an explicit, documented simplification left out of scope, same as `atlas_concurrency_demo` already accepts), current transform/alpha/clip state applied exactly as `draw_rounded_rect` already does, and a new `PIPELINE_MSDF_TEXT` pipeline id (the first real distinction between pipeline ids in the IR; `0` stays the implicit SDF-rect pipeline). A **miss** fires a real `request_insert` and renders nothing this frame -- DESIGN.md Section 2.6's placeholder-glyph fallback's minimal honest half, since "not yet requested," "requested but unresolved," and "evicted" are all indistinguishable from `lookup`'s own documented contract. A glyph with no real ink (whitespace) is filtered by outline emptiness before ever touching the atlas at all -- `tre_text::msdf`'s own doc comment already establishes this as the common, non-error case for e.g. U+0020 SPACE, sidestepping any need for `RasterSource::rasterize()`'s non-optional signature to see an empty/`None` case.

`tre-text` gained a new dependency on `tre-atlas` (the allowed direction -- `tre-atlas` itself stays content-agnostic, never depending back): the demo-only `GlyphRasterSource` glue, previously duplicated identically in `atlas_concurrency_demo.rs` and `atlas_eviction_demo.rs`, is promoted into real, reusable code (`tre_text::GlyphRasterSource`), and both demos updated to use it instead of their own local copies. `GlyphAtlasContext` bundles the four atlas-related parameters (`atlas`, `texture_handle`, `dimensions`, `current_frame`) a caller must otherwise thread through separately -- a correctness gap found during implementation, not in the original plan sketch: UV normalization needs the atlas's own pixel dimensions, which the plan's flat parameter list had omitted.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 4 new unit tests in `tre-engine` (24 total, up from 20) against a real system font and a real `AtlasOwner` background thread, covering cache-hit pen-advance correctness, cache-miss request firing, whitespace skipping, and transform/alpha/clip composition. The new capstone example, `canvas_draw_text_demo`, shapes and renders the real word "TEXT" end to end: a first `draw_text` call proves every glyph is a genuine cache miss (zero commands emitted); after polling the real atlas to resolution, a second call proves every glyph is now a cache hit (one real textured command per glyph), rendered through the existing, unmodified `bindless_textured.vert`/`msdf.frag` pipeline (Step 4.2.4) and read back as real, non-background GPU pixels per glyph. `atlas_concurrency_demo`/`atlas_eviction_demo` (the two demos touched by the `GlyphRasterSource` promotion) re-run manually, confirming their own existing assertions still pass unchanged.

### Step 5.1.3: Real Sort Key, Overlay Routing, Real Batch Flattening -- Status: Complete (2026-09-07)

Third and closing sub-step of Step 5.1 (`planning/archive/PLAN_PHASE5_STEP5_1_3.md`) -- the capstone, needing both real draw kinds/pipelines 5.1.1/5.1.2 built (`draw_rounded_rect`, `draw_text`) to be meaningfully provable. Three pieces:

**The real 64-bit sort key.** Every `DrawGeometry` command's `sort_key: 0` placeholder is replaced by `compute_sort_key(layer_id, pipeline_state_id, texture_handle, depth_id)`, packing ARCHITECTURE.md Section 4.1's canonical `(LayerID<<48)|(PipelineID<<32)|(TextureID<<20)|(DepthID)` layout, with debug asserts on the 12-bit Texture ID and 20-bit Depth ID fields (the latter is the exact overflow assert Section 4.1 itself documents as required). Depth ID is a single global, monotonically increasing counter (`next_depth_id`) incremented once per `DrawGeometry` command -- there is no widget tree or z-index resolver above this imperative `Canvas` API to derive a richer traversal-order index from, so call order is the only ordering this layer of the stack has.

**`Canvas::begin_overlay`/`end_overlay`.** DESIGN.md Section 7.2's `Canvas::begin_overlay(OverlayLayerPriority)` referenced a type that didn't exist in code; `OverlayLayerPriority(pub u16)` is now defined concretely, added to `OVERLAY_LAYER_BASE = 10_000` to produce the real Layer ID. `begin_overlay` pushes the resulting Layer ID onto a new `overlay_stack`, saves and resets the clip stack to "no clip, full window" (DESIGN.md Section 7.2's "decoupled from the parent container's scissor stack"), and emits a real `PushScissor` marker; `end_overlay` restores the exact prior clip and pops. Deliberately unrelated to `push_layer`/`pop_layer`'s own offscreen-compositing mechanism -- two different, easily-conflated meanings of "layer" that this step keeps strictly separate.

**Real batch flattening.** `flatten()`'s Phase 0 trivial pass-through is replaced with ARCHITECTURE.md Section 4.2's own three-step algorithm: every non-`DrawGeometry` command (`PushScissor`/`PopScissor`/`PushLayer`/`PopLayer`) is a hard barrier that sorting/merging never crosses -- the conservative reading of Section 4.2's own "single draw call per layer plane" *soft* target, which already defers cross-clip-boundary batching to a future, measurement-driven pass. Within each marker-free run, `flatten_run` sorts by `sort_key` (safe as an unstable sort, since `next_depth_id` guarantees no two commands in a frame ever tie) and merges adjacent commands sharing Layer+Pipeline+Texture (the key's top 44 bits) and identical `clip_bounds`, rewriting `indices` into a freshly built contiguous buffer -- `vertices` never moves, since every index is an absolute reference into it.

A real, predicted-in-`PLAN.md` consequence: any two adjacent `DrawGeometry` commands sharing Layer+Pipeline+Texture+`clip_bounds` (previously always emitted 1:1 per `draw_rounded_rect`/`draw_text` call) now merge. Two pre-existing files needed a real fix, not just a recompile: `canvas_state_stack_demo.rs`'s `RECT_C_COMMAND_INDEX` shifted from `3` to `2` (Rect A and Rect B now merge into one command), and `canvas_draw_text_demo.rs`'s frame-2 assertion ("one command per glyph") became "all 4 glyphs merge into one command" -- both caught by re-running every pre-existing example, not just by the type checker.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 36 unit tests in `tre-engine` (up from 24), including a direct reproduction of DESIGN.md Section 8's own worked example (`Rect1 -> Text -> Rect2 -> OverlayRect` collapsing to exactly 3 batches) and a white-box test of `flatten_run`'s `clip_bounds` check (unreachable via the public `Canvas` API today, since `clip_bounds` only ever changes alongside a marker -- kept as a regression guard). The new capstone example, `canvas_batch_flattening_demo`, is the first in this codebase to record more than one `draw_indexed` call and switch pipelines within a single frame, proving the merged batches still render all 4 logical shapes at their own correct, distinct positions. Every pre-existing example re-run manually end to end; added to the `vulkan-validation` CI job. This closes Step 5.1 (5.1.1-5.1.3) in full.

### Step 5.2: Multi-Threading & Lock-Free Sub-Canvases

* **Implementation Tasks:**

  1. Allocate a fixed-size `CommandArena` for every worker thread created by `create_sub_canvas()`.

  2. Thread $i$ writes $N_i$ commands to its local arena without any locking.

  3. During the flattening phase, the main thread computes the final global array location using atomic operations:
     `let write_offset = global_command_counter.fetch_add(n_i, Ordering::Relaxed);`

  4. Worker threads bulk-copy (`copy_from_slice`) their local arenas to the global arena at their acquired offset.

* **Technical Rationale:** The lock-free atomic merge ensures that stitching time remains practically zero (sub-microsecond), maximizing the benefits of multi-threaded UI traversal. Split into three sub-steps the same way Steps 3.3/4.2/4.3/5.1 were: 5.2.1 (`create_sub_canvas()` and thread-local recording -- nothing else is meaningfully testable without real `SubCanvas` output to work with), 5.2.2 (the real lock-free stitching primitive and wiring it into a multi-source `flatten`), 5.2.3 (the capstone: real OS worker threads recording concurrently, merged and rendered as real GPU pixels).

### Step 5.2.1: SubCanvas & Thread-Local Recording -- Status: Complete (2026-09-07)

First of Step 5.2's three sub-steps (`planning/archive/PLAN_PHASE5_STEP5_2_1.md`). `SubCanvas` is a thin wrapper around a private `RenderingCanvas` (`Deref`/`DerefMut` to it), not a second, parallel type -- every existing drawing method (`save`, `push_clip`, `begin_overlay`, `draw_rounded_rect`, `draw_text`) works on a `SubCanvas` unchanged, since a worker thread records into exactly the same kind of thread-local linear arena the root canvas already does.

The one field that must become genuinely shared is Depth ID: Step 5.1.3 made it a single monotonic `u32` counter specifically because no two `DrawGeometry` commands in a frame may ever share a sort key, and a plain per-thread counter starting at `0` in every worker would immediately violate that the instant two sub-canvases each recorded a first command. `next_depth_id` is promoted to `Arc<AtomicU32>`, cloned into every `SubCanvas` `create_sub_canvas()` produces -- `fetch_add`'s own atomicity is what keeps every command's Depth ID globally unique regardless of which thread recorded it. Layer ID has no such requirement (it only distinguishes standard content from the overlay plane) and stays exactly as private/thread-local as `state_stack`/`clip_stack` already were.

`create_sub_canvas()` enforces TECHNICAL.md Section 8's `available_parallelism() - 1` concurrency cap with an exact, panic-safe live count: a compare-exchange loop checks the cap before committing the increment (not fixing it up afterward), and `SubCanvas`'s own `Drop` impl decrements it -- exact even if a caller panics mid-use and that panic is later caught (TECHNICAL.md Section 9.4's `catch_unwind` FFI boundary means a caught panic doesn't necessarily end the process, so an approximate count could permanently wedge future sub-canvas creation). A `#[cfg(test)]`-only constructor overrides the cap with a small, deterministic value, so this panic behavior is testable independent of the test runner's real core count.

No merging or data extraction yet -- `RenderingCanvas::flatten(self)` takes `self` by value, which `Deref`/`DerefMut` cannot forward, and this sub-step adds no escape hatch of its own. That's Step 5.2.2's job.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: 41 unit tests in `tre-engine` (up from 36), including a real multi-thread stress test (4 real OS threads via `std::thread::spawn`, matching `MpscRingBuffer`'s and `atlas_concurrency_demo`'s own established "real threads, not a synthetic simulation" precedent) confirming zero Depth ID collisions across 200 concurrently-recorded commands, the cap's exact panic behavior, slot reuse after a `SubCanvas` is dropped, and that Layer ID sharing across sub-canvases is not an error. Zero new dependencies -- `Arc`/atomics/`available_parallelism` are all `std`. No new demo this sub-step (matching Steps 4.3.1/4.3.2's own precedent -- a `SubCanvas` alone has no way to be rendered yet; the real end-to-end GPU proof lands in the 5.2.3 capstone). All 19 pre-existing examples re-run manually as a regression check, since `next_sort_key`'s internal change touches a shared code path every example goes through.

### Step 5.2.2: Real Lock-Free Stitching -- Status: Complete (2026-09-07)

Second of Step 5.2's three sub-steps (`planning/archive/PLAN_PHASE5_STEP5_2_2.md`). 5.2.1 proved concurrent thread-local recording was safe but deliberately gave `SubCanvas` no way to hand its data back; this sub-step is that missing half.

`tre_memory::ScatterArena<T>` is a new lock-free primitive, following `MpscRingBuffer`'s own established `UnsafeCell<MaybeUninit<T>>` pattern rather than inventing a new one -- simpler, since nothing here is ever popped or reused mid-frame: one atomic bump counter (`fetch_add(count)`) hands out permanently-exclusive, disjoint ranges; `into_vec(self)` consumes the arena once, after every writer has finished, to read out the actually-written prefix. The key design choice: workers stitch *themselves*, not the coordinator -- `SubCanvas::stitch_into` is meant to be a worker thread's own last action before it exits, matching IMPLEMENTATION.md's own task-list wording ("worker threads bulk-copy... to the global arena at their acquired offset") rather than a sequential post-join merge, which would need no atomics at all and wouldn't earn the word "lock-free."

`stitch_into` performs copy-and-rebase, not a literal `copy_from_slice` of everything: vertices copy directly, but a raw index value is an absolute offset into the vertex buffer and a command's `vertex_offset` is an absolute offset into the index buffer, so each is shifted by however much space the *previous* reservation actually claimed -- discoverable only once that reservation's own `fetch_add` returns its start position. `FrameArena::flatten` then reuses Step 5.1.3's exact sort/merge algorithm (extracted into a shared `segment_and_flatten` free function `RenderingCanvas::flatten` also calls, zero behavior change for the single-canvas path) over the three arenas' merged contents.

Extracting a `SubCanvas`'s inner data resolved 5.2.1's own deferred question: `SubCanvas` implements `Drop`, and Rust forbids partially moving fields out of any `Drop` type, so `SubCanvas::stitch_into` uses `std::mem::take` to swap the real `RenderingCanvas` out for a throwaway placeholder before `self`'s own `Drop` runs normally afterward, releasing its `live_sub_canvases` slot exactly as any other drop would.

Arena overflow (a source's data would exceed a `ScatterArena`'s fixed capacity) returns `false` rather than growing -- DESIGN.md Section 2.6 already names the real long-term policy ("drops the lowest-priority pending draw commands... and reports a frame-budget diagnostic"), but implementing that actual priority ordering is a separate, larger feature; this sub-step only makes the failure observable.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: `tre-memory` gained 5 new tests (one a real 8-thread/4,000-item concurrent stress test for `ScatterArena`, matching `MpscRingBuffer`'s own precedent); `tre-engine` gained 3 new tests (up to 44 total from 41), including an exact-value rebasing check (two sub-canvases' rects stitched in sequence, hand-verifying the second source's indices shifted by exactly the first source's vertex count) and a real 4-worker-thread test in which each thread stitches its own `SubCanvas` into a shared `FrameArena` as its own last action, with the resulting frame's vertices and merged command count both confirmed correct regardless of thread scheduling order. No new demo this sub-step (matching 5.2.1's own precedent) -- the real end-to-end GPU proof is Step 5.2.3's capstone job. All 19 pre-existing examples re-run manually, zero regressions.

### Step 5.2.3: The Capstone -- Real Concurrent Recording, Rendered -- Status: Complete (2026-09-07), closing Step 5.2 in full

Third and closing sub-step of Step 5.2. Every mechanism this capstone needs already existed and was already unit-tested (`SubCanvas`/`create_sub_canvas`, Step 5.2.1; `tre_memory::ScatterArena`/`FrameArena`/`stitch_into`, Step 5.2.2) -- this sub-step added one trivial accessor (`RenderingCanvas::max_sub_canvases`, so a real caller can size its own worker-thread count against the actual configured cap instead of guessing) and proved the whole chain end to end with real OS threads and a real GPU render, the same role `atlas_concurrency_demo`/`atlas_eviction_demo` already played for their own features.

The new demo, `canvas_sub_canvas_demo`, spawns `min(available_parallelism() - 1, 4)` real worker threads, each holding its own `SubCanvas`: every one draws a rect, thread `0` draws inside a `begin_overlay`/`end_overlay` bracket, and the last thread also draws a real atlas-backed MSDF glyph. Every thread calls `stitch_into` on a shared `FrameArena` as its own last action before exiting -- the real "workers stitch themselves" pattern Step 5.2.2 was designed around, exercised for the first time under genuine concurrency rather than only in a unit test. The root canvas draws one more rect directly and stitches into the same arena, sharing its Depth ID counter with every worker (a real bug in this demo's own first draft used a second, independent root canvas for that rect, whose separate counter collided with a worker's -- fixed by drawing on, and later stitching, the very same `root` every `create_sub_canvas()` call already shared).

A genuinely new, honest property surfaced that no single-threaded demo could have: `canvas_batch_flattening_demo` (Step 5.1.3) always collapses to exactly 3 batches, but here the overlay worker's `begin_overlay`/`end_overlay` markers are hard run-segmentation barriers, and *which* other threads' plain rects land before vs. after that marker pair in the concurrently-stitched command array depends on real thread scheduling -- ARCHITECTURE.md Section 4.2's own "soft target, not a guarantee" caveat, actually exercised instead of only cited. The demo's own verification was revised accordingly: it asserts the batch *count* for the plain rects can vary (observed 1-2 across repeated runs on a 24-core machine), while the *content* (every rect's own 6 indices accounted for somewhere) and the hard guarantees (the overlay rect and the text glyph each always stay their own, never-merging batch) remain exact.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, and 15+ real runs of the new demo confirming the batch-count variability directly while every pixel and content assertion held every time. All 18 pre-existing examples re-run manually, zero regressions. Added to the `vulkan-validation` CI job. **This closes Step 5.2 (5.2.1-5.2.3) in full.**

### Step 5.3: Spatial Accessibility Tagging (a11y)

* **Implementation Tasks:**

  1. Implement `Canvas::tag_accessibility_node(node_id, bounds, role_flags)`.

  2. Construct a metadata extractor that runs in parallel with the RHI submission phase.

  3. Map the extracted boundaries directly to OS-level accessibility trees (e.g., implementing [`IRawElementProviderSimple`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationcore/nn-uiautomationcore-irawelementprovidersimple) for Windows UIA, and bridging to [AT-SPI2](https://gnome.pages.gitlab.gnome.org/at-spi2-core/) on Linux).

* **Technical Rationale:** Accessibility tools receive pixel-perfect representations of the visual output without forcing the layout engine to run a separate, redundant accessibility tree calculation. Split into three sub-steps the same way Steps 3.3/4.2/4.3/5.1/5.2 were, mapping 1:1 onto the three tasks above: 5.3.1 (`Canvas::tag_accessibility_node` and its IR-adjacent data, correctly threaded through Step 5.2's `SubCanvas`/`FrameArena` machinery -- nothing about a parallel extractor or an OS bridge is meaningfully buildable without real tagged spatial data existing first), 5.3.2 (the real Linux AT-SPI2 bridge and the concurrent metadata extractor, matching every prior OS-integration step's own "Linux complete; Windows/macOS deferred" precedent), 5.3.3 (the capstone: a real demo tagging a scene, publishing it, and verifying via a real AT-SPI2/D-Bus query that reported bounds match what was actually rendered).

### Step 5.3.1: `Canvas::tag_accessibility_node` -- Status: Complete (2026-09-07)

First of three sub-steps closing Step 5.3. `RenderingCanvas::tag_accessibility_node(node_id, x, y, width, height, role)` reuses `draw_rounded_rect`'s own established technique for handling an active rotation -- transforming all four local corners individually via `state.transform.transform_point`, not just an offset top-left -- then adds one step specific to accessibility: reducing those four transformed corners to a real axis-aligned min/max bounding box, since (unlike vertex geometry, which can stay rotated) an OS accessibility API needs a genuine axis-aligned rect regardless of how the local rect itself was rotated. `AccessibilityNodeId(u64)` and a small starter `AccessibilityRole` enum (`Generic`, `Button`, `TextLabel`, `Image`) are new, minimal public types -- not an attempt at AT-SPI2's ~130-role taxonomy, since no consumer exists yet (5.3.2) to demand richness beyond "what kind of element is this," the same "adapt the doc sketch to what's concretely buildable" precedent Step 5.1.2 already set for `DynamicTextLayout`/`Paint`. Tagged nodes stay a flat per-frame `Vec<AccessibilityNode>`, not a tree the engine builds -- DESIGN.md's own signature carries no parent-node parameter, so the UI framework already owns the real widget hierarchy.

`FlattenedFrame` gained a fourth field, `accessibility_nodes: Vec<AccessibilityNode>`, threaded through unchanged by the shared `segment_and_flatten` free function -- a flat move with no sort/merge logic, since accessibility nodes never interact with `flatten_run`'s marker-segmentation or batch-merging at all. Correctly threading tagged nodes through `SubCanvas`/`FrameArena`/`stitch_into` was required this sub-step, not deferred: `SubCanvas` already `Deref`/`DerefMut`s to every `RenderingCanvas` method for free (Step 5.2.1), including automatically whatever this sub-step added, so omitting `stitch_into` support would have silently dropped a real caller's worker-thread-tagged data. `FrameArena` gained a fourth `tre_memory::ScatterArena<AccessibilityNode>` (`AccessibilityNode` is `Copy`, fitting the existing primitive with zero changes to `tre-memory` itself); unlike vertices/indices/commands, accessibility nodes need no rebasing at all when merged -- nothing about one references a position in another array -- making this the simplest of the four arenas to stitch.

Two disclosed, deliberate simplifications: bounds are stored as `f32`, not yet rounded to whatever integer convention a real OS bridge needs (that bridge's own concern, 5.3.2); and reported bounds are transform-only, with no clip-stack intersection (matching `draw_rounded_rect`'s own established scope) -- a node scrolled partially out of view still reports its full transformed bounds, a real gap against DESIGN.md Section 5.2's "100% alignment" claim, disclosed here rather than silently assumed away.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: `tre-engine` gained 4 new tests (up to 48 total from 44) -- a pure-translation sanity check; a rotation-correctness test (a 90-degree rotation on a 10x4 rect produces the real bounding box `(-4, 0, 4, 10)`, the opposite aspect ratio from the naive top-left-only transform's wrong `(0, 0, 10, 4)`, compared within a small epsilon since `f32::sin_cos` on `FRAC_PI_2` doesn't land on exactly 0.0/1.0); a `SubCanvas` tagging a node and calling `stitch_into` carries it into a `FrameArena` correctly; and a real 4-worker-thread test extending Step 5.2.3's own capstone pattern, confirming every thread's tagged node survives concurrent stitching with its correct bounds regardless of scheduling. No new demo this sub-step (matching 5.2.1/5.3's own precedent) -- tagged data has nowhere real to go until 5.3.2's OS bridge exists; the real end-to-end proof is deferred to the 5.3.3 capstone. All 4 examples touching the shared `flatten`/`stitch_into`/`FrameArena` code path (`canvas_state_stack_demo`, `canvas_draw_text_demo`, `canvas_batch_flattening_demo`, `canvas_sub_canvas_demo`) re-run manually, zero regressions.

### Step 5.3.2: The Real Linux AT-SPI2 Bridge -- Status: Complete (2026-09-08)

Second of Step 5.3's three sub-steps. A new `tre-a11y` crate publishes Step 5.3.1's tagged accessibility nodes onto the real Linux AT-SPI2 accessibility bus via `accesskit`/`accesskit_unix` rather than a hand-rolled D-Bus implementation -- the same "depend on a vetted crate for a complex wire format" reasoning already applied to `fdsm` (MSDF) and `wide` (SIMD). Pinned to `accesskit = "=0.17.1"`/`accesskit_unix = "=0.13.1"`, the newest versions this workspace's declared `rust-version = "1.75"` can resolve (confirmed via `cargo add --dry-run`, the same technique `tre-svg` already used to pin `usvg`).

`A11yBridge::connect(app_name, toolkit_name, toolkit_version)` is infallible -- reading `accesskit_unix::Adapter::new`'s real source (not assuming) confirmed it never returns a `Result`, gracefully degrading to a permanently-inactive adapter when no AT-SPI2 registry is reachable. That same reading revealed `accesskit_unix` already owns a real, dedicated background thread (a process-wide `OnceLock`, spawned lazily on first use) running its own async executor and D-Bus connection, fed by an unbounded internal channel -- exactly the "runs in parallel with RHI submission... without blocking GPU submission" property this step's own task list asked for, already built into the library. This is a real, deliberate simplification from the original plan (which assumed `tre-a11y` would need to hand-roll its own background thread and bounded channel on top): `A11yBridge::publish` just mutates in-process state and calls `Adapter::update_if_active` directly on the caller's thread, verified never to block via a real 1000-call timing-bounded test.

`accesskit::TreeUpdate` requires exactly one root, but Step 5.3.1's tagged nodes are a flat list with none -- `tre-a11y` synthesizes one reserved root `NodeId(u64::MAX)` (a `Role::Window`) whose children are every published frame's tagged nodes, with `AccessibilityRole` mapped onto a small subset of `accesskit::Role` (`Generic` -> `GenericContainer`, `Button` -> `Button`, `TextLabel` -> `Label`, `Image` -> `Image`). The synthesized root's own bounds are the real axis-aligned union of every published node's bounds (a small, real addition beyond the original plan's own letter, made because it was cheap, correct, and directly useful to any AT client querying the root itself) -- reading `accesskit`'s own source confirmed its bounds convention (physical pixels, y-down, relative to the tree's container) already matches Step 5.3.1's own world-space contract exactly, so no coordinate conversion was needed.

Verified against a **real, live AT-SPI2 stack**, not a mock: this development machine already runs `at-spi-bus-launcher`/`at-spi2-registryd` with `org.a11y.Status.IsEnabled` true, so `tre-a11y`'s round-trip test genuinely discovers its own app in the real registry (`Registry.GetChildren`, filtered by a per-run-unique `ToolkitName`), descends through the real synthesized-root/tagged-node hierarchy, and confirms `Component.GetExtents`/the object path's own `NodeId` suffix match exactly what was published -- proven manually first via a throwaway probe-and-discover pair of example binaries before being written up as the crate's permanent test. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace: `tre-a11y` ships 6 tests (4 unit -- role-mapping, union-bounds-of-none, union-bounds-of-two-real-nodes, construct-then-drop; 1 unit timing test for the non-blocking guarantee; 1 real D-Bus integration test). CI's `test` job now installs `dbus-user-session`/`at-spi2-core` and wraps `cargo test --workspace` in `dbus-run-session`, relying on `org.a11y.Bus`'s own standard D-Bus service-activation file (the same mechanism a real desktop session or a Flatpak sandbox already relies on) rather than manually orchestrating daemons. **Checked against the actual GitHub-hosted runner (Step 5.3.3's own CI verification, REVIEW.md #126):** activation itself works, but this job's own round-trip test only passed by a hair (10.06s against its own 10s budget) on a fresh runner. Two real CI runs (one setting `org.a11y.Status.IsEnabled` explicitly, one not) measured statistically identical ~10.05-10.06s timings, directly disproving `IsEnabled` as the cause -- the real fix widens the discovery timeout to 30s instead, real margin over both observed worst cases.

Explicitly deferred, matching this step's own locked scope: Windows UIA/macOS NSAccessibility (Steps 1.1/1.2/4.1's own "Linux complete; Windows/macOS deferred" precedent, even though `accesskit` itself supports both); real UI action dispatch (`ActionHandler` accepts AT-SPI2 `Action` requests but performs none); human-readable accessible names/labels (Step 5.3.1's schema carries none yet); any tree hierarchy beyond the one synthesized root; wiring this bridge into any GPU-rendered example (Step 5.3.3's own capstone job); incremental/diffed `TreeUpdate`s (every `publish()` rebuilds the full node list).

### Step 5.3.3: The Capstone -- A Real Rendered Scene, Verified Live -- Status: Code complete, verified live on a real desktop (2026-09-08); CI's own `accessibility-validation` gate not yet passing -- Step 5.3 NOT yet closed

Third and closing sub-step of Step 5.3. A new demo, `canvas_accessibility_demo`, draws and tags 3 rects from the exact same local coordinates -- a plain `Generic` rect, a plain `Button` rect, and a **rotated** `Image` rect via `Affine2::from_translation_rotation_scale` -- then hands its tagged nodes to a second binary, `canvas_accessibility_verify`, which publishes and verifies them against a real, live AT-SPI2 registry (see below for why this is two real processes, not one). Together they check three independent systems against one shared source of truth, derived from a single real render: the real GPU framebuffer (read back and confirmed non-background at each rect's own real transformed center, including the rotated one), the tagged `AccessibilityNode` IR, and the real AT-SPI2 registry's own response. Neither prior sub-step could prove this alone: 5.3.1 never touched a real bus, and 5.3.2's own round-trip test used a synthetic, never-rendered node.

**A real, previously-undiscovered regression was found and fixed while building this demo.** `tre-a11y`'s `map_role` (Step 5.3.2) mapped `AccessibilityRole::Generic` onto `accesskit::Role::GenericContainer` -- but reading `accesskit_consumer::common_filter`'s own real source (the filter `accesskit_atspi_common` actually calls) shows it hard-codes `GenericContainer`/`TextRun` as always excluded from the platform tree entirely, the real accesskit equivalent of ARIA's `role="none"`/`"presentation"`. Every `Generic`-tagged node had been silently invisible to any real assistive technology since Step 5.3.2 shipped -- undetected because that step's own tests only ever published a `Button`-tagged node against a live bus, never a `Generic` one. This demo's own first draft caught it immediately: `Registry`-side `GetChildren` on the synthesized root returned 2 children instead of 3. Fixed by remapping `Generic -> Role::Unknown` (confirmed via the same filter source to be the correct, unfiltered choice) -- exactly the class of gap a real, combined, end-to-end capstone exists to catch that isolated per-layer tests did not.

**A second, real bug surfaced in the demo's own first-draft verification code, not the engine.** A `thread::scope` whose main closure asserts against the (pre-fix) regression above panicked -- but the spawned "keep publishing" thread's stop flag was a plain `AtomicBool` only ever cleared on the success path, so `thread::scope`'s own contract (join every spawned thread before returning, even while unwinding) hung the whole process forever instead of reporting the failure. Fixed with an RAII guard clearing the flag on any exit from the closure, panic included -- the same "prove it, don't assume it" pressure this project's own capstones repeatedly apply to demo code, not just engine code (Step 5.1.3's demo assertion fixes, Step 5.2.3's own two real bugs).

**A real, two-process architecture** (see REVIEW.md finding #126 for the full nine-push investigation). `canvas_accessibility_demo` renders and tags only, then hands its tagged `AccessibilityNode`s to a separate `canvas_accessibility_verify` binary -- confirmed via `ldd` to link zero Vulkan/X11/Wayland libraries -- via a small plain-text handoff file. `canvas_accessibility_verify` performs the real `A11yBridge::connect`/`publish` and the real AT-SPI2 verification, mirroring `tre-a11y`'s own already-reliable round-trip test exactly, just with externally-supplied, real-render-derived data instead of one synthetic literal. This also matches real AT-SPI2 practice more closely than a self-verifying process: a real screen reader is always separate from the application it inspects.

**CI's `accessibility-validation` job does not yet pass, and Step 5.3 is not yet closed.** An earlier version of this section claimed a "real root cause" (`IsEnabled` flipping only via a client-called `RegisterEvent`/`EventListenerRegistered` signal) and declared Step 5.3 closed on that basis -- both wrong, corrected here rather than left stale. That mechanism does exist in `at-spi2-core`'s upstream `main` branch, but not in the version actually installed by `apt-get install at-spi2-core` on the `ubuntu-latest` runner (2.52.0, confirmed via packages.ubuntu.com and a direct source diff against that exact tag) -- nineteen real CI pushes are recorded in REVIEW.md finding #126, most of them chasing that non-existent mechanism before this was caught.

The actual, precisely-located bug, confirmed via a plain `dbus-send` query independent of our own code: `ensure_accessibility_enabled` (in both `canvas_accessibility_verify.rs` and `tre-a11y/tests/round_trip.rs`) queries `org.a11y.Status`'s `IsEnabled` property on the **a11y bus** connection, but `at-spi-bus-launcher` owns `org.a11y.Bus`/`IsEnabled` on the **session bus** (`g_bus_own_name(G_BUS_TYPE_SESSION, ...)`, confirmed directly in its real source). Every `get_property` call therefore fails outright against a destination that doesn't exist on that bus, and `.unwrap_or(false)` silently turns that failure into the same `false` a real "not enabled yet" reading would produce -- fully explaining why nothing done to the actual `IsEnabled` mechanism itself ever had any visible effect. The fix (build the `status` proxy on the session bus, not the a11y bus) is identified but was not implemented or pushed -- REVIEW.md's stopping-point note explains why, and that the `test` job's own round-trip test may have been silently skipping via this same bug rather than genuinely passing, not yet checked.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, and directly on a real desktop session (not CI): `canvas_accessibility_demo` renders, tags, verifies pixels (every rect's real transformed center, including the rotated one, computed via the same `Affine2` used to draw and tag it), writes the output PNG, and writes the handoff file; `canvas_accessibility_verify` reads it, publishes via a real `A11yBridge`, and confirms every node's real AT-SPI2 `Component.GetExtents` matches its own real IR bounds exactly, with all 3 roles surviving as pairwise-distinct real AT-SPI2 roles. `demo/phase5_step5_3_3/` (README, run script running both binaries in sequence, output PNG) added, matching every prior demo-bearing step. All pre-existing examples re-run manually as the lighter "did adding a new dev-dependency break an unrelated build" check (this is the first time `tre-rhi-vulkan` depends on `tre-a11y`), zero regressions. **The code and its logic are real and verified; the CI gate proving it in a clean container is not yet green. Step 5.3 (5.3.1-5.3.3) stays open until `accessibility-validation` passes for real.**

## Phase 6: Sorting, Batching, & RHI Execution

**Correction (2026-09-08, found while starting real execution of this
phase, not during the September 2026 documentation review that touched
everything else on this page):** the two sub-steps immediately below are
this phase's original pre-execution outline, written before any of
Phase 5's real work existed -- they do not reflect what actually
happened. Real execution of this phase starts below them, under its own
real sub-step numbering (colliding on "Step 6.1" with the outline's own
different Step 6.1 -- disclosed here rather than silently renumbered
away). Also corrected: the color/HDR work this phase's own real planning
briefly proposed folding in was moved back to match this outline's own
pre-existing **Phase 7: Color Management & Compositing** below, once
this outline was actually found -- see `planning/archive/PLAN_PHASE6_
STEP6_1.md`'s "Scope decisions" for the full account.

### Step 6.1 (original outline): The 64-Bit Radix Batching Engine -- Status: Superseded, completed early via Step 5.1.3

* **Implementation Tasks:**

  1. Generate the 64-bit key for every IR command per the canonical bit layout in ARCHITECTURE.md Section 4.1 (Layer 16 / Pipeline 16 / Texture 12 / Depth 20 bits).

  2. Implement a 4-pass Radix Sort ($\mathcal{O}(N)$) utilizing local thread histograms, with pass widths matching the field boundaries above (not a fixed 16-bit digit per pass, since Texture and Depth are no longer 16 bits each).

  3. Calculate prefix sums over the histograms and scatter the `UiDrawCommand` array into a double-buffered secondary array.

  4. **Added in the September 2026 documentation review:** add a debug-build assert on Depth ID assignment that fires before the 20-bit field would overflow (ARCHITECTURE.md Section 4.1).

* **Technical Rationale:** Radix sort guarantees deterministic sub-millisecond sorting times even for extreme outliers (e.g., sorting $50,000$ draw commands in under $0.2\text{ ms}$).

* **Implementation status:** real, but not built under this heading -- `RenderingCanvas::next_sort_key` (the real 64-bit key, exactly this bit layout) and `flatten_run`'s real sort/merge pass were built as part of Step 5.1.3 ("Real Sort Key, Overlay Routing, Real Batch Flattening"), needed early to prove that step's own capstone. A linear `sort_unstable_by_key` is used rather than a hand-rolled 4-pass radix sort (task 2) -- real frame command counts are far below where radix's $\mathcal{O}(N)$ advantage over `sort_unstable_by_key`'s $\mathcal{O}(N \log N)$ would matter against the $\le 0.50\text{ ms}$ CPU budget, and no profiling data has ever shown otherwise; revisiting this is real, deferred future work if measurement ever justifies it, not a gap in what Step 5.1.3 proved. Task 4's overflow assert is real (`tre-engine`'s own Depth ID assignment).

### Step 6.2 (original outline): Dynamic Index Stitching -- Status: Tasks 1-2 complete via Step 5.1.3; task 3 is real Step 6.x execution, starting below

* **Implementation Tasks:**

  1. Implement a linear sweep pass over the sorted IR array. Identify contiguous blocks of commands where Layer, Pipeline, and Texture bits are identical.

  2. Consolidate these commands by applying relative offsets to the index buffer: $idx_{\text{global}} = idx_{\text{local}} + \text{vertex}_{\text{offset}}$.

  3. Emit a single `RhiCommandBuffer::draw_indexed` call for the entire aggregated batch, drastically lowering driver submission overhead.

* **Implementation status:** tasks 1-2 are real, also via Step 5.1.3 -- `flatten_run`'s batch-merge pass, `FlattenedFrame::commands` is real, sorted, merged `UiDrawCommand` output today. Task 3 -- actually calling `RhiCommandBuffer::draw_indexed` (and everything a real call needs: resolving which pipeline to bind, which texture, handling `PushScissor`/`PushLayer` markers along the way) -- was never built under this heading either; a pre-planning investigation for real execution (2026-09-08) found no generic consumer of `FlattenedFrame` exists at all, only two demos' own duplicated, hardcoded per-pipeline dispatch loops. That gap, broken down into real, buildable sub-steps, is what Step 6.1 below (and its own successors) actually builds.

### Step 6.1: Real Pipeline State Registry -- Status: Complete (2026-09-08)

Real execution of this phase's remaining work (see the correction note
above) begins here. `Canvas` has exactly two real drawing methods today
-- `draw_rounded_rect` (Phase 3) and `draw_text` (Phase 5.1.2) -- backed
by exactly two pipeline kinds: an implicit, undocumented id `0` (the
SDF-rect pipeline) and `PIPELINE_MSDF_TEXT = 1`, the only named
constant. Three other real pipeline kinds exist in `tre-rhi-vulkan`
(plain bindless-textured quad, a walking-skeleton-style flat-vertex-
color quad, and a stencil/cover pair for self-intersecting-path fills),
but every demo using them bypasses `Canvas`/`flatten()` entirely --
reaching them through `Canvas` needs new drawing methods (`draw_image`/
`draw_path`/`draw_svg`) this step does not build, so this step registers
only the two kinds `Canvas` can actually reach today, not all five.

A real `PipelineRegistry` (`tre-engine`, generic over the existing
`RhiPipelineState` trait -- no new RHI trait surface) maps a pipeline id
to its real pipeline object: `register(id, Box<dyn RhiPipelineState>)`
(panics on a duplicate id, matching this crate's established `pop_layer`/
`restore`-style precedent for invalid caller state) and `get(id) ->
Option<&dyn RhiPipelineState>` (returns `None`, not a panic, for an
unregistered id -- a real runtime condition a future executor should be
able to detect, distinct from a programmer error at registration time).
A new `PipelineKind` enum (`SdfRoundedRect = 0`, `MsdfText = 1`) gives
both currently-reachable kinds a real, type-safe name; `PIPELINE_MSDF_
TEXT` stays defined as a plain `u16` for existing call sites and the
sort-key-packing code, which only ever wants a bare 16-bit field.

**A real, previously-undiscovered `NO_TEXTURE` sentinel mismatch was
found and fixed while investigating the two demos' own duplicated
dispatch loops this registry exists to eventually replace (REVIEW.md
finding #127).** `draw_rounded_rect` emitted `UiDrawCommand::texture_
handle: 0` for "no texture bound"; `canvas_batch_flattening_demo.rs`/
`canvas_sub_canvas_demo.rs` each independently defined their own `const
NO_TEXTURE: u32 = u32::MAX` for RHI-side binding, reconciled only by
each demo's own hardcoded `if pipeline_state_id == PIPELINE_MSDF_TEXT`
branch. `0` is not safe to treat as "no texture" going forward -- it
collides with a real bindless index `0` a future textured pipeline could
validly use. Fixed by promoting `NO_TEXTURE: u32 = u32::MAX` into a real,
shared `tre-engine` constant and changing `draw_rounded_rect`'s own
emitted command to use it -- the one real `DrawGeometry`-emitting call
site that needed it; the `PushScissor`/`PopScissor`/`PushLayer`/
`PopLayer` marker commands (`element_count: 0`, never consumed as real
draw calls) were left at their existing `texture_handle: 0`, since that
field carries no meaning for a command a future executor will only ever
`continue` past.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace: `tre-engine` gained 4 new tests (up to 51 total from 48)
-- `PipelineRegistry` resolves registered ids back to the exact object,
`get()` on an unregistered id returns `None` not a panic, `register`
panics on a duplicate id, and `draw_rounded_rect`'s own existing command-
field test extended to assert `NO_TEXTURE` rather than `0`. No new demo
this sub-step -- nothing yet queries the registry at render time (Step
6.2 is what would), matching this project's own established precedent
(Steps 5.2.1/5.3.1/5.3.2) of deferring visual proof to the step that
gives foundational work a real consumer. `canvas_batch_flattening_demo`,
`canvas_sub_canvas_demo`, `sdf_rounded_rect_demo`, `canvas_state_stack_
demo`, and `canvas_draw_text_demo` (every demo touching `draw_rounded_
rect`/`flatten()`) re-run manually end to end, zero regressions --
confirming the `NO_TEXTURE` change is a genuine no-op for existing
render output, since none of them read `command.texture_handle` on the
SDF-rect branch today.

### Step 6.2: The Generic Frame Executor -- Status: Complete (2026-09-08)

The real work the pre-existing outline's own "Step 6.2: Dynamic Index
Stitching" task 3 named but never detailed ("Emit a single
`RhiCommandBuffer::draw_indexed` call for the entire aggregated batch")
-- tasks 1-2 of that original step (the sweep and index-offset
consolidation) are already real via Step 5.1.3, per the correction note
above. `canvas_batch_flattening_demo.rs:242-256` and
`canvas_sub_canvas_demo.rs:359-373` were confirmed, before this step,
to be byte-for-byte identical: both loop over `frame.commands`, skip
non-`DrawGeometry` markers, branch on `pipeline_state_id ==
PIPELINE_MSDF_TEXT` to choose a pipeline object and texture-bind policy,
then bind vertex/index buffers and call `draw_indexed`. With Step 6.1's
registry and unified `NO_TEXTURE` sentinel real, that branch collapses
to one generic `registry.get(command.pipeline_state_id)` lookup plus an
unconditional `bind_texture(0, command.texture_handle)` -- no
per-pipeline-kind special case at all.

A real `execute_draw_geometry_batches(frame, registry, vertex_buffer,
index_buffer, cmd_buffer)` (`tre-engine`, alongside `PipelineRegistry`)
does exactly this: skips `PushScissor`/`PopScissor`/`PushLayer`/
`PopLayer` markers (real handling is Steps 6.3/6.4), resolves each
`DrawGeometry` command's pipeline via the registry (panicking on an
unresolved id -- a static setup bug for this function's real callers,
not a transient condition, matching this crate's established
`pop_layer`/`restore`-style precedent), and issues the same
`set_pipeline`/`bind_texture`/`bind_vertex_buffer`/`bind_index_buffer`/
`draw_indexed` sequence both demos already had, just once, shared,
instead of duplicated. Vertex/index buffer upload stays the caller's
job -- both demos build theirs via `VulkanDevice::upload_buffer`, a
concrete, Vulkan-specific method with no place in a generic,
backend-agnostic `tre-engine` function; "Buffer Packing" is its own
distinct frame-lifecycle stage (DESIGN.md item 7) this function
deliberately doesn't perform.

Both demos were rewired to build a `PipelineRegistry` (registering
`PipelineKind::SdfRoundedRect`/`MsdfText` in place of their own bare
`rect_pipeline`/`msdf_pipeline` locals) and call the new shared
function, replacing their own duplicated loops -- with zero change to
either demo's own scene setup, buffer upload, or existing assertions.
Each demo's own local `NO_TEXTURE` constant (now redundant with Step
6.1's shared `tre_engine::NO_TEXTURE`) was removed.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace: `tre-engine` gained 3 new tests (up to 54 total from 51)
-- a `FakeCommandBuffer`/`FakeBuffer` test-double pair records the exact
call sequence (not just "some calls happened") for a hand-built frame
mixing marker commands with two `DrawGeometry` commands using two
different registered pipelines; an unregistered pipeline id panics with
a clear message; an empty command list makes zero calls. Both rewired
demos re-run against real Vulkan hardware: every existing assertion
(exact batch count/content at the IR level, real non-background pixels
at every rect's/glyph's own position) still passes unchanged -- the real
proof this step is a genuine behavioral no-op, now produced via shared
code. No new demo this sub-step (see IMPLEMENTATION.md's own plan for
why -- the two existing demos already exercise the only reachable
pipeline kinds, so a new demo would add process, not coverage).

* **Follow-up (2026-09-10):** `execute_frame` (the free function this step's own `PipelineRegistry`/`execute_draw_geometry_batches` work led to) still left every real caller hand-writing the identical `begin_frame`/(record)/`submit_and_present` sandwich around it -- found via this project's own `/review-project` process (REVIEW.md #200-201) to be duplicated across all 41 `tre-rhi-vulkan` examples plus `tre-python`'s `HeadlessRenderer`, roughly half of which don't even call `execute_frame` (hand-written draw calls instead). Added `tre_engine::submit_frame(device, swapchain, record: impl FnOnce(&mut dyn RhiCommandBuffer))`, generic over both, wrapping only `begin_frame`/`submit_and_present` and leaving the recording itself to the caller's closure -- migrated every one of those 41+1 real call sites to it, full real-GPU regression sweep clean.

* **Follow-up (2026-09-10):** a full-project review found `tre-engine/src/lib.rs` had grown to 6,862 lines holding four separable concerns inline (REVIEW.md #205). At the user's own explicit instruction ("Split lib.rs into modules"), split it into `input.rs` (platform input events/`FrameClock`), `canvas.rs` (the `RenderingCanvas`/`SubCanvas` IR-recording engine *and* the `FrameArena`/sort-batch pipeline that backs its own `flatten()` -- merged into one module rather than two, since the real coupling between them didn't match a clean two-way split), and `rhi.rs` (the entire RHI trait surface, `PipelineRegistry`, `execute_frame`/`submit_frame`). Every `tre_engine::X` public path preserved exactly via crate-root `pub use` re-exports -- the whole workspace rebuilt with zero changes needed outside `tre-engine` itself. `lib.rs` itself is now 3,490 lines, ~3,074 of which are its own pre-existing test module, deliberately left in place rather than split to match (a real, disclosed scope boundary -- see REVIEW.md's own account of why, and the real `pub(crate)`-visibility fallout the split required for several items that surviving test module still references directly).

### Step 6.3: Real Scissor Execution -- Status: Complete (2026-09-08)

`RhiCommandBuffer::set_scissor` was a real, working Vulkan method with
zero real callers before this step -- both demos' render loops, even
after Step 6.2's rewiring, explicitly skipped every non-`DrawGeometry`
command. `canvas_state_stack_demo.rs`'s own source said outright that
this wiring was deferred to exactly this step.

Two real findings shaped the design, both from reading real source, not
assumed. First: `push_clip`/`begin_overlay` (`tre-engine/src/lib.rs:789-
860`) both resolve the full clip rect directly into their own
`PushScissor` command's `clip_bounds` -- `push_clip` the real
intersected rect, `begin_overlay` the `FULL_WINDOW_CLIP` sentinel
(entering the overlay plane resets to unclipped); `pop_clip`/
`end_overlay`'s own `PopScissor` commands always carry a zeroed
`clip_bounds`, so a real executor needs its own runtime stack to know
what to restore, not the `Pop` command's own fields. Second: reading
`VulkanDevice::begin_frame`'s real source
(`tre-rhi-vulkan/src/lib.rs:1849-1856`) found it already calls a real
`cmd_set_scissor` covering the true framebuffer extent before returning
the command buffer -- so a scene that never calls `push_clip` at all
stays correctly, safely scissored with no extra work from this step.
But `FULL_WINDOW_CLIP` (`tre-engine`'s own IR-level sentinel, `{x:0,
y:0, width: u32::MAX, height: u32::MAX}`) is CPU-side-only -- passing it
literally to a real `set_scissor` call would be an invalid, out-of-
bounds scissor rect on real hardware, and `begin_overlay`'s own
`PushScissor` command carries this exact sentinel directly. A new
`full_window: &ScissorRect` parameter (the real framebuffer extent, the
only thing that genuinely knows it) is substituted for the sentinel
wherever it would otherwise reach the GPU.

`execute_draw_geometry_batches` (Step 6.2) was renamed to
`execute_frame`, since its scope is no longer just draw batches -- a
real `match` on `command.kind` now also processes `PushScissor`/
`PopScissor` with a runtime `Vec<ScissorRect>` clip stack, calling
`set_scissor` on every push/pop (no redundant-call elision, matching
Step 6.2's own "correctness before profiling-driven optimization"
precedent). `PushLayer`/`PopLayer` gained an empty match arm so the
function stays exhaustive -- real handling is Step 6.4.

Both existing callers (`canvas_batch_flattening_demo`, `canvas_sub_
canvas_demo`) were updated to the renamed function with their own real
`CANVAS_WIDTH`/`CANVAS_HEIGHT` as `full_window` -- a genuine no-op for
their rendered pixels, since neither nests a narrower `push_clip` inside
its own `begin_overlay` bracket. `canvas_state_stack_demo` is the one
demo with a real, narrower clip -- but a real, previously-unnoticed gap
surfaced while extending it: Rect C's own drawn geometry (`150,10,
30,30`) exactly matched its own clip rect, meaning clipping was a no-op
for that scene's own rendered pixels regardless of whether scissor
execution was real. Widened Rect C's drawn rect to `140,0,60,50`
(deliberately larger than its own clip on every side) so a real crop has
something real to prove, and rewired the demo onto `execute_frame`/a
real `PipelineRegistry` in place of its own single, command-stream-
ignorant `draw_indexed` call.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace: `tre-engine` gained 1 new test (up to 55 total from 54)
-- a nested `PushScissor(A)`/`PushScissor(B)`/`PopScissor`/`PopScissor`
frame asserts the exact `set_scissor` sequence (`A`, `B`, `A` restored
from the stack -- not `full_window` -- then `full_window` once the
stack truly empties); the existing 3 `execute_draw_geometry_batches_*`
tests were renamed and updated for the new parameter, one of them
extended to also cover the `FULL_WINDOW_CLIP` sentinel-substitution path
via `begin_overlay`-shaped markers. `canvas_state_stack_demo` re-run
against real Vulkan hardware: a pixel inside both Rect C's own geometry
and its clip rect reads real foreground; the corresponding pixel inside
the geometry but outside the clip reads real background -- the actual,
new, GPU-level proof this step exists to deliver, not just an IR-level
`clip_bounds` field. `canvas_batch_flattening_demo`/`canvas_sub_canvas_
demo` re-run with every existing assertion unchanged, confirming the
real no-op. All other pre-existing examples re-run manually, zero
regressions.

### Step 6.4.1: Real RHI Render-to-Texture Capability -- Status: Complete (2026-09-08)

Split from "Step 6.4: Real PushLayer/PopLayer Execution" after a real-code
investigation found it needed genuinely new, cross-cutting RHI trait
surface -- confirmed, not assumed: nothing anywhere ever renders into an
acquired transient target (`gc_demo.rs`/`memory_pools_demo.rs`
immediately release what they acquire), the only real `cmd_begin_
rendering` construction in the whole codebase is inside `VulkanDevice::
begin_frame`, hard-wired to the swapchain's own color *and* stencil
views, and a transient target's `bindless_index()` is `None` by original
design ("written to, not sampled from"). This step builds the real
capability, driven entirely by hand-written RHI calls -- no `Canvas`/IR
involvement; Step 6.4.2 wires `push_layer`/`pop_layer` to it.

Three new `RhiCommandBuffer` methods: `begin_render_to_texture`
(ends whatever rendering is active, barriers the target `UNDEFINED ->
COLOR_ATTACHMENT_OPTIMAL`, begins a fresh, cleared-to-transparent
rendering scope with no stencil attachment -- transient targets don't
have one), `end_render_to_texture` (ends that scope, barriers
`COLOR_ATTACHMENT_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL` -- both barriers'
old/new layouts are statically known given this scope, no per-texture
layout-tracking field needed), and `resume_swapchain_rendering`
(re-begins rendering into the swapchain `begin_frame` originally set up,
with `LOAD_OP_LOAD` so whatever was already drawn is preserved, not
erased -- requires `VulkanCommandBuffer` to stash the swapchain's own
view/extent at `begin_frame` time, since nothing previously persisted it
past that function's own local scope). A new `RhiDevice::register_
bindless`/`deregister_bindless` pair lets an already-rendered-into
texture become sample-able -- factored out of `VulkanTexture::
from_pixels`'s own existing allocate-slot-and-write-descriptor logic,
returning the allocated index directly rather than mutating the
texture's own `bindless_index()` (which `RhiTexture` exposes no setter
for, deliberately -- every other method on it is a read of state fixed
at construction). Deregistering before `release_transient_target`, not
after, was a deliberate design choice: that function's own existing
safety guard (Phase 2 Code Review finding #70) rejects any texture whose
`bindless_index()` is `Some`, and keeping that guard's logic completely
untouched was preferred over teaching it to distinguish a legitimately
transient-and-bindless texture from a genuinely misused `create_texture`-
sourced one.

Scoped deliberately to single-level layer use, not nested layers --
nothing calls `push_layer`/`pop_layer` at all today, so building "resume
an *outer* layer's own rendering without re-clearing it" now would be
speculative; `resume_swapchain_rendering` only ever needs to resume the
swapchain, named as real, honest future work rather than silently
dropped, the same deferral discipline this project already applies to
Windows/macOS accessibility, blur filters, and HDR.

**A real bug found by this step's own first real run, not designed
around in the abstract (REVIEW.md finding #128).** Vulkan's dynamic
viewport/scissor state is a persistent property of the command buffer,
not scoped to one `cmd_begin_rendering` instance. `begin_render_to_
texture` correctly sets its own viewport for the layer's own smaller
size, but the first draft of `resume_swapchain_rendering` never restored
it -- the new demo's composite draw silently rendered through the
*layer's* stale, smaller viewport instead of the swapchain's own real
one, with zero validation-layer warning (a too-small viewport isn't
itself invalid). Only the real pixel assertion caught it. Fixed by
having `resume_swapchain_rendering` explicitly restore both viewport and
scissor to the real swapchain extent as part of its own work, rather
than leaving it to a caller to remember.

New demo (`render_to_texture_demo.rs`, `demo/phase6_step6_4_1/`):
acquires a `100x80` `Rgba16Float` transient target, renders a real
rounded rect into it via the existing, unmodified `sdf_rounded_rect`
pipeline (built against the layer's own `R16G16B16A16_SFLOAT` format,
not the swapchain's -- dynamic rendering requires a pipeline's declared
color format to match whatever it's actually bound against), registers
it bindless, resumes swapchain rendering, and composites it back as a
textured quad via the existing, unmodified bindless-textured pipeline
and the default premultiplied-alpha blend state every pipeline already
carries (confirmed real and unconditional, `VulkanDevice::create_
pipeline`'s own blend-state setup) -- no special-casing needed for a
correct "over" composite. Two real pixel checks: the composited rect's
own deep interior reads exactly opaque foreground (content genuinely
rendered offscreen and survived the round trip); a point inside the
composited region but outside the rect's own rounded footprint reads
exactly the real background (the layer was genuinely cleared to
transparent, and default blending shows the background correctly
through it).

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace, plus `tre-engine`'s own `FakeCommandBuffer` test double
extended with the three new methods (recorded, not yet exercised by any
test -- nothing calls them through `execute_frame` until Step 6.4.2).
All 21 pre-existing Vulkan demos re-run manually end to end after the
viewport fix, zero regressions -- this step touched shared
`VulkanCommandBuffer`/`begin_frame` code every one of them depends on.
CI's `vulkan-validation` job gained the new example.

### Step 6.4.2: Wiring `push_layer`/`pop_layer` to Real Render-to-Texture -- Status: Complete (2026-09-08)

Wires `Canvas::push_layer`/`pop_layer`'s IR commands to Step 6.4.1's
capability inside `execute_frame`, driven by a real recorded scene
instead of hand-written RHI calls. Real-code investigation before
writing anything found the scope was bigger than "wire two empty match
arms": `LayerDesc` had no compositing position at all (`push_layer` had
silently hardcoded `clip_bounds.x`/`y` to `0`, never exercised by any
real caller), `layer_depth` was a bare `u32` balance counter that
couldn't recover a popped layer's own width/height/format, and nothing
in `Canvas` could reach a plain textured-quad pipeline (`PipelineKind`
deliberately excluded one at Step 6.1, since nothing could emit it
then).

`LayerDesc` gained `x: i32, y: i32` (matching `ScissorRect`'s own
untransformed, screen-space coordinate type). `RenderingCanvas::
layer_depth: u32` became `layer_stack: Vec<LayerDesc>` so `pop_layer`
has the original `LayerDesc` back. `PipelineKind` gained `TexturedQuad
= 2`. `pop_layer` now bakes real composite-quad geometry (4 vertices at
the popped `LayerDesc`'s own `x`/`y`/`width`/`height`, UV `(0,0)`-`(1,1)`,
fixed opaque white -- deliberately skipping both `state.transform` and
`premultiply_alpha`, unlike `draw_rounded_rect`: `LayerDesc.x`/`y` are
screen-space, matching how this command's own `clip_bounds` already used
them raw before this step, and `LayerDesc` carries no opacity field yet,
by its own original doc comment) into `self.vertices`/`self.indices`,
and records a `PopLayer` command with `pipeline_state_id:
PipelineKind::TexturedQuad as u16`, real `element_count`/`vertex_offset`,
but `texture_handle: NO_TEXTURE` -- a placeholder, since the real
bindless index doesn't exist until `execute_frame` renders into the
layer and registers it. `push_layer` now writes `desc.format` into this
same command's own otherwise-unused `pipeline_state_id` field via two
new private `texture_format_to_u16`/`u16_to_texture_format` conversions
(`TextureFormat` itself stays repr-less -- it's also part of
`create_texture`/`acquire_transient_target`'s public signatures, so
giving the whole type a `#[repr(u16)]` would have been a wider, unrelated
change).

`execute_frame` gained a `device: &dyn RhiDevice` parameter -- its
previous parameter list (`registry`/`vertex_buffer`/`index_buffer`/
`full_window`/`cmd_buffer`) had no way to reach `acquire_transient_
target`/`release_transient_target`/`register_bindless`/
`deregister_bindless`, all `RhiDevice` methods. Real `PushLayer`
handling decodes the format, acquires a target sized to the command's
own `clip_bounds` width/height, and `begin_render_to_texture`s into it;
every `DrawGeometry` between a `PushLayer` and its matching `PopLayer`
needs no layer-awareness of its own at all -- it draws into whatever
target `cmd_buffer` currently has bound, which is the layer's target
purely because of the command stream's own ordering. `PopLayer` ends
that render, registers it bindless, `resume_swapchain_rendering`s, then
draws its own baked composite quad using the just-registered index in
place of the IR's `NO_TEXTURE` placeholder -- the same kind of
sentinel substitution `PushScissor` already does for `FULL_WINDOW_CLIP`.
Deliberately scoped to one level: a nested `PushLayer` panics, matching
Step 6.4.1's own single-level scope for `resume_swapchain_rendering`.

**A real bug found and fixed before any test ran, not after -- caught
by code inspection, not a failed run (REVIEW.md finding #129).**
`segment_and_flatten`'s existing doc comment stated "every
marker passes through unchanged" -- true before this step, since no
marker ever carried real geometry. `PopLayer`'s new baked quad broke
that assumption: `flatten_run` only rewrites `vertex_offset`/copies
indices for commands inside a `DrawGeometry` run, so a `PopLayer`
command pushed through unchanged would carry a `vertex_offset` pointing
into the canvas's own raw, pre-flatten `indices` -- not the freshly
built `out_indices` buffer the returned `FlattenedFrame` (and the RHI
index buffer uploaded from it) actually contains. Fixed by having
`segment_and_flatten` rebase any boundary command whose `element_count >
0` the same way `flatten_run` already rebases `DrawGeometry` commands,
before this was ever exercised by a test or a real GPU run.

New demo (`canvas_layer_composite_demo.rs`, `demo/phase6_step6_4_2/`):
reproduces `render_to_texture_demo.rs`'s exact scene and pixel
coordinates, but recorded entirely through `Canvas`/`execute_frame` --
`push_layer`, one `draw_rounded_rect` in the layer's own local space,
`pop_layer`, `flatten()`, one `execute_frame` call. Same two real pixel
checks pass: the composited rect's own interior reads exactly opaque
foreground, and a point inside the composited region but outside the
rect's own footprint reads exactly the real background.

**Regression found and fixed (2026-09-08, REVIEW.md finding #152), while
checking whether Step 7.2.2 would be building on solid ground.** This
step's own `PushLayer` handling has always silently dropped a layer's
own content whenever `RhiDevice::acquire_transient_target`'s documented
"oversized borrow" fallback hands back a texture larger than requested
-- the same underlying mechanism as REVIEW.md finding #130's real root
cause (a render target's real dimensions silently diverging from what a
caller's vertex data assumed), just reached through this step's own
production compositing path instead of a hand-rolled demo. Confirmed via
a real GPU repro before being treated as fact: push/pop a 200x150 layer
(fresh alloc, then released), then push/pop a never-before-requested
50x40 layer -- the pool hands back the freed 200x150 texture, and the
smaller layer's own content vanishes from the composite, reading back
exactly the background clear color at its own center.

**Fix:** `RhiCommandBuffer::begin_render_to_texture`/`begin_render_to_
texture_no_end` gained explicit `logical_width`/`logical_height`
parameters -- the caller's own intended size, always already known --
used only to set `self.width`/`self.height` (`draw_indexed`'s own
NDC-mapping push-constant source). Viewport/scissor/render area stay
driven by the texture's real size unchanged; the resulting "stretch" is
exactly undone later by `PopLayer`'s own normalized-`(0,0)`-`(1,1)`-UV
composite read, so no other change was needed anywhere. `execute_frame`'s
`PushLayer` handling now passes `command.clip_bounds.width`/`.height`
explicitly rather than letting the callee infer size from the acquired
texture. Verified by a new `tre-engine` unit test (an oversized
`FakeTexture` proving `execute_frame` still passes the *requested* size)
and a new permanent real-GPU demo (`layer_oversize_regression_demo.rs`,
added to `ci.yml`), plus a full zero-regression sweep across every
pre-existing Vulkan demo. REVIEW.md finding #152 has the complete
technical account.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace (`tre-engine` now at 59 tests, up from 55 -- 4 new: a full
`FakeDevice`-driven push/pop round trip asserting the exact call order
across both `RhiDevice` and `RhiCommandBuffer`, a nested-`PushLayer`
panic, a `PopLayer`-with-no-active-`PushLayer` panic, and the composite
quad's own vertex-baking at the `LayerDesc`'s real position). All 22
Vulkan demos (the 21 from Step 6.4.1 plus the new one) re-run manually
end to end, zero regressions. CI's `vulkan-validation` job gained the
new example.

**A real, undisclosed limit found later (REVIEW.md finding #138, Phase
1-8 Comprehensive Review, 2026-09-08): two or more sequential
`PushLayer`/`PopLayer` pairs in one frame can alias the same bindless
descriptor slot and physical transient texture before the GPU ever
executes the earlier draw, silently corrupting rendered output with no
validation-layer warning.** `active_layer.is_none()` only rejects
*nested* `PushLayer` (this step's own documented scope); sequential,
sibling layers are explicitly allowed and are DESIGN.md Section 6.2's
own named common case (multiple blurred/glassmorphism panels in one
scene). Because `execute_frame` records an entire frame into one
command buffer submitted once at the end, `register_bindless`/
`deregister_bindless`/`release_transient_target` all run immediately at
*recording* time, not GPU execution time -- a second layer's `PopLayer`
can free and immediately reuse the exact slot/texture a first layer's
already-recorded (not yet executed) composite draw still references,
so by submission time the first layer's draw samples the second layer's
content instead of its own. No test exercises two sibling layers in one
frame; `main_loop_demo.rs` (Step 8.1.2) never calls `push_layer` at all,
so this has not yet manifested in any real demo. Real fix: defer
`deregister_bindless`/`release_transient_target` until the GPU has
actually finished with the resource (fence-gated, mirroring this
project's own generational-GC deferred-release pattern, TECHNICAL.md
Section 3.3) rather than immediately at recording time -- genuine new
architecture, not attempted opportunistically inside that review.

### Step 6.5: The Combining Capstone -- Status: Complete (2026-09-08); Phase 6 closed

Phase 6's own closer, named explicitly in Step 6.1's own original plan
(`planning/archive/PLAN_PHASE6_STEP6_1.md`'s "Scope decisions": "6.5: a
combining capstone -- one real scene exercising multiple pipelines and
real clipping together in one submitted frame (layer compositing joins
once 6.4 makes it real)") -- ready once Step 6.4.2 closed. Adds no new
`tre-engine`/RHI surface at all; a pure integration proof that every
`execute_frame` command kind (`DrawGeometry`/`PushScissor`/`PopScissor`/
`PushLayer`/`PopLayer`) interoperates correctly in one real scene, not
just each in its own isolated demo.

New demo (`canvas_combined_scene_demo.rs`, `demo/phase6_step6_5/`): one
`Canvas` scene, one `execute_frame` call. `push_clip`/`draw_rounded_rect`
(deliberately larger than the clip on every side, `canvas_state_stack_
demo.rs`'s own Rect C precedent)/`pop_clip` draws a rect directly onto
the swapchain via `PipelineKind::SdfRoundedRect`. `push_layer`/
`draw_text`/`pop_layer` renders a real shaped word ("OK", via a real
cascade font and a real background `AtlasOwner` -- the same warm-up-
then-poll-to-resolution pattern `canvas_draw_text_demo.rs` already
established, reused unchanged) into an offscreen layer via
`PipelineKind::MsdfText`, then composites it back via `PipelineKind::
TexturedQuad`. Three real pipeline ids, three real declared formats, no
id used at two formats in the same frame -- each pipeline plays exactly
one role in the scene (`SdfRoundedRect` only ever draws directly onto
the swapchain here, `MsdfText` only ever draws inside the layer), the
same constraint `canvas_layer_composite_demo.rs`'s own header comment
already documented.

Three real, independent pixel checks, reusing two already-established
verification patterns rather than inventing new ones: clip cropping
(`canvas_state_stack_demo.rs`'s inside-clip-vs-outside-clip check);
composited text (`canvas_draw_text_demo.rs`'s own per-glyph quad scan,
recomputed independently and offset by the layer's own composite
origin, not a single fragile center-pixel assertion); and a composited-
but-empty layer area showing real background
(`render_to_texture_demo.rs`'s own transparency check).

**No bugs found -- passed on its first real run.** Every prior Phase 6
sub-step (6.1 through 6.4.2) already found and fixed a real bug or
design gap before or during its own first run; this step's own first
run passed every assertion immediately, which is itself worth recording
honestly rather than manufacturing a finding -- the previous six steps'
own rigor is what made this one boring.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace (`tre-engine` untouched, still 59 tests -- confirmed, not
assumed, since this step adds no engine code). All 23 Vulkan demos (the
22 from Step 6.4.2 plus the new one) re-run manually end to end, zero
regressions. CI's `vulkan-validation` job gained the new example.

**Phase 6 ("Sorting, Batching, & RHI Execution") is closed.** Every real
task the phase's own original outline described -- generating/sorting
IR command keys, merging adjacent batches, and actually driving
`RhiCommandBuffer` calls through a `PipelineRegistry`-resolved pipeline,
including `PushScissor`/`PushLayer` markers -- is real, tested, and
proven both in isolation (Steps 6.1-6.4.2) and combined (this step).
Deferred, disclosed future work stays open, not silently dropped: true
nested layers (`PushLayer` while another is already active,
`execute_frame`'s own documented panic), visual filters/blur on a
composited layer (Phase 7 Step 7.2's own job), and the pre-existing,
unrelated `accessibility-validation` CI gate (REVIEW.md finding #126).

## Phase 7: Color Management & Compositing

### Step 7.1: Linear sRGB Conversions & HDR -- Status: Tasks 2-3 complete (2026-09-08); task 1 honestly scoped down, see below

* **Implementation Tasks:**

  1. Configure RHI swapchains for HDR execution (`VK_FORMAT_R16G16B16A16_SFLOAT`).

  2. Implement hardware-accelerated sRGB to Linear conversion directly in vertex/fragment shaders for color correctness prior to alpha blending, using the canonical piecewise formula defined once in TECHNICAL.md Section 6.2 -- do not re-derive it here.

  3. Implement the canonical identity-below-white, soft-knee-above-white tone mapping curve (TECHNICAL.md Section 6.3) as a final post-process step when the display's reported HDR headroom is less than the content's authored range. Do not use a full-range filmic curve like [ACES](https://docs.unity3d.com/Packages/com.unity.render-pipelines.core@17.0/manual/tonemapping.html#aces) by default -- this is a desktop UI engine, not a photo/film/video-editing tool, and ACES's deliberate contrast and desaturation shaping would visibly shift exact UI/brand colors that must render unchanged. Expose ACES (or another filmic curve) only as an explicit, opt-in per-`Canvas` style choice for creative-workstation/DAW integrations (DESIGN.md Section 3) that specifically want it for embedded video/image preview content.

* **Technical Rationale:** Blending in sRGB space causes dark fringes around anti-aliased geometry. Doing this math on the GPU in linear space ensures pristine transparency intersections. The tone-mapping curve choice (task 3) is a separate concern from the blend-space conversion (task 2): getting linear blending right prevents dark fringes on every frame; getting the tone-mapping curve right prevents the engine's own HDR support from being the thing that makes a UI's colors inconsistent.

* **Implementation status:** this step also closes REVIEW.md finding #92
  (Phase 4 Step 4.2.1, deferred here by its own original text) -- every
  real fragment shader passed `UiVertex::color` straight through with no
  conversion, despite its own doc comment already promising one "in
  shader"; a mid-tone gray `150` round-tripped to `202` (the swapchain's
  `_SRGB` format auto-encodes shader output on store, so an sRGB-authored
  value treated as already-linear gets encoded a second time). Direct
  inspection found 4 real shaders affected, not the 2 finding #92's own
  title names: `walking_skeleton.frag`, `sdf_rounded_rect.frag`,
  `bindless_textured.frag`'s no-texture-bound fallback branch, and
  `msdf.frag`. **Task 2** is real: a canonical `srgb_to_linear(vec3)`
  GLSL helper (duplicated per shader -- this project's `build.rs` invokes
  `glslc` one file at a time, no shared `#include` mechanism exists) is
  applied to `frag_color.rgb` (never `.a`, which carries no gamma curve)
  before any coverage/blend math in all 4 shaders; texture-sampling paths
  need no change (the MSDF atlas is deliberately non-color
  `Rgba8Unorm` data, and the layer-composite's sampled `Rgba16Float`
  target is already linear by format). Verified by a new demo
  (`linear_color_demo.rs`, `demo/phase7_step7_1/`) that draws an opaque,
  genuinely non-fixed-point `rgb(150, 100, 200)` rect and reads back its
  own deep interior (coverage `1.0`, no AA-edge or premultiply-alpha
  interaction) -- the real GPU result round-trips **exactly**:
  `[150, 100, 200]` out, matching the authored color precisely (`srgb_to_
  linear` and the swapchain's own hardware encode-on-store are exact
  inverses). The demo also computes what the old, unfixed double-encoding
  would have produced via an independent Rust reference implementation of
  the canonical encode formula -- `[202, 168, 229]`, exactly matching
  finding #92's own `150`->`202` worked example -- confirming the fix has
  real, measurable effect. All 24 Vulkan demos (the 23 pre-existing plus
  this new one) re-run manually after the shader change, zero
  regressions -- expected, since every pre-existing demo deliberately uses
  only gamma-invariant (`0`/`255` per channel) colors specifically so this
  defect couldn't affect them either way, per `sdf_rounded_rect_demo.rs`'s
  own header comment; confirmed, not assumed, given this step's own global
  shader blast radius.

  **Task 1 was attempted, then honestly reverted after actually running
  it, not merely deferred on paper.** `VulkanSwapchain::new`'s real
  surface-format search was first upgraded to prefer `R16G16B16A16_
  SFLOAT` when the real physical device/surface reports it. Running the
  3 demos that construct a genuine windowed swapchain against this
  project's own real dev machine (`walking_skeleton`/`input_demo`/
  `multi_window`) revealed exactly why a bare format-only match is
  unsafe: the real surface reports `R16G16B16A16_SFLOAT` paired only
  with `colorspace SRGB_NONLINEAR`, not a genuine wide-gamut/extended-
  linear colorspace (this project's own `ash` dependency doesn't even
  expose `VK_EXT_swapchain_colorspace`'s extended constants, e.g.
  `EXTENDED_SRGB_LINEAR_EXT`, as named symbols, so the two can't be
  distinguished here anyway). A float format has no implicit hardware
  encode-on-store the way an `_SRGB` format does, so presenting this
  step's own now-genuinely-linear shader output through an
  `SRGB_NONLINEAR`-tagged float surface would very likely display too
  dark on real hardware -- a real, disclosed correctness risk, not a
  hypothetical one, caught only by actually running the change rather
  than reasoning about it in the abstract. Reverted: `VulkanSwapchain::
  new` still selects `B8G8R8A8_SRGB` exactly as before, and now logs
  (`eprintln!`, matching this crate's own existing diagnostic precedent)
  whether the real surface *also* reports an FP16 format, so real future
  HDR work starts from an observed fact instead of an assumption. **Task
  3** is real as a pure primitive only: `tre_math::tone_map(linear:
  f32, headroom: f32) -> f32`, TECHNICAL.md Section 6.3's exact formula,
  5 new unit tests (identity at/below white, compression above it,
  continuity at the boundary, monotonicity, asymptotic approach to `1.0
  + headroom`) -- not wired into any real render path, since task 1's own
  real, buildable logic never actually selects a genuine HDR format on
  any hardware available to this project today, so there is no real
  trigger to wire it to yet, matching this crate's own `compose_batch`/
  `lerp_points_batch` "build and prove the primitive before its exact
  consumer exists" precedent. The real OS-level brightness-metadata hook
  DESIGN.md Section 11.2 describes stays real, disclosed, deferred future
  work -- confirmed via a real repo-wide grep that no such hook exists
  anywhere in code today.

### Step 7.2: Visual Filters (PushLayer Blurs)

* **Implementation Tasks:**

  1. Implement the Dual-Kawase blur algorithm for fast backdrop filtering.

  2. Execute a chain of downsample passes (averaging 4 surrounding pixels per step) followed by upsample passes.

  3. Map `Canvas::pop_layer` logic to redirect rendering out of the transient target, inject the Kawase down/up passes, and blend the final result back into the parent swapchain.

* **Technical Rationale:** The Dual-Kawase approach slashes memory bandwidth by iteratively reducing texture sizes, massively outperforming large-radius, single-pass Gaussian blurs.

### Step 7.2.1: Real Dual-Kawase Blur RHI Capability -- Status: Fixed and closed (2026-09-08)

Split from Step 7.2 after real investigation found genuinely new
shader/algorithm surface was needed and a real semantic fork (own-
content vs. true backdrop blur) DESIGN.md doesn't resolve on its own --
confirmed with the project owner via AskUserQuestion: split into
7.2.1/7.2.2, own-content blur only, true backdrop sampling explicitly
deferred (`planning/archive/PLAN_PHASE7_STEP7_2_1.md`).

Real investigation before writing any code found the split's own
initial framing overstated how much new RHI surface was needed: `begin_
render_to_texture`'s own existing implementation already ends whatever
rendering scope is active before beginning the next, so a real, chained
downsample/upsample sequence needs no new trait methods at all in
principle -- just correct sequencing of Step 6.4.1's own existing calls,
plus two new shaders (`kawase_downsample.frag`/`kawase_upsample.frag`,
TECHNICAL.md Section 5.5's own new canonical formula, paired with the
existing `bindless_textured.vert` unchanged).

**A real bug was found and fixed while building the first real demo.**
Chaining `end_render_to_texture(previous)` directly into `begin_render_
to_texture(next)` calls `cmd_end_rendering` twice for one active
rendering scope -- the second call finds nothing active, a real Vulkan
validation error. Fixed by adding `RhiCommandBuffer::begin_render_to_
texture_no_end` (identical to `begin_render_to_texture` except it never
calls `cmd_end_rendering` first), paired with a plain `end_render_to_
texture` call immediately before it. A first, combined design
(`chain_render_to_texture`, doing both the end-and-barrier and the next
begin in one call) was tried and reverted: it left no point between "the
previous texture is sampling-ready" and "the next render pass is already
active" to call `RhiDevice::register_bindless`, and calling it while a
pass *was* active turned out to matter (see below) -- this method's own
design was corrected mid-implementation once that was discovered, not
silently forced through.

**A second, deeper real bug was found and, after extensive
investigation, could not be resolved -- REVIEW.md finding #130 has the
full account.** Sampling a bindless texture (via the exact same,
already-proven `bindless_textured.frag` sampling expression Step 2.1/
6.4.1 already established) while the *active render target* is an
offscreen texture (reached via `begin_render_to_texture`/`begin_render_
to_texture_no_end`, rather than `resume_swapchain_rendering`) reads back
all-zero data on real hardware, with no validation error of any kind.
Twelve independent hypotheses were each directly tested and ruled out --
the double-`cmd_end_rendering` bug above; `register_bindless` timing
relative to an active render pass; `nonuniformEXT` decoration loss
through a local variable or function parameter; the 5-tap/8-tap
averaging math itself; descriptor-index reuse within one frame;
render-target format; render-target size; the shader source file itself
(the real, working `bindless_textured.frag`, swapped in unmodified,
*also* fails under this exact condition); whether `frag_uv`/
`pc.texture_index` reach the shader correctly (confirmed via direct
color visualization); descriptor-indexing device features (confirmed
enabled); and the pipeline/render-pass/compositing machinery itself
(confirmed working via a hardcoded, texture-independent color output).
No prior code in this project has ever exercised "sample a bindless
texture while rendering into a *different*, non-swapchain target" --
every existing bindless-sampling call happens while rendering into the
swapchain (`render_to_texture_demo.rs`'s own composite step included).

**Not fixed. Explicitly stopped, not silently abandoned.**
`begin_render_to_texture_no_end` is real, independently correct, and
kept -- verified via all 24 pre-existing Vulkan demos re-run manually,
zero regressions, since nothing existing calls it yet. `dual_kawase_
blur_demo.rs` and the two new shaders are kept as a real, precisely-
documented reproduction case for future debugging with a real GPU
frame-capture tool (e.g. RenderDoc), beyond what this session's own
CLI-based investigation could resolve -- deliberately **not** added to
`ci.yml`, since it currently fails its own real pixel assertions.
TECHNICAL.md Section 5.5's canonical Dual-Kawase formula is unaffected
and remains correct regardless of this blocker. Step 7.2.1 is not
closed; Step 7.2.2 (wiring to `push_layer`/`pop_layer`) cannot proceed
until this is resolved.

**Second session (2026-09-08): an independent external research review checked, four more hypotheses ruled out, root cause still not found -- full account in REVIEW.md finding #130's own updated write-up.** The project owner commissioned an independent AI deep-research report on this exact bug (kept at `documentation/DUAL-KAWASE_BLUR_RESEARCH.md`/`.pdf`); its twelve tested hypotheses matched the twelve above exactly, but it proposed four additional theoretical mechanisms, each checked against the real code and ruled out: an incompatible pipeline layout leaving the bindless descriptor set unbound (ruled out -- `set_pipeline` unconditionally rebinds it against every pipeline's own compatible layout); missing `UPDATE_AFTER_BIND`/`PARTIALLY_BOUND` descriptor flags (already correctly present); a transient target missing `SAMPLED_BIT` or a zeroing component swizzle (already correct); and a missing/incorrect pipeline barrier -- checked with **Vulkan synchronization validation** (`VK_LAYER_VALIDATE_SYNC=1`), a real diagnostic never previously tried, proven genuinely active via a positive-control test (deliberately removing a real barrier immediately produced a specific, real hazard error), then run clean against the actual, unmodified code with zero warnings. This rules out barrier defects with real evidence, not just careful reading. Step 7.2.1 stays open; the synchronization-validation technique is now a proven, available tool for the next session, and the report's own recommended RenderDoc frame-capture inspection remains the most promising untried path.

**Third step (2026-09-08): the bindless texture array itself is now real, reproducibly implicated as the root cause -- full account in REVIEW.md finding #130.** Prompted by asking how macOS/Windows/game engines actually implement real-time backdrop blur: none of them route a just-rendered, same-frame offscreen target through a large persistent bindless array shared with every other texture -- they use a small, plain, dedicated, conventionally-bound sampler for that one pass. A new isolated experiment, `dual_kawase_nonbindless_experiment.rs`, tested exactly this: the identical real render-to-texture lifecycle (`begin_render_to_texture`/`end_render_to_texture`/`begin_render_to_texture_no_end`/`resume_swapchain_rendering`, unmodified), but sampling through a hand-rolled, plain `COMBINED_IMAGE_SAMPLER` descriptor instead of `bindless_textures[]`. The composited result read real, correct foreground content -- not the all-zero/background symptom every bindless attempt produces -- consistently across 4 separate real runs. This is real, reproducible evidence isolating the bindless array's own interaction with a same-frame offscreen render target as the actual cause, not a coincidence. Added to `ci.yml` (it genuinely passes, unlike `dual_kawase_blur_demo.rs`, which stays bindless-based, unmodified, and excluded). Converting the real Dual-Kawase implementation to this non-bindless approach -- which would very plausibly close this finding for good -- is real follow-on work, not done as part of this diagnostic step. Step 7.2.1 stays open, now with a real, demonstrated path forward.

**Fourth step (2026-09-08): the bindless array was never actually the cause -- the real root cause found, fixed, and verified; Step 7.2.1 closes for real.** Converting `dual_kawase_blur_demo.rs`'s real 5-hop chain to the non-bindless approach the third step proved -- graduating the experiment's own descriptor/pipeline machinery in, one descriptor set per hop -- built cleanly but *still failed* past the second hop, reading back exactly the background clear color at the interior, the identical symptom every earlier attempt produced. This was unexpected: the third step's own experiment only ever exercised a single hop. Bisecting the chain hop by hop (2-hop: passed; 3-hop, reusing the downsample pipeline a second time: failed) and then substituting sources/pipelines/destinations one variable at a time (bindless-produced L0 sampled a second time into an offscreen target: passed; non-bindless-produced L1 sampled into the swapchain: passed; non-bindless-produced L1 sampled into a second offscreen target, with the pipeline object swapped out for a different one: still failed) isolated the failure to reading a texture that was itself produced by a non-bindless pass, specifically when the destination was a second offscreen target. Printing the acquired texture's own real `dimensions()` found the actual defect: `RhiDevice::acquire_transient_target`'s own documented "oversized borrow" fallback (it hands back a *larger* already-freed texture when no free bucket of the exact requested size exists yet) returned L0's freed 256x128 texture when L2's 64x32 bucket was requested for the first time -- and `RhiCommandBuffer::draw_indexed` (`tre-rhi-vulkan/src/lib.rs`) unconditionally performs its own, second `cmd_push_constants` call using `self.width`/`self.height` (the render target's own real, now-oversized dimensions), silently clobbering the correct, already-pushed per-hop `screen_size` right before the draw executes. This confined the actual draw to a small corner of the oversized backing image, leaving the real interior/center untouched at the clear color -- exactly the symptom chased since the second session, under three different framings (bindless array, missing barrier, "second render-to-texture round trip"), none of which were the real cause. **Fix**: every non-bindless downsample/upsample/composite pass now issues its draw via a raw `cmd_draw_indexed` call instead of `RhiCommandBuffer::draw_indexed`, so the wrapper's own redundant, potentially-stale push constant call never executes for these passes; the non-bindless conversion itself (a plain `COMBINED_IMAGE_SAMPLER` per hop, matching how other engines actually do this) is kept, since it remains a real, independently-reasonable design even though it wasn't what was actually broken. Verified: the full, real 5-hop chain's own pixel assertions (interior stays foreground; edge shows genuine partial blend) pass consistently across 5 separate real runs against actual GPU hardware, with clean cleanup (no leaked Vulkan objects) each time. `dual_kawase_blur_demo.rs`'s own header has the complete technical account. Added to `ci.yml`'s `vulkan-validation` job. `dual_kawase_nonbindless_experiment.rs`'s header is updated to note it carries the same latent `draw_indexed` hazard, just never triggered by its own single-hop shape. Step 7.2.1 is closed; Step 7.2.2 (wiring this real capability to `push_layer`/`pop_layer`) is real, separate future work.

### Step 7.2.2: Wire Dual-Kawase Blur to `push_layer`/`pop_layer` -- Status: Complete (2026-09-08)

Lets a caller request a real, GPU-accelerated blur on a compositing
layer via `Canvas::push_layer`/`pop_layer`, using the Dual-Kawase chain
Step 7.2.1 proved works on real hardware. Own-content blur only
(`planning/archive/PLAN_PHASE7_STEP7_2_1.md`'s own explicit scope
choice, unchanged) -- a layer's own newly-drawn content gets blurred
before compositing, not whatever is visually behind it on the
swapchain; true backdrop blur needs a real swapchain snapshot/copy
capability this project doesn't have yet, real, disclosed future work.

Two real design forks were confirmed with the project owner before
writing any code: `LayerDesc` gains a simple `blur: bool` (fixed chain
depth matching Step 7.2.1's own proven demo, no tunable radius/quality
yet -- that step explicitly deferred tunability to "once a real
consumer needs it," and this is that consumer, so the honest next
increment is the smallest real capability, not the ceiling); and the
blur mechanism is exposed as one purpose-built trait method
(`RhiCommandBuffer::apply_layer_blur`) rather than several smaller
primitives `execute_frame` would orchestrate itself -- matching this
project's own precedent for `begin_render_to_texture_no_end` (added
narrowly for exactly the chaining need it served, not as a speculative
primitive family).

**A real, load-bearing finding from this step's own pre-work.** Before
designing this step, a scratch test checked whether REVIEW.md finding
#152's general fix (`begin_render_to_texture`'s new `logical_width`/
`logical_height` parameters) also resolved finding #130's own original
bindless-sampling symptom -- since both findings shared the same root
mechanism. It did not: a plain two-hop chain (draw a square into L0,
downsample L0 -> L1 via the *original*, bindless `kawase_downsample.
frag`, entirely through standard `RhiCommandBuffer`/`RhiDevice` trait
calls, exactly `PopLayer`'s own existing pattern) still read back the
destination's own center as background. This means finding #130's
original symptom was never *fully* explained by #152's fix alone --
there is a second, still-unexplained defect specific to bindless
sampling under this exact condition (sampling while rendering into a
*different* offscreen target), disclosed here as a known gap rather
than investigated further (see REVIEW.md's own updated finding #130
write-up). This step instead reuses `dual_kawase_blur_demo.rs`'s own
already-proven non-bindless mechanism for its *internal* downsample/
upsample chain -- unaffected by this gap, since the chain's own *final*
composite step samples while rendering into the *swapchain*, exactly
the condition `PopLayer`'s own existing composite step already proves
works.

**Design.** `RhiCommandBuffer::apply_layer_blur(&mut self, device: &dyn
RhiDevice, source: &dyn RhiTexture, width: u32, height: u32) -> Box<dyn
RhiTexture>` -- `source` is a texture already `end_render_to_texture`'d
(sampling-ready); `width`/`height` are its own real, intended/logical
size. Internally acquires L1 (half), L2 (quarter), U1 (half) transient
targets, chains downsample L0(`source`)->L1->L2 then upsample
L2->U1->U0 (full) -- `dual_kawase_blur_demo.rs`'s own proven 4-hop
shape and non-bindless mechanism, graduated into real engine capability:
4 real descriptor sets (one per hop -- reusing a single set via
`vkUpdateDescriptorSets` between binds was tried first and rejected, a
real Vulkan validation error the first actual run caught, since this
descriptor set layout deliberately has no `UPDATE_AFTER_BIND` flag,
matching the demo's own proven plain layout exactly); a new dedicated
vertex shader, `fullscreen_quad_nonbindless.vert` (outputs NDC-authored
positions directly, letting one small, cached unit quad -- uploaded
once -- serve every hop of every call regardless of the caller's real
width/height, so no per-call vertex-buffer allocation is ever needed,
keeping this whole operation free of dynamic RHI allocation inside the
render tick, DESIGN.md Section 2.6, beyond its own one-time setup);
every draw issued via a raw `cmd_draw_indexed` call, matching REVIEW.md
finding #130's own real fix. Releases L1/L2/U1 internally; returns U0,
already sampling-ready -- the caller owns and releases it exactly like
any other transient target acquired directly.

The blur-specific descriptor set layout/pool/4 sets/sampler/pipeline
layout/2 pipelines/unit quad buffers are created lazily, once, cached
on `VulkanDevice` (`blur_resources: Arc<Mutex<Option<BlurResources>>>`,
the same established pattern `transient_pool` already uses) -- no
caller, `execute_frame` included, needs any awareness that blur
pipelines exist at all. `VulkanCommandBuffer` holds its own `Arc` clone
(plus `instance`/`physical_device`/`stencil_format` copies, the same
reason `bindless_descriptor_set` is already copied in at construction),
since `apply_layer_blur`'s own `device: &dyn RhiDevice` parameter is a
trait object with no way back to `VulkanDevice`'s own concrete fields.

`LayerDesc` gains `blur: bool`. Threaded through the IR by reusing
`PopLayer`'s own otherwise-inert `texture_handle` field (`0`/`1`) --
matching `PushLayer`'s own established precedent of smuggling
`LayerDesc` data through an otherwise-unused field
(`texture_format_to_u16`/`pipeline_state_id`) rather than widening
`UiDrawCommand`. `execute_frame`'s `PushLayer` handling now also tracks
the layer's own requested width/height alongside its texture (needed at
`PopLayer` time to call `apply_layer_blur` with the right size);
`PopLayer` branches on the smuggled flag -- if set, calls `apply_layer_
blur`, releases the original unblurred texture, and composites the
*returned* blurred one instead; if unset, the existing Step 6.4.2 path
is untouched.

**A real bug found and fixed by an actual GPU run, not caught by design
review.** The first real run of `layer_blur_demo.rs` hit a genuine
Vulkan validation error: `apply_layer_blur`'s own internal hops rebind
the command buffer's vertex/index buffers to its own small, unit-quad
ones, but nothing rebound the frame's *real* shared vertex/index
buffers afterward -- REVIEW.md finding #135's own "bound once, at the
top" invariant left the composite draw immediately following reading
through `apply_layer_blur`'s own tiny 24-byte index buffer at the
composite quad's real (much larger) offset, an out-of-bounds read
caught immediately by validation. Fixed by having `execute_frame`'s own
`PopLayer` handling re-bind the frame's real vertex/index buffers right
after the `apply_layer_blur` call returns -- the same "restore whatever
this call disturbed" responsibility `resume_swapchain_rendering`'s own
scissor-restore already established at Step 6.4.2.

**Verification.** New demo (`layer_blur_demo.rs`, `demo/phase7_step7_
2_2/`): reproduces `dual_kawase_blur_demo.rs`'s own real pixel-assertion
shape (bounded interior stays foreground; a point just outside the
original content's hard edge shows a genuine partial blend), recorded
entirely through `Canvas`/`execute_frame` -- `push_layer(&LayerDesc {
blur: true, .. })`, one `draw_rounded_rect` in the layer's own local
space, `pop_layer`, `flatten()`, one `execute_frame` call, no
hand-written RHI calls anywhere. Both real pixel assertions pass,
confirmed across 4 separate real runs against actual GPU hardware.
Added to `ci.yml`'s `vulkan-validation` job. Full regression sweep:
every pre-existing Vulkan demo re-run manually, zero regressions,
`canvas_layer_composite_demo.rs`/`canvas_combined_scene_demo.rs`
specifically (their own un-blurred `PushLayer`/`PopLayer` paths, proving
the `blur: false` branch stays exactly Step 6.4.2's own original
behavior). `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace, including a new `tre-engine` unit test proving
`execute_frame` calls `apply_layer_blur` and composites its returned
texture (not the original) when `blur: true`.

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

### Step 8.1.1: Real Frame Clock & Spring-Decay Primitives -- Status: Complete (2026-09-08)

Split from Step 8.1 after real investigation found task 3 (wiring the
entire engine into one real, continuously-running 8-stage loop) needs a
genuinely separate, larger effort of its own -- confirmed, not assumed:
no demo anywhere combines all 8 named stages together (`canvas_sub_
canvas_demo.rs` combines the most -- SubCanvas + stitch + atlas +
`execute_frame` -- but is single-frame and headless), and `execute_
frame` was found to hardcode a zero vertex/index buffer offset, so it
cannot yet accept a real per-frame ring-buffer-backed buffer at all
(`RhiDynamicRingBuffer::write`'s one real, non-test call site,
`memory_pools_demo.rs`, proves only pool/segment rotation mechanics,
never real per-frame streaming) -- confirmed with the project owner via
AskUserQuestion: split into 8.1.1/8.1.2, this step covers tasks 1-2
only (`PLAN.md`, archived to `planning/archive/PLAN_PHASE8_STEP8_1_1.md`).

Two new, independent, zero-consumer primitives, matching this project's
own established "build and prove the primitive before its exact
consumer exists" precedent (`Affine2::compose_batch`, `tone_map`):

`FrameClock` (`tre-engine`) wraps `std::time::Instant` -- the portable
primitive TECHNICAL.md Section 7.1's own `QueryPerformanceCounter`/
`clock_gettime(CLOCK_MONOTONIC)` requirement already names, since Rust's
standard library implements `Instant` on top of exactly those platform
APIs internally, needing no hand-rolled per-platform `#[cfg]` code. Its
one method, `tick(&mut self) -> f32`, returns the real elapsed seconds
since the previous call, with the first call returning exactly `0.0`
(no prior tick to measure from). Lives in `tre-engine`, not `tre-math`:
it owns real, mutable frame-to-frame state (the previous tick's own
timestamp), an engine-lifecycle concern, not a pure numeric function.

`spring_decay(current, target, lambda, dt) -> f32` (`tre-math`)
implements this step's own task 2 formula exactly, $x(t + \Delta t) =
x_{\text{target}} + (x(t) - x_{\text{target}}) \cdot e^{-\lambda \Delta
t}$ -- pure exponential decay toward a target, monotonic, never
overshoots. Deliberately *not* a mass-spring-damper ODE with
oscillation (position + velocity state, critical/under/over-damping):
"spring physics and lerp decay" names one formula, not two, and
implementing anything beyond it would be inventing scope the outline
never specified.

Neither primitive is wired to any real consumer yet -- deliberately:
`FrameClock`'s real consumer (the continuous main loop) and `spring_
decay`'s (real animation/UI state) are both Step 8.1.2's job or later,
matching how `Affine2::compose_batch` and `tone_map` were each built and
proven correct before their own real consumers existed.

Verified by `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across
the workspace. Real unit tests for both: `FrameClock` -- first `tick()`
returns `0.0`; two ticks separated by a real `std::thread::sleep`
report a delta consistent with the real sleep duration (a real, if
coarse, hardware-timing check, not a mocked clock). `spring_decay` --
`dt == 0.0` returns `current` unchanged; `lambda == 0.0` returns
`current` unchanged regardless of `dt`; a large `dt` converges to
within a small epsilon of `target`; monotonic approach across
increasing `dt` never crosses `target`. Confirmed via grep that no
existing demo or test touches either new item -- both are net-new,
zero-consumer additions, as scoped.

### Step 8.1.2: The Full 8-Stage Continuous Main Loop -- Status: Complete (2026-09-08)

Wires the entire engine into one real, continuously-running, windowed
loop executing this step's own strictly-enforced 8-stage sequence every
frame -- *Wait Fences -> Drain Events -> Multi-Thread Canvas ->
Sub-Canvas Stitch -> Tessellation/Atlas Check -> Radix Sort & Batch ->
Ring Buffer Packing -> RHI Submit & Present* -- and fixes the real,
previously-undiscovered blocker Step 8.1.1's own plan found:
`execute_frame` hardcoded a zero vertex/index buffer offset, so it could
never accept a real per-frame `RhiDynamicRingBuffer`-backed buffer
(`planning/archive/PLAN_PHASE8_STEP8_1_2.md`).

Real investigation before writing any code found every one of the 8
stages already had one real, individually-proven mechanism somewhere in
the codebase; no demo had ever combined all 8 together. Wait Fences is
already inside `RhiDevice::begin_frame` itself (`VulkanDevice::begin_
frame`'s own `wait_for_fences`/`reset_fences` calls), not a separate
call a loop needs to add. Drain Events is `PlatformConnection::poll_
events`, already used exactly this way by every prior windowed demo.
Multi-Thread Canvas/Sub-Canvas Stitch's real per-frame recipe
(`create_sub_canvas` + `std::thread::scope` + `stitch_into`) was already
fully proven by Step 5.2.3's capstone -- just never repeated across more
than one frame. Radix Sort & Batch (`FrameArena::flatten()`) and RHI
Submit & Present (`submit_and_present`) needed no new code at all.

**A second real gap was found, beyond `execute_frame`'s offset, by
reading `tre-atlas`'s `AtlasOwner` in full.** Its background thread
owns the shared atlas pixel buffer entirely privately -- the only way
to ever read it back is `AtlasOwner::join(self) -> Vec<u8>`, which
*stops the thread*. There is no live "peek at the atlas while it keeps
running" API, so a loop that tried to add genuinely new glyphs mid-run
would have no way to see them land on screen. Scoped around, not
silently ignored (see "Explicitly out of scope" below): this step's
atlas is fully pre-seeded before the loop starts, exactly matching
every prior text-drawing demo's own real, proven pattern -- the
Tessellation/Atlas Check stage is still real and exercised every frame
(`draw_text`'s own `atlas_context.atlas.lookup` call, real-touching
`SwmrSlotTable`'s Step 4.3.1 recency tracking), it simply never misses
during this demo's run.

**The `execute_frame` fix, precisely.** `RhiCommandBuffer::bind_vertex_
buffer`/`bind_index_buffer` already accepted an `offset: u32` parameter;
`execute_frame` itself called both with a hardcoded `0` literal
regardless. Fixed by replacing the separate `vertex_buffer`/`index_
buffer` parameters with two `BufferBinding<'a> { buffer: &'a dyn
RhiBuffer, offset: u32 }` values -- bundled, matching this crate's own
`GlyphAtlasContext` precedent for `draw_text`, both because a buffer and
its own offset are always meant to travel together and because
`clippy::too_many_arguments` fires at the resulting 9-parameter
signature otherwise (this project's own established response to that
lint, per `draw_text`'s own doc comment, is to bundle groupable
parameters, not blanket-allow the lint). All 12 pre-existing call sites
(7 `tre-engine` unit tests, 5 demos) updated to pass `offset: 0`,
preserving their exact prior behavior -- confirmed via `grep -rn
"execute_frame("` before making the change, then by re-running all 5
demos manually afterward, zero regressions.

New demo, `main_loop_demo.rs` (`demo/phase8_step8_1_2/`): a real
windowed, `CloseRequested`-aware, env-var-frame-capped loop
(`TRE_MAIN_LOOP_FRAMES`, default 90) matching `walking_skeleton.rs`/
`multi_window.rs`/`input_demo.rs`'s own proven shape. Every frame: 2
real worker threads (`std::thread::scope`, freshly spawned every frame)
each record a `SubCanvas` -- one draws a static rect, the other also
draws the one pre-seeded text glyph -- and stitch into a shared
`FrameArena` alongside the root canvas's own animated rect; the arena
flattens; the flattened vertex/index bytes are written into a real
`RhiDynamicRingBuffer` via `write()`; the offsets it returns feed the
now-fixed `execute_frame` directly; `submit_and_present` presents the
frame. Step 8.1.1's own two, until-now zero-consumer primitives get
their first real consumer here: `FrameClock::tick()` supplies a real
`dt` every frame, and `spring_decay` uses it to animate the root rect's
`x` position toward a fixed target.

**Verification, honestly adapted to a real constraint this
investigation also found.** `VulkanSwapchain` (a real window/compositor
surface) has no pixel-readback method, unlike `HeadlessSwapchain` --
every prior demo that asserts real pixels does so only headless. This
demo instead records its own real per-frame `dt` sequence and the
`x` position it actually drew each frame, then, after the loop ends,
independently replays `spring_decay` over that exact recorded `dt`
sequence and asserts it reproduces the exact position sequence the loop
drew, plus that the final position is strictly closer to the target
than the first and never overshoots it -- a stronger check than a
pixel read would give, since it verifies the formula was applied
correctly every single frame, not just that something moved. Run for
real against a live GPU and display during this step's own
implementation (not only under CI's `xvfb-run`): 90 frames presented,
zero Vulkan validation-layer errors, animation verified end to end
(`x` moved from `40.0` to `559.89` against a `560.0` target). All 5
demos whose `execute_frame` call site changed shape were also re-run
manually, zero regressions. Added to `ci.yml`'s `vulkan-validation` job.

## Explicitly out of scope (Step 8.1.2)

- **Live, mid-run atlas growth** -- a new `AtlasOwnerHandle` API to read
  the atlas's current pixels while its background thread keeps running,
  rather than only via the terminal, thread-stopping `join()`. Real,
  separate future engineering, named here rather than silently dropped.
- **A persistent, reused worker-thread pool** for Multi-Thread Canvas --
  `std::thread::scope` fresh every frame, matching Step 5.2.3's own
  proven recipe, is what this step builds; pooling is real future
  performance work, not named by this step's own task list.
- **SVG tessellation/morphing inside the loop**, and **Windows/macOS** --
  this project's own standing deferrals, unchanged.
- **Multi-frame `push_layer`/`pop_layer` round-trips** (REVIEW.md finding
  #150) -- this loop never composites an offscreen layer; that
  combination remains proven only single-frame/headless (Steps
  6.4.1/6.4.2/6.5). Also means the sibling-`PushLayer`/`PopLayer`-in-one-
  frame bindless/texture-aliasing bug (finding #138) is never exercised
  by this demo.
- **A cross-frame reuse API for `FrameArena`/`ScatterArena`/
  `RenderingCanvas`** (REVIEW.md finding #134) -- this loop constructs a
  fresh `FrameArena` and up to 3 fresh `RenderingCanvas`/`SubCanvas`
  instances every single frame (no `reset()`/reuse method exists on any
  of the three types), roughly 20+ heap allocations per frame for a
  trivial scene -- squarely inside DESIGN.md Section 2.1's own named
  "RenderingCanvas recording, intermediate representation flattening"
  zero-allocation boundary. Real future work: an in-place `reset()`/
  `clear()` API on all three types, so a real application can own one
  long-lived instance instead of rebuilding it every frame.
- **Recovering from a window resize** (REVIEW.md finding #151) -- the
  disclosed swapchain-recreation gap (finding #116, Step 1.1) means
  resizing this demo's window mid-run panics the whole process
  (`EngineError::SwapchainOutOfDate`, surfaced correctly but never
  handled anywhere). This is the first step where that long-disclosed
  gap becomes a concrete, guaranteed-panic risk in the project's own
  reference main-loop shape, not just a latent one in short demos.

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

#### Step 9.1: Correctness Test Suite -- Status: Complete (2026-09-09)

Real pre-work investigation, before writing any test, found this step's
own task list rested on two documented behaviors that had never actually
been built or no longer matched the real code -- both surfaced to the
project owner via `AskUserQuestion` rather than assumed, since each was
a real scope fork, not an implementation detail:

**Discrepancy 1 -- the radix sort never existed.** TECHNICAL.md Section
4 and ARCHITECTURE.md Section 4.1 both specified a linear $\mathcal{O}(N)$
4-pass radix sort for the 64-bit draw-command sort key (also named in
this file's own Architectural Decision Matrix above); `UiDrawCommand::
sort_key`'s own doc comment already called it a "64-bit Radix Sort Key."
The real `flatten_run` had always used `std::sort_unstable_by_key` (a
comparison sort) instead, unchanged since Step 5.1.3 first built it --
task 1's own "unit-test the radix sort against adversarial key
distributions" had no real subject to test. Resolved (confirmed via
AskUserQuestion: "Build the real radix sort now"): a genuine, from-
scratch LSD (least-significant-digit) radix sort, `radix_sort_by_key`
(`tre-engine`), 4 passes of a 16-bit digit each ($4 \times 16 = 64$
bits), each pass a counting sort (histogram, prefix-sum, scatter),
ping-ponging between caller-provided `items`/`scratch` buffers so no
per-call heap allocation is needed -- matching this project's own
established "allocate once, reuse across the frame" discipline already
used for transient pools/ring buffers. `segment_and_flatten` now
allocates one `sort_scratch` buffer up front and threads it through
every `flatten_run` call for the frame. Deliberately no small-N
comparison-sort fallback threshold: that is TECHNICAL.md Section 9.2's
own performance-tuning concern, out of scope for this correctness pass.
Adversarially tested per task 1's own list -- all-identical keys,
fully reverse-sorted input, keys clustered at every field boundary
(Layer/Pipeline/Texture/Depth ID, per ARCHITECTURE.md Section 4.1's bit
layout), maximum Depth ID values, empty and single-element runs, a
`should_panic` mismatched-scratch-length guard, and 200 rounds of
randomized inputs (both full-`u64`-range and narrow-range keys) checked
byte-for-byte against `sort_unstable_by_key`'s own output as the
independent oracle -- plus a `flatten_unbatched` (below) whole-pipeline
GPU proof that the new sort produces identical rendered output to the
old comparison sort ever did.

**Discrepancy 2 -- the documented atlas-exhaustion fallback was never
real.** DESIGN.md Section 2.6 described falling back to "a lower-
fidelity placeholder (e.g., a bounding-box glyph or solid-color swatch)"
when eviction cannot free enough atlas space; the real `AtlasOwner::
process_insert` has always silently dropped the request instead (`let
Some(rect) = packer.insert(..) else { return; }`), with a prior
session's own code comment already independently reasoning this
satisfies the section's "report, don't block" contract. Resolved
(confirmed via AskUserQuestion: "Correct the docs, test the real drop
behavior"): DESIGN.md Section 2.6 rewritten to describe the real
silent-drop behavior; task 2's own "placeholder glyph fallback" wording
retired as describing something that was never built. The real drop
path is now covered by a dedicated test (below) instead.

**Task 2, precisely characterized via test-driven investigation, not
assumption.** Two real, non-obvious boundary conditions in the
Guillotine atlas's eviction logic were found only because the first
version of the new fragmentation/eviction test failed against its own
wrong mental model, then were fixed and turned into real regression
coverage: `maybe_evict_stale_entries`'s `EVICTION_CAPACITY_THRESHOLD
= 0.85` check reads the atlas's **pre-insert** `used_fraction()` --
an insert sequence that would only cross 85% counting the incoming
request's own space never triggers eviction at all, so the first test
draft (three 81%-usage entries) saw nothing evicted. And `SwmrSlotTable
::scan_older_than`'s staleness comparison is **strict** (`last_used <
cutoff_frame`): an entry idle for *exactly* `EVICTION_MIN_IDLE_FRAMES`
(600) frames survives, only strictly-longer idle time is evicted -- the
second test draft assumed the boundary frame itself was evicted and
failed until corrected. Both corrected assumptions are now permanent,
passing regression tests (`eviction_boundary_is_correct_across_several_
entries_with_mixed_staleness_at_once`), alongside a 40-round sustained
insert/evict churn test and a dedicated test for the real silent-drop
path (`a_request_that_can_never_fit_even_after_a_full_eviction_pass_is_
silently_dropped`) proving a permanently-oversized request is dropped
every time, over 1000 real polling iterations, without ever wedging the
atlas thread or corrupting a subsequent normal insert.

**Task 3, the batching-equivalence test, needed one new API.**
`RenderingCanvas` only ever exposed `flatten()`, which always merges
adjacent same-Layer/Pipeline/Texture/clip commands -- there was no way
to isolate *batching itself* as the one variable under test. Added
`RenderingCanvas::flatten_unbatched()`, identical to `flatten()` in
every way except it passes `merge: false` through to `segment_and_
flatten`/`flatten_run`, which now gate their merge decision on that
flag. A real GPU demo, `batching_equivalence_demo.rs` (`crates/tre-rhi-
vulkan/examples/`, `demo/phase9_step9_1/`), records the identical
four-rect scene twice, renders both through the real, unmodified
`execute_frame`, and asserts the two framebuffers are byte-for-byte
identical -- confirming `flatten()`'s batched output (1 draw call) and
`flatten_unbatched()`'s unbatched output (4 draw calls) are visually
indistinguishable, exactly the invariant this step's own task 3
rationale names. Passed on the first real run, confirmed stable across
3 repeated runs. Added to `ci.yml`'s `vulkan-validation` job.

**Task 4, fuzzing, used a disclosed substitution for `cargo-fuzz`.**
`cargo fuzz --version` errors ("no such command") and `rustup toolchain
list` confirms only `stable-x86_64-unknown-linux-gnu` is installed --
real coverage-guided fuzzing needs a nightly toolchain unavailable in
this environment. `proptest` (added as a `tre-svg` dev-dependency) is a
well-established, stable-Rust property-testing crate serving the same
real intent: generating randomized/adversarial inputs and asserting
bounded behavior, with automatic shrinking of any failing case to a
minimal reproduction. Three properties, each bounded to a 2-second
per-case wall-clock budget: `parse_svg` never panics or hangs on
arbitrary byte sequences (0-4096 bytes); `parse_svg` never panics or
hangs on syntactically-plausible-but-adversarial path `d` data; `tri
angulate` never panics or hangs on arbitrary point sets (0-500 points,
full `f32` coordinate range). All three passed (256 generated cases
each, the crate's own default) with no shrinking ever required --
disclosed here, not silently substituted, per REVIEW.md.

**Full-workspace verification.** `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo build --workspace
--all-targets`, and `cargo test --workspace` all clean (`tre-engine`
72 tests, up from 64; `tre-atlas` 22 tests, up from 18; `tre-svg` 28
tests, up from 25). A full manual regression sweep of all 31 real
Vulkan demos re-run after the sort-algorithm swap under `flatten()`/
`execute_frame`'s entire draw pipeline: 30 passed; `canvas_
accessibility_verify` failed only on the same pre-existing, already-
documented environmental limitation as REVIEW.md finding #126 (no
AT-SPI registry daemon in this sandbox to flip `org.a11y.Status.
IsEnabled`), unrelated to any change this step made.

## Explicitly out of scope (Step 9.1)

- **Step 9.2** (zero-allocation debug guard and `PushLayer`/`PopLayer`
  balance-assertion CI gating) -- confirmed via AskUserQuestion as a
  real, separate future step, not part of this pass.
- **A small-N comparison-sort fallback threshold** for `radix_sort_by_
  key` -- a real, deliberate scope boundary; that is performance tuning
  (TECHNICAL.md Section 9.2's own concern), not correctness.
- **The placeholder-glyph atlas-exhaustion fallback** DESIGN.md Section
  2.6 previously described -- confirmed never real and not built now;
  the section is corrected to describe the real silent-drop behavior
  instead.
- **Real coverage-guided fuzzing via `cargo-fuzz`** -- needs a nightly
  Rust toolchain unavailable in this environment; `proptest` is a
  disclosed, real substitution serving the same intent, not a silent
  scope reduction.

### Step 9.2: Zero-Allocation & Balance Assertions in CI

* **Implementation Tasks:**

  1. Run the full test suite under the zero-allocation debug guard (TECHNICAL.md Section 3.4) and fail the build on any allocation observed during a render tick.

  2. Run the full test suite under the `PushLayer`/`PopLayer` balance assertion (Phase 2, Step 2.2) and fail the build on any nonzero depth at frame boundary.

* **Technical Rationale:** These are cheap, deterministic, always-on gates that turn two of the engine's most important invariants -- zero steady-state allocation and balanced transient resource acquisition -- into build failures instead of production incidents.

#### Step 9.2: Zero-Allocation & Balance Assertions in CI -- Status: Complete (2026-09-09)

Real pre-work investigation found this step's own two tasks in very
different states. Task 2 (the balance-assertion gate) turned out to
already be real: the `debug_assert_eq!`/`debug_assert!` checks in
`flatten()`/`flatten_unbatched()`/`FrameArena`'s stitch path already
existed (Phase 2 Step 2.2, extended through Step 5.1.3), were already
covered by 4 passing `should_panic` tests, and CI's `test`/`vulkan-
validation` jobs already build in debug mode (`cargo test`/`cargo run`,
no `--release`) -- an unbalanced stack anywhere already failed CI
before this step touched anything. This task needed one more explicit
regression test on the new reused-canvas path, not new enforcement
machinery.

Task 1 (the zero-allocation debug guard) was the opposite: TECHNICAL.md
Section 3.4 fully specified a custom `#[global_allocator]` wrapper
checking a thread-local "render tick active" flag, but nothing
implementing it existed anywhere -- confirmed via grep, zero hits for
`global_allocator`/`GlobalAlloc`/`thread_local` in the whole codebase.
Built for real: `tre_memory::DebugAllocGuard` (a `GlobalAlloc` wrapper
around `System`) and `tre_memory::RenderTickGuard` (the RAII scope
guard), in `tre-memory` rather than `tre-engine` -- `tre-engine` carries
`#![forbid(unsafe_code)]`, and implementing `GlobalAlloc` requires
`unsafe`; `tre-memory` is one of the four workspace locations
TECHNICAL.md Section 9.1 permits it in. A real, non-obvious
implementation detail found and fixed during development: both
`DebugAllocGuard::check`'s own violation panic and `RenderTickGuard::
begin`'s own nesting-guard panic must disarm the thread-local flag
*before* calling `panic!()` -- the panic machinery's own allocations
(backtrace capture, payload boxing) would otherwise re-enter the check
while the flag was still set, in one observed case causing a genuine
double-panic process abort rather than reporting the real violation.

**Wiring the guard to the real main loop required fixing REVIEW.md
finding #134 first, exactly as confirmed with the project owner.**
`main_loop_demo.rs`'s ~20+ per-frame allocations (a fresh root
`RenderingCanvas`, a fresh `SubCanvas` per worker, a fresh
`Arc<FrameArena>`, every iteration) were fixed by building every one of
those once, before the loop, and reusing them every frame instead:
`RenderingCanvas::reset()` (new -- clears every internal `Vec` while
keeping capacity, reseeds `state_stack` to one identity entry, resets
the shared `next_depth_id` counter to 0, available on `SubCanvas`
automatically via `DerefMut`); `RenderingCanvas::stitch_into`/
`SubCanvas::stitch_into` changed from consuming `self` to borrowing
`&self` (the method only ever copies data out, never needed ownership
-- this is what actually makes reusing a `SubCanvas` across frames
possible, and let `SubCanvas::stitch_into`'s own `std::mem::take`
Drop-workaround be deleted entirely); `FrameArena::flatten_into` (new
-- the non-consuming sibling of `flatten()`, draining each internal
`ScatterArena` via a new `ScatterArena::reset`/`drain_into` pair into a
caller-reused `FlattenedFrame` instead of allocating fresh output
`Vec`s every call, sharing its sort/merge core with the existing
`segment_and_flatten` via a new `sort_and_batch_into` helper). The old
`Arc<FrameArena>`/`Arc::try_unwrap` dance was removed entirely, not just
made reusable: `std::thread::scope` lets its spawned closures borrow a
plain owned `FrameArena` directly, so the `Arc` was never load-bearing.

**Two more real, previously-undetected allocation sources were found
only because the guard was actually run against real GPU hardware, not
assumed correct from code review** (REVIEW.md findings #156-158,
#157 fixed, #156/#158 disclosed as genuine, separate scope boundaries):
`radix_sort_by_key`'s own `counts` histogram buffer was allocated fresh
on every call, not just `scratch` as Step 9.1 intended -- fixed by
threading it through as a caller-provided, reused parameter, the same
pattern `scratch` already used. `VulkanDevice::begin_frame` allocates a
fresh `Box<dyn RhiCommandBuffer>` every frame despite reusing the
underlying Vulkan handle -- a real trait-boundary redesign, not fixed
here, disclosed in `main_loop_demo.rs`'s own header comment as the
reason RHI submission stays outside the guard's coverage. `std::thread::
scope` itself allocates an `Arc<ScopeData>` bookkeeping value on every
call -- real standard-library behavior, not a bug, and exactly the cost
of this project's own already-disclosed "fresh OS thread every frame,
no persistent pool" design (Step 8.1.2); the main thread's own guard
coverage is split into two spans bracketing the unguarded `thread::
scope` call, while each worker's own guard (started inside its spawned
closure) still covers that worker's real work in full.

**A minimal criterion performance suite was also built** (confirmed
with the project owner, beyond this step's own literal task list, since
TECHNICAL.md Section 9.2 explicitly names it as running "alongside" the
allocation guard): `crates/tre-engine/benches/frame_processing.rs`,
one representative benchmark (`record_and_flatten_10k_nodes`, at the
Architectural Decision Matrix's own stated ">10,000 active nodes"
scale) measuring real `RenderingCanvas` record-then-`flatten()` cost.
Real, honest result: ~796µs mean, roughly 1.6x the documented
$\le 0.50\text{ ms}$ budget -- the first time this budget has ever been
measured against real code. Confirmed with the project owner: this is
real, separate performance-tuning work, not fixed here, and the new
`ci.yml` `test`-job step that parses the bench's own reported time and
fails the build if it exceeds the budget is deliberately wired to fail
honestly on this real gap (REVIEW.md finding #159) rather than being
silently skipped or having its threshold quietly loosened to match
current reality.

**Full-workspace verification.** `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo build --workspace
--all-targets`, and `cargo test --workspace` all clean (`tre-memory` 41
tests, up from 32; `tre-engine` 78 tests, up from 72). `main_loop_demo`
re-run live against real GPU hardware multiple times: 90 real frames
each run, zero allocations detected inside any guarded span, the
existing spring-decay animation verification still passing exactly as
before. A full manual regression sweep of all 31 pre-existing Vulkan
demos, re-run after `stitch_into`'s signature change and
`radix_sort_by_key`'s new `counts` parameter: 30 passed; `canvas_
accessibility_verify` failed only on the same pre-existing,
already-documented environmental limitation as finding #126, unrelated
to this step.

## Explicitly out of scope (Step 9.2)

- **Fixing `begin_frame`'s per-frame `Box<dyn RhiCommandBuffer>`
  allocation** (finding #156) -- genuine `RhiDevice` trait-boundary
  redesign work, rippling through all 31 demo call sites; disclosed,
  not fixed.
- **A persistent, reused worker-thread pool** for Multi-Thread Canvas --
  still real, separate future work (Step 8.1.2's own disclosed
  boundary, unchanged); this step reuses each worker's `SubCanvas`
  *data*, never claimed to eliminate `std::thread::scope`'s own
  per-call allocation (finding #158).
- **Optimizing the sort/flatten pipeline to actually meet the
  documented $\le 0.50\text{ ms}$ budget** (finding #159) -- real,
  separate performance-tuning work (TECHNICAL.md Section 9.2's own
  scope); the new CI gate is deliberately left failing on this real,
  honestly-measured gap rather than quietly loosened.
- **A full per-demo-scene benchmark suite** -- this step builds one
  minimal, real, representative benchmark; TECHNICAL.md Section 9.2's
  own larger scope, not this step's task list.

## Phase 10: Cross-Language Bindings & Python UI Framework Integration (Added with the Rust/Python Language Decision)

### Step 10.1: Efficient Shape Primitives for External UI Frameworks (Added 2026-09-09, ahead of the C-ABI crate so the ABI has a real, designed surface to expose)

* **Implementation Tasks:**

  1. Define a shared `PrimitiveCommon` struct (`transform: Transform2D`, `opacity: f32`, `blend_mode: BlendMode`, `visibility: Visibility`, `hit_testable: bool`) embedded by composition in every concrete shape, plus a lightweight `Primitive` accessor trait (`common()`/`common_mut()`) so generic code can touch the shared fields without matching every shape variant -- not a `dyn Primitive` object-safety-driven hierarchy, since per-shape dynamic dispatch in the per-frame flattening pass would violate TECHNICAL.md Section 9.1's "no dynamic type inspection in hot paths" rule.

  2. Define `Rectangle`, `Circle`/`Ellipse` (unified via `radius: Vec2`), `Polygon` (covers triangle/hexagon/N-gon/star via `sides`/`star_points`), and `Path` (`commands: Vec<PathCommand>`) per ARCHITECTURE.md's new canonical shape-primitive section, wrapped in one `enum ShapePrimitive { Rectangle(Rectangle), Circle(Circle), Polygon(Polygon), Path(Path) }` for the same enum-dispatch-over-hot-path reasoning as task 1.

  3. Build `ShapeRegistry`, a hand-built generational slot arena (matching this project's own established "build the concurrency/memory primitive, don't reach for a crate" precedent -- `ScatterArena`, `SwmrSlotTable`, `MpscRingBuffer`) mapping a `ShapeId { index: u32, generation: u32 }` to a `ShapePrimitive` plus its state hooks (`active_animations: Vec<AnimationId>`, `layout_dirty: bool`, `clip_bounds: Option<ScissorRect>`) -- the retained-mode store an external UI framework holds handles into across many frames, distinct from `RenderingCanvas`'s existing per-frame immediate-mode IR.

  4. Build the per-frame flattening pass: for every `ShapeId` that is `layout_dirty` or has a non-empty `active_animations`, resolve its `Transform2D` to a real `Affine2` (`tre-math`, already built) and translate it into the existing `RenderingCanvas` draw calls (`draw_rounded_rect` today for the uniform-radius `Rectangle` case; `tre-svg`'s existing ear-clipping tessellator for filled `Path` shapes) -- shapes are a retained, ergonomic *description* layer that compiles down into the same IR/sort/batch pipeline every other caller uses, never a second rendering path.

* **Technical Rationale:** An external UI framework -- Python via Phase 10 Step 10.3's direct PyO3 binding (revised 2026-09-09, no longer routed through Step 10.2's `tre-ffi`; see DESIGN.md Section 2.7), or any other language via Step 10.2's `tre-ffi` C-ABI -- needs to create a shape once, mutate a handful of properties across many frames, and let the engine decide what actually needs re-recording -- the existing immediate-mode `Canvas` API requires re-issuing every draw call every frame even for static content. A retained shape layer that still funnels into the identical, already-proven IR/radix-sort/batch/RHI pipeline (Phases 5 and 9) gets ergonomics without a second, competing rendering path or its own correctness risk.

* **Explicitly out of scope, disclosed not silently dropped:** real GPU/tessellation work for visual features this step's own data model names but the renderer does not yet support -- per-corner `corner_radius`/`corner_smoothing` (today's `draw_rounded_rect` only takes one uniform radius, exactly the gap its own Step 3.2 doc comment already named), `Circle`/`Ellipse`'s `arc_length` partial-sweep rendering, `Polygon`/`Star` procedural geometry and `vertex_radius` corner rounding, `Path` rendering entirely (both stroking, per `stroke_line_cap`/`stroke_line_join`, *and* filling -- corrected during implementation, see this step's own "Status: Complete" write-up below for why filling turned out to need more than wiring an existing function), `border_color`/`border_thickness` outline rendering on any shape, and `FillStyle::Gradient` (DESIGN.md's own architecture diagram already names a "Dynamic Gradient & Pattern Fill Evaluator" as unbuilt). See `documentation/ARCHITECTURE.md`'s new shape-primitive section and the archived per-step plan for the full, itemized disposition of every field against what real rendering support exists today.

#### Step 10.1: Efficient Shape Primitives for External UI Frameworks -- Status: Complete (2026-09-09)

Real, in `crates/tre-engine/src/shapes.rs` (a new module, re-exported at
the crate root): `PrimitiveCommon`/`Transform2D` (resolved to a real
`tre_math::Affine2` via `Affine2::compose`, exactly the translate-then-
rotate-then-scale composition ARCHITECTURE.md Section 7.1 specifies),
`BlendMode`/`Visibility`, the `Primitive` accessor trait, `FillStyle`/
`GradientId`/`CornerRadii`, all four concrete shapes (`Rectangle`/
`Circle`/`Polygon`/`Path`) plus `ShapePrimitive`'s enum dispatch, and
`ShapeId`/`AnimationId`/`ShapeSlot`/`ShapeRegistry` (a hand-built
generational slot arena with real `insert`/`remove`/`get`/`get_mut`,
and `flatten_into`, the per-frame flattening pass).

**One real discrepancy found during implementation, corrected rather
than silently built around or silently expanded.** Task 4's own
original wording named `tre-svg`'s existing ear-clipping tessellator as
the real rendering path for filled `Path` shapes. Investigating what
that would actually take found it is not "wire an existing function" --
`tre-svg`'s tessellator consumes an already-flattened polygon point
list, and `Path::commands` is a `Vec<PathCommand>` of `MoveTo`/`LineTo`/
quadratic/cubic Bezier segments that nothing in this codebase yet
flattens into that point-list form (SVG's own path parsing, Step 3.3.1,
does this internally via the `usvg` dependency, not via any function
this module could call directly). Real, separate future work, not a
bug-fix-sized addition -- confirmed via investigation, not assumed;
`Path` rendering (fill and stroke both) is corrected into this step's
own "Explicitly out of scope" list above rather than left as a stale
claim of support that was never actually built.

**Verified.** 12 new `tre-engine` unit tests: `ShapeRegistry` insert/
remove/reuse/stale-handle-rejection, `Transform2D::to_affine2`'s own
composition order, `flatten_into`'s dirty/animating/`Hidden`/`Collapsed`
semantics, and a real panic for every field combination without
rendering support (non-uniform `corner_radius`, `Circle`). A new real
GPU demo, `shape_registry_demo.rs` (`demo/phase10_step10_1/`), proves
the one real-rendering-supported case (`Rectangle` with a uniform
`CornerRadii`, `FillStyle::Solid`, no border, no smoothing) produces
byte-for-byte identical pixels whether drawn directly via today's
`draw_rounded_rect` or via `ShapeRegistry::insert` + `flatten_into` --
confirmed stable across 3 real runs. `cargo fmt`/`clippy -D warnings`/
`build`/`test` clean across the whole workspace (`tre-engine` 90 tests,
up from 78). A full manual regression sweep of all 32 Vulkan demos
(the 31 pre-existing plus this step's own new one): 31 passed;
`canvas_accessibility_verify` failed only on the same pre-existing,
already-documented environmental limitation as finding #126, unrelated
to this step -- this step is purely additive (a new module, no existing
public API signature changed), so zero regressions were expected and
confirmed.

### Step 10.2: Full Shape Rendering Support (Added 2026-09-09)

* **Implementation Tasks:**

  1. Add a bindless binding-1 `STORAGE_BUFFER` (`RhiDevice::shape_style_buffer`) to `tre-rhi-vulkan`'s existing bindless descriptor set, holding per-shape `GpuRectStyle`/`GpuEllipseStyle` records (`crates/tre-engine/src/gpu_style.rs`) a vertex references by a numerically-encoded word index in one `UiVertex.params` slot -- `UiVertex`'s hard 32-byte layout has no room for non-uniform corner radii, a border, or corner smoothing otherwise. The existing texture array moves from binding 1 to binding 2 to keep the spec-required "`VARIABLE_DESCRIPTOR_COUNT` binding must be highest-numbered" invariant.

  2. Extend `Rectangle` rendering to real non-uniform corner radii, a real border, and corner smoothing, via a new `sdf_rect_styled.frag` shader/pipeline and `RenderingCanvas::draw_styled_rectangle` -- the original uniform-radius `draw_rounded_rect` path stays untouched for its existing callers.

  3. Build real `Circle`/`Ellipse` rendering (none existed before this step) via a new `sdf_ellipse.frag` shader/pipeline and `RenderingCanvas::draw_ellipse` -- border and a real, hard-edged `arc_length` angular sector cutoff both included.

  4. Build real `Polygon`/`Star` fill rendering: procedural boundary-point generation (`generate_polygon_points`) plus a from-center triangle fan (`fan_from_center`), routed through a new `RenderingCanvas::draw_flat_polygon` method and the existing (previously uncalled) `walking_skeleton` flat-color pipeline, now given `PipelineKind::FlatColor` and its first real `Canvas` caller.

  5. Build real, tested Bezier-flattening for `Path` (`shapes::flatten_path`, `flatten_cubic`/`flatten_quad`) -- the user's own explicit request this step. A small, self-contained duplicate of `tre_svg::flatten::flatten_cubic`/`flatten_quad`'s algorithm, not a shared dependency: `tre-svg` already depends on `tre-engine`, so the reverse dependency real code-sharing would need is circular.

  6. Build real hit-testing (`ShapeRegistry::hit_test`) for all four shape kinds -- the actual concrete need `hit_testable` (present since Step 10.1, read by nothing until now) exists for. Requires a new `tre_math::Affine2::invert` to map a world-space query point back into a shape's own local space.

* **Technical Rationale:** Step 10.1 shipped the shape-primitive data model and real rendering for exactly one narrow case; this step is what makes shapes actually usable as "the backbone of UI frameworks" (the user's own framing) -- real borders, real non-uniform corners, a real second shape kind (`Circle`/`Ellipse`), real polygon fill, and real hit-testing (routing clicks/hover to the right shape) are all things a UI framework needs immediately, not eventually. Reusing the bindless-array pattern already proven for textures (rather than growing `UiVertex`) keeps every pipeline that doesn't need per-shape style data exactly as fast as it already was.

* **Explicitly out of scope, disclosed not silently dropped:** `Polygon`/`Path` border/stroke rendering (no stroke tessellator built this step); `Path` fill rendering (needs a general, non-star-shaped triangulator this crate cannot reach without the circular-dependency problem task 5 names -- `flatten_path`'s own real output is available for a future step to build a renderer against); `FillStyle::Gradient`/`Texture` on any shape kind (a real, separate feature, deferred whole); non-`Normal` `BlendMode` (needs new RHI framebuffer-read capability, or fixed-function blend state for the subset that could use it -- neither built); rounded stroke caps on a partial-arc `Circle` (`arc_length < 360`, hard-edged cut only). See `documentation/ARCHITECTURE.md` Section 7.5's own "Implementation status" note for the full, itemized disposition of every field. **Both the `Polygon`/`Path` border/stroke gap and the `Path` fill gap named above were closed the same day** -- see the "Step 10.2 Follow-up" write-up immediately below.

#### Step 10.2: Full Shape Rendering Support -- Status: Complete (2026-09-09)

Real, in `crates/tre-engine/src/gpu_style.rs` (new), `crates/tre-engine/src/shapes.rs` (extended), `crates/tre-engine/src/lib.rs` (extended: `draw_styled_rectangle`/`draw_ellipse`/`draw_flat_polygon`, `PipelineKind::SdfRectStyled`/`SdfEllipse`/`FlatColor`, `RhiDevice::shape_style_buffer`), `crates/tre-rhi-vulkan/src/lib.rs` (extended: the binding-1 storage buffer, binding renumbering), two new shaders (`sdf_rect_styled.frag`, `sdf_ellipse.frag`), and `crates/tre-math/src/lib.rs` (new: `Affine2::invert`).

**A real GPU bug found and fixed during implementation, not assumed away.** The shape-style word index was originally encoded via `f32::from_bits` (a bit-cast) into the `UiVertex.params` float slot it travels through. For small word indices this produces a *subnormal* float -- and real GPU hardware (confirmed by actually running `shape_full_rendering_demo` against it, not assumed from documentation) silently flushed it to `0.0` somewhere between the vertex and fragment stage (denormal flush-to-zero, common, real ALU/interpolation behavior), so the shader read `style_index == 0` -- the wrong style record -- every time. Diagnosed by a real pixel scanline across the rendered circle (its visible radius was ~10px against a requested 50px, exactly matching what reading the *rectangle's* own style data as the circle's border/arc fields would produce), not guessed at. Fixed by carrying the index numerically (`as f32` / `uint(...)`) instead of by bit-cast -- every real word index is a small integer, exactly representable as a normal `f32`, with no denormal ever in play. `crates/tre-engine/src/gpu_style.rs`'s own `style_index_param` doc comment has the full account; REVIEW.md records it as a new finding.

**Two scope decisions worth naming, not silently made.** First, `sd_ellipse` (the ellipse SDF) uses a standard scaled-circle *approximation* (exact only when `radius.x == radius.y`), deliberately chosen over Inigo Quilez's exact closed-form quartic solve -- simpler to verify correct than hand-transcribing a quartic root solve from memory, at the cost of being an approximation for a true (non-circular) ellipse. Second, `corner_smoothing`'s superellipse blend (raising the corner falloff's norm from 2 toward 5 as smoothing -> 1) is a real, monotonic smoothing control but explicitly not a byte-for-byte match of any specific reference implementation's own squircle algorithm (e.g. Figma's).

**Verified.** `tre-engine`: 25 new tests (119 total, up from 94) -- `GpuRectStyle`/`GpuEllipseStyle` byte-layout round-trips, `style_index_param`'s numeric (not bit-cast) encoding, `draw_styled_rectangle`/`draw_ellipse`/`draw_flat_polygon`'s own vertex/style-record encoding, `generate_polygon_points`/`fan_from_center`, `flatten_cubic`/`flatten_quad`/`flatten_path`, and `hit_test` for all four shape kinds (including a rounded-corner exclusion test, an arc-wedge exclusion test, and a star-polygon concave-notch exclusion test). `tre-math`: 5 new `Affine2::invert` tests. A new real GPU demo, `shape_full_rendering_demo.rs` (`demo/phase10_step10_2/`), renders a non-uniform-corner bordered rectangle and a bordered 270-degree circle through `ShapeRegistry` and asserts real pixel correctness at 8 sample points (a sharp corner staying sharp, the opposite rounded corner genuinely excised by its own radius, border vs. fill color, and the circle's own excluded arc wedge) -- confirmed stable across repeated runs. `shape_registry_demo.rs` (Step 10.1's own byte-for-byte equivalence proof) still passes unchanged, confirming `draw_rounded_rect`'s trivial-case path is untouched. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the whole workspace.

#### Step 10.2 Follow-up: `lyon` Migration -- Real `Path` Fill/Stroke (2026-09-09)

* **Motivation.** REVIEW.md finding #163 (this step's own circular-dependency disclosure above) named the real blocker: `Path` fill needed a general triangulator, but the only one this workspace had (`tre_svg::triangulate`) lived in `tre-svg`, which already depends on `tre-engine` for `UiVertex` -- so `tre-engine` depending back on `tre-svg` would create a cycle Cargo refuses to compile. The user asked for this fixed for real, then, mid-implementation, pointed at [`lyon`](https://github.com/nical/lyon) (the current Rust ecosystem's de facto standard 2D tessellation library) as an alternative to the originally-planned shared-bridge-crate approach, and explicitly chose **full replacement**: migrate `tre-svg`'s own existing SVG tessellation pipeline to `lyon` too, retiring the hand-rolled ear-clipping triangulator (Step 3.3.1) and the stencil-and-cover GPU fallback technique (Step 3.3.3) entirely, so the whole project settles on one real tessellation backend instead of two.

* **The real fix for finding #163 turned out not to be a bridge crate at all.** Once both `tre-svg` and `tre-engine` each depend directly on the same external `lyon` crate, neither needs to reach into the other's tessellation code -- the cycle a bridge crate would have routed around simply doesn't need routing around. `tre-engine`'s `Cargo.toml` gained `lyon = "=1.0.19"` directly.

* **`tre-svg` side (full replacement of Steps 3.3.1 and 3.3.3's own hand-rolled work):** `triangulate.rs` (the ear-clipping triangulator, including its own three documented hard-won bug fixes) and `stencil.rs` (`fan_triangles`/`bounding_box`, the stencil-and-cover CPU-side support) are both deleted outright. A new `tessellate.rs` module exposes one function, `tessellate_fill(contours, fill_rule, rgba) -> Result<(Vec<UiVertex>, Vec<u32>), SvgError>`, backed by `lyon::tessellation::FillTessellator`'s real sweep-line algorithm -- which resolves self-intersecting contours and multi-contour compound shapes with real holes (via opposite winding under `NonZero`, or same winding under `EvenOdd`) directly, both cases the old ear-clipper could never handle at all (it only ever supported a single simple contour, rejecting everything else via the now-removed `SvgError::NotSimplePolygon`). `flatten.rs`'s `flatten_cubic`/`flatten_quad` become thin wrappers over `lyon_geom::{CubicBezierSegment, QuadraticBezierSegment}::flattened` (same public contract, confirmed via direct research against `lyon`'s real source that its `Flattened` iterator starts after the current point and ends exactly at the segment's own endpoint, matching the old hand-rolled algorithm's contract exactly). `tre_engine::FillRule` (used solely by the now-deleted `create_stencil_and_cover_pipelines`) and that Vulkan-side function itself (`crates/tre-rhi-vulkan/src/lib.rs`, ~204 lines) are both removed.

* **`tre-engine` side (the original ask, now unblocked): real `Path` fill and, as a natural low-marginal-cost extension, real `Path`/`Polygon` stroke.** `shapes.rs` gains `tessellate_fill`/`tessellate_stroke` (mirroring `tre-svg`'s own new module, backed by `lyon` directly) and `build_lyon_path`, plus conversions from the engine's own `LineJoin`/`LineCap` enums to `lyon::path`'s equivalents. `flatten_path` is split into the unchanged-signature public function and a new private `flatten_path_with_closed`, which tracks each subpath's own closedness via `PathCommand::Close` -- a real correctness fix found during this work: an explicitly-closed subpath needs a continuous stroke loop with no end caps, while an open subpath needs real caps at both ends, a distinction the original `flatten_path` discarded entirely. `flatten_polygon` gains a real border (previously `assert!(border_thickness == 0.0)`); a new `flatten_path_shape` replaces `ShapeRegistry::flatten_into`'s `Path` arm (previously `unimplemented!()`) with real fill and stroke, honoring `stroke_line_cap`/`stroke_line_join`.

* **A real test-assumption error, root-caused rather than papered over.** Two new tests asserting `frame.commands.len() == 2` (expecting separate draw commands for a shape's fill and its border) both failed with `actual == 1`. Diagnosed via a temporary isolated diagnostic test (added, confirmed `tessellate_stroke` itself produces correct geometry in isolation, then removed) before concluding the real explanation: Step 5.1.3's own pre-existing, correct batch-flattening logic in `Canvas::flatten()` merges adjacent `DrawGeometry` commands sharing the same pipeline/texture/clip bounds into one combined command -- intended, beneficial behavior (fewer GPU draw calls), not a bug introduced by this work. Both tests were fixed to instead compare `frame.vertices.len()` against a freshly-computed fill-only baseline (proving the border contributes real additional geometry) and assert every command uses the expected pipeline.

* **Verified.** `tre-svg`'s new `tessellate` module: 5 tests, including a real self-intersecting pentagram tessellating correctly (rejected outright by the old ear-clipper) and a real ring-with-a-hole via two contours. `tre-engine` gained matching fill/stroke tests plus real `ShapeRegistry::flatten_into` coverage for `Polygon` and `Path` borders. Every consumer of the retired API was found and updated: `svg_tessellation_demo.rs`, `svg_morph_demo.rs`, `text_shaping_demo.rs` (missed in the first sweep, caught only by a full `cargo build -p tre-rhi-vulkan --all-targets`), and `stencil_and_cover_demo.rs` (renamed to `self_intersecting_fill_demo.rs`, rewritten to prove the same textbook pentagram fill-rule disagreement via `tessellate_fill` directly, no stencil GPU technique needed). A new demo, `path_and_polygon_demo.rs` (`demo/phase10_step10_2_followup/`), proves the original ask end to end: a "donut" `Path` (two oppositely-wound subpaths -- a real compound shape with a hole) and a bordered hexagon `Polygon`, both rendered through `ShapeRegistry`, with real pixel assertions (ring fill, hole genuinely background, both shapes' own border colors). `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the whole workspace. See `documentation/REVIEW.md`'s closing note on finding #163 and its new finding documenting the full-replacement scope decision.

### Steps 10.2.1-10.2.6: Finishing Full Shape Rendering Support (Planned 2026-09-09)

Sub-numbered, not a renumbering -- these sit between the already-shipped
10.2 and the already-planned 10.3/10.4 below, the same dotted-decimal
convention Phase 3's own 3.3.1-3.3.3 already established. Full technical
plan, investigation findings, and scope decisions for each: `PLAN.md`
(archived per sub-step to `planning/archive/PLAN_PHASE10_STEP10_2_X.md` as
each lands). One-line summary per sub-step, kept current here and in the
TRE Build Tracker as each moves from PLANNED to DONE:

* **10.2.1 -- Gradient Fill (Linear + Radial).** Real `FillStyle::Gradient`
  rendering for all four shape kinds, evaluated in linear color space, via
  a new `GpuGradientStyle` style-buffer record and a new `fill_kind`
  dispatch shared with 10.2.2.
* **10.2.2 -- Texture Fill.** Real `FillStyle::Texture` rendering for all
  four shape kinds via the existing bindless texture array, extending
  10.2.1's `fill_kind` dispatch to a third branch.
* **10.2.3 -- Non-`Normal` Blend Modes.** Real `BlendMode` rendering
  (`Multiply`/`Screen`/`Overlay`/`SoftLight`/`ColorDodge`), primarily via
  `VK_EXT_blend_operation_advanced` (a real, researched correction to
  Step 10.2's own original assumption that this needs framebuffer-read
  support), with a disclosed, tested fallback to `Normal` if the real
  device lacks the extension.
* **10.2.4 -- SDF Fidelity: Exact Ellipse Distance Field & Corner-Smoothing
  Reconciliation.** Replaces `sd_ellipse`'s disclosed scaled-circle
  approximation with a verified-correct formula (researched from a real
  published source, not assumed), and resolves `corner_smoothing`'s
  unverified squircle-match disclosure one way or the other, honestly.
* **10.2.5 -- Rounded Stroke Caps on Partial-Arc Circles/Ellipses.** Real
  analytic SDF cap geometry at a partial arc's two cut edges, keeping the
  SDF pipeline's existing antialiasing (a real alternative -- routing
  through `lyon`'s own tessellated stroke caps -- was considered and
  rejected specifically because `draw_flat_polygon` has no antialiasing,
  and circles/rings are a highly AA-sensitive, common real UI element).
* **10.2.6 -- Zero-Allocation Live Verification for the Shape System.** A
  new demo wraps a real, mutating, mixed shape scene (exercising every
  feature landed by 10.2.1-10.2.5) in `tre_memory::RenderTickGuard`,
  closing ARCHITECTURE.md Section 7.5's disclosed "architecturally sound
  but not proven live" gap the same way `main_loop_demo` already proves
  its own claim.

#### Step 10.2.1: Gradient Fill (Linear + Radial) -- Status: Complete (2026-09-09)

Real, in `crates/tre-engine/src/gpu_style.rs` (new `GpuGradientStyle`
record + `GRADIENT_MAX_STOPS`/`GRADIENT_STYLE_WORDS`; `GpuRectStyle`/
`GpuEllipseStyle` extended with `fill_kind`/`gradient_word_index`),
`crates/tre-engine/src/shapes.rs` (new `GradientDef`/`GradientKind`/
`GradientStop`/`GradientError`, `ShapeRegistry::create_gradient`,
`build_gpu_gradient_style`/`write_gradient_style`, a shared
`resolve_style_fill` for `Rectangle`/`Circle` and `draw_polygon_fill` for
`Polygon`/`Path`), `crates/tre-engine/src/lib.rs` (`draw_styled_
rectangle`/`draw_ellipse` extended with `fill_kind`/`gradient_word_index`
parameters; new `draw_gradient_polygon`; new `PipelineKind::
GradientFill`), and a new shader, `gradient_fill.frag` (paired with the
existing `bindless_textured.vert`, not a new vertex shader).
`sdf_rect_styled.frag`/`sdf_ellipse.frag` each gained a duplicated
`eval_gradient` function (this codebase's shaders have no include
mechanism) reading the same `GpuGradientStyle` word layout.

**A real coordinate-space bug found and fixed by the demo's own first
real run, not assumed away.** `sdf_rect_styled.frag`/`sdf_ellipse.frag`'s
own `frag_uv` is CENTER-relative (an internal shader convention for
symmetric SDF math), but a `GradientDef`'s own points are authored in
`Rectangle`/`Circle`'s PUBLIC local space (bounding-box top-left at the
origin -- the same convention their own `corner_radius`/`border`
thinking already uses, per `flatten_circle`'s own established doc
comment). The demo's first real run caught this directly: a rectangle
gradient that should have shown a blended red-purple tone at one probe
instead showed pure, unmixed red -- diagnosed as the shader evaluating
`t` against the wrong origin, not a shader-math bug. Fixed by
`build_gpu_gradient_style` taking a `local_origin_offset` parameter
(`[half_width, half_height]` for `Rectangle`, `radius` for `Circle`,
`[0, 0]` for `Polygon`/`Path` -- whose own local space is already
center-relative by construction) and subtracting it from every point/
center before writing the record, so gradient authors keep thinking in
the same top-left-relative coordinates as every other shape property.

**Verified.** 11 new `tre-engine` tests (137 total, up from 126):
gradient validation (empty/too-many/out-of-range/out-of-order stops, a
non-positive radial radius), `GpuGradientStyle` byte-layout round-trips,
and `flatten_into` wiring for all three real code paths (`Rectangle`
forcing the styled pipeline even with no border/radii/smoothing, since
`draw_rounded_rect` has no fill-kind branch at all; `Circle`'s own
`fill_kind`/`gradient_word_index`; `Polygon` routing through
`PipelineKind::GradientFill` with the word index riding in
`texture_handle`). A new real GPU demo, `gradient_fill_demo.rs`
(`demo/phase10_step10_2_1/`), renders a bordered rectangle (linear,
red-to-blue), a bordered circle (radial, white-to-green), and a hexagon
(linear, red-to-green, via `GradientFill`) through `ShapeRegistry`, with
every probed pixel checked against an independent Rust reference
implementation of the exact same premultiplied, linear-space gradient
math (mirroring `translucent_flat_fill_demo.rs`'s own established
discipline) -- including proof that a gradient composes correctly with
an existing solid border. Every pre-existing demo re-run and confirmed
bit-for-bit unchanged. `cargo fmt`/`clippy -D warnings`/`build`/`test`
clean across the whole workspace.

#### Step 10.2.2: Texture Fill -- Status: Complete (2026-09-09)

Real, in `crates/tre-engine/src/gpu_style.rs` (new `StyleFill` struct
bundling `fill_kind`/`gradient_word_index`/`texture_index`; `GpuRectStyle`
extended to 10 words, `GpuEllipseStyle` to 7, each gaining `texture_index`),
`crates/tre-engine/src/shapes.rs` (`resolve_style_fill` returns `(Color,
StyleFill)` and now handles `FillStyle::Texture`; new `bounding_box_uvs`
helper for `Polygon`/`Path`; `draw_polygon_fill`'s `FillStyle::Texture`
arm wired), `crates/tre-engine/src/lib.rs` (`draw_styled_rectangle`/
`draw_ellipse` take one bundled `StyleFill` parameter instead of two
trailing `u32`s; new `draw_textured_polygon`), and `sdf_rect_styled.
frag`/`sdf_ellipse.frag` (each gained a `fill_kind == 2` texture branch,
declaring the same bindless sampler/texture-array bindings `bindless_
textured.frag` already uses -- no new descriptor infrastructure).
`Polygon`/`Path` need no new shader at all: they reuse the EXISTING
`PipelineKind::TexturedQuad`/`bindless_textured.frag` pipeline directly,
since sampling a texture is not new math the way gradient evaluation
was -- a real, disclosed departure from `PLAN.md`'s own original
suggestion (extending `GradientFill`'s shader with a texture branch),
made once the simpler reuse became obvious during implementation.

**A parameter-list refactor made along the way.** `draw_styled_
rectangle`/`draw_ellipse` were about to grow a fourth trailing
fill-selection parameter (`texture_index`, alongside Step 10.2.1's
`fill_kind`/`gradient_word_index`). Bundled all three into one new
`StyleFill` value instead (with a `StyleFill::SOLID` constant for the
common case), so the signature doesn't grow again the next time a fill
kind is added -- all 5 existing test call sites and both real callers
(`flatten_rectangle`/`flatten_circle`) updated to match.

**Verified.** 5 new `tre-engine` tests (143 total, up from 137):
`bounding_box_uvs`' own real normalization and degenerate (zero-extent)
fallback, and `flatten_into` wiring for all three real code paths
(`Rectangle` forcing the styled pipeline even with no border/radii/
smoothing; `Circle`'s own `fill_kind`/`texture_index`; `Polygon` routing
through `TexturedQuad` with real per-vertex UVs and no style-buffer
write at all). A new real GPU demo, `texture_fill_demo.rs`
(`demo/phase10_step10_2_2/`), renders a rectangle, a circle, a hexagon,
and a square path all filled with a real four-quadrant flag texture
(red/green/blue/yellow, pure 0/255 channel values for exact sRGB
round-tripping), each shape's own upper-left and lower-right quadrant
probed and confirmed against the texture's own known content -- proving
the UV mapping is correct for every shape kind and both code paths, not
just "some texture appeared." Every pre-existing demo re-run and
confirmed bit-for-bit unchanged, including `bindless_textures_demo.rs`
(confirming the shared `TexturedQuad` pipeline and bindless descriptor
bindings are untouched). `cargo fmt`/`clippy -D warnings`/`build`/`test`
clean across the whole workspace.

#### Step 10.2.3: Non-Normal Blend Modes -- Status: Complete (2026-09-09)

**A plan-invalidating finding, surfaced before any code was written.**
`PLAN.md`'s own primary path, `VK_EXT_blend_operation_advanced` (mapping
each `BlendMode` directly onto a hardware `VkBlendOp`), is NOT
implemented by RADV -- this project's own real dev GPU/driver (AMD
Radeon 890M, Mesa 26.2.2-arch3.2). Confirmed via direct `vulkaninfo`
inspection (the extension is absent from the device's advertised list)
and independently corroborated via Mesa's own release notes, ruling out
a one-off local misconfiguration. That path could never be exercised by
a real GPU demo on this project's own hardware -- a direct conflict with
this project's standing "real code, real GPU demos as the correctness
oracle" discipline. Presented to the user as a genuine three-way fork
(build the real `VK_KHR_dynamic_rendering_local_read` alternative, ship
only a capability-gated fallback, or build both); the user's first
response was a genuine clarifying question ("is there an external
package that would save time?" -- researched honestly: no relevant
blend-mode shader crate exists, and `vk-sync-fork` predates the new
`VK_IMAGE_LAYOUT_RENDERING_LOCAL_READ_KHR` layout and would be
inconsistent with this codebase's own hand-rolled-via-`ash` barrier
convention), then chose the real alternative explicitly: "use the
alternative, it sounds like the designed way to do it,
VK_KHR_dynamic_rendering_local_read."

**Real, in `crates/tre-rhi-vulkan/src/lib.rs`:**
- A real, disclosed capability query in `VulkanDevice::new` (mirroring
  `debug_validation_available`'s own pattern): `local_read_supported`,
  conditionally enabling the extension name and
  `vk::PhysicalDeviceDynamicRenderingLocalReadFeaturesKHR` in
  `device_create_info`'s `push_next` chain.
- A SEPARATE descriptor set (set 1, one `VK_DESCRIPTOR_TYPE_INPUT_
  ATTACHMENT` binding, fragment-stage-only), built once in `new` when
  supported (`BlendReadResources`) -- not folded into the existing
  bindless set 0, which would have required renumbering every other
  shader's bindings (`VARIABLE_DESCRIPTOR_COUNT` must stay the
  highest-numbered binding in a layout).
- `create_blend_pipeline_layout`/`create_blend_mode_pipeline`: a second
  pipeline layout (bindless set 0 + the new input-attachment set 1, same
  12-byte push-constant range) and a dedicated pipeline-creation
  function (near-duplicate of `create_pipeline`, matching `create_blur_
  pipeline`'s own existing "separate function, not a shared parameterized
  helper" precedent) with `blend_enable(false)` -- the shader computes
  the fully-composited color itself, so fixed-function hardware blending
  must stay off.
- `begin_frame`: the swapchain/headless color attachment lives in
  `VK_IMAGE_LAYOUT_RENDERING_LOCAL_READ_KHR` for the whole frame when
  `local_read_active` (device support AND, for `VulkanSwapchain`, a real
  per-surface `supported_usage_flags` check -- see `RhiSwapchain::
  supports_local_read_input_attachment` below), instead of the ordinary
  `COLOR_ATTACHMENT_OPTIMAL` -- a layout valid as a color attachment AND
  an input attachment simultaneously, so no extra transitions are needed
  between ordinary and blend-mode draws. The input-attachment descriptor
  is rewritten every frame (safe with no extra sync, since the fence
  wait at the top of `begin_frame` already guarantees the GPU is done
  with the prior frame's binding, under this device's single-frame-in-
  flight model).
- `insert_blend_read_barrier` (`VulkanCommandBuffer`): a real by-region
  barrier (`COLOR_ATTACHMENT_WRITE` -> `INPUT_ATTACHMENT_READ`) using
  this codebase's own already-established classic (non-`_2`)
  `vkCmdPipelineBarrier` API -- `INPUT_ATTACHMENT_READ`/`BY_REGION` are
  both core (non-sync2) enum values, so `VK_KHR_synchronization2` was
  never needed as a new dependency. Also binds set 1 against `self.
  pipeline_layout`, safe immediately after `set_pipeline` since
  `execute_frame` always calls both together for a `FlatColorBlend` draw.
  Called before EVERY such draw, not once per frame.
- `HeadlessSwapchain`'s color image gains `VK_IMAGE_USAGE_INPUT_
  ATTACHMENT_BIT` (a core, always-safe-to-declare flag on a manually
  allocated image). `VulkanSwapchain` cannot assume the same: a
  presentable surface's `imageUsage` must be a subset of that surface's
  own `VkSurfaceCapabilitiesKHR::supportedUsageFlags`, which is NOT
  spec-guaranteed to include `INPUT_ATTACHMENT` the way a manually
  allocated image's support always is -- queried for real and
  conditionally included, with the result exposed via a new `RhiSwapchain
  ::supports_local_read_input_attachment` trait method (`tre-engine/src/
  lib.rs`) that `begin_frame`/`submit_and_present` AND the layout the
  descriptor points at all require alongside the device-level query.
  This was a real gap the step's own first full demo-regression sweep
  caught: without it, every windowed demo would have broken (or, on a
  hypothetical surface lacking the flag, hit a validation error) purely
  from the device supporting the extension, regardless of whether that
  particular window's surface did.
- `resume_swapchain_rendering`: a second, distinct regression the same
  sweep caught. It hardcoded `COLOR_ATTACHMENT_OPTIMAL` when re-beginning
  rendering into the swapchain after a `PushLayer`/`PopLayer` redirect,
  but the swapchain image was actually left in `RENDERING_LOCAL_READ_KHR`
  by `begin_frame` -- `vkCmdBeginRendering` validation fails outright
  when the declared layout doesn't match the image's real one. Fixed by
  stashing the real layout `begin_frame` chose
  (`VulkanCommandBuffer::swapchain_color_layout`) and using it here too.

**Real, in `crates/tre-engine/src/lib.rs`/`shapes.rs`:**
`PipelineKind::FlatColorBlend`; `RhiDevice::local_read_blend_supported`/
`RhiCommandBuffer::insert_blend_read_barrier` trait methods;
`execute_frame`'s `DrawGeometry` arm special-cases `FlatColorBlend` to
call `insert_blend_read_barrier` before `draw_indexed`, mirroring
`PopLayer`'s own existing per-`PipelineKind` special-casing precedent;
`RenderingCanvas::draw_flat_polygon_blended` (mirrors `draw_flat_polygon`,
repurposing the existing `texture_handle`/push-constant slot to carry a
`blend_mode` value -- no growth of the shared `PushConstants` struct).
`shapes.rs`'s `draw_polygon_fill` gained a `blend_mode: BlendMode`
parameter: `FillStyle::Solid` dispatches to `draw_flat_polygon_blended`
only when `blend_mode != Normal` AND `device.local_read_blend_supported()`
-- unsupported hardware, or a plain `Normal` blend, falls back to the
existing `draw_flat_polygon` path, a real, disclosed degradation, never
a silent attempt to use resources that were never created. `Gradient`/
`Texture` fill arms are unchanged (blend mode not applied to them this
pass).

**Real, in `crates/tre-rhi-vulkan/shaders/flat_color_blend.frag`
(new, paired with the existing `walking_skeleton.vert`):** reads the
destination via `subpassInput`/`subpassLoad`, computes each of the five
W3C separable blend formulas (Multiply/Screen/Overlay/SoftLight/
ColorDodge) per channel in linear space, and writes the composited
result directly. Scoped to opaque source AND destination (both alphas
assumed 1) -- at full opacity the general W3C alpha-weighted compositing
formula collapses to `Co = B(Cb, Cs)` directly, so no unpremultiply/
premultiply step is needed; a shape drawn under active `Canvas` opacity
does not get that opacity correctly applied to a blend-mode fill in this
pass (disclosed follow-up work, not built speculatively here).

**Scope decisions, disclosed not accidental:** `Polygon`/`Path` solid
fill only (not `Rectangle`/`Circle`, not gradient/texture fill) -- one
shared pipeline with a runtime blend-mode branch (the same
`texture_index`/`gradient_word_index`-repurposing precedent), not one
pipeline per mode. Only correct against the swapchain/headless attachment
`begin_frame` sets up, not while a `PushLayer` render-to-texture target
is active (`begin_render_to_texture` never gets its own `RENDERING_
LOCAL_READ_KHR` layout or input-attachment descriptor write in this
pass).

**Verified.** 3 new `tre-engine` tests (146 total, up from 143):
`flatten_into`'s real fallback-to-`Normal`-on-unsupported-hardware
routing, real routing through `FlatColorBlend` when supported, and a
plain `Normal` blend mode still using `FlatColor` even on hardware that
supports the capability. A new real GPU demo, `blend_mode_demo.rs`
(`demo/phase10_step10_2_3/`), draws a background rectangle then six
polygon swatches (one per `BlendMode`, including `Normal` as a
routing-correctness control) on top of it; every swatch's own center
pixel is compared against an independent Rust reference implementation
of the exact same W3C blend formula -- on this real GPU, every channel
of every swatch matched the independent reference exactly or within 1 of
255 levels. Every pre-existing GPU demo re-run (including the four
`PushLayer`/`PopLayer` demos that surfaced the two real regressions
above, and the windowed-swapchain demos `walking_skeleton`/
`multi_window`/`input_demo`/`main_loop_demo`, run against this
machine's real X11 session rather than `xvfb-run`, unavailable locally)
and confirmed passing. `cargo fmt`/`clippy -D warnings`/`build`/`test`
clean across the whole workspace.

#### Step 10.2.4: SDF Fidelity: Exact Ellipse Distance Field & Corner-Smoothing Reconciliation -- Status: Complete (2026-09-09)

**Real research before any code, per this project's own standing
discipline.** Fetched Inigo Quilez's own published ellipse-distance
article (iquilezles.org/articles/ellipsedist) directly rather than
relying on memory: the exact point-to-ellipse distance is the root of a
quartic in general (no simpler true closed form exists), and IQ's own
article states the direct quartic solve is "both expensive and not very
stable" -- his real, published solution is a Newton-Raphson refinement
on the ellipse's implicit parametrization, 5 iterations by default. Two
variants exist (trigonometric, and a rotation-based variant tracking a
unit vector through a per-iteration rotation update instead of
re-deriving an angle via `atan`/`sin`/`cos` every step); implemented the
rotation-based one (fewer transcendental calls per iteration).

**Real, in `crates/tre-rhi-vulkan/shaders/sdf_ellipse.frag`:** `sd_ellipse`
replaced entirely -- the old "scaled circle" approximation
(`k1*(k1-1)/k2`, exact only when `r.x == r.y`) is gone, replaced by IQ's
real Newton-Raphson formula (with one small, disclosed numerical-
stability guard: `max(c*c-a*a, 0.0)` before the `sqrt`, since float
rounding can push that expression slightly negative right at
convergence, which IQ's own published code doesn't guard against).

**A real, measured finding that reframed the whole fix.** `tre-engine`'s
new `sdf_ellipse_fidelity` test module (independent Rust references:
the old formula, the new formula, and a fully independent brute-force
ground truth via dense parametric-boundary sampling) found that the old
approximation's fill/no-fill BOUNDARY was always exactly correct -- `k1
= length(p/r)` is exactly `1.0` everywhere ON the true ellipse boundary
by construction, so `k1*(k1-1)/k2` is always exactly `0` there,
regardless of angle. The real, practical defect was in the SDF's actual
MAGNITUDE away from the boundary (measured: 10.67px and 1.31px real
error at two off-axis points on a 3.5:1-eccentricity ellipse, vs.
essentially zero for the new formula) -- exactly the value
`border_thickness` rendering depends on (`inner_d = d + border_thickness`),
meaning a bordered, eccentric ellipse's border thickness would have
visibly varied around its own perimeter under the old formula, even
though its outer silhouette was always correct. This also explained why
`PLAN.md`'s own task language (expecting the old formula to show real
error at major/minor-axis reference points) didn't match what was
found there -- both formulas are exact on-axis for an exterior point (a
real, disclosed correction, `sdf_ellipse_exact_matches_analytic_
distance_on_the_major_and_minor_axes`'s own doc comment has the full
account, including a genuine ellipse-geometry subtlety found along the
way: an interior major-axis point can have its nearest boundary point be
a pair of symmetric OFF-axis points, not the vertex, whenever it sits
inside the ellipse's own evolute cusp).

**`corner_smoothing`'s own disclosed gap, resolved by research with NO
code change.** Fetched Figma's own blog post
(figma.com/blog/desperately-seeking-squircles) plus the real, widely-
cited open-source transcription of their algorithm
(github.com/tienphaw/figma-squircle) directly, rather than guessing:
Figma's construction is a real SVG path per corner -- two curvature-
continuous cubic Beziers plus a circular arc -- not an implicit
distance field at all. There is no simple closed form of THAT
construction to drop into `sdf_rect_styled.frag`'s own `corner_norm`
superellipse-exponent blend; computing an exact per-pixel distance to
an arbitrary Bezier curve is real, substantially harder, out-of-scope
work for this one bounded step. `corner_norm` is kept exactly as-is (a
real, legitimate, monotonic smoothing control), with a new `tre-engine`
test module (`corner_smoothing_fidelity`: an independent Rust
transcription of Figma's real corner-path-parameter algorithm, a real
SVG-arc endpoint-to-center solver, and a cubic Bezier evaluator)
quantifying the real, previously-unmeasured deviation: identical to
Figma's construction at `smoothing == 0` (both reduce to the same plain
circular-arc rounded corner), diverging up to ~71% of the corner radius
at `smoothing == 1.0` (confirmed to scale linearly with radius) -- a
real, visually significant difference disclosed precisely now, not left
as a vague "not verified" caveat.

**Verified.** 5 new `tre-engine` tests (151 total, up from 146): three
ellipse-fidelity tests (analytic axis-point exactness, brute-force
off-axis agreement for the new formula, brute-force off-axis error for
the old one) and two corner-smoothing-fidelity tests (exact match at
`smoothing == 0`, real quantified divergence at `smoothing == 0.5`/`1.0`).
A new real GPU demo, `ellipse_sdf_fidelity_demo.rs`
(`demo/phase10_step10_2_4/`) -- the first ever to draw a genuinely
non-circular ellipse (every prior demo's `radius.x == radius.y`, the one
case the old approximation already got right) -- draws a real,
eccentric, bordered ellipse and finds the real border/fill transition
pixel by bisecting on actual GPU-rendered color at four angles, each
confirmed against a second, independent CPU transcription of the exact
formula (sharing no code with either the shader or the unit tests).
Every pre-existing GPU demo re-run and confirmed passing, including the
three real consumers of the ellipse pipeline (`shape_full_rendering_
demo`, `gradient_fill_demo`, `texture_fill_demo`) at their own circular
cases. `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
whole workspace.

#### Step 10.2.5: Rounded Stroke Caps on Partial-Arc Circles/Ellipses -- Status: Complete (2026-09-09)

**Real, in `crates/tre-rhi-vulkan/shaders/sdf_ellipse.frag`:** a new
`cap_sdf(p, radius, angle, cap_radius)` function computes the signed
distance to a real circle of radius `border_thickness / 2`, centered on
the border band's own centerline (`border_thickness / 2` inward from
the ellipse boundary) at a given cut `angle` -- the standard 2D
"rounded line/capsule end" SDF technique, applied to an arc's own cut
angle instead of a straight segment's end. The cap center's radial
distance to the boundary at that angle (`boundary_t`) is a genuinely
EXACT closed form (the ellipse's own polar equation solved for `t`,
distinct from `sd_ellipse`'s own Newton refinement, which is needed
only for nearest-point distance from an arbitrary point, not a ray
from center at a known angle).

The main sector-cutoff branch now additionally unions in both caps via
`d = min(d, cap_sdf(...))` for each of the arc's two cut angles, gated
on `border_thickness > 0.0` (a borderless partial arc keeps its
existing flat cutoff -- there's no stroke to round the end of) --
`min()` only ever pulls `d` more negative ("more inside") near the two
cut points, so the plain swept interior and the rest of the excluded
wedge are both provably unaffected by construction, not just by
observation.

**One disclosed approximation remains, narrowed from Step 10.2.4's own
broader one:** the cap center is placed along the RADIAL direction at
each cut angle -- exact for a `Circle` (radial and local outward-normal
directions coincide there), a real, consistent approximation for a true
(non-uniform-radius) `Ellipse`, since the local normal generally
differs from radial there (the same distinction Step 10.2.4's own
ellipse-SDF research surfaced, REVIEW.md finding #173).

**Verified.** No CPU-side reference test is meaningful here (there is
no independent formula for "the correct antialiased 2D render of a
rounded cap" the way there was for `sd_ellipse`'s own distance value --
the real per-pixel shader behavior IS the thing being proven). Instead,
a new real GPU demo, `arc_rounded_cap_demo.rs`
(`demo/phase10_step10_2_5/`), draws a bordered quarter-circle arc and
samples real pixels a few degrees past each of the two cut angles at
the border band's own centerline radius: 4° past each cut (within the
cap's own bounded angular footprint) is confirmed border-colored --
proving the real rounding, not merely that the cutoff stopped
excluding anything -- while 25° past each cut is confirmed still
background, proving the rounding is real and BOUNDED. Every
pre-existing GPU demo re-run and confirmed passing, including
`shape_full_rendering_demo`'s own pre-existing 270°-sweep bordered
circle, which now visibly shows the same real rounded caps (its own
existing arc-wedge-exclusion assertion is unaffected, since the caps'
`min()` union never turns an excluded, far-from-both-caps fragment into
a filled one). `cargo fmt`/`clippy -D warnings`/`build`/`test` clean
across the whole workspace.

#### Step 10.2.6: Zero-Allocation Live Verification for the Shape System -- Status: Complete (2026-09-09)

**The sixth and final gap in Step 10.2's own original roadmap, closing
ARCHITECTURE.md Section 7.5's own disclosed gap.** `tre_memory::
RenderTickGuard`/`DebugAllocGuard` (Step 9.2) already proved `main_loop_
demo`'s own hand-drawn scene is genuinely zero-allocation, but no demo
had ever wrapped a `ShapeRegistry::flatten_into`-driven scene in that
same real, self-checking guard -- even though it reuses the exact same
already-proven `reset()`/`flatten_into` primitives underneath.

**Real, in `crates/tre-rhi-vulkan/examples/shape_registry_zero_alloc_
demo.rs`:** modeled directly on `main_loop_demo.rs`'s own established
warm-up-then-guard pattern (a single-threaded, headless simplification
-- no `SubCanvas` workers, no windowing/input/atlas, since this step is
about `ShapeRegistry`/`RenderingCanvas` specifically). One `root:
RenderingCanvas` stitches into one persistent `FrameArena`, exactly
like `main_loop_demo`'s own root canvas does alongside its worker
canvases -- a worker count of zero is not a new capability, just this
step's own new caller of the same reuse machinery. A real,
representative MIXED scene (not a strawman) covers every new fill/blend
feature Steps 10.2.1-10.2.5 added: a solid `Rectangle`, a gradient-
filled non-circular `Circle` (Step 10.2.4's exact ellipse SDF), a
texture-filled `Polygon`, a solid-fill `Polygon` under a non-`Normal`
`BlendMode` (Step 10.2.3), and a bordered partial-arc `Circle` (Step
10.2.5's rounded caps). 120 frames mutate real shape properties every
frame -- position and solid-fill color via `ShapeRegistry::get_mut`,
and a gradient's own stop colors via a new `ShapeRegistry::gradient_mut`
method (this step's own real, small API addition: no prior method let a
caller update an already-registered gradient's stops in place, and
`create_gradient` called fresh every frame would grow `self.gradients`
without bound, since `GradientId`'s own table has no generational
reuse) -- matching this project's own standing discipline of testing
what real, continuously-updating UI usage actually does.

**Two real, previously-undetected per-frame allocations found and
fixed** -- this step's own guard is the first to ever check `Polygon`/
texture-fill rendering under real allocation pressure:

- `generate_polygon_points`/`fan_from_center` (`crates/tre-engine/src/
  shapes.rs`) each returned a freshly heap-allocated `Vec` on every
  `Polygon` flatten. Fixed via new `generate_polygon_points_into`/
  `fan_from_center_into` siblings writing into two new persistent
  `ShapeRegistry`-owned scratch buffers (`polygon_points_scratch`/
  `polygon_triangles_scratch`) instead -- the original owned-`Vec`-
  returning `generate_polygon_points` is kept (still used by
  `hit_test_polygon`, not a per-frame guarded path); `fan_from_center`
  had no other real caller left, so it was removed outright rather than
  kept as dead code.
- `bounding_box_uvs` (Step 10.2.2's own texture-fill UV helper, shared
  by `Polygon`'s and `Path`'s `FillStyle::Texture` dispatch) did the
  same. Fixed via a new `bounding_box_uvs_into` sibling and a third
  scratch buffer (`polygon_uv_scratch`, shared by both shape kinds since
  `flatten_into`'s own loop processes one shape at a time). `draw_
  polygon_fill`'s signature grew an eighth parameter for this (a real,
  disclosed `#[allow(clippy::too_many_arguments)]`, not a design that
  needs a sub-struct -- every parameter is distinct, real per-call
  state, and both real callers already thread every one of them through
  from their own persistent scratch state).

**One real, deeper gap disclosed, not fixed.** `lyon`-backed
tessellation (`tessellate_fill`/`tessellate_stroke`) still constructs
fresh tessellator/path/`VertexBuffers` objects on every call -- used by
any `Path`'s own fill/stroke and any BORDERED `Polygon`. A substantially
larger reuse redesign than this step's own two fixes (would need
persistent, reusable `lyon` tessellator/buffer state threaded through
`ShapeRegistry`), the same category of deferred gap `main_loop_demo.rs`'s
own Step 9.2 already disclosed for RHI submission (`Box<dyn
RhiCommandBuffer>`) and `std::thread::scope` (`Arc<ScopeData>`). This
step's own demo scene deliberately uses only borderless shapes and no
`Path` so it never exercises this gap -- a bordered `Polygon`/`Path`
shape stays real and correct, just not yet zero-allocation.

**Verified.** 4 new `tre-engine` tests (`gradient_mut_mutates_an_
existing_gradients_stops_in_place`, `gradient_mut_returns_none_for_an_
out_of_range_id`, plus the `_into` siblings' own reuse/clearing tests
replacing the two retired `fan_from_center` tests and adding a new
`bounding_box_uvs_into_clears_any_prior_contents_before_refilling`
case). The new demo runs 120 real frames with zero heap allocations
after warm-up, and writes its final frame for visual disclosure. Every
pre-existing GPU demo re-run and confirmed passing, including `path_
and_polygon_demo` (a bordered `Polygon`/`Path` scene, proving this
step's fixes didn't change real behavior for the lyon-tessellated path
they deliberately don't touch) and `texture_fill_demo` (all four shape
kinds' own UV mapping, proving `bounding_box_uvs_into`'s real output is
unchanged from `bounding_box_uvs`'s own). `cargo fmt`/`clippy -D
warnings`/`build`/`test` clean across the whole workspace. **All six of
Step 10.2's own disclosed gaps (Steps 10.2.1-10.2.6) are now closed.**

### Step 10.3: The `tre-ffi` C-ABI Crate (for C, C++, and other non-Python bindings)

* **Renumbered 2026-09-09** from Step 10.2 to 10.3, when Step 10.2 was inserted ahead of it for full shape rendering support (shapes are what this boundary and Step 10.4's Python binding will actually expose -- finishing real rendering for all four primitives first avoids binding an API surface still mostly stubbed).

* **Revised 2026-09-09:** this step's own original rationale described `tre-ffi` as the boundary *every* language binding, Python included, would use. Confirmed with the project owner: Python now binds directly via PyO3 instead (Step 10.4, rewritten below) -- `tre-ffi` remains real, complete, and necessary, but its own audience is now every language *other* than Python. See DESIGN.md Section 2.7's "Cross-Language Boundary: Two Real Paths" and TECHNICAL.md Section 9.4.1 for the corrected, canonical account.

* **Implementation Tasks:**

  1. Define the complete public surface of the engine as `#[repr(C)]` opaque handles and `extern "C"` functions in a dedicated `tre-ffi` crate, per the ABI shape rules in TECHNICAL.md Section 9.4.1 -- every other crate in the workspace is still linked into the shipped `cdylib`/`staticlib` (Section 9.2), but none of them export their own `extern "C"` symbols; `tre-ffi` is the sole exporter. Step 10.1/10.2's `ShapeId` is exactly this pattern's own first real consumer for the languages that use it: an opaque handle plus `extern "C"` getter/setter functions, never raw struct-layout access across the boundary.

  2. Wrap every exported function body in `std::panic::catch_unwind`, translating any caught panic into the corresponding `EngineError` result code (DESIGN.md Section 2.6) rather than allowing it to unwind across the boundary.

  3. Build both `cdylib` (for any future non-Python dynamic-language binding) and `staticlib` (for a C++ host linking the engine directly) output targets from the same `tre-ffi` crate, demonstrating that the boundary is not specific to any one non-Python language.

  4. Build a real, dedicated non-Python test harness (a small C program linking the `staticlib`, or an equivalent) exercising `tre-ffi`'s own entry points directly -- Step 10.4's Python test suite no longer covers this boundary at all (it binds elsewhere), so `tre-ffi` needs its own real coverage, not an assumption that Python's own tests happen to also exercise it.

* **Technical Rationale:** Concentrating the entire non-Python FFI surface in one crate makes that language boundary auditable in a single place. This step no longer needs to prove the engine is "UI-framework-language-agnostic" in the unqualified sense the original rationale claimed -- Python itself is now a disclosed, privileged exception (Step 10.4) -- but `tre-ffi` still proves the engine's *core* makes no Python-specific assumption: any other language can reach the identical underlying functionality through this one stable boundary.

### Step 10.4: Python UI Framework Bindings (direct PyO3, not through `tre-ffi`)

* **Renumbered 2026-09-09** from Step 10.3 to 10.4, same reason as Step 10.3's own renumbering note above.

* **Revised 2026-09-09:** confirmed with the project owner -- the Python UI framework binds directly to `tre-engine`'s native Rust API via PyO3, in a new, dedicated `tre-python` crate that depends on `tre-engine` directly and does not depend on `tre-ffi` at all. This is a genuine, disclosed departure from this step's own original design (routing through `tre-ffi`, "so the Python bindings exercise the identical boundary any other language would use"), made for real, measured performance reasons: a `tre-ffi`-routed binding pays a double marshalling cost on every call (native Rust type -> C-compatible shadow type -> PyO3 conversion back to a Python object) and opaque-handle indirection for high-frequency calls (e.g., a Step 10.1/10.2 shape's own per-frame property mutation) that a direct binding has no reason to pay. See DESIGN.md Section 2.7's "Cross-Language Boundary: Two Real Paths" for the full disclosure -- Python no longer receives "no privileged access beyond any other language," and that change is stated there plainly, not left implicit.

* **Implementation Tasks:**

  1. Generate the Python extension module via [PyO3](https://pyo3.rs/) in a new `tre-python` crate, wrapping `tre-engine` (and, once built, Step 10.1/10.2's shape-primitive layer) types directly with `#[pyclass]`/`#[pymethods]` -- not `tre-ffi`'s C-ABI, and not calling into `tre-engine` internals through any intermediate shadow-type layer at all.

  2. Provide Pythonic ergonomics at this same binding layer: context managers for `Canvas.save()`/`restore()` scope pairs, Python exceptions raised via a `From<EngineError> for PyErr` impl (PyO3's own standard mechanism), and buffer-protocol views over headless frame readback buffers (DESIGN.md Section 4.3) to avoid an extra copy into Python. Step 10.1/10.2's shape primitives get a Pythonic `Rectangle`/`Circle`/`Polygon`/`Path` class each, each `#[pyclass]` wrapping its own `ShapeId` directly -- no opaque C handle in between.

  3. Release the GIL (`Python::allow_threads`) around any engine call that can block on a GPU fence (e.g., `RhiDevice::begin_frame`), so the Python UI framework's own threads are not serialized behind engine waits. Rely on PyO3's own built-in panic-to-exception conversion at the generated call boundary (TECHNICAL.md Section 9.4.2) rather than writing a `catch_unwind` wrapper of this crate's own -- there is no hand-written `extern "C"` entry point on this path for one to wrap.

  4. Add the Python-binding test suite as a required CI job (TECHNICAL.md Section 9.2), exercising the correctness suite (Phase 9, Step 9.1) through Python against `tre-python`/`tre-engine` directly -- a separate, still-required gate from `tre-ffi`'s own new dedicated test harness (Step 10.3 task 4), not a shared one, since the two boundaries no longer share entry points.

* **Technical Rationale:** A first-party binding for the project's own UI framework has no reason to pay a C-ABI marshalling tax that exists specifically to serve languages that have no other way to call into Rust. PyO3 lets Python bind to native Rust types and methods directly, with Rust's own ownership/`Drop` semantics integrating into CPython's reference counting automatically -- real, measurable per-call savings for a boundary this project's own UI framework crosses constantly, at the honestly-disclosed cost of no longer treating Python as "just another C-ABI consumer." `tre-ffi` (Step 10.3) remains the correct, complete, real integration point for every language that does not get this privileged first-party treatment.

* **Implementation status (Phase 10 Step 10.4, 2026-09-10): a real, working, deliberately bounded first slice.** Investigated the project's own real Python UI framework (`pySilver`, a separate, already-shipping v1.7 Beta package) before writing any binding code, at the project owner's direction ("work with pySilver project for this if you need"); found its actual rendering contract (a single instanced draw call over a flat `numpy` structured array of 144-byte GPU instances, 7 primitive kinds, its own WGSL SDF shader) is architecturally incompatible with `tre-engine`'s multi-pipeline `Canvas`/`ShapeRegistry` design, and that `pySilver`'s own `ARCHITECTURE.md` has zero TRE-integration detail despite its README's stated migration intent. Surfaced this honestly rather than guessing; the project owner's explicit choice was the generic PyO3 wrapper around `tre-engine`'s own existing API (this step's original plan above), not an attempt to match `pySilver`'s own instance-buffer format.

  Built: a new `tre-python` crate (`shapes.rs`: `Rectangle`/`Circle`/`Polygon`/`Path` + `ShapeRegistry`; `canvas.rs`: minimal `Canvas`; `renderer.rs`: `HeadlessRenderer`; `error.rs`: `TreError`; a module-level `rgba8()` function). Task-by-task account against the four tasks above:

  1. **Done for solid fill only.** `FillStyle::Gradient`/`Texture` are real in `tre-engine` but not yet exposed to Python -- a disclosed, bounded follow-up, matching this project's own precedent of shipping one fill kind before the next (Step 10.2.1/10.2.2). Every shape's `common.transform` beyond position and `blend_mode`/`visibility` also stay at Rust-side defaults; `opacity` is the one extra `PrimitiveCommon` field exposed, since it costs nothing and a real caller reaches for it immediately.
  2. **Partially done.** `TreError` (real `EngineError` -> Python exception via `create_exception!`) and `Canvas.save()`/`restore()` as a context manager are both real. The buffer-protocol zero-copy view over the readback buffer was **deliberately not built** -- a real, disclosed scope/safety trade-off: a custom `#[pyclass]` buffer-protocol type needs `unsafe` `__getbuffer__`/`__releasebuffer__` FFI code, which would have expanded the `unsafe`-permitted closed set (TECHNICAL.md Section 9.1) beyond the one already-justified probe-surface teardown call. `HeadlessRenderer::render()` instead returns a real `bytes` object (one necessary `Vec<u8>` -> `PyBytes` copy) -- `numpy.frombuffer`/`bytes` both wrap it with no *further* copy, which covers the common case at a smaller `unsafe` footprint.
  3. **Done.** The real GPU round trip (vertex/index upload -> `execute_frame` -> submit/present -> readback) runs inside `Python::detach` (PyO3 0.27's replacement for the plan's `allow_threads`, deprecated in this pinned version), releasing the GIL for the whole blocking span. This required a real, necessary fix upstream in `tre-engine`: `RhiPipelineState` gained a `Send + Sync` supertrait bound -- without it, `Box<dyn RhiPipelineState>` inside `PipelineRegistry` blocked `PyHeadlessRenderer` itself from being `Send`/`Sync`, which blocks `Python::detach` from being usable at all. The fix was the trait bound, not `#[pyclass(unsendable)]`, which would have silently defeated this task's own requirement.
  4. **Not done.** No CI job wired yet; `demo/phase10_step10_4/demo.py` is the real local correctness-oracle proof this pass produced instead (see below), matching every prior phase's own "real demo before CI automation" sequencing.

  Two more real, previously-undetected problems, found only by actually running the binding end to end (not by compiling it):

  * **A real segfault at Python interpreter shutdown**, root-caused to a classic Rust footgun: struct fields drop in *declaration* order, the opposite of local variables, which drop in *reverse* declaration order. Every RHI *example*'s local-variable ordering gets safe Vulkan teardown "for free"; `PyHeadlessRenderer`'s struct originally declared `device` before `swapchain`/`pipelines`, so the logical device was destroyed first, while `HeadlessSwapchain`/`PipelineRegistry` still held live handles built from it -- a real use-after-free. Fixed by reordering the fields so `device` drops last.
  * **`PyCircle`'s `x`/`y` semantics were undocumented and got them wrong on first use**: `tre_engine::Circle`'s position field is the bounding-box top-left (matching `Rectangle`'s own convention), not the center, per `flatten_circle`'s own doc comment in `tre-engine/src/shapes.rs`. The demo script's first draft assumed "center" and rendered the wrong pixel color at the expected coordinate, caught by asserting an *exact* pixel value rather than only "differs from background." Fixed by correcting the demo's math and adding this convention explicitly to `PyCircle`'s own doc comment so no future caller has to rediscover it the same way.

  Also closed a real Python-side usability gap: `tre_engine::rgba8(r, g, b, a) -> u32` (`u32::from_le_bytes([r, g, b, a])`) had no Python equivalent, so a caller would otherwise need to replicate that exact little-endian packing by hand. Exposed as a module-level `tre_python.rgba8()` function.

  **Verified:** `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo build --workspace --all-targets` / `cargo test --workspace` all clean, in both debug and `--release` profiles (the release build is what originally surfaced a real, previously-undetected `unused_mut` warning in `tre-rhi-vulkan`, unrelated to this step but fixed alongside it -- see REVIEW.md). `demo/phase10_step10_4/demo.py`, run against real GPU hardware via `maturin develop --release`, builds a `ShapeRegistry` with a red `Rectangle`, a green `Circle`, and a blue `Polygon`, renders via `HeadlessRenderer`, and asserts *exact* expected BGRA8 bytes at each shape's own center pixel (pure-primary colors were chosen deliberately: 0 and 255 are the two fixed points of the sRGB transfer function, so their readback bytes are exact regardless of the pipeline's internal linear/sRGB handling, unlike a partial-value color would be) -- confirming the full real path (Python construction -> Rust flatten -> real Vulkan draw -> readback -> Python bytes) end to end, not just that it compiles.

* **`/review-project` follow-up pass (2026-09-10):** a four-lens multi-agent review run immediately after this step, at the user's own explicit instruction, found and fixed a critical defect this step's own single-call demo had not exercised: `HeadlessRenderer.render()` called a second time on an unmutated registry produced an empty frame (a real Vulkan validation error in debug, a real segfault in release), because `ShapeRegistry::flatten_into`'s incremental-dirty semantics mean every shape stays skipped once already-flattened, and `render()` built a fresh, empty canvas on every call. Fixed via a new `ShapeRegistry::mark_all_dirty()` primitive, called internally before every flatten. Also fixed: unchecked `HeadlessRenderer` width/height and `Polygon` side-count reaching raw Vulkan/engine calls with no validation (both now raise `ValueError`), and `render()`'s receiver widened from `&self` to `&mut self` so PyO3's own borrow check prevents two threads from racing on the same single-buffered GPU state. See REVIEW.md's "`/review-project` Pass on `tre-python`" section (findings #196-199) for full detail.

* **Full-project review follow-up (2026-09-10):** a second review pass across the whole workspace found `render()` was re-tessellating the full scene and performing two fresh, unpooled `vkCreateBuffer`/`vkAllocateMemory` calls on every single call (REVIEW.md #203), and separately that `renderer.rs` bypassed the `RhiDevice` trait abstraction via a Vulkan-specific inherent method (#204). Fixed both together, at the user's own explicit direction ("Option B"): `PyHeadlessRenderer` now owns one persistent `RhiDynamicRingBuffer` (the same pattern `main_loop_demo.rs` already uses), reused across every `render()` call instead of allocating fresh buffers -- which, since `create_dynamic_ring_buffer` is a real `RhiDevice` trait method, also removed `tre-python`'s direct `ash` dependency entirely. Required widening `RhiBuffer: Send + Sync` at the base trait (not just `RhiDynamicRingBuffer`), the same category of fix as #192/#199. See REVIEW.md's "Finding #203 Fixed" section for the full account, including the real animation-loop and starvation-handling verification.

## Phase 11: Native Windowing Backend Migration (Added 2026-09-10)

### Step 11.1: Migrate `tre-platform` to a `winit`-Backed Implementation

**Status: Complete (2026-09-10).** A follow-up investigation into whether `tre-platform` (Phase 1 Steps 1.1/1.2) exposed a full window-chrome/lifecycle interface -- resize, position/movement, title bar with close/minimize/maximize, icon -- found real gaps (no post-creation title change, no minimize/maximize, no icon support anywhere in either hand-rolled backend) plus one concrete leftover bug (Wayland's `app_id` hardcoded to `"tre-walking-skeleton"`, a Phase-0 walking-skeleton artifact, regardless of the real window title). Position/movement was confirmed to be a genuine Wayland-protocol-level restriction no library can lift (already disclosed at Step 1.1, line ~97 above). The project owner's conclusion: a hand-rolled protocol integration is not the right long-term investment for robustness/correctness; `winit` -- the de facto standard, actively maintained Rust windowing crate -- should replace it.

Scope decision (confirmed with the project owner): this step is a **bounded backend swap, not a capability expansion**. `PlatformConnection`'s public API (`new`/`new_wayland`/`new_x11`, `create_window`, `poll_events`, `scale_factor`, `window_handle`, `HasDisplayHandle`) is preserved exactly, so none of the 40 pre-existing demo files in `crates/tre-rhi-vulkan/examples/` needed to change. Exposing winit's newly-available capabilities (`set_title`, minimize/maximize, icon) is explicitly deferred to a future step.

The hardest design question -- whether `create_window()` could stay a synchronous, imperative, on-demand method despite winit 0.30's callback-driven `ApplicationHandler`/`ActiveEventLoop` ownership model -- was resolved by reading winit 0.30.13's actual source (`~/.cargo/registry`), not assumed: `ActiveEventLoop` (required to create a `Window`) is only reachable inside an `ApplicationHandler` callback. Passing `timeout: Some(Duration::ZERO)` to `EventLoopExtPumpEvents::pump_app_events` was traced through both the X11 and Wayland `platform_impl` backends and confirmed to make `ApplicationHandler::new_events` run on every single pump call (not just `resumed`, which fires exactly once, on the very first pump). `create_window()` therefore stages the request and immediately calls `pump_app_events(Some(Duration::ZERO), ..)` itself, which drains it inside `new_events` before `create_window()` returns -- fully synchronous from the caller's perspective, with zero API-visible asynchrony.

Implemented: `crates/tre-platform/src/winit_backend.rs`, a single `WinitConnection` type replacing both the old `wayland.rs` (Wayland, `wayland-client`/`wayland-protocols`) and `x11.rs` (X11, `x11rb` XCB FFI) files -- winit unifies both backends behind one set of types, so there is no longer a reason to hand-maintain two protocol integrations. `PlatformConnection::Wayland`/`X11` both wrap this same `WinitConnection`, built via `winit`'s own `EventLoopBuilderExtWayland::with_wayland`/`EventLoopBuilderExtX11::with_x11`. `WindowId` allocation keeps the previous scheme exactly (an internal monotonic `u64` counter, independent of winit's own `WindowId`, since nothing outside `tre-platform` ever constructs a `WindowId` directly -- grep-confirmed workspace-wide before this step began). `WindowEvent`s translate to `tre_engine::InputEvent` 1:1 (`CloseRequested`, `Resized`, `CursorMoved`->`PointerMoved`, `MouseInput`->`PointerButton`, `KeyboardInput`->`KeyboardKey`), still routed through the same `tre_engine::InputEventQueue` (pointer-move coalescing unchanged). `key_code` now comes from `winit::platform::scancode::PhysicalKeyExtScancode::to_scancode()`, confirmed (via winit's own doc comment) to produce the exact same "Linux evdev keycode" numbering `InputEvent::KeyboardKey`'s own doc comment already contracted -- a like-for-like replacement, not a new semantic.

A real, incidental hardening became possible and was taken: `tre-platform` no longer contains any `unsafe` code at all (winit's `Window`/`EventLoop` implement `raw-window-handle` 0.6's traits directly, so no `RawWindowHandle`/`RawDisplayHandle` is ever hand-constructed here anymore), so the crate now carries `#![forbid(unsafe_code)]` and is removed from TECHNICAL.md Section 9.1's closed set of crates permitted to contain `unsafe` (now three: the RHI backend crates, the ring-buffer/arena allocators, and `tre-ffi`).

Two real, deliberate disclosures, not silent changes:

* `PlatformConnection::scale_factor`'s signature was initially kept at `-> i32` for zero API churn, even though winit supplies a real per-window `f64` (Wayland `wp-fractional-scale` falling back to integer scale; X11 `Xft.dpi`/RandR) in place of the previous connection-wide, last-seen-wins Wayland value and the hardcoded X11 `1`. **Fixed same-day, at the project owner's explicit follow-up direction (REVIEW.md finding #178):** both `PlatformConnection::scale_factor` and `WinitConnection::scale_factor` were widened to `-> f64`, returning winit's own value directly. A workspace-wide grep before making the change confirmed the only real caller anywhere was `tre-platform/examples/smoke_test.rs`'s own diagnostic `eprintln!` -- none of the 40 `tre-rhi-vulkan` demos call this method, so the "future, deliberately API-breaking step" originally anticipated here turned out to touch one real call site, not forty.
* Winit permits constructing an `EventLoop` only once per process, ever (`static EVENT_LOOP_CREATED: AtomicBool`, confirmed never reset on Linux) -- a real, permanent restriction the previous backends did not have. Confirmed to be a non-issue for every current call site (all 40 demos plus `smoke_test.rs` are separate `fn main()` binaries, each building exactly one `PlatformConnection`), but documented here so a future in-process multi-connection test doesn't discover it the hard way.

The Wayland `app_id` bug disappeared as a side effect of the migration (winit's `WindowAttributes` has no equivalent latent hardcoded-string bug) -- an incidental fix, not a claimed deliberate one.

**Verified end to end:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean (zero failures across the whole workspace). `crates/tre-platform/examples/smoke_test.rs` run against a real Wayland session and, forced via `TRE_FORCE_BACKEND=x11`, against XWayland -- both created a real window and received real `Resized`/`PointerMoved` events. All 41 `tre_platform`-dependent demos re-run on real GPU hardware (real Vulkan device, `VK_LAYER_KHRONOS_validation` enabled); `main_loop_demo.rs` (the project's own reference imperative main loop) completed 90 real frames with its animation and Step 9.2 zero-allocation guard both verified exactly as before the migration (note: `poll_events()` runs before that guard's span begins, so this does not itself prove winit's own per-call allocation profile -- an honest scope note, not an overclaim); `multi_window.rs` created two real windows on one `PlatformConnection` and rendered both for 120 frames, confirming the multi-window routing pattern this whole design was built to preserve. See `demo/phase11_step11_1/README.md` for the full verification account.

## Phase 12: Complete `tre-python` as a Real GUI-Framework Backend (Added 2026-09-10)

Phase 10 Step 10.4 shipped a deliberately narrow first slice of `tre-python`: solid-fill `Rectangle`/`Circle`/`Polygon`/`Path` shapes rendered headlessly, with `Canvas` a near-stub. That was the right scope for that step, but not enough for `pySilver` (the project's own separate, already-shipping Python UI framework) to build on. The project owner confirmed the final direction: `pySilver` will rewrite its own rendering code around whatever `tre-python` exposes -- this phase does not attempt to match `pySilver`'s current instance-buffer format at all -- and gave five explicit, numbered requirements: (1) complete `pySilver` support, (2) the best possible native PyO3 binding on `tre-engine`'s own terms, (3) a `Canvas` redesigned around what `tre-engine` actually does well rather than a 1:1 `RenderingCanvas` port, (4) extremely crisp, fast text plus gradient/texture fill, (5) real input events -- "without input events we do not have a GUI framework, we just have a pretty renderer." This phase is organized into five sections mirroring those five points; the plan (design rationale, real-API research, and the full section breakdown) is preserved in this project's own plan-mode history.

### Step 12.1: Real Input Events + Windowed Rendering -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Built: `crates/tre-python/src/input.rs` (new -- `WindowId`/`MouseButton`/`ElementState`/`InputEvent` bound directly as PyO3 types, `InputEvent` as a real "complex enum," each variant its own distinct Python class under `tre.InputEvent`, matching current PyO3 0.27 idiom) and `crates/tre-python/src/windowed_renderer.rs` (new -- `WindowedRenderer`, wrapping `tre_platform::PlatformConnection` + `tre_rhi_vulkan::VulkanSwapchain`, following `multi_window.rs`'s own proven window/surface/swapchain sequence). `renderer.rs` had four items (`setup_err`, `RenderError`/`render_err`, `RING_BUFFER_CAPACITY`, `validate_dimensions`) widened to `pub(crate)` so `windowed_renderer.rs` could reuse them rather than duplicating them.

Two real constraints, found only by actually building this (not assumed from the design):

* **PyO3 0.27's "complex enum" support rejects a true unit variant mixed with a data-carrying variant in the same enum** ("Unit variant is not yet supported in a complex enum"). `PyMouseButton`'s `Left`/`Right`/`Middle` (plain unit variants) sit alongside `Other(u16)` (a data variant), which made the enum "complex" and broke compilation. Fixed by making all three empty-tuple variants (`Left()`, not bare `Left`) -- a real, disclosed shape change to the not-yet-shipped binding (`tre.MouseButton.Left()`, not `tre.MouseButton.Left`).
* **`PyWindowedRenderer` cannot be `Send`.** It owns a real `tre_platform::PlatformConnection`, which on Linux wraps winit's X11/Wayland backends -- raw `Rc`/`RefCell`/FFI handles (an X11 IME pointer, a Wayland `calloop` event-loop `Rc`, several `xkbcommon` `NonNull` pointers) that are not `Send`, because the underlying platform APIs themselves are only safe to touch from the thread that created the connection -- a real OS-level constraint, not a bug to work around. Fixed with `#[pyclass(unsendable)]`, which makes PyO3 enforce that same single-thread rule at the Python boundary instead of falsely requiring thread-safety. This interacted with `render()`'s own GIL-released GPU submission: the `Python::detach` closure could not touch `self.connection` at all (a non-`Send` capture fails `Python::detach`'s own `Ungil` bound), so `render()` was structured to borrow only `device`/`ring_buffer`/the target window's `swapchain`+`pipelines` inside the detached closure, doing the real resize-recovery swapchain recreation (which does need `connection`) back on the GIL-holding thread between `detach` calls rather than inside one.

`WindowedRenderer` supports real multi-window use from day one (sharing one `VulkanDevice`, the same pattern `multi_window.rs` already proves), real resize recovery (`EngineError::SwapchainOutOfDate` triggers one retry against a freshly recreated swapchain at the window's last-known size -- a real fix for REVIEW.md finding #116's previously-undisclosed gap: every existing Rust demo's own `.expect()` on submit/present would have panicked on a mid-run resize), and the same real window-lifecycle methods Phase 11 Step 11.1 already proved (`set_title`/`set_minimized`/`is_minimized`/`set_maximized`/`is_maximized`/`set_icon`).

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. In `--release`, two pre-existing, unrelated `tre-engine` tests fail (`gpu_style::style_index_param_rejects_a_misaligned_offset_in_debug_builds`, `flatten_into_still_enforces_the_balance_assertion_on_the_reused_path`) -- both rely on a `debug_assert!` that is compiled out in release, a real, disclosed gap that predates this step (confirmed via `git status` showing zero uncommitted changes to `tre-engine`) and is outside this step's own file list; flagged separately rather than fixed here as out-of-scope for this section. `demo/phase12_step12_1/demo.py`, run via `maturin develop --release` against a real live Wayland session (`DISPLAY`/`WAYLAND_DISPLAY` both set) with real GPU hardware: created a real on-screen window, received a real `Resized` `InputEvent` back through `poll_events()`, rendered 120 real frames (a red rectangle, green circle, and blue hexagon) directly to the window's own swapchain with zero byte readback, exercised every window-lifecycle method without error, then created a second real window sharing the same `VulkanDevice`, rendered into it, and closed it -- the full real path (Python construction -> real compositor window -> real polled input -> real Vulkan submit/present, repeated across two independent windows) confirmed end to end, not just compiled.

### Step 12.2: `Text` as a First-Class `ShapePrimitive` -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Built: a new `crates/tre-engine/src/text.rs` module -- `FontId`/`FontRegistry` (owns real, caller-loaded font bytes, hands back fresh, cheap `skrifa::FontRef`/`rustybuzz::Face` views on demand -- both are real table-directory parses, not full font parses, so no self-referential struct is needed), `Text` (a new `ShapePrimitive` variant: retained-mode, dirty-tracked, mutable in place, unlike `RenderingCanvas::draw_text`'s own immediate-mode-only path), and `TextFlattenContext` (bundles the font table with the same `GlyphAtlasContext` `draw_text` already requires). `ShapeRegistry::flatten_into` gained a new `text_context: Option<&TextFlattenContext<'_>>` parameter -- `None` for the 40+ pre-existing Rust examples that never insert a `Text` shape (a one-line, disclosed, mechanical update at every real call site), `Some` required once a registry holds a live `Text` shape (enforced by a real panic, matching the existing stale-`GradientId` panic precedent). `rustybuzz` moved from `[dev-dependencies]` to `[dependencies]` in `tre-engine`'s own `Cargo.toml`, since shaping is now real, non-test code.

**Solid fill only, this pass** -- a real, disclosed scope boundary, not an oversight: `RenderingCanvas::draw_text`'s own glyph-quad path takes a flat `rgba: u32`, not a `FillStyle`; real gradient/texture-filled text needs new per-glyph UV-mapping work in `draw_text`/`emit_glyph_quad` themselves, deferred to a future step, matching this project's own established "solid fill first" sequencing for `Rectangle`/`Circle`/`Polygon`/`Path` (Phase 10 Steps 10.2/10.2.1/10.2.2). `ShapeRegistry::hit_test` reports `Text` shapes as never hit (a real, disclosed decision, not a bug): a `Text` shape has no cached bounding box (its real extent is only known after shaping, done lazily at flatten time), and real UI text is normally hit-tested via its containing control's own background shape anyway.

**A real, previously-undisclosed architectural gap, found only by researching this properly before writing any code (not assumed from the approved plan's own design):** `tre_atlas::AtlasOwner`'s only way to read the shared atlas pixel buffer was `join(self) -> Vec<u8>`, which stops the background thread for good -- there was no way for a live, long-running renderer to refresh a GPU texture as *new* glyphs resolve after the very first frame. Fixed with new, real, disclosed engine work in `crates/tre-atlas/src/owner.rs`: the atlas buffer moved from a thread-local `Vec<u8>` to a shared `Arc<Mutex<Vec<u8>>>`, plus a new `Arc<AtomicU64>` generation counter bumped on every real insert. Two new `AtlasOwnerHandle` methods: `snapshot() -> Vec<u8>` (locks and clones the current buffer -- a real, disclosed cost, since the owner's own writes are small scattered per-glyph rects with no dirty-region tracking) and `generation() -> u64` (a cheap atomic load, letting a renderer skip `snapshot()` entirely when nothing changed since its last upload). `join`'s own existing contract is unchanged.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-atlas` 22 -> 23 tests, `tre-engine` 155 -> 156, both new tests real and passing). In `--release`, `cargo test --workspace --release --no-fail-fast` (a more thorough sweep than Step 12.1's own fail-fast run) surfaced 5 pre-existing, unrelated failures across `tre-engine` (the same 2 already disclosed in Step 12.1) and, newly discovered, `tre-memory`'s `alloc_guard` module (3 tests) -- all the same root cause (a `#[should_panic]` test wrapping a `debug_assert!`-only guard, a no-op in release), all confirmed pre-existing via `git status` showing zero uncommitted changes to either crate beyond this step's own disclosed additions; flagged as one consolidated follow-up task rather than fixed here, since neither crate's own release-mode gap is in this step's file list. `crates/tre-engine/src/shapes.rs`'s own new test (`flatten_into_renders_a_text_shape_via_the_real_font_and_atlas_pipeline`) proves the whole path end to end against a real system cascade font (`tre_text::FontCascade::discover()`) and a real `AtlasOwner` background thread: a first `flatten_into` call is a real cache miss (zero commands, matching `draw_text`'s own documented contract), and after polling every real glyph to resolution, a second call renders real, non-empty geometry.

### Step 12.3: `tre-python` `Font`/`Text` API -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Built: `crates/tre-python/src/font.rs` (new -- `tre.Font.load(path)`/`load_bytes(data)`/`system_cascade()`, each eagerly validating the bytes against both `skrifa` and `rustybuzz` before ever handing back a `Font` object, so a caller gets a clean error at load time, never a later panic deep inside a `render()` call). `tre.Font` is deliberately NOT tied to any one registry's `FontId` -- it's a plain, reusable Python value; `PyShapeRegistry` (in `shapes.rs`) now owns its own private `tre_engine::FontRegistry` plus a `HashMap<u64, FontId>` cache keyed by each `Font`'s own construction-time `uid`, so `insert_text` registers a given `Font`'s bytes into that registry's table exactly once no matter how many `Text` shapes reference it. New `tre.Text(x, y, text, font, px_size, fill_color)` Python class and `registry.insert_text(text) -> ShapeId`, with the same finiteness validation (and a new `px_size > 0` check) every other `insert_*` method already enforces.

Both `PyHeadlessRenderer` and `PyWindowedRenderer` now own a new `TextAtlas` (`crates/tre-python/src/text_atlas.rs`, shared by both to avoid duplicating this setup): one real `tre_atlas::AtlasOwner` background thread plus one real GPU texture, refreshed from the atlas's own live pixel buffer via Step 12.2's new `snapshot()`/`generation()` -- but only every 15 frames (`REFRESH_INTERVAL_FRAMES`), and only when the generation actually changed. A real, disclosed limitation, not a silent gap: there is still no `RhiDevice` method to update an existing texture's pixels in place, so a refresh creates a brand-new texture (and a brand-new bindless slot, from the fixed 4096-slot bindless array) every time; the interval bounds how *often* this can happen for an app with a continuously-growing glyph vocabulary, not the underlying cost -- the real fix is a future `RhiDevice::update_texture`, deferred as disclosed follow-up work. Most real UI text settles on a bounded glyph vocabulary quickly, so this is a real, acceptable first-pass tradeoff.

Two more real fixes, found only by actually building this:

* `RhiTexture` gained the same `Send + Sync` supertrait bound `RhiBuffer`/`RhiPipelineState` already carry (from Step 10.4's own identical fix) -- without it, `TextAtlas`'s own `Box<dyn RhiTexture>` field blocked `PyHeadlessRenderer` from being `Send`/`Sync`, which blocks `Python::detach` from being usable at all.
* `pyo3`'s `py-clone` feature had to be enabled: `PyText`'s own `#[derive(Clone)]` needs `Py<PyFont>: Clone`, which PyO3 0.27 gates behind that feature.
* A real borrow-splitting fix in `PyShapeRegistry`: a plain `&self` accessor for its font table would borrow all of `self` for the accessor's return lifetime, blocking the separate `&mut self.inner` borrow `flatten_into` itself needs. Fixed with `inner_and_fonts(&mut self) -> (&mut ShapeRegistry, &FontRegistry)`, a direct dual field projection the borrow checker accepts as disjoint.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug; `--release` clean apart from the same 5 pre-existing, already-flagged failures (Step 12.2's own disclosure). `demo/phase12_step12_3/demo.py`, run via `maturin develop --release`: loads the real system cascade font, inserts a real `Text` shape, and renders it headlessly across enough real frames (with real wall-clock gaps) for the background atlas thread to resolve every glyph and the periodic texture refresh to catch up -- confirmed via real, non-background pixels appearing in the text's own bounding region, not merely "it didn't crash." A second, ad hoc real-window check (`WindowedRenderer` + the same `Text` shape, 30 real on-screen frames) confirms the identical path works windowed too.

### Step 12.4: Gradient + Texture Python Fill API -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** At the user's own explicit direction, every shape's `fill_color` field became a real `int | GradientId | Texture` union (not a separate `fill=` parameter) -- resolved into the matching `tre_engine::FillStyle` variant by a new `PyShapeRegistry::resolve_fill` at `insert_*` time, so existing code passing a plain RGBA8 `int` keeps working completely unchanged. Built: `crates/tre-python/src/gradient.rs` (new -- `tre.Gradient.linear`/`.radial` real gradient definitions, plus `PyGradientId`, wrapping `tre_engine::ShapeRegistry::create_gradient`'s own already-real, already-working API from Phase 10 Step 10.2.1 directly) and `crates/tre-python/src/texture.rs` (new -- `PyTexture`, `PyTextureFormat`). `PyHeadlessRenderer`/`PyWindowedRenderer` both gained a `create_texture(width, height, format, pixels) -> Texture` method, mirroring `RhiDevice::create_texture` directly -- texture creation is renderer-scoped (needs a live GPU device), unlike gradients (CPU-only, registry-scoped).

Every one of `PyRectangle`/`PyCircle`/`PyPolygon`/`PyPath`'s `fill_color` field changed type from `u32` to `Py<PyAny>` (an opaque Python object, resolved lazily); their `From<&PyX> for EngineX` conversions -- previously infallible, since a plain `u32 -> Color` cast never fails -- became `to_engine(&self, fill: FillStyle) -> EngineX` methods taking an already-resolved fill, since resolving `Py<PyAny>` into a `FillStyle` is fallible (an int, a `GradientId`, or a `Texture` -- anything else is a real `ValueError`) and needs `&mut PyShapeRegistry` + a `Python<'_>` token, neither of which a plain `From` impl can carry.

A real lifetime concern, found and fixed before it could bite a caller: a `Texture`'s underlying `Box<dyn RhiTexture>` (its real GPU resource) has nothing else keeping it alive once `resolve_fill` extracts its raw bindless `u32` index -- if the original Python `Texture` object were garbage-collected first, a shape would reference a dangling bindless slot. Fixed with `PyShapeRegistry::textures_kept_alive: Vec<Arc<Box<dyn RhiTexture>>>`, populated by `resolve_fill` on every texture-fill resolution, keeping each texture alive for exactly as long as the registry that references it exists.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug; `--release` clean apart from the same 5 pre-existing, already-flagged failures. `demo/phase12_step12_4/demo.py`, run via `maturin develop --release`: one real scene mixes all three fill kinds -- a plain-int solid rectangle (regression check), a linear-gradient rectangle (asserted to actually vary in color across its own width, not just differ from background), and a texture-filled circle (a real 4x4 solid-magenta texture, asserted to read back as *exact* magenta) -- all in a single real headless render.

### Step 12.5: `Canvas` Redesign -- Status: Complete (2026-09-10)

**Real, working, and verified end to end -- completes Phase 12.** Per the approved plan's own explicit direction ("do NOT just copy what `RenderingCanvas` has, tailor the `Canvas` to our engine"), `PyCanvas` was not ported 1:1 from `RenderingCanvas`'s ~20 methods. `ShapeRegistry` stays the primary way to describe *what exists*; `Canvas` becomes the real scene-assembly/compositing context shapes flatten into. At the user's own explicit direction, `push_clip`/`push_layer` are exposed as real Python context managers (`with canvas.clip(x, y, width, height): ...` / `with canvas.layer(x, y, width, height, blur=False): ...`), not bare push/pop method pairs a caller could mismatch -- `canvas.clip(...)`/`canvas.layer(...)` push immediately (evaluated once, synchronously, before the `with` block starts) and return a small guard object (`ClipGuard`/`LayerGuard`) whose `__exit__` pops. `canvas.tag_accessibility_node(node_id, x, y, width, height, role)` is a plain method (no matching pop), wrapping the real, already-working `RenderingCanvas::tag_accessibility_node` (Phase 5 Step 5.3.1) directly.

The real seam: both `PyHeadlessRenderer` and `PyWindowedRenderer` gained `flatten_into(canvas, registry)`, letting more than one `ShapeRegistry` -- with `clip`/`layer` scopes interleaved between them -- share one `Canvas` before a single `render_canvas(canvas)` (Headless) / `render_canvas(window, canvas)` (Windowed) call submits it. `render(registry)`'s own existing single-registry convenience wrapper is now implemented in terms of this same seam internally (constructs a throwaway `Canvas`, flattens once, submits) -- unchanged behavior for existing callers, confirmed by re-running every prior step's own demo.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug; `--release` clean apart from the same 5 pre-existing, already-flagged failures. `demo/phase12_step12_5/demo.py`, run via `maturin develop --release`: a full-canvas red background registry and a green registry flattened inside a `canvas.clip(...)` block share one canvas, submitted via `render_canvas` -- real pixel assertions confirm green strictly inside the clip rect and unclipped red immediately outside it on every side checked, plus a `render(registry)` regression re-check. (One genuine test-script mistake surfaced and was fixed during this verification, not a binding bug: `Circle`'s own `x`/`y` are its bounding-box top-left, not its center -- the identical real gotcha Phase 10 Step 10.4's own demo history already documents, re-discovered by this step's own demo script before being caught by its own pixel assertions -- exactly the kind of mistake this project's "assert exact expected values" discipline exists to catch.)

**Phase 12 is now complete.** All five sections of the approved plan -- real input events + windowed rendering, `Text` as a first-class engine primitive, the `Font`/`Text` Python API, gradient/texture fill, and this `Canvas` redesign -- are real, working, verified against real GPU hardware and a live compositor, and documented. `tre-python` now exposes windowed rendering with real input events, retained-mode text via real system fonts, gradient- and texture-filled shapes, and a real multi-registry compositing seam -- the "complete `pySilver` support" and "best damn rendering engine" the project owner's own governing directive for this phase asked for. Real, disclosed follow-up work carried forward (not silently dropped): gradient/texture fill for `Text` itself (currently solid-only); a real `RhiDevice::update_texture` to avoid `TextAtlas`'s own bindless-slot churn on repeated glyph-vocabulary growth; and the pre-existing, unrelated `tre-engine`/`tre-memory` release-mode test gaps flagged as a separate task earlier in this phase.

### Step 12.6: Real Parallel Recording (`SubCanvas` Exposed to Python) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end -- a real follow-up beyond the original five-section plan**, requested after the project owner asked whether the engine's real multi-threading (found and explained during this same conversation: a background `tre_atlas::AtlasOwner` thread and a `tre-rhi-vulkan` GC thread already run per renderer instance) extended to concurrent scene recording. `RenderingCanvas::create_sub_canvas`/`SubCanvas::stitch_into` (Phase 5 Step 5.2.x) were real, tested, but unreachable from Python. At the project owner's own explicit direction ("if Rust is handling the low-level threading and Python is just calling for parallel... optimize the threading on the rust side"), both `PyHeadlessRenderer` and `PyWindowedRenderer` gained `render_parallel(registries)` (windowed: `render_parallel(window, registries)`): Python supplies a list of `ShapeRegistry` objects; every threading decision (thread count, scheduling, merge order) happens entirely in Rust.

Real concurrency-safety was verified by reading the actual code, not assumed, before building anything: `SubCanvas::stitch_into`'s own `ScatterArena` reservations are lock-free; the real per-shape GPU style-buffer write path (`RhiDevice::shape_style_buffer`, used by `Rectangle`/`Circle`'s own flatten code) is mutex-protected in the real Vulkan backend (`VulkanRingBuffer::write`); the shared text atlas's request/lookup path is lock-free by design (Phase 4). None of `ShapeRegistry::flatten_into`'s own real work needed *new* synchronization -- the engine was already built for exactly this (`create_sub_canvas`'s own doc comment always said "intended to be moved into a real worker thread"), it simply had no Python-reachable seam before this step. The one genuinely serialized piece is the shared text atlas's own texture-refresh check, resolved once up front before any worker thread starts, then shared read-only.

A real, non-obvious implementation finding: `PyRefMut` (PyO3's own runtime borrow guard) is itself `!Send` (it carries a GIL token internally), so it cannot cross into a GIL-released closure at all -- not even just captured, unused. Each registry's guard stays on the calling thread for the whole call (keeping its own borrow-flag set, so concurrent Python-side misuse of the same registry is still correctly rejected); only a plain `&mut ShapeRegistry`, reborrowed out of each guard, actually moves into a worker thread via `std::thread::scope`. The whole feature required zero `unsafe` code -- Rust's own type system enforced every real invariant (aliasing, lifetime, thread-safety) at compile time once the design correctly separated "GIL-bound guard, stays put" from "plain Rust reference, safe to send."

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug; `--release` clean apart from the same 5 pre-existing, already-flagged failures. `demo/phase12_step12_6/demo.py` proves genuine parallelism, not merely correct output (which serial execution would also produce): 8 registries, each holding one deliberately expensive 400-segment `Path` (real `lyon` fill tessellation work), rendered sequentially via 8 `render()` calls versus one `render_parallel()` call across all 8 -- a representative run measured a real **3.52x** speedup. A second, ad hoc check confirmed the identical capability through `WindowedRenderer`.

Real, disclosed scope boundary: `render_parallel`'s own `FrameArena` has a fixed element capacity (comfortably large, not yet configurable) -- a caller whose combined parallel scene is genuinely larger gets a clean `TreError`, matching this project's own established "report, don't silently truncate or grow mid-frame" discipline for every other fixed-capacity buffer in the codebase.

## Phase 12, continued: GUI-Framework Gap Remediation (Added 2026-09-10)

Phase 12 was marked complete after Step 12.5 (all five sections of the original approved plan); this continuation covers real, additional work that followed directly from it, not a new numbered phase. A full desktop-GUI-framework gap assessment was run (grounded in the current Rust GUI ecosystem -- `taffy` for layout, `accesskit` for accessibility bridging, `winit`'s own already-available IME/drag-drop events, `rfd`/`tray-icon` for native dialogs/menus -- cross-referenced against a direct, file-and-line-cited read of tre's own code and documentation, not assumption). It found: a real, working SVG rendering pipeline (`tre-svg`) completely unreachable from `tre-python`; a real AT-SPI2 accessibility bridge (`tre-a11y`) wired into nothing; and nine other framework capabilities (layout, focus/tab-order, editable text/IME, clipboard, drag-and-drop, native menus/dialogs/tray, theming, a running animation timeline, cursor customization) with no implementation at all -- several by original design (layout/theming were always meant to be the UI framework's job, not tre's). At the project owner's own explicit direction, this phase tackles the free/cheap wins first (drag-and-drop, IME, cursor -- all already implemented by `winit`, just not forwarded; SVG -- the pipeline already works, just isn't bound to Python), defers the accessibility process-split problem (tracked separately), and plans the one genuine remaining architecture gap (editable text).

### Step 12.7: Drag-and-Drop / IME / Cursor Passthrough -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** `tre_engine::InputEvent` gained seven new variants -- `FileDropped`/`FileHovered`/`FileHoverCancelled` (real OS drag-and-drop) and `ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled` (real IME composition) -- translated in `tre-platform/src/winit_backend.rs` from winit's own real `WindowEvent::DroppedFile`/`HoveredFile`/`HoveredFileCancelled`/`Ime(..)`, verified directly against winit 0.30.13's real source (not assumed) before writing any code. `InputEvent` lost its `Copy` derive as a real, disclosed consequence (`PathBuf`/`String` payloads aren't `Copy`) -- every real call site already took it by value, so this needed no rewrites beyond one `*event` dereference in `tre-python`'s own `Resized`-handling loop, which became a plain reference match instead.

New `tre_platform::CursorIcon` (the real, complete CSS3/`cursor-icon` set winit itself exposes, 33 variants) plus `PlatformConnection::set_cursor`/`set_ime_allowed`, mirroring the existing `set_title`/`set_icon` pattern exactly. `set_ime_allowed` exists because winit itself requires it: no IME composition event fires for a window until this has been called with `true` for it -- a real platform requirement, not a tre design choice. All three are exposed on `tre-python`'s `WindowedRenderer` (`set_cursor`, `set_ime_allowed`), with a new `tre.CursorIcon` Python enum.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug; `--release` clean apart from the same 5 pre-existing, already-flagged failures. `demo/phase12_step12_7/demo.py` (kept under the `phase12_` naming to match its own Phase 12 Section-1 lineage) proves every new API call succeeds against a real window. **Real, disclosed scope limit**: this sandbox has no input-injection tool (`xdotool`/`wtype`/`ydotool` confirmed absent), so a real file drop or IME composition can't be synthesized to prove event *delivery* end to end -- what's verified is the real `WindowEvent` translation logic (checked against winit's own source) and that every new API call succeeds with no error.

### Step 12.8: SVG Binding -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** The gap assessment found `crates/tre-svg`'s real SVG-to-pixels pipeline (`usvg` parse -> curve-flatten -> `lyon` fill tessellation, including a working keyframe morph) completely unreachable from `tre-python` -- not a missing feature, a missing binding. `tre-engine` itself cannot depend on `tre-svg` at all (a real dependency cycle: `tre-svg` already depends on `tre-engine` for `UiVertex`), so the fix needed a small, genuinely new engine primitive rather than a plain new dependency line.

Built: a new, minimal `tre_engine::Svg` primitive (`common`, already-tessellated `positions`/`triangles`, one flat `fill_color`) that stores no `tre-svg` type at all -- it just holds plain positions/triangle indices, sidestepping the dependency cycle entirely. Its `flatten_into` dispatch reuses the *existing* `RenderingCanvas::draw_flat_polygon`/`FlatColor` pipeline unchanged -- no new shader, no new GPU path. `tre-python` (a leaf crate with no such cycle) gained a direct `tre-svg` dependency and a new `tre.Svg.parse(data, fill_color, fill_rule=...)` static constructor: parses real SVG bytes via `tre_svg::parse_svg`, tessellates via `tre_svg::tessellate_fill` (which correctly handles both winding rules and real compound-shape holes), keeps only each vertex's `.position` (discarding a placeholder color baked in only to satisfy `tessellate_fill`'s own signature), and hands the result to `registry.insert_svg(svg)` -- the same `insert_*` -> `ShapeId` pattern every other shape already follows.

**Solid fill only, this pass** -- the same real, disclosed boundary `Text` already established: `draw_flat_polygon` takes one flat `rgba`, not a `FillStyle`. A second, more fundamental real limitation, confirmed by reading `tre-svg`'s own parsing loop directly (`collect_polygons`): it checks only `path.fill().is_some()`, never the fill's actual paint/color, and discards per-path fill-rule too -- `tre-svg` today extracts geometry only. A caller supplies **one** solid color for the whole parsed document; a real multi-color SVG icon renders as a single flat color, not its original per-path colors. This is a real `tre-svg` limitation inherited as-is, not a choice made in this binding.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-engine` 156 -> 157 tests: a new test proves `ShapePrimitive::Svg` renders real geometry through `flatten_into`'s `FlatColor` dispatch). `--release` clean apart from the same 5 pre-existing, already-flagged failures. `demo/phase12_step12_8/demo.py`, run via `maturin develop --release` against real GPU hardware: parses two real SVG documents (a plain square, and a ring using `fill-rule="evenodd"` with a real punched-out hole) and asserts exact expected pixels both inside each shape and inside the ring's own hole -- proving both winding rules and real compound-shape holes work end to end from Python, not merely in Rust.

Not yet done: per-path multi-color/gradient extraction (both real `tre-svg` limitations); exposing `tre_svg::morph`/`morph_into` (real, working in Rust) to Python for animated SVG.

### Step 12.9: `border_enabled` + Real Scale/Rotation Exposure -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Two follow-up fixes from a direct Q&A pass on `tre-python`'s exposed functionality:

**A real `border_enabled: bool`.** Previously the only way `Rectangle`/`Circle`/`Polygon`/`Path` decided whether to draw a border was `border_thickness > 0.0` -- there was no way to turn a border off without discarding its configured width. All four `tre_engine` shapes gained a real `border_enabled` field; each of the four render-dispatch sites (`flatten_rectangle`/`flatten_circle`/`flatten_polygon`/`flatten_path_shape`) now computes an *effective* thickness (`if shape.border_enabled { shape.border_thickness } else { 0.0 }`) rather than mutating the stored value, so a caller can flip a border off and back on without losing its width. `tre-python`'s four matching classes gained the same field, wired through `to_engine()`.

**Real `scale_x`/`scale_y`/`rotation` exposure.** `tre_engine::Transform2D` already carried real `scale`/`rotation` fields (`to_affine2` composes scale, then rotation, then translation into a real `Affine2`) -- every native Rust demo already used them, but `tre-python`'s own `common()` helper silently hardcoded both to identity for every Python-inserted shape, a real gap this project's own GUI-framework gap assessment surfaced directly (`shapes.rs`'s own former top-level doc comment said so). `common()` was widened to take `scale_x`/`scale_y`/`rotation`, and `Rectangle`/`Circle`/`Polygon`/`Path`/`Text`/`Svg` all gained matching constructor parameters (defaulting to `1.0`/`1.0`/`0.0`, via `#[pyo3(signature = ...)]`), each validated finite before reaching `tre_engine`.

**A real, disclosed process constraint found while building the demo**: `HeadlessRenderer` opens a real `PlatformConnection` -- a genuine winit `EventLoop` -- and winit permits at most one `EventLoop` per process, ever. A first draft of `demo/phase12_step12_9/demo.py` created a fresh `HeadlessRenderer` per render call; the first one in the process always succeeded and every subsequent one always failed with `RuntimeError: failed to connect to the display server`. Fixed by creating exactly one renderer per process and reusing it for every render -- a real constraint worth carrying forward into any future demo or `pySilver`-side code that renders more than one frame.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase12_step12_9/demo.py`, run via `maturin develop --release` against real GPU hardware: an exact-byte comparison proves `border_enabled=False` with a non-zero `border_thickness` renders identically to no border at all, and toggling it off then back on restores the exact same pixels while the configured thickness survives untouched; exact-pixel assertions against hand-derived transform math prove `scale_x=2.0` stretches a rectangle's footprint to precisely the expected edge, and `rotation=pi/2` sweeps a wide rectangle into the exact bounding region `Transform2D::to_affine2`'s real composition order and `Affine2::from_rotation`'s real sign convention predict.

Real, disclosed scope limit carried forward: `Path` still has no `x`/`y` translation exposed at all (its commands are already authored in absolute coordinates) -- `scale_x`/`scale_y`/`rotation` were added to it anyway for consistency, applied about the fixed local origin `(0, 0)` its commands are drawn relative to; a real translated `Path` remains a separate gap, not addressed here.

## Phase 13: Timing, Tweening, Animation, Math, Shader, Shadow, and Editable-Text Systems (Added 2026-09-10)

A further round of Python-exposed-functionality Q&A (following the Phase 12 gap assessment) asked for `treAnimation`/`treTime`/`treTween` (a real dependency chain), SMIL SVG animation parsing, an evaluation of third-party Rust math libraries, vertex animation, a user-facing custom shader API, and shadow support for every renderable item -- plus the still-pending "plan out editable text" from the previous directive. A full plan covering all nine deliverables (grounded in direct source investigation of `FrameClock`/`AnimationId`/`spring_decay`, the Vulkan pipeline/shader creation constraints, `LayerDesc`'s existing real blur, and `tre_svg::morph`'s existing real point-interpolation) was written and approved before any implementation -- see `/home/phil/.claude/plans/warm-painting-squid.md` for the full design and dependency ordering (`treTime` -> `treTween` -> `treAnimation` -> {SMIL parsing, vertex animation}; math library and the shader API/shadows track are independent; editable text is independent of all of the above).

### Step 13.1: `treTime` (`tre.Clock`) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** `tre_engine::input::FrameClock` was already real (a `std::time::Instant`-backed monotonic frame timer, used natively in `main_loop_demo.rs`) but entirely unexposed to Python. `FrameClock` gained one new method, `elapsed() -> f32` (total real seconds since construction via a new `created_at: Instant` field, set once at `FrameClock::new()`), distinct from `tick()`'s own per-frame delta -- the real "t" a future `treTween`/`treAnimation` caller needs to sample a tween or timeline against, since a per-frame delta alone can't answer "how far into this animation are we."

`tre-python` gained a new `clock.rs` module: `tre.Clock()` wraps `FrameClock` directly (no new timing logic -- `tick()`/`elapsed()` delegate straight through), registered in `lib.rs`'s `#[pymodule]` alongside every other real binding.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-engine` 157 -> 158 tests: a new `frame_clock_elapsed_accumulates_independently_of_tick` test, verified against a real ~20ms sleep matching the existing `tick`-delta test's own style). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_1/demo.py`, run via `maturin develop --release`: proves `tick()` reports a real ~50ms delta after a real 50ms sleep, and `elapsed()` keeps growing across two real sleeps (~80ms total) even while an immediately-repeated `tick()` reports a near-zero delta -- confirming `elapsed()` genuinely tracks time since construction, not time since the last `tick()`.

Not yet done: `treTween` (Step 13.2) and `treAnimation` (Step 13.3), both of which will consume this `Clock` -- this step is the timing foundation only, per the approved plan's own dependency ordering.

### Step 13.2: `treTween` (`tre.Tween`/`tre.Easing`/`tre.Spring`) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** A new standalone crate, `crates/tre-tween`, the first genuinely new animation-domain crate in the workspace: the pure, stateless interpolation math layer `treAnimation` (Step 13.3) will sequence on top of. Depends on `glam` (0.29) -- the project's own math-library evaluation (Q9): the de facto standard, SIMD-backed Rust math crate for real-time graphics (Bevy/wgpu ecosystem), providing the `Vec2` vocabulary tweening needs. Deliberately additive, not a replacement: `tre-math`'s own `Affine2`/SIMD-batch functions, already proven in `tre-engine`'s hot flatten path, are untouched.

Three real pieces: `Easing` (the standard Robert Penner curve set -- linear, quad/cubic/quart/quint in/out/in-out, 13 named curves, each a pure `f32 -> f32` function); `Tween<T>` (generic over anything implementing a small local `Lerp` trait, implemented for `f32` and `glam::Vec2` today -- samples `elapsed` seconds by clamping into `[0, duration]` before applying the easing curve, so a caller always gets a well-defined `from`/`to` endpoint rather than extrapolation past the tween's own span); and `Spring`, a **real** damped mass-spring-damper integrator (semi-implicit/symplectic Euler) -- genuinely distinct from `tre_math::spring_decay`'s plain one-pole exponential smoothing, whose own doc comment explicitly says it can never overshoot. `Spring` both overshoots when underdamped and settles without overshoot when heavily overdamped, real spring behavior at both ends of the parameter space, tested against both regimes.

`tre-python` gained a new `tween.rs` module. `PyTween` solves a real PyO3 constraint -- `Tween<T>` is generic, but PyO3 cannot export a generic class -- by storing one of two concrete instantiations chosen from the Python arguments' own shape (`float` or a real `(x, y)` tuple), rejecting a mismatch (e.g. `Tween(0.0, (1.0, 2.0), ...)`) with a real `TypeError` rather than a confusing downstream failure. `PySpring`/`PyEasing` bind directly through with no new logic.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (new `tre-tween` crate: 12 tests -- every easing curve's `0->0`/`1->1` boundary and relative-speed ordering, tween clamping/zero-duration/Vec2 interpolation, both spring regimes). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_2/demo.py`, run via `maturin develop --release`: every assertion checks an exact hand-computed value (e.g. `EaseInQuad` at progress 0.5 of a `0..10` tween gives exactly `2.5`; an underdamped `Spring(200, 5, 1)` chasing target `1.0` overshoots past `1.05`; a heavily overdamped `Spring(50, 200, 1)` does not).

Real, disclosed scope limits: only `f32`/`glam::Vec2` are exposed as `Lerp` instantiations to Python today (`Vec3`/`Quat`/`Color` are real, straightforward additions once a real 3D or color-tween consumer exists, not built speculatively); `Tween`/`Spring` are pure math with no timeline/sequencing concept -- that's `treAnimation` (Step 13.3), not yet started.

### Step 13.3: `treAnimation` (`tre.Timeline`) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** A new crate, `crates/tre-animation`, providing `Timeline` -- a real sequencer of any number of concurrently-running `treTween::Tween<f32>`s, each with its own start time relative to one shared clock, advanced by real delta time each `advance(dt)` call. `Timeline` deliberately has no PyO3 dependency at all: it identifies every scheduled animation by an opaque `u64` target id, mirroring `tre_engine::shapes::AnimationId`'s own real, already-established pattern ("the registry/caller knows what the id means, the sequencer doesn't have to") -- keeping `tre-animation` reusable by any future caller, not just `tre-python`'s specific shape classes, and keeping the dependency graph pointing the right way (`tre-python` depends on `tre-animation`, never the reverse).

`tre-python`'s new `animation.rs` supplies the missing half: `PyTimeline::animate(target, property, to, duration, easing)` reads `target`'s current `property` value via Python's own `getattr`, schedules a real `Tween<f32>` against a fresh `u64` id, and remembers `(target, property)` for that id. `PyTimeline::advance(dt)` samples every entry and applies each result back via `setattr` -- this works generically for **any** `#[pyo3(get, set)]` field on **any** shape class (Rectangle/Circle/Polygon/Path/Text/Svg all already qualify) with zero per-shape dispatch code, directly reusing this session's own Step 12.9 `scale_x`/`scale_y`/`rotation`/`opacity` exposure as real animation targets for the first time since they were added. `Timeline::prune_finished` was designed to return the `target` ids it actually removed (not just drop them silently) specifically so `PyTimeline` can keep its own Python-object side-table in exact lockstep, rather than needing to guess or over-approximate.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (new `tre-animation` crate: 5 tests covering single-entry sampling, multi-call accumulation, a finished entry correctly reporting `still_running=false` while holding its exact `to` value, two entries with different start times sampling independently within one `Timeline`, and `prune_finished` reporting exactly the removed target ids). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_3/demo.py`, run via `maturin develop --release`: every hand-computed sample matches exactly (e.g. `x` at `t=0.25`/`1.0` of a `0->100` linear tween is exactly `25.0`), a finished animation holds at its exact `to` value with `still_running=False`, `prune_finished()` drops only the actually-finished entry, and -- the real integration proof this project's own verification discipline requires -- an animated rectangle is inserted into a real `ShapeRegistry` and rendered through `HeadlessRenderer`, with exact-pixel assertions confirming the shape rendered at its new animated position and NOT at its original one.

Real, disclosed scope limits: `Timeline` only animates `f32`-typed attributes (matching `treTween`'s `Lerp<f32>` Python exposure so far) -- animating a `(x, y)` pair as one `Vec2` timeline entry isn't wired into `PyTimeline::animate` yet, though `tre_tween::Tween<glam::Vec2>` already exists and could back it directly. No looping/yoyo/repeat-count exists yet -- each entry runs exactly once; real, straightforward future work once a caller needs it.

### Step 13.4: Real Blur-Based Shadows -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Answers the project owner's own question of whether "copy the shape, apply a gradient" is the best way to add shadows: **it is not**. `tre_engine::LayerDesc` already had a real `blur: bool` field applying a genuine, already-shipped Dual-Kawase blur (Step 7.2.2) to a layer's own content before compositing, and `canvas.layer(x, y, width, height, blur=True)` has been real, tested, Python-exposed API since Phase 12 Step 12.5. **No new rendering path was needed for shadows at all.** The only real gap was the geometry of sizing/positioning the shadow's own offscreen layer -- closed with one new pure function, `tre_engine::shadow_layer_bounds(x, y, width, height, offset_x, offset_y, blur_margin) -> (i32, i32, u32, u32)`: the union of a shape's own bounding box and its offset copy, expanded by `blur_margin` pixels on every side so the blur has room to spread without being clipped at the layer's own edge. Bound directly to Python as `tre.shadow_layer_bounds(...)`.

A real, disclosed coordinate-space finding made while building this step's own demo: content drawn inside a `canvas.layer(...)` block uses coordinates **local** to the layer's own top-left corner, not the outer canvas's absolute coordinates -- confirmed directly against `tre-engine`'s own RHI doc comment (`begin_render_to_texture`'s NDC mapping is driven only by the layer's own `logical_width`/`logical_height`, with no offset parameter) and then verified empirically (a small standalone script rendering one rectangle inside a layer and checking where it actually appeared in the final composited frame) before trusting it in the real demo's own shadow-positioning math.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-engine` 158 -> 160 tests: `shadow_layer_bounds`'s general case and its zero-offset/zero-margin identity case). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_4/demo.py`, run via `maturin develop --release`: proves the real shape renders fully opaque on top of its own shadow, and -- the proof a gradient-copy approach could never produce -- scans pixel darkness across the shadow's own *original, pre-blur* hard edge and finds a genuine, non-linear gradient straddling it (strictly between interior and background darkness exactly at the former edge, still measurably darker than background 5px past it, and exactly background far enough away) -- the signature of real blur, impossible from a solid alpha-blended duplicate, which would jump straight from interior to exact background right at that edge.

Real, disclosed scope limits carried forward from `LayerDesc.blur` itself: blur softness is fixed (no tunable radius yet) -- every shadow in this step has the same softness, real separate future RHI work. A convenience automating "insert a shape with a shadow" as one call (rather than manually building the shadow-colored silhouette registry, as this step's demo does explicitly) is real, straightforward follow-up once a concrete calling pattern from `pySilver` emerges -- not built speculatively here. A real, tunable SDF-based shadow (Q15's own "shader support and shadows" tie-in) remains a stretch goal gated on the custom shader API (not yet started), noted in the approved plan but not committed scope.

### Step 13.5: Editable Text (`tre.EditableText`) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** The still-pending "plan out editable text" item from the project owner's own earlier directive -- built on `tre_text::ShapedRun`/`ShapedGlyph` (already real: `cluster`/`x_advance` per glyph) and the `ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled` events already forwarded end to end since Step 12.7.

New `crates/tre-text/src/caret.rs`: `caret_positions(runs, text_len, px_size, units_per_em) -> Vec<CaretPosition>` and `hit_test(positions, x) -> usize`, using the *exact* pen-advance formula `tre_engine::text::flatten_text` itself renders with (`scale = px_size / units_per_em`, `pen.x += glyph.x_advance * scale`) -- read directly from that function's own source before writing this, not assumed, so a caret computed here lands exactly where the matching glyph actually renders. Pure logic, no font or Python dependency, so it's exactly unit-testable with synthetic `ShapedRun`/`ShapedGlyph` values (their own fields are `pub`) -- 3 new tests cover per-glyph placement, px_size/units_per_em scaling, and nearest-stop resolution including before-start/past-end clamping. A real, disclosed LTR-only limitation: `ShapedRun`s are in **visual** (not logical) order for bidi/RTL text, so the end-of-text stop's own placement assumes LTR.

New `tre-python::editable_text` module: `tre.EditableText(x, y, text, font, px_size, fill_color)` with `insert`/`delete_selection`/`delete_backward`/`set_caret`/`set_selection` (all maintaining a real UTF-8 char-boundary invariant -- `set_caret`/`set_selection` reject a byte offset that would split a multi-byte character), `hit_test(x)` (reshapes `text` against `font` fresh each call -- the same real, cheap operation `tre_engine::Text` itself performs every re-flatten), `handle_ime(event)` (real IME dispatch: `ImePreedit` updates a separate composing string without touching the committed `text`; `ImeCommit` splices its own text in via `insert()`), and `to_text()` (builds a real `tre.Text`, with any active `preedit` spliced in at the caret for real visual composing feedback, ready to insert into a `ShapeRegistry` and render).

A small, genuinely useful addition made along the way: `tre.WindowId` gained a real `#[new]` (`tre.WindowId(0)`) -- previously only a live `WindowedRenderer` could produce one, which made it impossible to synthesize a real `InputEvent` (needed to test/demo `handle_ime`) without a real window. A real, disclosed build-mode constraint found while first attempting to unit-test `PyEditableText` in Rust: `tre-python` is built with PyO3's `extension-module` feature, which does not link `libpython` (a real extension module gets those symbols from the host interpreter at load time) -- a native `cargo test` binary therefore cannot call any GIL-acquiring function (`Python::attach`, `Py::new`, etc.), so real coverage of `PyEditableText`'s behavior lives in this step's own Python-level demo instead, not a Rust unit test.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-text` 19 -> 22 tests). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_5/demo.py`, run via `maturin develop --release`: proves `insert`/`delete_selection` against exact expected text/caret state, `delete_backward` removing exactly one real character (not byte) from `"café"`, `set_caret` rejecting a boundary that would split `"é"`, `hit_test` against a real system font resolving to the exact start/end offsets and a strictly-between middle offset with proven monotonicity across increasing click positions, a full real IME event round-trip (`ImeEnabled` -> `ImePreedit` -> `ImeCommit` -> `ImeDisabled`), and a real render of `to_text()`'s own output (following the same real cache-miss/background-atlas retry pattern Step 12.3's own demo established) producing real, visible glyph ink.

Real, disclosed scope limits: single-line only, matching `tre_engine::Text`'s own current single-line scope (Step 12.2) -- a multi-line text *editor* (line wrapping, vertical caret movement) is real, separate future work, distinct from the multi-line text *label* rendering already supported. Caret/selection byte offsets are LTR-only-correct, matching `caret_positions`'s own disclosed limitation.

### Step 13.6: Vertex Animation (`tre.Svg.morph`) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Q12's "vertex animations" turned out to be *largely already possible* at the engine level: `tre_svg::morph`/`morph_into` already interpolates two equal-length point lists via a real SIMD primitive (`tre_math::lerp_points_batch`), and `tre_engine::Svg`'s own `positions` field (Step 12.8) is a plain, unvalidated public field a caller can already swap frame-to-frame. The one real gap was Python exposure -- closed with `tre.Svg.morph(from_, to, t) -> Svg`, interpolating two same-topology `Svg` meshes' own already-tessellated vertex positions while keeping `from_`'s own triangle indices unchanged (morphing changes positions, never mesh connectivity).

**Not a rename of `tre_svg::morph_into`**: that function operates on `tre_svg::Polygon`'s raw, un-triangulated contour points -- an earlier pipeline stage than `Svg`'s own already-tessellated `positions`. `Svg::morph` calls `tre_math::lerp_points_batch` directly instead (a new `tre-math` dependency for `tre-python`) -- the identical real SIMD primitive `morph_into` itself uses internally, applied to the right data shape for this real use case.

**No new `treAnimation` class needed.** `Svg.morph` is pure, stateless sampling, mirroring `Tween::sample`'s own contract exactly -- driving it frame-by-frame is just composing it with the already-shipped `tre.Tween`/`tre.Easing` machinery from Step 13.2 (`t = progress_tween.sample(elapsed)`, then `frame_svg = tre.Svg.morph(a, b, t)`), rather than a new bespoke sequencing type.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_6/demo.py`, run via `maturin develop --release`: `morph(big, small, t=0.0/1.0)` renders **byte-identical** to each real keyframe; `t=0.5` produces a genuinely intermediate size (a pixel inside the original big square but outside the real linearly-interpolated midpoint footprint reads background); morphing between mismatched vertex counts (a 4-point square vs. a 3-point triangle) raises a real `ValueError` naming the exact mismatch; and driving `t` through a real `Tween(0.0, 1.0, duration, Linear)` produces a real, monotonically shrinking sequence of on-screen widths across five sampled frames.

Real, disclosed scope limit carried forward from `tre_svg::morph` itself: only equal-topology morphing is supported -- morphing between visually similar but differently-triangulated meshes needs a real resampling step, out of scope here.

### Step 13.7: SMIL SVG Animation Parsing (`tre.parse_smil`) -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Q4's "SMIL/animation parsing for animated SVGs." Confirmed by reading `usvg`'s own source (not assumed): it does not process SMIL animation elements at all -- a static-resolution parser by design, the same real library `resvg` uses for static rendering. Real SMIL support needed a separate, direct XML pass: new `crates/tre-svg/src/smil.rs`, via `roxmltree` (already a transitive dependency of `usvg` itself, now pinned as a direct one), extracting real `<animate>`/`<animateTransform type="translate">` directives.

**Real, disclosed v1 scope**, deliberately not full SMIL spec compliance (a large surface: motion-path animation, complex `begin`/sync timing, `<animateColor>`, additive/accumulative animation): a single scalar `<animate>` attribute with either `values="a;b;c"` (a real keyframe list) or `from`/`to` (a real 2-keyframe list), `<animateTransform type="translate">` (a 2D point, same `values`/`from`-`to` support), and `dur` (`"Ns"`, `"Nms"`, or a bare number treated as seconds). `type="scale"`/`"rotate"`, `<animateMotion>`, `begin`, `repeatCount`, and `calcMode` are all real, disclosed gaps. An element missing a required attribute is silently skipped rather than rejecting the whole document -- one malformed directive among many real ones shouldn't break every other one. `SvgError` gained a new `MalformedXml(String)` variant for a genuinely malformed document.

`tre-python` exposes this directly: `tre.parse_smil(data: bytes) -> ParsedSmil`, with `ParsedSmil.animates: list[SmilAnimate]` and `.animate_translates: list[SmilAnimateTranslate]` (both plain `#[pyclass(get_all)]` data classes). **No new engine machinery was needed to drive it** -- `parse_smil` only extracts the real data an SVG author already wrote; a caller composes the extracted keyframes directly with the already-shipped `tre.Tween`/`tre.Easing` (Step 13.2) to animate a real shape, exactly the same composition pattern Step 13.6's vertex-animation step established.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-svg` 20 -> 28 tests: real `<animate>`/`<animateTransform>` extraction with both `from`/`to` and `values`, `dur` unit parsing across `s`/`ms`/bare forms, `type="rotate"` correctly ignored, a missing-attribute element skipped rather than erroring, and malformed-XML rejection). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_7/demo.py`, run via `maturin develop --release`: exact keyframe/duration extraction for both a real `<animate>` and a real `<animateTransform type="translate">`, an empty result for a real non-animated document, a real `ValueError` for malformed XML, and -- the full real integration proof -- the extracted translate keyframes driven through a real `Tween`, re-rendering a real `Svg` shape at 5 sampled points with the shape's own fill found exactly where its current SMIL-driven position says it should be at every frame.

Real, disclosed scope limits carried forward: `type="scale"`/`"rotate"`, `<animateMotion>`, `begin`/`repeatCount`/`calcMode`, and `<animateColor>` are all real gaps, not attempted in this pass.

### Step 13.8: Custom Shader API -- Status: Complete (2026-09-10)

**Real, working, and verified end to end.** Q13's "user-facing custom shader API" -- real user-supplied GLSL, compiled to SPIR-V at runtime via `shaderc` (the identical real Google `shaderc` library `build.rs`'s own `glslc` CLI invocation already wraps for this project's compile-time shaders, kept deliberately identical rather than a second, potentially-divergent compiler like `naga`). Feasibility was verified in complete isolation (a throwaway scratch crate outside this repository) before touching any real `Cargo.toml` -- this machine already has `libshaderc_combined` installed and discoverable via `pkg-config`, confirmed directly, not assumed.

**Real RHI constraints found and designed around**, confirmed by reading `VulkanDevice::create_pipeline`/`create_universal_pipeline_layout`'s own source before building anything: every real pipeline shares one fixed `UiVertex` vertex layout, triangle topology, and bindless descriptor set. A custom shader that accepts those constraints needs **no new RHI plumbing at all**: new `VulkanDevice::create_custom_pipeline` compiles the caller's fragment source and pairs it with `BINDLESS_TEXTURED_VERT` (widened from private to `pub(crate)`), the SAME real vertex shader `TexturedQuad`/`GradientFill`/`MsdfText` already use, via the *unmodified* `create_pipeline` method. And `execute_frame` already resolves any `pipeline_state_id` generically via `PipelineRegistry::get` -- not a hardcoded per-`PipelineKind` branch, confirmed directly -- so a freshly registered custom pipeline renders correctly with zero changes to the real draw-dispatch path.

The only genuinely new engine-level surface: `tre_engine::CustomShaded`, a plain UV quad `ShapePrimitive` (geometrically identical to `Rectangle`) carrying a `pipeline_id: u16` instead of a `FillStyle`, dispatched through a new `RenderingCanvas::draw_custom_shaded_quad` (mirrors `draw_flat_polygon`'s own vertex-emission pattern, real `0..1` UVs). A new `tre_engine::CUSTOM_PIPELINE_ID_BASE` (1000) constant gives custom ids real headroom above `PipelineKind`'s own 8 fixed ids. `EngineError` gained a new `ShaderCompilationFailed(String)` variant carrying `shaderc`'s own real compiler diagnostic (a caller authoring their own shader source genuinely needs to see *why* it failed) -- this required dropping `EngineError`'s own `Copy` derive (a `String` field can't be `Copy`), a real, safe, disclosed ripple with zero other call-site impact (error values are moved, never copied, elsewhere in the codebase).

`tre-python` exposes `renderer.create_custom_shader(fragment_source) -> CustomShaderId` (compiles and registers against that renderer's own `PipelineRegistry`, starting its own id counter from `CUSTOM_PIPELINE_ID_BASE`) and `tre.CustomShaded(x, y, width, height, shader_id, fill_color)` for insertion into any `ShapeRegistry`. A new `tre.CustomShaderId` handle mirrors `PyGradientId`'s own established "scoped to the registry/renderer that created it" pattern.

**Real, disclosed v1 interface contract** a fragment shader must declare: `layout(location = 0) in vec4 frag_color;`, `layout(location = 1) in vec2 frag_uv;`, `layout(location = 0) out vec4 out_color;`, and the identical 12-byte `PushConstants { vec2 screen_size; uint texture_index; }` block `bindless_textured.frag`'s own real source declares. No custom vertex shaders, no arbitrary vertex attributes, no compute shaders, no descriptor sets beyond the engine's own shared bindless set.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-engine` 160 -> 162 tests: a `CustomShaded` quad dispatches under its own caller-registered pipeline id with the exact expected vertex/UV layout, and its hit-test is a real axis-aligned rect test). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase13_step13_8/demo.py`, run via `maturin develop --release` against real GPU hardware: a hand-written GLSL shader outputting `vec4(frag_uv.x, frag_uv.y, 0.0, 1.0)` renders a real per-pixel gradient (red strictly increasing left-to-right, green strictly increasing top-to-bottom, blue pinned at exactly 0 everywhere -- the signature of genuine shader execution, not a flat/cached color); deliberately broken GLSL raises a real `ValueError` carrying `shaderc`'s own actual compiler diagnostic; and a `CustomShaded` quad renders correctly alongside a plain `Rectangle` in the same `ShapeRegistry`.

Real, disclosed scope limits: no custom vertex shaders, no compute shaders, no descriptor sets beyond the engine's own shared bindless set, no arbitrary vertex attributes. `fill_color` on `CustomShaded` is a plain flat color multiplier, not the polymorphic `int | GradientId | Texture` union every other shape accepts -- what a custom fragment shader does with it is entirely up to its own GLSL. A real, tunable SDF-based shadow (Q14/Q15's own "shader support and shadows" tie-in) could now build on this API as a genuine v2, noted in the approved plan but not part of this step's committed scope.

## Phase 13 Summary

All eight sections of the approved Phase 13 plan (`/home/phil/.claude/plans/warm-painting-squid.md`) are now complete: `treTime` (13.1), `treTween` (13.2), `treAnimation` (13.3), real blur-based shadows (13.4), editable text (13.5), vertex animation (13.6), SMIL SVG animation parsing (13.7), and the custom shader API (13.8) -- answering every one of the project owner's 15 Python-exposed-functionality questions plus the two immediate fixes (`border_enabled`, real scale/rotation exposure, both Phase 12 Step 12.9) from the governing directive that opened this phase.

## Phase 14: Real System Clipboard Access (Added 2026-09-11)

The [tre GUI Readiness assessment](https://claude.ai/code/artifact/2d7cafa8-bf78-4c65-ade1-a2f3c0362196) (updated after Phase 13) flagged clipboard support as its own recommendation #4, separate from the Phase 13 plan -- a new phase, not a Phase 13 addendum.

### Step 14.1: `tre.Clipboard` -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Real system clipboard text access via `arboard` -- the exact crate the readiness assessment itself named ("a small, self-contained cross-platform crate with no architectural entanglement with the rest of tre"). Feasibility was verified in a throwaway scratch crate, outside the repository, against this machine's real clipboard service before touching any real `Cargo.toml` -- the same discipline used for the `shaderc` dependency in Phase 13 Step 13.8.

New `crates/tre-platform/src/clipboard.rs`: `Clipboard::new()`/`get_text()`/`set_text()`, each mapping a real `arboard::Error` into `PlatformError::Other` with a real, specific message rather than a generic failure. `arboard`'s own `default-features = false` drops its `image-data` feature entirely -- the identical "smallest real slice first" precedent `tre-svg`'s own `usvg = { default-features = false }` already establishes; plain text only, a real, disclosed v1 scope. `tre-python` binds this directly as `tre.Clipboard`, marked `unsendable` (matching `PlatformConnection`'s own precedent for platform-connection state that isn't safely `Send`).

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-platform` 0 -> 1 test: a real round-trip against this machine's own live clipboard service, not mocked -- matching this project's own "real demos/tests as the correctness oracle" precedent for platform-level code, which previously had zero unit tests at all). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase14_step14_1/demo.py`, run via `maturin develop --release`: a basic round-trip using a fresh UUID marker per run (the system clipboard is real, shared, stateful OS state -- the test never assumes it starts empty), a second `set_text` fully replacing the first, and real multi-byte UTF-8 content (accented Latin, a CJK phrase, a real 4-byte-UTF-8 emoji) round-tripping byte-exact through the real platform clipboard service.

Not yet done: image/rich-text clipboard content (a real `arboard` feature, deliberately deferred); `EditableText` (Phase 13 Step 13.5) has no `.cut()`/`.copy()`/`.paste()` convenience methods wired to this `Clipboard` yet -- a real, natural next pairing now that both pieces exist independently.

### Step 14.2: Cut/Copy/Paste (`EditableText` + `Clipboard`) -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Wires the real system `Clipboard` (Step 14.1) into the real single-line text editor `EditableText` (Phase 13 Step 13.5) -- the natural pairing both steps' own READMEs already named as the obvious next step.

`EditableText.copy(clipboard)`/`.cut(clipboard)`/`.paste(clipboard)` take a `Clipboard` instance explicitly rather than `EditableText` owning one internally -- a real design choice: a real app has one system clipboard connection shared by every text field in it, not one per field, and `EditableText` never pays the real cost of opening a clipboard connection unless a caller actually invokes cut/copy/paste. `copy()` writes the active selection to the clipboard, leaving `text` unchanged; `cut()` does the same then removes the selection via the already-real `delete_selection()`; `paste()` reads the clipboard and calls the already-real `insert()`, reusing its existing "replace the active selection, if any" semantics rather than duplicating them. Both `copy()`/`cut()` are real no-ops (not errors) when no selection is active, matching every real text field's own standard behavior -- the clipboard's own prior content is left completely untouched. `PyClipboard::get_text`/`set_text` were widened from private to `pub(crate)` so `EditableText` can call them directly as plain Rust methods; no new public surface was added to `Clipboard` itself.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase14_step14_2/demo.py`, run via `maturin develop --release`: `copy()` leaves `text` unchanged; `cut()` removes the selection and lands the caret at its own start; `paste()` both inserts at a plain caret and correctly replaces an active selection; `copy()`/`cut()` with no selection leave the clipboard's own prior content completely untouched; and a real UTF-8 selection (`"café"`, a genuine 2-byte character) cuts and round-trips through the real clipboard byte-exact.

Not yet done: keyboard-shortcut detection (Ctrl+X/C/V) is deliberately not wired here -- matching this project's own established "layout-aware translation is the UI framework's job" stance for `KeyboardKey`'s own raw key codes; a real caller decides when to invoke these three methods from its own key-handling code.

### Step 14.3: Native File Dialogs (`tre.pick_file`/`pick_files`/`pick_folder`/`save_file`) -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Recommendation #5 from the readiness assessment: real native file dialogs via `rfd`, backed by the XDG desktop portal (`ashpd`) on Linux. Feasibility was verified in an isolated scratch crate before touching any real `Cargo.toml`, confirming this machine's live KDE Plasma session has both the GTK and KDE portal backend implementations registered, and that `rfd`/`tray-icon`/`muda` all link cleanly once `libxdo` -- an optional, X11-only accelerator-dispatch dependency of `tray-icon`/`muda`, irrelevant on this Wayland session -- is disabled via `default-features = false` (the initial feasibility pass hit a real `unable to find library -lxdo` linker error; no system package install was needed once that optional feature was turned off).

**A real bug the scratch-crate test almost missed**: the first feasibility pass only *constructed* an `rfd::FileDialog` builder and a `muda::Menu`, which built and linked cleanly with rfd's `tokio` feature -- but the very first real `pick_file()` call PANICKED at runtime: `"there is no reactor running, must be called from the context of a Tokio 1.x runtime"`. rfd's blocking API drives the portal's async calls itself via `pollster::block_on`, not a Tokio executor, so `tokio` was the wrong runtime feature despite compiling fine. Switching to `async-std` fixed it, confirmed by calling the real blocking API (not just building the request) in the same scratch crate across three repeated runs before changing the real dependency. This sharpens this project's own "verify feasibility in complete isolation" discipline (established for `shaderc`/Step 13.8, `arboard`/Step 14.1): isolated feasibility testing must call the real blocking/async API, not just link against it -- a build success and a link success are not proof a runtime call actually works.

New `crates/tre-platform/src/file_dialog.rs`: plain functions (`pick_file`/`pick_files`/`pick_folder`/`save_file`), not a struct -- unlike `Clipboard`, a file dialog has no persistent connection to hold; each call opens a fresh native dialog and blocks until the user responds. `tre-python` binds these directly as top-level `tre.pick_file` etc., releasing the GIL for the blocking call via `py.detach` (matching `PyHeadlessRenderer::submit_and_read_bgra`'s own established precedent for real blocking GPU work) -- a real native dialog can block on user interaction for an unbounded amount of real wall-clock time, and other Python threads must keep running while it does.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-platform` gains 1 new unit test: constructing the dialog builder with every real option set, without opening an actual interactive dialog). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase14_step14_3/demo.py`, run via `maturin develop --release`: malformed arguments raise a real `TypeError` before any dialog opens; the GIL is confirmed genuinely released during the blocking call (a background thread calls `pick_file()` while the main thread completes ~200 real, counted loop iterations in 2 real seconds); and the call is confirmed still genuinely blocked on a live portal round trip after that same 2-second wait (a silent no-op would have returned instantly instead).

**Real, disclosed limit on what an automated demo can prove**: a native file dialog needs a real human to click something in it -- there is no way to script a GTK/portal chooser window the way other demos in this project drive an in-process GPU renderer or a system service like the clipboard. Full interactive click-through (does the dialog actually return the path a human picked?) is real, separate manual verification a human running this demo at a real desktop must perform -- not something this automated test suite can do on its own.

### Step 14.4: System Tray + Native Menu (`tre.TrayIcon`/`tre.Menu`) -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** The second half of recommendation #5: a real system tray icon and native context menu, via `tray-icon` (which re-exports `muda`'s menu types under its own `menu` module, so `muda` is not a separate direct dependency), confirmed against this machine's real KDE Plasma session.

**A real, required integration constraint found during feasibility testing, not assumed**: on Linux, `tray-icon` needs a real GTK event loop pumped on the same thread that owns it -- `tre`'s own windowed rendering uses `winit`'s `EventLoop`, not GTK's. `tre_platform::tray` exposes `init()` (call once, before creating any `Menu`/`TrayIcon`) and `pump_events()` (call once per frame, alongside `PlatformConnection::poll_events()`, driving `gtk::events_pending()`/`gtk::main_iteration()` manually) rather than assuming tre's existing event loop already covers it. Feasibility was independently, manually confirmed by running the real binding under `dbus-monitor --session "interface='org.kde.StatusNotifierWatcher'"`: a real `RegisterStatusNotifierItem` D-Bus call and matching `StatusNotifierItemRegistered` signal from this machine's live `org.kde.StatusNotifierWatcher`, followed by a clean `StatusNotifierItemUnregistered` on process exit -- end-to-end proof the icon genuinely registers with the desktop shell, not just that the calls returned without error.

New `crates/tre-platform/src/tray.rs`: `init`/`pump_events`/`poll_events` as free functions (matching `tray-icon`'s own global-channel event model, not a per-instance callback) plus `Menu`/`TrayIcon` structs. `libxdo` (`tray-icon`'s own default feature, X11-only, used solely for `muda`'s *predefined* Copy/Cut/Paste/SelectAll menu items) is disabled via `default-features = false` -- irrelevant here since a real caller already has `Clipboard` (Step 14.1) and builds regular `MenuItem`s, and this machine's real session is Wayland anyway. `tre-python` binds these as `tre.tray_init`/`tray_pump_events`/`tray_poll_events` plus `tre.Menu`/`tre.TrayIcon`, both marked `unsendable` (matching `PyClipboard`'s own precedent for platform state that isn't safely `Send` -- GTK's thread-affinity requirement means these must stay pinned to whichever Python thread created them). `Menu` ownership is a real, single-owner transfer into `TrayIcon` at construction (matching `TrayIconBuilder::with_menu`'s own contract): the Rust side tracks this with an `Option<Menu>` that `.take()`s on consumption, and the Python side raises a real `ValueError` if a caller tries to keep using a `Menu` already attached to a `TrayIcon`.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-platform` gains 2 new unit tests: building a real menu with an item + separator via real GTK calls, and confirming `poll_events()` starts empty -- both run repeatedly across multiple `cargo test` invocations to rule out thread-affinity flakiness from GTK being initialized on a test-harness thread rather than a designated "main" thread; all runs passed consistently). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase14_step14_4/demo.py`, run via `maturin develop --release`: `tray_init()` succeeds against the real display server; a real `Menu` builds with a non-empty item id; `TrayIcon` creation consumes the `Menu` (reusing it afterwards raises `ValueError`); malformed icon data raises a real error instead of corrupting memory; and event polling runs cleanly with an honestly empty result (no human interaction occurred).

**Real, disclosed limit on what an automated demo can prove**: whether a human can actually see the tray icon and click it needs a human at a real desktop -- there is no way to script that click from this process, matching Step 14.3's own disclosed limit for file dialogs. Full click-through verification (does clicking a menu item produce the expected `TrayEvent.MenuItemClick` from `tray_poll_events()`?) is real, separate manual verification a human running this demo must perform.

**A real, previously-undisclosed bug found while investigating Phase 17 Step 17.1 (2026-09-11)**: `set_tooltip` is a genuine no-op on this machine's real Linux/GTK backend -- `tray-icon` 0.19.3's own `platform_impl/gtk/mod.rs` implements it as `pub fn set_tooltip<S>(&mut self, _tooltip: Option<S>) -> Result<()> { Ok(()) }`, silently discarding the argument and always returning success. This demo's own `check_tray_icon_creation_consumes_the_menu` originally treated `tray.set_tooltip("updated tooltip")` returning `Ok` as proof it "succeeded," which was never a valid inference on Linux -- corrected in the demo to assert only what's real (the call doesn't raise) and disclose the no-op directly, rather than implying a visible effect that was never actually checked.

Not yet done: predefined system menu items (`Copy`/`Cut`/`Paste`/`SelectAll`, `libxdo`-gated) are not exposed -- a real caller builds a menu with `Clipboard`-backed items instead. Tray icon image updates after creation (`set_icon`, distinct from `set_tooltip`) are also not yet exposed -- real, separate future work if a caller needs a dynamic tray icon (e.g. reflecting unread-count state). Both addressed in Phase 17 Step 17.1.

## Phase 14 Summary

All 4 sections of the GUI-readiness recommendations #4/#5 are complete: real system clipboard access (Step 14.1), cut/copy/paste wired into `EditableText` (Step 14.2), native file dialogs (Step 14.3), and system tray + native menu (Step 14.4). Each shipped as its own real, independently-verified step with a Rust-level test where feasible and a real Python demo with exact-value or honestly-disclosed-limit assertions, following this project's own established Phase 12/13 discipline throughout.

## Phase 15: Multi-Line Text Editing (Added 2026-09-11)

The GUI Readiness assessment's recommendation 9 ("multi-line text editing") was phrased as "line wrapping and vertical caret movement on top of the now-real single-line `EditableText`," implying multi-line/word-wrap *rendering* already existed and only *editing* was missing. Direct reading of `tre_engine::text::flatten_text` before writing this phase's plan disproved that -- it shaped every `Text` shape as one straight pen line with no `\n` handling, no line-height, and no per-line layout at all (an earlier, inaccurate claim in Step 13.5's own docs and the readiness artifact, now corrected there). Asked how far v1 should go, the project owner chose real greedy word-wrap by pixel width, not just hard-wrap on `\n` -- this phase builds that from scratch.

### Step 15.1: Real Multi-Line/Word-Wrap Text Rendering (`Text.wrap_width`) -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** New `tre_text::wrap::wrap_lines` (pure logic, unit-tested with synthetic glyph data, mirroring `caret.rs`'s own testing precedent) does the real line-breaking: `\n` always hard-breaks; word-wrap-by-width applies within each such segment only when a width is given (`None` degrades cleanly to hard-wrap-only, identical to `Some(f32::INFINITY)`) -- one algorithm handles both cases. `tre_engine::Text` gains `wrap_width: Option<f32>` (`None`, the default, keeps every existing caller's rendering byte-identical); `flatten_text` branches on it, calling `wrap_lines` and drawing each real line via `RenderingCanvas::draw_text` with that line's own glyph sub-slice, advancing `pen.y` by a real `line_height` between lines. `tre_text::caret` gains 2-D equivalents (`MultiLineCaretPosition`, `multiline_caret_positions`, `hit_test_2d`, `line_of`) for Step 15.2 to build on. `tre-python`'s `PyText` gains the matching `wrap_width` constructor param/field.

**Two real, previously-latent bugs found and fixed while building this**, neither part of the new code itself:

1. **`skrifa::Metrics::descent`'s real sign.** A signed OpenType value (this machine's default cascade font reports `ascent=1069, descent=-293`) -- negative, extending below the baseline. The first line-height formula written here, `ascent + descent + leading`, silently under-counted (adding a negative value shrinks the total), producing visibly overlapping lines in the real demo render. Confirmed via a real debug print against the real font before fixing to `ascent - descent + leading` -- exactly what the original Phase 15 plan had specified; the bug was the code drifting from its own plan while writing it, not a flaw in the plan's own reasoning.
2. **`rustybuzz` cluster offsets across bidi paragraphs**, in `tre_text::shape::shape_run` -- a real, pre-existing bug, only now exposed because nothing before this step ever shaped genuinely multi-paragraph (`\n`-containing) text and relied on absolute `cluster` values across paragraph boundaries. `unicode_bidi::BidiInfo` treats `\n` as a real paragraph separator, so `segment_runs` already splits multi-line text into one run per line -- but `shape_run` shaped each run through its own fresh `rustybuzz::UnicodeBuffer`, and `rustybuzz` reports `cluster` relative to *that buffer*, not the original full string. Every run after the first therefore reported `cluster` values reset near zero instead of its own true absolute byte offset -- diagnosed by rendering `"one\ntwo\nthree"` and finding the third line rendered as a stray 6px sliver instead of a full real line. Fixed by offsetting `info.cluster` by `run.text_range.start`. This also silently affected `caret_positions`/`EditableText.hit_test` for any pre-existing single-line caller that happened to pass multi-paragraph text through -- a real correctness fix, not scoped only to this step's own new code.

**Real, disclosed v1 scope limits**: LTR/single-script text only; whitespace-boundary word-wrap only (no UAX #14 hyphenation/punctuation/CJK-ideograph breaks); left-aligned only; a single word wider than `wrap_width` still gets its own line rather than being split further.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-text` gains 8 new `wrap_lines` unit tests plus 2 new 2-D caret-function tests, all passing; the pre-existing `shape`/`caret`/`fallback` suites still pass unchanged after the `shape_run` fix). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step) -- one additional `tre-atlas` failure observed once under full-workspace parallel load was confirmed flaky (passed in isolation and on a clean rerun), not a real regression. `demo/phase15_step15_1/demo.py`, run via `maturin develop --release` against real GPU hardware: `wrap_width=None` still renders as exactly one real ink band (byte-identical regression check); three `\n`-separated lines render as exactly 3 real ink bands with consistent measured spacing (44px at `px_size=32` on this machine's real font); a long sentence's own measured natural width, force-wrapped at ~40% of that width, produces multiple real lines whose spacing matches that same measured `line_height` exactly -- one consistent layout, proven against real rendered pixels.

Not yet done: multi-line *editing* (`EditableText` vertical caret movement, multi-line selection) -- Step 15.2, tracked separately.

### Step 15.2: Multi-Line Text Editing (`EditableText.wrap_width`, vertical caret movement, `selection_rects`) -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** The editing layer on top of Step 15.1's real multi-line/word-wrap rendering: `tre.EditableText(..., wrap_width=...)` gains `line_count()`, `hit_test_2d(x, y)`, `move_caret_up()`/`move_caret_down()` (real "sticky column" vertical movement), and `selection_rects()`. `insert`/`delete_selection`/`delete_backward`/`set_caret`/`set_selection`/`copy`/`cut`/`paste`/`handle_ime` are all deliberately unchanged -- they only ever touch the flat byte string, never line layout, so there is no duplicated editing logic for multi-line text; verified they still work correctly across a real multi-line selection. `PyEditableText` gains a private `compute_layout` (reshapes + rewraps `text` fresh each call, matching `hit_test`'s own "cheap, re-derive every time" precedent) shared by all four new methods. `selection_rects()` returns one `(x, y, width, height)` tuple per real visual line the active selection spans -- the exact design the original Phase 13 plan already called for.

**Two more real bugs found and fixed while building this**, both in brand-new code from this step:

1. **Reconstructing a pixel `y` to re-derive an already-known line.** The first `move_caret_vertically` routed through `hit_test_2d` by computing a target `y` at the target line's own midpoint and handing it back to `hit_test_2d`'s own `y -> line` resolution. `f32::round()` rounds half away from zero, so a `y` placed exactly at a line's own midpoint rounds to the line *below* it, silently skipping a real line on every other move. Fixed by searching the already-known target line's own stops directly for the nearest `x`, with no `y`/rounding involved.
2. **`hit_test_2d`'s own `y -> line` formula was wrong for genuine mid-line `y` values, independent of bug 1.** It used `.round()`, which finds the line whose *top* is nearest, not the line whose own span actually contains `y` -- misattributing roughly the bottom half of every line's real vertical extent to the line below it. Every existing test happened to use boundary-exact `y` values, so this went uncaught until a genuine mid-line `y` was added. Fixed to `.floor()`, plus a new regression test using real mid-line `y` values.
3. **A real, found line-tie-break inconsistency**, found via a genuine `down, down, up, up` repro landing one line short of where it started: at a byte offset sitting exactly at a line boundary, `tre_text::line_of` always prefers the earlier of the two tied lines -- correct for its own "what line is my caret visually on" contract, but wrong for repeated vertical movement landing exactly on such a boundary. An initial fix tied the tie-break to the current move's own direction, which was internally inconsistent between a `down` that arrives at a boundary and a following `up` leaving from it. The real, general rule: a caret at a boundary visually renders right before the *later* line's own first glyph -- so `move_caret_vertically` always prefers the later of any tied lines, for both directions, staying self-consistent across any real sequence of moves.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-text` gains 1 new `hit_test_2d` mid-line regression test). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). No native Rust unit tests exist for `move_caret_vertically`/the other new `PyEditableText` methods -- matching Step 13.5's own already-established constraint (the `extension-module` PyO3 feature doesn't link `libpython`, so native `cargo test` cannot call GIL-acquiring functions); real coverage lives in `demo/phase15_step15_2/demo.py`, run via `maturin develop --release`: `line_count()` on a 3-line hard-wrapped text is exactly 3; `move_caret_up`/`down` are real no-ops at the first/last line; a straight column-0 traversal down and back up an exact, hand-predicted byte-offset path (`0 -> 4 -> 10 -> 4 -> 0`, font-metric-independent); a sticky-column move from the shortest line's own end lands provably within the correct target line's own byte range; `selection_rects()` returns exactly 2 rects for a selection spanning 2 real lines and exactly 1 for a single-line selection; `cut`/`paste` round-trip a selection spanning a real line break byte-exact; and a real, finite `wrap_width` drives both `line_count()` and `move_caret_down()` correctly, reaching the last of N real wrapped lines in exactly N-1 steps.

**Real, disclosed scope limits**: no Shift+arrow selection-extension via vertical movement (`move_caret_up`/`down` always clear any active selection, matching `set_caret`'s own convention). All of Step 15.1's own `wrap_lines` scope limits (LTR-only, whitespace-boundary word-wrap, no hyphenation, left-aligned only) apply here unchanged.

### Step 15.3: Shift+Arrow Selection Extension -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Recommendation 12's first half: `tre.EditableText` gains `move_caret_left`/`move_caret_right` -- real, brand-new horizontal movement (this class had none before this step, only `set_caret` with a caller-supplied byte offset). `move_caret_up`/`move_caret_down` (Step 15.2) gain a new `extend: bool` parameter. All four share one real selection-anchor rule, a new `apply_caret_move`: `extend=False` always clears any active selection (matching `set_caret`'s own convention); `extend=True` starts a new selection anchored at the caret's own current position the first time it's used, then leaves that anchor untouched on every subsequent extend, in either direction -- the real Shift+arrow convention every desktop text field already has.

`move_caret_left`/`move_caret_right` are pure string operations -- UTF-8-char-boundary safe (matching `delete_backward`'s own safety), needing no shaping/layout at all, unlike `hit_test`/`move_caret_up`/`down`. Moving right past the end of one visual line crosses into the next for free, since this operates on the flat underlying string, not per-line.

**Real, disclosed v1 design choice, not an oversight**: a plain (non-extend) arrow always clears the active selection and moves one real character from the *current* caret -- it does not "collapse to the selection's own edge" the way some real editors do, matching `move_caret_up`/`down`'s own already-shipped convention rather than introducing an inconsistent second behavior for horizontal movement only.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). No native Rust unit tests exist for these new `PyEditableText` methods -- matching Step 13.5's own already-established constraint (the `extension-module` PyO3 feature doesn't link `libpython`); real coverage lives in `demo/phase15_step15_3/demo.py`, run via `maturin develop --release`: `move_caret_left`/`right` are real UTF-8-char-boundary-safe (tested against `"café"`'s own real 2-byte `é`) and real no-ops at the start/end of `text`; 5x `move_caret_right(extend=True)` from byte 0 anchors at 0 and selects exactly `"hello"`; reversing direction keeps the same anchor and shrinks the selection correctly, including shrinking all the way to an empty selection at the anchor and then a real no-op past it; a plain `move_caret_right()` afterward clears the selection and moves one real character from the current caret; `move_caret_down(extend=True)` x2 lands the caret exactly at the next line's own start (a real, zero-width selected segment there -- `selection_rects()` correctly reports 2 rects, not 3, until one more real character is selected on that third line); and `move_caret_up(extend=True)` afterward keeps the same anchor while shrinking the selection by one real line.

**Real, disclosed remaining scope**: word-by-word (Ctrl+arrow) and line-start/end (Home/End) jumps are real, separate follow-up work, not attempted here. All of Step 15.1's own `wrap_lines` scope limits still apply.

### Step 15.4: Word/Line Caret Jumps -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Recommendation 13's word/line jump half. `tre.EditableText` gains `move_caret_word_left`/`move_caret_word_right` (Ctrl+arrow) and `move_caret_line_start`/`move_caret_line_end` (Home/End). All four take the same `extend: bool` parameter Step 15.3 already established, sharing the identical `apply_caret_move` selection-anchor rule -- Ctrl+Shift+arrow and Shift+Home/End work for free.

Word jumps are real Unicode UAX #29 segmentation, via a new `tre_text::word` module wrapping `unicode-segmentation`'s `unicode_word_indices` (already a transitive dependency at 1.13.3; added directly). Pure string logic, no shaping needed. `unicode_word_indices` (not `split_word_bound_indices`) filters out pure whitespace/punctuation segments, so Ctrl+Right lands on the end of the next real word directly, skipping intervening punctuation/whitespace entirely.

Line jumps need real layout (via `compute_layout`), plus a new shared helper, `current_visual_line` -- the largest line index whose own `byte_range.start <= caret`. Derived directly from `lines` alone (no shaped positions needed), replacing the old "scan shaped positions, fall back to `line_of`" logic `move_caret_vertically` used before this step; at a real boundary tie it resolves the same way: toward the *later* tied line.

**A real, disclosed design subtlety caught before it became a bug**: `move_caret_line_end` does not simply use `lines[N].byte_range.end` -- that value includes any real trailing whitespace/newline the line's own wrap consumed, which is *also* line N+1's own start byte, the exact tie `move_caret_vertically` always resolves toward the later line. Landing `End` exactly there would make a following `move_caret_up`/`down` treat the caret as already on the next line. Fixed by trimming real trailing whitespace from the line's own text before measuring its end, landing right before the line's own `\n` instead -- at the real, disclosed cost that deliberately-typed trailing whitespace (rare) is skipped too. `move_caret_line_start` needed no such fix.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-text` gains 5 new `word.rs` unit tests -- real UAX #29 boundaries over plain strings, no synthetic glyph data needed). `--release` clean apart from the same 5 pre-existing, already-flagged `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase15_step15_4/demo.py`, run via `maturin develop --release`: Ctrl+Right/Left across `"the quick brown fox"` land at exact hand-computed word boundaries (`[3, 9, 15, 19]` forward, `[16, 10, 4, 0]` back), then real no-ops; word jumps correctly treat a real 2-byte `é` and a mid-word apostrophe as part of one word while skipping punctuation; Ctrl+Shift+Right extends a selection word by word from one fixed anchor; Home/End on a real hard-wrapped middle line land at its own exact content boundaries; and the tie-avoidance fix is proven directly: End on a non-last line then Down moves to the next line, not two lines down.

Not yet done: advanced line-breaking (real UAX #14 punctuation/hyphenation/CJK-ideograph break opportunities) -- recommendation 13's other half, tracked separately.

### Step 15.5: Advanced Line-Breaking (real UAX #14 via `unicode-linebreak`) -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Recommendation 13's line-breaking half. `tre_text::wrap::wrap_lines` is rebuilt on the real [`unicode-linebreak`](https://docs.rs/unicode-linebreak) crate, implementing actual [UAX #14](https://www.unicode.org/reports/tr14/) line-breaking -- replacing Step 15.1's own v1 hand-rolled whitespace/`\n`-boundary tokenizer with the real rule set (hyphens, punctuation attachment, CJK-ideograph boundaries, and more), the same "use the real algorithm, don't hand-roll UAX rule tables" precedent this crate's own `unicode-bidi`/`unicode-script` handling already follow. `WrappedLine`'s public shape and `wrap_lines`'s public signature are unchanged, so `flatten_text` (Step 15.1) and `compute_layout` (Step 15.2/15.4) needed no changes.

The algorithm walks `unicode_linebreak::linebreaks`'s break-opportunity stream (`Allowed`/`Mandatory`), greedily accepting each as a *candidate* to close the current line, retrying against an earlier candidate first if the current break's own content would overflow `max_width`.

**Two real design flaws were found and fixed before any code was written**, by hand-tracing the new algorithm against all 8 of `wrap.rs`'s pre-existing unit tests: (a) a `Mandatory` break must not close the line unconditionally at its own position if an earlier `Allowed` candidate should have closed it first to avoid overflow (traced concretely against `"aaaa bbbb"` @ `max_width=45`) -- fixed via one retry loop applied uniformly to both break kinds; (b) `unicode_linebreak` emits only one merged break when `\n` is a text's own final character, which would silently under-produce lines by one relative to this project's established "N trailing newlines -> N+1 lines" convention -- fixed via a post-loop `text.ends_with('\n')` special case.

**A third real bug was found only once machine-verified** (not caught by hand-tracing): `unicode_linebreak::linebreaks("")` yields nothing at all -- its "always at least one final break" guarantee does not hold for empty input, contradicting what its own docs implied. This broke the pre-existing `empty_text_produces_exactly_one_empty_line` test on the first real `cargo test` run against the new code. Fixed with a direct early return for `text.is_empty()`, confirmed against the crate's real behavior in an isolated scratch check before fixing, not by assumption.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-text` gains 4 new `wrap.rs` unit tests beyond the 8 pre-existing ones, which all still pass unchanged: a real UAX #14 hyphen-break test, a real UAX #14 forbidden-break-before-punctuation test, the empty-text fix's own regression test, and a test proving a `\n` glyph with a real nonzero advance is still excluded from both lines it borders). `--release` clean apart from the same 5 pre-existing, already-disclosed `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). `demo/phase15_step15_5/demo.py`, run via `maturin develop --release` against a real headless GPU renderer (reusing Step 15.1's own ink-band-scanning technique): `"wellknown-example"` (no whitespace anywhere, exactly one real UAX #14 break opportunity, the hyphen) forced to half its own natural width splits into exactly 2 real ink bands right at the hyphen -- something Step 15.1's own v1 could never do at any width; a full regression suite reproduces Step 15.1's own three checks verbatim and confirms identical real-pixel behavior for `wrap_width=None`, hard-wrap-on-`\n`, and whitespace word-wrap.

**Real, disclosed remaining scope**: still LTR/single-script only, still left-aligned only (both inherited, unchanged limits from Step 15.1); no hyphenation of an already-unbroken word (a single unbreakable token wider than `max_width` still gets its own line); covers *where* a line is allowed to break, not bidi-aware or justified layout.

## Phase 15 Summary

All five sections of GUI-readiness recommendations 9/12/13 are complete: real multi-line/word-wrap text rendering (Step 15.1, `Text.wrap_width`), multi-line editing (Step 15.2, `EditableText` vertical caret movement + `selection_rects`), Shift+arrow selection extension (Step 15.3, real horizontal movement + extend semantics for all four directions), word/line caret jumps (Step 15.4, real UAX #29 word boundaries + Home/End), and advanced line-breaking (Step 15.5, real UAX #14 via `unicode-linebreak`, replacing the v1 whitespace-only tokenizer). Along the way, this phase found and fixed eight real, previously-latent or newly-introduced bugs -- a signed-metric sign error, a cross-paragraph glyph-cluster offset bug in `tre_text::shape::shape_run` predating this phase entirely, a redundant-and-wrong pixel-to-line rounding indirection, a line-boundary tie-break inconsistency, a real tie between a line's own trailing-whitespace-inclusive end and the next line's start, a mandatory-break-overflow flaw and a trailing-newline-merge edge case in the new UAX #14 algorithm (both caught by hand-tracing before any code was written), and an empty-string edge case in `unicode-linebreak` itself contradicting its own documented guarantee (caught only once machine-verified) -- each diagnosed via real, concrete repros rather than assumption, and each disclosed in the relevant step's own README/IMPLEMENTATION.md entry rather than silently patched over.

## Phase 16: SDF-Based Soft Shadows (Added 2026-09-11)

### Step 16.1: SDF-Based Soft Shadows ("Shadow v2") -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** GUI Readiness recommendation 10: the real Dual-Kawase blur behind shadows (Phase 13 Step 13.4) has a fixed 4-hop chain (`BlurResources` in `crates/tre-rhi-vulkan/src/lib.rs`) with no tunable radius. Rather than reworking that fixed-hop-count RHI machinery into something variable-length -- a substantially bigger change than this recommendation's own text called for ("gated on the custom shader API") -- this step builds a real, continuously tunable SDF-based soft shadow entirely on the existing custom shader API (Phase 13 Step 13.8).

`tre_engine::shapes::CustomShaded` gains a real `params: [f32; 3]` field (`draw_custom_shaded_quad` previously hardcoded `[0.0; 3]` on every vertex); `PyCustomShaded` gains matching `param_x`/`param_y`/`param_z`. `VulkanDevice::create_custom_pipeline` now pairs a custom fragment shader with `SDF_ROUNDED_RECT_VERT` (which forwards `frag_params`) instead of `BINDLESS_TEXTURED_VERT` (which didn't) -- a strict superset, verified by rerunning `demo/phase13_step13_8/demo.py` unchanged, since a fragment shader may consume a subset of a vertex shader's outputs (legal SPIR-V interface matching).

`crates/tre-python/src/sdf_shadow.rs` (new): `tre.sdf_shadow_shader_source()` returns real GLSL reusing the exact, already-proven IQ rounded-box SDF `sdf_rect_styled.frag` already uses (`sd_rounded_box`, adapted to a single uniform radius), applying a continuous `smoothstep` falloff -- a real SDF soft edge, not a literal Gaussian blur (disclosed distinction: continuously tunable and free rounded corners via the reused SDF, but not an erf-based true Gaussian box-blur approximation). `tre.shadow_sdf_params(x, y, width, height, radius_px, blur_px)` converts real pixel values into the normalized `(radius_frac, sigma_x_frac, sigma_y_frac)` triple the shader expects -- normalized because `UiVertex.params` only has 3 float slots (growing it "would bloat every pipeline in the system," per `gpu_style.rs`'s own doc comment), not the 4 independent numbers (half-width, half-height, radius, sigma) a literal-pixel design would need.

**A real bug found only once this actually rendered** (not caught by design review): a fragment shader is never invoked outside the triangles it's rasterized on, so the first real render showed a hard cutoff at the `CustomShaded` quad's own edge regardless of `blur_px` -- there was no rasterized room outside the quad for the soft edge to extend into. Fixed by having `shadow_sdf_params` also return the quad enlarged by `blur_px` on every side (the same real margin concept `shadow_layer_bounds` already uses for the v1 blur path), with the shader rescaling `frag_uv` by `(1 + sigma_frac)` per axis so the logical box's own edge still lands at `d == 0` inside the larger quad.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug (`tre-python` gains 4 new `shadow_sdf_params` unit tests). `--release` clean apart from the same 5 pre-existing, already-disclosed `debug_assert!` failures. `demo/phase16_step16_1/demo.py`, run via `maturin develop --release` against a real headless GPU renderer: the same square shape at `blur_px` 0/8/16 produces real measured ink spreads of 0/6/13px past its own nominal edge -- strictly increasing, proving real continuous tunability, unlike the old blur's fixed hop count; `radius_frac=0` keeps a plain rectangle while `radius_frac=0.95` on the same square box visibly rounds its own literal corner pixel away to real background; and Phase 13 Step 13.4's own v1 blur-based shadow check, reproduced verbatim, still shows the same real blur gradient -- v2 is additive, not a replacement.

**Real, disclosed v1 scope limits**: single uniform corner radius, not per-corner (a 4th float this budget doesn't have); non-square-box radius/sigma are per-axis-normalized approximations, not literal pixel circles; the falloff is an SDF soft edge, not a true Gaussian convolution of a box.

## Phase 17: Tray/Clipboard Follow-ups (Added 2026-09-11)

### Step 17.1: Tray/Clipboard Follow-ups -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** GUI Readiness recommendation 11, both halves: predefined `Copy`/`Cut`/`Paste`/`SelectAll` menu items stay `libxdo`-gated (irrelevant on this Wayland session), and tray icon image updates after creation weren't exposed at all.

`tre_platform::tray::TrayIcon` gains `set_icon(rgba, width, height)` (wraps `tray_icon::Icon::from_rgba` + `TrayIcon::set_icon`) and `set_temp_dir_path(path)` (Linux-only, a real no-op elsewhere, wraps `tray-icon`'s own identical method) -- both mirrored on `PyTrayIcon`. On this machine's real Linux/GTK (`appindicator`) backend there is no in-memory icon API: each new icon is written to a real temporary PNG file on disk and the indicator is pointed at that path. `set_temp_dir_path` redirects where that file lands, letting a demo verify `set_icon`'s real effect automatically (reading the written PNG back and comparing pixels) instead of only checking the call didn't raise.

**A real, previously-undisclosed bug found while investigating this step, and fixed in Phase 14 Step 14.4's own docs/demo**: `set_tooltip` is a genuine no-op on this machine's real Linux/GTK backend -- `tray-icon` 0.19.3's own `platform_impl/gtk/mod.rs` implements it as `pub fn set_tooltip<S>(&mut self, _tooltip: Option<S>) -> Result<()> { Ok(()) }`, silently discarding the argument and always returning success. Step 14.4's own demo originally treated a successful `set_tooltip(...)` call as proof it "succeeded," which was never a valid inference on Linux -- corrected there (both `README.md` and `demo.py`) to assert only what's real (the call doesn't raise) and disclose the no-op directly, rather than silently leaving a false implication in already-shipped docs.

For the clipboard half, `tray-icon`'s own `Copy`/`Cut`/`Paste`/`SelectAll` predefined items remain disabled; the real answer the recommendation itself already gave -- build plain `Menu` items backed by `Clipboard` -- had never actually been demonstrated end to end. `demo/phase17_step17_1/demo.py` builds a real `Menu` with Copy/Cut/Paste items, gets back real distinct ids, and proves the exact id-based dispatch a real click handler would use correctly drives real `Clipboard.set_text`/`get_text` calls (including that an unrelated id dispatches to nothing) -- using `tre.TrayEvent.MenuItemClick`'s own direct Python constructibility (the same PyO3 "complex enum" pattern `PyMouseButton` already established) to drive the dispatch without a real click, the one part of this that genuinely can't be automated.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-disclosed `debug_assert!` failures. `demo/phase14_step14_4/demo.py` still passes unchanged after this step's own additive `TrayIcon` changes -- a real regression check. `demo/phase17_step17_1/demo.py`, run via `maturin develop --release` against this machine's real display server: two successive `set_icon` calls (blue, then green) each write a real, distinct PNG with the exact requested pixel content; malformed icon data raises a real error; and the Copy/Cut/Paste dispatch check round-trips real clipboard content correctly.

**Real, disclosed scope limits**: `set_icon`/`set_temp_dir_path`'s "writes a real file to disk" mechanism is specific to this machine's Linux/`appindicator` backend; other platforms' own `set_icon` implementations differ in mechanism behind the identical public API. Whether a human can actually see either update on their own desktop still needs a human at a real desktop, the same limit Step 14.4 already disclosed.

## Phase 18: Wire Accessibility Into tre-python (Added 2026-09-11)

GUI Readiness recommendation 7, the last real architectural gap: `tre-a11y` (a real, working AT-SPI2 bridge, Phase 5 Step 5.3.2) existed and was tested, but neither `tre-engine` nor `tre-python` depended on it. The recommendation's own long-standing framing -- that this needed "a real process-split" because `accesskit_unix`'s registration "never completes inside a process that also links real Vulkan/X11 shared libraries" -- was itself stale, carried forward across several phases after REVIEW.md finding #126's own later investigation directly disproved it. This phase corrects that record and wires the bridge in for real.

### Step 18.1: Fix the Real, Identified, Unimplemented Bug -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** `IMPLEMENTATION.md`'s own prior Step 5.3.3 entry stated outright that a real, identified fix was "not implemented or pushed": `ensure_accessibility_enabled` (in `crates/tre-a11y/tests/round_trip.rs` and `crates/tre-rhi-vulkan/examples/canvas_accessibility_verify.rs`) built its `org.a11y.Status` proxy on the a11y bus, but `at-spi-bus-launcher` owns `org.a11y.Bus`/`IsEnabled` on the SESSION bus (`g_bus_own_name(G_BUS_TYPE_SESSION, ...)`, confirmed by reading its real source) -- every `get_property` call silently failed against a nonexistent destination, and `.unwrap_or(false)` swallowed the failure as an indistinguishable "not enabled yet."

**Fixed** by building `status` on a real, separate session-bus connection in both files. Real, measured effect on this machine: `tre-a11y`'s round-trip test went from racing a 10s timeout (previously ~10.05s) to completing in 0.11s -- `IsEnabled` was true instantly once queried on the correct bus. Also corrected the stale "Vulkan/X11 same-process" theory, stated as settled fact in the doc comments of `canvas_accessibility_demo.rs`/`canvas_accessibility_verify.rs`, to match this entry's own account and `demo/phase5_step5_3_3/README.md`'s prior correction.

**Verified:** `cargo test -p tre-a11y` passes reliably and instantly across repeated runs; `demo/phase5_step5_3_3/run_canvas_accessibility_demo.sh` (the real two-process demo) still passes unchanged, closing Step 5.3 (5.3.1-5.3.3) in full for real -- its own binary now prints exactly that on success.

### Step 18.2: The Real Test This Project Had Never Actually Run -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** Nobody had ever tested whether a real Vulkan-linked process publishing its own AT-SPI2 tree actually works, because the Step 18.1 bug fully explained every CI symptom on its own. Two new example binaries in `tre-rhi-vulkan` answered this directly: `canvas_accessibility_single_process_check` (real headless Vulkan render + a real `A11yBridge::connect`/`publish`, all in ONE process) and `canvas_accessibility_single_process_check_verify` (a genuinely separate process querying the live registry while the first stays alive, publishing).

**Confirmed on this real desktop**: the separate verifier process found the app in the real registry and matched its `Component.GetExtents` exactly (`(10, 10, 60, 40)`), printing "SINGLE-PROCESS FEASIBILITY CONFIRMED." This settles the real question this phase exists to answer: the "process split" GUI Readiness recommendation 7 assumed was needed was never a genuine requirement for an *application* (the role `tre-python` fills) -- only, as `demo/phase5_step5_3_3/README.md` already disclosed, a real, separate reason to keep verification in its own process (matching real AT-SPI2 deployment practice). `canvas_accessibility_single_process_check` was removed after Step 18.3 shipped its own real demo covering identical ground through the actual Python binding; `canvas_accessibility_single_process_check_verify` was kept and is now reused as that demo's own real, independent verifier.

### Step 18.3: Wire A11yBridge Into tre-python -- Status: Complete (2026-09-11)

**Real, working, and verified end to end.** `crates/tre-engine/src/canvas.rs` gains `RenderingCanvas::accessibility_nodes()`, a plain getter over the field `tag_accessibility_node` already populates -- nothing outside the engine's own internal `FlattenedFrame` path read it before this. `crates/tre-python/src/canvas.rs` gains `PyAccessibilityNode` (a plain data mirror) and `Canvas.accessibility_nodes()`. New `crates/tre-python/src/a11y.rs`: `tre.A11yBridge(app_name, toolkit_name, toolkit_version)` binds `tre_a11y::A11yBridge::connect`/`.publish(nodes)` directly, in the SAME process -- no handoff file, no second binary in the real API -- `unsendable`, matching `PyTrayIcon`/`PyClipboard`'s own established precedent for platform-connection state.

**A real, disclosed constraint found while building the demo**: `A11yBridge` being `unsendable` means `publish()` must be called from the same Python thread that constructed it. An earlier draft published from a background `threading.Thread` and hit a real PyO3 panic ("is unsendable, but sent to another thread"). Fixed by keeping `publish()` calls on the main thread and running the real, separate AT-SPI2 verifier as a genuinely separate OS process (`subprocess.Popen`) instead -- the exact shape a real deployment needs anyway.

**A second real bug found while building the demo**: `renderer.render_canvas(canvas)` deliberately consumes the canvas's own recorded content (so the same `Canvas` can be reused next frame) -- calling `canvas.accessibility_nodes()` after `render_canvas` returned an empty list. Fixed by reading tagged nodes before rendering, the real pattern a caller must follow.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-disclosed `debug_assert!` failures. `demo/phase18_step18_3/demo.py`, run via `maturin develop --release`: a real Vulkan render confirmed at the tagged rect's own center pixel; `canvas.accessibility_nodes()` reports the exact tagged bounds/role; `tre.A11yBridge` connects in this same Vulkan-linked Python process; and a genuinely separate verifier process, spawned mid-publish, independently confirms the real, live AT-SPI2 `Component.GetExtents` matches exactly -- the first time this project's accessibility work has been proven end to end through the actual Python binding. `demo/phase5_step5_3_3/run_canvas_accessibility_demo.sh` still passes unchanged.

**Real, disclosed remaining scope**: Linux only; no real focus tracking; no tree hierarchy beyond the one synthesized root; no incremental/diffed `TreeUpdate`s -- all unchanged, pre-existing `tre_a11y::A11yBridge` scope limits. Windows UIA/macOS NSAccessibility remain out of scope, as they have been since Phase 5.

## Phase 18 Summary

GUI Readiness recommendation 7 is complete. The real root-cause investigation (REVIEW.md finding #126) had already found and disclosed the actual bug and disproved the "Vulkan/X11 same-process" theory in prior phases, but the fix itself had never been implemented, and nobody had drawn the real conclusion through for `tre-python`'s own integration. This phase: (1) implemented the real, previously-identified fix, closing Step 5.3 for real; (2) ran the real single-process feasibility test this project had never actually run, confirming empirically that no process split is needed for a real application; and (3) wired `tre.A11yBridge` into `tre-python` directly, single process, with a real Python demo proving it end to end. Two more real bugs were found and fixed along the way (an `unsendable`-thread violation and a consume-before-read ordering bug), both disclosed in Step 18.3's own entry above rather than silently patched.

## Phase 19: Focus / Keyboard Navigation (Added 2026-09-12)

The GUI Readiness assessment's own gap table marks "Focus / keyboard nav / tab order" fully missing -- the one remaining gap that's unambiguously tre's own responsibility (layout/theming, also missing, is deliberately scoped to pySilver per DESIGN.md). DESIGN.md's own architecture diagram has a box, never implemented, labeled "Spatial Hit Testing & Scene Tree Node Focus Manager," and `InputEvent` had no focus-change variant of any kind. Phase 18 Step 18.3's own "Real, disclosed remaining scope" paragraph already stated outright: "no real focus tracking" -- this phase closes that gap directly, keeping two distinct concepts separate rather than conflating them: real OS-level *window* focus, and in-app *widget* focus with real Tab-order traversal.

### Step 19.1: Real OS-Level Window Focus -- Status: Complete (2026-09-12)

**Real, working, and verified end to end.** `crates/tre-platform/src/winit_backend.rs`'s `Handler::window_event` match fell through to a catch-all `_ => {}` for `WindowEvent::Focused(bool)` -- real, silently dropped. Added `InputEvent::WindowFocused { window, focused }` to `crates/tre-engine/src/input.rs` (grouped after `Resized`, before `FileDropped`, matching DESIGN.md's own diagram grouping "...Focus, Resize"), wired it in `winit_backend.rs`'s match, and mirrored it into `crates/tre-python/src/input.rs`'s `PyInputEvent` complex enum plus its `From<InputEvent>` impl.

**Verified:** new `#[cfg(test)] mod tests` in `input.rs` (none existed there before) proves a pushed `WindowFocused` drains unmodified and, unlike `PointerMoved`, is never coalesced. `crates/tre-rhi-vulkan/examples/input_demo.rs`'s own exhaustive `InputEvent` match needed one new arm to keep compiling -- a real, expected consequence of widening a non-`#[non_exhaustive]` enum, fixed alongside.

### Step 19.2: `FocusManager` -- Real In-App Widget Focus and Tab Order -- Status: Complete (2026-09-12)

**Real, working, and verified end to end.** New module `crates/tre-engine/src/focus.rs`: `FocusableNode { node_id: AccessibilityNodeId, x, y, width, height, tab_index: Option<i32> }` (one per-frame-tagged focusable widget, reusing `AccessibilityNodeId` as identity rather than inventing a second parallel node-id system for the same widget tree) and `FocusManager` (persistent, NOT per-frame-cleared, in-memory state -- pure logic, no OS handle, unlike `Clipboard`/`TrayIcon`/`A11yBridge`'s own `unsendable` platform-connection precedent): `new()`, `focused()`, `set_focus(Option<AccessibilityNodeId>)`, `focus_next(&[FocusableNode])`, `focus_previous(&[FocusableNode])` (both wrap around).

**Tab order follows the real HTML `tabindex` convention** (WHATWG HTML Standard Section 6.6.7) rather than an invented rule: nodes with a positive `tab_index` come first, ascending, ties broken by input order; then nodes with `None`/`Some(0)`, in input order (this engine's own "geometry order" stand-in, since it has no widget tree to derive real document order from -- the same "flat list, not a tree" boundary `AccessibilityNode` already draws); nodes with a negative `tab_index` are excluded from `focus_next`/`focus_previous` entirely but remain directly targetable via `set_focus` -- HTML's own "focusable, but skip me during sequential Tab navigation" escape hatch. Citing an existing, external, non-tre-specific standard beats inventing bespoke tie-break semantics nobody would recognize.

`crates/tre-engine/src/canvas.rs`: factored `tag_accessibility_node`'s own original inline corner-transform + AABB logic into a shared `transform_bounds` helper (pure refactor -- the existing rotation test still passes unchanged), then added `tag_focusable`/`focusable_nodes` using it, mirroring `tag_accessibility_node`/`accessibility_nodes` exactly. New field `focusable_nodes: Vec<FocusableNode>` on `RenderingCanvas`, cleared in `reset()` alongside `accessibility_nodes.clear()`.

**Deliberate, disclosed scope cut**: `focusable_nodes` is NOT threaded through `flatten()`/`FrameArena`/`SubCanvas` merging the way `accessibility_nodes` is -- that plumbing exists for multi-threaded recording, and no real caller records focusable nodes off the main thread today. `focusable_nodes()` is read directly off the root `RenderingCanvas`, before `render_canvas()` consumes it -- the same "read before render" rule Step 18.3 already established and disclosed for `accessibility_nodes()`.

**Verified:** a self-contained `#[cfg(test)] mod tests` in `focus.rs` covers empty-list `focus_next` returning `None`, the full mixed positive/`None`/negative `tab_index` ordering example, wraparound in both directions, `set_focus` on an excluded negative-`tab_index` node, and recovery when the focused node was removed. A new canvas-level test in `lib.rs`'s shared test module confirms `tag_focusable` under an active rotation transform matches `tag_accessibility_node`'s own existing rotation-test bounds, proving `transform_bounds` is behavior-preserving. `cargo test -p tre-engine`: 172 passed, 0 failed.

### Step 19.3: `tre.FocusManager`/`Canvas.tag_focusable` Wired Into `tre-python` -- Status: Complete (2026-09-12)

**Real, working, and verified end to end.** New `crates/tre-python/src/focus.rs`, mirroring `a11y.rs`'s own `register()` pattern: `PyFocusableNode` (plain `#[pyo3(get)]` data mirror, matching `PyAccessibilityNode`) and `PyFocusManager` (`#[pyclass(name = "FocusManager")]`, deliberately **not** `unsendable` -- pure logic, no platform handle) with `focused()`, `set_focus(node_id)`, `focus_next(nodes)`, `focus_previous(nodes)`. Added `Canvas.tag_focusable(node_id, x, y, width, height, tab_index=None)`/`Canvas.focusable_nodes()` to `crates/tre-python/src/canvas.rs`. Registered in `crates/tre-python/src/lib.rs`: `mod focus;` alphabetically between `file_dialog;` and `font;`; `focus::register(m)?;` right after `canvas::PyAccessibilityNode`/`a11y::register(m)?` and before `renderer::PyHeadlessRenderer` -- the same accessibility-adjacent grouping those two already establish.

### Step 19.4: Real End-to-End Python Demo -- Status: Complete (2026-09-12)

**Real, working, and verified end to end.** `demo/phase19_step19_4/demo.py`, run via `maturin develop --release` against a real windowed renderer (`WindowFocused` only fires on a real OS window, not headless): tags three real rects in one frame (`tab_index=None`, `tab_index=1`, `tab_index=-1`), reads `focusable_nodes()` before `render_canvas()` consumes the canvas, asserts the exact `focus_next`/`focus_previous` id sequences including wraparound with the `tab_index=-1` node never appearing, calls `set_focus` directly on that same excluded node and confirms it took effect, and observes a real `WindowFocused(focused=True)` event via `poll_events()` on this real desktop.

**Real, disclosed remaining scope**: this sandbox has no input-injection or window-management tool (no `xdotool`/`wmctrl` -- confirmed absent) to synthesize another app stealing focus, so only the window-*gains*-focus half of `WindowFocused` is asserted unattended; fully automating the window-*loses*-focus half needs a second real window/process, or a human alt-tabbing away and back during a manual run -- matching Phase 17's own "a human still needs to look at some real UI results" disclosure discipline. `tre-engine` tracks no modifier-key state and parses no raw key codes for Tab -- a real UI framework recognizes Tab/Shift+Tab itself, matching the established boundary held since `InputEvent::KeyboardKey` and Phase 15's `EditableText`.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo build --workspace --all-targets`/`cargo test --workspace` all clean in debug. `--release` clean apart from the same 5 pre-existing, already-disclosed `debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`) -- no new release-mode failure. `demo/phase18_step18_3/demo.py` re-run unchanged: still passes, confirming the `transform_bounds` refactor is behavior-preserving.

## Phase 19 Summary

The GUI Readiness assessment's last unambiguously-tre-owned gap -- "Focus / keyboard nav / tab order," fully missing -- is closed. This phase: (1) wired real OS-level window focus through as `InputEvent::WindowFocused`, previously silently dropped by the winit backend; (2) built a real `FocusManager` with Tab-order traversal following the well-known HTML `tabindex` convention rather than an invented rule, reusing `AccessibilityNodeId` as node identity instead of a second parallel id system, and factoring the shared transform-bounds math out of `tag_accessibility_node` rather than duplicating it for `tag_focusable`; and (3) wired both into `tre-python` with a real end-to-end Python demo against a real window, disclosing exactly what could and couldn't be proven unattended rather than overclaiming automated coverage of the window-loses-focus path.
