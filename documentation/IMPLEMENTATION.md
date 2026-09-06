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
polygon check.

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
