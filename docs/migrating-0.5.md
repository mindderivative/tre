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

## 0.5.5

- **Resizing a window no longer stalls on KDE Wayland.** 0.5.4's vsync default made a
  window resized by its edge (an undecorated window with `resize_border`, most
  noticeably) stall in bursts of 0.1 to 1.1 s and trail the pointer. While a window is
  being resized it now presents without vsync, and returns to `present_mode` about
  0.4 s after the last resize; see [Window](reference/window.md). Nothing to change in
  an app. If you worked around it with `present_mode="low_latency"`, you can remove
  that and get the vsync saving back while something animates.

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

- **HiDPI scaling is available, and off.** `window.set(dpi_scaling=True)` lays
  the window out in logical pixels and draws it at the display's
  `scale_factor`, so an app written at 1x is the same size and sharp on a 2x
  screen. `width`, `height`, pointer positions and pixel scroll deltas become
  logical. It is off by default, so nothing changes for an app or framework that
  already multiplies by `scale_factor` itself; turn it on only if yours does not.
- **Animations cost what they run, not what the tree holds.** Each frame used to
  visit every node to advance its animations (0.6 to 1.3 ms at 9,000 nodes, even
  with one card animating). The tree now ticks only the nodes known to be
  animating, plus the nodes a new animation could have started on (the ones
  changed since the last tick), never a pass over the whole tree: 75 ns per frame
  in the same case. Image and SVG nodes are likewise tracked, not searched for each
  frame. Nothing to change in an app.
- **Scrolling a big view redraws less.** A scroll that moves a whole view of
  200 nodes or more is carried by copying the kept pixels, with only the strip
  it uncovers and the scrollbar redrawn; see [Window](reference/window.md).
  Pixels are identical, except that text edges in the copied area may differ by
  one level in a pixel or two from a fresh draw. `TRE_SCROLL_BLIT_OFF=1`
  turns it off.
- **The damage walk is about 2.4x cheaper.** Working out what changed each frame
  fingerprinted every visible node into a hashed map built from scratch: 3.7 ms
  at 9,000 nodes, now 1.5 ms, from keeping the records in a slot-indexed array
  reused between frames and not re-bounding nodes that paint only their box.
  The result is unchanged; the cost still grows with the tree.
- **Scrolling no longer runs layout.** A scroll view's or virtual list's offset used
  to be written into its content's layout position, so every scrolled frame
  re-ran layout (1.55 ms with 3,000 rows). The offset is now a paint-time shift,
  `Tree::scroll_shift`, that painting, hit testing, accessibility bounds and
  `absolute_position` all read, and layout skips a call whose answer it already
  has: 8.5 µs per scrolled frame. One visible consequence for code that
  reads layout directly: a scrolled child's layout position is where it sits
  unscrolled; use `absolute_position` for where it is on screen.
- **A node is 60% smaller.** `Node` went from 3,048 to 1,208 bytes: a running
  animation is boxed and exists only while it runs (it was inline in every
  animatable value, 1,640 bytes of paint state per node, now 304), and the two
  largest node kinds, text inputs and terminals, are boxed so every other node
  stops paying for them. 50,000 boxes now take about 120 MB (the baseline
  measured about 210 MB). Rust code that builds a node from state by hand
  wraps it: `NodeKind::TextField(Box::new(state))`.
- **Colours are drawn as specified.** A window rendered into an sRGB surface
  format, which is what the driver lists first on Linux and most desktops, so
  the GPU encoded the renderer's already-encoded colours a second time: a
  fill of `(103, 80, 164)` showed as `(170, 152, 210)`, and every colour was
  lighter and flatter than asked. The engine now chooses a non-sRGB format
  where one exists, and colours on screen are the ones you set. If you chose
  colours by eye against the old output, expect them to look darker and more
  saturated now; the numbers you wrote are what you get.
