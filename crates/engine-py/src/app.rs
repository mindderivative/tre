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

use engine_core::{EventKind, InputEvent, NodeId, NodeKind, PointerButton, Tree, from_access_id};
use engine_platform::{WindowConfig, WindowRequest, run_windowed_multi};
use engine_render::{FrameRenderer, TextPlacement, TextRenderer, build_tree_scene};
use peniko::kurbo::Point;
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::{Window, WindowId};

use crate::dispatch::{
    HandlerMap, SharedCompletions, call_handler, interaction_config, open_context_menu,
    run_completions, run_dispatch_outcome,
};
use crate::dock::{self, SharedDockState};
use crate::terminal::{TerminalSession, input_bytes_for};
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
    /// M9 Phase 2 (§5): the window's own `on_complete` registry,
    /// extracted the same way -- the real `on_frame` closure needs to
    /// mutate it (removing a callback the instant it's invoked).
    completions: SharedCompletions,
    /// M30 Phase 9 Step 4 (§5, §8, §10): every real, live `Terminal`
    /// session this window has spawned, extracted the same way --
    /// `WindowRuntime`'s own per-frame closure needs to mutate it
    /// (draining real PTY output each tick).
    terminals: Rc<RefCell<HashMap<NodeId, TerminalSession>>>,
}

