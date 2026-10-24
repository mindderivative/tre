//! 0.5.7 (#198): layer surfaces (`zwlr_layer_shell_v1`) beside `winit`'s windows.
//!
//! `winit` has no layer-shell role, so a layer surface is made on a second event
//! queue of the Wayland connection `winit` already owns (`design/layer-surface-spike.md`
//! shows it works and why). A helper thread owns that queue and everything Wayland
//! about the surfaces: it dispatches the queue, answers the compositor, translates
//! pointer, touch and keyboard events to `engine_core::InputEvent`, and reports what
//! happened as [`LayerEvent`]s through a callback the event loop turns into a
//! `PlatformEvent`. The loop thread draws: it builds the `wgpu` surface from the raw
//! handles in [`LayerEvent::Created`] and sends requests back through a
//! [`LayerController`].
//!
//! Both seats (this one's and `winit`'s) are sent every surface's events by the
//! compositor, so every handler here first checks the event's surface is one of ours.

use std::collections::HashMap;
use std::ffi::c_void;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::ptr::NonNull;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use engine_core::{InputEvent, PointerButton, ScrollDelta, TouchPhase, ctrl_shortcut};
use peniko::kurbo::Point;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::seat::relative_pointer::{
    RelativeMotionEvent, RelativePointerHandler, RelativePointerState,
};
use smithay_client_toolkit::reexports::protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::{GlobalList, registry_queue_init};
use smithay_client_toolkit::reexports::client::protocol::{
    wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface, wl_touch,
};
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, backend::ObjectId,
};
use smithay_client_toolkit::reexports::protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::touch::TouchHandler;
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use crate::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, LayerData, LayerShell, LayerSurface,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, ZwlrLayerSurfaceV1,
};
use smithay_client_toolkit::{
    delegate_compositor, delegate_output, delegate_pointer,
    delegate_registry, delegate_relative_pointer, delegate_seat, delegate_touch, registry_handlers,
};
use wayland_backend::client::Backend;
use winit::window::CursorIcon;

use crate::layer_config::{Anchors, KeyboardMode, LayerConfig, LayerKind};
use crate::layer_keys::{self, Keymap};
use crate::outputs::{OutputEvent, OutputInfo, effective_scale, transform_name};

/// A pointer to the process's `wl_display`, or to one of its `wl_surface`s: what a
/// `wgpu` surface is made from. Moved between threads, used on none but to build one.
#[derive(Clone, Copy, Debug)]
pub struct RawPtr(pub NonNull<c_void>);

// SAFETY: a pointer value; the owner of the object it names keeps it alive.
unsafe impl Send for RawPtr {}
unsafe impl Sync for RawPtr {}

/// A request to open a layer surface: what `engine-py` hands the loop.
pub struct LayerRequest {
    /// Which registered window this is; handed back in the hooks.
    pub token: u64,
    pub config: LayerConfig,
    /// Auto-exit after this many frames (a self-asserting demo or test), as a window's.
    pub max_frames: Option<u32>,
}

/// What a host needs to draw a layer surface once the compositor has sized it.
pub struct LayerSurfaceInfo {
    pub id: u64,
    pub display: RawPtr,
    pub surface: RawPtr,
    /// The buffer's size in physical pixels (`logical * scale`).
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    /// For `update`, `set_cursor` and `destroy`.
    pub controller: LayerController,
}

/// Something that happened to a layer surface that is not input or a frame.
#[derive(Debug)]
pub enum LayerNotice {
    Closed(CloseReason),
    OutputChanged(String),
    /// It could not be made.
    Failed(String),
    /// A margin drag ended: the final `(top, right, bottom, left)`.
    MarginMoved((i32, i32, i32, i32)),
    /// A setting the compositor cannot take: `(key, message)`.
    Warning(&'static str, String),
}

/// The host's side of the loop for layer surfaces.
pub struct LayerHooks {
    /// The surface has a size: build the GPU side from the info. `false` abandons it.
    pub on_created: Box<dyn FnMut(winit::window::WindowId, u64, LayerSurfaceInfo) -> bool>,
    /// `(id, token, notice)`.
    pub on_notice: Box<dyn FnMut(winit::window::WindowId, u64, LayerNotice)>,
    /// An output appeared, changed or went (only when `watch_outputs` is set, or a layer
    /// window has made the layer thread start).
    pub on_output: Box<dyn FnMut(OutputEvent)>,
    /// Start the layer thread at once, to report outputs, even with no layer window.
    pub watch_outputs: bool,
    /// The loop is ending: drop every GPU surface made from a layer surface now, while
    /// the Wayland connection is still alive (it is gone once the loop returns).
    pub on_shutdown: Box<dyn FnMut()>,
}

/// Why a layer surface ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The compositor closed it.
    Compositor,
    /// The output it was bound to went away.
    OutputRemoved,
    /// The app closed it.
    App,
}

/// What the layer thread tells the loop.
#[derive(Debug)]
pub enum LayerEvent {
    /// The surface exists: build its `wgpu` surface from `display` and `surface`.
    /// Sent once, before the first [`Configure`](Self::Configure).
    Created {
        id: u64,
        display: RawPtr,
        surface: RawPtr,
    },
    /// The compositor assigned a size (`width` x `height`, logical pixels) and the
    /// surface's scale is `scale`: the buffer is `width * scale` by `height * scale`.
    Configure {
        id: u64,
        width: u32,
        height: u32,
        scale: f64,
    },
    /// The surface's scale changed with the size staying the same.
    Rescale {
        id: u64,
        width: u32,
        height: u32,
        scale: f64,
    },
    /// The surface is gone.
    Closed { id: u64, reason: CloseReason },
    /// The surface could not be made (an output name that matches nothing, say).
    Failed { id: u64, message: String },
    /// Input for the surface, in physical pixels like the `winit` path's.
    Input { id: u64, event: InputEvent },
    /// The surface entered the output with this connector name.
    OutputChanged { id: u64, name: String },
    /// An output appeared, changed or went away (not sent for the ones that existed when
    /// the layer thread started).
    Output(OutputEvent),
    /// A margin drag the engine ran ended; `margin` is `(top, right, bottom, left)`.
    MarginMoved {
        id: u64,
        margin: (i32, i32, i32, i32),
    },
    /// A setting this compositor cannot take (an older layer shell): the surface opens anyway.
    Warning {
        id: u64,
        key: &'static str,
        message: String,
    },
}

/// Why the layer side could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerError {
    /// Not a Wayland session, or the compositor has no layer shell.
    Unsupported(String),
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for LayerError {}

