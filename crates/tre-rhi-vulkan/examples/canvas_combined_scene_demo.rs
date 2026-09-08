//! Phase 6 Step 6.5 proof: the combining capstone -- one real recorded
//! `Canvas` scene, submitted as a single `execute_frame` call, that
//! exercises real clipping, real layer compositing, and three real
//! pipelines *together*, proving they interoperate rather than each in
//! isolation. Named explicitly in Step 6.1's own original plan
//! (`planning/archive/PLAN_PHASE6_STEP6_1.md`'s "Scope decisions") as
//! Phase 6's own closer, matching Step 5.3.3's precedent of a final
//! capstone proving previously-separate pieces together.
//!
//! No prior demo combines all of this: `canvas_state_stack_demo.rs`
//! combines `DrawGeometry`+`PushScissor`/`PopScissor`;
//! `canvas_layer_composite_demo.rs` combines `DrawGeometry`+
//! `PushLayer`/`PopLayer`. This demo does both, plus real text, in one
//! scene:
//!
//! 1. `push_clip`/`draw_rounded_rect` (deliberately oversized vs. the
//!    clip, same precedent `canvas_state_stack_demo.rs`'s own Rect C
//!    set)/`pop_clip` -- a rect drawn directly onto the swapchain via
//!    `PipelineKind::SdfRoundedRect`, real GPU scissor cropping proven.
//! 2. `push_layer`/`draw_text` (real shaped word, real resolved MSDF
//!    atlas, `PipelineKind::MsdfText`)/`pop_layer` -- text rendered into
//!    an offscreen layer, then composited back via
//!    `PipelineKind::TexturedQuad`.
//!
//! Three real pipeline ids, three real declared formats, no id reused at
//! two formats in the same frame (`PipelineRegistry` maps one id to
//! exactly one pipeline object per frame, `canvas_layer_composite_demo.
//! rs`'s own header comment): `SdfRoundedRect` is built against the
//! swapchain's own `HEADLESS_FORMAT` (it only ever draws directly onto
//! it here); `MsdfText` is built against the layer's own `Rgba16Float`
//! (it only ever draws inside the layer here); `TexturedQuad` is built
//! against `HEADLESS_FORMAT` for the composite draw, exactly as
//! `canvas_layer_composite_demo.rs` already does it.
//!
//! Text needs a real, already-resolved atlas before the real scene is
//! recorded (`draw_text`'s own documented cache-miss-then-hit contract,
//! `canvas_draw_text_demo.rs`'s established two-frame pattern, reused
//! here unchanged): a throwaway warm-up `Canvas`/`draw_text` call fires
//! the real cache misses, this demo polls the real background
//! `AtlasOwner` thread to resolution, then the real combined scene is
//! recorded on a fresh `Canvas` using the now-resolved atlas.