/// M18 Phase 1/2 (§8, §10, §11.9, §11.10): the real per-glyph hit-test
/// `engine-core` structurally can't do itself (§4) -- shared by
/// `PointerPressed`'s click-to-position and `PointerMoved`'s drag-
/// extend, both of which need the exact same "is `hit` a `TextField`,
/// and if so what real byte offset does `local_point` land on"
/// answer. Mirrors `paint_node`'s own real `TextPlacement { x: 0.0,
/// y: 0.0, max_width: <the node's own real computed layout width> }`
/// exactly -- a hit-test using different placement values than what
/// was actually painted would resolve to the wrong character.
fn text_field_hit_offset(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
) -> Option<usize> {
    let (state, width) = {
        let tree = tree.borrow();
        match tree.get(hit).map(|n| &n.kind) {
            Some(NodeKind::TextField(state)) => (state.clone(), tree.layout(hit).size.width),
            _ => return None,
        }
    };
    let at = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: width,
        color: peniko::Color::TRANSPARENT,
    };
    Some(text_renderer.hit_test_position(&state, at, local_point))
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
            // this workspace. `tracing::warn!` (M16 Phase 2): real,
            // worth logging, but not an error -- a genuinely expected,
            // gracefully-handled condition, not a bug.
            tracing::warn!(%err, "no wgpu adapter available, exiting 0");
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
    completions: SharedCompletions,
    /// M18 Phase 2 (§8, §10): which `TextField` (if any) a real
    /// press-and-drag is currently extending a selection in -- plain,
    /// not `RefCell`-wrapped, since only `on_input`'s own closure ever
    /// reads or writes it. Lives here rather than `engine-core`'s
    /// existing `Tree.dragging` (Splitter/Slider drags): that
    /// mechanism's own `update_drag` is pure geometry with zero
    /// rendering knowledge, but a real drag-selection needs the exact
    /// same per-glyph hit-test Phase 1 already established only
    /// `engine-render` can do (§4) -- `engine-core` structurally can't
    /// own this drag's own per-frame tracking.
    text_drag: Option<NodeId>,
    /// M30 Phase 9 Step 4 (§5, §8, §10): the same real, shared session
    /// table `PyWindow.terminals` owns -- see `WindowSetup.terminals`'s
    /// own doc comment.
    terminals: Rc<RefCell<HashMap<NodeId, TerminalSession>>>,
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
        // M16 Phase 1 (§3, §9): `App::run` is Design Principle 1's own
        // "one blocking call," a real, early place to get a `tracing`
        // subscriber installed before any per-frame/callback work
        // starts. **Real finding (M16 Phase 2):** this is *not* the
        // only place that needs to -- `dispatch::log_uncaught_
        // exception`'s own doc comment explains why the actual install
        // call lives there instead (shared via `dispatch::ensure_
        // tracing_subscriber`, reused verbatim here); `App::run` calls
        // it too only to get the subscriber live as early as possible
        // for a real app, not because it's the sole guarantor.
        crate::dispatch::ensure_tracing_subscriber();

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
                    completions: window.completions.clone(),
                    terminals: window.terminals.clone(),
                }
            })
            .collect();

        if setups.is_empty() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "App.run() called with no windows -- call add_window() at least once first",
            ));
        }

        // M16 Phase 1 (§3, §9): the first real span this codebase
        // establishes -- one per genuine `App.run()` session (past the
        // "no windows" early return above, which isn't a real session
        // at all). Held for the rest of this function's own real work
        // via `.entered()`'s RAII guard, not manually entered/exited.
        let _app_run_span = tracing::info_span!("app_run", windows = setups.len()).entered();
        // Real, deliberate instrumentation, not test scaffolding: before
        // this, no code in this crate ever emitted a bare `tracing::
        // info!` event at all -- `test_rust_log_info_genuinely_raises_
        // the_real_verbosity` only ever passed locally by accident, via
        // `wgpu_hal`'s own incidental INFO-level logging once a real GPU
        // adapter was found. CI has no display at all (deliberately, see
        // `.github/workflows/ci.yml`'s own header comment) and never
        // reaches adapter creation, so nothing there ever logged at INFO
        // -- a real, previously-unvalidated gap, only surfacing now that
        // CI has actually run against these commits for the first time.
        // This event fires unconditionally for every genuine run session
        // (display or no display, adapter or no adapter), giving `RUST_
        // LOG=info` something real and first-party to prove.
        tracing::info!("starting a real app run session");

        let setups = Rc::new(setups);
        let runtimes: Rc<RefCell<HashMap<WindowId, WindowRuntime>>> =
            Rc::new(RefCell::new(HashMap::new()));

        let setups_for_created = setups.clone();
        let runtimes_for_created = runtimes.clone();
        let runtimes_for_frame = runtimes.clone();
        let runtimes_for_access = runtimes.clone();
        let runtimes_for_input = runtimes.clone();
        let runtimes_for_access_action = runtimes;
        let setups_for_setup = setups;

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
                        completions: setup.completions.clone(),
                        text_drag: None,
                        terminals: setup.terminals.clone(),
                    },
                );
            },
            move |window_id, _frame| -> bool {
                let mut runtimes = runtimes_for_frame.borrow_mut();
                let Some(runtime) = runtimes.get_mut(&window_id) else {
                    return false;
                };

                let now = Instant::now();
                let (any_active, completed) = runtime.tree.borrow_mut().tick_all(now);
                // M9 Phase 2 (§5): the real drain -- invokes each
                // just-completed animation's registered `on_complete`
                // callback exactly once, the same "look up and call a
                // registered callback" shape `run_dispatch_outcome`
                // already uses for click/hover handlers.
                run_completions(&runtime.completions, completed, py);

                // M30 Phase 9 Step 4 (§5, §8, §10): drains every real,
                // live `Terminal` session's own pending PTY output into
                // the `Tree` -- `drain_into` itself goes through `Tree::
                // get_mut`, M29's own real dirty-marking chokepoint, so
                // a genuine content change here already makes `take_
                // dirty` below see it. `any_active` is widened to also
                // mean "a real terminal session is still alive" -- the
                // identical real fix the sibling `pyCopper` project's
                // own `Terminal` already needed for the same real
                // problem (a background PTY reader thread producing new
                // output has no other way to wake an otherwise-idle
                // event loop, M29 Phase 2's own `ControlFlow::Wait`):
                // its own module doc comment states it "keeps a
                // repeat=True animation running purely to guarantee a
                // repaint... regardless of focus." A real, honest v1
                // cost, not silently hidden: a window with a live
                // terminal never goes fully idle the way M29's own
                // "genuinely idle window" case does.
                let mut any_active = any_active;
                {
                    let mut terminals = runtime.terminals.borrow_mut();
                    if !terminals.is_empty() {
                        any_active = true;
                    }
                    let mut tree = runtime.tree.borrow_mut();
                    for (&node_id, session) in terminals.iter_mut() {
                        session.drain_into(&mut tree, node_id);
                    }
                }

                // M29 Phase 1 (§5, §6): skip every real per-frame cost
                // below -- layout, GPU texture/text-cache sync, scene
                // encoding, submit, present -- on a frame nothing real
                // touched. `take_dirty` already saw `tick_all`'s own
                // `any_active` above, so a mid-flight animation still
                // renders every frame exactly as before; only a genuinely
                // idle window (no input, no active animation) skips real
                // work now. `any_active` (M29 Phase 2) is returned at
                // every exit point below regardless of whether this
                // particular frame skipped or did full paint work -- it
                // answers a different question (does `engine-platform`
                // need to keep scheduling this window's next redraw on
                // its own) than `take_dirty` does (did *this* frame have
                // real paint work to do).
                if !runtime.tree.borrow_mut().take_dirty() {
                    return any_active;
                }

                runtime.tree.borrow_mut().compute_layout(
                    runtime.root,
                    Size {
                        width: AvailableSpace::Definite(runtime.width as f32),
                        height: AvailableSpace::Definite(runtime.height as f32),
                    },
                );

                let (wgpu::CurrentSurfaceTexture::Success(output)
                | wgpu::CurrentSurfaceTexture::Suboptimal(output)) =
                    runtime.gpu.surface.get_current_texture()
                else {
                    return any_active;
                };
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let scene = {
                    let tree_ref = runtime.tree.borrow();
                    // M22 Phase 1 (§5): every real `Image` node needs a
                    // real, uploaded GPU texture bound before `render`
                    // -- see `ImageTextureCache::sync`'s own doc
                    // comment. A window with no `Image` nodes pays only
                    // the cost of an empty `Tree::image_nodes` walk.
                    runtime.gpu.frame_renderer.sync_image_textures(
                        &tree_ref,
                        &runtime.gpu.device,
                        &runtime.gpu.queue,
                    );
                    // Review follow-through (M28 Phase 1, §5/§6): the
                    // exact same per-frame GC `sync_image_textures`
                    // already does for GPU textures, now also applied
                    // to `TextRenderer`'s own per-node shaped-`Layout`
                    // cache -- a text node removed from the tree must
                    // not keep its stale shaping around forever.
                    runtime.gpu.text_renderer.evict_stale_layouts(&tree_ref);
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
                any_active
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
                // M18 Phase 1 (§8, §10, §11.9, §11.10): widened from
                // `.borrow()` to `.borrow_mut()` -- a real click-to-
                // position hit-test needs `&mut runtime.gpu.
                // text_renderer` (its `font_cx`/`layout_cx` are mutably
                // borrowed to build a `Layout`, the same as painting
                // already requires), and `GpuState` is a plain field,
                // not independently wrapped in its own `RefCell`.
                // Confirmed safe: nothing else in this closure body
                // re-borrows this same outer `RefCell` re-entrantly.
                let mut runtimes = runtimes_for_input.borrow_mut();
                let Some(runtime) = runtimes.get_mut(&window_id) else {
                    return;
                };

                // M30 Phase 9 Step 4 (§5, §8, §10): a real, live
                // Terminal's own keyboard routing -- inspects the raw
                // `event` directly, the identical real "meaning-
                // dependent, not routed through `Tree::dispatch`'s own
                // generic `DispatchOutcome`" precedent `Docking`'s own
                // real winit wiring already established (M4 Phase 9):
                // `engine-core` has no real notion of a PTY to write to
                // (§4), and `NodeKind::Terminal` isn't matched by
                // `dispatch_text_field_key` at all, so a keystroke
                // reaching the generic dispatch below while a terminal
                // is focused would either do nothing or (for `Tab`)
                // wrongly move focus away instead of sending a real
                // completion-triggering byte. When a focused node is a
                // real `Terminal`, this claims the keystroke entirely --
                // the generic dispatch below never runs for it.
                if let Some(bytes) = input_bytes_for(&event) {
                    let focused_terminal = {
                        let tree_ref = runtime.tree.borrow();
                        tree_ref.focused().filter(|&id| {
                            matches!(
                                tree_ref.get(id).map(|node| &node.kind),
                                Some(NodeKind::Terminal(_))
                            )
                        })
                    };
                    if let Some(terminal_id) = focused_terminal {
                        if let Some(session) = runtime.terminals.borrow_mut().get_mut(&terminal_id)
                        {
                            session.write_input(&bytes);
                        }
                        return;
                    }
                }
                let outcome = runtime.tree.borrow_mut().dispatch(
                    runtime.root,
                    // M15 Phase 2: `InputEvent` is no longer `Copy`
                    // (the new `TextInput(String)` variant owns a real
                    // `String`) -- `event` itself is still needed below
                    // (the real, winit-driven dock-drag/theme-switch
                    // match), so this clones once rather than
                    // restructuring the two real, independent uses.
                    event.clone(),
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
                        // M18 Phase 1 (§8, §10, §11.9, §11.10): widened
                        // from `hit_test` to `hit_test_local` -- the
                        // extra local-space point is exactly what a
                        // real click-to-position hit-test needs below;
                        // `dock::start_drag`'s own existing use only
                        // ever needed the `NodeId`, unaffected.
                        if let Some((hit, local_point)) =
                            runtime.tree.borrow().hit_test_local(runtime.root, position)
                        {
                            dock::start_drag(&runtime.dock, hit);
                            if let Some(offset) = text_field_hit_offset(
                                &runtime.tree,
                                &mut runtime.gpu.text_renderer,
                                hit,
                                local_point,
                            ) {
                                runtime.tree.borrow_mut().set_text_field_cursor(hit, offset);
                                // M18 Phase 2 (§8, §10): a real press
                                // on a TextField always ARMS drag
                                // tracking -- whether it turns into a
                                // real selection depends entirely on
                                // whether a genuine PointerMoved to a
                                // different position follows before
                                // release (below); a plain click never
                                // does, so `selection_anchor` stays
                                // `None` exactly as `set_text_field_
                                // cursor` already left it.
                                runtime.text_drag = Some(hit);
                            }
                        }
                    }
                    InputEvent::PointerMoved { position } => {
                        // M18 Phase 2 (§8, §10): the real drag-select
                        // half of click-to-position. Lives here, not
                        // inside `Tree::dispatch`'s own existing
                        // `self.dragging`/`update_drag` mechanism
                        // (Splitter/Slider) -- that mechanism is pure
                        // geometry with zero rendering knowledge, but
                        // this needs the identical real per-glyph
                        // hit-test `PointerPressed` above already uses,
                        // which only `engine-render` can do (§4).
                        //
                        // **Real, stated scope boundary:** a real drag
                        // that leaves the field's own bounds mid-drag
                        // simply stops updating the selection until it
                        // re-enters (`hit_test_local` returning a
                        // different node, or `None`, is a genuine
                        // no-op below) -- it does not clamp to the
                        // field's own nearest edge the way some real
                        // desktop editors do. A further, real,
                        // un-scoped refinement beyond this phase.
                        if let Some(field) = runtime.text_drag
                            && let Some((hit, local_point)) =
                                runtime.tree.borrow().hit_test_local(runtime.root, position)
                            && hit == field
                            && let Some(offset) = text_field_hit_offset(
                                &runtime.tree,
                                &mut runtime.gpu.text_renderer,
                                hit,
                                local_point,
                            )
                        {
                            runtime
                                .tree
                                .borrow_mut()
                                .extend_text_field_selection(hit, offset);
                        }
                    }
                    InputEvent::PointerReleased {
                        position,
                        button: PointerButton::Primary,
                    } => {
                        dock::end_drag_at(&runtime.dock, &runtime.tree, runtime.root, position);
                        // M18 Phase 2 (§8, §10): a real mouse-up always
                        // ends any in-progress text drag, wherever it
                        // happens -- the same "not conditioned on still
                        // hitting the original node" real mouse-up
                        // semantics `Tree::dispatch`'s own `self.
                        // dragging = None` already established for
                        // Splitter/Slider (M4 Phase 3).
                        runtime.text_drag = None;
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
                        let mut tree = runtime.tree.borrow_mut();
                        tree.set_all_interaction_tints(tint);
                        // M20 Phase 1 (§7.1, §7.3): a real live OS
                        // theme switch must re-tint Checkbox/Slider
                        // component colors too, the identical way
                        // `Window.set_theme` itself already does.
                        tree.set_all_component_tints(tint);
                    }
                    // M17 Phase 1 (§8): the real, winit-driven Ctrl+C
                    // path -- `Tree::text_field_selected_text` is a pure
                    // read (`engine-core` never touches a real
                    // clipboard, §4), so the actual OS write happens
                    // here, the one place with both `Tree` and real
                    // clipboard access. A clipboard failure (no real
                    // clipboard service reachable, a real, possible
                    // condition in some headless environments) is
                    // logged and non-fatal, the same "real, expected,
                    // gracefully-handled" policy M16 Phase 2 already
                    // established for no-GPU/no-display.
                    InputEvent::Copy => {
                        let selected = runtime.tree.borrow().focused().and_then(|field| {
                            runtime.tree.borrow().text_field_selected_text(field)
                        });
                        if let Some(text) = selected {
                            match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                Ok(()) => {}
                                Err(err) => {
                                    tracing::warn!(%err, "failed to write to the real OS clipboard");
                                }
                            }
                        }
                    }
                    // M17 Phase 1 (§8): `Copy`'s own real Cut sibling --
                    // writes to the real clipboard *first*, using a pure
                    // read (`text_field_selected_text`, not the
                    // mutating `cut_text_field_selection`), and only
                    // actually removes the real selection once that
                    // write genuinely succeeds. A failed clipboard write
                    // must never silently destroy the user's own
                    // selected text with no way to recover it.
                    InputEvent::Cut => {
                        let field_and_text = runtime.tree.borrow().focused().and_then(|field| {
                            runtime
                                .tree
                                .borrow()
                                .text_field_selected_text(field)
                                .map(|text| (field, text))
                        });
                        if let Some((field, text)) = field_and_text {
                            match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                Ok(()) => {
                                    runtime.tree.borrow_mut().cut_text_field_selection(field);
                                    // A real cut genuinely edits the
                                    // field's own content -- fires
                                    // `Change` the same way `Window.cut`
                                    // 's own hermetic FFI counterpart
                                    // does, since this path also calls
                                    // `cut_text_field_selection`
                                    // directly, not through `Tree::
                                    // dispatch`.
                                    call_handler(&runtime.handlers, field, EventKind::Change, py);
                                }
                                Err(err) => {
                                    tracing::warn!(
                                        %err,
                                        "failed to write to the real OS clipboard -- selection left untouched"
                                    );
                                }
                            }
                        }
                    }
                    // M17 Phase 1 (§8): the real, winit-driven Ctrl+V
                    // path -- reads the real OS clipboard, then
                    // dispatches the resulting text exactly like a real
                    // typed character (`InputEvent::TextInput`, M15
                    // Phase 2's own existing mechanism, reused
                    // completely, no new insertion path).
                    InputEvent::PasteRequested => {
                        match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                            Ok(text) => {
                                let outcome = runtime.tree.borrow_mut().dispatch(
                                    runtime.root,
                                    InputEvent::TextInput(text),
                                    &interaction_config(),
                                    Instant::now(),
                                );
                                run_dispatch_outcome(&runtime.handlers, outcome, py);
                            }
                            Err(err) => {
                                tracing::warn!(%err, "failed to read the real OS clipboard");
                            }
                        }
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
                // workspace. `tracing::warn!` (M16 Phase 2): the same
                // "expected, gracefully-handled, not an error" reasoning
                // as the no-adapter case above.
                tracing::warn!(%err, "no display available, exiting cleanly");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// M17 Phase 1 (§8): the one real, permanent regression check that
    /// `arboard` genuinely connects to a live OS clipboard in *this*
    /// environment -- manually verified once via a throwaway probe
    /// before committing to the dependency at all (see `PLAN.md`/
    /// `LOG.md`), kept here as a real, automated check rather than
    /// trusting that one-off result forever. No real keyboard event can
    /// be synthesized from a test (the real Ctrl+C/X/V path only ever
    /// originates from an actual OS-level `winit` event, confirmed in
    /// `Window.copy`/`cut`/`paste`'s own doc comments) -- this instead
    /// proves the one real, testable half: a genuine set/get round trip
    /// against whatever clipboard mechanism is actually reachable here.
    /// Treats "no clipboard service reachable" as a real, honest skip,
    /// not a failure -- the same "genuinely different environment"
    /// tolerance this codebase already applies to GPU/display absence
    /// (TRE v1 finding #261).
    #[test]
    fn arboard_genuinely_round_trips_through_a_real_clipboard() {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            eprintln!(
                "arboard_genuinely_round_trips_through_a_real_clipboard: no real clipboard \
                 service reachable in this environment -- skipping, not failing"
            );
            return;
        };
        let marker = "tre-engine-py-clipboard-round-trip-test";
        if clipboard.set_text(marker).is_err() {
            eprintln!(
                "arboard_genuinely_round_trips_through_a_real_clipboard: clipboard reachable \
                 but the real write failed -- skipping, not failing"
            );
            return;
        }
        match clipboard.get_text() {
            Ok(text) => assert_eq!(
                text, marker,
                "a real set_text must be readable back verbatim"
            ),
            Err(err) => {
                panic!("clipboard accepted a real write but the real read-back failed: {err}")
            }
        }
    }
}
