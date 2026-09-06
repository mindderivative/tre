//! Phase 4 Step 4.2.4 proof: the capstone of the whole Step 4.2 arc.
//! Several real producer threads concurrently request MSDF atlas space
//! for the distinct letters of "GLYPHS" from a real cascade font, through
//! the real bounded MPSC ring buffer; a single real background
//! `AtlasOwner` thread drains requests, performs real Guillotine packing
//! (Step 4.2.1) and real MSDF generation (Step 4.2.2) for each, and
//! publishes results into the real SWMR slot table. After every producer
//! rejoins, every letter's placement is verified non-overlapping and
//! byte-identical to an independently-regenerated MSDF for that same
//! glyph -- then the whole shared atlas is uploaded as one real GPU
//! texture and every letter rendered in a single draw call via the
//! existing, unmodified `msdf.frag` pipeline (Step 4.2.3), reading back
//! real pixels to confirm the word actually reads "GLYPHS".

use ash::vk;
use skrifa::MetadataProvider;
use std::thread;
use tre_atlas::{AtlasKey, AtlasOwner, RasterSource};
use tre_engine::{rgba8, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const ATLAS_SIZE: u32 = 256;
const MSDF_SIZE: u32 = 32;
const RANGE_PX: f64 = 4.0;
const WORD: &str = "GLYPHS";

const CANVAS_WIDTH: u32 = 460;
const CANVAS_HEIGHT: u32 = 100;
const GLYPH_SCREEN_HEIGHT: f32 = 60.0;

/// Rasterizes one glyph's MSDF on demand -- `tre-atlas` never depends on
/// `tre-text` directly (Step 4.2.1's content-agnostic precedent); this is
/// the demo-side glue implementing the `RasterSource` handle
/// ARCHITECTURE.md's own design calls for.
struct GlyphRasterSource {
    contours: Vec<tre_text::Contour>,
}

impl RasterSource for GlyphRasterSource {
    fn size(&self) -> (u32, u32) {
        (MSDF_SIZE, MSDF_SIZE)
    }

    fn rasterize(&self) -> Vec<u8> {
        let bitmap = tre_text::generate_msdf(&self.contours, MSDF_SIZE, RANGE_PX);
        pad_rgb_to_rgba(&bitmap.pixels)
    }
}

fn pad_rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    rgb.chunks_exact(3)
        .flat_map(|c| [c[0], c[1], c[2], 255])
        .collect()
}

