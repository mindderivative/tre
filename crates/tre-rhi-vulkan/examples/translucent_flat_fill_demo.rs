//! REVIEW.md finding #164 proof: `walking_skeleton.frag` (now
//! `PipelineKind::FlatColor`, `Polygon`/`Path` fill and stroke's real
//! shader) now premultiplies its own output by alpha, matching every
//! other real fragment shader in this codebase and ARCHITECTURE.md
//! Section 6.1's documented premultiplied-alpha blend state.
//!
//! The bug, precisely: the old shader output
//! `vec4(srgb_to_linear(frag_color.rgb), frag_color.a)` -- RGB was NOT
//! multiplied by alpha before the GPU's own premultiplied-alpha blend
//! equation (`ONE`, `ONE_MINUS_SRC_ALPHA`) ran. For fully-opaque colors
//! (`alpha == 1.0`) this is bit-identical to the correct, fixed output,
//! which is exactly why no prior demo caught it -- every one of them
//! (including this project's own new `path_and_polygon_demo.rs`) only
//! ever draws fully-opaque flat-fill geometry.
//!
//! This demo draws a genuinely translucent flat-fill rectangle (alpha
//! `120/255`) over the swapchain's own real background and proves two
//! things, not just "didn't crash": (1) the real GPU readback matches an
//! independent Rust reference implementation of the CORRECT premultiplied
//! blend, within a small disclosed tolerance; (2) that readback is
//! measurably different from what the OLD, unfixed non-premultiplied
//! shader would have produced -- proving this demo would actually have
//! failed before the fix, not passed either way.

use ash::vk;
use tre_engine::{rgba8, RenderingCanvas, RhiDevice};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 200;
const CANVAS_HEIGHT: u32 = 150;
const MARGIN: u32 = 20;
const RECT_WIDTH: u32 = 160;
const RECT_HEIGHT: u32 = 110;

/// A genuinely translucent, non-fixed-point foreground color -- none of
/// R/G/B is `0` or `255`, and alpha is neither `0` nor `255` either, so
/// the premultiplication this finding is about actually has something to
/// do.
const FOREGROUND_RGB: (u8, u8, u8) = (230, 90, 180);
const FOREGROUND_ALPHA: u8 = 120;

