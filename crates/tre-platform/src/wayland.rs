//! Native Wayland windowing via `wayland-client` + the `xdg-shell` stable
//! protocol, now consolidated to ONE shared `Connection`/`EventQueue` per
//! process (IMPLEMENTATION.md Step 1.2) rather than one per window --
//! matching how a real Wayland client actually talks to the compositor.
//! Requires the `system` (real `libwayland-client.so`) backend so the raw
//! pointers handed to `raw-window-handle` are genuine C pointers Vulkan's
//! `VK_KHR_wayland_surface` extension can use directly.
//!
//! Pointer/keyboard input (`wl_seat`) is bound once per connection and
//! routed to the correct window via the entered/focused `wl_surface`,
//! translated into [`tre_engine::InputEvent`]. Every event this backend
//! produces -- input and window lifecycle alike -- is pushed through one
//! shared [`tre_engine::InputEventQueue`], which owns pointer-move
//! coalescing centrally rather than each backend duplicating it.

use std::collections::HashMap;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use tre_engine::{ElementState, InputEvent, InputEventQueue, MouseButton, WindowId};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{
    wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_surface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::PlatformError;

/// One connection's worth of queued events, sized generously for a
/// per-frame drain of a handful of windows' worth of input.
const EVENT_QUEUE_CAPACITY: usize = 256;

struct WindowState {
    surface: wl_surface::WlSurface,
    configured: bool,
}

struct AppState {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    scale: i32,
    windows: HashMap<WindowId, WindowState>,
    surface_to_window: HashMap<ObjectId, WindowId>,
    pointer_focus: Option<WindowId>,
    keyboard_focus: Option<WindowId>,
    event_queue: InputEventQueue,
}

delegate_noop!(AppState: ignore wl_compositor::WlCompositor);

impl Dispatch<wl_surface::WlSurface, WindowId> for AppState {
    fn event(
        _state: &mut Self,
        _surface: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &WindowId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // No `wl_surface` event carries information this step needs
        // (frame callbacks/enter-output are not used yet).
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "xdg_wm_base" => {
                    state.wm_base = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "wl_output" => {
                    let _output: wl_output::WlOutput = registry.bind(name, version.min(2), qh, ());
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version.min(5), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for AppState {
    fn event(
        _state: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, WindowId> for AppState {
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        data: &WindowId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            if let Some(w) = state.windows.get_mut(data) {
                w.configured = true;
            }
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, WindowId> for AppState {
    fn event(
        state: &mut Self,
        _toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        data: &WindowId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if !state.windows.contains_key(data) {
            return;
        }
        match event {
            xdg_toplevel::Event::Close => {
                state
                    .event_queue
                    .push(InputEvent::CloseRequested { window: *data });
            }
            xdg_toplevel::Event::Configure { width, height, .. } if width > 0 && height > 0 => {
                state.event_queue.push(InputEvent::Resized {
                    window: *data,
                    width: width as u32,
                    height: height as u32,
                });
            }
            _ => {}
        }
    }
}

// Core `wl_output.scale` (integer HiDPI scale) -- see the original Step
// 1.1 rationale: single-monitor dev setups only, last-seen-wins.
impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(
        state: &mut Self,
        _output: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale { factor } = event {
            state.scale = factor;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for AppState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            let caps = match capabilities {
                wayland_client::WEnum::Value(c) => c,
                wayland_client::WEnum::Unknown(_) => return,
            };
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let window = state.surface_to_window.get(&surface.id()).copied();
                state.pointer_focus = window;
                if let Some(window) = window {
                    state.event_queue.push(InputEvent::PointerMoved {
                        window,
                        x: surface_x,
                        y: surface_y,
                    });
                }
            }
            wl_pointer::Event::Leave { .. } => {
                state.event_queue.flush_pending_move();
                state.pointer_focus = None;
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if let Some(window) = state.pointer_focus {
                    state.event_queue.push(InputEvent::PointerMoved {
                        window,
                        x: surface_x,
                        y: surface_y,
                    });
                }
            }
            wl_pointer::Event::Button {
                button,
                state: btn_state,
                ..
            } => {
                if let Some(window) = state.pointer_focus {
                    let element_state = match btn_state {
                        wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                            ElementState::Pressed
                        }
                        wayland_client::WEnum::Value(wl_pointer::ButtonState::Released) => {
                            ElementState::Released
                        }
                        _ => return,
                    };
                    state.event_queue.push(InputEvent::PointerButton {
                        window,
                        button: linux_button_code_to_mouse_button(button),
                        state: element_state,
                    });
                }
            }
            wl_pointer::Event::Frame => state.event_queue.flush_pending_move(),
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { surface, .. } => {
                state.keyboard_focus = state.surface_to_window.get(&surface.id()).copied();
            }
            wl_keyboard::Event::Leave { .. } => state.keyboard_focus = None,
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                if let Some(window) = state.keyboard_focus {
                    let element_state = match key_state {
                        wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed) => {
                            ElementState::Pressed
                        }
                        wayland_client::WEnum::Value(wl_keyboard::KeyState::Released) => {
                            ElementState::Released
                        }
                        _ => return,
                    };
                    state.event_queue.push(InputEvent::KeyboardKey {
                        window,
                        key_code: key,
                        state: element_state,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Linux evdev button codes (`linux/input-event-codes.h`), what
/// `wl_pointer::Event::Button::button` carries: `BTN_LEFT` = 0x110,
/// `BTN_RIGHT` = 0x111, `BTN_MIDDLE` = 0x112.
fn linux_button_code_to_mouse_button(code: u32) -> MouseButton {
    match code {
        0x110 => MouseButton::Left,
        0x111 => MouseButton::Right,
        0x112 => MouseButton::Middle,
        other => MouseButton::Other(other as u16),
    }
}

pub struct WaylandConnection {
    connection: Connection,
    event_queue: EventQueue<AppState>,
    state: AppState,
    next_window_id: u64,
    /// `xdg_surface`/`xdg_toplevel` protocol objects must stay alive for as
    /// long as their window does, even though this step never calls
    /// methods on them again after creation -- keeping them here (rather
    /// than letting the local bindings drop at the end of `create_window`)
    /// is what keeps the window itself from being destroyed.
    keep_alive: HashMap<WindowId, (xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel)>,
}

impl WaylandConnection {
    pub fn new() -> Result<Self, PlatformError> {
        let connection =
            Connection::connect_to_env().map_err(|_| PlatformError::ConnectionFailed)?;
        let mut event_queue: EventQueue<AppState> = connection.new_event_queue();
        let qh = event_queue.handle();

        let display = connection.display();
        let _registry = display.get_registry(&qh, ());

        let mut state = AppState {
            compositor: None,
            wm_base: None,
            seat: None,
            pointer: None,
            keyboard: None,
            scale: 1,
            windows: HashMap::new(),
            surface_to_window: HashMap::new(),
            pointer_focus: None,
            keyboard_focus: None,
            event_queue: InputEventQueue::with_capacity(EVENT_QUEUE_CAPACITY),
        };
        // One round-trip is enough for the compositor to advertise all its
        // globals; wl_compositor/xdg_wm_base/wl_output/wl_seat are always
        // advertised immediately on connect. A second round-trip lets
        // wl_seat's Capabilities event (which binds wl_pointer/wl_keyboard)
        // actually run before window creation needs them.
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| PlatformError::Other(e.to_string()))?;
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        if state.compositor.is_none() {
            return Err(PlatformError::ProtocolMissing("wl_compositor"));
        }
        if state.wm_base.is_none() {
            return Err(PlatformError::ProtocolMissing("xdg_wm_base"));
        }

        Ok(Self {
            connection,
            event_queue,
            state,
            next_window_id: 0,
            keep_alive: HashMap::new(),
        })
    }

    pub fn create_window(
        &mut self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<WindowId, PlatformError> {
        let qh = self.event_queue.handle();
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        let compositor = self
            .state
            .compositor
            .clone()
            .ok_or(PlatformError::ProtocolMissing("wl_compositor"))?;
        let wm_base = self
            .state
            .wm_base
            .clone()
            .ok_or(PlatformError::ProtocolMissing("xdg_wm_base"))?;

        let surface = compositor.create_surface(&qh, id);
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, id);
        let toplevel = xdg_surface.get_toplevel(&qh, id);
        toplevel.set_title(title.to_string());
        toplevel.set_app_id("tre-walking-skeleton".to_string());
        surface.commit();

        self.state.surface_to_window.insert(surface.id(), id);
        self.state.windows.insert(
            id,
            WindowState {
                surface,
                configured: false,
            },
        );

        // xdg-shell requires waiting for the first `xdg_surface.configure`
        // (and acking it) before any buffer may be attached -- Vulkan's WSI
        // attaches buffers on our behalf once the swapchain is created, so
        // we just need `configured` to be true before that.
        while !self.state.windows[&id].configured {
            self.event_queue
                .blocking_dispatch(&mut self.state)
                .map_err(|e| PlatformError::Other(e.to_string()))?;
        }

        self.keep_alive.insert(id, (xdg_surface, toplevel));

        let _ = width;
        let _ = height;
        Ok(id)
    }

    pub fn poll_events(&mut self) -> Vec<InputEvent> {
        let _ = self.connection.flush();
        let _ = self.event_queue.dispatch_pending(&mut self.state);
        self.state.event_queue.drain()
    }

    #[must_use]
    pub fn scale_factor(&self, _window: WindowId) -> i32 {
        self.state.scale
    }
}

impl HasDisplayHandle for WaylandConnection {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let display_ptr = self.connection.backend().display_ptr();
        let ptr = std::ptr::NonNull::new(display_ptr.cast()).ok_or(HandleError::Unavailable)?;
        // SAFETY: `display_ptr` is a genuine `wl_display*` for as long as
        // `self.connection` is alive, which outlives this borrow.
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(ptr)))
        })
    }
}

impl WaylandConnection {
    /// # Errors
    /// Returns [`HandleError::Unavailable`] if `window` is not a window
    /// created by this connection (e.g. it was already closed and removed).
    pub fn window_handle(&self, window: WindowId) -> Result<WindowHandle<'_>, HandleError> {
        let surface = &self
            .state
            .windows
            .get(&window)
            .ok_or(HandleError::Unavailable)?
            .surface;
        let surface_ptr = surface.id().as_ptr();
        let ptr = std::ptr::NonNull::new(surface_ptr.cast()).ok_or(HandleError::Unavailable)?;
        // SAFETY: `surface_ptr` is a genuine `wl_surface*` for as long as
        // the `WindowState` entry (and thus `surface`) is alive, which
        // outlives this borrow.
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(ptr)))
        })
    }
}
