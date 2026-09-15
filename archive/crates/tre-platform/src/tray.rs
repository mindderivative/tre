//! System tray icon + native context menu (GUI-readiness assessment
//! recommendation #5, tray half), binding `tray-icon` directly (which
//! itself re-exports `muda`'s menu types under [`tray_icon::menu`]).
//!
//! **Real, required integration constraint** (confirmed in isolated
//! feasibility testing against this machine's real KDE Plasma session,
//! not assumed): on Linux, `tray-icon` needs a real GTK event loop
//! pumped on the same thread that creates and owns it -- `tre`'s own
//! windowed rendering uses `winit`'s `EventLoop`, not GTK's, so a real
//! caller must call [`pump_events`] once per frame (alongside
//! `PlatformConnection::poll_events`) to keep the tray icon and its
//! menu responsive. [`init`] must be called once, before creating any
//! [`TrayIcon`] or [`Menu`], on that same thread.

use crate::PlatformError;

/// Must be called once, before creating any [`TrayIcon`] or [`Menu`], on
/// the thread that will own them.
///
/// # Errors
/// Returns [`PlatformError::Other`] if GTK itself fails to initialize
/// (e.g. no display server connection).
pub fn init() -> Result<(), PlatformError> {
    gtk::init().map_err(|e| {
        PlatformError::Other(format!("failed to initialize GTK for the system tray: {e}"))
    })
}

/// Pumps pending GTK events -- REQUIRED once per frame for the tray
/// icon/menu to register, receive clicks, or update at all, since
/// `tre`'s own event loop (`PlatformConnection::poll_events`,
/// `winit`-backed) does not drive GTK's own event loop.
pub fn pump_events() {
    while gtk::events_pending() {
        gtk::main_iteration();
    }
}

/// A real, native right-click context menu for a [`TrayIcon`].
pub struct Menu {
    inner: tray_icon::menu::Menu,
}

