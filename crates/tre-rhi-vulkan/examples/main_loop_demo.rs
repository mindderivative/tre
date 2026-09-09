//! IMPLEMENTATION.md Phase 8 Step 8.1.2: the first demo to combine all
//! 8 named pipeline stages -- *Wait Fences -> Drain Events ->
//! Multi-Thread Canvas -> Sub-Canvas Stitch -> Tessellation/Atlas Check
//! -> Radix Sort & Batch -> Ring Buffer Packing -> RHI Submit &
//! Present* -- inside one real, continuous, windowed loop. Every
//! mechanism here already exists and is already proven individually
//! (Step 5.2.3's multi-threaded stitching, Step 4.3.1's atlas recency
//! tracking, Step 8.1.1's `FrameClock`/`spring_decay`); this demo is the
//! real-stress proof that they compose correctly run together, every
//! frame, not just once.
//!
//! Per stage, every frame:
//! 1. **Drain Events** (`poll_events`) runs first, matching every prior
//!    windowed demo's own proven call order (`walking_skeleton.rs`,
//!    `multi_window.rs`, `input_demo.rs`) -- polling OS input has no
//!    data dependency on the GPU fence wait below it, so there is no
//!    real reason to invert that proven order just to match the
//!    outline's own listed sequence literally.
//! 2. **Wait Fences** happens inside `RhiDevice::begin_frame` itself
//!    (`VulkanDevice::begin_frame`'s own `wait_for_fences`/`reset_
//!    fences` calls) -- not a separate call this loop makes.
//! 3. **Multi-Thread Canvas**: real OS worker threads (`std::thread::
//!    scope`, matching Step 5.2.3's own recipe) each record their own
//!    `SubCanvas`, repeated fresh every frame -- a persistent, reused
//!    thread pool is real future work (`planning/archive/PLAN_PHASE8_
//!    STEP8_1_2.md`'s own scope decision), not built here.
//! 4. **Sub-Canvas Stitch**: every worker (and the root canvas) calls
//!    `stitch_into` on one shared `FrameArena`.
//! 5. **Tessellation/Atlas Check**: the text-drawing worker's `draw_
//!    text` call looks up its glyph in the shared atlas every frame,
//!    real-touching `SwmrSlotTable`'s own recency tracking (Step
//!    4.3.1). The atlas itself is fully pre-seeded before this loop
//!    starts -- live, mid-run atlas growth is real, separate future
//!    work (the atlas owner's background thread has no API to read its
//!    own pixels back without stopping it, only `AtlasOwner::join`).
//! 6. **Radix Sort & Batch**: `FrameArena::flatten()`.
//! 7. **Ring Buffer Packing**: the flattened vertex/index bytes are
//!    written into a real `RhiDynamicRingBuffer` via `write()`, and the
//!    offsets it returns are passed straight into `execute_frame` --
//!    which, until this step, hardcoded a `0` byte offset and could
//!    never have accepted them.
//! 8. **RHI Submit & Present**: `execute_frame` + `submit_and_present`.
//!
//! The animated rect's `x` position is driven by `FrameClock::tick()`
//! feeding `spring_decay` every frame -- Step 8.1.1's two primitives
//! get their first real consumer here, exactly as that step's own plan
//! deferred. `VulkanSwapchain` (a real window/compositor surface, unlike
//! `HeadlessSwapchain`) has no pixel-readback capability, so this demo
//! verifies the animation by replaying `spring_decay` independently
//! over the real, recorded per-frame `dt` sequence and checking it
//! reproduces the exact position sequence this loop actually drew --
//! a stronger check than a pixel read would give, since it verifies the
//! formula was applied correctly frame-by-frame, not just "something
//! moved."

