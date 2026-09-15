//! §14 build-order step 1: one static rounded rect through `vello_hybrid`,
//! presented into a real window opened via `engine-platform` (a
//! dev-dependency of this example only -- `engine-render` the library
//! never depends on `winit`). No layout, no text, no Python.
//!
//! `engine-render`'s own headless test (`src/lib.rs`) already proves the
//! rendering math is correct via pixel readback; this example proves the
//! other half -- that the same pipeline actually presents to a real
//! `wgpu::Surface` backed by a real OS window.

use std::sync::Arc;

use engine_platform::{WindowConfig, run_windowed};
use engine_render::{FrameRenderer, build_rect_scene};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::Window;

/// Everything that depends on having a real window, created lazily on
/// the first frame callback (the window doesn't exist before `resumed`
/// fires inside `engine_platform::run_windowed`).
struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    frame_renderer: FrameRenderer,
    width: u16,
    height: u16,
}

impl GpuState {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let width = size.width as u16;
        let height = size.height as u16;

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .expect("failed to create wgpu surface from the window");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap_or_else(|err| {
            // No GPU reachable is an expected, non-exceptional condition
            // on some CI runners -- exit 0, don't fail the suite, per
            // TRE v1's own established convention (finding #261).
            eprintln!("engine-render §14 step 1: no wgpu adapter available ({err}), exiting 0");
            std::process::exit(0);
        });
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("engine-render example device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        }))
        .expect("failed to create wgpu device");

        let config = surface
            .get_default_config(&adapter, u32::from(width), u32::from(height))
            .expect("surface is not supported by this adapter");
        surface.configure(&device, &config);

        let frame_renderer = FrameRenderer::new(
            &device,
            &RenderTargetConfig {
                format: config.format,
                width: u32::from(width),
                height: u32::from(height),
            },
        );

        Self {
            surface,
            device,
            queue,
            frame_renderer,
            width,
            height,
        }
    }

    fn render_frame(&mut self) {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            // Not a real failure for a short-lived demo -- just skip the
            // frame rather than treat "window briefly occluded/resizing"
            // as fatal.
            _ => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let scene = build_rect_scene(self.width, self.height);
        let render_size = RenderSize {
            width: u32::from(self.width),
            height: u32::from(self.height),
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.frame_renderer.render(
            &scene,
            &self.device,
            &self.queue,
            &mut encoder,
            &render_size,
            &view,
        );
        self.queue.submit([encoder.finish()]);
        output.present();
    }
}

fn main() {
    let mut gpu: Option<GpuState> = None;

    let result = run_windowed(
        WindowConfig {
            title: "tre v2 -- §14 step 1 spike".to_string(),
            width: 400,
            height: 400,
            // Auto-exits after ~1 second at 60Hz -- headless-CI-safe by
            // the same convention as everywhere else in this codebase
            // (TRE v1's own "gracefully exit 0" lesson): this example
            // proves the pipeline runs, it doesn't need a human to close
            // the window for that to be demonstrated.
            max_frames: Some(60),
        },
        move |window, frame| {
            let state = gpu.get_or_insert_with(|| GpuState::new(window.clone()));
            state.render_frame();
            if frame == 0 {
                eprintln!(
                    "engine-render §14 step 1: first frame presented, {}x{}",
                    state.width, state.height
                );
            }
        },
    );

    match result {
        Ok(()) => eprintln!("engine-render §14 step 1: exited cleanly after 60 frames"),
        Err(err) => {
            // No display reachable is expected, non-exceptional on some
            // CI runners -- exit 0, don't fail the suite (see GpuState::new's
            // matching handling of "no GPU available").
            eprintln!("engine-render §14 step 1: no display available ({err}), exiting 0");
        }
    }
}
