//! 0.5.0 M4 (issue #28): the title bar of a window whose app draws its own.
//!
//! On Windows and Linux an undecorated window has no OS title bar at all.
//! On macOS `winit` can't let the user resize such a window, so there
//! `decorations=False` keeps the window decorated but makes its title bar
//! transparent over full-size content: the app's content runs to the top
//! edge, the traffic lights stay, and so does resizing. `titlebar_inset`
//! says how much of the top-left corner the traffic lights take, so the
//! app lays its own bar out beside them.
//!
//! `winit` 0.30 sets this style only when a window is created
//! (`WindowAttributesExtMacOS`), so a live change is made on the `NSWindow`
//! directly, the way `winit` itself does at creation.

use winit::window::Window;

/// Whether `decorations=False` keeps the OS's window controls over the
/// content, rather than removing the title bar: true only on macOS.
pub const OVERLAY_TITLEBAR: bool = cfg!(target_os = "macos");

/// Shows or hides the OS's title bar and borders on an open window. On
/// macOS, hiding them makes the title bar a transparent overlay instead
/// (see the module docs); elsewhere it's `winit`'s `set_decorations`.
pub fn set_decorations(window: &Window, decorated: bool) {
    #[cfg(target_os = "macos")]
    if let Some(ns_window) = macos::ns_window(window) {
        macos::set_overlay(&ns_window, !decorated);
    }
    #[cfg(not(target_os = "macos"))]
    window.set_decorations(decorated);
}

/// The top-left area the OS's own window controls take over the content,
/// `(height, width)` in logical pixels: on macOS, for a window with an
/// overlay title bar, the title bar's height and the width of the traffic
/// lights with their margins; `(0.0, 0.0)` everywhere else, and in
/// fullscreen, where the traffic lights are hidden.
pub fn titlebar_inset(window: &Window) -> (f64, f64) {
    #[cfg(target_os = "macos")]
    if let (None, Some(ns_window)) = (window.fullscreen(), macos::ns_window(window)) {
        return macos::inset(&ns_window);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    (0.0, 0.0)
}

/// What a double-click on a title bar does, as the user has set it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleBarDoubleClick {
    /// Toggle maximize (on macOS, zoom): the default everywhere.
    Maximize,
    /// Minimize the window.
    Minimize,
    /// Nothing.
    Nothing,
}

impl TitleBarDoubleClick {
    /// macOS's settings: `AppleActionOnDoubleClick` (`"Maximize"`,
    /// `"Fill"`, or `"Zoom"` all maximize; `"Minimize"`; `"None"`), and, if
    /// that's unset, the older `AppleMiniaturizeOnDoubleClick`. Unset, a
    /// double-click maximizes, macOS's default.
    pub fn from_macos_defaults(action: Option<&str>, miniaturize: bool) -> Self {
        match action {
            Some("Minimize") => Self::Minimize,
            Some("None") => Self::Nothing,
            Some(_) => Self::Maximize,
            None if miniaturize => Self::Minimize,
            None => Self::Maximize,
        }
    }
}

/// What a double-click on a title bar does: the user's setting on macOS,
/// and maximize elsewhere, where there's no such setting to follow.
pub fn title_bar_double_click() -> TitleBarDoubleClick {
    #[cfg(target_os = "macos")]
    return macos::double_click();
    #[cfg(not(target_os = "macos"))]
    TitleBarDoubleClick::Maximize
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{
        NSView, NSWindow, NSWindowButton, NSWindowStyleMask, NSWindowTitleVisibility,
    };
    use objc2_foundation::{NSString, NSUserDefaults};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::TitleBarDoubleClick;

    pub(super) fn ns_window(
        window: &winit::window::Window,
    ) -> Option<objc2::rc::Retained<NSWindow>> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return None;
        };
        // SAFETY: `winit`'s handle points at the window's live content
        // view, and this runs on the main thread, where the loop runs.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        view.window()
    }

    /// The style `winit` gives a window created with a transparent title
    /// bar, full-size content, and a hidden title -- or back again. The mask
    /// is only set when it changes, since setting it re-lays the window out.
    pub(super) fn set_overlay(window: &NSWindow, overlay: bool) {
        let mask = window.styleMask();
        let wanted = if overlay {
            mask | NSWindowStyleMask::FullSizeContentView
        } else {
            mask & !NSWindowStyleMask::FullSizeContentView
        };
        if wanted != mask {
            window.setStyleMask(wanted);
        }
        window.setTitlebarAppearsTransparent(overlay);
        window.setTitleVisibility(if overlay {
            NSWindowTitleVisibility::NSWindowTitleHidden
        } else {
            NSWindowTitleVisibility::NSWindowTitleVisible
        });
    }

    pub(super) fn inset(window: &NSWindow) -> (f64, f64) {
        if !window
            .styleMask()
            .contains(NSWindowStyleMask::FullSizeContentView)
        {
            return (0.0, 0.0);
        }
        // AppKit's points are logical pixels. The layout rect is the part
        // of the content no title bar covers.
        // SAFETY: a property read on a live window, on the main thread.
        let height = window.frame().size.height - unsafe { window.contentLayoutRect() }.size.height;
        // The traffic lights run from the close button to the zoom button;
        // the margin before the first is repeated after the last.
        let width = match (
            window.standardWindowButton(NSWindowButton::NSWindowCloseButton),
            window.standardWindowButton(NSWindowButton::NSWindowZoomButton),
        ) {
            (Some(close), Some(zoom)) => {
                let (close, zoom) = (close.frame(), zoom.frame());
                zoom.origin.x + zoom.size.width + close.origin.x
            }
            _ => 0.0,
        };
        (height.max(0.0), width.max(0.0))
    }

    pub(super) fn double_click() -> TitleBarDoubleClick {
        // SAFETY: reads of the user's global defaults, with no
        // preconditions.
        unsafe {
            let defaults = NSUserDefaults::standardUserDefaults();
            let action = defaults
                .stringForKey(&NSString::from_str("AppleActionOnDoubleClick"))
                .map(|action| action.to_string());
            let miniaturize =
                defaults.boolForKey(&NSString::from_str("AppleMiniaturizeOnDoubleClick"));
            TitleBarDoubleClick::from_macos_defaults(action.as_deref(), miniaturize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TitleBarDoubleClick::{self, *};

    #[test]
    fn macos_double_click_setting_is_read_as_the_user_set_it() {
        let read = TitleBarDoubleClick::from_macos_defaults;
        assert_eq!(read(Some("Maximize"), false), Maximize);
        assert_eq!(
            read(Some("Fill"), false),
            Maximize,
            "macOS 14's name for zoom"
        );
        assert_eq!(read(Some("Minimize"), false), Minimize);
        assert_eq!(read(Some("None"), true), Nothing, "the newer key wins");
        assert_eq!(
            read(None, true),
            Minimize,
            "the older key, when the newer is unset"
        );
        assert_eq!(read(None, false), Maximize, "macOS's default");
    }
}
