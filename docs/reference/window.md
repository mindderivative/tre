# `Window`

Owns one node tree and the OS window it's painted into. Add windows to an
[`App`](app.md), then call `App.run()`.

**`Window(width=480, height=200, title="tre v2", decorations=True)`** —
raises `ValueError` for a zero width or height. `decorations=False` (0.5.0)
opens the window without the OS's title bar and borders, for the framework
to draw its own; see [Window controls and state](#window-controls-and-state).

```python
window = Window(width=640, height=400, title="Inbox")
```

## Nodes

**`create(kind, **props) -> Node`** makes a detached node of `kind` —
`"box"`, `"text"`, `"text_input"`, `"image"`, `"path"`, `"svg"`,
`"canvas"`, `"scroll_view"`, `"virtual_list"`, or `"terminal"` — and sets `props`
atomically, as `Node.set` does: a bad property raises `ValueError` and
nothing is created. Every kind and property is on
[Nodes and Properties](properties.md).

**`root`** is the tree's root node: a flex row with 16px padding and 16px
gaps, sized to the window. Attach nodes under it with `add_child`.

```python
card = window.create("box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=200, height=120,
                     corner_radius=12)
window.root.add_child(card)
```

## Layers

**`show_layer(node, anchor=None, placement="below", modal=False,
dismissible=True)`** shows `node` over the window's content, and
**`hide_layer(node)`** hides it — for menus, dialogs, tooltips, and sheets.
See [Layers](layers.md).

## Window controls and state

Since 0.5.0, a window can be run without the OS's title bar and borders
(`decorations=False`, or `set(decorations=...)` live), so the framework can
draw its own — and needs these to wire it up. See the
[custom-windowing design](../design/custom-windowing.md).

**`minimize()`**, **`maximize()`**, and **`restore()`** act on the open
window; before `App.run()` they set how it opens. **`close()`** closes the
window as if the user had: `close_requested` fires first and a listener can
cancel it. It happens on the loop's next turn, not during the call, and a
window that isn't open has nothing to close.

```python
minimize_button.on("click", lambda: window.minimize())
maximize_button.on("click", lambda: window.restore() if window.get("maximized") else window.maximize())
close_button.on("click", lambda: window.close())
window.on("maximized", lambda e: swap_icon(e.maximized))
window.on("active", lambda e: title_bar.set(opacity=1.0 if e.active else 0.6))
```

| Property | Set | Get |
| --- | --- | --- |
| `decorations` | Whether the OS draws the title bar and borders | The open window's answer |
| `fullscreen` | Borderless fullscreen on the window's monitor | Whether it is |
| `min_width`, `min_height` | The smallest size the user can resize to; 0 for none | The setting |
| `icon` | `(rgba, width, height)` — RGBA8 bytes, `width * height * 4` of them — or `None`; shown on Windows and X11 (Wayland and macOS take the app's icon from its desktop file or bundle) | — |
| `maximized`, `minimized` | — (use the methods above) | The open window's state, or before `App.run()` how it opens |
| `active` | — | Whether the window has the OS's focus |
| `platform` | — | `"wayland"`, `"x11"`, `"windows"`, or `"macos"` |

Each settable one applies live to an open window, or when `App.run()` opens
it. A window smaller than its minimum is grown to it, when the minimum is
set and after any resize: on Wayland a minimum otherwise only limits what
the user can drag it to, and leaving fullscreen can restore a smaller size.
A maximized or fullscreen window isn't resized to its minimum. Nor is a
window the compositor holds at its size: on Wayland a compositor can report a
window as tiled, and a tiled window may not resize itself (COSMIC reports
every window this way). The minimum is still passed to the compositor as a
hint, but the window keeps its size, and `get("width")` and `get("height")`
give the real one, so don't assume a minimum was reached. Two window
events report state changes: **`maximized`** (`event.maximized`) when the
window is maximized or restored, and **`active`** (`event.active`) when it
gains or loses focus — each only when the value changes.

On Windows an undecorated window keeps its shadow. macOS is different; see
below.

### macOS: the overlay title bar

`winit` can't let the user resize an undecorated macOS window, so there
`decorations=False` doesn't remove the title bar: it makes it transparent
and runs the content up under it. The window keeps its shadow, native
resizing, and the traffic lights (close, minimize, zoom) in its top-left
corner, over the app's own title bar. `get("decorations")` is still
`False`. Two read-only properties say what's there, so one layout works on
every platform:

| Property | Get |
| --- | --- |
| `titlebar_inset` | `(height, width)` in logical pixels: the title-bar strip's height and the traffic lights' width with their margins. `(0.0, 0.0)` for a decorated window, in fullscreen (the traffic lights are hidden), and on every other platform |
| `native_controls` | Whether the OS shows its own window controls over the content: `True` only for an undecorated macOS window outside fullscreen. A framework hides its own minimize/maximize/close when it is |

The **`titlebar_inset`** window event (`event.titlebar_inset`) fires when
the inset changes — entering or leaving fullscreen, say.

```python
window = Window(decorations=False)
bar = window.create("box", height=40, window_region="drag")

def lay_out_bar(inset=None):
    height, width = inset or window.get("titlebar_inset")
    bar.set(padding_left=width)  # clear of the traffic lights
    for button in own_buttons:
        button.set(visible=not window.get("native_controls"))

window.on("titlebar_inset", lambda e: lay_out_bar(e.titlebar_inset))
lay_out_bar()
```

A drag region can be taller than macOS's own title-bar strip; all of it
moves the window. The resize border is off on macOS, where the OS resizes
the window itself. A double-click on the drag region does what the user
chose in System Settings (Desktop & Dock, "Double-click a window's title
bar to"): zoom, minimize, or nothing.

### Title bar and borders

An undecorated window has no title bar to move it by and no border to resize
it by (0.5.0). The framework marks its own; the OS does the moving and
resizing.

```python
window = Window(width=800, height=600, decorations=False)
window.set(resize_border=6, min_width=320, min_height=200)
bar = window.create("box", height=36, window_region="drag")
close = window.create("box", width=36, height=36, focusable=True)
close.on("click", lambda: window.close())
bar.add_child(close)                              # a button in the bar stays a button
bar.on("pointer_cancel", lambda: bar.set(opacity=1.0))
```

**The drag region.** A primary press on a node with `window_region="drag"`,
or on a node inside it that isn't interactive, moves the window. A node is
interactive when it's `focusable`, a text field or terminal, has a `click`
listener, or holds the pointer capture; only the nodes from the one pressed
up to the drag region are checked. `window_region="none"` turns dragging off
for a node and what's inside it, and `"drag"` on a node inside a button turns
it back on (a draggable icon, say). The press still delivers `pointer_down`
first, then — because the OS takes the pointer for the move — `pointer_cancel`
to the pressed node: no `pointer_up` or `click` follows, and any capture is
released.

**Double-click** on the drag region toggles maximize (on macOS, what the
user chose; see above). The two presses must fall within the system's
double-click time (Windows, macOS; 500 ms elsewhere) and 4 px of each other.

**The window menu** is opt-in: `window.set(system_menu=True)`, on any
platform. Then a secondary press on the drag region opens the OS's window
menu (Restore, Move, Size, Minimize, Maximize, Close) where the platform has
one — Windows, and Wayland compositors that offer one; elsewhere nothing
opens — and ends the press with `pointer_cancel`, so no `secondary_click`
follows. On Windows, Alt+Space on an undecorated window opens it too. Off,
the default, a secondary press on the drag region is an ordinary one, for
the framework's own menu.

**The resize border.** `set(resize_border=N)` makes a press within `N`
logical pixels of an edge resize the window from that edge, or from a corner
where two edges meet. Over the border the pointer shows the matching resize
cursor. A press there never reaches a node, and nor does its release. The
border is off for a decorated window (the OS has one), on macOS (the OS
resizes it), while maximized or fullscreen, and at `0`, the default.

| Property | Set | Get |
| --- | --- | --- |
| `resize_border` | Border width in logical pixels, a number `>= 0` | The setting |
| `system_menu` | Whether a secondary press on the drag region (and Alt+Space on Windows) opens the OS's window menu; `False` by default | The setting |

In fullscreen the drag region and the border do nothing: their presses are
ordinary ones, delivered to the nodes as usual. The move, resize, and menu are the OS's and need a real
display; `simulate` exercises everything else — the rule, `pointer_cancel`,
double-click timing (with `advance`), and the border — without one.

## GPU health

Since 0.5.1, tre listens to the GPU, and three window events report what it
says. The device is polled each loop turn, and while GPU work is still running
an otherwise idle loop wakes about every 100 ms to poll; with the GPU idle it
costs nothing.

```python
window.on("gpu_lost", lambda e: log.error("GPU lost (%s): %s", e.reason, e.message))
window.on("gpu_error", lambda e: log.warning("GPU error: %s", e.message))
window.set(gpu_watchdog=10)   # opt in to gpu_stalled
window.on("gpu_stalled", lambda e: log.warning("a frame has run %.0f s", e.seconds))
```

- **A lost GPU ends the run.** A driver fault, or a hang the driver's own
  timeout caught (TDR on Windows, with Linux's and Metal's own timeouts),
  reaches tre as a lost device, and a lost device can't be restored. `gpu_lost`
  fires with `reason` (`"unknown"` for a fault, `"destroyed"` for a destroyed
  device) and the driver's `message`; then every window closes — a
  `close_requested` listener can't keep one open — and `App.run()` raises
  `RuntimeError("the GPU was lost: …; the run has ended")` instead of
  panicking or freezing. Nothing is rebuilt.
- **A GPU error doesn't panic.** `wgpu`'s default handler panics the process on
  any GPU error; tre replaces it. Each distinct error is logged once and fires
  `gpu_error` once, the draw that caused it is skipped, and the loop carries on.
- **The stall watchdog is off.** `window.set(gpu_watchdog=seconds)` turns it
  on (`None`, the default, turns it off): if a submitted frame hasn't completed
  after that many seconds, tre logs once and fires `gpu_stalled`. It only
  reports; stuck GPU work can't be cancelled. It matters most on a software
  adapter (the one CI uses), which has no driver timeout at all: a shader that
  never ends there is never reported lost.

`simulate("gpu_lost", reason=, message=)`, `simulate("gpu_error", message=)` and
`simulate("gpu_stalled", seconds=)` deliver each event to its listener with no GPU,
for tests.

## Window events and properties

**`on(event, handler)`** and **`off(event)`** listen to the window itself —
`resize`, `color_scheme`, `scale_factor`, `close_requested`, `closed`,
`dock_target`, `dock_drop`, (0.5.0) `maximized`, `active` and `titlebar_inset`,
and (0.5.1) `gpu_lost`, `gpu_error` and `gpu_stalled`; see
[Events and Listeners](events.md#window-listeners).

**`set(title=...)`** changes the title, live if the window is open.
**`set(partial_redraw=False)`** makes the window redraw all of itself every
frame. By default (`True`) each frame redraws only the parts of the window
that changed, and a frame where nothing visible changed draws nothing.
The pixels are the same either way; the switch is there for measuring and
for ruling partial redraw out when chasing a rendering bug. A window whose
surface the platform won't let Tesserae Engine copy into always redraws in full, and
logs a warning saying so; `get("partial_redraw")` reports the setting, and
`get("partial_redraw_active")` whether it's in effect (`None` until
`App.run()` opens the window).

**`set(glyph_cache=True)`** (0.5.4) draws text from a glyph cache instead of
drawing each glyph's outline every time. A label costs about a quarter as much to
build (a 36-character label, 28 µs to 6 µs; a screen of labels, 18.8 ms to 5.8 ms),
which matters for a window with hundreds of text nodes that redraw. It is not
pixel-identical: edge pixels differ from the outline path by up to about 45 of 255
(80 at a scale factor of 2), and the glyph cache is marked experimental upstream.
So it is off by default, and `get("glyph_cache")` reports it. Switching it redraws
the whole window once. `snapshot()` follows the window's setting.

**Scrolling** (0.5.4) is the one change a partial redraw can carry as a block. When a
scroll view or virtual list scrolls and nothing else changes in or over it, the window
copies the view's kept pixels by the scroll distance and redraws only the strip the
scroll uncovered, its scrollbar, and anything that did not move with it, instead of the
whole view. It does this only when the copy is exact and worth it: the view has an opaque
plain background (no gradient, border or rounded corners), the scroll is a whole number of
device pixels, everything under it moved by the same distance, no other node paints over
it, nothing above it fades, blends or blurs, and at least 200 nodes move with it (a copy
costs about as much as redrawing fewer). Anything else redraws as before. Text edges drawn
by the copy can differ from a fresh draw by one level in a pixel or two, because a glyph
is rasterised from where it sits in the window; nothing else differs, and a later full
redraw re-draws them. How much it saves depends on the machine and the content: it
removes the redraw of the view, but not the walk over its nodes. Set the environment
variable `TRE_SCROLL_BLIT_OFF=1` to turn it off while measuring or chasing a rendering bug.
**`set(transparent=True)`** (0.5.4) opens the window see-through: the OS gives it an alpha
channel, so a root `fill` with alpha below 255, or `(0, 0, 0, 0)`, shows the desktop
through it. With `decorations=False` that is a window of any shape: a rounded card,
a floating palette, a splash. It must be set before `App.run()`, because the OS fixes
it when the window is made (X11 can only choose it then); setting it on an open window
raises `ValueError`. Once the window is open, `get("transparent_active")` says whether
it took: `True` when the surface blends with the desktop, `False` where it can't
(there is no premultiplied or inherited alpha mode), and the window is then opaque, so
an app can fall back to a solid background. It is checked live on KDE under both
Wayland and X11, where a 50% red box over a green window behind it composites to exactly
`(128, 127, 0)`.

```python
window = Window(width=320, height=200, decorations=False)
window.set(transparent=True)
window.root.set(fill=(0, 0, 0, 0))                 # nothing of its own
card = window.create("box", width=300, height=180, x=10, y=10, position="absolute",
                     corner_radius=24, fill=(0x1C, 0x1B, 0x1F, 0xE6))   # 90% opaque, rounded
window.root.add_child(card)
```

**`set(blur_behind=True)`** asks the compositor to blur what is behind a transparent
window, live. It works only where the platform has it: Wayland with KDE's blur protocol,
and macOS; it is ignored on X11 and Windows, so treat it as a polish and keep the
window readable without it.

**`set(click_through=True)`** (0.5.4) makes the whole window ignore the pointer: clicks,
scrolls and hover go to whatever is behind it, and the window gets none. It takes effect
live, or at open if set before `App.run()`. The window is all or nothing: winit has no
per-region input shape, so a window whose opaque parts should still take clicks cannot
have its transparent parts pass them through. A platform that cannot do it raises
`ValueError`. A click-through window is for overlays and HUDs, so give the app a
keyboard shortcut or another window to turn it off again, since the window itself
can no longer be clicked.

**Placement (0.5.6).** `set(x=, y=)` puts the window's top-left corner at that place on the
desktop, in logical pixels; with only one of them, the other stays where the window is.
`get("x")`/`get("y")` read the window's place (before it opens, what was set), or `None`
where the system doesn't say. `center()` centres the open window on its monitor and
returns `False` if it can't. `set(always_on_top=True)` keeps the window above others,
`set(resizable=False)` stops the user resizing it, and `set(skip_taskbar=True)` keeps it
off the taskbar (Windows only; elsewhere it raises `ValueError`). All of them take effect
live, or at open if set before `App.run()`.

| | Windows | macOS | X11 | Wayland |
|---|---|---|---|---|
| `x`, `y`, `center()` | yes | yes | yes | no: the compositor places windows, and `get` returns `None` |
| `always_on_top` | yes | yes | a hint some window managers ignore | no |
| `resizable` | yes | yes | yes | yes |
| `skip_taskbar` | yes | no | no | no |

Window `parent`/`transient_for`, `modal` and centring on a parent are not provided: winit has
no modal windows on any platform, and a transient parent only on Windows.

Notes: draw straight-alpha colours as always; the renderer premultiplies for the
surface. An app that wants shadows draws them itself (the OS draws none around a
transparent undecorated window on every platform). A transparent window with partial
redraw still redraws exactly: cleared areas become transparent again.

## Timers (0.5.6)

**`window.after(ms, fn) -> TimerHandle`** calls `fn()` once, `ms` milliseconds from now.
**`window.every(ms, fn) -> TimerHandle`** calls it every `ms` milliseconds (at least 1)
until it is cancelled. `handle.cancel()` stops a timer and returns whether it was still
pending; `handle.active` says whether it is.

```python
toast = window.after(4000, dismiss_snackbar)
spinner = window.every(120, next_frame_of_spinner)
...
toast.cancel()
```

- **They run on the loop's thread**, with the window's other callbacks, in the frame, right
  after animations tick. A timer fires on the first frame at or after its time.
- **An idle window sleeps until the next timer.** A window with nothing animating and a
  snackbar's four-second timer redraws about once, when the timer fires, instead of sixty
  times a second to count down.
- **They run on the window's clock**, so `window.advance(ms)` moves them in tests: timers
  due inside the span run in order of time, each at its own moment, so `advance(1000)` with
  an `every(100)` runs ten times.
- **`every` keeps its own beat.** It does not drift by a frame per round. When the loop was
  too busy to run a tick on time, the tick is skipped, not replayed in a burst.
- **A timer set before `App.run()`** counts from when the window opens, with the time it had
  left kept. When the window closes its timers are dropped.
- **An exception in `fn`** is logged like any other callback's, and the timer goes on (an
  `every` keeps repeating; cancel it yourself to stop).
- A callback may cancel other timers, including ones due at the same moment, and may start
  new ones; a timer it starts runs from the next pass.

For a call from another thread, use `LoopHandle.call_soon`, which has no delay.

**`set(show_damage=True)`** makes each presented frame show what it
redrew: its damage rects tinted magenta, or, for a full redraw, the window's
edge outlined in orange. It's for seeing partial redraw work and for finding
redraws an app didn't mean to cause. The tint goes on the image the window
shows, never on the frame Tesserae Engine keeps, so it never changes what later frames
draw. Off by default.
**`set(present_mode=...)`** (0.5.4) chooses how frames are paced to the display.
`"vsync"`, the default, shows one frame per display refresh and lets the loop
wait for the display between frames, so a window with something animating uses a
few percent of a core. `"low_latency"` shows the newest frame at once
(`Mailbox`, where the surface has it, else vsync): a frame is never a refresh
old, but while something animates the loop renders as fast as it can and uses a
whole core. It takes effect at once, even in an open window. A window with
nothing changing costs nothing in either mode.

While a window is being **resized** (0.5.5), it is shown with the low-latency mode
whatever `present_mode` says, and goes back to the app's choice about 0.4 s after the
last resize, with one swapchain rebuild. A resize rebuilds the swapchain every frame,
and on Wayland a vsync present commits with a `wp_fifo_v1` barrier; KDE's compositor
then waited up to half a second before sending the next resize step, so an undecorated
window dragged by its edge trailed the pointer and kept moving after the button was
released. Nothing to set, and `present_mode` still means the steady-state choice.
**`set(dpi_scaling=True)`** (0.5.4) makes the window HiDPI-aware. The app
lays out in *logical* pixels and the engine draws at the display's
`scale_factor`: on a 2x screen a 100x60 box is 200x120 device pixels, and
text, borders and rounded corners are drawn at that resolution, so they stay
sharp instead of being stretched. `width`, `height`, the root's layout box,
pointer positions and pixel scroll deltas are all logical too, and shader
`frame.size` stays in device pixels. Off by default, since a framework that
already multiplies by `scale_factor` itself would be scaled twice; with it
off nothing changes. It takes effect live, and follows the window to a
monitor with a different scale. At a fractional scale such as 1.5, a node's layout
offset is rounded to a whole device pixel so its edges and glyphs sit on the grid.
This applies only to nodes whose ancestors add no scale of their own: under a node
with a paint `scale` (a hover zoom, say) offsets stay exact, so a child moves
smoothly as that scale animates instead of stepping by whole pixels.
**`get(name)`** reads `width`, `height`, `title`, `scale_factor` (`1.0`
until `App.run()` opens the window), `dark` — the OS's current appearance,
or `None` where it can't say ([Window properties](events.md#window-properties))
— `partial_redraw`, `partial_redraw_active`, `glyph_cache`, `show_damage`, or the
[window's controls and state](#window-controls-and-state) above.

**`resize(width, height)`** sets the window's size from code, and the
root's layout box follows. It fires no `resize` event — that reports a
change the user made; `simulate("resize", width=, height=)` stands in for
one.

## `frame_stats`

**`frame_stats(reset=False)`** (0.5.4) says what the window's frames cost, so an app or
a framework can find its own bottlenecks. It is always on and costs a few clock reads
a frame. It returns a dict:

```python
stats = window.frame_stats()
stats["frames"]      # frames drawn since the window opened
stats["skipped"]     # passes that found nothing to draw (the loop slept through them)
stats["last"]        # the last frame, or None
stats["recent"]      # the last 240 frames
```

A frame (`last`, and each `frame` event's `stats`) has, in milliseconds, `tick_ms`
(animations and timers), `layout_ms`, `configure_ms` (0.5.5: rebuilding the swapchain
for a new window size, zero on a frame that did not resize; it used to be counted in
`layout_ms`), `prepare_ms` (working out what changed),
`acquire_ms` (waiting for the swapchain to hand over an image), `draw_ms` (building
the scene, rendering and submitting), `present_ms` and `total_ms`, plus `cpu_ms`
(`total_ms` without `acquire_ms` and `present_ms`: the app's own cost), `redraw`
(`"nothing"` when the kept frame was shown again, `"full"`, or `"partial"`),
`damage_rects` and `damage_area` (the fraction of the window redrawn), `shader_passes`,
`nodes` and `width`/`height`.

`recent` gives `count`, `fps` (frames a second over those frames), `total_ms` and
`cpu_ms` each as `mean`, `p95` and `max`, `stage_ms` (each stage's mean), and
`redraws`, how many of the frames were `nothing`, `full` or `partial`.

```python
r = window.frame_stats()["recent"]
print(f"{r['fps']:.0f} fps, {r['cpu_ms']['p95']:.2f} ms cpu at p95, "
      f"{r['redraws']['partial']} of {r['count']} frames partial")
```

Read the stages with the display in mind: with vsync on (the default, see
`present_mode`), `acquire_ms` is the loop waiting for the next refresh, so a frame's
`total_ms` is near the refresh interval even when `cpu_ms` is a small part of it.
`cpu_ms` is the number to watch against a frame budget. `reset=True` clears the
history after reading it, for measuring one interaction. Nothing is recorded before
`App.run()` opens the window.

For every frame as it happens, `window.on("frame", handler)` (`event.stats` is that
frame's dict); it fires each frame, so keep the handler cheap.

### GPU time

Where the adapter supports timestamp queries (`stats["gpu_timing"]` says), the GPU's time
is measured on one frame in sixteen (timing every frame costs the CPU about 0.3 ms a frame,
as much as a small frame's whole cost): `gpu_ms` on those frames (`None` on the rest), and
`recent["gpu_ms"]` as the mean over the frames that have it. The numbers are read back a few frames after the frame was drawn,
without waiting for the GPU, so the newest frame's `gpu_ms` is `None` until its result
arrives, and a frame that found every slot still in flight goes unmeasured rather than
stall the loop. It times the frame's own submission (the scene's render, the copy to the
screen, a scroll shift); a shader pass or an effect's offscreen render is submitted
separately and is not in it.

### Where the time went

`window.set(profile_nodes=True)` times each node the paint walk reaches, so
`stats["profile"]` can say where the scene-building time of the last frame went:

```python
window.set(profile_nodes=True)
...
p = window.frame_stats()["profile"]
p["ms"], p["reached"]                       # all the nodes' time, and how many were reached
p["by_kind"]["text"]                        # {"reached": 30, "drawn": 4, "ms": 0.42}
for s in p["slowest"]:                      # the ten slowest nodes, slowest first
    print(s["kind"], s["ms"], s["node"])    # the Node itself (None if it has since gone)
```

A node's time is its own (the walk's decision, its layers and its drawing, not its
children's, which are nodes of their own). `drawn` counts those that drew themselves; the
others were reached but fell outside what a partial redraw repaints. It is the CPU work of
encoding the scene, not the GPU's, and costs two clock reads a node, so it is off by
default; `stats["profile"]` is `None` while it is.

### A trace for other tools

`window.start_trace(path)` writes every frame the window draws from then on to `path`
in the Chrome Trace Event Format, until `window.stop_trace()` (which returns how many
frames it holds). Open the file at [ui.perfetto.dev](https://ui.perfetto.dev) or
`chrome://tracing`: each frame is a slice with its stages (tick, layout, prepare,
acquire, draw, present) inside it on the loop's track, and the GPU's time is a slice on a
second track. The file is written as it goes, so a viewer can read one a crash cut short.
`start_trace` raises `OSError` if the file can't be created and `ValueError` if a trace
is already running.

### From another thread

A `Window` belongs to the thread that made it. `window.stats_handle()` (made on that
thread) returns a `StatsHandle` that any thread can hold: `handle.read()` returns the same
dict as `frame_stats()`, except that `profile` is always `None` (it names `Node`s). It
does not wait for the event loop, so it works while the loop is busy.

## `snapshot`

**`snapshot(width=None, height=None, scale=None, time=0.0)`** (0.5.4) returns
what the window draws as `(rgba, width, height)`: straight-alpha RGBA8 bytes,
`width * height * 4` of them, top row first. It renders the window's tree
offscreen, so it works before `App.run()` and on a machine with no display
(it uses a software GPU adapter where there is no GPU), and it leaves the
window as it was.

`width` and `height` are logical pixels and default to the window's own;
`scale` multiplies them into the pixels returned, and defaults to the
window's (`1.0` before it opens, or with `dpi_scaling` off). `time` is the
clock, in seconds, that an animated shader sees as `frame.time`. Animations
are drawn at their current values; they are not advanced. A size or scale
that isn't greater than 0 raises `ValueError`; no GPU, or a size the GPU
can't render, raises `RuntimeError`.

`tre.write_png(path, rgba, width, height)` saves the pixels as a PNG, and
`tre.png_bytes(rgba, width, height)` returns the file's bytes, both with the
standard library alone:

```python
import tre

window = Window(width=320, height=200)
...  # build the tree
tre.write_png("home.png", *window.snapshot())
tre.write_png("home@2x.png", *window.snapshot(scale=2))
```

What it returns is what the window shows: the same renderer draws both, and
a live window and its snapshot are pixel for pixel the same. That makes it
usable for visual regression tests (compare against a stored image) and for
documentation screenshots.

## `measure_text`

**`measure_text(text, font_family="Roboto", font_size=16, font_weight=400,
font_style="normal", letter_spacing=0, line_height=None, max_width=None,
wrap="word", max_lines=None, overflow="clip") -> (width, height)`**

The size `text` takes, laid out exactly as a text node with those properties
paints it: wrapped within `max_width` when given, cut to `max_lines`, ended
with an ellipsis for `overflow="ellipsis"`. The width is the widest shown
line without its trailing whitespace. A text node has no size of its own —
this is how a content-sized widget gets one. See
[Text](../guide/text.md#sizing-text-to-its-content).

## Clipboard

**`read_clipboard() -> str | None`** returns the OS clipboard's text, or
`None` when it holds no text or can't be reached. **`write_clipboard(text) ->
bool`** puts `text` on it, `False` when it can't be reached. Neither raises:
a headless environment may have no clipboard service, which is logged. A
window keeps what it wrote on the clipboard for as long as it runs.

A focused text input handles Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A itself —
copy, cut (which fires `change`), paste (typed input, so `input` fires), and
select all — and never copies from an `obscured` input.

## Testing without a display

**`simulate(event, node=None, **fields)`** delivers a synthetic event through
the same input pipeline as a live one — listeners, focus, text editing, a
focused terminal, the clipboard shortcuts, and docking drags all behave as
they would for a real mouse or keyboard:

```python
window.simulate("click", node=button)
window.simulate("pointer_move", node=card)          # hover
window.simulate("wheel", node=rows, delta_y=60)     # positive scrolls down
window.simulate("key_down", key="tab", shift=True)  # focus backward
window.simulate("input", text="hello")              # typing into the focused input
window.simulate("key_down", key="c", ctrl=True)     # copy its selection
```

Every event and field is in
[Events and Listeners](events.md#testing-without-a-display).

The `maximized` and `active` events fire only when the state changes, and before
`App.run()` `maximize()`, `restore()`, and `minimize()` change it without an
event, so `simulate("maximized", maximized=True)` straight after `maximize()` is
no change and fires nothing. Simulate the report on a fresh window, or leave the
state first ([Custom Title Bars](../guide/custom-title-bars.md#testing-without-a-display)).

**`advance(ms)`** moves this window's time forward by exactly `ms`
milliseconds, then runs animations, their `on_complete` callbacks, and
layout at the new time — deterministic time for tests, where `App.run()`
renders no frames:

```python
window.advance(0)            # pin the clock before starting animations
card.animate("opacity", 0.0, 200)
window.advance(100)
assert card.get("opacity") == 0.5
```

The first call pins the window's clock at the real current time; from then
on only `advance` moves it. Each window keeps its own time, and
`App.run()` returns every window it opens to the real clock.

## Docking

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone |
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it — moving it, if it's docked in another zone |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel |
| `start_panel_drag(panel)` | Starts dragging a docked panel; the drag reports through the `dock_target`/`dock_drop` window events |
| `undock_panel(panel)` | Takes `panel` out of its zone and off the tree; `dock_panel` can dock it again |

`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`.
See [Docking](../guide/docking.md) for a walkthrough.
