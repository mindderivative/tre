//! Phase 1 Step 1 proof: zero-window rendering (DESIGN.md Section 4.3).
//! Renders the same one-rect scene as the walking skeleton with no
//! display server involvement at all, reads the result back to CPU
//! memory, and writes it to a PNG -- proving `HeadlessSwapchain`
//! implements the exact same `RhiSwapchain` trait `VulkanSwapchain` does,
//! with no changes to the trait itself.
//!
//! This is also a more automatable verification path than a screenshot:
//! the output file's pixel content can be checked programmatically.

use ash::vk;
use tre_engine::{rgba8, RenderingCanvas, RhiDevice};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

fn main() {
    // Headless has no window at all, so `VulkanDevice::new` -- which
    // expects display/window handles to create its physical-device-probe
    // surface -- doesn't apply. A real headless-capable `VulkanDevice`
    // constructor (selecting a physical device without any surface probe)
    // is Phase 2 work; for this step's proof, reuse the ordinary
    // windowed bootstrap against an invisible 1x1 probe window instead of
    // building that constructor now, since the point being proven here is
    // `HeadlessSwapchain`, not device construction. See
    // documentation/REVIEW.md's Phase 1 Step 1 entry.
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre headless probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    // The probe surface is only needed transiently, to let `VulkanDevice::new`
    // pick a physical device/queue family -- headless mode never presents
    // to it, so it must be destroyed explicitly here rather than leaked
    // (the Vulkan validation layer caught exactly this leak during this
    // step's development; see documentation/REVIEW.md's Phase 1 Step 1
    // entry, which also notes the real fix: a surface-less device
    // selection path, deferred to Phase 2).
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    let width = 480u32;
    let height = 360u32;
    let swapchain =
        HeadlessSwapchain::new(&device, width, height).expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    let mut canvas = RenderingCanvas::new();
    canvas.draw_rounded_rect(90.0, 80.0, 300.0, 200.0, 0.0, rgba8(0x40, 0xE0, 0xA0, 0xFF)); // green
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

    // HEADLESS_FORMAT is B8G8R8A8; the `png` crate wants RGBA8.
    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }

    let out_path =
        std::env::var("TRE_HEADLESS_OUTPUT").unwrap_or_else(|_| "headless_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {width}x{height} headless render to {out_path}");
}
