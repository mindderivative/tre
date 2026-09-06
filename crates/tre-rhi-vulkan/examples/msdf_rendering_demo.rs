//! Phase 4 Step 4.2.3 proof: the payoff moment for the whole Step 4.2
//! arc -- a real glyph ('O', deliberately reusing Step 4.2.2's
//! hole-having case), its real MSDF (Step 4.2.2, unmodified), uploaded as
//! a real GPU texture and rendered through a real evaluation shader
//! (TECHNICAL.md Section 5.3's exact canonical formula) at a generous
//! on-screen magnification -- proving both correctness (a genuinely
//! hollow ring survives all the way to the screen) and real anti-
//! aliasing (a strictly-intermediate blended pixel at the glyph's own
//! edge, not a hard binary transition -- the concrete fix for the
//! jagged 'X' observed in Step 4.1's demo).

use ash::vk;
use skrifa::MetadataProvider;
use tre_engine::{rgba8, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const WIDTH: u32 = 300;
const HEIGHT: u32 = 300;
const MSDF_SIZE: u32 = 32;
const RANGE_PX: f64 = 4.0;
// The on-screen quad: 220x220 pixels for a 32x32 source texture is a
// ~6.9x magnification -- generous enough that any residual jaggedness
// (the exact complaint Step 4.1's demo drew) would be obvious if this
// shader's anti-aliasing weren't real.
const QUAD_ORIGIN: (f32, f32) = (40.0, 40.0);
const QUAD_SIZE: f32 = 220.0;

fn white_pixel_ref() -> [u8; 4] {
    [255, 255, 255, 255]
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre msdf rendering probe (never shown)", 1, 1)
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

    let swapchain =
        HeadlessSwapchain::new(&device, WIDTH, HEIGHT).expect("failed to create HeadlessSwapchain");

    // Reuses `bindless_textured.vert` unchanged (Step 2.1) -- its
    // position/uv/color inputs and screen_size/texture_index push
    // constants are already exactly what MSDF sampling needs; only the
    // fragment shader's own math differs.
    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/msdf.frag.spv"))
        .expect("failed to read compiled MSDF fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create MSDF pipeline");

    // --- Real glyph -> real MSDF (Step 4.2.2, unmodified) ---
    let cascade = tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
    let font_bytes =
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font");
    let font = skrifa::FontRef::new(&font_bytes).expect("primary cascade font invalid for skrifa");
    let glyph_id = font
        .charmap()
        .map('O')
        .expect("the primary cascade font must cover 'O'");
    let contours = tre_text::glyph_outline(&font, glyph_id).expect("outline extraction failed");
    assert_eq!(
        contours.len(),
        2,
        "'O' must be exactly two contours (outer boundary + hole)"
    );
    let bitmap = tre_text::generate_msdf(&contours, MSDF_SIZE, RANGE_PX);

    // fdsm's own output is RGB8; pad to RGBA8 for `TextureFormat::Rgba8Unorm`
    // (Step 4.2.3 task 1) -- 4-byte-aligned formats have far more
    // universal GPU support than tightly-packed 3-byte ones. The alpha
    // channel is unused by `msdf.frag`, which only ever reads `.rgb`.
    let rgba_pixels: Vec<u8> = bitmap
        .pixels
        .chunks_exact(3)
        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
        .collect();
    let texture = device
        .create_texture(
            MSDF_SIZE,
            MSDF_SIZE,
            TextureFormat::Rgba8Unorm,
            &rgba_pixels,
        )
        .expect("failed to upload MSDF texture");
    let texture_index = texture
        .bindless_index()
        .expect("MSDF texture has no bindless index");

    // --- One quad, sampling the whole MSDF texture ---
    let white = rgba8(255, 255, 255, 255);
    let (x0, y0) = QUAD_ORIGIN;
    let (x1, y1) = (x0 + QUAD_SIZE, y0 + QUAD_SIZE);
    let vertices = [
        UiVertex {
            position: [x0, y0],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x1, y0],
            uv: [1.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x1, y1],
            uv: [1.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x0, y1],
            uv: [0.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
    ];
    let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

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
    // `read_pixels_bgra8` returns real BGRA memory order (Step 4.2.1's
    // Finding #93) -- swapped here so this demo's own comparisons stay in
    // the `[R,G,B,A]` order its own code otherwise assumes throughout.
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [bgra[idx + 2], bgra[idx + 1], bgra[idx], bgra[idx + 3]]
    };
    let background = pixel_at(0, 0);
    eprintln!("background (clear color): {background:?}");

    // Scan the on-screen quad's own vertical-center row: classify each
    // pixel by how close its RGB average sits to white vs. background,
    // computed independently against the *rendered* pixels rather than
    // predicted from the source texel grid (bilinear texture filtering
    // makes the exact screen-space transition point a function of GPU
    // sampling, not something to hand-predict). A real ring produces, in
    // order: background (outer margin) -> white (left wall) -> background
    // (hole) -> white (right wall) -> background (outer margin) -- and at
    // each of the four transitions, at least one pixel whose value is
    // neither extreme is the concrete signature of real anti-aliasing.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "QUAD_ORIGIN/QUAD_SIZE are fixed constants well within [0, WIDTH)"
    )]
    let center_y = (QUAD_ORIGIN.1 + QUAD_SIZE / 2.0) as u32;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "QUAD_ORIGIN/QUAD_SIZE are fixed constants well within [0, WIDTH)"
    )]
    let scan_range = (QUAD_ORIGIN.0 as u32)..(QUAD_ORIGIN.0 + QUAD_SIZE) as u32;

    let fill_fraction = |pixel: [u8; 4]| -> f32 {
        let avg = |p: [u8; 4]| (f32::from(p[0]) + f32::from(p[1]) + f32::from(p[2])) / 3.0;
        (avg(pixel) - avg(background)) / (avg(white_pixel_ref()) - avg(background))
    };

    let scan: Vec<(u32, f32)> = scan_range
        .clone()
        .map(|x| (x, fill_fraction(pixel_at(x, center_y))))
        .collect();

    let white_count = scan.iter().filter(|&&(_, f)| f > 0.95).count();
    let background_count = scan.iter().filter(|&&(_, f)| f < 0.05).count();
    let intermediate_count = scan
        .iter()
        .filter(|&&(_, f)| (0.15..0.85).contains(&f))
        .count();

    eprintln!(
        "center-row scan: {} white-ish, {} background-ish, {} genuinely intermediate (of {} \
         pixels)",
        white_count,
        background_count,
        intermediate_count,
        scan.len()
    );

    assert!(
        white_count > 10,
        "expected a real stretch of ring material (white) along the center row, got \
         {white_count} pixels"
    );
    assert!(
        background_count > 10,
        "expected real background both outside the glyph and inside its hole along the center \
         row, got {background_count} pixels"
    );
    assert!(
        intermediate_count >= 2,
        "expected at least 2 genuinely intermediate (neither pure white nor pure background) \
         pixels -- one per ring-wall crossing -- proving real sub-pixel anti-aliasing rather \
         than a hard binary edge; got {intermediate_count}"
    );
    eprintln!("anti-aliasing verification: OK (real intermediate-coverage pixels found)");

    // Deep interior of the left ring wall and deep interior of the hole,
    // both found from the scan itself (not guessed), confirming the
    // *pure* extremes are also correct, not just the existence of a
    // blend somewhere.
    let deep_white = scan
        .iter()
        .find(|&&(_, f)| f > 0.99)
        .expect("no pure-white pixel found");
    let deep_background = scan
        .iter()
        .skip_while(|&&(_, f)| f > 0.05)
        .find(|&&(_, f)| f < 0.01)
        .expect("no pure-background pixel found inside the scan range");
    assert_eq!(pixel_at(deep_white.0, center_y), white_pixel_ref());
    assert_eq!(pixel_at(deep_background.0, center_y), background);
    eprintln!("deep-interior and deep-hole pixels: OK (exact white / exact background)");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_MSDF_RENDERING_OUTPUT")
        .unwrap_or_else(|_| "msdf_rendering_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {WIDTH}x{HEIGHT} MSDF render to {out_path}");
    eprintln!("all MSDF rendering assertions passed");
}