enum Command {
    Create(u64, LayerConfig),
    Update(u64, LayerConfig),
    Cursor(u64, CursorIcon),
    InputRegion(u64, Option<Vec<crate::layer_config::Rect>>),
    Margin(u64, (i32, i32, i32, i32)),
    BeginMarginDrag(u64),
    Destroy(u64),
    Shutdown,
}

/// A handle that sends requests to the layer thread; cheap to clone.
#[derive(Clone)]
pub struct LayerController {
    commands: Sender<Command>,
    wake: Arc<Mutex<UnixStream>>,
}

impl LayerController {
    fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            // One byte wakes the layer thread out of its poll.
            let _ = self.wake.lock().map(|mut w| w.write_all(&[1]));
        }
    }

    /// Makes the surface `id` from `config`; its events follow.
    pub fn create(&self, id: u64, config: LayerConfig) {
        self.send(Command::Create(id, config));
    }

    /// Applies `config` to the surface `id` in one commit.
    pub fn update(&self, id: u64, config: LayerConfig) {
        self.send(Command::Update(id, config));
    }

    /// Sets the pointer's shape over the surface.
    pub fn set_cursor(&self, id: u64, icon: CursorIcon) {
        self.send(Command::Cursor(id, icon));
    }

    /// Sets only the input region (`None`: the whole surface) and applies it at once, with
    /// a commit of its own: an idle surface draws no frame to carry it.
    pub fn set_input_region(&self, id: u64, region: Option<Vec<crate::layer_config::Rect>>) {
        self.send(Command::InputRegion(id, region));
    }

    /// Sets only the margin `(top, right, bottom, left)` and commits it at once: one cheap
    /// commit per animation step, with no other property re-sent.
    pub fn set_margin(&self, id: u64, margin: (i32, i32, i32, i32)) {
        self.send(Command::Margin(id, margin));
    }

    /// Starts moving the surface by its margins with the pointer, which must be pressed on
    /// it: the layer thread follows the pointer and ends the drag on release (then sends
    /// `MarginMoved`). A surface that cannot move on either axis ignores this.
    pub fn begin_margin_drag(&self, id: u64) {
        self.send(Command::BeginMarginDrag(id));
    }

    /// Destroys the surface; the `wgpu` surface must be dropped first.
    pub fn destroy(&self, id: u64) {
        self.send(Command::Destroy(id));
    }
}

/// The layer thread and its connection.
pub struct LayerHost {
    controller: LayerController,
    display: RawPtr,
    thread: Option<JoinHandle<()>>,
}

impl LayerHost {
    /// Starts the layer thread on `winit`'s `wl_display`. `on_event` runs on that
    /// thread for every event; it returns `false` once the loop has ended.
    pub fn start(
        display: NonNull<c_void>,
        on_event: impl Fn(LayerEvent) -> bool + Send + 'static,
    ) -> Result<Self, LayerError> {
        let (commands, command_rx) = mpsc::channel();
        let (wake_tx, wake_rx) = UnixStream::pair()
            .map_err(|e| LayerError::Unsupported(format!("no wake pipe: {e}")))?;
        wake_rx.set_nonblocking(true).ok();
        let (ready_tx, ready_rx) = mpsc::channel();
        let display_ptr = RawPtr(display);
        let thread = thread::Builder::new()
            .name("tre-layer".into())
            .spawn(move || run(display_ptr, on_event, command_rx, wake_rx, ready_tx))
            .map_err(|e| LayerError::Unsupported(format!("no layer thread: {e}")))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                controller: LayerController {
                    commands,
                    wake: Arc::new(Mutex::new(wake_tx)),
                },
                display: display_ptr,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(LayerError::Unsupported(
                "the layer thread ended at start".into(),
            )),
        }
    }

    pub fn controller(&self) -> LayerController {
        self.controller.clone()
    }

    pub fn display(&self) -> RawPtr {
        self.display
    }
}

impl Drop for LayerHost {
    fn drop(&mut self) {
        self.controller.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Whether this session's compositor offers `zwlr_layer_shell_v1`: `false` off Wayland,
/// on X11, and on a compositor without it (GNOME's). Asked once, on a connection of its
/// own, and remembered.
pub fn supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return false;
        }
        let Ok(conn) = Connection::connect_to_env() else {
            return false;
        };
        let Ok((globals, _queue)) = registry_queue_init::<ProbeState>(&conn) else {
            return false;
        };
        has_layer_shell(&globals)
    })
}

struct ProbeState;

impl
    Dispatch<
        wayland_client_registry::WlRegistry,
        smithay_client_toolkit::reexports::client::globals::GlobalListContents,
    > for ProbeState
{
    fn event(
        _: &mut Self,
        _: &wayland_client_registry::WlRegistry,
        _: <wayland_client_registry::WlRegistry as Proxy>::Event,
        _: &smithay_client_toolkit::reexports::client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

use smithay_client_toolkit::reexports::client::protocol::wl_registry as wayland_client_registry;

fn has_layer_shell(globals: &GlobalList) -> bool {
    globals
        .contents()
        .with_list(|list| list.iter().any(|g| g.interface == "zwlr_layer_shell_v1"))
}

fn transform_value(transform: wl_output::Transform) -> u32 {
    use wl_output::Transform as T;
    match transform {
        T::Normal => 0,
        T::_90 => 1,
        T::_180 => 2,
        T::_270 => 3,
        T::Flipped => 4,
        T::Flipped90 => 5,
        T::Flipped180 => 6,
        T::Flipped270 => 7,
        _ => 0,
    }
}

/// One output as plain data; `None` until the compositor has told us its name.
fn output_info(outputs: &OutputState, output: &wl_output::WlOutput) -> Option<OutputInfo> {
    let info = outputs.info(output)?;
    let name = info.name.clone()?;
    let transform = transform_name(transform_value(info.transform));
    let (x, y) = info.logical_position.unwrap_or(info.location);
    let mode = info.modes.iter().find(|m| m.current).map(|m| m.dimensions);
    let (width, height) = info
        .logical_size
        .or(mode)
        .map_or((0, 0), |(w, h)| (w.max(0) as u32, h.max(0) as u32));
    Some(OutputInfo {
        name,
        description: info.description.clone().unwrap_or_default(),
        x,
        y,
        width,
        height,
        scale_factor: effective_scale(mode, info.logical_size, transform, info.scale_factor),
        transform: transform.to_string(),
    })
}

/// The outputs this session's compositor has, read now on a connection of its own, in
/// order of position (top to bottom, left to right). Empty off Wayland, and where the
/// compositor does not report names (`wl_output` before version 4, with no xdg-output).
pub fn outputs() -> Vec<OutputInfo> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Vec::new();
    }
    let Ok(conn) = Connection::connect_to_env() else {
        return Vec::new();
    };
    let Ok((globals, mut queue)) = registry_queue_init::<OutputProbe>(&conn) else {
        return Vec::new();
    };
    let qh = queue.handle();
    let mut probe = OutputProbe {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
    };
    // The globals, then each output's events (and its xdg-output's).
    let _ = queue.roundtrip(&mut probe);
    let _ = queue.roundtrip(&mut probe);
    let mut found: Vec<OutputInfo> = probe
        .outputs
        .outputs()
        .filter_map(|o| output_info(&probe.outputs, &o))
        .collect();
    found.sort_by(|a, b| (a.y, a.x, &a.name).cmp(&(b.y, b.x, &b.name)));
    found
}

