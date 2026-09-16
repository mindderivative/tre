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

use engine_core::{InputEvent, NodeId, PointerButton, Tree, from_access_id};
use engine_platform::{WindowConfig, WindowRequest, run_windowed_multi};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::{Window, WindowId};

use crate::dispatch::{HandlerMap, interaction_config, open_context_menu, run_dispatch_outcome};
use crate::dock::{self, SharedDockState};
use crate::window::{PyWindow, SharedTheme};

#[pyclass(unsendable)]
pub struct App {
    windows: Vec<Py<PyWindow>>,
}

/// One registered window's data, extracted once (up front, while the
/// GIL is already held by `run()`) so the `winit` closures below never
/// need to touch a Python object -- they only ever see plain Rust data
/// they already own, the same "only thin data crosses into winit's own
/// callback world" discipline `engine-platform`'s own `PlatformEvent`
/// already follows. `handlers` is the one exception: `Node.
/// set_on_click`'s own real `Py<PyAny>` callbacks (M4 Phase 1 step 3)
/// have to be looked up by the `on_input` closure below on a real
/// activation, so this is the one Python-object-bearing field extracted
/// here rather than converted to plain data.
struct WindowSetup {
    tree: Rc<RefCell<Tree>>,
    root: NodeId,
    title: String,
    width: u32,
    height: u32,
    handlers: HandlerMap,
    /// M4 Phase 7 (§11.3): `anchor NodeId -> content NodeId`, plain
    /// data (no `Py<PyAny>`), extracted the same way `handlers` is.
    context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
    /// M4 Phase 9 (§11.4): real docking state, extracted the same way.
    dock: SharedDockState,
    /// M7 Phase 3 (§7.1): the window's own theme, extracted the same
    /// way -- the real live-switch path (`on_input`'s new `ThemeChanged`
    /// arm, below) needs to mutate it, so it stays a shared handle,
    /// never copied to a plain snapshot.
    theme: SharedTheme,
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
    handlers: HandlerMap,
    context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
    dock: SharedDockState,
    theme: SharedTheme,
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
                    handlers: window.handlers.clone(),
                    context_menus: window.context_menus.clone(),
                    dock: window.dock.clone(),
                    theme: window.theme.clone(),
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
        let runtimes_for_input = runtimes.clone();
        let runtimes_for_access_action = runtimes.clone();
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
                        handlers: setup.handlers.clone(),
                        context_menus: setup.context_menus.clone(),
                        dock: setup.dock.clone(),
                        theme: setup.theme.clone(),
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
            // M4 Phase 1 step 3: the real "meaning-dependent" half
            // `Tree::dispatch` leaves for its own caller (§2 Design
            // Principle 6) -- every mechanical consequence (hover, focus
            // movement, ripple-spawn-on-press) already happened inside
            // `dispatch` itself; this closure's only job is to look up
            // and call a registered `Node.set_on_click` handler when
            // `dispatch` reports a real activation.
            move |window_id, event| {
                let runtimes = runtimes_for_input.borrow();
                let Some(runtime) = runtimes.get(&window_id) else {
                    return;
                };
                let outcome = runtime.tree.borrow_mut().dispatch(
                    runtime.root,
                    event,
                    &interaction_config(),
                    Instant::now(),
                );
                run_dispatch_outcome(&runtime.handlers, outcome, py);
                // M4 Phase 7 (§11.3): the real, winit-driven path a
                // genuine right-click reaches -- `Window.right_click`/
                // `View.right_click` are the no-live-window-needed test
                // entry points, this is where an actual mouse arrives.
                open_context_menu(&runtime.tree, &runtime.context_menus, runtime.root, outcome);
                // M4 Phase 9 (§11.4): the real, winit-driven path a
                // genuine panel drag reaches -- `Window.start_panel_drag`/
                // `drop_panel_at` are the no-live-window-needed test
                // entry points, this is where an actual mouse arrives.
                // Inspects the raw `event` directly (not `outcome`) --
                // "which node is a drag handle" is meaning-dependent
                // bookkeeping only `engine-py`'s own `dock` module
                // knows, not something `Tree::dispatch` has any reason
                // to report through `DispatchOutcome`.
                match event {
                    InputEvent::PointerPressed {
                        position,
                        button: PointerButton::Primary,
                    } => {
                        if let Some(hit) = runtime.tree.borrow().hit_test(runtime.root, position) {
                            dock::start_drag(&runtime.dock, hit);
                        }
                    }
                    InputEvent::PointerReleased {
                        position,
                        button: PointerButton::Primary,
                    } => {
                        dock::end_drag_at(&runtime.dock, &runtime.tree, runtime.root, position);
                    }
                    // M7 Phase 3 (§7.1, Step 3): the real, winit-driven
                    // live theme switch -- `Window.set_theme`'s own
                    // no-live-window-needed counterpart, this is where
                    // an actual OS appearance change reaches. Updates
                    // which of the theme's two schemes is active, then
                    // re-pushes the freshly resolved "on-surface" color
                    // into every already-opted-in node the identical
                    // way `set_theme` itself does.
                    InputEvent::ThemeChanged { dark } => {
                        let mut state = runtime.theme.borrow_mut();
                        state.set_dark(dark);
                        let tint = state.on_surface();
                        drop(state);
                        runtime.tree.borrow_mut().set_all_interaction_tints(tint);
                    }
                    _ => {}
                }
            },
            // M4 Phase 2 (§10): a real screen reader naming a node to
            // activate or focus directly, routed through the exact same
            // `run_dispatch_outcome`/`handlers` path a mouse click or
            // `Window.click()` already uses -- one click-handling
            // mechanism, reached three ways now, not three separate ones.
            move |window_id, request| {
                let runtimes = runtimes_for_access_action.borrow();
                let Some(runtime) = runtimes.get(&window_id) else {
                    return;
                };
                let node = from_access_id(request.target_node);
                let mut tree = runtime.tree.borrow_mut();
                match request.action {
                    engine_core::Action::Click => {
                        let outcome = tree.activate(node);
                        drop(tree);
                        run_dispatch_outcome(&runtime.handlers, outcome, py);
                    }
                    engine_core::Action::Focus => {
                        let config = interaction_config();
                        tree.set_focus_to(
                            node,
                            config.focus_ring_opacity,
                            config.focus_ring_duration,
                            Instant::now(),
                        );
                    }
                    // No other accesskit action has real dispatch
                    // meaning yet (§14 step 7's own original minimal
                    // scope, still the right boundary here -- nothing
                    // in this codebase models scrolling, text
                    // selection, or custom actions).
                    _ => {}
                }
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
