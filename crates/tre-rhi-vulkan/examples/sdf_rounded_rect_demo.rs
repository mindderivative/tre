//! Phase 3 Step 3.2 proof: a real analytical SDF rounded rectangle
//! (IMPLEMENTATION.md Step 3.2, TECHNICAL.md Section 5.2), rendered through
//! the new `sdf_rounded_rect` shader pair and read back pixel-by-pixel --
//! not just "it compiles and doesn't crash."
//!
//! The rect (300x200, corner radius 40) is drawn with a 20px margin inside
//! a larger canvas so there's real background visible around it. Pure
//! white is used as the foreground color specifically so alpha=1 and
//! alpha=0 fragments round-trip to exact byte values regardless of
//! whatever color space Vulkan blends premultiplied-alpha attachments in
//! (a linear combination with weight (1,0) or (0,1) always equals one
//! operand exactly, independent of that question) -- the same "use exact
//! fixed points" trick `bindless_textures_demo` uses for its solid-color
//! textures.

use ash::vk;
use tre_engine::{rgba8, RenderingCanvas, RhiDevice};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const MARGIN: u32 = 20;
const RECT_WIDTH: u32 = 300;
const RECT_HEIGHT: u32 = 200;
const RADIUS: f32 = 40.0;

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre sdf rounded rect probe (never shown)", 1, 1)
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

    let width = RECT_WIDTH + MARGIN * 2;
    let height = RECT_HEIGHT + MARGIN * 2;
    let swapchain =
        HeadlessSwapchain::new(&device, width, height).expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();
    canvas.draw_rounded_rect(
        MARGIN as f32,
        MARGIN as f32,
        RECT_WIDTH as f32,
        RECT_HEIGHT as f32,
        RADIUS,
        white,
    );
    let frame = canvas.flatten();
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
    cmd_buffer.set_pipeline(&pipeline);
    cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&index_buffer, 0);
    cmd_buffer.draw_indexed(frame.indices.len() as u32, 0, 0);
    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * width + x) * 4) as usize;
        [bgra[idx], bgra[idx + 1], bgra[idx + 2], bgra[idx + 3]]
    };

    // Never touched by the draw call (outside the rect's own quad
    // entirely) -- reading it back gives the actual clear-color bytes
    // without needing to hardcode Vulkan's sRGB-attachment clear
    // conversion behavior.
    let background = pixel_at(0, 0);
    eprintln!("background (clear color): {background:?}");

    // Deep interior, far from every edge -- alpha must clamp to exactly
    // 1.0, so this must equal pure white exactly.
    let interior = pixel_at(MARGIN + RECT_WIDTH / 2, MARGIN + RECT_HEIGHT / 2);
    assert_eq!(
        interior,
        [255, 255, 255, 255],
        "rect interior must be exactly the foreground color, got {interior:?}"
    );
    eprintln!("interior: OK ({interior:?})");

    // Well inside the bounding box's corner but clearly outside the
    // rounding arc -- (MARGIN+5, MARGIN+5) relative to a center at
    // (MARGIN+150, MARGIN+100) with radius 40 gives a true SDF distance of
    // about +16px past the surface, comfortably past the ~1px AA
    // transition band. Alpha must clamp to exactly 0.0, so premultiplied
    // blending must reproduce the background exactly.
    let corner = pixel_at(MARGIN + 5, MARGIN + 5);
    assert_eq!(
        corner, background,
        "the cut-away corner must be exactly the background color, got {corner:?}"
    );
    eprintln!("corner (cut away by rounding): OK ({corner:?})");

    // A real, checked blend: scan a block of pixels around the top-left
    // rounding arc for one with a genuine partial alpha. The rect's FLAT
    // edges sit at exact integer canvas coordinates by construction here,
    // so their 1px-wide analytical AA ramp falls exactly between two pixel
    // centers (at half-integer offsets) with no fractional-coverage sample
    // in between -- confirmed empirically, not just in theory, during this
    // step's development. The rounded corner's arc has no such alignment:
    // its gradient isn't axis-aligned, so `fwidth(d)` there is genuinely
    // less than 1 and multiple pixels sample real partial coverage. This
    // is also a more representative check anyway, since it's the rounding
    // itself this step exists to prove.
    let aa_band_pixel = (25u32..45)
        .flat_map(|y| (25u32..45).map(move |x| (x, y)))
        .map(|(x, y)| pixel_at(x, y))
        .find(|&p| p != [255, 255, 255, 255] && p != background);
    let aa_band_pixel = aa_band_pixel
        .expect("expected at least one real anti-aliased blend pixel around the rounded corner");
    eprintln!("AA transition band: OK (found blended pixel {aa_band_pixel:?})");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_SDF_ROUNDED_RECT_OUTPUT")
        .unwrap_or_else(|_| "sdf_rounded_rect_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {width}x{height} SDF rounded-rect render to {out_path}");
    eprintln!("all SDF rounded-rect assertions passed");
}
