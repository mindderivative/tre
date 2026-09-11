//! Native OS window creation and input (ARCHITECTURE.md Section 1's
//! "Platform & Event Layer"). Linux only (Wayland primary, X11/XCB
//! fallback), per IMPLEMENTATION.md Step 1.1's scope decision.
//!
//! [`PlatformConnection`] owns ONE connection per backend, shared by every
//! window it creates, rather than one connection per window
//! (IMPLEMENTATION.md Step 1.2) -- matching how a real desktop client
//! actually talks to the display server, and letting `poll_events` drain
//! one shared event source instead of one per window.
//!
//! Both variants are backed by `winit` (Phase 11 Step 11.1, replacing the
//! previous hand-rolled `wayland-client`/`x11rb` protocol integrations --
//! see `winit_backend`'s own module doc for the migration rationale and
//! IMPLEMENTATION.md's Step 11.1 write-up), forced to a specific backend
//! via `winit`'s own `EventLoopBuilderExtWayland`/`EventLoopBuilderExtX11`.
//!
//! Unlike the hand-rolled backends it replaced, this crate needs no
//! `unsafe` of its own: `winit`'s `Window`/`EventLoop` implement
//! `raw-window-handle` 0.6's traits directly, so no `RawWindowHandle`/
//! `RawDisplayHandle` is ever constructed by hand here. `tre-platform` is
//! accordingly removed from TECHNICAL.md Section 9.1's closed set of
//! crates permitted to contain `unsafe`.
#![forbid(unsafe_code)]

mod clipboard;
mod file_dialog;
mod tray;
mod winit_backend;

pub use clipboard::Clipboard;
pub use file_dialog::{pick_file, pick_files, pick_folder, save_file, FileFilter};
use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, WindowHandle};
pub use tray::{
    init as tray_init, poll_events as tray_poll_events, pump_events as tray_pump_events, Menu,
    TrayEvent, TrayIcon,
};
pub use tre_engine::{ElementState, InputEvent, MouseButton, WindowId};

