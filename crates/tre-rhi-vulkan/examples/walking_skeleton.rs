//! IMPLEMENTATION.md Phase 0's walking skeleton: opens one window, clears
//! it to a color, and draws one `Canvas::draw_rounded_rect` command through
//! the real `Canvas -> IR -> RhiCommandBuffer::draw_indexed` pipeline.
//!
//! `winit` is a Phase-0-only expedient for window creation -- Phase 1
//! (IMPLEMENTATION.md Step 1.1) replaces this with the real native
//! per-platform bridges (Win32/Wayland/Cocoa) documented for the engine.

use std::sync::Arc;

use ash::vk;
use tre_engine::{rgba8, RenderingCanvas, RhiDevice};
use tre_rhi_vulkan::{VulkanBuffer, VulkanDevice, VulkanPipelineState, VulkanSwapchain};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

// Field order matters: Rust drops a struct's fields in DECLARATION order
// (not reverse), so everything that holds a handle into the Vulkan device
// must be declared -- and therefore dropped -- BEFORE `device` itself.
// Getting this backwards is exactly what produced the Vulkan validation
// layer's "N leaked objects" / use-after-destroy errors during the first
// working version of this example.
struct Renderer {
    swapchain: VulkanSwapchain,
    pipeline: VulkanPipelineState,
    vertex_buffer: VulkanBuffer,
    index_buffer: VulkanBuffer,
    index_count: u32,
    frame_count: u64,
    device: VulkanDevice,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // Even with correct field order, every one of this struct's Drop
        // impls calls a `vkDestroy*` that requires the GPU to be done with
        // that object first. A custom `Drop` runs BEFORE the automatic
        // per-field drops, so waiting here guarantees nothing downstream
        // destroys a resource the GPU might still be using.
        unsafe {
            let _ = self.device.device.device_wait_idle();
        }
    }
}

// Same rule as `Renderer`'s field order above: `renderer` owns a
// `VulkanSwapchain` tied to `window`'s Wayland/X11 surface, so `renderer`
// must drop (destroying the swapchain) before `window` does (destroying
// the surface it points at) -- hence `renderer` declared first here.
struct App {
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("tre walking skeleton (Phase 0)")
                        .with_inner_size(winit::dpi::LogicalSize::new(640.0, 480.0)),
                )
                .expect("failed to create window"),
        );

        use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
        let display_handle = window.display_handle().unwrap().as_raw();
        let window_handle = window.window_handle().unwrap().as_raw();

        let (device, surface_loader, surface) = VulkanDevice::new(display_handle, window_handle)
            .expect("failed to create VulkanDevice");

        let size = window.inner_size();
        let swapchain =
            VulkanSwapchain::new(&device, surface_loader, surface, size.width, size.height)
                .expect("failed to create VulkanSwapchain");

        let out_dir = env!("OUT_DIR");
        let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
            .expect("failed to read compiled vertex shader");
        let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
            .expect("failed to read compiled fragment shader");
        let pipeline = device
            .create_pipeline(&vertex_spv, &fragment_spv, swapchain.format())
            .expect("failed to create pipeline");

        // Phase 0: one Canvas call, one rounded rect (rendered as a flat
        // quad -- IMPLEMENTATION.md Phase 3.2 owns the real SDF shader).
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

        self.renderer = Some(Renderer {
            device,
            swapchain,
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            frame_count: 0,
        });
        self.window = Some(window);
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                let renderer = self.renderer.as_mut().expect("renderer not initialized");

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

                renderer.frame_count += 1;
                if renderer.frame_count % 60 == 0 {
                    eprintln!("frame {} presented", renderer.frame_count);
                }

                // Phase 0: exit after a fixed number of frames so this
                // walking skeleton is a scriptable proof, not a
                // human-operated demo. Overridable for manual/visual
                // verification runs without editing source each time.
                let frame_limit: u64 = std::env::var("TRE_WALKING_SKELETON_FRAMES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(120);
                if renderer.frame_count >= frame_limit {
                    event_loop.exit();
                } else {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App {
        window: None,
        renderer: None,
    };
    event_loop.run_app(&mut app).expect("event loop error");
    eprintln!("walking skeleton exited cleanly");
}
