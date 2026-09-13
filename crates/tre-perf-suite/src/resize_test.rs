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
//!
//! **#235 (separate symptom): "the window slowly trails then jumps to
//! the cursor" during an active drag.** Split acquire/present timing
//! isolated this to `vkAcquireNextImageKHR` itself blocking for
//! hundreds of milliseconds per frame while dragging -- never present,
//! never CPU-side work. Increasing the swapchain image count (the
//! cheapest candidate remedy) was tried and measured to make no
//! difference.
//!
//! **Option 2, shipped as the default `ResizeStrategy::Coarse`, via
//! pyCopper's own proven remedy for the identical symptom:** during a
//! drag, the swapchain is resized to the requested size rounded up to
//! the nearest `DRAG_COARSE_STEP` pixels rather than the exact live
//! size, so most of a drag's own resize events land in the same coarse
//! bucket and need no swapchain recreation at all; once the drag goes
//! quiet for `SETTLE_FRAMES` frames, one final resize snaps to the
//! exact size.
//!
//! **Revised after a live test showed real, visible squash/stretch
//! during a drag.** The first version of this fix left content
//! projected against the swapchain's own (coarse) extent, so there was
//! nothing to counteract the compositor's own scale-to-fit -- real,
//! visible distortion, not merely disclosed-and-accepted. Researching
//! pyCopper's *actual source* (`pycopper/src/pycopper/runtime/
//! engine.py`, not just its lessons file) found the real mechanism:
//! content is always projected using the true, exact window size, even
//! while the swapchain buffer itself sits at a coarser size -- the
//! compositor's scale-to-fit of the oversized buffer down to the real
//! window exactly cancels the resulting pre-stretch. This module now
//! does the identical thing via the new [`tre_engine::
//! submit_frame_with_logical_size`], and `DRAG_COARSE_STEP`/
//! `SETTLE_FRAMES` were changed to match pyCopper's own validated
//! production values (256px, a 3-frame settle countdown) exactly,
//! rather than this module's own earlier, untested 64px/wall-clock
//! guesses -- safe to be far coarser now that the projection fix makes
//! the mismatch invisible rather than merely smaller.
//!
//! **Option 3, `ResizeStrategy::Timeout`, added to let the project
//! owner compare it against Option 2 before deciding between them:**
//! resizes to the exact live size on every event (no coarse bucketing),
//! and instead bounds the per-frame `vkAcquireNextImageKHR` wait itself
//! via the new [`tre_engine::RhiDevice::begin_frame_with_timeout`] --
//! when it times out, this loop skips that tick's render entirely
//! (logged as a dedicated `acquire_skip` record) instead of blocking.
//! Never squashes/stretches (the swapchain always matches the real
//! size), but can visibly drop frames during a drag instead.
//!
//! **Further pursuit of #235: eliminating the compositor-resample blur
//! itself, not just capping it.** Option 2's stretch-then-let-the-
//! compositor-resample-back-down trick (above) has one residual,
//! precisely-understood cost: the resample itself is visibly soft during
//! an active drag. A first attempt to remove it by creating an
//! independent `wp_viewport` and calling `set_source` on it directly
//! crashed on the very first real drag (`wp_viewporter#63: error 0: the
//! specified surface already has a viewport`) -- `winit` already owns
//! exactly one `wp_viewport` per window, unconditionally, with no public
//! API to reach it. Fixed at the real root: a small, surgical patch to
//! `winit` itself (forked at the exact `v0.30.13` tag this workspace
//! already resolves, pinned via the workspace root `Cargo.toml`'s
//! `[patch.crates-io]`) adds `WindowExtWayland::set_viewport_source_crop`,
//! delegating to the viewport `winit` already owns and already calls
//! `set_destination` on every resize -- never a second one.
//!
//! That patch alone was not sufficient, though: `ResizeStrategy::Coarse`
//! initially kept calling `begin_frame_with_logical_size` (the stretch-
//! across-the-full-buffer trick) while ALSO cropping via the patched
//! `winit` -- and a live drag test showed *worse* jitter, not better.
//! The two are mutually exclusive strategies, not stackable: stretching
//! content to fill the whole oversized buffer, then cropping only its
//! top-left `real_size` corner unscaled, shows a continuously-zooming
//! fraction of the stretched scene, not the correctly-scaled whole
//! scene. The real fix replaces `begin_frame_with_logical_size` with the
//! new [`tre_engine::RhiDevice::begin_frame_with_viewport_crop`], which
//! confines the GPU viewport/scissor/render area to `real_size` itself --
//! rendering 1:1, undistorted, into just that sub-rectangle of the
//! (possibly larger) buffer -- paired with a
//! `tre_platform::PlatformConnection::set_viewport_source_crop` call of
//! the identical size so the compositor presents exactly that
//! sub-rectangle with no scaling at all. Net effect, if this holds up
//! under a real live drag test: zero resample, not just a smaller one.

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

