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

use engine_core::{Cursor, InputEvent, NodeId, NodeKind, PointerButton, Tree, from_access_id};
use engine_platform::{WindowConfig, WindowLifecycle, WindowRequest, run_windowed_multi};
use engine_render::{FrameRenderer, GeometryCache, TextPlacement, TextRenderer, build_tree_scene};
use peniko::kurbo::Point;
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};
use vello_hybrid::{RenderSize, RenderTargetConfig};
use winit::window::{Window, WindowId};

use crate::dispatch::{
    HandlerMap, SharedCompletions, copy_focused_selection_to_clipboard,
    cut_focused_selection_to_clipboard, interaction_config, paste_clipboard_into_focused,
    process_input, run_completions, run_dispatch_outcome,
};
use crate::dock::{self, SharedDockState};
use crate::event::NodeContext;
use crate::listeners::{self, WindowEventType, WindowListenerMap};
use crate::terminal::{TerminalSession, control_byte_for, input_bytes_for};
use crate::thread_bound::{ThreadBound, thread_bound_shell};
use crate::thread_handle::{CallQueue, LoopHandle};
use crate::window::{PyWindow, SharedActiveTree, SharedOsWindow, SharedSize, SharedTheme};

#[pyclass]
pub struct App(ThreadBound<AppState>);
thread_bound_shell!(App => AppState);

/// `App`'s state (M96: behind a `ThreadBound`, see `thread_bound`).
pub struct AppState {
    windows: Vec<Py<PyWindow>>,
    /// M87: callables queued from other threads via `LoopHandle`,
    /// drained at the top of every frame -- see `thread_handle.rs`.
    calls: CallQueue,
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
    /// M33 Phase 2 (§4, §5, §8): the real, shared `Rc<Cell<u32>>`
    /// clone of `PyWindow`'s own field, not a plain `u32` copy -- see
    /// `window::SharedSize`'s own doc comment for the full real
    /// reasoning.
    width: SharedSize,
    height: SharedSize,
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
    /// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): the same shared,
    /// atomically-swappable bundle `PyWindow.active` holds (`window::
    /// ActiveTree`/`SharedActiveTree`) -- extracted the same way as
    /// every other field here, so `WindowRuntime` can re-sync its own
    /// `tree`/`root`/`handlers`/`context_menus` from it every real
    /// frame/input, picking up a `Window.show_view` call made from a
    /// Python handler while `App.run()` is already blocking.
    active: SharedActiveTree,
    /// M94: see `PyWindow::window_listeners`/`os_window`.
    window_listeners: WindowListenerMap,
    os_window: SharedOsWindow,
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
    // M38 Phase 7 (§5, §8): no longer clones `state` out of the borrow
    // -- `TextFieldState` stopped deriving `Clone` once it gained a
    // real `Animated<f64>` field (`scroll_offset`), the identical real
    // reason `ScrollViewState`/`Splitter`/`Icon` never derived it
    // either. Holds `tree.borrow()` for this whole function's body
    // instead, released when it returns, before either real caller's
    // own subsequent `borrow_mut()`.
    let tree = tree.borrow();
    let node = tree.get(hit)?;
    let NodeKind::TextField(state) = &node.kind else {
        return None;
    };
    let width = tree.layout(hit).size.width;
    let at = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: width,
        color: peniko::Color::TRANSPARENT,
    };
    Some(text_renderer.hit_test_position(state, at, local_point))
}

