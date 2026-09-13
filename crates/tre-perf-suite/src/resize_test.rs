//! Interaction-Driven Resize Test: a real window rendering a fixed,
//! moderate scene continuously, while a human resizes it. Tracks how the
//! engine handles real swapchain recreation, buffer reallocation, and
//! viewport scaling under manual stress -- event-driven and variable-
//! length per the spec, not automated/scaled over time the way the
//! Time-Ramp test is.
//!
//! # History: two real bugs found via actual interactive runs
//!
//! **#230 (`DeviceLost`):** the project owner's own first real resize
//! crashed after a Wayland compositor error (`wp_fifo_manager_v1:
//! Attempted to create a second fifo surface for the wl_surface`). Root
//! cause: the original resize path tore down and rebuilt the whole
//! `Surface` -- including a brand-new `VulkanDevice::create_surface`
//! call -- on every resize, and the *new* surface's fifo-surface request
//! could reach the compositor while the *old* one (still bound to the
//! same `wl_surface`) had not yet been dropped.
//!
//! **#231 (redundant rebuilds):** even after #230's fix, every `Resized`
//! event drained from one `poll_events()` batch triggered its own full
//! rebuild, even when a later event in the same batch immediately
//! superseded it -- coalesced to act on only the batch's last size.
//!
//! **#232 (this fix): the real, root-level fix for both.** Recreating
//! the surface at all was never necessary -- only the `VkSwapchainKHR`
//! actually needs to change size. `Surface` now builds its surface
//! *once* and calls the new [`tre_rhi_vulkan::VulkanSwapchain::recreate`]
//! in place on every resize, which reuses the same `VkSurfaceKHR`
//! unconditionally. This closes #230 at its real cause (the surface is
//! never recreated, so the Wayland conflict can't occur) rather than
//! working around it, and it means `pipelines` never needs
//! re-registering on resize either -- a swapchain's color format is a
//! property of the surface, which this path never touches.

use std::time::Instant;

use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, BufferBinding, EngineError, FlattenedFrame, FrameClock, InputEvent,
    PipelineRegistry, RenderingCanvas, RhiDevice, ScissorRect, ShapeRegistry, WindowId,
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

