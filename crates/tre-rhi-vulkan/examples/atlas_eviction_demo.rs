//! Phase 4 Step 4.3.3 proof: the capstone of the whole Step 4.3 arc,
//! closing the Phase 1-4 review's finding #114 (atlas LRU eviction was
//! never built). A real 64x64 atlas exactly holds four 32x32 MSDF glyphs
//! (Step 4.2.2's real generator) with zero leftover space -- filling it
//! completely reaches 100% capacity, past DESIGN.md Section 10.2's 85%
//! eviction trigger. One glyph ('G') is kept fresh via a real `lookup`
//! at a later frame; the other three ('L', 'Y', 'P') are left untouched.
//! Requesting a fifth glyph ('H') 700 frames later forces a real
//! eviction pass: the three stale glyphs are reclaimed, 'G' survives
//! untouched, and 'H' lands in real, reused atlas space -- proven not
//! just by `lookup`'s own before/after results but by uploading the
//! finished atlas as one real GPU texture and rendering 'G' and 'H'
//! through the existing, unmodified `msdf.frag` pipeline (Step 4.2.3).

use ash::vk;
use skrifa::MetadataProvider;
use tre_atlas::{AtlasKey, AtlasOwner};
use tre_engine::{rgba8, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};
use tre_text::GlyphRasterSource;

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const ATLAS_SIZE: u32 = 64;
const MSDF_SIZE: u32 = 32;
const RANGE_PX: f64 = 4.0;

// DESIGN.md Section 10.2's own "N >= 600 frames" idle threshold -- the
// frame at which the demo requests its fifth glyph, well past it.
const EVICTION_FRAME: u64 = 700;

const CANVAS_WIDTH: u32 = 200;
const CANVAS_HEIGHT: u32 = 100;
const GLYPH_SCREEN_HEIGHT: f32 = 60.0;

