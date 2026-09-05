//! IMPLEMENTATION.md Phase 0's walking skeleton: opens one window, clears
//! it to a color, and draws one `Canvas::draw_rounded_rect` command through
//! the real `Canvas -> IR -> RhiCommandBuffer::draw_indexed` pipeline.
//!
//! Windowing is now `tre-platform` (Phase 1 Step 1's native Linux
//! windowing), replacing the `winit`-based Phase-0-only expedient this
//! example originally used.

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tre_engine::{rgba8, RenderingCanvas, RhiDevice};
use tre_platform::{PlatformWindow, WindowEvent};
use tre_rhi_vulkan::{VulkanBuffer, VulkanDevice, VulkanPipelineState, VulkanSwapchain};

// Field order matters: Rust drops a struct's fields in DECLARATION order
// (not reverse), so everything that holds a handle into the Vulkan device
// must be declared -- and therefore dropped -- BEFORE `device` itself, and
// `device`/`swapchain` before `window` (the swapchain is tied to the
// window's surface). Getting this backwards is exactly what produced the
// Vulkan validation layer's "N leaked objects" errors and a SIGSEGV during
// this example's original development -- see documentation/REVIEW.md
// findings #43.
struct Renderer {
    pipeline: VulkanPipelineState,
    vertex_buffer: VulkanBuffer,
    index_buffer: VulkanBuffer,
    index_count: u32,
    swapchain: VulkanSwapchain,
    device: VulkanDevice,
    window: PlatformWindow,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device.device_wait_idle();
        }
    }
}

fn main() {
    let window = PlatformWindow::new("tre walking skeleton (Phase 0)", 640, 480)
        .expect("failed to open window");

    let display_handle = window.display_handle().unwrap().as_raw();
    let window_handle = window.window_handle().unwrap().as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    let swapchain = VulkanSwapchain::new(&device, surface_loader, surface, 640, 480)
        .expect("failed to create VulkanSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, swapchain.format())
        .expect("failed to create pipeline");

    // Phase 0: one Canvas call, one rounded rect (rendered as a flat quad
    // -- IMPLEMENTATION.md Phase 3.2 owns the real SDF shader).
    let mut canvas = RenderingCanvas::new();
    canvas.draw_rounded_rect(170.0, 140.0, 300.0, 200.0, rgba8(0xE0, 0xA0, 0x40, 0xFF));
    let frame = canvas.flatten();

    let vertex_bytes: &[u8] = bytemuck::cast_slice(&frame.vertices);
    let index_bytes: &[u8] = bytemuck::cast_slice(&frame.indices);
    let vertex_buffer = device
        .upload_buffer(vertex_bytes, vk::BufferUsageFlags::VERTEX_BUFFER)
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(index_bytes, vk::BufferUsageFlags::INDEX_BUFFER)
        .expect("failed to upload index buffer");
    let index_count = frame.indices.len() as u32;

    let mut renderer = Renderer {
        pipeline,
        vertex_buffer,
        index_buffer,
        index_count,
        swapchain,
        device,
        window,
    };

    let frame_limit: u64 = std::env::var("TRE_WALKING_SKELETON_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let mut frame_count: u64 = 0;
    'render_loop: while frame_count < frame_limit {
        for event in renderer.window.poll_events() {
            if event == WindowEvent::CloseRequested {
                break 'render_loop;
            }
        }

        let (mut cmd_buffer, image) = renderer
            .device
            .begin_frame(&renderer.swapchain)
            .expect("begin_frame failed");
        cmd_buffer.set_pipeline(&renderer.pipeline);
        cmd_buffer.bind_vertex_buffer(&renderer.vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&renderer.index_buffer, 0);
        cmd_buffer.draw_indexed(renderer.index_count, 0, 0);
        renderer
            .device
            .submit_and_present(cmd_buffer, &renderer.swapchain, image)
            .expect("submit_and_present failed");

        frame_count += 1;
        if frame_count % 60 == 0 {
            eprintln!("frame {frame_count} presented");
        }
    }
    eprintln!("walking skeleton exited cleanly");
}
