//! `App` (§8's `PyApp`) -- collects registered `PyWindow`s and drives
//! them all together in one blocking `run()` call.
//!
//! **The `PyApp`/`PyWindow` split §11.1 needs, landing at exactly the
//! step that needs it.** Every earlier step's own module doc comment
//! ("splitting `PyWindow` back out is a refactor, not a redesign,
//! whenever step 14 actually needs a second window") predicted this
//! exact change -- `App` no longer owns a `Tree` or a size directly;
//! `PyWindow` (`window.rs`) does, one per window, and `App` only
//! orchestrates opening all of them together via `engine_platform::
//! run_windowed_multi`.
//!
//! **No `Python::detach` around the render loop, deliberately, despite
//! §9's own stated habit ("wrap layout/paint/render so per-frame work
//! never serializes behind the GIL... costs nothing today").** Real
//! finding (unchanged since step 6): `detach`'s closure must be `Ungil`,
//! which on stable pyo3 0.29 requires `Send` -- and every `PyWindow`'s
//! `Rc<RefCell<Tree>>` is `!Send` by design (§9). Revisit if a later
//! step introduces real GIL contention from a second thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use engine_core::{NodeId, Tree};
use engine_platform::{WindowConfig, WindowRequest, run_windowed_multi};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::{Window, WindowId};

use crate::window::PyWindow;

#[pyclass(unsendable)]
pub struct App {
    windows: Vec<Py<PyWindow>>,
}

/// One registered window's data, extracted once (up front, while the
/// GIL is already held by `run()`) so the `winit` closures below never
/// need to touch a Python object -- they only ever see plain Rust data
/// they already own, the same "only thin data crosses into winit's own
/// callback world" discipline `engine-platform`'s own `PlatformEvent`
/// already follows.
struct WindowSetup {
    tree: Rc<RefCell<Tree>>,
    root: NodeId,
    title: String,
    width: u32,
    height: u32,
}

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
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap_or_else(|err| {
            // No GPU reachable is an expected, non-exceptional
            // condition on some CI runners -- exit 0, don't fail the
            // process, per TRE v1's own established convention
            // (finding #261), applied identically everywhere else in
            // this workspace.
            eprintln!("engine-py: no wgpu adapter available ({err}), exiting 0");
            std::process::exit(0);
        });
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
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

struct WindowRuntime {
    tree: Rc<RefCell<Tree>>,
    root: NodeId,
    width: u32,
    height: u32,
    gpu: GpuState,
}

#[pymethods]
impl App {
    #[new]
    fn new() -> Self {
        Self {
            windows: Vec::new(),
        }
    }

    /// Registers `window` to be opened the next time `run()` is called
    /// -- §14 step 14's own "a second `PyWindow`" is exactly a second
    /// call to this before `run()`.
    fn add_window(&mut self, window: Py<PyWindow>) {
        self.windows.push(window);
    }

    /// The one blocking call (Design Principle 1) -- opens every
    /// registered `PyWindow` together and ticks/lays-out/renders each
    /// one's own `Tree` every frame, independently, until every window
    /// has closed or reached `max_frames`. `max_frames` applies to each
    /// window individually (the same headless-CI-safe convention every
    /// windowed test in this workspace already uses, TRE v1 finding
    /// #261), not to the app's own lifetime as a whole.
    #[pyo3(signature = (max_frames=None))]
    fn run(&self, py: Python<'_>, max_frames: Option<u32>) -> PyResult<()> {
        // Extracted once, up front, while `py` is already held --
        // see `WindowSetup`'s own doc comment for why nothing below
        // this point ever touches a Python object again.
        let setups: Vec<WindowSetup> = self
            .windows
            .iter()
            .map(|window| {
                let window = window.borrow(py);
                WindowSetup {
                    tree: window.tree.clone(),
                    root: window.root,
                    title: window.title.clone(),
                    width: window.width,
                    height: window.height,
                }
            })
            .collect();

        if setups.is_empty() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "App.run() called with no windows -- call add_window() at least once first",
            ));
        }

        let setups = Rc::new(setups);
        let runtimes: Rc<RefCell<HashMap<WindowId, WindowRuntime>>> =
            Rc::new(RefCell::new(HashMap::new()));

        let setups_for_created = setups.clone();
        let runtimes_for_created = runtimes.clone();
        let runtimes_for_frame = runtimes.clone();
        let runtimes_for_access = runtimes.clone();
        let setups_for_setup = setups.clone();

        let result = run_windowed_multi(
            move |window_id, token, window| {
                let setup = &setups_for_created[token as usize];
                let gpu = GpuState::new(window, setup.width, setup.height);
                runtimes_for_created.borrow_mut().insert(
                    window_id,
                    WindowRuntime {
                        tree: setup.tree.clone(),
                        root: setup.root,
                        width: setup.width,
                        height: setup.height,
                        gpu,
                    },
                );
            },
            move |window_id, _frame| {
                let mut runtimes = runtimes_for_frame.borrow_mut();
                let Some(runtime) = runtimes.get_mut(&window_id) else {
                    return;
                };

                let now = Instant::now();
                runtime.tree.borrow_mut().tick_all(now);
                runtime.tree.borrow_mut().compute_layout(
                    runtime.root,
                    Size {
                        width: AvailableSpace::Definite(runtime.width as f32),
                        height: AvailableSpace::Definite(runtime.height as f32),
                    },
                );

                let output = match runtime.gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    _ => return,
                };
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let scene = {
                    let tree_ref = runtime.tree.borrow();
                    build_tree_scene(
                        &tree_ref,
                        runtime.root,
                        runtime.width as u16,
                        runtime.height as u16,
                        runtime.gpu.frame_renderer.resources_mut(),
                        &mut runtime.gpu.text_renderer,
                    )
                };
                let render_size = RenderSize {
                    width: runtime.width,
                    height: runtime.height,
                };
                let mut encoder = runtime
                    .gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                runtime.gpu.frame_renderer.render(
                    &scene,
                    &runtime.gpu.device,
                    &runtime.gpu.queue,
                    &mut encoder,
                    &render_size,
                    &view,
                );
                runtime.gpu.queue.submit([encoder.finish()]);
                output.present();
            },
            // §14 step 7: every window this framework opens reports a
            // real accessibility tree, built fresh from that window's
            // own `Tree` -- unchanged by the multi-window split, just
            // looked up per `WindowId` now instead of assumed singular.
            move |window_id| {
                let runtimes = runtimes_for_access.borrow();
                let runtime = runtimes
                    .get(&window_id)
                    .expect("build_access_update requested for a window with no runtime state");
                runtime.tree.borrow().build_access_update(runtime.root)
            },
            move |opener| {
                for (index, setup) in setups_for_setup.iter().enumerate() {
                    opener.open_window(WindowRequest {
                        config: WindowConfig {
                            title: setup.title.clone(),
                            width: setup.width,
                            height: setup.height,
                            max_frames,
                        },
                        token: index as u64,
                    });
                }
            },
        );

        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                // A `run_windowed_multi` failure only ever means "no
                // display reachable" -- `GpuState::new`'s own no-adapter
                // case exits the process directly (above), so this is
                // the one remaining failure mode. Same TRE v1 finding
                // #261 convention as every other entry point in this
                // workspace.
                eprintln!("engine-py: no display available ({err}), exiting cleanly");
                Ok(())
            }
        }
    }
}
