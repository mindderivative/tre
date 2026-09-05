//! Native Wayland windowing via `wayland-client` + the `xdg-shell` stable
//! protocol. Requires the `system` (real `libwayland-client.so`) backend so
//! the raw pointers handed to `raw-window-handle` are genuine C pointers
//! Vulkan's `VK_KHR_wayland_surface` extension can use directly.

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use wayland_client::protocol::{wl_compositor, wl_output, wl_registry, wl_surface};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::{PlatformError, WindowEvent};

struct AppState {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    configured: bool,
    close_requested: bool,
    pending_resize: Option<(u32, u32)>,
    scale: i32,
}

delegate_noop!(AppState: ignore wl_compositor::WlCompositor);
delegate_noop!(AppState: ignore wl_surface::WlSurface);

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

impl Dispatch<xdg_surface::XdgSurface, ()> for AppState {
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for AppState {
    fn event(
        state: &mut Self,
        _toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Close => state.close_requested = true,
            xdg_toplevel::Event::Configure { width, height, .. } if width > 0 && height > 0 => {
                state.pending_resize = Some((width as u32, height as u32));
            }
            _ => {}
        }
    }
}

// Core `wl_output.scale` (integer HiDPI scale, always available -- no
// fractional-scale-v1 protocol extension needed for this step's minimum
// "report a scale factor" requirement). `wl_output` is a multi-instance
// global; the last one seen wins, which is fine for a single-monitor dev
// setup and acceptable for this step's scope.
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

pub struct WaylandWindow {
    connection: Connection,
    event_queue: EventQueue<AppState>,
    state: AppState,
    surface: wl_surface::WlSurface,
    _xdg_surface: xdg_surface::XdgSurface,
    _toplevel: xdg_toplevel::XdgToplevel,
}

impl WaylandWindow {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, PlatformError> {
        let connection =
            Connection::connect_to_env().map_err(|_| PlatformError::ConnectionFailed)?;
        let mut event_queue: EventQueue<AppState> = connection.new_event_queue();
        let qh = event_queue.handle();

        let display = connection.display();
        let _registry = display.get_registry(&qh, ());

        let mut state = AppState {
            compositor: None,
            wm_base: None,
            configured: false,
            close_requested: false,
            pending_resize: None,
            scale: 1,
        };
        // One round-trip is enough for the compositor to advertise all
        // its globals; wl_compositor/xdg_wm_base/wl_output are always
        // advertised immediately on connect.
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        let compositor = state
            .compositor
            .clone()
            .ok_or(PlatformError::ProtocolMissing("wl_compositor"))?;
        let wm_base = state
            .wm_base
            .clone()
            .ok_or(PlatformError::ProtocolMissing("xdg_wm_base"))?;

        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        toplevel.set_title(title.to_string());
        toplevel.set_app_id("tre-walking-skeleton".to_string());
        surface.commit();

        // xdg-shell requires waiting for the first `xdg_surface.configure`
        // (and acking it) before any buffer may be attached -- Vulkan's
        // WSI attaches buffers on our behalf once the swapchain is
        // created, so we just need `configured` to be true before that.
        while !state.configured {
            event_queue
                .blocking_dispatch(&mut state)
                .map_err(|e| PlatformError::Other(e.to_string()))?;
        }

        let _ = width;
        let _ = height;

        Ok(Self {
            connection,
            event_queue,
            state,
            surface,
            _xdg_surface: xdg_surface,
            _toplevel: toplevel,
        })
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        let _ = self.connection.flush();
        let _ = self.event_queue.dispatch_pending(&mut self.state);

        let mut events = Vec::new();
        if self.state.close_requested {
            events.push(WindowEvent::CloseRequested);
        }
        if let Some((w, h)) = self.state.pending_resize.take() {
            events.push(WindowEvent::Resized(w, h));
        }
        events
    }

    #[must_use]
    pub fn scale_factor(&self) -> i32 {
        self.state.scale
    }
}

impl HasDisplayHandle for WaylandWindow {
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

impl HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let surface_ptr = self.surface.id().as_ptr();
        let ptr = std::ptr::NonNull::new(surface_ptr.cast()).ok_or(HandleError::Unavailable)?;
        // SAFETY: `surface_ptr` is a genuine `wl_surface*` (a `wl_proxy*`
        // under the hood) for as long as `self.surface` is alive, which
        // outlives this borrow.
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(ptr)))
        })
    }
}
