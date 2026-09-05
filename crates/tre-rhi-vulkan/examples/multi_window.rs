//! Phase 1 Step 1 proof: two native windows, opened independently, sharing
//! ONE `VulkanDevice` (ARCHITECTURE.md Section 2.1's "Global `RhiDevice`,
//! per-window `RhiSwapchain`" model) -- not two separate devices relabeled
//! to look shared. Each window draws its own independently-colored rect
//! through its own swapchain and command buffer, proving the sharing is
//! real: closing one window leaves the other rendering normally.
//!
//! Both windows use the same auto-detected backend (Wayland or X11,
//! whichever `PlatformWindow::new` picks), matching how a real desktop app
//! actually runs -- one windowing backend per process, never both at once
//! (the two backends need different Vulkan instance extensions enabled;
//! see documentation/REVIEW.md's Phase 1 Step 1 entry for why this
//! example doesn't mix them).

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tre_engine::{rgba8, RenderingCanvas, RhiDevice};
use tre_platform::{PlatformWindow, WindowEvent};
use tre_rhi_vulkan::{VulkanBuffer, VulkanDevice, VulkanPipelineState, VulkanSwapchain};

struct WindowSlot {
    pipeline: VulkanPipelineState,
    vertex_buffer: VulkanBuffer,
    index_buffer: VulkanBuffer,
    index_count: u32,
    swapchain: VulkanSwapchain,
    window: PlatformWindow,
    open: bool,
}

fn make_window_slot(
    device: &VulkanDevice,
    title: &str,
    color: u32,
    vertex_spv: &[u8],
    fragment_spv: &[u8],
) -> WindowSlot {
    let window = PlatformWindow::new(title, 480, 360).expect("failed to open window");
    let display_handle = window.display_handle().unwrap().as_raw();
    let window_handle = window.window_handle().unwrap().as_raw();
    let (surface_loader, surface) = device
        .create_surface(display_handle, window_handle)
        .expect("failed to create surface for additional window");
    let swapchain = VulkanSwapchain::new(device, surface_loader, surface, 480, 360)
        .expect("failed to create swapchain");
    let pipeline = device
        .create_pipeline(vertex_spv, fragment_spv, swapchain.format())
        .expect("failed to create pipeline");

    let mut canvas = RenderingCanvas::new();
    canvas.draw_rounded_rect(90.0, 80.0, 300.0, 200.0, color);
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
    let index_count = frame.indices.len() as u32;

    WindowSlot {
        pipeline,
        vertex_buffer,
        index_buffer,
        index_count,
        swapchain,
        window,
        open: true,
    }
}

fn render_one(device: &VulkanDevice, slot: &mut WindowSlot) {
    let (mut cmd_buffer, image) = device
        .begin_frame(&slot.swapchain)
        .expect("begin_frame failed");
    cmd_buffer.set_pipeline(&slot.pipeline);
    cmd_buffer.bind_vertex_buffer(&slot.vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&slot.index_buffer, 0);
    cmd_buffer.draw_indexed(slot.index_count, 0, 0);
    device
        .submit_and_present(cmd_buffer, &slot.swapchain, image)
        .expect("submit_and_present failed");
}

fn main() {
    // Bootstrap the device from the FIRST window (needed to pick a
    // physical device/queue family at all); every subsequent window reuses
    // it via `VulkanDevice::create_surface` instead of re-running device
    // selection -- the actual thing this demo is proving.
    let first_window =
        PlatformWindow::new("tre multi-window demo -- A", 480, 360).expect("failed to open window");
    let display_handle = first_window.display_handle().unwrap().as_raw();
    let window_handle = first_window.window_handle().unwrap().as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    let swapchain_a = VulkanSwapchain::new(&device, surface_loader, surface, 480, 360)
        .expect("failed to create swapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");

    let pipeline_a = device
        .create_pipeline(&vertex_spv, &fragment_spv, swapchain_a.format())
        .expect("failed to create pipeline");
    let mut canvas_a = RenderingCanvas::new();
    canvas_a.draw_rounded_rect(90.0, 80.0, 300.0, 200.0, rgba8(0xE0, 0xA0, 0x40, 0xFF)); // amber
    let frame_a = canvas_a.flatten();
    let vertex_buffer_a = device
        .upload_buffer(
            bytemuck::cast_slice(&frame_a.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer_a = device
        .upload_buffer(
            bytemuck::cast_slice(&frame_a.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");
    let mut slot_a = WindowSlot {
        pipeline: pipeline_a,
        vertex_buffer: vertex_buffer_a,
        index_buffer: index_buffer_a,
        index_count: frame_a.indices.len() as u32,
        swapchain: swapchain_a,
        window: first_window,
        open: true,
    };

    let mut slot_b = make_window_slot(
        &device,
        "tre multi-window demo -- B",
        rgba8(0x40, 0xA0, 0xE0, 0xFF), // blue -- deliberately distinct from A's amber
        &vertex_spv,
        &fragment_spv,
    );

    let frame_limit: u64 = std::env::var("TRE_MULTI_WINDOW_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let mut frame_count: u64 = 0;
    while frame_count < frame_limit && (slot_a.open || slot_b.open) {
        if slot_a.open {
            for event in slot_a.window.poll_events() {
                if event == WindowEvent::CloseRequested {
                    slot_a.open = false;
                }
            }
            if slot_a.open {
                render_one(&device, &mut slot_a);
            }
        }
        if slot_b.open {
            for event in slot_b.window.poll_events() {
                if event == WindowEvent::CloseRequested {
                    slot_b.open = false;
                }
            }
            if slot_b.open {
                render_one(&device, &mut slot_b);
            }
        }

        frame_count += 1;
        if frame_count % 60 == 0 {
            eprintln!(
                "frame {frame_count}: window A {}, window B {}",
                if slot_a.open { "open" } else { "closed" },
                if slot_b.open { "open" } else { "closed" }
            );
        }
    }

    unsafe {
        let _ = device.device.device_wait_idle();
    }
    eprintln!("multi-window demo exited cleanly");
}
