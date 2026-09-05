# Log: Phase 1, Step 1 -- Linux Native Windowing, Multi-Window RhiDevice, Headless Mode

## Doc gaps found

None in the four core documents themselves this step -- the gaps found were all in the *implementation* built to satisfy ARCHITECTURE.md's existing "Global RhiDevice, per-window RhiSwapchain" model, not in the model itself.

## Bugs found (all caught by actually running the code, not by inspection)

### `VulkanDevice::submit_and_present`'s post-render layout transition assumed every `RhiSwapchain` is a real presentable swapchain
It unconditionally transitions the rendered image `COLOR_ATTACHMENT_OPTIMAL -> PRESENT_SRC_KHR` -- correct for `VulkanSwapchain`, meaningless for `HeadlessSwapchain`'s plain image. The Vulkan validation layer caught this immediately as a layout mismatch when `HeadlessSwapchain::present` assumed the image was still in `COLOR_ATTACHMENT_OPTIMAL`.

**Resolution (interim):** `HeadlessSwapchain::present`'s barrier now starts from the layout the image is actually in (`PRESENT_SRC_KHR`, tagging a non-swapchain image with it is unusual but valid -- it's just a layout tag) rather than the layout a windowed swapchain would need. **Real fix, deferred:** let each concrete `RhiSwapchain` control its own post-render transition instead of hardcoding one inside the shared `RhiDevice::submit_and_present` -- worth doing before more swapchain variants get built on this pattern.

### Leaked `VkSurfaceKHR` in the headless demo
`VulkanDevice::new` requires a window purely to probe present support while selecting a physical device -- awkward for headless mode, which has no real window at all. The headless demo created a throwaway probe window/surface to bootstrap the device and never destroyed the surface. The validation layer caught the leak at `vkDestroyInstance`.

**Resolution:** the demo now explicitly destroys the probe surface right after device creation. The underlying awkwardness -- headless mode needing a throwaway window just to bootstrap a device -- is a real API gap, deferred to Phase 2's device-selection work (a genuinely surface-less physical-device-selection path).

### `VulkanDevice::create_surface` didn't exist
Surface creation was embedded entirely inside `VulkanDevice::new`, with no way for a second window to get a surface without re-running physical device selection -- which would be wrong anyway (all windows must share the one device chosen at startup). Extracted as its own public method; `new` now calls the same underlying helper for its initial probe surface.

## Non-bugs worth recording (expected behavior, initially looked like bugs)

- **A Wayland surface with no buffer attached renders nothing at all** (unlike X11, which shows a blank mapped window with a real backing pixmap). The first windowing-only smoke test (before Vulkan was wired in) produced an invisible window; this is correct Wayland protocol behavior, not a failure to open the window. Confirmed by testing the X11 path instead, which showed a real, visible, titled window as expected -- and later by wiring up Vulkan, which attaches real buffers and made the Wayland window visible too.
- **xdg-shell gives clients no control over top-level window position.** The multi-window demo's two windows, opened with no position hints (Wayland has no mechanism for a client to request one), landed at the same compositor-chosen spot and visually overlapped in a screenshot. Verified via the terminal log and zero validation errors that both windows were genuinely open and independently rendering (distinct colors) throughout -- the overlap is a window-manager placement artifact, not evidence the sharing model is broken.

## Verification performed

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace`: all clean.
- Wayland and X11 (via XWayland) windowing smoke-tested standalone before Vulkan integration.
- All three examples (`walking_skeleton` migrated off `winit`, `multi_window`, `headless`) run against real hardware (AMD Radeon 890M) with `VK_LAYER_KHRONOS_validation` enabled: zero validation errors, clean shutdown.
- `multi_window`: 120 frames, both windows open and rendering the entire time; screenshotted.
- `headless`: output PNG inspected and confirmed pixel-correct (right color, right position, right size).