/// A throwaway state to read the outputs with.
struct OutputProbe {
    registry: RegistryState,
    outputs: OutputState,
}

impl OutputHandler for OutputProbe {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for OutputProbe {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}

delegate_output!(OutputProbe);
delegate_registry!(OutputProbe);

// ---- the layer thread ----

/// Local user data for the objects this module dispatches itself, so the impls do not
/// overlap the toolkit's.
struct FracData(u64);
struct Plain;

struct Surf {
    layer: LayerSurface,
    config: LayerConfig,
    surface: wl_surface::WlSurface,
    viewport: Option<WpViewport>,
    fractional: Option<WpFractionalScaleV1>,
    output: Option<wl_output::WlOutput>,
    /// The compositor's last size, in logical pixels.
    size: (u32, u32),
    scale: f64,
    announced: bool,
    /// The app has been told this compositor cannot take `exclusive_edge`.
    edge_warned: bool,
    /// The shape the app last asked for, applied whenever the pointer enters.
    cursor: CursorIcon,
}

struct Repeat {
    code: u32,
    next: Instant,
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    fractional_manager: Option<WpFractionalScaleManagerV1>,
    viewporter: Option<WpViewporter>,
    cursor_manager: Option<WpCursorShapeManagerV1>,
    cursor_device: Option<WpCursorShapeDeviceV1>,
    on_event: Box<dyn Fn(LayerEvent) -> bool + Send>,
    display: RawPtr,
    surfaces: HashMap<u64, Surf>,
    by_surface: HashMap<ObjectId, u64>,
    pointer_enter_serial: u32,
    pointer_focus: Option<u64>,
    keyboard_focus: Option<u64>,
    xkb: Option<Keymap>,
    repeat: Option<Repeat>,
    repeat_delay: Duration,
    repeat_interval: Option<Duration>,
    touch_positions: HashMap<i32, Point>,
    /// The outputs as last reported, so a removal can say which one went.
    known_outputs: HashMap<ObjectId, OutputInfo>,
    /// Set once the initial roundtrips are done: outputs that exist at start are not news.
    outputs_ready: bool,
    /// Surfaces the compositor has closed whose reason is not yet known: a compositor
    /// sends `closed` before it removes the output, so the reason is settled after a
    /// roundtrip (see `settle_closed`).
    closing: Vec<u64>,
    relative: RelativePointerState,
    relative_pointer: Option<ZwpRelativePointerV1>,
    /// The surface-local position of the pointer, logical pixels, from its last event.
    pointer_local: (f64, f64),
    drag: Option<MarginDrag>,
    alive: bool,
}

/// A margin drag the layer thread is running: the engine moves the surface with the
/// pointer so Python is never in the loop.
struct MarginDrag {
    id: u64,
    start: (i32, i32, i32, i32),
    /// Where the pointer was when the drag began, surface-local.
    press_local: (f64, f64),
    /// How far the pointer has moved since, logical pixels.
    moved: (f64, f64),
    /// The margin last sent.
    current: (i32, i32, i32, i32),
    /// Whether relative motion has arrived during this drag. An absolute pointer (a tablet,
    /// a virtual machine's, a remote desktop's) sends none, and then the drag follows the
    /// surface-local position instead.
    relative_seen: bool,
}

impl State {
    /// An output's data arrived or changed: report it once it has a name and a change.
    fn output_seen(&mut self, output: &wl_output::WlOutput, new: bool) {
        let Some(info) = output_info(&self.outputs, output) else {
            return;
        };
        let previous = self.known_outputs.insert(output.id(), info.clone());
        if !self.outputs_ready || previous.as_ref() == Some(&info) {
            return;
        }
        let _ = new;
        let event = if previous.is_none() {
            OutputEvent::Added(info)
        } else {
            OutputEvent::Updated(info)
        };
        self.emit(LayerEvent::Output(event));
    }

    fn emit(&mut self, event: LayerEvent) {
        if !(self.on_event)(event) {
            self.alive = false;
        }
    }

    fn input(&mut self, id: u64, event: InputEvent) {
        self.emit(LayerEvent::Input { id, event });
    }

    fn id_of(&self, surface: &wl_surface::WlSurface) -> Option<u64> {
        self.by_surface.get(&surface.id()).copied()
    }

    fn scale_of(&self, id: u64) -> f64 {
        self.surfaces.get(&id).map_or(1.0, |s| s.scale)
    }

    /// A position in surface-local logical pixels, as physical pixels.
    fn physical(&self, id: u64, position: (f64, f64)) -> Point {
        let scale = self.scale_of(id);
        Point::new(position.0 * scale, position.1 * scale)
    }

    fn apply(&mut self, id: u64, config: &LayerConfig) {
        let Some(surf) = self.surfaces.get_mut(&id) else {
            return;
        };
        let layer = &surf.layer;
        layer.set_layer(match config.layer {
            LayerKind::Background => Layer::Background,
            LayerKind::Bottom => Layer::Bottom,
            LayerKind::Top => Layer::Top,
            LayerKind::Overlay => Layer::Overlay,
        });
        layer.set_anchor(anchor_flags(config.anchor.0));
        layer.set_size(config.size.0, config.size.1);
        layer.set_exclusive_zone(config.exclusive_zone);
        layer.set_margin(
            config.margin.0,
            config.margin.1,
            config.margin.2,
            config.margin.3,
        );
        // The edge a corner's zone is for. The protocol cannot clear it once set, and the
        // app is told, once, when the compositor is too old to take it (layer-shell v4).
        let mut warn_edge = false;
        if let Some(edge) = config.exclusive_edge
            && !layer.set_exclusive_edge(anchor_flags(edge))
            && !surf.edge_warned
        {
            surf.edge_warned = true;
            warn_edge = true;
        }
        layer.set_keyboard_interactivity(match config.keyboard {
            KeyboardMode::None => KeyboardInteractivity::None,
            KeyboardMode::OnDemand => KeyboardInteractivity::OnDemand,
            KeyboardMode::Exclusive => KeyboardInteractivity::Exclusive,
        });
        set_region(&self.compositor, &surf.surface, &config.input_region);
        surf.config = config.clone();
        layer.commit();
        if warn_edge {
            self.emit(LayerEvent::Warning {
                id,
                key: "exclusive_edge",
                message: "this compositor offers zwlr_layer_shell_v1 below version 5, which \
                          has no exclusive edge: the setting is ignored"
                    .into(),
            });
        }
    }

