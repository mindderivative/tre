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

See [Window](reference/window.md#window-controls-and-state) for the
reference and `examples/custom_titlebar.py` for all of it in one window.

## 0.5.0.1

- **A real click in a text field no longer panics.** A mouse press in a text
  field or a terminal, and a drag to select in either, ended the app with
  `RefCell already borrowed`. It did in 0.4.4 and 0.5.0 alike, and only on a
  real window: `window.simulate` never reached the code, so no headless test
  saw it. Nothing in your code needs to change.

## 0.5.1

- **`padding` insets a node's own content.** Layout always reserved the
  padding, but a node drew its content at its own corner across its full width,
  so the padding did nothing visible. A text node's text, a text input's text
  and caret, a terminal's cells, an image, a path's view box and a canvas's
  painter coordinates now all start inside the padding and fit what is left;
  the node's `width` and `height` include it, as for a box, and its background,
  border and rounded corners stay on the whole box. A node with no padding is
  drawn exactly as before; one that set `padding` and relied on it being
  ignored should drop it. A canvas's painter `(0, 0)` is now its content box's
  corner, and a custom hit shape (`set_hit_test_circle`, `set_hit_test_path`)
  is in those coordinates; pointer events' `x` and `y` stay relative to the
  node's own corner. Clicks land on what is painted: a click in a text input
  or a terminal with padding hits the character or cell under it, and a click in
  a scrolled multiline field now hits the line under the pointer, where it
  used to resolve as if the field had not scrolled.
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
- **A GPU error no longer panics, and a lost GPU ends the run cleanly.** `wgpu`'s
  default handler panicked the process on any GPU error (a `PanicException` out
  of `App.run()`), and a hung or reset GPU left a frozen window with nothing to
  react to. An error is now logged once and fires a `gpu_error` window event, and
  the draw is skipped. A lost GPU fires `gpu_lost` and `App.run()` raises
  `RuntimeError("the GPU was lost: …")`. `window.set(gpu_watchdog=seconds)` opts in
  to a `gpu_stalled` event for a frame that never finishes. Code that caught
  `PanicException` to survive a GPU error should listen for `gpu_error` instead.
  See [GPU health](reference/window.md#gpu-health).
- **`tre.Shader` and `node.set(shader=...)`.** WGSL shaders arrive in 0.5.1 in
  stages. This stage adds the object and the property: a shader is checked when
  it is created, raising `tre.ShaderError` (a `ValueError`) with the line and
  column in your source, and every node kind accepts, stores, and reads back
  `shader`. A `mode="fill"` shader draws its node's box behind the node's own
  paint, an `animated=True` shader sees `frame.time` and keeps the window
  drawing, a `mode="effect"` shader transforms the node's own rendered content,
  and `inputs` let a shader sample image, video, and other shader nodes. See
  [Shader](reference/shader.md).
- **An idle window no longer starves other Python threads.** Through 0.5.0.1,
  `App.run()` waited for the next event with the GIL held, so a worker thread
  (a file watcher, the background loader in the
  [threading guide](guide/threading.md)) could not run while the window sat
  idle, and its `LoopHandle.call_soon` never arrived. The loop now releases the
  GIL for its wait alone; every callback still runs on the loop thread with the
  GIL held. A workaround that ticks the window to keep the loop spinning is no
  longer needed.

## 0.5.2

- **Python 3.12 or newer.** 0.5.2 drops the wheels for CPython 3.9, 3.10 and
  3.11 and for PyPy 3.11 (which is Python 3.11 compatible), and `requires-python`
  is `>=3.12`. `pip` on an older Python keeps installing 0.5.1 and earlier.
  Nothing in the API changes.

## 0.5.3

- **A text node's width is rounded up.** Layout rounds boxes to whole pixels, and
  a text node given the width `measure_text` returned (66.43, say) was laid out
  66 wide, so text that exactly fit wrapped. A text node's explicit `width`,
  `min_width` and `max_width` are now laid out rounded up, so such a width fits.
  `layout_width` can be up to a pixel wider than before for a text node with a
  fractional width; `get("width")` still reads back what you set. Whole-pixel
  widths, percentages, `auto`, and every other kind are unchanged. A workaround
  that rounds measured widths up yourself is no longer needed, and harmless.

## 0.5.4

- **An animating window is paced to the display.** The swapchain used the first
  present mode the driver listed, which on Linux with Mesa is `Mailbox`: it never
  waits for the display, so any continuous animation, even one box, ran the loop
  at thousands of frames a second and used a whole CPU core (measured: 97.5% of a
  core, now 7 to 9%). The default is now vsync. A still window was never affected
  and still costs nothing. An app that wants the old behavior, the newest frame at
  once, asks for it with `window.set(present_mode="low_latency")`, live or before
  `App.run()`. macOS and Windows already listed a vsync mode first, so little
  changes there.

