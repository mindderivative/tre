//! `App` (§8's `PyApp`, with `PyWindow`'s single-window slice folded in
//! directly for this step -- see the module doc note below) -- the
//! entry point a `.py` script actually imports and drives.
//!
//! **Deliberately one implicit window, not §8's separate
//! `PyApp`/`PyWindow` split.** That split exists for §11.1's multi-window
//! model, which is build-order step 14, not this one; `App` owning one
//! `Tree` and one window directly is the real minimal slice §14 step 6
//! asks for ("node creation + one property setter... drive step 2's
//! animation from a `.py` script"). Splitting `PyWindow` back out is a
//! refactor, not a redesign, whenever step 14 actually needs a second
//! window.
//!
//! **No `Python::detach` around the render loop, deliberately, despite
//! §9's own stated habit ("wrap layout/paint/render so per-frame work
//! never serializes behind the GIL... costs nothing today").** Real
//! finding: `detach`'s closure must be `Ungil`, which on stable pyo3
//! 0.29 requires `Send` (`unsafe impl<T: Send> Ungil for T {}`, checked
//! directly in pyo3's own source) -- and `Rc<RefCell<Tree>>` is `!Send`
//! by design (§9's own architecture decision, not an oversight). Wrapping
//! this loop in `detach` would need either an unsafe manual GIL release
//! bypassing `Ungil`'s safety guarantee, or migrating `Tree` off
//! `Rc<RefCell<>>` -- neither warranted, since (matching §9's own
//! reasoning for why this is "not a live requirement") nothing in this
//! step's scope contends for the GIL from a second thread. Revisit if a
//! later step introduces one.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use engine_platform::{WindowConfig, run_windowed};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Rect as TaffyRect, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::Window;

const PADDING: f32 = 16.0;
const GAP: f32 = 16.0;

use crate::node::Node;

#[pyclass(unsendable)]
pub struct App {
    tree: Rc<RefCell<Tree>>,
    root: NodeId,
    width: u32,
    height: u32,
}

#[pymethods]
impl App {
    #[new]
    #[pyo3(signature = (width=480, height=200))]
    fn new(width: u32, height: u32) -> Self {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Row,
                padding: TaffyRect {
                    left: length(PADDING),
                    right: length(PADDING),
                    top: length(PADDING),
                    bottom: length(PADDING),
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
        Self {
            tree: Rc::new(RefCell::new(tree)),
            root,
            width,
            height,
        }
    }

    /// §14 step 6's own "node creation" -- one shape (a colored rect, a
    /// child of the app's implicit root row) is the real minimal slice;
    /// more `NodeKind`s/parameters are additive whenever a later step
    /// needs them from Python specifically (building a whole tree from a
    /// parsed `view.yaml` via Python is `engine-spec`'s own future FFI
    /// surface, not this one).
    fn add_rect(&self, background: (u8, u8, u8, u8), width: f32, height: f32) -> Node {
        let (r, g, b, a) = background;
        let mut tree = self.tree.borrow_mut();
        let id = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(width),
                    height: length(height),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(r, g, b, a), 0.0, 0.0, 1.0),
        );
        tree.add_child(self.root, id);
        Node {
            id,
            tree: self.tree.clone(),
        }
    }

    /// The one blocking call (Design Principle 1) -- opens a real window,
    /// ticks/lays-out/renders this app's `Tree` every frame until closed
    /// or `max_frames` is reached. `max_frames` exists so a CI smoke test
    /// (or this crate's own tests) can run this to completion instead of
    /// waiting for a human to close the window -- the same
    /// headless-CI-safe convention every windowed Rust test in this
    /// workspace already uses (TRE v1 finding #261).
    #[pyo3(signature = (max_frames=None))]
    fn run(&self, max_frames: Option<u32>) -> PyResult<()> {
        let tree = self.tree.clone();
        let root = self.root;
        let width = self.width;
        let height = self.height;

        struct GpuState {
            surface: wgpu::Surface<'static>,
            device: wgpu::Device,
            queue: wgpu::Queue,
            frame_renderer: FrameRenderer,
            text_renderer: TextRenderer,
        }

        impl GpuState {
            fn new(window: Arc<Window>, width: u32, height: u32) -> Self {
                let instance = wgpu::Instance::default();
                let surface = instance
                    .create_surface(window)
                    .expect("failed to create wgpu surface from the window");
                let adapter =
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::default(),
                        force_fallback_adapter: false,
                        compatible_surface: Some(&surface),
                    }))
                    .unwrap_or_else(|err| {
                        // No GPU reachable is an expected, non-exceptional
                        // condition on some CI runners -- exit 0, don't fail
                        // the process, per TRE v1's own established
                        // convention (finding #261), applied identically
                        // everywhere else in this workspace.
                        eprintln!("engine-py: no wgpu adapter available ({err}), exiting 0");
                        std::process::exit(0);
                    });
                let (device, queue) =
                    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                        label: Some("engine-py app device"),
                        required_features: wgpu::Features::empty(),
                        ..Default::default()
                    }))
                    .expect("failed to create wgpu device");

                let config = surface
                    .get_default_config(&adapter, width, height)
                    .expect("surface is not supported by this adapter");
                surface.configure(&device, &config);

                let frame_renderer = FrameRenderer::new(
                    &device,
                    &RenderTargetConfig {
                        format: config.format,
                        width,
                        height,
                    },
                );

                Self {
                    surface,
                    device,
                    queue,
                    frame_renderer,
                    text_renderer: TextRenderer::new(),
                }
            }
        }

        let mut gpu: Option<GpuState> = None;
        let result = run_windowed(
            WindowConfig {
                title: "tre v2".to_string(),
                width,
                height,
                max_frames,
            },
            move |window, _frame| {
                let state = gpu.get_or_insert_with(|| GpuState::new(window.clone(), width, height));

                let now = Instant::now();
                tree.borrow_mut().tick_all(now);
                tree.borrow_mut().compute_layout(
                    root,
                    Size {
                        width: AvailableSpace::Definite(width as f32),
                        height: AvailableSpace::Definite(height as f32),
                    },
                );

                let output = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    _ => return,
                };
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let scene = {
                    let tree_ref = tree.borrow();
                    build_tree_scene(
                        &tree_ref,
                        root,
                        width as u16,
                        height as u16,
                        state.frame_renderer.resources_mut(),
                        &mut state.text_renderer,
                    )
                };
                let render_size = RenderSize { width, height };
                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                state.frame_renderer.render(
                    &scene,
                    &state.device,
                    &state.queue,
                    &mut encoder,
                    &render_size,
                    &view,
                );
                state.queue.submit([encoder.finish()]);
                output.present();
            },
        );

        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                // A `run_windowed` failure only ever means "no display
                // reachable" -- `GpuState::new`'s own no-adapter case
                // exits the process directly (above), so this is the
                // one remaining failure mode. Same TRE v1 finding #261
                // convention as every other entry point in this
                // workspace: expected and non-exceptional on a headless
                // CI runner, so this returns `Ok(())`, not a raised
                // Python exception -- `app.run()` behaving like every
                // other graceful-exit-on-missing-display call here,
                // not a special case a `.py` script has to catch.
                eprintln!("engine-py: no display available ({err}), exiting cleanly");
                Ok(())
            }
        }
    }
}
