//! Interaction-Driven Resize Test: a real window rendering a fixed,
//! moderate scene continuously, while a human resizes it. Tracks how the
//! engine handles real swapchain recreation, buffer reallocation, and
//! viewport scaling under manual stress -- event-driven and variable-
//! length per the spec, not automated/scaled over time the way the
//! Time-Ramp test is.
//!
//! Swapchain-plus-pipeline reconstruction on resize mirrors
//! `crates/tre-python/src/windowed_renderer.rs`'s own `WindowSlot::
//! create`-on-`SwapchainOutOfDate` sequence -- the workspace's only
//! existing precedent for rebuilding a live swapchain -- both
//! proactively (on `InputEvent::Resized`) and reactively (catching
//! `EngineError::SwapchainOutOfDate` as a fallback, the same belt-and-
//! suspenders `windowed_renderer.rs` itself uses).
//!
//! # A real bug found via the first actual interactive run
//!
//! The project owner's own first real resize crashed with `DeviceLost`
//! after a Wayland compositor error (`wp_fifo_manager_v1: Attempted to
//! create a second fifo surface for the wl_surface`). Root cause: the
//! obvious `surface = Surface::create(...)` reassignment constructs the
//! *new* `Surface` (which requests a brand-new `VkSurfaceKHR`/fifo
//! surface from the compositor) while the *old* `Surface` -- still bound
//! to the same underlying `wl_surface` -- has not been dropped yet, since
//! Rust only drops a variable's old value after the assignment's right-
//! hand side is fully evaluated. Wayland's `wp_fifo_manager_v1` protocol
//! allows only one fifo surface per `wl_surface` at a time, so the
//! second request is rejected, corrupting the surface and surfacing as
//! `DeviceLost` on the following `VulkanSwapchain::new`. Fixed by making
//! `surface` an `Option<Surface>` and [`rebuild_surface`] explicitly
//! setting it to `None` (running the old `Surface`'s `Drop`, which does
//! destroy its own `VkSurfaceKHR`) *before* constructing the replacement
//! -- `windowed_renderer.rs`'s own identical `HashMap::insert`-based
//! resize path has this same latent ordering issue, not fixed here since
//! it's shared, unrelated code outside this crate's own scope.

use std::time::Instant;

use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, submit_frame, BufferBinding, EngineError, FlattenedFrame, FrameClock,
    InputEvent, PipelineRegistry, RenderingCanvas, RhiDevice, ScissorRect, ShapeRegistry, WindowId,
};
use tre_platform::PlatformConnection;
use tre_rhi_vulkan::{register_shape_pipelines, VulkanDevice, VulkanSwapchain};

use crate::telemetry::{self, CpuMemSampler, Sample, TelemetryLog};
use crate::workload::{populate, Workload, WorkloadResources};

const INITIAL_WIDTH: u32 = 800;
const INITIAL_HEIGHT: u32 = 600;
const FIXED_SHAPE_COUNT: usize = 500;
const RING_BUFFER_CAPACITY: usize = 4 * 1024 * 1024;
const SAMPLE_INTERVAL_S: f64 = 0.25;

/// The part of the render target that a resize actually replaces --
/// mirrors `windowed_renderer.rs`'s own `WindowSlot` exactly (a fresh
/// surface + swapchain + freshly-recompiled shape pipelines against the
/// new swapchain format/extent).
struct Surface {
    swapchain: VulkanSwapchain,
    pipelines: PipelineRegistry,
    width: u32,
    height: u32,
}

impl Surface {
    fn create(
        device: &VulkanDevice,
        connection: &PlatformConnection,
        window: WindowId,
        width: u32,
        height: u32,
    ) -> Self {
        let display_handle = connection.display_handle().unwrap().as_raw();
        let window_handle = connection.window_handle(window).unwrap().as_raw();
        let (surface_loader, surface) = device
            .create_surface(display_handle, window_handle)
            .expect("failed to create surface");
        let swapchain = VulkanSwapchain::new(device, surface_loader, surface, width, height)
            .expect("failed to create VulkanSwapchain");
        let mut pipelines = PipelineRegistry::new();
        register_shape_pipelines(device, &mut pipelines, swapchain.format())
            .expect("failed to register shape pipelines");
        Self {
            swapchain,
            pipelines,
            width,
            height,
        }
    }
}

