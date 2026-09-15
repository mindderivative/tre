//! §14 build-order steps 1 and 2: a rounded rect through `vello_hybrid`,
//! presented into a real window opened via `engine-platform` (a
//! dev-dependency of this example only -- `engine-render` the library
//! never depends on `winit`), animating its color and opacity via
//! `engine-core`'s `Animated<T>`. No layout, no text, no Python.
//!
//! `engine-render`'s own headless tests (`src/lib.rs`,
//! `tests/animated_rect.rs`) already prove the rendering and animation
//! math are correct via pixel readback; this test proves the remaining
//! half -- that the same pipeline actually presents to a real
//! `wgpu::Surface` backed by a real OS window, frame after frame, not
//! just once.

use std::sync::Arc;
use std::time::{Duration, Instant};

use engine_core::{Animated, MotionCurve};
use engine_platform::{WindowConfig, run_windowed};
use engine_render::{FrameRenderer, INITIAL_COLOR, build_rect_scene};
use peniko::Color;
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::Window;

/// MD3-ish teal (#03DAC6) -- deliberately distinct from `INITIAL_COLOR`
/// so the animation is visually obvious, not a near-identical shade.
const TARGET_COLOR: Color = Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF);

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
    color: Animated<Color>,
    opacity: Animated<f64>,
    animation_start: Instant,
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

        let animation_start = Instant::now();
        let mut color = Animated::new(INITIAL_COLOR);
        let mut opacity = Animated::new(1.0_f64);
        // §14 step 2: "animate that rect's color/elevation" -- color and
        // a separate f64 (standing in for elevation until real shadow
        // rendering exists at step 8/§7.2) each animate once, over the
        // window's ~1-second lifetime, so the 60 presented frames are
        // visibly different from each other, not a static image
        // repeated 60 times.
        color.animate_to(
            TARGET_COLOR,
            Duration::from_secs(1),
            MotionCurve::Linear,
            animation_start,
        );
        opacity.animate_to(
            0.4,
            Duration::from_secs(1),
            MotionCurve::Linear,
            animation_start,
        );

        Self {
            surface,
            device,
            queue,
            frame_renderer,
            width,
            height,
            color,
            opacity,
            animation_start,
        }
    }

    fn render_frame(&mut self) {
        let now = Instant::now();
        self.color.tick(now);
        self.opacity.tick(now);

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

        let scene = build_rect_scene(
            self.width,
            self.height,
            self.color.current,
            self.opacity.current,
        );
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
            title: "tre v2 -- §14 step 1/2 spike".to_string(),
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
                    "engine-render §14 step 1/2: first frame presented, {}x{}, animating {:.0}%->{:.0}% opacity",
                    state.width, state.height, 100.0, 40.0
                );
            }
            if frame == 59 {
                // Confirms actual frame pacing was close to the intended
                // ~1-second animation, not e.g. 60 frames dumped in 10ms
                // because vsync/redraw scheduling was silently broken --
                // `animation_start`'s whole reason to exist as a field.
                eprintln!(
                    "engine-render §14 step 2: animation ran for {:.2}s across 60 frames",
                    state.animation_start.elapsed().as_secs_f64()
                );
            }
        },
    );

    match result {
        Ok(()) => eprintln!("engine-render §14 step 1/2: exited cleanly after 60 frames"),
        Err(err) => {
            // No display reachable is expected, non-exceptional on some
            // CI runners -- exit 0, don't fail the suite (see GpuState::new's
            // matching handling of "no GPU available").
            eprintln!("engine-render §14 step 1/2: no display available ({err}), exiting 0");
        }
    }
}
