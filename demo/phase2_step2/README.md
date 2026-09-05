# Demo: Phase 2, Step 2 -- GPU API Validation in Debug & CI Builds

```bash
./demo/phase2_step2/verify_automatic_validation.sh
```

Every earlier phase's demos required the operator to manually set `VK_LOADER_LAYERS_ENABLE`/`VK_INSTANCE_LAYERS` before running to get Vulkan validation. This script explicitly **unsets** those env vars and runs the headless example anyway, using the Vulkan loader's own diagnostic logging (`VK_LOADER_DEBUG=layer`) to prove `tre-rhi-vulkan`'s own code requested the layer:

```
[Vulkan Loader] LAYER:             VK_LAYER_KHRONOS_validation
[Vulkan Loader] LAYER:                     Type: Explicit
[Vulkan Loader] LAYER:                     Enabled By: By the Application
```

"Enabled By: By the Application" is the loader's own confirmation that this came from `VulkanDevice::new`'s code (`vk::InstanceCreateInfo::enabled_layer_names`), not an environment variable -- the actual thing this step changed.

**The other half of this step is CI**, not something you run locally: [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)'s `vulkan-validation` job installs a software Vulkan driver (`mesa-vulkan-drivers`) and a virtual display (`xvfb-run`), then runs all five examples -- `walking_skeleton`, `multi_window`, `headless`, `input_demo`, `memory_pools_demo` -- on every push. This is the first time any of them have run in CI at all (previously only compiled, never executed); check the Actions tab on GitHub to see it passing.

**What "the gate actually works" looked like when tested for real** (see `documentation/REVIEW.md`'s "Phase 2 Step 2 Implementation" section and `LOG.md` for the full trail): a deliberately broken build (`create_dynamic_ring_buffer(0)`, a guaranteed zero-size-buffer spec violation) was pushed to a scratch branch, and the real GitHub Actions job failed with exactly the expected error:

```
[Vulkan ERROR VALIDATION] Validation Error: [ VUID-VkBufferCreateInfo-size-00912 ] ...
```

...with a nonzero exit code, before being reverted. That's the proof this gate genuinely catches something, not just that it exists.

**Two real bugs were found building this, both by testing rather than assuming:**
- The debug messenger's first version called `std::process::exit()` on a validation error, which **hung indefinitely** instead of terminating (a GPU driver's own cleanup handler appears to deadlock against the in-progress Vulkan call). Fixed with `std::process::abort()` instead.
- CI had actually been failing since Phase 1 Step 1 -- three prior pushes, all red, undetected until this step's own verification work required `cargo build` to succeed on the runner. Three missing system packages (`libwayland-dev`, `libxcb1-dev`, `glslc`) are now installed in every CI job that compiles the workspace.
