# Plan: Phase 0, Step 1 -- Walking Skeleton

*Backfilled retroactively 2026-09-04 when the Phase/Step process was established; this plan reflects what was actually executed for Phase 0, reconstructed from IMPLEMENTATION.md's Phase 0 section and the work log.*

## Goal

Per IMPLEMENTATION.md Phase 0: stand up the thinnest possible vertical slice through `Canvas -> IR -> RHI -> pixel on screen`, using a single backend (Vulkan only) and single-threaded execution, to validate the shape of that contract before Phases 1-5 invest deeply in platform abstraction, memory pools, geometry, typography, and multi-threaded recording on top of assumptions that might be wrong.

## Tasks

1. Stand up a single-backend (Vulkan only), single-threaded, minimal `RhiDevice`/`RhiSwapchain` pair -- enough to open one window and clear it to a color.
2. Implement a stub `RenderingCanvas::draw_rounded_rect` that records exactly one `UiDrawCommand` into a fixed-size array (no ring buffer, no arena, no multi-threading yet).
3. Implement a trivial pass-through of the sort/flatten stage for the single-command case (no real radix sort needed yet).
4. Wire that one command through `RhiCommandBuffer::draw_indexed` to the swapchain and present it.
5. Confirm the full loop -- `Canvas` call in, pixel out -- runs and holds a stable frame time.

## Approach decided during execution

- **Windowing:** `winit` + `ash-window`, explicitly as a Phase-0-only expedient. Phase 1 (Step 1.1) replaces this with the documented native per-platform bridges (Win32/Wayland/Cocoa via `windows-rs`/`wayland-client`/`objc2`).
- **Shaders:** placeholder flat-color GLSL (via `glslc`), not the documented HLSL/DXC pipeline -- that toolchain and the real analytical SDF rounded-rect formula are IMPLEMENTATION.md Phase 3.2's job, out of Phase 0's scope. Compiled via a `build.rs` step matching TECHNICAL.md Section 9.3's "compile at build time, never at runtime" rule.
- **Synchronization:** deliberately simple -- one frame in flight, a full fence wait at the start of every `begin_frame`, rather than TECHNICAL.md Section 3.1's triple-buffered ring arenas (Phase 2's job).
- **RHI trait gaps:** ARCHITECTURE.md Section 6 referenced `RhiBuffer`/`RhiTexture`/`RhiPipelineState`/`RhiSwapchain` as `&dyn Rhi*` parameters but never defined them, and gave `begin_frame`/`submit_and_present` no error-return type. Both had to be resolved to make Phase 0 implementable at all -- see LOG_PHASE0_STEP1.md.

## Verification plan

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace` all clean.
- Run the walking skeleton example against real GPU hardware with `VK_LAYER_KHRONOS_validation` enabled -- zero validation errors required, not just "it didn't crash."
- Screenshot the running window to visually confirm the rendered output matches the `Canvas::draw_rounded_rect` call that produced it (position, size, color).

## Outcome

Complete. See LOG_PHASE0_STEP1.md for what was actually found during implementation, and documentation/REVIEW.md's "Phase 0 Implementation" section (findings #40-43) for the full detail.
