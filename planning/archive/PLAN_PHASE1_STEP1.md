# Plan: Phase 1, Step 1 -- Linux Native Windowing, Multi-Window RhiDevice, Headless Mode

## Scope decision (confirmed with project owner 2026-09-05)

IMPLEMENTATION.md Step 1.1 asks for native windowing on Windows, Linux, and macOS together. This machine can only build, run, and verify Linux -- and Phase 0 already demonstrated why "it compiles" is not "it works" for platform/graphics code (three real bugs were only found by running it). So this step is scoped to **Linux only** (both Wayland, the primary target, and X11 via XWayland -- both are genuinely testable on this machine, confirmed via `DISPLAY=:0` and an active XWayland socket). Windows and macOS native bridges become their own later steps (Phase 1 Step 2, Step 3), written when there's a way to actually run them.

## Goal

Replace Phase 0's `winit`-based windowing (an explicitly-temporary expedient) with real native Linux windowing, and generalize the Vulkan backend from Phase 0's implicit single-window assumption to genuine multi-window sharing of one `RhiDevice` -- per ARCHITECTURE.md Section 2.1's "Global RhiDevice, per-window RhiSwapchain" model. Add headless (no-window) rendering per DESIGN.md Section 4.3.

## Tasks

1. **New `tre-platform` crate.** ARCHITECTURE.md Section 1 names "Platform & Event Layer" as its own top-level architectural domain, distinct from the core engine and the RHI -- the original workspace scaffold didn't create a crate for it. Add one now: owns native window creation and (later, Step 1.2) the input event pump. Depends on `wayland-client`/`wayland-protocols` (Wayland) and `x11rb` (XCB/X11 fallback), producing `raw-window-handle` values that plug into Phase 0's existing `ash_window`-based surface creation unchanged.

2. **Native Wayland window creation** (primary path): `wl_compositor`/`xdg_wm_base`/`xdg_surface`/`xdg_toplevel` -- open a window, handle close requests, handle `xdg_toplevel::configure` for resize. Verified by actually running on this machine's Wayland session.

3. **Native X11 window creation** (fallback path, via XCB/`x11rb`): equivalent window lifecycle for X11/XWayland sessions. Verified by forcing an XCB connection against this machine's XWayland socket.

4. **Multi-window `RhiDevice` restructuring** in `tre-rhi-vulkan`: wrap `VulkanDevice` for shared ownership (`Arc`) across multiple `VulkanSwapchain` instances, one per open window, all created against the same physical device/queue family chosen once at startup.

5. **Resize / DPI handling** (Step 1.1 task 3): recreate the swapchain when a window reports a new size (`xdg_toplevel::configure` / X11 `ConfigureNotify`); report the Wayland `wl_surface` preferred buffer scale (or X11 equivalent) as a scale factor -- the engine reports it, consuming it into a "global UI scale factor" is the UI framework's job, out of scope here.

6. **Headless mode** (Step 1.1 task 4): a `HeadlessSwapchain` implementing the existing `RhiSwapchain` trait unchanged -- backed by a plain `VkImage` array with `VK_IMAGE_USAGE_TRANSFER_SRC_BIT` instead of a real `VkSwapchainKHR`, whose `present()` reads the image back to a CPU buffer instead of calling `vkQueuePresentKHR`. Proves the Phase 0 trait design generalizes to a fundamentally different backing (DESIGN.md Section 4.3's "zero-window execution").

## Demo plan (`demo/phase1_step1/`)

- **Multi-window demo:** opens two native windows side by side (testing both the Wayland and X11/XCB code paths, e.g. one of each, or both on whichever is the running session with an env var to force the other), each drawing its own independent rounded rect through the one shared `RhiDevice` -- visibly proving multi-window sharing works, not just a relabeled single-window path.
- **Headless demo:** renders the same scene with no window at all and writes the readback buffer to a PNG file, provable by opening the PNG -- a more automatable verification path than Phase 0's manual screenshot.

## Verification plan

Same discipline as Phase 0, not weakened for being "just windowing":
- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace.
- Both native windowing paths (Wayland and XWayland) actually run on this machine, with `VK_LAYER_KHRONOS_validation` enabled, zero errors.
- Multi-window demo run with validation enabled -- specifically watching for the shared-atlas-style concurrency hazards ARCHITECTURE.md Section 2.3 already anticipates for a future multi-window atlas (not yet built, but the `Arc<VulkanDevice>` sharing model introduced this step is the foundation it depends on).
- Headless demo's output PNG inspected to confirm correct pixel content.
- Resize actually exercised (drag a window edge) and confirmed not to crash or corrupt the image.

## Explicitly out of scope for this step

- Windows (Win32/DXGI) and macOS (AppKit/Metal) native bridges -- later steps.
- The input event queue / `InputEvent` translation / SPSC ring buffer -- IMPLEMENTATION.md Step 1.2, a separate step. This step only handles the minimum window-lifecycle events (close, resize/configure) needed to make windowing itself work.
- Real DPI-scale *consumption* (global UI scale factor) -- a UI-framework concern.
- Atlas/font/PSO caching on `RhiDevice` -- Phase 4/6's job; this step's "singleton" work is about the ownership/sharing model, not building those subsystems.