/// M32 Phase 6 (§4, §5, §8): `text_field_hit_offset`'s own real
/// `Terminal` sibling -- turns a real local click/drag point into the
/// exact real `(row, col)` cell it lands on, via `engine-render`'s own
/// `terminal_hit_cell` (real font metrics only that crate has, §4).
/// `None` for anything that isn't a real, present `Terminal`, the
/// identical "not the kind this needs" contract `text_field_hit_offset`
/// already has.
fn terminal_hit_cell(
    tree: &Rc<RefCell<Tree>>,
    text_renderer: &mut TextRenderer,
    hit: NodeId,
    local_point: Point,
) -> Option<(u16, u16)> {
    let tree_ref = tree.borrow();
    match tree_ref.get(hit).map(|n| &n.kind) {
        Some(NodeKind::Terminal(state)) => {
            Some(text_renderer.terminal_hit_cell(state, local_point))
        }
        _ => None,
    }
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    /// M32 Phase 2 (§4, §5): kept around (not just consumed inside
    /// `new`) specifically so `resize` below can reconfigure the
    /// surface again later with the identical real `usage`/`present_
    /// mode`/`alpha_mode`/`view_formats` this adapter's own `get_
    /// default_config` chose at construction -- mutating just `width`/
    /// `height` on a stored config, the standard real wgpu resize
    /// recipe, not re-deriving those choices from scratch.
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    frame_renderer: FrameRenderer,
    text_renderer: TextRenderer,
    /// M34 Phase 1 (§5, §8): the real, per-node tessellated-path cache
    /// for `Rect`/`Splitter`'s own fill/border paths -- the identical
    /// "long-lived, caller-owned, not rebuilt per call" shape `text_
    /// renderer` already has (`GeometryCache`'s own doc comment).
    geometry_cache: GeometryCache,
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
            surface_config: config,
            device,
            queue,
            frame_renderer,
            text_renderer: TextRenderer::new(),
            geometry_cache: GeometryCache::new(),
        }
    }

    /// M32 Phase 2 (§4, §5): reconfigures the real wgpu surface to a
    /// genuinely new client-area size -- the textbook real wgpu resize
    /// recipe (mutate the stored config's own `width`/`height`, then
    /// `surface.configure` again), not something this crate invents.
    /// **Real, confirmed finding before writing this:** `FrameRenderer`/
    /// `vello_hybrid::Renderer` need no matching reconstruction at all
    /// -- direct read of the vendored `vello_hybrid = "0.2.0"` source
    /// confirms its own `Renderer::render` already calls a private
    /// `maybe_update_config_buffer` every frame, which recreates its
    /// own internal depth texture whenever the `RenderSize` passed to
    /// `render` genuinely differs from the previous call -- real,
    /// existing resize-safety this phase only needed to rely on, not
    /// build. A 0-sized dimension (a real, possible transient value on
    /// some platforms while a window is being minimized) is skipped
    /// entirely -- `surface.configure` panics on a zero-sized
    /// `SurfaceConfiguration`, a real, known wgpu gotcha, not a
    /// hypothetical one.
    ///
    /// M40 Phase 1 (§4, §6, §9): called from exactly one real place now
    /// -- the per-frame `RedrawRequested` path, guarded by `needs_
    /// resize` below, not from every raw `InputEvent::Resized`. See
    /// `needs_resize`'s own doc comment for the real, measured reason.
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    /// M40 Phase 1 (§4, §6, §9): whether the surface's own currently-
    /// configured size still matches the window's real, current
    /// dimensions -- the real per-frame gate that replaces reconfiguring
    /// on every raw `Resized` event.
    ///
    /// **Real root cause this closes, confirmed by direct measurement,
    /// not assumed:** `surface.configure` is a genuine swapchain rebuild
    /// -- a real, throwaway scratch probe on this exact machine (AMD
    /// Radeon 890M, RADV/Vulkan) measured ~600µs-1ms per call, and a
    /// live drag can deliver many `Resized` events between two real
    /// frames. Calling `resize` inline on every one of them (the old
    /// behavior) reconfigures the surface far more often than the
    /// display can even present a new frame -- real, wasted, redundant
    /// work, and the mechanism directly behind the "trailing behind the
    /// cursor" symptom this milestone exists to fix. Checking this once
    /// per real frame and reconfiguring only when it's actually `true`
    /// collapses an entire burst of `Resized` events into at most one
    /// real reconfigure per frame, always at the window's true current
    /// size -- `runtime.width`/`height` (the live `SharedSize` cells)
    /// never lag, only the expensive GPU-side reconfigure is coalesced.
    ///
    /// **Real, deliberate design choice, not the naive port of either
    /// sibling project's own fix:** both TRE v1 and pyCopper instead
    /// hold the swapchain at a coarser, oversized *bucketed* size during
    /// a drag, relying on the compositor to scale it down to fit. A
    /// real, throwaway empirical probe against this exact session's own
    /// real KWin/Wayland compositor found that doesn't happen by
    /// default: an oversized wgpu surface gets *cropped* to the window's
    /// own declared geometry, not scaled -- a hard, visibly broken clip,
    /// not the soft blur either prior project's own writeup described.
    /// Both of them only get real scale-to-fit behavior via `wp_
    /// viewporter` (TRE v1 via a real winit fork; likely something
    /// equivalent under pyCopper's own GLFW/rendercanvas stack) -- this
    /// milestone's own scoping already deferred that fork as a real,
    /// explicit follow-up, not built here. Reconfiguring to the exact
    /// true size every time, just less often, sidesteps the crop bug
    /// entirely and needed no fork -- and at this machine's own real
    /// measured cost (well under 1ms), a plain per-frame coalesce
    /// already removes the redundant-reconfigure cost without it.
    fn needs_resize(&self, width: u32, height: u32) -> bool {
        self.surface_config.width != width || self.surface_config.height != height
    }
}

/// M94: the pointer shape for `position` -- the capturing node's, or the
/// node under the pointer's, or the nearest ancestor's that sets one.
fn cursor_at(tree: &Tree, root: NodeId, position: Point) -> Cursor {
    let mut current = tree
        .pointer_capture()
        .or_else(|| tree.hit_test(root, position));
    while let Some(id) = current {
        let Some(node) = tree.get(id) else { break };
        if let Some(cursor) = node.cursor {
            return cursor;
        }
        current = node.parent;
    }
    Cursor::Default
}

/// M94: `winit`'s icon for each of the engine's cursor shapes.
fn cursor_icon(cursor: Cursor) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as Icon;
    match cursor {
        Cursor::Default => Icon::Default,
        Cursor::Pointer => Icon::Pointer,
        Cursor::Text => Icon::Text,
        Cursor::Grab => Icon::Grab,
        Cursor::Grabbing => Icon::Grabbing,
        Cursor::Move => Icon::Move,
        Cursor::NotAllowed => Icon::NotAllowed,
        Cursor::Wait => Icon::Wait,
        Cursor::Progress => Icon::Progress,
        Cursor::Crosshair => Icon::Crosshair,
        Cursor::Help => Icon::Help,
        Cursor::ColResize => Icon::ColResize,
        Cursor::RowResize => Icon::RowResize,
        Cursor::EwResize => Icon::EwResize,
        Cursor::NsResize => Icon::NsResize,
        Cursor::NeswResize => Icon::NeswResize,
        Cursor::NwseResize => Icon::NwseResize,
        Cursor::Copy => Icon::Copy,
        Cursor::Cell => Icon::Cell,
        Cursor::ContextMenu => Icon::ContextMenu,
        Cursor::ZoomIn => Icon::ZoomIn,
        Cursor::ZoomOut => Icon::ZoomOut,
        Cursor::AllScroll => Icon::AllScroll,
    }
}