/// Replaces `*surface` with a freshly built one at `width`x`height`,
/// returning the rebuild's own wall-clock duration in milliseconds.
/// Explicitly drops the old `Surface` (`*surface = None`) before
/// constructing the new one -- see this module's own header comment for
/// why that ordering is load-bearing, not stylistic.
fn rebuild_surface(
    surface: &mut Option<Surface>,
    device: &VulkanDevice,
    connection: &PlatformConnection,
    window: WindowId,
    width: u32,
    height: u32,
) -> f32 {
    let rebuild_start = Instant::now();
    *surface = None;
    *surface = Some(Surface::create(device, connection, window, width, height));
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a real swapchain rebuild's own wall-clock duration never approaches f32's \
                  precision limits"
    )]
    let rebuild_ms = rebuild_start.elapsed().as_secs_f64() as f32 * 1000.0;
    rebuild_ms
}

pub fn run(out: Option<std::path::PathBuf>) {
    let mut connection = PlatformConnection::new().expect("failed to connect to display server");
    let window = connection
        .create_window(
            "tre-perf-suite: resize test (resize me!)",
            INITIAL_WIDTH,
            INITIAL_HEIGHT,
        )
        .expect("failed to open window");
    let display_handle = connection.display_handle().unwrap().as_raw();
    let window_handle = connection.window_handle(window).unwrap().as_raw();
    let (device, probe_surface_loader, probe_surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    // SAFETY: `probe_surface` was just created by `VulkanDevice::new`
    // against `probe_surface_loader`, both still valid here; the real
    // `Surface` built just below creates its own independent surface --
    // matching every headless renderer's own identical probe-surface
    // teardown (e.g. `tre-ffi`'s `build_renderer`).
    unsafe {
        probe_surface_loader.destroy_surface(probe_surface, None);
    }

    let mut surface = Some(Surface::create(
        &device,
        &connection,
        window,
        INITIAL_WIDTH,
        INITIAL_HEIGHT,
    ));
    let ring_buffer = device.create_dynamic_ring_buffer(RING_BUFFER_CAPACITY);
    let resources = WorkloadResources::build(&device);

    let mut registry = ShapeRegistry::new();
    populate(
        Workload::Shapes,
        &mut registry,
        &resources,
        FIXED_SHAPE_COUNT,
    );

    let log_path = out.unwrap_or_else(|| telemetry::default_log_path("resize", "interactive"));
    let mut log = TelemetryLog::create(&log_path).unwrap_or_else(|e| {
        panic!(
            "failed to open telemetry log at {}: {e}",
            log_path.display()
        )
    });
    println!("=== Interaction-Driven Resize Test -- resize the window; close it to stop ===");
    println!("writing telemetry to {}", log.path().display());

    let mut canvas = RenderingCanvas::new();
    let mut flattened = FlattenedFrame::default();
    let mut clip_stack: Vec<ScissorRect> = Vec::new();
    let mut clock = FrameClock::new();
    let mut cpu_mem = CpuMemSampler::new();
    let mut next_sample_at = 0.0;

    'outer: loop {
        let elapsed = f64::from(clock.elapsed());

        // Coalesce every `Resized` event this one `poll_events()` batch
        // drains down to just the *last* (i.e. current) size, instead of
        // rebuilding the swapchain once per queued event -- pyCopper's
        // own `LESSONS_LEARNED.md` names this exact anti-pattern
        // ("`rendercanvas`'s `_on_size_change` fires one full synchronous
        // render per event with no coalescing -- a genuine backlog, not
        // a metaphor"). A fast drag can queue several resize events
        // between two polls of this loop (which only polls once per
        // rendered frame); rebuilding the entire surface/swapchain/
        // pipeline set for each stale intermediate size, only to
        // immediately discard it for the next one, is pure waste this
        // coalescing removes for free -- the final rebuild below is
        // against the one size that's actually still current.
        let mut close_requested = false;
        let mut latest_resize: Option<(u32, u32)> = None;
        for event in connection.poll_events() {
            match event {
                InputEvent::CloseRequested { window: w } if w == window => close_requested = true,
                InputEvent::Resized {
                    window: w,
                    width,
                    height,
                } if w == window => {
                    latest_resize = Some((width, height));
                }
                _ => {}
            }
        }
        if close_requested {
            break 'outer;
        }
        if let Some((width, height)) = latest_resize {
            if surface
                .as_ref()
                .is_some_and(|s| s.width != width || s.height != height)
            {
                let rebuild_ms =
                    rebuild_surface(&mut surface, &device, &connection, window, width, height);
                log.write_resize_event(elapsed, width, height, rebuild_ms)
                    .expect("failed to write telemetry log");
            }
        }

        registry.mark_all_dirty();
        canvas.reset();
        registry.flatten_into(&mut canvas, &device, None);
        canvas.flatten_into(&mut flattened);

        let vertex_bytes: &[u8] = bytemuck::cast_slice(&flattened.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&flattened.indices);
        let vertex_offset = ring_buffer
            .write(vertex_bytes)
            .expect("ring buffer write failed for vertices");
        let index_offset = ring_buffer
            .write(index_bytes)
            .expect("ring buffer write failed for indices");

        let s = surface
            .as_ref()
            .expect("surface is always Some between rebuilds");
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: s.width,
            height: s.height,
        };

        // Real resize recovery, matching `windowed_renderer.rs`'s own
        // identical retry-once-against-a-freshly-recreated-swapchain
        // fallback: a resize can invalidate the swapchain before this
        // loop's own `InputEvent::Resized` handling above catches up
        // (or via any other `SwapchainOutOfDate` cause, e.g. a
        // minimize/restore), and this is the belt-and-suspenders path
        // for exactly that race, not the primary mechanism.
        let mut attempt_result = submit_frame(&device, &s.swapchain, |cmd_buffer| {
            execute_frame(
                &flattened,
                &s.pipelines,
                BufferBinding {
                    buffer: &*ring_buffer,
                    offset: vertex_offset,
                },
                BufferBinding {
                    buffer: &*ring_buffer,
                    offset: index_offset,
                },
                &full_window,
                &device,
                cmd_buffer,
                &mut clip_stack,
            );
        });
        if matches!(attempt_result, Err(EngineError::SwapchainOutOfDate)) {
            let (width, height) = (s.width, s.height);
            let rebuild_ms =
                rebuild_surface(&mut surface, &device, &connection, window, width, height);
            log.write_resize_event(elapsed, width, height, rebuild_ms)
                .expect("failed to write telemetry log");
            let s = surface
                .as_ref()
                .expect("just rebuilt above, always Some immediately after");
            attempt_result = submit_frame(&device, &s.swapchain, |cmd_buffer| {
                execute_frame(
                    &flattened,
                    &s.pipelines,
                    BufferBinding {
                        buffer: &*ring_buffer,
                        offset: vertex_offset,
                    },
                    BufferBinding {
                        buffer: &*ring_buffer,
                        offset: index_offset,
                    },
                    &full_window,
                    &device,
                    cmd_buffer,
                    &mut clip_stack,
                );
            });
        }
        attempt_result.expect("submit_frame failed even after one resize-recovery retry");

        let dt = clock.tick();
        let (cpu_pct, mem_rss_kb) = cpu_mem.sample();
        if elapsed >= next_sample_at {
            let sample = Sample {
                elapsed_s: elapsed,
                profile: "resize",
                workload: "interactive".to_string(),
                primitive_count: FIXED_SHAPE_COUNT,
                fps: if dt > 0.0 { 1.0 / dt } else { 0.0 },
                frame_time_ms: dt * 1000.0,
                cpu_pct,
                mem_rss_kb,
                gpu_pct: telemetry::read_gpu_busy_percent(),
            };
            log.write_sample(&sample)
                .expect("failed to write telemetry log");
            next_sample_at += SAMPLE_INTERVAL_S;
        }
    }

    println!("\nInteraction-Driven Resize Test complete.");
}
