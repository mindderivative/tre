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
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
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
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_touch, registry_handlers,
};
use wayland_backend::client::Backend;
use winit::window::CursorIcon;

use crate::layer_config::{Anchors, KeyboardMode, LayerConfig, LayerKind};
use crate::layer_keys::{self, Keymap};

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
}

/// The host's side of the loop for layer surfaces.
pub struct LayerHooks {
    /// The surface has a size: build the GPU side from the info. `false` abandons it.
    pub on_created: Box<dyn FnMut(winit::window::WindowId, u64, LayerSurfaceInfo) -> bool>,
    /// `(id, token, notice)`.
    pub on_notice: Box<dyn FnMut(winit::window::WindowId, u64, LayerNotice)>,
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
    alive: bool,
}

impl State {
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
        let mut anchor = Anchor::empty();
        for (bit, flag) in [
            (Anchors::TOP, Anchor::TOP),
            (Anchors::BOTTOM, Anchor::BOTTOM),
            (Anchors::LEFT, Anchor::LEFT),
            (Anchors::RIGHT, Anchor::RIGHT),
        ] {
            if config.anchor.contains(bit) {
                anchor |= flag;
            }
        }
        layer.set_anchor(anchor);
        layer.set_size(config.size.0, config.size.1);
        layer.set_exclusive_zone(config.exclusive_zone);
        layer.set_margin(
            config.margin.0,
            config.margin.1,
            config.margin.2,
            config.margin.3,
        );
        layer.set_keyboard_interactivity(match config.keyboard {
            KeyboardMode::None => KeyboardInteractivity::None,
            KeyboardMode::OnDemand => KeyboardInteractivity::OnDemand,
            KeyboardMode::Exclusive => KeyboardInteractivity::Exclusive,
        });
        surf.config = config.clone();
        layer.commit();
    }

    fn create(&mut self, qh: &QueueHandle<Self>, id: u64, config: LayerConfig) {
        let output = match &config.output {
            None => None,
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
            surface.clone(),
            Layer::Top,
            Some(config.namespace.clone()),
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
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        let id = self
            .surfaces
            .iter()
            .find(|(_, s)| s.layer.wl_surface() == layer.wl_surface())
            .map(|(id, _)| *id);
        let Some(id) = id else { return };
        // A surface bound to an output that is no longer there was closed because of it.
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
        self.destroy(id);
        self.emit(LayerEvent::Closed { id, reason });
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        let id = self
            .surfaces
            .iter()
            .find(|(_, s)| s.layer.wl_surface() == layer.wl_surface())
            .map(|(id, _)| *id);
        let Some(id) = id else { return };
        let (width, height) = configure.new_size;
        if let Some(surf) = self.surfaces.get_mut(&id) {
            // 0 means "your choice": the size the surface asked for.
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
            match event.kind {
                PointerEventKind::Enter { serial } => {
                    self.pointer_enter_serial = serial;
                    self.pointer_focus = Some(id);
                    self.input(id, InputEvent::PointerMoved { position });
                }
                PointerEventKind::Leave { .. } => {
                    if self.pointer_focus == Some(id) {
                        self.pointer_focus = None;
                    }
                    self.input(id, InputEvent::PointerLeft);
                }
                PointerEventKind::Motion { .. } => {
                    self.input(id, InputEvent::PointerMoved { position });
                }
                PointerEventKind::Press { button, .. } => {
                    if let Some(button) = button_of(button) {
                        self.input(id, InputEvent::PointerPressed { position, button });
                    }
                }
                PointerEventKind::Release { button, .. } => {
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
                        ScrollDelta::Pixels(-horizontal.absolute, -vertical.absolute)
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
delegate_touch!(State);
delegate_layer!(State);
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
        alive: true,
    };
    // Learn the outputs and seats before anything is asked of them.
    let _ = queue.roundtrip(&mut state);
    let _ = queue.roundtrip(&mut state);
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
                    if state.pointer_focus == Some(id)
                        && let Some(device) = &state.cursor_device
                    {
                        device.set_shape(state.pointer_enter_serial, shape_of(icon));
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
    }
    // Every surface is destroyed before the thread (and its queue) goes.
    let ids: Vec<u64> = state.surfaces.keys().copied().collect();
    for id in ids {
        state.destroy(id);
    }
    let _ = queue.flush();
}
