# Log: Phase 2, Step 2 -- GPU API Validation in Debug & CI Builds

## Doc gaps found

None -- TECHNICAL.md's "GPU API Validation" bullet and IMPLEMENTATION.md's Step 2.4 task list both matched what got built (Vulkan-only, DX12/Metal deferred) without correction, beyond adding "implemented as of" notes.

## Real bugs found (both caught by actually testing, not by reading docs or code review)

### The debug messenger's `std::process::exit()` hung instead of terminating
The first implementation of the `VK_EXT_debug_utils` callback called `std::process::exit(1)` on an `ERROR`-severity message. This looked correct (Rust's own docs describe `exit()` as terminating the process immediately) but wasn't tested against a real trigger until deliberately forcing one (Task 4's zero-byte-buffer test).

**How it was caught:** running the deliberately-broken `memory_pools_demo` locally hung indefinitely instead of exiting. A hard `timeout 10` wrapper confirmed this wasn't a slow shutdown -- the process was genuinely stuck (exit code 124, meaning `timeout` had to kill it) despite the callback's `eprintln!` printing the expected validation message first.

**Root cause (best available explanation, not independently proven against Mesa/RADV source):** `std::process::exit()` runs every registered `atexit` handler before actually terminating. The GPU driver (or a layer beneath it) appears to register its own cleanup handler that tries to reacquire a lock the still-on-the-stack `vkCreateBuffer` call -- the very call whose validation callback is calling `exit()` -- is already holding. Calling `exit()` from inside a callback invoked partway through a Vulkan API call is calling into driver cleanup code while the driver is mid-call, an ordering the driver was never designed to handle.

**Resolution:** switched to `std::process::abort()`, which raises `SIGABRT` directly and skips `atexit` entirely -- there is no cleanup to deadlock against. Re-verified with the identical test: exit code 134 (SIGABRT, core dumped), confirmed via the raw binary, via `cargo run`, and later again in the real CI environment (a completely different Vulkan implementation -- Mesa's lavapipe software renderer, not the real AMD driver used locally) before accepting the fix.

### CI has been failing since Phase 1 Step 1 -- three commits, completely undetected
Verifying this step's new `vulkan-validation` CI job required `cargo build` to succeed on the runner, which surfaced that it never had, going back to the commit that added `tre-platform` (Phase 1 Step 1). `gh run list --branch main` showed `failure` for that commit, the subsequent SAFETY-comments fix, and Phase 2 Step 1 -- three consecutive pushes, all red, none of them checked.

**Root cause:** three system dependencies the workspace needs just to compile were never installed on GitHub's `ubuntu-latest` runners:
- `libwayland-dev` -- `wayland-client`'s "system" feature (a real `libwayland-client.so`, not the pure-Rust backend) needs `wayland-client.pc` for its build script's `pkg-config` lookup.
- `libxcb1-dev` -- `x11rb`'s XCB FFI connection links against `libxcb` directly.
- `glslc` -- `tre-rhi-vulkan`'s build script shells out to it to compile the placeholder shaders.

Each failure masked the next: `wayland-sys`'s build script failed first (alphabetically/dependency-order earliest), so `libxcb`'s and `glslc`'s absence were never even reached until that first gap was fixed and the build progressed further.

**Why this went three commits without anyone noticing:** every one of those three steps was verified thoroughly *locally* (real hardware, real validation layers, real example runs) -- that discipline never lapsed. What lapsed was checking CI's own status after pushing. The project's initial CI-setup work (fixing the toolchain-component gap, `rust-toolchain.toml`'s `components` list) established the habit of checking `gh run view` when something was *suspected* broken, but no later step's process included checking it as a matter of course after an unrelated push, since CI was believed to already be working correctly.

**Resolution:** `libwayland-dev`, `libxcb1-dev`, and `glslc` added to every CI job's `apt-get install` step (`clippy`, `build`, `test`, and the new `vulkan-validation` job -- `fmt` doesn't compile anything, so it was never affected). Committed as its own, separately-described commit rather than folded into this step's feature commit, since it's a pre-existing regression this step's verification work happened to be the first to surface, not something Step 2.4 itself introduced.

**Process change:** run `gh run list --branch main --limit 1` after any push, not only when a job is specifically suspected of failing.

## Verification performed

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace`: all clean.
- Confirmed validation now loads automatically in debug builds with **no manual env vars set** (`VK_LOADER_LAYERS_ENABLE`/`VK_INSTANCE_LAYERS` explicitly unset): `VK_LOADER_DEBUG=all` showed `VK_LAYER_KHRONOS_validation` `Enabled By: By the Application`, not an environment variable -- direct, positive proof the code-level request works, not just an absence-of-errors inference.
- The full CI-gate proof (Task 4): on a scratch branch (`verify/step2-2-ci-gate`), pushed a deliberate, deterministic bug (`create_dynamic_ring_buffer(0)`, a guaranteed zero-size-buffer VUID violation), confirmed via `gh run view --log-failed` that the real GitHub Actions `vulkan-validation` job failed with the exact expected message (`VUID-VkBufferCreateInfo-size-00912`) and exit code 134, then reverted the bug and confirmed all five CI jobs (`rustfmt`, `clippy`, `build`, `test`, `vulkan-validation`) pass clean. The scratch branch was deleted after use.
- All five examples (`walking_skeleton`, `multi_window`, `headless`, `input_demo`, `memory_pools_demo`) verified passing in the real CI environment (Mesa lavapipe software renderer + `xvfb-run` virtual display) for the first time -- none of them had ever been executed in CI before this step, only compiled.
