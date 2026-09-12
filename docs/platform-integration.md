# Platform Integration

This page covers how the desktop-integration features fit together and what's specific to each platform. For exact method signatures and exceptions, see the [Python API](python-api/index.md) reference this page links into throughout -- nothing here duplicates a signature already documented there.

**Verified platform today: Linux (X11 and Wayland).** Windowing goes through [`winit`](https://github.com/rust-windowing/winit), which is itself cross-platform, but every feature on this page has only been exercised against a real Linux desktop. Windows and macOS are unverified -- not confirmed broken, just untested, and not a current project priority.

## Window chrome

`WindowedRenderer`'s [window chrome methods](python-api/rendering.md#window-chrome) (`set_title`, `set_minimized`/`set_maximized`, `set_icon`, `set_cursor`) work as you'd expect on both X11 and Wayland, with one real exception: **`set_icon` is a no-op on Wayland**. The protocol itself has no client-side window-icon mechanism -- a Wayland compositor derives an app's icon from its `.desktop` file, not a runtime API call. This isn't a bug in `tre-platform`; there's no Wayland API to call. X11 doesn't have this limitation.

Multi-monitor support is partial: `renderer.scale_factor` type per-window DPI queries work (backed by winit), but there's no monitor-enumeration API yet -- no way to list connected monitors or ask which one a window is on.

## Clipboard

[`tre.Clipboard`](python-api/desktop-integration.md#clipboard) wraps `arboard`, which talks to whatever clipboard mechanism the current session provides (X11 selections, or Wayland's `wl_data_device` protocol) -- no platform-specific code on your side. The real, disclosed scope limit is content type, not platform: plain UTF-8 text only, verified round-tripping accented Latin, CJK, and emoji correctly. Images and rich text are not supported.

Construct one `Clipboard` per application and reuse it -- opening the connection isn't free, and (per the [thread-affinity rule](python-api/index.md)) it must stay on the thread that created it.

## Native file dialogs

[`pick_file`/`pick_files`/`pick_folder`/`save_file`](python-api/desktop-integration.md#native-file-dialogs) go through the XDG desktop portal via `rfd` -- this means the *actual dialog UI* is drawn by whatever portal backend your desktop environment registers (GTK's portal on GNOME-based desktops, KDE's own portal on Plasma), not by this project. Both have been confirmed registered and working on a real KDE Plasma session. If no portal implementation is running (a minimal window manager with no desktop environment), these calls will fail -- there's no fallback to a toolkit-native dialog.

## System tray

The tray icon and menu (`tre.TrayIcon`/`tre.Menu`, see [Desktop Integration](python-api/desktop-integration.md#system-tray)) are the one feature area with real setup ordering to get right:

1. Call `tre.tray_init()` once, before constructing any `Menu`/`TrayIcon`, on the thread that will own them.
2. Call `tre.tray_pump_events()` **every frame** -- tray/menu state lives inside a real GTK event loop, and nothing else in `tre`'s own windowed loop drives it. Skipping this means the icon never actually appears, clicks never register, and icon updates never take effect.
3. Call `tre.tray_poll_events()` every frame too, alongside step 2, to drain queued clicks.

Under the hood this uses `libappindicator` on Linux, which needs a real tray-watcher service running (`org.kde.StatusNotifierWatcher` on KDE Plasma, confirmed via `dbus-monitor` to genuinely register) -- a desktop with no such service (again, a bare window manager) will fail to create a `TrayIcon` at all. Icon updates (`set_icon`) work by writing a real temporary PNG file to disk and repointing the indicator at it, since this backend has no in-memory icon API; `set_temp_dir_path` lets you redirect where that file lands if you need to.

**Predefined menu items are deliberately not used.** `tray-icon`'s built-in `Copy`/`Cut`/`Paste`/`SelectAll` items depend on `libxdo`, an X11-only library -- irrelevant, and non-functional, on a Wayland session. Build plain `Menu` items instead and back them with [`tre.Clipboard`](python-api/desktop-integration.md#clipboard) directly (dispatch on the item id from `TrayEvent.MenuItemClick`), which works identically on both X11 and Wayland.

## Accessibility

[`tre.A11yBridge`](python-api/accessibility-and-focus.md#accessibility) publishes your app's tagged accessibility tree over AT-SPI2, the real Linux desktop accessibility bus, using the `accesskit`/`accesskit_unix` crates. Everything runs in your application's own process -- there is no second process, handoff file, or IPC mechanism required for a real app to publish its own tree, even one doing real Vulkan rendering in the same process.

If no AT-SPI2 registry is reachable (no accessibility service running, or you're not on Linux), `A11yBridge` degrades gracefully: construction never fails, and `publish()` silently becomes a no-op rather than raising. You don't need to detect this yourself.

This is Linux-only. Windows UI Automation and macOS NSAccessibility are not implemented.

## Focus and keyboard navigation

[`tre.FocusManager`](python-api/accessibility-and-focus.md#focus-and-tab-order) is pure logic with no platform dependency at all -- it works identically everywhere `tre-python` runs. It's listed here because it's the natural counterpart to `A11yBridge`: a screen reader announcing your UI generally expects the same widget that's visually focused to be the one it's currently describing, so a real accessible app typically drives both from the same underlying state.

## Drag-and-drop and IME

Both arrive as ordinary [`InputEvent`](python-api/input-events.md) variants from `poll_events()` -- `FileDropped`/`FileHovered`/`FileHoverCancelled` for drag-and-drop, `ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled` for IME composition. There's no separate API to opt into drag-and-drop; it's always active. IME composition events only start arriving after you call `renderer.set_ime_allowed(window, True)` -- enable it only while a text field actually has focus, and disable it again when focus leaves, matching how a real IME expects to be toggled.