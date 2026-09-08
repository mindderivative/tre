//! Phase 5 Step 5.2.3 proof: the capstone of Step 5.2, closing it in
//! full. Every mechanism this demo exercises already exists and is
//! already unit-tested (`SubCanvas`/`create_sub_canvas`, Step 5.2.1;
//! `tre_memory::ScatterArena`/`FrameArena`/`stitch_into`, Step 5.2.2) --
//! this is the real-stress, real-GPU proof, the same role
//! `atlas_concurrency_demo` (Step 4.2.4) and `atlas_eviction_demo`
//! (Step 4.3.3) already played for their own features.
//!
//! `min(available_parallelism() - 1, 4)` real OS worker threads each
//! get their own `SubCanvas`: every one draws its own rect, thread `0`
//! draws inside a `begin_overlay`/`end_overlay` bracket, and the last
//! thread also draws a real atlas-backed MSDF glyph -- every thread
//! calls `stitch_into` on a shared `FrameArena` as its own last action
//! before it exits, the actual "workers stitch themselves" design
//! point Step 5.2.2 was built around. The root canvas draws one more
//! rect directly and stitches itself into the same arena. Reproduces
//! `canvas_batch_flattening_demo`'s own 3-batch proof shape (merged
//! plain rects, a standalone overlay rect, a standalone text glyph) --
//! but this time every contributing shape was recorded on a genuinely
//! concurrent thread, not by one single-threaded call sequence.

use ash::vk;
use tre_atlas::AtlasOwner;
use tre_engine::{
    execute_draw_geometry_batches, rgba8, CommandType, FrameArena, GlyphAtlasContext,
    OverlayLayerPriority, PipelineKind, PipelineRegistry, RenderingCanvas, RhiDevice,
    TextureFormat, PIPELINE_MSDF_TEXT,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const ATLAS_SIZE: u32 = 256;
const CANVAS_WIDTH: u32 = 350;
const CANVAS_HEIGHT: u32 = 150;

const RECT_SIZE: f32 = 40.0;
const RECT_Y: f32 = 20.0;
const RECT_X_STRIDE: f32 = 60.0;
const MAX_WORKERS: usize = 4;
/// Fixed regardless of the real worker count (always `<= MAX_WORKERS`
/// rects wide), so the root's own rect never collides with a worker's.
const ROOT_RECT_ORIGIN: (f32, f32) = (20.0 + MAX_WORKERS as f32 * RECT_X_STRIDE, RECT_Y);
const TEXT_ORIGIN: [f32; 2] = [20.0, 110.0];
const TEXT_PX_SIZE: f32 = 32.0;

fn worker_rect_origin(i: usize) -> (f32, f32) {
    #[allow(
        clippy::cast_precision_loss,
        reason = "MAX_WORKERS is a small constant, far below f32's exact-integer range"
    )]
    let x = 20.0 + i as f32 * RECT_X_STRIDE;
    (x, RECT_Y)
}