#[derive(Debug)]
pub enum PlatformError {
    ConnectionFailed,
    ProtocolMissing(&'static str),
    /// `window` does not identify a window created by this connection (it
    /// was never created here, or has already been closed and removed).
    UnknownWindow,
    Other(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed => write!(f, "failed to connect to the display server"),
            Self::ProtocolMissing(name) => write!(f, "required protocol/extension missing: {name}"),
            Self::UnknownWindow => write!(f, "window was not created by this connection"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// RGBA8 pixel data for [`PlatformConnection::set_icon`]. `rgba.len()` must
/// equal `width * height * 4`; a mismatch surfaces as
/// [`PlatformError::Other`] from `set_icon`, not a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowIcon {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// A named cursor appearance for [`PlatformConnection::set_cursor`]
/// (Phase 12 Step 12.7) -- the real, complete CSS3/`cursor-icon` set
/// `winit` itself already exposes, bound here directly rather than
/// re-inventing a smaller one: a real GUI framework needs resize
/// handles, text carets, and drag-state cursors just as much as the
/// default pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorIcon {
    #[default]
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
}

/// One shared display-server connection, owning every window created
/// through it. Pick a backend once per process (Wayland if available,
/// else X11) and create all of an application's windows from the same
/// `PlatformConnection` -- creating a second `PlatformConnection` opens a
/// second, independent connection to the display server, defeating the
/// point of this consolidation.
pub enum PlatformConnection {
    Wayland(winit_backend::WinitConnection),
    X11(winit_backend::WinitConnection),
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
        Ok(Self::Wayland(winit_backend::WinitConnection::new_wayland()?))
    }

    /// # Errors
    /// See [`PlatformConnection::new`].
    pub fn new_x11() -> Result<Self, PlatformError> {
        Ok(Self::X11(winit_backend::WinitConnection::new_x11()?))
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

    /// Real per-window DPI scale factor, at winit's own `f64` precision
    /// (REVIEW.md finding #178: widened from a previously API-stability-
    /// preserved `i32`, at the project owner's explicit direction).
    #[must_use]
    pub fn scale_factor(&self, window: WindowId) -> f64 {
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

    /// Changes `window`'s title after creation (`create_window`'s own
    /// `title` argument only sets it once, at creation).
    ///
    /// # Errors
    /// Returns [`PlatformError::UnknownWindow`] if `window` was not created
    /// by this connection.
    pub fn set_title(&self, window: WindowId, title: &str) -> Result<(), PlatformError> {
        match self {
            Self::Wayland(c) => c.set_title(window, title),
            Self::X11(c) => c.set_title(window, title),
        }
    }

    /// Requests `window` be minimized or un-minimized.
    ///
    /// This only sends the request -- neither this call nor the very next
    /// [`is_minimized`](Self::is_minimized) reflects the new state
    /// immediately. Confirmed by real testing, not assumed: on Linux,
    /// `set_minimized` sends a one-way protocol request to the
    /// compositor/window manager, and `is_minimized` reads a value only
    /// updated once that compositor's own confirmation is later received
    /// and processed by [`poll_events`](Self::poll_events) -- a real
    /// round trip, not a local flag this call sets directly. Call
    /// `poll_events` (possibly more than once, across real wall-clock
    /// time) before `is_minimized` reflects a just-requested change.
    ///
    /// # Platform-specific
    /// On Wayland, un-minimizing (`minimized: false`) is a protocol-level
    /// limitation winit itself cannot lift -- the request is sent (no
    /// error) but has no visible effect. Minimizing (`minimized: true`)
    /// works on both backends.
    ///
    /// # Errors
    /// Returns [`PlatformError::UnknownWindow`] if `window` was not created
    /// by this connection.
    pub fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError> {
        match self {
            Self::Wayland(c) => c.set_minimized(window, minimized),
            Self::X11(c) => c.set_minimized(window, minimized),
        }
    }

    /// Whether `window` is currently minimized, as of the last processed
    /// compositor/window-manager update (see
    /// [`set_minimized`](Self::set_minimized)'s own doc comment for why
    /// this can lag a just-sent request).
    ///
    /// Returns `None` if `window` is unknown to this connection, or if the
    /// state can't be determined -- on Wayland this is always `None`, the
    /// protocol has no way to query it.
    #[must_use]
    pub fn is_minimized(&self, window: WindowId) -> Option<bool> {
        match self {
            Self::Wayland(c) => c.is_minimized(window),
            Self::X11(c) => c.is_minimized(window),
        }
    }

    /// Requests `window` be maximized or restored. Works fully on both
    /// backends, but -- exactly like [`set_minimized`](Self::set_minimized)
    /// -- only sends the request; see that method's own doc comment for
    /// why an immediately-following [`is_maximized`](Self::is_maximized)
    /// will not yet reflect it.
    ///
    /// # Errors
    /// Returns [`PlatformError::UnknownWindow`] if `window` was not created
    /// by this connection.
    pub fn set_maximized(&self, window: WindowId, maximized: bool) -> Result<(), PlatformError> {
        match self {
            Self::Wayland(c) => c.set_maximized(window, maximized),
            Self::X11(c) => c.set_maximized(window, maximized),
        }
    }

    /// Whether `window` is currently maximized, as of the last processed
    /// compositor/window-manager update (see
    /// [`set_maximized`](Self::set_maximized)'s own doc comment for why
    /// this can lag a just-sent request). Returns `false` if `window` is
    /// unknown to this connection.
    #[must_use]
    pub fn is_maximized(&self, window: WindowId) -> bool {
        match self {
            Self::Wayland(c) => c.is_maximized(window),
            Self::X11(c) => c.is_maximized(window),
        }
    }

    /// Sets or clears `window`'s icon (`None` clears it).
    ///
    /// # Platform-specific
    /// Unsupported on Wayland -- the protocol has no client-side icon
    /// mechanism (icons come from the application's own desktop-file
    /// metadata, matched by `app_id`, entirely outside this call). The
    /// call still succeeds (no error) on Wayland; it is simply a no-op.
    /// Works on X11, subject to the window manager's own icon-size
    /// conventions.
    ///
    /// # Errors
    /// Returns [`PlatformError::UnknownWindow`] if `window` was not created
    /// by this connection, or [`PlatformError::Other`] if `icon`'s `rgba`
    /// buffer doesn't match `width * height * 4`.
    pub fn set_icon(
        &self,
        window: WindowId,
        icon: Option<WindowIcon>,
    ) -> Result<(), PlatformError> {
        match self {
            Self::Wayland(c) => c.set_icon(window, icon),
            Self::X11(c) => c.set_icon(window, icon),
        }
    }

    /// Sets `window`'s mouse cursor appearance (Phase 12 Step 12.7).
    ///
    /// # Errors
    /// Returns [`PlatformError::UnknownWindow`] if `window` was not created
    /// by this connection.
    pub fn set_cursor(&self, window: WindowId, icon: CursorIcon) -> Result<(), PlatformError> {
        match self {
            Self::Wayland(c) => c.set_cursor(window, icon),
            Self::X11(c) => c.set_cursor(window, icon),
        }
    }

    /// Enables or disables real IME composition for `window` (Phase 12
    /// Step 12.7). A real, required platform opt-in, not a tre-specific
    /// step: `winit` never emits `InputEvent::ImeEnabled`/`ImePreedit`/
    /// `ImeCommit`/`ImeDisabled` for a window until this has been called
    /// with `allowed: true` for it -- matching `Window::set_ime_allowed`'s
    /// own documented contract. A real text-input caller enables this
    /// only while an editable field actually has focus (composition
    /// candidate windows are visually intrusive when shown over a
    /// non-text-input UI), and disables it again when focus leaves.
    ///
    /// # Errors
    /// Returns [`PlatformError::UnknownWindow`] if `window` was not created
    /// by this connection.
    pub fn set_ime_allowed(&self, window: WindowId, allowed: bool) -> Result<(), PlatformError> {
        match self {
            Self::Wayland(c) => c.set_ime_allowed(window, allowed),
            Self::X11(c) => c.set_ime_allowed(window, allowed),
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
