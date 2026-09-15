# Plan: Phase 2, Step 2 -- GPU API Validation in Debug & CI Builds

## Scope decision (confirmed with project owner 2026-09-05)

Corresponds to IMPLEMENTATION.md's Step 2.4. Tasks 2 (DirectX 12) and 3 (Metal) are deferred entirely -- neither backend exists yet (Step 2.1 is itself deferred, and DX12/Metal specifically stay deferred even when that step happens, per the Step 1.1/2.1 precedent: no Windows/macOS machine to build or verify against). This step implements task 1 (Vulkan) and task 4 (release-build gating) for real.

**What this actually replaces:** every demo run in this project so far has been manually verified by the operator setting `VK_LOADER_LAYERS_ENABLE`/`VK_INSTANCE_LAYERS` env vars before running. This is exactly the kind of manual step that's easy to forget -- REVIEW.md findings #57/#58 (Phase 2 Step 1's two real bugs) were both caught this way, by a human remembering to re-run under validation after a change. This step makes validation load automatically in every debug build (no env vars needed) and makes forgetting it impossible in CI by actually running the Vulkan examples there for the first time.

**Graceful degradation, not a hard requirement:** requesting `VK_LAYER_KHRONOS_validation`/`VK_EXT_debug_utils` unconditionally would break `cargo run` for any contributor who hasn't installed the Vulkan validation layer package locally. `VulkanDevice::new` queries `vkEnumerateInstanceLayerProperties`/`vkEnumerateInstanceExtensionProperties` first and only requests them if actually available, logging a one-time warning if not -- validation gets stronger in CI (where the package is guaranteed installed) without becoming a hard dependency for local development.

**Why `std::process::exit`, not a panic, on an ERROR-severity message:** the debug messenger callback is an `extern "system" fn` called BY the Vulkan loader/driver (C code) -- unwinding a Rust panic across that FFI boundary back into non-Rust code is undefined behavior, not merely inadvisable (this is a stricter FFI boundary than `tre-ffi`'s already-established `panic = "unwind"` + `catch_unwind` pattern, which specifically wraps the panic before it can cross into C). `std::process::exit(1)` terminates immediately with a nonzero code -- enough to fail a CI job -- without ever unwinding through the callback.

**CI needs a software Vulkan driver and a virtual display, since GitHub-hosted runners have neither a GPU nor a display server.** `mesa-vulkan-drivers` (Ubuntu package providing `lavapipe`, Mesa's spec-compliant software Vulkan ICD) supplies the driver; `xvfb-run` supplies a virtual X11 display for `tre-platform`'s X11 fallback (no `WAYLAND_DISPLAY` exists in CI, so `PlatformConnection::new()` already falls back to X11 correctly, per its existing auto-detection). All five current examples need at least a probe window (headless mode's own deferred "surface-less device selection" gap, REVIEW.md #45, not fixed by this step), so every example run in the new CI job runs under the same `xvfb-run` wrapper.

## Goal

Validation loads automatically in every debug build without manual env vars, fails loudly (nonzero exit) on any real Vulkan validation error, and CI actually exercises every example under it -- closing the gap that let two real bugs slip past everything except a human remembering to check manually.

## Tasks

1. **Query-then-request validation layer/extension in `VulkanDevice::new`**, gated by `cfg(debug_assertions)` (matching TECHNICAL.md Section 3.4's existing release-build-compiles-out pattern, satisfying task 4 in the same gate): `vkEnumerateInstanceLayerProperties` for `VK_LAYER_KHRONOS_validation`, `vkEnumerateInstanceExtensionProperties` for `VK_EXT_debug_utils`; add both to `vk::InstanceCreateInfo` only if both are actually available, else proceed without them and print a one-time warning.

2. **Create a `vk::DebugUtilsMessengerEXT`** (debug builds, only if the extension was actually enabled per task 1) with a callback that prints every message (a placeholder for "the engine's own logging," which doesn't exist yet as a structured system -- `eprintln!` is the honest stand-in) and calls `std::process::exit(1)` on `ERROR` severity. Destroy it in `Drop for VulkanDevice`, before `destroy_instance`, mirroring Phase 2 Step 1's pool-before-device ordering fix.

3. **New CI job** (`.github/workflows/ci.yml`): installs `mesa-vulkan-drivers` and `vulkan-validationlayers` via `apt-get`, then runs all five examples (`walking_skeleton`, `multi_window`, `headless`, `input_demo`, `memory_pools_demo`) under `xvfb-run`, each with a short frame-count env var so the job finishes quickly.

4. **Prove the gate actually works, not just that it compiles**: temporarily reintroduce one of Phase 2 Step 1's two already-fixed bugs (or plant an equivalent deliberate one) on a scratch branch, push, and confirm via `gh` that the new CI job actually fails with the expected validation error -- then revert and confirm the real job passes clean. This is the same "verify for real" standard every other step in this project has been held to; a CI gate that was never seen to catch anything is unproven.

## Verification plan

- Local: every example still runs cleanly with NO manual `VK_LOADER_LAYERS_ENABLE`/`VK_INSTANCE_LAYERS` env vars set -- validation now loads on its own in debug builds.
- Local: `cargo fmt`/`clippy -D warnings`/`build`/`test` clean.
- CI: push and use `gh run view` (the same tool already used earlier in this project to root-cause the original CI toolchain gap) to confirm the new job runs, installs its packages, and all five examples pass.
- CI: the deliberate-bug proof in task 4 above, on a scratch branch, deleted afterward.

## Explicitly out of scope for this step

- DirectX 12 (`ID3D12Debug::EnableDebugLayer`) and Metal (`MTL_DEBUG_LAYER`) validation -- neither backend exists; deferred with them.
- Fixing the pre-existing "headless mode needs a throwaway probe window" API gap (REVIEW.md #45) -- CI works around it with `xvfb-run` rather than fixing the underlying gap, which stays tracked for Phase 2's later device-selection work.
- Building real structured engine logging -- the debug messenger's `eprintln!` is an honest placeholder, not a claim that logging infrastructure now exists.