fn main() {
    // --- How many real worker threads can we actually use on this
    // machine? Loud, clear failure on the one genuinely degenerate
    // case (a reported single-core machine) rather than silently
    // proving less than intended. ---
    let probe_root = RenderingCanvas::new();
    assert!(
        probe_root.max_sub_canvases() >= 1,
        "this demo needs at least 1 core beyond the main thread \
         (available_parallelism() - 1 was 0 on this machine)"
    );
    let worker_count = probe_root.max_sub_canvases().min(MAX_WORKERS);
    eprintln!(
        "using {worker_count} real worker threads (available_parallelism() - 1 = {}, capped at {MAX_WORKERS})",
        probe_root.max_sub_canvases()
    );

    // --- Real shaped glyph against a real cascade font, pre-seeded
    // into the atlas synchronously before any worker thread starts --
    // this demo's own point is concurrent recording/stitching, not the
    // atlas's already-proven cache-miss behavior. ---
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

    // --- Real device/pipelines, both already-existing and unmodified ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre sub-canvas probe (never shown)", 1, 1)
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
    let swapchain = HeadlessSwapchain::new(&device, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let rect_vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled rect vertex shader");
    let rect_fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled rect fragment shader");
    let rect_pipeline = device
        .create_pipeline(
            &rect_vertex_spv,
            &rect_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create rect pipeline");

    let msdf_vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled MSDF vertex shader");
    let msdf_fragment_spv = std::fs::read(format!("{out_dir}/msdf.frag.spv"))
        .expect("failed to read compiled MSDF fragment shader");
    let msdf_pipeline = device
        .create_pipeline(
            &msdf_vertex_spv,
            &msdf_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create MSDF pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRoundedRect as u16, Box::new(rect_pipeline));
    pipelines.register(PipelineKind::MsdfText as u16, Box::new(msdf_pipeline));

    let texture = device
        .create_texture(
            ATLAS_SIZE,
            ATLAS_SIZE,
            TextureFormat::Rgba8Unorm,
            &atlas_buffer,
        )
        .expect("failed to upload the shared atlas texture");
    let texture_index = texture
        .bindless_index()
        .expect("shared atlas texture has no bindless index");

    // --- Shared destination every root/worker canvas stitches into --
    let total_shapes = worker_count + 2; // + root's own rect + the text glyph
                                         // `begin_overlay`/`end_overlay` each emit their own marker command
                                         // (PushScissor/PopScissor) -- 2 extra commands beyond the logical
                                         // shape count, though neither consumes any vertices or indices.
    let command_capacity = total_shapes + 2;
    let arena = std::sync::Arc::new(FrameArena::with_capacity(
        total_shapes * 4,
        total_shapes * 6,
        command_capacity,
        0, // this demo doesn't tag any accessibility nodes (Step 5.3.1)
    ));

    // --- Root draws its own rect directly, before any worker thread is
    // spawned -- but is only *stitched* after every `create_sub_canvas`
    // call below, since `stitch_into` consumes it by value and every
    // sub-canvas still needs to borrow it (`&root`) to be created,
    // sharing its Depth ID counter. Drawing here and creating
    // sub-canvases from the very same `root` (not a second, separate
    // `RenderingCanvas::new()`) is what keeps root and every worker on
    // one shared counter -- a real bug in this demo's own first draft:
    // a second, independent root canvas had its own independent
    // counter, so its rect's Depth ID collided with a worker's. ---
    let mut root = RenderingCanvas::new();
    root.draw_rounded_rect(
        ROOT_RECT_ORIGIN.0,
        ROOT_RECT_ORIGIN.1,
        RECT_SIZE,
        RECT_SIZE,
        0.0,
        rgba8(255, 255, 255, 255),
    );

    // --- Real worker threads, each already holding its own SubCanvas
    // (created on the main thread, then moved in). `thread::scope`
    // (not `thread::spawn`) since `font`/`shaped`/`handle` are borrowed
    // local data, not `'static` -- scoped threads are guaranteed joined
    // (and any panic re-raised) before the scope itself returns, which
    // is exactly the lifetime this demo already relies on. ---
    std::thread::scope(|scope| {
        for i in 0..worker_count {
            let mut sub = root.create_sub_canvas();
            let arena = std::sync::Arc::clone(&arena);
            let (x, y) = worker_rect_origin(i);
            let is_overlay = i == 0;
            let is_text_worker = i == worker_count - 1;
            let font = &font;
            let shaped = &shaped;
            let handle = &handle;
            scope.spawn(move || {
                let white = rgba8(255, 255, 255, 255);
                if is_overlay {
                    sub.begin_overlay(OverlayLayerPriority(0));
                    sub.draw_rounded_rect(x, y, RECT_SIZE, RECT_SIZE, 0.0, white);
                    sub.end_overlay();
                } else {
                    sub.draw_rounded_rect(x, y, RECT_SIZE, RECT_SIZE, 0.0, white);
                }
                if is_text_worker {
                    let atlas_context = GlyphAtlasContext {
                        atlas: handle,
                        texture_handle: texture_index,
                        dimensions: (ATLAS_SIZE, ATLAS_SIZE),
                        current_frame: 0,
                    };
                    sub.draw_text(
                        shaped,
                        font,
                        0,
                        TEXT_ORIGIN,
                        TEXT_PX_SIZE,
                        white,
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

    // Every `create_sub_canvas()` borrow of `root` is done now (the
    // scope above has returned), so `root` can finally be consumed.
    assert!(
        root.stitch_into(&arena),
        "arena was sized exactly for this demo"
    );

    let arena =
        std::sync::Arc::try_unwrap(arena).unwrap_or_else(|_| panic!("all workers have joined"));
    let frame = arena.flatten();

    // --- IR level: content is guaranteed, exact batch *count* is not.
    //
    // canvas_batch_flattening_demo (Step 5.1.3, one single-threaded call
    // sequence) always collapses to exactly 3 batches. This demo
    // surfaces a real property that single-threaded recording never
    // could: the overlay worker's `begin_overlay`/`end_overlay` markers
    // are hard run-segmentation barriers (Step 5.1.3's own design), and
    // *which* other threads' plain rects landed before vs. after that
    // marker pair in the concurrently-stitched `commands` array depends
    // on scheduling -- exactly ARCHITECTURE.md Section 4.2's own "soft
    // target, not a guarantee" caveat, now actually exercised instead
    // of just cited. So the plain-rect content can arrive as anywhere
    // from 1 merged batch (every plain rect's stitch happened to land
    // on the same side of the overlay marker) up to `expected_plain_
    // rects` separate ones (every plain rect split apart) -- what's
    // still an absolute guarantee is that every rect's own 6 indices
    // are accounted for *somewhere*, and that the overlay/text content
    // never merges into the wrong plane or pipeline. ---
    let draws: Vec<_> = frame
        .commands
        .iter()
        .filter(|c| c.kind == CommandType::DrawGeometry)
        .collect();

    let expected_plain_rects = worker_count; // (worker_count - 1) non-overlay + 1 root
    let plain_rect_batches: Vec<_> = draws
        .iter()
        .filter(|c| c.pipeline_state_id == 0 && c.sort_key >> 48 == 0)
        .collect();
    assert!(
        !plain_rect_batches.is_empty(),
        "at least one standard-plane rect batch must exist"
    );
    let plain_rect_total: u32 = plain_rect_batches.iter().map(|c| c.element_count).sum();
    assert_eq!(
        plain_rect_total,
        u32::try_from(expected_plain_rects * 6).unwrap(),
        "every non-overlay worker rect plus the root's own rect must be accounted for \
         somewhere, however many pieces concurrent stitching happened to split them into"
    );

    let text_batches: Vec<_> = draws
        .iter()
        .filter(|c| c.pipeline_state_id == PIPELINE_MSDF_TEXT)
        .collect();
    assert_eq!(
        text_batches.len(),
        1,
        "the one real text glyph must never merge with anything else"
    );

    let overlay_batches: Vec<_> = draws
        .iter()
        .filter(|c| c.sort_key >> 48 == 10_000)
        .collect();
    assert_eq!(
        overlay_batches.len(),
        1,
        "the one overlay rect must never merge with anything else"
    );

    assert_eq!(
        draws.len(),
        plain_rect_batches.len() + text_batches.len() + overlay_batches.len(),
        "every draw must be exactly one of: a plain-plane rect, the text glyph, or the overlay \
         rect -- nothing else should exist"
    );
    eprintln!(
        "IR level: content correct from {worker_count} concurrent threads + root -- {} batch(es) \
         for {expected_plain_rects} plain rects (concurrent stitching order is not guaranteed to \
         merge them all), 1 for text, 1 for overlay -- OK",
        plain_rect_batches.len()
    );

    // --- Real GPU render ---
    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    execute_draw_geometry_batches(
        &frame,
        &pipelines,
        &vertex_buffer,
        &index_buffer,
        &mut *cmd_buffer,
    );
    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let background = pixel_at(0, 0);

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
    )]
    for i in 0..worker_count {
        let (x, y) = worker_rect_origin(i);
        let center = pixel_at((x + RECT_SIZE / 2.0) as u32, (y + RECT_SIZE / 2.0) as u32);
        assert_eq!(
            center,
            [255, 255, 255, 255],
            "worker thread {i}'s rect must render white at its own center"
        );
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
    )]
    let root_center = pixel_at(
        (ROOT_RECT_ORIGIN.0 + RECT_SIZE / 2.0) as u32,
        (ROOT_RECT_ORIGIN.1 + RECT_SIZE / 2.0) as u32,
    );
    assert_eq!(
        root_center,
        [255, 255, 255, 255],
        "the root canvas's own rect must render white at its own center"
    );
    if worker_count > 1 {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
        )]
        let between_first_two = pixel_at((20.0 + RECT_SIZE + 10.0) as u32, (RECT_Y + 20.0) as u32);
        assert_eq!(
            between_first_two, background,
            "the gap between two adjacent worker rects must stay background"
        );
    }
    eprintln!(
        "all {worker_count} worker rects + the root's own rect rendered correctly, at their own \
         distinct positions -- OK"
    );

    let half = TEXT_PX_SIZE / 2.0;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
    )]
    let (tx0, ty0, tx1, ty1) = (
        (TEXT_ORIGIN[0] - half) as u32,
        (TEXT_ORIGIN[1] - TEXT_PX_SIZE) as u32,
        (TEXT_ORIGIN[0] + half) as u32,
        TEXT_ORIGIN[1] as u32,
    );
    let text_found_fill = (tx0..tx1)
        .step_by(2)
        .flat_map(|x| (ty0..ty1).step_by(2).map(move |y| (x, y)))
        .any(|(x, y)| pixel_at(x, y) != background);
    assert!(
        text_found_fill,
        "the text glyph, drawn on a worker thread, must have rendered real, non-background \
         pixels"
    );
    eprintln!("text glyph (drawn on a worker thread) rendered real, non-background pixels -- OK");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_CANVAS_SUB_CANVAS_OUTPUT")
        .unwrap_or_else(|_| "canvas_sub_canvas_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} sub-canvas render to {out_path}");
    eprintln!("all canvas sub-canvas assertions passed");
}