    /// Emits `Closed` for each surface the compositor closed. A surface bound to an output
    /// that is no longer there was closed because of it, and that is only known once the
    /// removal has been read, so the caller roundtrips first.
    fn settle_closed(&mut self) {
        for id in std::mem::take(&mut self.closing) {
            let output_gone = self.surfaces.get(&id).is_some_and(|s| {
                s.output
                    .as_ref()
                    .is_some_and(|o| self.outputs.info(o).is_none())
            });
            let reason = if output_gone {
                CloseReason::OutputRemoved
            } else {
                CloseReason::Compositor
            };
            // Not destroyed here: the loop drops its `wgpu` surface first and then asks
            // for `Destroy`, so the `wl_surface` is never gone while a swapchain uses it.
            self.emit(LayerEvent::Closed { id, reason });
        }
    }

    fn create(&mut self, qh: &QueueHandle<Self>, id: u64, config: LayerConfig) {
        let output = match &config.output {
            None => {
                // Not the compositor's choice: it alternates between runs.
                let all: Vec<_> = self.outputs.outputs().collect();
                let infos: Vec<(String, i32, i32)> = all
                    .iter()
                    .filter_map(|o| output_info(&self.outputs, o).map(|i| (i.name, i.x, i.y)))
                    .collect();
                if infos.len() == all.len() {
                    crate::outputs::default_output(&infos).map(|i| all[i].clone())
                } else {
                    None
                }
            }
            Some(name) => {
                let found = self
                    .outputs
                    .outputs()
                    .find(|o| self.outputs.info(o).and_then(|i| i.name).as_deref() == Some(name));
                match found {
                    Some(output) => Some(output),
                    None => {
                        let known: Vec<String> = self
                            .outputs
                            .outputs()
                            .filter_map(|o| self.outputs.info(&o).and_then(|i| i.name))
                            .collect();
                        self.emit(LayerEvent::Failed {
                            id,
                            message: format!(
                                "no output named {name:?} (the compositor has: {})",
                                known.join(", ")
                            ),
                        });
                        return;
                    }
                }
            }
        };
        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            &surface,
            Layer::Top,
            &config.namespace,
            output.as_ref(),
        );
        let fractional = self
            .fractional_manager
            .as_ref()
            .map(|m| m.get_fractional_scale(&surface, qh, FracData(id)));
        let viewport = self
            .viewporter
            .as_ref()
            .map(|v| v.get_viewport(&surface, qh, Plain));
        self.by_surface.insert(surface.id(), id);
        self.surfaces.insert(
            id,
            Surf {
                layer,
                config: config.clone(),
                surface: surface.clone(),
                viewport,
                fractional,
                output,
                size: (0, 0),
                scale: 1.0,
                announced: false,
                edge_warned: false,
                cursor: CursorIcon::Default,
            },
        );
        self.apply(id, &config);
        self.emit(LayerEvent::Created {
            id,
            display: self.display,
            surface: RawPtr(
                NonNull::new(surface.id().as_ptr().cast::<c_void>()).expect("a live wl_surface"),
            ),
        });
    }

    fn destroy(&mut self, id: u64) {
        if self.drag.as_ref().is_some_and(|d| d.id == id) {
            self.drag = None;
        }
        if let Some(surf) = self.surfaces.remove(&id) {
            self.by_surface.remove(&surf.surface.id());
            if self.pointer_focus == Some(id) {
                self.pointer_focus = None;
            }
            if self.keyboard_focus == Some(id) {
                self.keyboard_focus = None;
                self.repeat = None;
            }
            if let Some(viewport) = surf.viewport {
                viewport.destroy();
            }
            if let Some(fractional) = surf.fractional {
                fractional.destroy();
            }
            drop(surf.layer);
            surf.surface.destroy();
        }
    }

    /// Tells the loop the size and scale, and sets the viewport so the buffer (physical
    /// pixels) covers the logical size.
    fn announce(&mut self, id: u64, resized: bool) {
        let Some(surf) = self.surfaces.get_mut(&id) else {
            return;
        };
        let (width, height) = surf.size;
        if width == 0 || height == 0 {
            return;
        }
        if let Some(viewport) = &surf.viewport {
            viewport.set_destination(width as i32, height as i32);
        } else if surf.scale.fract() == 0.0 && surf.scale >= 1.0 {
            surf.surface.set_buffer_scale(surf.scale as i32);
        }
        let scale = surf.scale;
        let first = !surf.announced;
        surf.announced = true;
        if resized || first {
            self.emit(LayerEvent::Configure {
                id,
                width,
                height,
                scale,
            });
        } else {
            self.emit(LayerEvent::Rescale {
                id,
                width,
                height,
                scale,
            });
        }
    }

    fn key(&mut self, code: u32, pressed: bool, repeat: bool) {
        let Some(id) = self.keyboard_focus else {
            return;
        };
        let Some(xkb) = &self.xkb else {
            return;
        };
        let keysym = xkb.keysym(code);
        let text = xkb.utf8(code);
        let modifiers = xkb.modifiers();
        let shift = modifiers.shift;
        let mut events = Vec::new();
        if let Some(name) = layer_keys::key_name(keysym, &text) {
            events.push(InputEvent::Key {
                name,
                pressed,
                repeat,
            });
        }
        if let Some(key) = layer_keys::engine_key(keysym) {
            events.push(if pressed {
                InputEvent::KeyPressed { key, shift }
            } else {
                InputEvent::KeyReleased { key, shift }
            });
        } else if pressed
            && modifiers.ctrl
            && let Some(letter) = ascii_letter(keysym)
            && let Some(event) = ctrl_shortcut(letter, shift)
        {
            events.push(event);
        } else if pressed && !text.is_empty() && !text.chars().any(char::is_control) {
            events.push(InputEvent::TextInput(text));
        }
        for event in events {
            self.input(id, event);
        }
    }
}

fn ascii_letter(keysym: u32) -> Option<char> {
    u8::try_from(keysym)
        .ok()
        .map(char::from)
        .filter(char::is_ascii_alphabetic)
}

