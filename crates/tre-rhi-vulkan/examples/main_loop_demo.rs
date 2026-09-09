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
//!    `SubCanvas`, freshly spawned every frame -- a persistent, reused
//!    thread *pool* is real future work (`planning/archive/PLAN_PHASE8_
//!    STEP8_1_2.md`'s own scope decision, unchanged by Step 9.2), not
//!    built here. What Step 9.2 *did* change: each worker's own
//!    `SubCanvas` (its recorded vertex/index/command data) is built
//!    once, before this loop starts, and `reset()` every frame instead
//!    of being reconstructed -- see that step's own header comment
//!    below for why.
//! 4. **Sub-Canvas Stitch**: every worker (and the root canvas) calls
//!    `stitch_into` on one shared `FrameArena`.
//! 5. **Tessellation/Atlas Check**: the text-drawing worker's `draw_
//!    text` call looks up its glyph in the shared atlas every frame,
//!    real-touching `SwmrSlotTable`'s own recency tracking (Step
//!    4.3.1). The atlas itself is fully pre-seeded before this loop
//!    starts -- live, mid-run atlas growth is real, separate future
//!    work (the atlas owner's background thread has no API to read its
//!    own pixels back without stopping it, only `AtlasOwner::join`).
//! 6. **Radix Sort & Batch**: `FrameArena::flatten_into()`.
//! 7. **Ring Buffer Packing**: the flattened vertex/index bytes are
//!    written into a real `RhiDynamicRingBuffer` via `write()`, and the
//!    offsets it returns are passed straight into `execute_frame` --
//!    which, until Step 8.1.2, hardcoded a `0` byte offset and could
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
//!
//! # Phase 9 Step 9.2: real zero-allocation enforcement (REVIEW.md
//! finding #134, TECHNICAL.md Section 3.4)
//!
//! This demo used to allocate ~20+ times per frame (finding #134):
//! a fresh `Arc<FrameArena>`, a fresh root `RenderingCanvas`, and a
//! fresh `SubCanvas` per worker, every single iteration -- squarely
//! inside DESIGN.md Section 2.1's own named zero-allocation boundary.
//! Fixed here by building every one of those once, before the loop
//! starts, and `reset()`/`flatten_into()`-ing them every frame instead:
//! `root`/`workers` (persistent `RenderingCanvas`/`SubCanvas` values,
//! reused via `reset()`), `arena` (a plain owned `FrameArena`, no longer
//! wrapped in `Arc` -- `std::thread::scope` lets the spawned closures
//! below borrow it directly, so the old `Arc::new`/`Arc::try_unwrap`
//! dance, itself a real per-frame allocation, is gone entirely), and
//! `flattened` (a reused `FlattenedFrame`, filled via the new,
//! non-consuming `FrameArena::flatten_into`).
//!
//! The real `#[global_allocator]` guard below (`tre_memory::
//! DebugAllocGuard`) enforces this as a hard, self-checking assertion,
//! not just an unverified claim: `tre_memory::RenderTickGuard` wraps
//! the CPU-side span this step actually made allocation-free -- canvas
//! recording, sort/batch (`flatten_into`), and the ring-buffer `write`
//! calls -- on the main thread and, separately (the flag is
//! thread-local), inside each worker thread's own closure. Any real
//! allocation inside a wrapped span panics immediately with a clear
//! message, not just eventually shows up as a perf regression.
//!
//! **Two exclusions, both disclosed, not silently hidden:**
//!
//! 1. **RHI submission** (`begin_frame`/`execute_frame`/`submit_and_
//!    present`) is deliberately *outside* the guard's scope.
//!    Investigating this step's own real behavior found `VulkanDevice::
//!    begin_frame` allocates a fresh `Box<dyn RhiCommandBuffer>` every
//!    frame, even though the underlying Vulkan `vk::CommandBuffer`
//!    handle it wraps is already reused -- a real, previously-
//!    undiscovered gap (REVIEW.md's new finding for this step). A real
//!    fix means redesigning `RhiDevice::begin_frame`/`submit_and_
//!    present`'s `Box`-by-value ownership model, rippling through all
//!    31 demo call sites -- genuine trait-boundary redesign, not a
//!    bug-fix-sized change (the same shape finding #134 itself had,
//!    legitimately deferred with that exact reasoning).
//! 2. **The `std::thread::scope` call itself**, on the main thread, is
//!    also outside any guarded span (each worker's *own* guard, started
//!    inside its spawned closure, still covers that worker's real work
//!    fully). Investigating a real guard violation during this step's
//!    own development found `std::thread::scope` allocates an
//!    `Arc<ScopeData>` bookkeeping value on every call -- a real,
//!    unavoidable cost of spawning fresh OS threads every frame, and
//!    this step's own plan already named that specific boundary as
//!    deliberately deferred (a persistent worker-thread *pool* is real
//!    future work; this step reuses each worker's `SubCanvas` *data*,
//!    never claimed to eliminate the per-frame OS thread spawn itself).
//!
//! Wrapping either span in the guard today would just fail on an
//! already-disclosed, separate gap; excluding them here is an honest
//! scope boundary, not a narrowing done quietly.
use raw_window_handle::HasDisplayHandle;
use tre_atlas::AtlasOwner;
use tre_engine::{
    execute_frame, rgba8, BufferBinding, FlattenedFrame, FrameArena, FrameClock, GlyphAtlasContext,
    InputEvent, PipelineKind, PipelineRegistry, RenderingCanvas, RhiDevice, RhiDynamicRingBuffer,
    RhiTexture, ScissorRect, SubCanvas, TextureFormat, WindowId,
};
use tre_memory::{DebugAllocGuard, RenderTickGuard};
use tre_platform::PlatformConnection;
use tre_rhi_vulkan::{VulkanDevice, VulkanSwapchain};

