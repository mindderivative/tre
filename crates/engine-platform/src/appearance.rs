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

/// 0.5.4 (#115): whether the user asked the OS to reduce motion --
/// `Some(true)` to cut animation, `Some(false)` for no preference, `None`
/// where the platform can't say. Linux reads the settings portal
/// (`reduced-motion`, or GNOME's `enable-animations` on an older portal),
/// Windows the client-area animation setting, macOS the accessibility
/// display option. `window` is the open OS window, if any (unused today).
pub fn current_reduced_motion(_window: Option<&Window>) -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        portal::reduced_motion()
    }
    #[cfg(target_os = "windows")]
    {
        windows_prefs::reduced_motion()
    }
    #[cfg(target_os = "macos")]
    {
        macos_prefs::reduced_motion()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

/// 0.5.4 (#115): whether the user asked the OS for more contrast
/// (`Some(true)`), has no such preference (`Some(false)`), or the platform
/// can't say (`None`). Linux reads the portal's `contrast` (or GNOME's
/// `high-contrast`), Windows the High Contrast theme, macOS "Increase
/// contrast".
pub fn current_high_contrast(_window: Option<&Window>) -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        portal::high_contrast()
    }
    #[cfg(target_os = "windows")]
    {
        windows_prefs::high_contrast()
    }
    #[cfg(target_os = "macos")]
    {
        macos_prefs::high_contrast()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

/// The portal's `reduced-motion` value: 1 asks for less, anything else
/// (0, no preference) does not.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn reduced_motion_of(value: u32) -> bool {
    value == 1
}

/// The portal's `contrast` value: 1 asks for more, anything else does not.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn high_contrast_of(value: u32) -> bool {
    value == 1
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
    fn read_value(namespace: &str, key: &str) -> zbus::Result<OwnedValue> {
        let connection = connect()?;
        let proxy = proxy(&connection)?;
        match proxy.call("ReadOne", &(namespace, key)) {
            Ok(value) => Ok(value),
            Err(_) => proxy.call("Read", &(namespace, key)),
        }
    }

    pub(crate) fn read() -> zbus::Result<Option<bool>> {
        let value = read_value(NAMESPACE, KEY)?;
        Ok(scheme_of(&value).and_then(super::scheme_is_dark))
    }

    fn bool_of(value: &Value<'_>) -> Option<bool> {
        match value {
            Value::Bool(b) => Some(*b),
            Value::Value(inner) => bool_of(inner),
            _ => None,
        }
    }

    /// A portal `u32` setting under `org.freedesktop.appearance`.
    fn appearance_u32(key: &str) -> Option<u32> {
        let value = read_value(NAMESPACE, key).ok()?;
        scheme_of(&value)
    }

    /// A GNOME setting the portal also serves, for a portal older than the
    /// appearance keys.
    fn gnome_bool(namespace: &str, key: &str) -> Option<bool> {
        let value = read_value(namespace, key).ok()?;
        bool_of(&value)
    }

    pub(crate) fn reduced_motion() -> Option<bool> {
        appearance_u32("reduced-motion")
            .map(super::reduced_motion_of)
            .or_else(|| {
                gnome_bool("org.gnome.desktop.interface", "enable-animations").map(|on| !on)
            })
    }

    pub(crate) fn high_contrast() -> Option<bool> {
        appearance_u32("contrast")
            .map(super::high_contrast_of)
            .or_else(|| gnome_bool("org.gnome.desktop.a11y.interface", "high-contrast"))
    }

    fn scheme_of(value: &Value<'_>) -> Option<u32> {
        match value {
            Value::U32(scheme) => Some(*scheme),
            Value::Value(inner) => scheme_of(inner),
            _ => None,
        }
    }

    /// A setting the portal announced a new value for.
    pub(crate) enum Change {
        Dark(bool),
        ReducedMotion(bool),
        HighContrast(bool),
    }

    /// Starts a thread that calls `on_change` with each new appearance,
    /// reduced-motion or contrast value the portal announces, until
    /// `on_change` returns `false` (its event loop has closed). No thread
    /// without a portal to listen to.
    pub(crate) fn watch(on_change: impl Fn(Change) -> bool + Send + 'static) {
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
                    proxy.receive_signal_with_args("SettingChanged", &[(0, NAMESPACE)])
                else {
                    return;
                };
                let mut last_dark = read().ok().flatten();
                let mut last_motion = reduced_motion();
                let mut last_contrast = high_contrast();
                for message in changes {
                    let Ok((_, key, value)) =
                        message.body().deserialize::<(String, String, OwnedValue)>()
                    else {
                        continue;
                    };
                    let Some(raw) = scheme_of(&value) else {
                        continue;
                    };
                    let change = match key.as_str() {
                        KEY => match super::scheme_is_dark(raw) {
                            Some(dark) if last_dark != Some(dark) => {
                                last_dark = Some(dark);
                                Change::Dark(dark)
                            }
                            _ => continue,
                        },
                        "reduced-motion" => {
                            let reduced = super::reduced_motion_of(raw);
                            if last_motion == Some(reduced) {
                                continue;
                            }
                            last_motion = Some(reduced);
                            Change::ReducedMotion(reduced)
                        }
                        "contrast" => {
                            let high = super::high_contrast_of(raw);
                            if last_contrast == Some(high) {
                                continue;
                            }
                            last_contrast = Some(high);
                            Change::HighContrast(high)
                        }
                        _ => continue,
                    };
                    if !on_change(change) {
                        return;
                    }
                }
            });
    }
}

#[cfg(target_os = "windows")]
mod windows_prefs {
    use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST, SystemParametersInfoW,
    };

    /// Animation is a setting that is on by default: reduced motion is it off.
    pub(super) fn reduced_motion() -> Option<bool> {
        let mut enabled: i32 = 1;
        // SAFETY: `enabled` is a live BOOL the call writes through.
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut enabled as *mut i32).cast(),
                0,
            )
        };
        (ok != 0).then_some(enabled == 0)
    }

    pub(super) fn high_contrast() -> Option<bool> {
        let mut info = HIGHCONTRASTW {
            cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
            dwFlags: 0,
            lpszDefaultScheme: std::ptr::null_mut(),
        };
        // SAFETY: `info` is a live HIGHCONTRASTW with its size set.
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                std::mem::size_of::<HIGHCONTRASTW>() as u32,
                (&mut info as *mut HIGHCONTRASTW).cast(),
                0,
            )
        };
        (ok != 0).then_some(info.dwFlags & HCF_HIGHCONTRASTON != 0)
    }
}

#[cfg(target_os = "macos")]
mod macos_prefs {
    use objc2_app_kit::NSWorkspace;

    pub(super) fn reduced_motion() -> Option<bool> {
        // SAFETY: a plain getter on the shared workspace.
        Some(unsafe { NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() })
    }

    pub(super) fn high_contrast() -> Option<bool> {
        // SAFETY: as above.
        Some(unsafe { NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast() })
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
    fn the_portals_motion_and_contrast_values_are_one_for_yes() {
        assert!(reduced_motion_of(1));
        assert!(!reduced_motion_of(0), "no preference is not reduced");
        assert!(!reduced_motion_of(2));
        assert!(high_contrast_of(1));
        assert!(!high_contrast_of(0));
    }

    #[test]
    fn no_window_falls_back_to_the_platform() {
        assert_eq!(current_dark(None), system_dark());
    }
}