impl Menu {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tray_icon::menu::Menu::new(),
        }
    }

    /// Appends a real, clickable menu item labeled `label`, returning
    /// its own real id -- matched against
    /// [`TrayEvent::MenuItemClick::item_id`](TrayEvent::MenuItemClick)
    /// from [`poll_events`] to tell which item was clicked.
    ///
    /// # Errors
    /// Returns [`PlatformError::Other`] if the platform menu backend
    /// rejects the append (real, rare).
    pub fn add_item(&self, label: &str, enabled: bool) -> Result<String, PlatformError> {
        let item = tray_icon::menu::MenuItem::new(label, enabled, None);
        let id = item.id().0.clone();
        self.inner
            .append(&item)
            .map_err(|e| PlatformError::Other(format!("failed to add menu item: {e}")))?;
        Ok(id)
    }

    /// Appends a real, non-clickable visual separator.
    ///
    /// # Errors
    /// See [`add_item`](Self::add_item).
    pub fn add_separator(&self) -> Result<(), PlatformError> {
        self.inner
            .append(&tray_icon::menu::PredefinedMenuItem::separator())
            .map_err(|e| PlatformError::Other(format!("failed to add menu separator: {e}")))
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

/// A real system tray icon.
pub struct TrayIcon {
    inner: tray_icon::TrayIcon,
}

impl TrayIcon {
    /// Creates a real system tray icon. `rgba` must be exactly
    /// `width * height * 4` bytes. `menu` (if given) becomes the icon's
    /// right-click context menu -- ownership is consumed, matching
    /// `tray-icon`'s own `TrayIconBuilder::with_menu` contract.
    ///
    /// # Errors
    /// Returns [`PlatformError::Other`] if `rgba` doesn't match
    /// `width * height * 4`, or the platform tray backend rejects
    /// creation (e.g. no tray-watcher service is running).
    pub fn new(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        tooltip: Option<&str>,
        menu: Option<Menu>,
    ) -> Result<Self, PlatformError> {
        let icon = tray_icon::Icon::from_rgba(rgba, width, height)
            .map_err(|e| PlatformError::Other(format!("invalid tray icon image data: {e}")))?;
        let mut builder = tray_icon::TrayIconBuilder::new().with_icon(icon);
        if let Some(tooltip) = tooltip {
            builder = builder.with_tooltip(tooltip);
        }
        if let Some(menu) = menu {
            builder = builder.with_menu(Box::new(menu.inner));
        }
        let inner = builder.build().map_err(|e| {
            PlatformError::Other(format!("failed to create the system tray icon: {e}"))
        })?;
        Ok(Self { inner })
    }

    /// Changes the tray icon's tooltip text after creation. `None`
    /// clears it.
    ///
    /// # Errors
    /// Returns [`PlatformError::Other`] if the platform tray backend
    /// rejects the update.
    pub fn set_tooltip(&self, tooltip: Option<&str>) -> Result<(), PlatformError> {
        self.inner
            .set_tooltip(tooltip)
            .map_err(|e| PlatformError::Other(format!("failed to set tray tooltip: {e}")))
    }

    /// Changes the tray icon's own image after creation (GUI-readiness
    /// recommendation #11's own "dynamic tray icon" follow-up -- e.g.
    /// reflecting an unread-count badge). `rgba` must be exactly
    /// `width * height * 4` bytes, the same real contract [`TrayIcon::
    /// new`] already enforces.
    ///
    /// # Errors
    /// Returns [`PlatformError::Other`] if `rgba` doesn't match
    /// `width * height * 4`, or the platform tray backend rejects the
    /// update.
    pub fn set_icon(&self, rgba: Vec<u8>, width: u32, height: u32) -> Result<(), PlatformError> {
        let icon = tray_icon::Icon::from_rgba(rgba, width, height)
            .map_err(|e| PlatformError::Other(format!("invalid tray icon image data: {e}")))?;
        self.inner
            .set_icon(Some(icon))
            .map_err(|e| PlatformError::Other(format!("failed to set tray icon: {e}")))
    }

    /// **Linux only** (a real no-op elsewhere, matching `tray-icon`'s
    /// own `set_temp_dir_path`): redirects the on-disk directory this
    /// machine's real GTK/`appindicator` backend writes each new icon
    /// image to as a temporary PNG file (`AppIndicator` has no in-memory
    /// icon API -- it reads icons from real files on disk). Exists so
    /// [`set_icon`](Self::set_icon)'s own real, on-this-machine effect
    /// can be verified automatically (reading the written PNG back and
    /// comparing pixels), not just "the call didn't raise" -- the
    /// default location (`$XDG_RUNTIME_DIR/tray-icon` or
    /// `/tmp/tray-icon`) works fine for a real caller that doesn't need
    /// this.
    pub fn set_temp_dir_path(&self, path: Option<&std::path::Path>) {
        self.inner.set_temp_dir_path(path);
    }
}

/// One real tray/menu event, drained by [`poll_events`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// The tray icon itself was clicked (left, right, or double click --
    /// `tray-icon` does not distinguish which in its own event type on
    /// every platform, so this is a real, disclosed v1 scope: click
    /// detection, not click-kind detection).
    IconClick,
    /// A menu item was activated. `item_id` matches the id
    /// [`Menu::add_item`] returned when the item was added.
    MenuItemClick { item_id: String },
}

/// Drains every real tray-icon/menu event queued since the last call --
/// call this once per frame, alongside [`pump_events`] (`tray-icon`
/// delivers events via a global channel, not a per-instance callback,
/// so this is a free function rather than a `TrayIcon` method).
#[must_use]
pub fn poll_events() -> Vec<TrayEvent> {
    let mut events = Vec::new();
    while tray_icon::TrayIconEvent::receiver().try_recv().is_ok() {
        events.push(TrayEvent::IconClick);
    }
    while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
        events.push(TrayEvent::MenuItemClick {
            item_id: event.id.0,
        });
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_with_an_item_and_a_separator_builds_without_error() {
        // A real, non-interactive check: GTK is genuinely initialized
        // and a real menu is genuinely built with real backend calls --
        // `init()` itself talks to the real display server, matching
        // `Clipboard`'s own test precedent of exercising a real platform
        // service rather than a mock.
        init().expect("gtk::init should succeed against this machine's real display server");
        let menu = Menu::new();
        let id = menu
            .add_item("Test Item", true)
            .expect("add_item should succeed");
        assert!(
            !id.is_empty(),
            "a real menu item must get a real, non-empty id"
        );
        menu.add_separator().expect("add_separator should succeed");
    }

    #[test]
    fn poll_events_returns_an_empty_vec_when_nothing_has_happened() {
        // No tray icon/menu interaction occurred, so the real global
        // event channels must be empty.
        assert!(poll_events().is_empty());
    }
}