/// TECHNICAL.md Section 6.2's canonical sRGB<->linear formulas -- a real,
/// independent Rust reference used to compute what the real GPU blend
/// SHOULD produce (and, separately, what the old broken shader WOULD
/// have produced), so this demo can compare real hardware output against
/// known-correct and known-broken math instead of asserting a single
/// deep-in-the-dark "will do" range.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encode(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn to_srgb_byte(c: f32) -> u8 {
    (srgb_encode(c.clamp(0.0, 1.0)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// The real GPU blend equation (`ONE`, `ONE_MINUS_SRC_ALPHA`, evaluated
/// in linear space around the swapchain's `_SRGB` attachment) applied to
/// a caller-supplied, already-computed `src_rgb` -- letting the two
/// callers below share this one blend step while differing only in
/// whether `src_rgb` was premultiplied by alpha first (the fix) or not
/// (the bug).
fn blend_over(src_rgb: [f32; 3], src_alpha: f32, dst_rgb: [f32; 3]) -> [u8; 3] {
    let mut out = [0u8; 3];
    for i in 0..3 {
        let blended = src_rgb[i] + dst_rgb[i] * (1.0 - src_alpha);
        out[i] = to_srgb_byte(blended);
    }
    out
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre translucent flat fill probe (never shown)", 1, 1)
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
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    let foreground = rgba8(
        FOREGROUND_RGB.0,
        FOREGROUND_RGB.1,
        FOREGROUND_RGB.2,
        FOREGROUND_ALPHA,
    );
    let mut canvas = RenderingCanvas::new();
    let x0 = MARGIN as f32;
    let y0 = MARGIN as f32;
    let x1 = (MARGIN + RECT_WIDTH) as f32;
    let y1 = (MARGIN + RECT_HEIGHT) as f32;
    canvas.draw_flat_polygon(
        &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        &[[0, 1, 2], [0, 2, 3]],
        foreground,
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
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };

    // The real background, read directly rather than assumed -- outside
    // the translucent rect entirely, so this is the swapchain's own
    // real clear color as this exact GPU/driver actually produced it.
    let background = pixel_at(0, 0);
    eprintln!("background: {background:?}");
    let background_linear = [
        srgb_to_linear(f32::from(background[0]) / 255.0),
        srgb_to_linear(f32::from(background[1]) / 255.0),
        srgb_to_linear(f32::from(background[2]) / 255.0),
    ];

    // Deep interior, far from every edge -- `draw_flat_polygon` has no
    // antialiasing (hard triangle edges only), so this is a pure,
    // uniform alpha-blend of the flat fill color over the background,
    // with no coverage/derivative uncertainty at all.
    let interior = pixel_at(MARGIN + RECT_WIDTH / 2, MARGIN + RECT_HEIGHT / 2);
    eprintln!("interior readback: {interior:?}");

    let foreground_linear = [
        srgb_to_linear(f32::from(FOREGROUND_RGB.0) / 255.0),
        srgb_to_linear(f32::from(FOREGROUND_RGB.1) / 255.0),
        srgb_to_linear(f32::from(FOREGROUND_RGB.2) / 255.0),
    ];
    let alpha = f32::from(FOREGROUND_ALPHA) / 255.0;

    // CORRECT (fixed): src RGB is premultiplied by alpha before the
    // blend equation runs.
    let premultiplied_src = [
        foreground_linear[0] * alpha,
        foreground_linear[1] * alpha,
        foreground_linear[2] * alpha,
    ];
    let expected = blend_over(premultiplied_src, alpha, background_linear);
    eprintln!("expected (correct, premultiplied): {expected:?}");

    const TOLERANCE: i32 = 4;
    for (channel, (&got, &want)) in ["R", "G", "B"]
        .iter()
        .zip(interior.iter().zip(expected.iter()))
    {
        let diff = (i32::from(got) - i32::from(want)).abs();
        assert!(
            diff <= TOLERANCE,
            "channel {channel}: fixed shader must premultiply alpha before blending, expected \
             {want} (within {TOLERANCE}), got {got} (diff {diff})"
        );
    }
    let alpha_diff = (i32::from(interior[3]) - 255).abs();
    assert!(
        alpha_diff <= TOLERANCE,
        "blended alpha over an opaque background must end up ~255 (opaque), got {} (diff {})",
        interior[3],
        alpha_diff
    );
    eprintln!("premultiplied blend: OK (matches the correct reference within tolerance)");

    // Prove the fix has real, measurable effect: the OLD, unfixed shader
    // left `src_rgb` un-premultiplied (full-brightness color leaking
    // through at less-than-full opacity), so its own blended result must
    // be measurably brighter than the correct, fixed one -- confirm the
    // real GPU result is NOT close to what that broken math would have
    // produced.
    let broken = blend_over(foreground_linear, alpha, background_linear);
    eprintln!("what the old, unfixed non-premultiplied shader would have produced: {broken:?}");
    for (channel, (&got, &broken)) in ["R", "G", "B"].iter().zip(interior.iter().zip(&broken)) {
        let diff = (i32::from(got) - i32::from(broken)).abs();
        assert!(
            diff > 10,
            "channel {channel}: the real, fixed result ({got}) must be measurably different \
             from what the old, unfixed non-premultiplied shader would have produced ({broken}) \
             -- otherwise this demo wouldn't actually be proving the fix has any effect"
        );
    }
    eprintln!("fix has real effect: OK (measurably different from the old, broken output)");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_TRANSLUCENT_FLAT_FILL_OUTPUT")
        .unwrap_or_else(|_| "translucent_flat_fill_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} translucent flat fill render to {out_path}");
    eprintln!("all translucent flat fill assertions passed -- REVIEW.md finding #164 is fixed");
}
