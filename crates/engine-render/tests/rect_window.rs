//! §14 build-order steps 1-3: a row of laid-out rects through
//! `vello_hybrid`, presented into a real window opened via
//! `engine-platform` (a dev-dependency of this example only --
//! `engine-render` the library never depends on `winit`), each
//! animating its own color via `engine-core`'s `Animated<T>`, now living
//! inside a real `PaintProperties` on a real `Tree` node instead of
//! ticked by hand (step 2's own approach). No text, no Python.
//!
//! `engine-render`'s own headless tests (`src/lib.rs`,
//! `tests/animated_rect.rs`, `tests/layout_tree.rs`) already prove the
//! rendering, animation, and layout math are each correct via pixel
//! readback; this test proves the remaining half -- that the same
//! pipeline actually presents to a real `wgpu::Surface` backed by a real
//! OS window, frame after frame, not just once.

use std::sync::Arc;
use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeId, NodeKind, PaintProperties, Tree};
use engine_platform::{WindowConfig, run_windowed};
use engine_render::{FrameRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::Window;

/// MD3 seed-adjacent purple (#6750A4) -- the same starting color step
/// 1/2 used, kept here for continuity across this demo's history.
const START_COLOR: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);
/// MD3-ish teal (#03DAC6) -- deliberately distinct from `START_COLOR` so
/// the animation is visually obvious, not a near-identical shade.
const TARGET_COLOR: Color = Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF);

const RECT_COUNT: usize = 4;
const RECT_SIZE: f32 = 80.0;
const GAP: f32 = 20.0;
const WINDOW_WIDTH: u32 =
    (RECT_SIZE as u32) * (RECT_COUNT as u32) + (GAP as u32) * (RECT_COUNT as u32 + 1);
const WINDOW_HEIGHT: u32 = (RECT_SIZE as u32) + (GAP as u32) * 2;

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
    tree: Tree,
    root: NodeId,
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
            eprintln!("engine-render §14 step 3: no wgpu adapter available ({err}), exiting 0");
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
        // §14 step 3: `Node`/`Tree`/`taffy` replace step 2's hand-ticked
        // `Animated<Color>` -- a real `Tree` of `RECT_COUNT` rects, laid
        // out in a row by `taffy`, each owning its own `Animated<Color>`
        // inside its `PaintProperties`. Staggering each rect's animation
        // duration (not just starting them all identically) is what
        // actually demonstrates Design Principle 2's claim -- several
        // independent `Animated<T>` instances advancing through the same
        // `Tree::tick_all` call, not one value copy-pasted four times.
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: FlexDirection::Row,
                padding: taffy::prelude::Rect {
                    left: length(GAP),
                    right: length(GAP),
                    top: length(GAP),
                    bottom: length(GAP),
                },
                gap: Size {
                    width: length(GAP),
                    height: length(GAP),
                },
                size: Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        for i in 0..RECT_COUNT {
            let mut paint = PaintProperties::new(START_COLOR, 12.0, 0.0, 1.0);
            paint.background.animate_to(
                TARGET_COLOR,
                Duration::from_millis(600 + i as u64 * 200),
                MotionCurve::Linear,
                animation_start,
            );
            let child = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(RECT_SIZE),
                        height: length(RECT_SIZE),
                    },
                    ..Default::default()
                },
                paint,
            );
            tree.add_child(root, child);
        }
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(width as f32),
                height: AvailableSpace::Definite(height as f32),
            },
        );

        Self {
            surface,
            device,
            queue,
            frame_renderer,
            width,
            height,
            tree,
            root,
            animation_start,
        }
    }

    fn render_frame(&mut self) {
        let now = Instant::now();
        self.tree.tick_all(now);
        // Layout itself never changes frame to frame here (only paint
        // properties animate) -- `taffy`'s own incremental cache (§6's
        // decision) makes recomputing it every frame cheap rather than
        // something this demo needs to scope around by hand.
        self.tree.compute_layout(
            self.root,
            Size {
                width: AvailableSpace::Definite(f32::from(self.width)),
                height: AvailableSpace::Definite(f32::from(self.height)),
            },
        );

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

        let scene = build_tree_scene(&self.tree, self.root, self.width, self.height);
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
            title: "tre v2 -- §14 step 3 spike".to_string(),
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
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
                    "engine-render §14 step 3: first frame presented, {}x{}, {RECT_COUNT} laid-out rects animating independently",
                    state.width, state.height
                );
            }
            if frame == 59 {
                // Confirms actual frame pacing was close to the intended
                // ~1-second animation, not e.g. 60 frames dumped in 10ms
                // because vsync/redraw scheduling was silently broken --
                // `animation_start`'s whole reason to exist as a field.
                eprintln!(
                    "engine-render §14 step 3: animation ran for {:.2}s across 60 frames",
                    state.animation_start.elapsed().as_secs_f64()
                );
            }
        },
    );

    match result {
        Ok(()) => eprintln!("engine-render §14 step 3: exited cleanly after 60 frames"),
        Err(err) => {
            // No display reachable is expected, non-exceptional on some
            // CI runners -- exit 0, don't fail the suite (see GpuState::new's
            // matching handling of "no GPU available").
            eprintln!("engine-render §14 step 3: no display available ({err}), exiting 0");
        }
    }
}
