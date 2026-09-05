//! Native OS window creation and input (ARCHITECTURE.md Section 1's
//! "Platform & Event Layer"). Linux only (Wayland primary, X11/XCB
//! fallback), per IMPLEMENTATION.md Step 1.1's scope decision.
//!
//! [`PlatformConnection`] owns ONE connection per backend (a single
//! `wayland_client::Connection` or `x11rb::xcb_ffi::XCBConnection`) shared
//! by every window it creates, rather than one connection per window
//! (IMPLEMENTATION.md Step 1.2) -- matching how a real desktop client
//! actually talks to the display server, and letting `poll_events` drain
//! one shared event source instead of one per window.
//!
//! One of the crates permitted to contain `unsafe` (TECHNICAL.md Section
//! 9.1): implementing `raw-window-handle`'s traits requires it, and the
//! X11 backend uses XCB FFI directly (`x11rb`'s `allow-unsafe-code`
//! feature) to get a real `xcb_connection_t*` for Vulkan's
//! `VK_KHR_xcb_surface`.
#![deny(unsafe_op_in_unsafe_fn)]

mod wayland;
mod x11;

use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, WindowHandle};
pub use tre_engine::{ElementState, InputEvent, MouseButton, WindowId};

#[derive(Debug)]
pub enum PlatformError {
    ConnectionFailed,
    ProtocolMissing(&'static str),
    Other(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed => write!(f, "failed to connect to the display server"),
            Self::ProtocolMissing(name) => write!(f, "required protocol/extension missing: {name}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// One shared display-server connection, owning every window created
/// through it. Pick a backend once per process (Wayland if available,
/// else X11) and create all of an application's windows from the same
/// `PlatformConnection` -- creating a second `PlatformConnection` opens a
/// second, independent connection to the display server, defeating the
/// point of this consolidation.
pub enum PlatformConnection {
    Wayland(wayland::WaylandConnection),
    X11(x11::X11Connection),
}

impl PlatformConnection {
    /// Picks Wayland if `WAYLAND_DISPLAY` is set, else falls back to X11.
    ///
    /// # Errors
    /// Returns [`PlatformError`] if the chosen backend fails to connect to
    /// the display server or is missing a required protocol/extension.
    pub fn new() -> Result<Self, PlatformError> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            Self::new_wayland()
        } else {
            Self::new_x11()
        }
    }

    /// # Errors
    /// See [`PlatformConnection::new`].
    pub fn new_wayland() -> Result<Self, PlatformError> {
        Ok(Self::Wayland(wayland::WaylandConnection::new()?))
    }

    /// # Errors
    /// See [`PlatformConnection::new`].
    pub fn new_x11() -> Result<Self, PlatformError> {
        Ok(Self::X11(x11::X11Connection::new()?))
    }

    /// Creates a new top-level window on this connection.
    ///
    /// # Errors
    /// Returns [`PlatformError`] if the compositor/window manager rejects
    /// window creation or a required protocol object is unavailable.
    pub fn create_window(
        &mut self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<WindowId, PlatformError> {
        match self {
            Self::Wayland(c) => c.create_window(title, width, height),
            Self::X11(c) => c.create_window(title, width, height),
        }
    }

    /// Drains pending events (window lifecycle + input) for every window
    /// on this connection. Call once per frame; never blocks.
    #[must_use]
    pub fn poll_events(&mut self) -> Vec<InputEvent> {
        match self {
            Self::Wayland(c) => c.poll_events(),
            Self::X11(c) => c.poll_events(),
        }
    }

    #[must_use]
    pub fn scale_factor(&self, window: WindowId) -> i32 {
        match self {
            Self::Wayland(c) => c.scale_factor(window),
            Self::X11(c) => c.scale_factor(window),
        }
    }

    /// # Errors
    /// Returns [`HandleError::Unavailable`] if `window` was not created by
    /// this connection.
    pub fn window_handle(&self, window: WindowId) -> Result<WindowHandle<'_>, HandleError> {
        match self {
            Self::Wayland(c) => c.window_handle(window),
            Self::X11(c) => c.window_handle(window),
        }
    }
}

impl HasDisplayHandle for PlatformConnection {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        match self {
            Self::Wayland(c) => c.display_handle(),
            Self::X11(c) => c.display_handle(),
        }
    }
}