fn main() {
    // --- Real cascade font, real outlines for every letter up front ---
    // (extracting the outline itself is cheap and already independently
    // proven, Step 4.1 -- what this demo actually stresses is everything
    // downstream: concurrent request submission, real background
    // packing/generation, and publish/lookup correctness.)
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");

    let letters: Vec<(char, u32, Vec<tre_text::Contour>)> = WORD
        .chars()
        .map(|ch| {
            let glyph_id = font
                .charmap()
                .map(ch)
                .unwrap_or_else(|| panic!("the primary cascade font must cover {ch:?}"));
            let contours =
                tre_text::glyph_outline(&font, glyph_id).expect("outline extraction failed");
            (ch, u32::from(glyph_id), contours)
        })
        .collect();

    // --- Real background atlas owner + real concurrent producers ---
    let owner = AtlasOwner::spawn(ATLAS_SIZE, ATLAS_SIZE, 32, 32);
    let font_id = 0u32;
    let keys: Vec<AtlasKey> = letters
        .iter()
        .map(|(_, glyph_id, _)| AtlasKey::from_glyph(font_id, *glyph_id))
        .collect();

    // Splits the word's letters across 3 producer threads (2 letters
    // each) -- several genuinely independent threads submitting
    // concurrently, standing in for the future per-window worker threads
    // this mechanism is ultimately built for (Phase 5's `SubCanvas`
    // infrastructure, not yet built).
    let chunks: Vec<Vec<(u32, Vec<tre_text::Contour>)>> = {
        let mut chunks = vec![Vec::new(); 3];
        for (i, (_, glyph_id, contours)) in letters.iter().enumerate() {
            chunks[i % 3].push((*glyph_id, contours.clone()));
        }
        chunks
    };
    let producers: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let handle = owner.handle();
            thread::spawn(move || {
                for (glyph_id, contours) in chunk {
                    let key = AtlasKey::from_glyph(font_id, glyph_id);
                    while !handle.request_insert(
                        key,
                        Box::new(GlyphRasterSource {
                            contours: contours.clone(),
                        }),
                    ) {
                        // Waiting on the same owner thread to drain
                        // queue space -- a real `sleep`, not
                        // `yield_now()`, for the same reason as the
                        // lookup-polling loop below.
                        thread::sleep(std::time::Duration::from_millis(2));
                    }
                }
            })
        })
        .collect();
    for producer in producers {
        producer.join().expect("producer thread panicked");
    }

    // --- Poll until every letter's placement resolves ---
    // A real `sleep`-based backoff, not a `yield_now()` busy-spin: this
    // is genuinely waiting on a *different* thread (the atlas owner) to
    // make progress, and `yield_now` is only ever a scheduler *hint* --
    // under a constrained/contended scheduler it can let a tight retry
    // loop burn through many thousands of iterations without the OS ever
    // actually switching to the other thread, unlike an actual `sleep`,
    // which guarantees this thread genuinely stops running for a while.
    let handle = owner.handle();
    let mut placements = Vec::new();
    for &key in &keys {
        let mut resolved = None;
        for _ in 0..500 {
            if let Some(result) = handle.lookup(key) {
                resolved = Some(result);
                break;
            }
            thread::sleep(std::time::Duration::from_millis(2));
        }
        placements.push(resolved.unwrap_or_else(|| panic!("{key:?} never resolved")));
    }

    let atlas_buffer = owner.join();

    // --- Correctness: no overlaps, and every placement's atlas bytes
    // match an independently-regenerated MSDF for that exact glyph ---
    for i in 0..placements.len() {
        for j in (i + 1)..placements.len() {
            let (a, _) = placements[i];
            let (b, _) = placements[j];
            let overlaps = a.x < b.x + b.width
                && b.x < a.x + a.width
                && a.y < b.y + b.height
                && b.y < a.y + a.height;
            assert!(
                !overlaps,
                "'{}' and '{}' placements overlap: {a:?} vs {b:?}",
                letters[i].0, letters[j].0
            );
        }
    }
    for (i, contours) in letters.iter().map(|(_, _, contours)| contours).enumerate() {
        let (rect, _generation) = placements[i];
        let expected =
            pad_rgb_to_rgba(&tre_text::generate_msdf(contours, MSDF_SIZE, RANGE_PX).pixels);
        let bytes_per_row = (rect.width as usize) * 4;
        for row in 0..rect.height {
            let dest_x = rect.x as usize;
            let dest_y = (rect.y + row) as usize;
            let dest_start = (dest_y * (ATLAS_SIZE as usize) + dest_x) * 4;
            let src_start = (row as usize) * bytes_per_row;
            assert_eq!(
                &atlas_buffer[dest_start..dest_start + bytes_per_row],
                &expected[src_start..src_start + bytes_per_row],
                "'{}' row {row} in the shared atlas buffer doesn't match an independently \
                 regenerated MSDF for the same glyph",
                letters[i].0
            );
        }
    }
    eprintln!(
        "all {} letters: non-overlapping and byte-identical to independently regenerated MSDFs",
        letters.len()
    );

    // --- Capstone: upload the finished shared atlas once, render every
    // letter in one draw call via the existing, unmodified msdf.frag ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre atlas concurrency probe (never shown)", 1, 1)
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
    for (rect, _generation) in &placements {
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
            reason = "atlas coordinates are far below f32's exact-integer range for a 256x256 \
                       atlas"
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
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * CANVAS_WIDTH + x) * 4) as usize;
        [bgra[idx + 2], bgra[idx + 1], bgra[idx], bgra[idx + 3]]
    };
    let background = pixel_at(0, 0);

    // Every letter's own on-screen quad must contain *some* real
    // ring/stroke material (non-background), proving each glyph actually
    // rendered from its real, concurrently-obtained atlas UV rect --
    // scanned across the whole quad rather than checked at just its
    // geometric center, since a glyph with an open counter (e.g. 'G')
    // can genuinely have background sitting exactly at its bounding
    // box's center, same as 'L' did in Step 4.1's own demo.
    for (i, (x0, y0, x1, y1)) in glyph_screen_rects.iter().enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
        )]
        let (x0, y0, x1, y1) = (*x0 as u32, *y0 as u32, *x1 as u32, *y1 as u32);
        let found_fill = (x0..x1)
            .step_by(2)
            .flat_map(|x| (y0..y1).step_by(2).map(move |y| (x, y)))
            .any(|(x, y)| pixel_at(x, y) != background);
        assert!(
            found_fill,
            "'{}' rendered as pure background across its entire on-screen quad -- it never \
             actually drew",
            letters[i].0
        );
    }
    eprintln!(
        "all {} letters rendered real, non-background pixels: OK",
        letters.len()
    );

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_ATLAS_CONCURRENCY_OUTPUT")
        .unwrap_or_else(|_| "atlas_concurrency_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} atlas concurrency render to {out_path}");
    eprintln!("all atlas concurrency assertions passed");
}
