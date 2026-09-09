//! Phase 5 Step 5.1.3 proof: the capstone of Step 5.1 -- real batch
//! flattening reproducing DESIGN.md Section 8's own worked example
//! (`Rect1(P1,Tex0) -> Text(P2,AtlasA) -> Rect2(P1,Tex0) -> OverlayRect
//! (P1,Tex0)` collapsing into exactly 3 real dispatched batches: Rect1
//! and Rect2 merge (same Layer/Pipeline/Texture/clip), Text stays alone
//! (a different pipeline), and OverlayRect stays alone despite sharing
//! Rect1/Rect2's own pipeline and texture (a different Layer ID, via
//! `begin_overlay`).
//!
//! This is the first demo in this codebase to record more than one real
//! `draw_indexed` call (and more than one pipeline bind) within a single
//! frame: it walks `flatten()`'s own real, sorted-and-merged
//! `commands`, switching between the existing `sdf_rounded_rect`
//! pipeline and the existing `bindless_textured.vert`/`msdf.frag`
//! pipeline per command's own `pipeline_state_id` -- proving the merged
//! batches are not just an IR-level count but still render every one of
//! the 4 logical shapes at its own correct, distinct position and color.

use ash::vk;
use tre_atlas::AtlasOwner;
use tre_engine::{
    execute_frame, rgba8, BufferBinding, CommandType, GlyphAtlasContext, OverlayLayerPriority,
    PipelineKind, PipelineRegistry, RenderingCanvas, RhiDevice, ScissorRect, TextureFormat,
    PIPELINE_MSDF_TEXT,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const ATLAS_SIZE: u32 = 256;
const CANVAS_WIDTH: u32 = 200;
const CANVAS_HEIGHT: u32 = 140;

// Non-overlapping 40x40 rects: Rect1 top-left, Rect2 top-right, OverlayRect
// bottom-left. Text sits bottom-right, clear of all three.
const RECT1_ORIGIN: (f32, f32) = (10.0, 10.0);
const RECT2_ORIGIN: (f32, f32) = (100.0, 10.0);
const OVERLAY_RECT_ORIGIN: (f32, f32) = (10.0, 70.0);
const RECT_SIZE: f32 = 40.0;
const TEXT_ORIGIN: [f32; 2] = [110.0, 110.0];
const TEXT_PX_SIZE: f32 = 32.0;

fn main() {
    // --- Real shaped glyph against a real cascade font, for the Text
    // batch -- same pattern canvas_draw_text_demo already established ---
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");
    let face = rustybuzz::Face::from_slice(&font_bytes, 0)
        .expect("primary cascade font invalid for rustybuzz");
    let runs = tre_text::shape_text(&face, "A").expect("shaping failed");
    let shaped = &runs[0];
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
        .create_window("tre canvas batch flattening probe (never shown)", 1, 1)
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

    // --- Record DESIGN.md Section 8's exact worked example ---
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();
    canvas.draw_rounded_rect(
        RECT1_ORIGIN.0,
        RECT1_ORIGIN.1,
        RECT_SIZE,
        RECT_SIZE,
        0.0,
        white,
    ); // Rect1
    let atlas_context = GlyphAtlasContext {
        atlas: &handle,
        texture_handle: texture_index,
        dimensions: (ATLAS_SIZE, ATLAS_SIZE),
        current_frame: 0,
    };
    canvas.draw_text(
        shaped,
        &font,
        0,
        TEXT_ORIGIN,
        TEXT_PX_SIZE,
        white,
        &atlas_context,
    ); // Text
    canvas.draw_rounded_rect(
        RECT2_ORIGIN.0,
        RECT2_ORIGIN.1,
        RECT_SIZE,
        RECT_SIZE,
        0.0,
        white,
    ); // Rect2
    canvas.begin_overlay(OverlayLayerPriority(0));
    canvas.draw_rounded_rect(
        OVERLAY_RECT_ORIGIN.0,
        OVERLAY_RECT_ORIGIN.1,
        RECT_SIZE,
        RECT_SIZE,
        0.0,
        white,
    ); // OverlayRect
    canvas.end_overlay();
    let frame = canvas.flatten();

    // --- IR-level: exactly 3 batches, matching the doc's own diagram ---
    let draws: Vec<_> = frame
        .commands
        .iter()
        .filter(|c| c.kind == CommandType::DrawGeometry)
        .collect();
    assert_eq!(
        draws.len(),
        3,
        "DESIGN.md Section 8's worked example must collapse to exactly 3 batches, got {}: {draws:?}",
        draws.len()
    );
    let rect_batch = draws
        .iter()
        .find(|c| c.pipeline_state_id == 0 && c.sort_key >> 48 == 0)
        .expect("standard-plane rect batch must exist");
    assert_eq!(
        rect_batch.element_count, 12,
        "Rect1+Rect2 must merge into one 12-index batch"
    );
    draws
        .iter()
        .find(|c| c.pipeline_state_id == PIPELINE_MSDF_TEXT)
        .expect("text batch must exist");
    draws
        .iter()
        .find(|c| c.pipeline_state_id == 0 && c.sort_key >> 48 == 10_000)
        .expect("overlay-plane rect batch must exist");
    eprintln!("IR level: exactly 3 batches, as DESIGN.md Section 8 predicts -- OK");

    // --- Real GPU render: walk the real, sorted-and-merged commands,
    // switching pipeline/texture per command's own fields -- the first
    // demo in this codebase issuing more than one draw_indexed call in
    // a single frame ---
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

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
    };
    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    execute_frame(
        &frame,
        &pipelines,
        BufferBinding {
            buffer: &vertex_buffer,
            offset: 0,
        },
        BufferBinding {
            buffer: &index_buffer,
            offset: 0,
        },
        &full_window,
        &device,
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

    let rect1_center = pixel_at(30, 30);
    assert_eq!(
        rect1_center,
        [255, 255, 255, 255],
        "Rect1 must render white at its own center"
    );
    let rect2_center = pixel_at(120, 30);
    assert_eq!(
        rect2_center,
        [255, 255, 255, 255],
        "Rect2 must render white at its own center"
    );
    let overlay_rect_center = pixel_at(30, 90);
    assert_eq!(
        overlay_rect_center,
        [255, 255, 255, 255],
        "OverlayRect must render white at its own center"
    );
    let between_rect1_and_rect2 = pixel_at(70, 30);
    assert_eq!(
        between_rect1_and_rect2, background,
        "the gap between Rect1 and Rect2 must stay background -- proves they're two distinct \
         shapes, not one overdrawn blob"
    );
    eprintln!(
        "all 3 SDF rects (merged Rect1+Rect2, standalone OverlayRect) rendered correctly -- OK"
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
        "the text glyph must have rendered real, non-background pixels"
    );
    eprintln!("text glyph rendered real, non-background pixels -- OK");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_CANVAS_BATCH_FLATTENING_OUTPUT")
        .unwrap_or_else(|_| "canvas_batch_flattening_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} canvas batch flattening render to {out_path}");
    eprintln!("all canvas batch flattening assertions passed");
}
