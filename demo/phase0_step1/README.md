# Demo: Phase 0, Step 1 -- Walking Skeleton

What this shows: a real window, opened by real code, cleared to a color by a real Vulkan device, with one shape drawn through the actual `Canvas` API -- the full `Canvas::draw_rounded_rect` -> intermediate representation -> `RhiCommandBuffer::draw_indexed` -> swapchain present pipeline described in the project's architecture docs, working end to end for the first time.

It is deliberately minimal: one window, one backend (Vulkan), one shape, no multi-threading, no real batching. See `planning/archive/PLAN_PHASE0_STEP1.md` for what this step was scoped to do and why.

## Prerequisites

- The Rust toolchain pinned in `rust-toolchain.toml` (installed automatically by `rustup` on first use).
- `glslc` (part of the Vulkan SDK / `shaderc`) on `PATH` -- compiles the placeholder shaders at build time.
- A working Vulkan 1.2+ driver and a display to open a window on.

## Run it

```bash
./demo/phase0_step1/run.sh
```

Or directly:

```bash
cargo run -p tre-rhi-vulkan --example walking_skeleton
```

You should see a window titled "tre walking skeleton (Phase 0)" containing an amber rounded rectangle on a dark background. It presents 120 frames (printing a line to the terminal every 60) and then exits on its own.

## Options

- **Run longer** (e.g. to take a screenshot before it closes): `TRE_WALKING_SKELETON_FRAMES=100000 cargo run -p tre-rhi-vulkan --example walking_skeleton`, then close the window yourself (Ctrl+C in the terminal, or the window's close button).
- **Verify with the Vulkan validation layer** (recommended -- this is how the bugs in `LOG_PHASE0_STEP1.md` were actually found):
  ```bash
  VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation cargo run -p tre-rhi-vulkan --example walking_skeleton
  ```
  A clean run prints no `Validation Error` lines.

## What to look for

- The window opens and shows the amber rectangle without a crash.
- The terminal prints `frame 60 presented` and `frame 120 presented`, then `walking skeleton exited cleanly`.
- With the validation layer enabled, no `Validation Error` lines appear at any point, including at shutdown.
