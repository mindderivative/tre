# Demo: Phase 2, Step 1 -- Zero-Allocation Ring Buffers & Transient Pools

```bash
./demo/phase2_step1/run_memory_pools_demo.sh
```

No window is shown -- like `demo/phase1_step1`'s headless demo, this proves memory management, not rendering. It bootstraps a `VulkanDevice` and drives 7 real frames through a `HeadlessSwapchain`, exercising the two primitives this step replaced `unimplemented!()` stubs with:

**1. The dynamic ring buffer** (`RhiDevice::create_dynamic_ring_buffer`, TECHNICAL.md Section 3.1): a real, persistently-mapped, host-coherent `VkBuffer` split into 3 frame-in-flight segments. The demo writes into it every frame and prints which segment landed where:

```
frame 0: segment 1, offsets 1048576 then 1049088 (gap 512 bytes -- 256-byte aligned)
frame 1: segment 2, offsets 2097152 then 2097664 (gap 512 bytes -- 256-byte aligned)
frame 2: segment 0, offsets 0 then 512 (gap 512 bytes -- 256-byte aligned)
...
```

Watch the segment number cycle 0, 1, 2, 0, 1, 2, ... across frames -- that's the triple-buffered rotation actually happening, not just compiling.

**2. The transient render target pool** (`RhiDevice::acquire_transient_target`/`release_transient_target`, TECHNICAL.md Section 3.2): the demo acquires and releases a same-sized target 20 times in a row, printing a hit/miss counter:

```
cycle 0: got 200x150 (bucket rounds 200x150 up to 256x256), hits=0 misses=1
cycle 1: got 200x150 (bucket rounds 200x150 up to 256x256), hits=1 misses=1
...
cycle 19: got 200x150 (bucket rounds 200x150 up to 256x256), hits=19 misses=1
```

Exactly one miss (the cold-start allocation), then every subsequent cycle is a pool hit -- steady-state zero-allocation reuse, which the demo itself `assert!`s on (it fails loudly if this regresses, not just prints a number and hopes you notice).

The demo's own assertions ARE the pass/fail signal -- if it prints `memory pools demo exited cleanly` at the end, every check passed.

**Verify with the Vulkan validation layer:**
```bash
VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation cargo run -p tre-rhi-vulkan --example memory_pools_demo
```

**What's actually new under the hood** (see `documentation/REVIEW.md`'s "Phase 2 Step 1 Implementation" section and `LOG.md` for the full detail, including two real bugs the validation layer caught while building this):
- `tre_engine::RhiDynamicRingBuffer` (a trait distinct from the plain `RhiBuffer`, since callers use a fundamentally different bump-allocate-per-frame pattern) and `tre-rhi-vulkan`'s `VulkanRingBuffer`.
- `tre_engine::TextureFormat`/`LayerDesc`, and `tre-rhi-vulkan`'s `VulkanTexture` backing the transient pool.
- `RenderingCanvas::push_layer`/`pop_layer` with a debug-build balance assertion -- deliberately IR-only for now (no direct pool hook yet), see `LOG.md`'s "Scope deviations" section for why.