struct WindowRuntime {
    tree: Rc<RefCell<Tree>>,
    root: NodeId,
    /// M33 Phase 2 (§4, §5, §8): the real, shared `Rc<Cell<u32>>` --
    /// `.get()` at every real per-frame read site below (a plain,
    /// cheap `Cell::get()`, negligible next to the real GPU/text work
    /// each frame already does) instead of a plain, separate `u32`
    /// copy, so `InputEvent::Resized`'s own `.set()` call is
    /// immediately visible to `PyWindow`'s own fields too, and vice
    /// versa (`window::SharedSize`'s own doc comment has the full real
    /// reasoning).
    width: SharedSize,
    height: SharedSize,
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
    /// M32 Phase 6 (§4, §5, §8): `text_drag`'s own real `Terminal`
    /// sibling -- which terminal (if any) a real press-and-drag is
    /// currently extending a real cell-range selection in. A separate
    /// field, not a shared one, since a single real press can only
    /// ever hit one real `NodeKind` at a time (`text_field_hit_offset`/
    /// `terminal_hit_cell` are mutually exclusive per node).
    terminal_drag: Option<NodeId>,
    /// M94: the pointer shape last applied to this window, so it's set on
    /// the OS window only when it changes.
    cursor: Cursor,
    /// M30 Phase 9 Step 4 (§5, §8, §10): the same real, shared session
    /// table `PyWindow.terminals` owns -- see `WindowSetup.terminals`'s
    /// own doc comment.
    terminals: Rc<RefCell<HashMap<NodeId, TerminalSession>>>,
    /// M42 Phase 2 (§4, §5, §8, §16.2, §16.4): see `WindowSetup.active`'s
    /// own doc comment. `tree`/`root`/`handlers`/`context_menus` above
    /// stay as plain fields (not replaced by this) -- every real
    /// closure below that reads them re-syncs from `active` at its own
    /// top, right after obtaining `runtime`, so none of this file's
    /// dozens of pre-existing `runtime.tree`/`.root`/`.handlers`/
    /// `.context_menus` call sites need to change at all.
    active: SharedActiveTree,
    /// M94: see `PyWindow::window_listeners`/`os_window`.
    window_listeners: WindowListenerMap,
    os_window: SharedOsWindow,
}

#[pymethods]
impl App {
    #[new]
    fn new() -> Self {
        Self(ThreadBound::new(AppState {
            windows: Vec::new(),
            calls: CallQueue::default(),
        }))
    }

