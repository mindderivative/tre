//! M106 (issue #18): the OS's light/dark appearance. `winit` 0.30 reports
//! it only on macOS and Windows -- `Window::theme()` is `None` on X11 and
//! only an app-set override on Wayland, and its `ThemeChanged` never fires
//! on Linux (both checked in its source). On Linux the desktop publishes
//! it through the XDG settings portal instead
//! (`org.freedesktop.appearance` / `color-scheme`), the same place
//! `winit`'s own Wayland decorations (`sctk-adwaita`) read it: it's read
//! here with `zbus`, which AccessKit's Linux backend already builds.

use winit::window::{Theme, Window};

/// The OS's current appearance: `Some(true)` for dark, `Some(false)` for
/// light, `None` where the platform can't say. `window` is the open OS
/// window, if any -- `winit` answers through it on macOS and Windows,
/// so there `None` until `App.run()` opens it. On Linux the portal
/// answers with or without a window.
pub fn current_dark(window: Option<&Window>) -> Option<bool> {
    window
        .and_then(Window::theme)
        .map(|theme| theme == Theme::Dark)
        .or_else(system_dark)
}

/// The portal's `color-scheme`, `None` without one (no session bus, no
/// portal, or no answer within the timeout). Always `None` off Linux.
pub fn system_dark() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        portal::read().ok().flatten()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// The portal's `color-scheme` value as a `dark` flag: 1 prefers dark,
/// 2 prefers light, and 0 ("no preference") is the desktop's default
/// look, which is light on GNOME and KDE -- so it reads as light rather
/// than `None`, or a framework following the OS would keep guessing.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn scheme_is_dark(scheme: u32) -> Option<bool> {
    match scheme {
        1 => Some(true),
        0 | 2 => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod portal {
    use std::time::Duration;

    use zbus::blocking::{Connection, Proxy, connection};
    use zbus::zvariant::{OwnedValue, Value};

    const DESTINATION: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";
    const INTERFACE: &str = "org.freedesktop.portal.Settings";
    const NAMESPACE: &str = "org.freedesktop.appearance";
    const KEY: &str = "color-scheme";

    /// A missing or slow portal must never stall a caller for long:
    /// `sctk-adwaita` gives it 100 ms for the same read.
    const TIMEOUT: Duration = Duration::from_millis(250);

    fn connect() -> zbus::Result<Connection> {
        connection::Builder::session()?
            .method_timeout(TIMEOUT)
            .build()
    }

    fn proxy(connection: &Connection) -> zbus::Result<Proxy<'static>> {
        Proxy::new(connection, DESTINATION, PATH, INTERFACE)
    }

    /// `ReadOne` (portal version 2+); `Read`, which wraps the value in a
    /// second variant, on older portals.
    pub(crate) fn read() -> zbus::Result<Option<bool>> {
        let connection = connect()?;
        let proxy = proxy(&connection)?;
        let value: OwnedValue = match proxy.call("ReadOne", &(NAMESPACE, KEY)) {
            Ok(value) => value,
            Err(_) => proxy.call("Read", &(NAMESPACE, KEY))?,
        };
        Ok(scheme_of(&value).and_then(super::scheme_is_dark))
    }

    fn scheme_of(value: &Value<'_>) -> Option<u32> {
        match value {
            Value::U32(scheme) => Some(*scheme),
            Value::Value(inner) => scheme_of(inner),
            _ => None,
        }
    }

    /// Starts a thread that calls `on_change` with each new appearance
    /// the portal announces, until `on_change` returns `false` (its event
    /// loop has closed). No thread without a portal to listen to.
    pub(crate) fn watch(on_change: impl Fn(bool) -> bool + Send + 'static) {
        let Ok(connection) = connect() else {
            return;
        };
        let _ = std::thread::Builder::new()
            .name("tre-appearance".into())
            .spawn(move || {
                let Ok(proxy) = proxy(&connection) else {
                    return;
                };
                let Ok(changes) =
                    proxy.receive_signal_with_args("SettingChanged", &[(0, NAMESPACE), (1, KEY)])
                else {
                    return;
                };
                let mut last = read().ok().flatten();
                for message in changes {
                    let Ok((_, _, value)) =
                        message.body().deserialize::<(String, String, OwnedValue)>()
                    else {
                        continue;
                    };
                    let Some(dark) = scheme_of(&value).and_then(super::scheme_is_dark) else {
                        continue;
                    };
                    if last == Some(dark) {
                        continue;
                    }
                    last = Some(dark);
                    if !on_change(dark) {
                        return;
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_values_map_to_dark_light_or_unknown() {
        assert_eq!(scheme_is_dark(1), Some(true));
        assert_eq!(scheme_is_dark(2), Some(false));
        assert_eq!(
            scheme_is_dark(0),
            Some(false),
            "no preference reads as light"
        );
        assert_eq!(scheme_is_dark(7), None);
    }

    #[test]
    fn no_window_falls_back_to_the_platform() {
        assert_eq!(current_dark(None), system_dark());
    }
}
