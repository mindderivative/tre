//! Phase 2 Step 1 proof: the real `RhiDevice::create_dynamic_ring_buffer`
//! and `acquire_transient_target`/`release_transient_target`
//! implementations (TECHNICAL.md Sections 3.1/3.2), replacing the
//! `unimplemented!()` stubs every earlier phase left in place.
//!
//! No window is needed -- like `headless.rs`, this bootstraps a
//! `VulkanDevice` via a throwaway probe surface, then drives real frames
//! through a `HeadlessSwapchain` purely to exercise the frame-in-flight
//! rotation `VulkanRingBuffer`/the transient pool's deferred growth both
//! depend on. Nothing is drawn -- this demo proves memory management, not
//! rendering.

use tre_engine::{RhiDevice, TextureFormat};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre memory pools probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    // SAFETY: matches headless.rs -- this probe surface is only needed
    // transiently to let `VulkanDevice::new` pick a physical device, and
    // must be destroyed explicitly (the Vulkan validation layer catches
    // the leak otherwise).
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    let swapchain =
        HeadlessSwapchain::new(&device, 64, 64).expect("failed to create HeadlessSwapchain");

    // --- Ring buffer: prove real segment rotation + alignment ---
    //
    // 3MB total / 3 segments = 1MB per segment, small enough to reason
    // about by hand while still exercising the same code path a real
    // 16-32MB buffer would.
    let ring_buffer = device.create_dynamic_ring_buffer(3 * 1024 * 1024);
    const SEGMENT_SIZE: u32 = 1024 * 1024;

    eprintln!("--- dynamic ring buffer: writing across 7 frames (2 full rotations + 1) ---");
    for frame in 0..7 {
        let (cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
        // No draw calls -- this demo proves memory management, not
        // rendering; an empty (cleared-to-background) frame is a valid,
        // real frame to submit.
        device
            .submit_and_present(cmd_buffer, &swapchain, image)
            .expect("submit_and_present failed");

        let a = ring_buffer
            .write(&[0xAAu8; 300])
            .expect("ring buffer write failed");
        let b = ring_buffer
            .write(&[0xBBu8; 40])
            .expect("ring buffer write failed");
        let segment = a / SEGMENT_SIZE;
        eprintln!(
            "frame {frame}: segment {segment}, offsets {a} then {b} (gap {} bytes -- 256-byte aligned)",
            b - a
        );
        assert_eq!(a % 256, 0, "offsets must be 256-byte aligned");
        assert_eq!(b % 256, 0, "offsets must be 256-byte aligned");
    }

    // --- Transient pool: prove steady-state reuse after warmup ---
    eprintln!();
    eprintln!("--- transient pool: 20 acquire/release cycles at the same size ---");
    for i in 0..20 {
        let texture = device
            .acquire_transient_target(200, 150, TextureFormat::Bgra8Srgb)
            .expect("failed to acquire transient target");
        let dims = texture.dimensions();
        device.release_transient_target(texture);
        let stats = device.transient_pool_stats();
        eprintln!(
            "cycle {i}: got {}x{} (bucket rounds 200x150 up to 256x256), hits={} misses={}",
            dims.0, dims.1, stats.hits, stats.misses
        );
    }
    let stats = device.transient_pool_stats();
    assert_eq!(
        stats.misses, 1,
        "only the very first acquire of a novel size should ever miss"
    );
    assert_eq!(
        stats.hits, 19,
        "every subsequent acquire of the same size must be a pool hit, not a fresh allocation"
    );

    eprintln!();
    eprintln!("--- transient pool: miss on a new size, then next-larger fallback ---");
    // A larger, never-before-requested size: first request misses and
    // (since nothing pooled is big enough yet either) allocates directly;
    // the *next* frame's begin_frame call would grow this bucket into the
    // pool for a REAL steady-state check, but this demo's point is
    // proving the miss/allocate path itself works without a validation
    // error, not re-deriving the steady-state check already done above.
    let bigger = device
        .acquire_transient_target(600, 600, TextureFormat::Bgra8Srgb)
        .expect("failed to acquire transient target");
    eprintln!(
        "requested 600x600, got {:?} (bucket rounds up to 1024x1024)",
        bigger.dimensions()
    );
    device.release_transient_target(bigger);
    let stats = device.transient_pool_stats();
    eprintln!("final stats: hits={} misses={}", stats.hits, stats.misses);
    assert_eq!(
        stats.misses, 2,
        "the new 600x600 size should be exactly one more miss"
    );

    unsafe {
        let _ = device.device.device_wait_idle();
    }
    eprintln!();
    eprintln!("memory pools demo exited cleanly");
}