/// REVIEW.md finding #235: the project owner's own choice of resize-stall
/// remedy for `run` to apply, selectable via `--resize-strategy` so
/// Option 2 and Option 3 can be compared directly against each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeStrategy {
    /// Option 2 (this module's own default): coarse-bucketed swapchain
    /// resizing during a drag, settling to the exact size at rest.
    Coarse,
    /// Option 3: exact-size resizing always, with a bounded
    /// `vkAcquireNextImageKHR` wait that skips a frame's render on
    /// timeout instead of blocking.
    Timeout,
}

const INITIAL_WIDTH: u32 = 800;
const INITIAL_HEIGHT: u32 = 600;
const FIXED_SHAPE_COUNT: usize = 500;
const RING_BUFFER_CAPACITY: usize = 4 * 1024 * 1024;
const SAMPLE_INTERVAL_S: f64 = 0.25;

/// REVIEW.md finding #235, Option 2: while a drag is active, the
/// swapchain targets `width`/`height` each rounded up to this many
/// pixels instead of the exact live size, so a burst of same-bucket
/// resize events needs no swapchain recreation at all. 256, matching
/// pyCopper's own validated production value (`Settings.resize_bucket`)
/// exactly -- ported into `crates/tre-python/src/windowed_renderer.rs`
/// at the identical value; kept in sync here deliberately.
const DRAG_COARSE_STEP: u32 = 256;
/// REVIEW.md finding #235, Option 2: frames the swapchain stays pinned
/// to a coarse size after the last real size change -- matching
/// pyCopper's own `SETTLE_FRAMES` exactly (a resize draws synchronously
/// per compositor configure, so this is naturally a handful of frames
/// after a drag stops, not a wall-clock delay tied to any particular
/// frame rate).
const SETTLE_FRAMES: u8 = 3;

/// The size to configure the swapchain at, and the new settle countdown
/// -- a direct Rust port of pyCopper's own `surface_size_for`
/// (`pycopper/src/pycopper/runtime/engine.py`), kept identical to the
/// copy in `crates/tre-python/src/windowed_renderer.rs` deliberately
/// (both are small enough that sharing a crate for this alone isn't
/// warranted, matching this workspace's own established precedent for
/// small, independently-duplicated glue code, e.g. every example's own
/// copy of the window/device bootstrap sequence).
///
/// Always rounds UP, deliberately, not to the nearest multiple --
/// tried rounding to nearest (REVIEW.md finding #235) to shrink the
/// worst-case mismatch, and the project owner caught a real perceptual
/// regression it introduced: rounding to nearest means the coarse
/// buffer is sometimes larger and sometimes smaller than the real
/// window depending which side of a bucket's own midpoint the live
/// size falls on, so the compositor's own scale direction flips between
/// shrink and stretch partway through a single continuous drag --
/// "the shapes are jittering up and down." Always rounding up keeps the
/// buffer >= the real size for the bucket's entire span, so the
/// compositor only ever shrinks, never both -- one consistent direction
/// beats a smaller but direction-flipping mismatch.
fn coarse_target_for(
    size: (u32, u32),
    previous: (u32, u32),
    settle: u8,
    bucket: u32,
) -> ((u32, u32), u8) {
    let settle = if size != previous {
        SETTLE_FRAMES
    } else {
        settle.saturating_sub(1)
    };
    if settle > 0 {
        (
            (
                size.0.div_ceil(bucket) * bucket,
                size.1.div_ceil(bucket) * bucket,
            ),
            settle,
        )
    } else {
        (size, settle)
    }
}

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

