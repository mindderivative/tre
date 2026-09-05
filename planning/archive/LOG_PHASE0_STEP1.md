# Log: Phase 0, Step 1 -- Walking Skeleton

*Backfilled retroactively 2026-09-04. Full detail lives in documentation/REVIEW.md's "Phase 0 Implementation" section (findings #40-43); this is the step-scoped summary.*

## Doc gaps found

### ARCHITECTURE.md Section 6's RHI trait sketch was incomplete
`RhiBuffer`, `RhiTexture`, `RhiPipelineState`, and `RhiSwapchain` were referenced as `&dyn Rhi*` trait-object parameters throughout `RhiDevice`/`RhiCommandBuffer` but never given their own method signatures. Discovered immediately on attempting to implement a concrete Vulkan backend against the sketch.

**Resolution:** defined all four traits in `tre-engine`, using an opaque-`u64`-handle pattern (a Vulkan handle reinterpreted via `ash::vk::Handle::as_raw`/`from_raw`) so concrete implementations exchange state through trait-method calls and return values, never through `std::any::Any` downcasting -- which TECHNICAL.md Section 9.1 explicitly bans from the per-frame path. ARCHITECTURE.md Section 6 updated in place with the real, validated definitions.

### `begin_frame`/`submit_and_present` had no error return type
Contradicted DESIGN.md Section 2.6, which requires device-loss/swapchain-out-of-date conditions to be "surfaced as a recoverable error" at exactly those calls.

**Resolution:** both now return `Result<_, EngineError>`.

## Bugs found

### A `u32` RGBA hex literal doesn't pack the way it visually reads
`0xE0_A0_40_FFu32` stored little-endian places `0xFF` at the lowest memory address, not `0xE0` -- backwards for what an `R8G8B8A8`-format vertex attribute expects. Caught visually: a verification screenshot showed a pink rectangle where an amber one was requested.

**Resolution:** added `tre_engine::rgba8(r, g, b, a) -> u32`, packing correctly via `u32::from_le_bytes`. Locked in with a unit test that reads the packed value back through `to_le_bytes` rather than asserting a specific numeric constant, so the test still documents intent to a future reader.

### Three real Vulkan object-lifecycle bugs (found only by running the code)
1. **Freeing a command buffer immediately after submitting it**, while its GPU work was still pending -- undefined behavior per the Vulkan spec (`VUID-vkFreeCommandBuffers-pCommandBuffers-00047`), caught immediately by the validation layer. Fixed by allocating one command buffer once and reusing it every frame (`vkResetCommandBuffer`) instead of allocate-then-free per frame.
2. **Reusing one `render_finished` semaphore across every frame.** The CPU-side fence only covers the queue submit's completion, not the separate, asynchronous present operation's -- a shared semaphore could still be referenced by a not-yet-retired present (`VUID-vkQueueSubmit-pSignalSemaphores-00067`). Fixed with one `render_finished` semaphore per swapchain image.
3. **Struct field drop order destroying dependencies before dependents, twice.** Rust drops a struct's own fields in *declaration* order, not reverse (the opposite of local-variable drop order) -- an easy thing to get backwards. Surfaced first as validation errors (device destroyed while buffers/pipeline still referenced it), then as a SIGSEGV inside `libwayland-client.so` (a window's surface destroyed before the swapchain built on it). Root-caused via `coredumpctl gdb`'s backtrace, not guessed. Fixed by reordering both affected structs so dependencies are declared -- and therefore dropped -- before what depends on them, plus an explicit `vkDeviceWaitIdle` in a custom `Drop` impl before any per-field cleanup runs.

## Verification performed

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace`: all clean.
- Ran against real hardware (AMD Radeon 890M, RADV/Mesa driver) with `VK_LAYER_KHRONOS_validation` enabled: 120 frames presented, zero validation errors, clean exit.
- Screenshot (via `spectacle`) confirmed the rendered rectangle's color and position matched the `Canvas::draw_rounded_rect` call that produced it.
