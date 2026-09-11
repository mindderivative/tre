//! `tre.TrayIcon`/`tre.Menu` (GUI-readiness assessment recommendation
//! #5, tray half) -- real system tray icon + native context menu,
//! binding directly to `tre_platform::tray`.
//!
//! Both classes are marked `unsendable` (matching `PyClipboard`'s own
//! precedent for platform-connection state that isn't safely `Send`):
//! GTK's own thread-affinity requirement (a tray icon must be created
//! and used on the same thread as its GTK event loop, confirmed in
//! `tre_platform::tray`'s own isolated feasibility testing) means these
//! must stay pinned to whichever Python thread created them.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::renderer::setup_err;

/// A real, native right-click context menu, attached to a [`PyTrayIcon`]
/// at construction time.
#[pyclass(name = "Menu", unsendable)]
pub struct PyMenu {
    /// `None` once consumed by `TrayIcon.__new__` -- ownership genuinely
    /// moves into the built tray icon at that point (matching
    /// `tray_icon::TrayIconBuilder::with_menu`'s own real, single-owner
    /// API), rather than pretending a `Menu` can be reused across
    /// multiple tray icons.
    inner: Option<tre_platform::Menu>,
}

#[pymethods]
impl PyMenu {
    #[new]
    fn new() -> Self {
        Self {
            inner: Some(tre_platform::Menu::new()),
        }
    }

    /// Appends a real, clickable menu item labeled `label`, returning
    /// its own real id -- matched against a later `TrayEvent.
    /// MenuItemClick.item_id` from `tray_poll_events()`.
    ///
    /// # Errors
    /// Raises `ValueError` if this `Menu` was already attached to a
    /// `TrayIcon`. Raises `RuntimeError` if the platform menu backend
    /// itself rejects the append (real, rare).
    fn add_item(&self, label: &str, enabled: bool) -> PyResult<String> {
        self.inner_ref()?
            .add_item(label, enabled)
            .map_err(setup_err)
    }

    /// Appends a real, non-clickable visual separator.
    ///
    /// # Errors
    /// See [`add_item`](Self::add_item).
    fn add_separator(&self) -> PyResult<()> {
        self.inner_ref()?.add_separator().map_err(setup_err)
    }
}

impl PyMenu {
    fn inner_ref(&self) -> PyResult<&tre_platform::Menu> {
        self.inner.as_ref().ok_or_else(|| {
            PyValueError::new_err(
                "this Menu has already been attached to a TrayIcon and can no longer be modified",
            )
        })
    }

    /// Takes ownership of the real inner `Menu`, consuming it. Called
    /// only from `TrayIcon.__new__`.
    fn take(&mut self) -> PyResult<tre_platform::Menu> {
        self.inner.take().ok_or_else(|| {
            PyValueError::new_err("this Menu has already been attached to a TrayIcon")
        })
    }
}

/// A real system tray icon.
#[pyclass(name = "TrayIcon", unsendable)]
pub struct PyTrayIcon {
    inner: tre_platform::TrayIcon,
}

#[pymethods]
impl PyTrayIcon {
    /// Creates a real system tray icon. `rgba` must be exactly
    /// `width * height * 4` bytes. `menu` (if given) becomes the icon's
    /// right-click context menu -- ownership is consumed from the
    /// `Menu` object passed in, matching `tray_icon::TrayIconBuilder::
    /// with_menu`'s own contract.
    ///
    /// `tray_init()` must be called once before the first `TrayIcon` is
    /// created, on this same thread.
    ///
    /// # Errors
    /// Raises `ValueError` if `menu` was already attached to another
    /// `TrayIcon`. Raises `RuntimeError` if `rgba` doesn't match
    /// `width * height * 4`, or the platform tray backend rejects
    /// creation (e.g. no tray-watcher service is running).
    #[new]
    #[pyo3(signature = (rgba, width, height, tooltip=None, menu=None))]
    fn new(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        tooltip: Option<String>,
        mut menu: Option<PyRefMut<'_, PyMenu>>,
    ) -> PyResult<Self> {
        let menu = match &mut menu {
            Some(m) => Some(m.take()?),
            None => None,
        };
        let inner = tre_platform::TrayIcon::new(rgba, width, height, tooltip.as_deref(), menu)
            .map_err(setup_err)?;
        Ok(Self { inner })
    }

    /// Changes the tray icon's tooltip text after creation. `None`
    /// clears it.
    ///
    /// # Errors
    /// Raises `RuntimeError` if the platform tray backend rejects the
    /// update.
    fn set_tooltip(&self, tooltip: Option<&str>) -> PyResult<()> {
        self.inner.set_tooltip(tooltip).map_err(setup_err)
    }
}

/// `tre_platform::tray::TrayEvent` -- a real tray-icon/menu event
/// drained by `tray_poll_events()`. Empty-tuple `IconClick()` (rather
/// than a bare unit variant) matches this project's own established
/// PyO3 0.27 "complex enum" workaround (`PyMouseButton`'s own doc
/// comment in `input.rs` explains the same real constraint): mixing a
/// true unit variant with a data-carrying one in the same `#[pyclass]`
/// enum is a compile error.
#[pyclass(name = "TrayEvent", eq)]
#[derive(Clone, PartialEq, Eq)]
pub enum PyTrayEvent {
    IconClick(),
    MenuItemClick { item_id: String },
}

impl From<tre_platform::TrayEvent> for PyTrayEvent {
    fn from(event: tre_platform::TrayEvent) -> Self {
        match event {
            tre_platform::TrayEvent::IconClick => Self::IconClick(),
            tre_platform::TrayEvent::MenuItemClick { item_id } => Self::MenuItemClick { item_id },
        }
    }
}

/// Must be called once, before creating any `Menu`/`TrayIcon`, on the
/// thread that will own them.
///
/// # Errors
/// Raises `RuntimeError` if GTK itself fails to initialize (e.g. no
/// display server connection).
#[pyfunction]
fn tray_init() -> PyResult<()> {
    tre_platform::tray_init().map_err(setup_err)
}

/// Pumps pending GTK events -- REQUIRED once per frame for the tray
/// icon/menu to register, receive clicks, or update at all, since
/// `tre`'s own windowed event loop does not drive GTK's own event loop.
#[pyfunction]
fn tray_pump_events() {
    tre_platform::tray_pump_events();
}

/// Drains every real tray-icon/menu event queued since the last call --
/// call once per frame, alongside `tray_pump_events()`.
#[pyfunction]
fn tray_poll_events() -> Vec<PyTrayEvent> {
    tre_platform::tray_poll_events()
        .into_iter()
        .map(PyTrayEvent::from)
        .collect()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMenu>()?;
    m.add_class::<PyTrayIcon>()?;
    m.add_class::<PyTrayEvent>()?;
    m.add_function(pyo3::wrap_pyfunction!(tray_init, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(tray_pump_events, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(tray_poll_events, m)?)?;
    Ok(())
}