/// The part of the render target a resize touches -- built once, at
/// window-open time; a resize after that only ever calls [`Surface::
/// resize`], never rebuilds this whole struct.
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

    /// Resizes in place: recreates only `self.swapchain`'s own
    /// `VkSwapchainKHR` and dependent resources (via `VulkanSwapchain::
    /// recreate`, reusing the same `VkSurfaceKHR` this `Surface` was
    /// created with) and updates `width`/`height` -- `self.pipelines` is
    /// deliberately untouched; see this module's own header comment.
    fn resize(&mut self, device: &VulkanDevice, width: u32, height: u32) -> f32 {
        let rebuild_start = Instant::now();
        self.swapchain
            .recreate(device, width, height)
            .expect("failed to recreate VulkanSwapchain");
        self.width = width;
        self.height = height;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a real swapchain rebuild's own wall-clock duration never approaches f32's \
                      precision limits"
        )]
        let rebuild_ms = rebuild_start.elapsed().as_secs_f64() as f32 * 1000.0;
        rebuild_ms
    }
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

    let mut surface = Surface::create(&device, &connection, window, INITIAL_WIDTH, INITIAL_HEIGHT);
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

    // Diagnostic-only, added to chase the "window slowly trails then
    // jumps to the cursor" lag the project owner reported during a live
    // resize drag -- REVIEW.md finding #235's own investigation. Any
    // single stage over this threshold is well beyond a 60Hz frame
    // budget (16.7ms) and gets logged with a per-stage breakdown so the
    // stall shows up as belonging to one specific layer (event-polling
    // vs. CPU recording vs. GPU submit/present) rather than "the loop".
    const STALL_THRESHOLD_MS: f32 = 8.0;

    'outer: loop {
        let elapsed = f64::from(clock.elapsed());

        let poll_start = Instant::now();
        // Coalesce every `Resized` event this one `poll_events()` batch
        // drains down to just the *last* (i.e. current) size, instead of
        // resizing once per queued event (REVIEW.md finding #231).
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
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a real per-frame stage duration never approaches f32's precision limits"
        )]
        let poll_ms = poll_start.elapsed().as_secs_f64() as f32 * 1000.0;
        if close_requested {
            break 'outer;
        }
        if let Some((width, height)) = latest_resize {
            if surface.width != width || surface.height != height {
                let rebuild_ms = surface.resize(&device, width, height);
                log.write_resize_event(elapsed, width, height, rebuild_ms)
                    .expect("failed to write telemetry log");
            }
        }

        let record_start = Instant::now();
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
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a real per-frame stage duration never approaches f32's precision limits"
        )]
        let record_ms = record_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: surface.width,
            height: surface.height,
        };

        // Diagnostic-only, split out of `submit_frame`'s own single
        // acquire+record+present sequence (REVIEW.md finding #235): the
        // first pass of this instrumentation proved the entire "trails
        // then jumps" stall lives somewhere inside `submit_frame` as a
        // whole (poll/record were always ~0ms while a stall was
        // happening); this second pass separately times `begin_frame`
        // (the `vkAcquireNextImageKHR` wait) from `submit_and_present`
        // (the `vkQueuePresentKHR` call) to find out which specific
        // Vulkan call is actually blocking.
        //
        // Real resize recovery, matching `windowed_renderer.rs`'s own
        // identical retry-once-against-a-freshly-resized-swapchain
        // fallback: a resize can invalidate the swapchain before this
        // loop's own `InputEvent::Resized` handling above catches up
        // (or via any other `SwapchainOutOfDate` cause, e.g. a
        // minimize/restore), and this is the belt-and-suspenders path
        // for exactly that race, not the primary mechanism.
        let acquire_start = Instant::now();
        let begin_result = device.begin_frame(&surface.swapchain);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a real per-frame stage duration never approaches f32's precision limits"
        )]
        let mut acquire_ms = acquire_start.elapsed().as_secs_f64() as f32 * 1000.0;
        let mut present_ms;
        let mut attempt_result = match begin_result {
            Ok((mut cmd_buffer, image)) => {
                execute_frame(
                    &flattened,
                    &surface.pipelines,
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
                    &mut *cmd_buffer,
                    &mut clip_stack,
                );
                let present_start = Instant::now();
                let result = device.submit_and_present(cmd_buffer, &surface.swapchain, image);
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "a real per-frame stage duration never approaches f32's precision \
                              limits"
                )]
                {
                    present_ms = present_start.elapsed().as_secs_f64() as f32 * 1000.0;
                }
                result
            }
            Err(e) => {
                present_ms = 0.0;
                Err(e)
            }
        };
        if matches!(attempt_result, Err(EngineError::SwapchainOutOfDate)) {
            let (width, height) = (surface.width, surface.height);
            let rebuild_ms = surface.resize(&device, width, height);
            log.write_resize_event(elapsed, width, height, rebuild_ms)
                .expect("failed to write telemetry log");
            let acquire_start2 = Instant::now();
            let begin_result2 = device.begin_frame(&surface.swapchain);
            #[allow(
                clippy::cast_possible_truncation,
                reason = "a real per-frame stage duration never approaches f32's precision limits"
            )]
            {
                acquire_ms += acquire_start2.elapsed().as_secs_f64() as f32 * 1000.0;
            }
            attempt_result = match begin_result2 {
                Ok((mut cmd_buffer, image)) => {
                    execute_frame(
                        &flattened,
                        &surface.pipelines,
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
                        &mut *cmd_buffer,
                        &mut clip_stack,
                    );
                    let present_start2 = Instant::now();
                    let result = device.submit_and_present(cmd_buffer, &surface.swapchain, image);
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "a real per-frame stage duration never approaches f32's \
                                  precision limits"
                    )]
                    {
                        present_ms += present_start2.elapsed().as_secs_f64() as f32 * 1000.0;
                    }
                    result
                }
                Err(e) => Err(e),
            };
        }
        attempt_result.expect("submit_frame failed even after one resize-recovery retry");
        if poll_ms > STALL_THRESHOLD_MS
            || record_ms > STALL_THRESHOLD_MS
            || acquire_ms > STALL_THRESHOLD_MS
            || present_ms > STALL_THRESHOLD_MS
        {
            log.write_frame_stall(elapsed, poll_ms, record_ms, acquire_ms, present_ms)
                .expect("failed to write telemetry log");
        }

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
