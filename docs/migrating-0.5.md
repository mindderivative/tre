# Upgrading to 0.5.x

No name was removed or renamed in 0.5.x: code written for 0.4.x runs
unchanged. 0.5.0 adds custom windowing — a window can drop the OS's title
bar and the framework draws its own; see
[Custom Title Bars](guide/custom-title-bars.md). A few behaviors changed, and
each is listed here with the release that changed it, in case your code or
tests relied on the old one.

## 0.5.0

### Changed

- **A window root's fill paints.** `window.root.set(fill=...)` was accepted
  but never drawn, from 0.2.0 on: the root showed the window's clear color
  (black) instead. It now paints, like any box's fill, with the root's
  corner radius and border. An app that set a root fill will now see it;
  one that relied on the black background should remove the fill.

### Added

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

## 0.5.0.1

- **A real click in a text field no longer panics.** A mouse press in a text
  field or a terminal, and a drag to select in either, ended the app with
  `RefCell already borrowed`. It did in 0.4.4 and 0.5.0 alike, and only on a
  real window: `window.simulate` never reached the code, so no headless test
  saw it. Nothing in your code needs to change.

## 0.5.1

- **`padding` on a text node insets its text.** Layout always reserved the
  padding, but the text was drawn at the node's corner across its full width,
  so the padding did nothing visible. The text now starts inside the padding
  and wraps in the width that's left, and the node's `width` and `height`
  include it, as for a box. A text node with no padding is drawn exactly as
  before; one that set `padding` and relied on it being ignored should drop
  it. A `text_input`, `terminal`, `image`, `path`, and `canvas` still ignore their
  own padding: put them in a padded box.
- **Every kind paints its own box.** `fill`, `stroke_color`, `stroke_width`,
  and `corner_radius` were accepted on every kind but painted only on a box
  (and a few on a text input, terminal, and path): `set` succeeded, `get` read
  the value back, and nothing showed. An image, canvas, scroll view, and
  virtual list now paint their `fill` behind their content, a text, text
  input, and terminal paint a border and rounded corners, and an image is
  clipped to its rounded box. A `text` and a `text_input` keep `fill` as the
  glyph color, and a path keeps `stroke_*` as its outline and ignores
  `corner_radius`. An app that set one of these on a kind that ignored it will
  now see it; remove it to go back.

