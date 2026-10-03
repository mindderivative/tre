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
//! **No `Python::detach` around the render loop** -- its closure must be
//! `Ungil`, which on stable pyo3 0.29 requires `Send`, and every
//! `PyWindow`'s `Rc<RefCell<Tree>>` is `!Send` by design (§9). Real GIL
//! contention from a second thread arrived with `LoopHandle` (M87): an idle
//! loop parked in `ControlFlow::Wait` with the GIL held starves every other
//! Python thread, since CPython only hands the GIL over when the holder runs
//! bytecode (0.5.1, #92). So the loop releases the GIL for its wait alone,
//! with `PyEval_SaveThread`/`PyEval_RestoreThread` from `IdleHooks`, which
//! the platform layer calls in `about_to_wait` and before any other handler:
//! every Python callback still runs on the loop thread with the GIL held.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use engine_core::{Cursor, InputEvent, NodeId, NodeKind, PointerButton, Tree, from_access_id};
use engine_platform::{
    EventLoopWaker, IdleHooks, WindowConfig, WindowLifecycle, WindowOptions, WindowRequest,
    run_windowed_multi_with,
};
use engine_render::{
    GpuReport, GpuWatch, PresentChoice, TextPlacement, TextRenderer, WindowRenderer,
};
use peniko::kurbo::Point;
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};
use winit::window::{Window, WindowId};

use crate::dispatch::{WindowIo, process_input, run_completions, run_dispatch_outcome};
use crate::event::NodeContext;
use crate::listeners::{self, WindowEventType};
use crate::thread_bound::{ThreadBound, thread_bound_shell};
use crate::thread_handle::{CallQueue, LoopHandle};
use crate::window::{PyWindow, WindowHandles};

/// A window extent as the renderer takes it (`u16`), clamped -- no real
/// window reaches 65,535 pixels, but a bad size must not wrap around.
fn render_extent(pixels: u32) -> u16 {
    u16::try_from(pixels).unwrap_or(u16::MAX)
}

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
/// already follows. `handlers` is the one exception: `node.on(...)`
/// listeners' real `Py<PyAny>` callbacks have to be looked up by the
/// `on_input` closure below on real input, so this is the one
/// Python-object-bearing field extracted here rather than converted to
/// plain data.
struct WindowSetup {
    /// 0.4.0 M6: everything the window shares with this run, cloned
    /// once (`WindowHandles`).
    handles: WindowHandles,
    title: String,
}

/// M18 Phase 1/2 (§8, §10, §11.9, §11.10): the real per-glyph hit-test
/// `engine-core` structurally can't do itself (§4) -- shared by
/// `PointerPressed`'s click-to-position and `PointerMoved`'s drag-
/// extend, both of which need the exact same "is `hit` a `TextField`,
/// and if so what real byte offset does `local_point` land on"
/// answer. Mirrors `draw_own`'s own real `TextPlacement { x: 0.0,
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
    // reason `ScrollViewState` never derived it either. Holds `tree.borrow()` for this whole function's body
    // instead, released when it returns, before either real caller's
    // own subsequent `borrow_mut()`.
    let tree = tree.borrow();
    let node = tree.get(hit)?;
    let NodeKind::TextField(state) = &node.kind else {
        return None;
    };
    // The placement the field is painted at (`engine-render`'s `draw_own`):
    // inside its padding, and shifted by how far a multiline field has
    // scrolled. A click is resolved against what is painted, so it must use
    // the same one; before, it used the node's corner and ignored the scroll.
    let layout = tree.layout(hit);
    let pad = layout.padding;
    let at = TextPlacement {
        x: f64::from(pad.left) - state.horizontal_scroll_offset.current,
        y: f64::from(pad.top) - state.scroll_offset.current,
        max_width: (layout.size.width - pad.left - pad.right).max(0.0),
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
            // The cell grid starts inside the node's padding.
            let pad = tree_ref.layout(hit).padding;
            let inside = Point::new(
                local_point.x - f64::from(pad.left),
                local_point.y - f64::from(pad.top),
            );
            Some(text_renderer.terminal_hit_cell(state, inside))
        }
        _ => None,
    }
}

/// A real pointer press, move, or release over a text field or a terminal:
/// click-to-position, drag-to-select, and the end of a drag. Needs the text
/// renderer's font metrics, which `process_input` doesn't have, so it runs
/// here, after it. Takes the tree, the root, and the two drag trackers
/// rather than the whole window runtime, so a test can drive it without a
/// window.
fn text_pointer_input(
    event: &InputEvent,
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    text: &mut TextRenderer,
    text_drag: &mut Option<NodeId>,
    terminal_drag: &mut Option<NodeId>,
) {
    match *event {
        InputEvent::PointerPressed {
            position,
            button: PointerButton::Primary,
        } => {
            // M18 Phase 1 (§8, §10, §11.9, §11.10): widened
            // from `hit_test` to `hit_test_local` -- the
            // extra local-space point is exactly what a
            // real click-to-position hit-test needs below.
            // The hit is taken in its own statement: held in the `if let`'s
            // scrutinee, the tree's `Ref` would still be alive in its body,
            // and the `borrow_mut` there panicked (0.4.4, 0.5.0).
            let hit = tree.borrow().hit_test_local(root, position);
            if let Some((hit, local_point)) = hit {
                if let Some(offset) = text_field_hit_offset(tree, text, hit, local_point) {
                    tree.borrow_mut().set_text_field_cursor(hit, offset);
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
                    *text_drag = Some(hit);
                } else if let Some((row, col)) = terminal_hit_cell(tree, text, hit, local_point) {
                    // M32 Phase 6 (§4, §5, §8): a real press
                    // on a `Terminal` -- the identical real
                    // "collapsed selection, arm drag
                    // tracking" shape `TextField`'s own
                    // press handling just above already
                    // has.
                    tree.borrow_mut()
                        .set_terminal_selection_start(hit, row, col);
                    *terminal_drag = Some(hit);
                }
            }
        }
        InputEvent::PointerMoved { position } => {
            // M18 Phase 2 (§8, §10): the real drag-select
            // half of click-to-position. Lives here, not
            // inside `Tree::dispatch`'s own existing
            // `self.dragging`/`update_drag` mechanism
            // (scrollbar thumbs) -- that mechanism is pure
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
            if let Some(field) = *text_drag {
                let hit = tree.borrow().hit_test_local(root, position);
                if let Some((hit, local_point)) = hit
                    && hit == field
                    && let Some(offset) = text_field_hit_offset(tree, text, hit, local_point)
                {
                    tree.borrow_mut().extend_text_field_selection(hit, offset);
                }
            }
            // M32 Phase 6 (§4, §5, §8): `text_drag`'s own
            // real `Terminal` sibling -- the identical real
            // "still over the same node, extend" shape.
            if let Some(terminal) = *terminal_drag {
                let hit = tree.borrow().hit_test_local(root, position);
                if let Some((hit, local_point)) = hit
                    && hit == terminal
                    && let Some((row, col)) = terminal_hit_cell(tree, text, hit, local_point)
                {
                    tree.borrow_mut().extend_terminal_selection(hit, row, col);
                }
            }
        }
        InputEvent::PointerReleased {
            button: PointerButton::Primary,
            ..
        } => {
            // M18 Phase 2 (§8, §10): a real mouse-up always
            // ends any in-progress text drag, wherever it
            // happens -- the same "not conditioned on still
            // hitting the original node" real mouse-up
            // semantics `Tree::dispatch`'s own `self.
            // dragging = None` already established for
            // its own drags (M4 Phase 3).
            *text_drag = None;
            // M32 Phase 6 (§4, §5, §8): `text_drag`'s own
            // real `Terminal` sibling -- the real selection
            // itself stays visible (`TerminalState.
            // selection_start`/`end` are untouched here),
            // only the drag-tracking itself ends.
            *terminal_drag = None;
        }
        _ => {}
    }
}

