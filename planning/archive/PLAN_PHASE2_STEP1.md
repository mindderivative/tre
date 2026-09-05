# Plan: Phase 2, Step 1 -- Zero-Allocation Ring Buffers & Transient Pools

## Scope decision (confirmed with project owner 2026-09-05)

IMPLEMENTATION.md orders Phase 2 as Step 2.1 (Vulkan bindless upgrade + DX12 + Metal backends) before Step 2.2 (ring buffers/transient pools). DX12 and Metal can't be built or verified for real on this Linux machine. **This step tackles Step 2.2 first** -- fully engine-side/Vulkan-backed memory-subsystem work, entirely verifiable here, and a prerequisite several later phases (text/atlas/SVG) will need. Step 2.1 is deferred to its own future step; when it happens, DX12/Metal will be deferred entirely (empty placeholder crates) rather than writing unverifiable code, mirroring Step 1.1's Windows/macOS precedent.

**Frame-in-flight model.** TECHNICAL.md Section 3.1 requires waiting on frame $N-3$'s fence before reusing that ring-buffer segment -- this implies genuine overlapping multi-frame-in-flight GPU/CPU execution. Phase 0 built only a single in-flight fence (fully synchronous: every frame waits for the GPU before returning, zero overlap). Moving to real 3-deep pipelining now would be a substantial, risky rewrite of the existing frame-submission code, touching every example. **Scope decision:** build the ring buffer with 3 real segments and correct per-segment fence tracking (structurally ready for real overlap, same "no redesign needed" precedent as Phase 1's SPSC ring buffer), but keep frame submission itself synchronous for now. Genuine overlapping submission is deferred to its own future step once there's a real workload to profile it against.

**Transient pool growth-on-miss.** DESIGN.md Section 2.6 says a correctly-sized target is "grown into the pool asynchronously for subsequent frames" after a miss. Implemented as: queue the needed bucket and allocate it at the *start* of the next frame (before the zero-alloc guard would engage for that frame), not via a background thread -- satisfies "no allocation during the render tick" without introducing threading this step doesn't otherwise need (Step 2.3's GC gets the real async thread later).

**Bucket sizing.** Width and height each round up independently via `u32::next_power_of_two()`. Simple, idiomatic for GPU texture pools, keeps the bucket count small for typical UI sizes.

## Doc fix found while planning

TECHNICAL.md Section 3.4 describes the (Phase 9, not this step's) zero-allocation guard as overriding "`operator new`/`operator delete` (and `malloc`/`free`)" -- leftover C++ phrasing from before the Rust migration. Rust's equivalent is a custom `#[global_allocator]` wrapper, not operator overriding. Will fix wording only; the guard's actual implementation is Phase 9 (Step 9.2) scope, not this step's.

## Goal

Replace `tre-rhi-vulkan`'s current `unimplemented!()` stubs for `create_dynamic_ring_buffer`/`acquire_transient_target`/`release_transient_target` with real implementations per TECHNICAL.md Sections 3.1/3.2/3.4 and IMPLEMENTATION.md Step 2.2, plus the `Canvas::push_layer`/`pop_layer` API and its balance assertion.

## Tasks

1. **`TextureFormat` in `tre-engine`.** A minimal engine-level enum (`Bgra8Srgb`, `Rgba16Float`, matching TECHNICAL Section 6.1's SDR/HDR swapchain formats) added to `RhiDevice::acquire_transient_target`'s signature (currently width/height only), so the pool can key on `(Width, Height, Format)` as Section 3.2 requires.

2. **Real `VulkanDevice::create_dynamic_ring_buffer`.** A host-visible/host-coherent `VkBuffer` (`HOST_VISIBLE | HOST_COHERENT`), 16-32MB per TECHNICAL 3.1, persistently mapped once (never per-frame mapped/unmapped), divided into 3 logical segments (one per frame-in-flight slot). Per-allocation offsets within a segment are 256-byte aligned (TECHNICAL 3.1's RHI dynamic-offset alignment). Exposes bump-allocation into the current segment.

3. **Real fence-gated segment reuse.** One fence per ring-buffer segment; writing into segment $N$ first waits on segment $N$'s own fence (last GPU work that read it). Under this step's synchronous-submission scope decision this wait is structurally correct but usually a no-op today, since nothing yet overlaps 3 frames deep.

4. **Real `acquire_transient_target`/`release_transient_target`.** An `FxHashMap<(u32, u32, TextureFormat), Vec<VulkanTexture>>` pool (checked-out/free lists) with power-of-two bucket rounding. A genuine miss borrows the next-larger existing bucket for the current frame (rendering into a sub-rect) and queues the correctly-sized bucket to be created at the *next* frame's start.

5. **`Canvas::push_layer`/`pop_layer`.** Emits `CommandType::PushLayer`/`PopLayer` IR commands and acquires/releases a transient target from the pool. Debug-build-only depth counter (per `Canvas`) asserting exactly zero at `flatten()` -- IMPLEMENTATION Step 2.2 task 5's balance assertion.

6. **New demo proving zero-allocation steady state.** Repeated `push_layer`/`pop_layer` calls at the same size/format across many frames, with a pool hit/miss counter printed -- expected to stop growing after the first frame(s), proving pool reuse rather than per-frame allocation.

7. Fix TECHNICAL.md Section 3.4's leftover `operator new`/`operator delete` C++ phrasing (wording only, see "Doc fix found while planning" above).

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace.
- New unit tests: bucket rounding (`next_power_of_two` boundaries), miss-borrows-next-larger-then-grows-next-frame behavior, and the balance assertion actually panics on an unbalanced `push_layer` (no matching `pop_layer` before `flatten()`).
- The new demo run against real hardware with `VK_LAYER_KHRONOS_validation` enabled: zero errors, and the printed hit/miss counter confirms steady-state pool reuse (no growth after warmup).

## Explicitly out of scope for this step

- Genuine overlapping multi-frame-in-flight GPU/CPU submission (ring buffer structure is built for it; actual overlapping submission deferred to its own future step).
- Step 2.1 (Vulkan bindless descriptor indexing, DX12, Metal) and Step 2.3 (generational GC) -- separate future steps.
- Step 2.4 (CI-enforced validation layers) and Phase 9's full global-allocator zero-allocation guard -- separate future steps; this step's balance assertion is narrower (PushLayer/PopLayer depth only, not all heap allocation).