fn button_of(code: u32) -> Option<PointerButton> {
    match code {
        0x110 => Some(PointerButton::Primary),
        0x111 => Some(PointerButton::Secondary),
        0x112 => Some(PointerButton::Middle),
        0x113 => Some(PointerButton::Back),
        0x114 => Some(PointerButton::Forward),
        _ => None,
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        // The integer scale only matters where the compositor has no fractional one.
        let Some(id) = self.id_of(surface) else {
            return;
        };
        if self.fractional_manager.is_some() {
            return;
        }
        if let Some(surf) = self.surfaces.get_mut(&id) {
            surf.scale = f64::from(factor.max(1));
        }
        self.announce(id, false);
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        let Some(id) = self.id_of(surface) else {
            return;
        };
        if let Some(name) = self.outputs.info(output).and_then(|i| i.name) {
            self.emit(LayerEvent::OutputChanged { id, name });
        }
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.output_seen(&output, true);
    }
    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.output_seen(&output, false);
    }
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.known_outputs.remove(&output.id())
            && self.outputs_ready
        {
            self.emit(LayerEvent::Output(OutputEvent::Removed(info)));
        }
    }
}

/// Sets the surface's input region from rectangles (`None`: the whole surface). Double
/// buffered like the rest of the surface's state: it takes effect at the next commit.
fn set_region(
    compositor: &CompositorState,
    surface: &wl_surface::WlSurface,
    rects: &Option<Vec<crate::layer_config::Rect>>,
) {
    match rects {
        None => surface.set_input_region(None),
        Some(rects) => {
            // If the compositor refuses a region object, keep what is set.
            if let Ok(region) = Region::new(compositor) {
                for &(x, y, w, h) in rects {
                    region.add(x, y, w, h);
                }
                surface.set_input_region(Some(region.wl_region()));
            }
        }
    }
}

impl State {
    /// The output the surface is on, as a logical `(width, height)`.
    fn output_size(&self, id: u64) -> Option<(i32, i32)> {
        let surf = self.surfaces.get(&id)?;
        let output = surf.output.as_ref()?;
        let info = self.outputs.info(output)?;
        let (w, h) = info
            .logical_size
            .or_else(|| info.modes.iter().find(|m| m.current).map(|m| m.dimensions))?;
        Some((w, h))
    }

    /// Starts a drag of `id` from the pointer's current position, if the pointer is on it
    /// and it can move on at least one axis (anchored to one edge of that axis only).
    fn begin_margin_drag(&mut self, id: u64) {
        let Some(surf) = self.surfaces.get(&id) else {
            return;
        };
        let a = surf.config.anchor;
        let movable = a.contains(Anchors::LEFT) != a.contains(Anchors::RIGHT)
            || a.contains(Anchors::TOP) != a.contains(Anchors::BOTTOM);
        if self.pointer_focus != Some(id) || !movable || self.output_size(id).is_none() {
            return;
        }
        let start = surf.config.margin;
        self.drag = Some(MarginDrag {
            id,
            start,
            press_local: self.pointer_local,
            moved: (0.0, 0.0),
            current: start,
            relative_seen: false,
        });
    }

    /// The margins for a drag, from the pure [`drag_margins`].
    fn dragged_margin(&self, drag: &MarginDrag) -> Option<(i32, i32, i32, i32)> {
        let surf = self.surfaces.get(&drag.id)?;
        let (out_w, out_h) = self.output_size(drag.id)?;
        Some(drag_margins(
            surf.config.anchor,
            drag.start,
            drag.moved,
            (out_w, out_h),
            (surf.size.0 as i32, surf.size.1 as i32),
        ))
    }

    /// Sends the margin if the drag moved it: one commit.
    fn apply_drag(&mut self) {
        let Some(drag) = &self.drag else {
            return;
        };
        let Some(margin) = self.dragged_margin(drag) else {
            return;
        };
        if margin == drag.current {
            return;
        }
        let id = drag.id;
        if let Some(surf) = self.surfaces.get_mut(&id) {
            surf.layer
                .set_margin(margin.0, margin.1, margin.2, margin.3);
            surf.config.margin = margin;
            surf.layer.commit();
        }
        if let Some(drag) = &mut self.drag {
            drag.current = margin;
        }
    }

    /// Without a relative-motion protocol: the drag follows the surface-local position.
    /// The surface moves under the pointer, so the offset already applied is added to
    /// the local delta; a commit not yet applied makes this overshoot a little, which
    /// the relative protocol avoids.
    fn drag_follow_local(&mut self, id: u64) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        if drag.id != id {
            return;
        }
        let applied = (
            f64::from(drag.current.3 - drag.start.3) + f64::from(drag.start.1 - drag.current.1),
            f64::from(drag.current.0 - drag.start.0) + f64::from(drag.start.2 - drag.current.2),
        );
        drag.moved = (
            applied.0 + (self.pointer_local.0 - drag.press_local.0),
            applied.1 + (self.pointer_local.1 - drag.press_local.1),
        );
        self.apply_drag();
    }

    /// The primary button went up on `id`: the drag is over; tell the engine the margin.
    fn end_margin_drag(&mut self, id: u64) {
        if self.drag.as_ref().is_none_or(|d| d.id != id) {
            return;
        }
        if let Some(drag) = self.drag.take() {
            self.emit(LayerEvent::MarginMoved {
                id,
                margin: drag.current,
            });
        }
    }
}

/// The margins `(top, right, bottom, left)` for a drag that has moved the pointer `moved`
/// logical pixels from where `start` was: the anchored edges follow the pointer, clamped so
/// the surface stays inside the output (`output` and `size` are logical). An axis anchored
/// to both edges, or to none, does not move.
pub(crate) fn drag_margins(
    anchor: Anchors,
    start: (i32, i32, i32, i32),
    moved: (f64, f64),
    output: (i32, i32),
    size: (i32, i32),
) -> (i32, i32, i32, i32) {
    let (dx, dy) = (moved.0.round() as i32, moved.1.round() as i32);
    let mut m = start;
    let (left, right) = (
        anchor.contains(Anchors::LEFT),
        anchor.contains(Anchors::RIGHT),
    );
    let (top, bottom) = (
        anchor.contains(Anchors::TOP),
        anchor.contains(Anchors::BOTTOM),
    );
    let room_x = (output.0 - size.0).max(0);
    let room_y = (output.1 - size.1).max(0);
    if left && !right {
        m.3 = (start.3 + dx).clamp(0, room_x);
    } else if right && !left {
        m.1 = (start.1 - dx).clamp(0, room_x);
    }
    if top && !bottom {
        m.0 = (start.0 + dy).clamp(0, room_y);
    } else if bottom && !top {
        m.2 = (start.2 - dy).clamp(0, room_y);
    }
    m
}

impl RelativePointerHandler for State {
    fn relative_pointer_motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &ZwpRelativePointerV1,
        _: &wl_pointer::WlPointer,
        event: RelativeMotionEvent,
    ) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        if self.pointer_focus != Some(drag.id) {
            return;
        }
        if !drag.relative_seen {
            // From here the exact deltas are the truth; restart from what the local
            // fallback had worked out, so the hand-over does not jump.
            drag.relative_seen = true;
        }
        drag.moved.0 += event.delta.0;
        drag.moved.1 += event.delta.1;
        self.apply_drag();
    }
}