fn main() {
    // --- Real cascade font, real outlines for the five distinct letters
    // this demo needs ---
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");

    let contours_for = |ch: char| -> Vec<tre_text::Contour> {
        let glyph_id = font
            .charmap()
            .map(ch)
            .unwrap_or_else(|| panic!("the primary cascade font must cover {ch:?}"));
        tre_text::glyph_outline(&font, glyph_id).expect("outline extraction failed")
    };
    let font_id = 0u32;
    let key_for = |ch: char| AtlasKey::from_glyph(font_id, ch as u32);

    // --- Fill the 64x64 atlas exactly: four 32x32 glyphs, zero leftover
    // space, reaching 100% capacity (past the 85% eviction trigger) ---
    let owner = AtlasOwner::spawn(ATLAS_SIZE, ATLAS_SIZE, 8, 8);
    let handle = owner.handle();

    let insert_and_wait = |ch: char, current_frame: u64| -> (tre_atlas::PackedRect, u16) {
        let key = key_for(ch);
        assert!(
            handle.request_insert(
                key,
                Box::new(GlyphRasterSource {
                    contours: contours_for(ch),
                    size: MSDF_SIZE,
                    range_px: RANGE_PX,
                }),
                current_frame,
            ),
            "'{ch}' request_insert failed -- queue unexpectedly full"
        );
        for _ in 0..500 {
            if let Some(result) = handle.lookup(key, current_frame) {
                return result;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("'{ch}' never resolved");
    };

    let (rect_g, _) = insert_and_wait('G', 0);
    insert_and_wait('L', 0);
    insert_and_wait('Y', 0);
    insert_and_wait('P', 0);
    eprintln!("filled the 64x64 atlas exactly: G, L, Y, P (4x 32x32, 100% capacity)");

    // 'G' is touched (kept fresh) well after the others; 'L'/'Y'/'P' are
    // never looked up again after their own frame-0 insertion, so their
    // recency stays exactly what insertion stamped it to: frame 0.
    assert!(
        handle.lookup(key_for('G'), EVICTION_FRAME).is_some(),
        "'G' must still resolve just before the eviction-triggering request"
    );

    // --- Requesting a fifth real glyph 700 frames later must trigger a
    // real eviction pass: 'L'/'Y'/'P' (idle since frame 0, now far past
    // the 600-frame threshold) are reclaimed; 'G' (touched above)
    // survives; 'H' lands in the real, reused space ---
    let (rect_h, _) = insert_and_wait('H', EVICTION_FRAME);

    assert!(
        handle.lookup(key_for('L'), EVICTION_FRAME).is_none(),
        "'L' was idle past the 600-frame threshold and must have been evicted"
    );
    assert!(
        handle.lookup(key_for('Y'), EVICTION_FRAME).is_none(),
        "'Y' was idle past the 600-frame threshold and must have been evicted"
    );
    assert!(
        handle.lookup(key_for('P'), EVICTION_FRAME).is_none(),
        "'P' was idle past the 600-frame threshold and must have been evicted"
    );
    let (rect_g_after, _) = handle
        .lookup(key_for('G'), EVICTION_FRAME)
        .expect("'G' must still resolve after the eviction pass");
    assert_eq!(
        rect_g_after, rect_g,
        "'G' (touched, not evicted) must keep its original placement"
    );
    assert!(
        !rect_h.overlaps(&rect_g_after),
        "'H' (the new, post-eviction insertion) must not overlap the surviving 'G'"
    );
    eprintln!(
        "eviction verified: L/Y/P reclaimed, G survived unchanged, H reused real freed space"
    );

    let atlas_buffer = owner.join();

    // --- Real GPU render: upload the finished atlas once, draw the two
    // still-resident glyphs (G, the survivor; H, the post-eviction
    // insertion) through the existing, unmodified msdf.frag pipeline ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre atlas eviction probe (never shown)", 1, 1)
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
    let vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/msdf.frag.spv"))
        .expect("failed to read compiled MSDF fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create MSDF pipeline");

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

    let white = rgba8(255, 255, 255, 255);
    let mut vertices: Vec<UiVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut pen_x: f32 = 20.0;
    #[allow(
        clippy::cast_precision_loss,
        reason = "CANVAS_HEIGHT is a small fixed constant"
    )]
    let baseline_y: f32 = (CANVAS_HEIGHT as f32 - GLYPH_SCREEN_HEIGHT) / 2.0;
    let mut glyph_screen_rects: Vec<(f32, f32, f32, f32)> = Vec::new();
    for rect in [rect_g_after, rect_h] {
        #[allow(
            clippy::cast_precision_loss,
            reason = "atlas/canvas coordinates are far below f32's exact-integer range"
        )]
        let (rect_w, rect_h) = (rect.width as f32, rect.height as f32);
        let screen_w = GLYPH_SCREEN_HEIGHT * (rect_w / rect_h);
        let (x0, y0) = (pen_x, baseline_y);
        let (x1, y1) = (pen_x + screen_w, baseline_y + GLYPH_SCREEN_HEIGHT);
        #[allow(
            clippy::cast_precision_loss,
            reason = "atlas coordinates are far below f32's exact-integer range for a 64x64 atlas"
        )]
        let (u0, v0, u1, v1) = (
            rect.x as f32 / ATLAS_SIZE as f32,
            rect.y as f32 / ATLAS_SIZE as f32,
            (rect.x + rect.width) as f32 / ATLAS_SIZE as f32,
            (rect.y + rect.height) as f32 / ATLAS_SIZE as f32,
        );
        let base = u32::try_from(vertices.len()).unwrap();
        vertices.extend_from_slice(&[
            UiVertex {
                position: [x0, y0],
                uv: [u0, v0],
                color: white,
                params: [0.0; 3],
            },
            UiVertex {
                position: [x1, y0],
                uv: [u1, v0],
                color: white,
                params: [0.0; 3],
            },
            UiVertex {
                position: [x1, y1],
                uv: [u1, v1],
                color: white,
                params: [0.0; 3],
            },
            UiVertex {
                position: [x0, y1],
                uv: [u0, v1],
                color: white,
                params: [0.0; 3],
            },
        ]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        glyph_screen_rects.push((x0, y0, x1, y1));
        pen_x = x1 + 8.0;
    }

    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    cmd_buffer.set_pipeline(&pipeline);
    cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&index_buffer, 0);
    cmd_buffer.bind_texture(0, texture_index);
    cmd_buffer.draw_indexed(indices.len() as u32, 0, 0);
    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let background = pixel_at(0, 0);

    // Both surviving/reused glyphs' own on-screen quads must contain
    // *some* real, non-background material -- scanned across the whole
    // quad, not just its geometric center (an open-counter glyph like
    // 'G' can have background sitting exactly there, the same lesson
    // Step 4.1's 'L' and Step 4.2.4's own demo already established).
    for (label, (x0, y0, x1, y1)) in [("G", glyph_screen_rects[0]), ("H", glyph_screen_rects[1])] {
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
            "'{label}' rendered as pure background across its entire on-screen quad -- it never \
             actually drew"
        );
    }
    eprintln!("both G (survivor) and H (post-eviction insertion) rendered real pixels: OK");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_ATLAS_EVICTION_OUTPUT")
        .unwrap_or_else(|_| "atlas_eviction_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} atlas eviction render to {out_path}");
    eprintln!("all atlas eviction assertions passed");
}