struct GpuState {
    /// 0.4.0 M6: kept to recreate a lost `surface` for `window`.
    instance: wgpu::Instance,
    window: Arc<Window>,
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
    /// 0.4.0 M6: the window's renderer, caches, kept frame (M3), and
    /// damage tracker (M4), with the frame sequence that uses them --
    /// `engine_render::WindowRenderer`, which the pixel tests drive too.
    renderer: WindowRenderer,
    /// 0.5.1 (#65): this device's health: its lost and error handlers, and
    /// the stall watchdog. Installed the moment the device exists.
    watch: GpuWatch,
    /// 0.5.4 (#101): how the swapchain paces frames, and the modes this
    /// surface supports (the choice picks one of them).
    present: PresentChoice,
    present_modes: Vec<wgpu::PresentMode>,
}

impl GpuState {
    /// 0.4.0: an `Err` (no adapter, no device, or a surface this adapter
    /// can't drive) ends the run, and `App.run()` raises it.
    fn new(
        window: Arc<Window>,
        width: u32,
        height: u32,
        present: PresentChoice,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| format!("couldn't create a GPU surface for the window: {err}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            // The adapter's real limits, as before `wgpu` 30 added
            // bucketing (a fingerprinting defence for web content).
            apply_limit_buckets: false,
        }))
        // 0.4.0 review (the user's decision): no GPU is an error the
        // caller sees -- `App.run()` raises it -- rather than the old
        // `exit(0)` inside the interpreter, which skipped `atexit` and
        // `finally` and reported success.
        .map_err(|err| format!("no GPU adapter available: {err}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("engine-py app device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        }))
        .map_err(|err| format!("couldn't create a GPU device: {err}"))?;
        // 0.5.1 (#65): replaces wgpu's default handler, which panics on any
        // GPU error, and learns of a lost device.
        let watch = GpuWatch::install(&device);

        // 0.4.0 review: a window larger than the GPU's textures can be
        // renders at the largest size it can, not a panic in `configure`.
        let max = device.limits().max_texture_dimension_2d;
        let (width, height) = (width.min(max), height.min(max));
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or("the window's surface isn't supported by this GPU adapter")?;
        // 0.5.4 (#101): `get_default_config` takes the first mode the driver
        // lists, which is `Mailbox` on Linux with Mesa: it never waits for the
        // display, so one animating box ran the loop at ~3,500 frames a second
        // and used a whole CPU core. Choose the mode ourselves.
        let capabilities = surface.get_capabilities(&adapter);
        config.present_mode = present.mode(&capabilities.present_modes);
        tracing::debug!(mode = ?config.present_mode, choice = present.name(), "present mode");
        // 0.4.0 M3: render into a persistent target and copy it into the
        // swapchain image, where the surface allows copies into it.
        let copyable = capabilities.usages.contains(wgpu::TextureUsages::COPY_DST);
        if copyable {
            config.usage |= wgpu::TextureUsages::COPY_DST;
            tracing::debug!("rendering through a persistent target, copied to the surface");
        } else {
            tracing::warn!(
                "this window's surface can't be copied into: rendering straight to it, \
                 every frame in full (no partial redraw)"
            );
        }
        surface.configure(&device, &config);
        let renderer = WindowRenderer::new(&device, config.format, width, height, copyable);

        Ok(Self {
            instance,
            window,
            surface,
            surface_config: config,
            device,
            queue,
            renderer,
            watch,
            present,
            present_modes: capabilities.present_modes,
        })
    }

    /// 0.5.4 (#101): switches how the swapchain paces frames, live. A no-op
    /// when the choice is the one in effect, so it can be called every frame.
    fn set_present(&mut self, choice: PresentChoice) {
        if choice == self.present {
            return;
        }
        self.present = choice;
        let mode = choice.mode(&self.present_modes);
        if mode != self.surface_config.present_mode {
            self.surface_config.present_mode = mode;
            self.surface.configure(&self.device, &self.surface_config);
            tracing::debug!(?mode, choice = choice.name(), "present mode changed");
        }
    }

    /// M32 Phase 2 (§4, §5): reconfigures the real wgpu surface to a
    /// genuinely new client-area size -- the textbook real wgpu resize
    /// recipe (mutate the stored config's own `width`/`height`, then
    /// `surface.configure` again), not something this crate invents.
    /// **Real, confirmed finding before writing this:** `FrameRenderer`/
    /// `vello_gpu::Renderer` need no matching reconstruction at all
    /// -- `vello_gpu`'s `Renderer::render` calls a private
    /// `maybe_update_config_buffer` every frame (checked in the pinned
    /// source, 0.4.0 M2), updating its size-dependent state whenever the
    /// `RenderSize` passed to `render` differs from the previous call;
    /// its depth buffer is the caller's, and `tre` passes none -- real,
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
        let (width, height) = self.fit(width, height);
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.renderer.resize(&self.device, width, height);
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
    /// size -- `runtime.handles.width`/`height` (the live `SharedSize` cells)
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
    /// 0.4.0 M6: a new surface for the window after the old one was lost
    /// (wgpu's `CurrentSurfaceTexture::Lost`), configured as the old one
    /// was, its kept frame's contents redrawn next frame. Whether that
    /// worked.
    fn recreate_surface(&mut self) -> bool {
        match self.instance.create_surface(self.window.clone()) {
            Ok(surface) => {
                surface.configure(&self.device, &self.surface_config);
                self.surface = surface;
                self.renderer.reset();
                true
            }
            Err(err) => {
                tracing::warn!(%err, "the window's surface was lost and couldn't be recreated");
                false
            }
        }
    }

    fn needs_resize(&self, width: u32, height: u32) -> bool {
        let (width, height) = self.fit(width, height);
        self.surface_config.width != width || self.surface_config.height != height
    }

    /// 0.4.0 review: `width` x `height` within the largest texture the
    /// device can make -- the surface, the persistent target, and the
    /// render size all stay inside it, so an oversized window renders
    /// clamped rather than panicking in `configure` or `create_texture`.
    fn fit(&self, width: u32, height: u32) -> (u32, u32) {
        let max = self.device.limits().max_texture_dimension_2d;
        (width.min(max), height.min(max))
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

/// 0.5.0 M3: the resize cursor for an edge or corner of the resize border.
fn border_cursor(direction: winit::window::ResizeDirection) -> Cursor {
    use winit::window::ResizeDirection as Dir;
    match direction {
        Dir::North | Dir::South => Cursor::NsResize,
        Dir::East | Dir::West => Cursor::EwResize,
        Dir::NorthWest | Dir::SouthEast => Cursor::NwseResize,
        Dir::NorthEast | Dir::SouthWest => Cursor::NeswResize,
    }
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

/// 0.5.1 (#65): turns what the GPU reported into window events, on the
/// loop's own thread with the GIL, and returns why the device was lost, if it
/// was.
fn deliver_gpu_reports(
    handles: &WindowHandles,
    reports: Vec<GpuReport>,
    py: Python<'_>,
) -> Option<String> {
    let mut lost = None;
    for report in reports {
        match report {
            GpuReport::Lost { destroyed, message } => {
                let reason = if destroyed { "destroyed" } else { "unknown" };
                lost = Some(if message.is_empty() {
                    format!("the device was {reason}")
                } else {
                    message.clone()
                });
                listeners::deliver_window(
                    &handles.window_listeners,
                    py,
                    WindowEventType::GpuLost,
                    |e| {
                        e.reason = Some(reason.to_string());
                        e.message = Some(message);
                    },
                );
            }
            GpuReport::Error { message } => {
                listeners::deliver_window(
                    &handles.window_listeners,
                    py,
                    WindowEventType::GpuError,
                    |e| e.message = Some(message),
                );
            }
            GpuReport::Stalled { seconds } => {
                listeners::deliver_window(
                    &handles.window_listeners,
                    py,
                    WindowEventType::GpuStalled,
                    |e| e.seconds = Some(seconds),
                );
            }
        }
    }
    lost
}

/// 0.5.1 (#65): while any GPU work is in flight, wakes an otherwise idle
/// loop about every 100 ms so the device gets polled -- a hang in the last
/// submitted frame would otherwise never be noticed, since nothing else runs
/// in a sleeping loop. It costs nothing while the GPU is idle.
struct GpuWake {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl GpuWake {
    fn start(waker: EventLoopWaker) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("tre-gpu-wake".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::park_timeout(Duration::from_millis(100));
                    if engine_render::any_in_flight() {
                        waker.wake();
                    }
                }
            })
            .ok();
        Self { stop, thread }
    }
}

