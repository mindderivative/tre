//! Native OS window creation (ARCHITECTURE.md Section 1's "Platform &
//! Event Layer"). Phase 1 Step 1 scope: Linux only (Wayland primary,
//! X11/XCB fallback). Produces `raw-window-handle` values that plug
//! directly into the existing `ash_window`-based Vulkan surface creation
//! from Phase 0, unchanged.
//!
//! One of the crates permitted to contain `unsafe` (TECHNICAL.md Section
//! 9.1): implementing `raw-window-handle`'s traits requires it, and the
//! X11 backend uses XCB FFI directly (`x11rb`'s `allow-unsafe-code`
//! feature) to get a real `xcb_connection_t*` for Vulkan's
//! `VK_KHR_xcb_surface`.
#![deny(unsafe_op_in_unsafe_fn)]

mod wayland;
mod x11;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// Minimal window lifecycle events -- enough to make windowing itself
/// work (close, resize). The full input event queue (`InputEvent`,
/// pointer/keyboard translation, the SPSC ring buffer) is
/// IMPLEMENTATION.md Step 1.2, a separate step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowEvent {
    CloseRequested,
    Resized(u32, u32),
}

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

pub enum PlatformWindow {
    Wayland(wayland::WaylandWindow),
    X11(x11::X11Window),
}

impl PlatformWindow {
    /// Picks Wayland if `WAYLAND_DISPLAY` is set, else falls back to X11.
    ///
    /// # Errors
    /// Returns [`PlatformError`] if the chosen backend fails to connect to
    /// the display server or is missing a required protocol/extension.
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, PlatformError> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            Self::new_wayland(title, width, height)
        } else {
            Self::new_x11(title, width, height)
        }
    }

    /// # Errors
    /// See [`PlatformWindow::new`].
    pub fn new_wayland(title: &str, width: u32, height: u32) -> Result<Self, PlatformError> {
        Ok(Self::Wayland(wayland::WaylandWindow::new(
            title, width, height,
        )?))
    }

    /// # Errors
    /// See [`PlatformWindow::new`].
    pub fn new_x11(title: &str, width: u32, height: u32) -> Result<Self, PlatformError> {
        Ok(Self::X11(x11::X11Window::new(title, width, height)?))
    }

    /// Drains pending window events (close/resize). Call once per frame;
    /// never blocks.
    #[must_use]
    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        match self {
            Self::Wayland(w) => w.poll_events(),
            Self::X11(w) => w.poll_events(),
        }
    }

    #[must_use]
    pub fn scale_factor(&self) -> i32 {
        match self {
            Self::Wayland(w) => w.scale_factor(),
            Self::X11(w) => w.scale_factor(),
        }
    }
}

impl HasDisplayHandle for PlatformWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        match self {
            Self::Wayland(w) => w.display_handle(),
            Self::X11(w) => w.display_handle(),
        }
    }
}

impl HasWindowHandle for PlatformWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        match self {
            Self::Wayland(w) => w.window_handle(),
            Self::X11(w) => w.window_handle(),
        }
    }
}