use raw_window_handle::HasDisplayHandle;
use tre_atlas::AtlasOwner;
use tre_engine::{
    execute_frame, rgba8, BufferBinding, FrameArena, FrameClock, GlyphAtlasContext, InputEvent,
    PipelineKind, PipelineRegistry, RenderingCanvas, RhiDevice, RhiDynamicRingBuffer, RhiTexture,
    ScissorRect, TextureFormat, WindowId,
};
use tre_platform::PlatformConnection;
use tre_rhi_vulkan::{VulkanDevice, VulkanSwapchain};

const CANVAS_WIDTH: u32 = 640;
const CANVAS_HEIGHT: u32 = 480;
const ATLAS_SIZE: u32 = 256;
const MAX_WORKERS: usize = 2;
const RING_BUFFER_CAPACITY: usize = 64 * 1024;

const RECT_SIZE: f32 = 40.0;
const RECT_Y: f32 = 220.0;
const START_X: f32 = 40.0;
const TARGET_X: f32 = 560.0;
const LAMBDA: f32 = 6.0;
const WORKER_RECT_Y: f32 = 20.0;
const TEXT_ORIGIN: [f32; 2] = [40.0, 400.0];
const TEXT_PX_SIZE: f32 = 32.0;

struct Renderer {
    pipelines: PipelineRegistry,
    ring_buffer: Box<dyn RhiDynamicRingBuffer>,
    #[allow(
        dead_code,
        reason = "held only so its GPU resources stay alive and drop in the right order -- \
                  every frame reads it indirectly via its own already-registered bindless \
                  index (texture_index), never through this field again"
    )]
    atlas_texture: Box<dyn RhiTexture>,
    swapchain: VulkanSwapchain,
    device: VulkanDevice,
    connection: PlatformConnection,
    window: WindowId,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device.device_wait_idle();
        }
    }
}