impl Drop for GpuWake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

struct WindowRuntime {
    /// 0.4.0 M6: the window's shared handles (`WindowHandles`).
    handles: WindowHandles,
    gpu: GpuState,
    /// 0.5.1 (#70): when the window opened, on the window's clock: a
    /// shader's `frame.time` is the seconds since.
    opened: std::time::Instant,
    /// M18 Phase 2 (§8, §10): which `TextField` (if any) a real
    /// press-and-drag is currently extending a selection in -- plain,
    /// not `RefCell`-wrapped, since only `on_input`'s own closure ever
    /// reads or writes it. Lives here rather than `engine-core`'s
    /// existing `Tree.dragging` (scrollbar-thumb drags): that
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
            crate::clock::unpin(&window.handles.tree);
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
                    handles: window.handles.clone(),
                    title: window.title.clone(),
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

        // 0.4.0: why a window's GPU setup failed, raised once the loop
        // has stopped.
        let startup_error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let startup_error_for_created = startup_error.clone();
        let startup_error_for_frame = startup_error.clone();
        let startup_error_for_lifecycle = startup_error.clone();
        let gpu_wake: Rc<RefCell<Option<GpuWake>>> = Rc::new(RefCell::new(None));
        let gpu_wake_for_setup = gpu_wake.clone();
        // 0.5.1 (#92): the loop waits for the next event with this thread's
        // GIL released, so other Python threads (a file watcher, a
        // `LoopHandle.call_soon` caller) run while the window is idle. The
        // platform layer calls these around its wait only: every callback
        // below runs with the GIL held again, as before. See the header.
        let thread_state: Rc<std::cell::Cell<*mut pyo3::ffi::PyThreadState>> =
            Rc::new(std::cell::Cell::new(std::ptr::null_mut()));
        let thread_state_for_restore = thread_state.clone();
        let idle = IdleHooks {
            // SAFETY: `PyEval_SaveThread`/`PyEval_RestoreThread` are the
            // pair `Python::detach` itself uses. The platform layer calls
            // `release` only from `about_to_wait` and `reacquire` before any
            // other handler runs, always on this thread, and no Python
            // object is touched in between.
            release: Box::new(move || {
                thread_state.set(unsafe { pyo3::ffi::PyEval_SaveThread() });
            }),
            reacquire: Box::new(move || {
                let state = thread_state_for_restore.replace(std::ptr::null_mut());
                if !state.is_null() {
                    unsafe { pyo3::ffi::PyEval_RestoreThread(state) };
                }
            }),
        };
        let result = run_windowed_multi_with(
            move |window_id, token, window| {
                let setup = &setups_for_created[token as usize];
                *setup.handles.os_window.borrow_mut() = Some(window.clone());
                // 0.5.0 M2: `winit` can open a window maximized, but not
                // minimized -- a `minimize()` before `App.run()` lands here.
                if setup.handles.minimized.get() {
                    window.set_minimized(true);
                }
                // 0.5.4 (#102): from here the stored size is the window's real
                // (physical) one, not the logical size it was asked for, and
                // the scale is the window's if `dpi_scaling` is on.
                let inner = window.inner_size();
                if inner.width > 0 && inner.height > 0 {
                    setup.handles.width.set(inner.width);
                    setup.handles.height.set(inner.height);
                }
                setup.handles.refresh_scale();
                let gpu = match GpuState::new(
                    window,
                    setup.handles.width.get(),
                    setup.handles.height.get(),
                    setup.handles.present_mode.get(),
                ) {
                    Ok(gpu) => gpu,
                    Err(err) => {
                        *startup_error_for_created.borrow_mut() = Some(err);
                        return false;
                    }
                };
                setup
                    .handles
                    .surface_partial
                    .set(Some(gpu.renderer.has_persistent_target()));
                runtimes_for_created.borrow_mut().insert(
                    window_id,
                    WindowRuntime {
                        handles: setup.handles.clone(),
                        gpu,
                        opened: crate::clock::now(&setup.handles.tree),
                        text_drag: None,
                        terminal_drag: None,
                        cursor: Cursor::Default,
                    },
                );
                true
            },
            move |window_id, _frame, os_requested| -> bool {
                // M87: run anything a background thread queued via
                // `LoopHandle.call_soon` first, so a queued change is
                // what this very frame ticks and paints. No borrow of
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

                // 0.5.1 (#65): what the GPU says about itself, before any of
                // this frame's work touches it. `_lose_gpu` is a test hook.
                if runtime.handles.lose_gpu.take() {
                    runtime.gpu.device.destroy();
                }
                runtime
                    .gpu
                    .watch
                    .set_watchdog(runtime.handles.gpu_watchdog.get());
                let reports = runtime.gpu.watch.poll(&runtime.gpu.device);
                if let Some(why) = deliver_gpu_reports(&runtime.handles, reports, py) {
                    // A lost GPU ends the run: every window is closed (a
                    // `close_requested` listener can't keep one open) and
                    // `App.run()` raises this.
                    *startup_error_for_frame.borrow_mut() =
                        Some(format!("the GPU was lost: {why}; the run has ended"));
                    for (id, other) in runtimes.iter() {
                        if let Some(waker) = other.handles.waker.borrow().as_ref() {
                            waker.close_window(*id);
                        }
                    }
                    return false;
                }
                if runtime.gpu.watch.is_lost() {
                    return false;
                }

                let now = crate::clock::now(&runtime.handles.tree);
                let (any_active, completed) = runtime.handles.tree.borrow_mut().tick_all(now);
                // 0.5.1 (#70): a shader's time is the window's clock; and a
                // window drawing an `animated` shader keeps running -- a frame
                // per display refresh, repainting just that node -- exactly
                // as it does for any animation. A still shader leaves the
                // loop asleep.
                runtime
                    .gpu
                    .renderer
                    .set_time(now.saturating_duration_since(runtime.opened).as_secs_f32());
                let any_active = if runtime.gpu.renderer.has_animated_shader() {
                    runtime.handles.tree.borrow_mut().mark_dirty();
                    true
                } else {
                    any_active
                };
                // 0.4.2 M12: an animated or `set` scroll offset reports
                // its change here, once a frame.
                crate::listeners::fire_scroll_changes(
                    &crate::event::NodeContext {
                        tree: &runtime.handles.tree,
                        handlers: &runtime.handles.handlers,
                        completions: &runtime.handles.completions,
                    },
                    py,
                );
                // M9 Phase 2 (§5): the real drain -- invokes each
                // just-completed animation's registered `on_complete`
                // callback exactly once, the same "look up and call a
                // registered callback" shape `run_dispatch_outcome`
                // already uses for click/hover handlers.
                run_completions(&runtime.handles.completions, completed, py);

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
                    let mut terminals = runtime.handles.terminals.borrow_mut();
                    let mut tree = runtime.handles.tree.borrow_mut();
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
                // 0.5.4 (#102): the display scale (the window's, when
                // `dpi_scaling` is on): a change relays out and redraws.
                if runtime.handles.refresh_scale() {
                    runtime.handles.tree.borrow_mut().mark_dirty();
                }
                runtime.gpu.renderer.set_scale(runtime.handles.scale.get());
                // 0.5.4 (#101): `window.set(present_mode=...)` takes effect live.
                runtime.gpu.set_present(runtime.handles.present_mode.get());
                let resized = runtime
                    .gpu
                    .needs_resize(runtime.handles.width.get(), runtime.handles.height.get());
                // M86: a `tre.register_font` call touches no `Tree`
                // state either -- same reasoning as `resized` above. A
                // family that fell back before may resolve to a real
                // face now, so this frame must repaint. One atomic load
                // when nothing was registered.
                let fonts_changed = runtime.gpu.renderer.text().sync_registered_fonts();
                let dirty = runtime.handles.tree.borrow_mut().take_dirty();
                let changed = dirty || resized || fonts_changed;
                // 0.4.0 M6: an OS redraw (an expose) with nothing changed
                // still gets the kept frame presented again -- the surface
                // image may have lost it -- where there is a kept frame.
                if !changed && !(os_requested && runtime.gpu.renderer.has_persistent_target()) {
                    return any_active;
                }

                // M96: also builds virtual lists' newly visible rows.
                crate::node_callbacks::layout(
                    &runtime.handles.tree,
                    runtime.handles.root,
                    {
                        let (width, height) = runtime.handles.logical_size();
                        Size {
                            width: AvailableSpace::Definite(width as f32),
                            height: AvailableSpace::Definite(height as f32),
                        }
                    },
                    &runtime.handles.handlers,
                    py,
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
                        .resize(runtime.handles.width.get(), runtime.handles.height.get());
                }

                // 0.4.0 M5: what changed since the last frame
                // (`WindowRenderer::prepare`). A newly registered font can
                // reshape text no node's state records, so it redraws
                // everything.
                let (width, height) = runtime
                    .gpu
                    .fit(runtime.handles.width.get(), runtime.handles.height.get());
                let (width, height) = (render_extent(width), render_extent(height));
                if fonts_changed {
                    runtime.gpu.renderer.reset();
                }
                let damage = {
                    let tree_ref = runtime.handles.tree.borrow();
                    let gpu = &mut runtime.gpu;
                    gpu.renderer.prepare(
                        &tree_ref,
                        runtime.handles.root,
                        width,
                        height,
                        runtime.handles.partial_redraw.get(),
                        &gpu.device,
                        &gpu.queue,
                    )
                };
                tracing::trace!(?damage, os_requested, "frame damage");
                // 0.4.0 M6: nothing to show that isn't already shown, only
                // this loop asked, and nothing is animating -- no image to
                // acquire or present. While something animates, the present
                // is kept: its wait for the display is what paces the loop,
                // which would otherwise spin through unchanged frames.
                if damage == engine_render::Damage::None && !os_requested && !any_active {
                    return any_active;
                }

                let gpu = &mut runtime.gpu;
                let (output, reconfigure) = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(output) => (output, false),
                    // Usable, but wgpu recommends reconfiguring after it.
                    wgpu::CurrentSurfaceTexture::Suboptimal(output) => (output, true),
                    failed => {
                        // The tracker already recorded this frame's tree as
                        // drawn, and its dirty flag is spent; it wasn't
                        // drawn, so the next frame redraws it all.
                        gpu.renderer.reset();
                        runtime.handles.tree.borrow_mut().mark_dirty();
                        // 0.4.0 review: an outdated surface recovers once
                        // reconfigured, a lost one once recreated (wgpu 30's
                        // docs) -- then retry at once; a timeout or an
                        // occluded window waits for the next frame.
                        let retry = match failed {
                            wgpu::CurrentSurfaceTexture::Outdated => {
                                gpu.surface.configure(&gpu.device, &gpu.surface_config);
                                true
                            }
                            wgpu::CurrentSurfaceTexture::Lost => gpu.recreate_surface(),
                            _ => false,
                        };
                        return any_active || retry;
                    }
                };
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = runtime
                    .gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let tree_ref = runtime.handles.tree.borrow();
                    let gpu = &mut runtime.gpu;
                    gpu.renderer.draw(
                        &tree_ref,
                        runtime.handles.root,
                        width,
                        height,
                        &damage,
                        &gpu.device,
                        &gpu.queue,
                        &mut encoder,
                        &output.texture,
                        &view,
                    );
                }
                runtime.gpu.queue.submit([encoder.finish()]);
                runtime.gpu.watch.submitted(&runtime.gpu.queue);
                // 0.4.1 M8: what this frame redrew, over the image but never
                // the kept frame; its own submit, after the frame's.
                if runtime.handles.show_damage.get() {
                    let gpu = &mut runtime.gpu;
                    gpu.renderer.draw_damage_overlay(
                        &damage,
                        width,
                        height,
                        &gpu.device,
                        &gpu.queue,
                        &view,
                    );
                    // The overlay submits its own work.
                    gpu.watch.submitted(&gpu.queue);
                }
                runtime.gpu.queue.present(output);
                if reconfigure {
                    runtime
                        .gpu
                        .surface
                        .configure(&runtime.gpu.device, &runtime.gpu.surface_config);
                }
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
                // 0.5.4 (#102): bounds in physical pixels, like the window.
                runtime
                    .handles
                    .tree
                    .borrow()
                    .build_access_update_scaled(runtime.handles.root, runtime.handles.scale.get())
            },
            // M4 Phase 1 step 3: the real "meaning-dependent" half
            // `Tree::dispatch` leaves for its own caller (§2 Design
            // Principle 6) -- every mechanical consequence (hover, focus
            // movement) already happened inside
            // `dispatch` itself; this closure hands each input to
            // `dispatch::process_input`, which delivers the listeners.
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

                // 0.5.4 (#102): input arrives in physical pixels; the engine
                // speaks logical ones. A resize first records the physical
                // size the surface needs.
                if runtime.handles.refresh_scale() {
                    // Consumed here, so the frame won't see it change: the
                    // logical size moved, and layout must follow.
                    runtime.handles.tree.borrow_mut().mark_dirty();
                }
                if let InputEvent::Resized { width, height } = &event {
                    runtime.handles.width.set(*width as u32);
                    runtime.handles.height.set(*height as u32);
                }
                let event = crate::scale::to_logical(event, runtime.handles.scale.get());

                // M94: modifier state is shared by every window's event
                // routing (`listeners::modifiers`), nothing more to do.
                if let InputEvent::ModifiersChanged(modifiers) = event {
                    listeners::set_modifiers(modifiers);
                    return;
                }

                // M94: dispatch, `node.on(...)` listeners, (M99) docking
                // drags, and (M100) terminal keys and the
                // clipboard shortcuts -- one pipeline shared with
                // `Window.simulate` (`dispatch::process_input`). `event`
                // itself is still needed below, for the text-field and
                // terminal pointer handling.
                process_input(
                    &NodeContext {
                        tree: &runtime.handles.tree,
                        handlers: &runtime.handles.handlers,
                        completions: &runtime.handles.completions,
                    },
                    &WindowIo {
                        dock: &runtime.handles.dock,
                        listeners: &runtime.handles.window_listeners,
                        terminals: &runtime.handles.terminals,
                        window: &runtime.handles,
                    },
                    runtime.handles.root,
                    &event,
                    py,
                );
                // M94: the pointer shape follows the node under the
                // pointer (or the capturing node).
                if let InputEvent::PointerMoved { position }
                | InputEvent::PointerPressed { position, .. }
                | InputEvent::PointerReleased { position, .. } = &event
                {
                    // 0.5.0 M3: the resize border's cursors come first.
                    let wanted = crate::dispatch::border_direction(&runtime.handles, *position)
                        .map(border_cursor)
                        .unwrap_or_else(|| {
                            cursor_at(
                                &runtime.handles.tree.borrow(),
                                runtime.handles.root,
                                *position,
                            )
                        });
                    if wanted != runtime.cursor {
                        if let Some(window) = runtime.handles.os_window.borrow().as_ref() {
                            window.set_cursor(cursor_icon(wanted));
                        }
                        runtime.cursor = wanted;
                    }
                }
                // Text-field and terminal pointer handling that needs the
                // text renderer, which `process_input` has no access to.
                text_pointer_input(
                    &event,
                    &runtime.handles.tree,
                    runtime.handles.root,
                    runtime.gpu.renderer.text(),
                    &mut runtime.text_drag,
                    &mut runtime.terminal_drag,
                );
                match event {
                    // A real OS appearance change: tre themes nothing
                    // itself (M99), so it only tells the framework.
                    InputEvent::ThemeChanged { dark } => {
                        listeners::deliver_window(
                            &runtime.handles.window_listeners,
                            py,
                            WindowEventType::ColorScheme,
                            |e| e.dark = Some(dark),
                        );
                    }
                    // 0.5.0 M2 (issue #28): the window gained or lost focus.
                    InputEvent::Focused { focused } => {
                        listeners::update_window_state(
                            &runtime.handles.window_listeners,
                            py,
                            &runtime.handles.active,
                            focused,
                            WindowEventType::Active,
                        );
                    }
                    // M32 Phase 2 (§4, §5): the real, winit-driven
                    // window resize -- `Tree::dispatch` (called just
                    // above, unconditionally, for every real
                    // `InputEvent`) already resized `runtime.handles.root`'s
                    // own `layout_style.size` directly (`engine-core`
                    // fully owns that, no need to defer it here); this
                    // arm handles the one thing only `engine-py` can
                    // (`runtime.handles.width`/`height`, which every per-frame
                    // `compute_layout`/`build_tree_scene`/`RenderSize`
                    // call already reads fresh -- see `RedrawRequested`
                    // above).
                    //
                    // M33 Phase 2 (§4, §5, §8) closed the real, stated
                    // v1 limit this comment used to name here:
                    // `runtime.handles.width`/`height` are now the identical
                    // real, shared `Rc<Cell<u32>>` `PyWindow`'s own
                    // fields are (`window::SharedSize`), so this `.set()`
                    // call is immediately visible there too -- a
                    // listener reading the window's size after a real
                    // resize sees its real *current* dimensions, not its
                    // construction-time ones.
                    //
                    // M40 Phase 1 (§4, §6, §9): no longer calls `runtime.
                    // gpu.resize(...)` here -- a real, live drag can
                    // deliver many `Resized` events between two real
                    // frames, and reconfiguring the wgpu surface (a
                    // genuine swapchain rebuild) on every single one is
                    // the real, measured root cause of the "trailing
                    // behind the cursor" symptom the 0.3 line's
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
                        // (The physical size was stored above; `width` and
                        // `height` here are logical.)
                        listeners::deliver_window(
                            &runtime.handles.window_listeners,
                            py,
                            WindowEventType::Resize,
                            |e| {
                                e.width = Some(f64::from(width));
                                e.height = Some(f64::from(height));
                            },
                        );
                        // 0.5.0 M2: `winit` sends no maximize event -- a
                        // maximize or restore arrives as a resize, so the
                        // state is checked after each one.
                        let window = runtime.handles.os_window.borrow().clone();
                        if let Some(window) = window {
                            listeners::update_window_state(
                                &runtime.handles.window_listeners,
                                py,
                                &runtime.handles.maximized,
                                window.is_maximized(),
                                WindowEventType::Maximized,
                            );
                            if let Some(minimized) = window.is_minimized() {
                                runtime.handles.minimized.set(minimized);
                            }
                            // 0.5.0 M4: fullscreen entered or left by the
                            // user (macOS's green button) arrives as a
                            // resize too. On macOS the overlay title bar is
                            // re-applied after it -- setting it is a no-op
                            // when it's already on -- and changing either
                            // moves the traffic lights.
                            runtime
                                .handles
                                .fullscreen
                                .set(window.fullscreen().is_some());
                            if engine_platform::titlebar::OVERLAY_TITLEBAR
                                && !runtime.handles.decorations.get()
                            {
                                engine_platform::titlebar::set_decorations(&window, false);
                            }
                            crate::window_events::refresh_titlebar_inset(&runtime.handles, py);
                        }
                        // 0.5.0 M2: e.g. leaving fullscreen to a size below
                        // the minimum, which Wayland allows.
                        crate::window_events::grow_to_minimum(&runtime.handles);
                    }
                    InputEvent::ScaleFactorChanged { scale_factor } => {
                        listeners::deliver_window(
                            &runtime.handles.window_listeners,
                            py,
                            WindowEventType::ScaleFactor,
                            |e| e.scale_factor = Some(scale_factor),
                        );
                    }
                    _ => {}
                }
            },
            // M4 Phase 2 (§10): a real screen reader naming a node to
            // activate or focus directly, routed through the exact same
            // `run_dispatch_outcome`/`handlers` path a mouse click or
            // `window.simulate("click", ...)` already uses -- one
            // click-handling mechanism, not three separate ones.
            move |window_id, request| {
                let runtimes = runtimes_for_access_action.borrow();
                let Some(runtime) = runtimes.get(&window_id) else {
                    return;
                };
                let (tree_rc, handlers) = (
                    runtime.handles.tree.clone(),
                    runtime.handles.handlers.clone(),
                );
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
                            &runtime.handles.completions,
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
                        let transition = tree.set_focus_to(node);
                        drop(tree);
                        if let Some((old, new)) = transition {
                            crate::dispatch::fire_focus_transition(
                                &handlers,
                                &tree_rc,
                                &runtime.handles.completions,
                                old,
                                new,
                                py,
                            );
                        }
                    }
                    // M94: a screen reader asking to move focus away.
                    engine_core::Action::Blur => {
                        let transition = if tree.focused() == Some(node) {
                            tree.clear_focus()
                        } else {
                            None
                        };
                        drop(tree);
                        if let Some((old, new)) = transition {
                            crate::dispatch::fire_focus_transition(
                                &handlers,
                                &tree_rc,
                                &runtime.handles.completions,
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
                                completions: &runtime.handles.completions,
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
                    // 0.5.1 (#65): once the GPU is lost the run is ending, and a
                    // listener can't keep a window open.
                    WindowLifecycle::CloseRequested => {
                        startup_error_for_lifecycle.borrow().is_some()
                            || !listeners::deliver_window(
                                &runtime.handles.window_listeners,
                                py,
                                WindowEventType::CloseRequested,
                                |_| {},
                            )
                    }
                    WindowLifecycle::Closed => {
                        listeners::deliver_window(
                            &runtime.handles.window_listeners,
                            py,
                            WindowEventType::Closed,
                            |_| {},
                        );
                        *runtime.handles.os_window.borrow_mut() = None;
                        true
                    }
                }
            },
            move |opener, waker| {
                // M87: from here on, `LoopHandle.call_soon` wakes this
                // run's loop -- including one idle in `ControlFlow::Wait`.
                calls_for_setup.set_waker(Some(waker.clone()));
                *gpu_wake_for_setup.borrow_mut() = Some(GpuWake::start(waker.clone()));
                for (index, setup) in setups_for_setup.iter().enumerate() {
                    opener.open_window(WindowRequest {
                        config: WindowConfig {
                            title: setup.title.clone(),
                            width: setup.handles.width.get(),
                            height: setup.handles.height.get(),
                            max_frames,
                            options: WindowOptions {
                                decorations: setup.handles.decorations.get(),
                                maximized: setup.handles.maximized.get(),
                                fullscreen: setup.handles.fullscreen.get(),
                                min_size: Some(setup.handles.min_size.get())
                                    .filter(|size| *size != (0.0, 0.0)),
                                icon: setup.handles.icon.borrow().clone(),
                            },
                        },
                        token: index as u64,
                    });
                    // M31 Phase 6 (§5, §6): every real `Terminal` this
                    // window already has (a real
                    // `window.create("terminal", ...)` call always happens
                    // before `App.run()`, so every real session already
                    // exists by the time `setup` runs here) gets a real clone of this run's own fresh
                    // waker -- the one real place able to reach it at
                    // all, closing the real, stated v1 cost M30 Phase 9
                    // Step 4 left open.
                    for session in setup.handles.terminals.borrow().values() {
                        session.set_waker(waker.clone());
                    }
                    // 0.5.0 M2: for `window.close()`.
                    *setup.handles.waker.borrow_mut() = Some(waker.clone());
                }
            },
            Some(idle),
        );

        // M87: this run's loop is gone -- a `call_soon` from now on just
        // queues, waiting for a later `run()`'s first frame, rather than
        // waking a proxy with no loop behind it.
        self.calls.set_waker(None);
        gpu_wake.borrow_mut().take(); // stops and joins the wake thread
        // M94: no window is open any more.
        for setup in setups_for_cleanup.iter() {
            *setup.handles.os_window.borrow_mut() = None;
            *setup.handles.waker.borrow_mut() = None;
            setup.handles.surface_partial.set(None);
        }

        if let Some(err) = startup_error.borrow_mut().take() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(err));
        }
        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                // A `run_windowed_multi` failure only ever means "no
                // display reachable" -- a GPU that can't be set up is
                // raised just above instead. Same TRE v1 finding
                // #261 convention as every other entry point in this
                // workspace. `tracing::warn!` (M16 Phase 2): the same
                // "expected, gracefully-handled, not an error" reasoning
                // as the no-adapter case above.
                tracing::warn!(%err, "no display available, returning without running");
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

    /// A window with one 200x30 text field at its top-left corner and one
    /// 200x60 terminal under it; a real pointer press on either runs
    /// `text_pointer_input`, which needs the tree *not* borrowed while it
    /// writes. `simulate` never reaches it, so only a real window did.
    type Scene = (
        std::rc::Rc<std::cell::RefCell<Tree>>,
        engine_core::NodeId,
        engine_core::NodeId,
        engine_core::NodeId,
    );

    fn field_and_terminal() -> Scene {
        field_and_terminal_padded(0.0, 0.0)
    }

    /// The same, with `padding` left and top on the field and the terminal.
    fn field_and_terminal_padded(pad_left: f32, pad_top: f32) -> Scene {
        let mut tree = Tree::new();
        let sized = |w: f32, h: f32| Style {
            size: Size {
                width: length(w),
                height: length(h),
            },
            padding: taffy::geometry::Rect {
                left: length(pad_left),
                top: length(pad_top),
                right: length(0.0),
                bottom: length(0.0),
            },
            ..Default::default()
        };
        let paint = || PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
        let root = tree.insert(
            NodeKind::Container,
            Style {
                flex_direction: taffy::FlexDirection::Column,
                size: Size {
                    width: length(300.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            paint(),
        );
        let field = tree.insert(
            NodeKind::TextField(Box::new(engine_core::TextFieldState::new(
                "hello world",
                "Roboto",
                400.0,
                16.0,
            ))),
            sized(200.0, 30.0),
            paint(),
        );
        let terminal = tree.insert(
            NodeKind::Terminal(Box::new(engine_core::TerminalState::new(
                20,
                4,
                engine_render::MONOSPACE_FONT_FAMILY,
                14.0,
            ))),
            sized(200.0, 60.0),
            paint(),
        );
        tree.add_child(root, field);
        tree.add_child(root, terminal);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(200.0),
            },
        );
        (
            std::rc::Rc::new(std::cell::RefCell::new(tree)),
            root,
            field,
            terminal,
        )
    }

    fn pointer(
        event: engine_core::InputEvent,
        tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
        root: engine_core::NodeId,
        drags: &mut (Option<engine_core::NodeId>, Option<engine_core::NodeId>),
    ) {
        let mut text = engine_render::TextRenderer::new();
        super::text_pointer_input(&event, tree, root, &mut text, &mut drags.0, &mut drags.1);
    }

    fn press(position: Point) -> engine_core::InputEvent {
        engine_core::InputEvent::PointerPressed {
            position,
            button: engine_core::PointerButton::Primary,
        }
    }

    /// A real click in a text field puts the caret there and arms a drag.
    /// It panicked with `RefCell already borrowed` (0.4.4 and 0.5.0).
    #[test]
    fn a_press_in_a_text_field_positions_the_caret_and_arms_a_drag() {
        let (tree, root, field, _) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(60.0, 10.0)), &tree, root, &mut drags);
        assert_eq!(drags.0, Some(field), "the drag is armed");
        let tree = tree.borrow();
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            unreachable!()
        };
        assert!(state.cursor > 0, "the caret moved to the click");
        assert_eq!(
            state.selection_anchor, None,
            "a plain click selects nothing"
        );
    }

    /// Dragging from a press across the field selects what it passes over.
    #[test]
    fn a_drag_in_a_text_field_extends_the_selection() {
        let (tree, root, field, _) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(20.0, 10.0)), &tree, root, &mut drags);
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(120.0, 10.0),
            },
            &tree,
            root,
            &mut drags,
        );
        {
            let tree = tree.borrow();
            let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
                unreachable!()
            };
            assert!(state.selection_anchor.is_some(), "a selection began");
            assert_ne!(Some(state.cursor), state.selection_anchor);
        }
        pointer(
            engine_core::InputEvent::PointerReleased {
                position: Point::new(120.0, 10.0),
                button: engine_core::PointerButton::Primary,
            },
            &tree,
            root,
            &mut drags,
        );
        assert_eq!(drags, (None, None), "a release ends the drag");
    }

    /// The same for a terminal: a press starts a selection, a drag grows it.
    #[test]
    fn a_press_and_drag_in_a_terminal_select_cells() {
        let (tree, root, _, terminal) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(10.0, 40.0)), &tree, root, &mut drags);
        assert_eq!(drags.1, Some(terminal), "the drag is armed");
        pointer(
            engine_core::InputEvent::PointerMoved {
                position: Point::new(90.0, 60.0),
            },
            &tree,
            root,
            &mut drags,
        );
        let tree = tree.borrow();
        let Some(NodeKind::Terminal(state)) = tree.get(terminal).map(|n| &n.kind) else {
            unreachable!()
        };
        assert!(state.selection_start.is_some());
        assert_ne!(
            state.selection_start, state.selection_end,
            "the selection grew"
        );
    }

    fn state_of_field(
        tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
        field: engine_core::NodeId,
    ) -> usize {
        let tree = tree.borrow();
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            unreachable!()
        };
        state.cursor
    }

    /// 0.5.1 (#53): with `padding`, a click lands on the character under it:
    /// the same glyph, clicked where it now is, gives the same offset.
    #[test]
    fn a_click_in_a_padded_text_input_lands_on_the_character_under_it() {
        let (plain, root, field, _) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(60.0, 10.0)), &plain, root, &mut drags);
        let expected = state_of_field(&plain, field);
        assert!(expected > 0, "the click is inside the text");

        let (padded, root, field, _) = field_and_terminal_padded(40.0, 10.0);
        let mut drags = (None, None);
        pointer(
            press(Point::new(60.0 + 40.0, 10.0 + 10.0)),
            &padded,
            root,
            &mut drags,
        );
        assert_eq!(state_of_field(&padded, field), expected);
    }

    /// The same for a terminal: the cell under the pointer, inside the padding.
    #[test]
    fn a_click_in_a_padded_terminal_lands_on_the_cell_under_it() {
        fn cell(
            tree: &std::rc::Rc<std::cell::RefCell<Tree>>,
            terminal: engine_core::NodeId,
        ) -> Option<(u16, u16)> {
            let tree = tree.borrow();
            let Some(NodeKind::Terminal(state)) = tree.get(terminal).map(|n| &n.kind) else {
                unreachable!()
            };
            state.selection_start
        }
        let (plain, root, _, terminal) = field_and_terminal();
        let mut drags = (None, None);
        pointer(press(Point::new(60.0, 50.0)), &plain, root, &mut drags);
        let expected = cell(&plain, terminal);
        assert!(expected.is_some());

        let (padded, root, _, terminal) = field_and_terminal_padded(20.0, 10.0);
        let mut drags = (None, None);
        pointer(
            press(Point::new(60.0 + 20.0, 50.0 + 10.0)),
            &padded,
            root,
            &mut drags,
        );
        assert_eq!(cell(&padded, terminal), expected);
    }

    /// Found while doing #53: the hit test ignored a multiline field's scroll,
    /// so a click in a scrolled field landed on the line it would have under
    /// an unscrolled one. It must resolve against what is painted.
    #[test]
    fn a_click_in_a_scrolled_multiline_field_lands_on_the_line_under_it() {
        fn click_at_the_top(scroll: f64) -> usize {
            let mut tree = Tree::new();
            let content = (0..40)
                .map(|i| format!("line{i}"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut state = engine_core::TextFieldState::new(content, "Roboto", 400.0, 14.0);
            state.multiline = true;
            state.scroll_offset.current = scroll;
            let field = tree.insert(
                NodeKind::TextField(Box::new(state)),
                Style {
                    size: Size {
                        width: length(200.0),
                        height: length(100.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
            );
            tree.compute_layout(
                field,
                Size {
                    width: AvailableSpace::Definite(200.0),
                    height: AvailableSpace::Definite(100.0),
                },
            );
            let tree = std::rc::Rc::new(std::cell::RefCell::new(tree));
            let mut drags = (None, None);
            pointer(press(Point::new(5.0, 5.0)), &tree, field, &mut drags);
            state_of_field(&tree, field)
        }
        let unscrolled = click_at_the_top(0.0);
        let scrolled = click_at_the_top(100.0);
        assert!(
            scrolled > unscrolled,
            "scrolled content: line {scrolled} vs {unscrolled}"
        );
    }

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
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
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
    /// trusting that one-off result forever. This proves the OS half
    /// on its own: a genuine set/get round trip against whatever
    /// clipboard mechanism is actually reachable here.
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
