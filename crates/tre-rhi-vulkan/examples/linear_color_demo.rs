//! Phase 7 Step 7.1 proof: the real shader-side sRGB-to-linear conversion
//! fix (REVIEW.md finding #92, first found at Phase 4 Step 4.2.1 and
//! explicitly deferred to this exact step). No prior demo could prove
//! this: every one of them deliberately uses only gamma-invariant
//! (`0`/`255` per channel) colors, per `sdf_rounded_rect_demo.rs`'s own
//! header comment -- exactly so this defect's fix (or lack of one)
//! couldn't affect their own assertions either way.
//!
//! The bug, precisely: the headless swapchain's format is `B8G8R8A8_
//! SRGB`, so the GPU automatically sRGB-encodes whatever a fragment
//! shader outputs on store. Before this step, every shader passed
//! `UiVertex::color` straight through -- an sRGB-authored value treated
//! as already-linear, then sRGB-encoded a *second* time. Finding #92's
//! own worked example: a mid-tone gray `150` round-trips to `202`.
//!
//! This demo proves the fix directly, not via a proxy: draws a fully
//! OPAQUE rect (coverage `1.0` deep in its own interior -- no AA-edge
//! derivative uncertainty, no CPU-side `premultiply_alpha` interaction
//! since `state.alpha` defaults to `1.0`) with a genuinely non-fixed-
//! point color. `srgb_to_linear` (now in the shader) and the swapchain's
//! own hardware sRGB-encode-on-store are exact inverses, so the real GPU
//! readback must equal the original authored color, within a small,
//! disclosed 8-bit tolerance -- and must be measurably *different* from
//! what the old, broken double-encoding would have produced, proving the
//! fix has real, provable effect, not just "didn't crash."

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

/// A genuinely non-fixed-point color -- none of R/G/B is `0` or `255` --
/// matching REVIEW.md finding #92's own worked example (`150`) on the
/// red channel, with green/blue chosen independently so all three
/// channels are checked, not just one.
const FOREGROUND: (u8, u8, u8) = (150, 100, 200);

/// The canonical sRGB encode formula (TECHNICAL.md Section 6.2's own
/// decode formula, inverted) -- a real, independent Rust reference used
/// only to compute what the *old*, unfixed shader would have produced
/// (double-encoding), so this demo can prove its own fix is measurably
/// different from that broken behavior, not just "close to right."
fn srgb_encode(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn broken_double_encode_byte(original: u8) -> u8 {
    let c = f32::from(original) / 255.0;
    (srgb_encode(c) * 255.0).round().clamp(0.0, 255.0) as u8
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre linear color probe (never shown)", 1, 1)
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
    let vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    let foreground = rgba8(FOREGROUND.0, FOREGROUND.1, FOREGROUND.2, 255);
    let mut canvas = RenderingCanvas::new();
    canvas.draw_rounded_rect(
        MARGIN as f32,
        MARGIN as f32,
        RECT_WIDTH as f32,
        RECT_HEIGHT as f32,
        0.0,
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

    // Deep interior, far from every edge -- alpha must clamp to exactly
    // 1.0 (a fully opaque draw), so this is a pure round-trip test of the
    // color-space conversion alone, with no AA/blend interaction at all.
    let interior = pixel_at(MARGIN + RECT_WIDTH / 2, MARGIN + RECT_HEIGHT / 2);
    eprintln!("interior readback: {interior:?}");

    const TOLERANCE: i32 = 4;
    let expected = [FOREGROUND.0, FOREGROUND.1, FOREGROUND.2];
    for (channel, (&got, &want)) in ["R", "G", "B"]
        .iter()
        .zip(interior.iter().zip(expected.iter()))
    {
        let diff = (i32::from(got) - i32::from(want)).abs();
        assert!(
            diff <= TOLERANCE,
            "channel {channel}: fixed shader must round-trip the authored color within \
             {TOLERANCE}, authored {want}, got {got} (diff {diff})"
        );
    }
    eprintln!("round-trip: OK (real GPU output matches the authored color within tolerance)");

    // Prove the fix has real, measurable effect: the old, unfixed
    // double-encoding would have produced a visibly different byte value
    // per channel -- confirm the real result is NOT close to that broken
    // value, so this isn't a check that would have passed either way.
    let broken = [
        broken_double_encode_byte(FOREGROUND.0),
        broken_double_encode_byte(FOREGROUND.1),
        broken_double_encode_byte(FOREGROUND.2),
    ];
    eprintln!("what the old, unfixed double-encoding would have produced: {broken:?}");
    for (channel, (&got, &broken)) in ["R", "G", "B"].iter().zip(interior.iter().zip(&broken)) {
        let diff = (i32::from(got) - i32::from(broken)).abs();
        assert!(
            diff > 10,
            "channel {channel}: the real, fixed result ({got}) must be measurably different \
             from what the old, unfixed double-encoding would have produced ({broken}) -- \
             otherwise this demo wouldn't actually be proving the fix has any effect"
        );
    }
    eprintln!("fix has real effect: OK (measurably different from the old, broken output)");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_LINEAR_COLOR_OUTPUT")
        .unwrap_or_else(|_| "linear_color_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} linear color render to {out_path}");
    eprintln!("all linear color assertions passed -- REVIEW.md finding #92 is fixed");
}