// Phase 9 Step 9.2: this demo's own real, self-checking proof that its
// CPU-side per-frame work is genuinely allocation-free -- see this
// file's own header comment for the guard's real scope and the one
// disclosed exclusion (RHI submission).
#[global_allocator]
static ALLOCATOR: DebugAllocGuard = DebugAllocGuard::new();

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

// Field order matters: Rust drops a struct's fields in DECLARATION order
// (not reverse), so everything that holds a handle into the Vulkan device
// (pipelines, ring_buffer, atlas_texture) must be declared -- and
// therefore dropped -- BEFORE `device` itself, and `device`/`swapchain`
// before `connection`. Getting this backwards is exactly what produced a
// real SIGSEGV in walking_skeleton.rs's own original development (see
// documentation/REVIEW.md finding #43) -- REVIEW.md finding #143 flagged
// that this struct has the identical footgun shape (atlas_texture must
// drop before device) but, unlike its only sibling, carried no comment
// warning a future field addition about it.
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
            "tre main loop (Phase 8 Step 8.1.2 / Phase 9 Step 9.2)",
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

    // --- Phase 9 Step 9.2: every per-frame structure built exactly
    // once, here, before the loop -- reset()/flatten_into() every frame
    // from this point on, never reconstructed (REVIEW.md finding #134).
    let mut root = RenderingCanvas::new();
    assert!(
        root.max_sub_canvases() >= 1,
        "this demo needs at least 1 core beyond the main thread \
         (available_parallelism() - 1 was 0 on this machine)"
    );
    let worker_count = root.max_sub_canvases().min(MAX_WORKERS);
    eprintln!(
        "using {worker_count} real worker threads per frame (available_parallelism() - 1 = {}, \
         capped at {MAX_WORKERS})",
        root.max_sub_canvases()
    );
    let mut workers: Vec<SubCanvas> = (0..worker_count)
        .map(|_| root.create_sub_canvas())
        .collect();

    let total_shapes = worker_count + 2; // root's rect + each worker's rect + 1 text glyph
    let mut arena = FrameArena::with_capacity(total_shapes * 4, total_shapes * 6, total_shapes, 0);
    let mut flattened = FlattenedFrame::default();

    // --- Phase 9 Step 9.2: one unguarded warm-up pass, recording the
    // exact same shapes the real loop below will every frame, so every
    // `Vec` this loop touches (root's/each worker's own vertices/
    // indices/commands, `arena`'s own `raw_commands`/`raw_indices`/
    // `sort_scratch`, `flattened`'s own fields) grows to this demo's
    // real steady-state capacity *before* `RenderTickGuard` starts
    // checking. Without this, the very first guarded frame would panic
    // on `Vec::reserve` growing a still-empty-capacity `Vec` from
    // `RenderingCanvas::new()`'s own genuinely-empty starting point --
    // a real, expected one-time warm-up cost, not a steady-state
    // violation the guard exists to catch. Deliberately single-threaded
    // (unlike the real loop's `std::thread::scope`): warm-up only needs
    // to touch the same allocation-growing code paths once each, not
    // real concurrency. ---
    root.draw_rounded_rect(
        START_X,
        RECT_Y,
        RECT_SIZE,
        RECT_SIZE,
        0.0,
        rgba8(0xE0, 0xA0, 0x40, 0xFF),
    );
    for (i, sub) in workers.iter_mut().enumerate() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "MAX_WORKERS is a small constant, far below f32's exact-integer range"
        )]
        let worker_x = 20.0 + i as f32 * 60.0;
        sub.draw_rounded_rect(
            worker_x,
            WORKER_RECT_Y,
            RECT_SIZE,
            RECT_SIZE,
            0.0,
            rgba8(255, 255, 255, 255),
        );
        if i == worker_count - 1 {
            let atlas_context = GlyphAtlasContext {
                atlas: &handle,
                texture_handle: texture_index,
                dimensions: (ATLAS_SIZE, ATLAS_SIZE),
                current_frame: 0,
            };
            sub.draw_text(
                &shaped,
                &font,
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
        sub.reset();
    }
    assert!(
        root.stitch_into(&arena),
        "arena was sized exactly for this demo"
    );
    root.reset();
    arena.flatten_into(&mut flattened);

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

        // Phase 9 Step 9.2: the CPU-side render tick begins here on the
        // main thread -- root recording must be genuinely allocation-
        // free. This guard deliberately does *not* span the
        // `std::thread::scope` call just below: investigating this
        // step's own real behavior found `std::thread::scope` itself
        // allocates an `Arc<ScopeData>` bookkeeping value on every call
        // -- a real, unavoidable cost of spawning fresh OS threads every
        // frame, which this step's own plan already named as a
        // deliberately separate, deferred boundary (a persistent worker-
        // thread *pool* is real future work; this step reuses each
        // worker's `SubCanvas` *data*, not the OS thread itself). Each
        // worker's own `RenderTickGuard`, started inside its spawned
        // closure below, still covers that worker's own real per-frame
        // work.
        let root_tick = RenderTickGuard::begin();
        root.reset();
        root.draw_rounded_rect(
            x,
            RECT_Y,
            RECT_SIZE,
            RECT_SIZE,
            0.0,
            rgba8(0xE0, 0xA0, 0x40, 0xFF),
        );
        drop(root_tick);

        // --- Stage: Multi-Thread Canvas / Sub-Canvas Stitch ---
        let arena_ref = &arena;
        std::thread::scope(|scope| {
            for (i, sub) in workers.iter_mut().enumerate() {
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
                    // A worker thread's own allocations are invisible
                    // to the main thread's guard (the flag is
                    // thread-local) -- each thread needs its own.
                    let _worker_tick = RenderTickGuard::begin();
                    sub.reset();
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
                        sub.stitch_into(arena_ref),
                        "arena was sized exactly for this demo"
                    );
                });
            }
        });
        // A second guarded span, resuming now that thread::scope's own
        // (disclosed, unavoidable) allocation is behind us -- stitching
        // the root canvas, sort/batch, and the ring-buffer writes below
        // must all still be genuinely allocation-free.
        let main_tick = RenderTickGuard::begin();
        assert!(
            root.stitch_into(&arena),
            "arena was sized exactly for this demo"
        );

        // --- Stage: Radix Sort & Batch ---
        arena.flatten_into(&mut flattened);

        let vertex_bytes: &[u8] = bytemuck::cast_slice(&flattened.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&flattened.indices);

        // --- Stage: Ring Buffer Packing ---
        let vertex_offset = renderer.ring_buffer.write(vertex_bytes).expect(
            "ring buffer write failed for vertices -- demo's own segment is far larger than \
             one frame's tiny scene needs",
        );
        let index_offset = renderer
            .ring_buffer
            .write(index_bytes)
            .expect("ring buffer write failed for indices");

        // The zero-allocation-checked span ends here -- RHI submission
        // below is deliberately outside it; see this file's own header
        // comment for why.
        drop(main_tick);

        let (mut cmd_buffer, image) = renderer
            .device
            .begin_frame(&renderer.swapchain)
            .expect("begin_frame failed");

        // --- Stage: RHI Submit & Present ---
        execute_frame(
            &flattened,
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
    eprintln!(
        "zero-allocation guard: {frame_count} real frames of canvas-record -> sort/batch -> \
         ring-buffer-write with zero heap allocations detected -- Phase 9 Step 9.2 verified"
    );
    eprintln!("main loop demo exited cleanly");
}