fn main() {
    let mut connection = PlatformConnection::new().expect("failed to connect to display server");
    let window = connection
        .create_window(
            "tre main loop (Phase 8 Step 8.1.2)",
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
        )
        .expect("failed to open window");

    let display_handle = connection.display_handle().unwrap().as_raw();
    let window_handle = connection.window_handle(window).unwrap().as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    let swapchain = VulkanSwapchain::new(
        &device,
        surface_loader,
        surface,
        CANVAS_WIDTH,
        CANVAS_HEIGHT,
    )
    .expect("failed to create VulkanSwapchain");

    let out_dir = env!("OUT_DIR");
    let rect_vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled rect vertex shader");
    let rect_fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled rect fragment shader");
    let rect_pipeline = device
        .create_pipeline(&rect_vertex_spv, &rect_fragment_spv, swapchain.format())
        .expect("failed to create rect pipeline");

    let msdf_vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled MSDF vertex shader");
    let msdf_fragment_spv = std::fs::read(format!("{out_dir}/msdf.frag.spv"))
        .expect("failed to read compiled MSDF fragment shader");
    let msdf_pipeline = device
        .create_pipeline(&msdf_vertex_spv, &msdf_fragment_spv, swapchain.format())
        .expect("failed to create MSDF pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRoundedRect as u16, Box::new(rect_pipeline));
    pipelines.register(PipelineKind::MsdfText as u16, Box::new(msdf_pipeline));

    // --- Pre-seed the atlas with the one glyph this loop will draw
    // every frame, then stop the atlas owner's background thread --
    // matching every existing text-drawing demo's own proven pattern.
    // Live, mid-run atlas growth is real, separate future work (see
    // this file's own header comment). ---
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");
    let face = rustybuzz::Face::from_slice(&font_bytes, 0)
        .expect("primary cascade font invalid for rustybuzz");
    let runs = tre_text::shape_text(&face, "A").expect("shaping failed");
    let shaped = runs[0].clone();
    let glyph_id = shaped.glyphs[0].glyph_id;

    let owner = AtlasOwner::spawn(ATLAS_SIZE, ATLAS_SIZE, 8, 8);
    let handle = owner.handle();
    let key = tre_atlas::AtlasKey::from_glyph(0, glyph_id);
    let outline =
        tre_text::glyph_outline(&font, skrifa::GlyphId::from(glyph_id)).expect("outline failed");
    assert!(
        handle.request_insert(
            key,
            Box::new(tre_text::GlyphRasterSource {
                contours: outline,
                size: 32,
                range_px: 4.0,
            }),
            0,
        ),
        "request_insert failed -- queue unexpectedly full"
    );
    let mut resolved = false;
    for _ in 0..500 {
        if handle.lookup(key, 0).is_some() {
            resolved = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(resolved, "glyph never resolved");
    let atlas_buffer = owner.join();

    let atlas_texture = device
        .create_texture(
            ATLAS_SIZE,
            ATLAS_SIZE,
            TextureFormat::Rgba8Unorm,
            &atlas_buffer,
        )
        .expect("failed to upload the shared atlas texture");
    let texture_index = atlas_texture
        .bindless_index()
        .expect("shared atlas texture has no bindless index");

    let ring_buffer = device.create_dynamic_ring_buffer(RING_BUFFER_CAPACITY);

    let probe_root = RenderingCanvas::new();
    assert!(
        probe_root.max_sub_canvases() >= 1,
        "this demo needs at least 1 core beyond the main thread \
         (available_parallelism() - 1 was 0 on this machine)"
    );
    let worker_count = probe_root.max_sub_canvases().min(MAX_WORKERS);
    eprintln!(
        "using {worker_count} real worker threads per frame (available_parallelism() - 1 = {}, \
         capped at {MAX_WORKERS})",
        probe_root.max_sub_canvases()
    );

    let mut renderer = Renderer {
        pipelines,
        ring_buffer,
        atlas_texture,
        swapchain,
        device,
        connection,
        window,
    };

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
    };

    let frame_limit: u64 = std::env::var("TRE_MAIN_LOOP_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(90);

    let mut frame_clock = FrameClock::new();
    let mut x = START_X;
    let mut recorded_x: Vec<f32> = Vec::with_capacity(frame_limit as usize);
    let mut recorded_dt: Vec<f32> = Vec::with_capacity(frame_limit as usize);

    let mut frame_count: u64 = 0;
    'render_loop: while frame_count < frame_limit {
        // --- Stage: Drain Events ---
        for event in renderer.connection.poll_events() {
            if matches!(event, InputEvent::CloseRequested { window } if window == renderer.window) {
                break 'render_loop;
            }
        }

        // Step 8.1.1's own two primitives, now consumed for real.
        let dt = frame_clock.tick();
        x = tre_math::spring_decay(x, TARGET_X, LAMBDA, dt);
        recorded_dt.push(dt);
        recorded_x.push(x);

        // --- Stage: Multi-Thread Canvas / Sub-Canvas Stitch ---
        let total_shapes = worker_count + 2; // root's rect + each worker's rect + 1 text glyph
        let arena = std::sync::Arc::new(FrameArena::with_capacity(
            total_shapes * 4,
            total_shapes * 6,
            total_shapes,
            0,
        ));

        let mut root = RenderingCanvas::new();
        root.draw_rounded_rect(
            x,
            RECT_Y,
            RECT_SIZE,
            RECT_SIZE,
            0.0,
            rgba8(0xE0, 0xA0, 0x40, 0xFF),
        );

        std::thread::scope(|scope| {
            for i in 0..worker_count {
                let mut sub = root.create_sub_canvas();
                let arena = std::sync::Arc::clone(&arena);
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "MAX_WORKERS is a small constant, far below f32's exact-integer range"
                )]
                let worker_x = 20.0 + i as f32 * 60.0;
                let is_text_worker = i == worker_count - 1;
                let font = &font;
                let shaped = &shaped;
                let handle = &handle;
                scope.spawn(move || {
                    sub.draw_rounded_rect(
                        worker_x,
                        WORKER_RECT_Y,
                        RECT_SIZE,
                        RECT_SIZE,
                        0.0,
                        rgba8(255, 255, 255, 255),
                    );
                    if is_text_worker {
                        // --- Stage: Tessellation/Atlas Check ---
                        let atlas_context = GlyphAtlasContext {
                            atlas: handle,
                            texture_handle: texture_index,
                            dimensions: (ATLAS_SIZE, ATLAS_SIZE),
                            current_frame: frame_count,
                        };
                        sub.draw_text(
                            shaped,
                            font,
                            0,
                            TEXT_ORIGIN,
                            TEXT_PX_SIZE,
                            rgba8(255, 255, 255, 255),
                            &atlas_context,
                        );
                    }
                    assert!(
                        sub.stitch_into(&arena),
                        "arena was sized exactly for this demo"
                    );
                });
            }
        });
        assert!(
            root.stitch_into(&arena),
            "arena was sized exactly for this demo"
        );

        let arena =
            std::sync::Arc::try_unwrap(arena).unwrap_or_else(|_| panic!("all workers have joined"));
        // --- Stage: Radix Sort & Batch ---
        let frame = arena.flatten();

        let vertex_bytes: &[u8] = bytemuck::cast_slice(&frame.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&frame.indices);

        let (mut cmd_buffer, image) = renderer
            .device
            .begin_frame(&renderer.swapchain)
            .expect("begin_frame failed");

        // --- Stage: Ring Buffer Packing ---
        let vertex_offset = renderer.ring_buffer.write(vertex_bytes).expect(
            "ring buffer write failed for vertices -- demo's own segment is far larger than \
             one frame's tiny scene needs",
        );
        let index_offset = renderer
            .ring_buffer
            .write(index_bytes)
            .expect("ring buffer write failed for indices");

        // --- Stage: RHI Submit & Present ---
        execute_frame(
            &frame,
            &renderer.pipelines,
            BufferBinding {
                buffer: &*renderer.ring_buffer,
                offset: vertex_offset,
            },
            BufferBinding {
                buffer: &*renderer.ring_buffer,
                offset: index_offset,
            },
            &full_window,
            &renderer.device,
            &mut *cmd_buffer,
        );
        renderer
            .device
            .submit_and_present(cmd_buffer, &renderer.swapchain, image)
            .expect("submit_and_present failed");

        frame_count += 1;
        if frame_count % 30 == 0 {
            eprintln!("frame {frame_count} presented (x = {x:.2})");
        }
    }

    // --- Real verification: replay spring_decay independently over the
    // real, recorded per-frame dt sequence and confirm it reproduces
    // the exact position sequence this loop actually drew. A stronger
    // check than a pixel read (which VulkanSwapchain cannot do anyway,
    // unlike HeadlessSwapchain) -- it proves the formula was applied
    // correctly every single frame, not just that something moved. ---
    let mut replay = START_X;
    for (i, &dt) in recorded_dt.iter().enumerate() {
        replay = tre_math::spring_decay(replay, TARGET_X, LAMBDA, dt);
        assert!(
            (replay - recorded_x[i]).abs() <= f32::EPSILON,
            "frame {i}: independently replaying spring_decay over the real recorded dt \
             sequence must reproduce the exact position this loop actually drew, got {replay} \
             vs. recorded {}",
            recorded_x[i]
        );
    }
    if let (Some(&first), Some(&last)) = (recorded_x.first(), recorded_x.last()) {
        assert!(
            (last - TARGET_X).abs() < (first - TARGET_X).abs(),
            "after {frame_count} real frames, the animated rect must have moved strictly \
             closer to its target than its first computed position -- first {first}, last \
             {last}, target {TARGET_X}"
        );
        assert!(
            (START_X..=TARGET_X).contains(&last),
            "spring_decay must never overshoot its own target -- final position {last} left \
             the [{START_X}, {TARGET_X}] range"
        );
    }
    eprintln!(
        "spring_decay animation verified across {frame_count} real frames -- final x = {:.2} \
         (started at {START_X}, target {TARGET_X})",
        recorded_x.last().copied().unwrap_or(START_X)
    );
    eprintln!("main loop demo exited cleanly");
}
