//! Native X11 windowing via `x11rb`'s XCB FFI connection type
//! (`allow-unsafe-code` feature), which -- unlike x11rb's default
//! pure-Rust socket connection -- links real `libxcb` and exposes a
//! genuine `xcb_connection_t*` for Vulkan's `VK_KHR_xcb_surface` extension.
//! Fallback path for X11/XWayland sessions (IMPLEMENTATION.md Step 1.1).

use std::ffi::c_void;
use std::num::NonZeroU32;
use std::ptr::NonNull;

use as_raw_xcb_connection::AsRawXcbConnection;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle, XcbDisplayHandle, XcbWindowHandle,
};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux, EventMask, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

use crate::{PlatformError, WindowEvent};

pub struct X11Window {
    connection: XCBConnection,
    screen_num: usize,
    window: u32,
    wm_delete_window: u32,
    pending_resize: Option<(u32, u32)>,
    close_requested: bool,
    last_size: (u32, u32),
}

impl X11Window {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, PlatformError> {
        let (connection, screen_num) =
            XCBConnection::connect(None).map_err(|_| PlatformError::ConnectionFailed)?;
        let screen = connection.setup().roots[screen_num].clone();
        let window = connection
            .generate_id()
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                screen.root,
                0,
                0,
                width as u16,
                height as u16,
                0,
                WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &CreateWindowAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
            )
            .map_err(|e| PlatformError::Other(e.to_string()))?;
        let _ = ChangeWindowAttributesAux::new(); // reserved for future input-mask setup (Step 1.2)

        connection
            .change_property8(
                x11rb::protocol::xproto::PropMode::REPLACE,
                window,
                x11rb::protocol::xproto::AtomEnum::WM_NAME,
                x11rb::protocol::xproto::AtomEnum::STRING,
                title.as_bytes(),
            )
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        let wm_protocols = connection
            .intern_atom(false, b"WM_PROTOCOLS")
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .reply()
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .atom;
        let wm_delete_window = connection
            .intern_atom(false, b"WM_DELETE_WINDOW")
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .reply()
            .map_err(|e| PlatformError::Other(e.to_string()))?
            .atom;
        connection
            .change_property32(
                x11rb::protocol::xproto::PropMode::REPLACE,
                window,
                wm_protocols,
                x11rb::protocol::xproto::AtomEnum::ATOM,
                &[wm_delete_window],
            )
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        connection
            .map_window(window)
            .map_err(|e| PlatformError::Other(e.to_string()))?;
        connection
            .flush()
            .map_err(|e| PlatformError::Other(e.to_string()))?;

        Ok(Self {
            connection,
            screen_num,
            window,
            wm_delete_window,
            pending_resize: None,
            close_requested: false,
            last_size: (width, height),
        })
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        let mut events = Vec::new();
        while let Ok(Some(event)) = self.connection.poll_for_event() {
            match event {
                Event::ClientMessage(msg) => {
                    if msg.data.as_data32()[0] == self.wm_delete_window {
                        self.close_requested = true;
                    }
                }
                Event::ConfigureNotify(cfg) => {
                    let size = (u32::from(cfg.width), u32::from(cfg.height));
                    if size != self.last_size {
                        self.last_size = size;
                        self.pending_resize = Some(size);
                    }
                }
                _ => {}
            }
        }
        if self.close_requested {
            events.push(WindowEvent::CloseRequested);
        }
        if let Some(size) = self.pending_resize.take() {
            events.push(WindowEvent::Resized(size.0, size.1));
        }
        events
    }

    #[must_use]
    pub fn scale_factor(&self) -> i32 {
        // X11 has no equivalent to Wayland's per-output integer scale in
        // the core protocol; Xft.dpi / RandR-based scale detection is a
        // reasonable future addition, out of this step's scope.
        1
    }
}

impl HasDisplayHandle for X11Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: raw pointer from `AsRawXcbConnection`, valid for as long
        // as `self.connection` is alive, which outlives this borrow.
        let conn_ptr = self.connection.as_raw_xcb_connection() as *mut c_void;
        let ptr = NonNull::new(conn_ptr);
        let handle = XcbDisplayHandle::new(ptr, self.screen_num as i32);
        Ok(unsafe { DisplayHandle::borrow_raw(RawDisplayHandle::Xcb(handle)) })
    }
}

impl HasWindowHandle for X11Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let window = NonZeroU32::new(self.window).ok_or(HandleError::Unavailable)?;
        let handle = XcbWindowHandle::new(window);
        // SAFETY: `self.window` is a valid XCB window ID for as long as
        // `self.connection` is alive, which outlives this borrow.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xcb(handle)) })
    }
}
