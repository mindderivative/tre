# Demo: Phase 1, Step 1 -- Linux Native Windowing, Multi-Window RhiDevice, Headless Mode

Two demos, showing the two halves of what this step built:

## 1. Multi-window RhiDevice sharing

```bash
./demo/phase1_step1/run_multi_window.sh
```

Opens two independent native windows (Wayland or X11, whichever this session uses), each with its own swapchain, pipeline, and vertex/index buffers, but **sharing one `VulkanDevice`** -- ARCHITECTURE.md Section 2.1's "Global `RhiDevice`, per-window `RhiSwapchain`" model, for real. Window A draws an amber rect, window B draws a blue one.

**Note on window placement:** Wayland's `xdg-shell` protocol gives clients no way to request a screen position for a top-level window (X11 does; Wayland deliberately doesn't). Depending on your compositor, the two windows may open stacked exactly on top of each other. This is expected -- drag one aside to see both. It does not affect what the demo is proving: check the terminal output (`window A open, window B open` every 60 frames) and, if you enable the Vulkan validation layer (below), the absence of any errors -- both are real, independent evidence that two windows are genuinely sharing one device and rendering correctly, regardless of how the window manager happened to stack them.

**Verify with the Vulkan validation layer:**
```bash
VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation cargo run -p tre-rhi-vulkan --example multi_window
```

## 2. Headless (zero-window) rendering

```bash
./demo/phase1_step1/run_headless.sh
```

Renders the same one-rect scene with **no window, no display server connection for rendering purposes** -- `HeadlessSwapchain` implements the exact same `RhiSwapchain` trait `VulkanSwapchain` does, backed by a plain GPU image instead of a real swapchain. The rendered image is read back to CPU memory and written to `demo/phase1_step1/headless_output.png`.

This is the more automatable of the two proofs: open the PNG (or diff it against a known-good reference) instead of eyeballing a live window. You should see a green rounded rect on a dark background, matching the `Canvas::draw_rounded_rect` call in `crates/tre-rhi-vulkan/examples/headless.rs`.

**Verify with the Vulkan validation layer:**
```bash
VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation cargo run -p tre-rhi-vulkan --example headless
```

## Prerequisites

Same as `demo/phase0_step1/`: the pinned Rust toolchain, `glslc` on `PATH`, a working Vulkan 1.2+ driver. The multi-window demo additionally needs a Wayland or X11 display server (this step is Linux-only -- see `planning/archive/PLAN_PHASE1_STEP1.md` for why). The headless demo needs a GPU but no display server at all beyond what's needed to bootstrap the device (see `planning/archive/LOG_PHASE1_STEP1.md` for the one known wrinkle there).
