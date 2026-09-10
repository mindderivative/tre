//! Phase 5 Step 5.1.2 proof: `Canvas::draw_text`, `tre-engine`'s first
//! real wiring into `tre-text`/`tre-atlas`. A real cascade font shapes a
//! real word ("TEXT") into one `ShapedRun`; `draw_text` is called twice
//! against the same real `AtlasOwner` background thread, proving both
//! halves of its documented cache contract:
//!
//! - **Frame 1** (every glyph a cache miss): `draw_text` fires a real
//!   `request_insert` per distinct glyph and emits zero commands --
//!   nothing renders yet, exactly PLAN.md's "report, don't block, render
//!   nothing this frame" contract.
//! - **Frame 2** (every glyph now resolved, after polling the real
//!   background thread to completion): a fresh `Canvas::draw_text` call
//!   against the same atlas handle now emits one real textured
//!   `DrawGeometry` command per glyph -- merged by Step 5.1.3's real
//!   batch flattening into a single command, since every glyph shares
//!   Layer/Pipeline/Texture/clip -- rendered through the existing,
//!   unmodified `bindless_textured.vert`/`msdf.frag` pipeline (Step
//!   4.2.4) -- read back as real GPU pixels confirming the word actually
//!   rendered.

use ash::vk;
use skrifa::MetadataProvider;
use tre_atlas::AtlasOwner;
use tre_engine::{
    rgba8, submit_frame, GlyphAtlasContext, RenderingCanvas, RhiDevice, TextureFormat,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const WORD: &str = "TEXT";
const FONT_ID: u32 = 0;
const ATLAS_SIZE: u32 = 256;
const PX_SIZE: f32 = 48.0;
const PEN_ORIGIN: [f32; 2] = [20.0, 90.0];

const CANVAS_WIDTH: u32 = 300;
const CANVAS_HEIGHT: u32 = 120;

fn main() {
    // --- Real shaped word against a real cascade font ---
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");
    let face = rustybuzz::Face::from_slice(&font_bytes, 0)
        .expect("primary cascade font invalid for rustybuzz");
    let runs = tre_text::shape_text(&face, WORD).expect("shaping failed");
    assert_eq!(
        runs.len(),
        1,
        "plain Latin text must be a single run: {runs:?}"
    );
    let shaped = &runs[0];

    // --- Real background atlas owner ---
    let owner = AtlasOwner::spawn(ATLAS_SIZE, ATLAS_SIZE, 8, 8);
    let handle = owner.handle();

    // --- Frame 1: every glyph is a cache miss ---
    let mut miss_frame_canvas = RenderingCanvas::new();
    let miss_frame_atlas_context = GlyphAtlasContext {
        atlas: &handle,
        texture_handle: 0, // never read: no command is expected to be emitted.
        dimensions: (ATLAS_SIZE, ATLAS_SIZE),
        current_frame: 0,
    };
    miss_frame_canvas.draw_text(
        shaped,
        &font,
        FONT_ID,
        PEN_ORIGIN,
        PX_SIZE,
        rgba8(255, 255, 255, 255),
        &miss_frame_atlas_context,
    );
    let miss_frame = miss_frame_canvas.flatten();
    assert!(
        miss_frame.commands.is_empty(),
        "every glyph of a brand-new word must be a cache miss on its first draw_text call, \
         got {} commands",
        miss_frame.commands.len()
    );
    eprintln!("frame 1 (cache miss): 0 commands emitted, as documented -- OK");

    // --- Poll (real sleep-based backoff -- this genuinely waits on a
    // different thread, the atlas owner, to make progress) until every
    // distinct glyph the word actually uses has resolved ---
    for glyph in &shaped.glyphs {
        let key = tre_atlas::AtlasKey::from_glyph(FONT_ID, glyph.glyph_id);
        let mut resolved = false;
        for _ in 0..500 {
            if handle.lookup(key, 0).is_some() {
                resolved = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(resolved, "glyph id {} never resolved", glyph.glyph_id);
    }
    let atlas_buffer = owner.join();
    eprintln!("all distinct glyphs resolved in the real atlas -- OK");

    // --- Frame 2: every glyph now a cache hit ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre canvas draw_text probe (never shown)", 1, 1)
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

    let mut hit_frame_canvas = RenderingCanvas::new();
    let hit_frame_atlas_context = GlyphAtlasContext {
        atlas: &handle,
        texture_handle: texture_index,
        dimensions: (ATLAS_SIZE, ATLAS_SIZE),
        current_frame: 0,
    };
    hit_frame_canvas.draw_text(
        shaped,
        &font,
        FONT_ID,
        PEN_ORIGIN,
        PX_SIZE,
        rgba8(255, 255, 255, 255),
        &hit_frame_atlas_context,
    );
    let frame = hit_frame_canvas.flatten();
    // Step 5.1.3: real batch flattening merges every glyph's command
    // into one -- all 4 share Layer 0 (standard content), the same
    // PIPELINE_MSDF_TEXT pipeline, the same atlas texture_handle, and
    // the same full-window clip_bounds (nothing in this demo ever
    // touches the clip stack).
    assert_eq!(
        frame.commands.len(),
        1,
        "every glyph sharing Layer+Pipeline+Texture+clip_bounds must merge into one command"
    );
    let merged_element_count =
        u64::try_from(shaped.glyphs.len()).expect("glyph count fits in u64") * 6;
    assert_eq!(
        u64::from(frame.commands[0].element_count),
        merged_element_count,
        "the merged command must carry every glyph's own 6 indices"
    );
    eprintln!(
        "frame 2 (cache hit): {} glyphs merged into 1 real command ({} indices) -- OK",
        shaped.glyphs.len(),
        frame.commands[0].element_count
    );

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/msdf.frag.spv"))
        .expect("failed to read compiled MSDF fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create MSDF pipeline");

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

    submit_frame(&device, &swapchain, |cmd_buffer| {
        cmd_buffer.set_pipeline(&pipeline);
        cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&index_buffer, 0);
        cmd_buffer.bind_texture(0, texture_index);
        cmd_buffer.draw_indexed(frame.indices.len() as u32, 0, 0);
    })
    .expect("submit_frame failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let background = pixel_at(0, 0);

    // --- Independently recompute each glyph's expected on-screen quad
    // (the same pen-accumulation formula draw_text itself uses,
    // recomputed here rather than trusting the implementation under
    // test) and confirm each one shows real, non-background fill
    // somewhere across its own quad -- scanning the whole quad, not just
    // its center, same precedent `atlas_concurrency_demo` already
    // established for a glyph whose bounding-box center can genuinely be
    // background (an open counter). ---
    #[allow(
        clippy::cast_precision_loss,
        reason = "a glyph's own units_per_em/advance/offset stay far below f32's exact-integer \
                   range for any real font/text"
    )]
    let units_per_em = f32::from(
        font.metrics(
            skrifa::instance::Size::unscaled(),
            skrifa::instance::LocationRef::default(),
        )
        .units_per_em,
    );
    let scale = PX_SIZE / units_per_em;
    let mut pen = PEN_ORIGIN;
    for (i, glyph) in shaped.glyphs.iter().enumerate() {
        let glyph_origin = [
            pen[0] + glyph.x_offset as f32 * scale,
            pen[1] - glyph.y_offset as f32 * scale,
        ];
        let half = PX_SIZE / 2.0;
        let (x0, y0, x1, y1) = (
            glyph_origin[0] - half,
            glyph_origin[1] - PX_SIZE,
            glyph_origin[0] + half,
            glyph_origin[1],
        );
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
        )]
        let (x0, y0, x1, y1) = (x0 as u32, y0 as u32, x1 as u32, y1 as u32);
        let found_fill = (x0..x1)
            .step_by(2)
            .flat_map(|x| (y0..y1).step_by(2).map(move |y| (x, y)))
            .any(|(x, y)| pixel_at(x, y) != background);
        assert!(
            found_fill,
            "glyph {i} (id {}) rendered as pure background across its entire on-screen quad -- \
             it never actually drew",
            glyph.glyph_id
        );
        pen[0] += glyph.x_advance as f32 * scale;
        pen[1] += glyph.y_advance as f32 * scale;
    }
    eprintln!(
        "all {} glyphs of {WORD:?} rendered real, non-background pixels: OK",
        shaped.glyphs.len()
    );

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_CANVAS_DRAW_TEXT_OUTPUT")
        .unwrap_or_else(|_| "canvas_draw_text_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} canvas draw_text render to {out_path}");
    eprintln!("all canvas draw_text assertions passed");
}
