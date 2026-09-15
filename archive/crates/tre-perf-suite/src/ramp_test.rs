//! Time-Ramp Stress Test: a real, on-screen window and swapchain,
//! geometrically scaling one workload's own primitive count
//! (`1 -> 2 -> 4 -> 8 -> ...`) at a fixed interval for a fixed total
//! duration, sampling telemetry throughout. One real `cargo run
//! --release` process covers every selected workload sequentially.
//!
//! Window/device/swapchain/pipeline construction mirrors
//! `crates/tre-rhi-vulkan/examples/main_loop_demo.rs`'s and
//! `crates/tre-python/src/windowed_renderer.rs`'s own identical
//! sequence (probe window -> `VulkanDevice::new` -> real window +
//! `VulkanSwapchain` -> `register_shape_pipelines`) -- written fresh
//! here rather than factored into a shared helper, matching every one
//! of that sequence's ~20 other independent call sites across the
//! workspace.

use raw_window_handle::HasDisplayHandle;
use tre_engine::{
    execute_frame, submit_frame, BufferBinding, FlattenedFrame, FrameClock, InputEvent,
    PipelineRegistry, RenderingCanvas, RhiDevice, ScissorRect, ShapeRegistry,
};
use tre_platform::PlatformConnection;
use tre_rhi_vulkan::{register_shape_pipelines, VulkanDevice, VulkanSwapchain};

use crate::telemetry::{self, CpuMemSampler, Sample, TelemetryLog};
use crate::workload::{populate, Workload, WorkloadResources};

const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;
// Real finding from this tool's own first real run (not assumed up
// front): 16MiB looked generous on paper but panicked at a 65,536-
// primitive tier (32 bytes/vertex * 4 vertices + 4 bytes/index * 6
// indices per rect = ~152 bytes/primitive, so 65,536 primitives alone
// need ~10MB *before* accounting for the ring buffer's own multi-frame-
// in-flight headroom) -- a real limitation of this tool's own buffer
// sizing, not a genuine engine breaking point the spec's own "identify
// the exact breaking points" goal cares about. Raised to comfortably
// cover `MAX_TIER_COUNT` with real multi-frame headroom, confirmed by
// re-running past the previous crash point.
const RING_BUFFER_CAPACITY: usize = 64 * 1024 * 1024;
const SAMPLE_INTERVAL_S: f64 = 0.25;
const MAX_TIER_COUNT: usize = 1 << 17; // 131,072 -- see RING_BUFFER_CAPACITY's own comment

pub struct RampConfig {
    pub workloads: Vec<Workload>,
    pub duration_s: f64,
    pub tier_seconds: f64,
    pub out: Option<std::path::PathBuf>,
}

pub fn run(config: RampConfig) {
    let mut connection = PlatformConnection::new().expect("failed to connect to display server");
    let window = connection
        .create_window("tre-perf-suite: ramp test", WINDOW_WIDTH, WINDOW_HEIGHT)
        .expect("failed to open window");
    let display_handle = connection.display_handle().unwrap().as_raw();
    let window_handle = connection.window_handle(window).unwrap().as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    let swapchain = VulkanSwapchain::new(
        &device,
        surface_loader,
        surface,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
    )
    .expect("failed to create VulkanSwapchain");
    let mut pipelines = PipelineRegistry::new();
    register_shape_pipelines(&device, &mut pipelines, swapchain.format())
        .expect("failed to register shape pipelines");
    let ring_buffer = device
        .create_dynamic_ring_buffer(RING_BUFFER_CAPACITY)
        .expect("failed to create dynamic ring buffer");
    let resources = WorkloadResources::build(&device);

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: WINDOW_WIDTH,
        height: WINDOW_HEIGHT,
    };

    for workload in config.workloads {
        println!(
            "\n=== Time-Ramp Stress Test: workload={} duration={}s tier_seconds={}s ===",
            workload.name(),
            config.duration_s,
            config.tier_seconds
        );
        let log_path = config
            .out
            .clone()
            .unwrap_or_else(|| telemetry::default_log_path("ramp", workload.name()));
        let mut log = TelemetryLog::create(&log_path).unwrap_or_else(|e| {
            panic!(
                "failed to open telemetry log at {}: {e}",
                log_path.display()
            )
        });
        println!("writing telemetry to {}", log.path().display());

        let mut registry = ShapeRegistry::new();
        let mut canvas = RenderingCanvas::new();
        let mut flattened = FlattenedFrame::default();
        let mut clip_stack: Vec<ScissorRect> = Vec::new();
        let mut clock = FrameClock::new();
        let mut cpu_mem = CpuMemSampler::new();

        let mut count: usize = 1;
        populate(workload, &mut registry, &resources, count);
        log.write_tier_change(0.0, workload.name(), count)
            .expect("failed to write telemetry log");

        let mut next_tier_at = config.tier_seconds;
        let mut next_sample_at = 0.0;
        let mut closed = false;

        loop {
            let elapsed = f64::from(clock.elapsed());
            if elapsed >= config.duration_s {
                break;
            }

            for event in connection.poll_events() {
                if matches!(event, InputEvent::CloseRequested { window: w } if w == window) {
                    closed = true;
                }
            }
            if closed {
                break;
            }

            if elapsed >= next_tier_at && count < MAX_TIER_COUNT {
                count *= 2;
                registry = ShapeRegistry::new();
                populate(workload, &mut registry, &resources, count);
                log.write_tier_change(elapsed, workload.name(), count)
                    .expect("failed to write telemetry log");
                next_tier_at += config.tier_seconds;
            }

            registry.mark_all_dirty();
            canvas.reset();
            registry.flatten_into(&mut canvas, &device, None);
            canvas.flatten_into(&mut flattened);

            let vertex_bytes: &[u8] = bytemuck::cast_slice(&flattened.vertices);
            let index_bytes: &[u8] = bytemuck::cast_slice(&flattened.indices);
            let vertex_offset = ring_buffer.write(vertex_bytes).expect(
                "ring buffer write failed for vertices -- RING_BUFFER_CAPACITY too small for \
                 this tier's own primitive count",
            );
            let index_offset = ring_buffer
                .write(index_bytes)
                .expect("ring buffer write failed for indices");

            submit_frame(&device, &swapchain, |cmd_buffer| {
                execute_frame(
                    &flattened,
                    &pipelines,
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
                )
                .expect("execute_frame failed");
            })
            .expect("submit_frame failed");

            let dt = clock.tick();
            let (cpu_pct, mem_rss_kb) = cpu_mem.sample();

            if elapsed >= next_sample_at {
                let sample = Sample {
                    elapsed_s: elapsed,
                    profile: "ramp",
                    workload: workload.name().to_string(),
                    primitive_count: count,
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

        if closed {
            println!("window closed -- stopping the ramp test early");
            return;
        }
    }

    println!("\nTime-Ramp Stress Test complete.");
}