impl State {
    fn id_of_layer_surface(&self, surface: &wl_surface::WlSurface) -> Option<u64> {
        self.surfaces
            .iter()
            .find(|(_, s)| s.layer.wl_surface() == surface)
            .map(|(id, _)| *id)
    }

    /// The compositor closed the surface (an output went away, say).
    fn layer_closed(&mut self, surface: &wl_surface::WlSurface) {
        let Some(id) = self.id_of_layer_surface(surface) else {
            return;
        };
        // The reason waits for the output's fate: `closed` comes before the removal.
        self.closing.push(id);
    }

    /// The compositor sized the surface (`0` on an axis: its choice, so the size it asked for).
    fn layer_configured(&mut self, surface: &wl_surface::WlSurface, width: u32, height: u32) {
        let Some(id) = self.id_of_layer_surface(surface) else {
            return;
        };
        if let Some(surf) = self.surfaces.get_mut(&id) {
            surf.size = (
                if width == 0 {
                    surf.config.size.0
                } else {
                    width
                },
                if height == 0 {
                    surf.config.size.1
                } else {
                    height
                },
            );
        }
        self.announce(id, true);
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: <ZwlrLayerShellV1 as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The layer shell global has no events.
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, LayerData> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        data: &LayerData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                // Acknowledged before the app hears of it, as the protocol needs.
                proxy.ack_configure(serial);
                state.layer_configured(&data.surface, width, height);
            }
            zwlr_layer_surface_v1::Event::Closed => state.layer_closed(&data.surface),
            _ => {}
        }
    }
}

/// `Anchors` bits (this crate's) as the protocol's anchor flags.
fn anchor_flags(bits: u8) -> Anchor {
    let mut anchor = Anchor::empty();
    for (bit, flag) in [
        (Anchors::TOP, Anchor::Top),
        (Anchors::BOTTOM, Anchor::Bottom),
        (Anchors::LEFT, Anchor::Left),
        (Anchors::RIGHT, Anchor::Right),
    ] {
        if bits & bit == bit {
            anchor |= flag;
        }
    }
    anchor
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer => {
                if let Ok(pointer) = self.seats.get_pointer(qh, &seat) {
                    self.relative_pointer = self.relative.get_relative_pointer(&pointer, qh).ok();
                    self.cursor_device = self
                        .cursor_manager
                        .as_ref()
                        .map(|m| m.get_pointer(&pointer, qh, Plain));
                }
            }
            Capability::Keyboard => {
                seat.get_keyboard(qh, Plain);
            }
            Capability::Touch => {
                let _ = self.seats.get_touch(qh, &seat);
            }
            _ => {}
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            self.cursor_device = None;
            self.relative_pointer = None;
            self.drag = None;
            self.pointer_focus = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let Some(id) = self.id_of(&event.surface) else {
                continue;
            };
            let position = self.physical(id, event.position);
            self.pointer_local = event.position;
            match event.kind {
                PointerEventKind::Enter { serial } => {
                    self.pointer_enter_serial = serial;
                    self.pointer_focus = Some(id);
                    // The compositor forgets the cursor on entry: set the app's again.
                    if let (Some(device), Some(surf)) =
                        (&self.cursor_device, self.surfaces.get(&id))
                    {
                        device.set_shape(serial, shape_of(surf.cursor));
                    }
                    self.input(id, InputEvent::PointerMoved { position });
                }
                PointerEventKind::Leave { .. } => {
                    if self.pointer_focus == Some(id) {
                        self.pointer_focus = None;
                    }
                    self.input(id, InputEvent::PointerLeft);
                }
                PointerEventKind::Motion { .. } => {
                    // Without relative motion (no protocol, or an absolute pointer that sends
                    // none) the drag follows the surface-local position; once relative
                    // motion arrives, `relative_pointer_motion` does.
                    if self.drag.as_ref().is_some_and(|d| !d.relative_seen) {
                        self.drag_follow_local(id);
                    }
                    self.input(id, InputEvent::PointerMoved { position });
                }
                PointerEventKind::Press { button, .. } => {
                    if let Some(button) = button_of(button) {
                        self.input(id, InputEvent::PointerPressed { position, button });
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if button_of(button) == Some(PointerButton::Primary) {
                        self.end_margin_drag(id);
                    }
                    if let Some(button) = button_of(button) {
                        self.input(id, InputEvent::PointerReleased { position, button });
                    }
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    // The protocol counts down and right as positive; the engine (as
                    // `winit`) counts up and left.
                    let delta = if horizontal.discrete != 0 || vertical.discrete != 0 {
                        ScrollDelta::Lines(
                            -f64::from(horizontal.discrete),
                            -f64::from(vertical.discrete),
                        )
                    } else {
                        let scale = self.scale_of(id);
                        ScrollDelta::Pixels(
                            -horizontal.absolute * scale,
                            -vertical.absolute * scale,
                        )
                    };
                    self.input(id, InputEvent::Scroll { delta, position });
                }
            }
        }
    }
}

