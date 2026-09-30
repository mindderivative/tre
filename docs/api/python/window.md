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
`"box"`, `"text"`, `"text_input"`, `"image"`, `"path"`, `"canvas"`,
`"scroll_view"`, `"virtual_list"`, or `"terminal"` — and sets `props`
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
[custom-windowing design](../../design/custom-windowing.md).

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
A maximized or fullscreen window isn't resized to its minimum. Two window
events report state changes: **`maximized`** (`event.maximized`) when the
window is maximized or restored, and **`active`** (`event.active`) when it
gains or loses focus — each only when the value changes.

On Windows an undecorated window keeps its shadow. On macOS, where `winit`
can't let the user resize an undecorated window, `decorations=False` will
keep the native title bar's controls over the content (0.5.0 M4).

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

**Double-click** on the drag region toggles maximize. The two presses must
fall within the system's double-click time (Windows, macOS; 500 ms elsewhere)
and 4 px of each other.

**The window menu.** A secondary press on the drag region opens the OS's
window menu (Restore, Move, Size, Minimize, Maximize, Close) where supported
— Windows, and Wayland compositors that offer one — and ends the press with
`pointer_cancel`, so no `secondary_click` follows. On Windows, Alt+Space on
an undecorated window opens it too. `window.set(system_menu=False)` turns
both off, for a framework that shows its own menu: the secondary press is
then an ordinary one.

**The resize border.** `set(resize_border=N)` makes a press within `N`
logical pixels of an edge resize the window from that edge, or from a corner
where two edges meet. Over the border the pointer shows the matching resize
cursor. A press there never reaches a node, and nor does its release. The
border is off for a decorated window (the OS has one), while maximized or
fullscreen, and at `0`, the default.

| Property | Set | Get |
| --- | --- | --- |
| `resize_border` | Border width in logical pixels, a number `>= 0` | The setting |
| `system_menu` | Whether a secondary press on the drag region (and Alt+Space on Windows) opens the OS's window menu; `True` by default | The setting |

In fullscreen the drag region and the border do nothing: their presses are
ordinary ones, delivered to the nodes as usual. The move, resize, and menu are the OS's and need a real
display; `simulate` exercises everything else — the rule, `pointer_cancel`,
double-click timing (with `advance`), and the border — without one.

## Window events and properties

**`on(event, handler)`** and **`off(event)`** listen to the window itself —
`resize`, `color_scheme`, `scale_factor`, `close_requested`, `closed`,
`dock_target`, `dock_drop`, and (0.5.0) `maximized` and `active`; see
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
**`set(show_damage=True)`** makes each presented frame show what it
redrew: its damage rects tinted magenta, or, for a full redraw, the window's
edge outlined in orange. It's for seeing partial redraw work and for finding
redraws an app didn't mean to cause. The tint goes on the image the window
shows, never on the frame Tesserae Engine keeps, so it never changes what later frames
draw. Off by default.
**`get(name)`** reads `width`, `height`, `title`, `scale_factor` (`1.0`
until `App.run()` opens the window), `dark` — the OS's current appearance,
or `None` where it can't say ([Window properties](events.md#window-properties))
— `partial_redraw`, `partial_redraw_active`, `show_damage`, or the
[window's controls and state](#window-controls-and-state) above.

**`resize(width, height)`** sets the window's size from code, and the
root's layout box follows. It fires no `resize` event — that reports a
change the user made; `simulate("resize", width=, height=)` stands in for
one.

## `measure_text`

**`measure_text(text, font_family="Roboto", font_size=16, font_weight=400,
font_style="normal", letter_spacing=0, line_height=None, max_width=None,
wrap="word", max_lines=None, overflow="clip") -> (width, height)`**

The size `text` takes, laid out exactly as a text node with those properties
paints it: wrapped within `max_width` when given, cut to `max_lines`, ended
with an ellipsis for `overflow="ellipsis"`. The width is the widest shown
line without its trailing whitespace. A text node has no size of its own —
this is how a content-sized widget gets one. See
[Text](../../guide/text.md#sizing-text-to-its-content).

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
See [Docking](../../guide/docking.md) for a walkthrough.