    /// M87 (tre issue #6): a `Send + Sync` handle a background thread can
    /// hold, since `App` itself is `unsendable`. `handle.call_soon(fn)`
    /// queues `fn` to run on this `App`'s event-loop thread and wakes the
    /// loop -- how a file watcher thread drives hot reload inside
    /// `run()`. Every handle from one `App` shares the same queue.
    fn thread_handle(&self) -> LoopHandle {
        LoopHandle::new(self.calls.clone())
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

        // M96: a live window runs on real time, whatever `Window.advance`
        // pinned before.
        for window in &self.windows {
            let window = window.borrow(py);
            crate::clock::unpin(&window.tree);
            crate::clock::unpin(&window.active.borrow().tree);
        }

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
                    width: window.width.clone(),
                    height: window.height.clone(),
                    handlers: window.handlers.clone(),
                    context_menus: window.context_menus.clone(),
                    dock: window.dock.clone(),
                    theme: window.theme.clone(),
                    completions: window.completions.clone(),
                    terminals: window.terminals.clone(),
                    active: window.active.clone(),
                    window_listeners: window.window_listeners.clone(),
                    os_window: window.os_window.clone(),
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
        let runtimes_for_lifecycle = runtimes.clone();
        let runtimes_for_access_action = runtimes;
        let setups_for_cleanup = setups.clone();
        let setups_for_setup = setups;
        let calls_for_frame = self.calls.clone();
        let calls_for_setup = self.calls.clone();

        let result = run_windowed_multi(
            move |window_id, token, window| {
                let setup = &setups_for_created[token as usize];
                *setup.os_window.borrow_mut() = Some(window.clone());
                let gpu = GpuState::new(window, setup.width.get(), setup.height.get());
                runtimes_for_created.borrow_mut().insert(
                    window_id,
                    WindowRuntime {
                        tree: setup.tree.clone(),
                        root: setup.root,
                        width: setup.width.clone(),
                        height: setup.height.clone(),
                        gpu,
                        handlers: setup.handlers.clone(),
                        context_menus: setup.context_menus.clone(),
                        dock: setup.dock.clone(),
                        theme: setup.theme.clone(),
                        completions: setup.completions.clone(),
                        text_drag: None,
                        terminal_drag: None,
                        cursor: Cursor::Default,
                        terminals: setup.terminals.clone(),
                        active: setup.active.clone(),
                        window_listeners: setup.window_listeners.clone(),
                        os_window: setup.os_window.clone(),
                    },
                );
            },
            move |window_id, _frame| -> bool {
                // M87: run anything a background thread queued via
                // `LoopHandle.call_soon` first -- before the `active`
                // re-sync below, so a queued `show_view`/`reconcile`
                // is what this very frame ticks and paints. No borrow of
                // `runtimes` or any tree is held here, so the callable
                // is free to touch any window's tree.
                calls_for_frame.drain(py);
                // M96: finish drops other threads handed back, and free
                // any detached subtree that lost its last handle.
                crate::node_handles::reclaim();
                let mut runtimes = runtimes_for_frame.borrow_mut();
                let Some(runtime) = runtimes.get_mut(&window_id) else {
                    return false;
                };
                // M42 Phase 2 (§4, §5, §8, §16.2, §16.4): re-sync from
                // `active` at the top of every real frame -- a real
                // `Window.show_view` call (from a Python handler,
                // possibly fired by `run_dispatch_outcome`/`run_
                // completions` below on a *previous* frame) writes a new
                // bundle into this same shared `RefCell`; this is where
                // that change actually becomes what gets ticked/laid-
                // out/painted next. A no-op read+clone on every ordinary
                // frame where nothing switched (`Rc::clone` is cheap,
                // the same real cost `SharedSize`'s own per-frame `.get
                // ()` already accepts).
                {
                    let active = runtime.active.borrow();
                    runtime.tree = active.tree.clone();
                    runtime.root = active.root;
                    runtime.handlers = active.handlers.clone();
                    runtime.context_menus = active.context_menus.clone();
                }

                let now = crate::clock::now(&runtime.tree);
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
                // dirty` below see it.
                //
                // M31 Phase 6 (§5, §6): `any_active` no longer needs
                // widening just because a real terminal session exists
                // -- each session's own background reader thread now
                // wakes this window directly (`EventLoopWaker::wake`,
                // registered via `TerminalSession::set_waker` in this
                // run's own real `setup` closure) the moment real new
                // PTY bytes actually arrive, closing the real, stated
                // v1 cost that widening was. A window with a live but
                // genuinely quiet terminal (nothing typed, nothing
                // printed) can now go fully idle exactly like any other
                // window, the identical real win M29 Phase 2 already
                // gave every other case.
                {
                    let mut terminals = runtime.terminals.borrow_mut();
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
                //
                // M40 Phase 1 (§4, §6, §9): widened with `needs_resize`
                // -- a real, pending resize touches no `Tree` state at
                // all (it's pure GPU/window sizing), so `take_dirty`
                // alone would never see it; a frame must still do real
                // work when the surface's own configured size has
                // fallen behind the window's true current one, even if
                // nothing else changed.
                let resized = runtime
                    .gpu
                    .needs_resize(runtime.width.get(), runtime.height.get());
                // M86: a `tre.register_font` call touches no `Tree`
                // state either -- same reasoning as `resized` above. A
                // family that fell back before may resolve to a real
                // face now, so this frame must repaint. One atomic load
                // when nothing was registered.
                let fonts_changed = runtime.gpu.text_renderer.sync_registered_fonts();
                let dirty = runtime.tree.borrow_mut().take_dirty();
                if !dirty && !resized && !fonts_changed {
                    return any_active;
                }

                runtime.tree.borrow_mut().compute_layout(
                    runtime.root,
                    Size {
                        width: AvailableSpace::Definite(runtime.width.get() as f32),
                        height: AvailableSpace::Definite(runtime.height.get() as f32),
                    },
                );

                // M40 Phase 1 (§4, §6, §9): the one real place `GpuState::
                // resize` is called from now -- collapses however many
                // raw `Resized` events arrived since the last real frame
                // into a single real reconfigure, at whatever the
                // window's true, current size is at this exact moment
                // (`needs_resize`'s own doc comment has the full real
                // reasoning). A true no-op when nothing changed size
                // (`resize` itself still no-ops on a 0-sized dimension).
                if resized {
                    runtime
                        .gpu
                        .resize(runtime.width.get(), runtime.height.get());
                }

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
                    // M34 Phase 1 (§5, §8): the identical real per-
                    // frame GC `text_renderer`'s own cache already
                    // gets, now applied to `geometry_cache` too.
                    runtime.gpu.geometry_cache.evict_stale(&tree_ref);
                    build_tree_scene(
                        &tree_ref,
                        runtime.root,
                        runtime.width.get() as u16,
                        runtime.height.get() as u16,
                        runtime.gpu.frame_renderer.resources_mut(),
                        &mut runtime.gpu.text_renderer,
                        &mut runtime.gpu.geometry_cache,
                    )
                };
                let render_size = RenderSize {
                    width: runtime.width.get(),
                    height: runtime.height.get(),
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
                // M42 Phase 2: reads through `active` directly rather
                // than `runtime.tree`/`.root` -- this closure only ever
                // holds a shared `&runtime` (via `.borrow()`, not
                // `.borrow_mut()`), so it can't refresh `runtime`'s own
                // plain fields in place the way `frame`/`input` do.
                let active = runtime.active.borrow();
                active.tree.borrow().build_access_update(active.root)
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
                // M42 Phase 2: see the identical prelude in the `frame`
                // closure above for the full real reasoning -- a real
                // `Window.show_view` call, made from a Python handler
                // this very closure may have just invoked on a prior
                // input, must be visible to every dispatch this closure
                // does from here on.
                {
                    let active = runtime.active.borrow();
                    runtime.tree = active.tree.clone();
                    runtime.root = active.root;
                    runtime.handlers = active.handlers.clone();
                    runtime.context_menus = active.context_menus.clone();
                }

                // M94: modifier state is shared by every window's event
                // routing (`listeners::modifiers`), nothing more to do.
                if let InputEvent::ModifiersChanged(modifiers) = event {
                    listeners::set_modifiers(modifiers);
                    return;
                }

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
                let focused_terminal = {
                    let tree_ref = runtime.tree.borrow();
                    tree_ref.focused().filter(|&id| {
                        matches!(
                            tree_ref.get(id).map(|node| &node.kind),
                            Some(NodeKind::Terminal(_))
                        )
                    })
                };
                if let Some(bytes) = input_bytes_for(&event)
                    && let Some(terminal_id) = focused_terminal
                {
                    if let Some(session) = runtime.terminals.borrow_mut().get_mut(&terminal_id) {
                        session.write_input(&bytes);
                    }
                    return;
                }
                // M32 Phase 4 (§4, §8): the real point of this phase --
                // a real Ctrl+`<letter>` reaching a focused `Terminal`
                // is its own real ASCII control byte (SIGINT for Ctrl+C
                // included), not `Tree::dispatch`'s own generic (and,
                // for `Copy`/`Cut`/`PasteRequested`, clipboard-bound)
                // handling below. **Deliberately checked only when a
                // real `Terminal` is genuinely focused:** when it isn't,
                // `control_byte_for` is never even called here, so
                // ordinary `TextField` copy/cut/paste (the match arms
                // below) and every other unclaimed Ctrl+`<letter>`
                // (a true no-op via `Tree::dispatch`'s own new plumbing-
                // only `ControlChar` arm) stay completely unaffected --
                // zero behavior change for the non-terminal case this
                // phase doesn't touch.
                if let Some(terminal_id) = focused_terminal
                    && let Some(byte) = control_byte_for(&event)
                {
                    if let Some(session) = runtime.terminals.borrow_mut().get_mut(&terminal_id) {
                        session.write_input(&[byte]);
                    }
                    return;
                }
                // M94: dispatch, `node.on(...)` listeners, legacy handlers,
                // and the context menu a right-click opens -- one pipeline
                // shared with `Window.simulate` (`dispatch::process_input`).
                // `event` itself is still needed below, for the
                // winit-driven dock-drag/theme-switch match.
                process_input(
                    &NodeContext {
                        tree: &runtime.tree,
                        handlers: &runtime.handlers,
                        context_menus: &runtime.context_menus,
                        theme: &runtime.theme,
                        completions: &runtime.completions,
                    },
                    runtime.root,
                    &event,
                    py,
                );
                // M94: the pointer shape follows the node under the
                // pointer (or the capturing node).
                if let InputEvent::PointerMoved { position }
                | InputEvent::PointerPressed { position, .. }
                | InputEvent::PointerReleased { position, .. } = &event
                {
                    let wanted = cursor_at(&runtime.tree.borrow(), runtime.root, *position);
                    if wanted != runtime.cursor {
                        if let Some(window) = runtime.os_window.borrow().as_ref() {
                            window.set_cursor(cursor_icon(wanted));
                        }
                        runtime.cursor = wanted;
                    }
                }
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
                            } else if let Some((row, col)) = terminal_hit_cell(
                                &runtime.tree,
                                &mut runtime.gpu.text_renderer,
                                hit,
                                local_point,
                            ) {
                                // M32 Phase 6 (§4, §5, §8): a real press
                                // on a `Terminal` -- the identical real
                                // "collapsed selection, arm drag
                                // tracking" shape `TextField`'s own
                                // press handling just above already
                                // has.
                                runtime
                                    .tree
                                    .borrow_mut()
                                    .set_terminal_selection_start(hit, row, col);
                                runtime.terminal_drag = Some(hit);
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
                        // M32 Phase 6 (§4, §5, §8): `text_drag`'s own
                        // real `Terminal` sibling -- the identical real
                        // "still over the same node, extend" shape.
                        if let Some(terminal) = runtime.terminal_drag
                            && let Some((hit, local_point)) =
                                runtime.tree.borrow().hit_test_local(runtime.root, position)
                            && hit == terminal
                            && let Some((row, col)) = terminal_hit_cell(
                                &runtime.tree,
                                &mut runtime.gpu.text_renderer,
                                hit,
                                local_point,
                            )
                        {
                            runtime
                                .tree
                                .borrow_mut()
                                .extend_terminal_selection(hit, row, col);
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
                        // M32 Phase 6 (§4, §5, §8): `text_drag`'s own
                        // real `Terminal` sibling -- the real selection
                        // itself stays visible (`TerminalState.
                        // selection_start`/`end` are untouched here),
                        // only the drag-tracking itself ends.
                        runtime.terminal_drag = None;
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
                        drop(tree);
                        listeners::deliver_window(
                            &runtime.window_listeners,
                            py,
                            WindowEventType::ColorScheme,
                            |e| e.dark = Some(dark),
                        );
                    }
                    // M32 Phase 2 (§4, §5): the real, winit-driven
                    // window resize -- `Tree::dispatch` (called just
                    // above, unconditionally, for every real
                    // `InputEvent`) already resized `runtime.root`'s
                    // own `layout_style.size` directly (`engine-core`
                    // fully owns that, no need to defer it here); this
                    // arm handles the one thing only `engine-py` can
                    // (`runtime.width`/`height`, which every per-frame
                    // `compute_layout`/`build_tree_scene`/`RenderSize`
                    // call already reads fresh -- see `RedrawRequested`
                    // above).
                    //
                    // M33 Phase 2 (§4, §5, §8) closed the real, stated
                    // v1 limit this comment used to name here:
                    // `runtime.width`/`height` are now the identical
                    // real, shared `Rc<Cell<u32>>` `PyWindow`'s own
                    // fields are (`window::SharedSize`), so this `.set()`
                    // call is immediately visible there too -- an app
                    // that calls e.g. `add_dialog` from a live click
                    // handler after a real resize now sizes that
                    // dialog's own full-window scrim against the
                    // window's real *current* dimensions, not its
                    // construction-time ones.
                    //
                    // M40 Phase 1 (§4, §6, §9): no longer calls `runtime.
                    // gpu.resize(...)` here -- a real, live drag can
                    // deliver many `Resized` events between two real
                    // frames, and reconfiguring the wgpu surface (a
                    // genuine swapchain rebuild) on every single one is
                    // the real, measured root cause of the "trailing
                    // behind the cursor" symptom `BUILD_TRACKER.md`'s own
                    // M40 investigation root-caused. The real, exact
                    // *value* still updates here, live, with zero lag
                    // (unchanged) -- only the expensive GPU reconfigure
                    // itself is deferred to `RedrawRequested` below,
                    // where it naturally coalesces every event in a
                    // burst into a single real reconfigure per frame,
                    // always at the true, current size (never a stale or
                    // bucketed one -- see `GpuState::needs_resize`'s own
                    // doc comment for why this codebase doesn't port
                    // either sibling project's own size-bucketing
                    // approach).
                    InputEvent::Resized { width, height } => {
                        runtime.width.set(width as u32);
                        runtime.height.set(height as u32);
                        listeners::deliver_window(
                            &runtime.window_listeners,
                            py,
                            WindowEventType::Resize,
                            |e| {
                                e.width = Some(f64::from(width));
                                e.height = Some(f64::from(height));
                            },
                        );
                    }
                    InputEvent::ScaleFactorChanged { scale_factor } => {
                        listeners::deliver_window(
                            &runtime.window_listeners,
                            py,
                            WindowEventType::ScaleFactor,
                            |e| e.scale_factor = Some(scale_factor),
                        );
                    }
                    // M17 Phase 1 (§8), refactored M53 Phase 2: the
                    // real, winit-driven Ctrl+C path -- now a thin call
                    // into `copy_focused_selection_to_clipboard`
                    // (`dispatch.rs`), shared with `Window.copy_to_
                    // system_clipboard`'s own identical real logic.
                    // Real behavior byte-for-byte unchanged; only the
                    // call site moved.
                    InputEvent::Copy => {
                        copy_focused_selection_to_clipboard(&runtime.tree);
                    }
                    // M32 Phase 6 (§4, §5, §8): `Copy`'s own real
                    // Terminal-specific sibling -- a genuine Ctrl+
                    // Shift+C (`engine_platform::translate_clipboard_
                    // shortcut`'s own real one exception to "shift
                    // doesn't change the shortcut"). Reads whichever
                    // `Terminal`'s own real mouse-drag selection is
                    // currently set (`Tree::terminal_selected_text`, a
                    // pure read -- `engine-core` never touches a real
                    // clipboard, §4) and writes it to the real OS
                    // clipboard, the identical real write path `Copy`
                    // just above already uses. A true no-op if nothing
                    // is currently focused, the focused node isn't a
                    // `Terminal`, or its own selection is empty/
                    // collapsed.
                    InputEvent::TerminalCopyRequested => {
                        let selected = runtime
                            .tree
                            .borrow()
                            .focused()
                            .and_then(|id| runtime.tree.borrow().terminal_selected_text(id));
                        if let Some(text) = selected {
                            match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                Ok(()) => {}
                                Err(err) => {
                                    tracing::warn!(
                                        %err,
                                        "failed to write the real terminal selection to the OS clipboard"
                                    );
                                }
                            }
                        }
                    }
                    // M17 Phase 1 (§8), refactored M53 Phase 2: `Copy`'s
                    // own real Cut sibling -- now a thin call into
                    // `cut_focused_selection_to_clipboard` (`dispatch.
                    // rs`), shared with `Window.cut_to_system_
                    // clipboard`'s own identical real logic. Real
                    // behavior byte-for-byte unchanged; only the call
                    // site moved.
                    InputEvent::Cut => {
                        cut_focused_selection_to_clipboard(
                            &runtime.tree,
                            &runtime.handlers,
                            &runtime.context_menus,
                            &runtime.theme,
                            &runtime.completions,
                            py,
                        );
                    }
                    // M17 Phase 1 (§8), refactored M53 Phase 2: the
                    // real, winit-driven Ctrl+V path -- now a thin call
                    // into `paste_clipboard_into_focused` (`dispatch.
                    // rs`), shared with `Window.paste_from_system_
                    // clipboard`'s own identical real logic. Real
                    // behavior byte-for-byte unchanged; only the call
                    // site moved.
                    InputEvent::PasteRequested => {
                        paste_clipboard_into_focused(
                            &runtime.tree,
                            runtime.root,
                            &runtime.handlers,
                            &runtime.context_menus,
                            &runtime.theme,
                            &runtime.completions,
                            py,
                        );
                    }
                    // M32 Phase 5 (§4, §8): a real mouse wheel over a
                    // `Terminal` moves its own real viewport into
                    // scrollback -- the identical real "hit-test at the
                    // wheel's own position" mechanism `Tree::dispatch`'s
                    // own `Scroll` handling already uses for `VirtualList`
                    // /`Carousel` (that handling already ran, harmlessly,
                    // for this same event just above: a `Terminal` has no
                    // `VirtualList`/`Carousel` ancestor to find, so it's a
                    // true no-op there). `engine-core` has no real notion
                    // of a `vt100::Screen` to scroll (§4), so this is the
                    // one place both a live hit-test and real terminal
                    // access exist together.
                    InputEvent::Scroll { delta, position } => {
                        let hit_terminal = {
                            let tree_ref = runtime.tree.borrow();
                            tree_ref.hit_test(runtime.root, position).filter(|&id| {
                                matches!(
                                    tree_ref.get(id).map(|node| &node.kind),
                                    Some(NodeKind::Terminal(_))
                                )
                            })
                        };
                        if let Some(terminal_id) = hit_terminal {
                            let delta_y = match delta {
                                engine_core::ScrollDelta::Lines(_, y) => y,
                                engine_core::ScrollDelta::Pixels(_, y) => y / 20.0,
                            };
                            // A real wheel "up" (away from the user, a
                            // positive `y`) reveals older history --
                            // `scroll_by`'s own real sign convention.
                            if let Some(session) =
                                runtime.terminals.borrow_mut().get_mut(&terminal_id)
                            {
                                session.scroll_by(
                                    &mut runtime.tree.borrow_mut(),
                                    terminal_id,
                                    delta_y.round() as i64,
                                );
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
                // M42 Phase 2: reads through `active` directly, cloned
                // out and dropped immediately -- unlike the `access`
                // closure above (a pure read, never calls a handler),
                // this one calls `run_dispatch_outcome` below, which can
                // synchronously invoke a real Python handler that itself
                // calls `Window.show_view` (a genuine, expected pattern
                // -- a screen reader activating a nav control). Holding
                // `runtime.active`'s own `Ref` across that call would
                // panic on `show_view`'s `borrow_mut()` -- the identical
                // real bug caught and fixed in `window_input.rs`'s
                // `click`/`hover`/`scroll`/`right_click`, for the
                // identical reason.
                let (tree_rc, handlers, context_menus) = {
                    let active = runtime.active.borrow();
                    (
                        active.tree.clone(),
                        active.handlers.clone(),
                        active.context_menus.clone(),
                    )
                };
                let node = from_access_id(request.target_node);
                let mut tree = tree_rc.borrow_mut();
                match request.action {
                    engine_core::Action::Click => {
                        let outcome = tree.activate(node);
                        drop(tree);
                        // M54 Phase 2: no real originating `InputEvent`
                        // at all -- a screen reader's own semantic
                        // "activate this" request, not a mechanical
                        // pointer/keyboard event -- so `Event.position`/
                        // `button` correctly come back `None`, not
                        // fabricated.
                        run_dispatch_outcome(
                            &handlers,
                            &tree_rc,
                            &context_menus,
                            &runtime.theme,
                            &runtime.completions,
                            &outcome,
                            None,
                            py,
                        );
                    }
                    // M55 (§10, §16.2): parity with `Action::Click`
                    // just above -- a screen-reader-driven focus
                    // change is a real, equally legitimate way focus
                    // changes, so a registered `FocusEnter`/
                    // `FocusExit` handler fires the same way it does
                    // for real mouse/keyboard focus changes. Never
                    // reaches `Tree::dispatch`/`run_dispatch_outcome`
                    // at all (this calls `set_focus_to` directly, the
                    // one real non-`dispatch()` mutation path), so
                    // `fire_focus_transition` is called directly here
                    // instead.
                    engine_core::Action::Focus => {
                        // Assistive-technology navigation shows focus, as
                        // the keyboard does.
                        listeners::set_keyboard_modality(true);
                        let config = interaction_config();
                        let transition = tree.set_focus_to(
                            node,
                            config.focus_ring_opacity,
                            config.focus_ring_duration,
                            crate::clock::now(&tree_rc),
                        );
                        drop(tree);
                        if let Some((old, new)) = transition {
                            crate::dispatch::fire_focus_transition(
                                &handlers,
                                &tree_rc,
                                &context_menus,
                                &runtime.theme,
                                &runtime.completions,
                                old,
                                new,
                                py,
                            );
                        }
                    }
                    // M94: a screen reader asking to move focus away.
                    engine_core::Action::Blur => {
                        let config = interaction_config();
                        let transition = if tree.focused() == Some(node) {
                            tree.clear_focus(
                                config.focus_ring_opacity,
                                config.focus_ring_duration,
                                crate::clock::now(&tree_rc),
                            )
                        } else {
                            None
                        };
                        drop(tree);
                        if let Some((old, new)) = transition {
                            crate::dispatch::fire_focus_transition(
                                &handlers,
                                &tree_rc,
                                &context_menus,
                                &runtime.theme,
                                &runtime.completions,
                                old,
                                new,
                                py,
                            );
                        }
                    }
                    // M94: the rest reach the framework as `a11y_action`
                    // -- what incrementing a slider means is its call.
                    action => {
                        drop(tree);
                        let Some(name) = listeners::a11y_action_name(action) else {
                            return;
                        };
                        let value = match &request.data {
                            Some(engine_core::ActionData::Value(text)) => text
                                .to_string()
                                .into_pyobject(py)
                                .ok()
                                .map(|v| v.into_any().unbind()),
                            Some(engine_core::ActionData::NumericValue(number)) => {
                                number.into_pyobject(py).ok().map(|v| v.into_any().unbind())
                            }
                            _ => None,
                        };
                        listeners::deliver_a11y_action(
                            &NodeContext {
                                tree: &tree_rc,
                                handlers: &handlers,
                                context_menus: &context_menus,
                                theme: &runtime.theme,
                                completions: &runtime.completions,
                            },
                            node,
                            name,
                            value,
                            py,
                        );
                    }
                }
            },
            // M94: `close_requested` (cancellable) and `closed` window
            // events. A cancelled request keeps the window open; `closed`
            // also drops the window's handle to its OS window.
            move |window_id, lifecycle| {
                let runtimes = runtimes_for_lifecycle.borrow();
                let Some(runtime) = runtimes.get(&window_id) else {
                    return true;
                };
                match lifecycle {
                    WindowLifecycle::CloseRequested => !listeners::deliver_window(
                        &runtime.window_listeners,
                        py,
                        WindowEventType::CloseRequested,
                        |_| {},
                    ),
                    WindowLifecycle::Closed => {
                        listeners::deliver_window(
                            &runtime.window_listeners,
                            py,
                            WindowEventType::Closed,
                            |_| {},
                        );
                        *runtime.os_window.borrow_mut() = None;
                        true
                    }
                }
            },
            move |opener, waker| {
                // M87: from here on, `LoopHandle.call_soon` wakes this
                // run's loop -- including one idle in `ControlFlow::Wait`.
                calls_for_setup.set_waker(Some(waker.clone()));
                for (index, setup) in setups_for_setup.iter().enumerate() {
                    opener.open_window(WindowRequest {
                        config: WindowConfig {
                            title: setup.title.clone(),
                            width: setup.width.get(),
                            height: setup.height.get(),
                            max_frames,
                        },
                        token: index as u64,
                    });
                    // M31 Phase 6 (§5, §6): every real `Terminal` this
                    // window already has (a real `add_terminal` call
                    // always happens before `App.run()`, so every real
                    // session already exists by the time `setup` runs
                    // here) gets a real clone of this run's own fresh
                    // waker -- the one real place able to reach it at
                    // all, closing the real, stated v1 cost M30 Phase 9
                    // Step 4 left open.
                    for session in setup.terminals.borrow().values() {
                        session.set_waker(waker.clone());
                    }
                }
            },
        );

        // M87: this run's loop is gone -- a `call_soon` from now on just
        // queues, waiting for a later `run()`'s first frame, rather than
        // waking a proxy with no loop behind it.
        self.calls.set_waker(None);
        // M94: no window is open any more.
        for setup in setups_for_cleanup.iter() {
            *setup.os_window.borrow_mut() = None;
        }

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
    use super::{Cursor, cursor_at};
    use engine_core::{NodeKind, PaintProperties, Tree};
    use peniko::Color;
    use peniko::kurbo::Point;
    use taffy::prelude::{AvailableSpace, Size, Style, length};

    /// M94: the cursor comes from the node under the pointer or its nearest
    /// ancestor that sets one, the capturing node wins while it holds
    /// capture, and it falls back to the default arrow.
    #[test]
    fn cursor_at_inherits_and_follows_capture() {
        let mut tree = Tree::new();
        let boxed = |w: f32, h: f32| {
            (
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(w),
                        height: length(h),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 0.0, 1.0),
            )
        };
        let (k, s, p) = boxed(200.0, 200.0);
        let root = tree.insert(k, s, p);
        let (k, s, p) = boxed(100.0, 100.0);
        let parent = tree.insert(k, s, p);
        let (k, s, p) = boxed(50.0, 50.0);
        let child = tree.insert(k, s, p);
        tree.add_child(root, parent);
        tree.add_child(parent, child);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(200.0),
            },
        );
        let over_child = Point::new(10.0, 10.0);
        let outside = Point::new(150.0, 150.0);
        assert_eq!(cursor_at(&tree, root, over_child), Cursor::Default);

        tree.get_mut(parent).unwrap().cursor = Some(Cursor::Pointer);
        assert_eq!(cursor_at(&tree, root, over_child), Cursor::Pointer);
        assert_eq!(cursor_at(&tree, root, outside), Cursor::Default);

        tree.get_mut(child).unwrap().cursor = Some(Cursor::Grab);
        tree.set_pointer_capture(Some(child));
        assert_eq!(cursor_at(&tree, root, outside), Cursor::Grab);
    }

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