- **A window can be read as pixels.** `window.snapshot()` returns what it draws
  as RGBA bytes, offscreen and without a display, and `tre.write_png` saves it
  (see [Window](reference/window.md#snapshot)).
- **Fills can be gradients.** A box's `fill` takes a `tre.Gradient`
  (`Gradient.linear`, `.radial`, `.sweep`) as well as a colour, and animates
  between compatible ones (see [Gradients](reference/paint.md#gradients)).
  Nothing changes for a fill set to a colour. A gradient is also accepted by a
  path's or text node's `fill`, by `stroke_color` (borders and path strokes) and
  by a canvas painter's `fill_rect`, `fill_circle` and `stroke_path`, each over
  the shape's own bounds.
- **Links, per-span size and family, and more static-text selection.** A text
  span takes `link` (a click fires a bubbling `link` event with `href`),
  `font_size` and `font_family`; selectable text selects a word on double-click and
  a line on triple-click, Ctrl+A and Shift+arrows work on it, and the pointer is an
  I-beam over it and a hand over a link; see
  [Rich text and selectable text](guide/text.md#rich-text-and-selectable-text).
  Nothing changes for text with no link and no new span fields, except the I-beam
  over selectable text.
- **Text selection and links reach screen readers.** Selectable text and text with
  link spans are exposed as text runs and link nodes with a text selection, and a
  screen reader can follow a link or set the selection; see
  [Text, selection and links](guide/accessibility.md#text-selection-and-links).
  Other text is unchanged.
- **Selection across texts.** A drag that starts in one selectable text and moves
  into others selects across them, and Copy joins the pieces with newlines; see
  [Rich text and selectable text](guide/text.md#rich-text-and-selectable-text).
  Before, only one text could hold a selection.
- **More keyboard selection in static text.** Ctrl+Shift+Left/Right move the
  selection a word and Shift+Up/Down a line, and Shift+arrows carry on into the next
  selectable text; see [Rich text and selectable text](guide/text.md#rich-text-and-selectable-text).
- **Text interaction in tests.** `Window.simulate` pointer, touch and key events now reach
  text selection, links, double and triple click, and Shift+Up/Down, so an app can test
  a link click or a drag selection headlessly; see
  [Testing without a display](reference/events.md#testing-without-a-display).
- **Colour filters.** `tre.Shader.filter(grayscale=1.0, ...)` is a ready-made
  effect shader for the CSS filter functions `saturate`, `brightness`, `contrast`,
  `grayscale`, `hue_rotate`, `invert` and `sepia`, applied in the order given; see
  [Shader.filter](reference/shader.md#shaderfilterfilters). Nothing changes for
  existing shaders.
- **Blur, frosted glass, and blend modes.** A node takes `blur` (a Gaussian blur
  of itself and its subtree), `backdrop_blur` (what is behind it, blurred,
  inside its box) and `blend_mode` (CSS `mix-blend-mode`); see
  [Blur, frosted glass, and blend modes](reference/paint.md#blur-frosted-glass-and-blend-modes).
  All are off by default and change nothing for existing nodes.
- **Text in any language, opt-in.** `tre.set_system_fonts(True)` lets text use
  the machine's installed fonts for the glyphs the bundled fonts lack (CJK,
  Hebrew, Indic, Thai, colour emoji), and the text guide has a recipe for
  shipping fonts instead (see [Other languages and
  emoji](guide/text.md#other-languages-and-emoji)). It is off by default, so
  nothing changes unless you turn it on.
- **Rich text and selectable text.** A text node's `spans` style ranges of its
  content (colour, weight, italic, underline, strikethrough), and
  `selectable=True` lets the user select and copy it; see [Rich text and
  selectable text](guide/text.md#rich-text-and-selectable-text). Both are off by
  default. A selectable text claims pointer events over its box; plain text still
  never does.
- **Touch and gestures.** Fingers arrive as `touch_start`, `touch_move`,
  `touch_end` and `touch_cancel`; `tap`, `long_press`, `pan` and `pinch` are
  recognized from them (a trackpad pinch too), the first finger also drives the
  pointer so existing click and hover code works, and a pan scrolls what is under
  it; see [Touch and gestures](guide/events-and-input.md#touch-and-gestures).
  Nothing changes on a machine with no touch screen. Written against simulated
  touches; not yet checked on touch hardware.
- **Files dragged from the OS.** `file_hover`, `file_hover_cancel` and `file_drop`
  events (with `paths`) reach the node under the pointer and the window; see
  [Files dragged from the OS](guide/events-and-input.md#files-dragged-from-the-os).
  They work on Windows, macOS and X11, not Wayland.
- **The OS's reduced-motion and increased-contrast preferences.**
  `window.get("reduced_motion")` and `get("high_contrast")` read them, and
  `reduced_motion` and `high_contrast` window events report changes; see
  [Window properties](reference/events.md#window-properties). The engine only
  reports them; honouring them is the app's.
- **Frame statistics.** `window.frame_stats()` reports what a window's frames cost
  (stage times, redraw kind, damage, nodes, recent fps and percentiles), and a
  `frame` window event fires per frame; see [frame_stats](reference/window.md#frame_stats).
  Always on, a few clock reads a frame. It now also reports GPU time where the adapter
  can measure it, can say which node kinds and nodes the scene-building time went to
  (`window.set(profile_nodes=True)`), can write a Chrome / Perfetto trace
  (`start_trace`/`stop_trace`), and can be read from another thread (`stats_handle()`).
- **Flicked scrollers coast.** Lifting a finger mid-flick on a scroll view or
  virtual list lets it coast to rest (see [Touch and
  gestures](guide/events-and-input.md#touch-and-gestures)), and an animation of
  a scroll offset is now stopped by a manual scroll instead of fighting it.
- **Transparent windows.** `window.set(transparent=True)` before `App.run()` opens a
  see-through window (with `get("transparent_active")` saying whether the surface
  can blend), and `set(blur_behind=True)` asks the compositor to blur behind it
  where it can; see [Window](reference/window.md). Two fixes ride along: the
  surface format now prefers an 8-bit one (Mesa on Wayland listed a ten-bit
  format first, which has two bits of alpha), and `snapshot()` now returns
  true straight alpha, as documented (it had returned the renderer's
  premultiplied pixels, so translucent pixels were too dark).
- **Spring animation.** `easing="spring"` or `("spring", bounce)` animates with a
  damped spring that lasts until it settles and, on a number, carries on the speed of
  the animation it interrupts; see [Springs](guide/animation.md#springs). Existing
  easings are unchanged.
- **Sticky headers.** `node.set(sticky=inset)` holds a node at the start edge of
  its scroll view while its parent scrolls past, like CSS `position: sticky`; see
  [Sticky headers](guide/nodes-and-layout.md#sticky-headers). Off by default.
- **Custom cursors.** `tre.CursorImage(rgba, width, height, hotspot)` is a pointer
  shape drawn from pixels, accepted anywhere a node's `cursor` takes a name; see
  [Cursors](reference/node.md#cursors). The named shapes are unchanged.
- **SVG documents.** `window.create("svg", svg=...)` paints a whole SVG file as
  one node (shapes, strokes, gradients, group opacity, clip paths), fitted into
  its box; see [SVG documents](reference/paint.md#svg-documents). It is a new
  kind, so nothing changes for existing nodes. It adds a dependency, `usvg`,
  with its `text` feature, so an SVG's text is drawn from the engine's own
  fonts (no system fonts). `svg_color` sets what `currentColor` is, `get("svg")`
  reads the source back, and a node sized on one side takes the other from the
  document. Masks and drop shadows are drawn too, and an SVG's text follows
  `set_system_fonts` and registered fonts.
  Raster pictures inside an SVG are the framework's to decode: pass the pixels
  as `svg_images`, keyed by `href`.
- **Click-through windows.** `window.set(click_through=True)` makes the whole
  window ignore the pointer, so clicks reach what is behind it; see
  [Transparent windows](reference/window.md). Off by default.
- **Cheaper text.** `window.set(glyph_cache=True)` draws text from a glyph cache,
  about four times cheaper to build per label, at the price of edge pixels that
  differ slightly from the default outline drawing (up to about 45 of 255); see
  [Window](reference/window.md). Off by default, so nothing changes unless asked.