impl TouchHandler for State {
    fn down(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        _: u32,
        surface: wl_surface::WlSurface,
        id: i32,
        position: (f64, f64),
    ) {
        let Some(surface_id) = self.id_of(&surface) else {
            return;
        };
        let point = self.physical(surface_id, position);
        self.touch_positions.insert(id, point);
        self.input(
            surface_id,
            InputEvent::Touch {
                id: id as u64,
                phase: TouchPhase::Started,
                position: point,
            },
        );
    }
    fn up(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        _: u32,
        id: i32,
    ) {
        // A touch belongs to the surface it landed on; one layer surface is the usual case.
        let point = self.touch_positions.remove(&id).unwrap_or(Point::ZERO);
        if let Some(surface_id) = self.surfaces.keys().next().copied() {
            self.input(
                surface_id,
                InputEvent::Touch {
                    id: id as u64,
                    phase: TouchPhase::Ended,
                    position: point,
                },
            );
        }
    }
    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        id: i32,
        position: (f64, f64),
    ) {
        if let Some(surface_id) = self.surfaces.keys().next().copied() {
            let point = self.physical(surface_id, position);
            self.touch_positions.insert(id, point);
            self.input(
                surface_id,
                InputEvent::Touch {
                    id: id as u64,
                    phase: TouchPhase::Moved,
                    position: point,
                },
            );
        }
    }
    fn shape(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: i32,
        _: f64,
        _: f64,
    ) {
    }
    fn orientation(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: i32,
        _: f64,
    ) {
    }
    fn cancel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_touch::WlTouch) {
        let positions: Vec<(i32, Point)> = self.touch_positions.drain().collect();
        if let Some(surface_id) = self.surfaces.keys().next().copied() {
            for (id, point) in positions {
                self.input(
                    surface_id,
                    InputEvent::Touch {
                        id: id as u64,
                        phase: TouchPhase::Cancelled,
                        position: point,
                    },
                );
            }
        }
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(State);
delegate_output!(State);
delegate_seat!(State);
delegate_pointer!(State);
delegate_relative_pointer!(State);
delegate_touch!(State);
delegate_registry!(State);

impl Dispatch<WpFractionalScaleManagerV1, Plain> for State {
    fn event(
        _: &mut Self,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as Proxy>::Event,
        _: &Plain,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WpViewporter, Plain> for State {
    fn event(
        _: &mut Self,
        _: &WpViewporter,
        _: <WpViewporter as Proxy>::Event,
        _: &Plain,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WpViewport, Plain> for State {
    fn event(
        _: &mut Self,
        _: &WpViewport,
        _: <WpViewport as Proxy>::Event,
        _: &Plain,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WpCursorShapeManagerV1, Plain> for State {
    fn event(
        _: &mut Self,
        _: &WpCursorShapeManagerV1,
        _: <WpCursorShapeManagerV1 as Proxy>::Event,
        _: &Plain,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WpCursorShapeDeviceV1, Plain> for State {
    fn event(
        _: &mut Self,
        _: &WpCursorShapeDeviceV1,
        _: <WpCursorShapeDeviceV1 as Proxy>::Event,
        _: &Plain,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpFractionalScaleV1, FracData> for State {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        data: &FracData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wp_fractional_scale_v1::Event::PreferredScale { scale } = event else {
            return;
        };
        let id = data.0;
        if let Some(surf) = state.surfaces.get_mut(&id) {
            surf.scale = f64::from(scale) / 120.0;
        }
        state.announce(id, false);
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, Plain> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &Plain,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    state.xkb = Keymap::from_fd(fd, size as usize);
                }
            }
            wl_keyboard::Event::Enter { surface, .. } => {
                if let Some(id) = state.id_of(&surface) {
                    state.keyboard_focus = Some(id);
                    state.input(id, InputEvent::Focused { focused: true });
                }
            }
            wl_keyboard::Event::Leave { surface, .. } => {
                if let Some(id) = state.id_of(&surface) {
                    if state.keyboard_focus == Some(id) {
                        state.keyboard_focus = None;
                    }
                    state.repeat = None;
                    state.input(id, InputEvent::Focused { focused: false });
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                let pressed = key_state == WEnum::Value(wl_keyboard::KeyState::Pressed);
                state.key(key, pressed, false);
                if pressed {
                    let repeats = state.xkb.as_ref().is_some_and(|x| x.repeats(key));
                    state.repeat = (repeats && state.repeat_interval.is_some()).then(|| Repeat {
                        code: key,
                        next: Instant::now() + state.repeat_delay,
                    });
                } else if state.repeat.as_ref().is_some_and(|r| r.code == key) {
                    state.repeat = None;
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(xkb) = &mut state.xkb {
                    xkb.update_mask(mods_depressed, mods_latched, mods_locked, group);
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                state.repeat_delay = Duration::from_millis(delay.max(0) as u64);
                state.repeat_interval =
                    (rate > 0).then(|| Duration::from_secs_f64(1.0 / f64::from(rate)));
            }
            _ => {}
        }
    }
}

/// The cursor shape protocol's name for a `winit` cursor.
fn shape_of(icon: CursorIcon) -> wp_cursor_shape_device_v1::Shape {
    use wp_cursor_shape_device_v1::Shape as S;
    match icon {
        CursorIcon::Default => S::Default,
        CursorIcon::ContextMenu => S::ContextMenu,
        CursorIcon::Help => S::Help,
        CursorIcon::Pointer => S::Pointer,
        CursorIcon::Progress => S::Progress,
        CursorIcon::Wait => S::Wait,
        CursorIcon::Cell => S::Cell,
        CursorIcon::Crosshair => S::Crosshair,
        CursorIcon::Text => S::Text,
        CursorIcon::VerticalText => S::VerticalText,
        CursorIcon::Alias => S::Alias,
        CursorIcon::Copy => S::Copy,
        CursorIcon::Move => S::Move,
        CursorIcon::NoDrop => S::NoDrop,
        CursorIcon::NotAllowed => S::NotAllowed,
        CursorIcon::Grab => S::Grab,
        CursorIcon::Grabbing => S::Grabbing,
        CursorIcon::EResize => S::EResize,
        CursorIcon::NResize => S::NResize,
        CursorIcon::NeResize => S::NeResize,
        CursorIcon::NwResize => S::NwResize,
        CursorIcon::SResize => S::SResize,
        CursorIcon::SeResize => S::SeResize,
        CursorIcon::SwResize => S::SwResize,
        CursorIcon::WResize => S::WResize,
        CursorIcon::EwResize => S::EwResize,
        CursorIcon::NsResize => S::NsResize,
        CursorIcon::NeswResize => S::NeswResize,
        CursorIcon::NwseResize => S::NwseResize,
        CursorIcon::ColResize => S::ColResize,
        CursorIcon::RowResize => S::RowResize,
        CursorIcon::AllScroll => S::AllScroll,
        CursorIcon::ZoomIn => S::ZoomIn,
        CursorIcon::ZoomOut => S::ZoomOut,
        _ => S::Default,
    }
}

fn run(
    display: RawPtr,
    on_event: impl Fn(LayerEvent) -> bool + Send + 'static,
    commands: Receiver<Command>,
    wake: UnixStream,
    ready: Sender<Result<(), LayerError>>,
) {
    // SAFETY: the pointer is `winit`'s `wl_display`, alive for the whole loop; the
    // backend does not own it (a foreign display is not closed on drop).
    let backend = unsafe { Backend::from_foreign_display(display.0.as_ptr().cast()) };
    let conn = Connection::from_backend(backend);
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        let _ = ready.send(Err(LayerError::Unsupported(
            "the Wayland registry is unavailable".into(),
        )));
        return;
    };
    let qh = queue.handle();
    let (Ok(compositor), Ok(layer_shell)) = (
        CompositorState::bind(&globals, &qh),
        LayerShell::bind(&globals, &qh),
    ) else {
        let _ = ready.send(Err(LayerError::Unsupported(
            "this compositor has no zwlr_layer_shell_v1 (GNOME's Mutter has none)".into(),
        )));
        return;
    };
    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        compositor,
        layer_shell,
        fractional_manager: globals.bind(&qh, 1..=1, Plain).ok(),
        viewporter: globals.bind(&qh, 1..=1, Plain).ok(),
        cursor_manager: globals.bind(&qh, 1..=2, Plain).ok(),
        cursor_device: None,
        on_event: Box::new(on_event),
        display,
        surfaces: HashMap::new(),
        by_surface: HashMap::new(),
        pointer_enter_serial: 0,
        pointer_focus: None,
        keyboard_focus: None,
        xkb: None,
        repeat: None,
        repeat_delay: Duration::from_millis(600),
        repeat_interval: Some(Duration::from_millis(40)),
        touch_positions: HashMap::new(),
        known_outputs: HashMap::new(),
        outputs_ready: false,
        closing: Vec::new(),
        relative: RelativePointerState::bind(&globals, &qh),
        relative_pointer: None,
        pointer_local: (0.0, 0.0),
        drag: None,
        alive: true,
    };
    // Learn the outputs and seats before anything is asked of them.
    let _ = queue.roundtrip(&mut state);
    let _ = queue.roundtrip(&mut state);
    let existing: Vec<wl_output::WlOutput> = state.outputs.outputs().collect();
    for output in existing {
        if let Some(info) = output_info(&state.outputs, &output) {
            state.known_outputs.insert(output.id(), info);
        }
    }
    state.outputs_ready = true;
    let _ = ready.send(Ok(()));

    let mut wake = wake;
    while state.alive {
        // Requests from the loop thread.
        let mut buffer = [0u8; 64];
        while matches!(wake.read(&mut buffer), Ok(n) if n > 0) {}
        let mut shutdown = false;
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Create(id, config) => state.create(&qh, id, config),
                Command::Update(id, config) => state.apply(id, &config),
                Command::Cursor(id, icon) => {
                    if let Some(surf) = state.surfaces.get_mut(&id) {
                        surf.cursor = icon;
                    }
                    if state.pointer_focus == Some(id)
                        && let Some(device) = &state.cursor_device
                    {
                        device.set_shape(state.pointer_enter_serial, shape_of(icon));
                    }
                }
                Command::InputRegion(id, region) => {
                    if let Some(surf) = state.surfaces.get_mut(&id) {
                        set_region(&state.compositor, &surf.surface, &region);
                        surf.config.input_region = region;
                        surf.surface.commit();
                    }
                }
                Command::BeginMarginDrag(id) => state.begin_margin_drag(id),
                Command::Margin(id, margin) => {
                    if let Some(surf) = state.surfaces.get_mut(&id) {
                        surf.layer
                            .set_margin(margin.0, margin.1, margin.2, margin.3);
                        surf.config.margin = margin;
                        surf.layer.commit();
                    }
                }
                Command::Destroy(id) => state.destroy(id),
                Command::Shutdown => shutdown = true,
            }
        }
        if shutdown {
            break;
        }
        // A held key repeats on this thread's clock.
        let now = Instant::now();
        if let (Some(repeat), Some(interval)) = (&state.repeat, state.repeat_interval)
            && now >= repeat.next
        {
            let code = repeat.code;
            state.repeat = Some(Repeat {
                code,
                next: now + interval,
            });
            state.key(code, true, true);
        }
        let _ = queue.flush();
        if queue.dispatch_pending(&mut state).is_err() {
            break;
        }
        if !state.closing.is_empty() {
            let _ = queue.roundtrip(&mut state);
            state.settle_closed();
        }
        let timeout = state.repeat.as_ref().map_or(-1, |r| {
            r.next
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32
        });
        if let Some(guard) = queue.prepare_read() {
            let mut fds = [
                libc::pollfd {
                    fd: guard.connection_fd().as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: wake.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            // SAFETY: two valid pollfds, live for the call.
            unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout) };
            if fds[0].revents & libc::POLLIN != 0 {
                if guard.read().is_err() {
                    break;
                }
            } else {
                drop(guard);
            }
        }
        if queue.dispatch_pending(&mut state).is_err() {
            break;
        }
        if !state.closing.is_empty() {
            let _ = queue.roundtrip(&mut state);
            state.settle_closed();
        }
    }
    // Every surface is destroyed before the thread (and its queue) goes.
    let ids: Vec<u64> = state.surfaces.keys().copied().collect();
    for id in ids {
        state.destroy(id);
    }
    let _ = queue.flush();
}

#[cfg(test)]
mod drag_tests {
    use super::*;

    fn anchored(edges: &[u8]) -> Anchors {
        let mut a = Anchors(0);
        for e in edges {
            a = Anchors(a.0 | e);
        }
        a
    }

    #[test]
    fn a_top_left_widget_follows_the_pointer_and_stops_at_the_edges() {
        let a = anchored(&[Anchors::TOP, Anchors::LEFT]);
        let (out, size) = ((1920, 1080), (200, 100));
        assert_eq!(
            drag_margins(a, (10, 0, 0, 20), (30.0, 40.0), out, size),
            (50, 0, 0, 50)
        );
        // Past the right and bottom edges: stops with the widget on the output.
        assert_eq!(
            drag_margins(a, (10, 0, 0, 20), (5000.0, 5000.0), out, size),
            (980, 0, 0, 1720)
        );
        // Past the top and left: stops at 0, never negative.
        assert_eq!(
            drag_margins(a, (10, 0, 0, 20), (-500.0, -500.0), out, size),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn a_bottom_right_widget_moves_the_other_way() {
        let a = anchored(&[Anchors::BOTTOM, Anchors::RIGHT]);
        let (out, size) = ((1000, 500), (100, 100));
        // The pointer moving right and down brings the widget nearer the right and bottom.
        assert_eq!(
            drag_margins(a, (0, 40, 40, 0), (10.0, 20.0), out, size),
            (0, 30, 20, 0)
        );
        assert_eq!(
            drag_margins(a, (0, 40, 40, 0), (500.0, 500.0), out, size),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn an_axis_anchored_to_both_edges_or_none_does_not_move() {
        // A bar across the top, anchored left and right as well: it can move down, not along.
        let bar = anchored(&[Anchors::TOP, Anchors::LEFT, Anchors::RIGHT]);
        assert_eq!(
            drag_margins(bar, (5, 6, 7, 8), (100.0, 50.0), (800, 600), (800, 30)),
            (55, 6, 7, 8)
        );
        // Anchored to nothing: neither axis moves.
        assert_eq!(
            drag_margins(Anchors(0), (1, 2, 3, 4), (9.0, 9.0), (800, 600), (50, 50)),
            (1, 2, 3, 4)
        );
    }

    #[test]
    fn a_widget_larger_than_the_output_stays_at_zero() {
        let a = anchored(&[Anchors::TOP, Anchors::LEFT]);
        assert_eq!(
            drag_margins(a, (0, 0, 0, 0), (50.0, 50.0), (100, 100), (300, 300)),
            (0, 0, 0, 0)
        );
    }
}
