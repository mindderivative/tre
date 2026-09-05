//! Phase 1 Step 2 proof: real pointer/keyboard input, routed to the
//! correct window. Opens TWO native windows (like `multi_window.rs`, on
//! one shared `PlatformConnection`) so the demo also proves per-window
//! routing -- moving/clicking/typing in window A must never print as an
//! event for window B and vice versa -- and prints every translated
//! `InputEvent` to the terminal, tagged with which window it came from,
//! while the scene keeps rendering. Step 1.1's demos couldn't exercise
//! input at all; this is the demo that shows it now works.

use ash::vk;
use raw_window_handle::HasDisplayHandle;
use tre_engine::{rgba8, InputEvent, RenderingCanvas, RhiDevice, WindowId};
use tre_platform::PlatformConnection;
use tre_rhi_vulkan::{VulkanBuffer, VulkanDevice, VulkanPipelineState, VulkanSwapchain};

struct WindowSlot {
    label: &'static str,
    pipeline: VulkanPipelineState,
    vertex_buffer: VulkanBuffer,
    index_buffer: VulkanBuffer,
    index_count: u32,
    swapchain: VulkanSwapchain,
    window: WindowId,
    open: bool,
}

fn make_window_slot(
    connection: &mut PlatformConnection,
    device: &VulkanDevice,
    title: &str,
    label: &'static str,
    color: u32,
    vertex_spv: &[u8],
    fragment_spv: &[u8],
) -> WindowSlot {
    let window = connection
        .create_window(title, 480, 360)
        .expect("failed to open window");
    let display_handle = connection.display_handle().unwrap().as_raw();
    let window_handle = connection.window_handle(window).unwrap().as_raw();
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
        label,
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

/// Resolves an `InputEvent`'s `WindowId` to whichever slot's label it
/// belongs to, so printed output reads "A"/"B" instead of an opaque
/// `WindowId(n)` -- the actual routing correctness check is that this
/// lookup is doing real work (matching the right slot), not that it
/// prints something.
fn label_for(window: WindowId, slot_a: &WindowSlot, slot_b: &WindowSlot) -> &'static str {
    if window == slot_a.window {
        slot_a.label
    } else if window == slot_b.window {
        slot_b.label
    } else {
        "?"
    }
}

fn main() {
    let mut connection = PlatformConnection::new().expect("failed to connect to display server");

    let window_a = connection
        .create_window("tre input demo -- A", 480, 360)
        .expect("failed to open window");
    let display_handle = connection.display_handle().unwrap().as_raw();
    let window_handle = connection.window_handle(window_a).unwrap().as_raw();
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
        label: "A",
        pipeline: pipeline_a,
        vertex_buffer: vertex_buffer_a,
        index_buffer: index_buffer_a,
        index_count: frame_a.indices.len() as u32,
        swapchain: swapchain_a,
        window: window_a,
        open: true,
    };

    let mut slot_b = make_window_slot(
        &mut connection,
        &device,
        "tre input demo -- B",
        "B",
        rgba8(0x40, 0xA0, 0xE0, 0xFF), // blue
        &vertex_spv,
        &fragment_spv,
    );

    eprintln!("two windows open -- A (amber) and B (blue).");
    eprintln!("move the mouse, click, and press keys in each window; every");
    eprintln!("event prints below tagged with the window it came from.");
    eprintln!("close both windows (or wait for the frame budget) to exit.");

    let frame_limit: u64 = std::env::var("TRE_INPUT_DEMO_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);

    let mut frame_count: u64 = 0;
    while frame_count < frame_limit && (slot_a.open || slot_b.open) {
        for event in connection.poll_events() {
            match event {
                InputEvent::CloseRequested { window } => {
                    let label = label_for(window, &slot_a, &slot_b);
                    eprintln!("[{label}] close requested");
                    if window == slot_a.window {
                        slot_a.open = false;
                    } else if window == slot_b.window {
                        slot_b.open = false;
                    }
                }
                InputEvent::PointerMoved { window, x, y } => {
                    let label = label_for(window, &slot_a, &slot_b);
                    eprintln!("[{label}] pointer moved to ({x:.1}, {y:.1})");
                }
                InputEvent::PointerButton {
                    window,
                    button,
                    state,
                } => {
                    let label = label_for(window, &slot_a, &slot_b);
                    eprintln!("[{label}] pointer button {button:?} {state:?}");
                }
                InputEvent::KeyboardKey {
                    window,
                    key_code,
                    state,
                } => {
                    let label = label_for(window, &slot_a, &slot_b);
                    eprintln!("[{label}] key {key_code} {state:?}");
                }
                InputEvent::Resized {
                    window,
                    width,
                    height,
                } => {
                    let label = label_for(window, &slot_a, &slot_b);
                    eprintln!("[{label}] resized to {width}x{height}");
                }
            }
        }

        if slot_a.open {
            render_one(&device, &mut slot_a);
        }
        if slot_b.open {
            render_one(&device, &mut slot_b);
        }
        frame_count += 1;
    }

    unsafe {
        let _ = device.device.device_wait_idle();
    }
    eprintln!("input demo exited cleanly");
}
