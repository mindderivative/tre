# Upgrading to 0.5.0

No name was removed or renamed in 0.5.0: code written for 0.4.x runs
unchanged. 0.5.0 adds custom windowing — a window can drop the OS's title
bar and the framework draws its own; see
[Custom Title Bars](guide/custom-title-bars.md). One behavior changed, and
it is listed first in case your app relied on the old one.

## Changed

- **A window root's fill paints.** `window.root.set(fill=...)` was accepted
  but never drawn, from 0.2.0 on: the root showed the window's clear color
  (black) instead. It now paints, like any box's fill, with the root's
  corner radius and border. An app that set a root fill will now see it;
  one that relied on the black background should remove the fill.

## Added

- **Undecorated windows.** `Window(decorations=False)` and
  `window.set(decorations=...)`. On macOS the title bar stays as a
  transparent overlay with the traffic lights.
- **Window controls and state.** `window.minimize()`, `maximize()`,
  `restore()`, and `close()` (which fires `close_requested` first);
  `get("maximized")`, `"minimized"`, `"active"`, and `"platform"`; the
  `maximized` and `active` window events.
- **Window properties.** `fullscreen`, `min_width`/`min_height`, and `icon`.
- **Title bars and borders.** The `window_region` node property (`"drag"`,
  `"none"`), a double-click to maximize, `window.set(resize_border=N)`, and
  the OS's window menu, opt-in with `window.set(system_menu=True)`.
- **`pointer_cancel`.** A node event: the OS took a press — to move the
  window, maximize, or show its menu — so no `pointer_up` or click follows.
  A widget that only listens for `pointer_up` and `click` is unaffected,
  and only presses on a drag region are taken.
- **macOS.** `get("titlebar_inset")`, `get("native_controls")`, and the
  `titlebar_inset` window event.

See [Window](api/python/window.md#window-controls-and-state) for the
reference and `examples/custom_titlebar.py` for all of it in one window.