pub fn run(out: Option<std::path::PathBuf>, strategy: ResizeStrategy) {
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
    let ring_buffer = device
        .create_dynamic_ring_buffer(RING_BUFFER_CAPACITY)
        .expect("failed to create dynamic ring buffer");
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

    // REVIEW.md finding #235, Option 2: `real_size` is the true, exact
    // requested window size (updated whenever a `Resized` event
    // arrives), tracked separately from `surface.width`/`surface.height`
    // (the swapchain's own actual, possibly-coarse configured size) --
    // `coarse_target_for`'s state (`previous_size`/`settle`) decides
    // when the two should differ, and `real_size` is what content gets
    // projected against either way (this module's own header comment).
    let mut real_size: (u32, u32) = (INITIAL_WIDTH, INITIAL_HEIGHT);
    let mut previous_size: (u32, u32) = (INITIAL_WIDTH, INITIAL_HEIGHT);
    let mut settle: u8 = 0;

    // REVIEW.md finding #235, Option 3: only `ResizeStrategy::Timeout`
    // bounds the acquire wait -- `Coarse` passes `u64::MAX`, which
    // `RhiDevice::begin_frame_with_timeout` treats identically to plain
    // `begin_frame`'s own unbounded wait (a real Vulkan implementation
    // never returns `VK_TIMEOUT` for a `u64::MAX` timeout), so this one
    // constant cleanly covers both strategies without branching the
    // acquire call itself.
    const ACQUIRE_TIMEOUT_MS: f32 = 20.0;
    let acquire_timeout_ns: u64 = match strategy {
        ResizeStrategy::Coarse => u64::MAX,
        ResizeStrategy::Timeout => 20_000_000,
    };

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
            real_size = (width, height);
        }
        match strategy {
            ResizeStrategy::Coarse => {
                // REVIEW.md finding #235, Option 2's real fix:
                // `coarse_target_for` decides whether the swapchain
                // should sit at a coarse bucket (still settling) or the
                // exact `real_size` (settled) -- content is projected
                // against `real_size` either way (see the `begin_frame_
                // with_logical_size` call below), so this coarse/exact
                // choice only ever affects how often the swapchain
                // itself gets rebuilt, never what gets drawn.
                let (target, new_settle) =
                    coarse_target_for(real_size, previous_size, settle, DRAG_COARSE_STEP);
                previous_size = real_size;
                settle = new_settle;
                if surface.width != target.0 || surface.height != target.1 {
                    let rebuild_ms = surface.resize(&device, target.0, target.1);
                    log.write_resize_event(elapsed, target.0, target.1, rebuild_ms)
                        .expect("failed to write telemetry log");
                }
            }
            ResizeStrategy::Timeout => {
                // No coarse bucketing: always resize to the exact live
                // size, isolating Option 3's own bounded-acquire effect
                // for a fair comparison against Option 2 above.
                if surface.width != real_size.0 || surface.height != real_size.1 {
                    let rebuild_ms = surface.resize(&device, real_size.0, real_size.1);
                    log.write_resize_event(elapsed, real_size.0, real_size.1, rebuild_ms)
                        .expect("failed to write telemetry log");
                }
            }
        }

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
        //
        // Acquire runs BEFORE the CPU-side recording/ring-buffer-write
        // block below, not after -- a real bug found running Option 3
        // for the first time (REVIEW.md finding #235): with recording
        // done unconditionally first, every timed-out-and-skipped frame
        // still wrote a fresh vertex/index copy into the ring buffer's
        // current frame-in-flight segment without `submit_and_present`
        // ever running to advance `frame_sync.frame_index` and free it
        // up -- a sustained run of skips (exactly what a real, heavy
        // resize-drag stall produces) exhausts that segment and panics
        // with "ring buffer write failed". Recording only after a
        // successful acquire means a skipped frame writes nothing.
        let acquire_start = Instant::now();
        let begin_result = match strategy {
            // REVIEW.md finding #235's further pursuit: render 1:1,
            // undistorted, into just the `real_size` sub-rectangle of
            // `surface`'s own (possibly coarser) swapchain buffer --
            // paired with this same loop's own `connection.
            // set_viewport_source_crop(window, real_size...)` call below,
            // which tells the compositor to present exactly that
            // sub-rectangle unscaled. Replaces the original Option 2 fix
            // (`begin_frame_with_logical_size`, which instead stretched
            // content across the FULL oversized buffer for a compositor
            // resample to cancel back down) -- the two are mutually
            // exclusive render paths for the same underlying mismatch, not
            // stackable (see `RhiDevice::begin_frame_with_viewport_crop`'s
            // own doc comment for why combining them double-transforms
            // and reads as worse jitter, not better).
            ResizeStrategy::Coarse => {
                device.begin_frame_with_viewport_crop(&surface.swapchain, real_size)
            }
            ResizeStrategy::Timeout => {
                device.begin_frame_with_timeout(&surface.swapchain, acquire_timeout_ns)
            }
        };
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a real per-frame stage duration never approaches f32's precision limits"
        )]
        let mut acquire_ms = acquire_start.elapsed().as_secs_f64() as f32 * 1000.0;
        if matches!(begin_result, Err(EngineError::AcquireTimedOut)) {
            // REVIEW.md finding #235, Option 3: skip this tick's render
            // entirely rather than retrying or blocking -- safe to bail
            // out here with no cleanup because `VulkanDevice::
            // begin_frame_impl` only resets the frame fence *after* a
            // successful acquire, so a timed-out acquire leaves it
            // exactly as the next iteration's own `wait_for_fences`
            // expects to find it, and (per the reordering above) no
            // ring-buffer write has happened yet this iteration either.
            log.write_acquire_skip(elapsed, ACQUIRE_TIMEOUT_MS)
                .expect("failed to write telemetry log");
            continue 'outer;
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

        // `real_size`, not `surface.width`/`surface.height`: under
        // `Coarse`, the GPU render area/viewport/scissor are now confined
        // to `real_size` (see `begin_frame_with_viewport_crop` above), not
        // the swapchain's own possibly-larger physical extent -- a clip
        // rect wider than that would ask for a scissor outside the actual
        // render area, which dynamic rendering does not allow. Under
        // `Timeout`, `surface` is always kept exactly at `real_size`
        // anyway (rebuilt to the exact live size every frame), so this is
        // a no-op simplification there, not a behavior change.
        let full_window = ScissorRect {
            x: 0,
            y: 0,
            width: real_size.0,
            height: real_size.1,
        };

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
                )
                .expect("execute_frame failed");
                // REVIEW.md finding #235's further pursuit: crop the
                // (possibly coarser) swapchain buffer down to the true,
                // exact `real_size` before presenting -- via the patched
                // `winit` fork's `wp_viewport.set_source` (see
                // `tre_platform::PlatformConnection::set_viewport_source_crop`'s
                // own doc comment). Only meaningful under `Coarse` (the
                // only strategy whose swapchain can sit at a size other
                // than `real_size` at all); skipped under `Timeout` for
                // clarity, since it would always be a no-op there.
                if matches!(strategy, ResizeStrategy::Coarse) {
                    let _ = connection.set_viewport_source_crop(window, real_size.0, real_size.1);
                }
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
            // Deliberately unbounded (no acquire timeout), unlike the
            // primary acquire above: this runs immediately after
            // `surface.resize` just built a fresh, correctly-sized
            // swapchain, which is not the stale-vs-live-size mismatch
            // findings #235's own bounded acquire (Option 3) targets --
            // and this whole arm is already a rare race-condition
            // fallback, not the hot path. Still uses `real_size` for the
            // viewport crop, for the identical reason the primary acquire
            // above does.
            let acquire_start2 = Instant::now();
            let begin_result2 = match strategy {
                ResizeStrategy::Coarse => {
                    device.begin_frame_with_viewport_crop(&surface.swapchain, real_size)
                }
                ResizeStrategy::Timeout => device.begin_frame(&surface.swapchain),
            };
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
                    )
                    .expect("execute_frame failed");
                    // Same crop as the primary attempt above -- this is
                    // just its rare SwapchainOutOfDate-recovery fallback,
                    // not a different rendering path.
                    if matches!(strategy, ResizeStrategy::Coarse) {
                        let _ =
                            connection.set_viewport_source_crop(window, real_size.0, real_size.1);
                    }
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