use skrifa::MetadataProvider;
use tre_atlas::AtlasOwner;
use tre_engine::{
    execute_frame, rgba8, CommandType, GlyphAtlasContext, LayerDesc, PipelineKind,
    PipelineRegistry, RenderingCanvas, RhiDevice, ScissorRect, TextureFormat,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 200;
const SWAPCHAIN_HEIGHT: u32 = 150;

// --- The clipped rect, drawn directly onto the swapchain ---
const CLIP_RECT: ScissorRect = ScissorRect {
    x: 10,
    y: 10,
    width: 50,
    height: 50,
};
// Deliberately larger than CLIP_RECT on every side (0..70 vs. clip's
// 10..60), so real scissor cropping has something to prove.
const RECT_GEOMETRY: (f32, f32, f32, f32) = (0.0, 0.0, 70.0, 70.0);

// --- The layer, composited elsewhere on the swapchain -- chosen not to
// overlap the clipped rect's own 0..70 x 0..70 footprint ---
const LAYER_WIDTH: u32 = 100;
const LAYER_HEIGHT: u32 = 60;
const COMPOSITE_ORIGIN: (i32, i32) = (90, 70);

// --- The real word drawn inside the layer, in the layer's own local
// coordinate space ---
const WORD: &str = "OK";
const FONT_ID: u32 = 0;
const ATLAS_SIZE: u32 = 256;
const PX_SIZE: f32 = 28.0;
const PEN_ORIGIN: [f32; 2] = [20.0, 45.0];

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

    // --- Real background atlas owner -- warm-up call to fire cache
    // misses, then poll to real resolution, matching canvas_draw_text_
    // demo.rs's own established two-frame pattern. ---
    let owner = AtlasOwner::spawn(ATLAS_SIZE, ATLAS_SIZE, 8, 8);
    let handle = owner.handle();

    let mut warm_up_canvas = RenderingCanvas::new();
    let warm_up_atlas_context = GlyphAtlasContext {
        atlas: &handle,
        texture_handle: 0, // never read: no command is expected to be emitted.
        dimensions: (ATLAS_SIZE, ATLAS_SIZE),
        current_frame: 0,
    };
    warm_up_canvas.draw_text(
        shaped,
        &font,
        FONT_ID,
        PEN_ORIGIN,
        PX_SIZE,
        rgba8(255, 255, 255, 255),
        &warm_up_atlas_context,
    );
    let warm_up_frame = warm_up_canvas.flatten();
    assert!(
        warm_up_frame.commands.is_empty(),
        "every glyph of a brand-new word must be a cache miss on its first draw_text call, \
         got {} commands",
        warm_up_frame.commands.len()
    );
    eprintln!("warm-up (cache miss): 0 commands emitted, as documented -- OK");

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

    // --- Real device/swapchain setup ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre canvas combined scene probe (never shown)", 1, 1)
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
    let swapchain = HeadlessSwapchain::new(&device, SWAPCHAIN_WIDTH, SWAPCHAIN_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    // --- The resolved atlas, uploaded once -- already bindless-
    // registered by create_texture's own construction, unlike a
    // transient target, so its bindless index is real and known here,
    // before the real scene is even recorded. ---
    let atlas_texture = device
        .create_texture(
            ATLAS_SIZE,
            ATLAS_SIZE,
            TextureFormat::Rgba8Unorm,
            &atlas_buffer,
        )
        .expect("failed to upload the resolved atlas texture");
    let atlas_texture_index = atlas_texture
        .bindless_index()
        .expect("resolved atlas texture has no bindless index");

    // --- Three pipelines, three real declared formats -- see this
    // file's own header for why each id needs exactly the format it
    // gets. ---
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

    let textured_vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled textured vertex shader");
    let msdf_fragment_spv = std::fs::read(format!("{out_dir}/msdf.frag.spv"))
        .expect("failed to read compiled MSDF fragment shader");
    let text_pipeline = device
        .create_pipeline(
            &textured_vertex_spv,
            &msdf_fragment_spv,
            ash::vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create text pipeline");

    let textured_fragment_spv = std::fs::read(format!("{out_dir}/bindless_textured.frag.spv"))
        .expect("failed to read compiled textured fragment shader");
    let composite_pipeline = device
        .create_pipeline(
            &textured_vertex_spv,
            &textured_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create composite pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRoundedRect as u16, Box::new(rect_pipeline));
    pipelines.register(PipelineKind::MsdfText as u16, Box::new(text_pipeline));
    pipelines.register(
        PipelineKind::TexturedQuad as u16,
        Box::new(composite_pipeline),
    );

    // --- The real combined scene -- no hand-written RHI calls anywhere
    // in this demo. ---
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();

    canvas.push_clip(&CLIP_RECT);
    canvas.draw_rounded_rect(
        RECT_GEOMETRY.0,
        RECT_GEOMETRY.1,
        RECT_GEOMETRY.2,
        RECT_GEOMETRY.3,
        0.0,
        white,
    );
    canvas.pop_clip();

    canvas.push_layer(&LayerDesc {
        x: COMPOSITE_ORIGIN.0,
        y: COMPOSITE_ORIGIN.1,
        width: LAYER_WIDTH,
        height: LAYER_HEIGHT,
        format: TextureFormat::Rgba16Float,
    });
    let scene_atlas_context = GlyphAtlasContext {
        atlas: &handle,
        texture_handle: atlas_texture_index,
        dimensions: (ATLAS_SIZE, ATLAS_SIZE),
        current_frame: 0,
    };
    canvas.draw_text(
        shaped,
        &font,
        FONT_ID,
        PEN_ORIGIN,
        PX_SIZE,
        white,
        &scene_atlas_context,
    );
    canvas.pop_layer();

    let frame = canvas.flatten();

    // --- IR-level sanity check: the exact 6-command sequence this
    // scene must record, in order -- PushScissor/DrawGeometry(rect)/
    // PopScissor/PushLayer/DrawGeometry(text, both glyphs merged, Step
    // 5.1.3's own real batch flattening)/PopLayer. ---
    assert_eq!(
        frame.commands.len(),
        6,
        "expected exactly 6 commands -- got {:?}",
        frame.commands.iter().map(|c| c.kind).collect::<Vec<_>>()
    );
    assert_eq!(frame.commands[0].kind, CommandType::PushScissor);
    assert_eq!(frame.commands[1].kind, CommandType::DrawGeometry);
    assert_eq!(frame.commands[2].kind, CommandType::PopScissor);
    assert_eq!(frame.commands[3].kind, CommandType::PushLayer);
    assert_eq!(frame.commands[4].kind, CommandType::DrawGeometry);
    assert_eq!(frame.commands[5].kind, CommandType::PopLayer);
    eprintln!(
        "IR level: PushScissor, DrawGeometry, PopScissor, PushLayer, DrawGeometry, PopLayer, \
         in order -- OK"
    );

    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.vertices),
            ash::vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.indices),
            ash::vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: SWAPCHAIN_WIDTH,
        height: SWAPCHAIN_HEIGHT,
    };

    // --- The real round trip -- driven entirely by execute_frame. ---
    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    execute_frame(
        &frame,
        &pipelines,
        &vertex_buffer,
        &index_buffer,
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
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    // Far from both the clipped rect and the composited layer.
    let background = pixel_at(150, 20);
    eprintln!("background (clear color): {background:?}");

    // --- Clip cropping: real GPU scissor test ---
    let inside_clip_and_geometry = pixel_at(30, 30);
    assert_eq!(
        inside_clip_and_geometry,
        [255, 255, 255, 255],
        "a point inside both the clip and the drawn geometry must be real foreground, \
         got {inside_clip_and_geometry:?}"
    );
    let inside_geometry_outside_clip = pixel_at(65, 65);
    assert_eq!(
        inside_geometry_outside_clip, background,
        "a point inside the drawn geometry but outside the clip must be real background \
         (genuinely cropped), got {inside_geometry_outside_clip:?}"
    );
    eprintln!("clip cropping: OK (real scissor test proven)");

    // --- Composited text: recompute each glyph's own local quad (same
    // formula draw_text/emit_glyph_quad use), offset by the layer's own
    // composite origin, and scan for any non-background fill -- same
    // methodology canvas_draw_text_demo.rs already established, not a
    // single fragile center-pixel check. ---
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
            glyph_origin[0] - half + COMPOSITE_ORIGIN.0 as f32,
            glyph_origin[1] - PX_SIZE + COMPOSITE_ORIGIN.1 as f32,
            glyph_origin[0] + half + COMPOSITE_ORIGIN.0 as f32,
            glyph_origin[1] + COMPOSITE_ORIGIN.1 as f32,
        );
        let (x0, y0, x1, y1) = (x0 as u32, y0 as u32, x1 as u32, y1 as u32);
        let found_fill = (x0..x1)
            .step_by(2)
            .flat_map(|x| (y0..y1).step_by(2).map(move |y| (x, y)))
            .any(|(x, y)| pixel_at(x, y) != background);
        assert!(
            found_fill,
            "glyph {i} (id {}) composited as pure background across its entire on-screen quad \
             -- it never actually rendered into the layer, or never survived compositing",
            glyph.glyph_id
        );
        pen[0] += glyph.x_advance as f32 * scale;
        pen[1] += glyph.y_advance as f32 * scale;
    }
    eprintln!("composited text: OK (every glyph shows real fill after compositing)");

    // A point well inside the layer's own composited bounds, but with no
    // glyph there -- must be real background, proving the layer was
    // genuinely cleared to transparent, not opaque or garbage.
    let composited_empty_area = pixel_at(
        (COMPOSITE_ORIGIN.0 + LAYER_WIDTH as i32 - 5) as u32,
        (COMPOSITE_ORIGIN.1 + 5) as u32,
    );
    assert_eq!(
        composited_empty_area, background,
        "a point inside the composited layer's own bounds but with no glyph there must show \
         real background, got {composited_empty_area:?}"
    );
    eprintln!("composited empty layer area: OK (real background shows through)");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_COMBINED_SCENE_OUTPUT")
        .unwrap_or_else(|_| "canvas_combined_scene_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        SWAPCHAIN_WIDTH,
        SWAPCHAIN_HEIGHT,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} combined scene to {out_path}");
    eprintln!("all combined scene assertions passed -- Phase 6's own combining capstone, done");
}
