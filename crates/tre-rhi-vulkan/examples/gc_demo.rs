//! Phase 2 Step 2.3 proof: real generational GC, run by a genuine
//! background OS thread, evicting stale entries from the transient
//! render-target pool (IMPLEMENTATION.md Step 2.3). The GC thread only
//! ever locks plain Rust state and moves values into a queue -- it never
//! calls a Vulkan function; the main thread does the actual destruction,
//! in `begin_frame`, after a real 3-frame grace period.
//!
//! Runs the REAL 600-frame age threshold and 3-frame grace period this
//! step specifies -- no shortened stand-in constants -- and proves it by
//! polling `transient_pool_stats()`'s `evictions`/`destroyed` counters
//! with a generous real timeout, not a fixed sleep.

use std::time::{Duration, Instant};

use tre_engine::{RhiDevice, TextureFormat};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

fn main() {
    // Same headless-probe-window bootstrap as the other examples --
    // `VulkanDevice::new` needs a transient surface to select a physical
    // device/queue family, destroyed immediately below since this demo
    // never presents to it.
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre gc demo probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    // A tiny headless target -- this demo never inspects a rendered
    // pixel, it only needs real `begin_frame`/`submit_and_present` cycles
    // to advance the real frame counter the GC thread and the main
    // thread's deferred-release drain both read.
    let swapchain =
        HeadlessSwapchain::new(&device, 64, 64).expect("failed to create HeadlessSwapchain");

    // Check ~25 distinct bucket sizes into the transient pool -- distinct
    // (width, height) pairs so each becomes its own free-list entry
    // instead of colliding into one reused bucket. All five widths/
    // heights are already exact powers of two, so the bucket key matches
    // the requested size exactly (no "next-larger fallback" surprises).
    // Total: 4 bytes/pixel * (256+512+1024+2048+4096)^2 =~ 240 MB,
    // comfortably past the 85%-of-128MB (~108.8 MB) GC trigger threshold.
    const SIZES: [u32; 5] = [256, 512, 1024, 2048, 4096];
    let mut total_bytes: u64 = 0;
    for &width in &SIZES {
        for &height in &SIZES {
            let texture = device.acquire_transient_target(width, height, TextureFormat::Bgra8Srgb);
            total_bytes += u64::from(width) * u64::from(height) * 4;
            device.release_transient_target(texture);
        }
    }
    eprintln!(
        "checked {} distinct sizes into the transient pool (~{} MB total)",
        SIZES.len() * SIZES.len(),
        total_bytes / (1024 * 1024)
    );

    let before = device.transient_pool_stats();
    assert_eq!(
        before.evictions, 0,
        "nothing should be stale yet -- every entry was just checked in"
    );

    // Advance real frames until the GC thread has evicted the now-stale
    // entries (frame count > 600) and the main thread's deferred-release
    // drain has actually destroyed them (a further 3-frame grace period).
    // No draw calls needed -- `begin_frame`/`submit_and_present` alone is
    // a complete, valid frame (a dynamic-rendering clear with nothing
    // drawn into it).
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut frames: u64 = 0;
    loop {
        let (cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
        device
            .submit_and_present(cmd_buffer, &swapchain, image)
            .expect("submit_and_present failed");
        frames += 1;

        let stats = device.transient_pool_stats();
        if stats.destroyed > 0 {
            eprintln!(
                "frame {frames}: evictions={}, destroyed={}",
                stats.evictions, stats.destroyed
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "GC did not evict+destroy anything within the timeout after {frames} frames \
             (evictions={}, destroyed={})",
            stats.evictions,
            stats.destroyed
        );
    }

    let after = device.transient_pool_stats();
    assert!(
        after.evictions > 0,
        "the background GC thread should have evicted the stale entries"
    );
    assert!(
        after.destroyed > 0,
        "the main thread should have destroyed evicted entries after their grace period"
    );
    // The real 600-frame age threshold, not a shortened stand-in: eviction
    // cannot have happened before the frame counter actually crossed it.
    assert!(
        frames > 600,
        "eviction happened before the real 600-frame age threshold (frames={frames})"
    );

    eprintln!(
        "gc_demo: {frames} frames, evictions={}, destroyed={}",
        after.evictions, after.destroyed
    );
    eprintln!("gc demo exited cleanly");
}
