//! Phase 10 Step 10.1 proof: a real GPU pixel-diff showing
//! `ShapeRegistry`'s retained-mode flattening pass produces byte-for-
//! byte identical output to the exact same scene drawn directly through
//! today's real immediate-mode `RenderingCanvas::draw_rounded_rect`
//! (Phase 3 Step 3.2) -- proving the shape-primitive layer is a real
//! convenience wrapper over the existing IR/sort/batch/RHI pipeline,
//! never a second, divergent rendering path (ARCHITECTURE.md Section 7's
//! own "why a retained-mode layer at all" rationale).
//!
//! The scene: one rounded rectangle, drawn once via the direct API and
//! once via `ShapeRegistry::insert` + `ShapeRegistry::flatten_into` --
//! the one shape/field combination with real rendering support today
//! (a uniform `CornerRadii`, zero `corner_smoothing`, `FillStyle::
//! Solid`, zero `border_thickness` -- see `tre-engine`'s own
//! `shapes.rs` module doc comment).

use ash::vk;
use tre_engine::{
    rgba8, submit_frame, CornerRadii, FillStyle, Rectangle, RenderingCanvas, ShapePrimitive,
    ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const MARGIN: u32 = 20;
const RECT_WIDTH: u32 = 300;
const RECT_HEIGHT: u32 = 200;
const RADIUS: f32 = 40.0;

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre shape registry probe (never shown)", 1, 1)
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

    let render = |frame: &tre_engine::FlattenedFrame| -> Vec<u8> {
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
            cmd_buffer.draw_indexed(frame.indices.len() as u32, 0, 0);
        })
        .expect("submit_frame failed");
        swapchain
            .read_pixels_bgra8()
            .expect("failed to read back pixels")
    };

    // --- Direct: today's real immediate-mode API, unchanged. ---
    let mut direct_canvas = RenderingCanvas::new();
    direct_canvas.draw_rounded_rect(
        MARGIN as f32,
        MARGIN as f32,
        RECT_WIDTH as f32,
        RECT_HEIGHT as f32,
        RADIUS,
        white,
    );
    let direct_pixels = render(&direct_canvas.flatten());
    eprintln!("direct draw_rounded_rect: rendered");

    // --- Registry: the exact same scene, described once as a retained
    // Rectangle and flattened through ShapeRegistry::flatten_into. ---
    let mut registry = ShapeRegistry::new();
    let mut rect = Rectangle::new([RECT_WIDTH as f32, RECT_HEIGHT as f32], white);
    rect.common.transform.position = [MARGIN as f32, MARGIN as f32];
    rect.corner_radius = CornerRadii::uniform(RADIUS);
    rect.fill = FillStyle::Solid(white);
    registry.insert(ShapePrimitive::Rectangle(rect));

    let mut registry_canvas = RenderingCanvas::new();
    registry.flatten_into(&mut registry_canvas, &device);
    let registry_pixels = render(&registry_canvas.flatten());
    eprintln!("ShapeRegistry::flatten_into: rendered");

    // --- The real equivalence check: every pixel, byte for byte. ---
    assert_eq!(
        direct_pixels.len(),
        registry_pixels.len(),
        "both renders must read back the exact same buffer size"
    );
    let mut first_mismatch: Option<usize> = None;
    for (i, (d, r)) in direct_pixels.iter().zip(registry_pixels.iter()).enumerate() {
        if d != r {
            first_mismatch = Some(i);
            break;
        }
    }
    assert!(
        first_mismatch.is_none(),
        "direct and ShapeRegistry-flattened renders must be byte-for-byte identical -- first \
         mismatch at byte offset {:?} (direct={:?}, registry={:?}); a mismatch here means the \
         shape-primitive layer is not a faithful convenience wrapper over the existing pipeline",
        first_mismatch,
        first_mismatch.map(|i| direct_pixels[i]),
        first_mismatch.map(|i| registry_pixels[i]),
    );
    eprintln!(
        "direct vs. ShapeRegistry: byte-for-byte identical across the whole framebuffer -- OK"
    );

    // --- Sanity: real, non-background content actually landed where
    // expected, so an all-broken-but-equal false pass can't slip
    // through as "equivalent." ---
    let pixel_at =
        |buf: &[u8], x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(buf, width, x, y) };
    let background = pixel_at(&direct_pixels, 0, 0);
    let interior = pixel_at(
        &direct_pixels,
        MARGIN + RECT_WIDTH / 2,
        MARGIN + RECT_HEIGHT / 2,
    );
    assert_ne!(
        interior, background,
        "the rectangle's own interior must have rendered real, non-background content"
    );
    eprintln!("rectangle interior rendered real, non-background content -- OK");

    let mut rgba_out = direct_pixels.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_SHAPE_REGISTRY_OUTPUT")
        .unwrap_or_else(|_| "shape_registry_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {width}x{height} shape registry equivalence render to {out_path}");
    eprintln!("all shape registry equivalence assertions passed");
}
