//! REVIEW.md finding #259: proves a `VulkanTexture` handed out by
//! `RhiDevice::create_texture` survives its owning `VulkanDevice` being
//! dropped first. Before the `Arc<DeviceOwner>` fix, the texture's `Drop`
//! freed its GPU image through a cloned `ash::Device` handle that dangled
//! the instant `VulkanDevice::drop` ran `vkDestroyDevice` -- a
//! use-after-free a software driver (lavapipe) surfaces as C-heap
//! corruption at teardown (the CI `python` job's `phase12_step12_4`).
//!
//! A `[[test]] harness = false` target under `tests/`, not a `#[test]`
//! function, because it must build a `winit` event loop, which panics off
//! the main thread (a plain libtest `#[test]` runs on a worker thread; a
//! `harness = false` target gets its own process and real main thread,
//! matching every other promoted example in this crate -- REVIEW.md
//! finding #261). `cargo test -p tre-rhi-vulkan` runs this on real
//! lavapipe in CI, where the regression actually reproduced. Run locally
//! under `valgrind --error-exitcode=1` with a software ICD to catch a
//! regression as an invalid free rather than relying on the driver to
//! corrupt the heap.

use raw_window_handle::HasDisplayHandle;
use tre_engine::{RhiDevice, TextureFormat};
use tre_rhi_vulkan::VulkanDevice;

fn main() {
    let mut probe = tre_platform::PlatformConnection::new().unwrap_or_else(|e| {
        eprintln!("no display server reachable in this environment ({e:?}) -- skipping");
        std::process::exit(0)
    });
    let window = probe
        .create_window("tre finding-259 probe (never shown)", 1, 1)
        .unwrap_or_else(|e| {
            eprintln!("could not open a probe/demo window in this environment ({e:?}) -- skipping");
            std::process::exit(0)
        });
    let display_handle = probe.display_handle().unwrap().as_raw();
    let window_handle = probe.window_handle(window).unwrap().as_raw();

    let (device, surface_loader, surface) = VulkanDevice::new(display_handle, window_handle)
        .unwrap_or_else(|e| {
            eprintln!("no Vulkan-capable device reachable in this environment ({e:?}) -- skipping");
            std::process::exit(0)
        });
    // The probe surface is only needed for physical-device selection.
    // SAFETY: just created against `surface_loader`; nothing else refs it.
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    // A real 2x2 RGBA texture through the public trait entry point -- the
    // same path a bindless, handed-out texture takes.
    let pixels = [255u8; 2 * 2 * 4];
    let texture = device
        .create_texture(2, 2, TextureFormat::Rgba8Unorm, &pixels)
        .expect("create_texture failed");

    // The whole point: drop the device FIRST, while the texture still
    // lives. The shared `Arc<DeviceOwner>` keeps the real `VkDevice` alive
    // until the texture's own `Drop` (below) frees its image; only then
    // does `vkDestroyDevice` run.
    drop(device);
    drop(texture);

    println!("texture_teardown_check: device dropped before texture, no use-after-free -- OK");
}
